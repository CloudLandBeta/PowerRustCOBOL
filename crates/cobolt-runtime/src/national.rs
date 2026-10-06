// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! National (`PIC N`) and UTF-8 (`PIC U`) character data — spec 077.
//!
//! A national or UTF-8 item's slot holds its **text**: exactly the characters
//! the item holds, padded with spaces. Its **byte image** — UTF-16 big-endian
//! for national, UTF-8 padded with X'20' for UTF-8 — exists only where bytes
//! are visible (a group's image, REDEFINES, a record, `BYTE-LENGTH`). Every
//! write goes through [`fit_class`] and every image through [`class_image`] and
//! back through [`class_text`], so the three can never disagree.
//!
//! A national position is one UTF-16 code unit, so a character outside the
//! Basic Multilingual Plane takes two of them, as IBM documents. A UTF-8
//! position (`PIC U(n)`) is one character, whatever its encoded length.

/// How a national or UTF-8 item holds its characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharClass {
    /// `PIC N(n)`: n UTF-16 code units, an image of 2·n bytes.
    National { chars: usize },
    /// `PIC U(n)`: n characters, an image of 4·n bytes.
    Utf8 { chars: usize },
    /// `PIC U BYTE-LENGTH n`: the whole characters that fit n bytes.
    Utf8Bytes { bytes: usize },
}

impl CharClass {
    /// The item's size in bytes — what `BYTE-LENGTH` returns and what it
    /// occupies in a group or a record.
    pub fn width(self) -> usize {
        match self {
            CharClass::National { chars } => 2 * chars,
            CharClass::Utf8 { chars } => 4 * chars,
            CharClass::Utf8Bytes { bytes } => bytes,
        }
    }

    /// The item's size in character positions — what `LENGTH` returns.
    /// A `BYTE-LENGTH` item has no fixed character count; its positions are
    /// its bytes.
    pub fn positions(self) -> usize {
        match self {
            CharClass::National { chars } | CharClass::Utf8 { chars } => chars,
            CharClass::Utf8Bytes { bytes } => bytes,
        }
    }

    pub fn is_national(self) -> bool {
        matches!(self, CharClass::National { .. })
    }

    /// The text used as `HIGH-VALUE` in this class: U+FFFF for national,
    /// U+10FFFF (IBM's UX'F48FBFBF') for UTF-8.
    pub fn high_value(self) -> char {
        if self.is_national() { '\u{FFFF}' } else { '\u{10FFFF}' }
    }
}

/// How many positions one character takes in `class`.
fn cost(class: CharClass, c: char) -> usize {
    match class {
        CharClass::National { .. } => c.len_utf16(),
        CharClass::Utf8 { .. } => 1,
        CharClass::Utf8Bytes { .. } => c.len_utf8(),
    }
}

/// The capacity of `class` in the units [`cost`] counts.
fn capacity(class: CharClass) -> usize {
    match class {
        CharClass::National { chars } | CharClass::Utf8 { chars } => chars,
        CharClass::Utf8Bytes { bytes } => bytes,
    }
}

/// Fit `text` into `class`: truncate on a whole character, pad with spaces.
///
/// `justified` (JUSTIFIED RIGHT) keeps the rightmost characters of the text's
/// significant part and pads on the left. A character that would only half
/// fit — a supplementary character in the last national position, a
/// three-byte character in the last two bytes — is dropped, and the position
/// it would have taken is a space.
pub fn fit_class(class: CharClass, text: &str, justified: bool) -> String {
    let cap = capacity(class);
    if justified {
        let significant = text.trim_end_matches(' ');
        let mut used = 0usize;
        let mut kept: Vec<char> = Vec::new();
        for c in significant.chars().rev() {
            let w = cost(class, c);
            if used + w > cap {
                break;
            }
            used += w;
            kept.push(c);
        }
        let mut out = " ".repeat(cap - used);
        out.extend(kept.into_iter().rev());
        return out;
    }
    let mut used = 0usize;
    let mut out = String::with_capacity(text.len().min(4 * cap) + cap);
    for c in text.chars() {
        let w = cost(class, c);
        if used + w > cap {
            break;
        }
        used += w;
        out.push(c);
    }
    out.extend(std::iter::repeat_n(' ', cap - used));
    out
}

