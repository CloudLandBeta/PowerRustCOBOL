// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The national and UTF-8 functions of a desktop module (DESKTOP-WASM-001 T50a1): the rules of
//! `cobolt_runtime::national` (spec 077) mirrored over borrowed bytes, for the runtime has no
//! allocator.
//!
//! * the code pages `NATIONAL-OF` and `DISPLAY-OF` convert from and to ([`CodePage`]);
//! * the U-functions, which each work on an argument's bytes: UTF-8 for an alphanumeric or UTF-8
//!   argument, UTF-16 big-endian for a national one ([`walk`], [`valid`], [`ulength`] …);
//! * Unicode case mapping, which `UPPER-CASE` and `LOWER-CASE` apply to national and UTF-8 data
//!   (the interpreter's `str::to_uppercase` and `str::to_lowercase`, [`lower_next`]).

/// Windows-1252 bytes 0x80–0x9F. The five bytes the code page leaves undefined are `None`.
const CP1252_HIGH: [Option<char>; 32] = [
    Some('\u{20AC}'),
    None,
    Some('\u{201A}'),
    Some('\u{0192}'),
    Some('\u{201E}'),
    Some('\u{2026}'),
    Some('\u{2020}'),
    Some('\u{2021}'),
    Some('\u{02C6}'),
    Some('\u{2030}'),
    Some('\u{0160}'),
    Some('\u{2039}'),
    Some('\u{0152}'),
    None,
    Some('\u{017D}'),
    None,
    None,
    Some('\u{2018}'),
    Some('\u{2019}'),
    Some('\u{201C}'),
    Some('\u{201D}'),
    Some('\u{2022}'),
    Some('\u{2013}'),
    Some('\u{2014}'),
    Some('\u{02DC}'),
    Some('\u{2122}'),
    Some('\u{0161}'),
    Some('\u{203A}'),
    Some('\u{0153}'),
    None,
    Some('\u{017E}'),
    Some('\u{0178}'),
];

/// A code page `NATIONAL-OF` and `DISPLAY-OF` convert from or to (the interpreter's `CodePage`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodePage {
    /// UTF-8, CCSID 1208.
    Utf8,
    /// Windows-1252, CCSID 1252.
    Windows1252,
    /// ISO-8859-1 (Latin-1), CCSID 819.
    Iso8859_1,
}

impl CodePage {
    /// The code page a program names, in any case: `UTF-8`, `UTF8`, `1208`; `WINDOWS-1252`,
    /// `WINDOWS1252`, `CP1252`, `1252`; `ISO-8859-1`, `ISO8859-1`, `LATIN-1`, `LATIN1`, `819`.
    pub fn parse(name: &[u8]) -> Option<CodePage> {
        let t = trim_ascii(name);
        let is = |s: &str| t.eq_ignore_ascii_case(s.as_bytes());
        if is("UTF-8") || is("UTF8") || is("1208") {
            Some(CodePage::Utf8)
        } else if is("WINDOWS-1252") || is("WINDOWS1252") || is("CP1252") || is("1252") {
            Some(CodePage::Windows1252)
        } else if is("ISO-8859-1") || is("ISO8859-1") || is("LATIN-1") || is("LATIN1") || is("819") {
            Some(CodePage::Iso8859_1)
        } else {
            None
        }
    }

    /// One byte of a single-byte code page as a character: a Windows-1252 byte that code page leaves
    /// undefined is U+001A, as IBM documents (`FUNCTION NATIONAL-OF`).
    pub fn decode_byte(self, b: u8) -> char {
        match (self, b) {
            (CodePage::Windows1252, 0x80..=0x9F) => CP1252_HIGH[(b - 0x80) as usize].unwrap_or('\u{1A}'),
            _ => b as char,
        }
    }

    /// A character as the bytes it takes in the code page, written to `out`; how many. A character a
    /// single-byte code page cannot hold becomes X'7F', IBM's substitution character (`DISPLAY-OF`).
    pub fn encode_char(self, c: char, out: &mut [u8; 4]) -> usize {
        match self {
            CodePage::Utf8 => c.encode_utf8(out).len(),
            CodePage::Iso8859_1 => {
                out[0] = u8::try_from(c as u32).unwrap_or(0x7F);
                1
            }
            CodePage::Windows1252 => {
                out[0] = match c as u32 {
                    0x80..=0x9F => 0x7F,
                    n @ 0..=0xFF => n as u8,
                    _ => CP1252_HIGH.iter().position(|m| *m == Some(c)).map_or(0x7F, |i| 0x80 + i as u8),
                };
                1
            }
        }
    }
}

