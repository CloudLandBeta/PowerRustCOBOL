// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 111: an `IndexedFile` control's verbs on a web form reach the remote
//! file **through the backend**, over the real channel — the web build's
//! lowering of the form's program (`cobolt-compiler`'s `web_remote_indexed`),
//! the interpreter's `CALL … WITH WEBSOCKET LINKAGE`, `rcrun`'s WebSocket
//! client with a bearer token, a TLS loopback server and the programs the Build
//! generates for the control (`cobolt-codegen`'s `remote_controls::indexed`).
//! A call without a token is refused.

mod support;

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use cobolt_codegen::remote_controls::{fixtures, generate_backend_serving};
use cobolt_compiler::web_remote_indexed::lower_indexed_files;
use cobolt_runtime::Interpreter;
use cobolt_server::client::{ClientConfig, SpecToken, WsClient};
use cobolt_wire::call::{ArgMode, ArgPayload, CallObserver, CallReply, CallRequest, Transport};
use cobolt_wire::error::CallError;
use serde_json::{json, Value};
use support::*;

const MY_EXPORTS: &str = "[contract]\nversion = \"1.0.0\"\n";
const PARTS: usize = 20;

/// The web form's program: what a developer writes for the control, verbs and
/// phrases as on the desktop.
const SHOP: &str = r#"       IDENTIFICATION DIVISION.
       PROGRAM-ID. SHOP.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT PARTS ASSIGN TO "data/parts.idx"
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
       01 WS-I PIC 9(3).
       01 WS-IXF-1-STATUS PIC XX VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN-PARA.
           OPEN OUTPUT PARTS
           CLOSE PARTS
           OPEN I-O PARTS
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 20
               MOVE WS-I TO PART-ID
               MOVE "part" TO PART-NAME
               MOVE WS-I TO PART-QTY
               WRITE PART-REC
                   INVALID KEY DISPLAY "WRITE-REFUSED " PART-ID
               END-WRITE
           END-PERFORM
           DISPLAY "WRITTEN"
           MOVE 5 TO PART-ID
           WRITE PART-REC
               INVALID KEY DISPLAY "DUPLICATE " WS-IXF-1-STATUS
               NOT INVALID KEY DISPLAY "NOT-REFUSED"
           END-WRITE
           MOVE 18 TO PART-ID
           START PARTS KEY IS >= PART-ID
               INVALID KEY DISPLAY "NO-START"
           END-START
           PERFORM UNTIL WS-IXF-1-STATUS NOT = "00"
               READ PARTS NEXT
                   AT END DISPLAY "END-OF-FILE"
                   NOT AT END DISPLAY "NEXT " PART-ID
               END-READ
           END-PERFORM
           MOVE 7 TO PART-ID
           READ PARTS
               INVALID KEY DISPLAY "NOT-FOUND 7"
               NOT INVALID KEY DISPLAY "KEYED " PART-ID " " PART-NAME " " PART-QTY
           END-READ
           MOVE "renamed" TO PART-NAME
           MOVE 700 TO PART-QTY
           REWRITE PART-REC
               INVALID KEY DISPLAY "NOT-REWRITTEN"
           END-REWRITE
           MOVE 7 TO PART-ID
           READ PARTS
               INVALID KEY DISPLAY "NOT-FOUND 7"
               NOT INVALID KEY DISPLAY "AFTER " PART-NAME " " PART-QTY
           END-READ
           MOVE 8 TO PART-ID
           DELETE PARTS
               INVALID KEY DISPLAY "NOT-DELETED"
           END-DELETE
           MOVE 8 TO PART-ID
           READ PARTS
               INVALID KEY DISPLAY "GONE 8 " WS-IXF-1-STATUS
           END-READ
           MOVE 3 TO PART-ID
           START PARTS KEY IS <= PART-ID
               INVALID KEY DISPLAY "NO-START-LE"
           END-START
           READ PARTS PREVIOUS
               AT END DISPLAY "NO-PREVIOUS"
               NOT AT END DISPLAY "PREVIOUS " PART-ID
           END-READ
           CLOSE PARTS
           DISPLAY "DONE"
           STOP RUN.
