// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Only the main form starts an application** — end to end, through the real
//! `rcrun` binary.
//!
//! The unit tests in `cobolt_compiler::main_form_guard` decide the verdicts;
//! these prove the CLI acts on them: the right exit code, and the decision
//! taken *before* anything is loaded or drawn.
//!
//! No case here reaches a window. A refusal and a corrupted project both exit
//! before the form is read, and the one case that must get *through* the gate
//! is pointed at a program that does not exist, so it dies on the next step
//! (exit 1) with no GUI. That is the whole point of the assertion: only a
//! request that passed the gate can fail that way.

use std::path::{Path, PathBuf};
use std::process::Command;

/// `rcrun` exit codes for the gate (see `form_gui.rs`).
const EXIT_CORRUPT: i32 = 3;
const EXIT_NOT_MAIN: i32 = 4;

fn temp_project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-cli-gate-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("forms")).unwrap();
    dir
}

/// Write `forms/<id>.cfrm` for each id, marking the ones in `main`, and a
/// manifest that tracks them all. Unsealed on purpose: a project that predates
/// the seal is the weakest case the gate has to hold in, so if the refusal
/// works here it works everywhere.
fn project(dir: &Path, ids: &[&str], main: &[&str]) {
    for id in ids {
        let mut form = cobolt_forms::Form::new(*id, *id, 640, 480);
        form.main_form = main.contains(id);
        cobolt_forms::save_form(&form, &dir.join(format!("forms/{id}.cfrm"))).unwrap();
    }
    let list = ids
        .iter()
        .map(|id| format!("\"forms/{id}.cfrm\""))
        .collect::<Vec<_>>()
        .join(", ");
    std::fs::write(
        dir.join("cobolt.toml"),
        format!(
            "[project]\nname = \"Gate\"\nversion = \"1.0.0\"\nmain = \"\"\n\n\
             [files]\nforms = [{list}]\n"
        ),
    )
    .unwrap();
}

/// Run `rcrun run-form <cfrm> <cbl> [extra…]` and return (exit code, stderr).
fn run_form(dir: &Path, cfrm: &str, extra: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rcrun"))
        .arg("run-form")
        .arg(dir.join(cfrm))
        // Deliberately absent: nothing that clears the gate here should get as
        // far as opening a window.
        .arg(dir.join("nowhere.cbl"))
        .args(extra)
        .output()
        .expect("rcrun runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn a_form_that_is_not_the_main_one_is_refused() {
    let dir = temp_project("not-main");
    project(&dir, &["SIGNON", "MENU"], &["SIGNON"]);

    let (code, err) = run_form(&dir, "forms/MENU.cfrm", &[]);
    assert_eq!(code, EXIT_NOT_MAIN, "stderr: {err}");
    assert!(err.contains("MENU") && err.contains("SIGNON"), "{err}");
    // The gate decides first: the missing program was never even looked for.
    assert!(!err.contains("nowhere.cbl"), "decided before loading: {err}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn two_forms_claiming_the_mark_is_a_corrupted_application() {
    let dir = temp_project("two-marks");
    project(&dir, &["SIGNON", "MENU"], &["SIGNON", "MENU"]);

    // Even asking for the *right* form fails: the project no longer says which
    // form that is, and a runtime does not guess.
    let (code, err) = run_form(&dir, "forms/SIGNON.cfrm", &[]);
    assert_eq!(code, EXIT_CORRUPT, "stderr: {err}");
    assert!(err.contains("CORRUPTED APPLICATION"), "{err}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_designer_runs_any_form() {
    let dir = temp_project("designer");
    project(&dir, &["SIGNON", "MENU"], &["SIGNON"]);

    let (code, err) = run_form(&dir, "forms/MENU.cfrm", &["--designer"]);
    assert_ne!(code, EXIT_NOT_MAIN, "the IDE's Run Form is not refused");
    assert!(err.contains("DESIGNER MODE"), "and it says so: {err}");
    // Past the gate, it failed on the missing program — which is only
    // reachable from the far side of the gate.
    assert!(err.contains("nowhere.cbl"), "{err}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_main_form_reaches_the_program() {
    let dir = temp_project("main-passes");
    project(&dir, &["SIGNON", "MENU"], &["SIGNON"]);

    let (code, err) = run_form(&dir, "forms/SIGNON.cfrm", &[]);
    assert_ne!(code, EXIT_NOT_MAIN, "stderr: {err}");
    assert_ne!(code, EXIT_CORRUPT, "stderr: {err}");
    assert!(err.contains("nowhere.cbl"), "it got to the program: {err}");
    // An unsealed project runs, and says once that it cannot be verified.
    assert!(err.contains("no main-form seal"), "{err}");

    let _ = std::fs::remove_dir_all(&dir);
}
