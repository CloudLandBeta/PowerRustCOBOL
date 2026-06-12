// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Expression and condition node types.

use cobolt_lexer::Span;
use serde::{Deserialize, Serialize};

// ── Literals ──────────────────────────────────────────────────────────────────

/// A compile-time literal value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    String(String),
    Integer(i64),
    Float(f64),
    /// Exact fixed-point decimal: `value = mantissa × 10^(-scale)`.
    /// e.g. `3.14` → `Decimal(314, 2)`. Preserves up to 31 significant digits.
    Decimal(i128, u8),
    Figurative(FigurativeConstant),
}

/// COBOL figurative constants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FigurativeConstant {
    Zero,       // ZERO / ZEROS / ZEROES
    Space,      // SPACE / SPACES
    HighValue,  // HIGH-VALUE / HIGH-VALUES
    LowValue,   // LOW-VALUE / LOW-VALUES
    Quote,      // QUOTE / QUOTES
    Null,       // NULL / NULLS
    All(Box<Literal>), // ALL literal
}

// ── Arithmetic & comparison operators ────────────────────────────────────────

/// Binary arithmetic operators (used inside `Expr::Arithmetic`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,  // unary minus
    Pos,  // unary plus (no-op, kept for fidelity)
}

/// Comparison operators used in `Condition::Comparison`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmpOp {
    Eq,  // =  / EQUAL TO
    Ne,  // <> / NOT EQUAL TO
    Lt,  // <  / LESS THAN
    Le,  // <= / LESS THAN OR EQUAL TO
    Gt,  // >  / GREATER THAN
    Ge,  // >= / GREATER THAN OR EQUAL TO
}

// ── Expressions ───────────────────────────────────────────────────────────────

/// An expression that evaluates to a value.
///
/// Most COBOL "receiving fields" and "sending fields" in statements are
/// `Expr`s — identifiers, subscripted items, literals, or arithmetic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    /// A literal constant.
    Literal(Literal, Span),

    /// A simple data-item name.
    Identifier(String, Span),

    /// Qualified name: `A OF B` or `A IN B`.
    Qualified {
        name: String,
        of: Box<Expr>,
        span: Span,
    },

    /// Subscripted table reference: `TABLE-ITEM(1)` or `TABLE-ITEM(WS-IDX)`.
    Subscript {
        base: Box<Expr>,
        indices: Vec<Expr>,
        span: Span,
    },

    /// Reference modification: `data-ref(start:[length])` — the `length` bytes of
    /// `base` starting at 1-based byte `start` (to end of field when omitted).
    RefMod {
        base: Box<Expr>,
        start: Box<Expr>,
        length: Option<Box<Expr>>,
        span: Span,
    },

    /// Intrinsic function call: `FUNCTION LENGTH(WS-NAME)`.
    FunctionCall {
        name: String,
        args: Vec<Expr>,
        span: Span,
    },

    /// Binary arithmetic expression.
    Arithmetic {
        op: ArithOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },

    /// Unary arithmetic expression.
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
        span: Span,
    },

    /// Visual-object **property reference** (PowerCOBOL-style), usable as a
    /// sending or receiving operand:
    ///   `"Caption" OF CmStatic1`
    ///   `"Text" OF "ListItems" (4) OF Listview1`
    /// `control` is the rightmost name (the control); `path` is the property
    /// chain from the control outward, so its **last** element is the property
    /// actually read/written. Property names are quoted string literals; an
    /// element may carry a 1-based subscript.
    PropertyRef {
        control: String,
        path: Vec<PropSeg>,
        span: Span,
    },

    /// Visual-object **method call** as an expression (PowerCOBOL OO style):
    ///   `Label-1::GetText()`  ·  `CheckBox-1::IsChecked()`
    /// Used where a value is needed (e.g. `MOVE obj::GetText() TO X`). The same
    /// form is also a statement ([`crate::stmt::Stmt::Invoke`]).
    MethodCall {
        object: String,
        method: String,
        args: Vec<Expr>,
        span: Span,
    },
}

/// One segment of a [`Expr::PropertyRef`] path: a property/collection name with
/// an optional 1-based subscript (`"ListItems" (4)`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropSeg {
    pub name: String,
    pub index: Option<Box<Expr>>,
}

impl Expr {
    /// Return the span of this expression node.
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal(_, s)          => *s,
            Expr::Identifier(_, s)       => *s,
            Expr::Qualified { span, .. } => *span,
            Expr::Subscript { span, .. } => *span,
            Expr::RefMod { span, .. }    => *span,
            Expr::FunctionCall { span, .. } => *span,
            Expr::Arithmetic { span, .. } => *span,
            Expr::Unary { span, .. }     => *span,
            Expr::PropertyRef { span, .. } => *span,
            Expr::MethodCall { span, .. } => *span,
        }
    }
}

// ── Conditions ────────────────────────────────────────────────────────────────

/// The class of a data item tested with `IF x IS NUMERIC`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataClass {
    Numeric,
    Alphabetic,
    AlphabeticLower,
    AlphabeticUpper,
}

/// The sign tested with `IF x IS POSITIVE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignCond {
    Positive,
    Negative,
    Zero,
}

/// A boolean condition — the argument to IF, EVALUATE, PERFORM UNTIL, etc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    /// `A = B`, `A > B`, etc.
    Comparison {
        lhs: Expr,
        op: CmpOp,
        rhs: Expr,
        span: Span,
    },

    /// `NOT condition`
    Not(Box<Condition>, Span),

    /// `condition-1 AND condition-2`
    And(Box<Condition>, Box<Condition>, Span),

    /// `condition-1 OR condition-2`
    Or(Box<Condition>, Box<Condition>, Span),

    /// `IF x IS NUMERIC / ALPHABETIC / …`
    ClassTest {
        expr: Expr,
        negated: bool,
        class: DataClass,
        span: Span,
    },

    /// `IF x IS POSITIVE / NEGATIVE / ZERO`
    SignTest {
        expr: Expr,
        negated: bool,
        sign: SignCond,
        span: Span,
    },

    /// A condition-name (88-level item) used directly as a condition.
    ConditionName(String, Span),

    /// An abbreviated combined relation with a bare operand object that the
    /// parser cannot disambiguate without the symbol table: in `a = b OR c`,
    /// the `c` is either an 88-level condition-name or the object of `a = c`.
    /// Resolved at runtime — if `name` is a known condition-name it is evaluated
    /// as one, otherwise as `subject op name`.
    NameOrAbbrev {
        subject: Box<Expr>,
        op: CmpOp,
        name: String,
        span: Span,
    },
}

impl Condition {
    pub fn span(&self) -> Span {
        match self {
            Condition::Comparison { span, .. } => *span,
            Condition::Not(_, s)               => *s,
            Condition::And(_, _, s)            => *s,
            Condition::Or(_, _, s)             => *s,
            Condition::ClassTest { span, .. }  => *span,
            Condition::SignTest { span, .. }   => *span,
            Condition::ConditionName(_, s)     => *s,
            Condition::NameOrAbbrev { span, .. } => *span,
        }
    }
}
