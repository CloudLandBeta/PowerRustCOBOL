// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tests: fixed-form COBOL source format (column-based layout).
//!
//! Fixed-form rules:
//! - Columns 1-6:  Sequence number (ignored)
//! - Column 7:     Indicator (* = comment, - = continuation, space = normal)
//! - Columns 8-72: Active source
//! - Columns 73+:  Identification area (ignored)

use cobolt_lexer::{tokenize, tokenize_with_comments, SourceFormat, Token};

fn toks_fixed(src: &str) -> Vec<Token> {
    tokenize(src, SourceFormat::Fixed)
        .into_iter()
        .map(|st| st.token)
        .collect()
}

#[test]
fn normal_fixed_line() {
    let src = "000100 MOVE WS-A TO WS-B.                                              \n";
    let t = toks_fixed(src);
    assert_eq!(t[0], Token::Move);
    assert_eq!(t[1], Token::Identifier("WS-A".to_string()));
    assert_eq!(t[2], Token::To);
    assert_eq!(t[3], Token::Identifier("WS-B".to_string()));
    assert_eq!(t[4], Token::Period);
}

#[test]
fn star_comment_excluded_by_default() {
    let src = "000100* This is a comment line\n000200 MOVE A TO B.\n";
    let t = toks_fixed(src);
    assert!(
        !t.iter()
            .any(|t| matches!(t, Token::Identifier(s) if s == "THIS")),
        "comment text should not appear as tokens"
    );
    assert!(t.contains(&Token::Move));
}

#[test]
fn star_comment_included_with_comments() {
    let src = "000100* This is a comment\n000200 MOVE A TO B.\n";
    let all = tokenize_with_comments(src, SourceFormat::Fixed);
    let has_comment = all
        .iter()
        .any(|st| matches!(&st.token, Token::Comment(c) if c.contains("This is a comment")));
    assert!(
        has_comment,
        "comment token should be present with tokenize_with_comments"
    );
}

#[test]
fn slash_comment_line() {
    let src = "000100/ Page break comment\n000200 STOP RUN.\n";
    let t = toks_fixed(src);
    assert!(t.contains(&Token::Stop));
    assert!(!t
        .iter()
        .any(|t| matches!(t, Token::Identifier(s) if s == "PAGE")));
}

/// **The 72-column limit is not enforced** (operator, 2026-08-05: "this is
/// archaic").
///
/// This test previously asserted the opposite — that anything from column 73 on
/// was discarded as the card-deck identification area. What that rule actually
/// did was delete code: silently, mid-token, with the error surfacing somewhere
/// else. It also made `EXEC RUST` unusable inside a form, because a generated
/// `.cbl` opens with a banner whose `*` is in column 7, so the file read as
/// fixed and the embedded Rust was chopped at 72.
///
/// **The trade-off, stated plainly:** a legacy deck that really does carry
/// sequence numbers in columns 73–80 will now see them as source. That is the
/// price of not destroying anyone's code, and it was accepted deliberately.
#[test]
fn text_past_column_72_is_kept() {
    // "000100 MOVE A TO B." = 19 bytes; the spaces put MYPROG at column 73.
    let src = "000100 MOVE A TO B.                                                     MYPROG\n";
    let t = toks_fixed(src);
    assert_eq!(t[0], Token::Move);
    assert!(
        t.iter()
            .any(|tok| matches!(tok, Token::Identifier(s) if s == "MYPROG")),
        "text past column 72 must be kept, not discarded"
    );
}

#[test]
fn sequence_numbers_ignored() {
    let src = "000100 DISPLAY \"Line 1\".\n000200 DISPLAY \"Line 2\".\n";
    let t = toks_fixed(src);
    assert!(
        !t.iter()
            .any(|tok| matches!(tok, Token::IntegerLiteral(100) | Token::IntegerLiteral(200))),
        "sequence numbers should not tokenize"
    );
    assert_eq!(
        t.iter()
            .filter(|t| matches!(t, Token::DisplayVerb | Token::Display))
            .count(),
        2,
        "should have two DISPLAY tokens"
    );
}

