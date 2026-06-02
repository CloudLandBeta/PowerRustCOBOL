// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tests for `EXEC RUST … END-EXEC` statement parsing.
//!
//! These tests verify that:
//!   1. The lexer correctly captures the Rust source verbatim.
//!   2. The parser produces `Stmt::ExecRust` with the correct source string.
//!   3. Multi-line Rust code, COBOL data references, and PowerCOBOL object
//!      calls all survive the lexer/parser round-trip intact.
//!   4. Multiple EXEC RUST blocks in one program work correctly.
//!   5. EXEC RUST may appear without a trailing period.

use cobolt_ast::stmt::Stmt;
use cobolt_ast::program::ProcedureBody;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn prog(stmts_src: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. TESTPROG.\n\
         PROCEDURE DIVISION.\nMAIN.\n{}\n",
        stmts_src
    )
}

fn parse_stmts(code: &str) -> Vec<Stmt> {
    let result = parse(tokenize(code, SourceFormat::Free));
    assert!(
        result.diagnostics.is_empty(),
        "Unexpected diagnostics: {:?}",
        result.diagnostics
    );
    let proc = result.program.unwrap().procedure;
    match proc.body {
        ProcedureBody::Paragraphs(mut paras) => {
            paras.pop().map(|p| p.stmts).unwrap_or_default()
        }
        ProcedureBody::Sections(secs) => secs
            .into_iter()
            .flat_map(|s| s.paragraphs)
            .flat_map(|p| p.stmts)
            .collect(),
    }
}

// ── Basic single-line EXEC RUST ───────────────────────────────────────────────

#[test]
fn exec_rust_basic() {
    let code = prog(
        "    EXEC RUST\n        let _ = 1 + 1;\n    END-EXEC.\n    STOP RUN.\n",
    );
    let stmts = parse_stmts(&code);
    assert!(!stmts.is_empty(), "expected at least one statement");
    if let Stmt::ExecRust { source, .. } = &stmts[0] {
        assert!(source.contains("let _ = 1 + 1;"), "source: {source:?}");
    } else {
        panic!("expected Stmt::ExecRust, got {:?}", stmts[0]);
    }
}

// ── Multi-line Rust with arithmetic ──────────────────────────────────────────

#[test]
fn exec_rust_multiline() {
    let src = "    EXEC RUST\n        let a = 10;\n        let b = 20;\n        let c = a + b;\n        assert_eq!(c, 30);\n    END-EXEC.\n    STOP RUN.\n";
    let stmts = parse_stmts(&prog(src));
    if let Stmt::ExecRust { source, .. } = &stmts[0] {
        assert!(source.contains("let a = 10;"), "source: {source:?}");
        assert!(source.contains("let b = 20;"), "source: {source:?}");
        assert!(source.contains("assert_eq!(c, 30);"), "source: {source:?}");
    } else {
        panic!("expected Stmt::ExecRust");
    }
}

// ── COBOL data item references (snake_case convention) ────────────────────────

#[test]
fn exec_rust_cobol_data_references() {
    // COBOL items WS-COUNT, WS-TOTAL, WS-FLAG are referenced by their
    // snake_case equivalents (ws_count, ws_total, ws_flag) in the Rust block.
    let rust_body = r#"
    EXEC RUST
        *ws_count += 1;
        if *ws_flag == b'Y' {
            *ws_total += *ws_count;
        }
    END-EXEC.
    STOP RUN.
"#;
    let stmts = parse_stmts(&prog(rust_body));
    if let Stmt::ExecRust { source, referenced_data, .. } = &stmts[0] {
        assert!(source.contains("ws_count"), "source: {source:?}");
        assert!(source.contains("ws_total"), "source: {source:?}");
        assert!(source.contains("ws_flag"), "source: {source:?}");
        // referenced_data is empty at parse time; filled by semantic pass
        assert!(
            referenced_data.is_empty(),
            "expected empty at parse time, got {referenced_data:?}"
        );
    } else {
        panic!("expected Stmt::ExecRust");
    }
}

// ── PowerCOBOL object / property access ──────────────────────────────────────

