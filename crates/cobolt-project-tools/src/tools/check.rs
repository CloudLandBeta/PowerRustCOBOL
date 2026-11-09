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

use crate::binding_guardian::{validate_binding_action_in, BindingActionGate};
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
    pub(crate) fn at(file: &str, severity: Severity, message: impl Into<String>) -> Self {
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
    // The Backend of a Web or Mobile project is COBOL like any other
    // (WASM-001 R12): checked with the rest.
    for rel in &view.backend {
        check_source(root, rel, &crates, &mut report);
    }
    // `[backend] state` is one of two spellings (WASM-003 Q2).
    if let Err(e) = view.backend_state_mode() {
        report.findings.push(Finding::at(&root.manifest_name(), Severity::Error, e));
    }
    // Exactly one main form (spec 037).
    if let Err(e) = cobolt_compiler::main_form_guard::read_designation(root.dir(), &view.forms) {
        report
            .findings
            .push(Finding::at(&root.manifest_name(), Severity::Error, e));
    }
    // A Web or Mobile project: what a browser cannot run is an error at its
    // line (WASM-002 R3), refused by the generator the Build runs. A form that
    // cannot be read was reported above; a program without the generator
    // linked checks the rest as before.
    if !view.kind.is_desktop() {
        if let Ok(req) = crate::web::WebBuildRequest::from_manifest(root.manifest()) {
            match crate::web::backend().check_with_warnings(&req) {
                Err(crate::web::WebBuildError::Refused(lines)) => {
                    report.findings.extend(lines.iter().map(|d| web_finding(d, Severity::Error)));
                }
                // WASM-004 T34: what Check warns about (a control a screen reader cannot name).
                Ok(warnings) => report.findings.extend(warnings.iter().map(|d| web_finding(d, Severity::Warning))),
                Err(_) => {}
            }
        }
    }
    Ok(report)
}

/// A refusal of the web build as a finding: `webforms/a.wcfrm (A ▸ BTN ▸ onClick):8`
/// gives the file, the code site, the form and the line.
fn web_finding(d: &crate::web::WebDiagnostic, severity: Severity) -> Finding {
    let (place, line) = match d.location.rsplit_once(':') {
        Some((p, n)) if n.parse::<u32>().is_ok() => (p, n.parse().unwrap_or(0)),
        _ => (d.location.as_str(), 0),
    };
    let (file, site) = match place.split_once(" (") {
        Some((f, s)) => (f, s.strip_suffix(')').map(str::to_owned)),
        None => (place, None),
    };
    let mut f = Finding::at(file, severity, d.message.clone());
    f.line = line;
    f.form = site
        .as_deref()
        .and_then(|s| s.split(" ▸ ").next())
        .map(str::to_owned);
    f.site = site;
    f
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
        e if cobolt_compiler::project_kind::is_form_extension(e) => check_form(root, &view, &rel, &crates, &mut report),
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
    if let Some(w) = form_name_mismatch(rel, view.kind) {
        report.findings.push(Finding::at(rel, Severity::Warning, w));
    }
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
    // "Definition for AI": one warning for the elements that say nothing, or too little, about themselves.
    if let Some(f) = crate::tools::definitions::missing_finding(&form, rel) {
        report.findings.push(f);
    }
    // The data-binding gate the IDE's Check/Build applies.
    let gate = validate_binding_action_in(&form, BindingActionGate::CheckProject, root.dir());
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
    // WASM-036 R21: what the stylesheets decide, as notes. Nothing here changes the result.
    if !view.kind.is_desktop() {
        css_notes(root, view, rel, &abs, &form, report);
    }
}

