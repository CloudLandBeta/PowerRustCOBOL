// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The coding-agent tools end to end (spec 080 AC5, AC6 regenerate half):
//! over `cobolt_mcp::serve` with a scripted client, and directly.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use cobolt_project_tools::{FileList, HeadlessHost, NoProject, ProjectHost, ProjectRoot, ProjectTools};
use serde_json::{json, Value};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/check_project");

/// A fresh copy of the error fixture.
fn fixture(name: &str) -> PathBuf {
    let dst = std::env::temp_dir().join(format!("prc-080-tools-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(Path::new(FIXTURE), &dst);
    dst
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap().flatten() {
        let to = dst.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), to).unwrap();
        }
    }
}

/// Make the fixture clean: the handler moves into a declared item.
fn make_clean(dir: &Path) {
    let p = dir.join("forms/MAIN-FORM.cfrm");
    let xml = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, xml.replace("MOVE 1 TO WS-NOT-DECLARED.", "CONTINUE.")).unwrap();
}

fn call(tools: &mut ProjectTools<impl ProjectHost>, name: &str, args: Value) -> Result<Value, String> {
    tools.call(name, &args)
}

/// Drive `serve` with a scripted client; the replies, in order.
fn exchange(tools: &mut ProjectTools<impl ProjectHost>, requests: &[Value]) -> Vec<Value> {
    let mut input = Vec::new();
    for r in requests {
        input.extend_from_slice(r.to_string().as_bytes());
        input.push(b'\n');
    }
    let mut out = Vec::new();
    cobolt_mcp::serve(&mut Cursor::new(input), &mut out, tools).unwrap();
    String::from_utf8(out)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// The text of a tools/call reply, and whether it was a tool error.
fn text_of(reply: &Value) -> (String, bool) {
    let r = &reply["result"];
    (
        r["content"][0]["text"].as_str().unwrap_or_default().to_owned(),
        r["isError"].as_bool().unwrap_or(false),
    )
}

#[test]
fn check_reports_the_form_site_and_line_inside_the_handler() {
    let dir = fixture("check");
    let mut tools = ProjectTools::new(HeadlessHost::new(&dir, "test"));
    let v = call(&mut tools, "check", json!({})).unwrap();
    let diags = v["diagnostics"].as_array().unwrap();
    let errors: Vec<&Value> = diags.iter().filter(|d| d["severity"] == "error").collect();
    assert_eq!(v["errors"].as_u64().unwrap() as usize, errors.len());
    let planted = errors
        .iter()
        .find(|d| d["message"].as_str().unwrap().contains("WS-NOT-DECLARED"))
        .unwrap_or_else(|| panic!("the planted error is reported: {v:#}"));
    assert_eq!(planted["file"], "forms/MAIN-FORM.cfrm", "{planted}");
    assert_eq!(planted["form"], "MAIN-FORM");
    assert_eq!(planted["site"], "MAIN-FORM ▸ BTN-OK ▸ onClick", "{planted}");
    assert_eq!(planted["line"], 4, "line 4 of the handler's own text: {planted}");
    assert_eq!(v["checked"], json!({"forms": 1, "indexed": 0, "sources": 1}));
    // Nothing was written: check validates in memory.
    assert!(!dir.join("generated").exists(), "check wrote generated code");
    println!(
        "check (error fixture): {} diagnostic(s), {} error(s); planted error at {} ▸ line {}",
        diags.len(),
        errors.len(),
        planted["site"],
        planted["line"]
    );

    // One file.
    let one = call(&mut tools, "check", json!({"path": "forms/MAIN-FORM.cfrm"})).unwrap();
    assert_eq!(one["errors"], v["errors"]);
    let src = call(&mut tools, "check", json!({"path": "src/helper.cbl"})).unwrap();
    assert_eq!(src["errors"], 0, "{src}");

    // Clean.
    make_clean(&dir);
    let clean = call(&mut tools, "check", json!({})).unwrap();
    assert_eq!(clean["errors"], 0, "{clean:#}");
    println!("check (clean fixture): 0 errors; one-file check of the form and of src/helper.cbl agree");
}

#[test]
fn check_reports_a_second_main_form() {
    let dir = fixture("two-mains");
    make_clean(&dir);
    let mut second = cobolt_forms::Form::new("SECOND", "SECOND", 300, 200);
    second.main_form = true;
    cobolt_forms::save_form(&second, &dir.join("forms/SECOND.cfrm")).unwrap();
    let mut tools = ProjectTools::new(HeadlessHost::new(&dir, "test"));
    call(&mut tools, "add_to_project", json!({"path": "forms/SECOND.cfrm"})).unwrap();
    let v = call(&mut tools, "check", json!({})).unwrap();
    let main_err = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["message"].as_str().unwrap().contains("main form"))
        .unwrap_or_else(|| panic!("a double designation is an error: {v:#}"));
    assert_eq!(main_err["severity"], "error");
    assert_eq!(main_err["file"], "CheckDemo.project.toml");
    println!("check: a second main form → error on the manifest: {}", main_err["message"]);
}

