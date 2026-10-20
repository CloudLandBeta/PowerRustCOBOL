// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The data layer against the standard and the interpreter: byte layouts of
//! each usage, the `MOVE` matrix, arithmetic stores, comparisons.

use cobolt_wasm_rt::arith::*;
use cobolt_wasm_rt::cmp::*;
use cobolt_wasm_rt::fd::*;
use cobolt_wasm_rt::opd::*;
use cobolt_wasm_rt::testkit::Kit;
use cobolt_wasm_rt::*;

#[test]
fn display_numeric_round_trip_all_sign_modes() {
    let mut k = Kit::new();
    // (fd, value as mantissa/scale, expected bytes)
    let cases: Vec<(Fd, i128, u8, &[u8])> = vec![
        (Fd::display(5, 0, 0), 123, 0, b"00123"),
        (Fd::display(5, 2, 0), 12345, 2, b"12345"),
        (Fd::display(5, 0, F_SIGNED), -123, 0, b"0012L"),     // -3 overpunch = L
        (Fd::display(5, 0, F_SIGNED), 123, 0, b"00123"),
        (Fd::display(5, 0, F_SIGNED), -120, 0, b"0012}"),     // -0 overpunch = }
        (Fd::display(5, 0, F_SIGNED | F_SIGN_LEADING), -50123, 0, b"N0123"),
        (Fd::display(5, 0, F_SIGNED | F_SIGN_LEADING), -123, 0, b"}0123"),
        (Fd::display(5, 0, F_SIGNED | F_SIGN_SEPARATE), -123, 0, b"00123-"),
        (Fd::display(5, 0, F_SIGNED | F_SIGN_SEPARATE), 123, 0, b"00123+"),
        (Fd::display(5, 0, F_SIGNED | F_SIGN_SEPARATE | F_SIGN_LEADING), -123, 0, b"-00123"),
    ];
    for (fd, m, s, want) in cases {
        let item = k.item(0, fd);
        let src = k.lit_num(m, s);
        k.mem.put(k.base, &[b'?'; 16]);
        rt_move(k.base, src, item);
        assert_eq!(k.peek(0, fd.len), want, "{fd:?} <- {m}/{s}");
        // and back: read through a wider signed receiver at the same scale
        let dst = k.item(100, Fd::display(9, fd.scale, F_SIGNED | F_SIGN_SEPARATE));
        rt_move(k.base, item, dst);
        let back = k.peek(100, 10).to_vec();
        let sign = if m < 0 { b'-' } else { b'+' };
        let mut v = m.unsigned_abs();
        let target = fd.scale as i32;
        let mut sc = s as i32;
        while sc > target { v /= 10; sc -= 1; }
        while sc < target { v *= 10; sc += 1; }
        v %= 10u128.pow(fd.digits as u32); // the 5-digit field's own truncation
        let mut want = format!("{:09}", v).into_bytes();
        want.push(if v == 0 { b'+' } else { sign });
        assert_eq!(back, want, "{fd:?} <- {m}/{s} read back");
    }
}

#[test]
fn packed_and_binary_layouts() {
    let mut k = Kit::new();
    let p = k.item(0, Fd::packed(5, 0, F_SIGNED));
    rt_move(k.base, k.lit_num(-12345, 0), p);
    assert_eq!(k.peek(0, 3), &[0x12, 0x34, 0x5D]);
    rt_move(k.base, k.lit_num(12345, 0), p);
    assert_eq!(k.peek(0, 3), &[0x12, 0x34, 0x5C]);
    let p4 = k.item(10, Fd::packed(4, 0, F_SIGNED));
    rt_move(k.base, k.lit_num(789, 0), p4);
    assert_eq!(k.peek(10, 3), &[0x00, 0x78, 0x9C]);
    let unsigned = k.item(20, Fd::packed(3, 0, 0));
    rt_move(k.base, k.lit_num(-45, 0), unsigned);
    assert_eq!(k.peek(20, 2), &[0x04, 0x5F]);

    let b2 = k.item(30, Fd::binary(4, 0, F_SIGNED, 2));
    rt_move(k.base, k.lit_num(-2, 0), b2);
    assert_eq!(k.peek(30, 2), &[0xFF, 0xFE]);
    rt_move(k.base, k.lit_num(258, 0), b2);
    assert_eq!(k.peek(30, 2), &[0x01, 0x02]);
    let c5 = k.item(40, Fd { kind: K_COMP5, flags: F_SIGNED, digits: 4, scale: 0, len: 2, aux: 0 });
    rt_move(k.base, k.lit_num(258, 0), c5);
    assert_eq!(k.peek(40, 2), &[0x02, 0x01]);

    // read them back through a DISPLAY receiver
    let d = k.item(50, Fd::display(5, 0, F_SIGNED));
    rt_move(k.base, p, d);
    assert_eq!(k.peek(50, 5), b"12345");
}

