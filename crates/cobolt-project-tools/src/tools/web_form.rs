// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **`render_form` and `run_form` for a web form** (WASM-008 T43, R9, R16):
//! a `.wcfrm` or `.mcfrm` is drawn by the system web view, headless — built
//! once for the call, served on loopback, opened in the helper
//! (`prc-web-preview`, [`crate::web::preview`]) and pictured or scripted there.
//! The answer is the desktop one plus `"engine"`. Without the helper the tool
//! says so: a web form is **never** pictured by the desktop engine.
//!
//! The build goes into a folder of its own under the system's temporary
//! folder, removed afterwards: the call writes nothing into the project, and
//! `dist/web` stays as the developer's last Build left it.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::host::ProjectHost;
use crate::root::ProjectRoot;
use crate::web::preview::{self, Emulation, PreviewSession};
use crate::web::{self, ServeOptions, ServerHandle, WebBuildRequest};

/// The engine that draws a web form on this system, as the answer names it.
pub const ENGINE: &str = if cfg!(target_os = "macos") {
    "webkit"
} else if cfg!(target_os = "windows") {
    "webview2"
} else {
    "webkitgtk"
};

/// Is `rel` a web or mobile form (drawn by the web view, not the desktop engine)?
pub fn is_web_form(rel: &str) -> bool {
    let ext = Path::new(rel).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    ext == "wcfrm" || ext == "mcfrm"
}

/// The first step of every script: the player draws the form after the page
/// has loaded, so nothing is read or pictured before it is there.
pub(crate) fn form_on_the_page() -> Value {
    json!({ "wait": { "control": "form" } })
}

/// A web form, built, served and open in the helper. Dropping it ends the
/// helper and the server and removes the build.
pub(crate) struct OpenPage {
    pub(crate) session: PreviewSession,
    _server: ServerHandle,
    pub(crate) work: PathBuf,
}

impl Drop for OpenPage {
    fn drop(&mut self) {
        self.session.close();
        let _ = std::fs::remove_dir_all(&self.work);
    }
}

fn open(root: &ProjectRoot, rel: &str, window: Option<[f32; 2]>, dpr: f32, dark: Option<bool>) -> Result<(OpenPage, cobolt_forms::Form), String> {
    let helper = preview::helper_path().ok_or_else(|| {
        format!(
            "a web form is drawn by the system web view through the headless helper {}, which is not installed beside \
             this program; it cannot be drawn here (a desktop picture of a web form would not be what the browser shows)",
            preview::HELPER_NAME
        )
    })?;
    open_with(&helper, root, rel, window, dpr, dark)
}

/// [`open`] with the helper given (the browser runner looks for it in more places than the IDE does).
pub(crate) fn open_with(helper: &std::path::Path, root: &ProjectRoot, rel: &str, window: Option<[f32; 2]>, dpr: f32, dark: Option<bool>) -> Result<(OpenPage, cobolt_forms::Form), String> {
    let abs = root.resolve(rel)?;
    if !abs.is_file() {
        return Err(format!("no form at {rel}"));
    }
    let form = cobolt_forms::load_form(&abs).map_err(|e| format!("{rel} does not load: {e}"))?;
    let backend = web::backend();
    if !backend.linked() {
        return Err(web::NOT_YET_LINKED_MESSAGE.to_owned());
    }
    let work = std::env::temp_dir().join(format!(
        "prc-web-form-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
    ));
    let mut request = WebBuildRequest::from_manifest(root.manifest())?;
    request.out_dir = work.join("web");
    let built = backend.build(&request).map_err(|e| {
        let _ = std::fs::remove_dir_all(&work);
        format!("the web build failed: {e}")
    });
    let report = built?;
    let server = backend
        .serve(&report.bundle_dir, &ServeOptions { port: 0, isolation: request.web.isolation.clone(), live_reload: false })
        .map_err(|e| {
            let _ = std::fs::remove_dir_all(&work);
            format!("the build could not be served: {e}")
        })?;
    let [w, h] = window.unwrap_or([form.width as f32, form.height as f32]);
    let (w, h) = ((w.round() as u32).max(64), (h.round() as u32).max(64));
    let url = format!("{}?pr-form={}", server.url(), form.name);
    let emulation = Emulation { dpr: Some(dpr), dark, ..Emulation::default() };
    let session = match PreviewSession::start(&url, w, h, &emulation, helper) {
        Ok(s) => s,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&work);
            return Err(e);
        }
    };
    let mut page = OpenPage { session, _server: server, work };
    let visible = page.session.ready(Duration::from_secs(30)).map_err(|e| format!("the web view did not start: {e}"))?;
    if visible == Some(true) {
        // R16: the helper is headless only; a window on show is refused, not used.
        return Err("the helper's window is on show on this system; a web form is drawn only with no window on show".to_owned());
    }
    Ok((page, form))
}

