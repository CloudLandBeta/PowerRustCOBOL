// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 084 R64, R69–R72 — the IDE's own panes, its IDE-wide settings, its
//! windows and state, and its widgets. Every operation calls what the IDE's
//! menu, button or window calls; the widget tool goes through the IDE's
//! built-in inspection plugin, as an accessibility client would.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::super::{example_project_manifest, CoboltApp, EXAMPLES};
use super::{arg_str, Wait};

/// Where the inspection plugin puts its answer.
pub(in crate::app) type Slot = Arc<Mutex<Option<egui_inspection::Response>>>;

/// What a request to the inspection plugin goes on to do once answered.
pub(in crate::app) enum Then {
    /// A picture of the window, cut to `rect` (logical points) when given.
    Shot { rect: Option<[f64; 4]> },
    /// A picture of one panel: the tree first, to find it.
    FindPanel { panel: String },
    /// The widget list.
    Tree,
    /// Act on a widget: the tree first, to find it.
    Find { act: Act },
    /// The events were applied: answer what was done.
    Acted { what: String },
}

/// One widget action of `ide_widget`.
pub(in crate::app) struct Act {
    action: String,
    target: String,
    text: String,
    delta: (f32, f32),
}

/// Labels a click must never press while work is unsaved (R72): the buttons
/// of a dialog that throw changes away, in every language the IDE speaks.
const DISCARDING: [&str; 10] = [
    "discard",
    "don't save",
    "dont save",
    "without saving",
    "no guardar",
    "descartar",
    "não salvar",
    "ne pas enregistrer",
    "保存しない",
    "不保存",
];

/// Send `req` to the IDE's inspection plugin; the answer lands in the slot.
fn submit(ctx: &egui::Context, req: egui_inspection::Request) -> Result<Slot, String> {
    let got: Slot = Arc::new(Mutex::new(None));
    let put = Arc::clone(&got);
    ctx.with_plugin(|p: &mut egui_inspection::InspectionPlugin| {
        p.submit(req, move |answer| {
            if let Ok(mut slot) = put.lock() {
                *slot = Some(answer);
            }
        })
    })
    .ok_or("the IDE's inspection plugin is not installed")?;
    ctx.request_repaint();
    Ok(got)
}

/// One widget of the accessibility tree, as `tree` answers it.
struct Widget {
    id: u64,
    role: String,
    label: String,
    value: String,
    rect: [f64; 4],
    disabled: bool,
}

fn widgets(update: &egui::accesskit::TreeUpdate) -> Vec<Widget> {
    update
        .nodes
        .iter()
        .filter_map(|(id, node)| {
            let b = node.bounds()?;
            let label = node.label().unwrap_or_default().trim().to_owned();
            let value = node.value().unwrap_or_default().trim().to_owned();
            if label.is_empty() && value.is_empty() {
                return None;
            }
            Some(Widget {
                id: id.0,
                role: format!("{:?}", node.role()),
                label,
                value,
                rect: [b.x0, b.y0, b.x1, b.y1],
                disabled: node.is_disabled(),
            })
        })
        .collect()
}

/// The widget `target` names: its id, else the label that matches exactly,
/// else the one that contains it — the smallest, which is the innermost.
fn find<'a>(all: &'a [Widget], target: &str) -> Option<&'a Widget> {
    if let Ok(id) = target.parse::<u64>() {
        if let Some(w) = all.iter().find(|w| w.id == id) {
            return Some(w);
        }
    }
    let t = target.to_lowercase();
    let area = |w: &&Widget| ((w.rect[2] - w.rect[0]) * (w.rect[3] - w.rect[1])) as i64;
    all.iter()
        .filter(|w| w.label.to_lowercase() == t)
        .min_by_key(area)
        .or_else(|| all.iter().filter(|w| w.label.to_lowercase().contains(&t)).min_by_key(area))
}

/// A project-relative or absolute path argument.
fn path_arg(app: &CoboltApp, args: &Value) -> Result<std::path::PathBuf, String> {
    let p = arg_str(args, "path").ok_or("'path' is required")?;
    let path = std::path::Path::new(p);
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        app.mcp_abs(&p.replace('\\', "/"))
    }
}

