// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The browser runner** (WASM-015 R2/R9): build a web project, serve it on loopback, open a form in the
//! system web view's headless helper at a preset size, run [`Step`]s on the page and answer with what they
//! read, what the page said, the errors, a picture and the engine. It is the library the test crate and the
//! agents' `run_form` share; the helper is the one the Preview ships (no WebDriver, no Chrome, no `eval`).
//!
//! A machine without a web view (no helper, no display) is not a failure of the code under test: the runner
//! answers [`RunnerError::NoWebView`] with the reason, and the caller skips by name (or fails, under
//! `PRC_REQUIRE_WEBVIEW=1`).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::root::ProjectRoot;
use crate::script::Step;
use crate::tools::web_form;
use crate::web::preview;

/// Why a run did not happen or did not finish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerError {
    /// The machine has no web view to drive; the reason says what is missing.
    NoWebView(String),
    /// Anything else: the build, the page, a step.
    Failed(String),
}

impl std::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunnerError::NoWebView(why) => write!(f, "no web view — {why}"),
            RunnerError::Failed(why) => f.write_str(why),
        }
    }
}

/// The web view that drew the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Engine {
    /// `webkit` (WKWebView, macOS), `webview2` (Windows) or `webkitgtk` (Linux).
    pub name: &'static str,
    /// `navigator.userAgent`, which carries the version.
    pub user_agent: String,
}

/// What a run produced.
#[derive(Debug, Clone)]
pub struct RunReport {
    /// The value each reading step answered, in order (steps that answer none give no entry).
    pub values: Vec<Value>,
    /// What the page wrote with `console.log`, one line each (the program's DISPLAY).
    pub displayed: Vec<String>,
    /// The page's errors (including `cobol-error`) and the steps that failed.
    pub errors: Vec<String>,
    /// The picture of where the script left the page.
    pub png: Option<Vec<u8>>,
    pub engine: Engine,
    /// Each step's name and the milliseconds it took.
    pub step_ms: Vec<(String, u128)>,
}

/// The engine's name on this system.
pub fn engine_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "webkit"
    } else if cfg!(target_os = "windows") {
        "webview2"
    } else {
        "webkitgtk"
    }
}

/// Where the helper is: the one the program ships beside itself, else beside the folder a test binary lives
/// in (`target/debug/deps/..`), else `PRC_WEB_PREVIEW`.
pub fn find_helper() -> Option<PathBuf> {
    // `PRC_WEB_PREVIEW` is authoritative: pointed nowhere, there is no helper (the skip is testable).
    if std::env::var_os(preview::HELPER_ENV).is_some() {
        return preview::helper_path();
    }
    if let Some(p) = preview::helper_path() {
        return Some(p);
    }
    let exe = std::env::current_exe().ok()?;
    let above = exe.parent()?.parent()?.join(preview::HELPER_NAME);
    above.is_file().then_some(above)
}

/// The step in the helper's vocabulary. The desktop spellings (`set`, `event`, `wait_ms`, `read`) are taken
/// as their browser equivalents; a step with none is refused with why.
pub fn to_helper(step: &Step) -> Result<Value, String> {
    Ok(match step {
        Step::Set { control, property, value } if matches!(property.to_ascii_lowercase().as_str(), "text" | "value") => {
            json!({"type": {"control": control, "text": value}})
        }
        Step::Set { property, .. } => return Err(format!("on a web form a step sets only Text (by typing); {property} is set by the program")),
        Step::Event { control, name, .. } if name.eq_ignore_ascii_case("onClick") => json!({"click": {"control": control}}),
        Step::Event { name, .. } => return Err(format!("on a web form a step raises only onClick (a real click); {name} is not raised")),
        Step::WaitMs(ms) => json!({"wait": {"ms": ms}}),
        Step::Read { control, property } => json!({"get": {"control": control, "property": property}}),
        Step::Dump => return Err("dump is not available for a web form: read each property".into()),
        Step::OpenForm(_) => return Err("open_form is not available for a web form yet".into()),
        other => other.to_json(),
    })
}

/// Does the step answer a value?
fn answers_a_value(helper_step: &Value) -> bool {
    helper_step.as_object().and_then(|o| o.keys().next()).is_some_and(|k| ["get", "text", "style", "rect", "focused", "snapshot", "page"].contains(&k.as_str()))
}

