// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! PROCEDURE DIVISION parser.
//!
//! Parses the optional `USING`/`RETURNING` header, then the body which is
//! either a flat sequence of paragraphs or a set of named sections each
//! containing paragraphs.

use cobolt_ast::program::{
    Paragraph, ProcedureBody, ProcedureDivision, Section, UseMode, UseProcedure,
};
use cobolt_lexer::{Span, Token};

use crate::parser::Parser;
use crate::stmt::parse_stmts;

// ── Entry point ───────────────────────────────────────────────────────────────

pub(crate) fn parse_procedure_division(p: &mut Parser) -> ProcedureDivision {
    let span = p.peek_span();

    if !p.at(&Token::Procedure) {
        p.emit_error("expected PROCEDURE DIVISION");
        return empty_procedure(span);
    }
    p.advance(); // PROCEDURE
    p.expect(&Token::Division);

    // USING data-item …
    let using = if p.eat(&Token::Using) {
        let mut names = Vec::new();
        // Accept BY REFERENCE / BY VALUE as optional prefixes
        p.eat(&Token::By);
        p.eat(&Token::Reference);
        while p.at_identifier() {
            let (name, _) = p.eat_identifier().unwrap();
            names.push(name);
        }
        names
    } else {
        Vec::new()
    };

    // RETURNING data-item
    let returning = if p.eat(&Token::Returning) {
        p.eat_identifier().map(|(n, _)| n)
    } else {
        None
    };

    p.expect_period();

    // Optional `DECLARATIVES … END DECLARATIVES.` block at the head of the body.
    let declaratives = parse_declaratives(p);

    let body = parse_procedure_body(p);

    ProcedureDivision { using, returning, declaratives, body, span }
}

fn empty_procedure(span: Span) -> ProcedureDivision {
    ProcedureDivision {
        using: Vec::new(),
        returning: None,
        declaratives: Vec::new(),
        body: ProcedureBody::Paragraphs(Vec::new()),
        span,
    }
}

// ── DECLARATIVES parser ─────────────────────────────────────────────────────────

/// Parse an optional `DECLARATIVES. … END DECLARATIVES.` block. Each declarative
/// is a SECTION whose first statement is `USE AFTER STANDARD ERROR PROCEDURE ON
/// …`, followed by the handler paragraphs. Returns an empty vec when there is no
/// DECLARATIVES block.
fn parse_declaratives(p: &mut Parser) -> Vec<UseProcedure> {
    if !p.at(&Token::Declaratives) {
        return Vec::new();
    }
    p.advance(); // DECLARATIVES
    p.expect_period();

    let mut procs = Vec::new();
    loop {
        while p.eat(&Token::Period) {}
        if p.at(&Token::Eof) {
            break;
        }
        // END DECLARATIVES.
        if p.at(&Token::End) && matches!(p.peek_at(1), Token::Declaratives) {
            p.advance(); // END
            p.advance(); // DECLARATIVES
            p.expect_period();
            break;
        }
        // Expect `section-name SECTION .`
        if matches!(p.peek(), Token::Identifier(_)) && matches!(p.peek_at(1), Token::Section) {
            let span = p.peek_span();
            let _ = p.eat_identifier(); // section name (not retained)
            p.advance(); // SECTION
            p.expect_period();

            let (files, modes, catch_all) = parse_use_clause(p);
            let stmts = parse_declarative_body(p);
            procs.push(UseProcedure { files, modes, catch_all, stmts, span });
        } else {
            // Unexpected token — recover to the next period to avoid looping.
            p.emit_error(format!(
                "expected a declarative SECTION header, found {:?}",
                p.peek()
            ));
            p.sync_to_period();
        }
    }
    procs
}