/// `name` lower-cased with only its letters and digits — how a menu entry or
/// a panel is matched ("File/Save Project" → "saveproject").
fn squash(name: &str) -> String {
    name.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

impl CoboltApp {
    // ── The Output panel and the diagnostics (R64) ──────────────────────────

    pub(super) fn mcp_read_output(&self, args: &Value) -> Value {
        let n = args.get("lines").and_then(Value::as_u64).unwrap_or(200).clamp(1, 5000) as usize;
        let text = self.output.all_text();
        let lines: Vec<&str> = text.lines().collect();
        let tail = &lines[lines.len().saturating_sub(n)..];
        json!({ "lines": tail, "total": lines.len() })
    }

    pub(super) fn mcp_read_diagnostics(&self, args: &Value) -> Value {
        let n = args.get("lines").and_then(Value::as_u64).unwrap_or(100).clamp(1, 5000) as usize;
        let root = self.project_dir();
        let rel = |p: &std::path::Path| match root.as_deref().and_then(|r| p.strip_prefix(r).ok()) {
            Some(r) => r.to_string_lossy().replace('\\', "/"),
            None => p.display().to_string(),
        };
        let mut files: Vec<_> = self.editor.diags.iter().filter(|(_, d)| !d.is_empty()).collect();
        files.sort_by(|a, b| a.0.cmp(b.0));
        let problems: Vec<Value> = files
            .into_iter()
            .flat_map(|(path, diags)| {
                diags.iter().map(move |d| {
                    let mut v = json!({
                        "file": rel(path),
                        "line": d.line,
                        "column": d.col,
                        "severity": format!("{:?}", d.severity).to_lowercase(),
                        "message": d.message,
                    });
                    if let Some(crate::runner::DiagOrigin::Site { form_path, .. }) = &d.origin {
                        v["form"] = json!(rel(form_path));
                    }
                    v
                })
            })
            .collect();
        let log: Vec<&str> = self.build_log.iter().map(|(_, l)| l.as_str()).collect();
        let tail = &log[log.len().saturating_sub(n)..];
        json!({
            "problems": problems,
            "build_log": tail,
            "build": match &self.build_outcome {
                Some(Ok(done)) => json!({ "ok": true, "message": done }),
                Some(Err(e)) => json!({ "ok": false, "message": e }),
                None => Value::Null,
            },
            "building": self.pending_build_rx.is_some(),
        })
    }

    // ── IDE-wide settings (R69) ─────────────────────────────────────────────

    pub(super) fn mcp_ide_settings(&self) -> Value {
        json!({
            "language": self.lang.code(),
            "languages": crate::i18n::Language::ALL.iter().map(|l| l.code()).collect::<Vec<_>>(),
            "debug": serde_json::to_value(&self.debug).unwrap_or(Value::Null),
            "build_folder": crate::ui_prefs::load_build_dir().map(|p| p.display().to_string()),
            "sdk_location": crate::ui_prefs::load_workspace_root().map(|p| p.display().to_string()),
            "line_numbers": self.editor.show_line_numbers,
            "note": "The IDE theme and the default form theme are the project's: project_settings and set_project_settings hold them.",
        })
    }

    pub(super) fn mcp_set_ide_settings(&mut self, args: &Value) -> Result<Value, String> {
        let values = args.get("values").and_then(Value::as_object).ok_or("'values' must be an object")?;
        // Validate every value before changing any (whole or nothing).
        let mut lang = None;
        let mut debug = None;
        let mut build_dir: Option<Option<std::path::PathBuf>> = None;
        let mut sdk: Option<Option<std::path::PathBuf>> = None;
        let mut line_numbers = None;
        for (key, value) in values {
            match key.as_str() {
                "language" => {
                    let code = value.as_str().unwrap_or_default();
                    lang = Some(crate::i18n::Language::from_code(code).ok_or_else(|| {
                        format!("'{code}' is not a language the IDE speaks: en, es, pt, fr, ja or zh")
                    })?);
                }
                "debug" => {
                    let mut now = serde_json::to_value(&self.debug).map_err(|e| e.to_string())?;
                    let patch = value.as_object().ok_or("'debug' must be an object of Debug Settings switches")?;
                    for (k, v) in patch {
                        if now.get(k).is_none() {
                            return Err(format!("'{k}' is not a Debug Settings switch"));
                        }
                        now[k] = v.clone();
                    }
                    debug = Some(
                        serde_json::from_value::<crate::debug_settings::DebugSettings>(now)
                            .map_err(|e| format!("Debug Settings: {e}"))?,
                    );
                }
                "build_folder" => {
                    build_dir = Some(match value.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                        Some(dir) => {
                            let dir = std::path::PathBuf::from(dir);
                            cobolt_compiler::check_build_root_usable(&dir)
                                .map_err(|p| format!("the Build Folder '{}' is {p}", dir.display()))?;
                            Some(dir)
                        }
                        None => None,
                    });
                }
                "sdk_location" => {
                    sdk = Some(match value.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                        Some(dir) => {
                            let dir = std::path::PathBuf::from(dir);
                            if !dir.is_dir() {
                                return Err(format!("the Platform SDK location '{}' is not a folder", dir.display()));
                            }
                            Some(dir)
                        }
                        None => None,
                    });
                }
                "line_numbers" => line_numbers = Some(value.as_bool().ok_or("'line_numbers' must be true or false")?),
                "port" | "token" | "mcp_port" | "access_token" => {
                    return Err("the tool server's port and access token are not settable through the tools".into())
                }
                "theme" | "form_theme" => {
                    return Err(format!("'{key}' is a project setting: change it with set_project_settings"))
                }
                other => return Err(format!("'{other}' is not an IDE-wide setting; ide_settings lists them")),
            }
        }
        let mut changed = Vec::new();
        if let Some(l) = lang {
            self.lang = l; // saved on this frame's language check
            changed.push("language");
        }
        if let Some(d) = debug {
            self.debug = d;
            self.debug.save();
            self.debug.apply_in_process();
            changed.push("debug");
        }
        if let Some(dir) = build_dir {
            crate::ui_prefs::save_build_dir(dir.as_deref());
            cobolt_compiler::set_build_root_override(dir);
            self.build_folder_problem = None;
            changed.push("build_folder");
        }
        if let Some(dir) = sdk {
            crate::ui_prefs::save_workspace_root(dir.as_deref());
            changed.push("sdk_location");
        }
        if let Some(on) = line_numbers {
            self.editor.show_line_numbers = on;
            changed.push("line_numbers");
        }
        Ok(json!({ "changed": changed, "settings": self.mcp_ide_settings() }))
    }

    // ── What the IDE shows (R70) ────────────────────────────────────────────

    pub(super) fn mcp_ide_state(&mut self) -> Value {
        let root = self.project_dir();
        let rel = |p: &std::path::Path| match root.as_deref().and_then(|r| p.strip_prefix(r).ok()) {
            Some(r) => r.to_string_lossy().replace('\\', "/"),
            None => p.display().to_string(),
        };
        let tabs: Vec<Value> = self
            .editor
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| {
                json!({ "path": rel(&t.path), "active": i == self.editor.active, "unsaved": t.dirty, "read_only": t.read_only })
            })
            .collect();
        let designer = |path: &std::path::Path, d: &crate::panels::designer::DesignerPanel| {
            json!({
                "path": rel(path),
                "form": d.form.name,
                "unsaved": d.dirty,
                "selected": d.selected_ids,
                "layer": d.tabs.active_layer(),
                "layers": d.form.layers.iter().map(|l| l.name.clone()).collect::<Vec<_>>(),
            })
        };
        let designers: Vec<Value> = self.designers.iter().map(|(p, d)| designer(p, d)).collect();
        json!({
            "project": self.project_path.as_ref().map(|p| p.display().to_string()),
            "project_name": self.cobolt_project.as_ref().map(|p| p.project.name.clone()),
            "tabs": tabs,
            "designers": designers,
            "main_pane_form": self.inspect.as_ref().map(|st| {
                let mut v = designer(&st.path, &st.designer);
                v["control"] = json!(st.ctrl_id);
                v
            }),
            "unsaved": self.has_unsaved_changes(),
            "panels": {
                "inspector": self.show_inspector,
                "code_search": self.show_code_search,
                "project_settings": self.show_project_settings,
                "grace_chat": self.show_grace_chat,
                "backend": self.backend_window.is_visible(),
                "line_numbers": self.editor.show_line_numbers,
            },
            "debugging": self.debug_active,
            "run": self.mcp_run_status(),
            "backend": self.backend_run.as_ref().map(|r| json!({ "address": r.url(), "running": r.is_running() })),
            "web_preview": self.web_preview.url(),
            "building": self.pending_build_rx.is_some(),
        })
    }

    // ── Pictures and widgets (R70, R72) ─────────────────────────────────────

    pub(super) fn mcp_ide_picture(&mut self, args: &Value) -> Result<Wait, Option<String>> {
        let ctx = self.egui_ctx.clone();
        match arg_str(args, "panel") {
            Some(panel) => {
                let got = submit(&ctx, egui_inspection::Request::GetTree).map_err(Some)?;
                Ok(Wait::Plugin { got, then: Then::FindPanel { panel: panel.to_owned() } })
            }
            None => {
                let got = submit(&ctx, egui_inspection::Request::GetScreenshot { pixels_per_point: Some(1.0) })
                    .map_err(Some)?;
                Ok(Wait::Plugin { got, then: Then::Shot { rect: None } })
            }
        }
    }

    pub(super) fn mcp_ide_widget(&mut self, args: &Value) -> Result<Wait, Option<String>> {
        let action = arg_str(args, "action").ok_or(Some("'action' is required".to_owned()))?;
        let then = match action {
            "tree" => Then::Tree,
            "click" | "type" | "scroll" => {
                let target = arg_str(args, "target").ok_or(Some(format!("'{action}' needs a 'target'")))?;
                let text = arg_str(args, "text").unwrap_or_default().to_owned();
                if action == "type" && text.is_empty() {
                    return Err(Some("'type' needs a 'text'".into()));
                }
                let num = |k: &str| args.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                Then::Find {
                    act: Act { action: action.to_owned(), target: target.to_owned(), text, delta: (num("dx"), num("dy")) },
                }
            }
            other => return Err(Some(format!("'{other}' is not a widget action: tree, click, type or scroll"))),
        };
        let got = submit(&self.egui_ctx.clone(), egui_inspection::Request::GetTree).map_err(Some)?;
        Ok(Wait::Plugin { got, then })
    }

    /// The next step of an inspection-plugin request, once its answer is in.
    pub(super) fn mcp_poll_plugin(&mut self, got: &mut Slot, then: &mut Then, expired: bool) -> Option<Result<Value, String>> {
        let answer = got.lock().ok().and_then(|mut s| s.take());
        let Some(answer) = answer else {
            return expired.then(|| Err("the IDE did not answer in time; is its window minimised?".to_owned()));
        };
        use egui_inspection::{Request, Response};
        // The answer, or the next request and what follows it.
        let next = |req: Request, t: Then| Err((req, t));
        let step: Result<Result<Value, String>, (Request, Then)> = match (answer, &*then) {
            (Response::Error { message }, _) => Ok(Err(message)),
            (Response::Screenshot(png), Then::Shot { rect }) => Ok(crop_png(&png.bytes, png.size, *rect)),
            (Response::Tree { accesskit, .. }, Then::FindPanel { panel }) => {
                let all = accesskit.as_ref().map(widgets).unwrap_or_default();
                let Some(w) = find(&all, panel) else {
                    return Some(Err(format!(
                        "no panel or widget labelled '{panel}' is on screen; ide_widget tree lists what is"
                    )));
                };
                next(Request::GetScreenshot { pixels_per_point: Some(1.0) }, Then::Shot { rect: Some(w.rect) })
            }
            (Response::Tree { accesskit, .. }, Then::Tree) => {
                let all = accesskit.as_ref().map(widgets).unwrap_or_default();
                let list: Vec<Value> = all
                    .iter()
                    .map(|w| {
                        json!({
                            "id": w.id.to_string(),
                            "role": w.role,
                            "label": w.label,
                            "value": if w.value.is_empty() { Value::Null } else { json!(w.value) },
                            "rect": w.rect.map(|v| v.round()),
                            "disabled": w.disabled,
                        })
                    })
                    .collect();
                Ok(Ok(json!({ "widgets": list })))
            }
            (Response::Tree { accesskit, .. }, Then::Find { act }) => {
                let all = accesskit.as_ref().map(widgets).unwrap_or_default();
                let Some(w) = find(&all, &act.target) else {
                    return Some(Err(format!("no widget labelled '{}' is on screen; ide_widget tree lists what is", act.target)));
                };
                if w.disabled && act.action != "scroll" {
                    return Some(Err(format!("'{}' is disabled", w.label)));
                }
                let lower = w.label.to_lowercase();
                if act.action == "click" && self.has_unsaved_changes() && DISCARDING.iter().any(|d| lower.contains(d)) {
                    return Some(Err(format!(
                        "'{}' would throw unsaved work away; save it first (save_project) or leave the choice to the developer",
                        w.label
                    )));
                }
                let at = egui::pos2(((w.rect[0] + w.rect[2]) / 2.0) as f32, ((w.rect[1] + w.rect[3]) / 2.0) as f32);
                let none = egui::Modifiers::NONE;
                let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: none };
                let (events, what) = match act.action.as_str() {
                    "click" => (vec![egui::Event::PointerMoved(at), press(true), press(false)], format!("clicked '{}'", w.label)),
                    "type" => (
                        vec![egui::Event::PointerMoved(at), press(true), press(false), egui::Event::Text(act.text.clone())],
                        format!("typed into '{}'", w.label),
                    ),
                    _ => (
                        vec![
                            egui::Event::PointerMoved(at),
                            egui::Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Point,
                                delta: egui::vec2(act.delta.0, act.delta.1),
                                phase: egui::TouchPhase::Move,
                                modifiers: none,
                            },
                        ],
                        format!("scrolled '{}'", w.label),
                    ),
                };
                next(Request::ApplyEvents { events }, Then::Acted { what })
            }
            (Response::Done, Then::Acted { what }) => Ok(Ok(json!({ "done": what }))),
            (other, _) => Ok(Err(format!("the IDE answered out of turn: {}", answer_kind(&other)))),
        };
        match step {
            Ok(answer) => Some(answer),
            Err((req, t)) => match submit(&self.egui_ctx, req) {
                Ok(slot) => {
                    *got = slot;
                    *then = t;
                    None
                }
                Err(e) => Some(Err(e)),
            },
        }
    }

    // ── Named operations on the IDE (R71) ───────────────────────────────────

    pub(super) fn mcp_ide_command(&mut self, args: &Value) -> Result<Value, String> {
        let command = arg_str(args, "command").ok_or("'command' is required")?;
        let empty = json!({});
        let a = args.get("args").filter(|v| v.is_object()).unwrap_or(&empty);
        match command {
            "open_tab" => {
                let path = path_arg(self, a)?;
                if !path.is_file() {
                    return Err(format!("'{}' is not a file", path.display()));
                }
                let is_form = matches!(
                    path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref(),
                    Some("cfrm" | "wcfrm" | "mcfrm")
                );
                if is_form {
                    self.load_form_from_path(path.clone());
                    return Ok(json!({ "opened": path.display().to_string(), "in": "designer" }));
                }
                self.open_in_editor(path.clone());
                if let Some(line) = a.get("line").and_then(Value::as_u64) {
                    self.editor.goto_line(line as u32, 1);
                }
                Ok(json!({ "opened": path.display().to_string(), "in": "editor" }))
            }
            "activate_tab" => {
                let path = path_arg(self, a)?;
                if let Some(i) = self.editor.tabs.iter().position(|t| t.path == path) {
                    self.editor.active = i;
                    return Ok(json!({ "active": path.display().to_string() }));
                }
                if self.designers.iter().any(|(p, _)| p == &path) {
                    self.designer_activation_requests.request(path.clone());
                    self.egui_ctx.request_repaint();
                    return Ok(json!({ "active": path.display().to_string(), "in": "designer" }));
                }
                Err(format!("'{}' is not open; open it with open_tab", path.display()))
            }
            "close_tab" => {
                let path = path_arg(self, a)?;
                if let Some(i) = self.editor.tabs.iter().position(|t| t.path == path) {
                    if self.editor.tabs[i].dirty {
                        return Err(format!("'{}' holds unsaved changes; save them first (save_project)", path.display()));
                    }
                    self.editor.tabs.remove(i);
                    if self.editor.active >= self.editor.tabs.len() && !self.editor.tabs.is_empty() {
                        self.editor.active = self.editor.tabs.len() - 1;
                    }
                    return Ok(json!({ "closed": path.display().to_string() }));
                }
                if let Some((_, d)) = self.designers.iter_mut().find(|(p, _)| p == &path) {
                    if d.dirty {
                        return Err(format!("the designer of '{}' holds unsaved changes; save them first (save_project)", path.display()));
                    }
                    d.close_requested = true;
                    self.egui_ctx.request_repaint();
                    return Ok(json!({ "closed": path.display().to_string(), "in": "designer" }));
                }
                Err(format!("'{}' is not open", path.display()))
            }
            "goto_line" => {
                let path = path_arg(self, a)?;
                let line = a.get("line").and_then(Value::as_u64).ok_or("'line' is required")? as u32;
                self.open_in_editor(path.clone());
                if !self.editor.goto_line(line, 1) {
                    return Err(format!("'{}' did not open in the editor", path.display()));
                }
                Ok(json!({ "at": { "path": path.display().to_string(), "line": line } }))
            }
            "select_controls" | "set_layer" => {
                let path = path_arg(self, a)?;
                let d = match self.designers.iter_mut().find(|(p, _)| p == &path) {
                    Some((_, d)) => d,
                    None => match self.inspect.as_mut().filter(|st| st.path == path) {
                        Some(st) => &mut st.designer,
                        None => return Err(format!("'{}' is not open in a designer; open it with open_tab", path.display())),
                    },
                };
                if command == "select_controls" {
                    let ids: Vec<String> = a
                        .get("ids")
                        .and_then(Value::as_array)
                        .ok_or("'ids' must be a list of control ids")?
                        .iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect();
                    if let Some(missing) = ids.iter().find(|id| d.form.find_control(id).is_none()) {
                        return Err(format!("the form has no control '{missing}'"));
                    }
                    d.selected_ids = ids.clone();
                    self.egui_ctx.request_repaint();
                    Ok(json!({ "selected": ids }))
                } else {
                    let layer = arg_str(a, "layer").ok_or("'layer' is required (\"Form\" for the form itself)")?;
                    if layer.eq_ignore_ascii_case("form") {
                        d.tabs.select_form();
                    } else {
                        let Some(l) = d.form.layers.iter().find(|l| l.name.eq_ignore_ascii_case(layer)) else {
                            return Err(format!("the form has no layer '{layer}'"));
                        };
                        let name = l.name.clone();
                        d.tabs.select_layer(&name);
                    }
                    self.egui_ctx.request_repaint();
                    Ok(json!({ "layer": layer }))
                }
            }
            "show_panel" | "hide_panel" => {
                let on = command == "show_panel";
                let panel = arg_str(a, "panel").ok_or("'panel' is required")?;
                match squash(panel).as_str() {
                    "inspector" => self.show_inspector = on,
                    "codesearch" | "search" => self.show_code_search = on,
                    "projectsettings" | "settings" => {
                        if on {
                            self.show_project_settings_pane();
                        } else {
                            self.show_project_settings = false;
                        }
                    }
                    "gracechat" | "grace" | "chat" => self.show_grace_chat = on,
                    "backend" => {
                        if self.backend_window.is_visible() != on {
                            self.backend_window.toggle();
                        }
                    }
                    "linenumbers" => self.editor.show_line_numbers = on,
                    _ => {
                        return Err(format!(
                            "'{panel}' is not a panel the IDE shows or hides: inspector, code_search, project_settings, \
                             grace_chat, backend, line_numbers"
                        ))
                    }
                }
                self.egui_ctx.request_repaint();
                Ok(json!({ "panel": panel, "shown": on }))
            }
            "menu" => self.mcp_menu(arg_str(a, "entry").or_else(|| arg_str(args, "entry")).ok_or("'entry' is required")?),
            other => Err(format!(
                "'{other}' is not an IDE command: open_tab, activate_tab, close_tab, goto_line, select_controls, \
                 set_layer, show_panel, hide_panel, menu"
            )),
        }
    }

    /// A menu entry, by its path ("File/Save Project") or its last part.
    fn mcp_menu(&mut self, entry: &str) -> Result<Value, String> {
        let parts: Vec<&str> = entry.split(['/', '>', '▸']).map(str::trim).filter(|s| !s.is_empty()).collect();
        let ctx = self.egui_ctx.clone();
        // Help/Examples/<name> opens one example.
        if let Some(i) = parts.iter().position(|p| squash(p) == "examples") {
            let names: Vec<&str> = EXAMPLES.iter().map(|e| e.project).collect();
            let Some(name) = parts.get(i + 1) else {
                return Err(format!("name the example: Help/Examples/<name>, one of {}", names.join(", ")));
            };
            let Some(ex) = EXAMPLES.iter().find(|e| e.project.eq_ignore_ascii_case(name)) else {
                return Err(format!("'{name}' is not an example: {}", names.join(", ")));
            };
            if example_project_manifest(ex).is_none() {
                return Err(format!("this installation ships no copy of {}", ex.project));
            }
            let ex = *ex;
            self.do_open_examples(&ex);
            return Ok(json!({ "done": format!("opened the example {}", ex.project) }));
        }
        let last = parts.last().copied().unwrap_or(entry);
        let key = squash(last);
        let dialog = |tool: &str| Err(format!("'{last}' opens a file dialog the developer answers; use the {tool} tool instead"));
        let what = match key.as_str() {
            "newproject" => return dialog("create_project"),
            "openproject" => return dialog("open_project"),
            "packageproject" => return dialog("package_project"),
            "saveproject" => {
                self.do_save_project();
                "saved the project"
            }
            "save" => {
                self.do_save();
                "saved"
            }
            "buildbinary" | "build" | "buildweb" => {
                if self.cobolt_project.is_none() {
                    return Err("no project is open in the IDE".into());
                }
                if self.pending_build_rx.is_some() || self.web_preview.building() {
                    return Err("a build is already running; read_diagnostics follows it".into());
                }
                self.do_build_binary_button();
                "started the build; read_diagnostics follows it"
            }
            "quit" | "exit" => {
                if self.has_unsaved_changes() {
                    return Err("the IDE holds unsaved work; save it first (save_project) or leave quitting to the developer".into());
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                "asked the IDE to close (this connection ends with it)"
            }
            "configureclaudecode" => {
                self.do_configure_claude_code();
                "configured Claude Code"
            }
            "codingagentsettings" => {
                self.cc_settings.toggle();
                if self.cc_settings.open {
                    self.probe_claude_code();
                }
                "toggled Coding Agent Settings"
            }
            "debugsettings" => {
                self.debug_modal.toggle();
                "toggled Debug Settings (set_ide_settings changes them directly)"
            }
            "platformsdklocation" | "sdklocation" => {
                self.sdk_modal_open = true;
                "opened Platform SDK Location (set_ide_settings changes it directly)"
            }
            "buildfolder" => {
                self.build_folder_modal_open = true;
                self.build_folder_problem = None;
                "opened Build Folder (set_ide_settings changes it directly)"
            }
            "reindexknowledgebases" | "reindex" => {
                if self.kb_reindex_rx.is_some() {
                    return Err("the Knowledge Bases are being reindexed already".into());
                }
                self.do_reindex_kb();
                "started reindexing the Knowledge Bases"
            }
            "documentation" | "developersguide" | "guide" => {
                self.doc_viewer.open(self.lang);
                "opened the Developer's Guide"
            }
            "idewalkthrough" | "walkthrough" => {
                let has_project = self.cobolt_project.is_some();
                self.walkthrough_shown = false;
                crate::ui_prefs::set_walkthrough_shown(false);
                use crate::panels::walkthrough::MenuAction;
                match crate::panels::walkthrough::on_menu_toggle(false, has_project) {
                    MenuAction::Start => {
                        self.walkthrough.start(&ctx);
                        "started the IDE walkthrough"
                    }
                    MenuAction::NeedsProject | MenuAction::Remember => "the walkthrough starts when a project opens",
                }
            }
            "taskwizard" => {
                self.toggle_task_wizard(&ctx);
                "toggled the Task Wizard"
            }
            "whatsnew" => {
                self.announcement.ask(&ctx);
                "opened What's New"
            }
            "about" => {
                self.about_open = true;
                "opened About"
            }
            // The Run menu.
            "run" => {
                self.do_run();
                "started the run; run_status follows it"
            }
            "debug" => {
                self.do_debug();
                "started debugging; debug_state follows it"
            }
            "stop" => {
                self.do_stop();
                "stopped"
            }
            "check" => {
                self.do_check();
                "checked; read_diagnostics answers the problems"
            }
            "runondevice" => {
                self.open_run_on_device(None);
                "opened Run on Device"
            }
            "startbackend" | "backend" => {
                self.start_backend();
                "started the Backend; backend_status follows it"
            }
            _ => {
                return Err(format!(
                    "'{entry}' is not a menu entry the tools open. File: Save Project, Save, Build Binary, Quit. \
                     Settings: Configure Claude Code, Coding Agent Settings, Debug Settings, Platform SDK Location, \
                     Build Folder, Reindex Knowledge Bases. Help: Documentation, Examples/<name>, IDE Walkthrough, \
                     Task Wizard, What's New, About. Run: Run, Debug, Stop, Check, Run on Device, Start Backend."
                ))
            }
        };
        ctx.request_repaint();
        Ok(json!({ "done": what }))
    }
}

