// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! R13: apart from its host calls the module is deterministic — the same
//! events and the same answers (`now`, `random`) give the same batches, byte
//! for byte, in each memory variant.

use cobolt_wasm::host::{FormHost, HostOptions};
use cobolt_wasm::module::{compile_project, FormSpec};
use cobolt_wasm::Options;
use cobolt_wasm_abi::scan::scan_module;

const SRC: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. DET-FORM.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
01 COBOL-QUIT PIC 9 VALUE 0.\n01 COBOL-EVENT-ID PIC X(64) VALUE SPACES.\n01 COBOL-CONTROL-ID PIC X(64) VALUE SPACES.\n01 FORM-NAME PIC X(64) VALUE 'DET-FORM'.\n\
01 WS-NOW PIC X(21).\n01 WS-TIME PIC 9(8).\n01 WS-R PIC 9V9(6).\n01 WS-N PIC 9(4) VALUE 0.\n01 WS-SUM PIC 9(8)V99 VALUE 0.\n\
PROCEDURE DIVISION.\nCOBOL-MAIN.\n    COBOL::\"INIT-FORM\" ( FORM-NAME )\n\
    PERFORM UNTIL COBOL-QUIT = 1\n        COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )\n\
        IF COBOL-CONTROL-ID = \"BTN-GO\"\n\
            ADD 1 TO WS-N\n            MOVE FUNCTION CURRENT-DATE TO WS-NOW\n            ACCEPT WS-TIME FROM TIME\n\
            COMPUTE WS-R = FUNCTION RANDOM\n            COMPUTE WS-SUM = WS-SUM + WS-R * 1000 + WS-N\n\
            MOVE WS-NOW TO LBL-NOW::Caption\n            MOVE WS-SUM TO LBL-SUM::Caption\n\
            DISPLAY WS-N \" \" WS-NOW \" \" WS-TIME \" \" WS-R \" \" WS-SUM\n        END-IF\n    END-PERFORM\n    STOP RUN.\n";

#[test]
fn the_same_events_give_the_same_batches() {
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(
        SRC,
        cobolt_lexer::SourceFormat::Free,
    ));
    let prog = parsed.program.expect("a program");
    let mut total = 0usize;
    for multi in [true, false] {
        let spec = FormSpec {
            name: "DET-FORM".into(),
            main: true,
            instances: 1,
            program: &prog,
            controls: vec![
                ("BTN-GO".into(), "Button".into()),
                ("LBL-NOW".into(), "Label".into()),
                ("LBL-SUM".into(), "Label".into()),
            ],
            bindings: vec![("BTN-GO".into(), "onClick".into())],
            layers: vec![],
            lines: vec![],
        };
        let c = compile_project(
            std::slice::from_ref(&spec),
            &Options {
                multi_memory: multi,
                when_compiled: Some("2026102312000000+0000".into()),
                ..Options::default()
            },
        )
        .unwrap();
        let click = scan_module(&c.wasm)
            .unwrap()
            .embedded_manifest()
            .unwrap()
            .event_index("onClick")
            .unwrap() as i32;
        let run = || {
            let opts = HostOptions {
                now_ms: Some(1_793_000_000_000.0),
                random: Some(0.25),
                record_batches: true,
                ..HostOptions::default()
            };
            let mut h = FormHost::with(&c.wasm, opts).unwrap();
            let (inst, _) = h.open_form(0, -1);
            for _ in 0..5 {
                assert_eq!(h.event(inst, 3, click, &[]), 0, "{:?}", h.outcome());
            }
            assert!(
                h.outcome().trapped.is_none() && h.outcome().errors.is_empty(),
                "{:?}",
                h.outcome()
            );
            h.batches()
        };
        let (a, b) = (run(), run());
        assert!(a.len() >= 5, "a batch per event: {}", a.len());
        assert_eq!(a, b, "multi={multi}: two runs of one script differ");
        let bytes: usize = a.iter().map(|x| x.len()).sum();
        total += bytes;
        eprintln!(
            "determinism (multi={multi}): 2 runs x 5 events, {} batches, {bytes} bytes, identical",
            a.len()
        );
    }
    eprintln!("determinism: {total} batch bytes compared across 2 variants, all identical");
}