#[test]
fn regenerate_writes_what_the_ide_generator_writes() {
    let dir = fixture("regenerate");
    let mut tools = ProjectTools::new(HeadlessHost::new(&dir, "test"));
    let v = call(&mut tools, "regenerate", json!({"path": "forms/MAIN-FORM.cfrm"})).unwrap();
    assert_eq!(v["written"], json!(["generated/MAIN-FORM.cbl"]));
    let form = cobolt_forms::load_form(&dir.join("forms/MAIN-FORM.cfrm")).unwrap();
    let expected = cobolt_codegen::generate_with_map(&form).0;
    let root = ProjectRoot::open(&dir).unwrap();
    let path = cobolt_project_tools::gen_paths::generated_cbl_path(
        Some(&[]),
        Some(root.dir()),
        &root.dir().join("forms/MAIN-FORM.cfrm"),
    );
    let written = std::fs::read_to_string(&path).unwrap();
    assert_eq!(written, expected, "byte-equal to the IDE's generator");
    let view = cobolt_compiler::project_manifest_view(&dir.join("CheckDemo.project.toml")).unwrap();
    assert_eq!(view.generated, ["generated/MAIN-FORM.cbl"], "recorded as generated");

    // An indexed definition: facade + two copybooks.
    std::fs::create_dir_all(dir.join("indexed")).unwrap();
    let def = sample_indexed();
    cobolt_indexed::save_indexed(dir.join("indexed/actors.cidx"), &def).unwrap();
    call(&mut tools, "add_to_project", json!({"path": "indexed/actors.cidx"})).unwrap();
    let v = call(&mut tools, "regenerate", json!({"path": "indexed/actors.cidx"})).unwrap();
    assert_eq!(
        v["written"],
        json!(["generated/actors-indexed.cbl", "COPYBOOKS/actors.SEL", "COPYBOOKS/actors.FD"])
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("generated/actors-indexed.cbl")).unwrap(),
        cobolt_codegen::generate_indexed(&def)
    );
    // All: every form and indexed definition.
    let all = call(&mut tools, "regenerate", json!({})).unwrap();
    assert_eq!(all["written"].as_array().unwrap().len(), 4);
    println!(
        "regenerate: form → {} bytes byte-equal to generate_with_map; indexed → 3 files; all → {} files",
        written.len(),
        all["written"].as_array().unwrap().len()
    );
}

fn sample_indexed() -> cobolt_indexed::IndexedDefinition {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<IndexedFile name="ACTORS-FILE" finalized="true" version="1.0">
  <assign-path>actors.idx</assign-path>
  <access-mode>dynamic</access-mode>
  <record-format fixed-length="46"/>
  <storage mode="disk" compression="false" persistence="false"/>
  <keys>
    <primary duplicates="false" ordering="ascending"><part field="ACTOR-ID" offset="0" length="6" encoding="bytes"/></primary>
  </keys>
  <fields>
    <Field level="1" name="ACTOR-RECORD" usage="display">
      <Field level="5" name="ACTOR-ID" pic="9(6)" usage="display" offset="0" length="6"/>
      <Field level="5" name="ACTOR-NAME" pic="X(40)" usage="display" offset="6" length="40"/>
    </Field>
  </fields>
</IndexedFile>
"#;
    cobolt_indexed::load_indexed_from_str(xml).unwrap_or_else(|e| panic!("sample .cidx: {e}"))
}

/// A host that reports one file as unsaved in the IDE.
struct Unsaved {
    inner: HeadlessHost,
    dirty: PathBuf,
}

