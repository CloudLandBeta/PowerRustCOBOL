// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `check` — the IDE's Check and Build gate, read-only.
//!
//! A form's program is generated and validated **in memory**: nothing is
//! written. A diagnostic in it is reported where the developer (or the agent)
//! can fix it — the `.cfrm`, the code site and the line inside that site's own
//! text (spec 053's source map) — and only a line codegen itself authored
//! keeps the generated file's name.

use std::path::Path;

use serde_json::{json, Value};

use crate::binding_guardian::{validate_binding_action, BindingActionGate};
use crate::gen_paths;
use crate::host::ProjectHost;
use crate::root::ProjectRoot;
use crate::validate_source::{self, Severity};

/// One diagnostic, located for the reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Project-relative file the problem is in.
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub severity: Severity,
    pub message: String,
    /// The form's name, for a diagnostic inside a form's code.
    pub form: Option<String>,
    /// The code site (`Form ▸ Control ▸ onClick`), for the same.
    pub site: Option<String>,
}

impl Finding {
    fn at(file: &str, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            file: file.to_owned(),
            line: 0,
            col: 0,
            severity,
            message: message.into(),
            form: None,
            site: None,
        }
    }

    pub fn to_json(&self) -> Value {
        let mut v = json!({
            "file": self.file,
            "line": self.line,
            "col": self.col,
            "severity": self.severity.as_str(),
            "message": self.message,
        });
        if let Some(f) = &self.form {
            v["form"] = json!(f);
        }
        if let Some(s) = &self.site {
            v["site"] = json!(s);
        }
        v
    }
}

/// The result of a check: every finding, and what was looked at.
#[derive(Debug, Default)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub forms: usize,
    pub indexed: usize,
    pub sources: usize,
}

impl Report {
    pub fn errors(&self) -> usize {
        self.findings.iter().filter(|f| f.severity == Severity::Error).count()
    }

    pub fn to_json(&self) -> Value {
        json!({
            "errors": self.errors(),
            "warnings": self.findings.iter().filter(|f| f.severity == Severity::Warning).count(),
            "checked": { "forms": self.forms, "indexed": self.indexed, "sources": self.sources },
            "diagnostics": self.findings.iter().map(Finding::to_json).collect::<Vec<_>>(),
        })
    }
}

pub fn run<H: ProjectHost>(host: &H, root: &ProjectRoot, path: Option<&str>) -> Result<Value, String> {
    let report = match path {
        None => check_project(host, root)?,
        Some(rel) => check_one(host, root, rel)?,
    };
    Ok(report.to_json())
}

/// The whole project.
pub fn check_project<H: ProjectHost>(host: &H, root: &ProjectRoot) -> Result<Report, String> {
    let view = cobolt_compiler::project_manifest_view(root.manifest())?;
    let crates = host.external_crates();
    let mut report = Report::default();
    for rel in &view.forms {
        check_form(root, &view, rel, &crates, &mut report);
    }
    for rel in &view.indexed {
        check_indexed(root, rel, &mut report);
    }
    for rel in &view.sources {
        check_source(root, rel, &crates, &mut report);
    }
    // Exactly one main form (spec 037).
    if let Err(e) = cobolt_compiler::main_form_guard::read_designation(root.dir(), &view.forms) {
        report
            .findings
            .push(Finding::at(&root.manifest_name(), Severity::Error, e));
    }
    Ok(report)
}

/// One file, chosen by its extension.
pub fn check_one<H: ProjectHost>(host: &H, root: &ProjectRoot, rel: &str) -> Result<Report, String> {
    let abs = root.resolve(rel)?;
    if !abs.is_file() {
        return Err(format!("'{rel}' does not exist"));
    }
    let rel = root.relative(&abs).unwrap_or_else(|| rel.to_owned());
    let view = cobolt_compiler::project_manifest_view(root.manifest())?;
    let crates = host.external_crates();
    let mut report = Report::default();
    match ext(&rel).as_str() {
        "cfrm" => check_form(root, &view, &rel, &crates, &mut report),
        "cidx" => check_indexed(root, &rel, &mut report),
        "cbl" | "cob" | "cpy" => check_source(root, &rel, &crates, &mut report),
        _ => return Err(format!("'{rel}' is not a form, an indexed definition or COBOL source")),
    }
    Ok(report)
}

