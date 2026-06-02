// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! EXEC RUST block executor.
//!
//! At this stage the executor operates in **interpreted mode**: the Rust
//! source captured in `Stmt::ExecRust::source` is evaluated as a simple
//! expression language using a lightweight embedded interpreter.  A future
//! version will use `proc_macro2` + `rustc_codegen_cranelift` (or similar)
//! to JIT-compile the block natively.
//!
//! # Current capability (interpreted mode)
//!
//! The interpreter understands a useful subset of Rust-like syntax:
//!
//! * Simple assignment:  `*ws_count = 42;`
//! * Compound assignment: `*ws_total += *ws_count;`
//! * Integer arithmetic: `+`, `-`, `*`, `/`
//! * Comments: `//` to end of line
//!
//! # Binding convention
//!
//! Before execution, the interpreter builds a local variable table from
//! `Stmt::ExecRust::referenced_data` (populated by `cobolt-semantic`).
//! Each COBOL data item is bound as a mutable i64/f64/string alias.
//!
//! After execution the modified values are written back to the environment.
//!
//! # Future: native compilation
//!
//! The plan is to generate a Rust source file with a preamble that binds
//! data items as typed `&mut` references, compile it into a shared object
//! with `rustc`, and `dlopen` it.  The API surface (function signature,
//! `CobolEnvironment`, `ObjectRegistry`) is already designed for this.

use cobolt_ast::stmt::Stmt;
use cobolt_lexer::Span;

use crate::{
    environment::CobolEnvironment,
    error::RuntimeError,
    objects::ObjectRegistry,
    value::CobolValue,
};

// ── Public entry point ────────────────────────────────────────────────────────

