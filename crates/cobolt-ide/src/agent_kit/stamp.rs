// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Stamps, the kit manifest and ownership** (spec 080 R3, R5, T4.4).
//!
//! Every kit file records the version that wrote it: a Markdown file in a
//! comment (after its frontmatter when it has one), `CLAUDE.md` on the begin
//! marker of the kit's section — plus a visible line, since the model does not
//! see comments there — and a JSON file in the kit manifest only
//! (`.claude/powerrustcobol-kit.json`), which also holds a SHA-256 of what the
//! kit wrote. That hash is the R3 stamp: a file whose bytes (or kit-owned part)
//! no longer match it was edited by the developer and is kept.
//!
//! Everything here is planned in memory first ([`plan`]); [`write`] puts the
//! files on disk and the manifest last, so an interrupted export leaves the
//! previous manifest and the next export treats a half-written file as edited.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::content::{KitFile, KitFileKind};
use super::KIT_MANIFEST;

pub const MANIFEST_FORMAT: &str = "powerrustcobol-agent-kit";
pub const MANIFEST_VERSION: u32 = 1;
/// Opens the kit's section of `CLAUDE.md`; the version follows it.
pub const SECTION_BEGIN: &str = "<!-- powerrustcobol-kit:begin";
/// Closes the kit's section of `CLAUDE.md`.
pub const SECTION_END: &str = "<!-- powerrustcobol-kit:end -->";

/// The kit manifest (plan §3.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KitManifest {
    pub format: String,
    pub version: u32,
    pub target: String,
    pub ide_version: String,
    pub kit_id: String,
    pub mcp_port: u16,
    pub files: Vec<FileEntry>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// One kit file in the manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    /// The kit owns only its delimited section of this file.
    #[serde(default, skip_serializing_if = "is_false")]
    pub section: bool,
    /// JSON the kit merged into a file that holds more than the kit's keys.
    #[serde(default, skip_serializing_if = "is_false")]
    pub merged: bool,
    pub written_by: String,
    /// Of the bytes written; of the section for `CLAUDE.md`; of the kit-owned
    /// value (canonical JSON) for a JSON file.
    pub sha256: String,
    /// For a JSON file: the value the kit owns, so the next export can tell
    /// the developer's changes to it from its own, and drop what it no longer
    /// owns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owned: Option<Value>,
}

/// What the export does with one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// The file did not exist.
    Write,
    /// The kit wrote it and nobody changed it: rewritten.
    Replace,
    /// The developer changed it (or it is not the kit's): left alone.
    KeepEdited,
    /// The kit's part was put into a file that holds the developer's too.
    Merge,
}

/// One file's outcome, worked out before anything is written.
#[derive(Debug, Clone)]
pub struct Planned {
    pub rel: String,
    pub decision: Decision,
    /// The whole file to write; `None` when it is kept.
    pub bytes: Option<String>,
    /// What the kit itself contributes — the part a redaction scan reads.
    pub kit_text: String,
    /// The file's manifest entry after this export, if it has one.
    pub entry: Option<FileEntry>,
}

pub fn sha256(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    format!("{:x}", h.finalize())
}

/// The version stamp of a Markdown kit file.
pub fn stamp_line(version: &str) -> String {
    format!("<!-- powerrustcobol-kit: {version} -->")
}

/// A Markdown body with its stamp: first line, or right after frontmatter.
pub fn stamped(kind: KitFileKind, body: &str, version: &str) -> String {
    let stamp = stamp_line(version);
    if kind == KitFileKind::FrontMatterMarkdown {
        if let Some(rest) = body.strip_prefix("---\n") {
            if let Some(close) = rest.find("\n---\n") {
                let head = &body[..4 + close + 5];
                return format!("{head}{stamp}\n{}", &body[head.len()..]);
            }
        }
    }
    format!("{stamp}\n{body}")
}

/// The kit's section of `CLAUDE.md`, markers included.
pub fn section_block(version: &str, body: &str) -> String {
    format!("{SECTION_BEGIN} {version} -->\n{}\n{SECTION_END}\n", body.trim_end())
}

