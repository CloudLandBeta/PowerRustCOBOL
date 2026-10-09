// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 091 (R33–R36; AC12): **the same COBOL program, the same answers, on the
//! three hosts that run a form** — `rcrun run-form`, a child form loaded into an
//! application shell's ContentPane, and a built binary (`--ignored`: it
//! compiles one).
//!
//! One form with two layers and six buttons whose handlers write and read a
//! layer's properties. A layer must start **hidden** (R35), a program's
//! `SET LAYER-1::Visible TO TRUE` must show it and the layer must **stay**
//! shown until the program hides it, the backdrop properties must read back
//! what was written, and COBOL must read a layer's property as it reads a
//! control's. The test prints each host's readings as a row, so a divergence is
//! visible to the eye as well as to the assertion.

use std::path::{Path, PathBuf};
use std::process::Command;

use cobolt_forms::model::{FormFormat, Layer};
use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue};
use serde_json::{json, Value};

struct Project {
    root: PathBuf,
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

/// A handler body in the form the designer stores one.
fn handler(statements: &str) -> String {
    format!(
        "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           {statements}\n"
    )
}

fn button(form: &mut Form, id: &str, y: i32, statements: &str) {
    let mut c = Control::new(id, ControlType::Button, 20, y);
    c.rect = cobolt_forms::model::Rect::new(20, y, 160, 28);
    c.set_prop("Caption", PropValue::String(id.into()));
    c.events.push(EventBinding {
        event: "onClick".into(),
        paragraph: format!("{id}--ONCLICK"),
        code: handler(statements),
    });
    form.controls.push(c);
}

/// The form every host runs.
fn layered_form() -> Form {
    let mut f = Form::new("LAYERS-DEMO", "Layers", 640, 480);
    // Loadable as a window and into a shell's pane, so every host can run it.
    f.form_format = FormFormat::Both;
    f.layers = vec![Layer::new("LAYER-1"), Layer::new("LAYER-2")];
    button(&mut f, "BTN-SHOW", 20, "SET LAYER-1::Visible TO TRUE");
    button(&mut f, "BTN-HIDE", 56, "SET LAYER-1::Visible TO FALSE");
    button(&mut f, "BTN-READ", 92, "MOVE LAYER-1::Visible TO LBL-RESULT::Caption");
    button(&mut f, "BTN-COLOUR", 128, "MOVE \"#FF0000FF\" TO LAYER-1::BackgroundColor");
    button(&mut f, "BTN-TRANSP", 164, "MOVE 40 TO LAYER-2::Transparency");
    let mut result = Control::new("LBL-RESULT", ControlType::Label, 20, 200);
    result.rect = cobolt_forms::model::Rect::new(20, 200, 300, 24);
    result.set_prop("Caption", PropValue::String("-".into()));
    f.controls.push(result);
    // A control that lives in the first layer, so the layer is not empty.
    let mut note = Control::new("LBL-IN-LAYER", ControlType::Label, 300, 20);
    note.rect = cobolt_forms::model::Rect::new(300, 20, 200, 24);
    note.set_prop("Caption", PropValue::String("inside Layer-1".into()));
    note.layer = Some("LAYER-1".into());
    f.controls.push(note);
    f
}

/// A project holding the layered form and the shell the embedded run loads it
/// into, with their generated programs.
fn project(tag: &str) -> Project {
    let root = std::env::temp_dir().join(format!("prc-091-hosts-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for d in ["forms", "generated", "src", "data"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    let form = layered_form();
    cobolt_forms::save_form(&form, &root.join("forms/layers-demo.cfrm")).unwrap();
    std::fs::write(root.join("generated/layers-demo.cbl"), cobolt_codegen::generate(&form)).unwrap();
    // The shell: any application shell works, so borrow the AWS demos'.
    let shell = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/aws/forms");
    for file in ["aws-shell.cfrm", "SideMenu-1.menu.yaml"] {
        std::fs::copy(shell.join(file), root.join("forms").join(file)).unwrap();
    }
    let shell_form = cobolt_forms::load_form(&root.join("forms/aws-shell.cfrm")).unwrap();
    std::fs::write(root.join("generated/aws-shell.cbl"), cobolt_codegen::generate(&shell_form)).unwrap();
    std::fs::write(
        root.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n",
    )
    .unwrap();
    let manifest = "[project]\nname = \"LayersHosts\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/layers-demo.cfrm\", \"forms/aws-shell.cfrm\"]\n\
         generated = [\"generated/layers-demo.cbl\", \"generated/aws-shell.cbl\"]\n";
    std::fs::write(root.join("layershosts.project.toml"), manifest).unwrap();
    Project { root }
}

/// One headless step list: what the program is asked to do, and what is read
/// after each answer. `open` loads the form into the shell's pane first.
fn steps(open: Option<&str>) -> Value {
    let click = |id: &str| json!({"event": {"control": id, "name": "onClick"}});
    let read = |ctrl: &str, prop: &str| json!({"read": {"control": ctrl, "property": prop}});
    let wait = || json!({"wait_ms": 500});
    let mut s = Vec::new();
    if let Some(form) = open {
        s.push(json!({"open_form": form}));
    }
    s.extend([
        // R35 — every layer starts hidden.
        read("LAYER-1", "Visible"),
        read("LAYER-2", "Visible"),
        // R33, R34 — a program shows it, and COBOL reads it back.
        click("BTN-SHOW"),
        wait(),
        read("LAYER-1", "Visible"),
        click("BTN-READ"),
        wait(),
        read("LBL-RESULT", "Caption"),
        // The backdrop properties read back what was written.
        click("BTN-COLOUR"),
        wait(),
        read("LAYER-1", "BackgroundColor"),
        click("BTN-TRANSP"),
        wait(),
        read("LAYER-2", "Transparency"),
        // …and it can be hidden again.
        click("BTN-HIDE"),
        wait(),
        read("LAYER-1", "Visible"),
        // A layer a program shows STAYS shown while other events run (R35).
        click("BTN-SHOW"),
        wait(),
        click("BTN-COLOUR"),
        wait(),
        read("LAYER-1", "Visible"),
    ]);
    Value::Array(s)
}

/// What the program answers, in order, from a headless run's stdout.
/// Booleans read as `true` / `false`, the one spelling every property has.
const WANT: [&str; 8] = ["false", "false", "true", "true", "#FF0000FF", "40", "false", "true"];
const READS: [&str; 8] = [
    "LAYER-1 Visible at start",
    "LAYER-2 Visible at start",
    "LAYER-1 Visible after SET TRUE",
    "COBOL reads LAYER-1::Visible",
    "LAYER-1 BackgroundColor",
    "LAYER-2 Transparency",
    "LAYER-1 Visible after SET FALSE",
    "LAYER-1 Visible, shown again, after another event",
];

fn readings(stdout: &str) -> Vec<String> {
    let report: Value = stdout
        .lines()
        .find_map(|l| l.strip_prefix("@RUN-FORM-RESULT "))
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_else(|| panic!("no result line in:\n{stdout}"));
    report["steps"]
        .as_array()
        .map(|steps| {
            steps
                .iter()
                .filter_map(|st| st["value"].as_str().map(|v| v.trim().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

fn run_form(p: &Project, form: &str, program: &str, script: &Value) -> String {
    let script_path = p.root.join(format!("steps-{}.json", form.replace('/', "-")));
    std::fs::write(&script_path, script.to_string()).unwrap();
    let out = Command::new(rcrun())
        .args(["run-form", form, program, "--designer", "--headless", script_path.to_str().unwrap(), "--headless-limit", "60"])
        .current_dir(&p.root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form {form} failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    stdout
}

fn report(host: &str, got: &[String]) {
    println!("\n  091 AC12 — {host}");
    for (i, want) in WANT.iter().enumerate() {
        let g = got.get(i).map(String::as_str).unwrap_or("(missing)");
        println!("    {:<52} {:<12} {}", READS[i], g, if g == *want { "ok" } else { "DIFFERS" });
    }
}

#[test]
fn a_layer_answers_the_same_under_run_form_and_as_an_embedded_child_form() {
    let p = project("rf");
    let direct = readings(&run_form(&p, "forms/layers-demo.cfrm", "generated/layers-demo.cbl", &steps(None)));
    report("rcrun run-form", &direct);
    assert_eq!(direct, WANT, "under Run Form");

    let embedded = readings(&run_form(&p, "forms/aws-shell.cfrm", "generated/aws-shell.cbl", &steps(Some("layers-demo"))));
    report("embedded child form (the shell's ContentPane)", &embedded);
    assert_eq!(embedded, WANT, "as an embedded child form");
}

/// The built binary gives the same answers. Builds one: `--ignored`.
#[test]
#[ignore = "builds a real binary; run with --ignored"]
fn a_layer_answers_the_same_in_a_built_binary() {
    let p = project("bin");
    let ws = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    // Unoptimised: this proves behaviour, not speed.
    let opts = cobolt_compiler::BuildOptions { verbose: false, workspace_root: Some(ws), debug: true, ..Default::default() };
    let built = cobolt_compiler::build_project(&p.root.join("layershosts.project.toml"), &opts)
        .unwrap_or_else(|e| panic!("the build failed: {e}"));
    let script = p.root.join("bin-steps.json");
    std::fs::write(&script, steps(Some("layers-demo")).to_string()).unwrap();
    let out = Command::new(&built.binary_path)
        .env("PRC_HEADLESS_SCRIPT", &script)
        .env("PRC_HEADLESS_LIMIT", "60")
        .current_dir(&p.root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    let got = readings(&stdout);
    report("the built binary", &got);
    assert_eq!(got, WANT, "in the built binary");
}

/// R37 — a misspelt layer property fails the build of the form, where a
/// misspelt control property would be refused only when it ran; and a layer the
/// form does not have is an unknown reference. Nothing else about the program
/// is wrong.
#[test]
fn an_unknown_layer_or_layer_property_fails_the_check_and_a_known_one_passes() {
    let analyse = |statements: &str| -> Vec<String> {
        let mut f = layered_form();
        f.controls.retain(|c| c.id != "BTN-READ");
        button(&mut f, "BTN-TRY", 300, statements);
        let src = cobolt_codegen::generate(&f);
        let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
        let program = parsed.program.expect("a program");
        cobolt_semantic::analyze_with(
            &program,
            &cobolt_semantic::AnalyzeOptions {
                known_objects: Some(cobolt_forms::toolbar::object_names(&f)),
                known_layers: Some(cobolt_forms::toolbar::layer_names(&f)),
                ..Default::default()
            },
        )
        .errors()
        .map(|d| d.message.clone())
        .collect()
    };
    assert!(analyse("SET LAYER-1::Visible TO TRUE").is_empty(), "a known property passes");
    assert!(analyse("MOVE 40 TO LAYER-2::Transparency").is_empty());

    let colour = analyse("MOVE \"#FF0000FF\" TO LAYER-1::Colour");
    assert_eq!(colour.len(), 1, "{colour:?}");
    // COBOL names reach the analyser upper-cased, as written in the program.
    assert!(
        colour[0].to_uppercase().contains("LAYER-1::COLOUR") && colour[0].contains("not a property of a layer"),
        "{colour:?}"
    );

    let ghost = analyse("SET LAYER-9::Visible TO TRUE");
    assert_eq!(ghost.len(), 1, "{ghost:?}");
    assert!(ghost[0].contains("LAYER-9"), "{ghost:?}");

    // R55 — the Non-Visuals tab is not a layer a program can address.
    let tab = analyse("SET NON-VISUALS::Visible TO TRUE");
    assert_eq!(tab.len(), 1, "{tab:?}");
    assert!(tab[0].to_uppercase().contains("NON-VISUALS"), "{tab:?}");
    println!("\n  091 R37, R55 — LAYER-1::Colour: {}\n               — LAYER-9::Visible: {}\n               — NON-VISUALS::Visible: {}", colour[0], ghost[0], tab[0]);
}
