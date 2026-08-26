// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! IDENTIFICATION DIVISION parser.

use cobolt_ast::program::IdentificationDivision;
use cobolt_lexer::{SpannedToken, Token};

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
        // Optional COBOL program attributes: [IS] {COMMON | INITIAL | RECURSIVE}
        // [PROGRAM]. Accepted and ignored — the interpreter resolves nested-program
        // CALLs through a flat registry, so COMMON/INITIAL impose no runtime
        // restriction (generated form modules emit user procedures `IS COMMON`).
        p.eat(&Token::Is);
        loop {
            if p.eat(&Token::Program) {
                continue;
            }
            let is_attr = matches!(
                p.peek(),
                Token::Identifier(s)
                    if matches!(s.to_ascii_uppercase().as_str(),
                                "COMMON" | "INITIAL" | "RECURSIVE")
            );
            if is_attr {
                p.advance();
                continue;
            }
            break;
        }
        // Optional trailing period after the program name / attributes
        p.eat(&Token::Period);
        name
    } else {
        p.emit_error("expected PROGRAM-ID paragraph");
        "<missing>".into()
    };

    // The optional comment-entry paragraphs: AUTHOR, INSTALLATION,
    // DATE-WRITTEN, DATE-COMPILED, SECURITY — and REMARKS, which COBOL-85
    // deleted but which older source still carries.
    //
    // They may appear in any order and any subset. Each takes a *comment-entry*:
    // free text that ends at the next paragraph or division header, not at the
    // next period (see `collect_comment_text`).
    let mut author: Option<String> = None;
    let mut installation: Option<String> = None;
    let mut date_written: Option<String> = None;
    let mut date_compiled: Option<String> = None;
    let mut security: Option<String> = None;

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
            // Was missing entirely: DATE-COMPILED is a keyword, so it never
            // reached the `Identifier` arm below and fell through to `break`,
            // ending the division early and leaving the parser demanding a
            // division header where a paragraph name sat.
            Token::DateCompiled => {
                p.advance();
                p.eat(&Token::Period);
                date_compiled = Some(collect_comment_text(p));
            }
            // INSTALLATION / SECURITY / REMARKS — recognised by name, because
            // they are not reserved words (see `NAMED_PARAGRAPHS`).
            Token::Identifier(ref s)
                if NAMED_PARAGRAPHS.contains(&s.to_ascii_uppercase().as_str())
                    && matches!(p.peek_at(1), Token::Period) =>
            {
                let name = s.to_ascii_uppercase();
                p.advance();
                p.eat(&Token::Period);
                let text = collect_comment_text(p);
                match name.as_str() {
                    "INSTALLATION" => installation = Some(text),
                    "SECURITY" => security = Some(text),
                    // REMARKS has no field: it is not a COBOL-85 paragraph, and
                    // adding one would change the bincode layout that compiled
                    // binaries embed. Accepted, then dropped.
                    _ => {}
                }
            }
            // Next division, or anything that is not a paragraph header.
            _ => break,
        }
    }

    IdentificationDivision {
        program_id,
        author,
        installation,
        date_written,
        date_compiled,
        security,
        span: start,
    }
}

/// The IDENTIFICATION paragraphs whose name is not a reserved word.
///
/// `AUTHOR`, `DATE-WRITTEN` and `DATE-COMPILED` are keywords and arrive as their
/// own tokens. These three are deliberately **not** keywords: reserving them
/// would break any existing program with a data item called `SECURITY`, which is
/// an ordinary enough name. They are recognised here, by name, and only inside
/// the IDENTIFICATION DIVISION.
///
/// `REMARKS` was deleted from COBOL in 1985 and NIST CCVS85 does not contain a
/// single one. It is accepted, and its text discarded, so source carried over
/// from COBOL-74 still compiles.
const NAMED_PARAGRAPHS: [&str; 3] = ["INSTALLATION", "SECURITY", "REMARKS"];

/// Does the token at `offset` start a new IDENTIFICATION paragraph, or the next
/// division — i.e. does it end the comment-entry now being collected?
///
/// Two things must both hold, and neither is sufficient alone:
///
/// 1. the token **begins a source line** — COBOL-85 ends a comment-entry at the
///    next entry in Area A, and this is the part of that rule that survives
///    source-format flattening;
/// 2. the token **has the shape of a header** — a paragraph keyword, one of
///    [`NAMED_PARAGRAPHS`] followed by a period, or a division keyword followed
///    by `DIVISION`.
///
/// Requiring both is what lets a comment-entry contain reserved words. CCVS85's
/// CM101M has `AUTOMATED DATA AND TELECOMMUNICATION SERVICE.` inside its
/// `INSTALLATION` entry: `DATA` fails (1) because it is mid-line, and would fail
/// (2) anyway because no `DIVISION` follows it.
fn ends_comment_entry(p: &Parser, offset: usize) -> bool {
    ends_comment_entry_at(&p.tokens, p.pos + offset)
}

