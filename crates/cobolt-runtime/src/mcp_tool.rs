// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The indexed-file tool an application exposes over MCP (spec 065).
//!
//! `cobolt-mcp` owns the protocol and knows nothing about COBOL. This module
//! owns the COBOL and knows nothing about transports. It lives here because
//! this is the crate that already has `cobolt-indexed` and `IndexedStore`.
//!
//! # The descriptions are the mechanism
//!
//! A user asks "how many contractors started in Q3" and names no file. The
//! model chooses, and it can only choose from what each tool says about itself
//! — the file's purpose and its columns' descriptions, exactly as the developer
//! wrote them in the Indexed File Editor. That is why [`FileDescription`] is
//! the centre of this module rather than a detail of it: without good
//! descriptions the tool is a keyword search with extra steps.
//!
//! # Descriptive fields only — and why that is a safety property
//!
//! A `.cidx` says two different things: what a file *means*, and how its
//! records are *laid out*. In a delivery the file sits beside the executable,
//! where whoever runs the application can edit it. So this module reads only
//! the first kind (spec 065 R30). Offsets, lengths, keys, record format and
//! storage mode are never taken from disk — those come from the compiled
//! program, which is not editable in the field.
//!
//! The consequence is worth stating plainly: a tampered or stale `.cidx` can
//! give a file a **wrong description**, which is visible and recoverable. It
//! can never move an offset, change a key, or make a record read as something
//! it is not. The layout is already compiled in; reading it twice would buy
//! nothing and risk everything.

use std::path::Path;

/// One column, as the model is told about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDescription {
    /// The COBOL data-item name, which stays English in every UI language.
    pub name: String,
    /// What the developer said this column means. May be empty.
    pub description: String,
}

/// What an indexed file says about itself.
///
/// Deliberately narrow: this struct is the *entire* surface the rest of the
/// tool sees of a delivered `.cidx`. It carries no offset, length, key or
/// storage mode, so a later change cannot casually start trusting one —
/// the type system is doing the remembering, not a comment (R30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDescription {
    /// The file's COBOL name, e.g. `ACTORS-FILE`.
    pub name: String,
    /// What the file is for. May be empty, which is a description problem, not
    /// an error.
    pub purpose: String,
    /// Its columns, in record order.
    pub columns: Vec<ColumnDescription>,
}

impl FileDescription {
    /// Does this file say enough about itself for a model to choose it?
    ///
    /// Not enforced anywhere — a file with no purpose is still listed, because
    /// hiding it would be a silent and confusing failure. It is here so a
    /// caller can *tell the developer* their descriptions are the reason the
    /// assistant keeps picking the wrong file.
    pub fn is_describable(&self) -> bool {
        !self.purpose.trim().is_empty()
    }
}

/// Read the descriptive metadata of one `.cidx`, or `None` if it cannot be read.
///
/// `stored` is the path exactly as a form or the project recorded it —
/// project-relative, like `indexed/actors.cidx`. It is anchored through
/// [`cobolt_forms::assets::resolve`], the same helper the DataGrid binding
/// already uses, so a delivered application resolves it against the folder it
/// anchors on rather than against whatever directory it was launched from.
///
/// A missing or malformed definition yields `None` and a warning (R31). It is
/// never fatal: one unreadable file loses one tool, and the rest keep serving.
pub fn read_description(stored: &str) -> Option<FileDescription> {
    let path = cobolt_forms::assets::resolve(stored);
    read_description_at(&path)
}

