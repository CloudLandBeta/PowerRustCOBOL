// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The coding-agent companion kit** (spec 080).
//!
//! What the IDE does for an external coding agent such as Claude Code: serve
//! the project tools of `cobolt-project-tools` over HTTP on `127.0.0.1`
//! ([`ide_host`]), and export the kit that tells the agent how to use them
//! ([`export`]):
//!
//! ```text
//! content::build (target-neutral) ─▶ KitWriter::files ─▶ stamp::plan
//!     ─▶ redact::check (refuse: nothing written) ─▶ stamp::write (manifest last)
//! ```

pub mod claude_code;
// Agent-neutral content and the reference pack live in `cobolt-project-tools`
// so `rcrun` serves them too (spec 084 T3); re-exported under their old paths.
pub use cobolt_project_tools::{content, reference};
pub mod ide_host;
pub mod redact;
pub mod stamp;

use std::path::{Path, PathBuf};

use crate::ai_bundle::Personal;
use crate::llm::LlmConfig;

use content::{KitWriter, RcrunLocation, ToolInfo};
use stamp::Decision;

/// The kit manifest, relative to the project folder (spec 080 R5, plan §3.2).
/// The IDE reads its `kit_id` to know which project an MCP request is for.
pub const KIT_MANIFEST: &str = ".claude/powerrustcobol-kit.json";

/// The `kit_id` of the kit written into `project_dir`, if it has one.
pub fn kit_id_of(project_dir: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(project_dir.join(KIT_MANIFEST)).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get("kit_id")
        .and_then(|k| k.as_str())
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_owned)
}

/// The coding agents a kit can be written for (R1, R20). One today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    ClaudeCode,
}

impl Target {
    fn writer(self) -> Box<dyn KitWriter> {
        match self {
            Target::ClaudeCode => Box::new(claude_code::ClaudeCodeWriter),
        }
    }

    /// The target a kit manifest names (`KitWriter::target`).
    pub fn from_id(id: &str) -> Option<Target> {
        [Target::ClaudeCode].into_iter().find(|t| t.writer().target() == id)
    }
}

/// Whether a kit written by `kit` should be offered a refresh by an IDE at
/// `version` (R6): only when there is a kit and another version wrote it.
pub fn needs_refresh(kit: Option<&stamp::KitManifest>, version: &str) -> bool {
    kit.is_some_and(|m| m.ide_version != version)
}

/// The refresh offer for the project in `project_dir`: the kit's version and
/// its target, when [`needs_refresh`] says so.
pub fn refresh_offer(project_dir: &Path, version: &str) -> Option<(String, Target)> {
    let kit = stamp::read_manifest(project_dir);
    if !needs_refresh(kit.as_ref(), version) {
        return None;
    }
    let kit = kit?;
    Some((kit.ide_version.clone(), Target::from_id(&kit.target)?))
}

/// What an export needs from the machine it runs on.
pub struct ExportContext<'a> {
    /// The IDE version that writes the kit (R5).
    pub ide_version: &'a str,
    /// The port the IDE serves the tools on (R11a) — baked into `.mcp.json`.
    pub mcp_port: u16,
    /// `rcrun` beside the IDE, if it is there.
    pub rcrun: Option<PathBuf>,
    /// The home folder, so a path under it is written without it (R4).
    pub home: Option<PathBuf>,
    pub personal: &'a Personal,
    /// The stored keys an export must never carry (R4).
    pub llm: &'a LlmConfig,
}

/// What an export did, file by file (R1: the Output panel lists it).
#[derive(Debug, Clone)]
pub struct ExportReport {
    pub files: Vec<(String, Decision)>,
    pub kit_id: String,
    /// Personal details replaced in the inserted values.
    pub replaced: usize,
    /// Files the refusal scan read.
    pub scanned: usize,
}

/// Why nothing was written.
#[derive(Debug, Clone)]
pub enum ExportError {
    /// The folder is not a project this export can read.
    NoProject(String),
    /// A stored key or a personal detail would have been written (R4).
    Refused(redact::Refusal),
    /// The disk refused a write.
    Failed(String),
}

impl ExportError {
    /// The Output line, in the IDE's language.
    pub fn message(&self, tr: &crate::i18n::Tr) -> String {
        match self {
            ExportError::Refused(r) => r.message(tr),
            ExportError::NoProject(e) | ExportError::Failed(e) => tr.agent_kit_refused.replacen("{}", e, 1),
        }
    }
}

