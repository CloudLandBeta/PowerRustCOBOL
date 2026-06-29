// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 012 regression: a control nested inside a container (it carries a
//! `parent` link and lives in the flat `form.controls` list) is still emitted in
//! the generated COBOL — `collect_all_controls` walks every control regardless of
//! containment, so nesting never drops a control from the program.

use cobolt_codegen::generate;
use cobolt_forms::{Control, ControlType, EventBinding, Form};

#[test]
fn nested_control_is_emitted_in_generated_cobol() {
    let mut form = Form::new("MAIN-FORM", "Demo", 640, 480);
    let panel = Control::new("Panel1", ControlType::Panel, 10, 10);
    let mut inner = Control::new("InnerBtn", ControlType::Button, 20, 20);
    inner.parent = Some("Panel1".into()); // nested inside the panel
    form.controls = vec![panel, inner];

    let out = generate(&form);
    // Both the container and its nested child get a `01 WS-<ID>.` group.
    assert!(
        out.contains("WS-Panel1"),
        "container missing from generated COBOL"
    );
    assert!(
        out.contains("WS-InnerBtn"),
        "nested control was dropped from generated COBOL:\n{out}"
    );
    // The developer banner contract is intact.
    assert!(
        out.contains("IDENTIFICATION DIVISION"),
        "missing program skeleton"
    );
}

#[test]
fn user_control_child_event_uses_full_qualified_id() {
    let mut form = Form::new("MAIN-FORM", "Demo", 640, 480);
    let mut root = Control::new("CustomerCard-1", ControlType::GroupBox, 10, 10);
    root.set_prop("UserControl".to_owned(), "CustomerCard");

    let mut child = Control::new("CustomerCard-1-Button1", ControlType::Button, 20, 20);
    child.parent = Some("CustomerCard-1".into());
    child.events.push(EventBinding::for_control(
        "CustomerCard-1-Button1",
        "onClick",
    ));
    form.controls = vec![root, child];

    let out = generate(&form);
    assert!(
        out.contains("WHEN \"CustomerCard-1-Button1\""),
        "event loop must dispatch by the deployed qualified child id:\n{out}"
    );
    assert!(
        out.contains("CALL \"CUSTOMERCARD-1-BUTTON1--ONCLICK\""),
        "event loop must call the qualified child handler:\n{out}"
    );
    assert!(
        out.contains("PROGRAM-ID. CUSTOMERCARD-1-BUTTON1--ONCLICK IS COMMON PROGRAM."),
        "nested handler program must keep the qualified child id:\n{out}"
    );
}