/// The item's bytes: UTF-16BE for national, UTF-8 padded with X'20' for UTF-8,
/// always exactly [`CharClass::width`] long.
pub fn class_image(class: CharClass, text: &str) -> Vec<u8> {
    // `fit_class` fills national and BYTE-LENGTH items to their exact width;
    // only `PIC U(n)` has room to spare, padded with X'20'.
    let fitted = fit_class(class, text, false);
    let mut out = match class {
        CharClass::National { .. } => utf16be(&fitted),
        CharClass::Utf8 { .. } | CharClass::Utf8Bytes { .. } => fitted.into_bytes(),
    };
    out.resize(class.width(), b' ');
    out
}

/// The text an image holds — the inverse of [`class_image`]. Bytes that are
/// not valid in the class (an unpaired surrogate, a broken UTF-8 sequence)
/// read as U+FFFD; the result is fitted to the class.
pub fn class_text(class: CharClass, bytes: &[u8]) -> String {
    let text = match class {
        CharClass::National { .. } => from_utf16be(bytes),
        CharClass::Utf8 { .. } | CharClass::Utf8Bytes { .. } => String::from_utf8_lossy(bytes).into_owned(),
    };
    fit_class(class, &text, false)
}

/// The text a function argument's bytes hold in `class`: UTF-16BE for
/// national, UTF-8 otherwise. Unlike [`class_text`] it is not fitted — an
/// argument is as long as it is.
pub fn class_bytes_text(class: CharClass, bytes: &[u8]) -> String {
    if class.is_national() { from_utf16be(bytes) } else { String::from_utf8_lossy(bytes).into_owned() }
}

/// `text` as UTF-16 big-endian bytes.
pub fn utf16be(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

/// UTF-16 big-endian bytes as text; an unpaired surrogate, and an odd final
/// byte, read as U+FFFD.
pub fn from_utf16be(bytes: &[u8]) -> String {
    let units = bytes.chunks(2).map(|c| if c.len() == 2 { u16::from_be_bytes([c[0], c[1]]) } else { 0xFFFD });
    char::decode_utf16(units).map(|r| r.unwrap_or('\u{FFFD}')).collect()
}

/// How many positions `text` takes in `class` — characters for UTF-8,
/// UTF-16 units for national, bytes for a `BYTE-LENGTH` item.
pub fn positions_of(class: CharClass, text: &str) -> usize {
    text.chars().map(|c| cost(class, c)).sum()
}

pub use cobolt_ast::intrinsics::CodePage;

/// Windows-1252 bytes 0x80–0x9F. The five bytes the code page leaves
/// undefined are `None`.
const CP1252_HIGH: [Option<char>; 32] = [
    Some('\u{20AC}'), None, Some('\u{201A}'), Some('\u{0192}'), Some('\u{201E}'), Some('\u{2026}'),
    Some('\u{2020}'), Some('\u{2021}'), Some('\u{02C6}'), Some('\u{2030}'), Some('\u{0160}'),
    Some('\u{2039}'), Some('\u{0152}'), None, Some('\u{017D}'), None, None, Some('\u{2018}'),
    Some('\u{2019}'), Some('\u{201C}'), Some('\u{201D}'), Some('\u{2022}'), Some('\u{2013}'),
    Some('\u{2014}'), Some('\u{02DC}'), Some('\u{2122}'), Some('\u{0161}'), Some('\u{203A}'),
    Some('\u{0153}'), None, Some('\u{017E}'), Some('\u{0178}'),
];

/// Bytes in `cp` as text (`FUNCTION NATIONAL-OF`). As IBM documents, a
/// single-byte character the code page does not define becomes U+001A and a
/// broken multi-byte sequence U+FFFD; nothing stops the program.
pub fn decode(cp: CodePage, bytes: &[u8]) -> String {
    match cp {
        CodePage::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
        CodePage::Iso8859_1 => bytes.iter().map(|&b| b as char).collect(),
        CodePage::Windows1252 => bytes
            .iter()
            .map(|&b| match b {
                0x80..=0x9F => CP1252_HIGH[(b - 0x80) as usize].unwrap_or('\u{1A}'),
                _ => b as char,
            })
            .collect(),
    }
}

/// Text as bytes in `cp` (`FUNCTION DISPLAY-OF`). A character a single-byte
/// code page cannot hold becomes X'7F', IBM's substitution character for
/// ASCII and Windows single-byte code pages.
pub fn encode(cp: CodePage, text: &str) -> Vec<u8> {
    match cp {
        CodePage::Utf8 => text.as_bytes().to_vec(),
        CodePage::Iso8859_1 => text.chars().map(|c| u8::try_from(c as u32).unwrap_or(0x7F)).collect(),
        CodePage::Windows1252 => text
            .chars()
            .map(|c| match c as u32 {
                0x80..=0x9F => 0x7F,
                n @ 0..=0xFF => n as u8,
                _ => CP1252_HIGH
                    .iter()
                    .position(|m| *m == Some(c))
                    .map_or(0x7F, |i| 0x80 + i as u8),
            })
            .collect(),
    }
}

/// What the operator typed, as text, for a national or UTF-8 receiver (spec
/// 077, Q7): UTF-8 when the bytes are UTF-8, Windows-1252 otherwise — the
/// encoding a Windows console without UTF-8 sends.
pub fn typed_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => decode(CodePage::Windows1252, bytes),
    }
}