/// Picture the open page; the PNG's bytes and pixels.
pub(crate) fn picture(page: &mut OpenPage) -> Result<(Vec<u8>, u32, u32), String> {
    let png = page.work.join("picture.png");
    let (w, h) = page.session.render(&png, Duration::from_secs(30))?;
    let bytes = std::fs::read(&png).map_err(|e| e.to_string())?;
    Ok((bytes, w, h))
}

/// `render_form` for a web form: the page once the form is on it.
pub fn render(host: &impl ProjectHost, root: &ProjectRoot, rel: &str, scale: f32, window: Option<[f32; 2]>, dark: Option<bool>) -> Result<(Vec<u8>, Value), String> {
    let (mut page, _form) = open(root, rel, window, scale, dark)?;
    let (_, _, errors) = page.session.script(vec![form_on_the_page()], 30)?;
    if !errors.is_empty() {
        return Err(format!("the form did not appear on the page: {}", errors.join("; ")));
    }
    let (png, w, h) = picture(&mut page)?;
    let mut answer = json!({
        "form": rel,
        "width": w,
        "height": h,
        "scale": scale,
        "note": "As the system web view draws the built page once the form is on it (its opening handlers have run).",
        "engine": ENGINE,
    });
    if let Some(dark) = dark {
        // WASM-036 R17: the colour scheme the page's `prefers-color-scheme` answered.
        answer["color_scheme"] = json!(if dark { "dark" } else { "light" });
    }
    if window.is_some() {
        // The viewport the page was laid out in, in CSS pixels.
        answer["window"] = json!([(w as f32 / scale).round(), (h as f32 / scale).round()]);
    }
    if host.unsaved(&root.resolve(rel)?) {
        answer["unsaved"] = json!("the IDE holds unsaved edits to this form: the picture shows the saved file");
    }
    Ok((png, answer))
}

/// A `run_form` step → the helper's step, or why it has none on a web form.
/// The helper's own steps (`click`, `type`, `get`, …) are taken as they are.
pub fn translate(step: &Value) -> Result<Value, String> {
    let field = |obj: &Value, k: &str| obj.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    if let Some(set) = step.get("set") {
        let (c, p) = (field(set, "control"), field(set, "property"));
        return match p.to_ascii_lowercase().as_str() {
            "text" | "value" => Ok(json!({ "type": { "control": c, "text": field(set, "value") } })),
            _ => Err(format!("on a web form a step sets only Text (by typing into {c}); {p} is set by the program")),
        };
    }
    if let Some(ev) = step.get("event") {
        let (c, n) = (field(ev, "control"), field(ev, "name"));
        return if n.eq_ignore_ascii_case("onClick") {
            Ok(json!({ "click": { "control": c } }))
        } else {
            Err(format!("on a web form a step raises only onClick (a real click on {c}); {n} is not raised"))
        };
    }
    if let Some(ms) = step.get("wait_ms").and_then(Value::as_u64) {
        return Ok(json!({ "wait": { "ms": ms } }));
    }
    if let Some(read) = step.get("read") {
        return Ok(json!({ "get": { "control": field(read, "control"), "property": field(read, "property") } }));
    }
    if step.get("dump").is_some() {
        return Err("dump is not available for a web form: read each property with a read step".to_owned());
    }
    if step.get("open_form").is_some() {
        return Err("open_form is not available for a web form yet".to_owned());
    }
    match step.as_object().filter(|o| o.len() == 1).and_then(|o| o.keys().next()) {
        Some(k) if ["click", "type", "wait", "get", "text", "style", "rect", "focused", "key"].contains(&k.as_str()) => Ok(step.clone()),
        _ => Err("unknown step: use set, event, wait_ms or read (or the web steps click, type, wait, get, text, style, rect, focused, key)".to_owned()),
    }
}