/// [`read_description`] against an already-resolved path.
///
/// Split out so tests can point at a fixture without touching the process-wide
/// asset anchor, which other tests in the same binary also rely on.
pub fn read_description_at(path: &Path) -> Option<FileDescription> {
    let def = match cobolt_indexed::load_indexed(path) {
        Ok(def) => def,
        Err(e) => {
            tracing::warn!(
                target: "mcp",
                "indexed definition '{}' cannot be read ({e:?}); its tool is not offered",
                path.display()
            );
            return None;
        }
    };

    // Only the descriptive fields cross this line. See the module note on R30.
    let columns = def
        .fields
        .iter()
        .flat_map(|field| field.all_leaves())
        .map(|leaf| ColumnDescription {
            name: leaf.name.clone(),
            description: leaf.comment.trim().to_string(),
        })
        .collect();

    let described = FileDescription {
        name: def.name.clone(),
        purpose: def.comment.trim().to_string(),
        columns,
    };

    // `load_indexed` never fails on CONTENT — only on I/O. Garbage parses to a
    // default definition named "UNNAMED" with no fields, and a truncated
    // document to an empty one. Neither is an error to that parser, and neither
    // is a tool: there is nothing to search and nothing to tell a model about.
    //
    // Filtering on "no columns" rather than on the parser's placeholder name
    // keeps this a statement about meaning instead of a guess about internals,
    // so a future parser default cannot quietly reopen the hole.
    if described.columns.is_empty() {
        tracing::warn!(
            target: "mcp",
            "indexed definition '{}' describes no columns; its tool is not offered",
            path.display()
        );
        return None;
    }

    Some(described)
}

/// The MCP tool name for a file, e.g. `ACTORS-FILE` → `search_actors_file`.
///
/// Stable and derived, never stored: a tool name that lived in a second place
/// could disagree with the file it names.
pub fn tool_name(file_name: &str) -> String {
    let slug: String = file_name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("search_{}", slug.trim_matches('_'))
}

