// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Statement parsers for the PROCEDURE DIVISION.
//!
//! Each `parse_*` function corresponds to one COBOL verb.  The top-level
//! dispatcher [`parse_stmt`] looks at the current token and delegates.

use cobolt_ast::expr::{CmpOp, Condition, Expr, Literal};
use cobolt_ast::stmt::{
    AcceptSource, AdvancingClause, CallArg, EvalSubject, ExecRustBinding, ExitKind, OpenMode,
    PerformTarget, ReadDirection, Stmt, UnstringTarget, VaryingAfter, WhenClause, WhenValue,
};
use cobolt_lexer::Token;

use crate::expr::{parse_condition, parse_expr, parse_literal};
use crate::parser::Parser;

// ── Public stop-condition helpers ─────────────────────────────────────────────

/// Returns `true` if the current token can start a new statement.
pub(crate) fn is_stmt_start(tok: &Token) -> bool {
    matches!(
        tok,
        Token::Move
            | Token::Add
            | Token::Subtract
            | Token::Multiply
            | Token::Divide
            | Token::Compute
            | Token::If
            | Token::Evaluate
            | Token::Perform
            | Token::Go
            | Token::GoTo   // GO-TO hyphenated (rare)
            | Token::GoBack // GO-BACK hyphenated (rare)
            | Token::Continue
            | Token::Stop
            | Token::Exit
            | Token::Open
            | Token::Close
            | Token::Read
            | Token::Write
            | Token::Rewrite
            | Token::Delete
            | Token::Start
            | Token::Accept
            | Token::Display
            | Token::StringVerb
            | Token::Unstring
            | Token::Inspect
            | Token::Sort
            | Token::Merge
            | Token::Call
            | Token::Invoke
            | Token::Initialize
            | Token::Set
            | Token::Cancel
            | Token::Play
            | Token::StopAnim
            | Token::ExecRustBlock(_)
            | Token::Try
            | Token::Throw
    )
}

/// Parse all statements until `stop(peek)` returns true or EOF.
/// Periods between sentences are consumed and ignored.
/// Paragraph/section headers are detected and break the loop without consuming.
pub(crate) fn parse_stmts(p: &mut Parser, stop: &dyn Fn(&Token) -> bool) -> Vec<Stmt> {
    let mut stmts = Vec::new();
    loop {
        // Consume optional sentence-terminating periods
        while p.eat(&Token::Period) {}

        let tok = p.peek().clone();

        if tok == Token::Eof || stop(&tok) {
            break;
        }

        // Paragraph or section header: Identifier [SECTION] .
        if matches!(tok, Token::Identifier(_))
            && matches!(p.peek_at(1), Token::Period | Token::Section)
        {
            break;
        }

        // Division header appearing mid-body
        if matches!(
            tok,
            Token::Procedure | Token::Environment | Token::Data | Token::Identification
        ) {
            break;
        }

        if let Some(stmt) = parse_stmt(p) {
            stmts.push(stmt);
        } else {
            // Unknown / unrecognised token — skip and try to recover
            if !p.at(&Token::Eof) {
                p.emit_error(format!("unexpected token in statement: {:?}", p.peek()));
                p.advance();
            } else {
                break;
            }
        }
    }
    stmts
}

// ── Statement dispatcher ──────────────────────────────────────────────────────

