// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Integration tests for cobolt-semantic.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze, Severity};

fn parse_program(src: &str) -> cobolt_ast::program::Program {
    let result = parse(tokenize(src, SourceFormat::Free));
    result.program.expect("program should parse")
}

// ── Symbol table ──────────────────────────────────────────────────────────────

#[test]
fn symbol_table_indexes_working_storage() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. SYMTEST.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-COUNT    PIC 9(4) VALUE 0.
01 WS-NAME     PIC X(30).
01 WS-RATE     PIC 9(5)V99 COMP-3.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
    let prog = parse_program(src);
    let result = analyze(&prog);
    let syms = &result.symbols;
    assert_eq!(syms.data_item_count(), 3);
    assert!(syms.has_data_item("WS-COUNT"));
    assert!(syms.has_data_item("WS-NAME"));
    assert!(syms.has_data_item("WS-RATE"));
    assert!(!syms.has_data_item("WS-MISSING"));
}

#[test]
fn symbol_table_rust_name_conversion() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. RUSTNAME.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-MY-LONG-FIELD PIC X(10).
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
    let prog = parse_program(src);
    let result = analyze(&prog);
    let info = result.symbols.data_item("WS-MY-LONG-FIELD").unwrap();
    assert_eq!(info.rust_name, "ws_my_long_field");
}

#[test]
fn symbol_table_indexes_paragraphs() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. PARATEST.
PROCEDURE DIVISION.
MAIN.
    PERFORM COMPUTE-TOTAL.
    STOP RUN.
COMPUTE-TOTAL.
    CONTINUE.
";
    let prog = parse_program(src);
    let result = analyze(&prog);
    assert!(result.symbols.has_procedure("MAIN"));
    assert!(result.symbols.has_procedure("COMPUTE-TOTAL"));
}

// ── Name resolution ───────────────────────────────────────────────────────────

#[test]
fn resolver_no_warnings_for_declared_items() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. RESTEST.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-A PIC 9(4).
01 WS-B PIC 9(4).
PROCEDURE DIVISION.
MAIN.
    MOVE WS-A TO WS-B.
    STOP RUN.
";
    let prog = parse_program(src);
    let result = analyze(&prog);
    // No warnings — both identifiers are declared
    let warnings: Vec<_> = result.warnings().collect();
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
}

#[test]
fn resolver_warns_undeclared_identifier() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. UNDECL.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-A PIC 9(4).
PROCEDURE DIVISION.
MAIN.
    MOVE WS-A TO WS-MISSING.
    STOP RUN.
";
    let prog = parse_program(src);
    let result = analyze(&prog);
    let warnings: Vec<_> = result.warnings().collect();
    assert!(
        warnings.iter().any(|w| w.message.contains("WS-MISSING")),
        "expected warning about WS-MISSING, got: {warnings:?}"
    );
}

// ── Type checking ─────────────────────────────────────────────────────────────

#[test]
fn type_checker_no_error_numeric_compute() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. TYPETEST.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-X PIC 9(4).
01 WS-Y PIC 9(4).
PROCEDURE DIVISION.
MAIN.
    COMPUTE WS-X = WS-Y + 1.
    STOP RUN.
";
    let prog = parse_program(src);
    let result = analyze(&prog);
    let errors: Vec<_> = result.errors().collect();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
}

#[test]
fn type_checker_error_alphanumeric_compute_target() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. TYPETEST2.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-NAME PIC X(30).
01 WS-NUM  PIC 9(4).
PROCEDURE DIVISION.
MAIN.
    COMPUTE WS-NAME = WS-NUM + 1.
    STOP RUN.
";
    let prog = parse_program(src);
    let result = analyze(&prog);
    let errors: Vec<_> = result.errors().collect();
    assert!(
        errors
            .iter()
            .any(|e| e.message.contains("WS-NAME") && e.message.contains("numeric")),
        "expected type error for WS-NAME in COMPUTE, got: {errors:?}"
    );
}

// ── EXEC RUST binding resolution ──────────────────────────────────────────────

