// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 109 S5 (R29, R35, AC15): a form that opts into compact storage loses
//! the `WS-<ID>` group of a control no other line names, and nothing else.

use cobolt_codegen::generate;
use cobolt_forms::{Control, ControlType, Form};

fn form(compact: bool, user_ws: &str) -> Form {
    let mut f = Form::new("MAIN-FORM", "Demo", 640, 480);
    f.controls = vec![
        Control::new("BTN-OK", ControlType::Button, 10, 10),
        Control::new("TXT-A", ControlType::TextBox, 10, 50),
        Control::new("LBL-1", ControlType::Label, 10, 90),
    ];
    f.user_ws_source = user_ws.to_string();
    f.compact_storage = compact;
    f
}

#[test]
fn off_by_default_every_control_keeps_its_group() {
    let out = generate(&form(false, ""));
    for id in ["BTN-OK", "TXT-A", "LBL-1"] {
        assert!(out.contains(&format!("01 WS-{id}.")), "{id} missing:\n{out}");
    }
}

#[test]
fn compact_leaves_out_the_groups_nobody_names() {
    let out = generate(&form(true, ""));
    for id in ["BTN-OK", "TXT-A", "LBL-1"] {
        assert!(!out.contains(&format!("WS-{id}")), "{id} kept:\n{out}");
    }
}

#[test]
fn compact_keeps_a_group_that_authored_storage_names() {
    let ws = "       01 WS-COPY REDEFINES WS-TXT-A-TEXT PIC X(5).\n";
    let out = generate(&form(true, ws));
    assert!(out.contains("01 WS-TXT-A."), "TXT-A dropped:\n{out}");
    assert!(!out.contains("01 WS-BTN-OK."), "BTN-OK kept:\n{out}");
    assert!(!out.contains("01 WS-LBL-1."), "LBL-1 kept:\n{out}");
}

#[test]
fn a_word_that_only_ends_in_the_name_does_not_keep_it() {
    // `XWS-TXT-A` is another word; `WS-TXT-A-COUNT` begins with the name and
    // keeps the group, because a false retention is the safe error.
    let other = generate(&form(true, "       01 XWS-TXT-A PIC X.\n"));
    assert!(!other.contains("01 WS-TXT-A."));
    let prefixed = generate(&form(true, "       01 WS-TXT-A-COUNT PIC 9.\n"));
    assert!(prefixed.contains("01 WS-TXT-A."));
}

#[test]
fn compact_output_is_the_full_output_minus_whole_groups() {
    let full = generate(&form(false, ""));
    let compact = generate(&form(true, ""));
    let kept: Vec<&str> = compact.lines().collect();
    let dropped: Vec<&str> = full.lines().filter(|l| !kept.contains(l)).collect();
    assert!(!dropped.is_empty());
    assert!(
        dropped.iter().all(|l| l.contains("WS-") || l.trim().is_empty()),
        "something other than a group went: {dropped:?}"
    );
}
