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
    validate_text_for(source, program, format, external_crates, None)
}

/// [`validate_text`], with the form whose program `source` is when there is
/// one: then a receiver (`X::…`) and a layer's properties are checked against
/// that form's objects, exactly as Run Form and Build check them.
fn validate_text_for(
    source: &str,
    program: &Path,
    format: SourceFormat,
    external_crates: Option<Vec<String>>,
    form: Option<&cobolt_forms::Form>,
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
                // With a form in view, its receivers (1.80.142) and its
                // layers' properties (spec 091 R37) are checked as Run Form
                // checks them, so the IDE's gate finds a handler still naming
                // a deleted control before the run does — and can say which
                // handler. A loose file has no form, and checks neither.
                known_objects: form.map(cobolt_forms::toolbar::object_names),
                known_layers: form.map(cobolt_forms::toolbar::layer_names),
                // Inside a project, a literal CALL must name a program, a
                // paragraph or a Common Code procedure of it (2026-10-08);
                // `None` — a path outside any project — checks nothing.
                known_programs: cobolt_compiler::common_code_names(program),
                // WASM-005 R4: the backend's exported programs, when the
                // project has imported them.
                remote_interface: cobolt_compiler::backend_interface(program),
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
    let mut diags = validate_text_for(&src, program, SourceFormat::Free, external_crates, Some(form));
    diags.extend(unknown_sql_connections(form, program));
    diags.extend(row_content_problems(form, &src));
    diags.extend(partiql_literal_problems(form, &src, program));
    (diags, src, map)
}

/// Spec 106 R17 — what makes a grid's `DATAGRID-ROW-CONTENT` unsafe or useless, for every
/// DataGrid that binds `onRowButtonClick`. The level-10 names are the columns' own names
/// (operator, Q1), so a clash is refused here rather than renamed away:
/// - a name that is also a data item of the form's program — the run unit keeps one store,
///   so the handler's field and the form's item would be one and the same;
/// - a reserved word, a name the group itself uses, two columns of one name, a name too long
///   or not a word at all;
/// - and, as a warning only, a Button column with neither caption nor icon (it shows each
///   row's cell value, as Button columns always have).
pub fn row_content_problems(form: &cobolt_forms::Form, generated: &str) -> Vec<Diag> {
    use cobolt_forms::row_content;
    fn walk<'a>(list: &'a [cobolt_forms::Control], out: &mut Vec<&'a cobolt_forms::Control>) {
        for c in list {
            out.push(c);
            walk(&c.children, out);
        }
    }
    let mut controls = Vec::new();
    walk(&form.controls, &mut controls);
    let grids: Vec<_> = controls
        .into_iter()
        .filter(|c| {
            c.control_type == cobolt_forms::ControlType::DataGrid
                && c.events.iter().any(|e| e.event == row_content::EVENT)
        })
        .collect();
    if grids.is_empty() {
        return Vec::new();
    }
    let names = main_program_data_names(generated);
    grids
        .into_iter()
        .flat_map(|g| problems_of(g, &names))
        .map(|p| Diag {
            line: 0,
            col: 0,
            message: p.message,
            severity: if p.is_error { Severity::Error } else { Severity::Warning },
        })
        .collect()
}

/// One grid's [`row_content_problems`], whether or not it binds the event yet — what the
/// *Edit DataGrid settings* window warns about as a column is named (R17), from the same
/// rules Check applies. `form_names` are [`form_data_names`] of the form's generated
/// program: a window keeps them while the form's own data does not change, so a keystroke
/// in a column title costs no generation.
pub fn grid_row_content_problems(
    grid: &cobolt_forms::Control,
    form_names: &std::collections::HashSet<String>,
) -> Vec<cobolt_forms::row_content::Problem> {
    problems_of(grid, form_names)
}

/// The data names a form's generated program declares, which no DATAGRID-ROW-CONTENT
/// item may also use (see [`row_content_problems`]).
pub fn form_data_names(generated: &str) -> std::collections::HashSet<String> {
    main_program_data_names(generated)
}

fn problems_of(
    grid: &cobolt_forms::Control,
    names: &std::collections::HashSet<String>,
) -> Vec<cobolt_forms::row_content::Problem> {
    let reserved = |w: &str| cobolt_lexer::keywords::lookup(&w.to_ascii_uppercase()).is_some();
    let advanced = cobolt_forms::model::DataGridAdvanced::from_control(grid);
    cobolt_forms::row_content::problems(&grid.id, &advanced, &reserved, names)
}