#[test]
fn hello_world_program_fixed() {
    let src = concat!(
        "000100 IDENTIFICATION DIVISION.\n",
        "000200 PROGRAM-ID. HELLO.\n",
        "000300* Author: Cobolt test\n",
        "000400 PROCEDURE DIVISION.\n",
        "000500 MAIN-PROC.\n",
        "000600     DISPLAY \"Hello, World!\"\n",
        "000700     STOP RUN.\n",
    );
    let t = toks_fixed(src);
    assert!(t.contains(&Token::Identification));
    assert!(t.contains(&Token::Division));
    assert!(t.contains(&Token::ProgramId));
    assert!(t.contains(&Token::Procedure));
    assert!(t
        .iter()
        .any(|tok| matches!(tok, Token::StringLiteral(s) if s == "Hello, World!")));
    assert!(t.contains(&Token::Stop));
    assert!(t.contains(&Token::Run));
}

#[test]
fn working_storage_declarations_fixed() {
    let src = concat!(
        "000100 DATA DIVISION.\n",
        "000200 WORKING-STORAGE SECTION.\n",
        "000300 01 WS-COUNTER         PIC 9(5)   VALUE ZERO.\n",
        "000400 01 WS-NAME            PIC X(30)  VALUE SPACES.\n",
        "000500 01 WS-AMOUNT          PIC 9(7)V99 COMP-3.\n",
    );
    let t = toks_fixed(src);
    assert!(t.contains(&Token::Data));
    assert!(t.contains(&Token::WorkingStorage));
    assert!(t.contains(&Token::Section));
    assert_eq!(t.iter().filter(|t| **t == Token::LevelNumber(1)).count(), 3);
    assert!(t.contains(&Token::Pic));
    assert!(t.contains(&Token::Zeros));
    assert!(t.contains(&Token::Spaces));
    assert!(t.contains(&Token::Comp3));
}

#[test]
fn perform_varying_fixed() {
    let src = concat!(
        "000100 PROCEDURE DIVISION.\n",
        "000200 MAIN-PROC.\n",
        "000300     PERFORM VARYING WS-IDX FROM 1 BY 1\n",
        "000400         UNTIL WS-IDX > 10\n",
        "000500         DISPLAY WS-IDX\n",
        "000600     END-PERFORM\n",
        "000700     STOP RUN.\n",
    );
    let t = toks_fixed(src);
    assert!(t.contains(&Token::Perform));
    assert!(t.contains(&Token::Varying));
    assert!(t.contains(&Token::From));
    assert!(t.contains(&Token::By));
    assert!(t.contains(&Token::Until));
    assert!(t.contains(&Token::Gt));
    // END-PERFORM is a scope-terminator keyword (like END-IF / END-EVALUATE),
    // which the parser eats to close the inline PERFORM block.
    assert!(
        t.contains(&Token::EndPerform),
        "END-PERFORM should tokenize as Token::EndPerform"
    );
}

#[test]
fn if_else_end_if_fixed() {
    let src = concat!(
        "000100     IF WS-FLAG = 1\n",
        "000200         MOVE \"YES\" TO WS-RESULT\n",
        "000300     ELSE\n",
        "000400         MOVE \"NO\" TO WS-RESULT\n",
        "000500     END-IF.\n",
    );
    let t = toks_fixed(src);
    assert!(t.contains(&Token::If));
    assert!(t.contains(&Token::Eq));
    assert!(t.contains(&Token::Move));
    assert!(t.contains(&Token::Else));
    assert!(t.contains(&Token::EndIf));
}

#[test]
fn evaluate_when_fixed() {
    let src = concat!(
        "000100     EVALUATE WS-CODE\n",
        "000200         WHEN 1\n",
        "000300             MOVE \"ONE\" TO WS-RESULT\n",
        "000400         WHEN 2\n",
        "000500             MOVE \"TWO\" TO WS-RESULT\n",
        "000600         WHEN OTHER\n",
        "000700             MOVE \"OTHER\" TO WS-RESULT\n",
        "000800     END-EVALUATE.\n",
    );
    let t = toks_fixed(src);
    assert!(t.contains(&Token::Evaluate));
    assert_eq!(t.iter().filter(|t| **t == Token::When).count(), 3);
    assert!(t.contains(&Token::Other));
    assert!(t.contains(&Token::EndEvaluate));
}

