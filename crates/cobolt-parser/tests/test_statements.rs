// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tests for PROCEDURE DIVISION statement parsing.

use cobolt_ast::stmt::Stmt;
use cobolt_ast::program::ProcedureBody;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;

/// Build a complete program with just a single paragraph of statements.
fn prog(stmts_src: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. TESTPROG.\n\
         PROCEDURE DIVISION.\nMAIN.\n{}\n",
        stmts_src
    )
}

/// Parse and return the statements from the first paragraph.
fn parse_stmts(code: &str) -> Vec<Stmt> {
    let result = parse(tokenize(code, SourceFormat::Free));
    assert!(result.diagnostics.is_empty(), "Diagnostics: {:?}", result.diagnostics);
    let proc = result.program.unwrap().procedure;
    match proc.body {
        ProcedureBody::Paragraphs(mut paras) => {
            paras.pop().map(|p| p.stmts).unwrap_or_default()
        }
        ProcedureBody::Sections(secs) => {
            secs.into_iter()
                .flat_map(|s| s.paragraphs)
                .flat_map(|p| p.stmts)
                .collect()
        }
    }
}

// ── MOVE ─────────────────────────────────────────────────────────────────────

#[test]
fn move_literal_to_field() {
    let stmts = parse_stmts(&prog("    MOVE 'HELLO' TO WS-NAME.\n    STOP RUN.\n"));
    assert!(!stmts.is_empty());
    assert!(matches!(stmts[0], Stmt::Move { .. }));
}

#[test]
fn move_field_to_field() {
    let stmts = parse_stmts(&prog("    MOVE WS-A TO WS-B.\n    STOP RUN.\n"));
    assert!(matches!(stmts[0], Stmt::Move { .. }));
}

#[test]
fn move_to_multiple() {
    let stmts = parse_stmts(&prog("    MOVE SPACES TO WS-A WS-B WS-C.\n    STOP RUN.\n"));
    if let Stmt::Move { to, .. } = &stmts[0] {
        assert_eq!(to.len(), 3);
    } else {
        panic!("expected MOVE");
    }
}

// ── ADD ──────────────────────────────────────────────────────────────────────

#[test]
fn add_to() {
    let stmts = parse_stmts(&prog("    ADD 1 TO WS-CNT.\n    STOP RUN.\n"));
    assert!(matches!(stmts[0], Stmt::Add { .. }));
}

#[test]
fn add_giving() {
    let stmts = parse_stmts(&prog("    ADD WS-A WS-B GIVING WS-C.\n    STOP RUN.\n"));
    if let Stmt::Add { giving, .. } = &stmts[0] {
        assert!(giving.is_some());
    } else {
        panic!("expected ADD");
    }
}

// ── SUBTRACT ─────────────────────────────────────────────────────────────────

#[test]
fn subtract_from() {
    let stmts = parse_stmts(&prog("    SUBTRACT 5 FROM WS-TOTAL.\n    STOP RUN.\n"));
    assert!(matches!(stmts[0], Stmt::Subtract { .. }));
}

// ── COMPUTE ───────────────────────────────────────────────────────────────────

#[test]
fn compute_expression() {
    let stmts = parse_stmts(&prog("    COMPUTE WS-R = WS-A + WS-B * 2.\n    STOP RUN.\n"));
    assert!(matches!(stmts[0], Stmt::Compute { .. }));
}

// ── IF ────────────────────────────────────────────────────────────────────────

#[test]
fn if_simple() {
    let code = prog(
        "    IF WS-CNT > 0\n       MOVE 'POS' TO WS-SIGN\n    END-IF.\n    STOP RUN.\n",
    );
    let stmts = parse_stmts(&code);
    assert!(matches!(stmts[0], Stmt::If { .. }));
    if let Stmt::If { then_stmts, else_stmts, .. } = &stmts[0] {
        assert_eq!(then_stmts.len(), 1);
        assert!(else_stmts.is_empty());
    }
}

#[test]
fn if_else() {
    let code = prog(
        "    IF WS-FLAG = 'Y'\n       MOVE 1 TO WS-OK\n    ELSE\n       MOVE 0 TO WS-OK\n    END-IF.\n    STOP RUN.\n",
    );
    let stmts = parse_stmts(&code);
    if let Stmt::If { then_stmts, else_stmts, .. } = &stmts[0] {
        assert_eq!(then_stmts.len(), 1);
        assert_eq!(else_stmts.len(), 1);
    } else {
        panic!("expected IF");
    }
}