/// The data names the generated MAIN program declares — every entry between its first
/// `DATA DIVISION` and its first `PROCEDURE DIVISION`, the buffer spec 106 adds left out
/// (it is the group's own). Read from the text: the generator writes one entry per line, a
/// level number then the name.
fn main_program_data_names(generated: &str) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    let mut in_data = false;
    for line in generated.lines() {
        let t = line.trim();
        let upper = t.to_ascii_uppercase();
        if upper.starts_with("DATA DIVISION") {
            in_data = true;
            continue;
        }
        if upper.starts_with("PROCEDURE DIVISION") {
            break;
        }
        if !in_data || t.starts_with("*>") {
            continue;
        }
        let mut words = t.split_whitespace();
        let (Some(level), Some(name)) = (words.next(), words.next()) else {
            continue;
        };
        if level.len() <= 2 && level.chars().all(|c| c.is_ascii_digit()) {
            let name = name.trim_end_matches('.').to_ascii_uppercase();
            if name != "FILLER" && name != cobolt_forms::row_content::BUFFER {
                names.insert(name);
            }
        }
    }
    names
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

/// Spec 108 R42 — every PartiQL statement written as a literal in a handler, checked
/// against the `.cidx` of the `IndexedFile` control it is sent to, with no data file
/// and nothing opened: an unknown field, a type mismatch, an `ORDER BY` outside the key,
/// a write that does not pin the whole RECORD KEY, a table that is not the control's.
/// A statement held in a data item is not checked. A scan is reported as a note.
///
/// A literal is read on one line of the generated program; a statement split across
/// lines with a continuation is left to run time.
pub fn partiql_literal_problems(form: &cobolt_forms::Form, generated: &str, program: &Path) -> Vec<Diag> {
    use cobolt_runtime::partiql::analyze::{analyze, RStmt};
    use cobolt_runtime::partiql::parser::{parse_script, parse_statement};
    use cobolt_runtime::partiql::plan::{classify, ReadClass};
    use cobolt_runtime::partiql::schema::Schema;
    fn walk<'a>(list: &'a [cobolt_forms::Control], out: &mut Vec<&'a cobolt_forms::Control>) {
        for c in list {
            out.push(c);
            walk(&c.children, out);
        }
    }
    let mut controls = Vec::new();
    walk(&form.controls, &mut controls);
    let project_dir = cobolt_compiler::find_project_manifest(program).and_then(|m| m.parent().map(Path::to_path_buf));
    let mut out = Vec::new();
    for c in controls.iter().filter(|c| c.control_type == cobolt_forms::ControlType::IndexedFile) {
        let cidx = c.get_prop("IndexedFile").map(|p| p.as_str().trim().to_string()).unwrap_or_default();
        if cidx.is_empty() {
            continue;
        }
        let raw = Path::new(&cidx);
        let path = if raw.is_absolute() {
            raw.to_path_buf()
        } else {
            match &project_dir {
                Some(d) if d.join(raw).exists() => d.join(raw),
                _ => cobolt_forms::assets::resolve(&cidx),
            }
        };
        let Ok(def) = cobolt_indexed::load_indexed(&path) else { continue };
        let table = Path::new(&cidx).file_stem().and_then(|s| s.to_str()).unwrap_or("").to_ascii_uppercase();
        let Ok(schema) = Schema::from_definition(&table, &def) else { continue };
        for (n, line) in generated.lines().enumerate() {
            let upper = line.to_ascii_uppercase();
            for (method, unit) in [("EXECUTETRANSACTION(", true), ("EXECUTE(", false)] {
                let needle = format!("{}::{}", c.id.to_ascii_uppercase(), method);
                let Some(at) = upper.find(&needle) else { continue };
                // Not the longer name's tail.
                if !unit && upper[..at + needle.len()].ends_with("TRANSACTION(") {
                    continue;
                }
                let rest = line[at + needle.len()..].trim_start();
                let Some(quote) = rest.chars().next().filter(|q| *q == '"' || *q == '\'') else { continue };
                let body = &rest[1..];
                let mut text = String::new();
                let mut chars = body.chars().peekable();
                let mut closed = false;
                while let Some(ch) = chars.next() {
                    if ch == quote {
                        if chars.peek() == Some(&quote) {
                            text.push(quote);
                            chars.next();
                            continue;
                        }
                        closed = true;
                        break;
                    }
                    text.push(ch);
                }
                if !closed {
                    continue;
                }
                let line_no = n as u32 + 1;
                let col = (at + 1) as u32;
                let mk = |message: String, severity: Severity| Diag { line: line_no, col, message: format!("{}: {message}", c.id), severity };
                let parsed = if unit { parse_script(&text) } else { parse_statement(&text).map(|p| vec![p]) };
                match parsed {
                    Err(e) => out.push(mk(e.text(), Severity::Error)),
                    Ok(list) => {
                        for p in &list {
                            match analyze(p, &schema, unit) {
                                Err(e) => out.push(mk(e.text(), Severity::Error)),
                                Ok(a) => {
                                    if let RStmt::Select(sel) = &a.stmt {
                                        if classify(sel, &schema) == ReadClass::Scan {
                                            out.push(mk(
                                                "this SELECT reads the whole file (a scan); name the key in the WHERE to read by it".to_string(),
                                                Severity::Info,
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    out
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

    /// Spec 106 R17 — a grid that binds `onRowButtonClick` with a column whose name cannot
    /// be a level-10 field of `DATAGRID-ROW-CONTENT` is a Check error, as is a Button
    /// column with nothing on it; the same grid without the event is not checked at all.
    #[test]
    fn row_content_names_are_checked_when_the_event_is_bound() {
        use cobolt_forms::model::{DataGridAdvanced, DataGridColumn, EventBinding, DATAGRID_ADVANCED_PROP};
        fn column(name: &str, kind: &str, caption: &str) -> DataGridColumn {
            DataGridColumn {
                id: name.into(),
                title: name.into(),
                source_name: name.into(),
                edit_control: kind.into(),
                button_caption: caption.into(),
                ..Default::default()
            }
        }
        fn form(columns: Vec<DataGridColumn>, bound: bool) -> cobolt_forms::Form {
            let mut form = cobolt_forms::Form::new("ACTORS", "Actors", 600, 400);
            let mut grid = cobolt_forms::Control::new("DG", cobolt_forms::ControlType::DataGrid, 0, 0);
            let advanced = DataGridAdvanced { columns, ..Default::default() };
            grid.set_prop(DATAGRID_ADVANCED_PROP, cobolt_forms::PropValue::String(advanced.to_json().unwrap()));
            if bound {
                grid.events.push(EventBinding::for_control("DG", "onRowButtonClick"));
            }
            form.add_control(grid);
            form
        }
        let errors = |f: &cobolt_forms::Form| -> Vec<String> {
            let (src, _) = cobolt_codegen::generate_with_map(f);
            row_content_problems(f, &src)
                .into_iter()
                .map(|d| format!("{:?}: {}", d.severity, d.message))
                .collect()
        };

        let clean = vec![column("ACTOR-ID", "Textbox", ""), column("EDIT", "Button", "Edit")];
        assert!(errors(&form(clean.clone(), true)).is_empty(), "{:?}", errors(&form(clean, true)));

        let (src, _) = cobolt_codegen::generate_with_map(&form(Vec::new(), true));
        let names = main_program_data_names(&src);
        assert!(names.contains("FORM-NAME"), "{names:?}");
        assert!(!names.contains(cobolt_forms::row_content::BUFFER), "the buffer is the group's own");

        let bad = vec![
            column("MOVE", "Textbox", ""),
            column("FORM-NAME", "Textbox", ""),
            column("ROW-NUMBER", "Textbox", ""),
            column("GO", "Button", ""),
        ];
        let found = errors(&form(bad.clone(), true));
        for word in ["Error: DataGrid DG: column name MOVE", "Error: DataGrid DG: column name FORM-NAME"] {
            assert!(found.iter().any(|m| m.starts_with(word)), "{word} not reported: {found:?}");
        }
        assert!(found.iter().any(|m| m.starts_with("Error:") && m.contains("ROW-NUMBER")), "{found:?}");
        assert!(
            found.iter().any(|m| m.starts_with("Warning:") && m.contains("\"GO\"")),
            "a Button column with nothing on it is a warning: {found:?}"
        );
        assert_eq!(found.len(), 4, "{found:?}");
        assert!(errors(&form(bad, false)).is_empty(), "no event, no group, nothing to check");
        for m in &found {
            println!("row content: {m}");
        }
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

    /// Spec 108 R42 (AC15): PartiQL statements written as literals in a handler are checked
    /// against the control's `.cidx` with no data file; a statement in a data item is not.
    #[test]
    fn partiql_literals_are_checked_against_the_cidx() {
        let dir = std::env::temp_dir().join(format!("prc108-check-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("generated")).unwrap();
        let cidx = dir.join("customers.cidx");
        std::fs::write(
            &cidx,
            r#"<?xml version="1.0" encoding="UTF-8"?><IndexedFile name="CUSTOMERS" finalized="true" version="1.0"><assign-path>data/customers.idx</assign-path><access-mode>dynamic</access-mode><record-format fixed-length="56"/><storage mode="disk" compression="false" persistence="false"/><keys><primary duplicates="false" ordering="ascending"><part field="CUST-ID" offset="0" length="8" encoding="bytes"/></primary></keys><fields><Field level="1" name="CUST-REC" usage="display"><Field level="5" name="CUST-ID" pic="9(8)" usage="display" offset="0" length="8"></Field><Field level="5" name="CUST-NAME" pic="X(40)" usage="display" offset="8" length="40"></Field><Field level="5" name="CUST-BALANCE" pic="S9(6)V99" usage="display" offset="48" length="8"></Field></Field></fields></IndexedFile>"#,
        )
        .unwrap();
        let mut form = cobolt_forms::Form::new("MAIN-FORM", "Main", 400, 300);
        let mut ixf = cobolt_forms::Control::new("IXF-CUST", cobolt_forms::ControlType::IndexedFile, 0, 0);
        ixf.set_prop("IndexedFile", cobolt_forms::PropValue::String(cidx.to_string_lossy().into()));
        form.controls.push(ixf);
        let program = dir.join("generated").join("main-form.cbl");
        let generated = "\
           SET WS-N TO IXF-CUST::Execute(\"SELECT COUNT(*) FROM CUSTOMERS\")\n\
           SET WS-N TO IXF-CUST::Execute(\"SELECT * FROM CUSTOMERS WHERE CUST-BALANCE = 'ten'\")\n\
           SET WS-N TO IXF-CUST::Execute(\"UPDATE CUSTOMERS SET CUST-NAME = 'x' WHERE CUST-NAME = 'Ana'\")\n\
           SET WS-N TO IXF-CUST::Execute(\"SELECT * FROM INVOICES\")\n\
           SET WS-N TO IXF-CUST::Execute(\"SELECT * FROM CUSTOMERS WHERE NO-SUCH-FIELD = 1\")\n\
           SET WS-N TO IXF-CUST::Execute(\"SELECT CUST-NAME FROM CUSTOMERS WHERE CUST-ID = 1\")\n\
           SET WS-N TO IXF-CUST::Execute(\"SELECT * FROM CUSTOMERS WHERE CUST-BALANCE > 1000.50\")\n\
           SET WS-N TO IXF-CUST::Execute( WS-STMT )\n";
        let d = partiql_literal_problems(&form, generated, &program);
        let at = |line: u32| d.iter().filter(|x| x.line == line).collect::<Vec<_>>();
        assert!(at(1)[0].message.contains("COUNT") && at(1)[0].severity == Severity::Error, "{d:?}");
        assert!(at(2)[0].message.contains("Number"), "{d:?}");
        assert!(at(3)[0].message.contains("RECORD KEY"), "{d:?}");
        assert!(at(4)[0].message.contains("ResourceNotFoundException"), "{d:?}");
        assert!(at(5)[0].message.contains("NO-SUCH-FIELD"), "{d:?}");
        assert!(at(6).is_empty(), "a keyed read is clean: {d:?}");
        assert!(at(7)[0].severity == Severity::Info && at(7)[0].message.contains("scan"), "{d:?}");
        assert!(at(8).is_empty(), "a statement in a data item is not checked: {d:?}");
        assert!(!dir.join("data").exists(), "no data file was touched");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
