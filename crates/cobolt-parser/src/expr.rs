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
        Token::Spaces => {
            p.advance();
            Some((FigurativeConstant::Space, span))
        }
        Token::Zeros => {
            p.advance();
            Some((FigurativeConstant::Zero, span))
        }
        Token::HighValues => {
            p.advance();
            Some((FigurativeConstant::HighValue, span))
        }
        Token::LowValues => {
            p.advance();
            Some((FigurativeConstant::LowValue, span))
        }
        Token::Quotes => {
            p.advance();
            Some((FigurativeConstant::Quote, span))
        }
        Token::Nulls => {
            p.advance();
            Some((FigurativeConstant::Null, span))
        }
        // ALL "x"  — Token::All followed by a literal
        Token::All => {
            p.advance();
            // `ALL` in front of another figurative constant is **redundant**:
            // a figurative constant already fills its receiver, so `ALL ZEROS`
            // means `ZEROS` and `ALL SPACES` means `SPACES`. COBOL-85 permits
            // the spelling and ignores the `ALL`; it used to be rejected with
            // "expected literal after ALL", because only a real literal was
            // accepted here.
            if let Some(fig) = match p.peek() {
                Token::Spaces => Some(FigurativeConstant::Space),
                Token::Zeros => Some(FigurativeConstant::Zero),
                Token::HighValues => Some(FigurativeConstant::HighValue),
                Token::LowValues => Some(FigurativeConstant::LowValue),
                Token::Quotes => Some(FigurativeConstant::Quote),
                Token::Nulls => Some(FigurativeConstant::Null),
                _ => None,
            } {
                p.advance();
                return Some((fig, span));
            }
            if let Some((lit, _)) = parse_literal_inner(p) {
                Some((FigurativeConstant::All(Box::new(lit)), span))
            } else {
                p.emit_error("expected a literal or figurative constant after ALL");
                None
            }
        }
        _ => None,
    }
}

// ── Decimal-point assembly ────────────────────────────────────────────────────
//
// COBOL writes the parts of one numeric literal as separate tokens whenever the
// decimal point is not the lexer's own `9.99` shape: `123,45` under
// `DECIMAL-POINT IS COMMA`, and `.5` in every dialect. Both are reassembled the
// same way — by **adjacency**. COBOL-85 requires a space after a separator
// comma and after a sentence-ending period, so "no gap between these tokens" is
// exactly what tells a decimal point from punctuation.

/// Is the token at `offset` glued to the one before it — no space between them?
fn glued(p: &Parser, offset: usize) -> bool {
    debug_assert!(offset > 0, "nothing precedes offset 0");
    p.peek_span_at(offset).start == p.peek_span_at(offset - 1).end
}

/// True when the current `.` opens a numeric literal rather than ending the
/// sentence — `.499` in `SUBTRACT A B .499 FROM C`.
///
/// COBOL-85 requires a space after a sentence-ending period, so a period glued
/// to its digits can only be a decimal point. Statement parsers that bound an
/// operand list with `Token::Period` must ask this first, or they stop at the
/// literal and read the rest of the statement as a new sentence.
pub(crate) fn at_leading_decimal_point(p: &Parser) -> bool {
    matches!(p.peek(), Token::Period)
        && glued(p, 1)
        && matches!(
            p.peek_at(1),
            Token::IntegerLiteral(_) | Token::LevelNumber(_)
        )
}

/// Read the token at `offset` as a run of decimal digits: its value, and **how
/// many digits were written**.
///
/// The width has to come from the span: the token has already parsed its text,
/// so `00001` arrives as the value `1` with no memory of the four leading
/// zeros — and those zeros are the difference between `.00001` and `.1`.
///
/// `LevelNumber` is accepted as well as `IntegerLiteral` because the lexer
/// treats a period as a line start (`lexer.rs`), so the digits right after a
/// decimal point are offered to the level-number rule: `.1` arrives as
/// `LevelNumber(1)` and `.09` as `LevelNumber(9)`, while `.999` arrives as
/// `IntegerLiteral(999)`.
fn digit_run(p: &Parser, offset: usize) -> Option<(i128, u8)> {
    let value = match p.peek_at(offset) {
        Token::IntegerLiteral(n) => *n as i128,
        Token::LevelNumber(n) => *n as i128,
        _ => return None,
    };
    let sp = p.peek_span_at(offset);
    Some((value, (sp.end - sp.start) as u8))
}

/// Combine an integer part with a fractional digit run: `12` + `(345, 3)` → the
/// exact fixed-point decimal `12.345`.
fn join_decimal(int_part: i128, frac: i128, scale: u8) -> Literal {
    let mantissa = int_part * 10_i128.pow(scale as u32) + frac;
    Literal::Decimal(mantissa, scale)
}

