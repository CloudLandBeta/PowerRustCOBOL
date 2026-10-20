// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Field descriptors: what the generator tells the routines about one item.
//!
//! A descriptor is 12 bytes, little-endian, laid out as
//!
//! | bytes | field | meaning |
//! |---|---|---|
//! | 0 | `kind` | one of the `K_*` constants |
//! | 1 | `flags` | a mask of the `F_*` constants |
//! | 2 | `digits` | numeric: digit positions *stored*; otherwise 0 |
//! | 3 | `scale` | numeric: digits right of the implied point (`i8`; negative = trailing `P`s, so the value is `stored × 10^-scale`) |
//! | 4..8 | `len` | bytes the item occupies |
//! | 8..12 | `aux` | edited kinds: address of the picture template; otherwise 0 |
//!
//! `digits` counts only the positions that occupy storage: `PIC 99PP` has
//! `digits = 2`, `scale = -2`; `PIC PP99` has `digits = 2`, `scale = 4`. This is
//! the only place those facts are written down on the module side; the
//! generator derives them from the `PICTURE`.

pub const FD_SIZE: usize = 12;

/// `PIC X`, `PIC A` and anything handled byte for byte.
pub const K_ALNUM: u8 = 0;
/// `USAGE DISPLAY` numeric: one character per digit, sign embedded or separate.
pub const K_NUM_DISPLAY: u8 = 1;
/// `COMP`, `COMP-4`, `BINARY`: two's-complement integer, **big-endian**.
pub const K_BINARY: u8 = 2;
/// `COMP-5`: two's-complement integer in the machine's order (little-endian).
pub const K_COMP5: u8 = 3;
/// `COMP-3`, `PACKED-DECIMAL`: binary-coded decimal, sign in the last nibble.
pub const K_PACKED: u8 = 4;
/// `COMP-1`: IEEE single.
pub const K_FLOAT32: u8 = 5;
/// `COMP-2`: IEEE double.
pub const K_FLOAT64: u8 = 6;
/// Numeric-edited `PICTURE`.
pub const K_NUM_EDITED: u8 = 7;
/// Alphanumeric-edited `PICTURE`.
pub const K_ALNUM_EDITED: u8 = 8;
/// `PIC N`, `USAGE NATIONAL`: UTF-16 code units.
pub const K_NATIONAL: u8 = 9;
/// A group item: alphanumeric as far as moves and comparisons go.
pub const K_GROUP: u8 = 10;
/// `USAGE INDEX`: a 32-bit unsigned occurrence displacement/number.
pub const K_INDEX: u8 = 11;

/// The item carries an operational sign (`PIC S…`).
pub const F_SIGNED: u8 = 1;
/// `SIGN LEADING` (default trailing).
pub const F_SIGN_LEADING: u8 = 2;
/// `SIGN … SEPARATE CHARACTER`.
pub const F_SIGN_SEPARATE: u8 = 4;
/// `JUSTIFIED RIGHT`.
pub const F_JUSTIFIED: u8 = 8;
/// `BLANK WHEN ZERO`.
pub const F_BLANK_ZERO: u8 = 16;
/// `PIC A…` (class `ALPHABETIC` applies).
pub const F_ALPHABETIC: u8 = 32;

/// A parsed field descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fd {
    pub kind: u8,
    pub flags: u8,
    pub digits: u8,
    pub scale: i8,
    pub len: u32,
    pub aux: u32,
}

impl Fd {
    /// Parse 12 bytes.
    pub fn parse(b: &[u8]) -> Fd {
        Fd {
            kind: b[0],
            flags: b[1],
            digits: b[2],
            scale: b[3] as i8,
            len: u32::from_le_bytes([b[4], b[5], b[6], b[7]]),
            aux: u32::from_le_bytes([b[8], b[9], b[10], b[11]]),
        }
    }

    /// Serialise (used by the native tests and by the generator's twin of this
    /// layout).
    pub fn to_bytes(&self) -> [u8; FD_SIZE] {
        let mut o = [0u8; FD_SIZE];
        o[0] = self.kind;
        o[1] = self.flags;
        o[2] = self.digits;
        o[3] = self.scale as u8;
        o[4..8].copy_from_slice(&self.len.to_le_bytes());
        o[8..12].copy_from_slice(&self.aux.to_le_bytes());
        o
    }

    pub fn signed(&self) -> bool {
        self.flags & F_SIGNED != 0
    }
    pub fn sign_leading(&self) -> bool {
        self.flags & F_SIGN_LEADING != 0
    }
    pub fn sign_separate(&self) -> bool {
        self.flags & F_SIGN_SEPARATE != 0
    }
    pub fn justified(&self) -> bool {
        self.flags & F_JUSTIFIED != 0
    }
    pub fn blank_when_zero(&self) -> bool {
        self.flags & F_BLANK_ZERO != 0
    }

    /// A category in which the item holds a number it can do arithmetic with
    /// (edited pictures hold characters; they de-edit on the way in).
    pub fn is_numeric(&self) -> bool {
        matches!(
            self.kind,
            K_NUM_DISPLAY | K_BINARY | K_COMP5 | K_PACKED | K_FLOAT32 | K_FLOAT64 | K_INDEX
        )
    }

    pub fn is_float(&self) -> bool {
        matches!(self.kind, K_FLOAT32 | K_FLOAT64)
    }

    /// Alphanumeric-like for moves and comparisons: holds characters.
    pub fn is_alnum_like(&self) -> bool {
        matches!(self.kind, K_ALNUM | K_GROUP | K_ALNUM_EDITED | K_NATIONAL)
    }
}

impl Fd {
    /// `PIC X(len)` and friends.
    pub fn alnum(len: u32) -> Fd {
        Fd { kind: K_ALNUM, flags: 0, digits: 0, scale: 0, len, aux: 0 }
    }

    /// A group of `len` bytes.
    pub fn group(len: u32) -> Fd {
        Fd { kind: K_GROUP, flags: 0, digits: 0, scale: 0, len, aux: 0 }
    }

    /// `USAGE DISPLAY` numeric with `digits` stored digits, `scale` decimals and
    /// the sign flags in `flags` (`F_SIGNED`, `F_SIGN_LEADING`, `F_SIGN_SEPARATE`).
    pub fn display(digits: u8, scale: i8, flags: u8) -> Fd {
        let sep = flags & F_SIGN_SEPARATE != 0 && flags & F_SIGNED != 0;
        Fd { kind: K_NUM_DISPLAY, flags, digits, scale, len: digits as u32 + sep as u32, aux: 0 }
    }

    /// `COMP-3` with `digits` digits.
    pub fn packed(digits: u8, scale: i8, flags: u8) -> Fd {
        Fd { kind: K_PACKED, flags, digits, scale, len: digits as u32 / 2 + 1, aux: 0 }
    }

    /// `COMP` (big-endian) of `len` bytes.
    pub fn binary(digits: u8, scale: i8, flags: u8, len: u32) -> Fd {
        Fd { kind: K_BINARY, flags, digits, scale, len, aux: 0 }
    }
}