/// `b` without the ASCII white space at either end.
pub fn trim_ascii(b: &[u8]) -> &[u8] {
    let ws = |x: &u8| matches!(x, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C);
    let s = b.iter().position(|x| !ws(x)).unwrap_or(b.len());
    let e = b.iter().rposition(|x| !ws(x)).map_or(s, |i| i + 1);
    &b[s..e.max(s)]
}

// ── The U-functions ──────────────────────────────────────────────────────────
//
// Positions are 1-based. `UPOS` and `UWIDTH` answer in bytes; `USUBSTR` counts characters;
// `UVALID` and `USUPPLEMENTARY` answer in bytes for UTF-8 and in UTF-16 code units for national.
// Ill-formed data is walked a byte (UTF-8) or a unit (UTF-16) at a time, as U+FFFD, so the
// functions never stop the program.

/// Each character of `bytes` as `(byte offset, byte width, character)`, until `f` answers `false`.
/// An odd final byte of national data is the high half of a code unit whose low half is zero. The
/// callback is a type parameter, not a trait object: a module has no table to call through.
pub fn walk<F: FnMut(usize, usize, char) -> bool>(bytes: &[u8], national: bool, f: &mut F) {
    if national {
        let n = bytes.len().div_ceil(2);
        let unit = |k: usize| u16::from_be_bytes([bytes[2 * k], *bytes.get(2 * k + 1).unwrap_or(&0)]);
        let mut i = 0;
        while i < n {
            let u = unit(i);
            let (width, c) = if (0xD800..0xDC00).contains(&u) && i + 1 < n && (0xDC00..0xE000).contains(&unit(i + 1)) {
                let c = 0x10000 + (((u as u32) - 0xD800) << 10) + ((unit(i + 1) as u32) - 0xDC00);
                (4, char::from_u32(c).unwrap_or('\u{FFFD}'))
            } else {
                (2, char::from_u32(u as u32).unwrap_or('\u{FFFD}'))
            };
            if !f(2 * i, width, c) {
                return;
            }
            i += width / 2;
        }
        return;
    }
    let mut i = 0;
    while i < bytes.len() {
        match core::str::from_utf8(&bytes[i..]) {
            Ok(rest) => {
                for (off, c) in rest.char_indices() {
                    if !f(i + off, c.len_utf8(), c) {
                        return;
                    }
                }
                return;
            }
            Err(e) => {
                let good = e.valid_up_to();
                if let Ok(rest) = core::str::from_utf8(&bytes[i..i + good]) {
                    for (off, c) in rest.char_indices() {
                        if !f(i + off, c.len_utf8(), c) {
                            return;
                        }
                    }
                }
                let bad = e.error_len().unwrap_or(bytes.len() - i - good).max(1);
                if !f(i + good, bad, '\u{FFFD}') {
                    return;
                }
                i += good + bad;
            }
        }
    }
}

/// `ULENGTH`: how many characters.
pub fn ulength(bytes: &[u8], national: bool) -> usize {
    let mut n = 0;
    walk(bytes, national, &mut |_, _, _| {
        n += 1;
        true
    });
    n
}

/// The `n`-th character (1-based) as `(byte offset, byte width)`.
fn nth(bytes: &[u8], national: bool, n: i64) -> Option<(usize, usize)> {
    if n < 1 {
        return None;
    }
    let (mut k, mut found) = (0i64, None);
    walk(bytes, national, &mut |off, width, _| {
        k += 1;
        if k == n {
            found = Some((off, width));
            false
        } else {
            true
        }
    });
    found
}

/// `UPOS(arg, n)`: the byte where the n-th character starts; 0 out of range.
pub fn upos(bytes: &[u8], national: bool, n: i64) -> usize {
    nth(bytes, national, n).map_or(0, |(off, _)| off + 1)
}

