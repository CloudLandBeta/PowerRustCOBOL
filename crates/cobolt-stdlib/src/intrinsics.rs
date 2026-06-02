// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Additional COBOL intrinsic functions.
//!
//! The core interpreter handles the most common intrinsics inline
//! (`LENGTH`, `UPPER-CASE`, `LOWER-CASE`, `NUMVAL`, `MAX`, `MIN`, `SQRT`,
//! `MOD`, `RANDOM`, `CURRENT-DATE`).  This module provides a callable table
//! for the rest and can be consulted for any name not recognised by the
//! interpreter.
//!
//! # Usage
//!
//! ```rust,no_run
//! use cobolt_stdlib::intrinsics::call_intrinsic;
//! use cobolt_runtime::value::CobolValue;
//!
//! let result = call_intrinsic("TRIM", &[CobolValue::from_str("  hi  ", 6)]);
//! ```

use cobolt_runtime::value::CobolValue;

/// Invoke a named intrinsic function with the supplied argument list.
///
/// Returns `None` if the function name is unknown (the caller should then
/// emit a warning and return zero/spaces).
pub fn call_intrinsic(name: &str, args: &[CobolValue]) -> Option<CobolValue> {
    match name.to_ascii_uppercase().as_str() {
        // ── String functions ──────────────────────────────────────────────────
        "TRIM" => {
            let s = first_str(args).trim().to_owned();
            let len = s.len();
            Some(CobolValue::from_str(&s, len.max(1)))
        }
        "TRIM-LEADING" => {
            let s = first_str(args).trim_start().to_owned();
            let len = s.len();
            Some(CobolValue::from_str(&s, len.max(1)))
        }
        "TRIM-TRAILING" => {
            let s = first_str(args).trim_end().to_owned();
            let len = s.len();
            Some(CobolValue::from_str(&s, len.max(1)))
        }
        "REVERSE" => {
            let s: String = first_str(args).chars().rev().collect();
            let len = s.len();
            Some(CobolValue::from_str(&s, len.max(1)))
        }
        "CONCATENATE" => {
            let s: String = args.iter().map(|v| v.as_display_string()).collect();
            let len = s.len();
            Some(CobolValue::from_str(&s, len.max(1)))
        }
        "SPACE-USAGE" => {
            // Returns the length of trailing spaces stripped.
            let s = first_str(args);
            let trimmed = s.trim_end();
            Some(CobolValue::from_i64((s.len() - trimmed.len()) as i64))
        }

        // ── Numeric functions ─────────────────────────────────────────────────
        "ABS"          => Some(CobolValue::from_f64(first_f64(args).abs())),
        "ACOS"         => Some(CobolValue::from_f64(first_f64(args).acos())),
        "ASIN"         => Some(CobolValue::from_f64(first_f64(args).asin())),
        "ATAN"         => Some(CobolValue::from_f64(first_f64(args).atan())),
        "COS"          => Some(CobolValue::from_f64(first_f64(args).cos())),
        "SIN"          => Some(CobolValue::from_f64(first_f64(args).sin())),
        "TAN"          => Some(CobolValue::from_f64(first_f64(args).tan())),
        "SQRT"         => Some(CobolValue::from_f64(first_f64(args).sqrt())),
        "LOG"          => Some(CobolValue::from_f64(first_f64(args).ln())),
        "LOG10"        => Some(CobolValue::from_f64(first_f64(args).log10())),
        "EXP"          => Some(CobolValue::from_f64(first_f64(args).exp())),
        "INTEGER"      => Some(CobolValue::from_i64(first_f64(args).floor() as i64)),
        "INTEGER-PART" => Some(CobolValue::from_i64(first_f64(args).trunc() as i64)),
        "FACTORIAL" => {
            let n = first_f64(args) as u64;
            Some(CobolValue::from_i64(factorial(n) as i64))
        }
        "MEAN" => {
            if args.is_empty() { return Some(CobolValue::from_f64(0.0)); }
            let sum: f64 = args.iter().map(|v| v.as_f64()).sum();
            Some(CobolValue::from_f64(sum / args.len() as f64))
        }
        "MEDIAN" => {
            if args.is_empty() { return Some(CobolValue::from_f64(0.0)); }
            let mut vals: Vec<f64> = args.iter().map(|v| v.as_f64()).collect();
            vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mid = vals.len() / 2;
            let med = if vals.len() % 2 == 0 {
                (vals[mid - 1] + vals[mid]) / 2.0
            } else {
                vals[mid]
            };
            Some(CobolValue::from_f64(med))
        }
        "VARIANCE" => {
            if args.len() < 2 { return Some(CobolValue::from_f64(0.0)); }
            let vals: Vec<f64> = args.iter().map(|v| v.as_f64()).collect();
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            let var = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
                / vals.len() as f64;
            Some(CobolValue::from_f64(var))
        }
        "STANDARD-DEVIATION" => {
            let var = call_intrinsic("VARIANCE", args)?.as_f64();
            Some(CobolValue::from_f64(var.sqrt()))
        }
        "SUM" => {
            let sum: f64 = args.iter().map(|v| v.as_f64()).sum();
            Some(CobolValue::from_f64(sum))
        }
        "PI"   => Some(CobolValue::from_f64(std::f64::consts::PI)),
        "E"    => Some(CobolValue::from_f64(std::f64::consts::E)),

        // ── Date functions ────────────────────────────────────────────────────
        "CURRENT-DATE" => {
            // 21-char: YYYYMMDDHHMMSSCC+HHMM
            let s = crate::intrinsics::current_date_21();
            Some(CobolValue::from_str(&s, 21))
        }
        "DATE-OF-INTEGER" => {
            // COBOL integer date (days since 1601-01-01) → YYYYMMDD
            let n = first_f64(args) as i64;
            let s = cobol_integer_date_to_string(n);
            Some(CobolValue::from_str(&s, 8))
        }
        "INTEGER-OF-DATE" => {
            // YYYYMMDD → COBOL integer date
            let s = first_str(args);
            let n = string_to_cobol_integer_date(&s);
            Some(CobolValue::from_i64(n))
        }

        _ => None,
    }
}

