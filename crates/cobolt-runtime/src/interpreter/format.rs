// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `FORMAT` and `LOCALE` at run time (spec 109 §5.2): interpolated strings,
//! `Format()`, and the text contexts — `DISPLAY` and `STRING` — of an item
//! declared with `FORMAT IS …`.
//!
//! **Where formatting happens.** Only where text is made of a number: the four
//! contexts above. `MOVE`, group items, files, SQL, `REDEFINES` and every
//! comparison or arithmetic see the stored value, byte for byte (R10).
//!
//! **Precedence** (§5.2): the explicit format of the call (`{X:percent}`,
//! `X::Format("percent")`), then the item's own `FORMAT`/`LOCALE` clauses, then
//! the application's default locale. The locale operand is read on EVERY
//! operation, so changing `WS-LOCALE` changes the next display and nothing else.
//!
//! **A format string is a name plus an optional currency:** `"currency BRL"`.
//! The currency of `currency` and `currency-short` comes from there, else from the
//! project's default currency, else it is an error.

use cobolt_ast::expr::{Expr, InterpPart};
use cobolt_lexer::Span;

use super::Interpreter;
use crate::error::RuntimeError;
use crate::value::CobolValue;

fn general(message: String) -> RuntimeError {
    RuntimeError::General { message }
}

/// `"currency BRL"` → (`"currency"`, `Some("BRL")`).
fn split_format(spec: &str) -> (&str, Option<&str>) {
    let mut words = spec.split_whitespace();
    let name = words.next().unwrap_or("");
    (name, words.next())
}

impl Interpreter {
    /// The locale tag a clause or an explicit argument names: a string literal, or
    /// an alphanumeric item read NOW, its trailing blanks not part of the tag.
    fn locale_tag(&mut self, locale: Option<&Expr>, span: Span) -> Result<String, RuntimeError> {
        match locale {
            Some(e) => Ok(self.eval_expr(e, span)?.as_display_string().trim().to_string()),
            None => Ok(self.env.format_default_locale().to_string()),
        }
    }

    /// Format `value` as `spec` ("number", "currency BRL" …) in `locale`.
    pub(super) fn format_value(
        &mut self,
        value: &CobolValue,
        spec: &str,
        locale: Option<&Expr>,
        span: Span,
    ) -> Result<String, RuntimeError> {
        let tag = self.locale_tag(locale, span)?;
        let (name, currency) = split_format(spec);
        let currency = currency.or_else(|| self.env.format_default_currency()).map(str::to_string);
        let Some(n) = value.as_exact() else {
            return Err(general(format!("FORMAT '{spec}' needs a numeric item, not a floating-point one")));
        };
        if matches!(value, CobolValue::String { bytes, .. } if crate::value::parse_decimal(String::from_utf8_lossy(bytes).trim()).is_none())
        {
            return Err(general(format!("FORMAT '{spec}' needs a numeric item; the value is text")));
        }
        cobolt_format::format_decimal(n.mantissa, n.decimals, name, &tag, currency.as_deref())
            .map_err(|e| general(e.to_string()))
    }

    /// The text of a data-item operand that carries a `FORMAT` clause, or `None`
    /// when it carries none (the caller then shows it as it always did).
    pub(super) fn formatted_item(&mut self, expr: &Expr, span: Span) -> Result<Option<String>, RuntimeError> {
        let name = match expr {
            Expr::Identifier(n, _) | Expr::Qualified { name: n, .. } => n.clone(),
            _ => return Ok(None),
        };
        let key = self.env.resolve_name(&name, &[]);
        let Some(clause) = self.env.symbol(&key).and_then(|s| s.format.clone()) else {
            return Ok(None);
        };
        let value = self.eval_expr(expr, span)?;
        self.format_value(&value, &clause.name, clause.locale.as_ref(), span).map(Some)
    }

    /// `F"…{expr:format}…"`: each expression evaluated once, left to right.
    pub(super) fn eval_interpolated(&mut self, parts: &[InterpPart], span: Span) -> Result<CobolValue, RuntimeError> {
        let mut out = String::new();
        for part in parts {
            match part {
                InterpPart::Text(t) => out.push_str(t),
                InterpPart::Value { expr, format: Some(spec) } => {
                    let value = self.eval_expr(expr, span)?;
                    // The item's own LOCALE applies to an explicit format too.
                    let locale = self.item_locale(expr);
                    out.push_str(&self.format_value(&value, spec, locale.as_ref(), span)?);
                }
                InterpPart::Value { expr, format: None } => match self.formatted_item(expr, span)? {
                    Some(s) => out.push_str(&s),
                    None => out.push_str(self.eval_expr(expr, span)?.as_display_string().trim_end()),
                },
            }
        }
        let n = out.len();
        Ok(CobolValue::from_str(&out, n))
    }

    /// The `LOCALE` operand of the item `expr` names, if it has one.
    fn item_locale(&mut self, expr: &Expr) -> Option<Expr> {
        let name = match expr {
            Expr::Identifier(n, _) | Expr::Qualified { name: n, .. } => n.clone(),
            _ => return None,
        };
        let key = self.env.resolve_name(&name, &[]);
        self.env.symbol(&key).and_then(|s| s.format.as_ref()).and_then(|f| f.locale.clone())
    }

    /// `X::Format()`, `X::Format("percent")`, `X::Format("currency", WS-LOCALE)`.
    pub(super) fn eval_format_method(
        &mut self,
        recv: &Expr,
        args: &[Expr],
        span: Span,
    ) -> Result<CobolValue, RuntimeError> {
        let value = self.eval_expr(recv, span)?;
        let text = match args.first() {
            Some(spec) => {
                let spec = self.eval_expr(spec, span)?.as_display_string();
                let locale = match args.get(1) {
                    Some(l) => Some(l.clone()),
                    None => self.item_locale(recv),
                };
                self.format_value(&value, spec.trim(), locale.as_ref(), span)?
            }
            None => match self.formatted_item(recv, span)? {
                Some(s) => s,
                None => {
                    return Err(general(
                        "Format() with no arguments needs an item declared with FORMAT IS …; \
                         name the format: Format(\"number\")"
                            .into(),
                    ))
                }
            },
        };
        let n = text.len();
        Ok(CobolValue::from_str(&text, n))
    }
}
