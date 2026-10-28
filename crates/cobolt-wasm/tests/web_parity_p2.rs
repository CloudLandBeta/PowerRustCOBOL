// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Web-module parity with the desktop interpreter for what a form handler
//! does every day: a figurative constant written to a property, a boolean
//! property compared with 1, `INITIALIZE` of a table with `OCCURS DEPENDING
//! ON`, and a handler that keeps working after a run-time error. Each value
//! the module must give is the one the desktop host gives for the same COBOL
//! (`rcrun run-form --headless`, 2026-10-28).

mod common;

use cobolt_wasm::host::FormHost;
use cobolt_wasm::module::{compile_project, FormSpec};
use cobolt_wasm::Options;
use cobolt_wasm_abi::scan::scan_module;

/// A one-form project in the shape `cobolt-codegen` writes; `handlers` are
/// `(control, event, body)`. Control ids follow the manifest: 3, 4, …
fn one_form(data: &str, controls: &[(&str, &str)], handlers: &[(&str, &str, &str)]) -> (Vec<u8>, cobolt_wasm_abi::manifest::Manifest) {
    let mut s = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. MAIN-FORM.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 COBOL-QUIT PIC 9 VALUE 0.\n01 COBOL-EVENT-ID PIC X(64) VALUE SPACES.\n\
         01 COBOL-CONTROL-ID PIC X(64) VALUE SPACES.\n01 FORM-NAME PIC X(64) VALUE 'MAIN-FORM'.\n{data}\
         PROCEDURE DIVISION.\nCOBOL-MAIN.\n    COBOL::\"INIT-FORM\" ( FORM-NAME )\n    PERFORM COBOL-EVENT-LOOP\n    STOP RUN.\n\
         COBOL-EVENT-LOOP.\n    PERFORM UNTIL COBOL-QUIT = 1\n        COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )\n\
         \x20       EVALUATE COBOL-CONTROL-ID\n"
    );
    for (c, e, _) in handlers {
        s.push_str(&format!(
            "            WHEN \"{c}\"\n                EVALUATE COBOL-EVENT-ID\n                    WHEN \"{e}\"\n                        CALL \"{c}--{}\"\n                END-EVALUATE\n",
            e.to_ascii_uppercase()
        ));
    }
    s.push_str("        END-EVALUATE\n    END-PERFORM.\n");
    for (c, e, body) in handlers {
        let p = format!("{c}--{}", e.to_ascii_uppercase());
        s.push_str(&format!("IDENTIFICATION DIVISION.\nPROGRAM-ID. {p} IS COMMON PROGRAM.\nPROCEDURE DIVISION.\n{body}    GOBACK.\nEND PROGRAM {p}.\n"));
    }
    s.push_str("END PROGRAM MAIN-FORM.\n");
    let program = common::parse(&s);
    let spec = FormSpec {
        name: "MAIN-FORM".into(),
        main: true,
        instances: 1,
        program: &program,
        controls: controls.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect(),
        bindings: handlers.iter().map(|(c, e, _)| (c.to_string(), e.to_string())).collect(),
        layers: vec![],
        lines: vec![],
    };
    let c = compile_project(&[spec], &Options { multi_memory: false, name: "p2".into(), ..Options::default() })
        .unwrap_or_else(|e| panic!("compile_project failed: {e}"));
    let m = scan_module(&c.wasm).unwrap().embedded_manifest().unwrap();
    (c.wasm, m)
}