/// `UWIDTH(arg, n)`: the n-th character's width in bytes; 0 out of range.
pub fn uwidth(bytes: &[u8], national: bool, n: i64) -> usize {
    nth(bytes, national, n).map_or(0, |(_, width)| width)
}

/// `UVALID`: 0 when the argument is well-formed; otherwise where the first ill-formed sequence
/// starts: its byte for UTF-8 (a sequence cut off at the end is reported at its lead byte), its
/// code unit for national.
pub fn uvalid(bytes: &[u8], national: bool) -> usize {
    if national {
        let n = bytes.len().div_ceil(2);
        let unit = |k: usize| u16::from_be_bytes([bytes[2 * k], *bytes.get(2 * k + 1).unwrap_or(&0)]);
        let mut i = 0;
        while i < n {
            match unit(i) {
                0xD800..=0xDBFF if i + 1 < n && (0xDC00..0xE000).contains(&unit(i + 1)) => i += 2,
                0xD800..=0xDFFF => return i + 1,
                _ => i += 1,
            }
        }
        // An odd final byte is half a code unit.
        return if bytes.len() % 2 == 1 { n } else { 0 };
    }
    match core::str::from_utf8(bytes) {
        Ok(_) => 0,
        Err(e) => e.valid_up_to() + 1,
    }
}

/// `USUPPLEMENTARY`: where the first character above U+FFFF starts (its byte for UTF-8, its code
/// unit for national), or 0.
pub fn usupplementary(bytes: &[u8], national: bool) -> usize {
    let mut at = 0;
    walk(bytes, national, &mut |off, _, c| {
        if (c as u32) > 0xFFFF {
            at = if national { off / 2 + 1 } else { off + 1 };
            false
        } else {
            true
        }
    });
    at
}

// ── Unicode case mapping ─────────────────────────────────────────────────────

/// Whether `c` is a cased letter: lower case, upper case or title case.
fn is_cased(c: char) -> bool {
    c.is_lowercase()
        || c.is_uppercase()
        || matches!(c as u32, 0x01C5 | 0x01C8 | 0x01CB | 0x01F2 | 0x1F88..=0x1F8F | 0x1F98..=0x1F9F | 0x1FA8..=0x1FAF | 0x1FBC | 0x1FCC | 0x1FFC)
}

/// Whether `c` is skipped when `LOWER-CASE` decides if a capital sigma ends a word. The standard's
/// `Case_Ignorable` set is a table; this holds what text carries between letters in practice: the
/// apostrophe, the full stop, the colon, the middle dot, the soft hyphen, the quotation marks that
/// stand for an apostrophe and the combining marks.
fn is_case_ignorable(c: char) -> bool {
    matches!(c as u32, 0x27 | 0x2E | 0x3A | 0x5E | 0x60 | 0xA8 | 0xAD | 0xAF | 0xB4 | 0xB7 | 0xB8 | 0x0300..=0x036F | 0x2018 | 0x2019 | 0x2024 | 0x2027 | 0xFF07 | 0xFF0E | 0xFF1A)
}

/// The characters `c` becomes in lower case, as `str::to_lowercase` maps it. A capital sigma is a
/// final sigma (U+03C2) at the end of a word: after a cased letter and before none. `prev_cased`
/// says whether the last character that was not ignorable was cased, and `rest` is what follows
/// `c`. The caller keeps `prev_cased` up to date with [`track_cased`].
pub fn lower_next<I: Iterator<Item = char> + Clone>(c: char, prev_cased: bool, rest: &I) -> LowerChars {
    if c == '\u{03A3}' {
        let mut ahead = rest.clone();
        let next_cased = ahead.find(|x| !is_case_ignorable(*x)).is_some_and(is_cased);
        return LowerChars { one: Some(if prev_cased && !next_cased { '\u{03C2}' } else { '\u{03C3}' }), many: None };
    }
    LowerChars { one: None, many: Some(c.to_lowercase()) }
}

/// The state [`lower_next`] needs after `c`: whether the last character that was not ignorable was
/// cased.
pub fn track_cased(prev_cased: bool, c: char) -> bool {
    if is_case_ignorable(c) {
        prev_cased
    } else {
        is_cased(c)
    }
}

/// The result of [`lower_next`]: one character, or what `char::to_lowercase` yields.
pub struct LowerChars {
    one: Option<char>,
    many: Option<core::char::ToLowercase>,
}