/// [`ends_comment_entry`] against a raw token slice.
///
/// The duplicate-division scanner (`Parser::detect_duplicate_declarations`)
/// walks the token stream before anything is parsed, so it needs the same rule
/// without a `Parser` to ask. Sharing one implementation is what keeps the two
/// from drifting — the scanner counting a `PROCEDURE DIVISION` that the parser
/// treats as prose is exactly the bug this avoids.
pub(crate) fn ends_comment_entry_at(tokens: &[SpannedToken], idx: usize) -> bool {
    let tok = match tokens.get(idx) {
        Some(st) => &st.token,
        None => return true, // past the end behaves like Eof
    };
    if matches!(tok, Token::Eof) {
        return true;
    }
    if !starts_line(tokens, idx) {
        return false;
    }
    let next = tokens.get(idx + 1).map(|st| &st.token);
    match tok {
        Token::Author | Token::DateWritten | Token::DateCompiled | Token::Identification => true,
        Token::Environment | Token::Data | Token::Procedure => next == Some(&Token::Division),
        Token::Identifier(s) => {
            NAMED_PARAGRAPHS.contains(&s.to_ascii_uppercase().as_str())
                && next == Some(&Token::Period)
        }
        _ => false,
    }
}

/// Does the token at `idx` begin a new source line?
pub(crate) fn starts_line(tokens: &[SpannedToken], idx: usize) -> bool {
    match (idx.checked_sub(1).and_then(|i| tokens.get(i)), tokens.get(idx)) {
        (Some(prev), Some(cur)) => cur.span.line > prev.span.line,
        (None, Some(_)) => true,
        _ => false,
    }
}

/// Is the token at `idx` an IDENTIFICATION paragraph header that introduces a
/// comment-entry? Returns the index just past the header (name + period).
pub(crate) fn comment_entry_header_at(tokens: &[SpannedToken], idx: usize) -> Option<usize> {
    let tok = &tokens.get(idx)?.token;
    let is_header = match tok {
        Token::Author | Token::DateWritten | Token::DateCompiled => true,
        Token::Identifier(s) => NAMED_PARAGRAPHS.contains(&s.to_ascii_uppercase().as_str()),
        _ => false,
    };
    if !is_header || !starts_line(tokens, idx) {
        return None;
    }
    // The paragraph name is followed by its period.
    match tokens.get(idx + 1).map(|st| &st.token) {
        Some(Token::Period) => Some(idx + 2),
        _ => None,
    }
}

/// Render one token of a comment-entry as text.
///
/// ⚠️ **This is a best-effort reconstruction, not the source verbatim.** The
/// parser is handed a token stream and never sees the source, so the original
/// spacing, capitalisation and punctuation cannot be recovered. Data-carrying
/// tokens render through `Display`, which is faithful; a reserved word renders
/// through `Debug`, which gives its variant name (`Data` → `DATA`) because
/// `Display` would flatten every keyword to the word "keyword".
///
/// Nothing consumes these fields today; they exist so the information is
/// retained rather than dropped. If a surface ever displays a comment-entry,
/// give the parser the source text and slice it by span instead.
fn comment_word(tok: &Token) -> String {
    match tok {
        Token::Identifier(_)
        | Token::IntegerLiteral(_)
        | Token::DecimalLiteral { .. }
        | Token::StringLiteral(_)
        | Token::LevelNumber(_) => tok.to_string(),
        Token::Period => ".".to_string(),
        other => format!("{other:?}").to_ascii_uppercase(),
    }
}

/// Collect a **comment-entry** — the free text of an IDENTIFICATION paragraph.
///
/// COBOL-85 makes this any text at all: it may contain reserved words, periods
/// and blank lines, and it runs across as many lines as the developer wrote.
/// It ends only where [`ends_comment_entry`] says it does.
///
/// The leading period after the paragraph name is consumed by the caller.
fn collect_comment_text(p: &mut Parser) -> String {
    let mut parts: Vec<String> = Vec::new();
    while !ends_comment_entry(p, 0) {
        let st = p.advance();
        parts.push(comment_word(&st.token));
    }
    parts.join(" ")
}
