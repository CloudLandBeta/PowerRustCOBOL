// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `kb_lookup` — the reference this binary carries, one entry at a time.
//!
//! The text is `cobolt_compiler::system_documentation()` — the same generated
//! documents the kit's reference pack is made of — minus the two that describe
//! the IDE's own agent mesh, which an external agent cannot use. Built-ins come
//! from `cobolt_runtime::builtins::BUILTINS`. A name found in neither does not
//! exist for the agent (spec 080 R7, R19).

use std::sync::OnceLock;

use serde_json::{json, Value};

/// The System KB documents an external agent may use (the reference pack's
/// selection, plan §1.7).
pub const EXCLUDED_DOCUMENTS: [&str; 2] = ["agents_registry.md", "ide_functionalities.md"];

/// Largest answer, in characters — a whole control section can be long.
pub const MAX_ANSWER_CHARS: usize = 12_000;
/// Most matching entries listed for a property, method or event.
pub const MAX_HITS: usize = 25;

fn documents() -> &'static [(&'static str, String)] {
    static DOCS: OnceLock<Vec<(&'static str, String)>> = OnceLock::new();
    DOCS.get_or_init(|| {
        cobolt_compiler::system_documentation()
            .into_iter()
            .filter(|(n, _)| !EXCLUDED_DOCUMENTS.contains(n))
            .collect()
    })
}

pub fn run(name: &str, kind: Option<&str>) -> Result<Value, String> {
    let name = name.trim().trim_matches('"').trim_matches('`');
    if name.is_empty() {
        return Err("kb_lookup needs a 'name'".to_owned());
    }
    let kind = kind.map(|k| k.trim().to_ascii_lowercase());
    let wants = |k: &str| kind.as_deref().is_none_or(|w| w == k);

    if wants("control") {
        if let Some(section) = control_section(name) {
            return Ok(json!({ "found": true, "kind": "control", "name": name, "text": cap(&section) }));
        }
    }
    if wants("builtin") {
        if let Some(b) = cobolt_runtime::builtins::builtin(name) {
            return Ok(json!({
                "found": true, "kind": "builtin", "name": b.name,
                "text": format!(
                    "COBOL::\"{}\" ( {} ) — {}. Written inline, as a statement; never CALL \"COBOL-{}\".",
                    b.name, if b.params.is_empty() { "—" } else { b.params }, b.description, b.name
                ),
            }));
        }
    }
    let hits = entry_hits(name, kind.as_deref());
    if !hits.is_empty() {
        let shown: Vec<Value> = hits
            .iter()
            .take(MAX_HITS)
            .map(|(doc, heading, line)| json!({ "document": doc, "section": heading, "entry": line }))
            .collect();
        return Ok(json!({
            "found": true, "kind": kind.unwrap_or_else(|| "entry".into()), "name": name,
            "matches": hits.len(), "entries": shown,
        }));
    }
    Ok(json!({
        "found": false, "name": name,
        "message": format!(
            "'{name}' is not in the PowerRustCOBOL reference. It does not exist: do not invent it — \
             write a gap report in docs/compiler-requests/ instead."
        ),
    }))
}

/// The `## Control: <name>` section, up to the next `## ` heading.
fn control_section(name: &str) -> Option<String> {
    for (_, text) in documents() {
        let mut out: Option<String> = None;
        for line in text.lines() {
            if let Some(found) = out.as_mut() {
                if line.starts_with("## ") {
                    return Some(std::mem::take(found));
                }
                found.push_str(line);
                found.push('\n');
            } else if let Some(n) = line.strip_prefix("## Control: ") {
                if n.trim().eq_ignore_ascii_case(name) {
                    out = Some(format!("{line}\n"));
                }
            }
        }
        if out.is_some() {
            return out;
        }
    }
    None
}

/// Every bullet entry ``- `<name>` `` or ``- `<name>(…)` ``, with the document
/// and the heading it sits under — narrowed by `kind` when given.
fn entry_hits(name: &str, kind: Option<&str>) -> Vec<(String, String, String)> {
    let lower = name.to_ascii_lowercase();
    let mut hits = Vec::new();
    for (doc, text) in documents() {
        let mut h2 = String::new();
        let mut h3 = String::new();
        for line in text.lines() {
            if let Some(h) = line.strip_prefix("## ") {
                h2 = h.trim().to_owned();
                h3.clear();
                continue;
            }
            if let Some(h) = line.strip_prefix("### ") {
                h3 = h.trim().to_owned();
                continue;
            }
            let Some(rest) = line.trim_start().strip_prefix("- `") else {
                continue;
            };
            let rest_lower = rest.to_ascii_lowercase();
            let Some(after) = rest_lower.strip_prefix(&lower) else {
                continue;
            };
            let is_call = after.starts_with('(');
            if !(after.starts_with('`') || is_call) {
                continue;
            }
            let heading = if h3.is_empty() { h2.clone() } else { format!("{h2} ▸ {h3}") };
            let hl = heading.to_ascii_lowercase();
            let keep = match kind {
                Some("method") => is_call || hl.contains("method"),
                Some("event") => hl.contains("event"),
                Some("property") => !is_call && hl.contains("propert"),
                _ => true,
            };
            if keep {
                hits.push((doc.to_string(), heading, line.trim().chars().take(600).collect()));
            }
        }
    }
    hits
}

/// Most results `kb_search` returns, and its default.
pub const MAX_SEARCH_HITS: usize = 25;
pub const DEFAULT_SEARCH_HITS: usize = 8;

/// `kb_search` (spec 084 R12): free-text search of the System Knowledge Base.
///
/// It reads the store the IDE's assistant searches
/// (`~/PowerRustCOBOL/data/chunked.data`, through the shared reader in
/// `cobolt_kb::system_store`), scored lexically so it needs no model. Where
/// that store does not exist yet — `rcrun` on a machine the IDE has not run
/// on — it ranks the same System KB sections this binary carries.
pub fn search(query: &str, limit: Option<u64>) -> Result<Value, String> {
    search_in(&cobolt_kb::system_store::ide_store_path(), query, limit)
}

pub(crate) fn search_in(store: &std::path::Path, query: &str, limit: Option<u64>) -> Result<Value, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("kb_search needs a 'query'".to_owned());
    }
    let limit = limit.map_or(DEFAULT_SEARCH_HITS, |n| (n as usize).clamp(1, MAX_SEARCH_HITS));
    let excluded = |path: &str| EXCLUDED_DOCUMENTS.iter().any(|d| path.ends_with(d));
    let stored = cobolt_kb::system_store::search_lexical(store, query, limit + EXCLUDED_DOCUMENTS.len() * 4)
        .unwrap_or_default();
    let (source, hits): (&str, Vec<Value>) = if stored.is_empty() {
        ("built-in", built_in_sections(query, limit))
    } else {
        (
            "ide-store",
            stored
                .into_iter()
                .filter(|h| !excluded(&h.source_path))
                .take(limit)
                .map(|h| json!({
                    "subject": h.subject, "kind": h.kind, "document": h.source_path,
                    "score": (h.score * 1000.0).round() / 1000.0, "text": cap(&h.content),
                }))
                .collect(),
        )
    };
    Ok(json!({ "query": query, "source": source, "found": !hits.is_empty(), "hits": hits }))
}

