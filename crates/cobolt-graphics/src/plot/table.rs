// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Reading a COBOL table as one block (WASM-018 R12, WASM-017 R11).
//!
//! "A COBOL table passed as one block: the count, the stride and the numeric
//! kind are described once and the values are read in one pass." A COBOL `OCCURS`
//! table of groups is a run of equal rows in memory. A [`TableSpec`] says how
//! many rows (`count`), how far apart they are (`stride`, bytes), how each
//! number is stored ([`NumKind`], exactly the `USAGE` clauses of the data
//! layout) and where the numbers sit inside a row (`columns`, byte offsets).
//! [`read_table`] then decodes the whole block into `f64` columns in one loop - no
//! call, no copy, no text per value. A hundred thousand values take a few
//! milliseconds (see `tests/plot.rs`).

use crate::error::{GraphicsError, Result};
use crate::limits::Limits;

/// How the sign of a `DISPLAY` number is stored.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ZonedSign {
    /// No sign: the digits are the magnitude.
    Unsigned,
    /// Overpunched on the last digit (`{`, `A`-`I` positive; `}`, `J`-`R` negative).
    TrailingOverpunch,
    /// Overpunched on the first digit.
    LeadingOverpunch,
    /// A separate `+` or `-` character after the digits.
    TrailingSeparate,
    /// A separate `+` or `-` character before the digits.
    LeadingSeparate,
}

/// How a number is stored in memory: the `USAGE` of the COBOL item.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NumKind {
    /// `COMP` / `COMP-5` binary integer of 1, 2, 4 or 8 bytes; `scale` implied decimals.
    Binary {
        /// Bytes: 1, 2, 4 or 8.
        bytes: u8,
        /// Whether it is signed.
        signed: bool,
        /// `COMP` is big-endian; `COMP-5` follows the machine (little-endian here).
        big_endian: bool,
        /// Digits after the implied decimal point (`PIC 9(5)V99` is 2).
        scale: u8,
    },
    /// `COMP-3` packed decimal: two digits per byte, the sign in the last nibble.
    Packed {
        /// Digits in the picture (the item takes `digits / 2 + 1` bytes).
        digits: u8,
        /// Implied decimals.
        scale: u8,
        /// Whether the picture has an `S`.
        signed: bool,
    },
    /// `DISPLAY` numeric: one character per digit.
    Zoned {
        /// Digits in the picture.
        digits: u8,
        /// Implied decimals.
        scale: u8,
        /// Where the sign is.
        sign: ZonedSign,
    },
    /// `COMP-1`: IEEE single precision.
    F32 {
        /// Byte order.
        big_endian: bool,
    },
    /// `COMP-2`: IEEE double precision.
    F64 {
        /// Byte order.
        big_endian: bool,
    },
}

impl NumKind {
    /// How many bytes one number takes.
    pub fn size(&self) -> usize {
        match *self {
            NumKind::Binary { bytes, .. } => bytes as usize,
            NumKind::Packed { digits, .. } => digits as usize / 2 + 1,
            NumKind::Zoned { digits, sign, .. } => digits as usize + matches!(sign, ZonedSign::TrailingSeparate | ZonedSign::LeadingSeparate) as usize,
            NumKind::F32 { .. } => 4,
            NumKind::F64 { .. } => 8,
        }
    }

    fn valid(&self) -> bool {
        match *self {
            NumKind::Binary { bytes, scale, .. } => matches!(bytes, 1 | 2 | 4 | 8) && scale <= 18,
            NumKind::Packed { digits, scale, .. } | NumKind::Zoned { digits, scale, .. } => (1..=31).contains(&digits) && scale <= digits.max(18),
            _ => true,
        }
    }