"#;

fn parse(src: &str) -> cobolt_ast::program::Program {
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<_> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("{}: {}", d.span.line, d.message)).collect();
    assert!(errors.is_empty(), "the form's program parses: {errors:?}");
    parsed.program.expect("a program")
}

/// A `Transport` that remembers which programs it was asked for, then hands the
/// call to `rcrun`'s own WebSocket client.
struct Spy {
    inner: WsClient,
    seen: Arc<Mutex<Vec<String>>>,
}

impl Transport for Spy {
    fn call(&mut self, request: &CallRequest) -> Result<CallReply, CallError> {
        self.seen.lock().unwrap().push(request.program.clone());
        self.inner.call(request)
    }

    fn call_observed(&mut self, request: &CallRequest, observer: &mut dyn CallObserver) -> Result<CallReply, CallError> {
        self.seen.lock().unwrap().push(request.program.clone());
        self.inner.call_observed(request, observer)
    }
}

fn client(rig: &Rig, token: bool) -> WsClient {
    let mut cfg = ClientConfig::new(format!("wss://localhost:{}/ws", rig.server.addr.port()), "1.0.0");
    cfg.client_name = "remote-indexed-file-test".to_owned();
    cfg.extra_root_pem = Some(rig.cert.cert_pem.as_bytes().to_vec());
    if token {
        cfg.token = Some(Arc::new(SpecToken(rig.issuer.access("clerk-1", AUDIENCE, 600, json!({"roles": ["clerk"]})))));
    }
    cfg.origin = Some(ORIGIN.to_owned());
    cfg.interface = Some(rig.interface());
    WsClient::new(cfg)
}

