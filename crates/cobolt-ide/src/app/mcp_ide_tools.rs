// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 084 Phase D — the MCP tools that need the running IDE, answered on its
//! UI thread from the requests [`crate::agent_kit::ide_host::IdeHost`] sends.
//!
//! Each tool calls the code the IDE's own menu, button or window calls, so a
//! change made through the connection is the change the developer would have
//! made by hand: the same project model, the same save and seal, the same undo.
//! A tool that must wait for a later frame (a picture of the window) keeps the
//! reply and answers it then; every other tool answers before it returns.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

use super::CoboltApp;

mod ide;
mod project;
mod run;

/// The reply channel of one IDE tool call.
pub(super) type Reply = Sender<Result<Value, String>>;

/// How often the out-of-band watch looks at the project (spec 084 R68).
const WATCH_EVERY: Duration = Duration::from_millis(1500);
/// The longest a tool waits for a later frame — under the listener's own
/// 60-second limit, so the answer always arrives.
const WAIT_LIMIT: Duration = Duration::from_secs(50);

/// What a tool that answers on a later frame waits for.
pub(super) enum Wait {
    /// The Web Preview's address.
    WebUrl,
    /// The Knowledge Base reindex to end.
    Reindex,
    /// The debuggee to stop again after a continue or a step: it was at
    /// `(paused, line, file)`.
    Debug { was: (bool, u32, String) },
    /// The Android Studio project to be written.
    NativeProject,
    /// The IDE's inspection plugin to answer (a picture, the widget tree, an
    /// action on a widget); `then` says what follows its answer.
    Plugin { got: ide::Slot, then: ide::Then },
}

/// The MCP tools' own state in the IDE.
#[derive(Default)]
pub(super) struct McpState {
    /// R68: the project's source files as last accepted.
    watch: Option<Watch>,
    last_watch: Option<Instant>,
    /// Tools answering on a later frame, with the moment they give up.
    waits: Vec<(Wait, Reply, Instant)>,
    /// The last run that ended: its name and its error, if it failed.
    last_run_end: Option<(String, Option<String>)>,
    /// How many Output lines there were when run_project started a run.
    run_mark: Option<usize>,
}

/// The project's source files, each with its modification time and length,
/// as they were after the last tool call or the last look.
struct Watch {
    root: PathBuf,
    /// [`crate::agent_kit::ide_host::IdeShared::calls_ended`] when taken.
    ended: u64,
    files: HashMap<PathBuf, (SystemTime, u64)>,
}

/// Whether R68 watches `rel`: the files a developer writes — forms, indexed
/// definitions, COBOL — never what the IDE or a build writes (`generated/`,
/// the manifest, the application's data, the build output).
fn watched(rel: &Path) -> bool {
    const SKIP: [&str; 9] = ["generated", "target", "dist", "bin", "build", "crates", "data", "node_modules", "KB"];
    let skipped = rel.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s.starts_with('.') || SKIP.contains(&s.as_ref())
    });
    let ext = rel.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    !skipped && matches!(ext.as_str(), "cfrm" | "wcfrm" | "mcfrm" | "cidx" | "cbl" | "cob" | "cpy")
}

/// Every watched file under `root`, with its modification time and length.
fn scan(root: &Path) -> HashMap<PathBuf, (SystemTime, u64)> {
    let mut out = HashMap::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if depth < 8 && !name.starts_with('.') && !["generated", "target", "dist", "bin", "build", "crates", "node_modules"].contains(&name.as_ref()) {
                    stack.push((path, depth + 1));
                }
            } else if watched(&rel) {
                out.insert(path, (meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), meta.len()));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 084 R68 watches what the developer writes, never what the IDE,
    /// a build or the application writes.
    #[test]
    fn the_watch_covers_sources_and_forms_only() {
        for yes in ["forms/MAIN.cfrm", "webforms/HOME.wcfrm", "mobileforms/A.mcfrm", "indexed/CUST.cidx", "src/T.cbl", "COPYBOOKS/R.CPY", "backend/src/B.cob"] {
            assert!(watched(Path::new(yes)), "{yes} is watched");
        }
        for no in ["generated/MAIN.cbl", "target/x.cbl", "dist/web/a.cbl", "data/CUST.dat", "Demo.project.toml", ".git/x.cbl", "assets/KB/a.cbl", "src/notes.txt", "bin/app"] {
            assert!(!watched(Path::new(no)), "{no} is not watched");
        }
    }
}

/// A string argument, trimmed; `None` when absent or blank.
pub(super) fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty())
}

