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

use cobolt_compiler::ExternalCrate;

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
    /// The Backend part of a Web or Mobile project (WASM-001): the programs and
    /// copybooks under `backend/`.
    Backend,
}

impl FileList {
    /// Every list, in the order `list_files` reports them.
    pub const ALL: [FileList; 7] = [
        FileList::Forms,
        FileList::Indexed,
        FileList::Sources,
        FileList::Generated,
        FileList::Assets,
        FileList::Documentation,
        FileList::Backend,
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
            FileList::Backend => "backend",
        }
    }

    /// Where a file of a project of `kind` belongs: its extension's list
    /// ([`FileList::of_path`]), except that under `backend/` in a Web or Mobile
    /// project a COBOL file is the Backend's, not Common Code.
    pub fn of_path_in(rel: &str, kind: cobolt_compiler::ProjectKind) -> FileList {
        let top = rel.replace('\\', "/");
        let in_backend = top
            .split('/')
            .next()
            .is_some_and(|seg| seg.eq_ignore_ascii_case(cobolt_compiler::project_kind::BACKEND_FOLDER))
            && top.contains('/');
        match FileList::of_path(rel) {
            FileList::Sources if kind.has_backend() && in_backend => FileList::Backend,
            other => other,
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
            e if cobolt_compiler::project_kind::is_form_extension(e) => FileList::Forms,
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

    /// The project's External Crates in full — what `add_crate` and
    /// `remove_crate` work from, where [`ProjectHost::external_crates`] carries
    /// only the `use`-line names. Read from the manifest on disk.
    fn crate_pins(&self) -> Result<Vec<ExternalCrate>, String> {
        let root = self.project().map_err(|e| e.message().to_owned())?;
        read_crate_pins(root.manifest())
    }

    /// Record the project's External Crates as `pins` — the whole list. The
    /// default writes the manifest, keeping every byte the tools do not own;
    /// the IDE records through its project model instead, so that it stays the
    /// only writer of its manifest while it is open.
    fn record_crates(&mut self, pins: &[ExternalCrate]) -> Result<(), String> {
        let root = self.project().map_err(|e| e.message().to_owned())?;
        write_crate_pins(root.manifest(), pins)
    }

    /// The registry crates are searched and downloaded from: the IDE-wide
    /// setting in the IDE, crates.io when headless. Never revealed by a tool
    /// (R13).
    fn crates_registry(&self) -> String {
        crate::external_crates::DEFAULT_REGISTRY.to_owned()
    }

    /// Why a build may not start now (the IDE is building), if it may not.
    fn build_blocked(&self) -> Option<String> {
        None
    }

    /// The version this server reports in `initialize`.
    fn version(&self) -> String;

    /// Create a new project in `folder` (spec 084 R15, R18) — exactly what
    /// New Project writes. A host that can also OPEN it does so.
    fn create_project(
        &mut self,
        folder: &Path,
        request: &crate::create::CreateRequest,
    ) -> Result<serde_json::Value, String> {
        crate::create::create_project_with(folder, request)
    }

    /// Open the project at `path` — a project folder or its manifest — as the
    /// one the tools act on (spec 084 R16).
    fn open_project(&mut self, _path: &Path) -> Result<serde_json::Value, String> {
        Err("this host cannot open a project".to_owned())
    }
}

// ── HeadlessHost (rcrun mcp) ────────────────────────────────────────────────

/// The host of `rcrun mcp`: the project is the manifest it was started with.
pub struct HeadlessHost {
    start: PathBuf,
    version: String,
    /// Called with the manifest after `create_project` / `open_project`
    /// switched to it; what it returns is added to the answer as `ide`.
    /// `rcrun` uses it to open the project in PowerRustCOBOL AI (spec 084
    /// R20a).
    on_switch: Option<Box<dyn Fn(&Path) -> String + Send>>,
}

impl HeadlessHost {
    /// A host for the project at `manifest_or_dir`. Nothing is read yet: a
    /// manifest that cannot be read makes every tool answer "no project open"
    /// rather than failing the server's start.
    pub fn new(manifest_or_dir: impl Into<PathBuf>, version: impl Into<String>) -> Self {
        Self {
            start: manifest_or_dir.into(),
            version: version.into(),
            on_switch: None,
        }
    }
}

impl HeadlessHost {
    /// Run `hook` with the manifest each time a tool switches project.
    pub fn on_switch(mut self, hook: impl Fn(&Path) -> String + Send + 'static) -> Self {
        self.on_switch = Some(Box::new(hook));
        self
    }

    fn switched(&self, mut answer: serde_json::Value, manifest: &Path) -> serde_json::Value {
        if let (Some(hook), Some(map)) = (&self.on_switch, answer.as_object_mut()) {
            map.insert("ide".into(), serde_json::Value::String(hook(manifest)));
        }
        answer
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

    /// The new project becomes the one this server acts on.
    fn create_project(
        &mut self,
        folder: &Path,
        request: &crate::create::CreateRequest,
    ) -> Result<serde_json::Value, String> {
        let answer = crate::create::create_project_with(folder, request)?;
        let manifest = folder.join(answer["project"].as_str().unwrap_or_default());
        self.start = manifest.clone();
        Ok(self.switched(answer, &manifest))
    }

    /// Switch this server to another project. The IDE is not involved here;
    /// launching it for the project is `rcrun`'s (spec 084 R20a).
    fn open_project(&mut self, path: &Path) -> Result<serde_json::Value, String> {
        let root = ProjectRoot::open(path).map_err(|_| "no PowerRustCOBOL project there".to_owned())?;
        self.start = root.manifest().to_path_buf();
        let answer = serde_json::json!({ "opened": true, "project": root.manifest_name() });
        Ok(self.switched(answer, root.manifest()))
    }

    fn version(&self) -> String {
        self.version.clone()
    }
}

/// Add `rel` to `[files] <list>` of the manifest at `manifest` (deduplicated,
/// `/` separators — the IDE's `add_file_to`; a generated file also leaves
/// `sources`, as `add_generated` does), re-seal the main-form designation
/// with `designation_record`, and write the file back.
///
/// **The developer's project file is theirs.** The edit goes through
/// `toml_edit`, and only the keys a tool owns are touched — the one
/// `[files]` list (plus `sources` for a generated file), `[forms] main-form`
/// and `main-form-seal`. Every other byte — comments, key order, blank lines,
/// tables the tools know nothing of — is written back exactly as it was read.
/// Nothing is written when nothing changed.
pub fn record_in_manifest(manifest: &Path, rel: &str, list: FileList) -> Result<(), String> {
    use toml_edit::{value, Array, DocumentMut, Item, Table};

    let text = std::fs::read_to_string(manifest)
        .map_err(|e| format!("the project file cannot be read ({e})"))?;
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e| format!("the project file cannot be parsed ({e})"))?;
    let rel = rel.replace('\\', "/");
    let mut changed = false;

    {
        if !doc.contains_key("files") {
            doc.insert("files", Item::Table(Table::new()));
        }
        let files = doc["files"]
            .as_table_like_mut()
            .ok_or_else(|| "[files] is not a table".to_owned())?;
        if list == FileList::Generated {
            if let Some(src) = files.get_mut("sources").and_then(|i| i.as_array_mut()) {
                let before = src.len();
                src.retain(|v| v.as_str() != Some(rel.as_str()));
                changed |= src.len() != before;
            }
        }
        if files.get(list.key()).is_none() {
            files.insert(list.key(), value(Array::new()));
        }
        let entries = files
            .get_mut(list.key())
            .and_then(|i| i.as_array_mut())
            .ok_or_else(|| format!("[files] {} is not a list", list.key()))?;
        if !entries.iter().any(|v| v.as_str() == Some(rel.as_str())) {
            entries.push(rel.as_str());
            changed = true;
        }
    }

    // The re-seal: what an IDE save restates on every save.
    let forms: Vec<String> = doc
        .get("files")
        .and_then(|f| f.get("forms"))
        .and_then(|i| i.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
    let project = doc
        .get("project")
        .and_then(|i| i.as_table_like())
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
        for (key, new) in [("main-form", main), ("main-form-seal", seal)] {
            let current = doc.get("forms").and_then(|f| f.get(key)).and_then(|v| v.as_str());
            if current == Some(new.as_str()) || (current.is_none() && new.is_empty()) {
                continue;
            }
            if !doc.contains_key("forms") {
                doc.insert("forms", Item::Table(Table::new()));
            }
            let forms_tbl = doc["forms"]
                .as_table_like_mut()
                .ok_or_else(|| "[forms] is not a table".to_owned())?;
            match forms_tbl.get_mut(key).and_then(|i| i.as_value_mut()) {
                // Keep the key's own spacing and any trailing comment.
                Some(v) => {
                    let decor = v.decor().clone();
                    *v = toml_edit::Value::from(new);
                    *v.decor_mut() = decor;
                }
                None => {
                    forms_tbl.insert(key, value(new));
                }
            }
            changed = true;
        }
    }

    if !changed {
        return Ok(());
    }
    std::fs::write(manifest, doc.to_string())
        .map_err(|e| format!("the project file cannot be written ({e})"))
}

/// One `[[crates]]` entry as a pin (spec 044 R8); `None` for a table that
/// names no crate or no version, which a build would not accept either.
fn pin_from_table(table: &toml_edit::Table) -> Option<ExternalCrate> {
    let text = |key: &str| table.get(key).and_then(|v| v.as_str()).map(str::to_owned);
    Some(ExternalCrate {
        name: text("name")?,
        requirement: text("requirement").unwrap_or_default(),
        version: text("version")?,
        features: table
            .get("features")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
            .unwrap_or_default(),
        url: text("url").unwrap_or_default(),
        alias: text("alias"),
    })
}

/// The project's External Crates as the manifest at `manifest` pins them, in
/// file order (spec 044 R8).
pub fn read_crate_pins(manifest: &Path) -> Result<Vec<ExternalCrate>, String> {
    let text = std::fs::read_to_string(manifest)
        .map_err(|e| format!("the project file cannot be read ({e})"))?;
    let doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e| format!("the project file cannot be parsed ({e})"))?;
    let Some(tables) = doc.get("crates").and_then(|item| item.as_array_of_tables()) else {
        return Ok(Vec::new());
    };
    tables
        .iter()
        .map(|t| pin_from_table(t).ok_or_else(|| "a [[crates]] entry has no name or no version".to_owned()))
        .collect()
}

