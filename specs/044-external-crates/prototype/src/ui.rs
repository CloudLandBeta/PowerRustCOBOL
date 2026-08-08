// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The interactive prototype of the IDE's **External Crates** dialog.
//!
//! Everything the spec's add/update/remove flows describe, as an egui window:
//! the registry field (R4), search-and-pick (R6), version requirement +
//! features (R7), the conflict verdicts in the log (R12–R15), the registered
//! list with per-crate Update / Remove and Update All (R2, R16–R18), the
//! confirmed removal (R19), and the manifest button (R24–R26).
//!
//! Reuse map: this file is the shape of the final `cobolt-ide` dialog —
//! immediate-mode UI on the main thread, every slow action (network,
//! resolver probe) on a worker thread reporting through an `mpsc` channel,
//! exactly the Runner/FormRuntime channel pattern the IDE already uses.
//! What changes in the IDE: `Tr` strings ×6 instead of literals, the glass
//! theme instead of egui defaults, and the registry base coming from the
//! IDE-wide settings instead of a text field.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

use crate::ops::{self, Note};
use crate::project::CratesFile;
use crate::registry::{Registry, SearchHit};

/// Worker → UI messages. One channel per running action.
enum Msg {
    Note(Note),
    Hits(Vec<SearchHit>),
    Finished(Result<String, String>),
}

/// One rendered log line.
enum LogLine {
    Info(String),
    Warn(String),
    Error(String),
}

pub fn run(registry_base: String, project_dir: PathBuf) -> Result<(), String> {
    let state = CratesFile::load(&project_dir).unwrap_or_default();
    let app = DialogApp {
        registry_base,
        project_dir,
        state,
        query: String::new(),
        hits: Vec::new(),
        sel_name: String::new(),
        sel_req: String::new(),
        sel_features: String::new(),
        log: vec![LogLine::Info(
            "External Crates prototype — search the registry, pick a crate, Add.".into(),
        )],
        busy: None,
        rx: None,
        confirm_remove: None,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([880.0, 660.0])
            .with_title("External Crates — spec 044 prototype"),
        ..Default::default()
    };
    eframe::run_native(
        "External Crates — spec 044 prototype",
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
    .map_err(|e| e.to_string())
}

struct DialogApp {
    registry_base: String,
    project_dir: PathBuf,
    state: CratesFile,

    query: String,
    hits: Vec<SearchHit>,
    sel_name: String,
    sel_req: String,
    sel_features: String,

    log: Vec<LogLine>,
    /// `Some(label)` while a worker runs; buttons disable, a spinner shows.
    busy: Option<String>,
    rx: Option<Receiver<Msg>>,
    /// Crate name awaiting the R19 confirmation.
    confirm_remove: Option<String>,
}

impl DialogApp {
    /// Spawn a worker for a slow action. The closure gets a registry built
    /// from the *current* base field (R4: applies to the next action) and a
    /// sender for progress notes; its return value ends the busy state.
    fn spawn(
        &mut self,
        label: &str,
        work: impl FnOnce(Registry, PathBuf, Sender<Msg>) -> Result<String, String> + Send + 'static,
    ) {
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.busy = Some(label.to_string());
        let base = self.registry_base.clone();
        let project = self.project_dir.clone();
        std::thread::spawn(move || {
            let registry = Registry::new(&base);
            let result = work(registry, project, tx.clone());
            let _ = tx.send(Msg::Finished(result));
        });
    }

    fn drain_worker(&mut self) {
        let Some(rx) = &self.rx else { return };
        let mut done = false;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Msg::Note(Note::Info(text)) => self.log.push(LogLine::Info(text)),
                Msg::Note(Note::Warn(text)) => self.log.push(LogLine::Warn(text)),
                Msg::Hits(hits) => {
                    if hits.is_empty() {
                        self.log.push(LogLine::Info("no crates match".into()));
                    }
                    self.hits = hits;
                }
                Msg::Finished(Ok(text)) => {
                    self.log.push(LogLine::Info(text));
                    done = true;
                }
                Msg::Finished(Err(text)) => {
                    self.log.push(LogLine::Error(text));
                    done = true;
                }
            }
        }
        if done {
            self.busy = None;
            self.rx = None;
            // An action may have changed the pin file — reflect it (R2).
            self.state = CratesFile::load(&self.project_dir).unwrap_or_default();
        }
    }

    fn features_vec(&self) -> Vec<String> {
        self.sel_features
            .split(',')
            .map(|f| f.trim().to_string())
            .filter(|f| !f.is_empty())
            .collect()
    }
}

