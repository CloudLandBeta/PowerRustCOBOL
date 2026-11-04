// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-018 T15: `Ctrl::Name(args)` is a subscript unless `Name` is a known method, so every
//! method of the seven graphics objects must be in `is_known_method`.

use cobolt_ast::methods::is_known_method;
use cobolt_graphics::{ObjectKind, Registry};

#[test]
fn every_graphics_method_is_in_the_closed_vocabulary() {
    let mut checked = 0usize;
    let mut missing: Vec<String> = Vec::new();
    for kind in ObjectKind::available() {
        for m in Registry::methods(kind) {
            checked += 1;
            if !is_known_method(&m.name) {
                missing.push(format!("{:?}::{}", kind, m.name));
            }
        }
    }
    println!("graphics methods checked: {checked} on {} object kinds, {} missing", ObjectKind::available().len(), missing.len());
    assert!(missing.is_empty(), "not in is_known_method (they would parse as subscripts): {missing:?}");
    assert!(checked >= 300, "the registry has {checked} methods; the plan counted 313");
}