/// The kind of an inspection answer, for an error message.
fn answer_kind(r: &egui_inspection::Response) -> &'static str {
    use egui_inspection::Response;
    match r {
        Response::Info { .. } => "info",
        Response::Tree { .. } => "a widget tree",
        Response::Screenshot(_) => "a picture",
        Response::Done => "done",
        Response::Settled { .. } => "settled",
        Response::Error { .. } => "an error",
    }
}

/// The window's picture as `png_base64`, cut to `rect` (logical points) when
/// given. The picture is taken at one physical pixel per point, so points and
/// pixels agree.
fn crop_png(bytes: &[u8], size: [u32; 2], rect: Option<[f64; 4]>) -> Result<Value, String> {
    let Some(r) = rect else {
        return Ok(json!({ "png_base64": cobolt_mcp::base64(bytes), "width": size[0], "height": size[1] }));
    };
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    let x = (r[0].max(0.0) as u32).min(img.width().saturating_sub(1));
    let y = (r[1].max(0.0) as u32).min(img.height().saturating_sub(1));
    let w = ((r[2] - r[0]).max(1.0) as u32).min(img.width() - x);
    let h = ((r[3] - r[1]).max(1.0) as u32).min(img.height() - y);
    let cut = img.crop_imm(x, y, w, h);
    let mut out = std::io::Cursor::new(Vec::new());
    cut.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(json!({ "png_base64": cobolt_mcp::base64(out.get_ref()), "width": w, "height": h }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(id: u64, label: &str, rect: [f64; 4]) -> Widget {
        Widget { id, role: "Button".into(), label: label.into(), value: String::new(), rect, disabled: false }
    }

    #[test]
    fn a_widget_is_found_by_id_exact_label_then_the_innermost_containing_one() {
        let all = vec![
            w(1, "Save Project", [0.0, 0.0, 100.0, 20.0]),
            w(2, "Save", [0.0, 30.0, 40.0, 50.0]),
            w(3, "Project tree", [0.0, 0.0, 400.0, 400.0]),
            w(4, "Project", [0.0, 0.0, 300.0, 300.0]),
        ];
        assert_eq!(find(&all, "3").map(|w| w.id), Some(3));
        assert_eq!(find(&all, "save").map(|w| w.id), Some(2), "exact label beats containing");
        assert_eq!(find(&all, "proj").map(|w| w.id), Some(1), "the smallest that contains it");
        assert!(find(&all, "nothing").is_none());
    }

    #[test]
    fn menu_entries_and_panels_match_by_letters_and_digits() {
        assert_eq!(squash("File/Save Project"), "filesaveproject");
        assert_eq!(squash("Save Project"), "saveproject");
        assert_eq!(squash("Developer's Guide"), "developersguide");
        assert_eq!(squash("code_search"), "codesearch");
    }

    #[test]
    fn a_picture_is_cut_to_the_panel() {
        let img = image::RgbaImage::from_pixel(40, 30, image::Rgba([10, 20, 30, 255]));
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut png, image::ImageFormat::Png).unwrap();
        let whole = crop_png(png.get_ref(), [40, 30], None).unwrap();
        assert_eq!((whole["width"].as_u64(), whole["height"].as_u64()), (Some(40), Some(30)));
        let cut = crop_png(png.get_ref(), [40, 30], Some([5.0, 5.0, 25.0, 15.0])).unwrap();
        assert_eq!((cut["width"].as_u64(), cut["height"].as_u64()), (Some(20), Some(10)));
        // A rectangle past the edge is cut at the edge.
        let edge = crop_png(png.get_ref(), [40, 30], Some([30.0, 20.0, 90.0, 90.0])).unwrap();
        assert_eq!((edge["width"].as_u64(), edge["height"].as_u64()), (Some(10), Some(10)));
    }
}
