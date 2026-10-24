// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `CALL … WITH WEBSOCKET LINKAGE` through the interpreter, against a loopback
//! transport double (WASM-005 AC4, AC5, AC10).
//!
//! The double is a closure from request to reply, so each case states exactly
//! what the backend would have answered. What is pinned: the JSON the
//! interpreter sends for a group; that `BY VALUE` never changes the caller's
//! copy, `BY REFERENCE` writes the answer back and `RETURNING` fills its group,
//! alone or together; that a reply that does not fit changes nothing; and that
//! every failure runs `ON EXCEPTION` with its class readable, while success runs
//! `NOT ON EXCEPTION`.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;
use cobolt_wire::call::{CallReply, CallRequest, Transport};
use cobolt_wire::error::{codes, CallError, ErrorClass};

type Handler = Box<dyn FnMut(&CallRequest) -> Result<CallReply, CallError> + Send>;

/// A transport that answers with a closure and remembers what it was asked.
struct Double {
    handler: Handler,
    seen: Arc<Mutex<Vec<CallRequest>>>,
}

impl Transport for Double {
    fn call(&mut self, request: &CallRequest) -> Result<CallReply, CallError> {
        self.seen.lock().unwrap().push(request.clone());
        (self.handler)(request)
    }
}

struct Run {
    shown: Vec<String>,
    seen: Vec<CallRequest>,
    result: Result<(), String>,
}

fn run_with(src: &str, handler: Option<Handler>) -> Run {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    let seen = Arc::new(Mutex::new(Vec::new()));
    if let Some(handler) = handler {
        interp.set_remote_transport(Box::new(Double { handler, seen: seen.clone() }));
    }
    let outcome = interp.run().map_err(|e| e.to_string());
    let shown = display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect();
    let seen = seen.lock().unwrap().clone();
    Run { shown, seen, result: outcome }
}

fn field(out: &[String], name: &str) -> String {
    out.iter()
        .find_map(|l| l.strip_prefix(&format!("{name}=[")).and_then(|r| r.strip_suffix(']')).map(|v| v.trim_end().to_owned()))
        .unwrap_or_else(|| panic!("expected {name}=[…]; got {out:?}"))
}

/// A caller whose paragraph is `body`, with the data every case shares.
fn caller(body: &str) -> String {
    format!(
        "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 CUSTOMER.
          05 CUST-ID    PIC 9(6).
          05 CUST-NAME  PIC X(12).
          05 BALANCE    PIC S9(5)V99.
          05 ORDERS     OCCURS 3.
             10 ORDER-NO  PIC 9(4).
       01 RESULT-REC.
          05 RC-CODE    PIC 99.
          05 RC-TEXT    PIC X(10).
       01 CLASS-NAME PIC X(16).
       01 ERR-CODE   PIC 9(5).
       01 ERR-TEXT   PIC X(60).
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE 7 TO CUST-ID
           MOVE \"Ana\" TO CUST-NAME
           MOVE 12.5 TO BALANCE
           MOVE 1001 TO ORDER-NO (1)
{body}
           STOP RUN.
"
    )
}

const SHOW: &str = "
           DISPLAY \"ID=[\" CUST-ID \"]\"
           DISPLAY \"NAME=[\" CUST-NAME \"]\"
           DISPLAY \"BAL=[\" BALANCE \"]\"
           DISPLAY \"O1=[\" ORDER-NO (1) \"]\"
           DISPLAY \"O2=[\" ORDER-NO (2) \"]\"
           DISPLAY \"RC=[\" RC-CODE \"]\"
           DISPLAY \"RT=[\" RC-TEXT \"]\"";

fn ok_reply(args: Vec<Option<&str>>, returning: Option<&str>) -> Result<CallReply, CallError> {
    Ok(CallReply {
        args: args.into_iter().map(|a| a.map(str::to_owned)).collect(),
        returning: returning.map(str::to_owned),
        warnings: Vec::new(),
    })
}

#[test]
fn the_group_goes_out_as_json_built_from_its_declaration() {
    let src = caller(
        "
           *> GET-CUSTOMER: asks the backend for one customer; CUSTOMER carries the key and receives the record.
           *> Uses:   CUST-ID = 7 (set above).
           *> Result: CUSTOMER = the record the backend sent back.

           CALL \"get-customer\" WITH WEBSOCKET LINKAGE USING BY REFERENCE CUSTOMER
               ON EXCEPTION DISPLAY \"FAILED\"
           END-CALL",
    );
    let run = run_with(&src, Some(Box::new(|_| ok_reply(vec![None], None))));
    run.result.expect("run");
    assert_eq!(run.seen.len(), 1);
    let req = &run.seen[0];
    assert_eq!(req.program, "GET-CUSTOMER", "the name is upper-cased");
    assert!(!req.wants_returning);
    assert_eq!(
        req.args[0].json,
        "{\"CUST-ID\":7,\"CUST-NAME\":\"Ana\",\"BALANCE\":12.50,\"ORDERS\":[{\"ORDER-NO\":1001},{\"ORDER-NO\":0},{\"ORDER-NO\":0}]}"
    );
}

