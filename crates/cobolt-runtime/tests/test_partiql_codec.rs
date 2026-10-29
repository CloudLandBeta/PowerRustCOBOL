// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 108, T4 — the field codec. `decode_field` must give what `distribute`
//! leaves in the environment, and `encode_field` must write what
//! `field_bytes` reads back, for every usage and sign form the layout holds.
//! `distribute` and `field_bytes` themselves are NIST-critical and untouched.
//!
//! One result block at the end (GOLDEN RULE #7).

#[path = "support/partiql_fixtures.rs"]
mod fx;

use cobolt_runtime::environment::CobolEnvironment;
use cobolt_runtime::files::{compute_layout, decode_field, encode_field, EncodeError, FieldPos, RecordLayout};
use cobolt_runtime::partiql::schema::parse_fd_text;
use cobolt_runtime::partiql::value::Value;
use cobolt_runtime::value::{CobolNumeric, CobolValue};
use fx::Lcg;

fn layout(fd: &str) -> RecordLayout {
    compute_layout(&parse_fd_text(fd).expect("fd parses"))
}

/// A Number that fits `digits` integer digits and `decimals` decimals.
fn number(rng: &mut Lcg, f: &FieldPos, signed: bool) -> CobolNumeric {
    let cap = f.len - usize::from(f.sign_separate);
    let digits = 1 + (rng.next() as usize % cap);
    let mut m: i128 = 0;
    for _ in 0..digits {
        m = m * 10 + (rng.next() % 10) as i128;
    }
    if signed && rng.next() % 2 == 0 {
        m = -m;
    }
    CobolNumeric::new(m, f.decimals)
}

fn text(rng: &mut Lcg, f: &FieldPos) -> String {
    let n = (rng.next() as usize) % (f.len + 1);
    let alphabet = ['A', 'b', 'Z', '9', '-', ' ', 'é', '/'];
    let mut s = String::new();
    while s.len() < n {
        let c = alphabet[(rng.next() % alphabet.len() as u64) as usize];
        if s.len() + c.len_utf8() <= n {
            s.push(c);
        }
    }
    s
}

