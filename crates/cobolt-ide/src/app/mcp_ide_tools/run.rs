// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Run, the Backend, the Web Preview, a Mobile device and the debugger
//! (spec 084 R62, R63) — the Run menu's own code. The developer's code runs
//! with its real effects, and the IDE shows it as if Run had been pressed.

use std::path::PathBuf;

use serde_json::{json, Value};

use super::super::{CoboltApp, WebIntent};
use super::{arg_str, Wait};
use crate::panels::debugger::DebugAction;

/// Most Output lines a run answer carries.
const RUN_LINES: usize = 400;

impl CoboltApp {
    /// The Output lines since `mark` (all of them when the panel was cleared
    /// since), the newest [`RUN_LINES`].
    fn mcp_output_since(&self, mark: Option<usize>) -> Vec<String> {
        let text = self.output.all_text();
        let lines: Vec<&str> = text.lines().collect();
        let from = mark.filter(|m| *m <= lines.len()).unwrap_or(0);
        let tail = &lines[from..];
        tail[tail.len().saturating_sub(RUN_LINES)..].iter().map(|s| (*s).to_owned()).collect()
    }

    fn mcp_runs_running(&mut self) -> Vec<String> {
        let mut out: Vec<String> = self.external_runs.iter().map(|r| r.form_name.clone()).collect();
        out.extend(self.built_runs.iter().map(|r| r.name.clone()));
        if !self.runner.is_finished() {
            out.push("console".to_owned());
        }
        out
    }

    // ── Run (R62) ───────────────────────────────────────────────────────────

    pub(super) fn mcp_run_project(&mut self) -> Result<Value, String> {
        self.mcp_root()?;
        let mark = self.output.all_text().lines().count();
        self.mcp.last_run_end = None;
        self.do_run();
        if self.stale_build_prompt.is_some() {
            return Err("the IDE is asking the developer whether to rebuild first (the last build is older than the code); ask them to answer it".to_owned());
        }
        if self.browser_confirm.is_some() {
            return Err("the IDE is asking the developer to confirm the browser it opens the page in; ask them to answer it".to_owned());
        }
        self.mcp.run_mark = Some(mark);
        Ok(json!({
            "started": true,
            "running": self.mcp_runs_running(),
            "next": "run_status for its output and whether it ended; stop_run to stop it"
        }))
    }

    pub(super) fn mcp_run_status(&mut self) -> Value {
        let running = self.mcp_runs_running();
        let building = self.pending_build_rx.is_some();
        let ended = self.mcp.last_run_end.clone().map(|(name, err)| {
            json!({ "name": name, "ok": err.is_none(), "error": err })
        });
        json!({
            "running": running,
            "building": building,
            "ended": ended,
            "output": self.mcp_output_since(self.mcp.run_mark),
        })
    }

    pub(super) fn mcp_stop_run(&mut self) -> Result<Value, String> {
        let stopped = self.mcp_runs_running();
        self.do_stop();
        for run in self.external_runs.iter_mut() {
            run.stop();
        }
        for run in self.built_runs.iter_mut() {
            run.stop();
        }
        if self.debug_active {
            self.handle_debug_action(DebugAction::Stop);
        }
        Ok(json!({ "stopped": stopped }))
    }

    // ── Backend and Web Preview (R62) ───────────────────────────────────────

    pub(super) fn mcp_start_backend(&mut self) -> Result<Value, String> {
        self.mcp_root()?;
        if self.backend_project_dir().is_none() {
            return Err("this project has no Backend (a Web or Mobile project with backend/exports.toml)".to_owned());
        }
        if !self.backend_run.as_ref().is_some_and(|r| r.is_running()) {
            self.start_backend();
        }
        Ok(self.mcp_backend_status())
    }

