// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Backend window of a Web or Mobile project, opened by the Backend button of the top bar
//! and by a new Backend run (operator, 2026-11-08). Three tabs:
//!
//! * **Log**: what the Backend prints;
//! * **Processes**: the processes involved, with their memory;
//! * **Traffic**: the HTTP exchanges and the WebSocket packets between the page and the Backend, as
//!   the inspector's proxy records them (`cobolt_project_tools::web::inspector`), with the headers
//!   and the pretty-printed JSON of each; the developer can **hold** the requests before the Backend
//!   sees them and the replies before the page sees them, and forward them **edited** or drop them.
//!
//! Closing the window leaves the Backend running; **Stop Backend** (in the window) and the Backend
//! button's click on a failed mark are the ways to stop it.
//!
//! Its size is its own: only the developer's drag on the grip changes it
//! (the model is `leaderboard_modal`).

use std::time::{Duration, Instant};

use cobolt_project_tools::web::inspector::{Decision, Direction, Entry, Inspector, Kind, State};
use egui::{Color32, RichText};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::i18n::Tr;
use crate::theme::Theme;

const DEFAULT_W: f32 = 940.0;
const DEFAULT_H: f32 = 640.0;
const MIN_W: f32 = 560.0;
const MIN_H: f32 = 360.0;
const MAX_W: f32 = 1800.0;
const MAX_H: f32 = 1200.0;
const GRIP: f32 = 14.0;
const GRIP_INSET: f32 = 3.0;
/// The title bar, the status line, the button and tab rows and the paddings: a constant
/// subtracted from the height to give the body, so the body never measures itself
/// against the space it was handed.
const CHROME_H: f32 = 150.0;
/// The log keeps the newest lines only.
const MAX_LINES: usize = 5000;
/// Memory is read this often.
const MEMORY_EVERY: Duration = Duration::from_secs(1);

/// What the developer asked for in a frame.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BackendWindowAction {
    /// Stop the Backend (the window's own button).
    pub stop: bool,
}

/// What the IDE knows about the running pieces, once a frame.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub running: bool,
    pub backend_pid: Option<u32>,
    /// The port the Preview serves the page on.
    pub page_port: Option<u16>,
    /// What the page calls (the inspector's proxy), when there is one.
    pub proxy_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Traffic,
    Processes,
    Log,
}

pub struct BackendWindow {
    /// Shown or hidden. Set when a Backend starts; cleared by a close.
    visible: bool,
    size: egui::Vec2,
    lines: Vec<String>,
    /// The address the Backend listens on, once it printed it.
    url: Option<String>,
    tab: Tab,
    /// The entry whose detail is shown.
    selected: Option<u64>,
    /// The edit box: the entry it belongs to and its text.
    editing: Option<(u64, String)>,
    /// The newest held entry seen, so a new one is selected once.
    last_held: u64,
    sys: System,
    memory: Vec<(Pid, u64)>,
    sampled: Option<Instant>,
}

impl Default for BackendWindow {
    fn default() -> Self {
        Self {
            visible: false,
            size: egui::vec2(DEFAULT_W, DEFAULT_H),
            lines: Vec::new(),
            url: None,
            tab: Tab::default(),
            selected: None,
            editing: None,
            last_held: 0,
            sys: System::new(),
            memory: Vec::new(),
            sampled: None,
        }
    }
}

/// The text a row of the Traffic tab shows for an entry: the packet's kind, its program and its id.
pub fn summary(e: &Entry) -> String {
    match e.kind {
        Kind::Http | Kind::Handshake => e.line.clone(),
        Kind::Close => "close".to_owned(),
        Kind::Binary => e.body.clone(),
        Kind::Text => match serde_json::from_str::<serde_json::Value>(&e.body) {
            Ok(v) => {
                let field = |k: &str| v.get(k).map(|x| x.as_str().map(str::to_owned).unwrap_or_else(|| x.to_string()));
                let mut parts: Vec<String> = Vec::new();
                for k in ["t", "program", "id"] {
                    if let Some(x) = field(k) {
                        parts.push(if k == "id" { format!("#{x}") } else { x });
                    }
                }
                if parts.is_empty() {
                    e.body.chars().take(80).collect()
                } else {
                    parts.join(" ")
                }
            }
            Err(_) => e.body.chars().take(80).collect(),
        },
    }
}

/// The body as the viewer shows it: pretty JSON when it is JSON, else as it is.
pub fn pretty(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body).ok().and_then(|v| serde_json::to_string_pretty(&v).ok()).unwrap_or_else(|| body.to_owned())
}

/// `1.2 MB`-style size for the Processes tab.
pub fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

impl BackendWindow {
    /// A Backend started: a fresh log, and the window up.
    pub fn begin(&mut self) {
        self.lines.clear();
        self.url = None;
        self.visible = true;
    }