/// P-1. `SET LBL::Caption TO SPACES` writes the blank text the desktop writes
/// (it wrote the FIG code byte 0x01 before), and every other figurative
/// constant writes the character the desktop writes.
#[test]
fn a_figurative_constant_written_to_a_property_is_its_character() {
    let body = "    SET LBL-A::Caption TO SPACES\n    SET LBL-A::Caption TO ZEROS\n    SET LBL-A::Caption TO QUOTES\n\
                \x20   SET LBL-A::Caption TO ALL \"x\"\n    MOVE SPACES TO LBL-A::Caption\n    MOVE ZERO TO LBL-A::Caption\n\
                \x20   SET LBL-A::Caption TO SPACES\n    IF LBL-A::Caption = SPACES DISPLAY \"CMP=EQ\" ELSE DISPLAY \"CMP=NE\" END-IF\n\
                \x20   SET LBL-A::Caption TO LOW-VALUES\n    SET LBL-A::Caption TO HIGH-VALUES\n";
    let (wasm, m) = one_form("", &[("BTN-1", "Button"), ("LBL-A", "Label")], &[("BTN-1", "onClick", body)]);
    let click = m.event_index("onClick").unwrap() as i32;
    let mut host = FormHost::new(&wasm).unwrap();
    let (inst, _) = host.open_form(0, -1);
    assert_eq!(host.event(inst, 3, click, &[]), 0, "{:?}", host.outcome());
    let o = host.outcome();
    let values: Vec<String> = o.sets.iter().map(|s| s.3.clone()).collect();
    // The desktop (`rcrun run-form --headless`): "", "0", "\"", "x", "", "0", "", then one character each.
    assert_eq!(values[..7], ["", "0", "\"", "x", "", "0", ""].map(String::from), "{values:?}");
    assert_eq!(values[7].as_bytes(), [0x00], "LOW-VALUES");
    assert_eq!(values[8].chars().count(), 1, "HIGH-VALUES is one character: {:?}", values[8]);
    assert!(values.iter().all(|v| !v.contains('\u{1}')), "no FIG code byte reaches the page: {values:?}");
    assert_eq!(common::wasm_lines(&o), vec!["CMP=EQ"]);
    eprintln!("P-1: 9 figurative writes = {values:?}; the read-back compares equal to SPACES");
}

/// P-2. A boolean property reads `true`/`false`, and `IF SW::Checked = 1`
/// holds when it reads `true` — the interpreter's `compare_values` rule. Plain
/// numbers and plain texts keep comparing as before.
#[test]
fn a_boolean_word_compares_with_one_and_zero_as_on_the_desktop() {
    let body = "    SET SW-1::Checked TO \"true\"\n\
                \x20   IF SW-1::Checked = 1 DISPLAY \"T=1 Y\" ELSE DISPLAY \"T=1 N\" END-IF\n\
                \x20   IF SW-1::Checked = 0 DISPLAY \"T=0 Y\" ELSE DISPLAY \"T=0 N\" END-IF\n\
                \x20   IF SW-1::Checked NOT = 1 DISPLAY \"T<>1 Y\" ELSE DISPLAY \"T<>1 N\" END-IF\n\
                \x20   SET SW-1::Checked TO \"false\"\n\
                \x20   IF SW-1::Checked = 0 DISPLAY \"F=0 Y\" ELSE DISPLAY \"F=0 N\" END-IF\n\
                \x20   IF SW-1::Checked = 1 DISPLAY \"F=1 Y\" ELSE DISPLAY \"F=1 N\" END-IF\n\
                \x20   IF SW-1::Checked = \"FALSE\" DISPLAY \"F=WORD Y\" ELSE DISPLAY \"F=WORD N\" END-IF\n\
                \x20   MOVE \"TRUE\" TO WS-W\n\
                \x20   IF WS-W = 1 DISPLAY \"W=1 Y\" ELSE DISPLAY \"W=1 N\" END-IF\n\
                \x20   IF WS-W = \"1\" DISPLAY \"W=Q1 Y\" ELSE DISPLAY \"W=Q1 N\" END-IF\n\
                \x20   IF WS-N = 1 DISPLAY \"N=1 Y\" ELSE DISPLAY \"N=1 N\" END-IF\n\
                \x20   MOVE \"7\" TO WS-W\n\
                \x20   IF WS-W = 7 DISPLAY \"7=7 Y\" ELSE DISPLAY \"7=7 N\" END-IF\n\
                \x20   IF WS-W = 1 DISPLAY \"7=1 Y\" ELSE DISPLAY \"7=1 N\" END-IF\n";
    let (wasm, m) = one_form(
        "01 WS-W PIC X(5) VALUE SPACES GLOBAL.\n01 WS-N PIC 9 VALUE 1 GLOBAL.\n",
        &[("BTN-1", "Button"), ("SW-1", "Switch")],
        &[("BTN-1", "onClick", body)],
    );
    let click = m.event_index("onClick").unwrap() as i32;
    let mut host = FormHost::new(&wasm).unwrap();
    let (inst, _) = host.open_form(0, -1);
    assert_eq!(host.event(inst, 3, click, &[]), 0, "{:?}", host.outcome());
    let lines = common::wasm_lines(&host.outcome());
    // The desktop's answers for the same conditions (compare_values).
    assert_eq!(
        lines,
        vec!["T=1 Y", "T=0 N", "T<>1 N", "F=0 Y", "F=1 N", "F=WORD Y", "W=1 Y", "W=Q1 Y", "N=1 Y", "7=7 Y", "7=1 N"]
    );
    eprintln!("P-2: 11 conditions as on the desktop: {lines:?}");
}

