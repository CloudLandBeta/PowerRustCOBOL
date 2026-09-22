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

/// Where one column's bytes live in a record.
///
/// **Supplied by the host, never read from a delivered `.cidx`** (R30). In a
/// built application the host is the interpreter, which knows the layout from
/// the program's own `FD`. That is the whole reason a tampered definition can
/// misdescribe a file and still not misread it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnLayout {
    pub name: String,
    pub offset: usize,
    pub len: usize,
}

/// Everything needed to *open and read* a file, as opposed to describe it.
///
/// Every field here is layout, so every field comes from the compiled program.
/// Keeping them in a separate struct from [`FileDescription`] is deliberate:
/// the two have different provenance and different trust, and a single struct
/// holding both would make that impossible to see at a call site.
#[derive(Debug, Clone)]
pub struct FileAccess {
    /// The data file, as the program's `ASSIGN TO` names it.
    pub path: std::path::PathBuf,
    pub record_len: usize,
    pub primary: crate::indexed::KeySpec,
    pub columns: Vec<ColumnLayout>,
    pub storage: cobolt_indexed::StorageMode,
}

impl FileAccess {
    /// Build access facts from a definition **at design time** — the IDE's
    /// Grid Browser case, and the case tests construct.
    ///
    /// ⚠️ **Not for a delivered `.cidx`.** Calling this on a file from a
    /// delivery folder would take layout from something the end user can edit,
    /// which is the exact hazard R30 exists to remove. The name says so on
    /// purpose, so a future call site has to argue with it.
    pub fn from_design_time_definition(
        def: &cobolt_indexed::IndexedDefinition,
        path: impl Into<std::path::PathBuf>,
    ) -> Self {
        let record_len = match def.record_format {
            cobolt_indexed::RecordFormatDef::Fixed { length } => length as usize,
            cobolt_indexed::RecordFormatDef::Variable { max_length, .. } => max_length as usize,
        };
        let (primary, _alternates) = crate::indexed_ide::key_specs_from_def(def);
        let columns = def
            .fields
            .iter()
            .flat_map(|f| f.all_leaves())
            .filter_map(|leaf| {
                Some(ColumnLayout {
                    name: leaf.name.clone(),
                    offset: leaf.offset? as usize,
                    len: leaf.length? as usize,
                })
            })
            .collect();
        Self {
            path: path.into(),
            record_len,
            primary,
            columns,
            storage: def.storage,
        }
    }
}

/// One file the application is willing to let a model consult.
#[derive(Debug, Clone)]
pub struct ConsultableFile {
    /// What the file says about itself — from the delivered `.cidx`.
    pub description: FileDescription,
    /// How to open and read it — from the compiled program.
    pub access: FileAccess,
}

/// The tools an application offers, and the gate in front of them.
///
/// # Nothing is consultable by default
///
/// An empty set offers no tools. That is not a degenerate case to tidy away
/// later — it is the safe default (spec 065 R16/R32). The opposite default
/// would mean a half-built application silently publishing every indexed file
/// the developer happened to have, and nobody would notice until it answered a
/// question it should not have.
#[derive(Debug, Clone, Default)]
pub struct IndexedToolSet {
    files: Vec<ConsultableFile>,
}

/// How many records one search may return before it reports truncation (R26).
pub const DEFAULT_SEARCH_LIMIT: usize = 100;

/// A hard ceiling no caller-supplied `limit` may exceed.
pub const MAX_SEARCH_LIMIT: usize = 1_000;

