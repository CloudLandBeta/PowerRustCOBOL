// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The COBOL `PICTURE` a control's contents obey.
//!
//! A TextBox feeds a COBOL data item, so what the box will hold has to be what
//! the item can hold. The mechanism is not a coercion at run time: the
//! generated item **carries the same PICTURE**, so a comparison against it is
//! right by construction. This module is the one place that says what a given
//! picture permits.
//!
//! A picture does two jobs here:
//!
//! * **Validator** — for each character position, which bytes are legal.
//!   `PIC A(3)` takes letters and spaces; `PIC 9(3)` takes digits; `PIC X(3)`
//!   takes any byte. That is the COBOL-85 reading of `A`, `9` and `X`, not the
//!   permissive one.
//! * **Mask** — what the box *shows*. A numeric-edited picture displays its
//!   edited form when the box is not focused and the plain stored value when it
//!   is, so `PIC ZZ9.99` holding `12.34` reads `" 12.34"` at rest and
//!   `"12.34"` under the caret. (One leading space: the picture is six
//!   character positions wide, and the edited text is exactly that wide.)
//!
//! Entry is **plain text validated per keystroke**, never a separator mask: the
//! box never pre-seeds grouping characters and never walks the caret over them.
//! The operator types `1234.56`; the picture decides whether each keystroke is
//! allowed, and the editing symbols appear only in the resting display.
//!
//! The decimal separator is the form's, not the picture's: under
//! `DECIMAL-POINT IS COMMA` a `,` is the decimal point and `.` is the grouping
//! character, and both the validator and the display follow that.

use crate::numedit;

/// What one character position of a *character* picture accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PicPos {
    /// `X` — any byte.
    Any,
    /// `A` — a letter or a space (COBOL-85 alphabetic).
    Alpha,
    /// `9` — a digit.
    Digit,
    /// `B`, `0` or `/` in an alphanumeric-edited picture: the position holds
    /// exactly this character and nothing else.
    Literal(char),
}

impl PicPos {
    /// Whether `c` is legal in this position.
    pub fn accepts(self, c: char) -> bool {
        match self {
            PicPos::Any => true,
            PicPos::Alpha => c.is_alphabetic() || c == ' ',
            PicPos::Digit => c.is_ascii_digit(),
            PicPos::Literal(l) => c == l,
        }
    }

    /// The character this position holds when nothing has been typed into it.
    pub fn filler(self) -> char {
        match self {
            PicPos::Literal(l) => l,
            _ => ' ',
        }
    }
}

/// The two shapes a picture can take, which decide how entry is validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PicShape {
    /// A character picture — `X`, `A`, `9` and the alphanumeric-edited
    /// insertions. Entry is **positional**: character *i* is checked against
    /// position *i*.
    Text(Vec<PicPos>),
    /// A numeric picture, edited or not. Entry is a plain number, so it is
    /// checked as a *shape* (sign, digits, one decimal separator) rather than
    /// position by position — the operator types `1234.56` into `ZZ,ZZ9.99`
    /// and the grouping comma is never keyed.
    Number {
        /// Digit positions before the decimal separator.
        int_digits: usize,
        /// Digit positions after it.
        frac_digits: usize,
        /// The picture can carry a sign (`S`, or a `+`/`-`/`CR`/`DB` symbol).
        signed: bool,
        /// The picture carries editing symbols, so the resting display differs
        /// from the stored value.
        edited: bool,
        /// The sign symbol is written at the picture's end (`ZZ9.99-`,
        /// `9(4)CR`), so a typed sign is normalised to the trailing position.
        sign_trailing: bool,
    },
}

/// A parsed `PICTURE`, together with the template it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    template: String,
    shape: PicShape,
    width: usize,
}

/// Default picture width for a single-line TextBox whose `MaximumLength` is 0.
pub const DEFAULT_SINGLE_LINE: usize = 256;
/// Default picture width for a multiline TextBox whose `MaximumLength` is 0.
pub const DEFAULT_MULTILINE: usize = 2048;

