// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! P-2 (second part): a boolean property compared with a FIGURATIVE constant, as
//! the desktop interpreter compares it. `compare_values` reads the constant the
//! way `literal_to_value` does — `ZERO`/`ZEROS`/`ZEROES` are the number 0, so
//! `IF SW-1::Checked = ZERO` holds for a switch the user turned off — and
//! `ALL literal` is sized to the other operand. Every expected line is the
//! desktop's answer for the same condition (`rcrun run-form --headless`, probe
//! `t2`, 2026-10-28: SW-1 after the click reads `false`, CHK-1 `1`, RB-1 `0`).

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
    let c = compile_project(&[spec], &Options { multi_memory: false, name: "p3".into(), ..Options::default() })
        .unwrap_or_else(|e| panic!("compile_project failed: {e}"));
    let m = scan_module(&c.wasm).unwrap().embedded_manifest().unwrap();
    (c.wasm, m)
}

/// The comparison family against what each toggle reads after the user's click.
#[test]
fn a_boolean_property_compares_with_figurative_constants_as_on_the_desktop() {
    let mut body = String::new();
    let mut want = Vec::new();
    // (control, the text the click left, condition, the desktop's answer)
    let cases: &[(&str, &str, &str, &str)] = &[
        ("SW-1", "false", "= ZERO", "Y"),
        ("SW-1", "false", "= ZEROS", "Y"),
        ("SW-1", "false", "= ZEROES", "Y"),
        ("SW-1", "false", "NOT = ZERO", "N"),
        ("SW-1", "false", "= 0", "Y"),
        ("SW-1", "false", "= \"0\"", "Y"),
        ("SW-1", "false", "= SPACE", "N"),
        ("SW-1", "false", "NOT = SPACES", "Y"),
        ("SW-1", "false", "= ALL \"0\"", "N"),
        ("SW-1", "false", "= ALL \"1\"", "N"),
        ("SW-1", "false", "= HIGH-VALUE", "N"),
        ("SW-1", "false", "= LOW-VALUE", "N"),
        ("SW-1", "false", "= QUOTE", "N"),
        ("SW-1", "true", "= ZERO", "N"),
        ("SW-1", "true", "NOT = ZERO", "Y"),
        ("CHK-1", "1", "= ZERO", "N"),
        ("CHK-1", "1", "= ALL \"1\"", "Y"),
        ("CHK-1", "1", "= \"true\"", "Y"),
        ("CHK-1", "1", "= 1", "Y"),
        ("RB-1", "0", "= ZERO", "Y"),
        ("RB-1", "0", "= ALL \"0\"", "Y"),
        ("RB-1", "0", "= \"FALSE\"", "Y"),
    ];
    for (i, (ctl, text, cond, answer)) in cases.iter().enumerate() {
        let prop = if *ctl == "RB-1" { "Selected" } else { "Checked" };
        body.push_str(&format!("    SET {ctl}::{prop} TO \"{text}\"\n"));
        body.push_str(&format!("    IF {ctl}::{prop} {cond} DISPLAY \"C{i} Y\" ELSE DISPLAY \"C{i} N\" END-IF\n"));
        want.push(format!("C{i} {answer}"));
    }
    // EVALUATE takes the same rule (desktop: `EV SW-1::Checked ZERO`, `EV2 SW-1::Checked ZEROS`).
    body.push_str("    SET SW-1::Checked TO \"false\"\n");
    body.push_str("    EVALUATE SW-1::Checked WHEN ZERO DISPLAY \"EV ZERO\" WHEN TRUE DISPLAY \"EV TRUE\" WHEN OTHER DISPLAY \"EV OTHER\" END-EVALUATE\n");
    body.push_str("    EVALUATE SW-1::Checked WHEN \"true\" DISPLAY \"EV2 q-true\" WHEN 1 DISPLAY \"EV2 one\" WHEN ZEROS DISPLAY \"EV2 ZEROS\" WHEN OTHER DISPLAY \"EV2 OTHER\" END-EVALUATE\n");
    want.push("EV ZERO".into());
    want.push("EV2 ZEROS".into());
    let (wasm, m) = one_form(
        "",
        &[("BTN-1", "Button"), ("SW-1", "Switch"), ("CHK-1", "CheckBox"), ("RB-1", "RadioButton")],
        &[("BTN-1", "onClick", &body)],
    );
    let click = m.event_index("onClick").unwrap() as i32;
    let mut host = FormHost::new(&wasm).unwrap();
    let (inst, _) = host.open_form(0, -1);
    assert_eq!(host.event(inst, 3, click, &[]), 0, "{:?}", host.outcome());
    let lines = common::wasm_lines(&host.outcome());
    let wrong: Vec<String> = lines.iter().zip(&want).filter(|(a, b)| a != b).map(|(a, b)| format!("{a} (desktop: {b})")).collect();
    assert_eq!(lines.len(), want.len(), "{lines:?}");
    assert!(wrong.is_empty(), "conditions that differ from the desktop: {wrong:?}");
    eprintln!("P-2: {} conditions and 2 EVALUATEs answer as the desktop", cases.len());
}