/// Record `pins` as the manifest's `[[crates]]` — the whole list, replacing what
/// was there (spec 044 R8, R19).
///
/// **The developer's project file is theirs**, as for [`record_in_manifest`]:
/// the edit goes through `toml_edit`, a pin that is already there unchanged
/// keeps its own table — comments and all — and every other byte is written
/// back exactly as it was read. Nothing is written when nothing changed, and an
/// empty list removes the key.
pub fn write_crate_pins(manifest: &Path, pins: &[ExternalCrate]) -> Result<(), String> {
    use toml_edit::{value, Array, ArrayOfTables, DocumentMut, Item, Table};

    let text = std::fs::read_to_string(manifest)
        .map_err(|e| format!("the project file cannot be read ({e})"))?;
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e| format!("the project file cannot be parsed ({e})"))?;
    let existing: Vec<Table> = doc
        .get("crates")
        .and_then(|item| item.as_array_of_tables())
        .map(|a| a.iter().cloned().collect())
        .unwrap_or_default();

    if pins.is_empty() {
        doc.remove("crates");
    } else {
        let mut list = ArrayOfTables::new();
        for pin in pins {
            let kept = existing.iter().find(|t| pin_from_table(t).as_ref() == Some(pin));
            list.push(kept.cloned().unwrap_or_else(|| {
                let mut t = Table::new();
                t.insert("name", value(pin.name.clone()));
                t.insert("requirement", value(pin.requirement.clone()));
                t.insert("version", value(pin.version.clone()));
                let mut features = Array::new();
                for f in &pin.features {
                    features.push(f.as_str());
                }
                t.insert("features", value(features));
                t.insert("url", value(pin.url.clone()));
                if let Some(alias) = &pin.alias {
                    t.insert("alias", value(alias.clone()));
                }
                t
            }));
        }
        doc.insert("crates", Item::ArrayOfTables(list));
    }

    let out = doc.to_string();
    if out == text {
        return Ok(());
    }
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

    /// The developer's project file is theirs: comments, key order, blank
    /// lines and unrelated tables survive a record byte for byte; only the
    /// edited keys change, and the seal they carry verifies.
    #[test]
    fn host_keeps_every_byte_it_does_not_own() {
        let dir = tmp("bytes");
        form(&dir, "MAIN", true);
        form(&dir, "ORDERS", false);
        let manifest = dir.join("Shop.project.toml");
        let before = "\
# Shop — the developer's own notes, kept at the top.
[project]
version = \"2.1.0\"   # bumped by hand
name = \"Shop\"
main = \"\"
structure = 1

# The zebra table sorts last alphabetically but sits here on purpose.
[zebra]
stripes = 12

[files]
# forms first, my way
forms = [\"forms/MAIN.cfrm\"]   # the door
sources = [\"src/a.cbl\", \"generated/MAIN.cbl\"]

[forms]
theme = \"nord\"
main-form = \"STALE\"   # keep this comment
main-form-seal = \"0000\"

[ide]
theme = \"dark-glass\"
";
        std::fs::write(&manifest, before).unwrap();
        record_in_manifest(&manifest, "forms/ORDERS.cfrm", FileList::Forms).unwrap();
        record_in_manifest(&manifest, "generated/MAIN.cbl", FileList::Generated).unwrap();
        let after = std::fs::read_to_string(&manifest).unwrap();

        let d = cobolt_compiler::main_form_guard::read_designation(
            &dir,
            &["forms/MAIN.cfrm".to_owned(), "forms/ORDERS.cfrm".to_owned()],
        )
        .unwrap()
        .unwrap();
        let seal = cobolt_compiler::main_form_guard::seal("Shop", &d.main_form_id, &d.form_ids);
        // What the file must be: the original with exactly the owned keys edited.
        let expected = before
            .replace(
                "forms = [\"forms/MAIN.cfrm\"]   # the door",
                "forms = [\"forms/MAIN.cfrm\", \"forms/ORDERS.cfrm\"]   # the door",
            )
            .replace(
                "sources = [\"src/a.cbl\", \"generated/MAIN.cbl\"]",
                "sources = [\"src/a.cbl\"]\ngenerated = [\"generated/MAIN.cbl\"]",
            )
            .replace("main-form = \"STALE\"", "main-form = \"MAIN\"")
            .replace("main-form-seal = \"0000\"", &format!("main-form-seal = \"{seal}\""));
        assert_eq!(after, expected, "only the owned keys may change");
        assert_eq!(
            authorize_form_start(&dir.join("forms/MAIN.cfrm"), None),
            StartVerdict::Allowed,
            "the written seal verifies"
        );
        let kept = before.lines().filter(|l| after.lines().any(|a| a == *l)).count();
        let edited = before.lines().count() - kept;
        println!(
            "host: {} bytes in, {} bytes out; {kept} of {} lines byte-identical, {edited} edited \
             (forms, sources, main-form, main-form-seal), 1 added (generated); seal verifies",
            before.len(),
            after.len(),
            before.lines().count()
        );
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

#[cfg(test)]
mod crate_pin_tests {
    use super::*;

    fn pin(name: &str, version: &str, alias: Option<&str>) -> ExternalCrate {
        ExternalCrate {
            name: name.into(),
            requirement: "1".into(),
            version: version.into(),
            features: vec!["rt".into()],
            url: format!("https://crates.io/crates/{name}"),
            alias: alias.map(str::to_owned),
        }
    }

    fn manifest(tag: &str, text: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prc-crates-pins-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Zap.project.toml");
        std::fs::write(&path, text).unwrap();
        path
    }

    const BEFORE: &str = "\
# Zap — the developer's own notes.
[project]
name = \"Zap\"   # by hand
main = \"\"
structure = 1

[files]
forms = [\"forms/MAIN.cfrm\"]

# Registered by hand-written note: keep this comment with its crate.
[[crates]]
name = \"csv\"
requirement = \"1\"
version = \"1.4.0\"
features = [\"rt\"]
url = \"https://crates.io/crates/csv\"

[zebra]
stripes = 12
";

    /// A pin is read the way the IDE's project model writes it, an alias and
    /// features included; a manifest with none reads as empty.
    #[test]
    fn pins_are_read_as_the_project_model_writes_them() {
        let path = manifest("read", BEFORE);
        assert_eq!(read_crate_pins(&path).unwrap(), [pin("csv", "1.4.0", None)]);
        let none = manifest("none", "[project]\nname = \"Zap\"\n");
        assert!(read_crate_pins(&none).unwrap().is_empty());
        let bad = manifest("bad", "[[crates]]\nrequirement = \"1\"\n");
        assert!(read_crate_pins(&bad).is_err(), "a pin with no name is an error, not a skipped row");
    }

    /// The developer's project file is theirs: adding a crate leaves every byte
    /// it does not own — comments, order, other tables — and the pin already
    /// there keeps its own comment.
    #[test]
    fn recording_a_pin_keeps_every_other_byte_and_the_pins_already_there() {
        let path = manifest("add", BEFORE);
        let mut pins = read_crate_pins(&path).unwrap();
        pins.push(pin("tokio", "1.40.0", None));
        write_crate_pins(&path, &pins).unwrap();

        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.starts_with(&BEFORE[..BEFORE.find("[zebra]").unwrap()]), "everything before the new pin is untouched:\n{after}");
        assert!(after.contains("# Registered by hand-written note: keep this comment with its crate."));
        assert!(after.contains("[zebra]\nstripes = 12"), "an unrelated table survives:\n{after}");
        assert_eq!(read_crate_pins(&path).unwrap(), pins, "what was written reads back");

        // Recording the same list again writes nothing.
        write_crate_pins(&path, &pins).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), after);
    }

    /// An aliased pin round-trips, and removing the last pin removes the key
    /// rather than leaving an empty table behind.
    #[test]
    fn an_alias_round_trips_and_an_empty_list_removes_the_key() {
        let path = manifest("alias", BEFORE);
        write_crate_pins(&path, &[pin("serde_json", "1.0.133", Some("sjson"))]).unwrap();
        assert_eq!(read_crate_pins(&path).unwrap(), [pin("serde_json", "1.0.133", Some("sjson"))]);
        write_crate_pins(&path, &[]).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("[[crates]]"), "{text}");
        assert!(text.contains("[zebra]"), "the rest is untouched:\n{text}");
    }
}
