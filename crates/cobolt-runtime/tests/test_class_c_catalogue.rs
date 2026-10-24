// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The class C catalogue (`cobolt_wire::controls`) against what exists
//! (WASM-005 T7, R12): every class C control of the web classification has a
//! row, and every row names an operation the desktop already has — a method of
//! the interpreter's member dispatch, a route of the AWS table, or a paragraph
//! of the IndexedFile facade. Nothing in the catalogue is invented.
//!
//! The check reads the sources the dispatch lives in, so a method renamed or
//! removed there turns this test red.

use cobolt_forms::web_class::{Class, TABLE};
use cobolt_wire::controls::{rows, of_control};

const INTERPRETER: &str = include_str!("../src/interpreter.rs");
const KB: &str = include_str!("../src/interpreter/kb.rs");
const AWS_ROUTES: &str = include_str!("../src/aws/routes.toml");
const CODEGEN: &str = include_str!("../../cobolt-codegen/src/lib.rs");

/// The body of the interpreter's `exec_method`, where member calls dispatch.
fn exec_method_body() -> &'static str {
    let start = INTERPRETER.find("fn exec_method(").expect("exec_method");
    let rest = &INTERPRETER[start..];
    let end = rest.find("\n    pub fn eval_expr(").expect("the function after exec_method");
    &rest[..end]
}

#[test]
fn every_class_c_control_has_a_row() {
    let class_c: Vec<&str> = TABLE.iter().filter(|r| r.class == Class::C).map(|r| r.control).collect();
    assert_eq!(class_c.len(), 24, "8 service controls + DataGrid + FileDropZone, and the 16 AWS controls");
    let mut missing = Vec::new();
    for control in &class_c {
        if of_control(control).next().is_none() {
            missing.push(*control);
        }
    }
    assert!(missing.is_empty(), "class C controls with no catalogue row: {missing:?}");
    // Maps is class B with class C operations, the Viewer class B with an upload
    // slot: they have rows too.
    assert!(of_control("Maps").next().is_some());
    assert!(of_control("Viewer").next().is_some());
}

#[test]
fn every_row_names_an_operation_the_runtime_has() {
    let dispatch = exec_method_body();
    let mut checked = 0;
    for op in rows() {
        let found = if op.control.starts_with("Aws") {
            // `[ops."AwsS3.GetObject"]` — the route the AWS dispatch looks up.
            let key = format!("[ops.\"{}.{}\"]", op.control, op.method);
            AWS_ROUTES.contains(&key)
        } else if op.control == "Viewer" && op.operation == "OPEN" {
            // The upload slot of a document the user opens: on the desktop, the
            // Viewer's `Source`, which the property layer resolves onto `View1Source`.
            op.method == "Source" && INTERPRETER.contains("prop.eq_ignore_ascii_case(\"Source\")")
        } else if op.control == "IndexedFile" {
            // The facade paragraph `<id>-<OP>.` codegen writes.
            CODEGEN.contains(&format!("{{id}}-{}.\\n", op.operation))
                || CODEGEN.contains(&format!("(\"{}\", ", op.operation))
        } else {
            let arm = format!("\"{}\"", op.method.to_ascii_uppercase());
            let in_dispatch = dispatch.contains(&arm) || (op.control == "KnowledgeBase" && KB.contains(&arm));
            in_dispatch && cobolt_ast::methods::is_known_method(op.method)
        };
        assert!(found, "{} {} (method {}) is not an operation the runtime has", op.control, op.operation, op.method);
        checked += 1;
    }
    println!("{checked} catalogue rows checked against the runtime");
    assert!(checked > 80);
}