/// WASM-036 R21 — a Web or Mobile form's stylesheet overrides, as `Info` notes: one per certain
/// override (control, property, effective value, the file and line that wins, and the scheme or
/// schemes it holds in) and one per selector the designer's subset cannot evaluate. Both schemes are
/// evaluated only when a sheet mentions `prefers-color-scheme`.
fn css_notes(root: &ProjectRoot, view: &cobolt_compiler::ManifestView, rel: &str, abs: &Path, form: &cobolt_forms::Form, report: &mut Report) {
    use cobolt_forms::css::{MediaEnv, Scheme};
    use cobolt_forms::css_effective::{effective, Layer, Origin, SheetSet, SheetSource};
    let project_rel = match (&view.web, &view.mobile) {
        (Some(w), _) => Some(w.stylesheet.clone()),
        (None, Some(m)) => Some(m.stylesheet.clone()),
        _ => None,
    };
    let mut sheets: Vec<(String, String, Origin)> = Vec::new();
    for (path, origin) in [
        (project_rel.and_then(|r| root.resolve(&r).ok()), Origin::Project),
        (Some(cobolt_forms::web_themes::stylesheet_beside(abs)), Origin::Form),
    ] {
        let Some(path) = path.filter(|p| p.is_file()) else { continue };
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let label = root.relative(&path).unwrap_or_else(|| path.to_string_lossy().into_owned());
        sheets.push((label, String::from_utf8_lossy(&bytes).into_owned(), origin));
    }
    if sheets.is_empty() {
        return;
    }
    let sources: Vec<SheetSource<'_>> = sheets.iter().map(|(label, text, origin)| SheetSource { label, text, origin: *origin }).collect();
    let set = SheetSet::parse(&sources);
    let both = sheets.iter().any(|(_, t, _)| t.contains("prefers-color-scheme"));
    let schemes: &[(Scheme, &str)] = if both { &[(Scheme::Light, "light"), (Scheme::Dark, "dark")] } else { &[(Scheme::Light, "light")] };
    // (control, property, file:line, value) -> the schemes it holds in.
    let mut notes: Vec<(String, String, String, String, Vec<&str>)> = Vec::new();
    fn walk(cs: &[cobolt_forms::Control], out: &mut Vec<String>) {
        for c in cs {
            out.push(c.id.clone());
            walk(&c.children, out);
        }
    }
    let mut ids = Vec::new();
    walk(&form.controls, &mut ids);
    for (scheme, name) in schemes {
        let env = MediaEnv { width: form.width as f32, height: form.height as f32, scheme: *scheme };
        for id in &ids {
            for e in effective(form, Some(id), &set, &env).entries.iter().filter(|e| e.layer >= Layer::Project) {
                let at = match &e.sheet {
                    Some(f) => format!("{f}:{}", e.line),
                    None => "CssStyle".to_owned(),
                };
                let value = e.value.to_property_text();
                match notes.iter_mut().find(|n| n.0 == *id && n.1 == e.property && n.2 == at && n.3 == value) {
                    Some(n) => n.4.push(name),
                    None => notes.push((id.clone(), e.property.to_owned(), at, value, vec![name])),
                }
            }
        }
    }
    for (id, property, at, value, in_schemes) in notes {
        report.findings.push(Finding::at(
            rel,
            Severity::Info,
            format!("stylesheet: {id} {property} is {value} from {at} [{}] (the stored value is not used there)", in_schemes.join(", ")),
        ));
    }
    for r in &set.rules {
        // Unreadable, or simplified past a pseudo-class with arguments (`:has()`, `:is()`, `:not()`): the page
        // may apply it where the designer cannot tell. A plain state (`:hover`) is "may" by design, not noted.
        for sel in r.selectors.iter().filter(|s| !s.readable() || (s.simplified && s.text.contains('('))) {
            let file = &sheets[r.source].0;
            report.findings.push(Finding::at(
                rel,
                Severity::Info,
                format!("stylesheet: {file}:{} the selector `{}` cannot be evaluated by the designer; the page may still apply it", r.line, sel.text),
            ));
        }
    }
}

