// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! User-defined functions (spec 109 §5.1, COBOL-2002 `FUNCTION-ID`).
//!
//! `FUNCTION DOUBLE-AMOUNT(WS-AMOUNT)` in an expression runs the unit written
//! `FUNCTION-ID. DOUBLE-AMOUNT.` and answers its RETURNING item. The call IS a
//! `CALL name USING BY CONTENT arg… RETURNING result` — same activation, same
//! parameter binding, same recursion — with the result item held in a hidden
//! slot of this call instead of a data item the caller declared, so no shared
//! scratch argument or result exists (R3): each activation has its own.
//!
//! Each argument is evaluated once, left to right (R2). `BY CONTENT` is how a
//! value argument reaches the callee: the function cannot change the caller's
//! item, which is what `BY VALUE` means to a function.

use cobolt_ast::expr::{Expr, Literal};
use cobolt_ast::stmt::CallArg;
use cobolt_lexer::Span;

use super::Interpreter;
use crate::error::RuntimeError;
use crate::value::CobolValue;

impl Interpreter {
    pub(super) fn call_user_function(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<CobolValue, RuntimeError> {
        let key = name.to_ascii_uppercase();
        let params = self.nested_registry.get(&key).map_or(0, |p| p.using.len());
        if args.len() != params {
            return Err(RuntimeError::General {
                message: format!(
                    "FUNCTION {key} takes {params} argument(s) and was given {}",
                    args.len()
                ),
            });
        }
        // Every argument is evaluated now, once, left to right, into a hidden
        // slot of its own: BY CONTENT then hands the callee a copy.
        self.function_depth += 1;
        let depth = self.function_depth;
        let mut using = Vec::with_capacity(args.len());
        let mut slots: Vec<String> = Vec::with_capacity(args.len() + 1);
        for (i, a) in args.iter().enumerate() {
            let slot = format!("COBOL-FN-ARG-{depth}-{i}");
            match self.eval_expr(a, span) {
                Ok(v) => self.env.raw_set(&slot, v),
                Err(e) => {
                    self.function_depth -= 1;
                    for s in &slots {
                        self.env.raw_take(s);
                    }
                    return Err(e);
                }
            }
            using.push(CallArg::ByContent(Expr::Identifier(slot.clone(), span)));
            slots.push(slot);
        }
        let result = format!("COBOL-FN-RESULT-{depth}");
        self.env.raw_set(&result, CobolValue::Unset);
        slots.push(result.clone());
        let program = Expr::Literal(Literal::String(key), span);
        let returning = Expr::Identifier(result.clone(), span);
        let outcome = self.exec_call(&program, &using, Some(&returning), &[], &[], span);
        let answer = self.env.get(&result).cloned().unwrap_or(CobolValue::Unset);
        for s in &slots {
            self.env.raw_take(s);
        }
        self.function_depth -= 1;
        outcome.map(|_| answer)
    }
}
