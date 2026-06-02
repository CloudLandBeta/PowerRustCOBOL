// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! IDENTIFICATION DIVISION parser.

use cobolt_ast::program::IdentificationDivision;
use cobolt_lexer::Token;

use crate::parser::Parser;

/// Parse the IDENTIFICATION (or ID) DIVISION header and its paragraphs.
///
/// ```text
/// IDENTIFICATION DIVISION.
/// PROGRAM-ID. program-name.
/// [AUTHOR. text.]
/// [DATE-WRITTEN. text.]
/// ```
pub(crate) fn parse_identification_division(p: &mut Parser) -> IdentificationDivision {
    let start = p.peek_span();

    // Accept IDENTIFICATION or ID
    if p.at(&Token::Identification) {
        p.advance();
        p.expect(&Token::Division);
        p.expect_period();
    } else {
        p.emit_error("expected IDENTIFICATION DIVISION");
    }

    // PROGRAM-ID. name.
    let program_id = if p.at(&Token::ProgramId) {
        p.advance();
        p.expect_period();
        let name = p.expect_identifier("PROGRAM-ID");
        // Optional trailing period after the program name
        p.eat(&Token::Period);
        name
    } else {
        p.emit_error("expected PROGRAM-ID paragraph");
        "<missing>".into()
    };

    // Optional informational paragraphs (AUTHOR, DATE-WRITTEN, etc.)
    // We collect their text as a raw string and skip to the next division.
    let mut author: Option<String> = None;
    let mut date_written: Option<String> = None;

    loop {
        match p.peek().clone() {
            Token::Author => {
                p.advance();
                p.eat(&Token::Period);
                author = Some(collect_comment_text(p));
            }
            Token::DateWritten => {
                p.advance();
                p.eat(&Token::Period);
                date_written = Some(collect_comment_text(p));
            }
            // Any other IDENTIFICATION paragraph we skip over.
            Token::Identifier(_) => {
                p.advance();
                p.eat(&Token::Period);
                collect_comment_text(p);
            }
            // Next division or EOF — stop.
            Token::Environment
            | Token::Data
            | Token::Procedure
            | Token::Eof => break,
            _ => break,
        }
    }

    IdentificationDivision {
        program_id,
        author,
        installation: None,
        date_written,
        date_compiled: None,
        security: None,
        span: start,
    }
}

/// Collect all tokens up to (but not including) the next period as a
/// space-separated string.  Consumes the period.
fn collect_comment_text(p: &mut Parser) -> String {
    let mut parts = Vec::new();
    while !matches!(p.peek(), Token::Period | Token::Eof)
        && !matches!(
            p.peek(),
            Token::Author
                | Token::DateWritten
                | Token::Environment
                | Token::Data
                | Token::Procedure
        )
    {
        let st = p.advance();
        parts.push(format!("{:?}", st.token));
    }
    p.eat(&Token::Period);
    parts.join(" ")
}