    pub(super) fn mcp_backend_status(&mut self) -> Value {
        let run = self.backend_run.as_ref();
        let url = run.and_then(|r| r.url()).map(str::to_owned);
        let answers = url.as_deref().is_some_and(answers_at);
        json!({
            "running": run.is_some_and(|r| r.is_running()),
            "failed": self.backend_failed,
            "address": url,
            "answers": answers,
            "output": self.mcp_output_since(None).into_iter().rev().take(60).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>(),
        })
    }

    pub(super) fn mcp_stop_backend(&mut self) -> Result<Value, String> {
        let was = self.backend_run.as_ref().is_some_and(|r| r.is_running());
        self.stop_backend();
        Ok(json!({ "stopped": was }))
    }

    pub(super) fn mcp_start_web_preview(&mut self) -> Result<Wait, Option<String>> {
        self.mcp_root().map_err(Some)?;
        if !self.is_web_project() {
            return Err(Some("the Web Preview is for a Web or Mobile project".to_owned()));
        }
        self.start_web(WebIntent::Show);
        if self.browser_confirm.is_some() {
            return Err(Some("the IDE is asking the developer to confirm the browser it opens the page in; ask them to answer it".to_owned()));
        }
        Ok(Wait::WebUrl)
    }

    pub(super) fn mcp_run_mobile(&mut self, args: &Value) -> Result<Value, String> {
        self.mcp_root()?;
        let target = arg_str(args, "target").ok_or("'target' is required: android or ios-simulator")?;
        match target {
            "android" => {
                if !self.is_android_project() {
                    return Err("this is not a Mobile project for Android".to_owned());
                }
                let runner = self.mobile_device_runner().ok_or("no Android tools were found on this machine (adb and the emulator)")?;
                let targets = runner.list(cobolt_compiler::project_kind::MobilePlatform::Android);
                let chosen = match arg_str(args, "device") {
                    Some(d) => targets
                        .iter()
                        .find(|t| t.id == d || t.label.eq_ignore_ascii_case(d))
                        .ok_or(format!("no device or emulator '{d}'; these are: {}", targets.iter().map(|t| t.label.clone()).collect::<Vec<_>>().join(", ")))?,
                    None => targets.first().ok_or("no Android device or emulator was found")?,
                };
                let manifest = self.project_path.clone().ok_or("the project has not been saved yet")?;
                self.web_preview.choose_device(&manifest, &chosen.id);
                let mark = self.output.all_text().lines().count();
                self.do_run();
                self.mcp.run_mark = Some(mark);
                Ok(json!({ "started": true, "device": chosen.label, "next": "run_status follows it" }))
            }
            "ios-simulator" => Err(
                "PowerRustCOBOL AI does not run a Mobile project on the iOS simulator yet: Build makes the simulator app, \
                 and Run shows the phone page in the browser. Write a gap report if the developer needs it."
                    .to_owned(),
            ),
            other => Err(format!("'{other}' is not a target: android or ios-simulator")),
        }
    }

    pub(super) fn mcp_android_studio(&mut self) -> Result<Wait, Option<String>> {
        self.mcp_root().map_err(Some)?;
        if !self.is_android_project() {
            return Err(Some("this is not a Mobile project for Android".to_owned()));
        }
        let manifest = self.project_path.clone().ok_or_else(|| Some("the project has not been saved yet".to_owned()))?;
        let req = crate::panels::native_project::request_from_manifest(&manifest).map_err(Some)?;
        let ctx = self.egui_ctx.clone();
        self.native_job.1.start(&ctx, req);
        self.native_job.0 = Some(manifest);
        Ok(Wait::NativeProject)
    }

    // ── Debugger (R63) ──────────────────────────────────────────────────────

    pub(super) fn mcp_debug_start(&mut self, args: &Value) -> Result<Value, String> {
        self.mcp_root()?;
        if let Some(rel) = arg_str(args, "path") {
            let abs = self.mcp_abs(rel)?;
            if !abs.is_file() {
                return Err(format!("{rel} is not a file of this project"));
            }
            self.open_in_editor(abs);
        }
        let mark = self.output.all_text().lines().count();
        self.do_debug();
        if self.stale_build_prompt.is_some() {
            return Err("the IDE is asking the developer whether to rebuild first; ask them to answer it".to_owned());
        }
        self.mcp.run_mark = Some(mark);
        Ok(json!({ "started": true, "next": "debug_state says where it stopped; debug_control drives it" }))
    }

