// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Validating COBOL the way the IDE does** — moved out of the IDE (spec 080
//! D2) so the IDE's form validation and Check, and the coding-agent `check`
//! tool, are one implementation rather than two that agree until the next edit.

use std::path::Path;

use cobolt_lexer::SourceFormat;

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

/// One diagnostic against a program's text: 1-based line and column, 0 when
/// the problem has no position (a copybook that could not be expanded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diag {
    pub line: u32,
    pub col: u32,
    pub message: String,
    pub severity: Severity,
}

/// The source format the IDE's Check assumes for a hand-written file: fixed
/// when any line carries a sequence area (six blanks or digits) followed by an
/// indicator, free otherwise.
pub fn source_format(source: &str) -> SourceFormat {
    if source.lines().any(|l| {
        let b = l.as_bytes();
        b.len() > 6 && b[6] != b' ' && b[..6].iter().all(|&c| c == b' ' || c.is_ascii_digit())
    }) {
        SourceFormat::Fixed
    } else {
        SourceFormat::Free
    }
}

/// Lex (with `COPY`/`REPLACE` expanded from `program`'s folder and the
/// project's), parse and analyse `source`. `external_crates` is the semantic
/// analyser's project context: `Some(lib names)` inside a project, `None` for
/// a loose file.
pub fn validate_text(
    source: &str,
    program: &Path,
    format: SourceFormat,
    external_crates: Option<Vec<String>>,
) -> Vec<Diag> {
    let mut diags = Vec::new();
    let tokens = match cobolt_lexer::preprocess_program(source, program, format) {
        Some(exp) => {
            for (i, e) in exp.errors.iter().enumerate() {
                diags.push(Diag {
                    line: exp.error_lines.get(i).copied().unwrap_or(0),
                    col: 0,
                    message: format!("copybook error: {e}"),
                    severity: Severity::Error,
                });
            }
            cobolt_lexer::tokenize_expansion(&exp)
        }
        None => cobolt_lexer::tokenize(source, format),
    };
    let parse_result = cobolt_parser::parse(tokens);
    for d in &parse_result.diagnostics {
        diags.push(Diag {
            line: d.span.line,
            col: d.span.col,
            message: d.message.clone(),
            severity: match d.severity {
                cobolt_parser::Severity::Error => Severity::Error,
                cobolt_parser::Severity::Warning => Severity::Warning,
            },
        });
    }
    if let Some(prog) = parse_result.program {
        let sem = cobolt_semantic::analyze_with(
            &prog,
            &cobolt_semantic::AnalyzeOptions {
                external_crates,
                // 049 R17 — as in the IDE: only the build-path check runs.
                form_formats: None,
                // A product gate: an undeclared item is an error.
                tolerate_undeclared: false,
                // No form context here: receivers are checked by Run Form
                // and Build (1.80.142), not by this lint yet.
                known_objects: None,
            },
        );
        for d in &sem.diagnostics {
            diags.push(Diag {
                line: d.span.line,
                col: d.span.col,
                message: d.message.clone(),
                severity: match d.severity {
                    cobolt_semantic::Severity::Error => Severity::Error,
                    cobolt_semantic::Severity::Warning => Severity::Warning,
                    cobolt_semantic::Severity::Info => Severity::Info,
                },
            });
        }
    }
    diags
}

/// A hand-written source file, with the IDE's fixed/free heuristic.
pub fn validate_source(path: &Path, text: &str, external_crates: Option<Vec<String>>) -> Vec<Diag> {
    validate_text(text, path, source_format(text), external_crates)
}

/// A form's **generated** program, validated in memory — nothing is written.
///
/// Validating the whole generated program (the source the interpreter runs,
/// spec 017) keeps every handler and the shared WORKING-STORAGE in one scope.
/// `program` is where that program is (or will be) written: its `COPY`
/// directives are expanded from there, as Run Form and a build expand them.
/// Returns the diagnostics, the generated text and its source map (spec 053).
pub fn validate_form_source(
    form: &cobolt_forms::Form,
    program: &Path,
    external_crates: Option<Vec<String>>,
) -> (Vec<Diag>, String, cobolt_codegen::SourceMap) {
    // Generated form source is always free-form.
    let (src, map) = cobolt_codegen::generate_with_map(form);
    let mut diags = validate_text(&src, program, SourceFormat::Free, external_crates);
    diags.extend(unknown_sql_connections(form, program));
    (diags, src, map)
}

