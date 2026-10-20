// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Project kinds through the coding-agent tools** (WASM-001 R10, WASM-015 R10,
//! R12): `create_project` with a kind and its options, `list_files`, `check`,
//! `add_to_project` and `build` on a Web or Mobile project.
//!
//! Every test prints its figures (GOLDEN RULE #7): what was exercised and how
//! long it took.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use cobolt_project_tools::web::{self, BuildReport, BundleSize, WebBuildBackend, WebBuildError, WebBuildRequest};
use cobolt_project_tools::{HeadlessHost, ProjectHost, ProjectTools};
use serde_json::{json, Value};

/// The web backend registry is process-wide; the tests that build take this.
static REGISTRY: Mutex<()> = Mutex::new(());

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-wasm001-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn call(tools: &mut ProjectTools<impl ProjectHost>, name: &str, args: Value) -> Result<Value, String> {
    tools.call(name, &args)
}

/// A tool set that starts with no project, as `rcrun mcp` does in a bare folder.
fn bare() -> ProjectTools<HeadlessHost> {
    ProjectTools::new(HeadlessHost::new(std::env::temp_dir().join("prc-wasm001-nothing-here"), "test"))
}

#[test]
fn every_creatable_template_makes_a_project_that_checks_clean() {
    let base = scratch("each");
    let mut tools = bare();
    let mut rows = Vec::new();
    for template in ["desktop", "web", "android-phone", "android-tablet", "iphone", "ipad"] {
        let folder = base.join(template);
        let t = Instant::now();
        let made = call(&mut tools, "create_project", json!({"folder": folder.to_string_lossy(), "name": "Shop", "template": template})).unwrap();
        let created_ms = t.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(made["template"], template);
        let kind = made["kind"].as_str().unwrap().to_owned();

        // The manifest says what it is.
        let manifest = folder.join(made["project"].as_str().unwrap());
        let view = cobolt_compiler::project_manifest_view(&manifest).unwrap();
        assert_eq!(view.kind.as_str(), kind);
        let backend = view.kind.has_backend();
        assert_eq!(view.backend.is_empty(), !backend, "{template}: Backend files only in a Web or Mobile project");
        assert_eq!(view.web.is_some(), view.kind == cobolt_compiler::ProjectKind::Web, "{template}");
        assert_eq!(view.mobile.is_some(), view.kind == cobolt_compiler::ProjectKind::Mobile, "{template}");
        assert_eq!(view.backend_exports.is_empty(), !backend, "{template}");

        // The folders of the kind are there, and a Web or Mobile project has no forms/ or bin/.
        for sub in cobolt_project_tools::create::folders_for(view.kind, backend) {
            assert!(folder.join(&sub).is_dir(), "{template}: {sub}/");
        }
        if view.kind != cobolt_compiler::ProjectKind::Desktop {
            assert!(!folder.join("forms").exists() && !folder.join("bin").exists(), "{template}");
            assert_eq!(view.forms, [format!("{}/main-form.cfrm", view.kind.forms_folder())]);
            assert!(folder.join(&view.forms[0]).is_file());
            // The designation is sealed: the runtime accepts the main form.
            assert_eq!(
                cobolt_compiler::main_form_guard::authorize_form_start(&folder.join(&view.forms[0]), None),
                cobolt_compiler::main_form_guard::StartVerdict::Allowed,
                "{template}"
            );
            // The sample form loads, is the main form and has its click handler.
            let form = cobolt_forms::load_form(&folder.join(&view.forms[0])).unwrap();
            assert!(form.main_form && form.responsive);
            let greet = form.controls.iter().find(|c| c.id == "BTN-GREET").expect("a button");
            assert!(greet.events.iter().any(|e| e.event == "onClick" && e.code.contains("Hello, ")), "{template}: a click handler");
        }

        // The tools act on it now, and it checks clean.
        let t = Instant::now();
        let listed = call(&mut tools, "list_files", json!({})).unwrap();
        assert_eq!(listed["kind"], kind);
        let checked = call(&mut tools, "check", json!({})).unwrap();
        let check_ms = t.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(checked["errors"], 0, "{template} checks clean: {checked}");
        rows.push(format!(
            "    {template:<15} {kind:<8} {:>2} folders  {:>4.1} ms to create  {:>6.1} ms to list and check  {} form, {} backend",
            made["folders"].as_array().unwrap().len(),
            created_ms,
            check_ms,
            view.forms.len(),
            view.backend.len()
        ));
    }
    println!("\n  create_project, one project of each creatable template, each checked clean:\n{}", rows.join("\n"));
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn the_kind_and_its_options_pick_the_template_and_shape_the_project() {
    let base = scratch("options");
    let mut tools = bare();
    let make = |tools: &mut ProjectTools<HeadlessHost>, tag: &str, args: Value| -> Result<(PathBuf, Value), String> {
        let folder = base.join(tag);
        let mut a = args;
        a["folder"] = json!(folder.to_string_lossy());
        a["name"] = json!("App");
        call(tools, "create_project", a).map(|v| (folder, v))
    };
    let manifest_of = |folder: &PathBuf| -> toml_edit::DocumentMut {
        let name = std::fs::read_dir(folder).unwrap().flatten().find(|e| e.file_name().to_string_lossy().ends_with(".project.toml")).unwrap();
        std::fs::read_to_string(name.path()).unwrap().parse().unwrap()
    };

    // `kind: web` is the web template; a standalone application style is recorded.
    let (f, made) = make(&mut tools, "web", json!({"kind": "web", "options": {"style": "standalone"}})).unwrap();
    assert_eq!(made["template"], "web");
    let doc = manifest_of(&f);
    assert_eq!(doc["project"]["kind"].as_str(), Some("web"));
    assert_eq!(doc["web"]["style"].as_str(), Some("standalone"));
    assert_eq!(doc["web"]["isolation"].as_str(), Some("require-corp"));
    assert_eq!(doc["backend"]["exports"][0].as_str(), Some("HEALTH"));

    // `kind: mobile` needs a platform and a device; with them it is that template.
    let e = make(&mut tools, "m0", json!({"kind": "mobile"})).unwrap_err();
    assert!(e.contains("platform"), "{e}");
    let e = make(&mut tools, "m1", json!({"kind": "mobile", "platform": "ios"})).unwrap_err();
    assert!(e.contains("device"), "{e}");
    let (f, made) = make(&mut tools, "m2", json!({"kind": "mobile", "platform": "ios", "device": "tablet", "options": {"orientation": "landscape", "min_os": "17.0"}})).unwrap();
    assert_eq!(made["template"], "ipad");
    let doc = manifest_of(&f);
    assert_eq!(doc["mobile"]["platform"].as_str(), Some("ios"));
    assert_eq!(doc["mobile"]["device"].as_str(), Some("tablet"));
    assert_eq!(doc["mobile"]["orientation"].as_str(), Some("landscape"));
    assert_eq!(doc["mobile"]["min_os"].as_str(), Some("17.0"));
    assert_eq!(doc["mobile"]["multi_memory"].as_bool(), Some(false), "iOS starts with multi-memory off (WASM-017 R9)");
    assert!(f.join("mobileforms/main-form.cfrm").is_file());

    // Clearing the Backend leaves a Web project without one.
    let (f, made) = make(&mut tools, "nobackend", json!({"kind": "web", "options": {"backend": false}})).unwrap();
    assert!(made["backend"].as_array().unwrap().is_empty());
    assert!(!f.join("backend").exists());
    let doc = manifest_of(&f);
    assert!(doc.get("backend").is_none(), "no [backend] table");
    assert!(doc["files"].get("backend").is_none());

    // Without the sample there is no form and no main.
    let (f, made) = make(&mut tools, "nosample", json!({"kind": "web", "options": {"sample": "false"}})).unwrap();
    assert_eq!(made["main"], "");
    assert!(!f.join("webforms/main-form.cfrm").exists());

    // Refusals say what would have worked.
    let e = make(&mut tools, "r1", json!({"kind": "tablet"})).unwrap_err();
    assert!(e.contains("desktop, web or mobile"), "{e}");
    let e = make(&mut tools, "r2", json!({"template": "web", "kind": "mobile"})).unwrap_err();
    assert!(e.contains("makes a web project"), "{e}");
    let e = make(&mut tools, "r3", json!({"template": "android-watch"})).unwrap_err();
    assert!(e.contains("cannot be created yet") && e.contains("iphone"), "{e}");
    let e = make(&mut tools, "r4", json!({"kind": "web", "options": {"orientation": "portrait"}})).unwrap_err();
    assert!(e.contains("has no option 'orientation'"), "{e}");
    let e = make(&mut tools, "r5", json!({"kind": "web", "options": {"style": "tabs"}})).unwrap_err();
    assert!(e.contains("spa"), "{e}");
    let e = make(&mut tools, "r6", json!({"kind": "web", "options": "style"})).unwrap_err();
    assert!(e.contains("object"), "{e}");
    for r in ["r1", "r2", "r3", "r4", "r5", "r6", "m0", "m1"] {
        assert!(!base.join(r).exists(), "a refused creation writes nothing ({r})");
    }
    println!("create_project: kind/platform/device pick the template; style, orientation, min_os, backend and sample reach the manifest; 8 bad requests refused with nothing written");
    let _ = std::fs::remove_dir_all(base);
}

/// A project an agent creates on the default template is what it was before kinds existed.
#[test]
fn a_desktop_project_is_what_it_always_was() {
    let base = scratch("desktop");
    let mut tools = bare();
    let folder = base.join("Inventory");
    let made = call(&mut tools, "create_project", json!({"folder": folder.to_string_lossy(), "name": "Inventory"})).unwrap();
    assert_eq!(made["kind"], "desktop");
    assert_eq!(made["main"], "src/main.cbl");
    assert_eq!(made["folders"].as_array().unwrap().len(), cobolt_project_tools::create::PROJECT_FOLDERS.len());
    let text = std::fs::read_to_string(folder.join("Inventory.project.toml")).unwrap();
    for absent in ["kind =", "[web]", "[mobile]", "[backend]", "[desktop]", "description =", "locale =", "backend ="] {
        assert!(!text.contains(absent), "a Desktop manifest has no {absent}");
    }
    assert!(folder.join("src/main.cbl").is_file());
    let listed = call(&mut tools, "list_files", json!({})).unwrap();
    assert!(listed["files"].get("backend").is_none() && listed.get("backend").is_none(), "no Backend in a Desktop project's listing");
    println!("create_project: the default request writes a Desktop manifest with none of the kind fields, and list_files shows no Backend");
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn list_files_shows_the_backend_and_the_web_assets_of_a_web_project() {
    let base = scratch("list");
    let mut tools = bare();
    let folder = base.join("Site");
    call(&mut tools, "create_project", json!({"folder": folder.to_string_lossy(), "name": "Site", "kind": "web"})).unwrap();
    std::fs::write(folder.join("webforms/main-form.css"), "body{margin:0}").unwrap();
    let listed = call(&mut tools, "list_files", json!({})).unwrap();
    assert_eq!(listed["kind"], "web");
    assert_eq!(listed["forms_folder"], "webforms");
    assert_eq!(listed["files"]["forms"][0]["path"], "webforms/main-form.cfrm");
    assert_eq!(listed["files"]["backend"][0], json!({"path": "backend/src/health.cbl", "exists": true}));
    assert_eq!(listed["backend"]["folder"], "backend");
    assert_eq!(listed["backend"]["exports"][0], "HEALTH");
    assert_eq!(listed["web_assets"]["stylesheets"][0], "webforms/main-form.css");
    assert_eq!(listed["web_assets"]["bundle"]["exists"], false, "nothing built yet");

    // A file under backend/ goes to the Backend's list, not Common Code.
    std::fs::write(folder.join("backend/src/greet.cbl"), "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. GREET.\n       PROCEDURE DIVISION.\n           GOBACK.\n").unwrap();
    let added = call(&mut tools, "add_to_project", json!({"path": "backend/src/greet.cbl"})).unwrap();
    assert_eq!(added["list"], "backend");
    std::fs::write(folder.join("src/shared.cbl"), "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. SHARED.\n       PROCEDURE DIVISION.\n           GOBACK.\n").unwrap();
    let added = call(&mut tools, "add_to_project", json!({"path": "src/shared.cbl"})).unwrap();
    assert_eq!(added["list"], "sources", "Common Code stays Common Code");
    let listed = call(&mut tools, "list_files", json!({})).unwrap();
    assert_eq!(listed["files"]["backend"].as_array().unwrap().len(), 2);
    let checked = call(&mut tools, "check", json!({})).unwrap();
    assert_eq!(checked["errors"], 0, "{checked}");
    assert!(checked["checked"]["sources"].as_u64().unwrap() >= 3, "the Backend's programs are checked with the rest: {checked}");

    // A Desktop project has no Backend list to add to, and its backend/ is a folder like any other.
    let desk = base.join("Desk");
    call(&mut tools, "create_project", json!({"folder": desk.to_string_lossy(), "name": "Desk"})).unwrap();
    std::fs::create_dir_all(desk.join("backend")).unwrap();
    std::fs::write(desk.join("backend/x.cbl"), "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. X.\n       PROCEDURE DIVISION.\n           GOBACK.\n").unwrap();
    let added = call(&mut tools, "add_to_project", json!({"path": "backend/x.cbl"})).unwrap();
    assert_eq!(added["list"], "sources");
    let e = call(&mut tools, "add_to_project", json!({"path": "backend/x.cbl", "list": "backend"})).unwrap_err();
    assert!(e.contains("only a Web or Mobile project"), "{e}");
    println!("list_files/add_to_project: Backend files and web assets listed for a Web project; routing by folder only where a Backend exists");
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn build_on_a_web_project_reports_the_bundle_or_says_why_not() {
    let _g = REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
    let base = scratch("build");
    let mut tools = bare();
    let folder = base.join("Site");
    call(&mut tools, "create_project", json!({"folder": folder.to_string_lossy(), "name": "Site", "kind": "web"})).unwrap();

    // No generator linked: the answer says so, in words, and nothing is claimed built.
    web::unlink();
    let v = call(&mut tools, "build", json!({})).unwrap();
    assert_eq!(v["status"], "not_linked", "{v}");
    assert!(v["reason"].as_str().unwrap().contains("WebAssembly generator"));
    assert!(!folder.join("dist/web").exists());

    // A generator linked: the report carries the size by part, the start-up and the warnings.
    struct Stub;
    impl WebBuildBackend for Stub {
        fn name(&self) -> &str {
            "stub"
        }
        fn build(&self, r: &WebBuildRequest) -> Result<BuildReport, WebBuildError> {
            std::fs::create_dir_all(&r.out_dir).unwrap();
            std::fs::write(r.out_dir.join("index.html"), "<html></html>").unwrap();
            std::fs::write(r.out_dir.join("program.wasm"), vec![0u8; 4096]).unwrap();
            Ok(BuildReport {
                bundle_dir: r.out_dir.clone(),
                size: BundleSize::of_dir(&r.out_dir),
                startup_ms: Some(41.5),
                warnings: vec!["the module is larger than 3 KiB".into()],
                duration_ms: 9,
            })
        }
    }
    web::link(Arc::new(Stub));
    let started = Instant::now();
    let v = call(&mut tools, "build", json!({})).unwrap();
    let ms = started.elapsed().as_millis();
    assert_eq!(v["status"], "built", "{v}");
    assert_eq!(v["engine"], "stub");
    assert_eq!(v["bundle"], "dist/web");
    assert_eq!(v["report"]["size"]["total_bytes"], 4096 + 13);
    assert_eq!(v["report"]["size"]["parts"][0]["name"], "program.wasm");
    assert_eq!(v["report"]["startup_ms"], 41.5);
    assert_eq!(v["variants"], json!({"multi_memory": true, "single_memory": true}), "the default matrix carries both modules");
    assert!(v.get("package").is_none(), "a Web project has no package step");
    let listed = call(&mut tools, "list_files", json!({})).unwrap();
    assert_eq!(listed["web_assets"]["bundle"]["exists"], true);
    assert_eq!(listed["web_assets"]["bundle"]["files"], 2);
    println!("build (web): not_linked says why; with a generator linked the answer carries bundle dir, size by part (4109 B), start-up and variants - build call {ms} ms");

    // A Mobile project's build says which package step it cannot perform, and why.
    let ios = base.join("Phone");
    call(&mut tools, "create_project", json!({"folder": ios.to_string_lossy(), "name": "Phone", "kind": "mobile", "platform": "ios", "device": "phone"})).unwrap();
    let v = call(&mut tools, "build", json!({})).unwrap();
    assert_eq!(v["status"], "built", "{v}");
    assert_eq!(v["package"]["built"], false);
    assert!(v["package"]["reason"].as_str().unwrap().len() > 20);
    assert_eq!(v["variants"], json!({"multi_memory": false, "single_memory": true}), "iOS carries the single-memory module");

    // A project with an error is refused before any build, as the desktop build refuses it.
    std::fs::write(folder.join("backend/src/health.cbl"), "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. HEALTH.\n       PROCEDURE DIVISION.\n           MOVE 1 TO WS-NOT-DECLARED.\n           GOBACK.\n").unwrap();
    let mut again = ProjectTools::new(HeadlessHost::new(&folder, "test"));
    let v = call(&mut again, "build", json!({})).unwrap();
    assert_eq!(v["status"], "refused", "{v}");
    web::unlink();
    let _ = std::fs::remove_dir_all(base);
}