/// Build the MCP tool that searches one described file.
///
/// The file's purpose becomes the tool's description and each column's
/// description becomes its parameter's — which is the entire selection
/// mechanism. A model picking between tools sees nothing else.
///
/// # Every parameter is a string, and that follows from R30
///
/// A column's COBOL `PIC` would say whether it holds digits or text, and a
/// richer schema could declare that. But `PIC` is layout, and layout is not
/// read from a delivered `.cidx` — so the schema cannot know it. Matching is
/// textual as a result, which costs little here and keeps the safety property
/// whole: a tampered definition still cannot change how a record is read.
pub fn tool_for(description: &FileDescription) -> cobolt_mcp::Tool {
    let mut properties = serde_json::Map::new();
    for column in &description.columns {
        let mut spec = serde_json::Map::new();
        spec.insert("type".into(), serde_json::Value::String("string".into()));
        if !column.description.is_empty() {
            spec.insert(
                "description".into(),
                serde_json::Value::String(column.description.clone()),
            );
        }
        properties.insert(column.name.clone(), serde_json::Value::Object(spec));
    }

    // A bound the caller may lower but not raise past the engine's own cap
    // (R26). Declared here so a model knows truncation is possible.
    properties.insert(
        "limit".into(),
        serde_json::json!({
            "type": "integer",
            "description": "Maximum records to return. Results may be truncated.",
        }),
    );

    let purpose = if description.purpose.is_empty() {
        // An undescribed file is still offered — hiding it would be a silent
        // failure the developer could not diagnose — but it says so, because
        // that sentence is what a model will otherwise have to guess at.
        format!(
            "Search {} (no description was written for this file).",
            description.name
        )
    } else {
        description.purpose.clone()
    };

    cobolt_mcp::Tool {
        name: tool_name(&description.name),
        description: Some(purpose),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": serde_json::Value::Object(properties),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `.cidx` shaped like the real one in `examples/PowerDemo3`, with the
    /// descriptions a developer would actually write.
    fn fixture(purpose: &str, id_note: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><IndexedFile name="ACTORS-FILE" finalized="true" version="1.0"><assign-path>data/idxfiles/actors.idx</assign-path><access-mode>dynamic</access-mode><record-format fixed-length="111"/><storage mode="disk" compression="false" persistence="false"/><comment><![CDATA[{purpose}]]></comment><keys><primary duplicates="false" ordering="ascending"><part field="ACTOR-ID" offset="0" length="9" encoding="bytes"/></primary></keys><fields><Field level="1" name="ACTORS-RECORD" usage="display"><Field level="5" name="ACTOR-ID" pic="9(9)" usage="display" offset="0" length="9"><comment><![CDATA[{id_note}]]></comment></Field><Field level="5" name="ACTOR-SALARY" pic="9(9)V99" usage="display" offset="99" length="11"><comment><![CDATA[Annual salary in whole currency units]]></comment></Field></Field></fields></IndexedFile>"#
        )
    }

    fn write_fixture(dir: &std::path::Path, name: &str, xml: &str) -> std::path::PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, xml).unwrap();
        p
    }

    fn temp(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let d = std::env::temp_dir().join(format!("prc-mcp-{tag}-{nanos}"));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// AC6 — what the running program reads equals what the `.cidx` holds.
    #[test]
    fn the_purpose_and_column_descriptions_are_what_the_cidx_holds() {
        let dir = temp("read");
        let p = write_fixture(
            &dir,
            "actors.cidx",
            &fixture(
                "One row per performer under contract",
                "Unique performer number",
            ),
        );

        let got = read_description_at(&p).expect("a well-formed definition reads");
        assert_eq!(got.name, "ACTORS-FILE");
        assert_eq!(got.purpose, "One row per performer under contract");
        assert_eq!(got.columns.len(), 2, "both leaves, in record order");
        assert_eq!(got.columns[0].name, "ACTOR-ID");
        assert_eq!(got.columns[0].description, "Unique performer number");
        assert_eq!(got.columns[1].name, "ACTOR-SALARY");
        assert_eq!(
            got.columns[1].description,
            "Annual salary in whole currency units"
        );
        assert!(got.is_describable());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC21 (first half) — an unreadable definition loses its tool, not the
    /// process.
    ///
    /// Note what this does **not** claim. `load_indexed` is deliberately
    /// lenient: `<IndexedFile name="X"><fields>` parses to an empty definition
    /// rather than an error, so truncation alone does not produce `None`. That
    /// is the parser's choice and not ours to override here — a definition that
    /// parses to nothing simply describes nothing, and is filtered later by
    /// describability rather than rejected now. Only content that is not a
    /// definition at all fails to read.
    #[test]
    fn an_unreadable_definition_yields_none_rather_than_panicking() {
        let dir = temp("unreadable");

        let missing = dir.join("nothing-here.cidx");
        assert_eq!(read_description_at(&missing), None, "absent is not fatal");

        let garbage = write_fixture(&dir, "garbage.cidx", "\u{0}\u{1}not a document at all");
        assert_eq!(read_description_at(&garbage), None, "unparseable is not fatal");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The parser's leniency, pinned at the boundary that matters.
    ///
    /// `load_indexed` accepts a truncated document and returns an empty
    /// definition — no error. That is its choice, and this test does not argue
    /// with it; it pins that **we** do not turn such a thing into a tool. A
    /// future parser change that starts rejecting partial documents outright
    /// leaves this test passing, which is the point.
    #[test]
    fn a_definition_describing_no_columns_is_not_offered() {
        let dir = temp("truncated");
        let p = write_fixture(&dir, "partial.cidx", "<IndexedFile name=\"X\"><fields>");
        assert_eq!(
            read_description_at(&p),
            None,
            "a definition with nothing to search is not a tool"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// R30, at the type level: the descriptive struct has nowhere to put a
    /// layout fact, so no later change can quietly start trusting one.
    #[test]
    fn a_description_carries_no_layout() {
        let dir = temp("no-layout");
        let p = write_fixture(&dir, "actors.cidx", &fixture("Performers", "The key"));
        let got = read_description_at(&p).unwrap();

        // The fixture declares offset 99 and length 11 for ACTOR-SALARY, and a
        // 111-character record. None of it is reachable from what we return.
        let rendered = format!("{got:?}");
        for layout in ["99", "111", "offset", "length", "9(9)V99"] {
            assert!(
                !rendered.contains(layout),
                "a layout fact leaked into the description: {layout} in {rendered}"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC8 — the schema carries the file's purpose and each column's text.
    #[test]
    fn the_schema_is_generated_from_the_developers_descriptions() {
        let dir = temp("schema");
        let p = write_fixture(
            &dir,
            "actors.cidx",
            &fixture(
                "One row per performer under contract",
                "Unique performer number",
            ),
        );
        let desc = read_description_at(&p).unwrap();
        let tool = tool_for(&desc);

        assert_eq!(tool.name, "search_actors_file");
        assert_eq!(
            tool.description.as_deref(),
            Some("One row per performer under contract"),
            "the file's purpose IS the tool description"
        );

        let props = &tool.input_schema["properties"];
        assert_eq!(
            props["ACTOR-ID"]["description"], "Unique performer number",
            "a column's description lands on its parameter"
        );
        assert_eq!(
            props["ACTOR-SALARY"]["description"],
            "Annual salary in whole currency units"
        );
        assert!(props["limit"].is_object(), "truncation is declared (R26)");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC7 — editing the description changes the schema, and nothing else is
    /// touched. The `.cidx` is the only author (R14).
    #[test]
    fn editing_a_description_changes_the_schema_with_no_second_place_to_edit() {
        let dir = temp("reedit");
        let p = write_fixture(&dir, "actors.cidx", &fixture("Before", "Key before"));
        let before = tool_for(&read_description_at(&p).unwrap());

        // Only the .cidx changes.
        std::fs::write(&p, fixture("After the rewrite", "Key after")).unwrap();
        let after = tool_for(&read_description_at(&p).unwrap());

        assert_eq!(before.description.as_deref(), Some("Before"));
        assert_eq!(after.description.as_deref(), Some("After the rewrite"));
        assert_eq!(
            after.input_schema["properties"]["ACTOR-ID"]["description"],
            "Key after"
        );
        assert_eq!(before.name, after.name, "the tool's identity is stable");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// R30 again, at the schema boundary: no parameter may declare a COBOL
    /// type, because the type would have had to come from `PIC`.
    #[test]
    fn no_parameter_declares_a_layout_derived_type() {
        let dir = temp("schema-layout");
        let p = write_fixture(&dir, "actors.cidx", &fixture("Performers", "The key"));
        let tool = tool_for(&read_description_at(&p).unwrap());
        let rendered = serde_json::to_string(&tool.input_schema).unwrap();

        assert!(
            !rendered.contains("9(9)") && !rendered.contains("111"),
            "a layout fact reached the schema: {rendered}"
        );
        assert_eq!(
            tool.input_schema["properties"]["ACTOR-SALARY"]["type"], "string",
            "textual matching is the price of not reading PIC"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An undescribed file is still offered, and says so — hiding it would be
    /// a silent failure the developer could not diagnose.
    #[test]
    fn an_undescribed_file_is_offered_but_admits_it() {
        let dir = temp("undescribed-tool");
        let p = write_fixture(&dir, "actors.cidx", &fixture("", ""));
        let tool = tool_for(&read_description_at(&p).unwrap());
        assert!(
            tool.description.as_deref().unwrap().contains("no description"),
            "the model is told the description is missing: {:?}",
            tool.description
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An empty purpose is a description problem, not an error — the file is
    /// still read, so the caller can tell the developer why retrieval is poor.
    #[test]
    fn an_undescribed_file_still_reads_but_reports_itself_as_thin() {
        let dir = temp("thin");
        let p = write_fixture(&dir, "actors.cidx", &fixture("", ""));
        let got = read_description_at(&p).expect("still readable");
        assert_eq!(got.purpose, "");
        assert!(!got.is_describable(), "the caller can warn about this");
        assert_eq!(got.columns.len(), 2, "columns are still listed");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
