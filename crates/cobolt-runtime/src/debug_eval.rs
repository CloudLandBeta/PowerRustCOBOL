// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Parsing what the developer types into a watch, a breakpoint condition, or
//! the console prompt.
//!
//! There is no public entry point for "parse one expression" — the parser is
//! built to read whole programs. Rather than carry a second, smaller COBOL
//! grammar that would quietly drift from the real one, a fragment is **wrapped
//! in a synthetic program** and the parser is asked to read that; the AST node
//! is then lifted back out.
//!
//! ```text
//!   WS-COUNT + 1     ->   …  PROCEDURE DIVISION.  DISPLAY WS-COUNT + 1.
//!   WS-N > 100       ->   …  PROCEDURE DIVISION.  IF WS-N > 100 CONTINUE END-IF.
//! ```
//!
//! So a watch accepts exactly what the language accepts — qualified names,
//! subscripts, reference modification, arithmetic, condition-names, `LENGTH OF`,
//! intrinsic functions — and gains each of them the day the parser does,
//! without a line of work here.

use cobolt_ast::expr::{Condition, Expr};
use cobolt_ast::program::ProcedureBody;
use cobolt_ast::stmt::Stmt;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};

/// The synthetic program a fragment is read inside. `{}` takes the fragment.
fn wrap(body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. DBGEVAL.\n\
         PROCEDURE DIVISION.\n\
         {body}\n"
    )
}

/// The first hard error the parser reported, phrased for a developer looking at
/// their own text rather than at our wrapper.
fn first_error(diags: &[cobolt_parser::Diagnostic]) -> Option<String> {
    diags
        .iter()
        .find(|d| d.severity == Severity::Error)
        .map(|d| d.message.clone())
}

/// Every statement of the wrapped program, whichever shape its body took.
///
/// The parser produces `Sections` or `Paragraphs` depending on what it read;
/// the wrapper has neither, so both are walked rather than assuming one.
fn statements(body: &ProcedureBody) -> Vec<&Stmt> {
    match body {
        ProcedureBody::Sections(secs) => secs
            .iter()
            .flat_map(|s| s.paragraphs.iter())
            .flat_map(|p| p.stmts.iter())
            .collect(),
        ProcedureBody::Paragraphs(paras) => {
            paras.iter().flat_map(|p| p.stmts.iter()).collect()
        }
    }
}

/// Parse a value expression: what a watch shows and what the console prints.
pub fn parse_expression(text: &str) -> Result<Expr, String> {
    let src = text.trim().trim_end_matches('.');
    if src.is_empty() {
        return Err("nothing to evaluate".into());
    }
    let result = parse(tokenize(&wrap(&format!("    DISPLAY {src}.")), SourceFormat::Free));
    if let Some(msg) = first_error(&result.diagnostics) {
        return Err(msg);
    }
    let program = result.program.ok_or_else(|| "could not read that".to_owned())?;
    for stmt in statements(&program.procedure.body) {
        if let Stmt::Display { operands, .. } = stmt {
            if let Some(e) = operands.first() {
                return Ok(e.clone());
            }
        }
    }
    Err(format!("{src} is not an expression"))
}

/// Parse a condition: a breakpoint's `condition`, and `IF`-style watches.
///
/// Kept apart from [`parse_expression`] because COBOL's conditions are their own
/// grammar — `WS-N > 100`, `WS-FLAG = \"Y\" AND NOT WS-DONE`, a bare 88-level —
/// and none of them is a value expression.
pub fn parse_condition(text: &str) -> Result<Condition, String> {
    let src = text.trim().trim_end_matches('.');
    if src.is_empty() {
        return Err("a condition cannot be empty".into());
    }
    let result = parse(tokenize(
        &wrap(&format!("    IF {src} CONTINUE END-IF.")),
        SourceFormat::Free,
    ));
    if let Some(msg) = first_error(&result.diagnostics) {
        return Err(msg);
    }
    let program = result.program.ok_or_else(|| "could not read that".to_owned())?;
    for stmt in statements(&program.procedure.body) {
        if let Stmt::If { condition, .. } = stmt {
            return Ok(condition.clone());
        }
    }
    Err(format!("{src} is not a condition"))
}

#[cfg(test)]
mod debug_eval_tests {
    use super::*;

    /// The point of reusing the real parser: a watch accepts what the LANGUAGE
    /// accepts, not a subset somebody remembered to reimplement.
    #[test]
    fn the_expression_forms_a_developer_would_type_all_parse() {
        for src in [
            "WS-COUNT",
            "WS-COUNT + 1",
            "WS-A * (WS-B - 2)",
            "WS-TABLE(3)",
            "BALANCE OF ACCOUNT",
            "WS-NAME(1:4)",
            "LENGTH OF WS-NAME",
            "FUNCTION LENGTH(WS-NAME)",
            "  WS-COUNT  ",
            "WS-COUNT.",
        ] {
            assert!(parse_expression(src).is_ok(), "should parse: {src:?}");
        }
    }

    #[test]
    fn the_condition_forms_a_developer_would_type_all_parse() {
        for src in [
            "WS-N > 100",
            "WS-N = 0",
            "WS-FLAG = \"Y\"",
            "WS-A > 1 AND WS-B < 2",
            "NOT WS-DONE",
            "WS-N >= 5 OR WS-N <= 1",
        ] {
            assert!(parse_condition(src).is_ok(), "should parse: {src:?}");
        }
    }

    /// A refusal must name what is wrong with the DEVELOPER'S text — never leak
    /// the wrapper program they did not write.
    #[test]
    fn nonsense_is_refused_with_a_readable_reason() {
        for bad in ["", "   ", "+", "WS-A +", "((("] {
            let err = parse_expression(bad).unwrap_err();
            assert!(!err.is_empty(), "{bad:?} must explain itself");
            assert!(
                !err.contains("DBGEVAL") && !err.contains("PROCEDURE DIVISION"),
                "the wrapper leaked into {err:?}"
            );
        }
        assert!(parse_condition("").is_err());
    }
}