/// Open `form` of the project at its own size (or `window`, in CSS pixels), run `steps` on it and report.
/// The page is waited for first: nothing runs before the form is on it.
pub fn run(root: &ProjectRoot, form: &str, window: Option<[f32; 2]>, steps: &[Step], limit_s: u32) -> Result<RunReport, RunnerError> {
    let helper = find_helper().ok_or_else(|| {
        RunnerError::NoWebView(format!("the helper {} is not installed beside this program (set PRC_WEB_PREVIEW)", preview::HELPER_NAME))
    })?;
    let (mut page, _form) = web_form::open_with(&helper, root, form, window, 1.0).map_err(|e| {
        if e.contains("web view") || e.contains("helper") {
            RunnerError::NoWebView(e)
        } else {
            RunnerError::Failed(e)
        }
    })?;
    let limit = limit_s.clamp(1, 120);
    let mut errors: Vec<String> = Vec::new();
    let mut displayed: Vec<String> = Vec::new();
    let mut values: Vec<Value> = Vec::new();
    let mut step_ms: Vec<(String, u128)> = Vec::new();

    let started = Instant::now();
    let (_, text, errs) = page.session.script(vec![web_form::form_on_the_page()], limit).map_err(RunnerError::Failed)?;
    if !errs.is_empty() {
        return Err(RunnerError::Failed(format!("the form did not appear on the page: {}", errs.join("; "))));
    }
    displayed.extend(text.lines().map(str::to_owned));
    step_ms.push(("form on the page".into(), started.elapsed().as_millis()));

    // The engine: the user agent, read like any other page property.
    let (ua, _, _) = page.session.script(vec![json!({"page": {"property": "navigator.userAgent"}})], limit).map_err(RunnerError::Failed)?;
    let user_agent = ua.first().and_then(Value::as_str).unwrap_or_default().to_owned();

    for step in steps {
        let helper_step = to_helper(step).map_err(RunnerError::Failed)?;
        let at = Instant::now();
        let (got, text, errs) = page.session.script(vec![helper_step.clone()], limit).map_err(RunnerError::Failed)?;
        step_ms.push((step.name().to_owned(), at.elapsed().as_millis()));
        if answers_a_value(&helper_step) {
            values.push(got.into_iter().next().unwrap_or(Value::Null));
        }
        displayed.extend(text.lines().map(str::to_owned));
        errors.extend(errs);
    }

    let png = web_form::picture(&mut page).ok().map(|(bytes, _, _)| bytes);
    Ok(RunReport { values, displayed, errors, png, engine: Engine { name: engine_name(), user_agent }, step_ms })
}

/// A readable one-line-per-step timing table for a test's result block.
pub fn timing_lines(report: &RunReport) -> Vec<String> {
    report.step_ms.iter().map(|(name, ms)| format!("{name:<18} {ms:>6} ms")).collect()
}

/// How long the whole run took.
pub fn total(report: &RunReport) -> Duration {
    Duration::from_millis(report.step_ms.iter().map(|(_, ms)| *ms as u64).sum())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_steps_map_to_the_helpers_and_the_rest_is_refused_with_why() {
        let set = Step::Set { control: "TXT".into(), property: "Text".into(), value: "Ana".into() };
        assert_eq!(to_helper(&set).unwrap(), json!({"type": {"control": "TXT", "text": "Ana"}}));
        let click = Step::Event { control: "BTN".into(), name: "onClick".into(), wait: true };
        assert_eq!(to_helper(&click).unwrap(), json!({"click": {"control": "BTN"}}));
        assert_eq!(to_helper(&Step::WaitMs(5)).unwrap(), json!({"wait": {"ms": 5}}));
        assert_eq!(to_helper(&Step::Read { control: "L".into(), property: "Text".into() }).unwrap(), json!({"get": {"control": "L", "property": "Text"}}));
        assert_eq!(to_helper(&Step::Rect { control: "L".into() }).unwrap(), json!({"rect": {"control": "L"}}));
        assert!(to_helper(&Step::Dump).is_err() && to_helper(&Step::OpenForm("X".into())).is_err());
        assert!(to_helper(&Step::Event { control: "B".into(), name: "onChange".into(), wait: true }).unwrap_err().contains("only onClick"));
    }

    #[test]
    fn a_missing_helper_is_no_web_view_not_a_failure() {
        let e = RunnerError::NoWebView("the helper is not installed".into());
        assert_eq!(e.to_string(), "no web view — the helper is not installed");
    }
}