/// Write the kit for `target` into the project at `project` (its folder or
/// its manifest). Everything is planned and scanned in memory first; a refusal
/// writes nothing, and the kit manifest is written last.
pub fn export(project: &Path, target: Target, ctx: &ExportContext) -> Result<ExportReport, ExportError> {
    let root = cobolt_project_tools::ProjectRoot::open(project).map_err(ExportError::NoProject)?;
    let dir = root.dir().to_path_buf();
    let view = cobolt_compiler::project_manifest_view(root.manifest()).map_err(ExportError::NoProject)?;

    // The values the export inserts, scrubbed before anything is rendered.
    let (project_name, mut replaced) = redact::scrub_value(ctx.personal, &view.name);
    let rcrun = match content::locate_rcrun(ctx.rcrun.as_deref(), ctx.home.as_deref()) {
        RcrunLocation::UnderHome(p) => {
            let (p, n) = redact::scrub_value(ctx.personal, &p);
            replaced += n;
            RcrunLocation::UnderHome(p)
        }
        RcrunLocation::Absolute(p) => {
            let (p, n) = redact::scrub_value(ctx.personal, &p);
            replaced += n;
            RcrunLocation::Absolute(p)
        }
        RcrunLocation::OnPath => RcrunLocation::OnPath,
    };
    let rcrun_text = match &rcrun {
        RcrunLocation::UnderHome(p) | RcrunLocation::Absolute(p) => p.clone(),
        RcrunLocation::OnPath => String::new(),
    };
    let inserted = [(claude_code::BRIEF_FILE, project_name.clone()), (".mcp.json", rcrun_text)];
    let inserted: Vec<(&str, &str)> = inserted.iter().map(|(f, v)| (*f, v.as_str())).collect();

    let old = stamp::read_manifest(&dir);
    let kit_id = old.as_ref().map(|m| m.kit_id.clone()).unwrap_or_else(stamp::new_kit_id);
    let tools = cobolt_project_tools::ProjectTools::<cobolt_project_tools::HeadlessHost>::tool_list()
        .into_iter()
        .map(|t| ToolInfo { name: t.name, description: t.description.unwrap_or_default() })
        .collect();
    let content = content::build(content::BuildInput {
        ide_version: ctx.ide_version.to_owned(),
        project_name,
        ide_url: format!("http://127.0.0.1:{}/mcp/{kit_id}", ctx.mcp_port),
        rcrun,
        tools,
        reference: reference::pack(ctx.ide_version),
    });

    let writer = target.writer();
    let files = writer.files(&content);
    let planned = stamp::plan(&dir, &files, old.as_ref(), ctx.ide_version);
    let manifest = stamp::manifest_for(writer.target(), ctx.ide_version, &kit_id, ctx.mcp_port, &planned);
    let manifest_text = stamp::manifest_text(&manifest);
    redact::check_inserted(&inserted, ctx.personal).map_err(ExportError::Refused)?;
    let scanned = redact::check(&planned, (KIT_MANIFEST, &manifest_text), ctx.llm).map_err(ExportError::Refused)?;
    stamp::write(&dir, &planned, &manifest).map_err(|e| ExportError::Failed(e.to_string()))?;
    Ok(ExportReport {
        files: planned.into_iter().map(|p| (p.rel, p.decision)).collect(),
        kit_id,
        replaced,
        scanned,
    })
}

#[cfg(test)]
mod tests {
    /// The reference documents `cobolt-project-tools` embeds for `rcrun` are
    /// byte-for-byte the ones the IDE's documentation viewer embeds (spec 084
    /// T3) — one source file each, two embeddings, no drift.
    #[test]
    fn the_reference_pack_embeds_the_same_documents_as_the_ide() {
        let pairs = [
            ("developers-guide-en.md", cobolt_project_tools::reference::DEVELOPERS_GUIDE),
            ("cobol85-supported-syntax-en.md", cobolt_project_tools::reference::SUPPORTED_SYNTAX),
        ];
        for (name, embedded) in pairs {
            assert_eq!(crate::docs_embed::embedded_doc(name), Some(embedded), "{name} differs");
        }
        println!("reference pack: {} documents identical to the IDE's embedded copies ({} bytes)",
            pairs.len(), pairs.iter().map(|(_, t)| t.len()).sum::<usize>());
    }

    use super::*;
    use std::collections::BTreeMap;

