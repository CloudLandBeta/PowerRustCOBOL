// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 077 AC17 (parity), the `rcrun run` and Run Form half: the national
//! and UTF-8 test programs give the same verdicts and counts under both. The
//! compiled-binary half is `cobolt-compiler/tests/test_national_build.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

const PROGRAMS: [(&str, &str); 4] = [
    ("national", "nat_basics.cbl"),
    ("national", "nat_functions.cbl"),
    ("national", "nat_indexed.cbl"),
    ("utf8", "utf8_basics.cbl"),
];

const MANIFEST: &str = "[project]
name = \"NatParity\"
version = \"1.0.0\"
main = \"src/nat_basics.cbl\"

[files]
forms = [\"forms/parity.cfrm\"]
";

const FORM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="PARITY-FORM" title="Parity" width="320" height="200" main-form="true">
</Form>
"#;

struct Project(PathBuf);
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The verdict and the counts — never the measured times.
fn results(out: &str) -> Vec<String> {
    out.lines()
        .map(|l| l.trim_end().to_string())
        .filter(|l| l.starts_with("PASS ") || l.starts_with("rows:") || l.starts_with("FAIL "))
        .collect()
}

fn rcrun(args: &[&str], dir: &Path) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rcrun")).args(args).current_dir(dir).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun {args:?} failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    stdout
}

#[test]
fn national_programs_agree_under_rcrun_run_and_run_form() {
    let root = std::env::temp_dir().join(format!("prc077-parity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _project = Project(root.clone());
    for d in ["src", "forms", "data"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    std::fs::write(root.join("natparity.project.toml"), MANIFEST).unwrap();
    std::fs::write(root.join("forms/parity.cfrm"), FORM).unwrap();
    std::fs::write(root.join("script.json"), "[]").unwrap();
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol");

    let mut table = Vec::new();
    for (dir, name) in PROGRAMS {
        std::fs::copy(tests.join(dir).join(name), root.join("src").join(name)).unwrap();
        let src = format!("src/{name}");
        let run = results(&rcrun(&["run", &src], &root));
        let form = results(&rcrun(
            &["run-form", "forms/parity.cfrm", &src, "--headless", "script.json", "--headless-limit", "60"],
            &root,
        ));
        let verdict = run.iter().find(|l| l.starts_with("PASS ")).cloned().unwrap_or_default();
        assert!(verdict.ends_with("FAIL 000"), "{name} under rcrun run: {run:?}");
        assert_eq!(run, form, "{name}: rcrun run and Run Form disagree");
        table.push(format!("{name}: {verdict}"));
    }
    println!("077 AC17 rcrun run = Run Form: {}", table.join("; "));
}