/// Spec 087 R40 — every `SqlDatabase` whose `SqlConnection` names none of
/// the project's SQL connections, read from the project file `program`
/// belongs to. A form outside any project has none to name.
fn unknown_sql_connections(form: &cobolt_forms::Form, program: &Path) -> Vec<Diag> {
    fn walk<'a>(list: &'a [cobolt_forms::Control], out: &mut Vec<&'a cobolt_forms::Control>) {
        for c in list {
            out.push(c);
            walk(&c.children, out);
        }
    }
    let mut controls = Vec::new();
    walk(&form.controls, &mut controls);
    let named: Vec<(&str, String)> = controls
        .iter()
        .filter(|c| c.control_type == cobolt_forms::ControlType::SqlDatabase)
        .filter_map(|c| {
            let name = c.get_prop("SqlConnection")?.as_str().trim().to_string();
            (!name.is_empty()).then_some((c.id.as_str(), name))
        })
        .collect();
    if named.is_empty() {
        return Vec::new();
    }
    let known: Vec<String> = cobolt_compiler::find_project_manifest(program)
        .and_then(|m| std::fs::read_to_string(m).ok())
        .and_then(|text| cobolt_forms::connections::parse_sql_connections(&text, "sql-connections").ok())
        .map(|(list, _)| list.into_iter().map(|c| c.name).collect())
        .unwrap_or_default();
    named
        .into_iter()
        .filter(|(_, name)| !known.iter().any(|k| k.trim().eq_ignore_ascii_case(name)))
        .map(|(id, name)| Diag {
            line: 0,
            col: 0,
            message: if known.is_empty() {
                format!("{id}: SqlConnection '{name}' names an SQL connection, but the project defines none")
            } else {
                format!(
                    "{id}: SqlConnection '{name}' is not one of the project's SQL connections ({})",
                    known.join(", ")
                )
            },
            severity: Severity::Error,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 087 R40 (AC17) — an `SqlConnection` naming none of the project's
    /// SQL connections is a Check error; a known name, in any case, is not.
    #[test]
    fn unknown_sql_connection_is_a_check_error() {
        let dir = std::env::temp_dir().join(format!("prc087-check-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("generated")).unwrap();
        std::fs::write(
            dir.join("shop.project.toml"),
            "[project]\nname = \"Shop\"\nversion = \"1.0.0\"\nmain = \"forms/main.cfrm\"\n\n\
             [[sql-connections]]\nname = \"SALES\"\npath = \"data/sales.db\"\n",
        )
        .unwrap();
        let mut form = cobolt_forms::Form::new("MAIN-FORM", "Main", 400, 300);
        let mut db = cobolt_forms::Control::new("DB-1", cobolt_forms::ControlType::SqlDatabase, 0, 0);
        db.set_prop("SqlConnection", cobolt_forms::PropValue::String("SALEZ".into()));
        form.controls.push(db);
        let program = dir.join("generated/main-form.cbl");
        let errors = |f: &cobolt_forms::Form| -> Vec<String> {
            validate_form_source(f, &program, None)
                .0
                .into_iter()
                .filter(|d| d.severity == Severity::Error)
                .map(|d| d.message)
                .collect()
        };
        let found = errors(&form);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("DB-1") && found[0].contains("SALEZ") && found[0].contains("SALES"), "{found:?}");
        form.controls[0].set_prop("SqlConnection", cobolt_forms::PropValue::String("sales".into()));
        assert!(errors(&form).is_empty(), "a known name is fine, in any case");
        let _ = std::fs::remove_dir_all(&dir);
        println!("unknown SqlConnection: '{}'", found[0]);
    }

    #[test]
    fn validate_source_heuristic_and_diagnostics() {
        let fixed = "       IDENTIFICATION DIVISION.\n      * a comment\n       PROGRAM-ID. A.\n";
        assert_eq!(source_format(fixed), SourceFormat::Fixed);
        let free = "IDENTIFICATION DIVISION.\nPROGRAM-ID. A.\n";
        assert_eq!(source_format(free), SourceFormat::Free);

        let good = "IDENTIFICATION DIVISION.\nPROGRAM-ID. A.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
                    01 WS-X PIC 9.\nPROCEDURE DIVISION.\n    MOVE 1 TO WS-X.\n    STOP RUN.\n";
        let d = validate_source(Path::new("a.cbl"), good, None);
        assert!(d.iter().all(|d| d.severity != Severity::Error), "{d:?}");
        let bad = good.replace("MOVE 1 TO WS-X", "MOVE 1 TO WS-NOT-DECLARED");
        let d = validate_source(Path::new("a.cbl"), &bad, None);
        let errors: Vec<_> = d.iter().filter(|d| d.severity == Severity::Error).collect();
        assert!(!errors.is_empty(), "an undeclared item is an error");
        assert_eq!(errors[0].line, 7, "{errors:?}");
        println!(
            "validate_source: fixed/free heuristic 2 cases; clean program 0 errors; undeclared item → {} error(s) at line {}",
            errors.len(),
            errors[0].line
        );
    }
}