#[test]
fn an_indexed_file_controls_verbs_reach_the_remote_file_with_a_token_and_are_refused_without_one() {
    let started = Instant::now();
    // ── The backend: the Build's generated programs for the control ──────────
    let dir = backend_dir("remote-indexed-file", MY_EXPORTS, &[]);
    std::fs::create_dir_all(dir.join("data")).unwrap();
    let data = dir.join("data").join("parts.idx");
    let data_text = data.to_string_lossy().into_owned();
    let form = fixtures::indexed_form("");
    let load = {
        let data_text = data_text.clone();
        move |_: &str| Ok(fixtures::parts_cidx(&data_text))
    };
    let generated = generate_backend_serving(std::slice::from_ref(&form), &load, &|_, _| true).expect("generates");
    let ids: Vec<&str> = generated.programs.iter().map(|p| p.program_id.as_str()).collect();
    assert_eq!(ids.len(), 9, "one program per verb: {ids:?}");
    let out = dir.join("generated");
    std::fs::create_dir_all(&out).unwrap();
    for p in &generated.programs {
        std::fs::write(out.join(&p.file_name), &p.source).unwrap();
    }
    let exports = generated.exports_toml();
    assert!(exports.contains("public = false"), "never public");
    assert!(!exports.contains("roles"), "any authenticated caller by default");
    std::fs::write(out.join("exports.toml"), exports).unwrap();
    let rig = rig(dir.clone(), "");
    let setup = started.elapsed();

    // ── The form's program, lowered as the web build lowers it ───────────────
    let mut program = parse(SHOP);
    let lowered = lower_indexed_files(&mut program, &form).expect("lowers");
    assert_eq!(lowered, 15, "the 15 verbs of the program became calls");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (_etx, erx) = mpsc::channel();
    let (stx, _srx) = mpsc::channel();
    let (dtx, drx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, erx, stx, dtx);
    interp.set_remote_transport(Box::new(Spy { inner: client(&rig, true), seen: Arc::clone(&seen) }));
    let run_started = Instant::now();
    interp.run().expect("the form runs");
    let ran = run_started.elapsed();
    let shown: Vec<String> = drx.try_iter().map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ")).collect();

    assert_eq!(
        shown,
        vec![
            "WRITTEN",
            "DUPLICATE 22",
            "NEXT 00018",
            "NEXT 00019",
            "NEXT 00020",
            "END-OF-FILE",
            "KEYED 00007 part                 0007",
            "AFTER renamed                  0700",
            "GONE 8 23",
            "PREVIOUS 00001",
            "DONE",
        ]
        .into_iter()
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>(),
        "what the form saw through the backend"
    );
    assert!(data.exists(), "the file lives with the backend");
    let calls = seen.lock().unwrap().clone();
    let count = |op: &str| calls.iter().filter(|p| p.ends_with(&format!("-IXF-1-{op}"))).count();
    assert_eq!((count("OPEN"), count("CLOSE"), count("WRITE"), count("REWRITE"), count("DELETE")), (2, 2, 21, 1, 1), "{calls:?}");
    assert_eq!(count("START"), 2);
    assert_eq!(count("READ-NEXT"), 4);
    assert_eq!(count("READ-PREVIOUS"), 1);
    assert_eq!(count("READ-INVALID"), 3);

    // ── The desktop's own answer for the same START and READ PREVIOUS, on the file the backend wrote ──
    let desktop = {
        let src = SHOP
            .replace("data/parts.idx", &data_text)
            .split("       PROCEDURE DIVISION.")
            .next()
            .unwrap()
            .to_owned()
            + "       PROCEDURE DIVISION.\n       MAIN-PARA.\n           OPEN INPUT PARTS\n           MOVE 3 TO PART-ID\n           START PARTS KEY IS <= PART-ID\n           READ PARTS PREVIOUS\n           DISPLAY \"PREVIOUS \" PART-ID\n           CLOSE PARTS\n           STOP RUN.\n";
        let (_e, erx) = mpsc::channel();
        let (stx, _s) = mpsc::channel();
        let (dtx, drx) = mpsc::channel();
        let mut local = Interpreter::new_with_channels(parse(&src), erx, stx, dtx);
        local.run().expect("the desktop path runs");
        drx.try_iter().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).collect::<Vec<_>>()
    };
    assert_eq!(shown[shown.len() - 2], desktop[0], "the backend answers READ PREVIOUS after START <= as the desktop does");

    // ── Without a token the same call is refused ─────────────────────────────
    let mut anonymous = client(&rig, false);
    let request = CallRequest {
        program: "RC-4-SHOP-5-IXF-1-READ-INVALID".to_owned(),
        args: vec![ArgPayload { mode: ArgMode::Value, json: json!({"KEY-MODE": "", "RECORD-IMAGE": "00007"}).to_string() }],
        wants_returning: true,
        idempotency_key: None,
    };
    let refused = anonymous.call(&request).expect_err("a call without a token is refused");
    let mut with_token = client(&rig, true);
    let answer: Value = serde_json::from_str(&with_token.call(&request).expect("the same call with a token").returning.unwrap()).unwrap();
    assert_eq!(answer["FILE-STATUS"], "00", "{answer}");
    assert!(answer["RESULT-IMAGE"].as_str().unwrap().starts_with("00007renamed"), "{answer}");
    println!(
        "spec 111 IndexedFile through the backend (real TLS channel, bearer token): form lowered to {lowered} calls; {} remote calls ran \
         (OPEN 2, CLOSE 2, WRITE 21 incl. 1 duplicate refused 22, START 2, READ NEXT 4, READ PREVIOUS 1, keyed READ 3, REWRITE 1, DELETE 1); \
         9 programs generated; without a token: refused ({refused}); setup {} ms, run {} ms ({:.0} calls/sec); 11/11 checks passed",
        calls.len(),
        setup.as_millis(),
        ran.as_millis(),
        calls.len() as f64 / ran.as_secs_f64().max(0.001)
    );
    let _ = PARTS;
    let _ = std::fs::remove_dir_all(&dir);
}