/// Rank the built-in System KB by `##`/`###` section, with the same hashing
/// scorer the store uses under the hashing embedder.
fn built_in_sections(query: &str, limit: usize) -> Vec<Value> {
    use cobolt_kb::embed::HashingEmbedder;
    let q = HashingEmbedder::vector(query);
    let mut scored: Vec<(f32, &str, String, String)> = Vec::new();
    for (doc, text) in documents() {
        let mut heading = String::from(*doc);
        let mut body = String::new();
        let mut flush = |heading: &str, body: &mut String, scored: &mut Vec<(f32, &str, String, String)>| {
            if !body.trim().is_empty() {
                let v = HashingEmbedder::vector(&format!("{heading}\n{body}"));
                let score: f32 = q.iter().zip(&v).map(|(a, b)| a * b).sum();
                if score > 0.0 {
                    scored.push((score, doc, heading.to_owned(), std::mem::take(body)));
                }
            }
            body.clear();
        };
        for line in text.lines() {
            if let Some(h) = line.strip_prefix("### ").or_else(|| line.strip_prefix("## ")) {
                flush(&heading, &mut body, &mut scored);
                heading = h.trim().to_owned();
            } else {
                body.push_str(line);
                body.push('\n');
            }
        }
        flush(&heading, &mut body, &mut scored);
    }
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored
        .into_iter()
        .take(limit)
        .map(|(score, doc, subject, body)| json!({
            "subject": subject, "kind": "section", "document": format!("Knowledge Base/{doc}"),
            "score": (score * 1000.0).round() / 1000.0, "text": cap(body.trim()),
        }))
        .collect()
}