/// The picture an existing TextBox gets when none was set: `PIC X(n)`, sized
/// from `MaximumLength`, or from the single-line / multiline default when that
/// is 0.
///
/// Once a picture *is* set explicitly its own width is authoritative and
/// `MaximumLength` no longer bounds the field.
pub fn default_textbox_picture(maximum_length: i64, multiline: bool) -> String {
    let n = if maximum_length > 0 {
        maximum_length as usize
    } else if multiline {
        DEFAULT_MULTILINE
    } else {
        DEFAULT_SINGLE_LINE
    };
    format!("X({n})")
}

/// Whether a form's `SPECIAL-NAMES` paragraph declares `DECIMAL-POINT IS
/// COMMA`, which swaps the roles of `.` and `,` in every picture on the form.
///
/// Read from the paragraph text the form carries rather than from a separate
/// switch, so there is one declaration and the generated program and the
/// designed controls cannot disagree about it.
pub fn decimal_comma_from_special_names(special_names: &str) -> bool {
    let up = special_names.to_ascii_uppercase();
    // Tolerant of the whitespace and line breaks a hand-edited paragraph
    // carries: the clause is recognised by its words, in order.
    let squashed: String = up.split_whitespace().collect::<Vec<_>>().join(" ");
    squashed.contains("DECIMAL-POINT IS COMMA") || squashed.contains("DECIMAL POINT IS COMMA")
}

/// The character a currency position prints, from `SPECIAL-NAMES. CURRENCY
/// [SIGN] [IS] literal`. `'$'` when the form declares none.
pub fn currency_from_special_names(special_names: &str) -> char {
    let up = special_names.to_ascii_uppercase();
    let squashed: String = up.split_whitespace().collect::<Vec<_>>().join(" ");
    let Some(at) = squashed.find("CURRENCY") else {
        return '$';
    };
    // Skip the optional `SIGN` and `IS` between the keyword and the literal.
    let rest = &squashed[at + "CURRENCY".len()..];
    for word in rest.split(' ') {
        match word {
            "" | "SIGN" | "IS" => continue,
            w => {
                // The literal is quoted; take its first character.
                return w
                    .trim_matches(|c| c == '\'' || c == '"' || c == '.')
                    .chars()
                    .next()
                    .unwrap_or('$');
            }
        }
    }
    '$'
}

/// Every character that only ever appears as an *editing* symbol. A picture
/// carrying one of these shows something other than what it stores.
fn is_editing_char(c: char) -> bool {
    matches!(c, 'Z' | '*' | '$' | '+' | '-' | ',' | '.' | 'B' | '0' | '/')
}

/// Whether the picture carries an editing symbol, **ignoring repeat counts**.
///
/// The digits inside `(n)` are a count, not positions: `9(10)` is a plain
/// ten-digit item. Testing the raw characters made that `0` look like a
/// zero-insertion symbol, so `PIC 9(10)` was treated as numeric-edited while
/// `PIC 9(4)` was not — the same kind of picture behaving two different ways.
fn has_editing_symbol(body: &str) -> bool {
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '(' {
            while i < chars.len() && chars[i] != ')' {
                i += 1;
            }
            i += 1; // step past the ')'
            continue;
        }
        if is_editing_char(chars[i]) {
            return true;
        }
        i += 1;
    }
    false
}