    /// R6: equal → no offer; older or newer → an offer; no or unreadable kit
    /// manifest → no offer (there is no kit).
    #[test]
    fn refresh_offered_only_for_a_kit_of_another_version() {
        let m = |v: &str| stamp::manifest_for("claude-code", v, "k-x", 5720, &[]);
        let cases = [
            ("equal", needs_refresh(Some(&m("1.80.71")), "1.80.71"), false),
            ("older", needs_refresh(Some(&m("1.80.70")), "1.80.71"), true),
            ("newer", needs_refresh(Some(&m("1.80.99")), "1.80.71"), true),
            ("no kit", needs_refresh(None, "1.80.71"), false),
        ];
        for (name, got, want) in cases {
            assert_eq!(got, want, "{name}");
        }
        let tmp = tempfile::tempdir().unwrap();
        assert!(refresh_offer(tmp.path(), "1.80.71").is_none(), "no manifest");
        std::fs::create_dir_all(tmp.path().join(".claude")).unwrap();
        std::fs::write(tmp.path().join(KIT_MANIFEST), "{ not json").unwrap();
        assert!(refresh_offer(tmp.path(), "1.80.71").is_none(), "unreadable manifest");
        std::fs::write(tmp.path().join(KIT_MANIFEST), stamp::manifest_text(&m("1.80.70"))).unwrap();
        assert_eq!(refresh_offer(tmp.path(), "1.80.71"), Some(("1.80.70".into(), Target::ClaudeCode)));
        println!("refresh: 4 version cases + 3 manifest cases (none, unreadable, older → offer for claude-code)");
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                copy_dir(&p, &to.join(e.file_name()));
            } else {
                std::fs::copy(&p, to.join(e.file_name())).unwrap();
            }
        }
    }

    /// AC1 (R1, R2, R5): an export into a copy of PowerChat writes every R2
    /// file, each carrying the version; every JSON file is in the manifest
    /// with its version and hash; a second export keeps nothing it should
    /// replace and keeps the kit id.
    #[test]
    fn export_into_powerchat_copy() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerChat");
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::copy(src.join("PowerChat.project.toml"), dir.join("PowerChat.project.toml")).unwrap();
        copy_dir(&src.join("forms"), &dir.join("forms"));
        copy_dir(&src.join("src"), &dir.join("src"));

        let llm = LlmConfig::load_defaults_for_test();
        let personal = Personal::new(None, None, None, None);
        let ctx = ExportContext {
            ide_version: "1.80.70",
            mcp_port: 5720,
            rcrun: None,
            home: None,
            personal: &personal,
            llm: &llm,
        };
        let first = export(dir, Target::ClaudeCode, &ctx).expect("export");

        // R2: every kind of file.
        let rels: Vec<&str> = first.files.iter().map(|(r, _)| r.as_str()).collect();
        for want in ["CLAUDE.md", ".claude/settings.json", ".mcp.json", ".claude/agents/powerrustcobol-reviewer.md",
                     "docs/powerrustcobol/README.md", "docs/powerrustcobol/developers-guide.md",
                     "docs/powerrustcobol/controls.md", "docs/powerrustcobol/builtins.md"] {
            assert!(rels.contains(&want), "{want} not in the report");
            assert!(dir.join(want).is_file(), "{want} not written");
        }
        assert_eq!(rels.iter().filter(|r| r.starts_with(".claude/skills/") && r.ends_with("/SKILL.md")).count(), 7);
        assert!(dir.join(KIT_MANIFEST).is_file(), "no kit manifest");

        // R5: the version in every file — a stamp, or the manifest for JSON.
        let manifest = stamp::read_manifest(dir).expect("manifest reads");
        assert_eq!(manifest.ide_version, "1.80.70");
        assert_eq!(kit_id_of(dir).as_deref(), Some(first.kit_id.as_str()));
        let mut by_kind: BTreeMap<&str, usize> = BTreeMap::new();
        for (rel, decision) in &first.files {
            assert_eq!(*decision, Decision::Write, "{rel}: a fresh project writes everything");
            let text = std::fs::read_to_string(dir.join(rel)).unwrap();
            let entry = manifest.files.iter().find(|e| &e.path == rel).unwrap_or_else(|| panic!("{rel} not in the manifest"));
            assert_eq!(entry.written_by, "1.80.70");
            let kind = if rel.ends_with(".json") {
                assert!(entry.owned.is_some() && entry.sha256.len() == 64, "{rel}: JSON without its stamp");
                serde_json::from_str::<serde_json::Value>(&text).expect("JSON parses");
                "json"
            } else if rel == "CLAUDE.md" {
                assert!(text.starts_with("<!-- powerrustcobol-kit:begin 1.80.70 -->\n"), "CLAUDE.md marker");
                assert!(text.contains("Written by PowerRustCOBOL AI 1.80.70"), "CLAUDE.md visible version");
                "section"
            } else if text.starts_with("---\n") {
                assert!(text.contains("\n---\n<!-- powerrustcobol-kit: 1.80.70 -->\n"), "{rel}: stamp after frontmatter");
                "frontmatter"
            } else {
                assert!(text.starts_with("<!-- powerrustcobol-kit: 1.80.70 -->\n"), "{rel}: stamp on line 1");
                "markdown"
            };
            *by_kind.entry(kind).or_default() += 1;
        }
        let mcp: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join(".mcp.json")).unwrap()).unwrap();
        assert_eq!(mcp["mcpServers"]["powerrustcobol-ide"]["url"], format!("http://127.0.0.1:5720/mcp/{}", first.kit_id));

        // A second export: nothing kept as edited, the same kit id.
        let second = export(dir, Target::ClaudeCode, &ExportContext { ide_version: "1.80.71", ..ctx }).expect("re-export");
        let mut decisions: BTreeMap<String, usize> = BTreeMap::new();
        for (rel, d) in &second.files {
            assert_eq!(*d, Decision::Replace, "{rel}: a re-export replaces its own unchanged files");
            *decisions.entry(format!("{d:?}")).or_default() += 1;
        }
        assert_eq!(second.kit_id, first.kit_id, "the kit id must survive a re-export");
        let claude = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
        assert_eq!(claude.matches(stamp::SECTION_BEGIN).count(), 1);
        assert!(claude.contains("begin 1.80.71"));
        println!(
            "export into a PowerChat copy: {} files written by kind {by_kind:?} + the kit manifest; \
             re-export {decisions:?}, kit id kept ({})",
            first.files.len(),
            first.kit_id
        );
    }
}