    /// Decode one number from exactly [`NumKind::size`] bytes. `None` for bytes that
    /// are not a number of this kind (a bad digit, a bad sign nibble, NaN).
    pub fn decode(&self, b: &[u8]) -> Option<f64> {
        if b.len() < self.size() {
            return None;
        }
        let scaled = |mant: f64, scale: u8| mant / pow10(scale);
        match *self {
            NumKind::Binary { bytes, signed, big_endian, scale } => {
                let n = bytes as usize;
                let mut v: u64 = 0;
                for i in 0..n {
                    let byte = if big_endian { b[i] } else { b[n - 1 - i] };
                    v = (v << 8) | byte as u64;
                }
                let mant = if signed {
                    let shift = 64 - 8 * n as u32;
                    (((v << shift) as i64) >> shift) as f64
                } else {
                    v as f64
                };
                Some(scaled(mant, scale))
            }
            NumKind::Packed { digits, scale, signed } => {
                let n = digits as usize / 2 + 1;
                let mut mant = 0f64;
                let mut nibbles = Vec::with_capacity(n * 2);
                for byte in &b[..n] {
                    nibbles.push(byte >> 4);
                    nibbles.push(byte & 0x0F);
                }
                let sign_nibble = nibbles[nibbles.len() - 1];
                let skip = nibbles.len() - 1 - digits as usize; // pad nibble when the digit count is even
                for d in &nibbles[skip..nibbles.len() - 1] {
                    if *d > 9 {
                        return None;
                    }
                    mant = mant * 10.0 + *d as f64;
                }
                let negative = match sign_nibble {
                    0xD | 0xB => true,
                    0xC | 0xA | 0xE | 0xF => false,
                    _ => return None,
                };
                if negative && !signed {
                    return None;
                }
                Some(scaled(if negative { -mant } else { mant }, scale))
            }
            NumKind::Zoned { digits, scale, sign } => {
                let d = digits as usize;
                let (neg, text): (bool, &[u8]) = match sign {
                    ZonedSign::Unsigned => (false, &b[..d]),
                    ZonedSign::TrailingOverpunch | ZonedSign::LeadingOverpunch => (false, &b[..d]),
                    ZonedSign::TrailingSeparate => (b[d] == b'-', &b[..d]),
                    ZonedSign::LeadingSeparate => (b[0] == b'-', &b[1..=d]),
                };
                if matches!(sign, ZonedSign::TrailingSeparate | ZonedSign::LeadingSeparate) {
                    let sg = if sign == ZonedSign::TrailingSeparate { b[d] } else { b[0] };
                    if sg != b'+' && sg != b'-' {
                        return None;
                    }
                }
                let mut mant = 0f64;
                let mut negative = neg;
                for (i, ch) in text.iter().enumerate() {
                    let digit = match *ch {
                        b'0'..=b'9' => *ch - b'0',
                        _ => {
                            // An overpunched digit may only be the one that carries the sign.
                            let carries = match sign {
                                ZonedSign::TrailingOverpunch => i == d - 1,
                                ZonedSign::LeadingOverpunch => i == 0,
                                _ => false,
                            };
                            if !carries {
                                return None;
                            }
                            let (dg, ng) = de_overpunch(*ch)?;
                            negative = ng;
                            dg
                        }
                    };
                    mant = mant * 10.0 + digit as f64;
                }
                Some(scaled(if negative { -mant } else { mant }, scale))
            }
            NumKind::F32 { big_endian } => {
                let a = [b[0], b[1], b[2], b[3]];
                let v = if big_endian { f32::from_be_bytes(a) } else { f32::from_le_bytes(a) } as f64;
                v.is_finite().then_some(v)
            }
            NumKind::F64 { big_endian } => {
                let a = [b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]];
                let v = if big_endian { f64::from_be_bytes(a) } else { f64::from_le_bytes(a) };
                v.is_finite().then_some(v)
            }
        }
    }
}

fn pow10(n: u8) -> f64 {
    const T: [f64; 19] = [1.0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16, 1e17, 1e18];
    T.get(n as usize).copied().unwrap_or(1e18)
}

