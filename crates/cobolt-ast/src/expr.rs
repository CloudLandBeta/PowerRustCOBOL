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
    /// An integer literal whose **written** digit count exceeds the canonical
    /// rendering of its value — i.e. one carrying leading zeros: `060820000200`
    /// is `IntegerDigits(60820000200, 12)`.
    ///
    /// Its *value* is `Integer`'s in every respect; the count exists because a
    /// numeric literal moved to an alphanumeric receiver transfers its
    /// characters **as written**. `MOVE 060820000200 TO <six PIC 99 children>`
    /// fills them with `06 08 20 00 02 00`, which the value alone can no longer
    /// say (NIST CCVS85 NC202A `ADD-TEST-F3-7`).
    ///
    /// A literal that needs no padding stays [`Literal::Integer`], so nothing
    /// that already worked changes shape.
    ///
    /// New variants go at the END — the AST is bincode-serialized by ordinal.
    IntegerDigits(i64, u8),
}

impl Literal {
    /// An integer literal's digits **as the program wrote them**, unsigned.
    ///
    /// This is what a numeric literal contributes to an alphanumeric receiver:
    /// COBOL-85 moves its characters, not its value, so `MOVE 2 TO <PIC X(4)>`
    /// leaves `"2   "` and `MOVE 0012 TO <PIC X(4)>` leaves `"0012"`. The
    /// receiver's width never enters into it — padding to the receiver would
    /// turn the first of those into `"0002"`.
    ///
    /// `None` for every literal that is not an integer.
    pub fn integer_digits(&self) -> Option<String> {
        match self {
            Literal::Integer(n) => Some(n.unsigned_abs().to_string()),
            Literal::IntegerDigits(n, digits) => Some(format!(
                "{:0>width$}",
                n.unsigned_abs(),
                width = *digits as usize
            )),
            _ => None,
        }
    }
}

/// COBOL figurative constants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FigurativeConstant {
    Zero,              // ZERO / ZEROS / ZEROES
    Space,             // SPACE / SPACES
    HighValue,         // HIGH-VALUE / HIGH-VALUES
    LowValue,          // LOW-VALUE / LOW-VALUES
    Quote,             // QUOTE / QUOTES
    Null,              // NULL / NULLS
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
    Concat,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg, // unary minus
    Pos, // unary plus (no-op, kept for fidelity)
}

/// Comparison operators used in `Condition::Comparison`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmpOp {
    Eq, // =  / EQUAL TO
    Ne, // <> / NOT EQUAL TO
    Lt, // <  / LESS THAN
    Le, // <= / LESS THAN OR EQUAL TO
    Gt, // >  / GREATER THAN
    Ge, // >= / GREATER THAN OR EQUAL TO
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

    /// One `::member` access in a chain (RustCOBOL OO style):
    ///   `Label-1::Caption` · `Label-1::GetText()` · `Grid-1::Rows(I)::Cols(2)::Value`
    ///
    /// The receiver `recv` is the root control [`Expr::Identifier`] or another
    /// `Member` (the chain is left-recursive). `parens` records whether `()` was
    /// written: it distinguishes a **property** (`::Value`, no parens — readable
    /// *and* an assignable lvalue) from a **call / subscript** (`::Method()` or
    /// `::Items(4)`, parens present — an rvalue, never a receiving field). `args`
    /// carries the subscript indices or the call arguments; which one it is is
    /// resolved at runtime from the member's kind (a collection ⇒ index, a method
    /// ⇒ call). A trailing-call chain is also valid as a statement
    /// ([`crate::stmt::Stmt::Invoke`]).
    Member {
        recv: Box<Expr>,
        member: String,
        args: Vec<Expr>,
        parens: bool,
        span: Span,
    },

    // ⚠️ **NEW VARIANTS GO AT THE END — never in the middle.**
    //
    // This enum is serialized with `bincode` and embedded in every compiled
    // binary (`cobolt-compiler`: lex → parse → semantic → bincode → deflate →
    // `include_bytes!`). bincode identifies a variant by its **ordinal**, so
    // inserting one renumbers every variant after it: an AST written before the
    // change is then read as the wrong variants, and the misaligned stream
    // surfaces as `invalid value: integer N, expected variant index 0 <= i < M`
    // when a built application starts.
    //
    // That is not hypothetical — `AllSubscript` was first added after
    // `Subscript`, which shifted `RefMod`, `FunctionCall`, `Arithmetic`,
    // `Unary` and `Member` by one and broke an already-built demo. Appending
    // keeps every existing ordinal, so only the new variant is unreadable by an
    // older binary, which is the honest and expected limit.
    /// The reserved word `ALL` used **as a subscript**, meaning *every*
    /// occurrence of the table in that dimension:
    ///
    /// ```cobol
    /// COMPUTE WS-NUM = FUNCTION MAX(IND(ALL)).
    /// COMPUTE WS-NUM = FUNCTION SUM(TBL(ALL, 2)).
    /// ```
    ///
    /// It is only meaningful inside [`Expr::Subscript::indices`] of an
    /// intrinsic-function argument, where the caller expands it to one
    /// argument per occurrence. It is **not** the figurative constant `ALL "X"`
    /// — that is a [`Literal`], and the two are told apart by position: inside
    /// a subscript list `ALL` is always this, because a figurative constant is
    /// never a legal subscript.
    AllSubscript(Span),
}

impl Expr {
    /// Return the span of this expression node.
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal(_, s) => *s,
            Expr::Identifier(_, s) => *s,
            Expr::Qualified { span, .. } => *span,
            Expr::Subscript { span, .. } => *span,
            Expr::AllSubscript(s) => *s,
            Expr::RefMod { span, .. } => *span,
            Expr::FunctionCall { span, .. } => *span,
            Expr::Arithmetic { span, .. } => *span,
            Expr::Unary { span, .. } => *span,
            Expr::Member { span, .. } => *span,
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

    /// A condition-name reached through a **reference** rather than a bare
    /// word: subscripted (`IF FIRSTZ (1)`) or qualified (`IF A OF IF-D32`).
    /// The expression names the 88-level item; its subscripts and qualifiers
    /// pick the occurrence of the host item the condition is tested against.
    ///
    /// 🔴 New variants belong at the END of this enum. `Condition` is
    /// bincode-serialized and a variant is identified by its **ordinal**, so
    /// inserting one in the middle renumbers every variant after it and an
    /// already-built binary decodes the wrong arm.
    ConditionRef(Box<Expr>, Span),

    /// `IF item IS [NOT] class-name`, where `class-name` was declared by a
    /// `CLASS` clause in SPECIAL-NAMES. It is not one of the four built-in
    /// classes [`DataClass`] names — the program defines which characters
    /// belong — so it carries the name and is resolved against
    /// `Program::classes` at run time.
    UserClassTest {
        expr: Expr,
        negated: bool,
        class: String,
        span: Span,
    },
}

impl Condition {
    pub fn span(&self) -> Span {
        match self {
            Condition::Comparison { span, .. } => *span,
            Condition::Not(_, s) => *s,
            Condition::And(_, _, s) => *s,
            Condition::Or(_, _, s) => *s,
            Condition::ClassTest { span, .. } => *span,
            Condition::SignTest { span, .. } => *span,
            Condition::ConditionName(_, s) => *s,
            Condition::NameOrAbbrev { span, .. } => *span,
            Condition::ConditionRef(_, s) => *s,
            Condition::UserClassTest { span, .. } => *span,
        }
    }
}
