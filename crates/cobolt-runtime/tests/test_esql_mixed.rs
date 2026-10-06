// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 087 T19 (AC18) — mixed forms in one program, and a password that
//! reaches no output, trace or debug event.

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use cobolt_lexer::{expand_copybooks, tokenize_expansion, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{DebugEvent, Interpreter};

fn esql_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/esql")
}

#[test]
fn ac18_mixed_forms_and_no_password_anywhere() {
    const SECRET: &str = "pw-AC18-never-shown";
    let scratch = tempfile::tempdir().unwrap();
    let src = std::fs::read_to_string(esql_dir().join("ac18_mixed.cbl"))
        .unwrap()
        .replace("@DIR@", &scratch.path().to_string_lossy())
        .replace("@SECRET@", SECRET);
    let exp = expand_copybooks(&src, &esql_dir(), SourceFormat::Free);
    assert!(exp.errors.is_empty(), "{:?}", exp.errors);
    let result = parse(tokenize_expansion(&exp));
    let errors: Vec<_> = result.diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let (_cmd_tx, cmd_rx) = mpsc::channel();
    let (ev_tx, ev_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(result.program.unwrap(), event_rx, state_tx, display_tx);
    interp.attach_debug_channels_running(cmd_rx, ev_tx, cobolt_runtime::new_breakpoints());
    interp.run().expect("run failed");
    let out: Vec<String> = display_rx.try_iter().map(|l| l.trim_end().to_string()).collect();
    println!("{}", out.join("\n"));
    let events: Vec<String> = ev_rx.try_iter().map(|e| format!("{e:?}")).collect();
    let verdict = out.iter().rev().find(|l| l.starts_with("PASS ")).expect("no PASS/FAIL line");
    assert!(verdict.ends_with("FAIL 000"), "{}", out.join("\n"));
    let sql_lines = events.iter().filter(|e| e.contains("Sql")).count();
    println!("debug events {} (SQL lines {sql_lines}); none holds the password", events.len());
    assert!(!out.iter().any(|l| l.contains(SECRET)), "the password reached the output");
    assert!(!events.iter().any(|e| e.contains(SECRET)), "the password reached a debug event");
    assert!(events.iter().any(|e| e.contains("USING ******")), "the CONNECT line is there, masked");
}
