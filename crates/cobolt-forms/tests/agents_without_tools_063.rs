// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 063 R65 (AC29): two or more agents on a form that are set to offer no tools are a
//! design-time warning; one alone is a plain chat and says nothing.

use cobolt_forms::{Control, ControlType, Form, PropValue};

fn agent(form: &mut Form, id: &str, protocol: Option<&str>) {
    let mut c = Control::new(id, ControlType::AgentObject, 0, 0);
    if let Some(p) = protocol {
        c.set_prop("ToolProtocol", PropValue::String(p.into()));
    }
    form.controls.push(c);
}

#[test]
fn two_agents_set_to_offer_no_tools_are_named_and_one_alone_is_not() {
    let mut form = Form::new("F", "F", 640, 480);
    agent(&mut form, "AGT-1", Some("None"));
    assert!(form.agents_without_tools().is_empty(), "one agent without tools is a plain chat");
    agent(&mut form, "AGT-2", Some("none"));
    assert_eq!(form.agents_without_tools(), ["AGT-1", "AGT-2"], "the property is read without regard to case");
    agent(&mut form, "AGT-3", Some("Native"));
    agent(&mut form, "AGT-4", Some("Fenced"));
    agent(&mut form, "AGT-5", None);
    assert_eq!(form.agents_without_tools(), ["AGT-1", "AGT-2"], "Native, Fenced and an unset property offer tools");
    form.controls.push(Control::new("BTN", ControlType::Button, 0, 0));
    assert_eq!(form.agents_without_tools().len(), 2, "only AgentObjects count");
}
