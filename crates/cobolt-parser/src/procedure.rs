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

use cobolt_ast::program::{Paragraph, ProcedureBody, ProcedureDivision, Section};
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

    let body = parse_procedure_body(p);

    ProcedureDivision { using, returning, body, span }
}

fn empty_procedure(span: Span) -> ProcedureDivision {
    ProcedureDivision {
        using: Vec::new(),
        returning: None,
        body: ProcedureBody::Paragraphs(Vec::new()),
        span,
    }
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