#[test]
fn partiql_codec() {
    let t0 = std::time::Instant::now();
    let mut rng = Lcg(42);
    let mut checked = 0usize;
    let mut forms: Vec<String> = Vec::new();
    let mut failures: Vec<String> = Vec::new();

    // Fields whose image the old paths build and read back: DISPLAY, COMP,
    // COMP-3, BINARY, SIGN LEADING/TRAILING [SEPARATE] — the layout gives a
    // numeric item `digits + decimals (+ 1 for a separate sign)` bytes whatever
    // its usage, which `decode_field` follows.
    let fd = "FD T.\n01 REC.\n\
      05 U1 PIC 9(5).\n05 U2 PIC 9(3)V99.\n05 S1 PIC S9(4).\n05 S2 PIC S9(6)V99.\n\
      05 C1 PIC S9(7)V99 COMP.\n05 C3 PIC S9(7)V99 COMP-3.\n05 B1 PIC S9(4) BINARY.\n\
      05 TX PIC X(12).\n05 AL PIC A(6).\n05 TRL PIC S9(5) SIGN IS TRAILING.\n05 LDG PIC S9(5) SIGN IS LEADING.\n";
    forms.push("DISPLAY, COMP, COMP-3, BINARY, signed overpunch, X, A".into());
    let lay = layout(fd);
    for f in lay.fields.iter().filter(|f| !f.is_group) {
        for _ in 0..400 {
            let mut env = CobolEnvironment::new();
            let key = f.name.clone();
            // Build the image the way a WRITE does (field_bytes through materialize).
            let (v, image) = if f.numeric {
                let n = number(&mut rng, f, f.signed);
                env.set(&key, CobolValue::Numeric(n.clone()));
                (Value::Num(n), lay.materialize(&env))
            } else {
                let s = text(&mut rng, f);
                env.set_str(&key, &s);
                (Value::text(&s), lay.materialize(&env))
            };
            // decode == what distribute leaves in the environment.
            let mut env2 = CobolEnvironment::new();
            lay.distribute(&mut env2, &image);
            let got = decode_field(f, &image);
            let via_env = match env2.get(&key) {
                Some(CobolValue::Numeric(n)) => Value::Num(n.clone()),
                Some(other) => Value::text(&other.as_display_string()),
                None => Value::Str(String::new()),
            };
            let same = |a: &Value, b: &Value| match (a, b) {
                (Value::Num(x), Value::Num(y)) => x.mantissa == y.mantissa && x.decimals == y.decimals,
                (a, b) => a == b,
            };
            if !same(&got, &via_env) {
                failures.push(format!("decode {}: {got:?} vs distribute {via_env:?}", f.name));
            }
            // and it is the value that was written.
            if !same(&got, &v) {
                failures.push(format!("round trip {}: {got:?} vs {v:?}", f.name));
            }
            // encode == what field_bytes wrote.
            let mut buf = vec![b' '; lay.len];
            match encode_field(f, &v, &mut buf) {
                Ok(()) => {
                    if buf[f.offset..f.offset + f.len] != image[f.offset..f.offset + f.len] {
                        failures.push(format!(
                            "encode {}: {:?} vs field_bytes {:?}",
                            f.name,
                            String::from_utf8_lossy(&buf[f.offset..f.offset + f.len]),
                            String::from_utf8_lossy(&image[f.offset..f.offset + f.len])
                        ));
                    }
                }
                Err(e) => failures.push(format!("encode {}: {e:?}", f.name)),
            }
            checked += 1;
        }
    }

    // A separate sign (leading and trailing): the old write path drops the
    // sign of a negative value, so these are held to a round trip and to the
    // image the standard describes, not to `field_bytes`.
    forms.push("SIGN LEADING SEPARATE, SIGN TRAILING SEPARATE".into());
    let sep = layout("FD T.\n01 R.\n05 SL PIC S9(4)V9 SIGN IS LEADING SEPARATE.\n05 ST PIC S9(4)V9 SIGN IS TRAILING SEPARATE.\n");
    for f in &sep.fields {
        for _ in 0..300 {
            let n = number(&mut rng, f, true);
            let mut buf = vec![b' '; sep.len];
            encode_field(f, &Value::Num(n.clone()), &mut buf).unwrap();
            let img = &buf[f.offset..f.offset + f.len];
            let digits = n.mantissa.unsigned_abs().to_string();
            let sign = if n.mantissa < 0 { '-' } else { '+' };
            let body = format!("{digits:0>w$}", w = f.len - 1);
            let want = if f.sign_leading { format!("{sign}{body}") } else { format!("{body}{sign}") };
            if img != want.as_bytes() {
                failures.push(format!("separate-sign image {}: {:?} vs {want}", f.name, String::from_utf8_lossy(img)));
            }
            if decode_field(f, &buf) != Value::Num(n.clone()) {
                failures.push(format!("separate-sign round trip {}", f.name));
            }
            checked += 1;
        }
    }

    // National and UTF-8 fields: the class image, both ways.
    forms.push("PIC N, PIC U".into());
    let nat = layout("FD T.\n01 R.\n05 NN PIC N(6).\n05 UU PIC U(6).\n");
    for f in &nat.fields {
        for s in ["", "añb", "日本語", "abcdef", "abcdefgh"] {
            let mut buf = vec![b' '; nat.len];
            encode_field(f, &Value::text(s), &mut buf).unwrap();
            let kept: String = s.chars().take(6).collect();
            if decode_field(f, &buf) != Value::text(&kept) {
                failures.push(format!("class {} {s:?}: {:?}", f.name, decode_field(f, &buf)));
            }
            checked += 1;
        }
    }

    // OCCURS and REDEFINES: every occurrence at its own place; a redefinition shows the same bytes.
    forms.push("OCCURS, REDEFINES".into());
    let occ = layout("FD T.\n01 R.\n05 Q PIC 9(3) OCCURS 4.\n05 K PIC X(6).\n05 KR REDEFINES K PIC 9(6).\n");
    let qs: Vec<&FieldPos> = occ.fields.iter().filter(|f| f.name == "Q").collect();
    let mut buf = vec![b' '; occ.len];
    for (i, q) in qs.iter().enumerate() {
        encode_field(q, &Value::int(100 + i as i64), &mut buf).unwrap();
    }
    for (i, q) in qs.iter().enumerate() {
        if decode_field(q, &buf) != Value::int(100 + i as i64) {
            failures.push(format!("occurrence {i}"));
        }
        checked += 1;
    }
    let k = occ.fields.iter().find(|f| f.name == "K").unwrap();
    let kr = occ.fields.iter().find(|f| f.name == "KR").unwrap();
    encode_field(k, &Value::text("123456"), &mut buf).unwrap();
    if k.offset != kr.offset || decode_field(kr, &buf) != Value::int(123456) {
        failures.push("REDEFINES".into());
    }
    checked += 2;

    // The rules of MOVE: a long String is cut on the right, decimals are cut,
    // too many integer digits are refused, a Number does not go into text.
    forms.push("truncation, overflow, type mismatch".into());
    let u2 = lay.fields.iter().find(|f| f.name == "U2").unwrap();
    let tx = lay.fields.iter().find(|f| f.name == "TX").unwrap();
    let mut buf = vec![b' '; lay.len];
    let mut rule = |ok: bool, what: &str| {
        if !ok {
            failures.push(what.to_string());
        }
        checked += 1;
    };
    encode_field(tx, &Value::text("abcdefghijklmnop"), &mut buf).unwrap();
    rule(decode_field(tx, &buf) == Value::text("abcdefghijkl"), "string cut on the right");
    encode_field(u2, &Value::Num(CobolNumeric::new(123456, 3)), &mut buf).unwrap();
    rule(decode_field(u2, &buf) == Value::Num(CobolNumeric::new(12345, 2)), "decimals cut");
    rule(encode_field(u2, &Value::int(1000), &mut buf) == Err(EncodeError::Overflow), "overflow refused");
    rule(buf[u2.offset..u2.offset + u2.len] == *b"12345", "nothing changed by the refused value");
    rule(encode_field(tx, &Value::int(1), &mut buf) == Err(EncodeError::TypeMismatch), "number into text");
    rule(encode_field(u2, &Value::text("x"), &mut buf) == Err(EncodeError::TypeMismatch), "text into number");
    encode_field(u2, &Value::Num(CobolNumeric::new(-150, 2)), &mut buf).unwrap();
    rule(decode_field(u2, &buf) == Value::Num(CobolNumeric::new(150, 2)), "unsigned keeps the magnitude");

    println!("\n=== test_partiql_codec ===");
    println!("forms exercised: {}", forms.join("; "));
    println!("fields checked: {checked} (each decode against distribute, each encode against field_bytes where the old path is right)");
    println!("elapsed {} ms; failures: {}", t0.elapsed().as_millis(), failures.len());
    for f in failures.iter().take(20) {
        println!("  FAIL {f}");
    }
    assert!(failures.is_empty());
}