/// Where the kit's section is in `text`: `Some(Ok(start..end))` with `end`
/// past the end marker's line, `Some(Err(()))` for a begin without an end.
fn find_section(text: &str) -> Option<Result<std::ops::Range<usize>, ()>> {
    let start = text.find(SECTION_BEGIN)?;
    let Some(end_at) = text[start..].find(SECTION_END) else {
        return Some(Err(()));
    };
    let mut end = start + end_at + SECTION_END.len();
    if text[end..].starts_with('\n') {
        end += 1;
    }
    Some(Ok(start..end))
}

pub fn canonical(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default() + "\n"
}

/// `existing` seen through the shape of `shape`: an object keeps only the
/// keys `shape` has, an array only the items `shape` holds. What the developer
/// added around the kit's part does not count as an edit of it.
fn project(existing: &Value, shape: &Value) -> Value {
    match (existing, shape) {
        (Value::Object(e), Value::Object(s)) => Value::Object(
            s.iter()
                .filter_map(|(k, sv)| e.get(k).map(|ev| (k.clone(), project(ev, sv))))
                .collect(),
        ),
        (Value::Array(e), Value::Array(s)) => Value::Array(e.iter().filter(|i| s.contains(i)).cloned().collect()),
        _ => existing.clone(),
    }
}

/// `existing` with the kit's `new` part put in: objects merge key by key,
/// arrays keep the developer's items and gain the kit's, a value the kit owns
/// is replaced. What the kit owned before (`old`) and owns no longer is taken
/// out.
fn merge(existing: &Value, new: &Value, old: Option<&Value>) -> Value {
    match (existing, new) {
        (Value::Object(e), Value::Object(n)) => {
            let old_obj = old.and_then(Value::as_object);
            let mut out: Map<String, Value> = e.clone();
            if let Some(o) = old_obj {
                for k in o.keys() {
                    if !n.contains_key(k) {
                        out.remove(k);
                    }
                }
            }
            for (k, nv) in n {
                let merged = match e.get(k) {
                    Some(ev) => merge(ev, nv, old_obj.and_then(|o| o.get(k))),
                    None => nv.clone(),
                };
                out.insert(k.clone(), merged);
            }
            Value::Object(out)
        }
        (Value::Array(e), Value::Array(n)) => {
            let dropped: Vec<&Value> = old
                .and_then(Value::as_array)
                .map(|o| o.iter().filter(|i| !n.contains(i)).collect())
                .unwrap_or_default();
            let mut out: Vec<Value> = e.iter().filter(|i| !dropped.contains(i)).cloned().collect();
            for i in n {
                if !out.contains(i) {
                    out.push(i.clone());
                }
            }
            Value::Array(out)
        }
        _ => new.clone(),
    }
}

/// The kit manifest in `project_dir`, if it has a readable one.
pub fn read_manifest(project_dir: &Path) -> Option<KitManifest> {
    let text = std::fs::read_to_string(project_dir.join(KIT_MANIFEST)).ok()?;
    let m: KitManifest = serde_json::from_str(&text).ok()?;
    (m.format == MANIFEST_FORMAT).then_some(m)
}

/// A new random kit id (plan §3.2): identifies the kit, nothing about anyone.
pub fn new_kit_id() -> String {
    let mut g = tiny_id::ShortCodeGenerator::new_lowercase_alphanumeric(10);
    format!("k-{}", g.next_string())
}

/// Decide, file by file, what an export does — nothing is written.
pub fn plan(project_dir: &Path, files: &[KitFile], old: Option<&KitManifest>, version: &str) -> Vec<Planned> {
    let old_entry = |rel: &str| old.and_then(|m| m.files.iter().find(|e| e.path == rel)).cloned();
    files
        .iter()
        .map(|f| {
            let current = std::fs::read_to_string(project_dir.join(&f.rel)).ok();
            let exists = project_dir.join(&f.rel).exists();
            let prior = old_entry(&f.rel);
            match f.kind {
                KitFileKind::Markdown | KitFileKind::FrontMatterMarkdown => {
                    plan_markdown(f, version, current, exists, prior)
                }
                KitFileKind::Section => plan_section(f, version, current, exists, prior),
                KitFileKind::Json => plan_json(f, version, current, exists, prior),
            }
        })
        .collect()
}

