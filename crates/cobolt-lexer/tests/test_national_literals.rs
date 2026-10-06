// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 077 T2 — national and UTF-8 literals: `N"…"`, `N'…'`, `NX"…"`,
//! `U"…"`, `U'…'`, `UX"…"`, IBM's `\u` / `\U` escapes, and each malformed
//! form reported with its reason.

use cobolt_lexer::{tokenize, SourceFormat, Token};

fn tokens(src: &str) -> Vec<Token> {
    tokenize(src, SourceFormat::Free).into_iter().map(|t| t.token).filter(|t| !matches!(t, Token::Eof)).collect()
}

fn one(src: &str) -> Token {
    let t = tokens(src);
    assert_eq!(t.len(), 1, "{src}: {t:?}");
    t.into_iter().next().unwrap()
}

#[test]
fn national_literals() {
    assert_eq!(one(r#"N"Ação""#), Token::NationalLiteral("Ação".into()));
    assert_eq!(one("n'Ação'"), Token::NationalLiteral("Ação".into()));
    assert_eq!(one(r#"N"say ""hi""""#), Token::NationalLiteral("say \"hi\"".into()));
    assert_eq!(one(r#"NX"00410063""#), Token::NationalLiteral("Ac".into()));
    // A surrogate pair is one character.
    assert_eq!(one(r#"nx"D83DDE00""#), Token::NationalLiteral("😀".into()));
}

#[test]
fn utf8_literals_and_ibm_escapes() {
    assert_eq!(one(r#"U"Ação""#), Token::Utf8Literal("Ação".into()));
    assert_eq!(one("u'café'"), Token::Utf8Literal("café".into()));
    assert_eq!(one(r#"UX"C3A7""#), Token::Utf8Literal("ç".into()));
    assert_eq!(one(r#"U"ç\U0001F600\\x""#), Token::Utf8Literal("ç😀\\x".into()));
    // A backslash before anything else is itself.
    assert_eq!(one(r#"U"a\b""#), Token::Utf8Literal("a\\b".into()));
}

#[test]
fn malformed_literals_name_their_fault() {
    let err = |src: &str| match one(src) {
        Token::Error(e) => e,
        other => panic!("{src} should be an error, got {other:?}"),
    };
    assert!(err(r#"NX"0041006""#).contains("four hex digits per character, and has 7"));
    assert!(err(r#"NX"00G1""#).contains("not hexadecimal"));
    assert!(err(r#"NX"D800""#).contains("unpaired surrogate"));
    assert!(err(r#"UX"C3""#).contains("not valid UTF-8"));
    assert!(err(r#"UX"C0AF""#).contains("not valid UTF-8"), "an overlong form");
    assert!(err(r#"UX"ABC""#).contains("two hex digits per byte"));
    assert!(err(r#"U"\uD800""#).contains("not a valid escape"), "a surrogate escape");
    assert!(err(r#"U"\u00E""#).contains("not a valid escape"));
}

#[test]
fn ordinary_words_and_strings_are_untouched() {
    assert_eq!(
        tokens(r#"MOVE NAME TO X"#).len(),
        4,
        "a word starting with N is a word"
    );
    // `N` followed by a space and a string stays a word and a string.
    assert_eq!(tokens(r#"N "x""#), [Token::Identifier("N".into()), Token::StringLiteral("x".into())]);
    assert_eq!(one(r#"X"41""#), Token::StringLiteral("A".into()));
}
