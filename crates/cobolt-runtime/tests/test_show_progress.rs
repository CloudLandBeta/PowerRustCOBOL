// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-014 T13 (R13, D-Progress): `COBOL::"WS-SHOW-PROGRESS"` declares where the next remote call shows its progress. The host-side
//! pieces the interpreter owns are checked here: each progress frame sets the declared controls, a click on the cancel control
//! ends the pending call with `application 8000`, and the declaration is spent when the call ends.

use std::sync::{mpsc, Arc, Mutex};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::interpreter::remote::cancel_control_clicked;
use cobolt_runtime::{Interpreter, StateUpdate};
use cobolt_wire::call::{CallObserver, CallReply, CallRequest, Transport};
use cobolt_wire::error::{codes, CallError, ErrorClass};

fn caller(body: &str) -> String {
    format!("       IDENTIFICATION DIVISION.\n       PROGRAM-ID. CALLER.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 ORDER-REC.\n          05 AMOUNT PIC 9(4).\n       01 WS-CLASS PIC X(16).\n       01 WS-CODE PIC X(8).\n       01 WS-MSG PIC X(120).\n       PROCEDURE DIVISION.\n       MAIN-PARA.\n           MOVE 5 TO AMOUNT\n{body}\n           STOP RUN.\n")
}

const CALL: &str = "           CALL \"BIG-JOB\" WITH WEBSOCKET LINKAGE USING BY REFERENCE ORDER-REC ON EXCEPTION DISPLAY \"failed\" NOT ON EXCEPTION DISPLAY \"ok\" END-CALL";

type Script = Box<dyn FnMut(&mut dyn CallObserver) -> Result<CallReply, CallError> + Send>;

struct Rig(Script);

impl Transport for Rig {
    fn call(&mut self, r: &CallRequest) -> Result<CallReply, CallError> {
        self.call_observed(r, &mut cobolt_wire::call::NoObserver)
    }
    fn call_observed(&mut self, _r: &CallRequest, o: &mut dyn CallObserver) -> Result<CallReply, CallError> {
        (self.0)(o)
    }
}

fn controls() -> Vec<(String, String, Vec<(String, String)>)> {
    vec![
        ("PRG-1".into(), "ProgressBar".into(), vec![]),
        ("LBL-MSG".into(), "Label".into(), vec![]),
        ("BTN-CANCEL".into(), "Button".into(), vec![]),
    ]
}

fn run(body: &str, script: Script) -> (Vec<StateUpdate>, Vec<String>) {
    let parsed = parse(tokenize(&caller(body), SourceFormat::Free));
    let (_e, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut i = Interpreter::new_with_channels(parsed.program.expect("program"), event_rx, state_tx, display_tx);
    i.seed_objects(controls());
    i.set_remote_transport(Box::new(Rig(script)));
    i.run().unwrap();
    (state_rx.try_iter().collect(), display_rx.try_iter().map(|s| s.trim_end().to_string()).collect())
}

#[test]
fn progress_frames_reach_the_declared_controls() {
    let (updates, shown) = run(
        &format!("           COBOL::\"WS-SHOW-PROGRESS\" ( PRG-1 LBL-MSG BTN-CANCEL )\n{CALL}\n{CALL}"),
        Box::new(|o| {
            o.progress(25, "reading");
            o.progress(100, "done");
            Ok(CallReply { args: vec![Some("{\"AMOUNT\":5}".into())], ..CallReply::default() })
        }),
    );
    let seen: Vec<String> = updates.iter().map(|u| format!("{}.{}={}", u.ctrl_id, u.prop, u.value)).collect();
    println!("progress seen by the host: {seen:?}");
    assert_eq!(seen, ["PRG-1.Value=25", "LBL-MSG.Caption=reading", "PRG-1.Value=100", "LBL-MSG.Caption=done"], "the second call, with no declaration, shows nothing");
    assert_eq!(shown, ["ok", "ok"]);
}

#[test]
fn cancel_control_ends_the_call_with_application_8000() {
    let (_, shown) = run(
        &format!("           COBOL::\"WS-SHOW-PROGRESS\" ( PRG-1 LBL-MSG BTN-CANCEL )\n{CALL}\n           COBOL::\"WS-LAST-ERROR\" ( WS-CLASS WS-CODE WS-MSG )\n           DISPLAY \"class=\" WS-CLASS \" code=\" WS-CODE"),
        Box::new(|o| {
            o.progress(10, "working");
            // The host, on its own thread, sees the click on the declared button while the interpreter is blocked here.
            assert!(cancel_control_clicked("", "BTN-CANCEL", "onClick"), "the click is recognised as the cancel control's");
            assert!(o.cancel_requested(), "and asks the pending call to cancel");
            Err(CallError::new(ErrorClass::Application, codes::CANCELLED, "the call was cancelled"))
        }),
    );
    println!("after the cancel: {shown:?}");
    assert_eq!(shown[0], "failed");
    assert!(shown[1].contains("APPLICATION") && shown[1].contains("8000"), "{shown:?}");
    // Spent: a click now is nobody's.
    assert!(!cancel_control_clicked("", "BTN-CANCEL", "onClick"), "the declaration ended with the call");
}

#[test]
fn a_control_the_form_does_not_have_fails_the_call_naming_it() {
    let ran = Arc::new(Mutex::new(false));
    let r2 = ran.clone();
    let (_, shown) = run(
        &format!("           COBOL::\"WS-SHOW-PROGRESS\" ( NO-SUCH )\n{CALL}\n           COBOL::\"WS-LAST-ERROR\" ( WS-CLASS WS-CODE WS-MSG )\n           DISPLAY \"msg=\" WS-MSG"),
        Box::new(move |_| {
            *r2.lock().unwrap() = true;
            Ok(CallReply::default())
        }),
    );
    println!("{shown:?}");
    assert!(!*ran.lock().unwrap(), "the call never reached the backend");
    assert_eq!(shown[0], "failed");
    assert!(shown[1].contains("NO-SUCH"), "{shown:?}");
}
