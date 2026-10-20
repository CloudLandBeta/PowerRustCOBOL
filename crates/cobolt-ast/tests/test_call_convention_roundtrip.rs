// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The calling convention of a `CALL` survives the two serialisations the AST
//! goes through: `serde_json` (the IDE's snapshots) and `bincode` (embedded in
//! every compiled binary). bincode names an enum variant, and a struct field's
//! place, by ordinal — so the new field is the LAST of `Stmt::Call` and the new
//! variant the LAST of `CallConvention`, which keeps every older ordinal where
//! it was (WASM-005 R1).

use cobolt_ast::expr::{Expr, Literal};
use cobolt_ast::stmt::{CallArg, CallConvention, Stmt};
use cobolt_ast::Span;

fn span() -> Span {
    Span::new(0, 0, 1, 1)
}

fn call(convention: CallConvention) -> Stmt {
    Stmt::Call {
        program: Expr::Literal(Literal::String("GET-CUSTOMER".into()), span()),
        using: vec![
            CallArg::ByValue(Expr::Identifier("CUST".into(), span())),
            CallArg::ByReference(Expr::Identifier("RES".into(), span())),
        ],
        returning: Some(Expr::Identifier("RES".into(), span())),
        on_exception: vec![],
        not_on_exception: vec![],
        span: span(),
        convention,
    }
}

#[test]
fn both_conventions_round_trip_through_bincode_and_json() {
    for convention in [CallConvention::Local, CallConvention::WebSocket] {
        let stmt = call(convention);
        let bytes = bincode::serialize(&stmt).expect("bincode");
        let back: Stmt = bincode::deserialize(&bytes).expect("bincode back");
        assert_eq!(back, stmt, "{convention:?} through bincode");
        let json = serde_json::to_string(&stmt).expect("json");
        let back: Stmt = serde_json::from_str(&json).expect("json back");
        assert_eq!(back, stmt, "{convention:?} through JSON");
    }
}

#[test]
fn the_convention_is_the_last_thing_in_the_encoding_and_local_is_zero() {
    // bincode 1.x writes an enum variant as a little-endian u32 and the fields of a
    // struct variant in declaration order: the final four bytes are therefore the
    // convention's ordinal. `Local` = 0 means a statement that never knew the field
    // keeps the ordinal it always implied; `WebSocket` = 1 is appended.
    let local = bincode::serialize(&call(CallConvention::Local)).unwrap();
    let ws = bincode::serialize(&call(CallConvention::WebSocket)).unwrap();
    assert_eq!(local.len(), ws.len());
    assert_eq!(&local[local.len() - 4..], &0u32.to_le_bytes());
    assert_eq!(&ws[ws.len() - 4..], &1u32.to_le_bytes());
    assert_eq!(local[..local.len() - 4], ws[..ws.len() - 4], "nothing before the convention moved");
}

#[test]
fn a_call_that_never_named_a_convention_reads_as_local() {
    assert_eq!(CallConvention::default(), CallConvention::Local);
}
