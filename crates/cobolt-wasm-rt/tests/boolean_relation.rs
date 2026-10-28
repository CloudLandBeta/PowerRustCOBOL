// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A boolean property compared with `=` / `NOT =`, as the interpreter's
//! `compare_values` (`cobolt-runtime/src/interpreter.rs`) does it: a boolean
//! WORD on one side makes it a comparison of truths, and the other side is
//! read as the interpreter's `literal_to_value` reads it — the figuratives
//! `ZERO`/`ZEROS`/`NULL` are the number 0, `SPACE`/`QUOTE`/`HIGH-VALUE`/
//! `LOW-VALUE` are characters no boolean is spelled with, and `ALL literal`
//! is its literal repeated to the other operand's length.
//!
//! The property's text sits in an alphanumeric item of its own length, which
//! is how the program reads a property (`SW-1::Checked` is `false` after the
//! user turned the switch off).

use cobolt_wasm_rt::cmp::*;
use cobolt_wasm_rt::fd::*;
use cobolt_wasm_rt::opd::*;
use cobolt_wasm_rt::testkit::Kit;
use cobolt_wasm_rt::*;

/// `text op other`, both ways round (the rule is symmetric).
fn holds(k: &mut Kit, text: &[u8], other: u32, op: u32) -> bool {
    let item = k.item(0, Fd::alnum(text.len() as u32));
    k.poke(0, text);
    let ab = rt_rel(k.base, item, other, op) == 1;
    let ba = rt_rel(k.base, other, item, op) == 1;
    assert_eq!(ab, ba, "{} against the other operand is not symmetric", String::from_utf8_lossy(text));
    ab
}

#[test]
fn figurative_constants_take_part_in_the_boolean_rule_as_the_desktop_reads_them() {
    let mut k = Kit::new();
    // (property text, the other operand, operator, the desktop's answer)
    let zero = k.fig(FIG_ZERO);
    let null = k.fig(FIG_NULL);
    let space = k.fig(FIG_SPACE);
    let quote = k.fig(FIG_QUOTE);
    let high = k.fig(FIG_HIGH);
    let low = k.fig(FIG_LOW);
    let all_0 = k.all_lit(b"0");
    let all_1 = k.all_lit(b"1");
    let all_false = k.all_lit(b"FALSE");
    let all_true = k.all_lit(b"TRUE");
    let cases: Vec<(&[u8], u32, u32, bool, &str)> = vec![
        // The judge's case: a switch the user turned off.
        (b"false", zero, OP_EQ, true, "false = ZERO"),
        (b"false", zero, OP_NE, false, "false NOT = ZERO"),
        (b"true", zero, OP_EQ, false, "true = ZERO"),
        (b"true", zero, OP_NE, true, "true NOT = ZERO"),
        (b"FALSE", zero, OP_EQ, true, "FALSE = ZERO (any case)"),
        (b"false", null, OP_EQ, true, "false = NULL"),
        (b"true", null, OP_EQ, false, "true = NULL"),
        // Characters: not a boolean, so a plain comparison, which never matches a word.
        (b"false", space, OP_EQ, false, "false = SPACE"),
        (b"false", space, OP_NE, true, "false NOT = SPACE"),
        (b"true", quote, OP_EQ, false, "true = QUOTE"),
        (b"true", high, OP_EQ, false, "true = HIGH-VALUE"),
        (b"false", low, OP_EQ, false, "false = LOW-VALUE"),
        (b"false", low, OP_NE, true, "false NOT = LOW-VALUE"),
        // ALL literal, sized to the property: ALL "0" against `false` is "00000".
        (b"false", all_0, OP_EQ, false, "false = ALL \"0\""),
        (b"true", all_1, OP_EQ, false, "true = ALL \"1\""),
        (b"false", all_false, OP_EQ, true, "false = ALL \"FALSE\""),
        (b"true", all_true, OP_EQ, true, "true = ALL \"TRUE\""),
        (b"true", all_false, OP_EQ, false, "true = ALL \"FALSE\" (TRUET... is no word)"),
    ];
    let mut wrong = Vec::new();
    for (text, other, op, want, what) in cases {
        if holds(&mut k, text, other, op) != want {
            wrong.push(what);
        }
    }
    assert!(wrong.is_empty(), "conditions that differ from the desktop: {wrong:?}");
}

#[test]
fn numbers_and_quoted_spellings_against_a_boolean_word() {
    let mut k = Kit::new();
    let one = k.lit_num(1, 0);
    let zero = k.lit_num(0, 0);
    let seven = k.lit_num(7, 0);
    let q1 = k.lit_alnum(b"1");
    let q0 = k.lit_alnum(b"0");
    let qt = k.lit_alnum(b"true");
    let qf = k.lit_alnum(b"FALSE");
    let cases: Vec<(&[u8], u32, u32, bool, &str)> = vec![
        (b"true", one, OP_EQ, true, "true = 1"),
        (b"true", zero, OP_EQ, false, "true = 0"),
        (b"false", zero, OP_EQ, true, "false = 0"),
        (b"false", one, OP_NE, true, "false NOT = 1"),
        (b"true", seven, OP_EQ, false, "true = 7 (7 is no boolean)"),
        (b"true", seven, OP_NE, true, "true NOT = 7"),
        (b"true", q1, OP_EQ, true, "true = \"1\""),
        (b"false", q0, OP_EQ, true, "false = \"0\""),
        (b"false", qt, OP_EQ, false, "false = \"true\""),
        (b"false", qf, OP_EQ, true, "false = \"FALSE\""),
        // What a CheckBox reads after a click on the desktop: digits, no word,
        // so the ordinary rules (a number against characters that spell one).
        (b"1", one, OP_EQ, true, "\"1\" = 1"),
        (b"0", k.fig(FIG_ZERO), OP_EQ, true, "\"0\" = ZERO"),
        (b"1", k.fig(FIG_ZERO), OP_EQ, false, "\"1\" = ZERO"),
    ];
    let mut wrong = Vec::new();
    for (text, other, op, want, what) in cases {
        if holds(&mut k, text, other, op) != want {
            wrong.push(what);
        }
    }
    assert!(wrong.is_empty(), "conditions that differ from the desktop: {wrong:?}");
}
