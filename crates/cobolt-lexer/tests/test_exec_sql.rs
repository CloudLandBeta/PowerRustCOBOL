// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 087 T2 — the lexer captures `EXEC SQL … END-EXEC` as one token.

use cobolt_lexer::{sql::SqlBlock, tokenize, Lexer, SourceFormat, Token};

fn blocks(src: &str, fmt: SourceFormat) -> Vec<SqlBlock> {
    tokenize(src, fmt)
        .into_iter()
        .filter_map(|t| match t.token {
            Token::ExecSqlBlock(b) => Some(*b),
            _ => None,
        })
        .collect()
}

/// The same program in fixed and in free format gives the same block text and
/// the same origin lines — a fixed-format comment line inside the block
/// included.
#[test]
fn fixed_and_free_format_capture_the_same_block() {
    let fixed = "\
       PROCEDURE DIVISION.
           EXEC SQL
             SELECT NAME
      * a comment line inside the block
               INTO :WS-NAME
               FROM CUSTOMER -- trailing comment
           END-EXEC.
           STOP RUN.
";
    let free = "\
       PROCEDURE DIVISION.
           EXEC SQL
             SELECT NAME
       *> a comment line inside the block
               INTO :WS-NAME
               FROM CUSTOMER -- trailing comment
           END-EXEC.
           STOP RUN.
";
    let a = blocks(fixed, SourceFormat::Fixed);
    let b = blocks(free, SourceFormat::Free);
    assert_eq!(a.len(), 1);
    // The SQL sent (comments dropped, R4) and the origin lines are identical;
    // a fixed-format comment line keeps the space after its `*`, which a
    // comment's own text may differ by.
    let sent = |x: &SqlBlock| cobolt_lexer::sql::strip_line_comments(&x.text);
    assert_eq!(sent(&a[0]), sent(&b[0]), "fixed and free formats must send the same SQL");
    assert_eq!((&a[0].lines, a[0].first_col, a[0].end_line), (&b[0].lines, b[0].first_col, b[0].end_line));
    assert_eq!(a[0].lines, [2, 3, 4, 5, 6, 7]);
    assert_eq!(a[0].end_line, 7);
    assert!(a[0].text.contains("INTO :WS-NAME"));
    // The statement after the block is still COBOL.
    let toks = tokenize(fixed, SourceFormat::Fixed);
    let i = toks.iter().position(|t| matches!(t.token, Token::ExecSqlBlock(_))).unwrap();
    assert_eq!(toks[i + 1].token, Token::Period);
    assert_eq!(toks[i + 2].token, Token::Stop);
    assert_eq!(toks[i].span.line, 2);
}

/// `EXEC SQL` inside an EXEC RUST body, a literal or a comment is not a block,
/// and `obj::Exec` is a method call.
#[test]
fn exec_sql_where_it_is_not_a_block() {
    let rust = "EXEC RUST let s = \"EXEC SQL SELECT 1 END-EXEC\"; END-EXEC.";
    let toks = tokenize(rust, SourceFormat::Free);
    assert!(toks.iter().any(|t| matches!(t.token, Token::ExecRustBlock(_))));
    assert!(!toks.iter().any(|t| matches!(t.token, Token::ExecSqlBlock(_))));

    assert!(blocks("DISPLAY \"EXEC SQL SELECT 1 END-EXEC\".", SourceFormat::Free).is_empty());
    assert!(blocks("*> EXEC SQL SELECT 1 END-EXEC\nSTOP RUN.", SourceFormat::Free).is_empty());

    let member = tokenize("INVOKE OBJ::Exec SQL END-EXEC.", SourceFormat::Free);
    assert!(!member.iter().any(|t| matches!(t.token, Token::ExecSqlBlock(_))), "{member:?}");
}

/// An apostrophe inside an SQL comment must not change how the COBOL after
/// the block is read — even when the COBOL lexer's own reading of it runs
/// across the block's end.
#[test]
fn sql_quoting_never_disturbs_the_cobol_that_follows() {
    for src in [
        "EXEC SQL SELECT 1 -- don't\n END-EXEC. MOVE 'Z' TO B.",
        "EXEC SQL SELECT 1 /* ' */ END-EXEC. MOVE 'Z' TO B.",
        "EXEC SQL SELECT 'END-EXEC' FROM T END-EXEC. MOVE 'Z' TO B.",
    ] {
        let toks = tokenize(src, SourceFormat::Free);
        let kinds: Vec<&Token> = toks.iter().map(|t| &t.token).collect();
        assert!(matches!(kinds[0], Token::ExecSqlBlock(_)), "{src}: {kinds:?}");
        assert_eq!(kinds[1], &Token::Period, "{src}: {kinds:?}");
        assert_eq!(kinds[2], &Token::Move, "{src}: {kinds:?}");
        assert_eq!(kinds[3], &Token::StringLiteral("Z".into()), "{src}: {kinds:?}");
        assert_eq!(kinds[5], &Token::Identifier("B".into()), "{src}: {kinds:?}");
    }
    let b = blocks("EXEC SQL SELECT 'END-EXEC' FROM T END-EXEC.", SourceFormat::Free);
    assert_eq!(b[0].text.trim(), "SELECT 'END-EXEC' FROM T");
}

/// A block with no `END-EXEC` is one error, on the line of its `EXEC`.
#[test]
fn a_missing_end_exec_is_one_error_on_the_exec_line() {
    let src = "MOVE 1 TO A.\nEXEC SQL\n SELECT 1\n FROM T.\nSTOP RUN.\n";
    let mut lx = Lexer::new(src, SourceFormat::Free);
    while let Some(t) = lx.next_token() {
        if t.token == Token::Eof {
            break;
        }
    }
    let errs = lx.errors();
    assert_eq!(errs.len(), 1, "{errs:?}");
    let msg = errs[0].to_string();
    assert!(msg.contains("EXEC SQL"), "{msg}");
    assert!(msg.contains(":2:") || msg.contains("2:"), "{msg}");
}
