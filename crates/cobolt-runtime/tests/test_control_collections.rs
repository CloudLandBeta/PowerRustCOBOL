// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 109 §5.4: `Descendants()`, `Controls(id …)` and `SetProperties("Name" value …)`
//! over a form's controls. Membership is the real parent, in document order; the
//! loop element IS the control; an invalid target or pair writes nothing.

use std::sync::mpsc;
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// (id, class, parent, visible) in document order. BTN-3 sits between BTN-1 and
/// PNL-B on the form and overlaps the panel, but belongs to the form, not PNL-A.
const TREE: &[(&str, &str, Option<&str>, &str)] = &[
    ("PNL-A", "Panel", None, "1"),
    ("BTN-1", "Button", Some("PNL-A"), "1"),
    ("BTN-3", "Button", None, "1"),
    ("PNL-B", "Panel", Some("PNL-A"), "1"),
    ("BTN-2", "Button", Some("PNL-B"), "0"),
];

fn run(procedure: &str) -> Result<Vec<String>, String> {
    let src = format!(
        "\
IDENTIFICATION DIVISION.
PROGRAM-ID. CC.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-X PIC X(10).
PROCEDURE DIVISION.
MAIN.
{procedure}
    STOP RUN.
"
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
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
    interp.seed_objects(TREE.iter().map(|(id, class, parent, visible)| {
        let mut props = vec![
            ("Name".to_string(), id.to_string()),
            ("Caption".to_string(), format!("cap-{id}")),
            ("Visible".to_string(), visible.to_string()),
            ("Enabled".to_string(), "1".to_string()),
        ];
        if let Some(p) = parent {
            props.push(("_Parent".to_string(), p.to_string()));
        }
        (id.to_string(), class.to_string(), props)
    }));
    match interp.run() {
        Ok(_) => Ok(display_rx.try_iter().collect()),
        Err(e) => Err(e.to_string()),
    }
}

fn shown(procedure: &str) -> String {
    run(procedure).expect("the program runs").join("|").trim().to_string()
}

#[test]
fn descendants_controls_and_setproperties_follow_the_form() {
    let t0 = Instant::now();
    let cases: Vec<(&str, String, &str)> = vec![
        (
            "Descendants: real parent, preorder, hidden kept, root and overlapping sibling out",
            shown("PERFORM VARYING CTL THROUGH PNL-A::Descendants()\n    DISPLAY CTL::Name\nEND-PERFORM"),
            "BTN-1|PNL-B|BTN-2",
        ),
        (
            "a leaf has none",
            shown("PERFORM VARYING CTL THROUGH BTN-1::Descendants()\n    DISPLAY CTL::Name\nEND-PERFORM\nDISPLAY \"end\""),
            "end",
        ),
        (
            "the element reads the control's property",
            shown("PERFORM VARYING CTL THROUGH PNL-B::Descendants()\n    DISPLAY CTL::Caption\nEND-PERFORM"),
            "cap-BTN-2",
        ),
        (
            "a method on the element reaches the control",
            shown("PERFORM VARYING CTL THROUGH PNL-A::Descendants()\n    CTL::SetProperty(\"Caption\" \"seen\")\nEND-PERFORM\nDISPLAY BTN-2::Caption BTN-3::Caption"),
            "seencap-BTN-3",
        ),
        (
            "Controls keeps the order of its arguments",
            shown("PERFORM VARYING CTL THROUGH ME::Controls(\"BTN-2\" \"BTN-1\")\n    DISPLAY CTL::Name\nEND-PERFORM"),
            "BTN-2|BTN-1",
        ),
        (
            "SetProperties writes every named property of every control",
            shown("ME::Controls(\"BTN-1\" \"BTN-3\")::SetProperties(\"Caption\" \"Hi\" \"Enabled\" \"0\")\nDISPLAY BTN-1::Caption BTN-3::Caption BTN-1::Enabled BTN-2::Caption"),
            "HiHifalsecap-BTN-2",
        ),
        (
            "SetProperties over Descendants",
            shown("PNL-A::Descendants()::SetProperties(\"Caption\" \"all\")\nDISPLAY BTN-1::Caption BTN-2::Caption PNL-A::Caption"),
            "allallcap-PNL-A",
        ),
    ];
    let mut failed = 0;
    for (name, got, want) in &cases {
        if got != want {
            failed += 1;
            eprintln!("FAIL {name}: got [{got}] want [{want}]");
        }
    }
    let refused: Vec<(&str, Result<Vec<String>, String>, &str)> = vec![
        ("a missing id", run("ME::Controls(\"BTN-1\" \"NOPE\")::SetProperties(\"Caption\" \"x\")"), "'NOPE' is not a control"),
        ("a duplicate id", run("ME::Controls(\"BTN-1\" \"btn-1\")::SetProperties(\"Caption\" \"x\")"), "named twice"),
        ("an odd pair list", run("ME::Controls(\"BTN-1\")::SetProperties(\"Caption\")"), "name/value pairs"),
        ("a collection is not a value", run("DISPLAY PNL-A::Descendants()"), "not a value"),
    ];
    for (name, got, want) in &refused {
        match got {
            Err(e) if e.contains(want) => {}
            other => {
                failed += 1;
                eprintln!("FAIL {name}: got {other:?}, wanted an error containing [{want}]");
            }
        }
    }
    // An invalid target writes nothing: BTN-1 keeps its caption.
    let untouched = run("ME::Controls(\"BTN-1\" \"NOPE\")::SetProperties(\"Caption\" \"x\")");
    assert!(untouched.is_err());
    let after = shown("DISPLAY BTN-1::Caption");
    if after != "cap-BTN-1" {
        failed += 1;
        eprintln!("FAIL a refused SetProperties wrote: [{after}]");
    }
    let total = cases.len() + refused.len() + 1;
    println!("================ test_control_collections ================");
    println!("cases: Descendants (parent, order, hidden, overlap), loop element = control, Controls order, SetProperties (many/over Descendants), 4 refusals, zero-write on a bad target");
    println!("run: {total} cases in {} ms", t0.elapsed().as_millis());
    println!("tally: {} passed, {failed} failed", total - failed);
    assert_eq!(failed, 0);
}