fn cap(s: &str) -> String {
    if s.chars().count() <= MAX_ANSWER_CHARS {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(MAX_ANSWER_CHARS).collect();
    out.push_str("\n… (truncated; the full section is in the resource powerrustcobol://reference/controls.md)");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 084 AC6: free text finds the subjects that name it — from the
    /// built-in sections when no store exists — and refuses an empty query.
    #[test]
    fn kb_search_finds_sidemenu_and_contentpane() {
        let missing = std::env::temp_dir().join("prc-084-no-store.data");
        let _ = std::fs::remove_file(&missing);
        let v = search_in(&missing, "SideMenu ContentPane", Some(5)).unwrap();
        assert_eq!(v["source"], "built-in");
        let hits = v["hits"].as_array().unwrap();
        assert!(!hits.is_empty() && hits.len() <= 5);
        let text = hits.iter().map(|h| format!("{} {}", h["subject"], h["text"])).collect::<String>();
        assert!(text.contains("SideMenu") && text.contains("ContentPane"), "both named in the hits");
        assert!(hits.iter().all(|h| !EXCLUDED_DOCUMENTS.iter().any(|d| h["document"].as_str().unwrap().ends_with(d))));
        assert!(search_in(&missing, "  ", None).is_err());
        let capped = search_in(&missing, "Button", Some(999)).unwrap();
        assert!(capped["hits"].as_array().unwrap().len() <= MAX_SEARCH_HITS);
        println!("kb_search (built-in): 'SideMenu ContentPane' -> {} hits naming both; empty query refused; limit capped at {MAX_SEARCH_HITS}", hits.len());
    }

    /// AC6 against the store the IDE's assistant searches, where this machine
    /// has one (the IDE installs it on first run); skipped, and said so, where
    /// it does not.
    #[test]
    fn kb_search_reads_the_ides_store_when_installed() {
        let store = cobolt_kb::system_store::ide_store_path();
        if !store.exists() {
            println!("kb_search (ide-store): no store at {} on this machine — skipped", store.display());
            return;
        }
        let v = search_in(&store, "SideMenu ContentPane", Some(8)).unwrap();
        assert_eq!(v["source"], "ide-store");
        let hits = v["hits"].as_array().unwrap();
        let text = hits.iter().map(|h| format!("{} {}", h["subject"], h["text"])).collect::<String>();
        assert!(text.contains("SideMenu") && text.contains("ContentPane"), "{v}");
        println!("kb_search (ide-store): 'SideMenu ContentPane' -> {} subjects from the assistant's store", hits.len());
    }

    #[test]
    fn kb_finds_real_names_and_refuses_invented_ones() {
        let mut table = Vec::new();
        for (name, kind) in [
            ("Button", None),
            ("Caption", Some("property")),
            ("onClick", None),
            ("AddRow", Some("method")),
            ("HTTP-GET", None),
            ("COBOL-HTTP-GET", Some("builtin")),
        ] {
            let v = run(name, kind).unwrap();
            assert_eq!(v["found"], true, "{name}: {v}");
            assert!(v.to_string().len() < MAX_ANSWER_CHARS * 3, "{name}: answer is bounded");
            table.push((name, v["kind"].as_str().unwrap_or("").to_owned(), "hit"));
        }
        for name in ["FlyToTheMoon", "SetGlitter", "COBOL-TELEPORT"] {
            let v = run(name, None).unwrap();
            assert_eq!(v["found"], false, "{name} must not be found: {v}");
            assert!(v["message"].as_str().unwrap().contains("gap report"));
            table.push((name, String::new(), "miss"));
        }
        assert!(run("", None).is_err());
        for (n, k, r) in &table {
            println!("  {n:<16} {k:<9} {r}");
        }
        println!(
            "kb: {} lookups — {} hits, {} misses (\"not in the reference\")",
            table.len(),
            table.iter().filter(|t| t.2 == "hit").count(),
            table.iter().filter(|t| t.2 == "miss").count()
        );
    }
}