impl IndexedToolSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark one file consultable.
    pub fn allow(&mut self, description: FileDescription, access: FileAccess) {
        self.files.push(ConsultableFile {
            description,
            access,
        });
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The tools to advertise. An unmarked file has none, however well its
    /// description would have matched.
    pub fn tools(&self) -> Vec<cobolt_mcp::Tool> {
        self.files.iter().map(|f| tool_for(&f.description)).collect()
    }

    fn find(&self, tool: &str) -> Option<&ConsultableFile> {
        self.files
            .iter()
            .find(|f| tool_name(&f.description.name) == tool)
    }

    /// Run one tool call.
    ///
    /// Read-only by construction: the file is opened `Input` and only
    /// [`IndexedStore::read_seq`] is used, so there is no code path here that
    /// could write, rewrite or delete (R18).
    pub fn call(&self, tool: &str, arguments: &serde_json::Value) -> cobolt_mcp::ToolResult {
        let Some(file) = self.find(tool) else {
            // Not "no results" — the tool does not exist, or is not consultable.
            return cobolt_mcp::ToolResult::failed(format!(
                "no consultable file is served by the tool '{tool}'"
            ));
        };

        let limit = arguments
            .get("limit")
            .and_then(serde_json::Value::as_u64)
            .map(|n| (n as usize).clamp(1, MAX_SEARCH_LIMIT))
            .unwrap_or(DEFAULT_SEARCH_LIMIT);

        // Every argument that names a column becomes a filter. Matching is
        // textual — see `tool_for` on why a column's type is not available.
        let filters: Vec<(&ColumnLayout, String)> = file
            .access
            .columns
            .iter()
            .filter_map(|col| {
                arguments
                    .get(&col.name)
                    .and_then(serde_json::Value::as_str)
                    .filter(|v| !v.trim().is_empty())
                    .map(|v| (col, v.trim().to_ascii_uppercase()))
            })
            .collect();

        match scan(&file.access, &filters, limit) {
            Err(e) => cobolt_mcp::ToolResult::failed(format!(
                "{} could not be searched: {e}",
                file.description.name
            )),
            Ok(Scan { rows, truncated }) if rows.is_empty() => {
                // Emphatically NOT an error (R19): the search ran and matched
                // nothing, which is an answer. A caller that cannot tell this
                // from a failure will retry a question that was already
                // answered correctly.
                cobolt_mcp::ToolResult::ok(vec![cobolt_mcp::Content::text(format!(
                    "{}: no records matched.",
                    file.description.name
                ))])
            }
            Ok(Scan { rows, truncated }) => {
                let mut out = String::new();
                // The answering file is named, and the records are shown, so an
                // answer can be traced to its source (R17).
                out.push_str(&format!(
                    "{}: {} record(s){}\n",
                    file.description.name,
                    rows.len(),
                    if truncated {
                        format!(" (truncated at {limit}; more exist)")
                    } else {
                        String::new()
                    }
                ));
                for row in &rows {
                    out.push_str(row);
                    out.push('\n');
                }
                cobolt_mcp::ToolResult::ok(vec![cobolt_mcp::Content::text(out)])
            }
        }
    }
}

