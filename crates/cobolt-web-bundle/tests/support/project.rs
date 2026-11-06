// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A project written by a test and built by the product's own Web Build (the generator, `GeneratorBackend`):
//! forms made in code, saved the way the IDE saves them (`cobolt_forms::xml::form_to_string`), their handlers
//! written in COBOL, a manifest with the application style, the SideMenu's items. The browser runs of the
//! navigation acceptance (WASM-011 T25) serve the bundle this builds.

use std::path::PathBuf;

use cobolt_forms::menu::{menu_yaml_path, save_menu, MenuDefinition, MenuItem};
use cobolt_forms::model::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildRequest};

/// A built application: the project folder and the bundle the Build wrote into it.
pub struct App {
    pub _dir: tempfile::TempDir,
    pub bundle: PathBuf,
}

/// What to build. `forms[0]` is the main form; `menus` are `(control id, menu)` written beside the forms
/// (`SideMenu-1.menu.yaml`), each saved with its integrity hash.
pub struct Spec<'a> {
    pub style: &'a str,
    pub forms: Vec<Form>,
    pub menus: Vec<(&'a str, MenuDefinition)>,
}

/// The program text of a handler: `lines` are the statements, one per element.
pub fn handler(lines: &[&str]) -> String {
    let mut s = String::from("       PROCEDURE DIVISION.\n");
    for l in lines {
        s.push_str("           ");
        s.push_str(l);
        s.push('\n');
    }
    s
}

/// A handler that needs data items of its own (`decls` are the `01` lines).
pub fn handler_with(decls: &[&str], lines: &[&str]) -> String {
    let mut s = String::from("       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n");
    for d in decls {
        s.push_str("       ");
        s.push_str(d);
        s.push('\n');
    }
    s.push_str(&handler(lines));
    s
}

/// A form of the given name with a label `LBL-OUT` and the rest `controls`.
pub fn form(name: &str, title: &str) -> Form {
    let mut f = Form::new(name, title, 640, 480);
    f.controls
        .push(Control::new("LBL-OUT", ControlType::Label, 272, 16));
    f
}

/// A button with a handler for `onClick`.
pub fn button(id: &str, caption: &str, x: i32, y: i32, code: String) -> Control {
    let mut c = Control::new(id, ControlType::Button, x, y);
    c.set_prop("Caption", PropValue::String(caption.to_owned()));
    let mut ev = EventBinding::for_control(id, "onClick");
    ev.code = code;
    c.events.push(ev);
    c
}

/// A form event (`onLoad`, `onClose`, …) with its handler.
pub fn form_event(f: &mut Form, event: &str, code: String) {
    let mut ev = EventBinding::new(
        event,
        format!(
            "{}--{}",
            f.name.to_ascii_uppercase(),
            event.to_ascii_uppercase()
        ),
    );
    ev.code = code;
    f.form_events.push(ev);
}

/// A SideMenu docked at the left, 264 wide, as a new Web project's.
pub fn side_menu(height: i32) -> Control {
    let mut c = Control::new("SideMenu-1", ControlType::SideMenu, -8, 0);
    c.rect = Rect {
        x: -8,
        y: 0,
        w: 264,
        h: height,
    };
    c
}

/// Write the project into a fresh temporary folder and build it.
pub fn build(spec: Spec<'_>) -> App {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let forms_dir = root.join("webforms");
    std::fs::create_dir_all(&forms_dir).unwrap();
    let mut listed = Vec::new();
    for f in &spec.forms {
        let rel = format!("webforms/{}.cfrm", f.name);
        std::fs::write(
            root.join(&rel),
            cobolt_forms::xml::form_to_string(f).unwrap(),
        )
        .unwrap();
        listed.push(format!("\"{rel}\""));
    }
    for (control, def) in &spec.menus {
        save_menu(&menu_yaml_path(&forms_dir, control), def)
            .unwrap_or_else(|e| panic!("the menu does not save: {e}"));
    }
    let manifest = format!(
        "[project]\nname = \"Acceptance.project\"\nversion = \"1.0.0\"\nmain = \"webforms/{}.cfrm\"\nkind = \"web\"\n\n[files]\nforms = [{}]\nsources = []\n\n[web]\nmulti_memory = true\nstyle = \"{}\"\n",
        spec.forms[0].name,
        listed.join(", "),
        spec.style
    );
    let path = root.join("Acceptance.project.toml");
    std::fs::write(&path, manifest).unwrap();
    let req = WebBuildRequest::from_manifest(&path)
        .unwrap_or_else(|e| panic!("the manifest does not load: {e}"));
    let report = GeneratorBackend
        .build(&req)
        .unwrap_or_else(|e| panic!("the project does not build: {e}"));
    App {
        bundle: report.bundle_dir,
        _dir: dir,
    }
}

/// A SideMenu's menu with one entry per `(id, label, form)`: choosing it loads the form.
pub fn menu_def(entries: &[(&str, &str, &str)]) -> MenuDefinition {
    let menu = entries
        .iter()
        .map(|(id, label, form)| {
            let mut item = MenuItem::new_action(*id, *label);
            item.action = Some(format!("open-form:{form}"));
            item
        })
        .collect();
    MenuDefinition {
        menu,
        hash: String::new(),
    }
}
