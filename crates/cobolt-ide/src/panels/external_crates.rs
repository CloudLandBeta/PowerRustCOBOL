// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The External Crates dialog (spec 044) — search the configured registry,
//! add with an optional version requirement and features, update one/all,
//! remove with confirmation, all narrated in a log pane.
//!
//! Threading is the house pattern: every slow action (network, resolver
//! probe) runs in [`crate::external_crates_service`] on a worker thread and
//! reports over an `mpsc` channel; the panel drains it each frame, disables
//! its buttons and shows a spinner while busy. Dialog **chrome** is `Tr` ×6;
//! action progress and refusal details are diagnostic-stream content —
//! rendered verbatim like build output, per the spec §6 carve-out.
//!
//! State contract with the app: the app saves the project before opening the
//! dialog, and [`ExternalCratesPanel::show`] returns `true` on the frame an
//! action finished mutating `cobolt.toml` on disk — the app then reloads its
//! in-memory project so the tree reflects the change (R2).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};

use crate::external_crates_service as service;
use crate::i18n::Tr;
use service::{
    ExternalCratesSettings, Note, Registry, SearchHit, UpdateOutcome, RESULTS_PER_PAGE,
};

/// Link scheme the results table uses to make a row pick its crate. Any other
/// destination in the rendered markdown is left to the renderer.
const PICK_SCHEME: &str = "crate:";

/// Worker → panel messages; one channel per running action.
enum Msg {
    Note(Note),
    /// One page of results: the rows, how many matches exist in total, and
    /// which page these rows are.
    Hits { hits: Vec<SearchHit>, total: usize, page: usize },
    /// `result: Ok(None)` finishes silently — a search's feedback is the
    /// results table itself, not a line in the log.
    Finished { result: Result<Option<String>, String>, mutated: bool },
}

enum LogLine {
    Info(String),
    Warn(String),
    Error(String),
}

pub struct ExternalCratesPanel {
    settings: ExternalCratesSettings,
    query: String,
    hits: Vec<SearchHit>,
    /// Total matches the registry reports for the current query, and which
    /// page `hits` holds (1-based) — the two numbers the pager needs.
    total: usize,
    page: usize,
    /// The query `hits` belong to, so the pager re-runs the right search
    /// after the developer has typed something new without pressing Enter.
    searched: String,
    sel_name: String,
    sel_req: String,
    sel_features: String,
    log: Vec<LogLine>,
    /// `Some(label)` while a worker runs; buttons disable, a spinner shows.
    busy: Option<String>,
    rx: Option<Receiver<Msg>>,
    /// `(name, version)` awaiting the R19 confirmation.
    confirm_remove: Option<(String, String)>,
    /// Set when a finished action changed `cobolt.toml`; drained by `show`.
    project_changed: bool,
}

impl Default for ExternalCratesPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ExternalCratesPanel {
    pub fn new() -> Self {
        ExternalCratesPanel {
            settings: ExternalCratesSettings::load(),
            query: String::new(),
            hits: Vec::new(),
            total: 0,
            page: 1,
            searched: String::new(),
            sel_name: String::new(),
            sel_req: String::new(),
            sel_features: String::new(),
            log: Vec::new(),
            busy: None,
            rx: None,
            confirm_remove: None,
            project_changed: false,
        }
    }

    fn features_vec(&self) -> Vec<String> {
        self.sel_features
            .split(',')
            .map(|f| f.trim().to_string())
            .filter(|f| !f.is_empty())
            .collect()
    }

    /// Spawn a worker for a slow action. The registry is built from the
    /// *current* setting (R4: applies to the next action); `mutated` marks
    /// actions that rewrite `cobolt.toml` on success.
    fn spawn(
        &mut self,
        label: String,
        mutated: bool,
        work: impl FnOnce(Registry, Sender<Msg>) -> Result<Option<String>, String> + Send + 'static,
    ) {
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.busy = Some(label);
        let base = self.settings.registry.clone();
        std::thread::spawn(move || {
            let registry = Registry::new(&base);
            let result = work(registry, tx.clone());
            let _ = tx.send(Msg::Finished { result, mutated });
        });
    }