/// P-3. `INITIALIZE` of a group holding an `OCCURS … DEPENDING ON` table
/// initialises it — every occurrence up to the maximum, the count included —
/// instead of stopping the handler with run-time error 1100 while the count
/// is 0. The desktop gives `N=00`, `T1=[   ]`, `T5=[   ]`.
#[test]
fn initialize_of_a_group_with_occurs_depending_on() {
    let body = "    MOVE 0 TO WS-N\n    INITIALIZE WS-G\n    DISPLAY \"INIT0 N=\" WS-N\n\
                \x20   MOVE 5 TO WS-N\n    MOVE \"zzz\" TO WS-T(5)\n    MOVE \"yyy\" TO WS-T(1)\n    MOVE 2 TO WS-N\n\
                \x20   INITIALIZE WS-G\n    DISPLAY \"AFTER N=\" WS-N\n    MOVE 5 TO WS-N\n\
                \x20   DISPLAY \"T1=[\" WS-T(1) \"] T5=[\" WS-T(5) \"]\"\n";
    let (wasm, m) = one_form(
        "01 WS-G GLOBAL.\n   05 WS-N PIC 99.\n   05 WS-T PIC X(3) OCCURS 0 TO 5 DEPENDING ON WS-N.\n",
        &[("BTN-1", "Button")],
        &[("BTN-1", "onClick", body)],
    );
    let click = m.event_index("onClick").unwrap() as i32;
    let mut host = FormHost::new(&wasm).unwrap();
    let (inst, _) = host.open_form(0, -1);
    let st = host.event(inst, 3, click, &[]);
    let o = host.outcome();
    assert_eq!(st, 0, "the handler ran to its end: {o:?}");
    assert!(o.errors.is_empty(), "{o:?}");
    assert_eq!(common::wasm_lines(&o), vec!["INIT0 N=00", "AFTER N=00", "T1=[   ] T5=[   ]"]);
    eprintln!("P-3: INITIALIZE with OCCURS DEPENDING ON at count 0 and 2: no error 1100, 5 of 5 occurrences reset");
}

/// FX-4b. After a run-time error in a Timer's handler the page cancels the run
/// that trapped (`pr_cancel(-1)`, ABI A5) and keeps delivering ticks: each
/// later tick runs the handler again — the instance must not be lost (a lost
/// one answered every tick with -4 "unknown form, control or event").
#[test]
fn the_next_tick_after_a_run_time_error_runs_the_handler() {
    let body = "    ADD 1 TO WS-TICKS\n    DISPLAY \"TICK \" WS-TICKS\n\
                \x20   IF WS-TICKS = 2\n        COMPUTE WS-R = FUNCTION MOD(WS-TICKS, WS-Z)\n    END-IF\n";
    let (wasm, m) = one_form(
        "01 WS-TICKS PIC 9(4) VALUE 0 GLOBAL.\n01 WS-Z PIC 9(4) VALUE 0 GLOBAL.\n01 WS-R PIC 9(4) VALUE 0 GLOBAL.\n",
        &[("TMR-1", "Timer")],
        &[("TMR-1", "onTick", body)],
    );
    let tick = m.event_index("onTick").unwrap() as i32;
    let mut host = FormHost::new(&wasm).unwrap();
    let (inst, _) = host.open_form(0, -1);
    let mut statuses = vec![];
    for _ in 0..4 {
        // A trap is followed by `pr_cancel(-1)`, as the page does (the test host does it itself).
        statuses.push(host.event(inst, 3, tick, &[]));
    }
    let o = host.outcome();
    assert_eq!(statuses, vec![0, -10, 0, 0], "{o:?}");
    assert_eq!(o.error_positions.len(), 1, "one error, reported once: {o:?}");
    assert_eq!(common::wasm_lines(&o), vec!["TICK 0001", "TICK 0002", "TICK 0003", "TICK 0004"]);
    eprintln!("FX-4b: ticks {statuses:?}; one ERROR; the handler ran on the two ticks after it");
}