/// The digit and the sign an ASCII overpunched byte carries (the convention of the
/// RustCOBOL runtime: `{`, `A`-`I` positive; `}`, `J`-`R` negative).
fn de_overpunch(b: u8) -> Option<(u8, bool)> {
    match b {
        b'{' => Some((0, false)),
        b'A'..=b'I' => Some((b - b'A' + 1, false)),
        b'}' => Some((0, true)),
        b'J'..=b'R' => Some((b - b'J' + 1, true)),
        _ => None,
    }
}

/// One numeric field of a table row: where it is and how it is stored.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Column {
    /// Byte offset of the field within a row.
    pub offset: usize,
    /// How the number is stored (the `USAGE` and `PICTURE` of the item).
    pub kind: NumKind,
}

/// Where the numbers are in a block of equal rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableSpec {
    /// Number of rows.
    pub count: usize,
    /// Bytes from the start of one row to the start of the next.
    pub stride: usize,
    /// The fields to read, in the order the series wants them.
    pub columns: Vec<Column>,
}

impl TableSpec {
    /// A table whose fields all have the same kind (an array of `COMP-2`, say), at
    /// the given offsets.
    pub fn uniform(count: usize, stride: usize, kind: NumKind, offsets: &[usize]) -> Self {
        Self { count, stride, columns: offsets.iter().map(|&offset| Column { offset, kind }).collect() }
    }
}

/// The decoded block: `columns[c][row]`. A value that could not be decoded (a bad
/// digit, a NaN) is `NaN`; the plot skips those points, and `bad` counts them.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    /// One vector per requested column.
    pub columns: Vec<Vec<f64>>,
    /// How many values could not be decoded.
    pub bad: usize,
}

