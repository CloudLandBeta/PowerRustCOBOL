// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **`COBOL::"PARTIQL"`, `"PARTIQL-FETCH"` and `"PARTIQL-ERROR"` in a desktop module** (ABI A56,
//! DESKTOP-WASM-001 T48), through the one `prn` import of the capability `partiql.files`:
//!
//! `pq_file(op, req, req_len, buf, cap) -> i64`
//!
//! The statements run on a file the program's own `SELECT` declares and holds open, by the
//! interpreter's own rules (`cobolt_runtime::partiql::door`): the schema is the file's first record
//! description (`files::FileDecl::definition`) with the keys found in it by name
//! (`files::FileDecl::named_keys`); the results stay with the program, one set for each file it names,
//! for `PARTIQL-FETCH` and `PARTIQL-ERROR`.
//!
//! The request at `req` (`req_len` bytes, little-endian words) names memory 0's bytes:
//!
//! ```text
//!  nfiles                       the program's files
//!  nfiles × (id, slot)          each one's number in `prfiles` and its slot
//!  file_ptr, file_len           the file argument, every character
//! RUN only:
//!  text_ptr, text_len           the statement, every character
//!  nargs
//!  nargs × (kind, ptr, len)     each `?` argument: its kind (`N` a number, `S` anything else) and text
//! ```
//!
//! The runner trims and upper-cases the file argument and trims the statement's end, as the
//! interpreter reads them. The answer's text goes to `buf`, at most `cap` bytes (its first, or with
//! [`TAIL`] the last of it without its trailing spaces), and the import returns a number in bits 32 to
//! 62 and the text's whole length in bits 0 to 31, or [`FAILED`]:
//!
//! - [`RUN`]: the text is the status, the number the count;
//! - [`FETCH`]: the text is the row, the number 1 when a row was delivered (`00`), 0 past the last (`10`);
//! - [`ERROR`]: the text is the last statement's error, the number 0.

/// `COBOL::"PARTIQL" ( file statement count status [arguments] )`.
pub const RUN: u32 = 1;
/// `COBOL::"PARTIQL-FETCH" ( file row status )`.
pub const FETCH: u32 = 2;
/// `COBOL::"PARTIQL-ERROR" ( file text )`.
pub const ERROR: u32 = 3;

/// Added to an operation: the text without its trailing spaces, and its **last** `cap` bytes written,
/// for a receiver that is `JUSTIFIED RIGHT` (as `http::TAIL`).
pub const TAIL: u32 = 0x100;

/// The runner could not run the call (an operation it does not know, a request it cannot read): the
/// module leaves the receivers as they were.
pub const FAILED: i64 = -1;

/// An argument's kind: a number (a numeric item by its declaration, a numeric literal, a computed
/// value), or anything else.
pub const NUMBER: u8 = b'N';
pub const TEXT: u8 = b'S';

/// The answer's number (bits 32 to 62).
pub fn number(answer: i64) -> u32 {
    ((answer as u64 >> 32) & 0x7FFF_FFFF) as u32
}

/// The answer text's whole length (bits 0 to 31); the bytes written are at most `cap`.
pub fn text_len(answer: i64) -> u32 {
    (answer as u64 & 0xFFFF_FFFF) as u32
}

/// The answer for `number` (at most 2³¹ − 1) and a text of `len` bytes.
pub fn answer(number: usize, len: usize) -> i64 {
    let n = number.min(0x7FFF_FFFF) as u64;
    ((n << 32) | (len.min(u32::MAX as usize) as u64)) as i64
}

/// A request, its texts still named by their place in memory 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The program's files: `(id, slot)`.
    pub files: Vec<(u32, u32)>,
    /// The file argument: `(ptr, len)`.
    pub file: (u32, u32),
    /// [`RUN`]: the statement.
    pub text: (u32, u32),
    /// [`RUN`]: the `?` arguments, `(kind, ptr, len)`.
    pub args: Vec<(u8, u32, u32)>,
}

/// Read the request of operation `op` (`TAIL` ignored); `None` when it is shorter than it says.
pub fn decode(op: u32, req: &[u8]) -> Option<Request> {
    let mut at = 0usize;
    let mut word = || -> Option<u32> {
        let w = req.get(at..at + 4)?;
        at += 4;
        Some(u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
    };
    let nfiles = word()?;
    let mut files = Vec::new();
    for _ in 0..nfiles {
        files.push((word()?, word()?));
    }
    let file = (word()?, word()?);
    let (mut text, mut args) = ((0, 0), Vec::new());
    if op & !TAIL == RUN {
        text = (word()?, word()?);
        let nargs = word()?;
        for _ in 0..nargs {
            let kind = word()?;
            args.push((u8::try_from(kind).ok()?, word()?, word()?));
        }
    }
    Some(Request { files, file, text, args })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(ws: &[u32]) -> Vec<u8> {
        ws.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    /// Each operation's request reads back as written, and a short one is refused.
    #[test]
    fn a_request_reads_back_and_a_short_one_is_refused() {
        let run = words(&[2, 0, 100, 3, 112, 500, 9, 520, 40, 2, NUMBER as u32, 600, 3, TEXT as u32, 610, 7]);
        let got = decode(RUN, &run).expect("a RUN request reads");
        assert_eq!(got.files, vec![(0, 100), (3, 112)]);
        assert_eq!(got.file, (500, 9));
        assert_eq!(got.text, (520, 40));
        assert_eq!(got.args, vec![(NUMBER, 600, 3), (TEXT, 610, 7)]);
        let fetch = words(&[1, 4, 200, 700, 8]);
        let got = decode(FETCH | TAIL, &fetch).expect("a FETCH request reads");
        assert_eq!((got.files, got.file, got.args.len()), (vec![(4, 200)], (700, 8), 0));
        let mut refused = 0;
        for cut in 0..run.len() {
            if decode(RUN, &run[..cut]).is_none() {
                refused += 1;
            }
        }
        assert_eq!(refused, run.len(), "every shortened RUN request is refused");
        let a = answer(42, 2);
        assert_eq!((number(a), text_len(a)), (42, 2));
        assert_eq!(number(answer(usize::MAX, 0)), 0x7FFF_FFFF, "a count past 2^31 - 1 is clamped");
        assert!(answer(usize::MAX, usize::MAX) >= 0, "an answer is never FAILED");
        println!("pq_file requests: 2 of 2 read back; {refused} of {} shortened RUN requests refused; 3 answers round-trip", run.len());
    }
}