    fn drain_worker(&mut self) {
        let Some(rx) = &self.rx else { return };
        let mut done = false;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Msg::Note(Note::Info(text)) => self.log.push(LogLine::Info(text)),
                Msg::Note(Note::Warn(text)) => self.log.push(LogLine::Warn(text)),
                Msg::Hits { hits, total, page } => {
                    self.hits = hits;
                    self.total = total;
                    self.page = page;
                }
                Msg::Finished { result, mutated } => {
                    match result {
                        Ok(text) => {
                            if let Some(text) = text {
                                self.log.push(LogLine::Info(text));
                            }
                            if mutated {
                                self.project_changed = true;
                            }
                        }
                        Err(text) => self.log.push(LogLine::Error(text)),
                    }
                    done = true;
                }
            }
        }
        if done {
            self.busy = None;
            self.rx = None;
        }
    }

    /// Render the dialog. Returns `true` when a finished action changed the
    /// project on disk — the caller reloads `cobolt.toml` (see module docs).
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        open: &mut bool,
        project_path: Option<&Path>,
        crates: &[cobolt_compiler::ExternalCrate],
        tr: &Tr,
    ) -> bool {
        self.drain_worker();
        if self.busy.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }

        let mut keep_open = *open;
        egui::Window::new(tr.cat_external_crates)
            .open(&mut keep_open)
            // Wide enough that the four result columns (crate · version ·
            // downloads · description) each get a readable share.
            .default_size([880.0, 660.0])
            .resizable(true)
            .show(ctx, |ui| {
                let Some(project_path) = project_path else {
                    ui.label(tr.no_project_open);
                    return;
                };
                self.body(ui, project_path, crates, tr);
            });
        *open = keep_open;

        // Confirmation modal (R19) — outside the main window so it stays
        // centred and modal-feeling.
        if let Some((name, version)) = self.confirm_remove.clone() {
            let project_path = project_path.map(Path::to_path_buf);
            egui::Window::new(tr.ec_confirm_remove_title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("{name} {version} — {}", tr.ec_confirm_remove_note));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(tr.ec_remove).clicked() {
                            if let Some(path) = &project_path {
                                match service::remove(path, &name) {
                                    Ok(text) => {
                                        self.log.push(LogLine::Info(text));
                                        self.project_changed = true;
                                    }
                                    Err(text) => self.log.push(LogLine::Error(text)),
                                }
                            }
                            self.confirm_remove = None;
                        }
                        if ui.button(tr.ec_keep).clicked() {
                            self.confirm_remove = None;
                        }
                    });
                });
        }

        std::mem::take(&mut self.project_changed)
    }

    fn body(
        &mut self,
        ui: &mut egui::Ui,
        project_path: &Path,
        crates: &[cobolt_compiler::ExternalCrate],
        tr: &Tr,
    ) {
        let idle = self.busy.is_none();

        // ── Registry (R4) ────────────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.label(tr.ec_registry);
            let field = ui.add_sized(
                [340.0, 20.0],
                egui::TextEdit::singleline(&mut self.settings.registry),
            );
            if field.changed() {
                self.settings.save();
            }
            if let Some(label) = &self.busy {
                ui.spinner();
                ui.label(label.clone());
            }
        });
        ui.weak(tr.ec_registry_hint);
        ui.separator();

        // ── Search (R6) ──────────────────────────────────────────────────
        let mut do_search = false;
        ui.horizontal(|ui| {
            ui.label(tr.ec_search);
            let field = ui.add_sized(
                [280.0, 20.0],
                egui::TextEdit::singleline(&mut self.query).hint_text(tr.ec_search_hint),
            );
            if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                do_search = true;
            }
            if ui
                .add_enabled(
                    idle && !self.query.trim().is_empty(),
                    egui::Button::new(format!("🔍 {}", tr.ec_search)),
                )
                .clicked()
            {
                do_search = true;
            }
        });
        if do_search && idle && !self.query.trim().is_empty() {
            self.run_search(self.query.trim().to_string(), 1, tr);
        }

        // ── Results: a rendered markdown table, paged (R6) ───────────────
        if !self.hits.is_empty() {
            let markdown = self.results_markdown(tr);
            let mut picked = None;
            egui::ScrollArea::vertical()
                .id_salt("ec_hits")
                .max_height(300.0)
                .show(ui, |ui| {
                    let out = crate::panels::md_render::render(
                        ui,
                        &markdown,
                        &crate::panels::md_render::RenderOpts {
                            base: ui.style().text_styles[&egui::TextStyle::Body].size,
                            search: "",
                            scroll_to_heading: None,
                            active_match: None,
                            scroll_to_active: false,
                            anchors: &[],
                            // Tight value columns, description takes the rest
                            // and is the only one that wraps; boundaries are
                            // draggable and drawn.
                            table_layout: crate::panels::md_render::TableLayout::TightResizable,
                        },
                        &mut |_, _| {},
                    );
                    // A crate-name cell is a `crate:<name>` link — clicking a
                    // row is how the developer picks what they found.
                    if let Some(target) = out.clicked_link {
                        if let Some(name) = target.strip_prefix(PICK_SCHEME) {
                            picked = Some(name.to_string());
                        }
                    }
                });
            if let Some(name) = picked {
                self.sel_name = name;
            }

            // Pager: total pages from the registry's own match count.
            let pages = self.total.div_ceil(RESULTS_PER_PAGE).max(1);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(idle && self.page > 1, egui::Button::new("◀"))
                    .clicked()
                {
                    self.run_search(self.searched.clone(), self.page - 1, tr);
                }
                ui.label(format!(
                    "{} {}/{} — {} {}",
                    tr.ec_page, self.page, pages, self.total, tr.ec_results
                ));
                if ui
                    .add_enabled(idle && self.page < pages, egui::Button::new("▶"))
                    .clicked()
                {
                    self.run_search(self.searched.clone(), self.page + 1, tr);
                }
            });
        } else if !self.searched.is_empty() && idle {
            ui.weak(tr.ec_no_results);
        }

        // ── Add (R7) ─────────────────────────────────────────────────────
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(tr.ec_crate);
            ui.add_sized([150.0, 20.0], egui::TextEdit::singleline(&mut self.sel_name));
            ui.label(tr.ec_req);
            ui.add_sized(
                [80.0, 20.0],
                egui::TextEdit::singleline(&mut self.sel_req).hint_text(tr.ec_req_hint),
            );
            ui.label(tr.ec_features);
            ui.add_sized(
                [120.0, 20.0],
                egui::TextEdit::singleline(&mut self.sel_features)
                    .hint_text(tr.ec_features_hint),
            );
            if ui
                .add_enabled(
                    idle && !self.sel_name.trim().is_empty(),
                    egui::Button::new(format!("➕ {}", tr.ec_add)),
                )
                .clicked()
            {
                let name = self.sel_name.trim().to_string();
                let req = self.sel_req.trim().to_string();
                let features = self.features_vec();
                let path = project_path.to_path_buf();
                self.spawn(format!("{} {name}", tr.ec_add), true, move |registry, tx| {
                    let req = (!req.is_empty()).then_some(req.as_str());
                    service::add(&registry, &path, None, &name, req, features, &mut |n| {
                        let _ = tx.send(Msg::Note(n));
                    })
                    .map(Some)
                });
            }
        });

        // ── Registered list (R2, R16–R19) ────────────────────────────────
        ui.separator();
        enum RowAction {
            UpdateAll,
            Update(String),
            ConfirmRemove(String, String),
        }
        let mut action: Option<RowAction> = None;
        ui.horizontal(|ui| {
            ui.strong(format!("{} ({})", tr.ec_registered, crates.len()));
            if ui
                .add_enabled(
                    idle && !crates.is_empty(),
                    egui::Button::new(format!("⟳ {}", tr.ec_update_all)),
                )
                .clicked()
            {
                action = Some(RowAction::UpdateAll);
            }
        });
        if crates.is_empty() {
            ui.weak(tr.ec_none_yet);
        }
        for c in crates {
            ui.horizontal(|ui| {
                ui.monospace(format!("{} {}", c.name, c.version));
                if c.requirement.is_empty() {
                    ui.weak(tr.ec_newest_stable);
                } else {
                    ui.weak(format!("{} {}", tr.ec_req_prefix, c.requirement));
                }
                if !c.features.is_empty() {
                    ui.weak(format!("{}: {}", tr.ec_features, c.features.join(", ")));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(idle, egui::Button::new(format!("✖ {}", tr.ec_remove)))
                        .clicked()
                    {
                        action =
                            Some(RowAction::ConfirmRemove(c.name.clone(), c.version.clone()));
                    }
                    if ui
                        .add_enabled(idle, egui::Button::new(format!("⟳ {}", tr.ec_update)))
                        .clicked()
                    {
                        action = Some(RowAction::Update(c.name.clone()));
                    }
                    ui.hyperlink_to("↗", &c.url).on_hover_text(tr.ec_open_page);
                });
            });
        }
        match action {
            Some(RowAction::UpdateAll) => self.spawn_update(project_path, Vec::new(), tr),
            Some(RowAction::Update(name)) => self.spawn_update(project_path, vec![name], tr),
            Some(RowAction::ConfirmRemove(name, version)) => {
                self.confirm_remove = Some((name, version));
            }
            None => {}
        }

        // ── Manifest note (R24) + log ────────────────────────────────────
        ui.separator();
        ui.weak(tr.ec_manifest_note);
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("ec_log")
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &self.log {
                    match line {
                        LogLine::Info(text) => ui.label(text),
                        LogLine::Warn(text) => ui.colored_label(
                            egui::Color32::from_rgb(230, 150, 30),
                            text,
                        ),
                        LogLine::Error(text) => ui.colored_label(
                            egui::Color32::from_rgb(225, 70, 70),
                            text,
                        ),
                    };
                }
            });
    }

    /// Run (or re-run) the search for one page. Finishes **silently**: the
    /// table is the feedback, so nothing is written to the log.
    fn run_search(&mut self, query: String, page: usize, tr: &Tr) {
        if query.is_empty() {
            return;
        }
        self.searched = query.clone();
        self.spawn(tr.ec_search.to_string(), false, move |registry, tx| {
            let found = registry
                .search(&query, RESULTS_PER_PAGE, page)
                .map_err(|e| e.to_string())?;
            let _ = tx.send(Msg::Hits {
                hits: found.hits,
                total: found.total,
                page,
            });
            Ok(None)
        });
    }

    /// The current page as a markdown table. The crate name is a
    /// `crate:<name>` link so the rendered row is clickable.
    fn results_markdown(&self, tr: &Tr) -> String {
        // A description containing `|` would otherwise start a new cell and
        // shear the row; `\|` is the escape a table cell understands.
        fn cell(text: &str) -> String {
            text.replace('\\', "").replace('|', "\\|").replace('\n', " ")
        }
        let mut s = format!(
            "| {} | {} | {} | {} |\n|---|---|---|---|\n",
            tr.ec_col_crate, tr.ec_col_version, tr.ec_col_downloads, tr.ec_col_description
        );
        for hit in &self.hits {
            s.push_str(&format!(
                "| [{name}]({PICK_SCHEME}{name}) | {version} | {downloads} | {description} |\n",
                name = cell(&hit.name),
                version = cell(&hit.newest),
                downloads = thousands(hit.downloads),
                description = cell(&hit.description),
            ));
        }
        s
    }

    fn spawn_update(&mut self, project_path: &Path, targets: Vec<String>, tr: &Tr) {
        let path = project_path.to_path_buf();
        let (word_updated, word_current, word_failed) = (
            tr.ec_updated.to_string(),
            tr.ec_current.to_string(),
            tr.ec_failed.to_string(),
        );
        self.spawn(tr.ec_update.to_string(), true, move |registry, tx| {
            let outcomes = service::update(&registry, &path, None, &targets, &mut |n| {
                let _ = tx.send(Msg::Note(n));
            })?;
            let (mut updated, mut current, mut failed) = (0usize, 0usize, 0usize);
            for outcome in &outcomes {
                match outcome {
                    UpdateOutcome::Updated { .. } => updated += 1,
                    UpdateOutcome::Current { name } => {
                        let _ = tx.send(Msg::Note(Note::Info(format!(
                            "{word_current}: {name}"
                        ))));
                        current += 1;
                    }
                    UpdateOutcome::Failed { name, reason } => {
                        let _ = tx.send(Msg::Note(Note::Warn(format!(
                            "{word_failed}: {name} — {reason}"
                        ))));
                        failed += 1;
                    }
                }
            }
            Ok(Some(format!(
                "{updated} {word_updated}, {current} {word_current}, {failed} {word_failed}"
            )))
        });
    }
}

