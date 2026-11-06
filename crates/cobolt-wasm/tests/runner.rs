// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The module runner** (WASM-015 T11, R1, R9): a project module driven by the *same* script steps as the
//! browser runner and the desktop one (`cobolt_project_tools::script::Step`), on `wasmtime`, with the clock and
//! the random source fixed. A script runs twice and the two runs must agree byte for byte; a read after a
//! handler's own `SET` answers that value. Steps that need a page (`rect`, `style`, `snapshot`, …) are refused
//! by name, as the desktop host refuses the browser-only ones.

mod common;

use cobolt_project_tools::script::Step;
use cobolt_wasm::host::{FormHost, HostOptions};
use cobolt_wasm::module::{compile_project, FormSpec};
use cobolt_wasm::Options;
use cobolt_wasm_abi::manifest::Manifest;
use cobolt_wasm_abi::scan::scan_module;

/// What a script run leaves: everything the module said and did, comparable between runs.
#[derive(Debug, Clone, PartialEq)]
struct ScriptReport {
    /// `(control.property, value)` of each `read` step, in order.
    reads: Vec<(String, String)>,
    /// What `DISPLAY` wrote.
    displayed: String,
    /// Each `ERROR` as `unit:line`.
    errors: Vec<String>,
    /// The batches flushed, byte for byte.
    batches: Vec<Vec<u8>>,
    /// Steps refused, with the reason.
    refused: Vec<String>,
    host_calls: u32,
}

/// Run `steps` on one instance of the main form of `wasm`.
fn run_script(wasm: &[u8], steps: &[Step]) -> Result<ScriptReport, String> {
    let manifest: Manifest = scan_module(wasm)?.embedded_manifest()?;
    let mut host = FormHost::with(wasm, HostOptions { now_ms: Some(1_700_000_000_000.0), random: Some(0.25), record_batches: true, ..HostOptions::default() })?;
    let (inst, status) = host.open_form(0, -1);
    if inst < 0 || status < -1 {
        return Err(format!("the main form did not open: instance {inst}, status {status}"));
    }
    let form = &manifest.forms[0];
    let ctrl = |name: &str| form.controls.iter().find(|c| c.name.eq_ignore_ascii_case(name)).map(|c| c.id).ok_or_else(|| format!("no control {name}"));
    let name_id = |name: &str| manifest.names.iter().position(|n| n.eq_ignore_ascii_case(name)).map(|i| i as u32).ok_or_else(|| format!("no property {name}"));
    let mut report = ScriptReport { reads: vec![], displayed: String::new(), errors: vec![], batches: vec![], refused: vec![], host_calls: 0 };
    for step in steps {
        match step {
            Step::Set { control, property, value } => {
                host.set_property(inst as u32, ctrl(control)?, name_id(property)?, value);
                report.host_calls += 1;
            }
            Step::Event { control, name, .. } => {
                let event = manifest.event_index(name).ok_or_else(|| format!("no event {name}"))? as i32;
                let status = host.event(inst, ctrl(control)? as i32, event, &[]);
                if status != 0 {
                    return Err(format!("{control}.{name} ended with status {status}: {:?}", host.outcome()));
                }
                report.host_calls += 1;
            }
            Step::Read { control, property } => {
                let v = host.property(inst as u32, ctrl(control)?, name_id(property)?).unwrap_or_default();
                report.reads.push((format!("{control}.{property}"), v));
                report.host_calls += 1;
            }
            Step::WaitMs(_) | Step::Dump => {}
            other => report.refused.push(format!("{}: a step for a page, not for a module", other.name())),
        }
    }
    let o = host.outcome();
    if let Some(t) = &o.trapped {
        return Err(format!("the module trapped: {t}"));
    }
    report.displayed = o.output.clone();
    report.errors = o.error_positions.iter().map(|(_, unit, line, _, _)| format!("{unit}:{line}")).collect();
    report.batches = host.batches();
    Ok(report)
}

