// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `no_std` engine against the original: same templates, same values, same
//! characters. The original (`tests/reference`) is the file this crate was
//! moved out of, kept here verbatim as the oracle.

mod reference;

use cobolt_numedit as new;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 ^ (self.0 >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A random picture built from the symbols the engine knows.
fn random_template(g: &mut Lcg) -> String {
    let pools: [&[&str]; 8] = [
        &["9", "Z", "*", "$", "+", "-", ",", "B", "0", "/"],
        &["9", "9", "Z", "Z", ","],
        &["$", "$", "$", "9", "9", ","],
        &["-", "-", "-", "9", ","],
        &["+", "+", "9", "9"],
        &["*", "*", "*", "9", ","],
        &["9", "9", "9", "B", "/"],
        &["Z", "Z", "9", "0"],
    ];
    let pool = pools[g.below(pools.len() as u64) as usize];
    let mut t = String::new();
    let n = 1 + g.below(7);
    for _ in 0..n {
        let s = pool[g.below(pool.len() as u64) as usize];
        if g.below(6) == 0 {
            t.push_str(&format!("{s}({})", 1 + g.below(4)));
        } else {
            t.push_str(s);
        }
    }
    if g.below(2) == 0 {
        t.push('.');
        for _ in 0..g.below(4) {
            t.push(['9', 'Z', '*', '9'][g.below(4) as usize]);
        }
    }
    match g.below(6) {
        0 => t.push_str("CR"),
        1 => t.push_str("DB"),
        2 => t.push('-'),
        3 => t.push('+'),
        _ => {}
    }
    if g.below(15) == 0 {
        t.push_str("PP");
    }
    t
}

#[test]
fn format_edited_matches_the_original() {
    let mut g = Lcg(0xC0B01);
    let mut cases = 0u64;
    let mut skipped = 0u64;
    for _ in 0..200_000 {
        let tpl = random_template(&mut g);
        let mantissa = match g.below(4) {
            0 => 0i128,
            1 => (g.below(2_000_000) as i128) - 1_000_000,
            2 => (g.next() as i128) * if g.below(2) == 0 { 1 } else { -1 },
            _ => ((g.next() as i128) << 20) * if g.below(2) == 0 { 1 } else { -1 },
        };
        let decimals = g.below(8) as u8;
        let comma = g.below(5) == 0;
        let currency = if g.below(5) == 0 { '<' } else { '$' };
        // Some random pictures are not valid COBOL and the original panics on
        // them; those cases say nothing about equivalence.
        let r = std::panic::catch_unwind(|| reference::format_edited(&tpl, mantissa, decimals, comma, currency));
        let Ok(want) = r else {
            skipped += 1;
            continue;
        };
        let mut buf = [0u8; new::MAX_OUT];
        let n = new::format_edited(tpl.as_bytes(), mantissa, decimals, comma, currency as u8, &mut buf);
        assert_eq!(
            std::str::from_utf8(&buf[..n]).unwrap(),
            want,
            "template {tpl:?} value {mantissa}/{decimals} comma={comma} currency={currency}"
        );
        assert_eq!(new::edited_width(tpl.as_bytes(), comma), reference::edited_width(&tpl, comma), "width {tpl:?}");
        assert_eq!(new::digit_counts(tpl.as_bytes(), comma), reference::digit_counts(&tpl, comma), "counts {tpl:?}");
        cases += 1;
    }
    println!("RESULT numedit differential: {cases} random (template, value) cases compared, 0 differences ({skipped} invalid pictures skipped: the original panics on them)");
    assert!(cases > 100_000, "too few valid cases: {cases}");
}

#[test]
fn deedit_matches_the_original() {
    let mut g = Lcg(0xDEED17);
    for _ in 0..100_000 {
        let alphabet: &[u8] = b"0123456789.,-+$ *BCRDbcrd/";
        let len = g.below(14) as usize;
        let s: String = (0..len).map(|_| alphabet[g.below(alphabet.len() as u64) as usize] as char).collect();
        let comma = g.below(4) == 0;
        assert_eq!(new::deedit(s.as_bytes(), comma), reference::deedit(&s, comma), "deedit {s:?} comma={comma}");
    }
}
