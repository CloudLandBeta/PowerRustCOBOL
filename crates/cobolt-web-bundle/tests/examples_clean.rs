// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The committed examples under the secret scan (WASM-013 T3, R7): `scan_project` over the **tracked** text files of
//! `examples/` (an untracked local file must not decide the result). A hit is printed as file, line and pattern
//! — never the value. The examples are the developer's own code and some carry live keys: the test reports, it
//! never edits, and it never opens a file to show what it found.

use cobolt_web_bundle::secrets::{is_text_path, scan_project, Severity, DEFAULT_ENTROPY_LEN};
use std::process::Command;

/// The hits the operator has been told about (file, pattern, how many). They are in the developer's own example
/// forms and documents, which this task never opens, prints or edits (user code is sacred; the demo forms may carry
/// live keys). The list only shrinks: a hit that is not here, or one more of a pattern than is here, fails the test.
const KNOWN_FOR_THE_OPERATOR: &[(&str, &str, usize)] = &[
    ("examples/PowerAnalytics/forms/main-form.cfrm", "a property named like a secret holds a value", 1),
    ("examples/PowerAnalytics/forms/main-form.recovered.cfrm", "a property named like a secret holds a value", 1),
    ("examples/PowerChat/forms/chat-form.cfrm", "a property named like a secret holds a value", 3),
    ("examples/PowerChat/forms/files-form.cfrm", "a property named like a secret holds a value", 1),
    ("examples/PowerDemo3/Knowledge Base/powerrustcobol/controls.md", "connection string with a password", 2),
    ("examples/PowerDemo3/forms/Non-Visual/agent-form.cfrm", "a property named like a secret holds a value", 1),
    ("examples/PowerDemo3/forms/Non-Visual/sqldatabase-form.cfrm", "a property named like a secret holds a value", 1),
];

#[test]
fn the_tracked_examples_hold_no_secret_shape_beyond_the_ones_the_operator_knows() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let listing = Command::new("git").current_dir(&root).args(["ls-files", "-z", "examples"]).output().expect("git runs");
    assert!(listing.status.success(), "git ls-files failed");
    let paths: Vec<String> = String::from_utf8_lossy(&listing.stdout).split('\0').filter(|p| !p.is_empty() && is_text_path(p)).map(String::from).collect();
    let files: Vec<(String, String)> = paths.iter().filter_map(|p| std::fs::read_to_string(root.join(p)).ok().map(|t| (p.clone(), t))).collect();
    let findings = scan_project(&files, DEFAULT_ENTROPY_LEN);
    let (errors, warnings): (Vec<_>, Vec<_>) = findings.iter().partition(|f| f.severity == Severity::Error);
    println!("examples scanned: {} tracked text files, {} errors, {} warnings", files.len(), errors.len(), warnings.len());
    let mut new = Vec::new();
    let mut seen: std::collections::BTreeMap<(String, &str), usize> = std::collections::BTreeMap::new();
    for f in &errors {
        // `Display` carries the file, the line and the pattern, and no value.
        println!("  {f}");
        *seen.entry((f.file.clone(), f.pattern)).or_default() += 1;
    }
    for ((file, pattern), n) in &seen {
        let known = KNOWN_FOR_THE_OPERATOR.iter().find(|(f, p, _)| f == file && p == pattern).map_or(0, |k| k.2);
        if *n > known {
            new.push(format!("{file}: {pattern} x{n} (known: {known})"));
        }
    }
    println!("known to the operator: {} files; new: {}", KNOWN_FOR_THE_OPERATOR.len(), new.len());
    assert!(new.is_empty(), "secret shapes in tracked examples that are not on the operator's list — report them, do not edit the example:\n{}", new.join("\n"));
}