fn ext(rel: &str) -> String {
    Path::new(rel)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn check_form(
    root: &ProjectRoot,
    view: &cobolt_compiler::ManifestView,
    rel: &str,
    crates: &[String],
    report: &mut Report,
) {
    let Ok(abs) = root.resolve(rel) else {
        report
            .findings
            .push(Finding::at(rel, Severity::Error, "the manifest lists a path outside the project"));
        return;
    };
    report.forms += 1;
    let form = match cobolt_forms::load_form(&abs) {
        Ok(f) => f,
        Err(e) => {
            report
                .findings
                .push(Finding::at(rel, Severity::Error, format!("the form cannot be loaded: {e}")));
            return;
        }
    };
    let program = gen_paths::generated_cbl_path(Some(&view.generated), Some(root.dir()), &abs);
    let gen_rel = root
        .relative(&program)
        .unwrap_or_else(|| format!("generated/{}", program.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    let (diags, _src, map) =
        validate_source::validate_form_source(&form, &program, Some(crates.to_vec()));
    for d in diags {
        let located = (d.line > 0).then(|| map.resolve(d.line)).flatten();
        report.findings.push(match located {
            // The developer's own code: the form, the site, the line inside it.
            Some((site, site_line)) => Finding {
                file: rel.to_owned(),
                line: site_line,
                col: d.col,
                severity: d.severity,
                message: d.message,
                form: Some(form.name.clone()),
                site: Some(site.display_path(&form.name)),
            },
            // A line codegen authored: named in the generated file.
            None => Finding {
                file: gen_rel.clone(),
                line: d.line,
                col: d.col,
                severity: d.severity,
                message: d.message,
                form: Some(form.name.clone()),
                site: None,
            },
        });
    }
    // The data-binding gate the IDE's Check/Build applies.
    let gate = validate_binding_action(&form, BindingActionGate::CheckProject);
    for f in gate.findings {
        let severity = match f.severity {
            cobolt_forms::GuardianSeverity::Blocker => Severity::Error,
            cobolt_forms::GuardianSeverity::Warning => Severity::Warning,
            cobolt_forms::GuardianSeverity::Info => Severity::Info,
        };
        let mut finding = Finding::at(
            rel,
            severity,
            format!("data binding {}: {} ({})", f.binding_id, f.message, f.code),
        );
        finding.form = Some(form.name.clone());
        report.findings.push(finding);
    }
}

fn check_indexed(root: &ProjectRoot, rel: &str, report: &mut Report) {
    let Ok(abs) = root.resolve(rel) else {
        report
            .findings
            .push(Finding::at(rel, Severity::Error, "the manifest lists a path outside the project"));
        return;
    };
    report.indexed += 1;
    for (severity, message) in indexed_findings(&abs) {
        report.findings.push(Finding::at(rel, severity, message));
    }
}

/// What `validate` and `check` say about one `.cidx`.
pub fn indexed_findings(abs: &Path) -> Vec<(Severity, String)> {
    let def = match cobolt_indexed::load_indexed(abs) {
        Ok(d) => d,
        Err(e) => return vec![(Severity::Error, format!("the definition cannot be loaded: {e}"))],
    };
    let mut out = Vec::new();
    if let Err(e) = cobolt_indexed::validate_definition(&def) {
        out.push((Severity::Error, e));
    }
    for w in cobolt_indexed::finalize_warnings(&def) {
        out.push((Severity::Warning, w));
    }
    out
}

fn check_source(root: &ProjectRoot, rel: &str, crates: &[String], report: &mut Report) {
    let Ok(abs) = root.resolve(rel) else {
        report
            .findings
            .push(Finding::at(rel, Severity::Error, "the manifest lists a path outside the project"));
        return;
    };
    let Ok(text) = std::fs::read_to_string(&abs) else {
        // A listed source that is missing is `list_files`' business; there is
        // nothing to check.
        return;
    };
    report.sources += 1;
    for d in validate_source::validate_source(&abs, &text, Some(crates.to_vec())) {
        report.findings.push(Finding {
            file: rel.to_owned(),
            line: d.line,
            col: d.col,
            severity: d.severity,
            message: d.message,
            form: None,
            site: None,
        });
    }
}
