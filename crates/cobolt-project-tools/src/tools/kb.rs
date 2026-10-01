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

fn cap(s: &str) -> String {
    if s.chars().count() <= MAX_ANSWER_CHARS {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(MAX_ANSWER_CHARS).collect();
    out.push_str("\n… (truncated; the full section is in docs/powerrustcobol/controls.md)");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
