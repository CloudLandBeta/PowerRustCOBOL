// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 077 AC17 (parity), the compiled-binary half: each national / UTF-8
//! test program, built as an application's main program and run, gives what
//! it gives under `rcrun run` and Run Form (`cobolt-cli/tests/national_parity.rs`)
//! — the lines below are what those report.

use std::path::{Path, PathBuf};
use std::time::Instant;

use cobolt_compiler::{build_project, BuildOptions};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap().to_path_buf()
}

/// The verdict and the counts of a result block — never the measured times.
fn results(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(|l| l.trim_end().to_string())
        .filter(|l| l.starts_with("PASS ") || l.starts_with("rows:") || l.starts_with("FAIL "))
        .collect()
}

#[test]
#[ignore = "builds real binaries; run with --ignored"]
fn national_programs_agree_in_a_built_binary() {
    let project = std::env::temp_dir().join(format!("prc077-parity-build-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join("src")).unwrap();
    let project = project.canonicalize().unwrap();
    let tests = workspace_root().join("tests/cobol");
    // The unoptimised build: this test proves behaviour, not speed, and a
    // release build of the whole application would be minutes of compiling.
    let opts = BuildOptions { verbose: false, workspace_root: Some(workspace_root()), debug: true, ..Default::default() };
    let expected: [(&str, &str, &[&str]); 4] = [
        (
            "national",
            "nat_basics.cbl",
            &["rows: bulk iterations 0100000 tallied 0200000", "PASS 018 FAIL 000"],
        ),
        ("national", "nat_functions.cbl", &["rows: round trips 0050000", "PASS 007 FAIL 000"]),
        (
            "national",
            "nat_indexed.cbl",
            &["rows: written 02000 read 02000 rewritten 02000 scanned 02000", "PASS 004 FAIL 000"],
        ),
        ("utf8", "utf8_basics.cbl", &["rows: bulk iterations 0100000 sum 001100000", "PASS 014 FAIL 000"]),
    ];
    let mut table = Vec::new();
    for (dir, name, want) in expected {
        std::fs::copy(tests.join(dir).join(name), project.join("src").join(name)).unwrap();
        // One project, its main program swapped: the build folder is reused.
        std::fs::write(
            project.join("natparity.project.toml"),
            format!("[project]\nname = \"Natparity\"\nversion = \"1.0.0\"\nmain = \"src/{name}\"\n"),
        )
        .unwrap();
        let t = Instant::now();
        let built = build_project(&project.join("natparity.project.toml"), &opts)
            .unwrap_or_else(|e| panic!("{name}: build failed: {e}"));
        let build_s = t.elapsed().as_secs_f32();
        let out = std::process::Command::new(&built.binary_path)
            .current_dir(&project)
            .output()
            .unwrap_or_else(|e| panic!("cannot run {name}: {e}"));
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(out.status.success(), "{name}: {stdout}");
        assert_eq!(results(&stdout), want, "{name}: the built binary disagrees:\n{stdout}");
        table.push(format!("{name}: {} (build {build_s:.1} s)", want[want.len() - 1]));
    }
    let _ = std::fs::remove_dir_all(&project);
    println!("077 AC17 built binary = rcrun run = Run Form: {}", table.join("; "));
}
