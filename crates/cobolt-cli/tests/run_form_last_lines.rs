// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Run Form keeps what a program DISPLAYs just before it ends. The host used
//! to close the window on the frame it saw the program finish, before that
//! frame's output was written, so the last lines — a closing message, a
//! test's verdict — never reached the Output pane.

use std::process::Command;

#[test]
fn run_form_keeps_the_lines_displayed_before_stop_run() {
    let root = std::env::temp_dir().join(format!("prc-run-form-last-lines-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("forms")).unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("last.project.toml"),
        "[project]\nname = \"Last\"\nversion = \"1.0.0\"\nmain = \"src/last.cbl\"\n\n[files]\nforms = [\"forms/last.cfrm\"]\n",
    )
    .unwrap();
    std::fs::write(
        root.join("forms/last.cfrm"),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Form name=\"LAST-FORM\" title=\"Last\" width=\"200\" height=\"100\" main-form=\"true\">\n</Form>\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/last.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. LAST.\n       PROCEDURE DIVISION.\n\
         \x20          DISPLAY \"FIRST LINE\".\n           DISPLAY \"LAST LINE\".\n           STOP RUN.\n",
    )
    .unwrap();
    std::fs::write(root.join("script.json"), "[]").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcrun"))
        .args(["run-form", "forms/last.cfrm", "src/last.cbl", "--headless", "script.json", "--headless-limit", "20"])
        .current_dir(&root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _ = std::fs::remove_dir_all(&root);
    let lines: Vec<&str> = stdout.lines().map(str::trim_end).filter(|l| l.ends_with(" LINE")).collect();
    assert_eq!(lines, ["FIRST LINE", "LAST LINE"], "every line reached the output:\n{stdout}");
    assert!(stdout.contains("\"program_ended\":true"), "{stdout}");
}