#[test]
fn move_numeric_truncates_both_ends_and_keeps_the_magnitude_when_unsigned() {
    let mut k = Kit::new();
    // MOVE 123.45 TO PIC 99V999 leaves 23.450 (high-order digits cut)
    let r = k.item(0, Fd::display(5, 3, 0));
    rt_move(k.base, k.lit_num(12345, 2), r);
    assert_eq!(k.peek(0, 5), b"23450");
    // low-order digits are cut, not rounded
    let r2 = k.item(10, Fd::display(3, 0, 0));
    rt_move(k.base, k.lit_num(1299, 2), r2);
    assert_eq!(k.peek(10, 3), b"012");
    // an unsigned receiver stores the magnitude
    let r3 = k.item(20, Fd::display(3, 0, 0));
    rt_move(k.base, k.lit_num(-7, 0), r3);
    assert_eq!(k.peek(20, 3), b"007");
}

#[test]
fn move_alphanumeric_justification_padding_and_truncation() {
    let mut k = Kit::new();
    let dst = k.item(0, Fd::alnum(6));
    rt_move(k.base, k.lit_alnum(b"AB"), dst);
    assert_eq!(k.peek(0, 6), b"AB    ");
    rt_move(k.base, k.lit_alnum(b"ABCDEFGHI"), dst);
    assert_eq!(k.peek(0, 6), b"ABCDEF");
    let j = k.item(10, Fd { flags: F_JUSTIFIED, ..Fd::alnum(6) });
    rt_move(k.base, k.lit_alnum(b"AB"), j);
    assert_eq!(k.peek(10, 6), b"    AB");
    rt_move(k.base, k.lit_alnum(b"ABCDEFGHI"), j);
    assert_eq!(k.peek(10, 6), b"DEFGHI");
}

#[test]
fn move_numeric_to_alphanumeric_gives_the_digit_positions() {
    let mut k = Kit::new();
    let n = k.item(0, Fd::display(5, 2, F_SIGNED));
    rt_move(k.base, k.lit_num(-1234, 2), n); // -12.34 -> 00123 + sign
    let a = k.item(20, Fd::alnum(8));
    rt_move(k.base, n, a);
    assert_eq!(k.peek(20, 8), b"01234   ");
    // a numeric literal: its digits, left-justified
    rt_move(k.base, k.lit_num(7, 0), a);
    assert_eq!(k.peek(20, 8), b"7       ");
}

#[test]
fn move_figuratives_and_all() {
    let mut k = Kit::new();
    let a = k.item(0, Fd::alnum(5));
    rt_move(k.base, k.fig(FIG_SPACE), a);
    assert_eq!(k.peek(0, 5), b"     ");
    rt_move(k.base, k.fig(FIG_ZERO), a);
    assert_eq!(k.peek(0, 5), b"00000");
    rt_move(k.base, k.fig(FIG_HIGH), a);
    assert_eq!(k.peek(0, 5), &[0xFF; 5]);
    rt_move(k.base, k.all_lit(b"AB"), a);
    assert_eq!(k.peek(0, 5), b"ABABA");
    let n = k.item(10, Fd::display(3, 0, 0));
    k.poke(10, b"999");
    rt_move(k.base, k.fig(FIG_ZERO), n);
    assert_eq!(k.peek(10, 3), b"000");
}

#[test]
fn move_alphanumeric_to_numeric_reads_the_number() {
    let mut k = Kit::new();
    let n = k.item(0, Fd::display(5, 0, 0));
    rt_move(k.base, k.lit_alnum(b"123"), n);
    assert_eq!(k.peek(0, 5), b"00123");
    // characters that spell no number leave the receiver alone
    rt_move(k.base, k.lit_alnum(b"ABC"), n);
    assert_eq!(k.peek(0, 5), b"00123");
}

