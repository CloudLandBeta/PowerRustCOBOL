// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! PIC-aware display formatting and validation for grid browser cells (AC7/AC8).

use crate::model::{FieldUsage, IndexedField};

/// Parsed subset of COBOL PIC clauses used in `.cidx` definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PicCategory {
    Alphanumeric,
    Alphabetic,
    Numeric,
    /// Single-byte `PIC 9` / `PIC 9(1)` used as an indicator.
    Indicator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPic {
    pub category: PicCategory,
    pub width: usize,
    pub signed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldEncodeError {
    InvalidCharacters,
    NumericOverflow,
    InvalidIndicator,
    WrongLength,
    Empty,
}

impl FieldEncodeError {
    pub fn as_str(self) -> &'static str {
        match self {
            FieldEncodeError::InvalidCharacters => "invalid characters for PIC",
            FieldEncodeError::NumericOverflow => "value too large for PIC",
            FieldEncodeError::InvalidIndicator => "indicator must be 0 or 1",
            FieldEncodeError::WrongLength => "value length exceeds PIC",
            FieldEncodeError::Empty => "value required",
        }
    }
}

/// Parse common PIC templates from `.cidx` (`X(20)`, `9(5)`, `9`, `A(10)`, `S9(4)`…).
pub fn parse_pic(pic: &str) -> ParsedPic {
    // Strip any custom-entry marker (zero-width) before parsing for display/encoding.
    let upper = pic
        .trim_start_matches('\u{200B}')
        .trim()
        .to_ascii_uppercase();
    let signed = upper.starts_with('S');
    let body = if signed { &upper[1..] } else { &upper[..] };

    // Compute total width by parsing body
    let mut width = 0;
    let mut category = PicCategory::Alphanumeric;

    let chars: Vec<char> = body.chars().collect();
    let mut i = 0;
    let mut has_numeric = false;
    let mut has_alphabetic = false;
    let mut has_alphanumeric = false;

    while i < chars.len() {
        let c = chars[i];
        match c {
            '9' => {
                has_numeric = true;
                let (count, next_i) = parse_count(&chars, i + 1);
                width += count;
                i = next_i;
            }
            'X' => {
                has_alphanumeric = true;
                let (count, next_i) = parse_count(&chars, i + 1);
                width += count;
                i = next_i;
            }
            'A' => {
                has_alphabetic = true;
                let (count, next_i) = parse_count(&chars, i + 1);
                width += count;
                i = next_i;
            }
            'V' => {
                // V is implicit decimal point, it consumes 0 bytes of disk space
                i += 1;
            }
            '.' | ',' | '$' | '+' | '-' | 'Z' | 'C' | 'D' | 'R' => {
                // editing/insertion characters
                width += 1;
                i += 1;
            }
            _ => {
                // fallback: count it as 1 character
                width += 1;
                i += 1;
            }
        }
    }

    // Determine category
    if has_alphanumeric {
        category = PicCategory::Alphanumeric;
    } else if has_alphabetic {
        category = PicCategory::Alphabetic;
    } else if has_numeric {
        if width == 1 {
            category = PicCategory::Indicator;
        } else {
            category = PicCategory::Numeric;
        }
    }

    ParsedPic {
        category,
        width,
        signed,
    }
}

fn parse_count(chars: &[char], mut i: usize) -> (usize, usize) {
    if i < chars.len() && chars[i] == '(' {
        i += 1;
        let start = i;
        while i < chars.len() && chars[i] != ')' {
            i += 1;
        }
        let end = i;
        if i < chars.len() && chars[i] == ')' {
            i += 1;
        }
        let inner_str: String = chars[start..end].iter().collect();
        if let Ok(count) = inner_str.trim().parse::<usize>() {
            return (count, i);
        }
    }
    (1, i)
}

/// Format raw DISPLAY bytes for a grid cell (AC7).
pub fn format_field_display(field: &IndexedField, bytes: &[u8]) -> String {
    if field.usage != FieldUsage::Display {
        return format_comp_display(bytes);
    }
    let pic = parse_pic(&field.pic);
    let slice = fit_slice(
        bytes,
        field.length.map(|l| l as usize).unwrap_or(bytes.len()),
    );
    match pic.category {
        PicCategory::Alphanumeric | PicCategory::Alphabetic => {
            String::from_utf8_lossy(slice).trim_end().to_string()
        }
        PicCategory::Numeric => format_numeric_display(slice, pic.width, pic.signed),
        PicCategory::Indicator => format_indicator_display(slice),
    }
}

/// Encode user input into DISPLAY bytes; returns exactly `len` bytes (AC8 validation).
pub fn encode_field_display(
    field: &IndexedField,
    input: &str,
    len: usize,
) -> Result<Vec<u8>, FieldEncodeError> {
    if field.usage != FieldUsage::Display {
        return Err(FieldEncodeError::InvalidCharacters);
    }
    let pic = parse_pic(&field.pic);
    let mut out = vec![b' '; len];
    match pic.category {
        PicCategory::Alphanumeric => encode_alphanumeric(input, len, &mut out)?,
        PicCategory::Alphabetic => encode_alphabetic(input, len, &mut out)?,
        PicCategory::Numeric => encode_numeric(input, len, pic.width, pic.signed, &mut out)?,
        PicCategory::Indicator => encode_indicator(input, len, &mut out)?,
    }
    Ok(out)
}