/// Parse `USE [GLOBAL] AFTER STANDARD {ERROR | EXCEPTION} PROCEDURE ON
/// {file… | INPUT | OUTPUT | I-O | EXTEND} .` Returns the covered files,
/// open-modes, and whether it is a catch-all (no target named).
fn parse_use_clause(p: &mut Parser) -> (Vec<String>, Vec<UseMode>, bool) {
    let mut files = Vec::new();
    let mut modes = Vec::new();

    if !p.eat(&Token::Use) {
        // Not a USE statement — nothing to associate; treat as catch-all.
        return (files, modes, true);
    }
    // Skip the descriptive words (GLOBAL / AFTER / STANDARD / ERROR /
    // EXCEPTION / PROCEDURE) up to `ON` or the terminating period.
    while !p.at(&Token::On) && !p.at(&Token::Period) && !p.at(&Token::Eof) {
        p.advance();
    }
    p.eat(&Token::On);

    // Targets: file names and/or open-modes, until the period.
    while !p.at(&Token::Period) && !p.at(&Token::Eof) {
        match p.peek() {
            Token::Input  => { p.advance(); modes.push(UseMode::Input); }
            Token::Output => { p.advance(); modes.push(UseMode::Output); }
            Token::IoMode => { p.advance(); modes.push(UseMode::Io); }
            Token::Extend => { p.advance(); modes.push(UseMode::Extend); }
            Token::Identifier(_) => {
                let (name, _) = p.eat_identifier().unwrap();
                files.push(name.to_ascii_uppercase());
            }
            _ => { p.advance(); } // skip noise words (e.g. commas already eaten)
        }
        p.eat(&Token::Comma);
    }
    p.expect_period();

    let catch_all = files.is_empty() && modes.is_empty();
    (files, modes, catch_all)
}

/// Collect the handler statements of a declarative section (all of its
/// paragraphs, flattened), stopping at the next section header or
/// `END DECLARATIVES`.
fn parse_declarative_body(p: &mut Parser) -> Vec<cobolt_ast::stmt::Stmt> {
    let mut stmts = Vec::new();
    loop {
        while p.eat(&Token::Period) {}
        if p.at(&Token::Eof) {
            break;
        }
        if p.at(&Token::End) && matches!(p.peek_at(1), Token::Declaratives) {
            break;
        }
        // Next declarative section header → stop.
        if matches!(p.peek(), Token::Identifier(_)) && matches!(p.peek_at(1), Token::Section) {
            break;
        }
        // Named paragraph header → take its statements; otherwise collect
        // orphan statements (parse_stmts stops at the next header on its own).
        if matches!(p.peek(), Token::Identifier(_)) && matches!(p.peek_at(1), Token::Period) {
            let para = parse_paragraph(p);
            stmts.extend(para.stmts);
        } else {
            let s = parse_stmts(p, &|tok| {
                matches!(
                    tok,
                    Token::End | Token::Eof | Token::Identification | Token::Environment | Token::Data
                )
            });
            if s.is_empty() {
                break;
            }
            stmts.extend(s);
        }
    }
    stmts
}

// ── Body parser ───────────────────────────────────────────────────────────────

fn parse_procedure_body(p: &mut Parser) -> ProcedureBody {
    // Scan ahead to decide whether the program uses sections.
    // If we see `Identifier SECTION Period` before any statements, use sections.
    let uses_sections = look_ahead_for_sections(p);

    if uses_sections {
        ProcedureBody::Sections(parse_sections(p))
    } else {
        ProcedureBody::Paragraphs(parse_paragraphs(p))
    }
}

/// Look ahead (without consuming) to decide if the body is sectioned.
fn look_ahead_for_sections(p: &Parser) -> bool {
    // Check if any of the first few items is `Identifier SECTION Period`
    let mut i = 0;
    loop {
        let tok0 = p.peek_at(i);
        match tok0 {
            Token::Eof => break,
            Token::Identifier(_) => {
                if matches!(p.peek_at(i + 1), Token::Section) {
                    return true;
                }
                // Could be a paragraph header — keep scanning
                i += 1;
            }
            Token::Period => { i += 1; }
            _ => {
                // We hit a statement verb — no sections
                if i > 20 { break; } // don't scan too far
                i += 1;
            }
        }
    }
    false
}

// ── Section parser ────────────────────────────────────────────────────────────