#[test]
fn exec_rust_bindings_resolved() {
    // Spec 041 R5: only `USAGE OBJECT REFERENCE RUST-<type>` items may cross
    // into a block, so the fixture binds Rust-typed objects. (It used to bind
    // PIC items, which the compiler now rejects — the test stayed green because
    // it only asserts the Info binding list, so it described a program that
    // would no longer build.) The subject under test is unchanged: which names
    // the binding pass resolves out of a block's source.
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. EXECTEST.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
REPOSITORY.
    CLASS RUST-STRING IS \"Rust.String\"
    CLASS RUST-I64 IS \"Rust.i64\"
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-COUNT  USAGE IS OBJECT REFERENCE RUST-I64.
01 WS-RESULT USAGE IS OBJECT REFERENCE RUST-I64.
01 WS-NAME   USAGE IS OBJECT REFERENCE RUST-STRING.
PROCEDURE DIVISION.
MAIN.
    EXEC RUST
        ws_count += 1;
        ws_result = ws_count * 2;
    END-EXEC.
    STOP RUN.
";
    let prog = parse_program(src);
    let result = analyze(&prog);

    // The fixture must be a program that actually builds: no R5 rejection.
    // Without this the test can pass while describing rejected code, which is
    // exactly what it did before the conversion.
    let errors: Vec<_> = result.errors().collect();
    assert!(
        !errors
            .iter()
            .any(|d| d.message.contains("not a USAGE OBJECT REFERENCE")),
        "the fixture binds an item R5 rejects: {errors:?}"
    );

    // The exec_rust pass should emit Info diagnostics listing the bindings
    let info_diags: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Info)
        .collect();

    assert!(
        !info_diags.is_empty(),
        "expected Info diagnostics from EXEC RUST binding pass"
    );
    let combined = info_diags
        .iter()
        .map(|d| d.message.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        combined.contains("ws_count"),
        "expected ws_count in bindings: {combined}"
    );
    assert!(
        combined.contains("ws_result"),
        "expected ws_result in bindings: {combined}"
    );
    // WS-NAME is not referenced in the Rust block — should not appear
    assert!(
        !combined.contains("ws_name"),
        "ws_name should not be in bindings: {combined}"
    );
}

#[test]
fn exec_rust_no_spurious_partial_matches() {
    // ws_count_extra should not match ws_count binding. Bound as an object
    // item per R5 — see the note on `exec_rust_bindings_resolved`.
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. PARTMATCH.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
REPOSITORY.
    CLASS RUST-I64 IS \"Rust.i64\"
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-COUNT USAGE IS OBJECT REFERENCE RUST-I64.
PROCEDURE DIVISION.
MAIN.
    EXEC RUST
        let ws_count_extra = 99;
    END-EXEC.
    STOP RUN.
";
    let prog = parse_program(src);
    let result = analyze(&prog);

    let info_diags: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Info)
        .collect();

    // ws_count should NOT match ws_count_extra
    assert!(
        info_diags.is_empty() || !info_diags.iter().any(|d| d.message.contains("ws_count")),
        "ws_count should not match ws_count_extra: {info_diags:?}"
    );
}

// ── Full program — no false positives ─────────────────────────────────────────

#[test]
fn clean_program_has_no_errors() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. CLEAN.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-COUNTER PIC 9(4) VALUE 0.
01 WS-LIMIT   PIC 9(4) VALUE 10.
PROCEDURE DIVISION.
MAIN.
    PERFORM LOOP-BODY UNTIL WS-COUNTER >= WS-LIMIT.
    STOP RUN.
LOOP-BODY.
    ADD 1 TO WS-COUNTER.
";
    let prog = parse_program(src);
    let result = analyze(&prog);
    let errors: Vec<_> = result.errors().collect();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
}

// ── GLOBAL placement (spec 009 R6) ──────────────────────────────────────────

#[test]
fn global_on_01_and_77_is_clean_009() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. GLOBOK.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-SHARED   PIC 9(4) GLOBAL.
77 WS-FLAG     PIC 9    GLOBAL.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
    let prog = parse_program(src);
    let result = analyze(&prog);
    let errors: Vec<_> = result.errors().collect();
    assert!(
        errors.is_empty(),
        "GLOBAL on 01/77 must be clean: {errors:?}"
    );
}

