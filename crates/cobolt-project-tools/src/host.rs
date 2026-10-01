// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The one seam where the IDE and the headless server differ** (spec 080
//! plan §1.5).
//!
//! The tools never write a project manifest themselves. They ask the host to
//! *record* a file, and the host does it the way its owner of the manifest
//! would: the IDE through its in-memory project and its own save (so the next
//! IDE save cannot clobber the agent's work), `rcrun mcp` by editing the file
//! and re-sealing the main-form designation exactly as an IDE save does.

use std::path::{Path, PathBuf};

use crate::root::ProjectRoot;

/// Why a tool has no project to act on (spec 080 R14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoProject {
    /// Nothing is open, or the manifest cannot be read.
    NoneOpen,
    /// The IDE has another project open than the one this kit belongs to.
    Different,
}

impl NoProject {
    /// What every tool answers in this state.
    pub fn message(&self) -> &'static str {
        match self {
            NoProject::NoneOpen => "no project open",
            NoProject::Different => {
                "the IDE has a different project open; open this kit's project in the IDE, \
                 or use the `powerrustcobol` server"
            }
        }
    }
}

/// A file list of the project manifest (`[files] <list>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileList {
    Sources,
    Forms,
    Indexed,
    Assets,
    Documentation,
    Generated,
}

impl FileList {
    /// Every list, in the order `list_files` reports them.
    pub const ALL: [FileList; 6] = [
        FileList::Forms,
        FileList::Indexed,
        FileList::Sources,
        FileList::Generated,
        FileList::Assets,
        FileList::Documentation,
    ];

    /// The manifest key.
    pub fn key(self) -> &'static str {
        match self {
            FileList::Sources => "sources",
            FileList::Forms => "forms",
            FileList::Indexed => "indexed",
            FileList::Assets => "assets",
            FileList::Documentation => "documentation",
            FileList::Generated => "generated",
        }
    }

    pub fn from_key(key: &str) -> Option<FileList> {
        FileList::ALL.into_iter().find(|l| l.key() == key.trim())
    }

    /// Where a file belongs by its extension — the IDE's `FileKind::from_path`
    /// routing (Common Code, Forms, Indexed Files, Documentation, Assets).
    /// Generated code is never routed here: only `regenerate` records it.
    pub fn of_path(rel: &str) -> FileList {
        let ext = Path::new(rel)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "cbl" | "cob" | "cpy" => FileList::Sources,
            "cfrm" => FileList::Forms,
            "cidx" => FileList::Indexed,
            "md" | "markdown" | "txt" | "rst" | "adoc" | "pdf" | "html" | "htm" => {
                FileList::Documentation
            }
            _ => FileList::Assets,
        }
    }
}

/// What a host provides to the tools.
pub trait ProjectHost: Send {
    /// The project the tools act on, or why there is none (R14).
    fn project(&self) -> Result<ProjectRoot, NoProject>;

    /// Record `rel` in the manifest's `list`, re-sealing the main-form
    /// designation exactly as an IDE save does.
    fn record(&mut self, rel: &str, list: FileList) -> Result<(), String>;

    /// A file the IDE holds unsaved changes for — a writing tool refuses it.
    fn unsaved(&self, _abs: &Path) -> bool {
        false
    }

    /// Tell the host what was written (the IDE reloads tabs and re-lights the
    /// tree).
    fn written(&mut self, _abs: &[PathBuf]) {}

    /// The SDK root a build uses; never revealed by any tool.
    fn workspace_root(&self) -> Option<PathBuf> {
        None
    }

    /// The `use`-line names of the project's registered External Crates.
    fn external_crates(&self) -> Vec<String>;

    /// Why a build may not start now (the IDE is building), if it may not.
    fn build_blocked(&self) -> Option<String> {
        None
    }

    /// The version this server reports in `initialize`.
    fn version(&self) -> String;
}

// ── HeadlessHost (rcrun mcp) ────────────────────────────────────────────────

/// The host of `rcrun mcp`: the project is the manifest it was started with.
pub struct HeadlessHost {
    start: PathBuf,
    version: String,
}

