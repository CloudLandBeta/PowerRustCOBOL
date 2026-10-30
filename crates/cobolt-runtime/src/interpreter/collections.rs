// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Collections (spec 109 §5.3): `Split`, `Map`, `Filter`, `SortByDescending`,
//! `Take` yield an ordered list of values; `Join`, `Sum` and `Max` reduce one to a
//! single value; `PERFORM VARYING element THROUGH collection` walks one.
//!
//! **A collection is not a COBOL value.** It exists only while a chain is being
//! evaluated, as the receiver of the next method, the source of a `THROUGH` loop
//! or the source of an assignment to a table. So `X::Split(",")` keeps answering
//! one part when it is used as a value (R38) and becomes a list only where a
//! collection is wanted.
//!
//! **The lambda is an expression.** `ITEM => ITEM::Trim()` binds `ITEM` for the
//! body and nothing else: the body is a value or a test, never a statement, so it
//! cannot assign or do I/O. A failing element fails the whole expression and
//! names the element's position (Q3).

use cobolt_ast::expr::{CmpOp, Expr, LambdaBody};
use cobolt_lexer::Span;

use super::{compare_values, Interpreter};
use crate::error::RuntimeError;
use crate::value::CobolValue;

/// The methods that turn a collection into a collection.
pub(super) fn is_collection_producer(name: &str) -> bool {
    matches!(
        name.to_ascii_uppercase().as_str(),
        "SPLIT" | "MAP" | "FILTER" | "SORTBYDESCENDING" | "TAKE"
    )
}

/// The methods that turn a collection into one value.
pub(super) fn is_collection_reducer(name: &str) -> bool {
    matches!(name.to_ascii_uppercase().as_str(), "JOIN" | "SUM" | "MAX")
}

fn general(message: String) -> RuntimeError {
    RuntimeError::General { message }
}