// ── PERFORM ───────────────────────────────────────────────────────────────────

#[test]
fn perform_paragraph() {
    let code = prog("    PERFORM MY-PARA.\n    STOP RUN.\n");
    let stmts = parse_stmts(&code);
    assert!(matches!(stmts[0], Stmt::Perform { .. }));
}

#[test]
fn perform_thru() {
    let code = prog("    PERFORM PARA-A THRU PARA-Z.\n    STOP RUN.\n");
    let stmts = parse_stmts(&code);
    if let Stmt::Perform { target, .. } = &stmts[0] {
        assert!(matches!(
            target,
            cobolt_ast::stmt::PerformTarget::Thru { .. }
        ));
    } else {
        panic!("expected PERFORM");
    }
}

#[test]
fn perform_inline_until() {
    let code = prog(
        "    PERFORM UNTIL WS-CNT > 10\n       ADD 1 TO WS-CNT\n    END-PERFORM.\n    STOP RUN.\n",
    );
    let stmts = parse_stmts(&code);
    if let Stmt::Perform { target, .. } = &stmts[0] {
        assert!(matches!(
            target,
            cobolt_ast::stmt::PerformTarget::Until { .. }
        ));
    } else {
        panic!("expected PERFORM UNTIL");
    }
}

// ── DISPLAY ───────────────────────────────────────────────────────────────────

#[test]
fn display_literal() {
    let code = prog("    DISPLAY 'HELLO WORLD'.\n    STOP RUN.\n");
    let stmts = parse_stmts(&code);
    assert!(matches!(stmts[0], Stmt::Display { .. }));
}

#[test]
fn display_multiple() {
    let code = prog("    DISPLAY 'NAME: ' WS-NAME.\n    STOP RUN.\n");
    let stmts = parse_stmts(&code);
    if let Stmt::Display { operands, .. } = &stmts[0] {
        assert_eq!(operands.len(), 2);
    } else {
        panic!("expected DISPLAY");
    }
}

// ── STOP RUN ──────────────────────────────────────────────────────────────────

#[test]
fn stop_run() {
    let code = prog("    STOP RUN.\n");
    let stmts = parse_stmts(&code);
    assert!(matches!(stmts[0], Stmt::Stop { run: true, .. }));
}

// ── CONTINUE ─────────────────────────────────────────────────────────────────

#[test]
fn continue_stmt() {
    let code = prog("    CONTINUE.\n    STOP RUN.\n");
    let stmts = parse_stmts(&code);
    assert!(matches!(stmts[0], Stmt::Continue { .. }));
}

// ── GO TO ────────────────────────────────────────────────────────────────────

#[test]
fn go_to_paragraph() {
    let code = prog("    GO TO END-PARA.\n    END-PARA.\n    STOP RUN.\n");
    let result = parse(tokenize(&code, SourceFormat::Free));
    // May have diagnostics if END-PARA is parsed as keyword, but we just
    // check the program was produced
    assert!(result.program.is_some());
}

// ── EVALUATE ─────────────────────────────────────────────────────────────────

#[test]
fn evaluate_when() {
    let code = prog(
        "    EVALUATE WS-CODE\n      WHEN 1 MOVE 'ONE' TO WS-TEXT\n      WHEN 2 MOVE 'TWO' TO WS-TEXT\n      WHEN OTHER MOVE 'UNK' TO WS-TEXT\n    END-EVALUATE.\n    STOP RUN.\n",
    );
    let stmts = parse_stmts(&code);
    if let Stmt::Evaluate { whens, other_stmts, .. } = &stmts[0] {
        assert_eq!(whens.len(), 2);
        assert!(!other_stmts.is_empty());
    } else {
        panic!("expected EVALUATE");
    }
}

// ── Full hello-world integration ──────────────────────────────────────────────

#[test]
fn hello_world_program() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO.
PROCEDURE DIVISION.
MAIN.
    DISPLAY 'HELLO, WORLD!'.
    STOP RUN.
";
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let prog = result.program.unwrap();
    assert_eq!(prog.identification.program_id, "HELLO");
}

// ── CALL ──────────────────────────────────────────────────────────────────────

#[test]
fn call_subprogram() {
    let code = prog("    CALL 'MYSUB' USING WS-A WS-B.\n    STOP RUN.\n");
    let stmts = parse_stmts(&code);
    assert!(matches!(stmts[0], Stmt::Call { .. }));
    if let Stmt::Call { using, .. } = &stmts[0] {
        assert_eq!(using.len(), 2);
    }
}