    pub fn push_line(&mut self, line: String) {
        self.lines.push(line);
        if self.lines.len() > MAX_LINES {
            let drop = self.lines.len() - MAX_LINES;
            self.lines.drain(..drop);
        }
    }

    pub fn set_url(&mut self, url: String) {
        self.url = Some(url);
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// The Backend button: opens the window, or closes it when it is open.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// The close button (or Esc): the window goes, the Backend keeps running.
    pub fn close(&mut self) {
        self.visible = false;
    }

    fn refresh_memory(&mut self, facts: &Facts) {
        if self.sampled.is_some_and(|t| t.elapsed() < MEMORY_EVERY) {
            return;
        }
        self.sampled = Some(Instant::now());
        let mut pids = vec![Pid::from_u32(std::process::id())];
        if let Some(p) = facts.backend_pid {
            pids.push(Pid::from_u32(p));
        }
        self.sys.refresh_processes_specifics(ProcessesToUpdate::Some(&pids), ProcessRefreshKind::new().with_memory());
        self.memory = pids.iter().filter_map(|p| self.sys.process(*p).map(|proc_| (*p, proc_.memory()))).collect();
    }

    fn memory_of(&self, pid: u32) -> String {
        self.memory.iter().find(|(p, _)| p.as_u32() == pid).map_or_else(|| "—".to_owned(), |(_, m)| megabytes(*m))
    }

    pub fn show(&mut self, ctx: &egui::Context, theme: &Theme, tr: &Tr, inspector: &Inspector, facts: &Facts) -> BackendWindowAction {
        let mut action = BackendWindowAction::default();
        if !self.visible {
            return action;
        }
        self.refresh_memory(facts);
        // Something new was held: show it.
        if let Some(held) = inspector.entries().iter().rev().find(|e| e.state == State::Held) {
            if held.id > self.last_held {
                self.last_held = held.id;
                self.selected = Some(held.id);
                self.tab = Tab::Traffic;
            }
        }
        ctx.request_repaint_after(Duration::from_millis(500));
        let mut open = true;
        let window = egui::Window::new(RichText::new(tr.backend_window_title).size(18.0).strong())
            .id(egui::Id::new("backend_window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .fixed_size(self.size)
            .default_pos([60.0, 80.0])
            .show(ctx, |ui| {
                let margin = ui.style().spacing.window_margin.sum().x;
                let stroke = 2.0 * ui.style().visuals.window_stroke.width;
                ui.set_width(self.size.x - margin - stroke);
                ui.horizontal(|ui| {
                    let (label, colour) = if facts.running {
                        (self.url.clone().map(|u| tr.backend_running.replacen("{}", &u, 1)).unwrap_or_else(|| "…".to_owned()), Color32::from_rgb(60, 190, 100))
                    } else {
                        (tr.backend_stopped.to_owned(), theme.text_dim)
                    };
                    if facts.running && self.url.is_none() {
                        ui.add(egui::Spinner::new());
                    }
                    ui.label(RichText::new(label).color(colour));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_enabled(facts.running, egui::Button::new(tr.backend_stop_here)).clicked() {
                            action.stop = true;
                        }
                    });
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for (tab, name) in [(Tab::Traffic, tr.backend_tab_traffic), (Tab::Processes, tr.backend_tab_processes), (Tab::Log, tr.backend_tab_log)] {
                        ui.selectable_value(&mut self.tab, tab, name);
                    }
                });
                ui.separator();
                let body_h = (self.size.y - CHROME_H).max(60.0);
                match self.tab {
                    Tab::Log => self.log_tab(ui, tr, body_h),
                    Tab::Processes => self.processes_tab(ui, tr, facts, inspector),
                    Tab::Traffic => self.traffic_tab(ui, theme, tr, inspector, body_h),
                }
            });
        if let Some(window) = window {
            self.grip(ctx, theme, window.response.rect);
        }
        if !open {
            self.close();
        }
        action
    }

    fn log_tab(&mut self, ui: &mut egui::Ui, tr: &Tr, body_h: f32) {
        if ui.button(tr.clipboard_copy).clicked() {
            ui.ctx().copy_text(self.lines.join("\n"));
        }
        let weak = ui.visuals().weak_text_color();
        egui::ScrollArea::vertical().max_height(body_h - 30.0).min_scrolled_height(body_h - 30.0).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
            for line in &self.lines {
                ui.label(RichText::new(line).monospace().color(weak));
            }
        });
    }

    fn processes_tab(&mut self, ui: &mut egui::Ui, tr: &Tr, facts: &Facts, inspector: &Inspector) {
        let me = std::process::id();
        egui::Grid::new("backend_processes").num_columns(4).striped(true).spacing([18.0, 6.0]).show(ui, |ui| {
            for h in [tr.backend_col_process, tr.backend_col_pid, tr.backend_col_memory, tr.backend_col_detail] {
                ui.label(RichText::new(h).strong());
            }
            ui.end_row();
            if let Some(pid) = facts.backend_pid {
                ui.label(tr.backend_proc_backend);
                ui.label(pid.to_string());
                ui.label(self.memory_of(pid));
                ui.label(self.url.clone().unwrap_or_default());
                ui.end_row();
            }
            ui.label(tr.backend_proc_ide);
            ui.label(me.to_string());
            ui.label(self.memory_of(me));
            ui.label(facts.page_port.map(|p| format!("127.0.0.1:{p}")).unwrap_or_default());
            ui.end_row();
            if let Some(url) = &facts.proxy_url {
                ui.label(tr.backend_proc_proxy);
                ui.label(me.to_string());
                ui.label("");
                ui.label(url);
                ui.end_row();
            }
        });
        ui.add_space(10.0);
        ui.label(RichText::new(tr.backend_connections).strong());
        egui::Grid::new("backend_connections").num_columns(5).striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
            for c in inspector.connections().iter().rev().take(30) {
                ui.label(format!("#{}", c.conn));
                ui.label(if c.websocket { "WebSocket" } else { "HTTP" });
                ui.label(&c.path);
                ui.label(if c.open { "●" } else { "○" });
                ui.label(if c.websocket { format!("{} · {} B", c.frames, c.bytes) } else { String::new() });
                ui.end_row();
            }
        });
    }

    fn traffic_tab(&mut self, ui: &mut egui::Ui, theme: &Theme, tr: &Tr, inspector: &Inspector, body_h: f32) {
        ui.horizontal(|ui| {
            let mut requests = inspector.holds_requests();
            if ui.checkbox(&mut requests, tr.backend_hold_requests).changed() {
                inspector.set_hold_requests(requests);
            }
            let mut replies = inspector.holds_replies();
            if ui.checkbox(&mut replies, tr.backend_hold_replies).changed() {
                inspector.set_hold_replies(replies);
            }
            if ui.button(tr.backend_clear).clicked() {
                inspector.clear();
                self.selected = None;
            }
        });
        let entries = inspector.entries();
        let list_h = (body_h * 0.42).max(80.0);
        egui::ScrollArea::vertical().id_salt("backend_traffic").max_height(list_h).min_scrolled_height(list_h).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
            if entries.is_empty() {
                ui.label(RichText::new(tr.backend_no_traffic).color(theme.text_dim));
            }
            for e in &entries {
                let arrow = if e.dir == Direction::ToBackend { "↑" } else { "↓" };
                let colour = match e.state {
                    State::Held => Color32::from_rgb(240, 170, 40),
                    State::Edited => Color32::from_rgb(90, 160, 240),
                    State::Dropped => Color32::from_rgb(229, 72, 77),
                    State::Passed => ui.visuals().text_color(),
                };
                let text = format!("#{:<4} {:>7} ms  c{} {} {}  {}", e.id, e.at_ms, e.conn, arrow, state_name(tr, e.state), summary(e));
                if ui.selectable_label(self.selected == Some(e.id), RichText::new(text).monospace().color(colour)).clicked() {
                    self.selected = Some(e.id);
                    self.editing = None;
                }
            }
        });
        ui.separator();
        let Some(entry) = self.selected.and_then(|id| entries.iter().find(|e| e.id == id)) else { return };
        self.detail(ui, theme, tr, inspector, entry, body_h - list_h - 40.0);
    }

    fn detail(&mut self, ui: &mut egui::Ui, theme: &Theme, tr: &Tr, inspector: &Inspector, entry: &Entry, height: f32) {
        egui::ScrollArea::vertical().id_salt("backend_detail").max_height(height.max(60.0)).min_scrolled_height(height.max(60.0)).auto_shrink([false, false]).show(ui, |ui| {
            if !entry.line.is_empty() {
                ui.label(RichText::new(&entry.line).monospace().strong());
            }
            if !entry.headers.is_empty() {
                ui.label(RichText::new(tr.backend_headers).strong());
                for (k, v) in &entry.headers {
                    ui.label(RichText::new(format!("{k}: {v}")).monospace().color(theme.text_dim));
                }
            }
            if entry.kind == Kind::Http {
                ui.label(RichText::new(tr.backend_http_note).color(theme.text_dim));
            }
            if entry.state == State::Held {
                if self.editing.as_ref().is_none_or(|(id, _)| *id != entry.id) {
                    self.editing = Some((entry.id, pretty(&entry.body)));
                }
                if let Some((_, text)) = self.editing.as_mut() {
                    ui.add(egui::TextEdit::multiline(text).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY).desired_rows(8));
                }
                ui.horizontal(|ui| {
                    if ui.button(tr.backend_forward).clicked() {
                        inspector.decide(entry.id, Decision::Forward(entry.body.clone()));
                        self.editing = None;
                    }
                    if ui.button(tr.backend_forward_edited).clicked() {
                        if let Some((_, text)) = self.editing.take() {
                            // The edit is sent as one line: the protocol's frames are compact JSON.
                            let compact = serde_json::from_str::<serde_json::Value>(&text).ok().and_then(|v| serde_json::to_string(&v).ok()).unwrap_or(text);
                            inspector.decide(entry.id, Decision::Forward(compact));
                        }
                    }
                    if ui.button(tr.backend_drop).clicked() {
                        inspector.decide(entry.id, Decision::Drop);
                        self.editing = None;
                    }
                });
            } else if !entry.body.is_empty() {
                ui.label(RichText::new(tr.backend_body).strong());
                if let Some(original) = &entry.original {
                    ui.label(RichText::new(pretty(original)).monospace().strikethrough().color(theme.text_dim));
                }
                let mut shown = pretty(&entry.body);
                ui.add(egui::TextEdit::multiline(&mut shown).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY).interactive(true).lock_focus(false));
                if ui.button(tr.clipboard_copy).clicked() {
                    ui.ctx().copy_text(shown);
                }
            }
        });
    }

    fn grip(&mut self, ctx: &egui::Context, theme: &Theme, window: egui::Rect) {
        let origin = window.max - egui::vec2(GRIP_INSET + GRIP, GRIP_INSET + GRIP);
        egui::Area::new(egui::Id::new("backend_window_resize_grip")).order(egui::Order::Foreground).fixed_pos(origin).show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(egui::vec2(GRIP, GRIP), egui::Sense::drag());
            if response.dragged() {
                self.size += response.drag_delta();
                self.size.x = self.size.x.clamp(MIN_W, MAX_W);
                self.size.y = self.size.y.clamp(MIN_H, MAX_H);
            }
            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNorthWest);
            }
            let colour = if response.hovered() || response.dragged() { theme.accent } else { theme.text_dim };
            for (i, offset) in [2.0_f32, 6.0, 10.0].iter().enumerate() {
                let len = GRIP - offset - 1.0;
                let stroke = egui::Stroke::new(if i == 0 { 1.6 } else { 1.2 }, colour);
                ui.painter().line_segment([egui::pos2(rect.right() - len, rect.bottom()), egui::pos2(rect.right(), rect.bottom() - len)], stroke);
            }
        });
    }
}