impl Picture {
    /// Parse `template` under the form's decimal-separator convention.
    ///
    /// An unrecognised or empty template degrades to a one-byte `X` rather than
    /// failing: a picture is a property a person types, and a half-typed one
    /// must not make the designer unusable.
    pub fn parse(template: &str, decimal_comma: bool) -> Picture {
        let raw = template.trim();
        let upper = raw.to_ascii_uppercase();
        // `S` is a sign *indicator*, not a position: strip it before the walk
        // so it never lands in the expansion.
        let signed_s = upper.starts_with('S');
        let body = if signed_s { &upper[1..] } else { &upper[..] };

        // A picture is a *character* picture as soon as it names a character
        // position. `9`/`Z`/`$`/`+`/`-`/`*` alone make it numeric.
        let has_char_pos = body.contains('X') || body.contains('A');
        let has_digit_pos = body.contains('9')
            || body.contains('Z')
            || body.contains('*')
            || body.contains('$')
            || body.contains('+')
            || body.contains('-');

        if has_char_pos || !has_digit_pos {
            let positions = expand_text(body);
            let width = positions.len().max(1);
            let positions = if positions.is_empty() {
                vec![PicPos::Any]
            } else {
                positions
            };
            return Picture {
                template: raw.to_string(),
                shape: PicShape::Text(positions),
                width,
            };
        }

        let (int_digits, frac_digits) = numedit::digit_counts(body, decimal_comma);
        let edited = has_editing_symbol(body);
        // A sign the picture can *hold*: `S`, or any of the symbols that print
        // one. A `+`/`-` anywhere in the template gives the item a sign.
        let signed = signed_s
            || body.contains('+')
            || body.contains('-')
            || upper.contains("CR")
            || upper.contains("DB");
        // Where a typed sign is normalised to. Only a sign symbol written at
        // the *end* of the picture puts it there; `S9(4)` and `----9.99` both
        // carry it in front, which is where a person types it.
        let tail = body.trim_end();
        let sign_trailing = tail.ends_with('-')
            || tail.ends_with('+')
            || tail.ends_with("CR")
            || tail.ends_with("DB");

        // The stored width is the edited width for an edited picture and the
        // digit count for a plain one — `PIC 9(4)V99` occupies six bytes, the
        // implied point occupying none.
        let width = if edited {
            numedit::edited_width(body, decimal_comma)
        } else {
            int_digits + frac_digits
        };

        Picture {
            template: raw.to_string(),
            shape: PicShape::Number {
                int_digits,
                frac_digits,
                signed,
                edited,
                sign_trailing,
            },
            width: width.max(1),
        }
    }

    /// The template this picture was parsed from.
    pub fn template(&self) -> &str {
        &self.template
    }

    /// The shape entry is validated against.
    pub fn shape(&self) -> &PicShape {
        &self.shape
    }

    /// Character width of the item — what the generated COBOL declares.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Whether the resting display differs from the stored value.
    pub fn is_edited(&self) -> bool {
        matches!(self.shape, PicShape::Number { edited: true, .. })
    }

    /// Whether the item is numeric.
    pub fn is_numeric(&self) -> bool {
        matches!(self.shape, PicShape::Number { .. })
    }

    /// Whether a sign may be typed at all.
    pub fn is_signed(&self) -> bool {
        match self.shape {
            PicShape::Number { signed, .. } => signed,
            PicShape::Text(_) => false,
        }
    }

    /// The longest raw (unedited) text the box will hold. For a number this is
    /// the digits plus a sign and a decimal separator, which is shorter than
    /// the edited width — the grouping characters are never typed.
    pub fn max_raw_len(&self) -> usize {
        match &self.shape {
            PicShape::Text(p) => p.len(),
            PicShape::Number {
                int_digits,
                frac_digits,
                signed,
                ..
            } => {
                int_digits
                    + frac_digits
                    + usize::from(*frac_digits > 0)
                    + usize::from(*signed)
            }
        }
    }

    /// Whether `raw` is a legal **complete** value for this picture.
    pub fn accepts(&self, raw: &str, decimal_comma: bool) -> bool {
        match &self.shape {
            PicShape::Text(positions) => {
                let chars: Vec<char> = raw.chars().collect();
                chars.len() <= positions.len()
                    && chars.iter().zip(positions).all(|(c, p)| p.accepts(*c))
            }
            PicShape::Number { .. } => self
                .scan_number(raw, decimal_comma)
                .is_some_and(|n| n.digits > 0),
        }
    }

    /// Whether `raw` is legal **so far** — the per-keystroke check.
    ///
    /// This is looser than [`Picture::accepts`] in exactly one way: a number
    /// that has been started but not finished (`"-"`, `"12."`) passes, because
    /// a person has to be able to type it.
    pub fn accepts_partial(&self, raw: &str, decimal_comma: bool) -> bool {
        match &self.shape {
            PicShape::Text(_) => self.accepts(raw, decimal_comma),
            PicShape::Number { .. } => self.scan_number(raw, decimal_comma).is_some(),
        }
    }

