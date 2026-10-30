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
        "SPLIT" | "MAP" | "FILTER" | "SORTBYDESCENDING" | "TAKE" | "DESCENDANTS" | "CONTROLS"
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
        // Spec 109 §5.4: the controls of the running form.
        if method == "DESCENDANTS" || method == "CONTROLS" {
            return self.eval_control_collection(&method, recv, args, span).map(Some);
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
        // Over controls the element IS the control: `EL::Hide()` and `EL::Caption`
        // reach it, while it also reads as its id.
        let controls = Self::is_control_collection(collection);
        let mut outcome = Ok(());
        for item in items {
            if controls {
                // Only the alias: were the id also stored as a data item, `EL::Caption`
                // would be read as a method of that text. `EL::Name` is the id.
                self.control_alias.insert(key.clone(), item.as_display_string().trim().to_string());
            } else {
                self.env.raw_set(&key, item);
            }
            match self.exec_loop_body(stmts) {
                super::LoopStep::Continue => {}
                super::LoopStep::Break => break,
                super::LoopStep::Err(e) => {
                    outcome = Err(e);
                    break;
                }
            }
        }
        self.control_alias.remove(&key);
        if !controls {
            match previous {
                Some(p) => self.env.raw_set(&key, p),
                None => {
                    self.env.raw_take(&key);
                }
            }
        } else if let Some(p) = previous {
            self.env.raw_set(&key, p);
        }
        outcome
    }

    /// `true` when `expr` is a chain that answers controls (`C::Descendants()`,
    /// `ME::Controls("A" "B")`): the loop element is then the control itself.
    pub(super) fn is_control_collection(expr: &Expr) -> bool {
        matches!(expr, Expr::Member { member, parens: true, .. }
            if member.eq_ignore_ascii_case("DESCENDANTS") || member.eq_ignore_ascii_case("CONTROLS"))
    }

    /// `C::Descendants()` and `ME::Controls(id …)`: the control ids, as text, in the
    /// order §5.4 fixes — a stable depth-first preorder by the form's document order
    /// for `Descendants`, argument order for `Controls`.
    fn eval_control_collection(
        &mut self,
        method: &str,
        recv: &Expr,
        args: &[Expr],
        span: Span,
    ) -> Result<Vec<CobolValue>, RuntimeError> {
        let id_text = |id: &str| CobolValue::from_str(id, id.len());
        if method == "CONTROLS" {
            let mut seen: Vec<String> = Vec::new();
            for a in args {
                let id = self.eval_expr(a, span)?.as_display_string().trim().to_string();
                if !self.objects.contains(&id) {
                    return Err(general(format!("Controls: '{id}' is not a control of this form")));
                }
                if seen.iter().any(|s| s.eq_ignore_ascii_case(&id)) {
                    return Err(general(format!("Controls: '{id}' is named twice")));
                }
                seen.push(id);
            }
            return Ok(seen.iter().map(|id| id_text(id)).collect());
        }
        // Descendants: the receiver names one control (an element of a control
        // loop counts, through its alias).
        let root = match recv {
            Expr::Identifier(name, _) => self.member_root_key(name),
            _ => return Err(general("Descendants() is a method of a control".into())),
        };
        if !self.objects.contains(&root) {
            return Err(general(format!("Descendants: '{root}' is not a control of this form")));
        }
        if self.control_tree.is_empty() {
            return Err(general("Descendants needs a running form: no controls were seeded".into()));
        }
        let mut out = Vec::new();
        self.collect_descendants_of(&root, &mut out, 0);
        Ok(out.iter().map(|id| id_text(id)).collect())
    }

    /// The controls under `id` by actual parent membership (never by overlap),
    /// root excluded, hidden ones included, depth-first in document order.
    fn collect_descendants_of(&self, id: &str, out: &mut Vec<String>, depth: usize) {
        if depth > 64 {
            return; // a cycle in a hand-edited form: stop rather than loop
        }
        for (child, parent) in &self.control_tree {
            if parent.as_deref().is_some_and(|p| p.eq_ignore_ascii_case(id)) {
                out.push(child.clone());
                self.collect_descendants_of(child, out, depth + 1);
            }
        }
    }

    /// `<controls>::SetProperties("Name" value …)` (§5.4): the values are evaluated once;
    /// every target and every pair is checked BEFORE the first write, so an invalid
    /// one writes nothing; then the writes follow the collection's order through the
    /// ordinary property path, so the host is told as it is for any `SET`.
    pub(super) fn eval_set_properties(
        &mut self,
        recv: &Expr,
        args: &[Expr],
        span: Span,
    ) -> Result<Option<CobolValue>, RuntimeError> {
        if !Self::is_control_collection(recv) {
            return Ok(None);
        }
        let targets = self.eval_collection(recv, span)?.unwrap_or_default();
        if args.is_empty() || args.len() % 2 != 0 {
            return Err(general(
                "SetProperties takes name/value pairs: SetProperties(\"Name\" value \"Name\" value …)".into(),
            ));
        }
        let mut pairs: Vec<(String, String)> = Vec::with_capacity(args.len() / 2);
        for pair in args.chunks(2) {
            let name = self.eval_expr(&pair[0], span)?.as_display_string().trim().to_string();
            if name.is_empty() {
                return Err(general("SetProperties: a property name is empty".into()));
            }
            let value = self.eval_expr(&pair[1], span)?.as_display_string().trim_end().to_string();
            pairs.push((name, value));
        }
        let ids: Vec<String> = targets.iter().map(|t| t.as_display_string().trim().to_string()).collect();
        for id in &ids {
            if !self.objects.contains(id) {
                return Err(general(format!("SetProperties: '{id}' is not a control of this form (nothing was written)")));
            }
        }
        for id in &ids {
            for (name, value) in &pairs {
                self.set_control_property(id, name, value.clone());
            }
        }
        Ok(Some(CobolValue::from_str("", 1)))
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