impl Iterator for LowerChars {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        if let Some(c) = self.one.take() {
            return Some(c);
        }
        self.many.as_mut()?.next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lower(s: &str) -> std::string::String {
        let mut out = std::string::String::new();
        let mut prev = false;
        let mut it = s.chars();
        while let Some(c) = it.next() {
            for l in lower_next(c, prev, &it) {
                out.push(l);
            }
            prev = track_cased(prev, c);
        }
        out
    }

    #[test]
    fn code_pages_are_named_in_any_case_or_by_ccsid() {
        for n in ["utf-8", " UTF8 ", "1208"] {
            assert_eq!(CodePage::parse(n.as_bytes()), Some(CodePage::Utf8), "{n}");
        }
        for n in ["Windows-1252", "cp1252", "1252"] {
            assert_eq!(CodePage::parse(n.as_bytes()), Some(CodePage::Windows1252), "{n}");
        }
        for n in ["iso-8859-1", "LATIN1", "819"] {
            assert_eq!(CodePage::parse(n.as_bytes()), Some(CodePage::Iso8859_1), "{n}");
        }
        assert_eq!(CodePage::parse(b"EBCDIC"), None);
    }

    #[test]
    fn single_byte_code_pages_convert_and_substitute() {
        let mut o = [0u8; 4];
        assert_eq!(CodePage::Iso8859_1.encode_char('ç', &mut o), 1);
        assert_eq!(o[0], 0xE7);
        CodePage::Iso8859_1.encode_char('€', &mut o);
        assert_eq!(o[0], 0x7F);
        CodePage::Windows1252.encode_char('€', &mut o);
        assert_eq!(o[0], 0x80);
        CodePage::Windows1252.encode_char('\u{85}', &mut o);
        assert_eq!(o[0], 0x7F);
        assert_eq!(CodePage::Windows1252.decode_byte(0x80), '€');
        assert_eq!(CodePage::Windows1252.decode_byte(0x81), '\u{1A}');
        assert_eq!(CodePage::Iso8859_1.decode_byte(0x80), '\u{80}');
    }

    #[test]
    fn the_u_functions_count_characters_and_bytes() {
        let t = "Aç€😀".as_bytes();
        assert_eq!(ulength(t, false), 4);
        assert_eq!((upos(t, false, 1), upos(t, false, 2), upos(t, false, 3), upos(t, false, 4), upos(t, false, 5)), (1, 2, 4, 7, 0));
        assert_eq!((uwidth(t, false, 1), uwidth(t, false, 2), uwidth(t, false, 3), uwidth(t, false, 4)), (1, 2, 3, 4));
        assert_eq!(usupplementary(t, false), 7);
        assert_eq!(uvalid(t, false), 0);
        assert_eq!(uvalid(b"ab\xFFcd", false), 3);
        // A sequence cut off at the end is reported at its lead byte.
        assert_eq!(uvalid(b"ab\xE2\x82", false), 3);
        // National: UTF-16 big-endian, a supplementary character is four bytes and two units.
        let n = [0u8, b'A', 0xD8, 0x3D, 0xDE, 0x00, 0, b'z'];
        assert_eq!(ulength(&n, true), 3);
        assert_eq!((upos(&n, true, 2), upos(&n, true, 3), uwidth(&n, true, 2)), (3, 7, 4));
        assert_eq!(usupplementary(&n, true), 2);
        assert_eq!(uvalid(&n, true), 0);
        assert_eq!(uvalid(&[0xD8, 0x00, 0, b'x'], true), 1);
        assert_eq!(uvalid(&[0, b'x', 0x41], true), 2);
        assert_eq!(upos(t, false, 0), 0);
    }

    #[test]
    fn case_mapping_is_unicode_with_the_final_sigma() {
        assert_eq!(lower("ÉLAN"), "élan");
        assert_eq!(lower("ΟΔΥΣΣΕΥΣ"), "οδυσσευς");
        assert_eq!(lower("Σ"), "σ");
        assert_eq!(lower("ΑΣ ΑΣ"), "ας ας");
        assert_eq!(lower("İ"), "i\u{307}");
        assert_eq!("ação".to_uppercase(), "AÇÃO");
    }
}