impl ProjectHost for Unsaved {
    fn project(&self) -> Result<ProjectRoot, NoProject> {
        self.inner.project()
    }
    fn record(&mut self, rel: &str, list: FileList) -> Result<(), String> {
        self.inner.record(rel, list)
    }
    fn unsaved(&self, abs: &Path) -> bool {
        std::fs::canonicalize(abs).ok() == std::fs::canonicalize(&self.dirty).ok()
    }
    fn external_crates(&self) -> Vec<String> {
        Vec::new()
    }
    fn version(&self) -> String {
        "test".into()
    }
}

#[test]
fn regenerate_refuses_an_unsaved_target_and_writes_nothing() {
    let dir = fixture("unsaved");
    let host = Unsaved {
        inner: HeadlessHost::new(&dir, "test"),
        dirty: dir.join("forms/MAIN-FORM.cfrm"),
    };
    let before = std::fs::read_to_string(dir.join("CheckDemo.project.toml")).unwrap();
    let mut tools = ProjectTools::new(host);
    let err = call(&mut tools, "regenerate", json!({})).unwrap_err();
    assert!(err.contains("unsaved"), "{err}");
    assert!(!dir.join("generated").exists(), "nothing written");
    assert_eq!(std::fs::read_to_string(dir.join("CheckDemo.project.toml")).unwrap(), before);
    println!("regenerate: unsaved target refused (\"{err}\"), 0 files written, manifest unchanged");
}

#[test]
fn register_adds_a_form_and_the_seal_still_verifies() {
    let dir = fixture("register");
    make_clean(&dir);
    // Seal the project first, as an IDE save would have.
    let mut tools = ProjectTools::new(HeadlessHost::new(&dir, "test"));
    call(&mut tools, "add_to_project", json!({"path": "forms/MAIN-FORM.cfrm"})).unwrap();
    let other = cobolt_forms::Form::new("ORDERS", "Orders", 300, 200);
    cobolt_forms::save_form(&other, &dir.join("forms/ORDERS.cfrm")).unwrap();
    let v = call(&mut tools, "add_to_project", json!({"path": "forms/ORDERS.cfrm"})).unwrap();
    assert_eq!(v, json!({"added": "forms/ORDERS.cfrm", "list": "forms", "project": "CheckDemo.project.toml"}));
    let view = cobolt_compiler::project_manifest_view(&dir.join("CheckDemo.project.toml")).unwrap();
    assert_eq!(view.forms, ["forms/MAIN-FORM.cfrm", "forms/ORDERS.cfrm"]);
    use cobolt_compiler::main_form_guard::{authorize_form_start, StartVerdict};
    assert_eq!(
        authorize_form_start(&dir.join("forms/MAIN-FORM.cfrm"), None),
        StartVerdict::Allowed,
        "{}",
        std::fs::read_to_string(dir.join("CheckDemo.project.toml")).unwrap()
    );
    assert!(matches!(
        authorize_form_start(&dir.join("forms/ORDERS.cfrm"), None),
        StartVerdict::Refused { .. }
    ));
    // Refusals: missing file, generated list, outside path.
    assert!(call(&mut tools, "add_to_project", json!({"path": "forms/NOPE.cfrm"})).is_err());
    assert!(call(&mut tools, "add_to_project", json!({"path": "src/helper.cbl", "list": "generated"})).is_err());
    assert!(call(&mut tools, "add_to_project", json!({"path": "../x.cfrm"})).is_err());
    println!("register: ORDERS added to forms, seal verifies (MAIN Allowed, ORDERS Refused), 3 refusals");
}