#[test]
fn global_on_subordinate_item_is_error_009() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. GLOBBAD.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-GROUP.
   05 WS-CHILD PIC 9(4) GLOBAL.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
    let prog = parse_program(src);
    let result = analyze(&prog);
    let errors: Vec<_> = result.errors().collect();
    assert!(
        errors
            .iter()
            .any(|e| e.message.contains("GLOBAL is valid only")),
        "GLOBAL on a level-05 item must be flagged: {errors:?}"
    );
}

#[test]
fn fd_is_global_parses_and_is_clean_009() {
    // `FD … IS GLOBAL` (spec 009 R6) parses and raises no placement error.
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. FDGLOB.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT F ASSIGN TO \"f.dat\"
        ORGANIZATION IS LINE SEQUENTIAL.
DATA DIVISION.
FILE SECTION.
FD F IS GLOBAL.
01 F-REC PIC X(80).
WORKING-STORAGE SECTION.
01 WS-EOF PIC 9 VALUE 0.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
";
    let prog = parse_program(src);
    let result = analyze(&prog);
    let errors: Vec<_> = result.errors().collect();
    assert!(
        errors.is_empty(),
        "FD IS GLOBAL must parse clean: {errors:?}"
    );
}

// ── EXEC RUST: what may cross into a block (spec 041 R5, R6) ─────────────────

/// **AC6 / R5** — a `PIC` item has no Rust type to bind to. Its value is a
/// scaled decimal or a fixed-width space-padded field, so handing one to
/// compiled Rust would mean re-implementing COBOL's numeric and padding rules
/// in generated code. It must be rejected, by name.
#[test]
fn exec_rust_rejects_a_pic_item() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. PICBIND.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-COUNT PIC 9(4) VALUE 0.
PROCEDURE DIVISION.
MAIN.
    EXEC RUST
        ws_count += 1;
    END-EXEC.
    STOP RUN.
";
    let result = analyze(&parse_program(src));
    let errors: Vec<_> = result.errors().collect();
    assert!(
        errors
            .iter()
            .any(|d| d.message.contains("WS-COUNT")
                && d.message.contains("not a USAGE OBJECT REFERENCE")),
        "expected WS-COUNT to be rejected by name, got: {errors:?}"
    );
}

/// R5 — the same reference against a `USAGE OBJECT REFERENCE` item is fine.
#[test]
fn exec_rust_accepts_an_object_reference_item() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. OBJBIND.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
REPOSITORY.
    CLASS RUST-STRING IS \"Rust.String\"
DATA DIVISION.
WORKING-STORAGE SECTION.
01 USER-NAME USAGE IS OBJECT REFERENCE RUST-STRING VALUE \"ada\".
PROCEDURE DIVISION.
MAIN.
    EXEC RUST
        user_name.push_str(\" lovelace\");
    END-EXEC.
    STOP RUN.
";
    let result = analyze(&parse_program(src));
    let errors: Vec<_> = result.errors().collect();
    assert!(
        !errors.iter().any(|d| d.message.contains("user_name")
            || d.message.contains("USER-NAME")),
        "a Rust-typed object must bind cleanly, got: {errors:?}"
    );
}

/// **AC7 / R6** — the bound name becomes a Rust identifier verbatim, so a COBOL
/// name that lands on a Rust keyword cannot be used. `TYPE` is a name COBOL
/// programs pick quite naturally.
#[test]
fn exec_rust_rejects_a_name_that_is_a_rust_keyword() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. KEYWORDBIND.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
REPOSITORY.
    CLASS RUST-STRING IS \"Rust.String\"
DATA DIVISION.
WORKING-STORAGE SECTION.
01 TYPE USAGE IS OBJECT REFERENCE RUST-STRING VALUE \"x\".
PROCEDURE DIVISION.
MAIN.
    EXEC RUST
        type.push_str(\"y\");
    END-EXEC.
    STOP RUN.
";
    let result = analyze(&parse_program(src));
    let errors: Vec<_> = result.errors().collect();
    assert!(
        errors
            .iter()
            .any(|d| d.message.contains("Rust keyword") && d.message.contains("TYPE")),
        "expected a keyword-collision error naming TYPE, got: {errors:?}"
    );
}