// ── The U-functions (spec 077, Q6) ───────────────────────────────────────────
//
// Each works on an argument's BYTES, as IBM documents: UTF-8 for an
// alphanumeric or UTF-8 argument, UTF-16 big-endian for a national one.
// Positions are 1-based. UPOS and UWIDTH answer in bytes; USUBSTR counts
// characters; UVALID and USUPPLEMENTARY answer in bytes for UTF-8 and in
// UTF-16 code units for national.

/// One argument of a U-function.
#[derive(Debug, Clone, Copy)]
pub struct UArg<'a> {
    pub bytes: &'a [u8],
    pub national: bool,
}

/// Each character of `arg` as `(byte offset, byte width, code point)`.
/// Ill-formed data is walked a byte (UTF-8) or a unit (UTF-16) at a time, as
/// U+FFFD, so the functions never stop the program.
fn chars_of(arg: UArg) -> Vec<(usize, usize, char)> {
    let mut out = Vec::new();
    if arg.national {
        let units: Vec<u16> = arg.bytes.chunks(2).map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)])).collect();
        let mut i = 0;
        while i < units.len() {
            let u = units[i];
            if (0xD800..0xDC00).contains(&u) && units.get(i + 1).is_some_and(|l| (0xDC00..0xE000).contains(l)) {
                let c = char::decode_utf16([u, units[i + 1]]).next().and_then(Result::ok).unwrap_or('\u{FFFD}');
                out.push((2 * i, 4, c));
                i += 2;
            } else {
                out.push((2 * i, 2, char::from_u32(u as u32).unwrap_or('\u{FFFD}')));
                i += 1;
            }
        }
        return out;
    }
    let mut i = 0;
    let b = arg.bytes;
    while i < b.len() {
        match std::str::from_utf8(&b[i..]) {
            Ok(rest) => {
                for (off, c) in rest.char_indices() {
                    out.push((i + off, c.len_utf8(), c));
                }
                break;
            }
            Err(e) => {
                let good = e.valid_up_to();
                let rest = std::str::from_utf8(&b[i..i + good]).unwrap_or_default();
                for (off, c) in rest.char_indices() {
                    out.push((i + off, c.len_utf8(), c));
                }
                let bad = e.error_len().unwrap_or(b.len() - i - good).max(1);
                out.push((i + good, bad, '\u{FFFD}'));
                i += good + bad;
            }
        }
    }
    out
}

/// `ULENGTH`: how many characters.
pub fn ulength(arg: UArg) -> usize {
    chars_of(arg).len()
}