#[test]
fn tools_over_serve_list_check_refuse_paths_and_answer_no_project() {
    let dir = fixture("serve");
    let mut tools = ProjectTools::new(HeadlessHost::new(&dir, "1.2.3"));
    let replies = exchange(
        &mut tools,
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"check","arguments":{}}}),
            json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"validate","arguments":{"path":"../outside.cfrm"}}}),
            json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"check","arguments":{"path":"/etc/passwd"}}}),
        ],
    );
    assert_eq!(replies.len(), 5, "the notification is not answered");
    assert_eq!(replies[0]["result"]["serverInfo"]["version"], "1.2.3");
    let names: Vec<&str> = replies[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["list_files", "check", "regenerate", "add_to_project", "build", "validate", "kb_lookup", "render_form", "run_form", "add_powerchat", "create_project", "open_project", "kb_search"]
    );
    let (check_text, is_err) = text_of(&replies[2]);
    assert!(!is_err);
    let check: Value = serde_json::from_str(&check_text).unwrap();
    assert!(check["errors"].as_u64().unwrap() >= 1);
    assert!(check_text.contains("\"file\":\"forms/MAIN-FORM.cfrm\""), "{check_text}");
    assert!(check_text.contains("\"line\":4"), "{check_text}");
    for r in &replies[3..5] {
        let (t, is_err) = text_of(r);
        assert!(is_err, "a path outside the project is refused: {t}");
        assert!(!t.contains(&*std::env::temp_dir().to_string_lossy()), "no machine path in a refusal: {t}");
    }
    println!(
        "serve: initialize + tools/list ({} tools) + check (file + line 4) + 2 path refusals",
        names.len()
    );

    // No project: every PROJECT tool answers "no project open"; the knowledge
    // tools need no project and still answer (spec 084).
    let mut none = ProjectTools::new(HeadlessHost::new(dir.join("missing.project.toml"), "x"));
    let (mut refused, mut knowledge) = (0, 0);
    for name in names.iter().filter(|n| **n != "create_project" && **n != "open_project") {
        let reply = exchange(
            &mut none,
            &[json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":{"path":"forms/MAIN-FORM.cfrm","name":"Button","query":"Button Caption"}}})],
        );
        let (t, is_err) = text_of(&reply[0]);
        if name.starts_with("kb_") {
            assert!(!is_err && t.contains("\"found\":true"), "{name} answers without a project: {t}");
            knowledge += 1;
        } else {
            assert!(is_err && t == "no project open", "{name}: {t}");
            refused += 1;
        }
    }
    assert_eq!((refused, knowledge), (9, 2));
    println!("serve: no project → {refused} project tools answered \"no project open\", {knowledge} knowledge tools answered");
}

#[test]
fn list_files_and_validate_through_the_tool_set() {
    let dir = fixture("list-validate");
    let mut tools = ProjectTools::new(HeadlessHost::new(&dir, "test"));
    let v = call(&mut tools, "list_files", json!({})).unwrap();
    assert_eq!(v["files"]["forms"], json!([{"path":"forms/MAIN-FORM.cfrm","exists":true}]));
    let ok = call(&mut tools, "validate", json!({"path": "forms/MAIN-FORM.cfrm"})).unwrap();
    assert_eq!(ok["valid"], true);
    std::fs::write(dir.join("forms/BROKEN.cfrm"), "<Form name=\"X\"><Control").unwrap();
    let bad = call(&mut tools, "validate", json!({"path": "forms/BROKEN.cfrm"})).unwrap();
    assert_eq!(bad["valid"], false, "{bad}");
    std::fs::create_dir_all(dir.join("indexed")).unwrap();
    cobolt_indexed::save_indexed(dir.join("indexed/a.cidx"), &sample_indexed()).unwrap();
    let cidx = call(&mut tools, "validate", json!({"path": "indexed/a.cidx"})).unwrap();
    assert_eq!(cidx["valid"], true, "{cidx}");
    println!("list_files + validate: 1 form listed; valid form, broken form, valid .cidx (3 verdicts)");
}