/// Returns true if the given name is a known intrinsic function.
pub fn is_known(name: &str) -> bool {
    call_intrinsic(name, &[]).is_some()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn first_str(args: &[CobolValue]) -> String {
    args.first().map(|v| v.as_display_string()).unwrap_or_default()
}

fn first_f64(args: &[CobolValue]) -> f64 {
    args.first().map(|v| v.as_f64()).unwrap_or(0.0)
}

fn factorial(n: u64) -> u64 {
    (1..=n.min(20)).product()
}

fn current_date_21() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let date = days_to_yyyymmdd(days);
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{date}{h:02}{m:02}{s:02}00+0000")
}

fn days_to_yyyymmdd(mut days: u64) -> String {
    let mut year = 1970u64;
    loop {
        let dy = if is_leap(year) { 366 } else { 365 };
        if days < dy { break; }
        days -= dy;
        year += 1;
    }
    let md = if is_leap(year) {
        [31u64, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31u64, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    for days_in_m in &md {
        if days < *days_in_m { break; }
        days -= days_in_m;
        month += 1;
    }
    format!("{year:04}{month:02}{:02}", days + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Very simplified COBOL integer date (days since 1601-01-01 in the Gregorian
/// calendar, as per the COBOL 2002 standard).
fn cobol_integer_date_to_string(n: i64) -> String {
    // Approximate: treat as days since Unix epoch offset by a constant.
    // 1601-01-01 is 134774 days before 1970-01-01.
    let unix_days = (n - 134774).max(0) as u64;
    days_to_yyyymmdd(unix_days)
}

fn string_to_cobol_integer_date(s: &str) -> i64 {
    // Parse YYYYMMDD, approximate.
    if s.len() < 8 { return 0; }
    let year: u64 = s[0..4].parse().unwrap_or(1970);
    let month: u64 = s[4..6].parse().unwrap_or(1);
    let day: u64 = s[6..8].parse().unwrap_or(1);
    // Days since 1970-01-01 (very rough).
    let y_days = (year.saturating_sub(1970)) * 365
        + (year.saturating_sub(1970)) / 4;
    let m_days: u64 = [0u64, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334]
        .get(month.saturating_sub(1) as usize)
        .copied()
        .unwrap_or(0);
    let unix_days = y_days + m_days + day.saturating_sub(1);
    // Add offset to COBOL integer date origin (1601-01-01).
    (unix_days + 134774) as i64
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_removes_spaces() {
        let v = call_intrinsic("TRIM", &[CobolValue::from_str("  hello  ", 9)]).unwrap();
        assert_eq!(v.as_display_string(), "hello");
    }

    #[test]
    fn abs_of_negative() {
        let v = call_intrinsic("ABS", &[CobolValue::from_f64(-3.5)]).unwrap();
        assert!((v.as_f64() - 3.5).abs() < 1e-9);
    }

    #[test]
    fn mean_of_values() {
        let args = vec![
            CobolValue::from_i64(10),
            CobolValue::from_i64(20),
            CobolValue::from_i64(30),
        ];
        let v = call_intrinsic("MEAN", &args).unwrap();
        assert!((v.as_f64() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn unknown_returns_none() {
        assert!(call_intrinsic("NONEXISTENT", &[]).is_none());
    }
}