    /// Whether typing `c` at `caret` (a character index into `raw`) is allowed.
    ///
    /// The candidate string is built and checked as a whole, so a keystroke
    /// that would push a later character out of its class — inserting a letter
    /// in front of the digits of `PIC XXX999` — is refused, which a
    /// position-only test would miss.
    pub fn accepts_keystroke(
        &self,
        raw: &str,
        caret: usize,
        c: char,
        decimal_comma: bool,
    ) -> bool {
        let mut candidate: Vec<char> = raw.chars().collect();
        let at = caret.min(candidate.len());
        candidate.insert(at, c);
        let candidate: String = candidate.into_iter().collect();
        self.accepts_partial(&candidate, decimal_comma)
    }

    /// Move a typed sign to where the picture puts it, and drop a second one.
    ///
    /// The operator may type the sign at either end; the picture decides where
    /// it ends up. A picture that cannot hold a sign loses it entirely — that
    /// holds however the value reached here, so a value that arrived from
    /// somewhere other than the keyboard cannot leave a sign on an unsigned
    /// item.
    pub fn normalize_sign(&self, raw: &str, _decimal_comma: bool) -> String {
        let PicShape::Number {
            signed,
            sign_trailing,
            ..
        } = self.shape
        else {
            return raw.to_string();
        };
        let body: String = raw.chars().filter(|c| *c != '+' && *c != '-').collect();
        if !signed {
            return body;
        }
        // The sign is read the way `numedit::deedit` reads it — a `-` anywhere
        // in the value makes it negative — so the box and the interpreter
        // cannot disagree about which values are negative.
        if !raw.contains('-') {
            return body;
        }
        if sign_trailing {
            format!("{body}-")
        } else {
            format!("-{body}")
        }
    }

    /// What the box shows.
    ///
    /// Focused, that is the raw stored value — the operator edits what the item
    /// holds. Unfocused, a numeric-edited picture shows its edited form and a
    /// character picture is padded to its own width.
    pub fn display(
        &self,
        raw: &str,
        focused: bool,
        decimal_comma: bool,
        currency: char,
    ) -> String {
        if focused {
            return raw.to_string();
        }
        match &self.shape {
            PicShape::Text(positions) => {
                let mut out: String = raw.chars().take(positions.len()).collect();
                let have = out.chars().count();
                for p in positions.iter().skip(have) {
                    out.push(p.filler());
                }
                out
            }
            PicShape::Number { edited: false, .. } => raw.to_string(),
            PicShape::Number { .. } => {
                let (mantissa, decimals) = numedit::deedit(raw, decimal_comma);
                numedit::format_edited(
                    &self.template,
                    mantissa,
                    decimals,
                    decimal_comma,
                    currency,
                )
            }
        }
    }
}

/// What a scan of a raw numeric entry found.
struct NumScan {
    /// Digit positions actually typed, across both sides of the separator.
    /// Zero means the entry carries no value yet — a lone sign, or `"."`.
    digits: usize,
}

impl Picture {
    /// Read `raw` as a plain typed number and check it against the picture's
    /// digit budget. `None` means it is not a number this picture can hold.
    ///
    /// Accepts an unfinished entry — a lone sign, or digits ending in the
    /// decimal separator — so the per-keystroke check can use it.
    fn scan_number(&self, raw: &str, decimal_comma: bool) -> Option<NumScan> {
        let PicShape::Number {
            int_digits,
            frac_digits,
            signed,
            ..
        } = self.shape
        else {
            return None;
        };
        let dec_char = if decimal_comma { ',' } else { '.' };

        let mut seen_sign = false;
        let mut seen_point = false;
        let mut int_seen = 0usize;
        let mut frac_seen = 0usize;

        for c in raw.chars() {
            match c {
                '+' | '-' => {
                    // One sign only. Which end it sits at is not policed here:
                    // the operator may type it at either, and `normalize_sign`
                    // moves it to where the picture puts it.
                    if !signed || seen_sign {
                        return None;
                    }
                    seen_sign = true;
                }
                c if c == dec_char => {
                    if seen_point || frac_digits == 0 {
                        return None;
                    }
                    seen_point = true;
                }
                c if c.is_ascii_digit() => {
                    if seen_point {
                        frac_seen += 1;
                        if frac_seen > frac_digits {
                            return None;
                        }
                    } else {
                        int_seen += 1;
                        if int_seen > int_digits {
                            return None;
                        }
                    }
                }
                _ => return None,
            }
        }
        Some(NumScan {
            digits: int_seen + frac_seen,
        })
    }
}

