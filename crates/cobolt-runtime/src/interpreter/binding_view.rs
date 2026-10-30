// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The view options of a READ-ONLY data binding (spec 109 S8): the order its rows are
//! shown in, and how a numeric column is written.
//!
//! The form host publishes them as `_BindingOrderBy` (`FIELD` or `-FIELD` for
//! descending) and `_BindingFormats` (`FIELD=format` lines, the `cobolt-format`
//! spelling) and **only for a read-only binding** — the Guardian refuses them on a
//! writable one, because a writable grid finds a row by its line (spec 095) and sends
//! an edit's cell as displayed. So by the time a grid reaches here with these props
//! nothing is written back through it.
//!
//! The order is applied first, on the values as the source gave them (a numeric column
//! sorts as numbers, a text column as text, ties keep the source's order); the format
//! is applied after, to the cells of the columns that name one. A cell that is not a
//! number (blank, NULL) is left as it is. The rows of the source are never touched.

use super::collections::value_gt;
use super::Interpreter;
use crate::value::CobolValue;

impl Interpreter {
    /// Order and format `rows` (tab-joined cells, one entry per grid line, columns in
    /// the order of `columns`); `origins` — the source line each row came from, when the
    /// path keeps them — moves with its row. Answers both, in the order shown.
    pub(super) fn apply_binding_view(
        &mut self,
        control: &str,
        columns: &[String],
        rows: Vec<String>,
        origins: Vec<String>,
    ) -> (Vec<String>, Vec<String>) {
        let order = self.obj_get(control, "_BindingOrderBy");
        let formats = self.obj_get(control, "_BindingFormats");
        if order.trim().is_empty() && formats.trim().is_empty() {
            return (rows, origins);
        }
        let column = |name: &str| columns.iter().position(|c| c.eq_ignore_ascii_case(name.trim()));
        let mut cells: Vec<Vec<String>> = rows.iter().map(|r| r.split('\t').map(str::to_owned).collect()).collect();
        let mut origins = origins;
        let keep_origins = origins.len() == cells.len();

        let order = order.trim();
        if let Some(at) = column(order.trim_start_matches('-')) {
            let descending = order.starts_with('-');
            let mut idx: Vec<usize> = (0..cells.len()).collect();
            let key = |i: usize| cells[i].get(at).cloned().unwrap_or_default();
            let as_value = |s: String| CobolValue::from_str(s.trim(), s.trim().len());
            // Stable: equal keys keep the source's order.
            idx.sort_by(|&a, &b| {
                let (ka, kb) = (as_value(key(a)), as_value(key(b)));
                let (a_after_b, b_after_a) = (value_gt(&ka, &kb), value_gt(&kb, &ka));
                let asc = if a_after_b {
                    std::cmp::Ordering::Greater
                } else if b_after_a {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                };
                if descending {
                    asc.reverse()
                } else {
                    asc
                }
            });
            cells = idx.iter().map(|&i| cells[i].clone()).collect();
            if keep_origins {
                origins = idx.iter().map(|&i| origins[i].clone()).collect();
            }
        }

        let locale = self.env.format_default_locale().to_owned();
        let currency = self.env.format_default_currency().map(str::to_owned);
        for line in formats.lines() {
            let Some((field, spec)) = line.split_once('=') else { continue };
            let Some(at) = column(field) else { continue };
            let mut words = spec.split_whitespace();
            let name = words.next().unwrap_or("").to_owned();
            let code = words.next().map(str::to_owned).or_else(|| currency.clone());
            for row in &mut cells {
                let Some(cell) = row.get_mut(at) else { continue };
                if let Some(n) = crate::value::parse_decimal(cell.trim()) {
                    if let Ok(text) =
                        cobolt_format::format_decimal(n.mantissa, n.decimals, &name, &locale, code.as_deref())
                    {
                        *cell = text;
                    }
                }
            }
        }
        (cells.into_iter().map(|r| r.join("\t")).collect(), origins)
    }
}
