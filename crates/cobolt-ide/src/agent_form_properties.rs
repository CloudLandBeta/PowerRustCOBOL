// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `form.properties` — what the in-IDE agents are told about the values a Web or Mobile form's
//! stylesheets decide (WASM-036 R22). Read-only: it answers, it never writes (a change goes through
//! the change-set and the panel's own path, T18).
//!
//! The main thread publishes where the open designer's form is (its model, its file, the project's
//! stylesheet, the colour scheme the designer shows); the tool, which runs on a worker thread, reads
//! the stylesheets from disk when it is called, so what it answers is never older than the files.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use cobolt_forms::css::{MediaEnv, Scheme};
use cobolt_forms::css_effective::{Origin, SheetSource};
use cobolt_forms::Form;

/// What the main thread publishes about the open Web or Mobile form.
#[derive(Debug, Clone)]
pub struct Published {
    pub form: Form,
    /// The form's file; its stylesheet is beside it.
    pub form_file: PathBuf,
    /// The project folder, and the project stylesheet's path in it.
    pub project_dir: PathBuf,
    pub project_sheet: Option<String>,
    pub theme_id: String,
    pub width: f32,
    pub height: f32,
    pub scheme: Scheme,
}

fn slot() -> &'static Mutex<Option<Published>> {
    static S: OnceLock<Mutex<Option<Published>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// Publish the open form (a Web or Mobile one), or `None` when no such designer is open.
pub fn publish(p: Option<Published>) {
    if let Ok(mut s) = slot().lock() {
        *s = p;
    }
}

fn published() -> Option<Published> {
    slot().lock().ok().and_then(|s| s.clone())
}

/// The project's and the form's stylesheets as they are on disk now: `(label, text, origin)`.
pub fn read_sheets(project_dir: &Path, project_sheet: Option<&str>, form_file: &Path) -> Vec<(String, String, Origin)> {
    let mut out = Vec::new();
    let label = |p: &Path| p.strip_prefix(project_dir).unwrap_or(p).to_string_lossy().replace('\\', "/");
    let mut add = |path: PathBuf, origin: Origin| {
        if let Ok(b) = std::fs::read(&path) {
            out.push((label(&path), String::from_utf8_lossy(&b).into_owned(), origin));
        }
    };
    if let Some(rel) = project_sheet {
        add(project_dir.join(rel), Origin::Project);
    }
    add(cobolt_forms::web_themes::stylesheet_beside(form_file), Origin::Form);
    out
}

fn env_of(p: &Published) -> MediaEnv {
    MediaEnv { width: p.width, height: p.height, scheme: p.scheme }
}

/// `form.properties` — `{"control": "<name>"}`, or `{}` for the form.
pub fn observe(args: &serde_json::Value) -> crate::tool_exec::ToolResult {
    use crate::tool_exec::ToolResult;
    let Some(p) = published() else {
        return ToolResult::err(
            "no Web or Mobile form is open in a designer",
            "form.properties answers only while a Web or Mobile form is open in the designer; a Desktop form has no stylesheet.".to_string(),
        );
    };
    let sheets = read_sheets(&p.project_dir, p.project_sheet.as_deref(), &p.form_file);
    let sources: Vec<SheetSource<'_>> = sheets.iter().map(|(label, text, origin)| SheetSource { label, text, origin: *origin }).collect();
    let target = args.get("control").and_then(|v| v.as_str());
    match cobolt_forms::css_effective::properties_report(&p.form, &sources, &p.theme_id, &env_of(&p), target) {
        Ok(v) => ToolResult::ok(
            format!("read the properties of {}", target.unwrap_or("the form")),
            serde_json::to_string_pretty(&v).unwrap_or_default(),
        ),
        Err(e) => ToolResult::err(e.clone(), e),
    }
}