/// The steps that answer a value, in the helper's vocabulary.
fn reads_a_value(step: &Value) -> bool {
    step.as_object().and_then(|o| o.keys().next()).is_some_and(|k| ["get", "text", "style", "rect", "focused"].contains(&k.as_str()))
}

/// `run_form` for a web form: the steps through real DOM events, then a picture.
pub fn run(root: &ProjectRoot, rel: &str, steps: &[Value], limit_s: u64) -> Result<(Option<Vec<u8>>, Value), String> {
    let started = Instant::now();
    let (mut page, _form) = open(root, rel, None, 1.0, None)?;
    // The script: the form first, then each step the helper has; the others
    // are reported as not done, with why.
    let mut script = vec![form_on_the_page()];
    let mut placed: Vec<Result<usize, String>> = Vec::new();
    for step in steps {
        match translate(step) {
            Ok(s) => {
                script.push(s);
                placed.push(Ok(script.len()));
            }
            Err(why) => placed.push(Err(why)),
        }
    }
    let limit = u32::try_from(limit_s.clamp(1, 120)).unwrap_or(20);
    let (timed_out, values, text, errors) = match page.session.script(script.clone(), limit) {
        Ok((values, text, errors)) => (false, values, text, errors),
        Err(e) if e.contains("did not finish") || e.contains("did not answer") => (true, Vec::new(), String::new(), vec![e]),
        Err(e) => return Err(e),
    };
    // Which script step failed, by its number (`step N (op): why`).
    let failed = |n: usize| errors.iter().find(|e| e.starts_with(&format!("step {n} ("))).cloned();
    if let Some(e) = failed(1) {
        return Err(format!("the form did not appear on the page: {e}"));
    }
    let mut out_steps = Vec::new();
    let mut reads = serde_json::Map::new();
    let mut next_value = values.into_iter();
    for (i, (step, at)) in steps.iter().zip(&placed).enumerate() {
        match at {
            Err(why) => out_steps.push(json!({ "step": i, "done": false, "why": why })),
            Ok(n) => {
                let value = if reads_a_value(&script[*n - 1]) { next_value.next() } else { None };
                let mut report = json!({ "step": i, "done": !timed_out && failed(*n).is_none() });
                if let Some(e) = failed(*n) {
                    report["why"] = json!(e);
                } else if timed_out {
                    report["why"] = json!("time limit reached");
                }
                if let Some(read) = step.get("read") {
                    let key = format!("{}::{}", read["control"].as_str().unwrap_or_default(), read["property"].as_str().unwrap_or_default());
                    reads.insert(key.clone(), value.clone().unwrap_or(Value::Null));
                    report["read"] = json!(key);
                }
                if let Some(v) = value {
                    report["value"] = v;
                }
                out_steps.push(report);
            }
        }
    }
    // What the page reported that is not a step's own error: its run-time errors.
    let page_errors: Vec<&String> = errors.iter().filter(|e| !e.starts_with("step ")).collect();
    let shot = if timed_out { None } else { picture(&mut page).ok() };
    let mut answer = json!({
        "form": rel,
        "steps": out_steps,
        "reads": reads,
        "program_ended": false,
        "on_pane": Value::Null,
        "windows": [],
        "timed_out": timed_out,
        "elapsed_ms": started.elapsed().as_millis() as u64,
        "picture": shot.as_ref().map(|(_, w, h)| json!({ "width": w, "height": h })),
        "display": text.lines().collect::<Vec<_>>(),
        "engine": ENGINE,
    });
    if !page_errors.is_empty() {
        answer["runtime_error"] = json!(page_errors.iter().map(|e| e.as_str()).collect::<Vec<_>>().join("\n"));
    }
    Ok((shot.map(|(png, _, _)| png), answer))
}
