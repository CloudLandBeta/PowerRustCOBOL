// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-011 R3, AC10 on a real Web project: FormFormat keeps its meaning and its build-time check. A handler that opens
//! an `Embedded` form with `OpenFormAsync`, and a SideMenu item that loads a `Standalone` form into the ContentPane, stop
//! Check with the Desktop Build's own messages (049 R17); the same project with the forms `Both` checks clean.

use cobolt_forms::model::{Control, ControlType, EventBinding, Form, FormFormat};
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildError, WebBuildRequest};
use std::path::PathBuf;

/// A Web project of three forms: `main-form` (a SideMenu whose item loads `pane-form`, a button that opens
/// `panel-form`), `panel-form` and `pane-form` with the formats given.
fn project(tag: &str, panel: FormFormat, pane: FormFormat) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-wasm011-r3-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    std::fs::write(
        dir.join("Shop.project.toml"),
        "[project]\nname = \"Shop.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
         [files]\nforms = [\"webforms/main-form.wcfrm\", \"webforms/panel-form.wcfrm\", \"webforms/pane-form.wcfrm\"]\n",
    )
    .unwrap();
    let mut main = Form::new("main-form", "Shop", 640, 480);
    main.main_form = true;
    main.controls.push(Control::new("SIDE-1", ControlType::SideMenu, 0, 0));
    let mut btn = Control::new("BTN-OPEN", ControlType::Button, 300, 20);
    btn.events.push(EventBinding {
        event: "onClick".into(),
        paragraph: "BTN-OPEN--ONCLICK".into(),
        code: "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           INVOKE ME::\"OpenFormAsync\"(\"panel-form\").\n".into(),
    });
    main.controls.push(btn);
    cobolt_forms::xml::save_form(&main, &dir.join("webforms/main-form.wcfrm")).unwrap();
    // Through `save_menu`: a sidecar carries the HMAC `load_menu` verifies.
    let def: cobolt_forms::menu::MenuDefinition =
        serde_json::from_str(r#"{"menu":[{"id":"go","label":"Pane","action":"open-form:pane-form"}]}"#).unwrap();
    cobolt_forms::menu::save_menu(&cobolt_forms::menu::menu_yaml_path(&dir.join("webforms"), "SIDE-1"), &def).unwrap();
    for (name, format) in [("panel-form", panel), ("pane-form", pane)] {
        let mut f = Form::new(name, name, 320, 200);
        f.form_format = format;
        cobolt_forms::xml::save_form(&f, &dir.join(format!("webforms/{name}.wcfrm"))).unwrap();
    }
    dir
}

#[test]
fn an_embedded_form_opened_by_a_verb_and_a_standalone_form_loaded_by_a_menu_stop_check_with_the_desktop_messages() {
    let dir = project("bad", FormFormat::Embedded, FormFormat::Standalone);
    let req = WebBuildRequest::from_manifest(&dir.join("Shop.project.toml")).unwrap();
    let ds = match GeneratorBackend.check(&req) {
        Err(WebBuildError::Refused(ds)) => ds,
        other => panic!("expected a refusal, got {other:?}"),
    };
    let lines: Vec<String> = ds.iter().map(|d| d.to_string()).collect();
    println!("{}", lines.join("\n"));
    assert!(
        ds.iter().any(|d| d.message.contains("form 'panel-form' has FormFormat Embedded") && d.message.contains("(049 R17)") && d.location.contains("main-form")),
        "the open of the Embedded form, at the handler: {lines:?}"
    );
    assert!(
        ds.iter().any(|d| d.message.contains("loads form 'pane-form', whose FormFormat is Standalone") && d.location.contains("SIDE-1")),
        "the menu item loading a Standalone form: {lines:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);

    let dir = project("good", FormFormat::Both, FormFormat::Both);
    let req = WebBuildRequest::from_manifest(&dir.join("Shop.project.toml")).unwrap();
    match GeneratorBackend.check(&req) {
        Ok(()) => {}
        Err(WebBuildError::Refused(ds)) => {
            let format_lines: Vec<String> = ds.iter().filter(|d| d.message.contains("FormFormat")).map(|d| d.to_string()).collect();
            assert!(format_lines.is_empty(), "`Both` passes both paths: {format_lines:?}");
        }
        Err(e) => panic!("{e}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}