/// `UPOS(arg, n)`: the byte where the n-th character starts; 0 out of range.
pub fn upos(arg: UArg, n: i64) -> usize {
    nth(arg, n).map_or(0, |(off, _, _)| off + 1)
}

/// `UWIDTH(arg, n)`: the n-th character's width in bytes; 0 out of range.
pub fn uwidth(arg: UArg, n: i64) -> usize {
    nth(arg, n).map_or(0, |(_, w, _)| w)
}

fn nth(arg: UArg, n: i64) -> Option<(usize, usize, char)> {
    if n < 1 {
        return None;
    }
    chars_of(arg).get(n as usize - 1).copied()
}

/// `USUBSTR(arg, start, len)`: `len` characters from the `start`-th. IBM
/// leaves a range past the end undefined; here it is the characters that
/// exist, and an empty result before the first.
pub fn usubstr(arg: UArg, start: i64, len: i64) -> String {
    if start < 1 || len < 1 {
        return String::new();
    }
    chars_of(arg).into_iter().skip(start as usize - 1).take(len as usize).map(|(_, _, c)| c).collect()
}

/// `UVALID`: 0 when the argument is well-formed; otherwise where the first
/// ill-formed sequence starts — its byte for UTF-8 (a sequence cut off at
/// the end is reported at its lead byte), its code unit for national.
pub fn uvalid(arg: UArg) -> usize {
    if arg.national {
        let units: Vec<u16> = arg.bytes.chunks(2).map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)])).collect();
        let mut i = 0;
        while i < units.len() {
            match units[i] {
                0xD800..=0xDBFF if units.get(i + 1).is_some_and(|l| (0xDC00..0xE000).contains(l)) => i += 2,
                0xD800..=0xDFFF => return i + 1,
                _ => i += 1,
            }
        }
        // An odd final byte is half a code unit.
        return if arg.bytes.len() % 2 == 1 { units.len() } else { 0 };
    }
    match std::str::from_utf8(arg.bytes) {
        Ok(_) => 0,
        Err(e) => e.valid_up_to() + 1,
    }
}

/// `USUPPLEMENTARY`: where the first character above U+FFFF starts — its
/// byte for UTF-8, its code unit for national — or 0.
pub fn usupplementary(arg: UArg) -> usize {
    chars_of(arg)
        .into_iter()
        .find(|(_, _, c)| (*c as u32) > 0xFFFF)
        .map_or(0, |(off, _, _)| if arg.national { off / 2 + 1 } else { off + 1 })
}

#[cfg(test)]
mod tests {
    use super::*;

    const N5: CharClass = CharClass::National { chars: 5 };
    const U5: CharClass = CharClass::Utf8 { chars: 5 };
    const B5: CharClass = CharClass::Utf8Bytes { bytes: 5 };

    #[test]
    fn widths_and_positions() {
        assert_eq!((N5.width(), N5.positions()), (10, 5));
        assert_eq!((U5.width(), U5.positions()), (20, 5));
        assert_eq!((B5.width(), B5.positions()), (5, 5));
    }

    #[test]
    fn fit_pads_and_truncates_on_whole_characters() {
        assert_eq!(fit_class(N5, "Ação", false), "Ação ");
        assert_eq!(fit_class(N5, "Ação ok", false), "Ação ");
        assert_eq!(fit_class(U5, "日本語のテキスト", false), "日本語のテ");
        // 'ç' and 'ã' take two bytes each: "Aç" is three, "Açã" five, and
        // the 'o' that follows would make six.
        assert_eq!(fit_class(B5, "Ação", false), "Açã");
        assert_eq!(fit_class(B5, "A€", false), "A€ ");
        // A three-byte character never splits: two bytes left, it is dropped.
        assert_eq!(fit_class(CharClass::Utf8Bytes { bytes: 3 }, "A€", false), "A  ");
    }