/// WASM-001 Q2, plan D-T15b/c: a form whose file name belongs to another kind
/// (a `.cfrm` made by 2.0.x in a Web project, a `.wcfrm` copied into a Desktop
/// one) still loads and is checked; this warning says so. Nothing is renamed.
fn form_name_mismatch(rel: &str, kind: cobolt_compiler::ProjectKind) -> Option<String> {
    let path = Path::new(rel);
    let named = cobolt_compiler::ProjectKind::of_form_extension(path.extension()?.to_str()?)?;
    if named == kind {
        return None;
    }
    let stem = path.file_stem()?.to_string_lossy();
    Some(format!(
        "'{rel}' is a {} form file name in a {} project; rename it to {stem}.{} (the form loads as it is)",
        named.as_str(),
        kind.as_str(),
        kind.form_extension()
    ))
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
    // A copybook is a fragment with no divisions of its own: it is checked through the
    // programs that `COPY` it, and read alone it is always an error.
    if ext(rel) == "cpy" {
        return;
    }
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

#[cfg(test)]
mod copybook_tests {
    use super::*;

    /// A copybook listed in the manifest (a Backend's `backend/COPYBOOKS/*.cpy`) is a fragment:
    /// it is checked through the programs that `COPY` it, never alone.
    #[test]
    fn a_listed_copybook_is_not_checked_as_a_program() {
        let dir = std::env::temp_dir().join(format!("prc-check-cpy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("backend/COPYBOOKS")).unwrap();
        std::fs::write(
            dir.join("Shop.project.toml"),
            "[project]\nname = \"Shop.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n[files]\nbackend = [\"backend/COPYBOOKS/rec.cpy\"]\n",
        )
        .unwrap();
        std::fs::write(dir.join("backend/COPYBOOKS/rec.cpy"), "       01 REC.\n          05 A PIC X.\n").unwrap();
        let root = ProjectRoot::open(&dir).unwrap();
        let mut report = Report::default();
        check_source(&root, "backend/COPYBOOKS/rec.cpy", &[], &mut report);
        assert!(report.findings.is_empty(), "{:?}", report.findings);
        assert_eq!(report.sources, 0, "it is not counted as a checked program");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod css_notes_tests {
    use super::*;

    /// WASM-036 R21 / AC17: a Web form's stylesheet overrides are `Info` notes — one per certain
    /// override with file, line and scheme, one per selector the designer cannot evaluate — and they
    /// change neither the errors nor the warnings of the report.
    #[test]
    fn the_overrides_are_notes_with_file_line_and_scheme_and_change_no_result() {
        let t0 = std::time::Instant::now();
        let dir = std::env::temp_dir().join(format!("prc-check-css-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::create::create_project_with(&dir, &crate::create::CreateRequest::new("Shop").template("web")).unwrap();
        let root = ProjectRoot::open(&dir).unwrap();
        let view = cobolt_compiler::project_manifest_view(root.manifest()).unwrap();
        let rel = view.forms[0].clone();
        let abs = root.resolve(&rel).unwrap();
        let form = cobolt_forms::load_form(&abs).unwrap();
        let button = form.controls.iter().find(|c| c.control_type.as_str() == "Button").expect("the starter form has a button").id.clone();
        let sheet = cobolt_forms::web_themes::stylesheet_beside(&abs);
        let sheet_rel = root.relative(&sheet).unwrap();
        let original = std::fs::read_to_string(&sheet).unwrap();
        let notes = |extra: &str| {
            std::fs::write(&sheet, format!("{original}{extra}")).unwrap();
            let mut r = Report::default();
            css_notes(&root, &view, &rel, &abs, &form, &mut r);
            r
        };
        // Without anything of the developer's: whatever the generated theme sheet decides is listed, and nothing else.
        let before = notes("").findings.len();
        let extra = format!("\n#{button} {{ background-color: #112233 }}\n@media (prefers-color-scheme: dark) {{ #{button} {{ background-color: #111111 }} }}\n.pr-button:has(> .x) {{ color: red }}\n");
        let r = notes(&extra);
        for f in &r.findings {
            println!("{:?} {}: {}", f.severity, f.file, f.message);
            assert_eq!(f.severity, Severity::Info, "a note, never a warning or an error");
        }
        let line_of = |needle: &str| original.lines().count() + 1 + extra.lines().position(|l| l.contains(needle)).unwrap();
        let bg: Vec<&Finding> = r.findings.iter().filter(|f| f.message.contains(&format!("{button} BackgroundColor"))).collect();
        assert!(bg.iter().any(|f| f.message.contains(&format!("#112233ff from {sheet_rel}:{}", line_of("#112233")) ) && f.message.contains("[light]")), "{bg:?}");
        assert!(bg.iter().any(|f| f.message.contains("#111111ff") && f.message.contains("[dark]")), "{bg:?}");
        let has: Vec<&Finding> = r.findings.iter().filter(|f| f.message.contains(":has(")).collect();
        assert_eq!(has.len(), 1, "one note for the :has() selector: {has:?}");
        assert!(has[0].message.contains(&format!("{sheet_rel}:{}", line_of(":has("))));
        assert!(r.findings.len() > before);
        assert_eq!((r.errors(), r.findings.iter().filter(|f| f.severity == Severity::Warning).count()), (0, 0), "the result is unchanged");
        // Only light is evaluated when no sheet says prefers-color-scheme.
        let light_only = notes("\n#X { color: red }\n");
        assert!(light_only.findings.iter().all(|f| !f.message.contains("[dark]") && !f.message.contains("light, dark")));
        let _ = std::fs::remove_dir_all(&dir);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        println!("┌─ check: stylesheet notes\n│ light and dark override notes with file:line, one :has() note, all Info, errors/warnings unchanged, light-only without a dark query\n│ time: {ms:.1} ms\n└─ 7/7 passed, 0 failed");
    }
}
