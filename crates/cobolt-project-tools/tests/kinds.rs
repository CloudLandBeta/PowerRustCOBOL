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
            // WASM-001 Q2: a web form is .wcfrm, a mobile form .mcfrm.
            assert_eq!(view.forms, [format!("{}/main-form.{}", view.kind.forms_folder(), view.kind.form_extension())]);
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
    // WASM-003 D5: the exports live in backend/exports.toml, not in the manifest.
    assert!(doc.get("backend").is_none(), "no [backend] table in a new manifest");
    assert!(std::fs::read_to_string(f.join("backend/exports.toml")).unwrap().contains("program = \"HEALTH\""));

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
    assert!(f.join("mobileforms/main-form.mcfrm").is_file());

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
    assert!(!f.join("webforms/main-form.wcfrm").exists());

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
    assert_eq!(listed["files"]["forms"][0]["path"], "webforms/main-form.wcfrm");
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
                ..BuildReport::default()
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

/// WASM-001 Q2 (T15.3): every tool that takes a form treats a Web project's
/// `.wcfrm` and a Mobile project's `.mcfrm` as one.
#[test]
fn every_form_tool_takes_the_web_and_mobile_form_extensions() {
    use cobolt_mcp::McpHandler;
    use cobolt_project_tools::tools::{render::Renderer, run::{FormRunner, RunOutcome}, Shared};
    use std::path::Path;
    let base = scratch("ext");
    for (kind, template, form) in [("web", "web", "webforms/main-form.wcfrm"), ("mobile", "android-phone", "mobileforms/main-form.mcfrm")] {
        let folder = base.join(kind);
        call(&mut bare(), "create_project", json!({"folder": folder.to_string_lossy(), "name": "Ext", "template": template})).unwrap();
        let renderer: Renderer = Arc::new(|cfrm: &Path, _: &Path, _: &cobolt_project_tools::tools::render::Picture| {
            assert!(cobolt_compiler::project_kind::is_form_path(cfrm));
            Ok((vec![0x89, b'P', b'N', b'G'], [10, 10]))
        });
        let runner: FormRunner = Arc::new(|cfrm: &Path, cbl: &Path, _: &[Value], _: u64| {
            assert!(cobolt_compiler::project_kind::is_form_path(cfrm) && cbl.is_file());
            Ok(RunOutcome { report: json!({"ran": true}), display: vec![], png: None })
        });
        let shared = Arc::new(Shared::new().with_renderer(renderer).with_runner(runner));
        let mut tools = ProjectTools::with_shared(HeadlessHost::new(&folder, "test"), shared);

        let listed = call(&mut tools, "list_files", json!({})).unwrap();
        assert_eq!(listed["files"]["forms"][0]["path"], form);
        println!("{kind:<6} list_files  lists {form} among the forms");
        let validated = call(&mut tools, "validate", json!({"path": form})).unwrap();
        assert_eq!(validated["valid"], true, "{validated}");
        println!("{kind:<6} validate    loads {form} as a form ({} controls)", validated["controls"]);
        let regenerated = call(&mut tools, "regenerate", json!({"path": form})).unwrap();
        println!("{kind:<6} regenerate  wrote {}", regenerated["written"]);
        assert!(regenerated.to_string().contains("generated/"), "{regenerated}");
        let checked = call(&mut tools, "check", json!({"path": form})).unwrap();
        assert_eq!(checked["errors"], 0, "{checked}");
        println!("{kind:<6} check       checks {form} as a form, {} errors", checked["errors"]);
        let r = tools.call_tool("run_form", &json!({"path": form}));
        assert_eq!(r.is_error, None, "{:?}", r.content);
        println!("{kind:<6} run_form    runs {form}");
        let r = tools.call_tool("render_form", &json!({"path": form}));
        assert_eq!(r.is_error, None, "{:?}", r.content);
        println!("{kind:<6} render_form pictures {form}");
        let added = call(&mut tools, "add_to_project", json!({"path": form})).unwrap_or_else(|e| json!({"refused": e}));
        assert!(added.get("list").is_none_or(|l| l == "forms"), "{added}");
    }
    let _ = std::fs::remove_dir_all(base);
}