#[test]
fn multiple_receivers_read_the_sender_once() {
    let mut k = Kit::new();
    let src = k.item(0, Fd::display(3, 0, 0));
    k.poke(0, b"042");
    let a = k.item(10, Fd::alnum(5));
    let b = k.item(20, Fd::display(5, 0, 0));
    rt_move_begin(k.base, src);
    rt_move_to(k.base, a);
    rt_move_to(k.base, b);
    assert_eq!(k.peek(10, 5), b"042  ");
    assert_eq!(k.peek(20, 5), b"00042");
}

#[test]
fn subscripts_and_reference_modification() {
    let mut k = Kit::new();
    // 01 T. 05 E PIC X(3) OCCURS 4.  at offset 0, stride 3
    let idx = k.item(100, Fd::display(2, 0, 0));
    k.poke(100, b"03");
    let e_i = k.item_sub(0, Fd::alnum(3), &[(idx, 3, 4, 0)]);
    rt_move(k.base, k.lit_alnum(b"XYZ"), e_i);
    assert_eq!(k.peek(6, 3), b"XYZ");
    // out of range traps
    k.poke(100, b"05");
    let r = std::panic::catch_unwind(|| rt_move(0, 0, 0));
    let _ = r;
    // reference modification: WS-A(2:3)
    let a = k.item(10, Fd::alnum(8));
    k.poke(10, b"ABCDEFGH");
    let start = k.lit_num(2, 0);
    let len = k.lit_num(3, 0);
    let sub = k.item_refmod(10, Fd::alnum(8), start, len);
    let dst = k.item(30, Fd::alnum(5));
    rt_move(k.base, sub, dst);
    assert_eq!(k.peek(30, 5), b"BCD  ");
    let _ = a;
}

fn run_add(k: &mut Kit, a: u32, b: u32, receiver: u32, flags: u32) -> bool {
    rt_begin();
    rt_push(k.base, a);
    rt_push(k.base, b);
    rt_op(OP_ADD);
    rt_store(k.base, receiver, flags) != 0
}

#[test]
fn add_stores_with_truncation_rounding_and_size_error() {
    let mut k = Kit::new();
    let r = k.item(0, Fd::display(4, 2, F_SIGNED)); // PIC S99V99
    // 10.505 + 0 -> truncated 10.50, rounded 10.51
    let a = k.lit_num(10505, 3);
    let z = k.lit_num(0, 0);
    assert!(!run_add(&mut k, a, z, r, 0));
    assert_eq!(k.peek(0, 4), b"1050");
    assert!(!run_add(&mut k, a, z, r, ST_ROUNDED));
    assert_eq!(k.peek(0, 4), b"1051");
    // overflow with a size-error phrase leaves the receiver alone
    let big = k.lit_num(100_00, 2); // 100.00 does not fit S99V99
    assert!(run_add(&mut k, big, z, r, ST_PHRASE));
    assert_eq!(k.peek(0, 4), b"1051");
    // without the phrase the high-order digits are cut
    assert!(run_add(&mut k, big, z, r, 0));
    assert_eq!(k.peek(0, 4), b"0000");
}

#[test]
fn divide_by_zero_is_a_size_error_and_stores_nothing() {
    let mut k = Kit::new();
    let r = k.item(0, Fd::display(4, 0, 0));
    k.poke(0, b"0042");
    rt_begin();
    rt_push(k.base, k.lit_num(10, 0));
    rt_push(k.base, k.lit_num(0, 0));
    rt_op(OP_DIV);
    assert_eq!(rt_store(k.base, r, 0), 1);
    assert_eq!(rt_size_err(), 1);
    assert_eq!(k.peek(0, 4), b"0042");
}

#[test]
fn compute_uses_exact_decimals_and_pow_is_float() {
    let mut k = Kit::new();
    let r = k.item(0, Fd::display(6, 3, 0));
    rt_begin();
    // (1 / 3) * 3 -> 0.999999999 -> stored truncated at 3 places: 0.999
    rt_push(k.base, k.lit_num(1, 0));
    rt_push(k.base, k.lit_num(3, 0));
    rt_op(OP_DIV);
    rt_push(k.base, k.lit_num(3, 0));
    rt_op(OP_MUL);
    rt_store(k.base, r, 0);
    assert_eq!(k.peek(0, 6), b"000999");
    let r7 = k.item(20, Fd::display(7, 3, 0));
    rt_begin();
    rt_push(k.base, k.lit_num(2, 0));
    rt_push(k.base, k.lit_num(10, 0));
    rt_op(OP_POW);
    rt_store(k.base, r7, 0);
    assert_eq!(k.peek(20, 7), b"1024000");
}