/// The context's override lines for `form`, from the published designer when it is this form,
/// otherwise from the files on disk at the form's size in Light. Empty when no stylesheet decides
/// anything.
pub fn context_lines(form: &Form, form_file: Option<&Path>, project_dir: Option<&Path>, project_sheet: Option<&str>, theme_id: &str) -> String {
    let (Some(form_file), Some(project_dir)) = (form_file, project_dir) else { return String::new() };
    let (w, h, scheme) = match published().filter(|p| p.form.name == form.name) {
        Some(p) => (p.width, p.height, p.scheme),
        None => (form.width as f32, form.height as f32, Scheme::Light),
    };
    let sheets = read_sheets(project_dir, project_sheet, form_file);
    let sources: Vec<SheetSource<'_>> = sheets.iter().map(|(label, text, origin)| SheetSource { label, text, origin: *origin }).collect();
    let env = MediaEnv { width: w, height: h, scheme };
    let lines = cobolt_forms::css_effective::override_lines(form, &sources, theme_id, &env);
    if lines.is_empty() {
        return lines;
    }
    format!(
        "STYLESHEET OVERRIDES ({} scheme, {}x{}): a value below is what the page uses; setting the property in the form file does not change it while the rule stays. Read `form.properties` for every mapped property.\n{lines}",
        if scheme == Scheme::Dark { "dark" } else { "light" },
        w as i64,
        h as i64,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::model::{Control, ControlType};

    /// The published slot is process-wide: the tests that use it take turns.
    fn turn() -> std::sync::MutexGuard<'static, ()> {
        static L: Mutex<()> = Mutex::new(());
        L.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn fixture(tag: &str) -> (PathBuf, Published) {
        let dir = std::env::temp_dir().join(format!("prc-form-props-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("webforms")).unwrap();
        std::fs::write(dir.join("webforms/application.css"), ".pr-button { color: #112233 }\n").unwrap();
        std::fs::write(
            dir.join("webforms/main-form.css"),
            "#BTN-1 { background-color: #445566 }\n@media (prefers-color-scheme: dark) { #BTN-1 { background-color: #111111 } }\n",
        )
        .unwrap();
        let mut form = Form::new("MAIN-FORM", "Main", 400, 300);
        form.controls.push(Control::new("BTN-1", ControlType::Button, 10, 10));
        form.controls.push(Control::new("BTN-2", ControlType::Button, 10, 60));
        let p = Published {
            form,
            form_file: dir.join("webforms/main-form.wcfrm"),
            project_dir: dir.clone(),
            project_sheet: Some("webforms/application.css".into()),
            theme_id: cobolt_forms::theme::resolve_theme_id(None, None),
            width: 400.0,
            height: 300.0,
            scheme: Scheme::Light,
        };
        (dir, p)
    }

    fn row<'a>(v: &'a serde_json::Value, property: &str) -> &'a serde_json::Value {
        v["properties"].as_array().unwrap().iter().find(|r| r["property"] == property).unwrap_or_else(|| panic!("no row for {property}"))
    }

    /// WASM-036 R22 / AC18: `form.properties` answers every mapped property of a control and of the
    /// form with the stored value, the value in effect and where it comes from, in Light and in Dark.
    #[test]
    fn form_properties_gives_stored_effective_and_source_in_light_and_dark() {
        let _turn = turn();
        let t0 = std::time::Instant::now();
        let (dir, mut p) = fixture("tool");
        let before = observe(&serde_json::json!({}));
        publish(None);
        let none = observe(&serde_json::json!({ "control": "BTN-1" }));
        assert!(!none.ok, "nothing is open: an error that says so");
        let _ = before;
        publish(Some(p.clone()));
        let mut cases = 0;
        for (scheme, bg, bg_src) in [(Scheme::Light, "#445566ff", "webforms/main-form.css:1"), (Scheme::Dark, "#111111ff", "webforms/main-form.css:2")] {
            p.scheme = scheme;
            publish(Some(p.clone()));
            for control in ["BTN-1", "BTN-2"] {
                let r = observe(&serde_json::json!({ "control": control }));
                assert!(r.ok, "{}", r.detail);
                let v: serde_json::Value = serde_json::from_str(&r.detail).unwrap();
                assert_eq!(v["target"], control);
                assert_eq!(v["scheme"], if scheme == Scheme::Dark { "dark" } else { "light" });
                assert!(v["properties"].as_array().unwrap().len() >= 10, "every mapped property");
                let fg = row(&v, "ForegroundColor");
                assert_eq!((fg["effective"].as_str(), fg["source"].as_str()), (Some("#112233ff"), Some("webforms/application.css:1")), "the project rule, both buttons");
                let b = row(&v, "BackgroundColor");
                if control == "BTN-1" {
                    assert_eq!((b["effective"].as_str(), b["source"].as_str()), (Some(bg), Some(bg_src)), "{scheme:?}");
                } else {
                    assert_ne!(b["source"].as_str(), Some(bg_src), "BTN-2 is not named by that rule");
                }
                assert!(b.get("stored").is_some(), "the stored value is always there");
                cases += 1;
            }
            let f = observe(&serde_json::json!({}));
            let v: serde_json::Value = serde_json::from_str(&f.detail).unwrap();
            assert_eq!(v["target"], "Form");
            assert!(row(&v, "BackgroundColor")["stored"].is_string(), "the form's own stored background");
            cases += 1;
        }
        assert!(!observe(&serde_json::json!({ "control": "NOPE" })).ok, "an unknown control is named as not found");
        publish(None);
        let _ = std::fs::remove_dir_all(&dir);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        println!("┌─ form.properties: the read-only tool\n│ {cases} answers (2 buttons + the form, Light and Dark): stored, effective, source; nothing open -> error; unknown control -> error\n│ time: {ms:.1} ms\n└─ {cases}/{cases} passed, 0 failed");
    }

    /// The context lists the overrides for a Web or Mobile form, and says nothing when no sheet decides anything.
    #[test]
    fn the_context_lists_the_overrides_and_the_contract_says_the_tool_is_read_only() {
        let _turn = turn();
        let (dir, p) = fixture("ctx");
        publish(None);
        let lines = context_lines(&p.form, Some(&p.form_file), Some(&dir), Some("webforms/application.css"), &p.theme_id);
        println!("{lines}");
        assert!(lines.starts_with("STYLESHEET OVERRIDES (light scheme, 400x300)"), "{lines}");
        assert!(lines.contains("BTN-1: ") && lines.contains("BackgroundColor=#445566ff (webforms/main-form.css:1)") && lines.contains("ForegroundColor=#112233ff (webforms/application.css:1)"), "{lines}");
        assert!(lines.contains("BTN-2: ForegroundColor=#112233ff"), "{lines}");
        std::fs::write(dir.join("webforms/main-form.css"), "").unwrap();
        std::fs::write(dir.join("webforms/application.css"), "").unwrap();
        assert!(context_lines(&p.form, Some(&p.form_file), Some(&dir), Some("webforms/application.css"), &p.theme_id).is_empty(), "no override, no section");
        let declared: std::collections::HashSet<String> = ["form.properties".to_string()].into();
        let c = crate::tool_exec::tool_contract_appendix(&declared);
        assert!(c.contains("form_properties") && c.to_lowercase().contains("read-only"));
        assert!(crate::tool_exec::tool_contract_appendix(&Default::default()).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