impl HeadlessHost {
    /// A host for the project at `manifest_or_dir`. Nothing is read yet: a
    /// manifest that cannot be read makes every tool answer "no project open"
    /// rather than failing the server's start.
    pub fn new(manifest_or_dir: impl Into<PathBuf>, version: impl Into<String>) -> Self {
        Self {
            start: manifest_or_dir.into(),
            version: version.into(),
        }
    }
}

impl ProjectHost for HeadlessHost {
    fn project(&self) -> Result<ProjectRoot, NoProject> {
        let root = ProjectRoot::open(&self.start).map_err(|_| NoProject::NoneOpen)?;
        // Readable AND parseable, or there is no project.
        cobolt_compiler::project_manifest_view(root.manifest()).map_err(|_| NoProject::NoneOpen)?;
        Ok(root)
    }

    fn record(&mut self, rel: &str, list: FileList) -> Result<(), String> {
        let root = self.project().map_err(|e| e.message().to_owned())?;
        record_in_manifest(root.manifest(), rel, list)
    }

    fn external_crates(&self) -> Vec<String> {
        self.project()
            .ok()
            .and_then(|r| cobolt_compiler::project_manifest_view(r.manifest()).ok())
            .map(|v| v.crates)
            .unwrap_or_default()
    }

    fn version(&self) -> String {
        self.version.clone()
    }
}