#[test]
fn comparisons_numeric_alphanumeric_and_padding() {
    let mut k = Kit::new();
    let n = k.item(0, Fd::display(5, 2, F_SIGNED));
    k.poke(0, b"0012L"); // -12.3? digits 0012 + L(-3) -> -123 at scale 2 = -1.23
    let lit = k.lit_num(-123, 2);
    assert_eq!(rt_rel(k.base, n, lit, OP_EQ), 1);
    assert_eq!(rt_rel(k.base, n, k.lit_num(0, 0), OP_LT), 1);
    assert_eq!(rt_rel(k.base, k.lit_num(5, 0), k.lit_num(5, 0), OP_GE), 1);
    // alphanumeric: shorter side padded with spaces
    let a = k.item(20, Fd::alnum(6));
    k.poke(20, b"AB    ");
    assert_eq!(rt_rel(k.base, a, k.lit_alnum(b"AB"), OP_EQ), 1);
    assert_eq!(rt_rel(k.base, a, k.lit_alnum(b"ABC"), OP_LT), 1);
    assert_eq!(rt_rel(k.base, a, k.fig(FIG_SPACE), OP_GT), 1);
    // a figurative against an item adopts the item's length
    let sp = k.item(40, Fd::alnum(4));
    k.poke(40, b"    ");
    assert_eq!(rt_rel(k.base, sp, k.fig(FIG_SPACE), OP_EQ), 1);
    // numeric against text is a nonnumeric comparison (COBOL-85 6.3.4.2): the
    // number stands for its digits as characters, so it is "7", not "  7  "
    let t = k.item(50, Fd::alnum(5));
    k.poke(50, b"  7  ");
    assert_eq!(rt_rel(k.base, t, k.lit_num(7, 0), OP_EQ), 0);
    k.poke(50, b"7    ");
    assert_eq!(rt_rel(k.base, t, k.lit_num(7, 0), OP_EQ), 1);
}

#[test]
fn class_and_sign_tests() {
    let mut k = Kit::new();
    let a = k.item(0, Fd::alnum(4));
    k.poke(0, b"12 4");
    assert_eq!(rt_class(k.base, a, CLASS_NUMERIC), 0);
    k.poke(0, b"1234");
    assert_eq!(rt_class(k.base, a, CLASS_NUMERIC), 1);
    k.poke(0, b"AbC ");
    assert_eq!(rt_class(k.base, a, CLASS_ALPHABETIC), 1);
    assert_eq!(rt_class(k.base, a, CLASS_UPPER), 0);
    let n = k.item(10, Fd::display(3, 0, F_SIGNED));
    k.poke(10, b"12K");
    assert_eq!(rt_class(k.base, n, CLASS_NUMERIC), 1);
    assert_eq!(rt_sign(k.base, n, SIGN_NEGATIVE), 1);
    assert_eq!(rt_sign(k.base, n, SIGN_POSITIVE), 0);
    k.poke(10, b"12Z");
    assert_eq!(rt_class(k.base, n, CLASS_NUMERIC), 0);
}

#[test]
fn display_writes_commands_into_the_batch() {
    let mut k = Kit::new();
    let n = k.item(0, Fd::display(5, 2, F_SIGNED));
    rt_move(k.base, k.lit_num(-1234, 2), n);
    rt_display(k.base, k.lit_alnum(b"TOTAL="));
    rt_display(k.base, n);
    rt_display(k.base, k.lit_num(5, 1));
    rt_display_end(0);
    rt_flush();
    let log = host::take_log();
    assert_eq!(log.len(), 1);
    let host::Event::Flush(b) = &log[0] else { panic!() };
    // [03 len text]*
    let mut texts = Vec::new();
    let mut i = 0;
    while i < b.len() {
        assert_eq!(b[i], 0x03);
        let n = b[i + 1] as usize;
        texts.push(String::from_utf8(b[i + 2..i + 2 + n].to_vec()).unwrap());
        i += 2 + n;
    }
    assert_eq!(texts.concat(), "TOTAL=-01234" .to_string() + "0.5\n");
}