/// A text the page receives is UTF-8, as the ABI requires and the desktop
/// gives (`from_utf8_lossy`): an item that cuts a character in half (`ação`
/// into `PIC X(2)`, a Backend name cut at 40 bytes) reaches a property and the
/// console with U+FFFD in place of the stray byte. Before, the stray byte made
/// the page refuse the whole batch. The desktop: `CUT=[a\u{FFFD}]`.
#[test]
fn a_character_cut_in_half_reaches_the_page_as_the_replacement_character() {
    let body = "    MOVE \"a\u{e7}\u{e3}o\" TO WS-CUT\n    SET LBL-A::Caption TO WS-CUT\n    DISPLAY \"CUT=[\" WS-CUT \"]\"\n\
                \x20   SET LBL-A::Caption TO \"ok\"\n";
    let (wasm, m) = one_form("01 WS-CUT PIC X(2) GLOBAL.\n", &[("BTN-1", "Button"), ("LBL-A", "Label")], &[("BTN-1", "onClick", body)]);
    let click = m.event_index("onClick").unwrap() as i32;
    let mut host = FormHost::new(&wasm).unwrap();
    let (inst, _) = host.open_form(0, -1);
    assert_eq!(host.event(inst, 3, click, &[]), 0, "{:?}", host.outcome());
    let o = host.outcome();
    let values: Vec<String> = o.sets.iter().map(|s| s.3.clone()).collect();
    assert_eq!(values, vec!["a\u{FFFD}".to_string(), "ok".to_string()], "every command of the batch arrived");
    assert_eq!(common::wasm_lines(&o), vec!["CUT=[a\u{FFFD}]"]);
    eprintln!("UTF-8: a cut character crosses as U+FFFD in a property and in DISPLAY; the rest of the batch arrives");
}

/// A form program as `cobolt-codegen` writes one with an `onLoad` handler: the start-up calls it from
/// the first paragraph, then waits for events.
fn startup_form(data: &str, onload: &str, handlers: &[(&str, &str, &str)], controls: &[(&str, &str)]) -> (Vec<u8>, cobolt_wasm_abi::manifest::Manifest) {
    let mut s = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. MAIN-FORM.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 COBOL-QUIT PIC 9 VALUE 0.\n01 COBOL-EVENT-ID PIC X(64) VALUE SPACES.\n\
         01 COBOL-CONTROL-ID PIC X(64) VALUE SPACES.\n01 FORM-NAME PIC X(64) VALUE 'MAIN-FORM'.\n{data}\
         PROCEDURE DIVISION.\nCOBOL-MAIN.\n    COBOL::\"INIT-FORM\" ( FORM-NAME )\n    CALL \"MAIN-FORM--ONLOAD\"\n    PERFORM COBOL-EVENT-LOOP\n    STOP RUN.\n\
         COBOL-EVENT-LOOP.\n    PERFORM UNTIL COBOL-QUIT = 1\n        COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )\n\
         \x20       EVALUATE COBOL-CONTROL-ID\n"
    );
    for (c, e, _) in handlers {
        s.push_str(&format!(
            "            WHEN \"{c}\"\n                EVALUATE COBOL-EVENT-ID\n                    WHEN \"{e}\"\n                        CALL \"{c}--{}\"\n                END-EVALUATE\n",
            e.to_ascii_uppercase()
        ));
    }
    s.push_str("        END-EVALUATE\n    END-PERFORM.\n");
    s.push_str(&format!("IDENTIFICATION DIVISION.\nPROGRAM-ID. MAIN-FORM--ONLOAD IS COMMON PROGRAM.\nPROCEDURE DIVISION.\n{onload}    GOBACK.\nEND PROGRAM MAIN-FORM--ONLOAD.\n"));
    for (c, e, body) in handlers {
        let p = format!("{c}--{}", e.to_ascii_uppercase());
        s.push_str(&format!("IDENTIFICATION DIVISION.\nPROGRAM-ID. {p} IS COMMON PROGRAM.\nPROCEDURE DIVISION.\n{body}    GOBACK.\nEND PROGRAM {p}.\n"));
    }
    s.push_str("END PROGRAM MAIN-FORM.\n");
    let program = common::parse(&s);
    let spec = FormSpec {
        name: "MAIN-FORM".into(),
        main: true,
        instances: 1,
        program: &program,
        controls: controls.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect(),
        bindings: handlers.iter().map(|(c, e, _)| (c.to_string(), e.to_string())).collect(),
        layers: vec![],
        lines: vec![],
    };
    let c = compile_project(&[spec], &Options { multi_memory: false, name: "p2s".into(), ..Options::default() })
        .unwrap_or_else(|e| panic!("compile_project failed: {e}"));
    let m = scan_module(&c.wasm).unwrap().embedded_manifest().unwrap();
    (c.wasm, m)
}

