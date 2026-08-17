// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A toolbar button's `procedure:` and `open-modal:` reach real COBOL.
//!
//! Both actions were offered by the Toolbar Editor and documented in the guide
//! while reaching nothing at all: the generated event loop dispatches what sits
//! in a control's `events` table, and a toolbar button is deliberately not a
//! `Control` — the toolbar owns its layout — so the per-control walk never saw
//! one. Codegen now reads each ToolBar's definition and emits a `WHEN` under the
//! button's derived `<toolbar>-<group>-<button>` id, which is the same id the
//! renderer fires the press under.
//!
//! This test lives here rather than in `cobolt-codegen` because the thing worth
//! pinning is not the text — it is that the text is **real COBOL**. The two
//! statements are built by hand, and `INVOKE ME::"OpenFormSync"("F")` inside an
//! `EVALUATE` branch either parses or it does not.

use cobolt_codegen::generate;
use cobolt_forms::toolbar::{
    button_control_id, ToolbarButton, ToolbarDef, ToolbarGroup, TOOLBAR_DEF_PROP,
};
use cobolt_forms::{Control, ControlType, Form, PropValue};
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};

/// A form carrying one ToolBar whose group holds `(button id, action)` buttons,
/// plus the user procedure a `procedure:` button names.
fn toolbar_form(buttons: &[(&str, &str)], procedures: &[&str]) -> Form {
    let mut form = Form::new("MAIN-FORM", "Toolbar", 800, 600);
    let mut group = ToolbarGroup::new("group-1", "File");
    for (id, action) in buttons {
        let mut b = ToolbarButton::new(*id, "");
        b.set_icon("folder-open");
        b.action = (*action).to_owned();
        group.buttons.push(b);
    }
    let def = ToolbarDef {
        groups: vec![group],
        button_gap: 4,
    };
    let mut bar = Control::new("TOOLBAR-1", ControlType::ToolBar, 0, 0);
    bar.set_prop(TOOLBAR_DEF_PROP, PropValue::String(def.to_json().unwrap()));
    form.add_control(bar);
    for name in procedures {
        form.user_procedures
            .push(cobolt_forms::model::UserProcedure {
                name: (*name).to_owned(),
                code: String::new(),
            });
    }
    form
}

fn parse_errors(src: &str) -> Vec<String> {
    parse(tokenize(src, SourceFormat::detect(src)))
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("line {}: {}", d.span.line, d.message))
        .collect()
}

#[test]
fn toolbar_procedure_and_open_modal_generate_source_that_parses() {
    let form = toolbar_form(
        &[
            ("button-1", "procedure:UPDATE-TOTAL"),
            ("button-2", "open-modal:CUST-LOOKUP"),
        ],
        &["UPDATE-TOTAL"],
    );
    let src = generate(&form);

    let errors = parse_errors(&src);
    assert!(
        errors.is_empty(),
        "the generated source must be real COBOL: {errors:#?}\n{src}"
    );

    // The `WHEN` waits for exactly the id the renderer fires the press under.
    let save = button_control_id("TOOLBAR-1", "group-1", "button-1");
    let find = button_control_id("TOOLBAR-1", "group-1", "button-2");
    assert_eq!(save, "TOOLBAR-1-GROUP-1-BUTTON-1");
    for (id, statement) in [
        (&save, "CALL \"UPDATE-TOTAL\""),
        (&find, "INVOKE ME::\"OpenFormSync\"(\"CUST-LOOKUP\")"),
    ] {
        let when = format!("WHEN \"{id}\"");
        let at = src
            .find(&when)
            .unwrap_or_else(|| panic!("no {when} in the event loop\n{src}"));
        let branch = &src[at..];
        let end = branch.find("END-EVALUATE").unwrap_or(branch.len());
        assert!(
            branch[..end].contains(statement),
            "{when} must run `{statement}`\n{}",
            &branch[..end]
        );
    }

    // A user procedure is a nested program, `IS COMMON`, which is what makes the
    // outer program's event loop able to CALL it at all.
    assert!(
        src.contains("PROGRAM-ID. UPDATE-TOTAL IS COMMON PROGRAM."),
        "the procedure the button names must be callable from the loop\n{src}"
    );

    println!(
        "\n  Toolbar dispatch, end to end — a ToolBar with `procedure:UPDATE-TOTAL` and \
         `open-modal:CUST-LOOKUP` generates {} lines of COBOL with 0 parse errors; each \
         button gets a WHEN under its derived id ({save} / {find}) running \
         CALL \"UPDATE-TOTAL\" and INVOKE ME::\"OpenFormSync\"(\"CUST-LOOKUP\"), and the \
         named procedure is emitted IS COMMON so the loop can reach it\n",
        src.lines().count()
    );
}

/// The actions the loop must NOT take over: `event` IS the toolbar's own
/// `onClick`, and the platform actions are carried out without COBOL's help. A
/// form holding only those must still generate source that parses — in
/// particular it must not emit an `EVALUATE` with no `WHEN` in it.
#[test]
fn the_other_actions_stay_out_of_the_event_loop() {
    let form = toolbar_form(
        &[
            ("button-1", "event"),
            ("button-2", "print:/tmp/report.pdf"),
            ("button-3", "copy"),
            // Named nothing: reported as a comment, never dispatched.
            ("button-4", "procedure:"),
        ],
        &[],
    );
    let src = generate(&form);

    let errors = parse_errors(&src);
    assert!(
        errors.is_empty(),
        "a toolbar with nothing to dispatch must still generate real COBOL: \
         {errors:#?}\n{src}"
    );
    for n in 1..=4 {
        let id = button_control_id("TOOLBAR-1", "group-1", &format!("button-{n}"));
        assert!(
            !src.contains(&format!("WHEN \"{id}\"")),
            "button-{n} must not be dispatched by the loop\n{src}"
        );
    }
    assert!(
        src.contains("asks to run a procedure but names none"),
        "…and the empty target must be reported, not dropped\n{src}"
    );
    // The loop has nothing to dispatch, so it says so rather than opening an
    // EVALUATE it cannot fill.
    assert!(
        src.contains("No event handlers defined yet"),
        "an EVALUATE with no WHEN is not COBOL\n{src}"
    );

    println!(
        "\n  Toolbar dispatch, exclusions — `event`, `print:`, `copy` and an empty \
         `procedure:` produce 0 WHEN branches and 0 parse errors; the empty target is \
         reported as a comment and the loop falls back to its no-handlers branch \
         instead of emitting an EVALUATE with nothing in it\n"
    );
}