/// Parse a bare (non-figurative) literal: string, integer, or float.
fn parse_literal_inner(p: &mut Parser) -> Option<(Literal, Span)> {
    let span = p.peek_span();
    match p.peek().clone() {
        Token::StringLiteral(s) => {
            p.advance();
            Some((Literal::String(s), span))
        }
        Token::IntegerLiteral(n) => {
            // Under DECIMAL-POINT IS COMMA, `123,45` is one decimal literal:
            // an integer, an *adjacent* comma, and an *adjacent* integer (no
            // spaces — a comma followed by a space is still a separator).
            if p.decimal_comma {
                if matches!(p.peek_at(1), Token::Comma) && glued(p, 1) && glued(p, 2) {
                    if let Some((frac, scale)) = digit_run(p, 2) {
                        p.advance(); // integer
                        p.advance(); // comma
                        p.advance(); // fractional digits
                        return Some((join_decimal(n as i128, frac, scale), span));
                    }
                }
            } else if let (Token::Comma, Token::IntegerLiteral(frac)) =
                (p.peek_at(1).clone(), p.peek_at(2).clone())
            {
                // The SAME adjacency, with the clause absent. `8,49` is then
                // neither a numeric literal (the decimal point is `.` here) nor
                // a separator comma (COBOL-85 requires a space after one), so
                // it can only be a comma decimal written without declaring the
                // convention. Taking the `8` and dropping `,49` silently gave
                // the item a wrong value that nothing reported — say what is
                // wrong and, above all, where to fix it.
                let int_end = p.peek_span().end;
                let comma_sp = p.peek_span_at(1);
                let frac_sp = p.peek_span_at(2);
                if comma_sp.start == int_end && frac_sp.start == comma_sp.end {
                    p.emit_error(format!(
                        "'{n},{frac}' reads as a comma decimal separator, but this \
                         compilation unit does not declare it. A comma is only a \
                         decimal point under `SPECIAL-NAMES. DECIMAL-POINT IS COMMA.`, \
                         which COBOL-85 allows only in the OUTERMOST program — on the \
                         form, not in a nested handler or procedure. Add it there, or \
                         write the literal as '{n}.{frac}'."
                    ));
                }
            }
            p.advance();
            Some((Literal::Integer(n), span))
        }
        Token::DecimalLiteral { mantissa, scale } => {
            p.advance();
            Some((Literal::Decimal(mantissa, scale), span))
        }

        // A number that OPENED a line, where an operand was meant.
        //
        // The lexer classifies a number at the start of a line as a level
        // number — and it usually is one, which is why the classification
        // exists — but a statement whose operand list spills onto the next line
        // puts an ordinary integer in exactly that position:
        //
        //     SUBTRACT DNAME-1
        //              1 FROM ERROR-COUNTER.
        //
        // The lexer cannot tell them apart, because a level number is only
        // recognisable from context. Accepting the spelling **here** — in the
        // literal parser, which is only reached where an expression is
        // expected — is exact rather than lenient: the DATA DIVISION matches
        // its entries on `Token::LevelNumber` before any expression is parsed,
        // so a real level number never arrives at this arm.
        Token::LevelNumber(n) => {
            p.advance();
            Some((Literal::Integer(n as i64), span))
        }

        // A numeric literal that begins with the decimal point: `.5`, `.00001`.
        //
        // COBOL-85 allows this — a numeric literal must only not *end* with a
        // decimal point — and the NIST CCVS85 suite depends on it heavily, most
        // of all in the intrinsic-function module (`FUNCTION ACOS(.999)`).
        //
        // The lexer cannot form this token itself: a leading dot also starts a
        // numeric-edited PICTURE (`PIC .9999/99999,99999,99`), and the two are
        // indistinguishable lexically. Resolving it here is what keeps PICTURE
        // parsing untouched — `parse_pic_clause` never calls this function.
        //
        // Adjacency is the whole rule. COBOL-85 requires a space after a
        // sentence-ending period, so a period glued to its digits can only be a
        // decimal point; one followed by a space or a newline is a terminator
        // and is left alone.
        Token::Period if glued(p, 1) => {
            let (frac, scale) = digit_run(p, 1)?;
            p.advance(); // the point
            p.advance(); // the digits
            Some((join_decimal(0, frac, scale), span))
        }

        // The same literal with the roles swapped, under
        // `SPECIAL-NAMES. DECIMAL-POINT IS COMMA.`
        Token::Comma if p.decimal_comma && glued(p, 1) => {
            let (frac, scale) = digit_run(p, 1)?;
            p.advance(); // the point
            p.advance(); // the digits
            Some((join_decimal(0, frac, scale), span))
        }

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

    // TRUE / FALSE as an ordinary operand — sugar for 1 and 0, and nothing
    // more (operator, 2026-08-20). Putting them here rather than in each
    // statement is what makes them work everywhere an operand is allowed at
    // once: `IF x = TRUE`, `IF x NOT = FALSE`, `PERFORM UNTIL x = FALSE`,
    // `INVOKE obj "m" USING TRUE`, `WHEN TRUE` — one rule, no per-statement
    // list to keep in step.
    //
    // This does NOT disturb the two places TRUE/FALSE already meant something:
    // `SET <88-name> TO TRUE` still routes through `exec_move`'s condition-name
    // branch (it receives the same literal 1 it always did), and
    // `EVALUATE TRUE`/`EVALUATE FALSE` are recognised by `parse_evaluate`
    // before any expression is parsed, so a bare subject still means "match the
    // WHEN conditions", not "the number 1".
    if p.at(&Token::True_) {
        p.advance();
        return Some(Expr::Literal(Literal::Integer(1), span));
    }
    if p.at(&Token::False_) {
        p.advance();
        return Some(Expr::Literal(Literal::Integer(0), span));
    }

    // Unary minus
    if p.at(&Token::Minus) {
        p.advance();
        let operand = parse_primary(p)?;
        let sp = span.merge(operand.span());
        return Some(Expr::Unary {
            op: UnaryOp::Neg,
            operand: Box::new(operand),
            span: sp,
        });
    }

    // Unary plus (no-op, kept for source fidelity)
    if p.at(&Token::Plus) {
        p.advance();
        let operand = parse_primary(p)?;
        let sp = span.merge(operand.span());
        return Some(Expr::Unary {
            op: UnaryOp::Pos,
            operand: Box::new(operand),
            span: sp,
        });
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
        // Intrinsic names normally lex as identifiers, but a few collide with a
        // COBOL keyword and arrive as their own token — e.g. RANDOM (from
        // `ACCESS MODE IS RANDOM`). Accept those by name so `FUNCTION RANDOM`
        // parses as the intrinsic; otherwise the keyword token was left stuck
        // and the argument loop below could spin forever.
        let name = if let Some((n, _)) = p.eat_identifier() {
            n
        } else if p.at(&Token::Random) {
            p.advance();
            "RANDOM".to_string()
        } else {
            p.expect_identifier("FUNCTION name")
        };
        let mut args = Vec::new();
        if p.eat(&Token::LParen) {
            while !p.at(&Token::RParen) && !p.at(&Token::Eof) {
                let before = p.pos;
                args.push(parse_expr(p));
                p.eat(&Token::Comma);
                // Liveness guard: if the argument consumed nothing (an
                // unparseable token that is neither ',' nor ')' nor EOF, e.g. a
                // stray keyword), stop rather than loop forever. `expect(RParen)`
                // below emits the diagnostic. A parser must always terminate.
                if p.pos == before {
                    break;
                }
            }
            p.expect(&Token::RParen);
        }
        let sp = span.merge(p.peek_span());
        return Some(Expr::FunctionCall {
            name,
            args,
            span: sp,
        });
    }

    // Figurative constants / literals
    if let Some((lit, sp)) = parse_literal(p) {
        return Some(Expr::Literal(lit, sp));
    }

    // Identifier (optionally subscripted, reference-modified and/or qualified)
    if let Some((name, id_span)) = p.eat_identifier() {
        let mut expr = Expr::Identifier(name.clone(), id_span);

        // `( … )` after a name is either a subscript `(i[,j])` or a reference
        // modification `(start:[length])`.
        expr = parse_subscript_or_refmod(p, expr, id_span);

        // Qualified: `IDENT OF/IN qual [OF/IN qual]…`, up to the standard's 49
        // levels. The chain is collected first and nested afterwards because
        // the runtime reads it **innermost-first** — `A OF B OF C` is
        // `Qualified{A, of: Qualified{B, of: C}}`. Nesting it as it was read
        // put the growing chain in the `name` slot, where only a plain
        // identifier fits, so every qualifier but the last was dropped and
        // `TBL-ITEM-1 OF TABLE-LEVEL-1A OF TABLE-LEVEL-2B` resolved to
        // whichever `TBL-ITEM-1` was declared first.
        let mut quals: Vec<(String, Span)> = Vec::new();
        while p.at(&Token::Of) || p.at(&Token::In) {
            p.advance();
            quals.push(p.eat_identifier().unwrap_or_else(|| {
                p.emit_error("expected qualifier name after OF/IN");
                ("<missing>".into(), p.peek_span())
            }));
        }
        if let Some((last, last_span)) = quals.pop() {
            let mut of_expr = Expr::Identifier(last, last_span);
            for (q, qs) in quals.into_iter().rev() {
                let sp = qs.merge(of_expr.span());
                of_expr = Expr::Qualified {
                    name: q,
                    of: Box::new(of_expr),
                    span: sp,
                };
            }
            let sp = expr.span().merge(of_expr.span());
            expr = match expr {
                Expr::Identifier(n, _) => Expr::Qualified {
                    name: n,
                    of: Box::new(of_expr),
                    span: sp,
                },
                // `TBL-ITEM (I) OF GRP` — the subscript was written before the
                // qualifiers, so it stays wrapped around the qualified name.
                Expr::Subscript {
                    base,
                    indices,
                    span: _,
                } if matches!(*base, Expr::Identifier(..)) => {
                    let name = match *base {
                        Expr::Identifier(n, _) => n,
                        _ => unreachable!("guarded by the matches! above"),
                    };
                    Expr::Subscript {
                        base: Box::new(Expr::Qualified {
                            name,
                            of: Box::new(of_expr),
                            span: sp,
                        }),
                        indices,
                        span: sp,
                    }
                }
                other => Expr::Qualified {
                    name: "<qual>".into(),
                    of: Box::new(of_expr),
                    span: other.span().merge(sp),
                },
            };
        }

        // A subscript may follow the COMPLETE qualified name — which is the
        // order COBOL-85 actually specifies:
        //
        //     data-name-1 [OF data-name-2]… [(subscript…)]
        //
        // `CELL OF COLS OF ROWS (IDX-A IDX-B)` (NC135A) went unparsed: the
        // subscript above is read before the OF chain, so nothing consumed the
        // trailing list and the generic parenthesised-expression rule reported
        // `expected RParen` at the second subscript. Both orders are accepted —
        // the pre-qualification form has always been allowed here and nothing
        // that relied on it changes.
        expr = parse_subscript_or_refmod(p, expr, id_span);

        // Inline member-access chain: `obj::Caption`, `obj::GetText()`,
        // `Grid::Rows(I)::Cols(2)::Value` — a postfix `::` loop over `expr`.
        return Some(parse_member_chain(p, expr));
    }

    None
}

/// Apply a postfix `::member [ ( args ) ]` chain to an already-parsed base
/// expression, building nested [`Expr::Member`] nodes. `parens` records whether
/// `()` followed the member (a method call / subscript) versus a bare property.
/// Returns `base` unchanged when no `::` follows.
/// Parse one entry of a subscript list.
///
/// Almost always an ordinary expression. The exception is the reserved word
/// `ALL` standing alone, which COBOL-85 gives a second meaning **in a subscript
/// position**: every occurrence of the table in that dimension, so a whole
/// table can be handed to a statistical intrinsic —
/// `FUNCTION MAX(IND(ALL))`, `FUNCTION SUM(TBL(ALL, 2))`.
///
/// **Position is what distinguishes it** from the figurative constant `ALL "X"`,
/// not what follows it. A subscript is an integer expression or an index-name;
/// a figurative constant is never a legal subscript, so inside these
/// parentheses `ALL` can only be the table-wide meaning. Deciding by lookahead
/// instead would be wrong the moment a subscript follows it — `TBL(ALL, 2)`
/// reaches the parser as `TBL(ALL 2)` once the separator comma is dropped, and
/// a lookahead rule reads that as the figurative constant `ALL 2`.
///
/// Because the decision lives here rather than in the lexer, `MOVE ALL "X" TO Y`
/// — which is not a subscript position — is untouched.
fn parse_subscript_index(p: &mut Parser) -> Expr {
    if p.at(&Token::All) {
        let span = p.peek_span();
        p.advance();
        return Expr::AllSubscript(span);
    }
    parse_expr(p)
}

/// Parse an optional `( … )` suffix on a data reference.
///
/// The parenthesis is either a **subscript** list — `TABLE (1 2)` — or a
/// **reference modification** — `TEXT (3:5)`. They are told apart by the first
/// `:`, which only reference modification can contain.
///
/// Returns `expr` untouched when there is no parenthesis, so the caller can
/// apply it unconditionally.
fn parse_subscript_or_refmod(p: &mut Parser, expr: Expr, id_span: Span) -> Expr {
    if !p.at(&Token::LParen) {
        return expr;
    }
    p.advance();
    // A subscript list reads a space-then-glued sign as a signed literal
    // opening the next subscript; restore the flag on the way out so a nested
    // reference outside the parenthesis is unaffected.
    let outer_subscript = p.in_subscript;
    p.in_subscript = true;
    let first = parse_subscript_index(p);

    if p.at(&Token::Colon) {
        p.in_subscript = outer_subscript;
        // Reference modification: IDENT(start:[length])
        p.advance();
        let length = if p.at(&Token::RParen) {
            None
        } else {
            Some(Box::new(parse_expr(p)))
        };
        p.expect(&Token::RParen);
        let sp = id_span.merge(p.peek_span());
        return Expr::RefMod {
            base: Box::new(expr),
            start: Box::new(first),
            length,
            span: sp,
        };
    }

    // Subscript: IDENT(i[,j…])
    //
    // COBOL-85 separates subscripts with a SPACE; the comma is an optional
    // separator that means the same thing, so `TABLE (1 1 1)`,
    // `TABLE (1, 1, 1)` and `TABLE (1 ,1, 1)` are one and the same reference.
    // The list is therefore bounded by the closing parenthesis, not by the
    // comma — bounding it by the comma read only the first subscript of the
    // spaced form and then reported `expected RParen` at the second.
    let mut indices = vec![first];
    while !p.at(&Token::RParen) && !p.at(&Token::Eof) {
        let before = p.pos;
        p.eat(&Token::Comma);
        indices.push(parse_subscript_index(p));
        // Liveness guard: with the comma no longer guaranteed to advance the
        // cursor, a subscript that consumes nothing must end the list rather
        // than spin. `expect(RParen)` below reports it.
        if p.pos == before {
            break;
        }
    }
    p.in_subscript = outer_subscript;
    p.expect(&Token::RParen);
    let sp = id_span.merge(p.peek_span());
    let mut out = Expr::Subscript {
        base: Box::new(expr),
        indices,
        span: sp,
    };

    // A reference modification may follow a subscript: t(i)(s:l)
    if p.at(&Token::LParen) {
        p.advance();
        let start = parse_expr(p);
        p.expect(&Token::Colon);
        let length = if p.at(&Token::RParen) {
            None
        } else {
            Some(Box::new(parse_expr(p)))
        };
        p.expect(&Token::RParen);
        let sp = id_span.merge(p.peek_span());
        out = Expr::RefMod {
            base: Box::new(out),
            start: Box::new(start),
            length,
            span: sp,
        };
    }
    out
}

pub(crate) fn parse_member_chain(p: &mut Parser, mut base: Expr) -> Expr {
    while *p.peek() == Token::Colon && *p.peek_at(1) == Token::Colon {
        let start = base.span();
        p.advance(); // first ':'
        p.advance(); // second ':'
                     // Member name: a bare identifier (preferred) or a quoted string (tolerated
                     // for symmetry with classic INVOKE / older completion output).
        let member = p
            .eat_identifier()
            .map(|(n, _)| n)
            .or_else(|| take_string_literal(p))
            .unwrap_or_default();
        let mut args = Vec::new();
        let mut parens = false;
        if p.at(&Token::LParen) {
            parens = true;
            p.advance();
            // The argument list is bounded by the closing parenthesis. It used
            // to break the moment an argument was not followed by a comma,
            // which stopped after the first argument once separator commas
            // stopped being tokens.
            while !p.at(&Token::RParen) && !p.at(&Token::Eof) {
                let before = p.pos;
                args.push(parse_expr(p));
                p.eat(&Token::Comma);
                if p.pos == before {
                    break;
                }
            }
            p.expect(&Token::RParen);
        }
        let sp = start.merge(p.peek_span());
        base = Expr::Member {
            recv: Box::new(base),
            member,
            args,
            parens,
            span: sp,
        };

        // Parse any subscript or refmod applied directly to the member expression, e.g. `::split("::")(WS-I)`
        if p.at(&Token::LParen) {
            p.advance();
            let first = parse_expr(p);
            if p.at(&Token::Colon) {
                p.advance();
                let length = if p.at(&Token::RParen) {
                    None
                } else {
                    Some(Box::new(parse_expr(p)))
                };
                p.expect(&Token::RParen);
                let sp = base.span().merge(p.peek_span());
                base = Expr::RefMod {
                    base: Box::new(base),
                    start: Box::new(first),
                    length,
                    span: sp,
                };
            } else {
                // Same bound as the subscript list above: the closing
                // parenthesis, not the comma.
                let mut indices = vec![first];
                while !p.at(&Token::RParen) && !p.at(&Token::Eof) {
                    let before = p.pos;
                    p.eat(&Token::Comma);
                    indices.push(parse_expr(p));
                    if p.pos == before {
                        break;
                    }
                }
                p.expect(&Token::RParen);
                let sp = base.span().merge(p.peek_span());
                base = Expr::Subscript {
                    base: Box::new(base),
                    indices,
                    span: sp,
                };
            }
        }
    }
    base
}

/// Consume `TRUE`/`FALSE` when it is being used as a **truth test** rather than
/// as an operand (`x IS TRUE`, `x NOT FALSE`), returning the value it stands
/// for — 1 and 0, the whole of what these keywords mean here.
///
/// Distinct from [`parse_primary`], which takes them as operands. Both exist
/// because `x = TRUE` and `x IS TRUE` reach this file by different routes and
/// have to end up as the same comparison.
fn take_truth_literal(p: &mut Parser) -> Option<(i64, cobolt_lexer::Span)> {
    let span = p.peek_span();
    if p.at(&Token::True_) {
        p.advance();
        return Some((1, span));
    }
    if p.at(&Token::False_) {
        p.advance();
        return Some((0, span));
    }
    None
}

/// Consume a string literal if the current token is one.
pub(crate) fn take_string_literal(p: &mut Parser) -> Option<String> {
    if let Token::StringLiteral(s) = p.peek() {
        let s = s.clone();
        p.advance();
        Some(s)
    } else {
        None
    }
}

/// Left/right binding powers for binary arithmetic operators.
fn infix_bp(tok: &Token) -> Option<(u8, u8)> {
    match tok {
        Token::Ampersand => Some((1, 2)),
        Token::Plus | Token::Minus => Some((3, 4)),
        Token::Star | Token::Slash => Some((5, 6)),
        Token::Power => Some((8, 7)), // right-associative
        _ => None,
    }
}

fn tok_to_arithop(tok: &Token) -> ArithOp {
    match tok {
        Token::Plus => ArithOp::Add,
        Token::Minus => ArithOp::Sub,
        Token::Star => ArithOp::Mul,
        Token::Slash => ArithOp::Div,
        Token::Power => ArithOp::Pow,
        Token::Ampersand => ArithOp::Concat,
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
        // `ELEM (IN1 +3)` — the sign belongs to the literal that opens the next
        // subscript, so the expression for this one ends here.
        if p.starts_signed_subscript() {
            break;
        }
        let tok = p.peek().clone();
        match infix_bp(&tok) {
            Some((l_bp, r_bp)) if l_bp >= min_bp => {
                let op = tok_to_arithop(&tok);
                p.advance();
                let rhs = parse_expr_bp(p, r_bp);
                let sp = lhs.span().merge(rhs.span());
                lhs = Expr::Arithmetic {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                    span: sp,
                };
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
    peek_ident_upper_at(p, 0)
}

/// `peek_ident_upper` at a lookahead offset — used to see past an optional
/// `NOT` before deciding whether a class or sign word follows.
fn peek_ident_upper_at(p: &Parser, offset: usize) -> Option<String> {
    if let Token::Identifier(s) = p.peek_at(offset) {
        Some(s.to_uppercase())
    } else {
        None
    }
}

/// True if `tok` can begin a relational operator (symbolic or word form).
fn is_relop_start(tok: &Token) -> bool {
    matches!(
        tok,
        Token::Eq
            | Token::NotEq
            | Token::Lt
            | Token::Gt
            | Token::LtEq
            | Token::GtEq
            | Token::Equal
            | Token::Greater
            | Token::Less
    )
}

/// Parse one relational operator — symbolic (`=` `<>` `<` `>` `<=` `>=`) or word
/// (`EQUAL [TO]`, `GREATER [OR EQUAL] [THAN]`, `LESS [OR EQUAL] [THAN]`) — and
/// return its `CmpOp`. `None` if the current token is not a relational operator.
fn parse_relop(p: &mut Parser) -> Option<CmpOp> {
    let op = match p.peek().clone() {
        Token::Eq => CmpOp::Eq,
        Token::NotEq => CmpOp::Ne,
        Token::Lt => CmpOp::Lt,
        Token::Gt => CmpOp::Gt,
        Token::LtEq => CmpOp::Le,
        Token::GtEq => CmpOp::Ge,
        Token::Equal => {
            p.advance();
            p.eat(&Token::To);
            return Some(CmpOp::Eq);
        }
        // `OR EQUAL TO` may be written on either side of the optional `THAN`:
        // `GREATER OR EQUAL TO x` and `GREATER THAN OR EQUAL TO x` are the same
        // operator. Looking only before it left `OR EQUAL TO x` behind as a
        // separate term of the condition.
        Token::Greater => {
            p.advance();
            let mut ge = check_or_equal(p);
            p.eat(&Token::Than);
            ge = ge || check_or_equal(p);
            return Some(if ge { CmpOp::Ge } else { CmpOp::Gt });
        }
        Token::Less => {
            p.advance();
            let mut le = check_or_equal(p);
            p.eat(&Token::Than);
            le = le || check_or_equal(p);
            return Some(if le { CmpOp::Le } else { CmpOp::Lt });
        }
        _ => return None,
    };
    p.advance();
    Some(op)
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

    // Parenthesised condition — unless what follows the matching `)` shows the
    // parenthesis to be an arithmetic operand instead (see below).
    if p.at(&Token::LParen) && !paren_opens_operand(p) {
        p.advance();
        let cond = parse_condition(p);
        p.expect(&Token::RParen);
        return cond;
    }

    // A bare `TRUE` / `FALSE` as the WHOLE condition: `IF TRUE`,
    // `PERFORM UNTIL TRUE`. Taken before the operand parser, so it stays a
    // condition instead of becoming a lone literal with nothing to compare
    // against — which is "expected comparison operator in condition".
    //
    // Only when nothing follows that would make it an operand: `IF TRUE = X`
    // is a comparison and must still parse as one.
    if matches!(p.peek(), Token::True_ | Token::False_)
        && !matches!(p.peek_at(1), Token::Is)
        && !is_relop_start(p.peek_at(1))
    {
        let (value, vspan) = take_truth_literal(p).expect("just matched");
        return Condition::Comparison {
            lhs: Expr::Literal(Literal::Integer(1), vspan),
            op: CmpOp::Eq,
            rhs: Expr::Literal(Literal::Integer(value), vspan),
            span: vspan,
        };
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
                "NUMERIC" => Some(DataClass::Numeric),
                "ALPHABETIC" => Some(DataClass::Alphabetic),
                "ALPHABETIC-LOWER" => Some(DataClass::AlphabeticLower),
                "ALPHABETIC-UPPER" => Some(DataClass::AlphabeticUpper),
                _ => None,
            };
            if let Some(c) = class {
                p.advance();
                let sp = span.merge(p.peek_span());
                return Condition::ClassTest {
                    expr: lhs,
                    negated,
                    class: c,
                    span: sp,
                };
            }
        }

        // A class the program itself declared in SPECIAL-NAMES. Checked after
        // the four built-in classes so a program cannot shadow them, and
        // before the sign test so a class called `POSITIVE` still loses.
        if let Some(name) = peek_ident_upper(p) {
            if p.classes.iter().any(|(c, _)| *c == name) {
                p.advance();
                let sp = span.merge(p.peek_span());
                return Condition::UserClassTest {
                    expr: lhs,
                    negated,
                    class: name,
                    span: sp,
                };
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
                return Condition::SignTest {
                    expr: lhs,
                    negated,
                    sign: s,
                    span: sp,
                };
            }
        }
        if p.at(&Token::Zeros) {
            p.advance();
            let sp = span.merge(p.peek_span());
            return Condition::SignTest {
                expr: lhs,
                negated,
                sign: SignCond::Zero,
                span: sp,
            };
        }

        // Relational operator after IS [NOT]: `IS [NOT] {= | EQUAL TO | GREATER …}`.
        if let Some(base) = parse_relop(p) {
            let op = if negated { negate_cmp(base) } else { base };
            let rhs = parse_expr(p);
            let sp = span.merge(rhs.span());
            return Condition::Comparison {
                lhs,
                op,
                rhs,
                span: sp,
            };
        }

        // `IS [NOT] TRUE|FALSE` — a truth test. TRUE and FALSE are sugar for 1
        // and 0, so this is `= 1` / `= 0`, negated to `<>` after `IS NOT`.
        if let Some((value, vspan)) = take_truth_literal(p) {
            return Condition::Comparison {
                lhs,
                op: if negated { CmpOp::Ne } else { CmpOp::Eq },
                rhs: Expr::Literal(Literal::Integer(value), vspan),
                span: span.merge(vspan),
            };
        }

        p.emit_error("unrecognised IS clause in condition");
        return Condition::ConditionName("<error>".into(), span);
    }

    // **The `IS` is optional** in a class or sign condition. COBOL-85 writes
    // `IF X IS NUMERIC` and `IF X NUMERIC` alike, and `EVALUATE X NUMERIC`
    // (NC225A) has no `IS` at all — there is nowhere to put one.
    //
    // A leading `NOT` is consumed only if a class or sign word really follows,
    // so `a NOT = b` still reaches the relational path below with its `NOT`
    // intact.
    {
        let after_not = usize::from(p.at(&Token::Not));
        let word = peek_ident_upper_at(p, after_not);
        let class = word.as_deref().and_then(|n| match n {
            "NUMERIC" => Some(DataClass::Numeric),
            "ALPHABETIC" => Some(DataClass::Alphabetic),
            "ALPHABETIC-LOWER" => Some(DataClass::AlphabeticLower),
            "ALPHABETIC-UPPER" => Some(DataClass::AlphabeticUpper),
            _ => None,
        });
        // `ZERO` is a lexer keyword (the figurative constant), not an
        // identifier, so the sign test has to look for the token as well as the
        // word — exactly as the `IS` form below already does.
        let sign = match word.as_deref() {
            Some("POSITIVE") => Some(SignCond::Positive),
            Some("NEGATIVE") => Some(SignCond::Negative),
            _ if matches!(p.peek_at(after_not), Token::Zeros) => Some(SignCond::Zero),
            _ => None,
        };
        // A SPECIAL-NAMES class, with the `IS` left out — which is how NC174A
        // writes every one of them (`IF WS-A NOT ORDINAL-A-ONLY`).
        if class.is_none() && sign.is_none() {
            if let Some(name) = word.filter(|n| p.classes.iter().any(|(c, _)| c == n)) {
                let negated = after_not == 1;
                if negated {
                    p.advance(); // NOT
                }
                p.advance(); // the class name
                return Condition::UserClassTest {
                    expr: lhs,
                    negated,
                    class: name,
                    span: span.merge(p.peek_span()),
                };
            }
        }
        if class.is_some() || sign.is_some() {
            let negated = after_not == 1;
            if negated {
                p.advance(); // NOT
            }
            p.advance(); // the class / sign word
            let sp = span.merge(p.peek_span());
            return match class {
                Some(c) => Condition::ClassTest {
                    expr: lhs,
                    negated,
                    class: c,
                    span: sp,
                },
                None => Condition::SignTest {
                    expr: lhs,
                    negated,
                    sign: sign.expect("one of the two matched"),
                    span: sp,
                },
            };
        }
    }

    // The same test with `IS` left out: `x TRUE`, `x NOT FALSE`. Checked before
    // the relational-operator paths below because there is no operator here to
    // find — without this the condition ends at `x`, the rest of the line is
    // left over, and the statement fails on a stray `NOT`.
    {
        let bare_not = p.at(&Token::Not)
            && matches!(p.peek_at(1), Token::True_ | Token::False_);
        if bare_not {
            p.advance(); // NOT
        }
        if let Some((value, vspan)) = take_truth_literal(p) {
            return Condition::Comparison {
                lhs,
                op: if bare_not { CmpOp::Ne } else { CmpOp::Eq },
                rhs: Expr::Literal(Literal::Integer(value), vspan),
                span: span.merge(vspan),
            };
        }
    }

    // A bare leading `NOT` before a relational operator (no `IS`): `a NOT = b`,
    // `a NOT > b`, `a NOT EQUAL b` → the negated comparison.
    let lead_not = p.at(&Token::Not) && is_relop_start(p.peek_at(1));
    if lead_not {
        p.advance(); // NOT
    }

    // Relational comparison — symbolic (`=` `<>` `<` …) or word (`EQUAL TO`,
    // `GREATER [THAN]`, `LESS [THAN]`), with or without the leading NOT.
    if let Some(base) = parse_relop(p) {
        let op = if lead_not { negate_cmp(base) } else { base };
        let rhs = parse_expr(p);
        let sp = span.merge(rhs.span());
        return Condition::Comparison {
            lhs,
            op,
            rhs,
            span: sp,
        };
    }

    // No comparison operator → the expression names a condition (an 88-level
    // item). A condition-name is referenced like any other item, so besides a
    // bare word it can be subscripted (`IF FIRSTZ (1)`) or qualified
    // (`IF A OF IF-D32`) — those carry the occurrence the 88 is tested in, so
    // they keep the whole reference rather than just its leaf name.
    match lhs {
        Expr::Identifier(name, s) => Condition::ConditionName(name, s),
        other @ (Expr::Subscript { .. } | Expr::Qualified { .. }) => {
            let s = other.span();
            Condition::ConditionRef(Box::new(other), s)
        }
        other => {
            p.emit_error("expected comparison operator in condition");
            Condition::ConditionName("<error>".into(), other.span())
        }
    }
}

/// `true` when the parenthesis at the cursor encloses an arithmetic
/// **expression** rather than a condition: no relational or logical operator
/// appears at its own nesting level. `WHEN (33 + (99 - 43))` is a selection
/// object; `WHEN (A GREATER B)` is a condition.
pub(crate) fn paren_encloses_expression(p: &Parser) -> bool {
    if !matches!(p.peek(), Token::LParen) {
        return false;
    }
    let mut depth = 0usize;
    let mut i = 0usize;
    loop {
        let t = p.peek_at(i);
        match t {
            Token::LParen => depth += 1,
            Token::RParen => {
                depth -= 1;
                if depth == 0 {
                    return true;
                }
            }
            Token::Eof => return false,
            _ => {
                // Any depth, not just the outermost: redundant parentheses are
                // legal and the suite writes them six deep —
                // `((((((0 - CONT-D EQUAL TO CONT-D OR -11 + CONT-F))))))`.
                // A relational or logical operator *anywhere* inside rules out
                // an arithmetic expression, so looking only at level 1 read
                // that whole group as one.
                if is_relop_start(t)
                    || matches!(t, Token::Is | Token::And | Token::Or | Token::Not)
                {
                    return false;
                }
            }
        }
        i += 1;
    }
}

/// `true` when the parenthesis at the cursor is the start of an **operand**,
/// not of a nested condition.
///
/// `PERFORM … UNTIL (WRK + 12) = 100` and `WHEN (33 + (99 - 43))` both open
/// with `(`, and reading either as a parenthesised condition fails on the
/// arithmetic inside it. What tells them apart is what follows the matching
/// `)`: an operator (relational or arithmetic) can only follow an operand, and
/// so can the end of the whole condition when it is an `EVALUATE` object.
/// Scanning to the matching parenthesis costs one pass and leaves the cursor
/// and the diagnostics untouched, which trial-parsing would not.
fn paren_opens_operand(p: &Parser) -> bool {
    let mut depth = 0usize;
    let mut i = 0usize;
    loop {
        match p.peek_at(i) {
            Token::LParen => depth += 1,
            Token::RParen => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Token::Eof => return false,
            _ => {}
        }
        i += 1;
    }
    let after = p.peek_at(i + 1);
    is_relop_start(after)
        || matches!(
            after,
            Token::Is
                | Token::Not
                | Token::Plus
                | Token::Minus
                | Token::Star
                | Token::Slash
                | Token::Power
        )
}

/// The subject (`lhs`) of the right-most `Comparison` in a condition, used to
/// expand abbreviated combined conditions (`a > 1 AND < 9`).
fn rightmost_subject(c: &Condition) -> Option<&Expr> {
    match c {
        Condition::Comparison { lhs, .. } => Some(lhs),
        // An abbreviation already carries the subject it reused, and a further
        // abbreviation chains off it: `a = b OR c AND "d"` reuses `a` twice.
        Condition::NameOrAbbrev { subject, .. } => Some(subject),
        // The right side may be a condition-name, which carries no subject of
        // its own; the abbreviation then reaches back past it —
        // `a = b OR c AND "d"` compares `a` with `"d"`.
        Condition::And(a, b, _) | Condition::Or(a, b, _) => {
            rightmost_subject(b).or_else(|| rightmost_subject(a))
        }
        Condition::Not(inner, _) => rightmost_subject(inner),
        _ => None,
    }
}

/// The subject + operator of the right-most `Comparison`, for expanding a
/// *literal-object* abbreviation (`a = 1 OR 2` → `a = 1 OR a = 2`).
fn rightmost_comparison(c: &Condition) -> Option<(Expr, CmpOp)> {
    match c {
        Condition::Comparison { lhs, op, .. } => Some((lhs.clone(), *op)),
        Condition::NameOrAbbrev { subject, op, .. } => Some(((**subject).clone(), *op)),
        Condition::And(a, b, _) | Condition::Or(a, b, _) => {
            rightmost_comparison(b).or_else(|| rightmost_comparison(a))
        }
        Condition::Not(inner, _) => rightmost_comparison(inner),
        _ => None,
    }
}

/// `true` when the token `offset` ahead is a class or sign word — the tail of a
/// class/sign condition written without its optional `IS`.
fn at_class_or_sign_word(p: &Parser, offset: usize) -> bool {
    if matches!(p.peek_at(offset), Token::Zeros) {
        return true;
    }
    matches!(
        peek_ident_upper_at(p, offset).as_deref(),
        Some(
            "NUMERIC"
                | "ALPHABETIC"
                | "ALPHABETIC-LOWER"
                | "ALPHABETIC-UPPER"
                | "POSITIVE"
                | "NEGATIVE"
        )
    )
}

/// `true` when the operand at the cursor is the head of an **arithmetic
/// expression** — an operator follows it — so the whole expression is the
/// abbreviation object, not just the operand.
fn starts_arithmetic_object(p: &Parser) -> bool {
    // A leading `+`/`-` is a unary sign, and no condition may begin with one,
    // so the object is arithmetic however it continues: `… EQUAL TO CONT-D OR
    // -11 + CONT-F` reads all of `-11 + CONT-F` as the reused subject's object.
    let signed = matches!(p.peek(), Token::Plus | Token::Minus);
    let head = usize::from(signed);
    if !matches!(
        p.peek_at(head),
        Token::Identifier(_) | Token::IntegerLiteral(_) | Token::DecimalLiteral { .. }
    ) {
        return false;
    }
    if signed {
        return true;
    }
    matches!(
        p.peek_at(1),
        Token::Plus | Token::Minus | Token::Star | Token::Slash | Token::Power
    )
}

/// `true` when the single-token operand at the cursor is followed by a
/// relational operator — so it is the *subject* of a full relation, not the
/// object of an abbreviation.
fn starts_relation_after_operand(p: &Parser) -> bool {
    let next = p.peek_at(1);
    is_relop_start(next) || matches!(next, Token::Is) || matches!(next, Token::Not)
}

/// True if the current token starts a bare literal operand (the object of a
/// literal-object abbreviation). Identifiers are excluded — a bare identifier
/// after AND/OR remains a condition-name (88-level), which the parser cannot
/// distinguish from a data-item object without the symbol table.
fn at_literal_object(p: &Parser) -> bool {
    matches!(
        p.peek(),
        Token::IntegerLiteral(_)
            | Token::DecimalLiteral { .. }
            | Token::StringLiteral(_)
            | Token::Spaces
            | Token::Zeros
            | Token::HighValues
            | Token::LowValues
            | Token::Quotes
            | Token::Nulls
            | Token::AllLiteral
    )
}

/// True if the current token is a *bare* identifier object of an abbreviated
/// relation: an identifier not followed by a relational operator, qualifier, or
/// `AND` (so OR/term-end). Used to route `a = b OR c` to the continuation parser
/// while leaving `c AND …` to the normal AND parser (precedence).
fn at_bare_object(p: &Parser) -> bool {
    if !matches!(p.peek(), Token::Identifier(_)) {
        return false;
    }
    // `… AND SIGN-1 ZERO` is a full sign condition on a new subject, not an
    // abbreviation object: the class/sign word after the name belongs to it.
    if at_class_or_sign_word(p, 1) {
        return false;
    }
    !matches!(
        p.peek_at(1),
        Token::Eq
            | Token::NotEq
            | Token::Lt
            | Token::Gt
            | Token::LtEq
            | Token::GtEq
            | Token::Greater
            | Token::Less
            | Token::Equal
            | Token::Is
            | Token::Of
            | Token::In
            | Token::LParen
            | Token::And
            | Token::Not
    )
}

/// True if the current token begins a relational operator (the signal for an
/// operator-prefixed abbreviated condition, e.g. the `< 9` in `a > 1 AND < 9`).
fn at_relop(p: &Parser) -> bool {
    matches!(
        p.peek(),
        Token::Eq
            | Token::NotEq
            | Token::Lt
            | Token::Gt
            | Token::LtEq
            | Token::GtEq
            | Token::Greater
            | Token::Less
            | Token::Equal
    )
}

/// Parse one relational operator + RHS as a comparison reusing `subject`.
fn parse_abbrev_comparison(p: &mut Parser, subject: &Expr) -> Condition {
    let span = p.peek_span();
    let negated = p.eat(&Token::Not);
    let op = if p.eat(&Token::Equal) {
        p.eat(&Token::To);
        if negated {
            CmpOp::Ne
        } else {
            CmpOp::Eq
        }
    } else if p.eat(&Token::Greater) {
        let ge = check_or_equal(p);
        p.eat(&Token::Than);
        let base = if ge { CmpOp::Ge } else { CmpOp::Gt };
        if negated {
            negate_cmp(base)
        } else {
            base
        }
    } else if p.eat(&Token::Less) {
        let le = check_or_equal(p);
        p.eat(&Token::Than);
        let base = if le { CmpOp::Le } else { CmpOp::Lt };
        if negated {
            negate_cmp(base)
        } else {
            base
        }
    } else {
        let t = p.peek().clone();
        p.advance();
        match t {
            Token::Eq => {
                if negated {
                    CmpOp::Ne
                } else {
                    CmpOp::Eq
                }
            }
            Token::NotEq => CmpOp::Ne,
            Token::Lt => {
                if negated {
                    CmpOp::Ge
                } else {
                    CmpOp::Lt
                }
            }
            Token::Gt => {
                if negated {
                    CmpOp::Le
                } else {
                    CmpOp::Gt
                }
            }
            Token::LtEq => {
                if negated {
                    CmpOp::Gt
                } else {
                    CmpOp::Le
                }
            }
            Token::GtEq => {
                if negated {
                    CmpOp::Lt
                } else {
                    CmpOp::Ge
                }
            }
            _ => CmpOp::Eq,
        }
    };
    let rhs = parse_expr(p);
    let sp = span.merge(rhs.span());
    Condition::Comparison {
        lhs: subject.clone(),
        op,
        rhs,
        span: sp,
    }
}

/// A continuation term after AND/OR: an operator-prefixed abbreviation reuses the
/// preceding subject; otherwise a fresh primary condition.
fn parse_continuation(p: &mut Parser, prev: &Condition) -> Condition {
    // An abbreviation may keep the optional `IS` a full relation allows:
    // `… GREATER THAN A AND IS NOT LESS THAN B`. The word carries no meaning of
    // its own; the operator behind it decides. Left in place it reached the
    // operand parser as "expected expression, found Is".
    if p.at(&Token::Is)
        && (is_relop_start(p.peek_at(1))
            || (matches!(p.peek_at(1), Token::Not) && is_relop_start(p.peek_at(2))))
    {
        p.advance();
    }
    if at_relop(p)
        || (p.at(&Token::Not) && {
            let n = p.peek_at(1);
            matches!(
                n,
                Token::Eq
                    | Token::NotEq
                    | Token::Lt
                    | Token::Gt
                    | Token::LtEq
                    | Token::GtEq
                    | Token::Greater
                    | Token::Less
                    | Token::Equal
            )
        })
    {
        if let Some(subject) = rightmost_subject(prev) {
            return parse_abbrev_comparison(p, &subject.clone());
        }
    }
    // Literal-object abbreviation: reuse the previous subject AND operator.
    //
    // Only when the literal is the whole term. `… GREATER THAN CCON-1 AND 20
    // LESS THAN CCON-3` writes a **complete** relation whose subject happens to
    // be a literal; reading the `20` as an abbreviation object consumed it and
    // left `LESS THAN CCON-3` with nothing in front of it.
    if at_literal_object(p) && !starts_relation_after_operand(p) {
        if let Some((subject, op)) = rightmost_comparison(prev) {
            let span = p.peek_span();
            let rhs = parse_expr(p);
            let sp = span.merge(rhs.span());
            return Condition::Comparison {
                lhs: subject,
                op,
                rhs,
                span: sp,
            };
        }
    }
    // An **arithmetic expression** as the abbreviation object:
    // `… OR EQUAL TO CCON-1 OR 8 OR CCON-3 - 1`. The operand is only the head of
    // the object here, so taking it alone left `- 1` behind.
    if starts_arithmetic_object(p) {
        if let Some((subject, op)) = rightmost_comparison(prev) {
            let span = p.peek_span();
            let rhs = parse_expr(p);
            let sp = span.merge(rhs.span());
            return Condition::Comparison {
                lhs: subject,
                op,
                rhs,
                span: sp,
            };
        }
    }
    // Identifier-object abbreviation vs. condition-name: a *bare* identifier
    // (not followed by a relational operator) after AND/OR, when the previous
    // term is a comparison, is ambiguous — it is either an 88-level
    // condition-name or the object of the reused subject/operator. Emit a
    // NameOrAbbrev node and let the runtime decide via its 88-level metadata.
    if let Token::Identifier(name) = p.peek().clone() {
        let next = p.peek_at(1);
        let bare = !at_class_or_sign_word(p, 1)
            && !matches!(
            next,
            Token::Eq
                | Token::NotEq
                | Token::Lt
                | Token::Gt
                | Token::LtEq
                | Token::GtEq
                | Token::Greater
                | Token::Less
                | Token::Equal
                | Token::Is
                | Token::Of
                | Token::In
                | Token::LParen
                | Token::Not
        );
        if bare {
            if let Some((subject, op)) = rightmost_comparison(prev) {
                let span = p.peek_span();
                p.advance();
                return Condition::NameOrAbbrev {
                    subject: Box::new(subject),
                    op,
                    name: name.to_ascii_uppercase(),
                    span,
                };
            }
        }
    }
    parse_condition_primary(p)
}

fn parse_condition_and(p: &mut Parser) -> Condition {
    parse_condition_and_ctx(p, None)
}

/// `AND` terms, with the condition already parsed to the **left of this term**
/// available for abbreviation.
///
/// The subject an abbreviation reuses may live further left than the term it
/// attaches to: in `a = b OR c AND "d"` the AND's own left operand is the
/// condition-name `c`, which carries no subject, and the `"d"` has to reach
/// back to `a`. Without the outer context that literal had nothing to compare
/// against and the condition failed to parse.
fn parse_condition_and_ctx(p: &mut Parser, outer: Option<&Condition>) -> Condition {
    let mut lhs = parse_condition_primary(p);
    while p.at(&Token::And) {
        p.advance();
        let ctx = match outer {
            Some(o) => Condition::Or(
                Box::new(o.clone()),
                Box::new(lhs.clone()),
                lhs.span(),
            ),
            None => lhs.clone(),
        };
        let rhs = parse_continuation(p, &ctx);
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
        // `… OR NOT < TEN` — a negated operator-prefixed abbreviation. Without
        // the `NOT` in this test it fell to the full-condition path, which has
        // no subject to put in front of the `<`.
        let negated_relop = p.at(&Token::Not) && is_relop_start(p.peek_at(1));
        // `… EQUAL TO CONT-D OR -11 + CONT-F` — an *arithmetic* abbreviation
        // object. No condition may begin with a sign or continue into an
        // arithmetic operator, so this can only reuse the preceding subject;
        // without the test it fell to the full-condition path, which then
        // demanded a relational operator the source never had.
        let rhs = if at_relop(p)
            || negated_relop
            || at_literal_object(p)
            || at_bare_object(p)
            || starts_arithmetic_object(p)
        {
            // AND still binds tighter than OR, so an abbreviated OR-term keeps
            // absorbing `AND …` terms of its own: in
            // `a = b OR c AND "d"` the `AND "d"` belongs to the `OR c` term.
            // Returning straight to the OR loop left it unread.
            let mut r = parse_continuation(p, &lhs);
            while p.at(&Token::And) {
                p.advance();
                let next = parse_continuation(p, &r);
                let sp = r.span().merge(next.span());
                r = Condition::And(Box::new(r), Box::new(next), sp);
            }
            r
        } else {
            parse_condition_and_ctx(p, Some(&lhs))
        };
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
