// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 111: the web build's lowering of an IndexedFile control's verbs
//! (`cobolt_compiler::web_remote_indexed::lower_indexed_files`). A form program
//! in the shape `cobolt-codegen` writes — `SELECT`, `FD`, the verbs on the
//! file — is lowered, then run by the interpreter against a scripted transport:
//! the endpoint and the record image each verb sends, the developer's own
//! `AT END` / `INVALID KEY` phrases running on the status the answer carries.
//! What the pass refuses is checked too.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use cobolt_ast::program::Program;
use cobolt_compiler::web_remote_indexed::{lower_indexed_files, served_files};
use cobolt_forms::{Control, ControlType, Form, PropValue};
use cobolt_runtime::Interpreter;
use cobolt_wire::call::{CallReply, CallRequest, Transport};
use cobolt_wire::error::CallError;
use serde_json::{json, Value};

fn parse(src: &str) -> Program {
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<_> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
    assert!(errors.is_empty(), "{errors:?}");
    parsed.program.expect("a program")
}

fn parts_form() -> Form {
    let mut f = Form::new("SHOP", "Shop", 400, 300);
    let mut ix = Control::new("IXF-1", ControlType::IndexedFile, 0, 0);
    ix.set_prop("IndexedFile", PropValue::String("data/parts.cidx".into()));
    f.controls.push(ix);
    f.controls.push(Control::new("BTN-GO", ControlType::Button, 0, 0));
    f
}

/// The form's program as the codegen shapes it around the developer's `body`.
fn program(body: &str) -> String {
    format!(
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. SHOP.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT PARTS ASSIGN TO \"data/parts.idx\"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS PART-ID.
       DATA DIVISION.
       FILE SECTION.
       FD  PARTS
           RECORD CONTAINS 29 CHARACTERS.
       01 PART-REC.
          05 PART-ID   PIC 9(5).
          05 PART-NAME PIC X(20).
          05 PART-QTY  PIC 9(4).
       WORKING-STORAGE SECTION.
       01 WS-STATUS PIC XX VALUE SPACES.
       01 WS-IXF-1-STATUS PIC XX VALUE SPACES.
       01 WS-OUT PIC X(60) VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN-PARA.
{body}           STOP RUN.
"
    )
}

/// A transport that records what it was sent and answers from a script.
struct Script {
    sent: Arc<Mutex<Vec<(String, Value)>>>,
    answers: Vec<Value>,
}

impl Transport for Script {
    fn call(&mut self, request: &CallRequest) -> Result<CallReply, CallError> {
        let args: Value = serde_json::from_str(&request.args[0].json).unwrap();
        self.sent.lock().unwrap().push((request.program.clone(), args));
        let answer = if self.answers.is_empty() { json!({"FILE-STATUS": "00", "RESULT-IMAGE": "", "PAST-END": 0}) } else { self.answers.remove(0) };
        Ok(CallReply { returning: Some(answer.to_string()), args: Vec::new(), ..Default::default() })
    }
}

fn run(src: &str, answers: Vec<Value>) -> (Vec<String>, Vec<(String, Value)>) {
    let mut p = parse(src);
    let n = lower_indexed_files(&mut p, &parts_form()).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(n > 0, "something was lowered");
    let (_etx, erx) = mpsc::channel();
    let (stx, _srx) = mpsc::channel();
    let (dtx, drx) = mpsc::channel();
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut i = Interpreter::new_with_channels(p, erx, stx, dtx);
    i.set_remote_transport(Box::new(Script { sent: Arc::clone(&sent), answers }));
    i.run().expect("the lowered form runs");
    let shown: Vec<String> = drx.try_iter().map(|s| s.trim().to_owned()).collect();
    let sent = sent.lock().unwrap().clone();
    (shown, sent)
}

fn image(id: &str, name: &str, qty: &str) -> String {
    format!("{id:0>5}{name:<20}{qty:0>4}")
}