fn keep(f: &KitFile, kit_text: String, prior: Option<FileEntry>) -> Planned {
    Planned { rel: f.rel.clone(), decision: Decision::KeepEdited, bytes: None, kit_text, entry: prior }
}

fn plan_markdown(f: &KitFile, version: &str, current: Option<String>, exists: bool, prior: Option<FileEntry>) -> Planned {
    let bytes = stamped(f.kind, &f.body, version);
    let entry = FileEntry {
        path: f.rel.clone(),
        section: false,
        merged: false,
        written_by: version.into(),
        sha256: sha256(&bytes),
        owned: None,
    };
    let decision = match (&current, &prior) {
        _ if !exists => Decision::Write,
        (Some(text), Some(p)) if sha256(text) == p.sha256 => Decision::Replace,
        (Some(text), _) if *text == bytes => Decision::Replace,
        _ => return keep(f, bytes, prior),
    };
    Planned { rel: f.rel.clone(), decision, kit_text: bytes.clone(), bytes: Some(bytes), entry: Some(entry) }
}

fn plan_section(f: &KitFile, version: &str, current: Option<String>, exists: bool, prior: Option<FileEntry>) -> Planned {
    let block = section_block(version, &f.body);
    let entry = FileEntry {
        path: f.rel.clone(),
        section: true,
        merged: false,
        written_by: version.into(),
        sha256: sha256(&block),
        owned: None,
    };
    let done = |decision, bytes: String| Planned {
        rel: f.rel.clone(),
        decision,
        bytes: Some(bytes),
        kit_text: block.clone(),
       
        entry: Some(entry.clone()),
    };
    if !exists {
        return done(Decision::Write, block.clone());
    }
    let Some(text) = current else {
        return keep(f, block, prior);
    };
    match find_section(&text) {
        None => {
            // The developer's own CLAUDE.md: the section goes after it, after a
            // blank line, and every byte of theirs stays where it is.
            let mut out = text.clone();
            if !out.is_empty() {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push('\n');
            }
            out.push_str(&block);
            done(Decision::Merge, out)
        }
        Some(Err(())) => keep(f, block, prior),
        Some(Ok(range)) => {
            let present = &text[range.clone()];
            let ours = match &prior {
                Some(p) => sha256(present) == p.sha256,
                None => present == block,
            };
            if !ours {
                return keep(f, block, prior);
            }
            let out = format!("{}{}{}", &text[..range.start], block, &text[range.end..]);
            let only_ours = range.start == 0 && range.end == text.len();
            done(if only_ours { Decision::Replace } else { Decision::Merge }, out)
        }
    }
}

fn plan_json(f: &KitFile, version: &str, current: Option<String>, exists: bool, prior: Option<FileEntry>) -> Planned {
    let new: Value = serde_json::from_str(&f.body).unwrap_or(Value::Null);
    let kit_text = canonical(&new);
    let hash = sha256(&kit_text);
    let entry = |merged: bool| FileEntry {
        path: f.rel.clone(),
        section: false,
        merged,
        written_by: version.into(),
        sha256: hash.clone(),
        owned: Some(new.clone()),
    };
    if !exists {
        return Planned {
            rel: f.rel.clone(),
            decision: Decision::Write,
            bytes: Some(pretty(&new)),
            kit_text,
           
            entry: Some(entry(false)),
        };
    }
    let Some(existing) = current.as_deref().and_then(|t| serde_json::from_str::<Value>(t).ok()).filter(Value::is_object)
    else {
        return keep(f, kit_text, prior);
    };
    let old_owned = prior.as_ref().and_then(|p| p.owned.clone());
    if let (Some(p), Some(o)) = (&prior, &old_owned) {
        if sha256(&canonical(&project(&existing, o))) != p.sha256 {
            return keep(f, kit_text, prior);
        }
    }
    let only_ours = match (&prior, &old_owned) {
        (Some(p), Some(o)) => !p.merged && existing == *o,
        _ => existing == new,
    };
    if only_ours {
        return Planned {
            rel: f.rel.clone(),
            decision: Decision::Replace,
            bytes: Some(pretty(&new)),
            kit_text,
           
            entry: Some(entry(false)),
        };
    }
    Planned {
        rel: f.rel.clone(),
        decision: Decision::Merge,
        bytes: Some(pretty(&merge(&existing, &new, old_owned.as_ref()))),
        kit_text,
       
        entry: Some(entry(true)),
    }
}

