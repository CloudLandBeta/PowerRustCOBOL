// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A TextBox's `Picture` reaches the generated COBOL.
//!
//! The point of the property is that the comparison is right *by
//! construction*: the generated item carries the same PICTURE the box
//! validates against, so nothing has to be coerced at run time. Both the
//! `-TEXT` and the `-VALUE` item therefore declare it.

use cobolt_codegen::generate;
use cobolt_forms::{Control, ControlType, Form};

/// The `05 WS-<ID>-<suffix>` line from the generated program.
fn decl<'a>(out: &'a str, name: &str) -> &'a str {
    out.lines()
        .find(|l| l.contains(name))
        .unwrap_or_else(|| panic!("no declaration for {name} in:\n{out}"))
        .trim()
}

fn form_with_textbox(configure: impl FnOnce(&mut Control)) -> String {
    let mut form = Form::new("MAIN-FORM", "Demo", 640, 480);
    let mut tb = Control::new("Amount", ControlType::TextBox, 10, 10);
    configure(&mut tb);
    form.controls = vec![tb];
    generate(&form)
}

#[test]
fn an_explicit_picture_declares_both_the_text_and_the_value_item() {
    let out = form_with_textbox(|tb| tb.set_prop("Picture", "S9(4)V99"));
    assert_eq!(
        decl(&out, "WS-Amount-TEXT"),
        "05 WS-Amount-TEXT       PIC S9(4)V99 VALUE 0."
    );
    assert_eq!(
        decl(&out, "WS-Amount-VALUE"),
        "05 WS-Amount-VALUE      PIC S9(4)V99 VALUE 0."
    );
}

#[test]
fn a_plain_numeric_item_is_seeded_with_a_numeric_literal() {
    // `VALUE SPACES` on a plain numeric item is not COBOL; the caption is not
    // a legal seed for one either, so it is dropped rather than emitted.
    let out = form_with_textbox(|tb| {
        tb.set_prop("Picture", "9(5)");
        tb.set_prop("Text", "ignored");
    });
    assert!(decl(&out, "WS-Amount-TEXT").ends_with("PIC 9(5) VALUE 0."));
}

#[test]
fn a_numeric_edited_item_takes_an_alphanumeric_seed() {
    // A numeric-**edited** item's VALUE is an alphanumeric literal, so it can
    // carry the caption — truncated to what the picture actually holds.
    let out = form_with_textbox(|tb| {
        tb.set_prop("Picture", "ZZ9.99");
        tb.set_prop("Text", "abcdefghij");
    });
    assert_eq!(
        decl(&out, "WS-Amount-TEXT"),
        "05 WS-Amount-TEXT       PIC ZZ9.99 VALUE 'abcdef'."
    );
}

#[test]
fn without_a_picture_the_width_comes_from_maximum_length() {
    let out = form_with_textbox(|tb| tb.set_prop("MaximumLength", 40));
    assert!(decl(&out, "WS-Amount-TEXT").contains("PIC X(40)"));
    assert!(decl(&out, "WS-Amount-VALUE").contains("PIC X(40)"));
}

#[test]
fn an_explicit_picture_overrides_maximum_length() {
    // Once a picture is set its own width is authoritative — `MaximumLength`
    // no longer bounds the field.
    let out = form_with_textbox(|tb| {
        tb.set_prop("MaximumLength", 40);
        tb.set_prop("Picture", "X(12)");
    });
    assert!(decl(&out, "WS-Amount-TEXT").contains("PIC X(12)"));
    assert!(!out.contains("PIC X(40)"));
}

#[test]
fn a_multiline_box_with_no_bounds_gets_the_multiline_default() {
    let out = form_with_textbox(|tb| tb.set_prop("Multiline", true));
    assert!(decl(&out, "WS-Amount-TEXT").contains("PIC X(2048)"));
}

#[test]
fn a_control_that_holds_no_cobol_value_is_untouched() {
    // Only a TextBox carries a picture; every other control keeps the
    // caption-sized `PIC X(n)` it has always had.
    let mut form = Form::new("MAIN-FORM", "Demo", 640, 480);
    form.controls = vec![Control::new("Ok", ControlType::Button, 10, 10)];
    let out = generate(&form);
    assert!(decl(&out, "WS-Ok-TEXT").contains("PIC X(256) VALUE 'Ok'"));
}

#[test]
fn an_empty_caption_is_seeded_with_spaces_not_an_empty_literal() {
    // `VALUE ''` is not a COBOL-85 literal — one needs at least one character.
    let out = form_with_textbox(|tb| tb.set_prop("Text", ""));
    assert!(decl(&out, "WS-Amount-TEXT").ends_with("VALUE SPACES."));
    assert!(!out.contains("VALUE ''"));
}
