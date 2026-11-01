// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `COBOL::"AUTH-OP"` (WASM-009 D4, D15): the one built-in a generated `AUTH-*`
//! program calls. With the backend's identity hook it sends the request group as
//! JSON and reads the answer into the result group; anywhere else — no backend
//! hooks at all (`rcrun run-form`, a child form, a compiled desktop binary), or
//! a backend call that did not come through the `/auth/*` routes — it is the same
//! run-time error.

use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc, Mutex};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::interpreter::remote::{BackendHooks, IdentityHook, AUTH_OP_OUTSIDE_BACKEND};
use cobolt_runtime::Interpreter;
use cobolt_wire::error::{codes, CallError, ErrorClass};

const PROGRAM: &str = "       IDENTIFICATION DIVISION.
       PROGRAM-ID. AUTH-SIGN-IN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 AUTH-REQUEST.
          05 USERNAME       PIC X(40).
          05 PASSWORD       PIC X(40).
       01 AUTH-RESULT.
          05 OUTCOME        PIC X(30).
          05 ACCESS-TOKEN   PIC X(60).
          05 EXPIRES-IN     PIC 9(9).
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE \"ana\" TO USERNAME
           MOVE \"right-password\" TO PASSWORD
           COBOL::\"AUTH-OP\" ( \"SIGN-IN\" AUTH-REQUEST AUTH-RESULT )
           DISPLAY OUTCOME
           DISPLAY ACCESS-TOKEN
           DISPLAY EXPIRES-IN
           STOP RUN.
";

fn interp() -> (Interpreter, mpsc::Receiver<String>) {
    let result = parse(tokenize(PROGRAM, SourceFormat::Free));
    assert!(result.diagnostics.iter().all(|d| d.severity != Severity::Error), "parse errors: {:?}", result.diagnostics);
    let (_e, event_rx) = mpsc::channel();
    let (state_tx, _s) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    (Interpreter::new_with_channels(result.program.expect("program"), event_rx, state_tx, display_tx), display_rx)
}

fn hooks(identity: Option<Arc<dyn IdentityHook>>) -> BackendHooks {
    BackendHooks { progress: None, emit: None, files: None, cancel_requested: Arc::new(AtomicBool::new(false)), identity }
}

struct Fake(Mutex<Vec<(String, String)>>);

impl IdentityHook for Fake {
    fn run(&self, op: &str, request: &str) -> Result<String, CallError> {
        self.0.lock().unwrap().push((op.to_string(), request.to_string()));
        if op == "SIGN-IN" {
            Ok(r#"{"OUTCOME":"OK","ACCESS-TOKEN":"at.ana.1","EXPIRES-IN":300}"#.to_string())
        } else {
            Err(CallError::new(ErrorClass::Server, codes::INTERNAL, "The sign-in service is not available; try again later."))
        }
    }
}

#[test]
fn without_the_backend_auth_op_is_the_same_run_time_error() {
    let (mut i, _rx) = interp();
    let e = i.run().expect_err("no backend").to_string();
    assert!(e.contains(AUTH_OP_OUTSIDE_BACKEND), "{e}");
    // A backend call that did not come through the /auth routes (a WebSocket export).
    let (mut i, _rx) = interp();
    i.set_backend_hooks(hooks(None));
    let e = i.run().expect_err("no identity hook").to_string();
    assert!(e.contains(AUTH_OP_OUTSIDE_BACKEND) && e.contains("/auth routes"), "{e}");
}

#[test]
fn with_the_hook_the_request_goes_as_json_and_the_answer_fills_the_result_group() {
    let fake = Arc::new(Fake(Mutex::new(Vec::new())));
    let (mut i, rx) = interp();
    i.set_backend_hooks(hooks(Some(fake.clone())));
    i.run().expect("the call runs");
    let shown: Vec<String> = rx.try_iter().map(|s| s.trim_end().to_string()).collect();
    assert_eq!(shown, vec!["OK", "at.ana.1", "000000300"]);
    let calls = fake.0.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "SIGN-IN");
    let req: serde_json::Value = serde_json::from_str(&calls[0].1).unwrap();
    assert_eq!(req, serde_json::json!({"USERNAME": "ana", "PASSWORD": "right-password"}));
}

#[test]
fn a_service_error_ends_the_call_with_its_message_and_no_detail() {
    let fake = Arc::new(Fake(Mutex::new(Vec::new())));
    let (mut i, _rx) = {
        let src = PROGRAM.replace("\"SIGN-IN\"", "\"REFRESH\"");
        let result = parse(tokenize(&src, SourceFormat::Free));
        let (_e, event_rx) = mpsc::channel();
        let (state_tx, _s) = mpsc::channel();
        let (display_tx, display_rx) = mpsc::channel();
        (Interpreter::new_with_channels(result.program.unwrap(), event_rx, state_tx, display_tx), display_rx)
    };
    i.set_backend_hooks(hooks(Some(fake)));
    let e = i.run().expect_err("the service failed").to_string();
    assert!(e.contains("REFRESH") && e.contains("not available"), "{e}");
}