/// The manifest after this export: every planned file's entry, in order.
pub fn manifest_for(target: &str, version: &str, kit_id: &str, mcp_port: u16, planned: &[Planned]) -> KitManifest {
    KitManifest {
        format: MANIFEST_FORMAT.into(),
        version: MANIFEST_VERSION,
        target: target.into(),
        ide_version: version.into(),
        kit_id: kit_id.into(),
        mcp_port,
        files: planned.iter().filter_map(|p| p.entry.clone()).collect(),
    }
}

pub fn manifest_text(m: &KitManifest) -> String {
    serde_json::to_string_pretty(m).unwrap_or_default() + "\n"
}

/// Write every planned file, then the manifest — last.
pub fn write(project_dir: &Path, planned: &[Planned], manifest: &KitManifest) -> std::io::Result<()> {
    for p in planned {
        if let Some(bytes) = &p.bytes {
            let path = project_dir.join(&p.rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, bytes)?;
        }
    }
    let path = project_dir.join(KIT_MANIFEST);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, manifest_text(manifest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kit(version: &str) -> Vec<KitFile> {
        vec![
            KitFile { rel: "CLAUDE.md".into(), body: format!("# Brief\n\nWritten by {version}.\n"), kind: KitFileKind::Section },
            KitFile {
                rel: ".claude/skills/demo/SKILL.md".into(),
                body: "---\nname: demo\ndescription: \"d\"\n---\n\n# demo\n".into(),
                kind: KitFileKind::FrontMatterMarkdown,
            },
            KitFile { rel: "docs/powerrustcobol/README.md".into(), body: "# Pack\n".into(), kind: KitFileKind::Markdown },
            KitFile {
                rel: ".mcp.json".into(),
                body: r#"{"mcpServers":{"powerrustcobol":{"type":"stdio","command":"rcrun"}}}"#.into(),
                kind: KitFileKind::Json,
            },
            KitFile {
                rel: ".claude/settings.json".into(),
                body: r#"{"permissions":{"allow":["Edit(./**)","mcp__powerrustcobol__check"],"deny":["Bash"]}}"#.into(),
                kind: KitFileKind::Json,
            },
        ]
    }

    fn export(dir: &Path, version: &str) -> Vec<(String, Decision)> {
        let old = read_manifest(dir);
        let files = kit(version);
        let planned = plan(dir, &files, old.as_ref(), version);
        let id = old.map(|m| m.kit_id).unwrap_or_else(new_kit_id);
        write(dir, &planned, &manifest_for("test", version, &id, 5720, &planned)).unwrap();
        planned.into_iter().map(|p| (p.rel, p.decision)).collect()
    }

    fn decision(r: &[(String, Decision)], rel: &str) -> Decision {
        r.iter().find(|(p, _)| p == rel).unwrap().1
    }

    /// AC2 + R3 + R5: the developer's text around the section survives byte
    /// for byte, an edited skill is kept and reported, a deleted kit file is
    /// written again, a developer's own `.mcp.json` server survives a merge,
    /// and the kit id is kept.
    #[test]
    fn developer_text_and_edited_skill_survive() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let before = "# My project\n\nMy own notes — keep them.\n";
        let after = "\n## Later notes\nAlso mine.\n";
        std::fs::write(d.join("CLAUDE.md"), before).unwrap();
        std::fs::write(d.join(".mcp.json"), r#"{"mcpServers":{"mine":{"type":"stdio","command":"my-server"}}}"#).unwrap();

        let first = export(d, "1.80.70");
        assert_eq!(decision(&first, "CLAUDE.md"), Decision::Merge);
        assert_eq!(decision(&first, ".mcp.json"), Decision::Merge);
        assert_eq!(decision(&first, ".claude/settings.json"), Decision::Write);
        let id1 = read_manifest(d).unwrap().kit_id;

        // The developer writes after the section, edits the skill, deletes
        // the README, adds a permission of their own.
        let claude = std::fs::read_to_string(d.join("CLAUDE.md")).unwrap();
        std::fs::write(d.join("CLAUDE.md"), format!("{claude}{after}")).unwrap();
        let skill = d.join(".claude/skills/demo/SKILL.md");
        let edited = std::fs::read_to_string(&skill).unwrap() + "\nMy extra step.\n";
        std::fs::write(&skill, &edited).unwrap();
        std::fs::remove_file(d.join("docs/powerrustcobol/README.md")).unwrap();
        let mut s: Value = serde_json::from_str(&std::fs::read_to_string(d.join(".claude/settings.json")).unwrap()).unwrap();
        s["permissions"]["allow"].as_array_mut().unwrap().push("WebFetch(domain:example.com)".into());
        std::fs::write(d.join(".claude/settings.json"), s.to_string()).unwrap();

        let second = export(d, "1.80.71");
        let text = std::fs::read_to_string(d.join("CLAUDE.md")).unwrap();
        assert!(text.starts_with(before), "the developer's text before the section changed");
        assert!(text.ends_with(after), "the developer's text after the section changed");
        assert!(text.contains("<!-- powerrustcobol-kit:begin 1.80.71 -->"), "the section was not refreshed");
        assert_eq!(text.matches(SECTION_BEGIN).count(), 1);
        assert_eq!(decision(&second, "CLAUDE.md"), Decision::Merge);
        assert_eq!(decision(&second, ".claude/skills/demo/SKILL.md"), Decision::KeepEdited);
        assert_eq!(std::fs::read_to_string(&skill).unwrap(), edited, "the edited skill was overwritten");
        assert_eq!(decision(&second, "docs/powerrustcobol/README.md"), Decision::Write);
        let readme = std::fs::read_to_string(d.join("docs/powerrustcobol/README.md")).unwrap();
        assert!(readme.starts_with("<!-- powerrustcobol-kit: 1.80.71 -->\n"));
        let mcp: Value = serde_json::from_str(&std::fs::read_to_string(d.join(".mcp.json")).unwrap()).unwrap();
        assert_eq!(mcp["mcpServers"]["mine"]["command"], "my-server", "the developer's server was lost");
        assert_eq!(mcp["mcpServers"]["powerrustcobol"]["command"], "rcrun");
        let s: Value = serde_json::from_str(&std::fs::read_to_string(d.join(".claude/settings.json")).unwrap()).unwrap();
        assert!(s["permissions"]["allow"].as_array().unwrap().contains(&"WebFetch(domain:example.com)".into()));
        assert_eq!(decision(&second, ".claude/settings.json"), Decision::Merge);
        let m = read_manifest(d).unwrap();
        assert_eq!(m.kit_id, id1, "the kit id changed");
        let skill_entry = m.files.iter().find(|e| e.path.ends_with("SKILL.md")).unwrap();
        assert_eq!(skill_entry.written_by, "1.80.70", "a kept file keeps its old entry");

        // The developer removes a kit permission: that is an edit, kept.
        let mut s2 = s.clone();
        s2["permissions"]["deny"] = Value::Array(vec![]);
        std::fs::write(d.join(".claude/settings.json"), s2.to_string()).unwrap();
        let third = export(d, "1.80.72");
        assert_eq!(decision(&third, ".claude/settings.json"), Decision::KeepEdited);

        // Frontmatter stays first, the stamp right after it.
        let fm = stamped(KitFileKind::FrontMatterMarkdown, "---\nname: x\n---\n\nbody\n", "9.9.9");
        assert!(fm.starts_with("---\nname: x\n---\n<!-- powerrustcobol-kit: 9.9.9 -->\n"));

        println!(
            "stamp: {} + {} bytes of developer text byte-equal around the section; decisions 1st {:?}; \
             2nd {:?}; 3rd settings.json {:?}",
            before.len(),
            after.len(),
            first,
            second,
            decision(&third, ".claude/settings.json")
        );
    }
}