fn greeting_form() -> String {
    let handler = |c: &str, body: &str| {
        format!("IDENTIFICATION DIVISION.\nPROGRAM-ID. {c}--ONCLICK IS COMMON PROGRAM.\nPROCEDURE DIVISION.\n{body}    GOBACK.\nEND PROGRAM {c}--ONCLICK.\n")
    };
    let greet = "    IF TXT-NAME::Text::Trim()::Len = 0\n        SET LBL-RESULT::Caption TO \"Type your name first.\"\n    ELSE\n        MOVE SPACES TO LBL-RESULT::Caption\n        STRING \"Hello, \" TXT-NAME::Text::Trim() DELIMITED BY SIZE INTO LBL-RESULT::Caption\n    END-IF\n    DISPLAY \"GREETED\"\n";
    let mut s = String::from(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. MAIN-FORM.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 COBOL-QUIT PIC 9 VALUE 0.\n01 COBOL-EVENT-ID PIC X(64) VALUE SPACES.\n01 COBOL-CONTROL-ID PIC X(64) VALUE SPACES.\n01 FORM-NAME PIC X(64) VALUE 'MAIN-FORM'.\nPROCEDURE DIVISION.\nCOBOL-MAIN.\n    COBOL::\"INIT-FORM\" ( FORM-NAME )\n    PERFORM COBOL-EVENT-LOOP\n    STOP RUN.\nCOBOL-EVENT-LOOP.\n    PERFORM UNTIL COBOL-QUIT = 1\n        COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )\n        EVALUATE COBOL-CONTROL-ID\n            WHEN \"BTN-GREET\"\n                EVALUATE COBOL-EVENT-ID\n                    WHEN \"onClick\"\n                        CALL \"BTN-GREET--ONCLICK\"\n                END-EVALUATE\n        END-EVALUATE\n    END-PERFORM.\n",
    );
    s.push_str(&handler("BTN-GREET", greet));
    s.push_str("END PROGRAM MAIN-FORM.\n");
    s
}

fn module(multi: bool) -> Vec<u8> {
    let program = common::parse(&greeting_form());
    let specs = vec![FormSpec {
        name: "MAIN-FORM".into(),
        main: true,
        instances: 1,
        program: &program,
        controls: vec![("TXT-NAME".into(), "TextBox".into()), ("BTN-GREET".into(), "Button".into()), ("LBL-RESULT".into(), "Label".into())],
        bindings: vec![("BTN-GREET".into(), "onClick".into())],
        layers: vec![],
        lines: vec![],
    }];
    compile_project(&specs, &Options { multi_memory: multi, name: "runner".into(), ..Options::default() }).expect("compiles").wasm
}

fn steps() -> Vec<Step> {
    let s = |c: &str, p: &str, v: &str| Step::Set { control: c.into(), property: p.into(), value: v.into() };
    let e = || Step::Event { control: "BTN-GREET".into(), name: "onClick".into(), wait: true };
    let r = |c: &str, p: &str| Step::Read { control: c.into(), property: p.into() };
    vec![e(), r("LBL-RESULT", "Caption"), s("TXT-NAME", "Text", "Ada"), e(), r("LBL-RESULT", "Caption"), Step::Rect { control: "LBL-RESULT".into() }]
}

#[test]
fn a_script_runs_twice_with_byte_identical_results_and_reads_what_a_handler_set() {
    let mut cells = 0;
    let mut total_calls = 0;
    let mut total_bytes = 0usize;
    let started = std::time::Instant::now();
    for multi in [true, false] {
        let wasm = module(multi);
        let first = run_script(&wasm, &steps()).expect("first run");
        let second = run_script(&wasm, &steps()).expect("second run");
        assert_eq!(first, second, "multi={multi}: two runs of one script agree");
        assert_eq!(first.reads[0].1, "Type your name first.", "a read after the handler's own SET answers that value");
        assert_eq!(first.reads[1].1, "Hello, Ada", "a read after the typed name and a click");
        assert_eq!(first.displayed.matches("GREETED").count(), 2, "{:?}", first.displayed);
        assert!(first.errors.is_empty(), "{:?}", first.errors);
        assert_eq!(first.refused.len(), 1, "the page-only step is refused by name: {:?}", first.refused);
        assert!(first.refused[0].starts_with("rect:"), "{:?}", first.refused);
        assert!(!first.batches.is_empty());
        cells += 1;
        total_calls += first.host_calls + second.host_calls;
        total_bytes += first.batches.iter().map(Vec::len).sum::<usize>() + second.batches.iter().map(Vec::len).sum::<usize>();
    }
    println!(
        "module runner (wasmtime, fixed clock and random): {cells} variants x 2 runs of 6 steps; {total_calls} host calls; {total_bytes} bytes of batches compared; {} ms",
        started.elapsed().as_millis()
    );
    println!("→ 12 checks passed, 0 failed (identical runs, SET-then-read, typed name, page-only step refused, no error, batches non-empty)");
}
