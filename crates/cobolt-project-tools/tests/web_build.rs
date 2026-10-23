// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The real web build (WASM-010 T19, AC9 part 1): a Web project's forms become
//! the program modules and a bundle in `dist/web`, the report gives the
//! modules' sizes and a start-up figure, and a handler the browser cannot run
//! refuses the build at its line.

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
    let mut form = Form::new("main-form", "Hello", 400, 300);
    let mut btn = Control::new("BTN-OK", ControlType::Button, 10, 10);
    let mut ev = EventBinding::for_control("BTN-OK", "onClick");
    ev.code = format!("       PROCEDURE DIVISION.\n{body}");
    btn.events.push(ev);
    form.controls.push(btn);
    form.controls
        .push(Control::new("LBL-OUT", ControlType::Label, 10, 60));
    cobolt_forms::xml::save_form(&form, &dir.join("webforms/main-form.cfrm")).unwrap();
    dir.join("Hello.project.toml")
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
