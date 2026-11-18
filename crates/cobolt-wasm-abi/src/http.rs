// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The HTTP built-ins of a desktop module** (ABI A45, DESKTOP-WASM-001 T42): `COBOL::"HTTP-GET"`,
//! `"HTTP-POST"`, `"HTTP-PUT"`, `"HTTP-DELETE"`, `"HTTP-SET-HEADER"`, `"HTTP-CLEAR-HEADERS"` and
//! `"HTTP-DOWNLOAD"`, through the one `prn` import of the capability `http`:
//!
//! `http_call(op, a, a_len, b, b_len, buf, cap) -> i64`
//!
//! The runner keeps one HTTP client for each form instance (and one for a console program), as each
//! form's interpreter keeps its own, so the headers `HTTP-SET-HEADER` sets are that window's, shared by
//! its handlers and the procedures they call. It runs each call with the interpreter's own client
//! (`cobolt_runtime::HttpClient`), so the request, the headers sent, the redirects, the timeouts and
//! the text answered when no answer came back are the same on both paths.
//!
//! The two texts are the call's arguments as the interpreter reads them (the runner trims them as the
//! interpreter does). The answer is a number and a text: the runner writes the text at `buf`, at most
//! `cap` bytes, and returns the number in bits 32 to 47 and the text's whole length in bits 0 to 31,
//! or [`FAILED`].

/// `HTTP-GET ( url response http-status )`: `a` is the URL; the text is the body, the number the
/// status code (0 when no answer came back, the body then saying why).
pub const GET: u32 = 1;
/// `HTTP-POST ( url body response http-status )`: `a` is the URL, `b` the body.
pub const POST: u32 = 2;
/// `HTTP-PUT ( url body response http-status )`.
pub const PUT: u32 = 3;
/// `HTTP-DELETE ( url response http-status )`.
pub const DELETE: u32 = 4;
/// `HTTP-SET-HEADER ( name value )`: `a` is the name, `b` the value; nothing is answered.
pub const SET_HEADER: u32 = 5;
/// `HTTP-CLEAR-HEADERS`: nothing is answered.
pub const CLEAR_HEADERS: u32 = 6;
/// `HTTP-DOWNLOAD ( url file http-status status )`: `a` is the URL, `b` the file as the program
/// names it; the text is the status word (`OK`, `BADURL`, `TIMEOUT`, `TOOBIG`, `HTTP`, `WRITE`, `NET`
/// or `NOFEATURE`), the number the status code.
pub const DOWNLOAD: u32 = 7;

/// Added to an operation: the text without its trailing spaces, and its **last** `cap` bytes written
/// rather than its first, for a receiver that is `JUSTIFIED RIGHT` (the interpreter puts a text into one
/// so: its end kept, without the spaces after it). The length answered is that of the text without its
/// trailing spaces.
pub const TAIL: u32 = 0x100;

/// The runner could not run the call (an operation it does not know, memory it cannot reach): the
/// module leaves the receivers as they were.
pub const FAILED: i64 = -1;

/// The answer's number (bits 32 to 47).
pub fn number(answer: i64) -> u32 {
    ((answer as u64 >> 32) & 0xFFFF) as u32
}

/// The answer text's whole length (bits 0 to 31); the bytes written are at most `cap`.
pub fn text_len(answer: i64) -> u32 {
    (answer as u64 & 0xFFFF_FFFF) as u32
}

/// The answer for `number` and a text of `len` bytes.
pub fn answer(number: u16, len: usize) -> i64 {
    ((u64::from(number) << 32) | (len.min(u32::MAX as usize) as u64)) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A number and a length go into one answer and come out again, at their limits too.
    #[test]
    fn an_answer_carries_its_number_and_its_length() {
        let cases: [(u16, usize); 6] = [(0, 0), (200, 17), (404, 0), (999, 1 << 20), (u16::MAX, u32::MAX as usize), (301, usize::MAX)];
        for (n, len) in cases {
            let a = answer(n, len);
            assert!(a >= 0, "an answer is never FAILED: {n} {len}");
            assert_eq!(number(a), u32::from(n));
            assert_eq!(text_len(a) as usize, len.min(u32::MAX as usize));
        }
        println!("http answers: {} of {} round trips equal", cases.len(), cases.len());
    }
}
