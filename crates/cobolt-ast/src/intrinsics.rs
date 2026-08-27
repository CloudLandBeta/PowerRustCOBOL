// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The set of intrinsic functions RustCOBOL implements.
//!
//! # Why the list lives here
//!
//! Two crates need it and neither can see the other's copy. The semantic
//! analyser must reject `FUNCTION NO-SUCH-THING(1)` at **compile** time, and
//! the runtime is what actually implements each function — but
//! `cobolt-semantic` cannot depend on `cobolt-runtime` (`cobolt-stdlib`
//! already does, so it would cycle). `cobolt-ast` is the crate both of them
//! already depend on, and the *vocabulary* of the language is a reasonable
//! thing for the AST crate to define, next to the `Expr::FunctionCall` node
//! that carries the name.
//!
//! # Keeping it honest
//!
//! A list that can drift from the implementation is worse than no list: it
//! would reject a function that works, or admit one that silently returns
//! zero. `cobolt-runtime`'s `intrinsic_names_match_the_implementation` test
//! asserts that every name here is handled by `eval_function`, so the two
//! cannot disagree without a test going red.

/// Every intrinsic function `Interpreter::eval_function` implements.
///
/// COBOL-85 standard set. Names are upper-case; look-ups fold case.
pub const INTRINSIC_FUNCTIONS: &[&str] = &[
    "ABS",
    "ACOS",
    "ANNUITY",
    "ASIN",
    "ATAN",
    "BYTE-LENGTH",
    "CHAR",
    "CONCATENATE",
    "COS",
    "CURRENT-DATE",
    "DATE-OF-INTEGER",
    "DAY-OF-INTEGER",
    "EXP",
    "EXP10",
    "FACTORIAL",
    "FRACTION-PART",
    "INTEGER",
    "INTEGER-OF-DATE",
    "INTEGER-OF-DAY",
    "INTEGER-PART",
    "LENGTH",
    "LENGTH-AN",
    "LOG",
    "LOG10",
    "LOWER-CASE",
    "MAX",
    "MEAN",
    "MEDIAN",
    "MIDRANGE",
    "MIN",
    "MOD",
    "NUMVAL",
    "NUMVAL-C",
    "NUMVAL-F",
    "ORD",
    "ORD-MAX",
    "ORD-MIN",
    "PI",
    "PRESENT-VALUE",
    "RANDOM",
    "RANGE",
    "REM",
    "REVERSE",
    "SIN",
    "SQRT",
    "STANDARD-DEVIATION",
    "STORED-CHAR-LENGTH",
    "SUM",
    "TAN",
    "TEST-NUMVAL",
    "TRIM",
    "UPPER-CASE",
    "VARIANCE",
    "WHEN-COMPILED",
    "YEAR-TO-YYYY",
];

/// True when `name` is an intrinsic function RustCOBOL implements.
pub fn is_intrinsic(name: &str) -> bool {
    INTRINSIC_FUNCTIONS
        .iter()
        .any(|f| f.eq_ignore_ascii_case(name))
}

/// The implemented function whose name is closest to `name`, when one is close
/// enough to be worth suggesting.
///
/// A misspelling is far more likely than a genuinely unknown function, so the
/// diagnostic is much more useful when it can say *did you mean*. The distance
/// budget scales with the length of the name so that `SQRTT` suggests `SQRT`
/// while a completely unrelated word suggests nothing at all — a wrong
/// suggestion is worse than none.
pub fn closest_intrinsic(name: &str) -> Option<&'static str> {
    let upper = name.to_ascii_uppercase();
    let budget = match upper.len() {
        0..=3 => 1,
        4..=7 => 2,
        _ => 3,
    };
    INTRINSIC_FUNCTIONS
        .iter()
        .map(|f| (*f, edit_distance(&upper, f)))
        .filter(|(_, d)| *d <= budget)
        .min_by_key(|(f, d)| (*d, f.len()))
        .map(|(f, _)| f)
}

/// Levenshtein distance, two rows at a time.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_names_are_recognised_in_any_case() {
        assert!(is_intrinsic("MAX"));
        assert!(is_intrinsic("max"));
        assert!(is_intrinsic("Current-Date"));
        assert!(is_intrinsic("STANDARD-DEVIATION"));
    }

    #[test]
    fn unknown_names_are_not() {
        assert!(!is_intrinsic("NO-SUCH-THING"));
        assert!(!is_intrinsic(""));
    }

    #[test]
    fn a_near_miss_suggests_the_real_name() {
        assert_eq!(closest_intrinsic("SQRTT"), Some("SQRT"));
        assert_eq!(closest_intrinsic("UPPERCASE"), Some("UPPER-CASE"));
        assert_eq!(closest_intrinsic("LENGHT"), Some("LENGTH"));
    }

    /// A wrong suggestion is worse than none — an unrelated word gets silence.
    #[test]
    fn something_unrelated_suggests_nothing() {
        assert_eq!(closest_intrinsic("COMPUTE-PAYROLL-TOTAL"), None);
        assert_eq!(closest_intrinsic("ZZZZZZZZ"), None);
    }

    #[test]
    fn the_list_is_sorted_and_has_no_duplicates() {
        let mut sorted = INTRINSIC_FUNCTIONS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.as_slice(),
            INTRINSIC_FUNCTIONS,
            "keep the list sorted and duplicate-free so additions are easy to review"
        );
    }
}
