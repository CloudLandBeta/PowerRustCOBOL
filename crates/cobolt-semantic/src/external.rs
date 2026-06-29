// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Validate `EXTERNAL` and `GLOBAL` data placement (specs 005, 009).
//!
//! The `EXTERNAL` and `GLOBAL` clauses are valid only on `01`/`77`-level items
//! (and `FD`s). A subordinate item (`05`, `10`, …) carrying either clause is an
//! error. (Run-unit description-consistency for the same EXTERNAL name across
//! programs is checked at run time when the shared store is populated — see the
//! runtime.) `FD … IS GLOBAL` (spec 009 R6) is accepted by the parser and makes
//! the file/records visible to the form's nested procedures.

use cobolt_ast::data::DataDecl;
use cobolt_ast::program::{DataSection, Program};

use crate::{SemanticDiagnostic, Severity};

/// Check this program and all nested programs.
pub fn check(program: &Program, diagnostics: &mut Vec<SemanticDiagnostic>) {
    check_program(program, diagnostics);
    for nested in &program.nested_programs {
        check(nested, diagnostics);
    }
}

fn check_program(program: &Program, diagnostics: &mut Vec<SemanticDiagnostic>) {
    let Some(data) = &program.data else { return };
    for section in &data.sections {
        match section {
            DataSection::WorkingStorage(items)
            | DataSection::LocalStorage(items)
            | DataSection::Linkage(items) => {
                for decl in items {
                    check_decl(decl, diagnostics);
                }
            }
            DataSection::FileSection(fds) => {
                for fd in fds {
                    for rec in &fd.records {
                        check_decl(rec, diagnostics);
                    }
                }
            }
            DataSection::Screen(_) => {}
        }
    }
}

fn check_decl(decl: &DataDecl, diagnostics: &mut Vec<SemanticDiagnostic>) {
    let misplaced = decl.level != 1 && decl.level != 77;
    let who = || {
        decl.name
            .as_deref()
            .map(|n| format!(" '{n}'"))
            .unwrap_or_default()
    };
    if decl.is_external && misplaced {
        diagnostics.push(SemanticDiagnostic {
            severity: Severity::Error,
            message: format!(
                "EXTERNAL is valid only on 01/77-level items (or FDs); found on a \
                 level-{:02} item{}",
                decl.level,
                who()
            ),
            span: decl.span,
        });
    }
    // GLOBAL placement (spec 009 R6): valid on 01/77 items and FDs only.
    if decl.is_global && misplaced {
        diagnostics.push(SemanticDiagnostic {
            severity: Severity::Error,
            message: format!(
                "GLOBAL is valid only on 01/77-level items (or FDs); found on a \
                 level-{:02} item{}",
                decl.level,
                who()
            ),
            span: decl.span,
        });
    }
    for child in &decl.children {
        check_decl(child, diagnostics);
    }
}