/// FX-4b (scout D9). `onLoad` waits for the Backend (a remote call suspends the start-up), then stops on a
/// run-time error. The desktop abandons the handler and the form goes on serving its events; the page's
/// `pr_cancel(-1)` now takes the start-up on to the event loop, `pr_tick()` names the instance and its
/// Timer's ticks run. Before, the cancel freed the instance while the page kept its form, and every tick
/// was answered -4 "unknown form, control or event" (the error storm).
#[test]
fn a_start_up_that_stops_after_a_remote_call_goes_on_to_the_event_loop() {
    let onload = "    MOVE \"loading\" TO LBL-A::Caption\n    CALL \"P\" WITH WEBSOCKET LINKAGE USING BY REFERENCE REC\n    DISPLAY \"NOT REACHED\"\n";
    let (wasm, m) = startup_form(
        "01 REC GLOBAL.\n    05 QTY PIC 9(2) VALUE 5.\n01 WS-TICKS PIC 9(4) VALUE 0 GLOBAL.\n",
        onload,
        &[("TMR-1", "onTick", "    ADD 1 TO WS-TICKS\n    DISPLAY \"TICK \" WS-TICKS\n")],
        &[("TMR-1", "Timer"), ("LBL-A", "Label")],
    );
    let tick = m.event_index("onTick").unwrap() as i32;
    let mut host = FormHost::new(&wasm).unwrap();
    let (inst, status) = host.open_form(0, -1);
    assert_eq!((inst, status), (0, 1), "the start-up waits for the Backend: {:?}", host.outcome());
    let task = host.tick();
    // The trap is followed by `pr_cancel(-1)`, as the page does (the test host does it itself).
    assert_eq!(host.resume(task, br#"{"id":0,"error":{"class":"transport","code":7,"message":"down"}}"#), -10, "run-time error 1110 in onLoad");
    assert_eq!((host.tick(), host.status()), (0, 0), "the instance went on to its event loop: {:?}", host.outcome());
    let statuses: Vec<i32> = (0..3).map(|_| host.event(inst, 3, tick, &[])).collect();
    let o = host.outcome();
    assert_eq!(statuses, vec![0, 0, 0], "no tick answered -4: {o:?}");
    assert_eq!(o.error_positions.len(), 1, "{o:?}");
    assert!(!o.output.contains("NOT REACHED"));
    assert_eq!(common::wasm_lines(&o), vec!["TICK 0001", "TICK 0002", "TICK 0003"]);
    assert_eq!(host.dbg_live(), 1);
    eprintln!("FX-4b start-up: error 1110 after a remote call in onLoad, cancel -> instance {} idle, 3 ticks ran", host.tick());
}

/// The same with no remote call: the start-up traps inside `pr_open_form`; after the cancel the
/// instance is alive and idle, and `pr_tick()` names it for the page to open its form.
#[test]
fn a_start_up_that_stops_at_once_goes_on_to_the_event_loop() {
    let onload = "    MOVE 9 TO WS-I\n    MOVE \"x\" TO WS-T(WS-I)\n    DISPLAY \"NOT REACHED\"\n";
    let (wasm, m) = startup_form(
        "01 WS-T PIC X OCCURS 3 GLOBAL.\n01 WS-I PIC 9 VALUE 0 GLOBAL.\n",
        onload,
        &[("BTN-1", "onClick", "    DISPLAY \"CLICKED\"\n")],
        &[("BTN-1", "Button")],
    );
    let click = m.event_index("onClick").unwrap() as i32;
    let mut host = FormHost::new(&wasm).unwrap();
    let (inst, _) = host.open_form(0, -1);
    // The trap is followed by `pr_cancel(-1)`, as the page does (the test host does it itself).
    assert_eq!(inst, -10, "the start-up trapped in pr_open_form");
    assert_eq!((host.tick(), host.status()), (0, 0), "{:?}", host.outcome());
    assert_eq!(host.event(0, 3, click, &[]), 0, "{:?}", host.outcome());
    let o = host.outcome();
    assert_eq!(o.error_positions.len(), 1, "{o:?}");
    assert_eq!(o.error_positions[0].0, 1100, "{o:?}");
    assert_eq!(common::wasm_lines(&o), vec!["CLICKED"]);
    eprintln!("FX-4b start-up: error 1100 in pr_open_form, cancel -> instance 0 idle, a click ran");
}
