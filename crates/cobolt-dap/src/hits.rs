// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Hit conditions — "stop the 500th time round the loop, not the first".
//!
//! DAP leaves the syntax to the adapter and clients converge on a small
//! algebra: a bare count, a comparison, or a modulus. Parsing lives here rather
//! than in the interpreter because it is pure text arithmetic with no COBOL in
//! it, and because a malformed condition must be rejected when the breakpoint
//! is *set* — telling the developer immediately — instead of quietly never
//! firing.

use std::fmt;

/// A parsed hit condition, evaluated against the number of times a breakpoint's
/// location has been reached (1 on the first arrival).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitCondition {
    /// `= n` / `== n` / bare `n`: fire on exactly the n-th hit.
    Equals(u64),
    /// `>= n`: fire on the n-th hit and every one after.
    AtLeast(u64),
    /// `> n`.
    GreaterThan(u64),
    /// `<= n`: fire until the n-th hit, then stop firing.
    AtMost(u64),
    /// `< n`.
    LessThan(u64),
    /// `% n`: fire on every n-th hit.
    Multiple(u64),
}

/// Why a hit condition was rejected. Carries text the developer can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitConditionError(pub String);

impl fmt::Display for HitConditionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for HitConditionError {}

impl HitCondition {
    /// Parse a DAP hit-condition string. Whitespace anywhere is ignored.
    pub fn parse(text: &str) -> Result<Self, HitConditionError> {
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.is_empty() {
            return Err(HitConditionError("a hit condition cannot be empty".into()));
        }

        // Longest operators first: `>=` must not be read as `>`.
        let (ctor, digits): (fn(u64) -> Self, &str) = if let Some(r) = compact.strip_prefix(">=") {
            (Self::AtLeast, r)
        } else if let Some(r) = compact.strip_prefix("<=") {
            (Self::AtMost, r)
        } else if let Some(r) = compact.strip_prefix("==") {
            (Self::Equals, r)
        } else if let Some(r) = compact.strip_prefix('>') {
            (Self::GreaterThan, r)
        } else if let Some(r) = compact.strip_prefix('<') {
            (Self::LessThan, r)
        } else if let Some(r) = compact.strip_prefix('=') {
            (Self::Equals, r)
        } else if let Some(r) = compact.strip_prefix('%') {
            (Self::Multiple, r)
        } else {
            (Self::Equals, compact.as_str())
        };

        let n: u64 = digits.parse().map_err(|_| {
            HitConditionError(format!(
                "{text:?} is not a hit condition — expected a count like 5, \
                 a comparison like >= 5, or a multiple like % 3"
            ))
        })?;

        // `% 0` would divide by zero and `= 0` can never be true (the first hit
        // is 1); both are mistakes worth naming rather than accepting.
        if n == 0 && matches!(ctor(0), Self::Multiple(_) | Self::Equals(_)) {
            return Err(HitConditionError(format!(
                "{text:?} can never fire — hit counts start at 1"
            )));
        }
        Ok(ctor(n))
    }

    /// Should the breakpoint fire on hit number `hits` (1-based)?
    pub fn fires_at(&self, hits: u64) -> bool {
        match *self {
            Self::Equals(n) => hits == n,
            Self::AtLeast(n) => hits >= n,
            Self::GreaterThan(n) => hits > n,
            Self::AtMost(n) => hits <= n,
            Self::LessThan(n) => hits < n,
            Self::Multiple(n) => n != 0 && hits % n == 0,
        }
    }
}

#[cfg(test)]
mod hit_condition_tests {
    use super::*;

    #[test]
    fn every_supported_spelling_parses() {
        for (text, want) in [
            ("5", HitCondition::Equals(5)),
            ("= 5", HitCondition::Equals(5)),
            ("==5", HitCondition::Equals(5)),
            (">= 5", HitCondition::AtLeast(5)),
            (">5", HitCondition::GreaterThan(5)),
            ("<= 5", HitCondition::AtMost(5)),
            ("< 5", HitCondition::LessThan(5)),
            ("% 3", HitCondition::Multiple(3)),
            ("  >=  500  ", HitCondition::AtLeast(500)),
        ] {
            assert_eq!(HitCondition::parse(text).unwrap(), want, "parsing {text:?}");
        }
    }

    /// `>=` before `>`: the classic ordering bug, which would turn "stop from
    /// the 5th hit on" into "stop from the 6th" and be nearly invisible.
    #[test]
    fn the_two_character_operators_win_over_their_prefixes() {
        assert_eq!(HitCondition::parse(">=2").unwrap(), HitCondition::AtLeast(2));
        assert_eq!(HitCondition::parse("<=2").unwrap(), HitCondition::AtMost(2));
        assert!(HitCondition::parse(">=2").unwrap().fires_at(2));
        assert!(!HitCondition::parse(">2").unwrap().fires_at(2));
    }

    #[test]
    fn firing_follows_the_arithmetic() {
        let cases: [(&str, [bool; 6]); 6] = [
            // hits 1..=6
            ("3", [false, false, true, false, false, false]),
            (">=3", [false, false, true, true, true, true]),
            (">3", [false, false, false, true, true, true]),
            ("<=3", [true, true, true, false, false, false]),
            ("<3", [true, true, false, false, false, false]),
            ("%3", [false, false, true, false, false, true]),
        ];
        for (text, expected) in cases {
            let cond = HitCondition::parse(text).unwrap();
            for (i, want) in expected.iter().enumerate() {
                let hits = i as u64 + 1;
                assert_eq!(cond.fires_at(hits), *want, "{text:?} at hit {hits}");
            }
        }
    }

    /// Rejected at set time, with a reason — not accepted and then silently
    /// never firing, which is indistinguishable from a broken debugger.
    #[test]
    fn nonsense_is_refused_with_something_to_read() {
        for bad in ["", "   ", "abc", ">", ">=x", "5x", "%0", "=0", "0"] {
            let err = HitCondition::parse(bad).unwrap_err();
            assert!(!err.0.is_empty(), "{bad:?} must explain itself");
        }
    }

    #[test]
    fn a_zero_multiple_never_divides_by_zero() {
        // Unreachable through parse(), but the guard must hold if one is built
        // directly.
        assert!(!HitCondition::Multiple(0).fires_at(7));
    }
}