/// Execute an `EXEC RUST` block.
///
/// Binds COBOL data items into a local scope, runs the interpreted Rust
/// source, and writes back any modified values.
pub fn execute(
    stmt: &Stmt,
    env: &mut CobolEnvironment,
    objects: &mut ObjectRegistry,
) -> Result<(), RuntimeError> {
    let (source, _referenced_data, span) = match stmt {
        Stmt::ExecRust { source, referenced_data, span } => (source, referenced_data, *span),
        _ => return Ok(()),
    };

    // Strip `//` line comments so the interpreter doesn't choke on them.
    let clean: String = source
        .lines()
        .map(|line| {
            if let Some(idx) = line.find("//") {
                &line[..idx]
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Process each `;`-terminated statement.
    for raw_stmt in clean.split(';') {
        let s = raw_stmt.trim();
        if s.is_empty() { continue; }

        interpret_statement(s, env, objects, span)?;
    }

    Ok(())
}

// ── Interpreted statement mini-evaluator ─────────────────────────────────────

/// Interpret a single Rust statement (assignment / compound assignment).
fn interpret_statement(
    src: &str,
    env: &mut CobolEnvironment,
    _objects: &mut ObjectRegistry,
    span: Span,
) -> Result<(), RuntimeError> {
    // Compound assignment: `*ws_count += expr`
    for op in &["+=", "-=", "*=", "/="] {
        if let Some((lhs, rhs)) = src.split_once(op) {
            let name = cobol_name(lhs.trim());
            let rhs_val = eval_expr(rhs.trim(), env, span)?;
            let lhs_val = env.get(&name)
                .cloned()
                .unwrap_or(CobolValue::from_i64(0));
            let result = apply_compound(lhs_val, rhs_val, op, span)?;
            env.set(&name, result);
            return Ok(());
        }
    }

    // Simple assignment: `*ws_count = expr` or `ws_count = expr`
    if let Some((lhs, rhs)) = src.split_once('=') {
        // Avoid matching `==`
        if !rhs.starts_with('=') && !lhs.ends_with('!') {
            let name = cobol_name(lhs.trim());
            let value = eval_expr(rhs.trim(), env, span)?;
            env.set(&name, value);
            return Ok(());
        }
    }

    // `let` declarations: `let x = expr` — bind locally, ignore for now
    if src.starts_with("let ") {
        return Ok(()); // MVP: let bindings are no-ops
    }

    // Unknown statement form — log and continue (don't hard-fail).
    tracing::debug!("EXEC RUST: unrecognised statement: {src:?}");
    Ok(())
}

/// Evaluate a simple expression to a `CobolValue`.
/// Supports: literals, `*ident` / `ident`, binary `+`, `-`, `*`, `/`, `as i64`.
fn eval_expr(
    src: &str,
    env: &CobolEnvironment,
    span: Span,
) -> Result<CobolValue, RuntimeError> {
    let src = src.trim();

    // Strip trailing `as i64` / `as f64` casts (no-op at runtime)
    let src = src
        .trim_end_matches("as i64")
        .trim_end_matches("as f64")
        .trim_end_matches("as i32")
        .trim();

    // Try integer literal
    if let Ok(n) = src.parse::<i64>() {
        return Ok(CobolValue::from_i64(n));
    }
    // Try float literal
    if let Ok(f) = src.parse::<f64>() {
        return Ok(CobolValue::from_f64(f));
    }
    // String literal  'hello' or "hello"
    if (src.starts_with('\'') && src.ends_with('\''))
        || (src.starts_with('"') && src.ends_with('"'))
    {
        let inner = &src[1..src.len() - 1];
        return Ok(CobolValue::from_str(inner, inner.len()));
    }

    // Binary operator — split on the last low-precedence operator
    if let Some(v) = try_binary(src, env, span)? {
        return Ok(v);
    }

    // Parenthesised expression
    if src.starts_with('(') && src.ends_with(')') {
        return eval_expr(&src[1..src.len() - 1], env, span);
    }

    // Data-item dereference: `*ws_count` or bare `ws_count`
    let name = cobol_name(src);
    if let Some(v) = env.get(&name) {
        return Ok(v.clone());
    }

    // Unknown identifier — return zero rather than error
    tracing::debug!("EXEC RUST eval: unknown identifier '{name}'");
    Ok(CobolValue::from_i64(0))
}

/// Try to split `src` on a binary operator and evaluate both sides.
fn try_binary(
    src: &str,
    env: &CobolEnvironment,
    span: Span,
) -> Result<Option<CobolValue>, RuntimeError> {
    // Search from right to get lowest-precedence operator first.
    for op in &['+', '-', '*', '/'] {
        // Walk backwards to find the operator (skip inside parentheses)
        let bytes = src.as_bytes();
        let mut depth = 0i32;
        let mut i = bytes.len();
        while i > 0 {
            i -= 1;
            match bytes[i] {
                b')' => depth += 1,
                b'(' => depth -= 1,
                b if b == *op as u8 && depth == 0 && i > 0 => {
                    let lhs_str = &src[..i];
                    let rhs_str = &src[i + 1..];
                    if lhs_str.is_empty() || rhs_str.is_empty() { break; }
                    let lhs = eval_expr(lhs_str, env, span)?;
                    let rhs = eval_expr(rhs_str, env, span)?;
                    let result = match op {
                        '+' => numeric_op(&lhs, &rhs, |a, b| a + b, span)?,
                        '-' => numeric_op(&lhs, &rhs, |a, b| a - b, span)?,
                        '*' => numeric_op(&lhs, &rhs, |a, b| a * b, span)?,
                        '/' => {
                            let d = rhs.as_f64();
                            if d == 0.0 {
                                return Err(RuntimeError::DivisionByZero { span });
                            }
                            CobolValue::from_f64(lhs.as_f64() / d)
                        }
                        _ => unreachable!(),
                    };
                    return Ok(Some(result));
                }
                _ => {}
            }
        }
    }
    Ok(None)
}

fn numeric_op(
    lhs: &CobolValue,
    rhs: &CobolValue,
    op: impl Fn(f64, f64) -> f64,
    span: Span,
) -> Result<CobolValue, RuntimeError> {
    Ok(CobolValue::from_f64(op(lhs.as_f64(), rhs.as_f64())))
}

fn apply_compound(
    lhs: CobolValue,
    rhs: CobolValue,
    op: &str,
    span: Span,
) -> Result<CobolValue, RuntimeError> {
    let l = lhs.as_f64();
    let r = rhs.as_f64();
    let result = match op {
        "+=" => l + r,
        "-=" => l - r,
        "*=" => l * r,
        "/=" => {
            if r == 0.0 { return Err(RuntimeError::DivisionByZero { span }); }
            l / r
        }
        _ => l,
    };
    // Preserve the receiver type (numeric vs float)
    match lhs {
        CobolValue::Numeric(ref n) => {
            let mut v = CobolValue::Numeric(n.clone());
            v.assign(&CobolValue::from_f64(result));
            Ok(v)
        }
        _ => Ok(CobolValue::from_f64(result)),
    }
}

/// Convert a Rust snake_case / dereference expression to a COBOL data-item name.
///
/// `*ws_count` → `WS-COUNT`, `ws_name` → `WS-NAME`
fn cobol_name(s: &str) -> String {
    s.trim_start_matches('*')
        .trim()
        .to_ascii_uppercase()
        .replace('_', "-")
}
