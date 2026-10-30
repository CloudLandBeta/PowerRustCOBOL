// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Interpolated strings (spec 109 §5.2, R4): `F"Page {WS-PAGE:number} of {WS-PAGES}"`.
//!
//! The lexer hands over the raw body. It is split here into text and
//! expressions:
//!
//! * `{{` and `}}` are a literal `{` and `}`;
//! * `{expr}` or `{expr:format}` — the format is introduced by a SINGLE `:`; a
//!   `::` is always the member operator, so `{WS-A::Trim():number}` is the
//!   expression `WS-A::Trim()` with the format `number`, and a `:` inside
//!   parentheses (`WS-A(1:3)`) or a quoted string is not a format separator;
//! * a string literal inside an expression uses the other quote character (or
//!   the doubled quote, already reduced by the lexer), and a brace inside it is
//!   text;
//! * an unmatched brace or an invalid expression is reported with the literal's
//!   source line and the expression named.
//!
//! The expression is parsed by the ordinary expression parser, so it can be
//! anything an operand can be — and nothing else: the text is never run as
//! source, and no SQL is built from it.

use cobolt_ast::expr::{Expr, InterpPart};
use cobolt_lexer::{tokenize, SourceFormat, Token};

use crate::expr::parse_expr;
use crate::parser::Parser;

/// One piece of the body before the expressions are parsed.
#[derive(Debug, PartialEq)]
enum Raw {
    Text(String),
    Value { source: String, format: Option<String> },
}

/// Split a body into text and `{…}` pieces.
fn split(body: &str) -> Result<Vec<Raw>, String> {
    let chars: Vec<char> = body.chars().collect();
    let mut out = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '{' if chars.get(i + 1) == Some(&'{') => {
                text.push('{');
                i += 2;
            }
            '}' if chars.get(i + 1) == Some(&'}') => {
                text.push('}');
                i += 2;
            }
            '}' => return Err("unmatched '}' in an interpolated string (write '}}' for a brace)".into()),
            '{' => {
                if !text.is_empty() {
                    out.push(Raw::Text(std::mem::take(&mut text)));
                }
                let start = i + 1;
                let mut j = start;
                let mut depth = 0i32;
                let mut quote: Option<char> = None;
                let mut sep: Option<usize> = None;
                let mut closed = false;
                while j < chars.len() {
                    let c = chars[j];
                    if let Some(q) = quote {
                        if c == q {
                            quote = None;
                        }
                    } else {
                        match c {
                            '"' | '\'' => quote = Some(c),
                            '(' => depth += 1,
                            ')' => depth -= 1,
                            '}' if depth <= 0 => {
                                closed = true;
                                break;
                            }
                            ':' if depth <= 0
                                && chars.get(j + 1) != Some(&':')
                                && (j == start || chars[j - 1] != ':') =>
                            {
                                sep = Some(j);
                            }
                            _ => {}
                        }
                    }
                    j += 1;
                }
                if !closed {
                    return Err("unmatched '{' in an interpolated string (write '{{' for a brace)".into());
                }
                let (src, fmt) = match sep {
                    Some(k) => (
                        chars[start..k].iter().collect::<String>(),
                        Some(chars[k + 1..j].iter().collect::<String>().trim().to_string()),
                    ),
                    None => (chars[start..j].iter().collect::<String>(), None),
                };
                if src.trim().is_empty() {
                    return Err("an interpolated string has an empty '{}': name an expression".into());
                }
                if fmt.as_deref() == Some("") {
                    return Err(format!("'{{{}:}}' names no format after the ':'", src.trim()));
                }
                out.push(Raw::Value { source: src.trim().to_string(), format: fmt });
                i = j + 1;
            }
            c => {
                text.push(c);
                i += 1;
            }
        }
    }
    if !text.is_empty() {
        out.push(Raw::Text(text));
    }
    Ok(out)
}

/// The expression of one `{…}`, parsed by the ordinary expression parser. `None`
/// (with the error reported) when it is not exactly one expression.
fn parse_piece(p: &mut Parser, source: &str) -> Option<Expr> {
    let tokens = tokenize(source, SourceFormat::Free);
    let mut sub = Parser::new(tokens);
    let expr = parse_expr(&mut sub);
    while matches!(sub.peek(), Token::Period) {
        sub.advance();
    }
    let complete = sub.at(&Token::Eof);
    if !sub.diagnostics.is_empty() || !complete {
        let why = sub
            .diagnostics
            .first()
            .map(|d| d.message.clone())
            .unwrap_or_else(|| "it is not a single expression".to_string());
        p.emit_error(format!("invalid expression {{{source}}} in an interpolated string: {why}"));
        return None;
    }
    Some(expr)
}

/// Build the [`Expr::Interpolated`] of a lexed body; on a fault, report it and
/// answer an empty string so one bad literal is one diagnostic.
pub(crate) fn parse_interpolated(p: &mut Parser, body: &str, span: cobolt_lexer::Span) -> Expr {
    let raws = match split(body) {
        Ok(r) => r,
        Err(e) => {
            p.emit_error(e);
            return Expr::Interpolated { parts: Vec::new(), span };
        }
    };
    let mut parts = Vec::with_capacity(raws.len());
    for raw in raws {
        match raw {
            Raw::Text(t) => parts.push(InterpPart::Text(t)),
            Raw::Value { source, format } => {
                if let Some(expr) = parse_piece(p, &source) {
                    parts.push(InterpPart::Value { expr, format });
                }
            }
        }
    }
    Expr::Interpolated { parts, span }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_braces_and_expressions_split_as_r4_says() {
        assert_eq!(split("plain").unwrap(), vec![Raw::Text("plain".into())]);
        assert_eq!(split("a{{b}}c").unwrap(), vec![Raw::Text("a{b}c".into())]);
        assert_eq!(
            split("Page {P:number} of {N}").unwrap(),
            vec![
                Raw::Text("Page ".into()),
                Raw::Value { source: "P".into(), format: Some("number".into()) },
                Raw::Text(" of ".into()),
                Raw::Value { source: "N".into(), format: None },
            ]
        );
    }

    #[test]
    fn a_double_colon_is_the_member_operator_not_the_format() {
        assert_eq!(
            split("{WS-A::Trim():number}").unwrap(),
            vec![Raw::Value { source: "WS-A::Trim()".into(), format: Some("number".into()) }]
        );
        assert_eq!(
            split("{WS-A::Len}").unwrap(),
            vec![Raw::Value { source: "WS-A::Len".into(), format: None }]
        );
    }

    #[test]
    fn a_colon_in_parentheses_or_in_a_string_is_not_a_format() {
        assert_eq!(
            split("{WS-A(1:3)}").unwrap(),
            vec![Raw::Value { source: "WS-A(1:3)".into(), format: None }]
        );
        assert_eq!(
            split("{X::Replace('a:b', \"c}d\")}").unwrap(),
            vec![Raw::Value { source: "X::Replace('a:b', \"c}d\")".into(), format: None }]
        );
    }

    #[test]
    fn unmatched_braces_and_empty_pieces_are_faults() {
        assert!(split("a { b").unwrap_err().contains("unmatched '{'"));
        assert!(split("a } b").unwrap_err().contains("unmatched '}'"));
        assert!(split("a {} b").unwrap_err().contains("empty"));
        assert!(split("{X:}").unwrap_err().contains("no format"));
    }
}