/// Spec 084 AC10 (R19, R20): every project-tool answer names its project; a
/// call naming the open project — by folder, a folder inside it, its manifest
/// or its name — runs; a call naming another project is refused with both
/// names, and touches nothing.
#[test]
fn every_answer_names_its_project_and_another_project_is_refused() {
    let dir = fixture("project-arg");
    let mut tools = ProjectTools::new(HeadlessHost::new(&dir, "test"));
    let manifest = "CheckDemo.project.toml";
    for (tool, args) in [
        ("list_files", json!({})),
        ("check", json!({})),
        ("validate", json!({"path": "forms/MAIN-FORM.cfrm"})),
    ] {
        let v = call(&mut tools, tool, args).unwrap();
        let named = v["project"].as_str().unwrap_or_default();
        assert!(named == manifest || named == "CheckDemo", "{tool} names its project: {v}");
    }
    let forms = dir.join("forms");
    let accepted = [
        dir.to_string_lossy().into_owned(),
        forms.to_string_lossy().into_owned(),
        dir.join(manifest).to_string_lossy().into_owned(),
        "CheckDemo".to_owned(),
        manifest.to_owned(),
    ];
    for asked in &accepted {
        assert!(call(&mut tools, "list_files", json!({"project": asked})).is_ok(), "accepted: {asked}");
    }
    let elsewhere = std::env::temp_dir().join(format!("prc-084-elsewhere-{}", std::process::id()));
    std::fs::create_dir_all(&elsewhere).unwrap();
    let refused = [elsewhere.to_string_lossy().into_owned(), "PowerChat".to_owned()];
    for asked in &refused {
        let e = call(&mut tools, "check", json!({"project": asked})).unwrap_err();
        assert!(e.contains(asked.as_str()) && e.contains(manifest), "both names in the refusal: {e}");
    }
    // The schema offers the argument on project tools, not on knowledge tools.
    let listed = ProjectTools::<HeadlessHost>::tool_list_with_project();
    for t in &listed {
        let has = t.input_schema["properties"].get("project").is_some();
        assert_eq!(has, !cobolt_project_tools::tools::project_free(&t.name), "{}", t.name);
    }
    let _ = std::fs::remove_dir_all(&elsewhere);
    let _ = std::fs::remove_dir_all(&dir);
    println!(
        "project naming: 3 answers named, {} ways of naming the open project accepted, {} other projects refused with both names",
        accepted.len(),
        refused.len()
    );
}

