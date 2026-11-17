// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The rows an AWS operation leaves on its control, as `GetRow` and `GetField` read them: in every
//! build (one without AWS answers them from an empty set) and for every host (DESKTOP-WASM-001 T48).

use serde_json::Value;

/// `GetRow( i )` and `GetField( i field )`: the last operation's row `i` (1-based) as JSON, or a
/// field of it, matched first exactly and then without regard to case. Empty when there is no such
/// row or field.
pub fn row_value(rows: Option<&Vec<Value>>, i: &str, field: Option<&str>) -> String {
    let Some(row) = i.trim().parse::<usize>().ok().filter(|n| *n >= 1).and_then(|n| rows?.get(n - 1)) else {
        return String::new();
    };
    let Some(field) = field else {
        return row.to_string();
    };
    let field = field.trim();
    let value = row.get(field).or_else(|| row.as_object()?.iter().find(|(k, _)| k.eq_ignore_ascii_case(field)).map(|(_, v)| v));
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(v) => v.to_string(),
    }
}