#[test]
fn a_served_file_is_found_by_its_control_and_its_select() {
    let p = parse(&program("           CONTINUE\n"));
    let files = served_files(&p, &parts_form());
    assert_eq!(files.len(), 1);
    assert_eq!((files[0].control.as_str(), files[0].file.as_str(), files[0].record.as_str()), ("IXF-1", "PARTS", "PART-REC"));
    assert_eq!(files[0].key, "PART-ID");
    // A form that declares no SELECT for the file serves nothing (PartiQL only).
    let mut bare = parse(&program("           CONTINUE\n"));
    bare.environment = None;
    assert!(served_files(&bare, &parts_form()).is_empty());
}

#[test]
fn every_verb_becomes_a_call_to_its_endpoint_and_the_phrases_run_on_the_status() {
    let started = Instant::now();
    let body = "           OPEN I-O PARTS
           MOVE 7 TO PART-ID
           MOVE \"bolt\" TO PART-NAME
           MOVE 12 TO PART-QTY
           WRITE PART-REC
               INVALID KEY DISPLAY \"DUP\"
               NOT INVALID KEY DISPLAY \"WRITTEN\"
           END-WRITE
           MOVE 7 TO PART-ID
           START PARTS KEY IS >= PART-ID
               INVALID KEY DISPLAY \"NO-START\"
           END-START
           READ PARTS NEXT
               AT END DISPLAY \"EOF\"
               NOT AT END DISPLAY \"GOT \" PART-NAME
           END-READ
           READ PARTS NEXT
               AT END DISPLAY \"EOF\"
               NOT AT END DISPLAY \"GOT \" PART-NAME
           END-READ
           MOVE 99 TO PART-ID
           READ PARTS
               INVALID KEY DISPLAY \"NOT-FOUND \" WS-IXF-1-STATUS
           END-READ
           MOVE 7 TO PART-ID
           MOVE 30 TO PART-QTY
           REWRITE PART-REC
               INVALID KEY DISPLAY \"NO-REWRITE\"
           END-REWRITE
           DELETE PARTS
               INVALID KEY DISPLAY \"NO-DELETE\"
           END-DELETE
           CLOSE PARTS
";
    let answers = vec![
        json!({"FILE-STATUS": "00", "RESULT-IMAGE": "", "PAST-END": 0}),                                  // OPEN
        json!({"FILE-STATUS": "00", "RESULT-IMAGE": image("7", "bolt", "12"), "PAST-END": 0}),             // WRITE
        json!({"FILE-STATUS": "00", "RESULT-IMAGE": "", "PAST-END": 0}),                                  // START
        json!({"FILE-STATUS": "00", "RESULT-IMAGE": image("7", "bolt", "12"), "PAST-END": 0}),             // READ NEXT
        json!({"FILE-STATUS": "10", "RESULT-IMAGE": "", "PAST-END": 1}),                                  // READ NEXT past the end
        json!({"FILE-STATUS": "23", "RESULT-IMAGE": "", "PAST-END": 0}),                                  // READ by key, not found
        json!({"FILE-STATUS": "23", "RESULT-IMAGE": "", "PAST-END": 0}),                                  // REWRITE refused
        json!({"FILE-STATUS": "00", "RESULT-IMAGE": "", "PAST-END": 0}),                                  // DELETE
        json!({"FILE-STATUS": "00", "RESULT-IMAGE": "", "PAST-END": 0}),                                  // CLOSE
    ];
    let (shown, sent) = run(&program(body), answers);
    let programs: Vec<&str> = sent.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(
        programs,
        vec![
            "RC-4-SHOP-5-IXF-1-OPEN",
            "RC-4-SHOP-5-IXF-1-WRITE",
            "RC-4-SHOP-5-IXF-1-START",
            "RC-4-SHOP-5-IXF-1-READ-NEXT",
            "RC-4-SHOP-5-IXF-1-READ-NEXT",
            "RC-4-SHOP-5-IXF-1-READ-INVALID",
            "RC-4-SHOP-5-IXF-1-REWRITE",
            "RC-4-SHOP-5-IXF-1-DELETE",
            "RC-4-SHOP-5-IXF-1-CLOSE",
        ]
    );
    assert_eq!(sent[0].1["KEY-MODE"], "I-O");
    assert_eq!(sent[1].1["RECORD-IMAGE"].as_str().unwrap().trim_end(), image("7", "bolt", "12").trim_end(), "WRITE sends the record area");
    assert_eq!(sent[2].1["KEY-MODE"], "GE", "START KEY >= positions GE");
    // The first READ NEXT continues from where START positioned; the second from the record read.
    assert_eq!(sent[3].1["KEY-MODE"], "GE");
    assert_eq!(sent[4].1["KEY-MODE"], "GT");
    assert!(sent[4].1["RECORD-IMAGE"].as_str().unwrap().starts_with("00007bolt"), "the cursor is the record read: {}", sent[4].1);
    assert_eq!(sent[5].1["KEY-MODE"].as_str().unwrap().trim(), "", "a keyed read does not position");
    assert_eq!(shown, vec!["WRITTEN", "GOT bolt", "EOF", "NOT-FOUND 23", "NO-REWRITE"]);
    println!(
        "spec 111 lowering: 9 verbs of OPEN, WRITE, START, READ NEXT x2, READ by key, REWRITE, DELETE, CLOSE became 9 calls to 9 endpoints; \
         phrases run on the answered status (WRITTEN, GOT, EOF, NOT-FOUND 23, NO-REWRITE); {} ms",
        started.elapsed().as_millis()
    );
}