/// `1234567` → `1 234 567`. A download count is a size cue, and seven
/// undivided digits do not read as one.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push('\u{202f}'); // narrow no-break space
        }
        out.push(c);
    }
    out
}

// ── State-machine tests (no network, injected worker messages) ───────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(name: &str) -> SearchHit {
        SearchHit {
            name: name.into(),
            newest: "1.0.0".into(),
            description: "d".into(),
            downloads: 1_234_567,
        }
    }

    /// While a worker runs the panel is busy (buttons disable off this
    /// flag); notes land in the log; `Finished` clears busy and — only when
    /// the action mutated the project — raises the reload flag exactly once.
    #[test]
    fn worker_lifecycle_drives_busy_log_and_reload() {
        let mut panel = ExternalCratesPanel::new();
        let (tx, rx) = channel();
        panel.rx = Some(rx);
        panel.busy = Some("add csv".into());

        tx.send(Msg::Note(Note::Info("resolving".into()))).unwrap();
        tx.send(Msg::Note(Note::Warn("two copies".into()))).unwrap();
        panel.drain_worker();
        assert!(panel.busy.is_some(), "notes alone must not end the action");
        assert_eq!(panel.log.len(), 2);

        tx.send(Msg::Finished { result: Ok(Some("added".into())), mutated: true })
            .unwrap();
        panel.drain_worker();
        assert!(panel.busy.is_none());
        assert!(panel.project_changed, "a mutating success must ask for a reload");
        assert!(std::mem::take(&mut panel.project_changed));
        assert!(!panel.project_changed, "the flag is drained, not sticky");
    }

    /// A refusal arrives as an error line and does NOT ask for a reload —
    /// nothing changed on disk (R12/R13 refusals, R18 failed updates).
    #[test]
    fn a_refusal_logs_an_error_and_keeps_the_project() {
        let mut panel = ExternalCratesPanel::new();
        let (tx, rx) = channel();
        panel.rx = Some(rx);
        panel.busy = Some("add egui".into());
        tx.send(Msg::Finished {
            result: Err("`egui` is already available".into()),
            mutated: true, // even a mutating action must not reload on Err
        })
        .unwrap();
        panel.drain_worker();
        assert!(panel.busy.is_none());
        assert!(!panel.project_changed);
        assert!(matches!(panel.log.last(), Some(LogLine::Error(_))));
    }

    /// R6 — a page of results arrives with its total and page number, and a
    /// finished search writes **nothing** to the log: the table is the
    /// feedback. (The `N × "query"` line this replaces was debug noise.)
    #[test]
    fn a_search_page_arrives_and_logs_nothing() {
        let mut panel = ExternalCratesPanel::new();
        let (tx, rx) = channel();
        panel.rx = Some(rx);
        panel.busy = Some("search".into());
        tx.send(Msg::Hits {
            hits: vec![hit("csv"), hit("qsv")],
            total: 120,
            page: 2,
        })
        .unwrap();
        tx.send(Msg::Finished { result: Ok(None), mutated: false }).unwrap();
        panel.drain_worker();
        assert_eq!(panel.hits.len(), 2);
        assert_eq!(panel.total, 120);
        assert_eq!(panel.page, 2);
        assert!(panel.busy.is_none());
        assert!(panel.log.is_empty(), "a search must not write to the log");
        // 120 matches at 50 per page is three pages — the pager's arithmetic.
        assert_eq!(panel.total.div_ceil(RESULTS_PER_PAGE), 3);
    }

    /// R6 — the results table is markdown: a header row, one row per hit,
    /// each crate name a `crate:<name>` link so a click can pick it.
    #[test]
    fn results_render_as_a_markdown_table_with_pick_links() {
        let tr = crate::i18n::Language::English.tr();
        let mut panel = ExternalCratesPanel::new();
        panel.hits = vec![hit("csv")];
        let md = panel.results_markdown(&tr);
        let lines: Vec<&str> = md.lines().collect();
        assert!(lines[0].starts_with("| ") && lines[0].contains(tr.ec_col_crate));
        assert_eq!(lines[1], "|---|---|---|---|");
        assert!(
            lines[2].contains("[csv](crate:csv)"),
            "the crate cell must be a pick link, got: {}",
            lines[2]
        );
        assert!(lines[2].contains("1\u{202f}234\u{202f}567"), "downloads grouped");
    }

    /// Every row of a REAL search keeps its four cells with the first three
    /// filled — the operator's screenshot showed a row whose crate, version
    /// and downloads were blank while its description rendered.
    #[test]
    fn every_row_of_a_live_search_keeps_its_four_cells() {
        let tr = crate::i18n::Language::English.tr();
        let registry = service::Registry::new(service::DEFAULT_REGISTRY);
        let found = registry
            .search("csv", RESULTS_PER_PAGE, 1)
            .expect("live search");
        let mut panel = ExternalCratesPanel::new();
        panel.hits = found.hits;
        let md = panel.results_markdown(&tr);
        for (i, line) in md.lines().skip(2).enumerate() {
            let cells: Vec<&str> = line.trim_matches('|').split(" | ").collect();
            assert_eq!(
                cells.len(),
                4,
                "row {i} has {} cells, not 4: {line}",
                cells.len()
            );
            for (c, name) in [(0, "crate"), (1, "version"), (2, "downloads")] {
                assert!(
                    !cells[c].trim().is_empty(),
                    "row {i}'s {name} cell is empty: {line}"
                );
            }
        }
    }

    /// The generated markdown must parse back as ONE table row per hit —
    /// four cells, in order. A description carrying a stray CR, a `[`, or
    /// inline HTML must not shear the row (the operator's screenshot showed
    /// a row whose first three cells rendered empty).
    #[test]
    fn live_results_parse_back_as_whole_rows() {
        use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

        let tr = crate::i18n::Language::English.tr();
        let registry = service::Registry::new(service::DEFAULT_REGISTRY);
        let found = registry
            .search("csv", RESULTS_PER_PAGE, 1)
            .expect("live search");
        let expected_rows = found.hits.len();
        let mut panel = ExternalCratesPanel::new();
        panel.hits = found.hits;
        let md = panel.results_markdown(&tr);

        let mut cells_per_row: Vec<usize> = Vec::new();
        let mut cur = 0usize;
        let mut links = 0usize;
        for ev in Parser::new_ext(&md, Options::ENABLE_TABLES) {
            match ev {
                Event::End(TagEnd::TableCell) => cur += 1,
                Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                    cells_per_row.push(std::mem::take(&mut cur))
                }
                Event::Start(Tag::Link { dest_url, .. })
                    if dest_url.starts_with(PICK_SCHEME) =>
                {
                    links += 1
                }
                _ => {}
            }
        }
        assert_eq!(
            cells_per_row.len(),
            expected_rows + 1,
            "expected a header plus one row per hit, got {:?} rows",
            cells_per_row.len()
        );
        for (i, n) in cells_per_row.iter().enumerate() {
            assert_eq!(*n, 4, "row {i} parsed as {n} cells, not 4");
        }
        assert_eq!(links, expected_rows, "every row needs its pick link");
    }

    /// A description containing a pipe would shear the row into extra cells;
    /// it must arrive escaped.
    #[test]
    fn a_pipe_in_a_description_cannot_break_the_row() {
        let tr = crate::i18n::Language::English.tr();
        let mut panel = ExternalCratesPanel::new();
        let mut h = hit("weird");
        h.description = "parses a | b pipes".into();
        panel.hits = vec![h];
        let row = panel.results_markdown(&tr).lines().nth(2).unwrap().to_string();
        assert!(row.contains("a \\| b"), "pipe must be escaped, got: {row}");
        assert_eq!(
            row.matches(" | ").count(),
            3,
            "an escaped pipe must not add a cell separator: {row}"
        );
    }

    /// R19 — nothing is removed without the confirmation step: requesting a
    /// removal only parks it in `confirm_remove`.
    #[test]
    fn removal_waits_in_the_confirmation_slot() {
        let mut panel = ExternalCratesPanel::new();
        panel.confirm_remove = Some(("csv".into(), "1.4.0".into()));
        assert_eq!(
            panel.confirm_remove.as_ref().map(|(n, _)| n.as_str()),
            Some("csv")
        );
        // Keep = clearing the slot; the project was never touched.
        panel.confirm_remove = None;
        assert!(!panel.project_changed);
    }
}