/// Decode a whole table in one pass.
pub fn read_table(block: &[u8], spec: &TableSpec, limits: &Limits) -> Result<Table> {
    if spec.columns.is_empty() || spec.columns.len() > 8 {
        return Err(GraphicsError::invalid("a table has from 1 to 8 columns"));
    }
    if spec.columns.iter().any(|c| !c.kind.valid()) {
        return Err(GraphicsError::invalid("the numeric kind is not valid (bytes must be 1, 2, 4 or 8; digits 1 to 31)"));
    }
    limits.check_series_points(spec.count as u64)?;
    if spec.count == 0 {
        return Ok(Table { columns: vec![Vec::new(); spec.columns.len()], bad: 0 });
    }
    let last_end = spec.columns.iter().map(|c| c.offset.saturating_add(c.kind.size())).max().unwrap_or(0);
    if spec.stride < last_end && spec.count > 1 {
        return Err(GraphicsError::invalid(format!("the stride ({}) is shorter than the row ({last_end} bytes): rows would overlap", spec.stride)));
    }
    let needed = (spec.count - 1)
        .checked_mul(spec.stride)
        .and_then(|v| v.checked_add(last_end))
        .ok_or_else(|| GraphicsError::invalid("the table is too large"))?;
    if block.len() < needed {
        return Err(GraphicsError::invalid(format!("the block holds {} byte(s) but {} rows of stride {} need {needed}", block.len(), spec.count, spec.stride)));
    }
    let mut columns: Vec<Vec<f64>> = spec.columns.iter().map(|_| Vec::with_capacity(spec.count)).collect();
    let mut bad = 0;
    let sizes: Vec<usize> = spec.columns.iter().map(|c| c.kind.size()).collect();
    for row in 0..spec.count {
        let base = row * spec.stride;
        for (c, col) in spec.columns.iter().enumerate() {
            let at = base + col.offset;
            match col.kind.decode(&block[at..at + sizes[c]]) {
                Some(v) => columns[c].push(v),
                None => {
                    bad += 1;
                    columns[c].push(f64::NAN);
                }
            }
        }
    }
    Ok(Table { columns, bad })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_integers_in_both_byte_orders_and_signs() {
        let be = NumKind::Binary { bytes: 4, signed: true, big_endian: true, scale: 0 };
        assert_eq!(be.decode(&[0, 0, 1, 0]), Some(256.0));
        assert_eq!(be.decode(&[0xFF, 0xFF, 0xFF, 0xFE]), Some(-2.0));
        let le = NumKind::Binary { bytes: 2, signed: false, big_endian: false, scale: 2 };
        assert_eq!(le.decode(&[0x39, 0x30]), Some(123.45));
        let b8 = NumKind::Binary { bytes: 1, signed: true, big_endian: true, scale: 0 };
        assert_eq!(b8.decode(&[0x80]), Some(-128.0));
        let b64 = NumKind::Binary { bytes: 8, signed: true, big_endian: true, scale: 0 };
        assert_eq!(b64.decode(&(-5i64).to_be_bytes()), Some(-5.0));
    }

    #[test]
    fn packed_decimal_comp3() {
        let k = NumKind::Packed { digits: 5, scale: 2, signed: true };
        assert_eq!(k.size(), 3);
        assert_eq!(k.decode(&[0x12, 0x34, 0x5C]), Some(123.45));
        assert_eq!(k.decode(&[0x12, 0x34, 0x5D]), Some(-123.45));
        assert_eq!(k.decode(&[0x00, 0x00, 0x1F]), Some(0.01));
        assert_eq!(k.decode(&[0x1A, 0x34, 0x5C]), None, "a digit nibble above 9");
        assert_eq!(k.decode(&[0x12, 0x34, 0x50]), None, "a bad sign nibble");
        // An even digit count carries a pad nibble at the front.
        let even = NumKind::Packed { digits: 4, scale: 0, signed: true };
        assert_eq!(even.size(), 3);
        assert_eq!(even.decode(&[0x01, 0x23, 0x4C]), Some(1234.0));
        let un = NumKind::Packed { digits: 3, scale: 0, signed: false };
        assert_eq!(un.decode(&[0x12, 0x3F]), Some(123.0));
        assert_eq!(un.decode(&[0x12, 0x3D]), None, "a negative sign in an unsigned item");
    }

    #[test]
    fn zoned_display_numbers_with_every_sign_style() {
        let u = NumKind::Zoned { digits: 5, scale: 2, sign: ZonedSign::Unsigned };
        assert_eq!(u.decode(b"12345"), Some(123.45));
        assert_eq!(u.decode(b"12a45"), None);
        let t = NumKind::Zoned { digits: 3, scale: 0, sign: ZonedSign::TrailingOverpunch };
        assert_eq!(t.decode(b"12C"), Some(123.0));
        assert_eq!(t.decode(b"12L"), Some(-123.0), "L is -3");
        assert_eq!(t.decode(b"120"), Some(120.0));
        assert_eq!(t.decode(b"12}"), Some(-120.0));
        assert_eq!(t.decode(b"1B3"), None, "the sign belongs on the last digit only");
        let l = NumKind::Zoned { digits: 3, scale: 0, sign: ZonedSign::LeadingOverpunch };
        assert_eq!(l.decode(b"J45"), Some(-145.0));
        let ts = NumKind::Zoned { digits: 3, scale: 1, sign: ZonedSign::TrailingSeparate };
        assert_eq!(ts.size(), 4);
        assert_eq!(ts.decode(b"123-"), Some(-12.3));
        assert_eq!(ts.decode(b"123+"), Some(12.3));
        assert_eq!(ts.decode(b"123 "), None);
        let ls = NumKind::Zoned { digits: 3, scale: 0, sign: ZonedSign::LeadingSeparate };
        assert_eq!(ls.decode(b"-045"), Some(-45.0));
    }

    #[test]
    fn floats() {
        let k = NumKind::F64 { big_endian: false };
        assert_eq!(k.decode(&2.5f64.to_le_bytes()), Some(2.5));
        assert_eq!(k.decode(&f64::NAN.to_le_bytes()), None);
        assert_eq!(k.decode(&f64::INFINITY.to_le_bytes()), None);
        let k = NumKind::F32 { big_endian: true };
        assert_eq!(k.decode(&1.5f32.to_be_bytes()), Some(1.5));
    }

    #[test]
    fn a_record_with_fields_of_different_usages_is_read_in_one_pass() {
        // Rows of (x: PIC 9(3) DISPLAY, y: PIC S9(5)V99 COMP-3) = 3 + 4 bytes, one byte of filler: stride 8.
        let mut block = Vec::new();
        for i in 0..5u32 {
            block.extend_from_slice(format!("{:03}", i * 10).as_bytes());
            let v = (i * 111 + 7) as u64;
            let digits = format!("{:07}", v * 100 + 25);
            let d: Vec<u8> = digits.bytes().map(|c| c - b'0').collect();
            block.extend_from_slice(&[d[0] << 4 | d[1], d[2] << 4 | d[3], d[4] << 4 | d[5], d[6] << 4 | 0xC]);
            block.push(b' ');
        }
        let spec = TableSpec {
            count: 5,
            stride: 8,
            columns: vec![
                Column { offset: 0, kind: NumKind::Zoned { digits: 3, scale: 0, sign: ZonedSign::Unsigned } },
                Column { offset: 3, kind: NumKind::Packed { digits: 7, scale: 2, signed: true } },
            ],
        };
        let t = read_table(&block, &spec, &Limits::default()).unwrap();
        assert_eq!(t.columns[0], vec![0.0, 10.0, 20.0, 30.0, 40.0]);
        assert_eq!(t.columns[1], vec![7.25, 118.25, 229.25, 340.25, 451.25]);
        assert_eq!(t.bad, 0);
    }

    #[test]
    fn bad_values_become_nan_and_are_counted() {
        let spec = TableSpec::uniform(3, 2, NumKind::Zoned { digits: 2, scale: 0, sign: ZonedSign::Unsigned }, &[0]);
        let t = read_table(b"1234x6"[..6].as_ref(), &spec, &Limits::default()).unwrap();
        assert_eq!(t.columns[0][0], 12.0);
        assert_eq!(t.columns[0][1], 34.0);
        assert!(t.columns[0][2].is_nan());
        assert_eq!(t.bad, 1);
    }

    #[test]
    fn nonsense_specs_are_refused_without_reading_out_of_bounds() {
        let k = NumKind::Binary { bytes: 4, signed: true, big_endian: true, scale: 0 };
        let lim = Limits::default();
        let ok = |count, stride, cols: Vec<usize>, len| read_table(&vec![0u8; len], &TableSpec::uniform(count, stride, k, &cols), &lim);
        assert!(ok(4, 4, vec![0], 16).is_ok());
        assert!(ok(4, 4, vec![0], 15).is_err(), "block too short");
        assert!(ok(4, 3, vec![0], 100).is_err(), "rows overlap");
        assert!(ok(2, 8, vec![4], 100).is_ok(), "a column may sit anywhere inside the stride");
        assert!(ok(2, 4, vec![2], 100).is_err(), "the column runs past the row");
        assert!(ok(1, 0, vec![0], 4).is_ok(), "a single row needs no stride");
        assert!(ok(3, 4, vec![], 100).is_err());
        assert!(ok(usize::MAX / 2, usize::MAX / 2, vec![0], 100).is_err(), "no overflow");
        let lim = Limits { max_series_points: 10, ..Limits::default() };
        let e = read_table(&[0; 400], &TableSpec::uniform(100, 4, k, &[0]), &lim).unwrap_err();
        assert!(e.to_string().contains("max_series_points"), "{e}");
        let bad = NumKind::Binary { bytes: 3, signed: true, big_endian: true, scale: 0 };
        assert!(read_table(&[0; 12], &TableSpec::uniform(4, 3, bad, &[0]), &Limits::default()).is_err());
    }
}