impl CoboltApp {
    /// Once a frame: the tools waiting on a later frame, and the out-of-band
    /// watch (R68).
    pub(super) fn mcp_frame(&mut self, _ctx: &egui::Context) {
        let now = Instant::now();
        if !self.mcp.waits.is_empty() {
            let waits = std::mem::take(&mut self.mcp.waits);
            for (mut wait, reply, until) in waits {
                match self.mcp_poll_wait(&mut wait, now >= until) {
                    Some(answer) => {
                        let _ = reply.send(answer);
                    }
                    None => self.mcp.waits.push((wait, reply, until)),
                }
            }
            self.egui_ctx.request_repaint_after(Duration::from_millis(100));
        }
        self.mcp_watch_out_of_band(now);
    }

    /// A run ended (the run loops call this): kept for `run_status`.
    pub(super) fn mcp_note_run_end(&mut self, name: &str, error: Option<String>) {
        self.mcp.last_run_end = Some((name.to_owned(), error));
    }

    /// Spec 084 R68 — while Claude Code is connected, a watched project file
    /// that changed on disk between tool calls, and that is not what the IDE
    /// holds for it (its own save), gets an Output line, and the next tool
    /// answer names it. Changes while a call runs are the call's.
    fn mcp_watch_out_of_band(&mut self, now: Instant) {
        if self.mcp.last_watch.is_some_and(|t| now.saturating_duration_since(t) < WATCH_EVERY) {
            return;
        }
        self.mcp.last_watch = Some(now);
        let listening = self.agent_tools.listening_port().is_some();
        let activity = self.agent_tools.activity();
        let Some(root) = self.project_dir() else {
            self.mcp.watch = None;
            return;
        };
        if !crate::panels::claude_mascot::connected(listening, &activity, now) {
            self.mcp.watch = None;
            return;
        }
        if !activity.running.is_empty() {
            return;
        }
        let ended = self.agent_tools.calls_ended();
        let files = scan(&root);
        if let Some(w) = self.mcp.watch.as_ref().filter(|w| w.root == root && w.ended == ended) {
            let mut outside: Vec<PathBuf> = files
                .iter()
                .filter(|(p, st)| w.files.get(*p) != Some(*st))
                .map(|(p, _)| p.clone())
                .filter(|p| !self.ide_holds(p))
                .collect();
            outside.sort();
            if !outside.is_empty() {
                let rels: Vec<String> = outside
                    .iter()
                    .map(|p| p.strip_prefix(&root).unwrap_or(p).to_string_lossy().replace('\\', "/"))
                    .collect();
                let tr = self.lang.tr();
                for rel in &rels {
                    self.output.push_status(tr.mcp_out_of_band.replacen("{}", rel, 1));
                }
                self.agent_tools.report_out_of_band(&rels);
            }
        }
        self.mcp.watch = Some(Watch { root, ended, files });
    }

    /// Whether the IDE holds exactly what is on disk at `abs` — the IDE wrote
    /// it (a designer's or an editor tab's save).
    fn ide_holds(&self, abs: &Path) -> bool {
        if let Some((_, d)) = self.designers.iter().find(|(p, _)| p == abs) {
            return match (super::load_form(abs), cobolt_forms::form_to_string(&d.form)) {
                (Ok(disk), Ok(held)) => cobolt_forms::form_to_string(&disk).is_ok_and(|t| t == held),
                _ => false,
            };
        }
        if let Some(tab) = self.editor.tabs.iter().find(|t| t.path == abs) {
            return std::fs::read(abs).is_ok_and(|b| String::from_utf8_lossy(&b) == tab.content);
        }
        false
    }

