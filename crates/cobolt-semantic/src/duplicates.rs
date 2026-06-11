// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Detection of redeclared unique procedure elements (paragraphs and sections).
//!
//! COBOL requires procedure names to be unique within their scope: a section
//! name must be unique within the program, and a paragraph name must be unique
//! within its containing section (or within the program when no sections are
//! used). A source that defines the same name twice is rejected with a hard
//! [`Severity::Error`] so the program cannot be run until the conflict is fixed.
//!
//! Duplicate *division* headers and a duplicate `PROGRAM-ID` are not visible
//! here — the AST keeps only one of each — so they are detected earlier, in the
//! parser's token-stream scan.

use std::collections::HashSet;

use cobolt_ast::program::{ProcedureBody, Program};

use crate::{SemanticDiagnostic, Severity};

/// Synthetic name the parser assigns to statements that appear outside any
/// explicit paragraph/section header. These are not user declarations and must
/// never be reported as duplicates.
const IMPLICIT: &str = "<implicit>";

/// Check the program and all of its nested programs for redeclared procedure
/// names.
pub fn check(program: &Program, diagnostics: &mut Vec<SemanticDiagnostic>) {
    check_program(program, diagnostics);
    for nested in &program.nested_programs {
        check(nested, diagnostics);
    }
}

fn check_program(program: &Program, diagnostics: &mut Vec<SemanticDiagnostic>) {
    match &program.procedure.body {
        ProcedureBody::Paragraphs(paras) => {
            let mut seen: HashSet<String> = HashSet::new();
            for para in paras {
                record(&para.name, para.span, "paragraph", &mut seen, diagnostics);
            }
        }
        ProcedureBody::Sections(secs) => {
            let mut seen_sections: HashSet<String> = HashSet::new();
            for sec in secs {
                record(&sec.name, sec.span, "section", &mut seen_sections, diagnostics);
                // Paragraph names must be unique within their own section.
                let mut seen_paras: HashSet<String> = HashSet::new();
                for para in &sec.paragraphs {
                    record(&para.name, para.span, "paragraph", &mut seen_paras, diagnostics);
                }
            }
        }
    }
}

/// Record one name; emit an error if it was already seen in this scope.
fn record(
    name: &str,
    span: cobolt_lexer::Span,
    kind: &str,
    seen: &mut HashSet<String>,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    if name.is_empty() || name == IMPLICIT {
        return;
    }
    // COBOL names are case-insensitive.
    let key = name.to_ascii_uppercase();
    if !seen.insert(key) {
        diagnostics.push(SemanticDiagnostic {
            severity: Severity::Error,
            message: format!(
                "{kind} '{name}' is declared more than once; {kind} names must be unique"
            ),
            span,
        });
    }
}
