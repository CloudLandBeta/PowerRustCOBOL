// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Expression and condition parsers.
//!
//! # Grammar (simplified)
//!
//! ```text
//! expr      ::= unary (binop unary)*          -- Pratt
//! unary     ::= ('-' | '+') primary | primary
//! primary   ::= literal | figurative | ident subscript? qualified*
//!             | FUNCTION name '(' args ')' | '(' expr ')'
//!
//! condition ::= and_cond ('OR' and_cond)*
//! and_cond  ::= atom ('AND' atom)*
//! atom      ::= 'NOT' atom | '(' condition ')' | expr cmp_op expr
//!             | expr 'IS' ['NOT'] (class | sign | cmp_kw expr)
//!             | ident   -- condition-name (88-level)
//! ```

use cobolt_ast::expr::{
    ArithOp, CmpOp, Condition, DataClass, Expr, FigurativeConstant, Literal, SignCond, UnaryOp,
};
use cobolt_lexer::{Span, Token};

use crate::parser::Parser;

// ── Literal helpers ───────────────────────────────────────────────────────────

/// Try to parse a figurative constant at the current position.
/// Returns `None` if the current token is not a figurative constant.
pub(crate) fn try_parse_figurative(p: &mut Parser) -> Option<(FigurativeConstant, Span)> {
    let span = p.peek_span();
    match p.peek().clone() {
        Token::Spaces     => { p.advance(); Some((FigurativeConstant::Space,     span)) }
        Token::Zeros      => { p.advance(); Some((FigurativeConstant::Zero,      span)) }
        Token::HighValues => { p.advance(); Some((FigurativeConstant::HighValue, span)) }
        Token::LowValues  => { p.advance(); Some((FigurativeConstant::LowValue,  span)) }
        Token::Quotes     => { p.advance(); Some((FigurativeConstant::Quote,     span)) }
        Token::Nulls      => { p.advance(); Some((FigurativeConstant::Null,      span)) }
        // ALL "x"  — Token::All followed by a literal
        Token::All => {
            p.advance();
            if let Some((lit, _)) = parse_literal_inner(p) {
                Some((FigurativeConstant::All(Box::new(lit)), span))
            } else {
                p.emit_error("expected literal after ALL");
                None
            }
        }
        _ => None,
    }
}

/// Parse a bare (non-figurative) literal: string, integer, or float.
fn parse_literal_inner(p: &mut Parser) -> Option<(Literal, Span)> {
    let span = p.peek_span();
    match p.peek().clone() {
        Token::StringLiteral(s)  => { p.advance(); Some((Literal::String(s),  span)) }
        Token::IntegerLiteral(n) => { p.advance(); Some((Literal::Integer(n), span)) }
        Token::FloatLiteral(f)   => { p.advance(); Some((Literal::Float(f),   span)) }
        _ => None,
    }
}

/// Parse a literal value (figurative constants included).
pub(crate) fn parse_literal(p: &mut Parser) -> Option<(Literal, Span)> {
    if let Some((fc, sp)) = try_parse_figurative(p) {
        return Some((Literal::Figurative(fc), sp));
    }
    parse_literal_inner(p)
}

// ── Expression parser (Pratt) ─────────────────────────────────────────────────

/// Parse a primary expression (leaf node — no binary ops).
fn parse_primary(p: &mut Parser) -> Option<Expr> {
    let span = p.peek_span();

    // Unary minus
    if p.at(&Token::Minus) {
        p.advance();
        let operand = parse_primary(p)?;
        let sp = span.merge(operand.span());
        return Some(Expr::Unary { op: UnaryOp::Neg, operand: Box::new(operand), span: sp });
    }

    // Unary plus (no-op, kept for source fidelity)
    if p.at(&Token::Plus) {
        p.advance();
        let operand = parse_primary(p)?;
        let sp = span.merge(operand.span());
        return Some(Expr::Unary { op: UnaryOp::Pos, operand: Box::new(operand), span: sp });
    }

    // Parenthesised expression
    if p.at(&Token::LParen) {
        p.advance();
        let inner = parse_expr(p);
        p.expect(&Token::RParen);
        return Some(inner);
    }

    // FUNCTION name ( args )
    if p.at(&Token::Function) {
        p.advance();
        let name = p.expect_identifier("FUNCTION name");
        let mut args = Vec::new();
        if p.eat(&Token::LParen) {
            while !p.at(&Token::RParen) && !p.at(&Token::Eof) {
                args.push(parse_expr(p));
                p.eat(&Token::Comma);
            }
            p.expect(&Token::RParen);
        }
        let sp = span.merge(p.peek_span());
        return Some(Expr::FunctionCall { name, args, span: sp });
    }

    // Figurative constants / literals
    if let Some((lit, sp)) = parse_literal(p) {
        return Some(Expr::Literal(lit, sp));
    }

    // Identifier (optionally subscripted and/or qualified)
    if let Some((name, id_span)) = p.eat_identifier() {
        let mut expr = Expr::Identifier(name.clone(), id_span);

        // Subscript: IDENT ( expr , expr … )
        if p.at(&Token::LParen) {
            p.advance();
            let mut indices = Vec::new();
            loop {
                indices.push(parse_expr(p));
                if !p.eat(&Token::Comma) {
                    break;
                }
            }
            p.expect(&Token::RParen);
            let sp = id_span.merge(p.peek_span());
            expr = Expr::Subscript { base: Box::new(expr), indices, span: sp };
        }

        // Qualified: IDENT OF/IN qualifier
        while p.at(&Token::Of) || p.at(&Token::In) {
            p.advance();
            let (qual, qual_span) = p.eat_identifier().unwrap_or_else(|| {
                p.emit_error("expected qualifier name after OF/IN");
                ("<missing>".into(), p.peek_span())
            });
            let inner_name = match &expr {
                Expr::Identifier(n, _) => n.clone(),
                _ => "<qual>".into(),
            };
            let sp = expr.span().merge(qual_span);
            expr = Expr::Qualified {
                name: inner_name,
                of: Box::new(Expr::Identifier(qual, qual_span)),
                span: sp,
            };
        }

        return Some(expr);
    }

    None
}