/// Spec 084 AC8 (R15, R16, R18): `create_project` makes, in a new folder, a
/// project `check` accepts and the tools then act on; it refuses a folder that
/// holds anything and changes nothing there; `open_project` switches back.
#[test]
fn create_project_makes_a_checkable_project_and_open_project_switches() {
    let first = fixture("create-first");
    let mut tools = ProjectTools::new(HeadlessHost::new(&first, "test"));
    let base = std::env::temp_dir().join(format!("prc-084-create-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let fresh = base.join("Inventory");

    let made = call(&mut tools, "create_project", json!({"folder": fresh.to_string_lossy(), "name": "Inventory"})).unwrap();
    assert_eq!(made["project"], "Inventory.project.toml");
    // The golden rules: a project an agent creates wears Spatial, for the IDE
    // and as the forms' default.
    let manifest: toml_edit::DocumentMut = std::fs::read_to_string(fresh.join("Inventory.project.toml")).unwrap().parse().unwrap();
    assert_eq!((manifest["ide"]["theme"].as_str(), manifest["forms"]["theme"].as_str()), (Some("spatial"), Some("spatial")));
    assert!(fresh.join("Inventory.project.toml").is_file() && fresh.join("src/main.cbl").is_file());
    for sub in cobolt_project_tools::create::PROJECT_FOLDERS {
        assert!(fresh.join(sub).is_dir(), "{sub}/ created");
    }
    let listed = call(&mut tools, "list_files", json!({})).unwrap();
    assert_eq!(listed["manifest"], "Inventory.project.toml", "the tools now act on the new project");
    let checked = call(&mut tools, "check", json!({})).unwrap();
    assert_eq!(checked["errors"], 0, "a new project checks clean: {checked}");

    // A folder holding anything is refused, and left exactly as it was.
    let busy = base.join("busy");
    std::fs::create_dir_all(&busy).unwrap();
    std::fs::write(busy.join("notes.txt"), "keep me").unwrap();
    assert!(call(&mut tools, "create_project", json!({"folder": busy.to_string_lossy(), "name": "X"})).is_err());
    let entries: Vec<_> = std::fs::read_dir(&busy).unwrap().collect();
    assert_eq!(entries.len(), 1, "nothing written into a non-empty folder");
    assert_eq!(std::fs::read_to_string(busy.join("notes.txt")).unwrap(), "keep me");
    assert!(call(&mut tools, "create_project", json!({"folder": "relative/x", "name": "X"})).is_err(), "relative refused");

    // Open the first project again.
    let opened = call(&mut tools, "open_project", json!({"path": first.to_string_lossy()})).unwrap();
    assert_eq!(opened["project"], "CheckDemo.project.toml");
    assert_eq!(call(&mut tools, "list_files", json!({})).unwrap()["manifest"], "CheckDemo.project.toml");
    assert!(call(&mut tools, "open_project", json!({"path": busy.to_string_lossy()})).is_err(), "no project there");

    let _ = std::fs::remove_dir_all(&base);
    let _ = std::fs::remove_dir_all(&first);
    println!(
        "create_project: manifest + {} folders + main, check 0 errors, tools switched; non-empty and relative folders refused untouched; open_project switched back",
        cobolt_project_tools::create::PROJECT_FOLDERS.len()
    );
}

/// Spec 084 R30: `render_form` hands its form to the injected renderer and
/// answers with image content plus what the picture is; a non-form and a
/// server without a renderer are refused.
#[test]
fn render_form_answers_with_an_image() {
    use cobolt_mcp::McpHandler;
    let dir = fixture("render");
    let stub: cobolt_project_tools::tools::render::Renderer = std::sync::Arc::new(|cfrm: &Path, project: &Path, picture: &cobolt_project_tools::tools::render::Picture| {
        assert!(cfrm.ends_with("forms/MAIN-FORM.cfrm") && cfrm.starts_with(project));
        // A form whose minimum is 300x200, designed at that.
        let [w, h] = picture.window.map(|[w, h]| [w.max(300.0), h.max(200.0)]).unwrap_or([300.0, 200.0]);
        Ok((vec![0x89, b'P', b'N', b'G'], [(w * picture.scale) as usize, (h * picture.scale) as usize]))
    });
    let shared = std::sync::Arc::new(cobolt_project_tools::tools::Shared::new().with_renderer(stub));
    let mut tools = ProjectTools::with_shared(HeadlessHost::new(&dir, "test"), shared);
    let r = tools.call_tool("render_form", &json!({"path": "forms/MAIN-FORM.cfrm", "scale": 2}));
    assert_eq!(r.is_error, None);
    assert_eq!(serde_json::to_value(&r.content[0]).unwrap()["type"], "image");
    let meta: Value = serde_json::from_str(r.content[1].as_text().unwrap()).unwrap();
    assert_eq!((meta["width"].as_u64(), meta["height"].as_u64()), (Some(600), Some(400)));
    assert_eq!(meta["project"], "CheckDemo.project.toml");
    assert!(meta.get("window").is_none(), "no window asked, none reported");
    // In a window of another size: laid out there, or held at the minimum.
    let r = tools.call_tool("render_form", &json!({"path": "forms/MAIN-FORM.cfrm", "width": 1200, "height": 700}));
    let meta: Value = serde_json::from_str(r.content[1].as_text().unwrap()).unwrap();
    assert_eq!(meta["window"], json!([1200.0, 700.0]));
    assert!(meta.get("held_at_minimum").is_none());
    let r = tools.call_tool("render_form", &json!({"path": "forms/MAIN-FORM.cfrm", "width": 100, "height": 100}));
    let meta: Value = serde_json::from_str(r.content[1].as_text().unwrap()).unwrap();
    assert_eq!(meta["window"], json!([300.0, 200.0]));
    assert!(meta["held_at_minimum"].as_str().unwrap().contains("300x200"));
    assert_eq!(tools.call_tool("render_form", &json!({"path": "src/main.cbl"})).is_error, Some(true), "not a form");
    let mut bare = ProjectTools::new(HeadlessHost::new(&dir, "test"));
    let r = bare.call_tool("render_form", &json!({"path": "forms/MAIN-FORM.cfrm"}));
    assert!(r.is_error == Some(true) && r.content[0].as_text().unwrap().contains("not available"));
    let _ = std::fs::remove_dir_all(&dir);
    println!("render_form: image + meta (600x400 at scale 2, project named); non-form and renderer-less server refused");
}

/// Spec 084 AC20 (R33): the patterns pack is listed and readable as
/// resources — at least the five named patterns — and every pattern, put in
/// a project of its own, loads and passes `check`.
#[test]
fn every_pattern_is_served_and_passes_check_in_a_project_of_its_own() {
    use cobolt_mcp::McpHandler;
    use cobolt_project_tools::patterns::{PATTERNS, PATTERN_PREFIX};
    let start = fixture("patterns");
    let mut tools = ProjectTools::new(HeadlessHost::new(&start, "test"));
    let listed: Vec<String> = tools.list_resources().into_iter().map(|r| r.uri).collect();
    for name in ["README.md", "application-shell", "contentpane-form", "indexed-maintenance", "rest-call", "agent-chat"] {
        let uri = format!("{PATTERN_PREFIX}{name}");
        assert!(listed.contains(&uri), "{uri} listed");
        let doc = tools.read_resource(&uri).expect("readable").text;
        assert!(doc.starts_with("# "), "{uri}: a document");
    }

    let base = std::env::temp_dir().join(format!("prc-084-patterns-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    for p in PATTERNS {
        let dir = base.join(p.name);
        call(&mut tools, "create_project", json!({"folder": dir.to_string_lossy(), "name": "Pattern"})).unwrap();
        for (rel, text) in p.files {
            let abs = dir.join(rel);
            std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
            std::fs::write(&abs, text).unwrap();
            if rel.ends_with(".yaml") {
                continue; // a menu sidecar travels beside its form, unregistered
            }
            call(&mut tools, "validate", json!({"path": rel})).unwrap_or_else(|e| panic!("{}: {rel} does not load: {e}", p.name));
            call(&mut tools, "add_to_project", json!({"path": rel})).unwrap_or_else(|e| panic!("{}: add {rel}: {e}", p.name));
        }
        // The .cidx first: a form bound to it reads its generated copybooks.
        for (rel, _) in p.files.iter().filter(|(r, _)| r.ends_with(".cidx")).chain(p.files.iter().filter(|(r, _)| r.ends_with(".cfrm"))) {
            call(&mut tools, "regenerate", json!({"path": rel})).unwrap_or_else(|e| panic!("{}: regenerate {rel}: {e}", p.name));
        }
        let checked = call(&mut tools, "check", json!({})).unwrap();
        assert_eq!(checked["errors"], 0, "{}: {checked}", p.name);
        let forms = p.files.iter().filter(|(r, _)| r.ends_with(".cfrm")).count() as u64;
        assert_eq!(checked["checked"]["forms"].as_u64(), Some(forms), "{}: the form itself was checked: {checked}", p.name);
    }
    let _ = std::fs::remove_dir_all(&base);
    let _ = std::fs::remove_dir_all(&start);
    println!("patterns: {} served + index; each checks clean in a project of its own", PATTERNS.len());
}

/// Spec 084 AC19 (R34): the instructions carry the architecture section and
/// the map — and every tool and resource the map names is one the server
/// actually offers.
#[test]
fn the_instructions_map_only_real_tools_and_resources() {
    use cobolt_mcp::McpHandler;
    let dir = fixture("map");
    let mut tools = ProjectTools::new(HeadlessHost::new(&dir, "test"));
    let text = tools.instructions().unwrap();
    assert!(text.contains("How an application is built:") && text.contains("Where to look:"), "{text}");
    // The golden rules for every application, each one told in full.
    assert!(text.contains("Golden rules for every application you build:"), "{text}");
    for (id, rule) in cobolt_project_tools::content::APPLICATION_RULES {
        assert!(text.contains(rule), "the {id} rule is in the instructions");
    }
    let names: Vec<String> = tools.list_tools().into_iter().map(|t| t.name).collect();
    let uris: Vec<String> = tools.list_resources().into_iter().map(|r| r.uri).collect();
    let mut named = 0;
    for (_, answer) in cobolt_project_tools::content::RESOURCE_MAP {
        for word in answer.split([',', ' ', '(', ')']).filter(|w| !w.is_empty()) {
            if word.starts_with("powerrustcobol://") {
                assert!(uris.iter().any(|u| u == word), "{word} is served");
                named += 1;
            } else if word.contains('_') || ["check", "regenerate"].contains(&word) {
                if word == "in_shell" {
                    continue;
                }
                assert!(names.iter().any(|n| n == word), "{word} is a tool");
                named += 1;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    println!("instructions: architecture + map; {named} tools and resources named, every one offered");
}

/// Spec 085 AC3, AC4 (R6–R13): `add_powerchat` on a new project whose main
/// form carries a side menu — PowerChat's forms in their own folder, none of
/// them a main form or themed, the host's name on them, everything checking
/// clean, and an Assistant item opening it modeless; asked again, or over a
/// form with one of its names, it refuses and writes nothing.
#[test]
fn add_powerchat_makes_powerchat_the_applications_own() {
    let start = fixture("add-powerchat");
    let mut tools = ProjectTools::new(HeadlessHost::new(&start, "test"));
    let base = std::env::temp_dir().join(format!("prc-085-add-powerchat-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let project = base.join("Inventory");
    call(&mut tools, "create_project", json!({"folder": project.to_string_lossy(), "name": "Inventory"})).unwrap();
    let main = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="MAIN-FORM" title="Inventory Pro" width="1200" height="800" main-form="true">
  <Control id="SideMenu-1" type="SideMenu" x="0" y="0" w="240" h="800" tab-order="0" z-order="0" visible="true" enabled="true">
  </Control>
</Form>
"#;
    std::fs::write(project.join("forms/main-form.cfrm"), main).unwrap();
    let mut home = cobolt_forms::menu::MenuItem::new_action("home", "Home");
    home.action = Some("home".into());
    let def = cobolt_forms::menu::MenuDefinition { menu: vec![home], hash: String::new() };
    let menu_path = cobolt_forms::menu::menu_yaml_path(&project.join("forms"), "SideMenu-1");
    cobolt_forms::menu::save_menu(&menu_path, &def).unwrap();
    call(&mut tools, "add_to_project", json!({"path": "forms/main-form.cfrm"})).unwrap();
    call(&mut tools, "regenerate", json!({"path": "forms/main-form.cfrm"})).unwrap();

    let added = call(&mut tools, "add_powerchat", json!({})).unwrap();
    assert_eq!(added["check"]["errors"], 0, "everything checks clean: {}", added["check"]);
    assert_eq!(added["added"], 14);
    assert_eq!(added["branding"]["name"], "Inventory Pro");
    let menu = cobolt_forms::menu::load_menu(&menu_path).unwrap();
    let item = menu.menu.iter().find(|i| i.id == "assistant").expect("an Assistant item");
    assert_eq!(item.action.as_deref(), Some("open-standalone-async:chat-form"), "modeless, its own window");
    // Exactly one main form, none themed, the host's name on them.
    let listed = call(&mut tools, "list_files", json!({})).unwrap();
    let forms: Vec<String> = listed["files"]["forms"].as_array().map(|a| a.iter().filter_map(|v| v["path"].as_str().map(str::to_owned)).collect()).unwrap_or_default();
    assert_eq!(forms.len(), 15, "the host's form and PowerChat's 14: {listed}");
    let mains = forms.iter().filter(|f| cobolt_forms::load_form(&project.join(f)).unwrap().main_form).count();
    assert_eq!(mains, 1, "the host's main form stays the only one");
    let chat = std::fs::read_to_string(project.join("forms/powerchat/chat-form.cfrm")).unwrap();
    assert!(chat.contains("title=\"Inventory Pro\"") && !chat.contains("glass-style="));
    assert!(project.join("Assets/powerchat/flags/en.png").is_file() && project.join("samples/powerchat/main-prompt.md").is_file());

    // Again: refused, nothing changed.
    let before = std::fs::read_to_string(&menu_path).unwrap();
    assert!(call(&mut tools, "add_powerchat", json!({})).is_err());
    assert_eq!(std::fs::read_to_string(&menu_path).unwrap(), before);

    // A project with a form of one of PowerChat's names: refused, nothing written.
    let other = base.join("Clash");
    call(&mut tools, "create_project", json!({"folder": other.to_string_lossy(), "name": "Clash"})).unwrap();
    std::fs::write(other.join("forms/chat-form.cfrm"), main.replace("MAIN-FORM", "CHAT-FORM")).unwrap();
    call(&mut tools, "add_to_project", json!({"path": "forms/chat-form.cfrm"})).unwrap();
    let refused = call(&mut tools, "add_powerchat", json!({})).unwrap_err();
    assert!(refused.contains("forms/chat-form.cfrm"), "{refused}");
    assert!(!other.join("forms/powerchat").exists(), "nothing written");

    let _ = std::fs::remove_dir_all(&base);
    let _ = std::fs::remove_dir_all(&start);
    println!("add_powerchat: 14 forms + menu + 7 pictures + 3 defaults, check 0 errors, one main form, Assistant item modeless; second add and a name clash refused untouched");
}