/// Add `rel` to `[files] <list>` of the manifest at `manifest` (deduplicated,
/// `/` separators — the IDE's `add_file_to`; a generated file also leaves
/// `sources`, as `add_generated` does), re-seal the main-form designation
/// with `designation_record`, and write the file back. Every key the tools
/// do not own survives the round trip. Nothing is written when nothing
/// changed.
pub fn record_in_manifest(manifest: &Path, rel: &str, list: FileList) -> Result<(), String> {
    let text = std::fs::read_to_string(manifest)
        .map_err(|e| format!("the project file cannot be read ({e})"))?;
    let mut doc: toml::Value =
        toml::from_str(&text).map_err(|e| format!("the project file cannot be parsed ({e})"))?;
    let rel = rel.replace('\\', "/");
    let table = doc
        .as_table_mut()
        .ok_or_else(|| "the project file is not a table".to_owned())?;

    let files = table
        .entry("files")
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .ok_or_else(|| "[files] is not a table".to_owned())?;
    let mut changed = false;
    if list == FileList::Generated {
        if let Some(src) = files.get_mut("sources").and_then(|v| v.as_array_mut()) {
            let before = src.len();
            src.retain(|v| v.as_str() != Some(rel.as_str()));
            changed |= src.len() != before;
        }
    }
    let entries = files
        .entry(list.key())
        .or_insert_with(|| toml::Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| format!("[files] {} is not a list", list.key()))?;
    if !entries.iter().any(|v| v.as_str() == Some(rel.as_str())) {
        entries.push(toml::Value::String(rel.clone()));
        changed = true;
    }

    // The re-seal: what an IDE save restates on every save.
    let forms: Vec<String> = files
        .get("forms")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
    let project = table
        .get("project")
        .and_then(|v| v.as_table())
        .ok_or_else(|| "the project file has no [project] table".to_owned())?;
    let name = project
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_owned();
    let structure = project
        .get("structure")
        .and_then(|v| v.as_integer())
        .unwrap_or(0)
        .max(0) as u32;
    if let Some((main, seal)) = cobolt_compiler::main_form_guard::designation_record(
        structure,
        manifest.parent(),
        &name,
        &forms,
    ) {
        let forms_tbl = table
            .entry("forms")
            .or_insert_with(|| toml::Value::Table(Default::default()))
            .as_table_mut()
            .ok_or_else(|| "[forms] is not a table".to_owned())?;
        for (key, value) in [("main-form", main), ("main-form-seal", seal)] {
            if forms_tbl.get(key).and_then(|v| v.as_str()) != Some(value.as_str()) {
                forms_tbl.insert(key.to_owned(), toml::Value::String(value));
                changed = true;
            }
        }
    }

    if !changed {
        return Ok(());
    }
    let out = toml::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(manifest, out).map_err(|e| format!("the project file cannot be written ({e})"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_compiler::main_form_guard::{authorize_form_start, StartVerdict};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("prc-080-host-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("forms")).unwrap();
        d
    }

    fn form(dir: &Path, id: &str, main: bool) {
        let mut f = cobolt_forms::Form::new(id, id, 400, 300);
        f.main_form = main;
        cobolt_forms::save_form(&f, &dir.join(format!("forms/{id}.cfrm"))).unwrap();
    }

    #[test]
    fn host_records_a_form_and_the_seal_verifies() {
        let dir = tmp("record");
        form(&dir, "MAIN", true);
        form(&dir, "SECOND", false);
        let manifest = dir.join("Demo.project.toml");
        std::fs::write(
            &manifest,
            "[project]\nname = \"Demo\"\nstructure = 1\nversion = \"1.0.0\"\nmain = \"\"\n\
             [files]\nforms = [\"forms/MAIN.cfrm\"]\nsources = [\"generated/MAIN.cbl\"]\n\
             [ide]\ntheme = \"nord\"\n[custom-table]\nkeep = \"me\"\n",
        )
        .unwrap();
        let mut host = HeadlessHost::new(&dir, "test");
        assert!(host.project().is_ok());

        host.record("forms\\SECOND.cfrm", FileList::Forms).unwrap();
        host.record("generated/MAIN.cbl", FileList::Generated).unwrap();
        let v = cobolt_compiler::project_manifest_view(&manifest).unwrap();
        assert_eq!(v.forms, ["forms/MAIN.cfrm", "forms/SECOND.cfrm"]);
        assert_eq!(v.generated, ["generated/MAIN.cbl"]);
        assert!(v.sources.is_empty(), "a generated file leaves sources");

        let text = std::fs::read_to_string(&manifest).unwrap();
        assert!(text.contains("keep = \"me\""), "an unknown table survives:\n{text}");
        assert!(text.contains("theme = \"nord\""));
        assert_eq!(
            authorize_form_start(&dir.join("forms/MAIN.cfrm"), None),
            StartVerdict::Allowed,
            "the re-seal verifies:\n{text}"
        );
        // A second record of the same file writes nothing.
        let before = std::fs::metadata(&manifest).unwrap().modified().unwrap();
        host.record("forms/SECOND.cfrm", FileList::Forms).unwrap();
        assert_eq!(std::fs::read_to_string(&manifest).unwrap(), text);
        let _ = before;
        println!("host: 2 records (form, generated), seal verifies (Allowed), 2 foreign keys kept, 1 duplicate record wrote nothing");
    }

    #[test]
    fn host_with_an_unreadable_manifest_has_no_project() {
        let dir = tmp("unreadable");
        let manifest = dir.join("Demo.project.toml");
        std::fs::write(&manifest, "this is = [not toml").unwrap();
        let mut host = HeadlessHost::new(&manifest, "test");
        assert_eq!(host.project().unwrap_err(), NoProject::NoneOpen);
        assert!(host.record("forms/X.cfrm", FileList::Forms).is_err());
        let missing = HeadlessHost::new(dir.join("missing.project.toml"), "test");
        assert_eq!(missing.project().unwrap_err(), NoProject::NoneOpen);
        println!("host: unparseable and missing manifests → NoProject (2 cases)");
    }

    #[test]
    fn file_lists_route_by_extension() {
        let cases = [
            ("forms/A.cfrm", FileList::Forms),
            ("indexed/a.cidx", FileList::Indexed),
            ("src/x.cbl", FileList::Sources),
            ("COPYBOOKS/a.cpy", FileList::Sources),
            ("docs/a.md", FileList::Documentation),
            ("assets/a.png", FileList::Assets),
        ];
        for (p, l) in cases {
            assert_eq!(FileList::of_path(p), l, "{p}");
        }
        println!("host: {} extension routes checked", cases.len());
    }
}
