// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `validate` — does one `.cfrm` or `.cidx` load, and is it well formed.

use serde_json::{json, Value};

use crate::root::ProjectRoot;
use crate::tools::check::indexed_findings;
use crate::validate_source::Severity;

pub fn run(root: &ProjectRoot, path: &str) -> Result<Value, String> {
    let abs = root.resolve(path)?;
    if !abs.is_file() {
        return Err(format!("'{path}' does not exist"));
    }
    let ext = abs
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "cfrm" => Ok(match cobolt_forms::load_form(&abs) {
            Ok(form) => json!({
                "path": path, "valid": true, "form": form.name,
                "controls": form.controls.len(), "main_form": form.main_form,
                "errors": [], "warnings": [],
            }),
            Err(e) => json!({
                "path": path, "valid": false,
                "errors": [format!("the form cannot be loaded: {e}")], "warnings": [],
            }),
        }),
        "cidx" => {
            let found = indexed_findings(&abs);
            let pick = |s: Severity| {
                found
                    .iter()
                    .filter(|(sev, _)| *sev == s)
                    .map(|(_, m)| m.clone())
                    .collect::<Vec<_>>()
            };
            let errors = pick(Severity::Error);
            Ok(json!({
                "path": path, "valid": errors.is_empty(),
                "errors": errors, "warnings": pick(Severity::Warning),
            }))
        }
        _ => Err(format!("'{path}' is not a .cfrm or a .cidx")),
    }
}