impl Interpreter {
    /// `Some(list)` when `expr` is a collection-producing chain
    /// (`RAW::Split(",")::Map(ITEM => …)`), `None` when it is anything else — the
    /// caller then evaluates it as the ordinary value it is.
    pub(super) fn eval_collection(
        &mut self,
        expr: &Expr,
        span: Span,
    ) -> Result<Option<Vec<CobolValue>>, RuntimeError> {
        let Expr::Member { recv, member, args, parens: true, .. } = expr else {
            return Ok(None);
        };
        let method = member.to_ascii_uppercase();
        if !is_collection_producer(&method) {
            return Ok(None);
        }
        if method == "SPLIT" {
            // `Split` is a collection only as the receiver of another
            // collection method (the caller asks); its receiver is the text.
            let sep = match args.first() {
                Some(a) => self.eval_expr(a, span)?.as_display_string(),
                None => " ".to_string(),
            };
            let text = self.eval_expr(recv, span)?.as_display_string();
            let parts = if sep.is_empty() {
                vec![text]
            } else {
                text.split(sep.as_str()).map(str::to_string).collect()
            };
            return Ok(Some(parts.iter().map(|p| CobolValue::from_str(p, p.len())).collect()));
        }
        let Some(items) = self.eval_collection(recv, span)? else {
            return Err(general(format!(
                "'{member}' needs a collection: the receiver of '::{member}()' must be a \
                 Split(…) or another collection method"
            )));
        };
        match method.as_str() {
            "MAP" => {
                let (param, body) = lambda_arg(args, member)?;
                let LambdaBody::Value(value) = body else {
                    return Err(general("Map needs a lambda that computes a value, not a test".into()));
                };
                let mut out = Vec::with_capacity(items.len());
                for (i, item) in items.into_iter().enumerate() {
                    out.push(self.with_element(param, item, i, member, |me| me.eval_expr(value, span))?);
                }
                Ok(Some(out))
            }
            "FILTER" => {
                let (param, body) = lambda_arg(args, member)?;
                let LambdaBody::Test(test) = body else {
                    return Err(general("Filter needs a lambda that is a test, such as ITEM <> \"\"".into()));
                };
                let mut out = Vec::new();
                for (i, item) in items.into_iter().enumerate() {
                    let keep = self.with_element(param, item.clone(), i, member, |me| {
                        me.eval_condition(test).map(|b| CobolValue::from_i64(b as i64))
                    })?;
                    if !keep.is_zero() {
                        out.push(item);
                    }
                }
                Ok(Some(out))
            }
            "SORTBYDESCENDING" => {
                // Stable and non-mutating: the input list is consumed here and a
                // new one answered. A lambda names the key; without one the
                // element is its own key.
                let mut keyed: Vec<(CobolValue, CobolValue)> = Vec::with_capacity(items.len());
                for (i, item) in items.into_iter().enumerate() {
                    let key = match args.first() {
                        Some(Expr::Lambda { param, body, .. }) => match body.as_ref() {
                            LambdaBody::Value(v) => {
                                self.with_element(param, item.clone(), i, member, |me| me.eval_expr(v, span))?
                            }
                            LambdaBody::Test(_) => {
                                return Err(general("SortByDescending needs a lambda that computes a key".into()))
                            }
                        },
                        Some(other) => {
                            return Err(general(format!(
                                "SortByDescending takes a lambda such as ITEM => ITEM::Len(){}",
                                if matches!(other, Expr::Lambda { .. }) { "" } else { " (not a plain value)" }
                            )))
                        }
                        None => item.clone(),
                    };
                    keyed.push((key, item));
                }
                // Descending and stable: equal keys keep their input order.
                keyed.sort_by(|a, b| {
                    if gt(&a.0, &b.0) {
                        std::cmp::Ordering::Less
                    } else if gt(&b.0, &a.0) {
                        std::cmp::Ordering::Greater
                    } else {
                        std::cmp::Ordering::Equal
                    }
                });
                Ok(Some(keyed.into_iter().map(|(_, v)| v).collect()))
            }
            "TAKE" => {
                let n = match args.first() {
                    Some(a) => self.eval_expr(a, span)?.as_i64().unwrap_or(-1),
                    None => -1,
                };
                if n < 0 {
                    return Err(general("Take needs a count of zero or more".into()));
                }
                Ok(Some(items.into_iter().take(n as usize).collect()))
            }
            _ => Ok(None),
        }
    }

