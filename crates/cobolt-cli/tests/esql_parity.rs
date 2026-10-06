// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 087 AC14 (parity), the `rcrun` half: the AC2, AC7 and AC9 programs
//! give the same results under `rcrun run` and under Run Form (`rcrun
//! run-form`, off screen) in a project whose SQL connection `SALES` they
//! reach by name. The embedded child form and the compiled binary are proven
//! in `cobolt-form-host` and in the gated build test, on the same programs.

use std::path::{Path, PathBuf};
use std::process::Command;

const PROGRAMS: [&str; 3] = ["ac2_host_structure.cbl", "ac7_cursors.cbl", "ac9_named_connection.cbl"];

const MANIFEST: &str = "[project]
name = \"Parity\"
version = \"1.0.0\"
main = \"src/ac2_host_structure.cbl\"

[files]
forms = [\"forms/parity.cfrm\"]

[[sql-connections]]
name = \"SALES\"
path = \"data/sales.db\"
create-if-missing = true
";

const FORM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="PARITY-FORM" title="Parity" width="320" height="200" main-form="true">
</Form>
"#;

/// A scratch project, removed when the test ends.
struct Project(PathBuf);
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The lines that must agree between hosts: the verdict and the counts —
/// never the elapsed time.
fn results(out: &str) -> Vec<String> {
    out.lines()
        .map(|l| l.trim_end().to_string())
        .filter(|l| l.starts_with("PASS ") || l.starts_with("rows:") || l.starts_with("FAIL "))
        .map(|l| without_timings(&l))
        .collect()
}

/// A result line without what was measured (`in 000000310 ms`,
/// `(000025641 rows/s)`), which differs from run to run by nature.
fn without_timings(line: &str) -> String {
    let words: Vec<&str> = line.split_whitespace().collect();
    let digits = |w: &str| !w.is_empty() && w.trim_start_matches('(').bytes().all(|b| b.is_ascii_digit());
    let mut out = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let next = words.get(i + 1).copied().unwrap_or("");
        if digits(words[i]) && (next.starts_with("ms") || next.starts_with("rows/s")) {
            i += 2;
            continue;
        }
        out.push(words[i]);
        i += 1;
    }
    out.join(" ")
}

fn rcrun(args: &[&str], dir: &Path) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rcrun")).args(args).current_dir(dir).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun {args:?} failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    stdout
}

#[test]
fn ac2_ac7_ac9_agree_under_rcrun_run_and_run_form() {
    let root = std::env::temp_dir().join(format!("prc087-parity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let project = Project(root.clone());
    for d in ["src", "forms", "data"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    std::fs::write(root.join("parity.project.toml"), MANIFEST).unwrap();
    std::fs::write(root.join("forms/parity.cfrm"), FORM).unwrap();
    std::fs::write(root.join("script.json"), "[]").unwrap();
    let esql = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/esql");

    let mut table = Vec::new();
    for name in PROGRAMS {
        std::fs::copy(esql.join(name), root.join("src").join(name)).unwrap();
        let src = format!("src/{name}");
        let run = results(&rcrun(&["run", &src, "--source-format", "free"], &root));
        let form = results(&rcrun(
            &["run-form", "forms/parity.cfrm", &src, "--headless", "script.json", "--headless-limit", "60"],
            &root,
        ));
        let verdict = run.iter().find(|l| l.starts_with("PASS ")).cloned().unwrap_or_default();
        assert!(verdict.ends_with("FAIL 000"), "{name} under rcrun run: {run:?}");
        assert_eq!(run, form, "{name}: rcrun run and Run Form disagree");
        println!("{name} → {run:?}");
        table.push(format!("{name}: {verdict}"));
    }
    drop(project);
    println!("AC14 rcrun run = Run Form: {}", table.join("; "));
}