#[test]
fn a_failed_call_leaves_status_30_and_runs_no_phrase() {
    struct Down;
    impl Transport for Down {
        fn call(&mut self, _: &CallRequest) -> Result<CallReply, CallError> {
            Err(CallError::validation("down".to_string(), "RC-IX-RESULT"))
        }
    }
    let mut p = parse(&program(
        "           OPEN INPUT PARTS
           DISPLAY \"STATUS \" WS-IXF-1-STATUS
           READ PARTS NEXT AT END DISPLAY \"EOF\" NOT AT END DISPLAY \"GOT\" END-READ
",
    ));
    lower_indexed_files(&mut p, &parts_form()).unwrap();
    let (_etx, erx) = mpsc::channel();
    let (stx, _srx) = mpsc::channel();
    let (dtx, drx) = mpsc::channel();
    let mut i = Interpreter::new_with_channels(p, erx, stx, dtx);
    i.set_remote_transport(Box::new(Down));
    i.run().unwrap();
    let shown: Vec<String> = drx.try_iter().map(|s| s.trim().to_owned()).collect();
    assert_eq!(shown, vec!["STATUS 30"], "the file status says the call failed; neither AT END nor NOT AT END ran");
}

#[test]
fn what_the_backend_cannot_serve_is_refused_at_the_developers_line() {
    let cases = [
        ("           ROLLBACK\n", "ROLLBACK"),
        ("           START PARTS KEY IS NOT = PART-ID\n", "NOT ="),
    ];
    for (body, what) in cases {
        let mut p = parse(&program(body));
        let errs = lower_indexed_files(&mut p, &parts_form()).expect_err(what);
        assert!(errs.iter().any(|e| e.message.contains(what) && e.line > 0), "{what}: {errs:?}");
    }
}

#[test]
fn a_program_without_the_file_is_left_alone() {
    let mut p = parse("       IDENTIFICATION DIVISION.\n       PROGRAM-ID. SHOP.\n       PROCEDURE DIVISION.\n           STOP RUN.\n");
    assert_eq!(lower_indexed_files(&mut p, &parts_form()).unwrap(), 0);
}
