// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tests for IDENTIFICATION DIVISION parsing.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;

fn src(s: &str) -> String {
    format!(
        "{}\nPROCEDURE DIVISION.\nMAIN.\n    STOP RUN.\n",
        s
    )
}

#[test]
fn minimal_identification() {
    let code = src("IDENTIFICATION DIVISION.\nPROGRAM-ID. HELLO.");
    let result = parse(tokenize(&code, SourceFormat::Free));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let prog = result.program.unwrap();
    assert_eq!(prog.identification.program_id, "HELLO");
    assert!(prog.identification.author.is_none());
}

#[test]
fn identification_with_author() {
    let code = src(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. MYAPP.\nAUTHOR. EMERSON.",
    );
    let result = parse(tokenize(&code, SourceFormat::Free));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let id = result.program.unwrap().identification;
    assert_eq!(id.program_id, "MYAPP");
    assert!(id.author.is_some());
}

#[test]
fn identification_with_date_written() {
    let code = src(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. DATEPROG.\nDATE-WRITTEN. 2024-01-15.",
    );
    let result = parse(tokenize(&code, SourceFormat::Free));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let id = result.program.unwrap().identification;
    assert_eq!(id.program_id, "DATEPROG");
    assert!(id.date_written.is_some());
}

#[test]
fn identification_program_id_hyphenated() {
    let code = src("IDENTIFICATION DIVISION.\nPROGRAM-ID. MY-PROGRAM.");
    let result = parse(tokenize(&code, SourceFormat::Free));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result.program.unwrap().identification.program_id,
        "MY-PROGRAM"
    );
}