/// What a registered-row button asked for, applied after the list loop.
enum RowAction {
    Update(String),
    ConfirmRemove(String),
}

impl eframe::App for DialogApp {
    // egui 0.36: the App renders into a root `Ui`; panels host on it and the
    // Context comes from `ui.ctx()` (spec-027 upgrade conventions).
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root_ui.ctx().clone();
        self.drain_worker();
        if self.busy.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }

        egui::Panel::top("registry").show(root_ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("External Crates");
                if let Some(label) = &self.busy {
                    ui.spinner();
                    ui.label(label.clone());
                }
            });
            ui.horizontal(|ui| {
                ui.label("Registry:");
                ui.add_sized(
                    [360.0, 20.0],
                    egui::TextEdit::singleline(&mut self.registry_base),
                );
                ui.weak("crates.io-compatible; used by the next action (IDE-wide setting)");
            });
            ui.add_space(6.0);
        });

        egui::CentralPanel::default().show(root_ui, |ui| {
            let idle = self.busy.is_none();

            // ── Search (R6) ──────────────────────────────────────────────
            ui.add_space(4.0);
            let mut do_search = false;
            ui.horizontal(|ui| {
                ui.label("Search:");
                let field = ui.add_sized(
                    [300.0, 20.0],
                    egui::TextEdit::singleline(&mut self.query).hint_text("e.g. csv"),
                );
                if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    do_search = true;
                }
                if ui
                    .add_enabled(idle && !self.query.trim().is_empty(), egui::Button::new("🔍 Search"))
                    .clicked()
                {
                    do_search = true;
                }
            });
            if do_search && idle && !self.query.trim().is_empty() {
                let query = self.query.trim().to_string();
                self.spawn("searching…", move |registry, _project, tx| {
                    let hits = registry.search(&query, 10).map_err(|e| e.to_string())?;
                    let n = hits.len();
                    let _ = tx.send(Msg::Hits(hits));
                    Ok(format!("{n} result(s) for \"{query}\""))
                });
            }

            if !self.hits.is_empty() {
                egui::ScrollArea::vertical()
                    .id_salt("hits")
                    .max_height(150.0)
                    .show(ui, |ui| {
                        for hit in &self.hits {
                            let selected = self.sel_name == hit.name;
                            let mut desc = hit.description.clone();
                            if desc.len() > 70 {
                                desc.truncate(69);
                                desc.push('…');
                            }
                            let text = format!("{}  {}  — {}", hit.name, hit.newest, desc);
                            if ui.selectable_label(selected, text).clicked() {
                                self.sel_name = hit.name.clone();
                            }
                        }
                    });
            }

            // ── Add (R7) ─────────────────────────────────────────────────
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Crate:");
                ui.add_sized([160.0, 20.0], egui::TextEdit::singleline(&mut self.sel_name));
                ui.label("Version req:");
                ui.add_sized(
                    [90.0, 20.0],
                    egui::TextEdit::singleline(&mut self.sel_req).hint_text("newest"),
                );
                ui.label("Features:");
                ui.add_sized(
                    [140.0, 20.0],
                    egui::TextEdit::singleline(&mut self.sel_features).hint_text("a, b"),
                );
                if ui
                    .add_enabled(idle && !self.sel_name.trim().is_empty(), egui::Button::new("➕ Add"))
                    .clicked()
                {
                    let name = self.sel_name.trim().to_string();
                    let req = self.sel_req.trim().to_string();
                    let features = self.features_vec();
                    self.spawn(&format!("adding {name}…"), move |registry, project, tx| {
                        let req = (!req.is_empty()).then_some(req.as_str());
                        ops::add(&registry, &project, None, false, &name, req, features, &mut |n| {
                            let _ = tx.send(Msg::Note(n));
                        })
                    });
                }
            });

            // ── Registered list (R2, R16–R19) ────────────────────────────
            ui.separator();
            let mut action: Option<RowAction> = None;
            ui.horizontal(|ui| {
                ui.strong(format!("Registered ({})", self.state.crates.len()));
                if ui
                    .add_enabled(idle && !self.state.crates.is_empty(), egui::Button::new("⟳ Update All"))
                    .clicked()
                {
                    action = Some(RowAction::Update(String::new()));
                }
            });
            if self.state.crates.is_empty() {
                ui.weak("none yet — search above and Add one");
            }
            for c in &self.state.crates {
                ui.horizontal(|ui| {
                    ui.monospace(format!("{} {}", c.name, c.version));
                    let requirement = if c.requirement.is_empty() {
                        "newest stable".to_string()
                    } else {
                        format!("req {}", c.requirement)
                    };
                    ui.weak(requirement);
                    if !c.features.is_empty() {
                        ui.weak(format!("features: {}", c.features.join(", ")));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_enabled(idle, egui::Button::new("✖ Remove")).clicked() {
                            action = Some(RowAction::ConfirmRemove(c.name.clone()));
                        }
                        if ui.add_enabled(idle, egui::Button::new("⟳ Update")).clicked() {
                            action = Some(RowAction::Update(c.name.clone()));
                        }
                        ui.hyperlink_to("↗", &c.url);
                    });
                });
            }
            match action {
                Some(RowAction::Update(name)) => {
                    let targets: Vec<String> =
                        if name.is_empty() { Vec::new() } else { vec![name] };
                    self.spawn("updating…", move |registry, project, tx| {
                        ops::update(&registry, &project, None, false, &targets, &mut |n| {
                            let _ = tx.send(Msg::Note(n));
                        })
                    });
                }
                Some(RowAction::ConfirmRemove(name)) => self.confirm_remove = Some(name),
                None => {}
            }

            // ── Manifest (R24–R26) ───────────────────────────────────────
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(idle, egui::Button::new("📄 Write rust_manifest.md"))
                    .clicked()
                {
                    match ops::write_manifest(&self.project_dir, None) {
                        Ok(text) => self.log.push(LogLine::Info(text)),
                        Err(text) => self.log.push(LogLine::Error(text)),
                    }
                }
                ui.weak(format!(
                    "→ {}",
                    self.project_dir.join("dist/rust_manifest.md").display()
                ));
            });

            // ── Log ──────────────────────────────────────────────────────
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("log")
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &self.log {
                        match line {
                            LogLine::Info(text) => ui.label(text),
                            LogLine::Warn(text) => ui.colored_label(
                                egui::Color32::from_rgb(230, 150, 30),
                                format!("warning: {text}"),
                            ),
                            LogLine::Error(text) => ui.colored_label(
                                egui::Color32::from_rgb(225, 70, 70),
                                format!("error: {text}"),
                            ),
                        };
                    }
                });
        });

        // ── R19 confirmation modal ───────────────────────────────────────
        if let Some(name) = self.confirm_remove.clone() {
            egui::Window::new("Remove crate?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(&ctx, |ui| {
                    ui.label(format!(
                        "Remove `{name}` and delete its vendored source?\n\
                         Blocks still using it will fail Check as unregistered."
                    ));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Remove").clicked() {
                            match ops::remove(&self.project_dir, &name) {
                                Ok(text) => self.log.push(LogLine::Info(text)),
                                Err(text) => self.log.push(LogLine::Error(text)),
                            }
                            self.state =
                                CratesFile::load(&self.project_dir).unwrap_or_default();
                            self.confirm_remove = None;
                        }
                        if ui.button("Keep").clicked() {
                            self.confirm_remove = None;
                        }
                    });
                });
        }
    }
}