    /// `Join`, `Sum` and `Max`: the receiver's collection reduced to one value.
    /// `None` when the receiver is not a collection.
    pub(super) fn eval_reduction(
        &mut self,
        recv: &Expr,
        member: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Option<CobolValue>, RuntimeError> {
        let Some(items) = self.eval_collection(recv, span)? else {
            return Ok(None);
        };
        match member.to_ascii_uppercase().as_str() {
            "JOIN" => {
                let sep = match args.first() {
                    Some(a) => self.eval_expr(a, span)?.as_display_string(),
                    None => String::new(),
                };
                let text = items.iter().map(|v| v.as_display_string()).collect::<Vec<_>>().join(&sep);
                Ok(Some(CobolValue::from_str(&text, text.len())))
            }
            // The sum of nothing is zero (R15).
            "SUM" => Ok(Some(items.iter().fold(CobolValue::from_i64(0), |acc, v| acc.add_val(v)))),
            "MAX" => {
                let mut it = items.into_iter();
                let Some(first) = it.next() else {
                    return Err(general("Max of an empty collection has no value".into()));
                };
                Ok(Some(it.fold(first, |best, v| if gt(&v, &best) { v } else { best })))
            }
            _ => Ok(None),
        }
    }

    /// The elements a `THROUGH` loop walks, snapshotted now: a collection chain,
    /// or the occurrences of an OCCURS table named without a subscript.
    fn collection_to_walk(&mut self, expr: &Expr, span: Span) -> Result<Vec<CobolValue>, RuntimeError> {
        if let Some(items) = self.eval_collection(expr, span)? {
            return Ok(items);
        }
        let table = match expr {
            Expr::Identifier(name, _) => Some(name.clone()),
            _ => None,
        };
        if let Some(name) = table {
            if let Some(count) = self.env.table_occurrences(&name).filter(|n| *n > 0) {
                let mut out = Vec::with_capacity(count);
                for i in 1..=count {
                    let occurrence = Expr::Subscript {
                        base: Box::new(expr.clone()),
                        indices: vec![Expr::Literal(cobolt_ast::expr::Literal::Integer(i as i64), span)],
                        span,
                    };
                    out.push(self.eval_expr(&occurrence, span)?);
                }
                return Ok(out);
            }
        }
        Err(general(
            "PERFORM VARYING … THROUGH needs a collection: a Split(…) chain, a Map/Filter/… \
             result, or an OCCURS table"
                .into(),
        ))
    }

    /// `PERFORM VARYING element THROUGH collection … END-PERFORM`. The element
    /// exists only inside the loop; `EXIT PERFORM [CYCLE]` and nesting behave as in
    /// the other inline PERFORMs.
    pub(super) fn exec_perform_through(
        &mut self,
        element: &str,
        collection: &Expr,
        stmts: &[cobolt_ast::stmt::Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let items = self.collection_to_walk(collection, span)?;
        let key = element.to_ascii_uppercase();
        let previous = self.env.raw_take(&key);
        let mut outcome = Ok(());
        for item in items {
            self.env.raw_set(&key, item);
            match self.exec_loop_body(stmts) {
                super::LoopStep::Continue => {}
                super::LoopStep::Break => break,
                super::LoopStep::Err(e) => {
                    outcome = Err(e);
                    break;
                }
            }
        }
        match previous {
            Some(p) => self.env.raw_set(&key, p),
            None => {
                self.env.raw_take(&key);
            }
        }
        outcome
    }

    /// Run `f` with the lambda's element name bound to `value`, then take the name
    /// away again. A failure is reported with the element's 1-based position.
    fn with_element(
        &mut self,
        param: &str,
        value: CobolValue,
        index: usize,
        method: &str,
        f: impl FnOnce(&mut Self) -> Result<CobolValue, RuntimeError>,
    ) -> Result<CobolValue, RuntimeError> {
        let key = param.to_ascii_uppercase();
        let previous = self.env.raw_take(&key);
        self.env.raw_set(&key, value);
        let result = f(self);
        match previous {
            Some(p) => self.env.raw_set(&key, p),
            None => {
                self.env.raw_take(&key);
            }
        }
        result.map_err(|e| general(format!("{method}: element {} failed: {e}", index + 1)))
    }
}

/// The lambda a `Map`/`Filter`/`SortByDescending` call was given.
fn lambda_arg<'a>(args: &'a [Expr], method: &str) -> Result<(&'a str, &'a LambdaBody), RuntimeError> {
    match args.first() {
        Some(Expr::Lambda { param, body, .. }) => Ok((param.as_str(), body.as_ref())),
        _ => Err(general(format!("{method} takes a lambda: {method}(ITEM => …)"))),
    }
}

/// `a > b`, numerically when both sides read as numbers (the pieces of a
/// `Split("3,10,7")` are text, and `"7" > "10"` as text), as text otherwise.
fn gt(a: &CobolValue, b: &CobolValue) -> bool {
    let number = |v: &CobolValue| match v {
        CobolValue::String { bytes, .. } => crate::value::parse_decimal(String::from_utf8_lossy(bytes).trim()),
        other => other.as_exact(),
    };
    match (number(a), number(b)) {
        (Some(x), Some(y)) => x.cmp(&y) == std::cmp::Ordering::Greater,
        _ => compare_values(a, b, CmpOp::Gt),
    }
}