    /// Follow a file or folder that the file tools renamed (`to` = its new
    /// project-relative path) or removed (`to` = `None`) in the project model,
    /// the open tabs and designers, and save the project, which re-seals it.
    pub(super) fn mcp_relist(&mut self, from: &str, to: Option<&str>) -> Result<(), String> {
        let root = self.project_dir().ok_or("no project is open in the IDE")?;
        let from = from.replace('\\', "/").trim_end_matches('/').to_owned();
        let proj = self.cobolt_project.as_mut().ok_or("no project is open in the IDE")?;
        match to {
            Some(to) => {
                let to = to.replace('\\', "/").trim_end_matches('/').to_owned();
                if root.join(&to).is_dir() {
                    proj.rename_prefix(&from, &to);
                } else {
                    proj.move_entry(&from, &to);
                }
                self.rewrite_open_paths(&root, &from, &to);
            }
            None => {
                proj.remove_file(&from);
                let _ = proj.drain_under(&from);
                let abs = root.join(&from);
                self.close_views_under(&abs);
                self.editor.tabs.retain(|t| t.path != abs);
                if self.editor.active >= self.editor.tabs.len() && !self.editor.tabs.is_empty() {
                    self.editor.active = self.editor.tabs.len() - 1;
                }
                self.designers.retain(|(p, _)| p != &abs);
            }
        }
        let path = self.project_path.clone().ok_or("the project has not been saved yet")?;
        let proj = self.cobolt_project.as_ref().ok_or("no project is open in the IDE")?;
        crate::project_model::save_project(proj, &path).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Answer one IDE tool call (spec 084 R56–R72). `reply` is answered before
    /// this returns, except by the tools that wait for a later frame.
    pub(super) fn mcp_ide_tool(&mut self, name: &str, args: &Value, reply: Reply) {
        // The tools whose answer comes from a later frame keep the reply.
        let waits = match name {
            "start_web_preview" => self.mcp_start_web_preview(),
            "project_kb_reindex" => self.mcp_reindex(),
            "debug_control" => self.mcp_debug_control(args),
            "create_android_studio_project" => self.mcp_android_studio(),
            "ide_picture" => self.mcp_ide_picture(args),
            "ide_widget" => self.mcp_ide_widget(args),
            "test_sql_connection" => {
                // Connecting may take seconds: it runs off the UI thread.
                match self.mcp_sql_test_job(args) {
                    Ok(job) => {
                        std::thread::spawn(move || {
                            let _ = reply.send(job());
                        });
                    }
                    Err(e) => {
                        let _ = reply.send(Err(e));
                    }
                }
                return;
            }
            _ => Err(None),
        };
        let result = match waits {
            Ok(wait) => {
                self.mcp.waits.push((wait, reply, Instant::now() + WAIT_LIMIT));
                return;
            }
            Err(Some(e)) => Err(e),
            Err(None) => self.mcp_ide_tool_now(name, args),
        };
        let _ = reply.send(result);
    }

    fn mcp_ide_tool_now(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        match name {
            "apply_change_set" => self.mcp_apply_change_set(args),
            "project_settings" => self.mcp_project_settings(),
            "set_project_settings" => self.mcp_set_project_settings(args),
            "sql_connections" => self.mcp_sql_connections(),
            "set_sql_connection" => self.mcp_set_sql_connection(args),
            "remove_sql_connection" => self.mcp_remove_sql_connection(args),
            "save_project" => self.mcp_save_project(),
            "package_project" => self.mcp_package_project(args),
            "upgrade_project" => self.mcp_upgrade_project(),
            "run_project" => self.mcp_run_project(),
            "run_status" => Ok(self.mcp_run_status()),
            "stop_run" => self.mcp_stop_run(),
            "start_backend" => self.mcp_start_backend(),
            "backend_status" => Ok(self.mcp_backend_status()),
            "stop_backend" => self.mcp_stop_backend(),
            "run_mobile" => self.mcp_run_mobile(args),
            "debug_start" => self.mcp_debug_start(args),
            "debug_breakpoints" => self.mcp_debug_breakpoints(args),
            "debug_state" => Ok(self.mcp_debug_state(args)),
            "project_kb_search" => self.mcp_project_kb_search(args),
            "indexed_records" => self.mcp_indexed_records(args),
            "read_output" => Ok(self.mcp_read_output(args)),
            "read_diagnostics" => Ok(self.mcp_read_diagnostics(args)),
            "beautify" => self.mcp_beautify(args),
            "ide_settings" => Ok(self.mcp_ide_settings()),
            "set_ide_settings" => self.mcp_set_ide_settings(args),
            "ide_state" => Ok(self.mcp_ide_state()),
            "ide_command" => self.mcp_ide_command(args),
            other => Err(format!(
                "'{other}' is not a tool this IDE answers; if it should be, write a gap report with write_gap_report"
            )),
        }
    }

    /// The project folder, or the error every project tool answers without one.
    pub(super) fn mcp_root(&self) -> Result<PathBuf, String> {
        self.project_dir().ok_or_else(|| "no project is open in the IDE".to_owned())
    }

    /// `rel` resolved inside the project folder.
    pub(super) fn mcp_abs(&self, rel: &str) -> Result<PathBuf, String> {
        let root = self.mcp_root()?;
        let rel = Path::new(rel);
        if rel.is_absolute() || rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err(format!("'{}' is not a path inside the project", rel.display()));
        }
        Ok(root.join(rel))
    }
}