#[test]
fn spans_are_non_zero() {
    let src = "000100 MOVE WS-A TO WS-B.\n";
    let sts = tokenize(src, SourceFormat::Fixed);
    for st in &sts {
        assert!(
            st.span.start < st.span.end || matches!(st.token, Token::Eof),
            "token {:?} has zero-width span",
            st.token
        );
    }
}

#[test]
fn eof_token_present() {
    use cobolt_lexer::Lexer;
    let src = "000100 STOP RUN.\n";
    let mut lexer = Lexer::new(src, SourceFormat::Fixed);
    let all: Vec<_> = std::iter::from_fn(|| lexer.next_token()).collect();
    assert!(
        all.last().map(|st| &st.token) == Some(&Token::Eof),
        "last token should be Eof"
    );
}

// ── Debugging lines (COBOL-85) ───────────────────────────────────────────────
//
// A `D` in the indicator area marks a **debugging line**. The standard's
// default is that it is a *comment*; it is compiled only when the program says
// `SOURCE-COMPUTER. … WITH DEBUGGING MODE.`
//
// The two fixed-format flatteners used to disagree: the strict path treated `D`
// as a comment, the relaxed path compiled it unconditionally. Operator ruling,
// 2026-08-26: follow the standard, in fixed format only.

/// Without the clause, a `D` line contributes no source.
#[test]
fn a_debug_line_is_a_comment_without_debugging_mode() {
    let src = "      * comment\n\
               000100 IDENTIFICATION DIVISION.\n\
               000200D    DISPLAY \"TRACE\".\n\
               000300     DISPLAY \"REAL\".\n";
    for fmt in [SourceFormat::Fixed, SourceFormat::FixedStrict] {
        let toks = tokenize(src, fmt);
        let strings: Vec<&String> = toks
            .iter()
            .filter_map(|st| match &st.token {
                Token::StringLiteral(s) => Some(s),
                _ => None,
            })
            .collect();
        assert_eq!(
            strings,
            vec![&"REAL".to_string()],
            "{fmt:?}: the D line must not be compiled: {strings:?}"
        );
    }
}

/// With the clause, the same line IS source.
#[test]
fn a_debug_line_is_compiled_with_debugging_mode() {
    let src = "000100 IDENTIFICATION DIVISION.\n\
               000150 SOURCE-COMPUTER. XYZ WITH DEBUGGING MODE.\n\
               000200D    DISPLAY \"TRACE\".\n\
               000300     DISPLAY \"REAL\".\n";
    for fmt in [SourceFormat::Fixed, SourceFormat::FixedStrict] {
        let toks = tokenize(src, fmt);
        let strings: Vec<&String> = toks
            .iter()
            .filter_map(|st| match &st.token {
                Token::StringLiteral(s) => Some(s),
                _ => None,
            })
            .collect();
        assert!(
            strings.contains(&&"TRACE".to_string()),
            "{fmt:?}: WITH DEBUGGING MODE must compile the D line: {strings:?}"
        );
        assert!(strings.contains(&&"REAL".to_string()), "{fmt:?}");
    }
}

/// The phrase inside a COMMENT does not switch debugging on — CCVS85 programs
/// describe debugging mode in their banners.
#[test]
fn the_phrase_in_a_comment_does_not_enable_debugging() {
    let src = "000100* THIS PROGRAM TESTS WITH DEBUGGING MODE BEHAVIOUR\n\
               000200D    DISPLAY \"TRACE\".\n\
               000300     DISPLAY \"REAL\".\n";
    let toks = tokenize(src, SourceFormat::FixedStrict);
    let strings: Vec<&String> = toks
        .iter()
        .filter_map(|st| match &st.token {
            Token::StringLiteral(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(strings, vec![&"REAL".to_string()], "{strings:?}");
}

/// **Free format has no indicator area**, so it has no debugging lines: a `D`
/// there is an ordinary COBOL word and nothing special happens to it.
#[test]
fn free_format_has_no_debug_indicator() {
    let toks = tokenize("MOVE D TO WS-X.\n", SourceFormat::Free);
    assert!(
        toks.iter()
            .any(|st| matches!(&st.token, Token::Identifier(s) if s == "D")),
        "a data item named D must survive in free format: {toks:?}"
    );
}
