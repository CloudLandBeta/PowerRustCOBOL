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
use cobolt_forms::{Control, ControlType, Form};

#[test]
fn nested_control_is_emitted_in_generated_cobol() {
    let mut form = Form::new("MAIN-FORM", "Demo", 640, 480);
    let panel = Control::new("Panel1", ControlType::Panel, 10, 10);
    let mut inner = Control::new("InnerBtn", ControlType::Button, 20, 20);
    inner.parent = Some("Panel1".into()); // nested inside the panel
    form.controls = vec![panel, inner];

    let out = generate(&form);
    // Both the container and its nested child get a `01 WS-<ID>.` group.
    assert!(out.contains("WS-Panel1"), "container missing from generated COBOL");
    assert!(
        out.contains("WS-InnerBtn"),
        "nested control was dropped from generated COBOL:\n{out}"
    );
    // The developer banner contract is intact.
    assert!(out.contains("IDENTIFICATION DIVISION"), "missing program skeleton");
}
