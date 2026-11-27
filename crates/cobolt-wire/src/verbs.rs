// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The rules of the Backend's verbs (`COBOL::"WS-FAIL"` and its seven siblings) that do not depend on who runs the program
//! (spec 114 T36: one implementation of each rule).
//!
//! A Backend program is run by the interpreter (`cobolt-runtime`) or, built as a WebAssembly module, by the Backend runner
//! (`cobolt-runner`). Both read a verb's arguments their own way, and both then apply what is here: how long a failure's
//! message may be, what a progress percentage rounds to, how long a pause may last, which words a refusal uses and what the
//! file verbs' status says. A rule that lived in two places would one day differ, and the two engines would answer one
//! call two ways.

use crate::error::codes;

/// The longest message `WS-FAIL` sends, in characters.
pub const FAIL_MESSAGE_CHARS: usize = 200;
/// The longest detail `WS-FAIL` sends, in characters.
pub const FAIL_DETAIL_CHARS: usize = 4096;
/// The longest message `WS-PROGRESS` sends, in characters.
pub const PROGRESS_MESSAGE_CHARS: usize = 120;
/// The longest `WS-PAUSE`, in milliseconds.
pub const PAUSE_MAX_MS: f64 = 600_000.0;
/// `WS-PAUSE` waits in slices of this many milliseconds, so a cancel is seen within one.
pub const PAUSE_SLICE_MS: u64 = 10;

/// What the file verbs' status item says.
pub const FILE_OK: &str = "OK";
/// The id names no file the caller may reach, or the path cannot be published.
pub const FILE_NOFILE: &str = "NOFILE";
/// The call carries no file service.
pub const FILE_NOBACKEND: &str = "NOBACKEND";

/// The code `WS-FAIL` reports: the value rounded, and 1 to [`codes::APPLICATION_MAX`] (8000 and above belong to the platform).
/// `Err` is the words of the run-time error a code outside the range is.
pub fn fail_code(value: f64) -> Result<u32, String> {
    let code = value.round();
    if (1.0..=f64::from(codes::APPLICATION_MAX)).contains(&code) {
        Ok(code as u32)
    } else {
        Err(format!(
            "COBOL::\"WS-FAIL\": the code must be 1 to {}; {} and above belong to the platform",
            codes::APPLICATION_MAX,
            codes::APPLICATION_MAX + 1
        ))
    }
}

/// `WS-FAIL`'s message: its trailing spaces cut, then its first [`FAIL_MESSAGE_CHARS`] characters.
pub fn fail_message(raw: &str) -> String {
    raw.trim_end().chars().take(FAIL_MESSAGE_CHARS).collect()
}

/// `WS-FAIL`'s detail: the same, [`FAIL_DETAIL_CHARS`] characters, and `None` when nothing is left.
pub fn fail_detail(raw: &str) -> Option<String> {
    let d: String = raw.trim_end().chars().take(FAIL_DETAIL_CHARS).collect();
    (!d.is_empty()).then_some(d)
}

/// `WS-PROGRESS`'s percentage: the value held to 0 to 100, then rounded.
pub fn progress_percent(value: f64) -> u8 {
    value.clamp(0.0, 100.0).round() as u8
}

/// `WS-PROGRESS`'s message: its trailing spaces cut, then its first [`PROGRESS_MESSAGE_CHARS`] characters.
pub fn progress_message(raw: &str) -> String {
    raw.trim_end().chars().take(PROGRESS_MESSAGE_CHARS).collect()
}

/// `WS-PAUSE`'s milliseconds: the value held to 0 to [`PAUSE_MAX_MS`], without its decimals.
pub fn pause_ms(value: f64) -> u64 {
    value.clamp(0.0, PAUSE_MAX_MS) as u64
}

/// The run-time error of a verb that acts on a call, run by something that is not serving one.
pub fn not_serving(verb: &str) -> String {
    format!("COBOL::\"{verb}\" belongs to a program the backend exports; this run is not serving a call")
}

/// The run-time error of `WS-EMIT` in a program that was not exported as a stream.
pub const EMIT_NOT_A_STREAM: &str = "COBOL::\"WS-EMIT\" is for a program exported as a stream";

/// The run-time error of `WS-EMIT` for a group that cannot be written as JSON; `why` is the mapping's own words.
pub fn emit_unwritable(why: &str) -> String {
    format!("COBOL::\"WS-EMIT\": {why}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rules_cut_and_clamp_as_the_interpreter_always_did() {
        assert_eq!(fail_code(1.0), Ok(1));
        assert_eq!(fail_code(7999.4), Ok(7999));
        assert_eq!(fail_code(4200.5), Ok(4201), "rounded, not cut");
        assert!(fail_code(0.4).unwrap_err().contains("1 to 7999"));
        assert!(fail_code(8000.0).unwrap_err().contains("8000 and above"));
        assert!(fail_code(-3.0).is_err());
        assert_eq!(fail_message("card declined   "), "card declined");
        assert_eq!(fail_message(&"é".repeat(300)).chars().count(), FAIL_MESSAGE_CHARS);
        assert_eq!(fail_detail("   "), None);
        assert_eq!(fail_detail("{\"retry\":false} "), Some("{\"retry\":false}".to_owned()));
        assert_eq!(fail_detail(&"x".repeat(5000)).unwrap().len(), FAIL_DETAIL_CHARS);
        assert_eq!(progress_percent(-5.0), 0);
        assert_eq!(progress_percent(33.5), 34);
        assert_eq!(progress_percent(250.0), 100);
        assert_eq!(progress_message(&"m".repeat(500)).len(), PROGRESS_MESSAGE_CHARS);
        assert_eq!(pause_ms(-1.0), 0);
        assert_eq!(pause_ms(40.9), 40);
        assert_eq!(pause_ms(1e12), 600_000);
        assert!(not_serving("WS-FAIL").contains("not serving a call"));
        println!("verbs: 20 rules of the Backend's verbs checked");
    }
}
