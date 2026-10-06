// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! How a statement went: SQLSTATE and SQLCODE (spec 087 R18, R19).
//!
//! SQLSTATE is the standard five-character code. SQLCODE is derived from it by
//! one published rule, so the two never disagree and a program may test
//! either:
//!
//! | SQLSTATE                         | SQLCODE                        |
//! |----------------------------------|--------------------------------|
//! | `00000` success                  | 0                              |
//! | `02000` no data                  | +100                           |
//! | class `01` (warning)             | the five characters as a number when all are digits (`01004` → +1004), otherwise +1000 |
//! | error, all five digits           | the negated number (`23505` → −23505) |
//! | error whose subclass has letters | −(class × 1000) (`42P01` → −42000) |
//! | any other error                  | −99000                         |

/// An SQL failure or warning: the standard code and the database's own words.
#[derive(Debug, Clone, PartialEq)]
pub struct SqlError {
    pub sqlstate: String,
    pub message: String,
    /// The database's native error number, 0 when it has none.
    pub native: i64,
}

impl SqlError {
    pub fn new(sqlstate: &str, message: impl Into<String>) -> Self {
        Self { sqlstate: sqlstate.to_string(), message: message.into(), native: 0 }
    }

    pub fn with_native(mut self, native: i64) -> Self {
        self.native = native;
        self
    }
}

impl std::fmt::Display for SqlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SQLSTATE {}: {}", self.sqlstate, self.message)
    }
}

/// The codes the runtime raises itself.
pub mod code {
    pub const SUCCESS: &str = "00000";
    pub const NO_DATA: &str = "02000";
    pub const TRUNCATED: &str = "01004";
    pub const DESCRIPTOR_TOO_SMALL: &str = "01005";
    pub const CARDINALITY: &str = "21000";
    pub const NULL_NO_INDICATOR: &str = "22002";
    pub const OUT_OF_RANGE: &str = "22003";
    pub const INVALID_CHARACTER_VALUE: &str = "22018";
    pub const INVALID_CURSOR_STATE: &str = "24000";
    pub const WRONG_TARGET_COUNT: &str = "07002";
    pub const PREPARED_NOT_FOUND: &str = "07003";
    pub const CANNOT_CONNECT: &str = "08001";
    pub const CONNECTION_EXISTS: &str = "08002";
    pub const NO_CONNECTION: &str = "08003";
    pub const NOT_SUPPORTED: &str = "0A000";
    pub const INVALID_AUTHORIZATION: &str = "28000";
    pub const GENERAL: &str = "HY000";
}

/// The outcome class of a SQLSTATE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Warning,
    NoData,
    Error,
}

pub fn outcome(sqlstate: &str) -> Outcome {
    match sqlstate.get(..2) {
        Some("00") => Outcome::Success,
        Some("01") => Outcome::Warning,
        Some("02") => Outcome::NoData,
        _ => Outcome::Error,
    }
}

/// SQLCODE for a SQLSTATE, by the rule in this module's documentation.
pub fn sqlcode(sqlstate: &str) -> i64 {
    let s = sqlstate.trim();
    let all_digits = s.len() == 5 && s.bytes().all(|b| b.is_ascii_digit());
    match outcome(s) {
        Outcome::Success => 0,
        Outcome::NoData => 100,
        Outcome::Warning => {
            if all_digits {
                s.parse().unwrap_or(1000)
            } else {
                1000
            }
        }
        Outcome::Error => {
            if all_digits {
                -s.parse::<i64>().unwrap_or(99000)
            } else if s.len() >= 2 && s.as_bytes()[..2].iter().all(|b| b.is_ascii_digit()) {
                -(s[..2].parse::<i64>().unwrap_or(99) * 1000)
            } else {
                -99000
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule's published examples.
    #[test]
    fn sqlcode_follows_the_published_rule() {
        let cases = [
            ("00000", 0),
            ("02000", 100),
            ("01004", 1004),
            ("01P01", 1000),
            ("23505", -23505),
            ("42601", -42601),
            ("42P01", -42000),
            ("22003", -22003),
            ("HY000", -99000),
            ("08001", -8001),
        ];
        for (state, want) in cases {
            assert_eq!(sqlcode(state), want, "SQLCODE of {state}");
        }
        assert_eq!(outcome("01004"), Outcome::Warning);
        assert_eq!(outcome("02000"), Outcome::NoData);
        assert_eq!(outcome("42P01"), Outcome::Error);
        println!("SQLSTATE → SQLCODE: {cases:?}");
    }
}
