// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Free-format **block literals** — the ``` fence.
//!
//! COBOL-85 has no multi-line literal. Continuation is a fixed-format column
//! mechanism (a `-` in column 7), so free-format source had no way to write a
//! literal that spans lines, and no way to write one containing quotation marks
//! without doubling every one of them.
//!
//! The fence borrows Markdown's notation: the text is the lines *between* the
//! fences, taken verbatim.
//!
//! ```text
//!     MOVE
//!     ```
//!     Hello, World!
//!     ```
//!     TO WS-GREETING.
//! ```
//!
//! This is a **language extension**, not COBOL-85 conformance.

use cobolt_lexer::{tokenize, SourceFormat, Token};

fn literals(src: &str, fmt: SourceFormat) -> Vec<String> {
    tokenize(src, fmt)
        .into_iter()
        .filter_map(|st| match st.token {
            Token::StringLiteral(s) => Some(s),
            _ => None,
        })
        .collect()
}

/// The operator's own example: a one-line block is that line, with **no**
/// trailing newline — it behaves exactly as a quoted literal would.
#[test]
fn a_one_line_block_is_that_line() {
    let src = "SET x TO\n```\nHello, World!\n```\n";
    assert_eq!(
        literals(src, SourceFormat::Free),
        vec!["Hello, World!".to_string()]
    );
}

/// Interior newlines are kept — the whole point of the construct.
#[test]
fn interior_newlines_are_kept() {
    let src = "MOVE\n```\nline one\nline two\nline three\n```\nTO WS-X.\n";
    assert_eq!(
        literals(src, SourceFormat::Free),
        vec!["line one\nline two\nline three".to_string()]
    );
}

/// The text is verbatim: quotation marks and apostrophes need no doubling,
/// which is what makes the construct useful for JSON, SQL and HTML.
#[test]
fn the_text_is_verbatim_with_no_escaping() {
    let src = "MOVE\n```\n{\"name\": \"O'Brien\", \"ok\": true}\n```\nTO WS-JSON.\n";
    assert_eq!(
        literals(src, SourceFormat::Free),
        vec![r#"{"name": "O'Brien", "ok": true}"#.to_string()]
    );
}

/// An empty block is the empty string, not an error.
#[test]
fn an_empty_block_is_the_empty_string() {
    let src = "MOVE\n```\n```\nTO WS-X.\n";
    assert_eq!(literals(src, SourceFormat::Free), vec![String::new()]);
}

/// The tokens around the block are untouched, so it is usable as an operand.
#[test]
fn the_surrounding_statement_still_parses() {
    let src = "MOVE\n```\nHi\n```\nTO WS-X.\n";
    let toks: Vec<Token> = tokenize(src, SourceFormat::Free)
        .into_iter()
        .map(|st| st.token)
        .collect();
    assert_eq!(toks[0], Token::Move);
    assert_eq!(toks[1], Token::StringLiteral("Hi".into()));
    assert_eq!(toks[2], Token::To);
    assert!(toks.iter().any(|t| matches!(t, Token::Identifier(s) if s == "WS-X")));
    assert!(toks.contains(&Token::Period));
    // The fence itself never reaches the consumer.
    assert!(!toks.contains(&Token::Fence), "{toks:?}");
}

/// Anything after the opening fence is a language tag, not content — the text
/// starts on the NEXT line, which is what the operator specified.
#[test]
fn a_tag_after_the_opening_fence_is_not_content() {
    let src = "MOVE\n```json\n{\"a\": 1}\n```\nTO WS-X.\n";
    assert_eq!(
        literals(src, SourceFormat::Free),
        vec![r#"{"a": 1}"#.to_string()]
    );
}

/// COBOL keywords inside a block are text, not code.
#[test]
fn cobol_inside_a_block_is_text() {
    let src = "MOVE\n```\nMOVE 1 TO X.\n```\nTO WS-X.\n";
    assert_eq!(
        literals(src, SourceFormat::Free),
        vec!["MOVE 1 TO X.".to_string()]
    );
    // Exactly one MOVE token — the one outside the block.
    let moves = tokenize(src, SourceFormat::Free)
        .into_iter()
        .filter(|st| st.token == Token::Move)
        .count();
    assert_eq!(moves, 1);
}

/// An opening fence with no closing fence is reported where it was OPENED.
/// End of file is not the useful place to look.
#[test]
fn an_unterminated_block_is_reported_at_its_opening() {
    let src = "MOVE\n```\nnever closed\n";
    let toks = tokenize(src, SourceFormat::Free);
    assert!(
        toks.iter().any(|st| matches!(&st.token, Token::Error(e) if e.contains("block literal"))),
        "expected an unterminated-block error: {toks:?}"
    );
}

/// **Fixed format has no block literals.** It has an indicator column and a
/// sequence area, so a line of backticks there means something else — say so
/// rather than silently producing a literal nobody asked for.
#[test]
fn a_fence_in_fixed_format_is_refused() {
    let src = "000100 MOVE\n000200 ```\n000300 Hello\n000400 ```\n";
    for fmt in [SourceFormat::Fixed, SourceFormat::FixedStrict] {
        let toks = tokenize(src, fmt);
        assert!(
            toks.iter()
                .any(|st| matches!(&st.token, Token::Error(e) if e.contains("free format"))),
            "{fmt:?} should refuse a block literal: {toks:?}"
        );
    }
}

/// A closing fence may be indented — real code is indented.
#[test]
fn the_closing_fence_may_be_indented() {
    let src = "MOVE\n    ```\n    Hello\n    ```\n    TO WS-X.\n";
    assert_eq!(
        literals(src, SourceFormat::Free),
        vec!["    Hello".to_string()],
        "leading whitespace inside the block is content, verbatim"
    );
}
