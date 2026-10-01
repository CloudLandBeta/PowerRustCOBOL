// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! An INDEXED file's keys are fields of its own record.
//!
//! `RECORD KEY IS data-name` and every `ALTERNATE RECORD KEY` name a data item
//! described in the file's FD (COBOL-85, the RECORD KEY clause's syntax
//! rules). Nothing checked it: a program whose FD renamed the key field while
//! the SELECT kept the old name compiled cleanly, WRITE answered `00`, and a
//! READ by key never found the record (operator report, 2026-09-30, LugSys:
//! "mudei o nome da chave da FD, mas esqueci de alterar no RECORD KEY da
//! Select, e não acusou erro"). It is an error here, before the program runs.

use cobolt_ast::data::DataDecl;
use cobolt_ast::program::{DataSection, FileOrganization, Program};

use crate::{SemanticDiagnostic, Severity};

/// Check this program and all nested programs.
pub fn check(program: &Program, diagnostics: &mut Vec<SemanticDiagnostic>) {
    check_program(program, diagnostics);
    for nested in &program.nested_programs {
        check(nested, diagnostics);
    }
}

fn names_in(decl: &DataDecl, out: &mut Vec<String>) {
    if decl.level == 88 {
        return;
    }
    if let Some(n) = &decl.name {
        out.push(n.to_ascii_uppercase());
    }
    for c in &decl.children {
        names_in(c, out);
    }
}

fn check_program(program: &Program, diagnostics: &mut Vec<SemanticDiagnostic>) {
    let Some(env) = &program.environment else { return };
    let Some(io) = &env.input_output else { return };
    let Some(data) = &program.data else { return };
    for fc in &io.file_controls {
        if fc.organization != FileOrganization::Indexed {
            continue;
        }
        // The FD of this file; without one another check reports the file.
        let fd = data.sections.iter().find_map(|s| match s {
            DataSection::FileSection(fds) => fds.iter().find(|fd| fd.name.eq_ignore_ascii_case(&fc.name)),
            _ => None,
        });
        let Some(fd) = fd else { continue };
        let mut fields = Vec::new();
        fd.records.iter().for_each(|r| names_in(r, &mut fields));
        let mut keys: Vec<(&str, &str)> = Vec::new();
        if let Some(k) = &fc.record_key {
            keys.push(("RECORD KEY", k.as_str()));
        }
        for a in &fc.alternate_keys {
            keys.push(("ALTERNATE RECORD KEY", a.field.as_str()));
        }
        for (clause, key) in keys {
            if fields.iter().any(|f| f.eq_ignore_ascii_case(key.trim())) {
                continue;
            }
            diagnostics.push(SemanticDiagnostic {
                severity: Severity::Error,
                message: format!(
                    "{clause} {key} of file '{}' is not a field of its record — the key must be \
                     declared in FD {}; name the field that holds the key, or declare it there",
                    fc.name, fd.name
                ),
                span: fc.span,
            });
        }
    }
}
