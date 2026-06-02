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
}

impl Expr {
    /// Return the span of this expression node.
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal(_, s)          => *s,
            Expr::Identifier(_, s)       => *s,
            Expr::Qualified { span, .. } => *span,
            Expr::Subscript { span, .. } => *span,
            Expr::FunctionCall { span, .. } => *span,
            Expr::Arithmetic { span, .. } => *span,
            Expr::Unary { span, .. }     => *span,
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
        }
    }
}