    #[test]
    fn a_supplementary_character_takes_two_national_positions() {
        // U+1F600 is a surrogate pair in UTF-16.
        assert_eq!(fit_class(N5, "ab😀cd", false), "ab😀c");
        assert_eq!(positions_of(N5, "ab😀cd"), 6);
        // One position left: the pair does not fit and is not split.
        assert_eq!(fit_class(CharClass::National { chars: 3 }, "ab😀", false), "ab ");
        // A UTF-8 position is one character, whatever its length.
        assert_eq!(fit_class(U5, "ab😀cdef", false), "ab😀cd");
    }

    #[test]
    fn justified_keeps_the_right_end() {
        assert_eq!(fit_class(N5, "Ação", true), " Ação");
        assert_eq!(fit_class(N5, "Coração  ", true), "ração");
        // o, ã, ç: one, two and two bytes — exactly five.
        assert_eq!(fit_class(B5, "Ação", true), "ção");
    }

    #[test]
    fn national_image_is_utf16be_padded_with_national_spaces() {
        let img = class_image(CharClass::National { chars: 3 }, "Aç");
        assert_eq!(img, vec![0x00, 0x41, 0x00, 0xE7, 0x00, 0x20]);
        let pair = class_image(CharClass::National { chars: 2 }, "😀");
        assert_eq!(pair, vec![0xD8, 0x3D, 0xDE, 0x00]);
    }

    #[test]
    fn utf8_images_are_padded_with_x20() {
        assert_eq!(class_image(CharClass::Utf8 { chars: 2 }, "ç"), b"\xC3\xA7      ".to_vec());
        assert_eq!(class_image(B5, "Aç"), b"A\xC3\xA7  ".to_vec());
    }

    #[test]
    fn images_round_trip() {
        for (class, text) in [
            (N5, "Ação "),
            (N5, "日本語😀"),
            (U5, "ab😀cd"),
            (B5, "Açã"),
            (CharClass::National { chars: 1 }, "\u{FFFF}"),
            (CharClass::Utf8 { chars: 1 }, "\u{10FFFF}"),
        ] {
            let fitted = fit_class(class, text, false);
            assert_eq!(class_text(class, &class_image(class, &fitted)), fitted, "{class:?} {text:?}");
            assert_eq!(class_image(class, &fitted).len(), class.width());
        }
    }

    #[test]
    fn invalid_bytes_read_as_replacement_characters() {
        // An unpaired high surrogate, then 'A'.
        assert_eq!(class_text(CharClass::National { chars: 2 }, &[0xD8, 0x00, 0x00, 0x41]), "\u{FFFD}A");
        assert_eq!(class_text(CharClass::Utf8 { chars: 2 }, b"\xC3A"), "\u{FFFD}A");
        // An odd final byte.
        assert_eq!(from_utf16be(&[0x00, 0x41, 0x00]), "A\u{FFFD}");
    }

    #[test]
    fn high_values() {
        assert_eq!(N5.high_value(), '\u{FFFF}');
        assert_eq!(U5.high_value(), '\u{10FFFF}');
    }

    #[test]
    fn code_pages_round_trip_and_substitute() {
        let text = "Ação €";
        for cp in [CodePage::Utf8, CodePage::Windows1252] {
            assert_eq!(decode(cp, &encode(cp, text)), text, "{cp:?}");
        }
        assert_eq!(encode(CodePage::Windows1252, "Ação €"), b"A\xE7\xE3o \x80".to_vec());
        // ISO-8859-1 has no euro sign: X'7F'.
        assert_eq!(encode(CodePage::Iso8859_1, "€"), vec![0x7F]);
        assert_eq!(encode(CodePage::Windows1252, "日"), vec![0x7F]);
        // 0x81 is undefined in Windows-1252: U+001A. A broken UTF-8
        // sequence: U+FFFD.
        assert_eq!(decode(CodePage::Windows1252, &[0x81]), "\u{1A}");
        assert_eq!(decode(CodePage::Utf8, &[0xC3]), "\u{FFFD}");
    }

    #[test]
    fn typed_text_falls_back_to_windows_1252() {
        assert_eq!(typed_text("Olá, João".as_bytes()), "Olá, João");
        assert_eq!(typed_text(b"Ol\xE1, Jo\xE3o"), "Olá, João");
    }