/// WASM-001 Q2, plan D-T15b/c (T15.7): a form named for another kind is kept,
/// checked, and warned about exactly once; a matching name draws no warning.
#[test]
fn check_warns_once_about_a_form_named_for_another_kind() {
    let base = scratch("mismatch");
    let folder = base.join("Old");
    call(&mut bare(), "create_project", json!({"folder": folder.to_string_lossy(), "name": "Old", "template": "web"})).unwrap();
    let mut tools = ProjectTools::new(HeadlessHost::new(&folder, "test"));
    let mismatch = |v: &Value| -> Vec<String> {
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "warning" && d["message"].as_str().unwrap().contains("form file name"))
            .map(|d| d["message"].as_str().unwrap().to_owned())
            .collect()
    };
    let clean = call(&mut tools, "check", json!({})).unwrap();
    assert!(mismatch(&clean).is_empty(), "a matching .wcfrm draws no warning: {clean}");

    // What 2.0.x made: the same form named `.cfrm`.
    std::fs::rename(folder.join("webforms/main-form.wcfrm"), folder.join("webforms/main-form.cfrm")).unwrap();
    let manifest = folder.join("Old.project.toml");
    let text = std::fs::read_to_string(&manifest).unwrap().replace("main-form.wcfrm", "main-form.cfrm");
    std::fs::write(&manifest, &text).unwrap();
    let checked = call(&mut tools, "check", json!({})).unwrap();
    let found = mismatch(&checked);
    println!("check on a Web project with webforms/main-form.cfrm: {} mismatch warning(s): {found:?}", found.len());
    assert_eq!(found.len(), 1, "{checked}");
    assert!(found[0].contains("rename it to main-form.wcfrm"), "{}", found[0]);
    assert_eq!(checked["errors"], 0, "the form still loads and checks: {checked}");
    assert!(folder.join("webforms/main-form.cfrm").is_file() && !folder.join("webforms/main-form.wcfrm").exists(), "nothing renamed");
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), text, "the manifest is untouched");
    let _ = std::fs::remove_dir_all(base);
}

/// WASM-003 T2 (R1, R22; C5): a new Web or Mobile project's Backend has its
/// list of exports — `HEALTH`, public — and a development deployment with no
/// secret and no `unsafe_*`; neither is in `[files] backend`, and creating
/// over an existing `exports.toml` leaves it byte for byte.
#[test]
fn a_new_backend_has_its_exports_and_a_development_deployment() {
    use cobolt_project_tools::create::{self, CreateRequest, BACKEND_CONFIG, BACKEND_EXPORTS};
    let base = scratch("backend-scaffold");
    for template in ["web", "android-phone"] {
        let folder = base.join(template);
        let started = Instant::now();
        create::create_project_with(&folder, &CreateRequest::new("Shop").template(template)).unwrap();
        let exports = std::fs::read_to_string(folder.join(BACKEND_EXPORTS)).unwrap();
        let parsed: toml::Value = toml::from_str(&exports).unwrap();
        assert_eq!(parsed["contract"]["version"].as_str(), Some("1.0.0"), "{template}");
        let list = parsed["export"].as_array().unwrap();
        assert_eq!(list.len(), 1, "{template}");
        assert_eq!(list[0]["program"].as_str(), Some("HEALTH"));
        assert_eq!(list[0]["public"].as_bool(), Some(true));

        let config = std::fs::read_to_string(folder.join(BACKEND_CONFIG)).unwrap();
        let parsed: toml::Value = toml::from_str(&config).unwrap();
        let listen = parsed["server"]["listen"].as_str().unwrap();
        assert!(listen.parse::<std::net::SocketAddr>().unwrap().ip().is_loopback(), "{template}: {listen}");
        let keys: Vec<&String> = parsed.as_table().unwrap().keys().collect();
        assert_eq!(keys, ["server"], "{template}: only [server], no unsafe_* and no secret");
        assert!(!config.contains("password") && !config.contains("secret ="), "{template}");

        let view = cobolt_compiler::project_manifest_view(&folder.join("Shop.project.toml")).unwrap();
        assert_eq!(view.backend, ["backend/src/health.cbl"], "{template}: the settings files are not COBOL");
        println!("    {template:<14} exports.toml {:>3} bytes, backend.toml {:>3} bytes, listen {listen}, {:.1} ms", exports.len(), config.len(), started.elapsed().as_secs_f64() * 1000.0);
    }

    // Written once: the developer's thereafter.
    let folder = base.join("web");
    let mine = "[contract]\nversion = \"2.1.0\"\n\n[[export]]\nprogram = \"HEALTH\"\npublic = true\nroles = []\n";
    std::fs::write(folder.join(BACKEND_EXPORTS), mine).unwrap();
    let plan = create::plan(&CreateRequest::new("Shop").template("web"), &cobolt_project_tools::templates::builtin()).unwrap();
    plan.write_into(&folder, &plan.manifest_name).unwrap();
    assert_eq!(std::fs::read_to_string(folder.join(BACKEND_EXPORTS)).unwrap(), mine, "an existing exports.toml is never rewritten");
    println!("  an existing exports.toml ({} bytes) is left byte for byte", mine.len());
    let _ = std::fs::remove_dir_all(base);
}

