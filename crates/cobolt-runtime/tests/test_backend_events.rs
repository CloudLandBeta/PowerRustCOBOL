// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-014 T9 (R2, R6): what a transport reports about the backend's version and about a warning reaches the
//! application's MAIN form as `onBackendVersion` / `onBackendWarning`, whichever interpreter made the call.
//! Its own test binary: the main form's address is process-wide (`set_main_form_events`).

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::interpreter::remote::set_main_form_events;
use cobolt_runtime::{FormEvent, Interpreter};
use cobolt_wire::call::{CallObserver, CallReply, CallRequest, Transport};
use cobolt_wire::error::CallError;

const SRC: &str = "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. CALLER.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 ORDER-REC.\n          05 AMOUNT PIC 9(4).\n       PROCEDURE DIVISION.\n       MAIN-PARA.\n           MOVE 5 TO AMOUNT\n           CALL \"CHARGE\" WITH WEBSOCKET LINKAGE USING BY REFERENCE ORDER-REC ON EXCEPTION DISPLAY \"failed\" NOT ON EXCEPTION DISPLAY \"ok\" END-CALL\n           STOP RUN.\n";

struct Rig;

impl Transport for Rig {
    fn call(&mut self, r: &CallRequest) -> Result<CallReply, CallError> {
        self.call_observed(r, &mut cobolt_wire::call::NoObserver)
    }
    fn call_observed(&mut self, _r: &CallRequest, o: &mut dyn CallObserver) -> Result<CallReply, CallError> {
        o.backend_version("warn", "this application is older than the backend", "1.2.0");
        o.warning("CHARGE is deprecated: use CHARGE-V2");
        Ok(CallReply { args: vec![Some("{\"AMOUNT\":5}".into())], warnings: vec!["CHARGE is deprecated: use CHARGE-V2".into()], ..CallReply::default() })
    }
}

#[test]
fn a_version_notice_and_a_warning_reach_the_main_form_as_events() {
    let (main_tx, main_rx) = mpsc::channel::<FormEvent>();
    set_main_form_events(main_tx, "MAIN-FORM");
    let parsed = parse(tokenize(SRC, SourceFormat::Free));
    // A child form's interpreter: its own channels, nothing to do with the main form's.
    let (_e, event_rx) = mpsc::channel();
    let (state_tx, _s) = mpsc::channel();
    let (display_tx, _d) = mpsc::channel();
    let mut child = Interpreter::new_with_channels(parsed.program.expect("program"), event_rx, state_tx, display_tx);
    child.set_remote_transport(Box::new(Rig));
    child.run().unwrap();
    let got: Vec<FormEvent> = main_rx.try_iter().collect();
    for e in &got {
        println!("main form hears {} on {} with {:?}", e.event_id, e.ctrl_id, e.value);
    }
    assert_eq!(got.len(), 2);
    assert_eq!((got[0].ctrl_id.as_str(), got[0].event_id.as_str(), got[0].value.as_str()), ("MAIN-FORM", "onBackendVersion", "warn\tthis application is older than the backend\t1.2.0"));
    assert_eq!((got[1].event_id.as_str(), got[1].value.as_str()), ("onBackendWarning", "CHARGE\tCHARGE is deprecated: use CHARGE-V2"));
}
