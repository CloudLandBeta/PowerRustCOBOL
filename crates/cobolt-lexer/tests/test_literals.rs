// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tests: string, numeric, and level-number literals.

use cobolt_lexer::{tokenize, SourceFormat, Token};

fn toks(src: &str) -> Vec<Token> {
    tokenize(src, SourceFormat::Free)
        .into_iter()
        .map(|st| st.token)
        .collect()
}

#[test]
fn double_quoted_string() {
    let t = toks(r#"MOVE "Hello, World!" TO WS-MSG."#);
    assert_eq!(t[0], Token::Move);
    assert_eq!(t[1], Token::StringLiteral("Hello, World!".to_string()));
}

#[test]
fn single_quoted_string() {
    let t = toks("MOVE 'Hello' TO WS-NAME.");
    assert_eq!(t[1], Token::StringLiteral("Hello".to_string()));
}

#[test]
fn empty_string() {
    let t = toks(r#"MOVE "" TO WS-X."#);
    assert_eq!(t[1], Token::StringLiteral("".to_string()));
}

#[test]
fn string_with_spaces() {
    let t = toks(r#"MOVE "  spaces  " TO WS-X."#);
    assert_eq!(t[1], Token::StringLiteral("  spaces  ".to_string()));
}

#[test]
fn string_with_special_chars() {
    let t = toks(r#"MOVE "Hello/World-123" TO WS-X."#);
    assert_eq!(t[1], Token::StringLiteral("Hello/World-123".to_string()));
}

#[test]
fn integer_literal_positive() {
    let t = toks("ADD 42 TO WS-COUNT.");
    assert_eq!(t[1], Token::IntegerLiteral(42));
}

#[test]
fn integer_literal_zero() {
    let t = toks("MOVE 0 TO WS-X.");
    let tok = &t[1];
    assert!(
        matches!(tok, Token::IntegerLiteral(0)),
        "expected IntegerLiteral(0), got {tok:?}"
    );
}

#[test]
fn large_integer() {
    let t = toks("MOVE 99999 TO WS-X.");
    assert_eq!(t[1], Token::IntegerLiteral(99999));
}

#[test]
fn decimal_literal_is_exact() {
    let t = toks("COMPUTE WS-PI = 3.14159.");
    let tok = t.iter().find(|t| matches!(t, Token::DecimalLiteral { .. }));
    assert!(tok.is_some(), "expected a DecimalLiteral token");
    if let Some(Token::DecimalLiteral { mantissa, scale }) = tok {
        assert_eq!(*mantissa, 314159);
        assert_eq!(*scale, 5);
    }
}

#[test]
fn decimal_literal_zero_fraction() {
    let t = toks("MOVE 1.0 TO WS-X.");
    let tok = t.iter().find(|t| matches!(t, Token::DecimalLiteral { .. }));
    assert!(matches!(
        tok,
        Some(Token::DecimalLiteral {
            mantissa: 10,
            scale: 1
        })
    ));
}

#[test]
fn decimal_literal_preserves_31_digits() {
    // 18 integer + 13 fractional digits must survive exactly (f64 cannot).
    let t = toks("MOVE 123456789012345678.1234567890123 TO WS-X.");
    let tok = t.iter().find_map(|t| match t {
        Token::DecimalLiteral { mantissa, scale } => Some((*mantissa, *scale)),
        _ => None,
    });
    assert_eq!(tok, Some((1234567890123456781234567890123_i128, 13)));
}

#[test]
fn level_01() {
    let t = toks("01 WS-RECORD.");
    assert_eq!(t[0], Token::LevelNumber(1));
    assert_eq!(t[1], Token::Identifier("WS-RECORD".to_string()));
}

#[test]
fn level_05() {
    let t = toks("05 WS-NAME PIC X(30).");
    assert_eq!(t[0], Token::LevelNumber(5));
}

#[test]
fn level_77() {
    let t = toks("77 WS-FLAG PIC 9.");
    assert_eq!(t[0], Token::LevelNumber(77));
}

#[test]
fn level_88() {
    let t = toks("88 WS-FLAG-ON VALUE 1.");
    assert_eq!(t[0], Token::LevelNumber(88));
}

#[test]
fn level_66() {
    let t = toks("66 WS-ALIAS RENAMES WS-X.");
    assert_eq!(t[0], Token::LevelNumber(66));
}

#[test]
fn non_level_numbers_are_integers() {
    let t = toks("MOVE 50 TO WS-X.");
    assert_eq!(t[1], Token::IntegerLiteral(50));
    let t2 = toks("ADD 100 TO WS-X.");
    assert_eq!(t2[1], Token::IntegerLiteral(100));
}

#[test]
fn arithmetic_operators() {
    let t = toks("COMPUTE X = A + B - C * D / E ** 2.");
    assert!(t.contains(&Token::Plus));
    assert!(t.contains(&Token::Minus));
    assert!(t.contains(&Token::Star));
    assert!(t.contains(&Token::Slash));
    assert!(t.contains(&Token::Power));
}

#[test]
fn comparison_operators() {
    let t = toks("IF A = B AND C < D AND E > F AND G <= H AND I >= J.");
    assert!(t.contains(&Token::Eq));
    assert!(t.contains(&Token::Lt));
    assert!(t.contains(&Token::Gt));
    assert!(t.contains(&Token::LtEq));
    assert!(t.contains(&Token::GtEq));
}

#[test]
fn not_equal_operator() {
    let t = toks("IF A <> B");
    assert!(t.contains(&Token::NotEq));
}

#[test]
fn period_comma_parens() {
    let t = toks("ADD A, B TO C (1).");
    assert!(t.contains(&Token::Comma));
    assert!(t.contains(&Token::LParen));
    assert!(t.contains(&Token::RParen));
    assert!(t.contains(&Token::Period));
}

#[test]
fn comments_excluded_by_default() {
    use cobolt_lexer::tokenize;
    let src = "MOVE A TO B. *> this is a comment";
    let t = tokenize(src, SourceFormat::Free);
    assert!(
        !t.iter().any(|st| matches!(st.token, Token::Comment(_))),
        "comments should not appear in default tokenize() output"
    );
}

#[test]
fn comments_included_when_requested() {
    use cobolt_lexer::tokenize_with_comments;
    let src = "MOVE A TO B. *> this is a comment";
    let t = tokenize_with_comments(src, SourceFormat::Free);
    let comment = t.iter().find(|st| matches!(st.token, Token::Comment(_)));
    assert!(comment.is_some(), "expected a comment token");
    if let Some(st) = comment {
        assert_eq!(st.token, Token::Comment("this is a comment".to_string()));
    }
}

#[test]
fn pic_clause_tokenizes() {
    let t = toks("05 WS-NAME PIC X(30) VALUE SPACES.");
    assert!(t.contains(&Token::Pic));
    assert!(t
        .iter()
        .any(|t| matches!(t, Token::Identifier(s) if s.starts_with('X'))));
}

#[test]
fn pic_numeric_tokenizes() {
    let t = toks("05 WS-AMT PIC 9(7)V99 COMP-3.");
    assert!(t.contains(&Token::Comp3));
}

// ── Hexadecimal literals (COBOL-85 standard) ────────────────────────────────
//
// `X"09"` used to lex as the word `X` followed by the string `09`, so a
// standard literal the Developer's Guide already documented was a parse error
// at the `X` — a developer splitting the Maps control's TAB-separated
// `ResponseBody` hit it immediately.

#[test]
fn hex_literal_is_a_string_literal() {
    let t = toks(r#"UNSTRING WS-IN DELIMITED BY X"09" INTO WS-A."#);
    assert!(
        t.contains(&Token::StringLiteral("\t".to_string())),
        "X\"09\" must be the tab character: {t:?}"
    );
}

#[test]
fn hex_literal_takes_one_character_per_digit_pair() {
    let t = toks(r#"MOVE X"0D0A" TO WS-CRLF."#);
    assert_eq!(t[1], Token::StringLiteral("\r\n".to_string()));
}

#[test]
fn hex_literal_accepts_lowercase_x_and_single_quotes() {
    for src in [r#"MOVE x"41" TO W."#, r#"MOVE X'41' TO W."#, r#"MOVE x'41' TO W."#] {
        let t = toks(src);
        assert_eq!(t[1], Token::StringLiteral("A".to_string()), "src: {src}");
    }
}

/// The `X` prefix must not swallow ordinary code: an item genuinely named `X`,
/// a `PIC X` clause, and a quoted string that merely CONTAINS an X all lex the
/// way they always did. Only a bare `X` written flush against a quote is a hex
/// literal, which is exactly what the standard reserves that spelling for.
#[test]
fn hex_pattern_does_not_disturb_ordinary_x_words() {
    let t = toks(r#"01 X PIC X(10) VALUE "plain"."#);
    assert!(
        t.iter().any(|t| matches!(t, Token::Identifier(s) if s == "X")),
        "an item named X must stay an identifier: {t:?}"
    );
    assert!(t.contains(&Token::StringLiteral("plain".to_string())));

    // The overwhelmingly common shape: an X INSIDE a string. The literal is
    // matched from its opening quote, so the X never begins a token.
    let t = toks(r#"MOVE ALL "X" TO PERF-PAYLOAD."#);
    assert!(t.contains(&Token::StringLiteral("X".to_string())), "{t:?}");

    // A word merely ENDING in X keeps its string separate.
    let t = toks(r#"MOVE WS-X "abc" TO W."#);
    assert!(t.contains(&Token::StringLiteral("abc".to_string())), "{t:?}");
    assert!(t.iter().any(|t| matches!(t, Token::Identifier(s) if s == "WS-X")));
}

/// `X"…"` is the standard's hex spelling, so content that is not whole bytes
/// is a MALFORMED hex literal — rejected, never quietly re-read as the word
/// `X` beside a string, which would hide the developer's typo.
#[test]
fn a_malformed_hex_literal_is_rejected_not_silently_reinterpreted() {
    for src in [r#"DISPLAY X"hello"."#, r#"DISPLAY X"091"."#] {
        let t = toks(src);
        assert!(
            t.iter().any(|t| matches!(t, Token::Error(_))),
            "{src} is not a whole number of bytes and must not lex clean: {t:?}"
        );
    }
}