fn state_name<'a>(tr: &'a Tr, state: State) -> &'a str {
    match state {
        State::Passed => tr.backend_state_passed,
        State::Held => tr.backend_state_held,
        State::Edited => tr.backend_state_edited,
        State::Dropped => tr.backend_state_dropped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: Kind, body: &str) -> Entry {
        Entry { id: 1, conn: 1, at_ms: 0, dir: Direction::ToBackend, kind, line: String::new(), headers: Vec::new(), body: body.to_owned(), state: State::Passed, original: None }
    }

    #[test]
    fn the_window_opens_with_the_button_and_a_close_leaves_the_backend_alone() {
        let mut w = BackendWindow::default();
        assert!(!w.is_visible());
        w.toggle();
        assert!(w.is_visible(), "the Backend button opens it");
        w.close();
        assert!(!w.is_visible());
        w.begin();
        assert!(w.is_visible(), "a new run opens it too");
        w.toggle();
        assert!(!w.is_visible(), "the button closes it again");
    }

    #[test]
    fn the_log_keeps_the_newest_lines_and_a_new_run_starts_clean() {
        let mut w = BackendWindow::default();
        w.begin();
        for i in 0..MAX_LINES + 10 {
            w.push_line(format!("line {i}"));
        }
        assert_eq!(w.lines.len(), MAX_LINES);
        assert_eq!(w.lines.first().unwrap(), "line 10");
        w.set_url("ws://127.0.0.1:8443".into());
        w.begin();
        assert!(w.lines.is_empty() && w.url.is_none());
    }

    #[test]
    fn a_packet_reads_as_its_kind_program_and_id_and_json_is_pretty_printed() {
        assert_eq!(summary(&entry(Kind::Text, "{\"t\":\"call\",\"id\":7,\"program\":\"QUOTE-GET\"}")), "call QUOTE-GET #7");
        assert_eq!(summary(&entry(Kind::Text, "not json at all")), "not json at all");
        assert_eq!(summary(&entry(Kind::Close, "")), "close");
        assert_eq!(pretty("{\"a\":1}"), "{\n  \"a\": 1\n}");
        assert_eq!(pretty("plain"), "plain");
        assert_eq!(megabytes(3 * 1024 * 1024 / 2), "1.5 MB");
    }
}
