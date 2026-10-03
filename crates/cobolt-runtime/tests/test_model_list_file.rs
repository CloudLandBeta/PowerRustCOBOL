// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 085 D4a — the application's model list is the application's: the
//! runtime keeps it in `settings/models.json` in the application's folder.
//! A program sees the entries already there without handing anything over,
//! lists them, and its changes are written back — and no key ever reaches
//! the file. Its own test binary: the list is opened once per process.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    let errors: Vec<String> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("{}:{}: {}", d.span.line, d.span.col, d.message))
        .collect();
    assert!(errors.is_empty(), "parse errors: {errors:#?}");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(result.program.expect("program"), event_rx, state_tx, display_tx);
    interp.run().expect("run");
    drop(interp);
    display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect()
}

#[test]
fn the_application_keeps_its_model_list() {
    let started = std::time::Instant::now();
    let base = std::env::temp_dir().join(format!("prc-085-models-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("settings")).unwrap();
    // What another form of the application — or an earlier run — left.
    std::fs::write(
        base.join(cobolt_runtime::model_list::FILE),
        r#"{"version":1,"entries":[{"name":"Company model","api":"openai","url":"https://models.example/v1","model":"gpt-x"}]}"#,
    )
    .unwrap();
    cobolt_runtime::key_store::set_key_store(std::sync::Arc::new(cobolt_runtime::key_store::MemoryKeyStore::default()));
    cobolt_forms::assets::set_base(&base);

    let out = run(r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. MODELS.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 N   PIC 9(3).
01 NM  PIC X(30).
01 AP  PIC X(12).
01 UR  PIC X(40).
01 MD  PIC X(20).
01 ST  PIC X(40).
PROCEDURE DIVISION.
    COBOL::"MODEL-COUNT" ( N )
    DISPLAY "COUNT " N
    COBOL::"MODEL-GET" ( 1 NM AP UR MD )
    DISPLAY "FIRST " FUNCTION TRIM(NM) "|" FUNCTION TRIM(AP) "|" FUNCTION TRIM(UR) "|" FUNCTION TRIM(MD)
    COBOL::"MODEL-SET" ( "Local llama" "ollama" "http://localhost:11434" "llama3" ST )
    COBOL::"KEY-SET" ( "Company model" "sk-SECRET-085" ST )
    COBOL::"MODEL-COUNT" ( N )
    DISPLAY "COUNT " N
    COBOL::"MODEL-GET" ( 2 NM )
    DISPLAY "SECOND " FUNCTION TRIM(NM)
    COBOL::"MODEL-GET" ( 9 NM )
    DISPLAY "NINTH [" NM(1:3) "]"
    COBOL::"MODEL-REMOVE" ( "Company model" )
    COBOL::"MODEL-COUNT" ( N )
    DISPLAY "COUNT " N
    STOP RUN.
"#);
    assert_eq!(
        out,
        [
            "COUNT 001",
            "FIRST Company model|openai|https://models.example/v1|gpt-x",
            "COUNT 002",
            "SECOND Local llama",
            "NINTH [   ]",
            "COUNT 001",
        ],
        "{out:#?}"
    );
    let file = std::fs::read_to_string(base.join(cobolt_runtime::model_list::FILE)).unwrap();
    let json: serde_json::Value = serde_json::from_str(&file).unwrap();
    let names: Vec<&str> = json["entries"].as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Local llama"], "the change was written back: {file}");
    assert!(!file.contains("sk-SECRET-085"), "no key in the list's file");
    let _ = std::fs::remove_dir_all(&base);

    println!("── spec 085 the application's model list ────────────────");
    println!("  read      : 1 entry left in settings/models.json, seen with no MODEL-SET");
    println!("  listed    : MODEL-COUNT 1 → 2 → 1; MODEL-GET 1, 2, and 9 (out of range: spaces)");
    println!("  written   : MODEL-SET + MODEL-REMOVE → the file holds exactly [Local llama]");
    println!("  keys      : KEY-SET went to the key store; the list's file holds no key");
    println!("  elapsed   : {} ms", started.elapsed().as_millis());
}
