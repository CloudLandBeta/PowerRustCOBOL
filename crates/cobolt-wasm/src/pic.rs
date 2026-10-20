// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `PICTURE` strings, read once into the facts the layout needs.

use cobolt_ast::data::PicKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pic {
    pub kind: PicKind,
    /// `S` present.
    pub signed: bool,
    /// Digit positions that occupy storage (`9`s).
    pub digits: u32,
    /// Digit positions right of the implied point that occupy storage.
    pub decimals: u32,
    /// `P` positions left of the digits (`PP9(3)`: value = 0.00ddd).
    pub lead_p: u32,
    /// `P` positions right of the digits (`9(3)PP`: value = ddd00).
    pub trail_p: u32,
    /// Character positions in the picture (storage length of alphanumeric and
    /// edited items; a `CR` or `DB` is two).
    pub chars: u32,
}

impl Pic {
    /// The scale the routines use: digits right of the point, negative for
    /// trailing `P`.
    pub fn scale(&self) -> i32 {
        if self.trail_p > 0 {
            -(self.trail_p as i32)
        } else if self.lead_p > 0 {
            // `PP999`: the point sits left of the P's, so every stored digit is
            // a decimal and the P's are extra leading zeros.
            (self.digits + self.lead_p) as i32
        } else {
            self.decimals as i32
        }
    }
}

/// Expand `X(5)`-style repeat counts into a flat list of symbol characters.
fn expand(template: &str) -> Vec<char> {
    let cs: Vec<char> = template.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        i += 1;
        if i < cs.len() && cs[i] == '(' {
            // A repeat count follows.
            let mut j = i + 1;
            let mut n: u32 = 0;
            while j < cs.len() && cs[j].is_ascii_digit() {
                n = n.saturating_mul(10).saturating_add(cs[j] as u32 - '0' as u32);
                j += 1;
            }
            if j < cs.len() && cs[j] == ')' {
                for _ in 0..n {
                    out.push(c.to_ascii_uppercase());
                }
                i = j + 1;
                continue;
            }
        }
        out.push(c.to_ascii_uppercase());
    }
    out
}

pub fn parse(template: &str, kind: PicKind) -> Pic {
    let syms = expand(template);
    let mut p = Pic { kind, signed: false, digits: 0, decimals: 0, lead_p: 0, trail_p: 0, chars: 0 };
    match kind {
        PicKind::Numeric => {
            let mut after_point = false;
            let mut seen_digit = false;
            for &c in &syms {
                match c {
                    'S' => p.signed = true,
                    'V' | '.' => after_point = true,
                    'P' => {
                        if seen_digit {
                            p.trail_p += 1;
                        } else {
                            p.lead_p += 1;
                        }
                    }
                    '9' => {
                        seen_digit = true;
                        p.digits += 1;
                        if after_point {
                            p.decimals += 1;
                        }
                    }
                    _ => {}
                }
            }
            p.chars = p.digits;
        }
        PicKind::NumericEdited | PicKind::AlphanumericEdited => {
            // One character position per symbol; `CR` and `DB` fill two.
            let mut i = 0;
            let mut n = 0;
            while i < syms.len() {
                let two = i + 1 < syms.len()
                    && ((syms[i] == 'C' && syms[i + 1] == 'R') || (syms[i] == 'D' && syms[i + 1] == 'B'));
                if two {
                    n += 2;
                    i += 2;
                    continue;
                }
                // `V` and `S`-less sign-free `P` occupy no position.
                if syms[i] != 'V' && syms[i] != 'P' {
                    n += 1;
                }
                i += 1;
            }
            p.chars = n;
        }
        PicKind::National => {
            p.chars = syms.iter().filter(|c| **c == 'N').count() as u32;
        }
        PicKind::Utf8 => {
            p.chars = syms.iter().filter(|c| **c == 'U').count() as u32;
        }
        _ => {
            p.chars = syms.len() as u32;
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_pictures() {
        let p = parse("S9(5)V99", PicKind::Numeric);
        assert_eq!((p.signed, p.digits, p.decimals, p.scale()), (true, 7, 2, 2));
        let p = parse("9(3)PP", PicKind::Numeric);
        assert_eq!((p.digits, p.trail_p, p.scale()), (3, 2, -2));
        let p = parse("PP9(3)", PicKind::Numeric);
        assert_eq!((p.digits, p.lead_p, p.scale()), (3, 2, 5));
        let p = parse("V99", PicKind::Numeric);
        assert_eq!((p.digits, p.decimals), (2, 2));
        let p = parse("99V", PicKind::Numeric);
        assert_eq!((p.digits, p.decimals), (2, 0));
    }

    #[test]
    fn alphanumeric_and_edited_lengths() {
        assert_eq!(parse("X(30)", PicKind::Alphanumeric).chars, 30);
        assert_eq!(parse("AAXX", PicKind::Alphabetic).chars, 4);
        assert_eq!(parse("ZZ,ZZ9.99CR", PicKind::NumericEdited).chars, 11);
        assert_eq!(parse("$$$,$$9.99", PicKind::NumericEdited).chars, 10);
        assert_eq!(parse("N(4)", PicKind::National).chars, 4);
    }
}