/// Encode from a checkbox widget (`true` → `1`, `false` → `0`).
pub fn encode_indicator_bool(checked: bool, len: usize) -> Vec<u8> {
    let mut out = vec![b' '; len];
    let ch = if checked { b'1' } else { b'0' };
    out[len.saturating_sub(1)] = ch;
    out
}

/// Parse indicator / checkbox bytes to bool.
pub fn indicator_bool(bytes: &[u8]) -> bool {
    bytes.iter().any(|&b| b == b'1')
}

fn fit_slice<'a>(bytes: &'a [u8], len: usize) -> &'a [u8] {
    &bytes[..bytes.len().min(len)]
}

fn format_numeric_display(slice: &[u8], width: usize, signed: bool) -> String {
    let s = String::from_utf8_lossy(slice);
    let trimmed = s.trim_end();
    if trimmed.is_empty() {
        return String::new();
    }
    if signed && trimmed.starts_with('-') {
        let digits: String = trimmed[1..]
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect();
        if digits.is_empty() {
            return String::new();
        }
        return format!("-{digits}");
    }
    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return String::new();
    }
    // Trim leading zeros for readability while preserving at least one digit.
    let mut d = digits.trim_start_matches('0').to_string();
    if d.is_empty() {
        d = "0".into();
    }
    if d.len() > width {
        d = digits;
    }
    d
}

fn format_indicator_display(slice: &[u8]) -> String {
    if indicator_bool(slice) {
        "1".into()
    } else {
        "0".into()
    }
}

fn format_comp_display(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join("")
}

fn encode_alphanumeric(input: &str, len: usize, out: &mut [u8]) -> Result<(), FieldEncodeError> {
    if input.as_bytes().len() > len {
        return Err(FieldEncodeError::WrongLength);
    }
    out.fill(b' ');
    out[..input.len()].copy_from_slice(input.as_bytes());
    Ok(())
}

fn encode_alphabetic(input: &str, len: usize, out: &mut [u8]) -> Result<(), FieldEncodeError> {
    if !input.chars().all(|c| c.is_ascii_alphabetic() || c == ' ') {
        return Err(FieldEncodeError::InvalidCharacters);
    }
    encode_alphanumeric(input, len, out)
}

fn encode_numeric(
    input: &str,
    len: usize,
    width: usize,
    signed: bool,
    out: &mut [u8],
) -> Result<(), FieldEncodeError> {
    let t = input.trim();
    if t.is_empty() {
        return Err(FieldEncodeError::Empty);
    }
    let (neg, digits): (bool, String) = if signed && t.starts_with('-') {
        (
            true,
            t[1..].chars().filter(|c| c.is_ascii_digit()).collect(),
        )
    } else {
        (false, t.chars().filter(|c| c.is_ascii_digit()).collect())
    };
    if digits.is_empty() {
        return Err(FieldEncodeError::InvalidCharacters);
    }
    if digits.len() > width {
        return Err(FieldEncodeError::NumericOverflow);
    }
    let padded = format!("{digits:0>width$}");
    let encoded = if neg { format!("-{padded}") } else { padded };
    if encoded.len() > len {
        return Err(FieldEncodeError::WrongLength);
    }
    out.fill(b' ');
    let start = len.saturating_sub(encoded.len());
    out[start..start + encoded.len()].copy_from_slice(encoded.as_bytes());
    Ok(())
}

fn encode_indicator(input: &str, len: usize, out: &mut [u8]) -> Result<(), FieldEncodeError> {
    let t = input.trim();
    let checked = match t {
        "1" | "Y" | "y" | "YES" | "yes" | "TRUE" | "true" => true,
        "0" | "N" | "n" | "NO" | "no" | "FALSE" | "false" | "" => false,
        _ => return Err(FieldEncodeError::InvalidIndicator),
    };
    let encoded = encode_indicator_bool(checked, len);
    out.copy_from_slice(&encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FieldUsage;

    fn field(pic: &str, len: u32) -> IndexedField {
        IndexedField {
            level: 5,
            name: "F".into(),
            pic: pic.into(),
            usage: FieldUsage::Display,
            offset: Some(0),
            length: Some(len),
            comment: String::new(),
            grid_control: None,
            occurs: None,
            redefines: None,
            synchronized: false,
            children: Vec::new(),
        }
    }

    #[test]
    fn format_numeric_alpha_indicator() {
        let n = field("9(5)", 5);
        assert_eq!(format_field_display(&n, b"00123"), "123");
        let x = field("X(20)", 20);
        assert_eq!(format_field_display(&x, b"HELLO               "), "HELLO");
        let ind = field("9", 1);
        assert_eq!(format_field_display(&ind, b"1"), "1");
        assert_eq!(format_field_display(&ind, b"0"), "0");
    }

    #[test]
    fn encode_rejects_bad_numeric_and_indicator() {
        let n = field("9(5)", 5);
        assert!(encode_field_display(&n, "abcde", 5).is_err());
        assert!(encode_field_display(&n, "123456", 5).is_err());
        assert!(encode_field_display(&n, "123", 5).is_ok());
        let ind = field("9", 1);
        assert!(encode_field_display(&ind, "2", 1).is_err());
        let enc = encode_indicator_bool(true, 1);
        assert_eq!(enc, vec![b'1']);
    }
}