#[test]
fn by_value_never_changes_the_callers_copy() {
    let src = caller(&format!(
        "
           CALL \"RENAME\" WITH WEBSOCKET LINKAGE USING BY VALUE CUSTOMER
           {SHOW}"
    ));
    // The backend answers with a changed group; BY VALUE must ignore it.
    let run = run_with(&src, Some(Box::new(|_| ok_reply(vec![Some("{\"CUST-NAME\":\"CHANGED\"}")], None))));
    run.result.expect("run");
    assert_eq!(field(&run.shown, "NAME"), "Ana", "the caller's copy is untouched");
}

#[test]
fn by_reference_writes_the_answer_back_by_name_and_leaves_absent_members_alone() {
    let src = caller(&format!(
        "
           CALL \"RENAME\" WITH WEBSOCKET LINKAGE USING BY REFERENCE CUSTOMER
           {SHOW}"
    ));
    let run = run_with(
        &src,
        Some(Box::new(|_| ok_reply(vec![Some("{\"BALANCE\":-3.05,\"CUST-NAME\":\"Beatriz\",\"ORDERS\":[{},{\"ORDER-NO\":2002}]}")], None))),
    );
    run.result.expect("run");
    assert_eq!(field(&run.shown, "NAME"), "Beatriz");
    assert_eq!(field(&run.shown, "O2"), "2002");
    assert_eq!(field(&run.shown, "O1"), "1001", "an element the answer left empty is unchanged");
    assert_eq!(field(&run.shown, "ID"), "000007", "a member the answer omits is unchanged");
}

#[test]
fn returning_fills_its_group_and_by_reference_is_the_default() {
    let src = caller(&format!(
        "
           CALL \"CHECK-CUSTOMER\" WITH WEBSOCKET LINKAGE USING CUSTOMER RETURNING RESULT-REC
           {SHOW}"
    ));
    let run = run_with(
        &src,
        Some(Box::new(|_| ok_reply(vec![Some("{\"CUST-NAME\":\"Checked\"}")], Some("{\"RC-CODE\":3,\"RC-TEXT\":\"refused\"}")))),
    );
    run.result.expect("run");
    assert!(run.seen[0].wants_returning);
    assert_eq!(field(&run.shown, "NAME"), "Checked", "no BY phrase means BY REFERENCE");
    assert_eq!(field(&run.shown, "RC"), "03");
    assert_eq!(field(&run.shown, "RT"), "refused");
}

#[test]
fn by_value_and_returning_together() {
    let src = caller(&format!(
        "
           CALL \"CHECK-CUSTOMER\" WITH WEBSOCKET LINKAGE USING BY VALUE CUSTOMER RETURNING RESULT-REC
           {SHOW}"
    ));
    let run = run_with(&src, Some(Box::new(|_| ok_reply(vec![None], Some("{\"RC-CODE\":9}")))));
    run.result.expect("run");
    assert_eq!(field(&run.shown, "NAME"), "Ana");
    assert_eq!(field(&run.shown, "RC"), "09");
}

#[test]
fn an_elementary_item_is_a_bare_scalar() {
    let src = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 AMOUNT PIC S9(5)V99.
       01 LABEL-TXT PIC X(8).
       01 AMOUNT-ED PIC ZZZZ9.99.
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE 19.9 TO AMOUNT
           MOVE \"abc\" TO LABEL-TXT
           CALL \"PRICE\" WITH WEBSOCKET LINKAGE USING BY REFERENCE AMOUNT BY VALUE LABEL-TXT
           MOVE AMOUNT TO AMOUNT-ED
           DISPLAY \"AMOUNT=[\" AMOUNT-ED \"]\"
           STOP RUN.
";
    let run = run_with(src, Some(Box::new(|_| ok_reply(vec![Some("23.45"), None], None))));
    run.result.expect("run");
    assert_eq!(run.seen[0].args[0].json, "19.90");
    assert_eq!(run.seen[0].args[1].json, "\"abc\"");
    assert_eq!(field(&run.shown, "AMOUNT"), "   23.45");
}

