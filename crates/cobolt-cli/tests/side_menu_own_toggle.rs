// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A program folds and opens its OWN SideMenu** — through the real `rcrun`.
//!
//! A child form had `super::<menu>::Collapse()` / `::Open()` (spec 049 R44); the
//! form that owns the rail had nothing: its `Collapse()`, its `Open()` and its
//! own writes of `Collapsed` left the rail where it was (operator, 2026-10-15).
//!
//! The headless run cannot resize its window, so the proof is the one the shell
//! leaves behind every time it applies a pane change: the operator's remembered
//! choice (`shell.toml`, spec 049 R9). `HOME` is a private folder, so the run
//! starts with no remembered choice and cannot touch the developer's own.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

fn button(id: &str, y: i32, body: &str) -> String {
    format!(
        "  <Control id=\"{id}\" type=\"Button\" x=\"300\" y=\"{y}\" w=\"200\" h=\"32\" tab-order=\"9\" z-order=\"9\" visible=\"true\" enabled=\"true\">\n    <Property name=\"Caption\">{id}</Property>\n    <Event name=\"onClick\" paragraph=\"{id}--ONCLICK\"><![CDATA[       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           DISPLAY \"RAN-{id}\".\n           {body}\n]]></Event>\n  </Control>\n"
    )
}

/// A project holding an application shell — the AWS demos' main form, a SideMenu
/// with its menu — plus three buttons that drive the rail from COBOL.
fn project(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("prc-own-toggle-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for d in ["forms", "generated", "src", "data"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/aws/forms");
    std::fs::copy(fixtures.join("SideMenu-1.menu.yaml"), root.join("forms/SideMenu-1.menu.yaml")).unwrap();
    let xml = std::fs::read_to_string(fixtures.join("aws-shell.cfrm")).unwrap();
    let at = xml.rfind("</Form>").unwrap();
    let buttons = [
        button("BTN-FOLD", 40, "SideMenu-1::Collapse()."),
        button("BTN-OPEN", 80, "SideMenu-1::Open()."),
        button("BTN-PROP", 120, "MOVE 1 TO SideMenu-1::Collapsed."),
    ]
    .concat();
    std::fs::write(root.join("forms/aws-shell.cfrm"), format!("{}{}{}", &xml[..at], buttons, &xml[at..])).unwrap();
    let form = cobolt_forms::load_form(&root.join("forms/aws-shell.cfrm")).unwrap();
    std::fs::write(root.join("generated/aws-shell.cbl"), cobolt_codegen::generate(&form)).unwrap();
    std::fs::write(
        root.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("owntoggle.project.toml"),
        "[project]\nname = \"OwnToggle\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/aws-shell.cfrm\"]\ngenerated = [\"generated/aws-shell.cbl\"]\n",
    )
    .unwrap();
    root
}

/// Where the shell remembers the rail for `AWS-SHELL` under `home`.
fn remembered(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    let data = home.join("Library/Application Support");
    #[cfg(not(target_os = "macos"))]
    let data = home.join(".local/share");
    data.join("cobolt/apps/AWS-SHELL/shell.toml")
}

/// Run the shell headless, click `button` (or none), and answer what the program
/// displayed and what the shell then remembered. `before` seeds the remembered
/// choice, as an operator who had worked the ☰ would have.
fn run(tag: &str, button: Option<&str>, before: Option<&str>) -> (Vec<String>, Option<String>) {
    let root = project(tag);
    let home = root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    if let Some(state) = before {
        let path = remembered(&home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, state).unwrap();
    }
    let mut steps = vec![json!({"wait_ms": 900})];
    if let Some(id) = button {
        steps.push(json!({"event": {"control": id, "name": "onClick"}}));
        steps.push(json!({"wait_ms": 700}));
    }
    let script = root.join("steps.json");
    std::fs::write(&script, Value::Array(steps).to_string()).unwrap();
    let out = Command::new(rcrun())
        .args([
            "run-form",
            "forms/aws-shell.cfrm",
            "generated/aws-shell.cbl",
            "--designer",
            "--headless",
            script.to_str().unwrap(),
            "--headless-limit",
            "60",
        ])
        .env("HOME", &home)
        .current_dir(&root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    // The program's own DISPLAY lines come out on stdout as they happen.
    assert!(stdout.lines().any(|l| l.starts_with("@RUN-FORM-RESULT ")), "no result line in:\n{stdout}");
    let shown: Vec<String> = stdout.lines().filter(|l| l.starts_with("RAN-")).map(str::to_owned).collect();
    let state = std::fs::read_to_string(remembered(&home)).ok();
    let _ = std::fs::remove_dir_all(&root);
    (shown, state)
}

#[test]
fn a_program_folds_and_opens_the_rail_of_its_own_form() {
    // Nothing clicked: the designed state is where the application OPENS, and is
    // never remembered as the operator's choice.
    let (shown, idle) = run("idle", None, None);
    assert!(shown.is_empty(), "nothing ran: {shown:?}");
    assert_eq!(idle, None, "an idle shell remembers nothing");

    // `Collapse()` folds it — and the shell remembers that, as it does for the ☰.
    let (shown, fold) = run("fold", Some("BTN-FOLD"), None);
    assert_eq!(shown, vec!["RAN-BTN-FOLD".to_owned()], "the handler ran");
    assert_eq!(fold.as_deref().map(str::trim), Some("collapsed = true"), "Collapse() must fold the rail");

    // `Open()` opens a rail the operator had folded.
    let (shown, open) = run("open", Some("BTN-OPEN"), Some("collapsed = true\n"));
    assert_eq!(shown, vec!["RAN-BTN-OPEN".to_owned()]);
    assert_eq!(open.as_deref().map(str::trim), Some("collapsed = false"), "Open() must open the rail");

    // Writing the property does the same as the verb.
    let (shown, prop) = run("prop", Some("BTN-PROP"), None);
    assert_eq!(shown, vec!["RAN-BTN-PROP".to_owned()]);
    assert_eq!(prop.as_deref().map(str::trim), Some("collapsed = true"), "MOVE 1 TO rail::Collapsed must fold it");

    let said = |s: &Option<String>| s.as_deref().map(str::trim).unwrap_or("nothing remembered").to_owned();
    println!(
        "own-form rail through rcrun: idle -> {}; Collapse() -> {}; Open() on a folded rail -> {}; MOVE 1 TO Collapsed -> {}",
        said(&idle), said(&fold), said(&open), said(&prop),
    );
}
