// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 106 — the record image of the row a DataGrid's Button column was clicked on.
//!
//! The generated main program owns `DATAGRID-ROW-BUFFER PIC X(n)`; the handler describes
//! the same bytes as `DATAGRID-ROW-CONTENT` (`cobolt_forms::row_content`). This builds the
//! bytes: ROW-NUMBER, ROW-NAME, then each column's cell in its own picture's DISPLAY form,
//! so the handler's fields read the values — a signed number with its sign overpunched on
//! the last digit, an implied decimal point taking no byte — exactly as a record READ into
//! the same description would.

use cobolt_forms::picture::{PicShape, Picture};
use cobolt_forms::row_content::{self, Field};

/// The bytes of `DATAGRID-ROW-CONTENT` for the row numbered `row_number` (1-based, in
/// `Rows`), clicked on the button named `button`, holding `values` — one per field, in
/// order; a missing one is empty.
pub(crate) fn image(
    fields: &[Field],
    row_number: usize,
    button: &str,
    values: &[String],
    decimal_comma: bool,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(row_content::record_width(fields));
    out.extend(cell(&row_number.to_string(), row_content::ROW_NUMBER.1, decimal_comma));
    out.extend(cell(button, row_content::ROW_NAME.1, decimal_comma));
    for (i, f) in fields.iter().enumerate() {
        let v = values.get(i).map(String::as_str).unwrap_or("");
        out.extend(cell(v, &f.pic, decimal_comma));
    }
    out
}

/// One value in picture `pic`'s DISPLAY form, exactly its width.
fn cell(value: &str, pic: &str, decimal_comma: bool) -> Vec<u8> {
    let picture = Picture::parse(pic, decimal_comma);
    let width = picture.width();
    match picture.shape() {
        PicShape::Text(_) => fit(value, width),
        PicShape::Number { edited: true, .. } => {
            fit(&picture.display(value.trim(), false, decimal_comma, '$'), width)
        }
        PicShape::Number { int_digits, frac_digits, signed, .. } => {
            number(value, *int_digits, *frac_digits, *signed, decimal_comma)
        }
    }
}

/// `value` left-justified in `width` bytes: cut at a character boundary, padded with
/// spaces.
fn fit(value: &str, width: usize) -> Vec<u8> {
    let mut cut = value.len().min(width);
    while !value.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut out = value.as_bytes()[..cut].to_vec();
    out.resize(width, b' ');
    out
}

/// A plain numeric DISPLAY item: `int_digits + frac_digits` digits, the decimal point
/// implied, high-order digits cut as a MOVE cuts them, and — when the picture is signed and
/// the value negative — the sign overpunched on the last digit.
fn number(value: &str, int_digits: usize, frac_digits: usize, signed: bool, decimal_comma: bool) -> Vec<u8> {
    let point = if decimal_comma { ',' } else { '.' };
    let text = value.trim();
    let negative = text.starts_with('-') || text.ends_with('-');
    let (int_part, frac_part) = match text.split_once(point) {
        Some((i, f)) => (i, f),
        None => (text, ""),
    };
    let digits = |s: &str| -> String { s.chars().filter(char::is_ascii_digit).collect() };
    let int_d = digits(int_part);
    let frac_d = digits(frac_part);
    let mut int_s: String = int_d.chars().rev().take(int_digits).collect::<String>().chars().rev().collect();
    while int_s.len() < int_digits {
        int_s.insert(0, '0');
    }
    let mut frac_s: String = frac_d.chars().take(frac_digits).collect();
    while frac_s.len() < frac_digits {
        frac_s.push('0');
    }
    let mut out: Vec<u8> = format!("{int_s}{frac_s}").into_bytes();
    let is_zero = out.iter().all(|&b| b == b'0');
    if signed && negative && !is_zero {
        if let Some(last) = out.last_mut() {
            *last = crate::files::overpunch(*last - b'0', true);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(name: &str, pic: &str) -> Field {
        Field {
            name: name.into(),
            pic: pic.into(),
            width: cobolt_forms::row_content::pic_width(pic),
            column_id: name.into(),
        }
    }

    #[test]
    fn the_row_is_laid_out_field_by_field() {
        let fields = [f("ACTOR-ID", "9(5)"), f("ACTOR-NAME", "X(12)"), f("SALARY", "S9(5)V99")];
        let bytes = image(
            &fields,
            2,
            "EDIT",
            &["42".into(), "Ann Smith".into(), "-123.45".into()],
            false,
        );
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            format!("000000002{:<30}00042{:<12}001234N", "EDIT", "Ann Smith")
        );
    }

    #[test]
    fn numbers_take_their_pictures_shape() {
        assert_eq!(cell("1234.5", "9(5)V99", false), b"0123450");
        assert_eq!(cell("1234567", "9(3)", false), b"567", "a MOVE cuts the high-order digits");
        assert_eq!(cell("-7", "9(3)", false), b"007", "an unsigned picture holds no sign");
        assert_eq!(cell("-0", "S9(3)", false), b"000", "minus zero is zero");
        assert_eq!(cell("12,5", "9(2)V9", true), b"125", "DECIMAL-POINT IS COMMA");
        assert_eq!(cell("abc", "9(3)", false), b"000", "no digits is zero");
        assert_eq!(cell("1.234,50", "9(5)V99", true), b"0123450", "grouping is ignored");
    }

    #[test]
    fn text_is_cut_and_padded_on_a_character_boundary() {
        assert_eq!(cell("Ana", "X(5)", false), b"Ana  ");
        // "ção" is 5 bytes (ç and ã take two each): X(4) holds "çã" exactly, and X(3)
        // cannot hold half of "ã", so it keeps "ç" and pads.
        assert_eq!(cell("ção", "X(4)", false), "çã".as_bytes());
        assert_eq!(cell("ção", "X(3)", false), "ç ".as_bytes());
    }
}