#[test]
fn a_reply_that_does_not_fit_changes_nothing_in_any_group() {
    let src = caller(&format!(
        "
           CALL \"CHECK-CUSTOMER\" WITH WEBSOCKET LINKAGE USING CUSTOMER RETURNING RESULT-REC
               ON EXCEPTION
                   COBOL::\"WS-LAST-ERROR\" ( CLASS-NAME ERR-CODE ERR-TEXT )
                   DISPLAY \"CLASS=[\" CLASS-NAME \"]\"
                   DISPLAY \"CODE=[\" ERR-CODE \"]\"
           END-CALL
           {SHOW}"
    ));
    // The first argument's answer is fine, the RETURNING answer names an unknown member.
    let run = run_with(
        &src,
        Some(Box::new(|_| ok_reply(vec![Some("{\"CUST-NAME\":\"Nope\"}")], Some("{\"NOT-A-MEMBER\":1}")))),
    );
    run.result.expect("run");
    assert_eq!(field(&run.shown, "CLASS"), "VALIDATION");
    assert_eq!(field(&run.shown, "CODE"), "04000");
    assert_eq!(field(&run.shown, "NAME"), "Ana", "the good answer was not written either");
}

#[test]
fn every_failure_class_reaches_on_exception_with_class_code_and_message() {
    let cases: Vec<(CallError, &str, &str)> = vec![
        (CallError::transport(codes::NO_CONNECTION, "no connection"), "TRANSPORT", "01000"),
        (CallError::new(ErrorClass::Authentication, codes::TOKEN_EXPIRED, "token expired"), "AUTHENTICATION", "02002"),
        (CallError::new(ErrorClass::Authorisation, codes::ROLE_REFUSED, "role refused"), "AUTHORISATION", "03000"),
        (CallError::validation("bad payload", "REC.X"), "VALIDATION", "04000"),
        (CallError::new(ErrorClass::NotFound, codes::UNKNOWN_PROGRAM, "unknown program"), "NOT-FOUND", "05000"),
        (CallError::new(ErrorClass::Version, codes::CLIENT_TOO_OLD, "update"), "VERSION", "06000"),
        (CallError::new(ErrorClass::Limit, codes::RATE_LIMITED, "slow down"), "LIMIT", "07001"),
        (CallError::new(ErrorClass::Application, 42, "duplicate key").with_detail("{\"KEY\":\"A1\"}"), "APPLICATION", "00042"),
        (CallError::new(ErrorClass::Server, codes::INTERNAL, "boom"), "SERVER", "09000"),
    ];
    let src = caller(
        "
           CALL \"ANY\" WITH WEBSOCKET LINKAGE USING BY VALUE CUSTOMER
               ON EXCEPTION
                   COBOL::\"WS-LAST-ERROR\" ( CLASS-NAME ERR-CODE ERR-TEXT )
                   DISPLAY \"CLASS=[\" CLASS-NAME \"]\"
                   DISPLAY \"CODE=[\" ERR-CODE \"]\"
                   DISPLAY \"TEXT=[\" ERR-TEXT \"]\"
                   MOVE RETURN-CODE TO ERR-CODE
                   DISPLAY \"RCODE=[\" ERR-CODE \"]\"
               NOT ON EXCEPTION
                   DISPLAY \"OK\"
           END-CALL",
    );
    for (err, class, code) in cases {
        let message = err.message.clone();
        let e = err.clone();
        let run = run_with(&src, Some(Box::new(move |_| Err(e.clone()))));
        run.result.expect("run");
        assert_eq!(field(&run.shown, "CLASS"), class);
        assert_eq!(field(&run.shown, "CODE"), code);
        assert_eq!(field(&run.shown, "RCODE"), code, "RETURN-CODE carries the built-in's code");
        assert_eq!(field(&run.shown, "TEXT"), message);
        assert!(!run.shown.iter().any(|l| l == "OK"), "NOT ON EXCEPTION must not run after a failure");
    }
}