fn parse_sections(p: &mut Parser) -> Vec<Section> {
    let mut sections = Vec::new();
    loop {
        if p.at(&Token::Eof) { break; }
        // Division header or END PROGRAM — stop
        if matches!(p.peek(), Token::Environment | Token::Data | Token::Identification | Token::End) {
            break;
        }
        // Skip any stray periods
        while p.eat(&Token::Period) {}
        if p.at(&Token::Eof) { break; }

        // Expect `identifier SECTION .`
        if !matches!(p.peek(), Token::Identifier(_)) {
            // Not a section header — try to recover
            p.emit_error(format!("expected section name, found {:?}", p.peek()));
            p.sync_to_period();
            continue;
        }
        if !matches!(p.peek_at(1), Token::Section) {
            // Could be an implicit paragraph in a sectioned program — collect
            // any paragraphs before a section header or EOF
            let paras = parse_paragraphs_until_section(p);
            if !paras.is_empty() {
                sections.push(Section {
                    name: "<implicit>".into(),
                    paragraphs: paras,
                    span: Span::dummy(),
                });
            }
            continue;
        }

        let span = p.peek_span();
        let (name, _) = p.eat_identifier().unwrap();
        p.advance(); // SECTION
        p.expect_period();

        let paragraphs = parse_paragraphs_until_section(p);
        sections.push(Section { name, paragraphs, span });
    }
    sections
}

fn parse_paragraphs_until_section(p: &mut Parser) -> Vec<Paragraph> {
    let mut paragraphs = Vec::new();
    loop {
        if p.at(&Token::Eof) { break; }
        if matches!(p.peek(), Token::Environment | Token::Data | Token::Identification | Token::End) {
            break;
        }
        while p.eat(&Token::Period) {}
        if p.at(&Token::Eof) { break; }

        // Section header → stop collecting paragraphs for the current section
        if matches!(p.peek(), Token::Identifier(_))
            && matches!(p.peek_at(1), Token::Section)
        {
            break;
        }

        // Paragraph header: Identifier Period
        if matches!(p.peek(), Token::Identifier(_))
            && matches!(p.peek_at(1), Token::Period)
        {
            let para = parse_paragraph(p);
            paragraphs.push(para);
        } else {
            break;
        }
    }
    paragraphs
}

// ── Paragraph parser ──────────────────────────────────────────────────────────

fn parse_paragraphs(p: &mut Parser) -> Vec<Paragraph> {
    let mut paragraphs = Vec::new();
    loop {
        if p.at(&Token::Eof) { break; }
        if matches!(p.peek(), Token::Environment | Token::Data | Token::Identification | Token::End) {
            break;
        }
        while p.eat(&Token::Period) {}
        if p.at(&Token::Eof) { break; }

        // Paragraph header: Identifier Period
        if matches!(p.peek(), Token::Identifier(_))
            && matches!(p.peek_at(1), Token::Period)
        {
            let para = parse_paragraph(p);
            paragraphs.push(para);
        } else {
            // Orphaned statements (common in simple programs without paragraph names)
            // Collect them into an implicit paragraph
            let span = p.peek_span();
            let stmts = parse_stmts(p, &|tok| {
                // Stop at next paragraph candidate, division, or END PROGRAM
                matches!(tok, Token::Environment | Token::Data | Token::Identification | Token::End | Token::Eof)
            });
            if !stmts.is_empty() {
                paragraphs.push(Paragraph {
                    name: "<implicit>".into(),
                    stmts,
                    span,
                });
            } else {
                break; // Prevent infinite loop
            }
        }
    }
    paragraphs
}

fn parse_paragraph(p: &mut Parser) -> Paragraph {
    let span = p.peek_span();
    let (name, _) = p.eat_identifier().unwrap(); // paragraph name
    p.expect_period(); // the period after the name

    // Collect statements until the next paragraph/section header or division end
    let stmts = parse_stmts(p, &|tok| {
        matches!(
            tok,
            Token::Environment | Token::Data | Token::Identification | Token::End | Token::Eof
        )
    });

    Paragraph { name, stmts, span }
}
