// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The two value types a PartiQL statement over an indexed file knows: a
//! Number (exact, with its scale — never a float) and a String.

use std::cmp::Ordering;

use crate::value::CobolNumeric;

/// A value of a field, a literal or a parameter (spec 108, §5.2).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A numeric field's value with its scale: `PIC S9(6)V99` holding
    /// `0012345{` is `Num(-123.45)`.
    Num(CobolNumeric),
    /// An alphanumeric, alphabetic, national or UTF-8 field's content without
    /// trailing spaces.
    Str(String),
}

/// A comparison or an arithmetic operation between a Number and a String.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeMismatch;

impl Value {
    /// A Number from an integer.
    pub fn int(n: i64) -> Value {
        Value::Num(CobolNumeric::integer(n))
    }

    /// A String, with the trailing spaces a fixed field would add removed.
    pub fn text(s: &str) -> Value {
        Value::Str(s.trim_end_matches(' ').to_string())
    }

    pub fn is_num(&self) -> bool {
        matches!(self, Value::Num(_))
    }

    /// The compare of §5.2: Number with Number (exact, scale-normalised),
    /// String with String (by character), anything else a mismatch.
    pub fn compare(&self, other: &Value) -> Result<Ordering, TypeMismatch> {
        match (self, other) {
            (Value::Num(a), Value::Num(b)) => Ok(a.cmp(b)),
            (Value::Str(a), Value::Str(b)) => Ok(a.cmp(b)),
            _ => Err(TypeMismatch),
        }
    }

    /// `+`: Number plus Number. A String has no arithmetic.
    pub fn add(&self, other: &Value) -> Result<Value, TypeMismatch> {
        match (self, other) {
            (Value::Num(a), Value::Num(b)) => Ok(Value::Num(a.add(b))),
            _ => Err(TypeMismatch),
        }
    }

    /// `-`: Number minus Number.
    pub fn sub(&self, other: &Value) -> Result<Value, TypeMismatch> {
        match (self, other) {
            (Value::Num(a), Value::Num(b)) => Ok(Value::Num(a.sub(b))),
            _ => Err(TypeMismatch),
        }
    }

    /// The text a result set shows: a Number with its decimal point
    /// (`-123.45`), a String as it is.
    pub fn display(&self) -> String {
        match self {
            Value::Num(n) => n.to_string(),
            Value::Str(s) => s.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_compare_by_value_not_by_scale() {
        let a = Value::Num(CobolNumeric::new(1050, 2));
        let b = Value::Num(CobolNumeric::new(105, 1));
        assert_eq!(a.compare(&b), Ok(Ordering::Equal));
        assert_eq!(a.compare(&Value::int(11)), Ok(Ordering::Less));
    }

    #[test]
    fn a_number_is_never_compared_with_a_string() {
        assert_eq!(Value::int(1).compare(&Value::text("1")), Err(TypeMismatch));
        assert_eq!(Value::text("a").add(&Value::int(1)), Err(TypeMismatch));
    }

    #[test]
    fn arithmetic_is_exact() {
        let a = Value::Num(CobolNumeric::new(1010, 2));
        let b = Value::Num(CobolNumeric::new(5, 1));
        assert_eq!(a.sub(&b).unwrap().display(), "9.60");
        assert_eq!(a.add(&b).unwrap().display(), "10.60");
    }

    #[test]
    fn text_drops_trailing_spaces_only() {
        assert_eq!(Value::text("  Ana  "), Value::Str("  Ana".into()));
    }
}