/// Left/right binding powers for binary arithmetic operators.
fn infix_bp(tok: &Token) -> Option<(u8, u8)> {
    match tok {
        Token::Plus | Token::Minus => Some((1, 2)),
        Token::Star | Token::Slash => Some((3, 4)),
        Token::Power               => Some((6, 5)), // right-associative
        _ => None,
    }
}

fn tok_to_arithop(tok: &Token) -> ArithOp {
    match tok {
        Token::Plus  => ArithOp::Add,
        Token::Minus => ArithOp::Sub,
        Token::Star  => ArithOp::Mul,
        Token::Slash => ArithOp::Div,
        Token::Power => ArithOp::Pow,
        _ => unreachable!(),
    }
}

fn parse_expr_bp(p: &mut Parser, min_bp: u8) -> Expr {
    let mut lhs = match parse_primary(p) {
        Some(e) => e,
        None => {
            let span = p.peek_span();
            p.emit_error(format!("expected expression, found {:?}", p.peek()));
            Expr::Literal(Literal::Integer(0), span)
        }
    };

    loop {
        let tok = p.peek().clone();
        match infix_bp(&tok) {
            Some((l_bp, r_bp)) if l_bp >= min_bp => {
                let op = tok_to_arithop(&tok);
                p.advance();
                let rhs = parse_expr_bp(p, r_bp);
                let sp = lhs.span().merge(rhs.span());
                lhs = Expr::Arithmetic { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span: sp };
            }
            _ => break,
        }
    }
    lhs
}

/// Parse an arithmetic expression.
pub(crate) fn parse_expr(p: &mut Parser) -> Expr {
    parse_expr_bp(p, 0)
}

// ── Condition parser ──────────────────────────────────────────────────────────

fn negate_cmp(op: CmpOp) -> CmpOp {
    match op {
        CmpOp::Eq => CmpOp::Ne,
        CmpOp::Ne => CmpOp::Eq,
        CmpOp::Lt => CmpOp::Ge,
        CmpOp::Le => CmpOp::Gt,
        CmpOp::Gt => CmpOp::Le,
        CmpOp::Ge => CmpOp::Lt,
    }
}

/// If the current token is an identifier, return its name upper-cased.
fn peek_ident_upper(p: &Parser) -> Option<String> {
    if let Token::Identifier(s) = p.peek() {
        Some(s.to_uppercase())
    } else {
        None
    }
}

/// After consuming GREATER or LESS, check for `OR EQUAL [TO]` phrase.
/// Returns `true` and consumes the tokens if present.
fn check_or_equal(p: &mut Parser) -> bool {
    // Only consume OR if the next token after it is EQUAL
    if p.at(&Token::Or) && matches!(p.peek_at(1), Token::Equal) {
        p.advance(); // OR
        p.advance(); // EQUAL
        p.eat(&Token::To);
        return true;
    }
    false
}