/// A Web project made the way the New Project dialog makes one (no theme picked, not an agent) wears Spatial and
/// opens as an application shell (the operator, 2026-10-25): the sample sits to the right of a SideMenu rail
/// with its footer panel and its sealed menu file, and the page it builds gets the stand-in desktop. A theme the
/// dialog does pick is kept, and a Mobile project is neither Spatial by default nor a shell.
#[test]
fn a_new_web_project_wears_spatial_and_opens_as_a_shell_unless_another_theme_is_picked() {
    use cobolt_project_tools::create;
    use cobolt_project_tools::templates::CreateRequest;
    let base = scratch("web-shell");
    let started = Instant::now();
    let mut rows = Vec::new();
    for (tag, template, theme, spatial) in [("web", "web", "", true), ("web-picked", "web", "liquid-glass", false), ("phone", "android-phone", "", false)] {
        let folder = base.join(tag);
        let mut request = CreateRequest::new("Shop").template(template);
        request.theme = theme.to_owned();
        let plan = create::plan(&request, &cobolt_project_tools::templates::builtin()).unwrap();
        plan.write_into(&folder, &plan.manifest_name).unwrap();

        let manifest: toml_edit::DocumentMut = std::fs::read_to_string(folder.join(&plan.manifest_name)).unwrap().parse().unwrap();
        let (ide, forms) = (manifest["ide"]["theme"].as_str().unwrap_or("").to_owned(), manifest["forms"]["theme"].as_str().unwrap_or("").to_owned());
        assert_eq!(ide == "spatial" && forms == "spatial", spatial, "{tag}: the IDE theme is {ide:?} and the forms' {forms:?}");

        let form = cobolt_forms::load_form(&folder.join(&plan.main)).unwrap();
        let rail = form.controls.iter().find(|c| c.id == "SideMenu-1");
        assert_eq!(rail.is_some(), template == "web", "{tag}: a Web project is a shell, a Mobile one is not");
        let menu_file = folder.join("webforms/SideMenu-1.menu.yaml");
        if let Some(rail) = rail {
            let r = rail.rect;
            assert_eq!((r.x, r.y, r.w, r.h), (-8, 0, 264, form.height as i32), "{tag}: dropped as the designer drops it");
            let footer = cobolt_forms::model::side_menu_footer_id("SideMenu-1");
            assert!(form.controls.iter().any(|c| c.id == footer && c.parent.as_deref() == Some("SideMenu-1")), "{tag}: the rail's footer panel");
            for id in ["LBL-TITLE", "LBL-HINT", "TXT-NAME", "BTN-GREET", "LBL-RESULT"] {
                let c = form.controls.iter().find(|c| c.id == id).unwrap_or_else(|| panic!("{tag}: {id}"));
                assert!(c.rect.x >= r.x + r.w, "{tag}: {id} sits to the right of the rail (x = {})", c.rect.x);
            }
            let menu = cobolt_forms::menu::load_menu(&menu_file).unwrap_or_else(|e| panic!("{tag}: the sealed menu: {e:?}"));
            assert_eq!(menu.menu.len(), 1, "{tag}: one row");
        } else {
            assert!(!menu_file.exists(), "{tag}: no menu file without a rail");
        }

        // The Spatial shell builds, and its page gets the stand-in desktop behind it.
        let mut built = String::new();
        if spatial {
            let req = WebBuildRequest::from_manifest(&folder.join(&plan.manifest_name)).unwrap();
            cobolt_project_tools::web::generator::GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{tag}: {e}"));
            let css = std::fs::read_dir(folder.join("dist/web")).unwrap().filter_map(|e| e.ok().map(|e| e.path())).find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("app.") && n.to_string_lossy().ends_with(".css"))).expect("app.css");
            assert!(std::fs::read_to_string(css).unwrap().contains("background: #4A4642;"), "{tag}: the stand-in desktop");
            built = ", built with the stand-in desktop".to_owned();
        }
        rows.push(format!("    {tag:<11} theme {ide:?}/{forms:?}, {}{built}", if rail.is_some() { "a SideMenu shell (rail, footer, sealed menu)" } else { "no shell" }));
    }
    println!("\n  new projects as the dialog makes them, {} ms:\n{}", started.elapsed().as_millis(), rows.join("\n"));
    let _ = std::fs::remove_dir_all(base);
}