    pub(super) fn mcp_debug_breakpoints(&mut self, args: &Value) -> Result<Value, String> {
        let root = self.mcp_root()?;
        if args.get("clear_all").and_then(Value::as_bool).unwrap_or(false) {
            self.editor.breakpoints.clear();
        }
        let points = |key: &str| -> Result<Vec<(PathBuf, u32)>, String> {
            let mut out = Vec::new();
            for p in args.get(key).and_then(Value::as_array).cloned().unwrap_or_default() {
                let file = p.get("file").and_then(Value::as_str).ok_or(format!("each of `{key}` needs a `file`"))?;
                let line = p.get("line").and_then(Value::as_u64).filter(|l| *l > 0).ok_or(format!("each of `{key}` needs a 1-based `line`"))?;
                let rel = std::path::Path::new(file);
                if rel.is_absolute() || rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                    return Err(format!("'{file}' is not a path inside the project"));
                }
                out.push((root.join(rel), line as u32));
            }
            Ok(out)
        };
        for (path, line) in points("set")? {
            if !self.editor.breakpoints.get(&path).is_some_and(|s| s.contains(&line)) {
                self.editor.toggle_breakpoint_at(&path, line);
            }
        }
        for (path, line) in points("clear")? {
            if self.editor.breakpoints.get(&path).is_some_and(|s| s.contains(&line)) {
                self.editor.toggle_breakpoint_at(&path, line);
            }
        }
        let mut now: Vec<Value> = Vec::new();
        for (path, lines) in &self.editor.breakpoints {
            let mut lines: Vec<u32> = lines.iter().copied().collect();
            lines.sort_unstable();
            for l in lines {
                now.push(json!({ "file": path.strip_prefix(&root).unwrap_or(path).to_string_lossy().replace('\\', "/"), "line": l }));
            }
        }
        Ok(json!({ "breakpoints": now }))
    }

    pub(super) fn mcp_debug_control(&mut self, args: &Value) -> Result<Wait, Option<String>> {
        if !self.debug_active {
            return Err(Some("no debug session runs; start one with debug_start".to_owned()));
        }
        let action = match arg_str(args, "action") {
            Some("continue") => DebugAction::Continue,
            Some("pause") => DebugAction::Pause,
            Some("step_over") => DebugAction::StepOver,
            Some("step_into") => DebugAction::StepIn,
            Some("step_out") => DebugAction::StepOut,
            Some("stop") => DebugAction::Stop,
            other => return Err(Some(format!("'{}' is not an action: continue, pause, step_over, step_into, step_out or stop", other.unwrap_or("")))),
        };
        let was = (self.debugger.is_paused(), self.debugger.current_line(), self.debugger.source_path().to_owned());
        self.handle_debug_action(action);
        Ok(Wait::Debug { was })
    }

    pub(super) fn mcp_debug_state(&mut self, args: &Value) -> Value {
        if let Some(names) = args.get("variables").and_then(Value::as_array) {
            let mut list = self.debugger.watch_expressions();
            for n in names.iter().filter_map(Value::as_str) {
                if !list.iter().any(|w| w.eq_ignore_ascii_case(n)) {
                    list.push(n.to_owned());
                }
            }
            if list.len() != self.debugger.watch_expressions().len() {
                self.debugger.set_watches(&list);
            }
        }
        let root = self.project_dir().unwrap_or_default();
        let file = self.debugger.source_path().to_owned();
        let file = std::path::Path::new(&file).strip_prefix(&root).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or(file);
        let stack: Vec<Value> = self
            .debugger
            .stack()
            .into_iter()
            .map(|(program, paragraph, line)| json!({ "program": program, "paragraph": paragraph, "line": line }))
            .collect();
        let watches: Vec<Value> = self
            .debugger
            .watches
            .iter()
            .map(|w| json!({ "expression": w.expression, "value": w.value, "error": w.error }))
            .collect();
        let wanted: Vec<String> = args
            .get("variables")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_ascii_uppercase)).collect())
            .unwrap_or_default();
        let variables: Vec<Value> = self
            .debugger
            .variables()
            .into_iter()
            .filter(|(name, ..)| wanted.is_empty() || wanted.contains(&name.to_ascii_uppercase()))
            .take(200)
            .map(|(name, value, pic)| json!({ "name": name, "value": value, "pic": pic }))
            .collect();
        json!({
            "active": self.debug_active,
            "paused": self.debugger.is_paused(),
            "reason": self.debugger.stop_reason_text(),
            "file": file,
            "line": self.debugger.current_line(),
            "paragraph": self.debugger.current_paragraph(),
            "stack": stack,
            "variables": variables,
            "watches": watches,
            "note": "a watch shows its value once the debugger has evaluated it at this stop; call debug_state again",
        })
    }

    // ── Waiting on a later frame ────────────────────────────────────────────

    /// The answer to a waiting tool, once its condition holds or `expired`;
    /// `None` to keep waiting.
    pub(super) fn mcp_poll_wait(&mut self, wait: &mut Wait, expired: bool) -> Option<Result<Value, String>> {
        match wait {
            Wait::Plugin { got, then } => self.mcp_poll_plugin(got, then, expired),
            Wait::WebUrl => match self.web_preview.url() {
                Some(url) => Some(Ok(json!({ "address": url, "backend": self.backend_run.as_ref().and_then(|r| r.url()) }))),
                None if expired => Some(Ok(json!({ "address": Value::Null, "building": true, "note": "still building; read_output follows it, and start_web_preview again answers the address" }))),
                None => None,
            },
            Wait::Reindex => {
                if self.kb_reindex_rx.is_none() {
                    let tail = self.mcp_output_since(None);
                    Some(Ok(json!({ "done": true, "output": tail.into_iter().rev().take(5).collect::<Vec<_>>() })))
                } else if expired {
                    Some(Ok(json!({ "done": false, "note": "still indexing; read_output follows it" })))
                } else {
                    None
                }
            }
            Wait::Debug { was } => {
                let now = (self.debugger.is_paused(), self.debugger.current_line(), self.debugger.source_path().to_owned());
                let settled = !self.debug_active || (now.0 && now != *was);
                if settled || expired {
                    Some(Ok(self.mcp_debug_state(&json!({}))))
                } else {
                    None
                }
            }
            Wait::NativeProject => {
                if self.native_job.1.running() && !expired {
                    return None;
                }
                Some(match &self.native_job.1.last {
                    Some(crate::panels::native_project::Outcome::Ready(dir)) => Ok(json!({ "created": dir.display().to_string() })),
                    Some(other) => Err(other.text(&self.lang.tr())),
                    None => Ok(json!({ "running": true, "note": "still writing; read_output follows it" })),
                })
            }
        }
    }

    pub(super) fn mcp_reindex(&mut self) -> Result<Wait, Option<String>> {
        self.mcp_root().map_err(Some)?;
        if self.kb_reindex_rx.is_none() {
            self.do_reindex_kb();
        }
        Ok(Wait::Reindex)
    }
}

/// Whether something accepts a connection at `url`'s host and port.
fn answers_at(url: &str) -> bool {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let host_port = rest.split('/').next().unwrap_or_default();
    let addr = if host_port.contains(':') { host_port.to_owned() } else { format!("{host_port}:80") };
    use std::net::ToSocketAddrs;
    addr.to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .is_some_and(|a| std::net::TcpStream::connect_timeout(&a, std::time::Duration::from_millis(300)).is_ok())
}