/// Expand a character picture into one [`PicPos`] per stored byte.
fn expand_text(body: &str) -> Vec<PicPos> {
    let chars: Vec<char> = body.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let pos = match chars[i] {
            'X' => Some(PicPos::Any),
            'A' => Some(PicPos::Alpha),
            '9' => Some(PicPos::Digit),
            'B' => Some(PicPos::Literal(' ')),
            '0' => Some(PicPos::Literal('0')),
            '/' => Some(PicPos::Literal('/')),
            _ => None,
        };
        i += 1;
        let Some(pos) = pos else { continue };
        // An optional repeat count: `X(20)`, `A(3)`.
        if chars.get(i) == Some(&'(') {
            let mut j = i + 1;
            let mut n = 0usize;
            let mut any = false;
            while j < chars.len() && chars[j].is_ascii_digit() {
                n = n * 10 + (chars[j] as usize - '0' as usize);
                any = true;
                j += 1;
            }
            if any && chars.get(j) == Some(&')') {
                for _ in 0..n.max(1) {
                    out.push(pos);
                }
                i = j + 1;
                continue;
            }
        }
        out.push(pos);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_character_picture_expands_one_position_per_byte() {
        let p = Picture::parse("X(4)", false);
        assert_eq!(p.width(), 4);
        assert!(matches!(p.shape(), PicShape::Text(v) if v.len() == 4));
        assert!(!p.is_numeric());
    }

    #[test]
    fn alphabetic_takes_letters_and_space_only() {
        let p = Picture::parse("A(3)", false);
        assert!(p.accepts("ABC", false));
        assert!(p.accepts("A B", false));
        assert!(!p.accepts("AB1", false));
        assert!(!p.accepts("AB-", false));
    }

    #[test]
    fn nine_in_a_character_picture_takes_digits_only() {
        let p = Picture::parse("XX99", false);
        assert!(p.accepts("ab12", false));
        assert!(!p.accepts("ab1c", false));
        // A letter in front pushes a digit into an `X` slot and a letter into a
        // `9` slot — refused as a whole, which a position-only test would miss.
        assert!(!p.accepts_keystroke("ab12", 0, 'z', false));
    }

    #[test]
    fn x_takes_any_byte() {
        let p = Picture::parse("X(5)", false);
        assert!(p.accepts("a1-$ ", false));
        assert!(!p.accepts("toolong", false));
    }

    #[test]
    fn an_insertion_position_holds_exactly_its_character() {
        let p = Picture::parse("XXBXX", false);
        assert_eq!(p.width(), 5);
        assert!(p.accepts("ab cd", false));
        assert!(!p.accepts("abxcd", false));
    }

    #[test]
    fn a_signed_numeric_picture_takes_digits_and_a_sign() {
        let p = Picture::parse("S9(4)", false);
        assert!(p.is_numeric());
        assert!(p.is_signed());
        assert!(!p.is_edited());
        assert_eq!(p.width(), 4);
        assert!(p.accepts("1234", false));
        assert!(p.accepts("-1234", false));
        assert!(p.accepts("+1234", false));
        assert!(!p.accepts("12345", false), "one digit over the budget");
        assert!(!p.accepts("12a4", false));
        assert!(!p.accepts("--12", false), "a second sign");
    }

    #[test]
    fn an_unsigned_numeric_picture_refuses_a_sign() {
        let p = Picture::parse("9(4)", false);
        assert!(!p.is_signed());
        assert!(!p.accepts("-123", false));
    }

    #[test]
    fn the_decimal_separator_follows_the_form_not_the_picture() {
        // `ZZZ.ZZ9,99-` reads with `,` as the decimal point under
        // DECIMAL-POINT IS COMMA, and `.` as grouping.
        let p = Picture::parse("ZZZ.ZZ9,99-", true);
        assert!(p.is_numeric());
        assert!(p.is_signed());
        assert!(p.is_edited());
        assert!(p.accepts("123456,78", true), "comma is the decimal point");
        assert!(!p.accepts("123456.78", true), "a period is grouping, not typed");

        // The same template with the default convention splits at the period.
        let q = Picture::parse("ZZZ.ZZ9", false);
        assert!(q.accepts("123.456", false));
    }

    #[test]
    fn entry_is_plain_text_never_a_separator_mask() {
        // The grouping comma of `ZZ,ZZ9.99` is never keyed: the operator types
        // the digits and the point, and nothing else.
        let p = Picture::parse("ZZ,ZZ9.99", false);
        assert!(p.accepts("1234.56", false));
        assert!(!p.accepts("1,234.56", false), "the grouping char is not typed");
        // Five integer digit positions (`Z Z Z Z 9` — the comma is grouping,
        // not a digit), two fractional, and the decimal point itself.
        assert_eq!(p.max_raw_len(), 5 + 2 + 1);
    }

    #[test]
    fn a_partial_number_is_typeable() {
        let p = Picture::parse("S9(3)V99", false);
        assert!(p.accepts_partial("-", false));
        assert!(p.accepts_partial("12.", false));
        assert!(!p.accepts("-", false), "not a complete value");
        // But an over-long one is refused the moment it is keyed.
        assert!(p.accepts_keystroke("12", 2, '3', false));
        assert!(!p.accepts_keystroke("123", 3, '4', false));
    }

    #[test]
    fn display_is_edited_at_rest_and_raw_under_the_caret() {
        let p = Picture::parse("ZZ9.99", false);
        // Six character positions (`Z Z 9 . 9 9`), so ONE leading space, not
        // two — the edited text is exactly as wide as the picture.
        assert_eq!(p.display("12.34", false, false, '$'), " 12.34");
        assert_eq!(p.display("12.34", true, false, '$'), "12.34");
    }

    /// The worked table in the Developer's Guide, pinned. A documented example
    /// that drifts is worse than none, so every row is asserted here and the
    /// widths are checked against the picture's own.
    #[test]
    fn the_documented_display_table_is_accurate() {
        for (template, raw, at_rest) in [
            ("ZZ9.99", "12.34", " 12.34"),
            ("ZZ,ZZ9.99", "1234.5", " 1,234.50"),
            ("$$,$$9.99", "1234.5", "$1,234.50"),
            ("**,**9.99", "12.3", "****12.30"),
            ("9(4).99CR", "-12.34", "0012.34CR"),
        ] {
            let p = Picture::parse(template, false);
            assert_eq!(
                p.display(raw, false, false, '$'),
                at_rest,
                "{template} holding {raw}"
            );
            // Focused, the box always shows what the item holds.
            assert_eq!(p.display(raw, true, false, '$'), raw, "{template} focused");
            assert_eq!(
                at_rest.chars().count(),
                p.width(),
                "{template}: edited text must be exactly the picture's width"
            );
        }
    }

    #[test]
    fn a_character_picture_pads_to_its_width_at_rest() {
        let p = Picture::parse("X(6)", false);
        assert_eq!(p.display("ab", false, false, '$'), "ab    ");
        assert_eq!(p.display("ab", true, false, '$'), "ab");
    }

    #[test]
    fn a_plain_numeric_picture_shows_what_it_stores() {
        let p = Picture::parse("9(4)", false);
        assert_eq!(p.display("0123", false, false, '$'), "0123");
    }

    /// A repeat count is a **count**, not positions. The digits inside `(n)`
    /// once made `9(10)` look edited (its `0` read as a zero-insertion symbol)
    /// while `9(4)` did not — the same kind of picture treated two ways.
    #[test]
    fn a_repeat_count_does_not_make_a_picture_edited() {
        for template in ["9(4)", "9(10)", "9(30)", "S9(10)", "9(10)V99"] {
            let p = Picture::parse(template, false);
            assert!(
                !p.is_edited(),
                "{template} is a plain numeric item, not an edited one"
            );
        }
        assert_eq!(Picture::parse("9(10)", false).width(), 10);
        assert_eq!(Picture::parse("9(10)V99", false).width(), 12);
        assert_eq!(Picture::parse("9(10)", false).display("123", false, false, '$'), "123");
        // …and the genuinely edited ones still are.
        for template in ["ZZ9.99", "$$,$$9.99", "**,**9.99", "9(4).99CR", "9(3)B9(2)"] {
            assert!(Picture::parse(template, false).is_edited(), "{template}");
        }
    }

    #[test]
    fn a_sign_typed_at_either_end_lands_where_the_picture_puts_it() {
        // A trailing sign symbol keeps it at the end...
        let trailing = Picture::parse("ZZ9.99-", false);
        assert_eq!(trailing.normalize_sign("-12.34", false), "12.34-");
        assert_eq!(trailing.normalize_sign("12.34-", false), "12.34-");
        // ...and a leading one, or none spelled at all, keeps it in front.
        let leading = Picture::parse("S9(4)", false);
        assert_eq!(leading.normalize_sign("1234-", false), "-1234");
        assert_eq!(leading.normalize_sign("-1234", false), "-1234");
        // A positive value carries no sign either way.
        assert_eq!(trailing.normalize_sign("12.34", false), "12.34");
    }

    #[test]
    fn an_unsigned_picture_drops_a_typed_sign() {
        let p = Picture::parse("9(4)", false);
        assert_eq!(p.normalize_sign("-1234", false), "1234");
    }

    #[test]
    fn the_default_picture_is_sized_from_maximum_length() {
        assert_eq!(default_textbox_picture(40, false), "X(40)");
        assert_eq!(default_textbox_picture(0, false), "X(256)");
        assert_eq!(default_textbox_picture(0, true), "X(2048)");
        // An explicit picture is authoritative: 40 here is not consulted.
        assert_eq!(Picture::parse("X(12)", false).width(), 12);
    }

    #[test]
    fn the_separator_convention_is_read_from_the_forms_special_names() {
        assert!(!decimal_comma_from_special_names(""));
        assert!(decimal_comma_from_special_names(
            "       DECIMAL-POINT IS COMMA."
        ));
        // Tolerant of the line breaks and spacing a hand-edited paragraph
        // carries — the clause is recognised by its words.
        assert!(decimal_comma_from_special_names(
            "SPECIAL-NAMES.\n           DECIMAL-POINT\n           IS   COMMA."
        ));
        assert!(!decimal_comma_from_special_names("CURRENCY SIGN IS '€'."));
    }

    #[test]
    fn the_currency_character_is_read_from_the_forms_special_names() {
        assert_eq!(currency_from_special_names(""), '$');
        assert_eq!(currency_from_special_names("CURRENCY SIGN IS 'E'."), 'E');
        assert_eq!(currency_from_special_names("CURRENCY IS 'F'."), 'F');
        assert_eq!(
            currency_from_special_names("DECIMAL-POINT IS COMMA. CURRENCY SIGN IS 'K'."),
            'K'
        );
    }

    #[test]
    fn a_half_typed_picture_degrades_instead_of_failing() {
        // A picture is a property a person types; a partial one must not make
        // the designer unusable.
        let p = Picture::parse("", false);
        assert_eq!(p.width(), 1);
        assert!(p.accepts("q", false));
        let q = Picture::parse("X(", false);
        assert_eq!(q.width(), 1);
    }

    #[test]
    fn currency_and_check_protection_edit_at_rest() {
        let p = Picture::parse("$$,$$9.99", false);
        assert!(p.is_edited());
        assert_eq!(p.display("1234.5", false, false, '$'), "$1,234.50");
        let star = Picture::parse("**,**9.99", false);
        assert_eq!(star.display("12.3", false, false, '$'), "****12.30");
    }

    #[test]
    fn cr_and_db_are_signs_at_the_tail() {
        let p = Picture::parse("9(4).99CR", false);
        assert!(p.is_signed());
        assert_eq!(p.normalize_sign("-12.34", false), "12.34-");
        assert_eq!(p.display("-12.34", false, false, '$'), "0012.34CR");
    }
}
