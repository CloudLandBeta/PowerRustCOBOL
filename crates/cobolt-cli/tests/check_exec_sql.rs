// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 087 T9 — `rcrun check` reports the embedded-SQL errors on the same
//! lines as the coding-agent `check` tool and the IDE (the fixture is shared
//! with `cobolt-project-tools/tests/exec_sql_check.rs`), and fails.

use std::process::Command;

#[test]
fn rcrun_check_reports_every_r46_error_on_its_line_and_fails() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cobolt-project-tools/tests/fixtures/esql_r46.cbl");
    let source = std::fs::read_to_string(&fixture).unwrap();
    let dir = std::env::temp_dir().join(format!("esql-rcrun-check-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("checkme.cbl");
    std::fs::write(&path, &source).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_rcrun"))
        .args(["check", path.to_str().unwrap(), "--source-format", "free"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "check must fail:\n{stderr}");

    // `<file>:<line>:<col>: error: <message>` — the line of each error.
    let prefix = format!("{}:", path.display());
    let mut lines: Vec<u32> = stderr
        .lines()
        .filter(|l| l.contains(": error: "))
        .filter_map(|l| l.strip_prefix(&prefix)?.split(':').next()?.parse().ok())
        .collect();
    lines.sort_unstable();
    let mut expected: Vec<u32> = source
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("*> E:"))
        .map(|(i, _)| i as u32 + 1)
        .collect();
    expected.sort_unstable();
    println!("rcrun check: {} errors on lines {lines:?}", lines.len());
    assert_eq!(lines, expected, "\n{stderr}");
}
