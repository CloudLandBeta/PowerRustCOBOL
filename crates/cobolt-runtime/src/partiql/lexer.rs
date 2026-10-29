// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The statement lexer (spec 108, §5.1).
//!
//! Keywords are matched case-insensitively by the parser, on [`Tok::Ident`].
//! A name is bare or in double quotes (a quoted name is never a keyword). A
//! hyphen continues a name when a letter, digit or underscore follows it, so
//! `CUST-ID` and `QTY-1` are names and `QTY - 1` is a subtraction. Strings use
//! single quotes with `''` for a quote; a backslash is an ordinary character.

use super::{PartiqlError, Pos};

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    /// A bare name or keyword, as written.
    Ident(String),
    /// A double-quoted name, never a keyword.
    Quoted(String),
    /// A single-quoted string, unescaped.
    Str(String),
    /// A number as written (`12`, `12.50`, `-3`, `1e3`).
    Num(String),
    Param,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Dot,
    Colon,
    Semi,
    Star,
    Plus,
    Minus,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub pos: Pos,
    /// Byte range in the statement text.
    pub start: usize,
    pub end: usize,
}

fn name_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn name_part(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether a token ends a value, so that a following `-` is subtraction and
/// not the sign of a number.
fn ends_value(t: &Tok) -> bool {
    matches!(
        t,
        Tok::Ident(_) | Tok::Quoted(_) | Tok::Str(_) | Tok::Num(_) | Tok::Param | Tok::RParen | Tok::RBracket
    )
}

/// Lex `text` into tokens ending with [`Tok::Eof`].
pub fn lex(text: &str) -> Result<Vec<Token>, PartiqlError> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out: Vec<Token> = Vec::new();
    let (mut i, mut line, mut col) = (0usize, 1u32, 1u32);
    let byte_at = |i: usize| chars.get(i).map_or(text.len(), |c| c.0);
    while i < chars.len() {
        let (b, c) = chars[i];
        let pos = Pos { line, col };
        // Advance helper state is updated inline so columns stay exact.
        if c == '\n' {
            i += 1;
            line += 1;
            col = 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            col += 1;
            continue;
        }
        let start_i = i;
        let tok = match c {
            '\'' => {
                let mut s = String::new();
                i += 1;
                let mut closed = false;
                while i < chars.len() {
                    let ch = chars[i].1;
                    if ch == '\'' {
                        if chars.get(i + 1).map(|c| c.1) == Some('\'') {
                            s.push('\'');
                            i += 2;
                            continue;
                        }
                        i += 1;
                        closed = true;
                        break;
                    }
                    s.push(ch);
                    i += 1;
                }
                if !closed {
                    return Err(PartiqlError::validation("unterminated string", Some(pos)));
                }
                Tok::Str(s)
            }
            '"' => {
                let mut s = String::new();
                i += 1;
                let mut closed = false;
                while i < chars.len() {
                    let ch = chars[i].1;
                    if ch == '"' {
                        if chars.get(i + 1).map(|c| c.1) == Some('"') {
                            s.push('"');
                            i += 2;
                            continue;
                        }
                        i += 1;
                        closed = true;
                        break;
                    }
                    s.push(ch);
                    i += 1;
                }
                if !closed {
                    return Err(PartiqlError::validation("unterminated quoted name", Some(pos)));
                }
                if s.is_empty() {
                    return Err(PartiqlError::validation("empty quoted name", Some(pos)));
                }
                Tok::Quoted(s)
            }
            c if name_start(c) => {
                let mut s = String::new();
                while i < chars.len() {
                    let ch = chars[i].1;
                    if name_part(ch) {
                        s.push(ch);
                        i += 1;
                    } else if ch == '-' && chars.get(i + 1).is_some_and(|n| name_part(n.1)) {
                        s.push(ch);
                        i += 1;
                    } else {
                        break;
                    }
                }
                Tok::Ident(s)
            }
            c if c.is_ascii_digit()
                || (c == '-'
                    && chars.get(i + 1).is_some_and(|n| n.1.is_ascii_digit())
                    && !out.last().is_some_and(|t| ends_value(&t.tok))) =>
            {
                let mut s = String::new();
                if c == '-' {
                    s.push('-');
                    i += 1;
                }
                while i < chars.len() && chars[i].1.is_ascii_digit() {
                    s.push(chars[i].1);
                    i += 1;
                }
                if chars.get(i).map(|c| c.1) == Some('.')
                    && chars.get(i + 1).is_some_and(|n| n.1.is_ascii_digit())
                {
                    s.push('.');
                    i += 1;
                    while i < chars.len() && chars[i].1.is_ascii_digit() {
                        s.push(chars[i].1);
                        i += 1;
                    }
                }
                // An exponent: `e3`, `E+3`, `e-3`.
                if matches!(chars.get(i).map(|c| c.1), Some('e' | 'E')) {
                    let mut j = i + 1;
                    if matches!(chars.get(j).map(|c| c.1), Some('+' | '-')) {
                        j += 1;
                    }
                    if chars.get(j).is_some_and(|n| n.1.is_ascii_digit()) {
                        while i < j {
                            s.push(chars[i].1);
                            i += 1;
                        }
                        while i < chars.len() && chars[i].1.is_ascii_digit() {
                            s.push(chars[i].1);
                            i += 1;
                        }
                    }
                }
                // A number glued to a name (`12AB`) is neither: report it here.
                if chars.get(i).is_some_and(|n| name_start(n.1)) {
                    return Err(PartiqlError::validation(
                        format!("unexpected '{}' after number {}", chars[i].1, s),
                        Some(Pos { line, col: col + (i - start_i) as u32 }),
                    ));
                }
                Tok::Num(s)
            }
            '?' => {
                i += 1;
                Tok::Param
            }
            '(' => {
                i += 1;
                Tok::LParen
            }
            ')' => {
                i += 1;
                Tok::RParen
            }
            '[' => {
                i += 1;
                Tok::LBracket
            }
            ']' => {
                i += 1;
                Tok::RBracket
            }
            '{' => {
                i += 1;
                Tok::LBrace
            }
            '}' => {
                i += 1;
                Tok::RBrace
            }
            ',' => {
                i += 1;
                Tok::Comma
            }
            '.' => {
                i += 1;
                Tok::Dot
            }
            ':' => {
                i += 1;
                Tok::Colon
            }
            ';' => {
                i += 1;
                Tok::Semi
            }
            '*' => {
                i += 1;
                Tok::Star
            }
            '+' => {
                i += 1;
                Tok::Plus
            }
            '-' => {
                i += 1;
                Tok::Minus
            }
            '=' => {
                i += 1;
                Tok::Eq
            }
            '<' => match chars.get(i + 1).map(|c| c.1) {
                Some('=') => {
                    i += 2;
                    Tok::Le
                }
                Some('>') => {
                    i += 2;
                    Tok::Ne
                }
                _ => {
                    i += 1;
                    Tok::Lt
                }
            },
            '>' => {
                if chars.get(i + 1).map(|c| c.1) == Some('=') {
                    i += 2;
                    Tok::Ge
                } else {
                    i += 1;
                    Tok::Gt
                }
            }
            '!' if chars.get(i + 1).map(|c| c.1) == Some('=') => {
                i += 2;
                Tok::Ne
            }
            other => {
                return Err(PartiqlError::validation(
                    format!("unexpected character '{other}'"),
                    Some(pos),
                ));
            }
        };
        // Columns advance by characters consumed; strings may span lines.
        let consumed = &chars[start_i..i];
        for &(_, ch) in consumed {
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        out.push(Token { tok, pos, start: b, end: byte_at(i) });
    }
    out.push(Token {
        tok: Tok::Eof,
        pos: Pos { line, col },
        start: text.len(),
        end: text.len(),
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        lex(s).unwrap().into_iter().map(|t| t.tok).collect()
    }

    fn id(s: &str) -> Tok {
        Tok::Ident(s.into())
    }

    #[test]
    fn a_hyphen_inside_a_name_is_part_of_the_name() {
        assert_eq!(toks("QTY-1"), vec![id("QTY-1"), Tok::Eof]);
        assert_eq!(toks("CUST-ID"), vec![id("CUST-ID"), Tok::Eof]);
        assert_eq!(
            toks("QTY - 1"),
            vec![id("QTY"), Tok::Minus, Tok::Num("1".into()), Tok::Eof]
        );
        assert_eq!(
            toks("QTY -1"),
            vec![id("QTY"), Tok::Minus, Tok::Num("1".into()), Tok::Eof]
        );
    }

    #[test]
    fn numbers_in_every_written_form() {
        for n in ["12", "12.50", "-3", "1e3", "2.5E-2", "0"] {
            assert_eq!(toks(n), vec![Tok::Num(n.into()), Tok::Eof], "{n}");
        }
        // A sign after `=` or `(` belongs to the number; after a value it is subtraction.
        assert_eq!(
            toks("A = -3"),
            vec![id("A"), Tok::Eq, Tok::Num("-3".into()), Tok::Eof]
        );
        assert_eq!(
            toks("3 -3"),
            vec![Tok::Num("3".into()), Tok::Minus, Tok::Num("3".into()), Tok::Eof]
        );
    }

    #[test]
    fn strings_double_their_quote_and_keep_backslashes() {
        assert_eq!(toks("'O''Brien'"), vec![Tok::Str("O'Brien".into()), Tok::Eof]);
        assert_eq!(toks(r"'a\b'"), vec![Tok::Str(r"a\b".into()), Tok::Eof]);
        assert_eq!(toks("''"), vec![Tok::Str(String::new()), Tok::Eof]);
    }

    #[test]
    fn a_quoted_name_is_never_a_keyword() {
        assert_eq!(toks("\"VALUE\""), vec![Tok::Quoted("VALUE".into()), Tok::Eof]);
        assert_eq!(toks("\"A\"\"B\""), vec![Tok::Quoted("A\"B".into()), Tok::Eof]);
    }

    #[test]
    fn operators_and_punctuation() {
        assert_eq!(
            toks("<> != <= >= < > = ? ( ) [ ] { } , . : ; * +"),
            vec![
                Tok::Ne,
                Tok::Ne,
                Tok::Le,
                Tok::Ge,
                Tok::Lt,
                Tok::Gt,
                Tok::Eq,
                Tok::Param,
                Tok::LParen,
                Tok::RParen,
                Tok::LBracket,
                Tok::RBracket,
                Tok::LBrace,
                Tok::RBrace,
                Tok::Comma,
                Tok::Dot,
                Tok::Colon,
                Tok::Semi,
                Tok::Star,
                Tok::Plus,
                Tok::Eof
            ]
        );
    }

    #[test]
    fn keywords_are_idents_with_their_case_kept() {
        assert_eq!(toks("select From"), vec![id("select"), id("From"), Tok::Eof]);
    }

    #[test]
    fn every_token_carries_a_line_and_a_column() {
        let t = lex("SELECT *\n  FROM x").unwrap();
        assert_eq!(t[0].pos, Pos { line: 1, col: 1 });
        assert_eq!(t[1].pos, Pos { line: 1, col: 8 });
        assert_eq!(t[2].pos, Pos { line: 2, col: 3 });
        assert_eq!(t[3].pos, Pos { line: 2, col: 8 });
        assert_eq!(&"SELECT *\n  FROM x"[t[3].start..t[3].end], "x");
    }

    #[test]
    fn an_unterminated_string_is_reported_where_it_starts() {
        let e = lex("SELECT 'abc").unwrap_err();
        assert_eq!(e.exception, "ValidationException");
        assert_eq!(e.pos, Some(Pos { line: 1, col: 8 }));
        assert!(e.message.contains("unterminated string"));
    }

    #[test]
    fn an_unexpected_character_is_reported_with_its_position() {
        let e = lex("a = #").unwrap_err();
        assert_eq!(e.pos, Some(Pos { line: 1, col: 5 }));
    }

    #[test]
    fn a_number_glued_to_a_name_is_refused() {
        assert!(lex("12AB").is_err());
    }
}