    // IBM's own examples (Enterprise COBOL 6.4 LR, the U-function chapters).
    const KAEFER: UArg = UArg { bytes: b"K\xC3\xA4fer", national: false };

    fn tb() -> Vec<u8> {
        // nx'005400F6006200750072D858DC6B0073'
        vec![0x00, 0x54, 0x00, 0xF6, 0x00, 0x62, 0x00, 0x75, 0x00, 0x72, 0xD8, 0x58, 0xDC, 0x6B, 0x00, 0x73]
    }

    #[test]
    fn ibm_examples_utf8() {
        assert_eq!(ulength(KAEFER), 5);
        assert_eq!((1..=6).map(|n| upos(KAEFER, n)).collect::<Vec<_>>(), [1, 2, 4, 5, 6, 0]);
        assert_eq!((1..=5).map(|n| uwidth(KAEFER, n)).collect::<Vec<_>>(), [1, 2, 1, 1, 1]);
        assert_eq!(usubstr(KAEFER, 1, 2), "Kä");
        assert_eq!(usubstr(KAEFER, 2, 2), "äf");
        assert_eq!(usubstr(KAEFER, 3, 2), "fe");
        assert_eq!(uvalid(KAEFER), 0);
        let decomposed = UArg { bytes: b"a\xCC\x88K", national: false };
        assert_eq!((ulength(decomposed), upos(decomposed, 3), uwidth(decomposed, 2)), (3, 4, 2));
        let clef = UArg { bytes: b"  \xF0\x9D\x84\x9E", national: false };
        assert_eq!(usupplementary(clef), 3);
        let b = UArg { bytes: b"\xC3\xA4\xF0\xA1\xB7\xA4K", national: false };
        assert_eq!(usupplementary(b), 3);
    }

    #[test]
    fn ibm_examples_national() {
        let bytes = tb();
        let b = UArg { bytes: &bytes, national: true };
        assert_eq!(ulength(b), 7);
        assert_eq!((1..=7).map(|n| upos(b, n)).collect::<Vec<_>>(), [1, 3, 5, 7, 9, 11, 15]);
        assert_eq!((1..=7).map(|n| uwidth(b, n)).collect::<Vec<_>>(), [2, 2, 2, 2, 2, 4, 2]);
        assert_eq!(usubstr(b, 1, 2), "Tö");
        assert_eq!(usubstr(b, 6, 2).chars().count(), 2);
        assert_eq!(uvalid(b), 0);
        let clef = [0x00, 0x20, 0x00, 0x20, 0xD8, 0x34, 0xDD, 0x1E];
        assert_eq!(usupplementary(UArg { bytes: &clef, national: true }), 3);
        // nx'0054D9C3006200750072D858DC6B0073': unpaired high surrogate at unit 2.
        let c = [0x00, 0x54, 0xD9, 0xC3, 0x00, 0x62, 0x00, 0x75, 0x00, 0x72, 0xD8, 0x58, 0xDC, 0x6B, 0x00, 0x73];
        assert_eq!(uvalid(UArg { bytes: &c, national: true }), 2);
        // nx'005400F60062DC01…': a lone low surrogate at unit 4.
        let d = [0x00, 0x54, 0x00, 0xF6, 0x00, 0x62, 0xDC, 0x01, 0x00, 0x72, 0xD8, 0x58, 0xDC, 0x6B, 0x00, 0x73];
        assert_eq!(uvalid(UArg { bytes: &d, national: true }), 4);
    }

    #[test]
    fn out_of_range_and_ill_formed() {
        assert_eq!((upos(KAEFER, 0), uwidth(KAEFER, -1), upos(KAEFER, 9)), (0, 0, 0));
        assert_eq!(usubstr(KAEFER, 4, 10), "er");
        assert_eq!(usubstr(KAEFER, 9, 1), "");
        // A sequence cut off at the end is reported at its lead byte.
        assert_eq!(uvalid(UArg { bytes: b"A\xC3", national: false }), 2);
        assert_eq!(uvalid(UArg { bytes: b"AB\xFF", national: false }), 3);
    }
}