#[test]
fn exec_rust_object_access() {
    let rust_body = r#"
    EXEC RUST
        if let Some(form) = cobolt_objects.get("FORM1") {
            form.set_property("Caption", "Hello from Rust!");
            form.set_property("Visible", true);
        }
        cobol_env.set("WS-STATUS", 0);
    END-EXEC.
    STOP RUN.
"#;
    let stmts = parse_stmts(&prog(rust_body));
    if let Stmt::ExecRust { source, .. } = &stmts[0] {
        assert!(source.contains("cobolt_objects"), "source: {source:?}");
        assert!(source.contains("cobol_env"), "source: {source:?}");
        assert!(source.contains("FORM1"), "source: {source:?}");
    } else {
        panic!("expected Stmt::ExecRust");
    }
}

// ── Multiple EXEC RUST blocks in one program ──────────────────────────────────

#[test]
fn exec_rust_multiple_blocks() {
    let src = "\
    EXEC RUST\n        let x = 1;\n    END-EXEC.\n\
    EXEC RUST\n        let y = 2;\n    END-EXEC.\n\
    STOP RUN.\n";
    let stmts = parse_stmts(&prog(src));

    let exec_blocks: Vec<_> = stmts
        .iter()
        .filter(|s| matches!(s, Stmt::ExecRust { .. }))
        .collect();
    assert_eq!(exec_blocks.len(), 2, "expected 2 ExecRust blocks");

    if let Stmt::ExecRust { source, .. } = &exec_blocks[0] {
        assert!(source.contains("let x = 1;"), "block 0: {source:?}");
    }
    if let Stmt::ExecRust { source, .. } = &exec_blocks[1] {
        assert!(source.contains("let y = 2;"), "block 1: {source:?}");
    }
}

// ── EXEC RUST without trailing period ────────────────────────────────────────

#[test]
fn exec_rust_no_trailing_period() {
    // END-EXEC without a following period — the next statement starts immediately.
    let src = "    EXEC RUST\n        let z = 99;\n    END-EXEC\n    STOP RUN.\n";
    let stmts = parse_stmts(&prog(src));
    assert!(
        stmts.iter().any(|s| matches!(s, Stmt::ExecRust { .. })),
        "expected ExecRust statement, got: {stmts:?}"
    );
}

// ── Rust code with Rust keywords that overlap COBOL keywords ─────────────────

#[test]
fn exec_rust_rust_keywords_dont_confuse_parser() {
    // "if", "let", "return", "use" are not COBOL keywords; "move" IS a COBOL
    // keyword but inside the Rust block it should be captured verbatim.
    let rust_body = "    EXEC RUST\n        let v = vec![1, 2, 3];\n        let doubled: Vec<_> = v.into_iter().map(|x| x * 2).collect();\n        let _ = move || doubled.len();\n    END-EXEC.\n    STOP RUN.\n";
    let stmts = parse_stmts(&prog(rust_body));
    if let Stmt::ExecRust { source, .. } = &stmts[0] {
        assert!(source.contains("into_iter"), "source: {source:?}");
        assert!(source.contains("move ||"), "source: {source:?}");
    } else {
        panic!("expected Stmt::ExecRust");
    }
}

// ── Full integration: EXEC RUST in a complete program with DATA DIVISION ──────

#[test]
fn exec_rust_full_program() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. RUSTDEMO.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-COUNT    PIC 9(4) VALUE 0.
01 WS-RESULT   PIC 9(8) VALUE 0.
PROCEDURE DIVISION.
MAIN.
    EXEC RUST
        *ws_count = 42;
        *ws_result = (*ws_count as i64) * (*ws_count as i64);
        // ws_result now holds 1764
    END-EXEC.
    STOP RUN.
";
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.is_empty(),
        "diagnostics: {:?}",
        result.diagnostics
    );
    let prog = result.program.unwrap();
    assert_eq!(prog.identification.program_id, "RUSTDEMO");
    // Confirm DATA DIVISION was parsed
    assert!(prog.data.is_some(), "expected DATA DIVISION");
    // Confirm EXEC RUST reached the procedure
    let proc = prog.procedure;
    let all_stmts: Vec<_> = match proc.body {
        ProcedureBody::Paragraphs(paras) => paras.into_iter().flat_map(|p| p.stmts).collect(),
        ProcedureBody::Sections(secs) => secs
            .into_iter()
            .flat_map(|s| s.paragraphs)
            .flat_map(|p| p.stmts)
            .collect(),
    };
    assert!(
        all_stmts.iter().any(|s| matches!(s, Stmt::ExecRust { .. })),
        "expected ExecRust in procedure, got: {all_stmts:?}"
    );
}