/// The over-the-wire front door.
///
/// Every method here **delegates**. That is the whole design: [`serve`] and a
/// COBOL `CALL` reach the same [`IndexedToolSet::tools`] and
/// [`IndexedToolSet::call`], so neither door can grow behaviour the other
/// lacks (spec 065 R22). The parity test is what keeps it honest; this impl
/// being boring is what makes the parity test true.
///
/// [`serve`]: cobolt_mcp::serve
impl cobolt_mcp::McpHandler for IndexedToolSet {
    fn server_info(&self) -> cobolt_mcp::ServerInfo {
        cobolt_mcp::ServerInfo {
            name: "PowerRustCOBOL".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }

    fn list_tools(&mut self) -> Vec<cobolt_mcp::Tool> {
        IndexedToolSet::tools(self)
    }

    fn call_tool(&mut self, name: &str, arguments: &serde_json::Value) -> cobolt_mcp::ToolResult {
        IndexedToolSet::call(self, name, arguments)
    }
}

struct Scan {
    rows: Vec<String>,
    truncated: bool,
}

/// Walk the file in key order, keeping records that match every filter.
fn scan(
    access: &FileAccess,
    filters: &[(&ColumnLayout, String)],
    limit: usize,
) -> Result<Scan, String> {
    use crate::indexed::{IndexedStore, OpenMode, ReadDir};

    let mut engine = open_for_reading(access);
    // INPUT, never I-O: the engine is given no opportunity to write (R18).
    let st = engine.open(OpenMode::Input);
    if st != crate::indexed::status::OK {
        return Err(format!("OPEN INPUT failed: FILE STATUS {st}"));
    }

    let mut rows = Vec::new();
    let mut truncated = false;
    loop {
        let (record, st) = engine.read_seq(ReadDir::Next);
        let Some(record) = record else { break };
        if st != crate::indexed::status::OK && st != crate::indexed::status::DUP_ALT_OK {
            break;
        }
        if filters.iter().all(|(col, want)| {
            field_text(&record, col)
                .to_ascii_uppercase()
                .contains(want.as_str())
        }) {
            if rows.len() == limit {
                truncated = true;
                break;
            }
            rows.push(render_row(&record, &access.columns));
        }
    }
    engine.close();
    Ok(Scan { rows, truncated })
}

fn open_for_reading(access: &FileAccess) -> Box<dyn crate::indexed::IndexedStore> {
    use cobolt_indexed::StorageMode;
    match access.storage {
        StorageMode::Memory => {
            let mut f = crate::indexed::IndexedFile::new(
                &access.path,
                access.record_len,
                access.primary.clone(),
                Vec::new(),
            );
            f.set_strict_metadata(false);
            Box::new(f)
        }
        StorageMode::Disk => match crate::indexed_ide::sniff_disk_format(&access.path) {
            crate::indexed_ide::DiskFormat::Redb => {
                let mut f = crate::indexed_redb::RedbIndexedFile::new(
                    &access.path,
                    access.record_len,
                    access.primary.clone(),
                    Vec::new(),
                );
                f.set_strict_metadata(false);
                Box::new(f)
            }
            crate::indexed_ide::DiskFormat::Prcidxd1 => {
                Box::new(crate::indexed_disk::DiskIndexedFile::new(
                    &access.path,
                    access.record_len,
                    access.primary.clone(),
                    Vec::new(),
                ))
            }
        },
    }
}

fn field_text(record: &[u8], col: &ColumnLayout) -> String {
    let end = (col.offset + col.len).min(record.len());
    let start = col.offset.min(end);
    String::from_utf8_lossy(&record[start..end]).trim().to_string()
}

fn render_row(record: &[u8], columns: &[ColumnLayout]) -> String {
    columns
        .iter()
        .map(|c| format!("{}={}", c.name, field_text(record, c)))
        .collect::<Vec<_>>()
        .join("  ")
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

    /// Build a real PRCIDXD1 file matching the `.cidx` fixture's layout:
    /// `ACTOR-ID` at 0..9, `ACTOR-SALARY` at 99..110, 111-byte records.
    fn build_indexed_fixture(path: &std::path::Path, rows: &[(&str, &str)]) {
        use crate::indexed::{IndexedStore, KeySpec, OpenMode, status};
        let primary = KeySpec {
            offset: 0,
            len: 9,
            duplicates: false,
        };
        let mut f = crate::indexed_disk::DiskIndexedFile::new(path, 111, primary, Vec::new());
        assert_eq!(f.open(OpenMode::Output), status::OK, "fixture OPEN OUTPUT");
        for (id, salary) in rows {
            let mut rec = vec![b' '; 111];
            rec[0..9].copy_from_slice(format!("{id:>9}").as_bytes());
            rec[99..110].copy_from_slice(format!("{salary:>11}").as_bytes());
            assert_eq!(f.write(&rec), status::OK, "fixture WRITE {id}");
        }
        f.close();
    }

    fn access_for(path: &std::path::Path) -> FileAccess {
        FileAccess {
            path: path.to_path_buf(),
            record_len: 111,
            primary: crate::indexed::KeySpec {
                offset: 0,
                len: 9,
                duplicates: false,
            },
            columns: vec![
                ColumnLayout {
                    name: "ACTOR-ID".into(),
                    offset: 0,
                    len: 9,
                },
                ColumnLayout {
                    name: "ACTOR-SALARY".into(),
                    offset: 99,
                    len: 11,
                },
            ],
            storage: cobolt_indexed::StorageMode::Disk,
        }
    }

    /// A tool set with one marked file, backed by real records.
    fn one_file_set(dir: &std::path::Path, rows: &[(&str, &str)]) -> IndexedToolSet {
        let cidx = write_fixture(
            dir,
            "actors.cidx",
            &fixture("One row per performer", "Unique performer number"),
        );
        let data = dir.join("actors.idx");
        build_indexed_fixture(&data, rows);

        let mut set = IndexedToolSet::new();
        set.allow(read_description_at(&cidx).unwrap(), access_for(&data));
        set
    }

    /// AC10 — nothing is consultable until it is marked, however good a match
    /// its description would have been.
    #[test]
    fn an_unmarked_file_is_neither_listed_nor_callable() {
        let empty = IndexedToolSet::new();
        assert!(empty.is_empty());
        assert!(empty.tools().is_empty(), "an empty set offers no tools");

        let result = empty.call("search_actors_file", &serde_json::json!({}));
        assert_eq!(
            result.is_error,
            Some(true),
            "calling an unoffered tool is a failure, not an empty answer"
        );
    }

    /// AC11, AC15 — a search names its file and returns its records.
    #[test]
    fn a_search_returns_matching_records_and_names_its_file() {
        let dir = temp("search");
        let set = one_file_set(
            &dir,
            &[("1", "100000"), ("2", "250000"), ("3", "100000")],
        );

        assert_eq!(set.tools().len(), 1);
        let result = set.call(
            "search_actors_file",
            &serde_json::json!({"ACTOR-SALARY": "100000"}),
        );
        assert_eq!(result.is_error, None, "a successful search is not an error");

        let cobolt_mcp::Content::Text { text } = &result.content[0];
        assert!(text.contains("ACTORS-FILE"), "the answering file: {text}");
        assert!(text.contains("2 record(s)"), "two matched: {text}");
        assert!(text.contains("ACTOR-ID=1"), "{text}");
        assert!(text.contains("ACTOR-ID=3"), "{text}");
        assert!(!text.contains("ACTOR-ID=2"), "non-matching row leaked: {text}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC13 — no match is an answer, not a failure. A caller that cannot tell
    /// them apart will retry a question already answered correctly.
    #[test]
    fn a_search_matching_nothing_is_not_an_error() {
        let dir = temp("nomatch");
        let set = one_file_set(&dir, &[("1", "100000")]);

        let empty = set.call(
            "search_actors_file",
            &serde_json::json!({"ACTOR-SALARY": "999999"}),
        );
        assert_eq!(empty.is_error, None, "an empty result is a successful one");
        let cobolt_mcp::Content::Text { text } = &empty.content[0];
        assert!(text.contains("no records matched"), "{text}");

        let broken = set.call("search_nothing_at_all", &serde_json::json!({}));
        assert_eq!(broken.is_error, Some(true), "a real failure is marked");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC16 — the scan is bounded and says so.
    #[test]
    fn a_large_search_is_bounded_and_reports_truncation() {
        let dir = temp("bounded");
        let rows: Vec<(String, String)> = (1..=40)
            .map(|n| (n.to_string(), "100000".to_string()))
            .collect();
        let borrowed: Vec<(&str, &str)> = rows
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        let set = one_file_set(&dir, &borrowed);

        let result = set.call(
            "search_actors_file",
            &serde_json::json!({"ACTOR-SALARY": "100000", "limit": 5}),
        );
        let cobolt_mcp::Content::Text { text } = &result.content[0];
        assert!(text.contains("5 record(s)"), "the cap held: {text}");
        assert!(text.contains("truncated at 5"), "truncation reported: {text}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC12 — searching writes nothing.
    #[test]
    fn a_search_leaves_the_data_file_byte_identical() {
        let dir = temp("readonly");
        let set = one_file_set(&dir, &[("1", "100000"), ("2", "250000")]);
        let data = dir.join("actors.idx");

        let before = std::fs::read(&data).expect("fixture exists");
        let _ = set.call(
            "search_actors_file",
            &serde_json::json!({"ACTOR-SALARY": "100000"}),
        );
        let after = std::fs::read(&data).unwrap();
        assert_eq!(before, after, "a search must not write");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC9, as far as a test can reach it.
    ///
    /// The criterion is "a question naming no file returns records from the
    /// correct file" — and the *choosing* is the model's, so no test here can
    /// assert it. What a test can assert is that choosing is **possible**:
    /// several files each become a distinctly named, distinctly described
    /// tool, and each returns only its own records. If that ever stopped
    /// holding, no model could succeed however good its judgement.
    #[test]
    fn several_files_become_distinctly_described_tools() {
        let dir = temp("distinct");

        let actors_cidx = write_fixture(
            &dir,
            "actors.cidx",
            &fixture("One row per performer under contract", "Performer number"),
        );
        let actors_data = dir.join("actors.idx");
        build_indexed_fixture(&actors_data, &[("1", "100000")]);

        // A second file, described for a different question entirely.
        let staff_cidx = write_fixture(
            &dir,
            "staff.cidx",
            &fixture("One row per employment record", "Employee number")
                .replace("ACTORS-FILE", "STAFF-FILE"),
        );
        let staff_data = dir.join("staff.idx");
        build_indexed_fixture(&staff_data, &[("9", "555000")]);

        let mut set = IndexedToolSet::new();
        set.allow(read_description_at(&actors_cidx).unwrap(), access_for(&actors_data));
        set.allow(read_description_at(&staff_cidx).unwrap(), access_for(&staff_data));

        let tools = set.tools();
        assert_eq!(tools.len(), 2);
        assert_ne!(tools[0].name, tools[1].name, "tools are distinguishable");
        assert_ne!(
            tools[0].description, tools[1].description,
            "and so are their descriptions — this is what a model chooses on"
        );
        assert!(tools
            .iter()
            .any(|t| t.description.as_deref() == Some("One row per employment record")));

        // Each answers from its own file and no other.
        let from_staff = set.call("search_staff_file", &serde_json::json!({"ACTOR-ID": "9"}));
        let cobolt_mcp::Content::Text { text } = &from_staff.content[0];
        assert!(text.contains("STAFF-FILE"), "{text}");
        assert!(text.contains("ACTOR-ID=9"), "{text}");
        assert!(!text.contains("ACTOR-ID=1"), "files must not bleed: {text}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC15 — a `STORAGE IS MEMORY` file is searchable, *when it persists*.
    ///
    /// Worth stating precisely, because the obvious reading of R24 is wrong.
    /// A MEMORY file's records live in the RAM of whoever opened it, and the
    /// tool opens its own handle. So there is something to search only when
    /// the file was declared `WITH PERSISTENCE` and a container exists on
    /// disk. Without it the tool is not failing — there is genuinely nothing
    /// there for a second reader.
    #[test]
    fn a_persisted_memory_file_is_searchable() {
        use crate::indexed::{IndexedStore, KeySpec, OpenMode, status};

        let dir = temp("memory");
        let data = dir.join("actors.mem");
        let primary = KeySpec {
            offset: 0,
            len: 9,
            duplicates: false,
        };

        let mut f = crate::indexed::IndexedFile::new(&data, 111, primary.clone(), Vec::new());
        f.set_persist(true);
        assert_eq!(f.open(OpenMode::Output), status::OK);
        let mut rec = vec![b' '; 111];
        rec[0..9].copy_from_slice(b"        7");
        rec[99..110].copy_from_slice(b"     777000");
        assert_eq!(f.write(&rec), status::OK);
        f.close();

        let cidx = write_fixture(
            &dir,
            "actors.cidx",
            &fixture("Performers held in memory", "Unique performer number"),
        );
        let mut acc = access_for(&data);
        acc.storage = cobolt_indexed::StorageMode::Memory;

        let mut set = IndexedToolSet::new();
        set.allow(read_description_at(&cidx).unwrap(), acc);

        let result = set.call("search_actors_file", &serde_json::json!({"ACTOR-ID": "7"}));
        assert_eq!(result.is_error, None, "the memory engine opened: {result:?}");
        let cobolt_mcp::Content::Text { text } = &result.content[0];
        assert!(
            text.contains("ACTOR-ID=7"),
            "a persisted MEMORY file is searchable: {text}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC21 (second half) — one unreadable definition costs one tool, and the
    /// others keep serving.
    #[test]
    fn a_broken_definition_costs_only_its_own_tool() {
        let dir = temp("survives");
        let good = one_file_set(&dir, &[("1", "100000")]);
        // The broken one is simply never allowed — `read_description_at`
        // returned None, so there is nothing to add.
        let broken = write_fixture(&dir, "broken.cidx", "not a definition at all");
        assert_eq!(read_description_at(&broken), None);

        assert_eq!(good.tools().len(), 1, "the healthy file still serves");
        let result = good.call("search_actors_file", &serde_json::json!({"ACTOR-ID": "1"}));
        assert_eq!(result.is_error, None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC14 — the parity guard. The same question, asked in process and asked
    /// over the wire, must give the same answer.
    ///
    /// This is the test that stops R22 rotting. Two front doors onto one
    /// definition is only true while something checks it, and the moment one
    /// door grows a special case the other does not have, this fails.
    #[test]
    fn the_two_front_doors_agree() {
        use cobolt_mcp::McpHandler;

        let dir = temp("parity");
        let mut set = one_file_set(
            &dir,
            &[("1", "100000"), ("2", "250000"), ("3", "100000")],
        );
        let args = serde_json::json!({"ACTOR-SALARY": "100000"});

        // Door one: in process, no serialization, no transport.
        let in_process = IndexedToolSet::call(&set, "search_actors_file", &args);

        // Door two: through the real server loop, as a foreign client sees it.
        let request = serde_json::json!({
            "jsonrpc":"2.0","id":1,"method":"tools/call",
            "params":{"name":"search_actors_file","arguments":args}
        });
        let mut input =
            io_cursor(serde_json::to_string(&request).unwrap() + "\n");
        let mut output = Vec::new();
        cobolt_mcp::serve(&mut input, &mut output, &mut set).expect("clean stream");

        let line = String::from_utf8(output).unwrap();
        let response: cobolt_mcp::Response =
            serde_json::from_str(line.trim()).expect("a JSON-RPC response");
        let over_the_wire: cobolt_mcp::ToolResult =
            serde_json::from_value(response.result.expect("a result")).unwrap();

        assert_eq!(
            in_process, over_the_wire,
            "the two doors disagreed — R22 is broken"
        );

        // And the tool lists agree too.
        let listed_in_process = IndexedToolSet::tools(&set);
        let listed_over_trait = set.list_tools();
        assert_eq!(listed_in_process, listed_over_trait);

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn io_cursor(s: String) -> std::io::Cursor<Vec<u8>> {
        std::io::Cursor::new(s.into_bytes())
    }

    /// AC3 (second half) — a client completes the handshake against the real
    /// tool set and receives a well-formed result.
    #[test]
    fn a_client_handshakes_lists_and_calls_against_the_real_tool_set() {
        let dir = temp("e2e");
        let mut set = one_file_set(&dir, &[("1", "100000")]);

        let script = [
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize",
                               "params":{"protocolVersion":"2025-06-18"}}),
            serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
            serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call",
                               "params":{"name":"search_actors_file",
                                         "arguments":{"ACTOR-ID":"1"}}}),
        ];
        let mut input = io_cursor(
            script
                .iter()
                .map(|r| serde_json::to_string(r).unwrap())
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        );
        let mut output = Vec::new();
        cobolt_mcp::serve(&mut input, &mut output, &mut set).unwrap();

        let replies: Vec<cobolt_mcp::Response> = String::from_utf8(output)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(replies.len(), 3);
        assert_eq!(
            replies[0].result.as_ref().unwrap()["serverInfo"]["name"],
            "PowerRustCOBOL"
        );
        let tools = replies[1].result.as_ref().unwrap()["tools"]
            .as_array()
            .unwrap();
        assert_eq!(tools[0]["name"], "search_actors_file");
        assert!(
            replies[2].result.as_ref().unwrap()["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("ACTOR-ID=1"),
            "the call returned the record"
        );

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