/// Parse a single condition atom (possibly preceded by NOT).
fn parse_condition_primary(p: &mut Parser) -> Condition {
    let span = p.peek_span();

    // NOT condition
    if p.at(&Token::Not) {
        p.advance();
        let inner = parse_condition_primary(p);
        let sp = span.merge(inner.span());
        return Condition::Not(Box::new(inner), sp);
    }

    // Parenthesised condition
    if p.at(&Token::LParen) {
        p.advance();
        let cond = parse_condition(p);
        p.expect(&Token::RParen);
        return cond;
    }

    // Parse LHS arithmetic expression
    let lhs = parse_expr(p);

    // IS [NOT] class / sign / keyword-comparison
    if p.at(&Token::Is) {
        p.advance();
        let negated = p.eat(&Token::Not);

        // Class test: NUMERIC, ALPHABETIC, ALPHABETIC-LOWER, ALPHABETIC-UPPER
        if let Some(name) = peek_ident_upper(p) {
            let class = match name.as_str() {
                "NUMERIC"           => Some(DataClass::Numeric),
                "ALPHABETIC"        => Some(DataClass::Alphabetic),
                "ALPHABETIC-LOWER"  => Some(DataClass::AlphabeticLower),
                "ALPHABETIC-UPPER"  => Some(DataClass::AlphabeticUpper),
                _ => None,
            };
            if let Some(c) = class {
                p.advance();
                let sp = span.merge(p.peek_span());
                return Condition::ClassTest { expr: lhs, negated, class: c, span: sp };
            }
        }

        // Sign test: POSITIVE, NEGATIVE (identifiers), ZERO (Token::Zeros)
        if let Some(name) = peek_ident_upper(p) {
            let sign = match name.as_str() {
                "POSITIVE" => Some(SignCond::Positive),
                "NEGATIVE" => Some(SignCond::Negative),
                _ => None,
            };
            if let Some(s) = sign {
                p.advance();
                let sp = span.merge(p.peek_span());
                return Condition::SignTest { expr: lhs, negated, sign: s, span: sp };
            }
        }
        if p.at(&Token::Zeros) {
            p.advance();
            let sp = span.merge(p.peek_span());
            return Condition::SignTest { expr: lhs, negated, sign: SignCond::Zero, span: sp };
        }

        // Keyword comparisons: EQUAL TO, GREATER [OR EQUAL] [THAN], LESS [OR EQUAL] [THAN]
        if p.eat(&Token::Equal) {
            p.eat(&Token::To);
            let rhs = parse_expr(p);
            let op = if negated { CmpOp::Ne } else { CmpOp::Eq };
            let sp = span.merge(rhs.span());
            return Condition::Comparison { lhs, op, rhs, span: sp };
        }
        if p.eat(&Token::Greater) {
            let ge = check_or_equal(p);
            p.eat(&Token::Than);
            let rhs = parse_expr(p);
            let base = if ge { CmpOp::Ge } else { CmpOp::Gt };
            let op = if negated { negate_cmp(base) } else { base };
            let sp = span.merge(rhs.span());
            return Condition::Comparison { lhs, op, rhs, span: sp };
        }
        if p.eat(&Token::Less) {
            let le = check_or_equal(p);
            p.eat(&Token::Than);
            let rhs = parse_expr(p);
            let base = if le { CmpOp::Le } else { CmpOp::Lt };
            let op = if negated { negate_cmp(base) } else { base };
            let sp = span.merge(rhs.span());
            return Condition::Comparison { lhs, op, rhs, span: sp };
        }

        p.emit_error("unrecognised IS clause in condition");
        return Condition::ConditionName("<error>".into(), span);
    }

    // Symbolic comparison operators: =, <>, <, >, <=, >=
    let tok = p.peek().clone();
    if matches!(tok, Token::Eq | Token::NotEq | Token::Lt | Token::Gt | Token::LtEq | Token::GtEq) {
        p.advance();
        let op = match &tok {
            Token::Eq    => CmpOp::Eq,
            Token::NotEq => CmpOp::Ne,
            Token::Lt    => CmpOp::Lt,
            Token::Gt    => CmpOp::Gt,
            Token::LtEq  => CmpOp::Le,
            Token::GtEq  => CmpOp::Ge,
            _ => unreachable!(),
        };
        let rhs = parse_expr(p);
        let sp = span.merge(rhs.span());
        return Condition::Comparison { lhs, op, rhs, span: sp };
    }

    // No comparison operator → treat the expression as a condition-name (88-level).
    match lhs {
        Expr::Identifier(name, s) => Condition::ConditionName(name, s),
        other => {
            p.emit_error("expected comparison operator in condition");
            Condition::ConditionName("<error>".into(), other.span())
        }
    }
}

fn parse_condition_and(p: &mut Parser) -> Condition {
    let mut lhs = parse_condition_primary(p);
    while p.at(&Token::And) {
        p.advance();
        let rhs = parse_condition_primary(p);
        let sp = lhs.span().merge(rhs.span());
        lhs = Condition::And(Box::new(lhs), Box::new(rhs), sp);
    }
    lhs
}

fn parse_condition_or(p: &mut Parser) -> Condition {
    let mut lhs = parse_condition_and(p);
    while p.at(&Token::Or) {
        // Guard: don't consume OR that's part of GREATER/LESS OR EQUAL
        // (those are consumed inside parse_condition_primary before returning)
        p.advance();
        let rhs = parse_condition_and(p);
        let sp = lhs.span().merge(rhs.span());
        lhs = Condition::Or(Box::new(lhs), Box::new(rhs), sp);
    }
    lhs
}

/// Parse a full boolean condition with AND / OR precedence.
/// AND binds more tightly than OR.
pub(crate) fn parse_condition(p: &mut Parser) -> Condition {
    parse_condition_or(p)
}