/// Try to parse one statement at the current position.
/// Returns `None` if the current token does not start a known statement.
pub(crate) fn parse_stmt(p: &mut Parser) -> Option<Stmt> {
    // Statements introduced by a non-keyword word (SEARCH, UNLOCK, ALTER).
    if let Token::Identifier(w) = p.peek() {
        match w.to_ascii_uppercase().as_str() {
            "SEARCH" => return Some(parse_search(p)),
            "UNLOCK" | "ALTER" => return Some(parse_recognized_noop(p)),
            _ => {}
        }
    }
    match p.peek().clone() {
        // SORT I/O verbs — recognized; tied to the (incomplete) SORT runtime.
        Token::Release | Token::Return_ => Some(parse_recognized_noop(p)),
        Token::Move       => Some(parse_move(p)),
        Token::Add        => Some(parse_add(p)),
        Token::Subtract   => Some(parse_subtract(p)),
        Token::Multiply   => Some(parse_multiply(p)),
        Token::Divide     => Some(parse_divide(p)),
        Token::Compute    => Some(parse_compute(p)),
        Token::If         => Some(parse_if(p)),
        Token::Evaluate   => Some(parse_evaluate(p)),
        Token::Perform    => Some(parse_perform(p)),
        Token::Go | Token::GoTo | Token::GoBack => Some(parse_go(p)),
        Token::Continue   => Some(parse_continue(p)),
        Token::Stop       => Some(parse_stop(p)),
        Token::Exit       => Some(parse_exit(p)),
        Token::Open       => Some(parse_open(p)),
        Token::Close      => Some(parse_close(p)),
        Token::Read       => Some(parse_read(p)),
        Token::Write      => Some(parse_write(p)),
        Token::Rewrite    => Some(parse_rewrite(p)),
        Token::Delete     => Some(parse_delete(p)),
        Token::Start      => Some(parse_start(p)),
        Token::Accept     => Some(parse_accept(p)),
        Token::Display    => Some(parse_display(p)),
        Token::StringVerb => Some(parse_string_verb(p)),
        Token::Unstring   => Some(parse_unstring(p)),
        Token::Inspect    => Some(parse_inspect(p)),
        Token::Sort       => Some(parse_sort(p)),
        Token::Merge      => Some(parse_merge(p)),
        Token::Call       => Some(parse_call(p)),
        Token::Invoke     => Some(parse_invoke(p)),
        Token::Initialize => Some(parse_initialize_as_move(p)),
        Token::Set        => Some(parse_set(p)),
        Token::Cancel              => { p.advance(); p.eat_identifier(); None } // skip CANCEL
        // CoBolt animation verbs — parse as a no-op skip to end of sentence
        Token::Play | Token::StopAnim => {
            p.advance(); // consume PLAY / STOP-ANIMATION
            // skip everything until period or start of next statement
            while !matches!(p.peek(),
                Token::Period | Token::Eof |
                Token::Move | Token::If | Token::Perform | Token::Evaluate |
                Token::Add  | Token::Subtract | Token::Compute | Token::Call |
                Token::Invoke | Token::Stop | Token::GoBack | Token::Exit |
                Token::Display | Token::Accept | Token::Read | Token::Write |
                Token::Play | Token::StopAnim
            ) {
                p.advance();
            }
            None
        }
        Token::ExecRustBlock(_)   => Some(parse_exec_rust(p)),
        Token::Try                => Some(parse_try_catch(p)),
        Token::Throw              => Some(parse_throw(p)),
        _                          => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Individual statement parsers
// ─────────────────────────────────────────────────────────────────────────────

// ── MOVE ──────────────────────────────────────────────────────────────────────

fn parse_move(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // MOVE

    // MOVE CORRESPONDING / CORR
    if p.at(&Token::Corresponding) {
        p.advance();
        let from = parse_expr(p);
        p.expect(&Token::To);
        let to = parse_expr(p);
        return Stmt::MoveCorresponding { from, to, span };
    }

    let from = parse_expr(p);
    p.expect(&Token::To);

    let mut to = Vec::new();
    // Collect one or more receiving fields
    loop {
        to.push(parse_expr(p));
        // Keep collecting if another identifier/literal follows but not a keyword
        if !is_expr_start(p) {
            break;
        }
    }

    Stmt::Move { from, to, span }
}

// ── ADD ───────────────────────────────────────────────────────────────────────

/// Parse a list of arithmetic receivers, each optionally followed by `ROUNDED`:
/// `id1 [ROUNDED] id2 [ROUNDED] …`. Stops at `stop` tokens / end of sentence.
fn parse_receivers(p: &mut Parser, stop: &dyn Fn(&Token) -> bool) -> Vec<(Expr, bool)> {
    let mut out = Vec::new();
    while !stop(p.peek())
        && !p.at_end_of_sentence()
        && !p.at(&Token::Eof)
        && is_expr_start(p)
    {
        let e = parse_expr(p);
        let rounded = p.eat(&Token::Rounded);
        out.push((e, rounded));
    }
    out
}

fn parse_add(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // ADD

    // ADD CORRESPONDING group TO group [ROUNDED]
    if p.at(&Token::Corresponding) {
        p.advance();
        let from = parse_expr(p);
        p.eat(&Token::To);
        let to = parse_expr(p);
        let rounded = p.eat(&Token::Rounded);
        let (_se, _nse) = parse_size_error(p, &Token::EndAdd);
        p.eat(&Token::EndAdd);
        return Stmt::AddCorresponding { from, to, rounded, span };
    }

    let mut operands = Vec::new();
    // Collect sending operands until TO or GIVING (ADD a b GIVING c has no TO)
    while !p.at(&Token::To)
        && !p.at(&Token::Giving)
        && !p.at(&Token::Eof)
        && !p.at(&Token::Period)
    {
        if is_expr_start(p) {
            operands.push(parse_expr(p));
        } else {
            break; // unknown token — don't spin forever
        }
    }
    p.eat(&Token::To);

    // Receiving fields (each with optional ROUNDED), until GIVING or end.
    let to = parse_receivers(p, &|t| matches!(t, Token::Giving | Token::EndAdd));
    let giving = if p.eat(&Token::Giving) {
        parse_receivers(p, &|t| matches!(t, Token::EndAdd))
    } else {
        Vec::new()
    };
    let (on_size_error, not_on_size_error) = parse_size_error(p, &Token::EndAdd);
    p.eat(&Token::EndAdd);

    Stmt::Add { operands, to, giving, on_size_error, not_on_size_error, span }
}

/// Parse the optional `[ON] SIZE ERROR imp … [NOT ON SIZE ERROR imp …]` tail of an
/// arithmetic statement, scoped by its `END-…` terminator. Returns the two
/// imperative bodies (each empty when absent).
fn parse_size_error(p: &mut Parser, end: &Token) -> (Vec<Stmt>, Vec<Stmt>) {
    let mut on_se = Vec::new();
    let mut not_se = Vec::new();
    // `[ON] SIZE ERROR imperative …`
    if try_eat_size_error_phrase(p) {
        let e = end.clone();
        on_se = parse_stmts(p, &move |tok| *tok == Token::Not || *tok == e);
    }
    // `NOT [ON] SIZE ERROR imperative …`
    if p.at(&Token::Not) {
        p.eat(&Token::Not);
        try_eat_size_error_phrase(p);
        let e = end.clone();
        not_se = parse_stmts(p, &move |tok| *tok == e);
    }
    (on_se, not_se)
}

/// Consume `[ON] SIZE ERROR`. The lexer emits `SIZE` as the `SizeError` token and
/// `ERROR` as a separate word, so both are eaten here. Returns whether it matched.
fn try_eat_size_error_phrase(p: &mut Parser) -> bool {
    let has = p.at(&Token::SizeError)
        || (p.at(&Token::On) && *p.peek_at(1) == Token::SizeError);
    if !has {
        return false;
    }
    p.eat(&Token::On); // optional ON
    p.eat(&Token::SizeError); // the SIZE keyword
    if let Token::Identifier(s) = p.peek() {
        if s.eq_ignore_ascii_case("ERROR") {
            p.advance();
        }
    }
    true
}

// ── SUBTRACT ──────────────────────────────────────────────────────────────────

fn parse_subtract(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // SUBTRACT

    // SUBTRACT CORRESPONDING group FROM group [ROUNDED]
    if p.at(&Token::Corresponding) {
        p.advance();
        let from = parse_expr(p);
        p.eat(&Token::From);
        let to = parse_expr(p);
        let rounded = p.eat(&Token::Rounded);
        let (_se, _nse) = parse_size_error(p, &Token::EndSubtract);
        p.eat(&Token::EndSubtract);
        return Stmt::SubtractCorresponding { from, to, rounded, span };
    }

    let mut operands = Vec::new();
    while !p.at(&Token::From)
        && !p.at(&Token::Giving)
        && !p.at(&Token::Eof)
        && !p.at(&Token::Period)
    {
        if is_expr_start(p) {
            operands.push(parse_expr(p));
        } else {
            break;
        }
    }
    p.eat(&Token::From);

    let from = parse_receivers(p, &|t| matches!(t, Token::Giving | Token::EndSubtract));
    let giving = if p.eat(&Token::Giving) {
        parse_receivers(p, &|t| matches!(t, Token::EndSubtract))
    } else {
        Vec::new()
    };
    let (on_size_error, not_on_size_error) = parse_size_error(p, &Token::EndSubtract);
    p.eat(&Token::EndSubtract);

    Stmt::Subtract { operands, from, giving, on_size_error, not_on_size_error, span }
}

// ── MULTIPLY ──────────────────────────────────────────────────────────────────

fn parse_multiply(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // MULTIPLY
    let lhs = parse_expr(p);
    p.eat(&Token::By);
    let by = parse_expr(p);
    // `MULTIPLY a BY b ROUNDED` (b is receiver) or `… GIVING r1 [ROUNDED] r2 …`.
    let rounded = p.eat(&Token::Rounded);
    let giving = if p.eat(&Token::Giving) {
        parse_receivers(p, &|t| matches!(t, Token::EndMultiply))
    } else {
        Vec::new()
    };
    let (on_size_error, not_on_size_error) = parse_size_error(p, &Token::EndMultiply);
    p.eat(&Token::EndMultiply);
    Stmt::Multiply { lhs, by, giving, rounded, on_size_error, not_on_size_error, span }
}

// ── DIVIDE ────────────────────────────────────────────────────────────────────

fn parse_divide(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // DIVIDE
    let lhs = parse_expr(p);
    // BY or INTO
    p.eat(&Token::By);
    p.eat(&Token::Into);
    let by = parse_expr(p);
    // `DIVIDE a INTO b ROUNDED` (b is receiver) or `… GIVING r1 [ROUNDED] r2 …
    // [REMAINDER r]`.
    let rounded = p.eat(&Token::Rounded);
    let giving = if p.eat(&Token::Giving) {
        parse_receivers(p, &|t| matches!(t, Token::Remainder | Token::EndDivide))
    } else {
        Vec::new()
    };
    let remainder = if p.eat(&Token::Remainder) { Some(parse_expr(p)) } else { None };
    let (on_size_error, not_on_size_error) = parse_size_error(p, &Token::EndDivide);
    p.eat(&Token::EndDivide);
    Stmt::Divide { lhs, by, giving, remainder, rounded, on_size_error, not_on_size_error, span }
}

// ── COMPUTE ───────────────────────────────────────────────────────────────────

fn parse_compute(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // COMPUTE
    // `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expression`.
    let mut targets = Vec::new();
    while !p.at(&Token::Eq) && !p.at(&Token::Eof) && !p.at(&Token::Period) {
        let t = parse_expr(p);
        let rounded = p.eat(&Token::Rounded);
        targets.push((t, rounded));
        if !is_expr_start(p) {
            break;
        }
    }
    p.expect(&Token::Eq); // =
    let expr = parse_expr(p);
    let (on_size_error, not_on_size_error) = parse_size_error(p, &Token::EndCompute);
    p.eat(&Token::EndCompute);
    Stmt::Compute { targets, expr, on_size_error, not_on_size_error, span }
}

// ── IF ────────────────────────────────────────────────────────────────────────

fn parse_if(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // IF

    let condition = parse_condition(p);

    // Optional THEN
    if let Token::Identifier(ref s) = p.peek().clone() {
        if s.to_uppercase() == "THEN" { p.advance(); }
    }

    let then_stmts = parse_stmts(p, &|tok| {
        matches!(tok, Token::Else | Token::EndIf)
    });

    let else_stmts = if p.eat(&Token::Else) {
        parse_stmts(p, &|tok| matches!(tok, Token::EndIf))
    } else {
        Vec::new()
    };

    p.eat(&Token::EndIf);

    Stmt::If { condition, then_stmts, else_stmts, span }
}

// ── EVALUATE ──────────────────────────────────────────────────────────────────

fn parse_evaluate(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // EVALUATE

    // One or more subjects, separated by ALSO.
    let mut subjects = Vec::new();
    loop {
        let subj = match p.peek().clone() {
            Token::True_  => { p.advance(); EvalSubject::True_ }
            Token::False_ => { p.advance(); EvalSubject::False_ }
            _             => EvalSubject::Expr(parse_expr(p)),
        };
        subjects.push(subj);
        if !p.at(&Token::Also) { break; }
        p.advance(); // ALSO
    }

    let mut whens: Vec<WhenClause> = Vec::new();
    let mut other_stmts: Vec<Stmt> = Vec::new();

    while p.at(&Token::When) {
        let when_span = p.peek_span();
        p.advance(); // WHEN

        if p.at(&Token::Other) {
            p.advance();
            other_stmts = parse_stmts(p, &|tok| {
                matches!(tok, Token::EndEvaluate | Token::When)
            });
            break;
        }

        // Collect one or more values for this WHEN
        let mut values = Vec::new();
        loop {
            let wv = parse_when_value(p);
            values.push(wv);
            if !p.at(&Token::Also) { break; }
            p.advance(); // ALSO — for multiple subjects (simplified: collect more values)
        }

        let stmts = parse_stmts(p, &|tok| {
            matches!(tok, Token::When | Token::EndEvaluate)
        });

        let ws = when_span.merge(p.peek_span());
        whens.push(WhenClause { values, stmts, span: ws });
    }

    p.eat(&Token::EndEvaluate);

    Stmt::Evaluate { subjects, whens, other_stmts, span }
}

fn parse_when_value(p: &mut Parser) -> WhenValue {
    // ANY
    if let Token::Identifier(ref s) = p.peek().clone() {
        if s.to_uppercase() == "ANY" {
            p.advance();
            return WhenValue::Any;
        }
    }
    // OTHER (also valid here)
    if p.at(&Token::Other) {
        p.advance();
        return WhenValue::Other;
    }
    // NOT {literal | literal THRU literal | condition}
    if p.eat(&Token::Not) {
        let inner = parse_when_value(p);
        return WhenValue::Not(Box::new(inner));
    }
    if let Some((lit, _)) = parse_literal(p) {
        if p.at(&Token::Through) {
            p.advance();
            if let Some((lit2, _)) = parse_literal(p) {
                return WhenValue::Range(lit, lit2);
            }
        }
        return WhenValue::Literal(lit);
    }
    // Condition-based WHEN
    let cond = parse_condition(p);
    WhenValue::Condition(cond)
}

// ── PERFORM ───────────────────────────────────────────────────────────────────

fn parse_perform(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // PERFORM

    // PERFORM VARYING …
    if p.at(&Token::Varying) {
        let target = parse_perform_varying(p);
        return Stmt::Perform { target, span };
    }

    // PERFORM UNTIL … (inline, test-before default)
    if p.at(&Token::Until) {
        p.advance();
        let condition = parse_condition(p);
        let stmts = parse_stmts(p, &|tok| matches!(tok, Token::EndPerform));
        p.eat(&Token::EndPerform);
        let target = PerformTarget::Until { condition, test_before: true, stmts };
        return Stmt::Perform { target, span };
    }

    // PERFORM TEST BEFORE/AFTER UNTIL …
    if p.at(&Token::Test) {
        p.advance();
        let test_before = !p.eat(&Token::After);
        p.eat(&Token::Before);
        p.expect(&Token::Until);
        let condition = parse_condition(p);
        let stmts = parse_stmts(p, &|tok| matches!(tok, Token::EndPerform));
        p.eat(&Token::EndPerform);
        let target = PerformTarget::Until { condition, test_before, stmts };
        return Stmt::Perform { target, span };
    }

    // Inline `PERFORM n TIMES … END-PERFORM` (count then TIMES, no paragraph).
    if (matches!(p.peek(), Token::IntegerLiteral(_)) || p.at_identifier())
        && matches!(p.peek_at(1), Token::Times)
    {
        let count = parse_expr(p);
        p.eat(&Token::Times);
        let stmts = parse_stmts(p, &|tok| matches!(tok, Token::EndPerform));
        p.eat(&Token::EndPerform);
        return Stmt::Perform { target: PerformTarget::Times { count, stmts }, span };
    }

    // Must have a paragraph/section name next
    if !p.at_identifier() {
        // Bare PERFORM with no argument — just a no-op stub
        let target = PerformTarget::Inline { stmts: Vec::new() };
        return Stmt::Perform { target, span };
    }

    let (name, _) = p.eat_identifier().unwrap();

    // PERFORM name THRU name
    let to_name = if p.at(&Token::Through) {
        p.advance();
        p.eat_identifier().map(|(n, _)| n)
    } else {
        None
    };

    // PERFORM name n TIMES
    if p.at(&Token::Times) || matches!(p.peek(), Token::IntegerLiteral(_)) {
        // Could be "PERFORM PARA 5 TIMES" or just TIMES after a number
        let count_expr = if matches!(p.peek(), Token::IntegerLiteral(_)) {
            let e = parse_expr(p);
            p.eat(&Token::Times);
            e
        } else {
            p.advance(); // eat TIMES
            Expr::Literal(Literal::Integer(1), span)
        };
        // Simple times — the paragraph name forms a non-inline target
        // For the AST, wrap as Times with para call inside
        let para_stmt = Stmt::Perform {
            target: if let Some(ref t) = to_name {
                PerformTarget::Thru {
                    from: name.clone(),
                    to: t.clone(),
                    span,
                }
            } else {
                PerformTarget::Paragraph(name, span)
            },
            span,
        };
        let target = PerformTarget::Times { count: count_expr, stmts: vec![para_stmt] };
        return Stmt::Perform { target, span };
    }

    // PERFORM name UNTIL cond [WITH TEST BEFORE/AFTER]
    if p.at(&Token::Until)
        || p.at(&Token::With)
        || p.at(&Token::Test)
    {
        let test_before = if p.eat(&Token::With) {
            p.eat(&Token::Test);
            if p.eat(&Token::After) { false } else { p.eat(&Token::Before); true }
        } else if p.eat(&Token::Test) {
            if p.eat(&Token::After) { false } else { p.eat(&Token::Before); true }
        } else {
            true
        };
        p.eat(&Token::Until);
        let condition = parse_condition(p);
        let para_stmt = Stmt::Perform {
            target: if let Some(ref t) = to_name {
                PerformTarget::Thru { from: name.clone(), to: t.clone(), span }
            } else {
                PerformTarget::Paragraph(name, span)
            },
            span,
        };
        let target = PerformTarget::Until { condition, test_before, stmts: vec![para_stmt] };
        return Stmt::Perform { target, span };
    }

    // PERFORM name VARYING …
    if p.at(&Token::Varying) {
        let varying_target = parse_perform_varying(p);
        // Embed the paragraph reference inside the varying
        // For MVP, we just return the varying target (paragraph ignored)
        return Stmt::Perform { target: varying_target, span };
    }

    // Simple PERFORM name [THRU name]
    let target = if let Some(t) = to_name {
        PerformTarget::Thru { from: name, to: t, span }
    } else {
        PerformTarget::Paragraph(name, span)
    };
    Stmt::Perform { target, span }
}

fn parse_perform_varying(p: &mut Parser) -> PerformTarget {
    p.advance(); // VARYING
    let var = parse_expr(p);
    p.eat(&Token::From);
    let from = parse_expr(p);
    p.eat(&Token::By);
    let by = parse_expr(p);
    p.eat(&Token::Until);
    let until = parse_condition(p);

    // AFTER sub-varying clauses
    let mut after = Vec::new();
    while p.at(&Token::After) {
        p.advance();
        let av = parse_expr(p);
        p.eat(&Token::From);
        let af = parse_expr(p);
        p.eat(&Token::By);
        let ab = parse_expr(p);
        p.eat(&Token::Until);
        let au = parse_condition(p);
        after.push(VaryingAfter { var: av, from: af, by: ab, until: au });
    }

    let stmts = parse_stmts(p, &|tok| matches!(tok, Token::EndPerform));
    p.eat(&Token::EndPerform);

    PerformTarget::Varying { var, from, by, until, stmts, after }
}

// ── GO TO / GO BACK ───────────────────────────────────────────────────────────

fn parse_go(p: &mut Parser) -> Stmt {
    let span = p.peek_span();

    // Token::GoBack → GO-BACK form
    if p.at(&Token::GoBack) {
        p.advance();
        return Stmt::GoBack { span };
    }

    p.advance(); // GO or GO-TO

    // GO BACK (two-word form: GO + Identifier("BACK"))
    if let Token::Identifier(ref s) = p.peek().clone() {
        if s.to_uppercase() == "BACK" {
            p.advance();
            return Stmt::GoBack { span };
        }
    }

    // GO TO paragraph [paragraph …] [DEPENDING ON data-item]
    p.eat(&Token::To); // consume TO if present

    let mut targets = Vec::new();
    while p.at_identifier() {
        let (name, _) = p.eat_identifier().unwrap();
        targets.push(name);
        // Stop before DEPENDING
        if p.at(&Token::Depending) { break; }
    }

    if p.at(&Token::Depending) {
        p.advance();
        p.eat(&Token::On);
        let depending = parse_expr(p);
        return Stmt::GoToDepending { targets, depending, span };
    }

    let target = targets.into_iter().next().unwrap_or_else(|| {
        p.emit_error("expected paragraph name after GO TO");
        "<missing>".into()
    });
    Stmt::GoTo { target, span }
}

// ── CONTINUE ──────────────────────────────────────────────────────────────────

fn parse_continue(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // CONTINUE
    Stmt::Continue { span }
}

/// Recognize a verb we don't execute yet (UNLOCK, ALTER, RELEASE, RETURN):
/// consume its operands up to the sentence end / next verb and emit a no-op.
fn parse_recognized_noop(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // the verb
    while !p.at(&Token::Period) && !p.at(&Token::Eof) && !is_stmt_start(p.peek()) {
        p.advance();
    }
    Stmt::Continue { span }
}

/// Parse `SEARCH [ALL] table [VARYING idx] [AT END imp] {WHEN cond imp}…
/// END-SEARCH`.
fn parse_search(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // SEARCH (an identifier)
    let all = is_word(p.peek(), "ALL") && { p.advance(); true };
    let table = parse_expr(p);

    let varying = if p.at(&Token::Varying) {
        p.advance();
        Some(parse_expr(p))
    } else {
        None
    };

    // AT END imperative
    let stop = |t: &Token| matches!(t, Token::When | Token::EndSearch);
    let at_end = if eat_at_end(p) { parse_stmts(p, &stop) } else { Vec::new() };

    // WHEN condition imperative …
    let mut whens = Vec::new();
    while p.at(&Token::When) {
        p.advance();
        let cond = parse_condition(p);
        let body = parse_stmts(p, &stop);
        whens.push((cond, body));
    }

    p.eat(&Token::EndSearch);
    Stmt::Search { all, table, varying, at_end, whens, span }
}

// ── STOP ──────────────────────────────────────────────────────────────────────

fn parse_stop(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // STOP
    let run = p.eat(&Token::Run);
    let literal = if !run {
        parse_literal(p).map(|(l, _)| l)
    } else {
        None
    };
    Stmt::Stop { run, literal, span }
}

// ── EXIT ──────────────────────────────────────────────────────────────────────

fn parse_exit(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // EXIT
    // EXIT [PROGRAM | PERFORM [CYCLE] | PARAGRAPH | SECTION] — the qualifier
    // words PERFORM / PARAGRAPH / SECTION / CYCLE arrive as identifier tokens.
    let kind = if p.eat(&Token::Program) {
        ExitKind::Program
    } else if p.at(&Token::Perform) {
        p.advance();
        if matches!(ident_upper(p).as_deref(), Some("CYCLE")) {
            p.advance();
            ExitKind::PerformCycle
        } else {
            ExitKind::Perform
        }
    } else {
        match ident_upper(p).as_deref() {
            Some("PARAGRAPH") => { p.advance(); ExitKind::Paragraph }
            Some("SECTION")   => { p.advance(); ExitKind::Section }
            _ => ExitKind::Point,
        }
    };
    Stmt::Exit { kind, span }
}

// ── OPEN ──────────────────────────────────────────────────────────────────────

fn parse_open(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // OPEN

    let mode = match p.peek().clone() {
        Token::Input  => { p.advance(); OpenMode::Input }
        Token::Output => { p.advance(); OpenMode::Output }
        Token::IoMode => { p.advance(); OpenMode::InputOutput }
        Token::Extend => { p.advance(); OpenMode::Extend }
        _ => {
            p.emit_error(format!("expected INPUT/OUTPUT/I-O/EXTEND, found {:?}", p.peek()));
            OpenMode::Input
        }
    };

    let mut files = Vec::new();
    while p.at_identifier() {
        let (name, _) = p.eat_identifier().unwrap();
        files.push(name);
    }

    Stmt::Open { mode, files, span }
}

// ── CLOSE ─────────────────────────────────────────────────────────────────────

fn parse_close(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // CLOSE
    let mut files = Vec::new();
    while p.at_identifier() {
        let (name, _) = p.eat_identifier().unwrap();
        files.push(name);
    }
    Stmt::Close { files, span }
}

// ── READ ──────────────────────────────────────────────────────────────────────

/// True if `tok` is an identifier equal (case-insensitively) to `w`.
fn is_word(tok: &Token, w: &str) -> bool {
    matches!(tok, Token::Identifier(s) if s.eq_ignore_ascii_case(w))
}

/// Consume an `AT END` phrase — either the single `AtEnd` token or the two
/// words `AT` `END`. Returns whether it was present.
/// Consume an `INVALID KEY` phrase (`InvalidKey` [`KEY`]).
fn eat_invalid_key(p: &mut Parser) -> bool {
    if p.at(&Token::InvalidKey) {
        p.advance(); // INVALID
        p.eat(&Token::Key); // optional trailing KEY word
        return true;
    }
    false
}

/// Consume a `NOT INVALID KEY` phrase (`NotInvalidKey`, or `NOT INVALID` [`KEY`]).
fn eat_not_invalid_key(p: &mut Parser) -> bool {
    if p.at(&Token::NotInvalidKey) {
        p.advance();
        p.eat(&Token::Key);
        return true;
    }
    if p.at(&Token::Not) && matches!(p.peek_at(1), Token::InvalidKey) {
        p.advance(); // NOT
        p.advance(); // INVALID
        p.eat(&Token::Key);
        return true;
    }
    false
}

fn eat_at_end(p: &mut Parser) -> bool {
    if p.eat(&Token::AtEnd) {
        return true;
    }
    if is_word(p.peek(), "AT") && matches!(p.peek_at(1), Token::End | Token::AtEnd) {
        p.advance(); // AT
        p.advance(); // END
        return true;
    }
    false
}

/// Consume a `NOT AT END` phrase (`NotAtEnd`, or `NOT` [`AT`] `END`).
fn eat_not_at_end(p: &mut Parser) -> bool {
    if p.eat(&Token::NotAtEnd) {
        return true;
    }
    if p.at(&Token::Not) {
        let n1 = p.peek_at(1);
        let n2 = p.peek_at(2);
        let is_not_at_end = matches!(n1, Token::AtEnd | Token::End)
            || (is_word(n1, "AT") && matches!(n2, Token::End | Token::AtEnd));
        if is_not_at_end {
            p.advance(); // NOT
            if is_word(p.peek(), "AT") { p.advance(); }
            p.eat(&Token::AtEnd);
            p.eat(&Token::End);
            return true;
        }
    }
    false
}

fn parse_read(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // READ
    let file = p.expect_identifier("READ file name");
    p.eat(&Token::Record); // optional RECORD keyword

    // Optional NEXT / PREVIOUS direction (plain words, not keywords). Either may
    // be preceded/followed by the noise word RECORD.
    let direction = if is_word(p.peek(), "NEXT") {
        p.advance();
        p.eat(&Token::Record);
        ReadDirection::Next
    } else if is_word(p.peek(), "PREVIOUS") {
        p.advance();
        p.eat(&Token::Record);
        ReadDirection::Previous
    } else {
        ReadDirection::Default
    };

    let into = if p.at(&Token::Into) {
        p.advance();
        Some(parse_expr(p))
    } else {
        None
    };

    let key = if p.at(&Token::Key) {
        p.advance();
        p.eat(&Token::Is);
        Some(parse_expr(p))
    } else {
        None
    };

    // AT END … [NOT AT END …]. `AT END` may arrive as the single `AtEnd` token
    // or as the two words `AT` + `END`; likewise `NOT AT END`.
    let stop = |tok: &Token| matches!(
        tok,
        Token::Not | Token::NotAtEnd | Token::NotInvalidKey | Token::InvalidKey | Token::EndRead
    );
    let at_end = if eat_at_end(p) { parse_stmts(p, &stop) } else { Vec::new() };
    let not_at_end = if eat_not_at_end(p) { parse_stmts(p, &stop) } else { Vec::new() };
    // INVALID KEY / NOT INVALID KEY (random reads use these instead of AT END).
    let invalid_key = if eat_invalid_key(p) { parse_stmts(p, &stop) } else { Vec::new() };
    let not_invalid_key = if eat_not_invalid_key(p) { parse_stmts(p, &stop) } else { Vec::new() };

    p.eat(&Token::EndRead);

    Stmt::Read {
        file, into, key, direction,
        at_end, not_at_end, invalid_key, not_invalid_key, span,
    }
}

// ── WRITE ─────────────────────────────────────────────────────────────────────

fn parse_write(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // WRITE
    let record = parse_expr(p);

    let from = if p.at(&Token::From) {
        p.advance();
        Some(parse_expr(p))
    } else {
        None
    };

    // BEFORE/AFTER ADVANCING lines LINES
    let advancing = if p.at(&Token::Before) || p.at(&Token::After) || p.at(&Token::Advancing) {
        let before = p.eat(&Token::Before);
        if !before { p.eat(&Token::After); }
        p.eat(&Token::Advancing);
        let lines = parse_expr(p);
        p.eat(&Token::Line);
        p.eat(&Token::Lines);
        Some(AdvancingClause { lines, before })
    } else {
        None
    };

    let stop = |tok: &Token| matches!(tok, Token::Not | Token::NotInvalidKey | Token::InvalidKey | Token::EndWrite);
    let invalid_key = if eat_invalid_key(p) { parse_stmts(p, &stop) } else { Vec::new() };
    let not_invalid_key = if eat_not_invalid_key(p) { parse_stmts(p, &stop) } else { Vec::new() };

    p.eat(&Token::EndWrite);

    Stmt::Write { record, from, advancing, invalid_key, not_invalid_key, span }
}

// ── REWRITE ───────────────────────────────────────────────────────────────────

fn parse_rewrite(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // REWRITE
    let record = parse_expr(p);
    let from = if p.at(&Token::From) { p.advance(); Some(parse_expr(p)) } else { None };
    let stop = |tok: &Token| matches!(tok, Token::Not | Token::NotInvalidKey | Token::InvalidKey | Token::EndRewrite);
    let invalid_key = if eat_invalid_key(p) { parse_stmts(p, &stop) } else { Vec::new() };
    let not_invalid_key = if eat_not_invalid_key(p) { parse_stmts(p, &stop) } else { Vec::new() };
    p.eat(&Token::EndRewrite);
    Stmt::Rewrite { record, from, invalid_key, not_invalid_key, span }
}

// ── DELETE ────────────────────────────────────────────────────────────────────

fn parse_delete(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // DELETE
    let file = p.expect_identifier("DELETE file name");
    p.eat(&Token::Record);
    let stop = |tok: &Token| matches!(tok, Token::Not | Token::NotInvalidKey | Token::InvalidKey | Token::EndDelete);
    let invalid_key = if eat_invalid_key(p) { parse_stmts(p, &stop) } else { Vec::new() };
    let not_invalid_key = if eat_not_invalid_key(p) { parse_stmts(p, &stop) } else { Vec::new() };
    p.eat(&Token::EndDelete);
    Stmt::Delete { file, invalid_key, not_invalid_key, span }
}

// ── START ─────────────────────────────────────────────────────────────────────

fn parse_start(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // START
    let file = p.expect_identifier("START file name");

    let key = if p.at(&Token::Key) {
        p.advance();
        // IS [NOT] EQUAL/GREATER/LESS …
        let op = parse_start_key_op(p);
        let field = parse_expr(p);
        Some((op, field))
    } else {
        None
    };

    let stop = |tok: &Token| matches!(tok, Token::Not | Token::NotInvalidKey | Token::InvalidKey | Token::EndStart);
    let invalid_key = if eat_invalid_key(p) { parse_stmts(p, &stop) } else { Vec::new() };
    let not_invalid_key = if eat_not_invalid_key(p) { parse_stmts(p, &stop) } else { Vec::new() };
    p.eat(&Token::EndStart);
    Stmt::Start { file, key, invalid_key, not_invalid_key, span }
}

fn parse_start_key_op(p: &mut Parser) -> CmpOp {
    p.eat(&Token::Is);
    let negated = p.eat(&Token::Not);
    if p.eat(&Token::Equal) {
        p.eat(&Token::To);
        if negated { CmpOp::Ne } else { CmpOp::Eq }
    } else if p.eat(&Token::Greater) {
        p.eat(&Token::Than);
        // GREATER THAN OR EQUAL TO → ≥
        if p.eat(&Token::Or) {
            p.eat(&Token::Equal);
            p.eat(&Token::To);
            if negated { CmpOp::Lt } else { CmpOp::Ge }
        } else if negated { CmpOp::Le } else { CmpOp::Gt }
    } else if p.eat(&Token::Less) {
        p.eat(&Token::Than);
        // LESS THAN OR EQUAL TO → ≤
        if p.eat(&Token::Or) {
            p.eat(&Token::Equal);
            p.eat(&Token::To);
            if negated { CmpOp::Gt } else { CmpOp::Le }
        } else if negated { CmpOp::Ge } else { CmpOp::Lt }
    } else if p.at(&Token::Eq) {
        p.advance();
        if negated { CmpOp::Ne } else { CmpOp::Eq }
    } else if p.at(&Token::Gt) {
        p.advance();
        if negated { CmpOp::Le } else { CmpOp::Gt }
    } else if p.at(&Token::Lt) {
        p.advance();
        if negated { CmpOp::Ge } else { CmpOp::Lt }
    } else if p.at(&Token::GtEq) {
        p.advance();
        if negated { CmpOp::Lt } else { CmpOp::Ge }
    } else if p.at(&Token::LtEq) {
        p.advance();
        if negated { CmpOp::Gt } else { CmpOp::Le }
    } else {
        CmpOp::Eq
    }
}

// ── ACCEPT ────────────────────────────────────────────────────────────────────

fn parse_accept(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // ACCEPT
    let target = parse_expr(p);

    // Optional screen position before / after FROM (parsed, not executed —
    // terminal screen handling is superseded by the form designer).
    eat_accept_screen(p);

    let from = if p.at(&Token::From) {
        p.advance();
        Some(parse_accept_source(p))
    } else {
        None
    };

    // Optional trailing screen position / attribute phrases.
    eat_accept_screen(p);

    Stmt::Accept { target, from, span }
}

/// Consume the screen-handling phrases of an extended ACCEPT — `AT {nnnn |
/// LINE n [COLUMN n]}` and `WITH attribute …` — which PowerRustCOBOL recognizes
/// but does not execute (the visual designer supersedes SCREEN SECTION I/O).
fn eat_accept_screen(p: &mut Parser) {
    let stop = |p: &Parser| {
        p.at(&Token::With) || p.at(&Token::From) || p.at(&Token::Upon)
            || p.at(&Token::No) || p.at(&Token::Eof) || p.at(&Token::Period)
            || is_stmt_start(p.peek())
    };
    loop {
        if is_word(p.peek(), "AT") {
            p.advance(); // AT — then the position tokens (LINE/COLUMN words + numbers)
            while !stop(p) {
                p.advance();
            }
        } else if p.at(&Token::With) && !is_word(p.peek_at(1), "NO") {
            p.advance(); // WITH — then attribute words up to the next clause.
            while !stop(p) {
                p.advance();
            }
        } else {
            break;
        }
    }
}

fn parse_accept_source(p: &mut Parser) -> AcceptSource {
    if let Some(name) = ident_upper(p) {
        let src = match name.as_str() {
            "DATE"        => Some(AcceptSource::Date),
            "TIME"        => Some(AcceptSource::Time),
            "DAY"         => Some(AcceptSource::Day),
            "DAY-OF-WEEK" => Some(AcceptSource::DayOfWeek),
            "COMMAND-LINE" => Some(AcceptSource::CommandLine),
            // Recognized but read as a no-op (argument / environment registers).
            "ARGUMENT-NUMBER" | "ARGUMENT-VALUE" | "ENVIRONMENT-VALUE" => {
                Some(AcceptSource::CommandLine)
            }
            _ => None,
        };
        if let Some(s) = src {
            p.advance();
            return s;
        }
        // Two-word registers: `ESCAPE KEY`, `CRT STATUS` — recognized no-ops.
        if name == "ESCAPE" || name == "CRT" {
            p.advance(); // ESCAPE / CRT
            p.advance(); // KEY / STATUS (keyword or identifier)
            return AcceptSource::CommandLine;
        }
        // ENVIRONMENT "name"
        if name == "ENVIRONMENT" {
            p.advance();
            if let Some((s, _)) = p.eat_string() {
                return AcceptSource::Environment(s);
            }
            return AcceptSource::Environment("<missing>".into());
        }
        // mnemonic name
        p.advance();
        AcceptSource::Environment(name)
    } else {
        AcceptSource::Date // fallback
    }
}

// ── DISPLAY ───────────────────────────────────────────────────────────────────

fn parse_display(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // DISPLAY

    let mut operands = Vec::new();
    // Collect operands until UPON, WITH, NO, AT (screen position), or end.
    while is_expr_start(p)
        && !p.at(&Token::Upon)
        && !p.at(&Token::With)
        && !p.at(&Token::No)
        && !is_word(p.peek(), "AT")
    {
        operands.push(parse_expr(p));
    }

    // Optional screen position / attribute phrases (recognized, not executed).
    eat_accept_screen(p);

    let upon = if p.eat(&Token::Upon) {
        p.eat_identifier().map(|(s, _)| s)
    } else {
        None
    };

    // WITH NO ADVANCING  or  NO ADVANCING
    p.eat(&Token::With); // optional WITH
    let no_advancing = p.eat(&Token::No) && { p.eat(&Token::Advancing); true };

    Stmt::Display { operands, upon, no_advancing, span }
}

// ── STRING verb ───────────────────────────────────────────────────────────────

fn parse_string_verb(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // STRING

    let mut operands: Vec<(Expr, Option<Expr>)> = Vec::new();

    // Collect (source DELIMITED BY delimiter) pairs
    while is_expr_start(p)
        && !p.at(&Token::Into)
        && !p.at(&Token::Eof)
    {
        let src = parse_expr(p);
        let delim = if p.at(&Token::Delimited) {
            p.advance();
            p.eat(&Token::By);
            // `DELIMITED BY SIZE`: the lexer maps the bare word SIZE to the
            // `SizeError` token (reserved for ON SIZE ERROR). Recognise it here
            // and treat it as the SIZE delimiter (append the whole source).
            if p.at(&Token::SizeError) {
                let sp = p.peek_span();
                p.advance();
                Some(Expr::Literal(Literal::String("SIZE".to_string()), sp))
            } else {
                Some(parse_expr(p))
            }
        } else {
            None
        };
        operands.push((src, delim));
    }

    p.expect(&Token::Into);
    let into = parse_expr(p);

    let pointer = if p.at(&Token::With) {
        p.advance();
        p.eat(&Token::Pointer);
        Some(parse_expr(p))
    } else if p.at(&Token::Pointer) {
        p.advance();
        Some(parse_expr(p))
    } else {
        None
    };

    // [ON OVERFLOW imp] [NOT ON OVERFLOW imp] [END-STRING]
    let stop = |t: &Token| matches!(t, Token::Not) || matches!(t, Token::EndString);
    let on_overflow = if eat_on_overflow(p) { parse_stmts(p, &stop) } else { Vec::new() };
    let not_on_overflow = if p.at(&Token::Not) {
        p.advance();
        eat_on_overflow(p);
        parse_stmts(p, &|t| matches!(t, Token::EndString))
    } else {
        Vec::new()
    };
    if p.at(&Token::EndString) { p.advance(); }
    p.eat(&Token::EndCall);

    Stmt::String_ { operands, into, pointer, on_overflow, not_on_overflow, span }
}

/// Consume `[ON] OVERFLOW` of a STRING/UNSTRING. Returns whether it matched.
fn eat_on_overflow(p: &mut Parser) -> bool {
    let has = is_word(p.peek(), "OVERFLOW")
        || (p.at(&Token::On) && is_word(p.peek_at(1), "OVERFLOW"));
    if !has {
        return false;
    }
    p.eat(&Token::On);
    if is_word(p.peek(), "OVERFLOW") { p.advance(); }
    true
}

// ── UNSTRING ──────────────────────────────────────────────────────────────────

fn parse_unstring(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // UNSTRING
    let from = parse_expr(p);

    // DELIMITED BY [ALL] delim [OR [ALL] delim …]
    let mut delimited_by = Vec::new();
    let mut all = false;
    if p.eat(&Token::Delimited) {
        p.eat(&Token::By);
        all = p.eat(&Token::All);
        delimited_by.push(parse_expr(p));
        while p.at(&Token::Or) {
            p.advance();
            p.eat(&Token::All);
            delimited_by.push(parse_expr(p));
        }
    }

    p.expect(&Token::Into);

    let mut into: Vec<UnstringTarget> = Vec::new();
    loop {
        if !is_expr_start(p) { break; }
        let tgt = parse_expr(p);
        let delimiter = if p.at(&Token::Delimited) {
            p.advance();
            p.eat(&Token::In);
            Some(parse_expr(p))
        } else {
            None
        };
        let count = if p.at(&Token::Count) {
            p.advance();
            p.eat(&Token::In);
            Some(parse_expr(p))
        } else {
            None
        };
        into.push(UnstringTarget { target: tgt, delimiter, count });
        p.eat(&Token::Comma);
    }

    let tallying = if p.at(&Token::Tallying) {
        p.advance();
        p.eat(&Token::In);
        Some(parse_expr(p))
    } else {
        None
    };

    let pointer = if p.at(&Token::With) {
        p.advance();
        p.eat(&Token::Pointer);
        Some(parse_expr(p))
    } else {
        None
    };

    // [ON OVERFLOW imp] [NOT ON OVERFLOW imp] [END-UNSTRING]
    let stop = |t: &Token| matches!(t, Token::Not) || matches!(t, Token::EndUnstring);
    let on_overflow = if eat_on_overflow(p) { parse_stmts(p, &stop) } else { Vec::new() };
    let not_on_overflow = if p.at(&Token::Not) {
        p.advance();
        eat_on_overflow(p);
        parse_stmts(p, &|t| matches!(t, Token::EndUnstring))
    } else {
        Vec::new()
    };
    if p.at(&Token::EndUnstring) { p.advance(); }

    Stmt::Unstring {
        from, delimited_by, all, into, pointer, tallying, on_overflow, not_on_overflow, span,
    }
}

// ── INSPECT ───────────────────────────────────────────────────────────────────

fn parse_inspect(p: &mut Parser) -> Stmt {
    use cobolt_ast::stmt::InspectSpec;

    let span = p.peek_span();
    p.advance(); // INSPECT
    let target = parse_expr(p);

    // CONVERTING literal TO literal  (simplest form)
    if p.at(&Token::Converting) {
        p.advance();
        let from = parse_expr(p);
        p.expect(&Token::To);
        let to = parse_expr(p);
        return Stmt::Inspect { target, spec: InspectSpec::Converting { from, to }, span };
    }

    // TALLYING … [REPLACING …]
    if p.at(&Token::Tallying) {
        p.advance();
        let tallies = parse_tally_specs(p);
        if p.at(&Token::Replacing) {
            p.advance();
            let specs = parse_replace_specs(p);
            return Stmt::Inspect {
                target,
                spec: InspectSpec::TallyingReplacing(tallies, specs),
                span,
            };
        }
        return Stmt::Inspect { target, spec: InspectSpec::Tallying(tallies), span };
    }

    // REPLACING …
    if p.at(&Token::Replacing) {
        p.advance();
        let specs = parse_replace_specs(p);
        return Stmt::Inspect { target, spec: InspectSpec::Replacing(specs), span };
    }

    // Fallback: empty tallying
    Stmt::Inspect { target, spec: InspectSpec::Tallying(Vec::new()), span }
}

/// Parse an optional `[BEFORE|AFTER] INITIAL delimiter` region qualifier.
fn parse_inspect_region(p: &mut Parser) -> cobolt_ast::stmt::InspectRegion {
    use cobolt_ast::stmt::InspectRegion;
    let mut region = InspectRegion::default();
    loop {
        let before = p.at(&Token::Before);
        let after = p.at(&Token::After);
        if !before && !after { break; }
        p.advance(); // BEFORE / AFTER
        // optional INITIAL
        if matches!(ident_upper(p).as_deref(), Some("INITIAL")) { p.advance(); }
        let delim = parse_expr(p);
        if before { region.before = Some(delim); } else { region.after = Some(delim); }
    }
    region
}

fn parse_tally_specs(p: &mut Parser) -> Vec<cobolt_ast::stmt::TallySpec> {
    use cobolt_ast::stmt::{TallyFor, TallySpec};
    let mut tallies: Vec<TallySpec> = Vec::new();
    while is_expr_start(p) && !p.at(&Token::Replacing) {
        let counter = parse_expr(p);
        p.eat_for_kw(); // FOR keyword
        let mut for_ = Vec::new();
        loop {
            let kind = if p.at(&Token::Characters) { p.advance(); TallyFor::Characters }
                else if p.at(&Token::All)      { p.advance(); TallyFor::All(parse_expr(p)) }
                else if p.at(&Token::Leading)  { p.advance(); TallyFor::Leading(parse_expr(p)) }
                else if p.at(&Token::Trailing) { p.advance(); TallyFor::Trailing(parse_expr(p)) }
                else { break; };
            let region = parse_inspect_region(p);
            for_.push((kind, region));
        }
        tallies.push(TallySpec { counter, for_ });
    }
    tallies
}

fn parse_replace_specs(p: &mut Parser) -> Vec<cobolt_ast::stmt::ReplaceSpec> {
    use cobolt_ast::stmt::{ReplaceSpec, ReplaceWhat};
    let mut specs: Vec<ReplaceSpec> = Vec::new();
    loop {
        let what = if p.at(&Token::Characters) { p.advance(); ReplaceWhat::Characters }
            else if p.at(&Token::All)     { p.advance(); ReplaceWhat::All(parse_expr(p)) }
            else if p.at(&Token::Leading) { p.advance(); ReplaceWhat::Leading(parse_expr(p)) }
            else if p.at(&Token::Trailing){ p.advance(); ReplaceWhat::Trailing(parse_expr(p)) }
            else if p.at_identifier() && ident_upper(p).as_deref() == Some("FIRST") {
                p.advance(); ReplaceWhat::First(parse_expr(p))
            }
            else { break; };
        // CHARACTERS BY x   /   {ALL|LEADING|…} x BY y
        p.eat(&Token::By);
        let by = parse_expr(p);
        let region = parse_inspect_region(p);
        specs.push(ReplaceSpec { what, by, region });
    }
    specs
}

// ── SORT ──────────────────────────────────────────────────────────────────────

fn parse_sort(p: &mut Parser) -> Stmt {
    use cobolt_ast::stmt::SortKey;
    let span = p.peek_span();
    p.advance(); // SORT
    let file = p.expect_identifier("SORT file name");

    let mut keys = Vec::new();
    while p.at(&Token::Ascending) || p.at(&Token::Descending) {
        let ascending = p.eat(&Token::Ascending);
        p.eat(&Token::Descending);
        p.eat(&Token::Key);
        p.eat(&Token::Is);
        let mut fields = Vec::new();
        while is_expr_start(p) && !p.at(&Token::Ascending) && !p.at(&Token::Descending) {
            fields.push(parse_expr(p));
        }
        keys.push(SortKey { ascending, fields });
    }

    // WITH DUPLICATES IN ORDER — ignored
    let duplicates = if p.at(&Token::With) {
        p.advance();
        let d = if let Token::Identifier(ref s) = p.peek().clone() {
            s.to_uppercase() == "DUPLICATES"
        } else { false };
        if d { p.advance(); p.eat(&Token::In); if p.at_identifier() { p.advance(); } }
        d
    } else {
        false
    };

    // INPUT PROCEDURE IS name [THRU name]
    let input_proc = if p.at(&Token::Input) {
        p.advance();
        p.eat(&Token::Procedure);
        p.eat(&Token::Is);
        let name = p.expect_identifier("INPUT PROCEDURE name");
        if p.at(&Token::Through) { p.advance(); p.eat_identifier(); }
        Some(name)
    } else {
        None
    };

    // OUTPUT PROCEDURE IS name [THRU name]
    let output_proc = if p.at(&Token::Output) {
        p.advance();
        p.eat(&Token::Procedure);
        p.eat(&Token::Is);
        let name = p.expect_identifier("OUTPUT PROCEDURE name");
        if p.at(&Token::Through) { p.advance(); p.eat_identifier(); }
        Some(name)
    } else {
        None
    };

    p.eat(&Token::EndSort);

    Stmt::Sort { file, keys, duplicates, input_proc, output_proc, span }
}

// ── MERGE ─────────────────────────────────────────────────────────────────────

fn parse_merge(p: &mut Parser) -> Stmt {
    use cobolt_ast::stmt::SortKey;
    let span = p.peek_span();
    p.advance(); // MERGE
    let file = p.expect_identifier("MERGE file name");

    let mut keys = Vec::new();
    while p.at(&Token::Ascending) || p.at(&Token::Descending) {
        let ascending = p.eat(&Token::Ascending);
        p.eat(&Token::Descending);
        p.eat(&Token::Key);
        p.eat(&Token::Is);
        let mut fields = Vec::new();
        while is_expr_start(p) && !p.at(&Token::Ascending) && !p.at(&Token::Descending) {
            fields.push(parse_expr(p));
        }
        keys.push(SortKey { ascending, fields });
    }

    let output_proc = if p.at(&Token::Output) {
        p.advance();
        p.eat(&Token::Procedure);
        p.eat(&Token::Is);
        let name = p.expect_identifier("OUTPUT PROCEDURE name");
        if p.at(&Token::Through) { p.advance(); p.eat_identifier(); }
        Some(name)
    } else {
        None
    };

    p.eat(&Token::EndMerge);
    Stmt::Merge { file, keys, output_proc, span }
}

// ── CALL ──────────────────────────────────────────────────────────────────────

fn parse_call(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // CALL

    let program = parse_expr(p);

    // USING [BY {REFERENCE|CONTENT|VALUE}] args…
    let mut using = Vec::new();
    if p.eat(&Token::Using) {
        loop {
            // BY comes first, then the mode keyword (all optional)
            p.eat(&Token::By);
            let by_ref = p.eat(&Token::Reference);
            let by_val = !by_ref && p.eat(&Token::Value);
            let by_cont = !by_ref && !by_val && {
                let is_content = matches!(ident_upper(p).as_deref(), Some("CONTENT"));
                if is_content { p.advance(); }
                is_content
            };

            if !is_expr_start(p) { break; }
            let arg = parse_expr(p);
            let call_arg = if by_val      { CallArg::ByValue(arg)     }
                else if by_cont           { CallArg::ByContent(arg)    }
                else                      { CallArg::ByReference(arg)  };
            using.push(call_arg);
        }
    }

    let returning = if p.eat(&Token::Returning) { Some(parse_expr(p)) } else { None };

    // [ON] {EXCEPTION | OVERFLOW} imperative … [NOT [ON] {EXCEPTION|OVERFLOW} …]
    let stop = |t: &Token| matches!(t, Token::Not | Token::EndCall);
    let mut on_exception: Vec<Stmt> = Vec::new();
    if eat_on_exception(p) {
        on_exception = parse_stmts(p, &stop);
    }
    // NOT [ON] {EXCEPTION | OVERFLOW} — runs when the call resolves.
    let mut not_on_exception: Vec<Stmt> = Vec::new();
    if p.at(&Token::Not) {
        p.advance();
        eat_on_exception(p);
        not_on_exception = parse_stmts(p, &|t| matches!(t, Token::EndCall));
    }
    p.eat(&Token::EndCall);

    Stmt::Call { program, using, returning, on_exception, not_on_exception, span }
}

/// Consume `[ON] {EXCEPTION | OVERFLOW}` of a CALL. Returns whether it matched.
fn eat_on_exception(p: &mut Parser) -> bool {
    let is_kw = |t: &Token| matches!(t, Token::Exception) || is_word(t, "OVERFLOW");
    let has = is_kw(p.peek()) || (p.at(&Token::On) && is_kw(p.peek_at(1)));
    if !has {
        return false;
    }
    p.eat(&Token::On);
    if p.at(&Token::Exception) {
        p.advance();
    } else if is_word(p.peek(), "OVERFLOW") {
        p.advance();
    }
    true
}

// ── INVOKE (OO-COBOL) ─────────────────────────────────────────────────────────
//
// OO extension: INVOKE object-ref 'method' [USING …] [RETURNING …]
// The parser does not execute INVOKE; it just skips tokens until the implicit
// sentence boundary (period or start of a new statement verb) so that the rest
// of the program can be compiled cleanly.

fn parse_invoke(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // INVOKE

    // Consume everything until period, EOF, or a new statement verb.
    loop {
        let tok = p.peek().clone();
        if matches!(tok, Token::Eof | Token::Period) { break; }
        // Stop before the next statement verb (but NOT before clause keywords
        // like USING, BY, VALUE, RETURNING which belong to INVOKE).
        if is_stmt_start(&tok) { break; }
        p.advance();
    }
    p.eat(&Token::Period);

    Stmt::Continue { span }
}

// ── INITIALIZE (simplified as MOVE SPACES) ────────────────────────────────────

fn parse_initialize_as_move(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // INITIALIZE
    let mut items = Vec::new();
    while is_expr_start(p) {
        items.push(parse_expr(p));
    }
    // REPLACING … (skip — TODO: honour the REPLACING category map)
    if p.at(&Token::Replacing) {
        while !p.at_end_of_sentence() && !p.at(&Token::Eof) { p.advance(); }
    }
    Stmt::Initialize { items, span }
}

// ── SET ───────────────────────────────────────────────────────────────────────

fn parse_set(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.advance(); // SET

    let mut targets = Vec::new();
    while is_expr_start(p) && !p.at(&Token::To) {
        // Stop before the UP/DOWN of `SET idx UP/DOWN BY n`.
        if matches!(ident_upper(p).as_deref(), Some("UP") | Some("DOWN")) {
            break;
        }
        targets.push(parse_expr(p));
    }

    // SET idx {UP|DOWN} BY n  → encode as ADD n TO idx / SUBTRACT n FROM idx.
    if let Some(dir) = ident_upper(p) {
        if dir == "UP" || dir == "DOWN" {
            p.advance(); // UP / DOWN
            p.eat(&Token::By);
            let amount = parse_expr(p);
            let recvs: Vec<(Expr, bool)> = targets.into_iter().map(|t| (t, false)).collect();
            return if dir == "DOWN" {
                Stmt::Subtract {
                    operands: vec![amount], from: recvs, giving: Vec::new(),
                    on_size_error: Vec::new(), not_on_size_error: Vec::new(), span,
                }
            } else {
                Stmt::Add {
                    operands: vec![amount], to: recvs, giving: Vec::new(),
                    on_size_error: Vec::new(), not_on_size_error: Vec::new(), span,
                }
            };
        }
    }

    p.eat(&Token::To);

    // TO TRUE / FALSE / expression
    let from = match p.peek().clone() {
        Token::True_  => { p.advance(); Expr::Literal(Literal::Integer(1), span) }
        Token::False_ => { p.advance(); Expr::Literal(Literal::Integer(0), span) }
        _             => parse_expr(p),
    };

    // Encode as MOVE
    Stmt::Move { from, to: targets, span }
}

// ── EXEC RUST ─────────────────────────────────────────────────────────────────

/// Parse an `EXEC RUST … END-EXEC` statement.
///
/// By the time this is called the lexer has already captured the entire block
/// into a single [`Token::ExecRustBlock(source)`] token.  All we do here is
/// consume that token, optionally eat a trailing period, and build the AST
/// node.
///
/// The `referenced_data` field is left empty; the semantic pass fills it in
/// by scanning `source` for snake_case names that correspond to known COBOL
/// data items.
fn parse_exec_rust(p: &mut Parser) -> Stmt {
    let span = p.peek_span();

    // Consume the ExecRustBlock token and extract the source string.
    let source = if let Token::ExecRustBlock(src) = p.peek().clone() {
        p.advance();
        src
    } else {
        p.emit_error("internal: parse_exec_rust called without ExecRustBlock token");
        String::new()
    };

    // Optional trailing period (sentence terminator after END-EXEC).
    p.eat(&Token::Period);

    Stmt::ExecRust {
        source,
        referenced_data: Vec::new(), // populated by semantic pass
        span,
    }
}

// ── TRY / CATCH EXCEPTION / FINALLY ──────────────────────────────────────────

/// Parse:
/// ```text
/// TRY
///     <stmts>
/// CATCH EXCEPTION <name>
///     <stmts>
/// [ FINALLY
///     <stmts> ]
/// END-TRY
/// ```
fn parse_try_catch(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.expect(&Token::Try);

    // TRY body
    let try_stmts = parse_stmts(p, &|t| {
        matches!(t, Token::Catch | Token::Finally | Token::EndTry | Token::Eof)
    });

    // CATCH EXCEPTION <name>  (optional)
    let mut exception_var  = None;
    let mut catch_stmts    = Vec::new();
    if p.eat(&Token::Catch) {
        p.eat(&Token::Exception); // optional EXCEPTION keyword
        // next token should be the variable name
        if let Token::Identifier(name) = p.peek().clone() {
            exception_var = Some(name.clone());
            p.advance();
        }
        catch_stmts = parse_stmts(p, &|t| {
            matches!(t, Token::Finally | Token::EndTry | Token::Eof)
        });
    }

    // FINALLY (optional)
    let mut finally_stmts = Vec::new();
    if p.eat(&Token::Finally) {
        finally_stmts = parse_stmts(p, &|t| {
            matches!(t, Token::EndTry | Token::Eof)
        });
    }

    p.eat(&Token::EndTry);
    p.eat(&Token::Period);

    Stmt::TryCatch { try_stmts, exception_var, catch_stmts, finally_stmts, span }
}

// ── THROW / RAISE ─────────────────────────────────────────────────────────────

fn parse_throw(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    p.expect(&Token::Throw);
    let message = parse_expr(p);
    p.eat(&Token::Period);
    Stmt::Throw { message, span }
}

// ── Utilities ─────────────────────────────────────────────────────────────────

/// True if the current token can start an expression.
pub(crate) fn is_expr_start(p: &Parser) -> bool {
    matches!(
        p.peek(),
        Token::Identifier(_)
            | Token::IntegerLiteral(_)
            | Token::DecimalLiteral { .. }
            | Token::StringLiteral(_)
            | Token::Spaces
            | Token::Zeros
            | Token::HighValues
            | Token::LowValues
            | Token::Quotes
            | Token::Nulls
            | Token::All
            | Token::Plus
            | Token::Minus
            | Token::LParen
            | Token::Function
    )
}

/// Return the current identifier as an uppercase string without consuming it.
fn ident_upper(p: &Parser) -> Option<String> {
    if let Token::Identifier(s) = p.peek() {
        Some(s.to_uppercase())
    } else {
        None
    }
}

// Needed for Token::For (not a keyword — would be an identifier "FOR")
impl Parser {
    pub(crate) fn eat_for_kw(&mut self) -> bool {
        if let Token::Identifier(ref s) = self.peek().clone() {
            if s.to_uppercase() == "FOR" {
                self.advance();
                return true;
            }
        }
        false
    }
}