#[test]
fn success_runs_not_on_exception_and_clears_the_last_error() {
    let src = caller(
        "
           CALL \"ANY\" WITH WEBSOCKET LINKAGE USING BY VALUE CUSTOMER
               ON EXCEPTION DISPLAY \"FIRST-FAILED\"
           END-CALL
           CALL \"ANY\" WITH WEBSOCKET LINKAGE USING BY VALUE CUSTOMER
               ON EXCEPTION DISPLAY \"FAILED\"
               NOT ON EXCEPTION DISPLAY \"OK\"
           END-CALL
           COBOL::\"WS-LAST-ERROR\" ( CLASS-NAME ERR-CODE ERR-TEXT )
           DISPLAY \"CLASS=[\" CLASS-NAME \"]\"
           DISPLAY \"CODE=[\" ERR-CODE \"]\"
           MOVE RETURN-CODE TO ERR-CODE
           DISPLAY \"RCODE=[\" ERR-CODE \"]\"",
    );
    // The first call fails, so both mechanisms hold a code; the second
    // succeeds and must clear both.
    let mut calls = 0;
    let run = run_with(
        &src,
        Some(Box::new(move |_| {
            calls += 1;
            if calls == 1 {
                Err(CallError::new(ErrorClass::Limit, codes::RATE_LIMITED, "slow down"))
            } else {
                ok_reply(vec![None], None)
            }
        })),
    );
    run.result.expect("run");
    assert!(run.shown.iter().any(|l| l == "FIRST-FAILED"), "the first call must fail: {:?}", run.shown);
    assert!(run.shown.iter().any(|l| l == "OK"));
    assert!(!run.shown.iter().any(|l| l == "FAILED"));
    assert_eq!(field(&run.shown, "CLASS"), "");
    assert_eq!(field(&run.shown, "CODE"), "00000");
    assert_eq!(field(&run.shown, "RCODE"), "00000", "a success sets RETURN-CODE to 0");
}

#[test]
fn without_a_transport_the_call_fails_as_not_configured() {
    let src = caller(
        "
           CALL \"ANY\" WITH WEBSOCKET LINKAGE USING BY VALUE CUSTOMER
               ON EXCEPTION
                   COBOL::\"WS-LAST-ERROR\" ( CLASS-NAME ERR-CODE ERR-TEXT )
                   DISPLAY \"CLASS=[\" CLASS-NAME \"]\"
                   DISPLAY \"CODE=[\" ERR-CODE \"]\"
           END-CALL",
    );
    let run = run_with(&src, None);
    run.result.expect("run");
    assert_eq!(field(&run.shown, "CLASS"), "TRANSPORT");
    assert_eq!(field(&run.shown, "CODE"), "01003");
}

#[test]
fn a_failure_without_on_exception_is_a_run_time_error() {
    let src = caller(
        "
           CALL \"ANY\" WITH WEBSOCKET LINKAGE USING BY VALUE CUSTOMER
           DISPLAY \"NEVER\"",
    );
    let run = run_with(&src, Some(Box::new(|_| Err(CallError::new(ErrorClass::Server, codes::INTERNAL, "boom")))));
    let err = run.result.expect_err("a failed call with no ON EXCEPTION must stop the run");
    assert!(err.contains("WITH WEBSOCKET LINKAGE failed") && err.contains("boom"), "{err}");
    assert!(!run.shown.iter().any(|l| l == "NEVER"));
}

#[test]
fn a_group_whose_count_is_outside_its_table_is_refused_before_sending() {
    let src = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 BATCH.
          05 N PIC 99.
          05 ITEM PIC 9(3) OCCURS 1 TO 4 DEPENDING ON N.
       01 CLASS-NAME PIC X(16).
       01 ERR-CODE   PIC 9(5).
       01 ERR-TEXT   PIC X(60).
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE 9 TO N
           CALL \"ADD-ALL\" WITH WEBSOCKET LINKAGE USING BATCH
               ON EXCEPTION
                   COBOL::\"WS-LAST-ERROR\" ( CLASS-NAME ERR-CODE ERR-TEXT )
                   DISPLAY \"CLASS=[\" CLASS-NAME \"]\"
           END-CALL
           STOP RUN.
";
    let run = run_with(src, Some(Box::new(|_| ok_reply(vec![None], None))));
    run.result.expect("run");
    assert_eq!(field(&run.shown, "CLASS"), "VALIDATION");
    assert!(run.seen.is_empty(), "nothing was sent");
}

#[test]
fn a_call_inside_a_nested_handler_finds_the_forms_group() {
    let src = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FORM-PROG.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 PICK GLOBAL.
          05 KEY-ID PIC 9(3).
          05 KEY-TXT PIC X(6).
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE 5 TO KEY-ID
           CALL \"HANDLER\"
           DISPLAY \"TXT=[\" KEY-TXT \"]\"
           STOP RUN.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. HANDLER IS COMMON.
       PROCEDURE DIVISION.
           CALL \"LOOKUP\" WITH WEBSOCKET LINKAGE USING BY REFERENCE PICK
           GOBACK.
       END PROGRAM HANDLER.
       END PROGRAM FORM-PROG.
";
    let run = run_with(src, Some(Box::new(|_| ok_reply(vec![Some("{\"KEY-TXT\":\"found\"}")], None))));
    run.result.expect("run");
    assert_eq!(run.seen[0].args[0].json, "{\"KEY-ID\":5,\"KEY-TXT\":\"\"}");
    assert_eq!(field(&run.shown, "TXT"), "found");
}
