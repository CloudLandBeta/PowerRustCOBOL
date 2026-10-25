// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The real web build (WASM-010 T19, AC9 part 1): a Web project's forms become
//! the program modules and a bundle in `dist/web`, the report gives the
//! modules' sizes and a start-up figure, and a handler the browser cannot run
//! refuses the build at its line. WASM-002 T13 checks what the Build writes:
//! the page, the modules `module_variants` asks for, the player, `config.json`
//! and `_headers`, and nothing that is an interpreter, an AST or a runtime (AC4b).

use cobolt_forms::model::{Control, ControlType, EventBinding, Form};
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildError, WebBuildRequest};
use std::path::PathBuf;

const MANIFEST: &str =
    "[project]\nname = \"Hello.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
    [files]\nforms = [\"webforms/main-form.cfrm\"]\n\n[web]\nmulti_memory = true\n";

/// A Web project whose one form has a button and a label; the button's
/// `onClick` handler is `body`.
fn project(tag: &str, body: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-webbuild-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    std::fs::write(dir.join("Hello.project.toml"), MANIFEST).unwrap();
    write_form(&dir, "main-form", body);
    dir.join("Hello.project.toml")
}

/// `webforms/<name>.cfrm`: a button whose `onClick` handler is `body`, and a label.
fn write_form(dir: &std::path::Path, name: &str, body: &str) {
    let mut form = Form::new(name, "Hello", 400, 300);
    let mut btn = Control::new("BTN-OK", ControlType::Button, 10, 10);
    let mut ev = EventBinding::for_control("BTN-OK", "onClick");
    ev.code = format!("       PROCEDURE DIVISION.\n{body}");
    btn.events.push(ev);
    form.controls.push(btn);
    form.controls
        .push(Control::new("LBL-OUT", ControlType::Label, 10, 60));
    cobolt_forms::xml::save_form(&form, &dir.join(format!("webforms/{name}.cfrm"))).unwrap();
}

/// A Web project with the `[web]` table `web` and one form per name, each the
/// hello-world form of [`write_form`].
fn project_with(tag: &str, web: &str, forms: &[&str]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-webbuild-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    let list: Vec<String> = forms
        .iter()
        .map(|f| format!("\"webforms/{f}.cfrm\""))
        .collect();
    let manifest = format!(
        "[project]\nname = \"Hello.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
         [files]\nforms = [{}]\n\n{web}\n",
        list.join(", ")
    );
    std::fs::write(dir.join("Hello.project.toml"), manifest).unwrap();
    for f in forms {
        write_form(&dir, f, "           MOVE \"Hi\" TO LBL-OUT::Caption.\n");
    }
    dir.join("Hello.project.toml")
}

/// Every file under `dir`, by its path relative to `dir`.
fn snapshot(dir: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn walk(
        base: &std::path::Path,
        dir: &std::path::Path,
        out: &mut std::collections::BTreeMap<String, Vec<u8>>,
    ) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                let rel = p
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn a_web_project_builds_to_modules_and_a_bundle() {
    let manifest = project("ok", "           MOVE \"Hi\" TO LBL-OUT::Caption.\n");
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    assert!(req.variants.multi_memory, "{:?}", req.variants);
    let report = GeneratorBackend
        .build(&req)
        .unwrap_or_else(|e| panic!("{e}"));
    let wasm: Vec<_> = report
        .size
        .parts
        .iter()
        .filter(|p| p.name.contains("program") && p.name.ends_with(".wasm"))
        .collect();
    assert!(
        !wasm.is_empty(),
        "the program modules are in the bundle: {:?}",
        report.size.parts
    );
    for p in &wasm {
        assert!(p.bytes > 1000, "{p:?}");
    }
    let ms = report
        .startup_ms
        .expect("the start-up is measured on wasmtime");
    assert!(ms > 0.0, "{ms}");
    assert!(
        report.bundle_dir.join("index.html").is_file(),
        "{:?}",
        report.size.parts
    );
    // WASM-016 R13: the report states the player's size and each part's, as the player's build recorded them.
    let record: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(cobolt_project_tools::web::generator::player_dir().unwrap().join("player-build.json")).unwrap(),
    )
    .unwrap();
    let level = record["profile"]["opt_level"].as_str().unwrap();
    let mut want: Vec<(String, u64)> = record["measurements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["opt_level"] == level)
        .map(|m| (m["variant"].as_str().unwrap().to_owned(), m["after_wasm_opt_bytes"].as_u64().unwrap()))
        .collect();
    want.extend(record["parts"].as_array().unwrap().iter().map(|p| (format!("parts/{}", p["name"].as_str().unwrap()), p["after_wasm_opt_bytes"].as_u64().unwrap())));
    let got: Vec<(String, u64)> = report.player.iter().map(|p| (p.name.clone(), p.bytes)).collect();
    assert_eq!(got, want);
    assert!(got.iter().any(|(n, _)| n == "plain") && got.iter().any(|(n, _)| n.starts_with("parts/")), "{got:?}");
    assert_eq!(report.to_json()["player"].as_array().unwrap().len(), got.len());
    eprintln!("player sizes in the report: {got:?}");
    // A second build replaces the bundle in one step and leaves nothing beside it.
    let again = GeneratorBackend.build(&req).unwrap();
    assert_eq!(again.size.parts.len(), report.size.parts.len());
    let stray: Vec<_> = std::fs::read_dir(report.bundle_dir.parent().unwrap())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with('.'))
        .collect();
    assert!(stray.is_empty(), "{stray:?}");
    eprintln!(
        "web build: {} files, {} bytes; modules {:?}; start-up {ms:.2} ms (wasmtime, native); {} warnings",
        report.size.parts.len(),
        report.size.total_bytes,
        wasm.iter().map(|p| (p.name.clone(), p.bytes)).collect::<Vec<_>>(),
        report.warnings.len()
    );
    let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
}

#[test]
fn an_exec_sql_handler_refuses_the_build_at_its_line() {
    let manifest = project("sql", "           EXEC SQL DELETE FROM T END-EXEC.\n");
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    match GeneratorBackend.build(&req) {
        Err(WebBuildError::Refused(ds)) => {
            let d = ds
                .iter()
                .find(|d| d.message.contains("EXEC SQL"))
                .unwrap_or_else(|| panic!("{ds:?}"));
            assert!(
                d.location.starts_with("webforms/main-form.cfrm (")
                    && d.location.contains("onClick"),
                "{d:?}"
            );
            assert_eq!(
                d.kind,
                cobolt_project_tools::web::WebDiagnosticKind::BackendOnly
            );
            eprintln!("refused: {d}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert!(!req.out_dir.exists(), "a refused build writes no bundle");
    let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
}

// ── WASM-002 T13: what the Web Build writes into `dist/web` ──────────────────

/// A program module of the bundle: `program.<hash>.wasm` or
/// `program.single.<hash>.wasm`, at the bundle's top level.
fn program_modules(
    files: &std::collections::BTreeMap<String, Vec<u8>>,
) -> Vec<(&String, &Vec<u8>)> {
    files
        .iter()
        .filter(|(p, _)| p.starts_with("program.") && p.ends_with(".wasm"))
        .collect()
}

#[test]
fn the_bundle_holds_the_page_the_modules_the_player_and_nothing_else() {
    // Chromium only, the single-memory fallback off: `module_variants` asks
    // for the multi-memory module alone.
    let manifest = project_with(
        "ac4b",
        // WASM-017 D5: multi-memory only is the fallback turned off.
        "[web]\nmulti_memory = true\nsingle_memory_fallback = false\n\n[web.browsers]\nfirefox = false\nsafari = false\nios_webview = false\nandroid_webview = false\n",
        &["main-form"],
    );
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    assert!(
        req.variants.multi_memory && !req.variants.single_memory,
        "{:?}",
        req.variants
    );
    let report = GeneratorBackend
        .build(&req)
        .unwrap_or_else(|e| panic!("{e}"));
    let files = snapshot(&report.bundle_dir);
    for want in ["index.html", "config.json", "_headers"] {
        assert!(
            files.contains_key(want),
            "{want} missing: {:?}",
            files.keys()
        );
    }
    assert!(
        files
            .keys()
            .any(|p| p.starts_with("player/") && p.ends_with(".wasm")),
        "the player is copied"
    );
    let modules = program_modules(&files);
    assert_eq!(modules.len(), 1, "{:?}", files.keys());
    assert!(!modules[0].0.contains(".single."), "{}", modules[0].0);

    // The single-memory variant alone when multi-memory is off.
    let manifest = project_with(
        "ac4b-single",
        "[web]\nmulti_memory = false\n",
        &["main-form"],
    );
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    assert!(
        !req.variants.multi_memory && req.variants.single_memory,
        "{:?}",
        req.variants
    );
    let report = GeneratorBackend
        .build(&req)
        .unwrap_or_else(|e| panic!("{e}"));
    let single = snapshot(&report.bundle_dir);
    let modules = program_modules(&single);
    assert_eq!(modules.len(), 1, "{:?}", single.keys());
    assert!(
        modules[0].0.starts_with("program.single."),
        "{}",
        modules[0].0
    );

    // AC4b: each module imports only the host API of ABI v1 §3 and is a
    // bundleable module (exports, fixed memories).
    for (files, single_memory) in [(&files, false), (&single, true)] {
        for (name, bytes) in program_modules(files) {
            let scan = cobolt_wasm_abi::scan::scan_module(bytes).unwrap();
            let imports: Vec<String> = scan
                .imports
                .iter()
                .map(|i| format!("{}.{}", i.module, i.name))
                .collect();
            for i in &scan.imports {
                assert!(
                    i.module == cobolt_wasm_abi::codes::IMPORT_MODULE
                        && cobolt_wasm_abi::codes::PRODUCTION_IMPORTS.contains(&i.name.as_str()),
                    "{name} imports {}.{}",
                    i.module,
                    i.name
                );
            }
            let problems = cobolt_wasm_abi::scan::bundle_problems(bytes, single_memory);
            assert!(problems.is_empty(), "{name}: {problems:?}");
            eprintln!("{name}: {} bytes, imports {imports:?}", bytes.len());
        }
        // AC4b: no interpreter, AST or runtime library. The desktop build's AST
        // is bincode + gzip; the runtime would be another WebAssembly file.
        for (path, bytes) in files.iter() {
            let lower = path.to_ascii_lowercase();
            for word in ["interpreter", "runtime", "ast.", ".bin", ".gz", ".bincode"] {
                assert!(!lower.contains(word), "{path} looks like {word}");
            }
            assert!(!bytes.starts_with(&[0x1f, 0x8b]), "{path} is a gzip stream");
            if lower.ends_with(".wasm") {
                assert!(
                    lower.starts_with("program.") || lower.starts_with("player/"),
                    "{path} is neither the program module nor the player"
                );
            }
        }
    }
    eprintln!("AC4b: {} and {} files checked", files.len(), single.len());
    for tag in ["ac4b", "ac4b-single"] {
        let _ = std::fs::remove_dir_all(
            std::env::temp_dir().join(format!("prc-webbuild-{tag}-{}", std::process::id())),
        );
    }
}

#[test]
fn a_standalone_project_has_one_entry_page_per_form() {
    let manifest = project_with(
        "standalone",
        "[web]\nstyle = \"standalone\"\n",
        &["main-form", "second-form"],
    );
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    let report = GeneratorBackend
        .build(&req)
        .unwrap_or_else(|e| panic!("{e}"));
    let files = snapshot(&report.bundle_dir);
    for page in ["index.html", "main-form.html", "second-form.html"] {
        assert!(
            files.contains_key(page),
            "{page} missing: {:?}",
            files.keys()
        );
    }
    let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
}

#[test]
fn a_failed_build_leaves_the_previous_bundle_as_it_was() {
    let manifest = project("atomic", "           MOVE \"Hi\" TO LBL-OUT::Caption.\n");
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    GeneratorBackend
        .build(&req)
        .unwrap_or_else(|e| panic!("{e}"));
    let before = snapshot(&req.out_dir);
    write_form(
        req.project_dir.as_path(),
        "main-form",
        "           EXEC SQL DELETE FROM T END-EXEC.\n",
    );
    assert!(matches!(
        GeneratorBackend.build(&req),
        Err(WebBuildError::Refused(_))
    ));
    let after = snapshot(&req.out_dir);
    assert_eq!(before.len(), after.len());
    assert!(before == after, "the first bundle changed");
    eprintln!(
        "atomic: {} files byte-identical after a refused build",
        after.len()
    );
    let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
}

#[test]
fn a_desktop_project_is_refused_before_anything_is_written() {
    let dir = std::env::temp_dir().join(format!("prc-webbuild-desktop-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("Desk.project.toml");
    std::fs::write(
        &manifest,
        "[project]\nname = \"Desk.project\"\nversion = \"1.0.0\"\nmain = \"\"\n\n[files]\nforms = []\n",
    )
    .unwrap();
    let err = WebBuildRequest::from_manifest(&manifest).unwrap_err();
    assert!(err.contains("Desktop project"), "{err}");
    assert!(
        !dir.join("dist").exists(),
        "nothing is written for a Desktop project"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ── WASM-017 T7: "multi-memory only" is refused while WebKit is in the matrix ──

#[test]
fn multi_memory_only_with_webkit_refuses_check_and_build_017() {
    let manifest = project_with("mm-only-webkit", "[web]\nmulti_memory = true\nsingle_memory_fallback = false\n", &["main-form"]);
    let ds = refusals(&manifest);
    assert_eq!(ds.len(), 1, "{ds:?}");
    assert_eq!(ds[0].kind, cobolt_project_tools::web::WebDiagnosticKind::MultiMemoryOnlyWithWebKit);
    assert_eq!(ds[0].subject, "Safari, iOS web view");
    assert_eq!(ds[0].location, "Hello.project.toml");
    eprintln!("refused: {}", ds[0]);
    let _ = std::fs::remove_dir_all(manifest.parent().unwrap());

    // Without WebKit the same setting builds the multi-memory module alone.
    let manifest = project_with(
        "mm-only",
        "[web]\nmulti_memory = true\nsingle_memory_fallback = false\n\n[web.browsers]\nsafari = false\nios_webview = false\n",
        &["main-form"],
    );
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
    let modules: Vec<String> = program_modules(&snapshot(&report.bundle_dir)).into_iter().map(|(p, _)| p.clone()).collect();
    assert_eq!(modules.len(), 1, "{modules:?}");
    assert!(!modules[0].contains(".single."), "{modules:?}");
    eprintln!("fallback off, no WebKit: {modules:?}");
    let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
}

// ── WASM-017 T6: one payload limit ──────────────────────────────────────────

/// The project's payload limit reaches the module manifest's `frame_size` and
/// `config.json`'s `max_frame`; a project that sets none gets 384 KiB in both.
#[test]
fn the_payload_limit_reaches_the_module_and_the_player_017() {
    let mut seen = Vec::new();
    for (tag, web, want) in [("payload-default", "[web]\nmulti_memory = true\n", 384 * 1024u64), ("payload-512", "[web]\nmulti_memory = true\nmax_payload_bytes = 524288\n", 512 * 1024)] {
        let manifest = project_with(tag, web, &["main-form"]);
        let req = WebBuildRequest::from_manifest(&manifest).unwrap();
        assert_eq!(u64::from(req.web.max_payload_bytes), want);
        let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
        let files = snapshot(&report.bundle_dir);
        let config: serde_json::Value = serde_json::from_slice(&files["config.json"]).unwrap();
        assert_eq!(config["backend"]["max_frame"].as_u64(), Some(want), "config.json max_frame");
        let url = config["manifest"]["url"].as_str().unwrap();
        let module_manifest: serde_json::Value = serde_json::from_slice(&files[url]).unwrap();
        assert_eq!(module_manifest["limits"]["frame_size"].as_u64(), Some(want), "manifest frame_size");
        seen.push(format!("{tag}: frame_size {want}, max_frame {want}"));
        let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
    }
    eprintln!("payload limit: {}", seen.join("; "));
}

// ── WASM-017 T3: `Instances` out of range refuses Check and Build ───────────

#[test]
fn instances_out_of_range_refuses_check_and_build_naming_the_form_017() {
    let manifest = project("instances", "           MOVE \"Hi\" TO LBL-OUT::Caption.\n");
    let path = manifest.parent().unwrap().join("webforms/main-form.cfrm");
    let mut lines = Vec::new();
    for (n, ok) in [(0u32, false), (1, true), (64, true), (65, false)] {
        let mut form = cobolt_forms::load_form(&path).unwrap();
        form.instances = Some(n);
        cobolt_forms::xml::save_form(&form, &path).unwrap();
        let req = WebBuildRequest::from_manifest(&manifest).unwrap();
        if ok {
            GeneratorBackend
                .check(&req)
                .unwrap_or_else(|e| panic!("Instances {n} is accepted: {e}"));
            lines.push(format!("{n}: accepted"));
        } else {
            let ds = refusals(&manifest);
            let d = ds
                .iter()
                .find(|d| d.kind == cobolt_project_tools::web::WebDiagnosticKind::InstancesOutOfRange)
                .unwrap_or_else(|| panic!("{ds:?}"));
            assert_eq!(d.location, "webforms/main-form.cfrm", "{d:?}");
            assert_eq!(d.subject, "main-form", "the subject is the form: {d:?}");
            assert!(d.message.contains("main-form") && d.message.contains(&n.to_string()), "{d:?}");
            lines.push(format!("{n}: refused — {d}"));
        }
    }
    eprintln!("Instances, 4 values: {}", lines.join("; "));
    let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
}

// ── WASM-002 T14: the portable subset at Check and at Build (R3, AC3) ───────

/// A Web project whose main form declares `file_control` / `file_section` and
/// whose button's `onClick` handler is `code` (its own divisions included).
fn portable_project(tag: &str, file_control: &str, file_section: &str, code: &str) -> PathBuf {
    let manifest = project(tag, "           CONTINUE.\n");
    let path = manifest.parent().unwrap().join("webforms/main-form.cfrm");
    let mut form = cobolt_forms::load_form(&path).unwrap();
    form.cobol_structure.file_control = file_control.into();
    form.cobol_structure.file_section = file_section.into();
    form.controls[0].events[0].code = code.into();
    cobolt_forms::xml::save_form(&form, &path).unwrap();
    manifest
}

fn refusals(manifest: &std::path::Path) -> Vec<cobolt_project_tools::web::WebDiagnostic> {
    let req = WebBuildRequest::from_manifest(manifest).unwrap();
    let checked = GeneratorBackend.check(&req);
    let built = GeneratorBackend.build(&req);
    assert!(!req.out_dir.exists(), "a refused build writes no bundle");
    match (checked, built) {
        (Err(WebBuildError::Refused(c)), Err(WebBuildError::Refused(b))) => {
            assert_eq!(c, b, "Check and Build refuse the same lines");
            c
        }
        other => panic!("expected Check and Build to refuse: {other:?}"),
    }
}

const SEQ_SELECT: &str =
    "           SELECT IN-FILE ASSIGN TO \"in.dat\"\n               ORGANIZATION IS SEQUENTIAL.";
const SEQ_FD: &str = "       FD  IN-FILE IS GLOBAL.\n       01  IN-REC PIC X(80).";

#[test]
fn portable_a_disk_file_refuses_check_and_build_at_the_handler_line() {
    let manifest = portable_project(
        "portable-disk",
        SEQ_SELECT,
        SEQ_FD,
        "       PROCEDURE DIVISION.\n           OPEN INPUT IN-FILE.\n",
    );
    let ds = refusals(&manifest);
    let d = &ds[0];
    eprintln!("refused: {d}");
    assert!(
        d.location
            .starts_with("webforms/main-form.cfrm (main-form ▸ BTN-OK ▸ onClick):"),
        "{d:?}"
    );
    assert!(d.location.ends_with(":2"), "the handler's own line: {d:?}");
    assert!(
        d.message.starts_with("file IN-FILE ") && d.subject == "IN-FILE",
        "the construct (the disk file) is named: {d:?}"
    );
    assert!(
        d.message.contains("backend"),
        "an alternative is named: {d:?}"
    );

    // The agents' and the IDE's Check: the same line, as an error.
    cobolt_project_tools::web::link(std::sync::Arc::new(GeneratorBackend));
    let root = cobolt_project_tools::ProjectRoot::open(&manifest).unwrap();
    let host = cobolt_project_tools::HeadlessHost::new(manifest.clone(), "test");
    let report = cobolt_project_tools::tools::check::check_project(&host, &root).unwrap();
    let f = report
        .findings
        .iter()
        .find(|f| f.message == d.message)
        .unwrap_or_else(|| panic!("{:?}", report.findings));
    assert_eq!(
        f.severity,
        cobolt_project_tools::validate_source::Severity::Error
    );
    assert_eq!(
        (f.file.as_str(), f.line),
        ("webforms/main-form.cfrm", 2),
        "{f:?}"
    );
    assert_eq!(f.site.as_deref(), Some("main-form ▸ BTN-OK ▸ onClick"));
    assert_eq!(f.form.as_deref(), Some("main-form"));
    let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
}

#[test]
fn portable_a_memory_indexed_file_and_an_in_memory_sort_build() {
    // The files are the handler's own (a handler is a nested program; the
    // form's FILE SECTION is not visible to it, WASM-010's form programs).
    let manifest = portable_project(
        "portable-memory",
        "",
        "",
        "       ENVIRONMENT DIVISION.\n       INPUT-OUTPUT SECTION.\n       FILE-CONTROL.\n           SELECT CUST ASSIGN TO \"cust\"\n               ORGANIZATION IS INDEXED\n               ACCESS MODE IS DYNAMIC\n               RECORD KEY IS CUST-ID\n               STORAGE IS MEMORY\n               FILE STATUS IS CUST-FS.\n           SELECT WORK-FILE ASSIGN TO \"work.tmp\".\n       DATA DIVISION.\n       FILE SECTION.\n       FD  CUST.\n       01  CUST-REC.\n           05 CUST-ID   PIC 9(4).\n           05 CUST-NAME PIC X(10).\n       SD  WORK-FILE.\n       01  W-REC PIC X(5).\n       WORKING-STORAGE SECTION.\n       01 CUST-FS PIC XX.\n       PROCEDURE DIVISION.\n       MAIN-PARA.\n           OPEN I-O CUST.\n           MOVE 1 TO CUST-ID.\n           MOVE \"ADA\" TO CUST-NAME.\n           WRITE CUST-REC.\n           CLOSE CUST.\n           SORT WORK-FILE ON ASCENDING KEY W-REC\n               INPUT PROCEDURE IS GIVE-RECORDS\n               OUTPUT PROCEDURE IS TAKE-RECORDS.\n           GOBACK.\n       GIVE-RECORDS.\n           MOVE \"B\" TO W-REC.\n           RELEASE W-REC.\n       TAKE-RECORDS.\n           RETURN WORK-FILE AT END CONTINUE END-RETURN.\n",
    );
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    GeneratorBackend
        .check(&req)
        .unwrap_or_else(|e| panic!("{e}"));
    let report = GeneratorBackend
        .build(&req)
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(report.bundle_dir.join("index.html").is_file());
    let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
}

#[test]
fn portable_every_r3_construct_is_refused_with_an_alternative() {
    let ws = "       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 H PIC 9(4).\n       01 ST PIC X(10).\n       01 BODY PIC X(100).\n       PROCEDURE DIVISION.\n";
    let indexed = |storage: &str| {
        format!("           SELECT CUST ASSIGN TO \"cust\"\n               ORGANIZATION IS INDEXED\n               ACCESS MODE IS DYNAMIC\n               RECORD KEY IS CUST-ID{storage}.")
    };
    let cust_fd =
        "       FD  CUST IS GLOBAL.\n       01  CUST-REC.\n           05 CUST-ID PIC 9(4).";
    // (what R3 names, file-control, file section, handler)
    let cases: Vec<(&str, String, &str, String)> = vec![
        ("a file on disk", SEQ_SELECT.into(), SEQ_FD, "       PROCEDURE DIVISION.\n           OPEN INPUT IN-FILE.\n".into()),
        ("an INDEXED file on disk (the disk engines, redb)", indexed(""), cust_fd, "       PROCEDURE DIVISION.\n           OPEN I-O CUST.\n".into()),
        // Working assumption (plan §7, to confirm): WITH PERSISTENCE writes to disk.
        ("STORAGE IS MEMORY WITH PERSISTENCE", indexed("\n               STORAGE IS MEMORY WITH PERSISTENCE"), cust_fd, "       PROCEDURE DIVISION.\n           OPEN I-O CUST.\n".into()),
        ("SQL", String::new(), "", "       PROCEDURE DIVISION.\n           EXEC SQL DELETE FROM T END-EXEC.\n".into()),
        ("SQL (a database built-in)", String::new(), "", format!("{ws}           CALL \"COBOL-OPEN-DB\" USING \"x.db\" H ST.\n")),
        // The runtime answers HTTP over a native socket, on its own threads for
        // the asynchronous calls (cobolt-runtime interpreter.rs `thread::spawn`).
        ("a native socket (HTTP)", String::new(), "", format!("{ws}           CALL \"COBOL-HTTP-GET\" USING \"https://example.invalid\" BODY ST.\n")),
        // The runtime's child processes are the AWS host (aws/process.rs `spawn`),
        // reached through the AWS credentials built-ins.
        ("a child process (the AWS host)", String::new(), "", format!("{ws}           CALL \"COBOL-AWS-SET-CREDENTIALS\" USING \"p\" ST.\n")),
        ("an embedded secret (a key)", String::new(), "", format!("{ws}           CALL \"COBOL-KEY-SET\" USING \"OPENAI\" \"k\" ST.\n")),
        ("EXEC RUST", String::new(), "", "       PROCEDURE DIVISION.\n           EXEC RUST\n               let _x = 1;\n           END-EXEC.\n".into()),
    ];
    let mut walked = 0;
    for (i, (what, fc, fs, code)) in cases.iter().enumerate() {
        let manifest = portable_project(&format!("portable-r3-{i}"), fc, fs, code);
        let ds = refusals(&manifest);
        for d in &ds {
            assert!(
                d.location
                    .starts_with("webforms/main-form.cfrm (main-form ▸ BTN-OK ▸ onClick):"),
                "{what}: located at the handler: {d:?}"
            );
            assert!(
                d.message.contains("backend")
                    || d.message.contains("CALL … WITH WEBSOCKET LINKAGE"),
                "{what}: the refusal names an alternative: {d}"
            );
        }
        eprintln!("{what}: {}", ds[0]);
        walked += 1;
        let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
    }
    eprintln!("R3: {walked} constructs refused, each naming an alternative");
}

// ── WASM-002 T15: which player the Build copies ─────────────────────────────

#[test]
fn player_a_missing_folder_fails_naming_it() {
    use cobolt_project_tools::web::generator::player_dir_from;
    let missing = std::env::temp_dir().join(format!("prc-no-player-{}", std::process::id()));
    let err = player_dir_from(Some(missing.clone()), None).unwrap_err();
    assert!(
        err.starts_with(cobolt_project_tools::web::PLAYER_UNAVAILABLE_PREFIX),
        "{err}"
    );
    assert!(
        err.contains(&missing.display().to_string()),
        "the folder is named: {err}"
    );

    // A folder without its `player-build.json` is not a player either.
    std::fs::create_dir_all(missing.join("plain")).unwrap();
    let err = player_dir_from(Some(missing.clone()), None).unwrap_err();
    assert!(err.contains("player-build.json"), "{err}");
    eprintln!("player: {err}");
    let _ = std::fs::remove_dir_all(&missing);
}

#[test]
fn player_the_folder_beside_the_program_comes_first_and_the_committed_one_builds() {
    use cobolt_project_tools::web::generator::player_dir_from;
    let exe = std::env::temp_dir().join(format!("prc-exe-dir-{}", std::process::id()));
    let beside = exe.join("assets/web-player");
    std::fs::create_dir_all(&beside).unwrap();
    std::fs::write(beside.join("player-build.json"), "{}").unwrap();
    assert_eq!(player_dir_from(None, Some(exe.clone())).unwrap(), beside);
    let _ = std::fs::remove_dir_all(&exe);

    // No folder beside the program: the committed player of the repository.
    let committed = player_dir_from(None, Some(exe)).unwrap();
    assert!(
        committed.join("player-build.json").is_file(),
        "{}",
        committed.display()
    );
    let manifest = project("player", "           MOVE \"Hi\" TO LBL-OUT::Caption.\n");
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    let report = GeneratorBackend
        .build(&req)
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(report
        .size
        .parts
        .iter()
        .any(|p| p.name.starts_with("player/")));
    let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
}

// ── WASM-002 T17: a secret in a `.wcfrm` stops the Build (R7, AC5) ──────────

#[test]
fn secret_in_a_wcfrm_property_stops_the_build_naming_file_and_pattern() {
    let dir = std::env::temp_dir().join(format!("prc-webbuild-secret-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    std::fs::write(
        dir.join("Hello.project.toml"),
        "[project]\nname = \"Hello.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
         [files]\nforms = [\"webforms/main-form.wcfrm\"]\n",
    )
    .unwrap();
    // AWS's documented example key, built from fragments so that this file
    // holds none for a repository scan to find. A recognisable fake.
    let key = format!("A{}{}", "KIA", "IOSFODNN7EXAMPLE");
    let mut form = Form::new("main-form", "Hello", 400, 300);
    let mut lbl = Control::new("LBL-KEY", ControlType::Label, 10, 10);
    lbl.set_prop("Caption", key.as_str());
    form.controls.push(lbl);
    cobolt_forms::xml::save_form(&form, &dir.join("webforms/main-form.wcfrm")).unwrap();
    let req = WebBuildRequest::from_manifest(&dir.join("Hello.project.toml")).unwrap();
    // A first, clean bundle stays as it is.
    std::fs::create_dir_all(&req.out_dir).unwrap();
    std::fs::write(req.out_dir.join("index.html"), "previous").unwrap();
    match GeneratorBackend.build(&req) {
        Err(WebBuildError::Failed(why)) => {
            eprintln!("secret: {why}");
            assert!(!why.contains(&key), "the value is never shown: {why}");
            assert!(why.contains("main-form"), "the file is named: {why}");
            assert!(
                why.contains("secret (cloud access key id)"),
                "the pattern is named: {why}"
            );
        }
        other => panic!("expected the build to stop on the key, got {other:?}"),
    }
    assert_eq!(
        std::fs::read_to_string(req.out_dir.join("index.html")).unwrap(),
        "previous"
    );
    assert_eq!(
        std::fs::read_dir(&req.out_dir).unwrap().count(),
        1,
        "dist/web unchanged"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ── WASM-002 T18: a Web build needs no tool (R6, AC6 Web half) ──────────────

const NO_TOOL_CHILD: &str = "PRC_WEBBUILD_NO_TOOL_CHILD";

/// Run in a child process with `PATH` empty: no `cargo`, `rustc`,
/// `wasm-bindgen` or `node` can be found. A no-op in the ordinary run.
#[test]
fn no_tool_child_build() {
    if std::env::var_os(NO_TOOL_CHILD).is_none() {
        return;
    }
    assert_eq!(std::env::var("PATH").unwrap_or_default(), "");
    let manifest = project("no-tool", "           MOVE \"Hi\" TO LBL-OUT::Caption.\n");
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    let report = GeneratorBackend
        .build(&req)
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(report.bundle_dir.join("index.html").is_file());
    println!("NO-TOOL-BUILT {} files", report.size.parts.len());
    let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
}

#[test]
fn no_tool_the_hello_world_builds_with_path_emptied() {
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "no_tool_child_build",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("PATH", "")
        .env(NO_TOOL_CHILD, "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stdout}\n{stderr}");
    assert!(
        stdout.contains("NO-TOOL-BUILT"),
        "the child built: {stdout}\n{stderr}"
    );
    eprintln!(
        "{}",
        stdout
            .lines()
            .find(|l| l.contains("NO-TOOL-BUILT"))
            .unwrap_or_default()
    );
}

// ── WASM-002 T19: the Build's own bundle under node (AC1) ───────────────────

/// The node side of `cobolt-web-bundle`'s end-to-end tests: the browser's
/// environment (fetch with integrity, Worker, WebAssembly) and its helpers.
/// Reached by path, since `cobolt-web-bundle` cannot depend on this crate.
#[path = "../../cobolt-web-bundle/tests/support/harness.rs"]
mod harness;

#[test]
fn node_the_builds_own_hello_world_answers_a_click() {
    let node = std::process::Command::new("node").arg("--version").output();
    let version = match node {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_owned(),
        _ => {
            panic!("node is required for the end-to-end test (WASM-015) and was not found on PATH")
        }
    };
    let manifest = project("node", "           MOVE \"Hi\" TO LBL-OUT::Caption.\n");
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    assert!(
        req.variants.multi_memory && req.variants.single_memory,
        "{:?}",
        req.variants
    );
    let report = GeneratorBackend
        .build(&req)
        .unwrap_or_else(|e| panic!("{e}"));

    // The environment of the shared harness, then this test's own scenarios.
    let preamble = harness::HARNESS
        .split("// ── the core scenarios")
        .next()
        .unwrap();
    let script = format!(
        "{preamble}\n\
for (const program of ['multi', 'single']) {{\n\
  await scenario(`the Build's hello-world answers a click (${{program}})`, async () => {{\n\
    const app = await boot({{ overrides: {{ program, mode: 'main' }} }});\n\
    eq(app.choice.program, program, 'program variant chosen');\n\
    check(await click(app, 'BTN-OK'), 'BTN-OK has a handler');\n\
    eq(caption(app), 'Hi', 'the click wrote the caption');\n\
    await app.unmount();\n\
  }});\n\
}}\n\
console.log(JSON.stringify({{ results, metrics }}));\n\
process.exit(0);\n"
    );
    let tools =
        std::env::temp_dir().join(format!("prc-webbuild-node-tools-{}", std::process::id()));
    std::fs::create_dir_all(&tools).unwrap();
    std::fs::write(tools.join("harness.mjs"), script).unwrap();
    std::fs::write(tools.join("worker-shim.mjs"), harness::WORKER_SHIM).unwrap();
    let run = std::process::Command::new("node")
        .arg(tools.join("harness.mjs"))
        .arg(&report.bundle_dir)
        .arg(tools.join("worker-shim.mjs"))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&run.stdout);
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        run.status.success(),
        "node exited with {}:\n{stderr}\n{stdout}",
        run.status
    );
    let line = stdout
        .lines()
        .last()
        .expect("the harness printed its report");
    let out: serde_json::Value =
        serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}\n{stderr}"));
    let results = out["results"].as_object().unwrap();
    for (name, r) in results {
        assert_eq!(r["pass"].as_bool(), Some(true), "{name}: {}", r["error"]);
    }
    assert_eq!(results.len(), 2);
    eprintln!(
        "AC1 under node {version}: {} scenarios passed",
        results.len()
    );
    let _ = std::fs::remove_dir_all(&tools);
    // `PRC_WEB_E2E_KEEP=1` keeps the bundle, to serve it to a browser
    // (`cargo run -p cobolt-web-bundle --example serve -- <dir>`): the Chrome
    // half of AC1/AC2 and the other browsers of the matrix (T19, T24).
    if std::env::var_os("PRC_WEB_E2E_KEEP").is_some() {
        println!("bundle kept: {}", report.bundle_dir.display());
    } else {
        let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
    }
}

/// Build the project of `manifest` and read the `app.css` of its bundle.
fn built_app_css(manifest: &std::path::Path) -> String {
    let req = WebBuildRequest::from_manifest(manifest).unwrap();
    GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
    let web = manifest.parent().unwrap().join("dist/web");
    let css = std::fs::read_dir(&web)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("app.") && n.to_string_lossy().ends_with(".css")))
        .expect("app.css in the bundle");
    std::fs::read_to_string(css).unwrap()
}

/// A see-through theme (Spatial, the Web template's) has nothing behind it on a
/// page: the Build paints the stand-in desktop its pictures are drawn over, so
/// its white text never sits on a light page. Any other theme leaves the page's
/// own colour.
#[test]
fn a_see_through_theme_gets_a_backdrop_and_another_theme_does_not() {
    let app_css = built_app_css;
    let spatial = project_with("seethrough", "[web]\nmulti_memory = true\n\n[forms]\ntheme = \"spatial\"\n", &["main-form"]);
    let css = app_css(&spatial);
    assert!(css.contains("background: #4A4642;"), "{css}");
    let plain = project_with("opaque", "[web]\nmulti_memory = true\n", &["main-form"]);
    let css = app_css(&plain);
    assert!(!css.contains("#4A4642"), "{css}");
    eprintln!("see-through backdrop: Spatial gets #4A4642 behind the app, Liquid Glass keeps the page colour");
}

/// The Form Designer paints a form whose own face is translucent over its canvas (a new Web project's main
/// form is `00000000`), and a page has no desktop behind it: such a main form gets the same stand-in as a
/// see-through theme, whatever its theme. A window `Transparency` or a translucent end of a gradient do the
/// same; an opaque colour, even one that spells its alpha, keeps the page's own.
#[test]
fn a_translucent_main_form_gets_a_backdrop_whatever_its_theme() {
    type Edit = fn(&mut Form);
    let cases: [(&str, Edit, bool); 7] = [
        ("an opaque form (the default)", |_| {}, false),
        ("colour 00000000", |f| f.background_color = "00000000".into(), true),
        ("colour #2E313880", |f| f.background_color = "#2E313880".into(), true),
        ("colour #2E3138FF", |f| f.background_color = "#2E3138FF".into(), false),
        ("window Transparency 40", |f| f.transparency = 40, true),
        ("gradient ending in #00000000", |f| {
            f.background_gradient_enabled = true;
            f.background_gradient_start_color = "#FFFFFFFF".into();
            f.background_gradient_end_color = "#00000000".into();
        }, true),
        ("a translucent colour under an opaque gradient", |f| {
            f.background_color = "00000000".into();
            f.background_gradient_enabled = true;
            f.background_gradient_start_color = "#FFFFFFFF".into();
            f.background_gradient_end_color = "#000000FF".into();
        }, false),
    ];
    let started = std::time::Instant::now();
    let mut lines = Vec::new();
    for (i, (what, edit, expect)) in cases.iter().enumerate() {
        let manifest = project_with(&format!("translucent{i}"), "[web]\nmulti_memory = true\n", &["main-form"]);
        let file = manifest.parent().unwrap().join("webforms/main-form.cfrm");
        let mut form = cobolt_forms::xml::load_form_from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        edit(&mut form);
        cobolt_forms::xml::save_form(&form, &file).unwrap();
        let got = built_app_css(&manifest).contains("background: #4A4642;");
        assert_eq!(got, *expect, "{what}: the backdrop is {}", if got { "there" } else { "missing" });
        lines.push(format!("  {what}: {}", if got { "backdrop #4A4642" } else { "the page's own colour" }));
    }
    eprintln!("translucent main form, {} cases in {} ms:\n{}", cases.len(), started.elapsed().as_millis(), lines.join("\n"));
}

/// WASM-016 R11, Q6: `player = "…"` in `[web]` pins an older player. A pin to a player this IDE does not keep
/// stops the Build, lists the players it keeps (the current one among them), and writes no bundle.
#[test]
fn a_pin_to_a_player_that_is_not_kept_stops_the_build_and_lists_the_kept_ones() {
    use cobolt_project_tools::web::PlayerPinError;
    let manifest = project_with("pinned", "[web]\nmulti_memory = true\nplayer = \"0.0.1\"\n", &["main-form"]);
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    assert_eq!(req.web.player.as_deref(), Some("0.0.1"), "the pin is read from the manifest");
    match GeneratorBackend.build(&req) {
        Err(WebBuildError::PlayerPin(PlayerPinError::NotKept { pinned, kept })) => {
            println!("pinned {pinned}; kept {kept:?}");
            assert_eq!(pinned, "0.0.1");
            let current = std::fs::read_to_string(cobolt_project_tools::web::generator::player_dir().unwrap().join("player-build.json")).unwrap();
            let current: serde_json::Value = serde_json::from_str(&current).unwrap();
            assert!(kept.iter().any(|k| Some(k.as_str()) == current["versions"]["player"].as_str()), "the current player is kept: {kept:?}");
        }
        other => panic!("expected the pin to stop the build, got {other:?}"),
    }
    assert!(!req.out_dir.exists(), "no bundle was written");
    let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
}

/// WASM-016 T14: the program that builds may name the player's folder (the IDE: beside its executable). A named
/// folder is used, and one that is not there is the missing player, never another folder found by search.
#[test]
fn the_build_copies_the_player_from_the_folder_it_is_given() {
    let manifest = project_with("playerdir", "[web]\nmulti_memory = true\n", &["main-form"]);
    let mut req = WebBuildRequest::from_manifest(&manifest).unwrap();
    req.player_dir = Some(cobolt_project_tools::web::generator::player_dir().unwrap());
    let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
    assert!(report.player.iter().any(|p| p.name == "plain"), "{:?}", report.player);
    req.player_dir = Some(manifest.parent().unwrap().join("no-such-player"));
    match GeneratorBackend.build(&req) {
        Err(WebBuildError::Failed(why)) => {
            assert!(why.starts_with(cobolt_project_tools::web::PLAYER_UNAVAILABLE_PREFIX) && why.contains("no-such-player"), "{why}");
        }
        other => panic!("expected the missing player, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
}

/// WASM-016 AC1: a Web project — COBOL compiled by the generator — builds with no Rust toolchain on the machine. The
/// test runs itself again with a `PATH` of the system folders only (the child proves `cargo`, `rustc` and `rustup`
/// cannot be started), building from the committed `assets/web-player`.
#[test]
fn a_web_project_builds_with_no_rust_toolchain_on_the_path() {
    const CHILD: &str = "PR_WEB_BUILD_NO_TOOLCHAIN";
    if std::env::var_os(CHILD).is_some() {
        for tool in ["cargo", "rustc", "rustup"] {
            assert!(std::process::Command::new(tool).arg("--version").output().is_err(), "{tool} can be started");
        }
        let manifest = project_with("notoolchain", "[web]\nmulti_memory = true\n", &["main-form"]);
        let mut req = WebBuildRequest::from_manifest(&manifest).unwrap();
        req.player_dir = Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/web-player"));
        let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
        let names: Vec<&str> = report.size.parts.iter().map(|p| p.name.as_str()).collect();
        for must in ["player/plain-", "player/threaded-", "program.", "program.single.", "index.html", "forms/main-form."] {
            assert!(names.iter().any(|n| n.starts_with(must)), "no {must} in {names:?}");
        }
        println!("built with no toolchain: {} files, {} bytes", names.len(), report.size.total_bytes);
        let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
        return;
    }
    let system = if cfg!(windows) { r"C:\Windows\System32" } else { "/usr/bin:/bin" };
    let run = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["a_web_project_builds_with_no_rust_toolchain_on_the_path", "--exact", "--nocapture"])
        .env(CHILD, "1")
        .env("PATH", system)
        .env_remove("CARGO")
        .env_remove("RUSTUP_TOOLCHAIN")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(run.status.success(), "{stdout}\n{}", String::from_utf8_lossy(&run.stderr));
    let line = stdout.lines().find(|l| l.starts_with("built with no toolchain")).expect("the child built");
    println!("{line} (PATH = {system})");
}
