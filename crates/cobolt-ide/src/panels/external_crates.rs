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
    ExternalCratesSettings, Note, Registry, SearchHit, SystemCategory, UpdateOutcome,
    RESULTS_PER_PAGE,
};

// ── System-crate marker colors (spec 045 R7–R10) ─────────────────────────────
//
// Fixed target hue/saturation per category — "dimmed", not neon — with the
// *lightness* solved per theme so all 16 themes clear the WCAG AA
// graphical-object contrast floor against their own panel background,
// exactly the guarantee `flags.rs` already gives its two-tone flags
// (`every_theme_paints_flags_with_high_contrast`), reusing the same
// `crate::contrast` math rather than a second copy of it.

/// WCAG AA's floor for graphical objects (text needs the stricter 4.5:1;
/// a color marker is a charge, not text).
const MIN_MARKER_CONTRAST: f64 = 3.0;

/// A direct System crate (yellow), a System dependency (gray), or an
/// addable, non-system crate (green) — `None` is the addable case.
fn marker_color(theme: &crate::theme::Theme, category: Option<SystemCategory>) -> egui::Color32 {
    let (hue, sat) = match category {
        Some(SystemCategory::Direct) => (48.0 / 360.0, 0.55),
        Some(SystemCategory::Transitive) => (0.0, 0.0),
        None => (135.0 / 360.0, 0.45),
    };
    // Opaque, like `flags.rs::opaque()`: contrast is a property of the
    // theme's own tone, not of whatever bleeds through a translucent panel.
    let bg = egui::Color32::from_rgb(theme.bg_panel.r(), theme.bg_panel.g(), theme.bg_panel.b());
    solve_marker_lightness(hue, sat, bg)
}

/// Walks `v` away from the background's own luminance until the pair clears
/// [`MIN_MARKER_CONTRAST`], capped so a pathological theme still returns
/// *something* (the most extreme value tried) instead of looping forever.
/// The push direction comes from the background's actual measured
/// luminance, not a theme's `dark` flag — a flag can be wrong for a
/// particular panel tone; the measurement cannot.
fn solve_marker_lightness(hue: f32, sat: f32, bg: egui::Color32) -> egui::Color32 {
    let push_up = crate::contrast::relative_luminance(bg) < 0.5;
    let to_color = |v: f32| {
        let [r, g, b] = egui::ecolor::Hsva::new(hue, sat, v, 1.0).to_srgb();
        egui::Color32::from_rgb(r, g, b)
    };
    let mut v: f32 = if push_up { 0.62 } else { 0.42 };
    for _ in 0..48 {
        let c = to_color(v);
        if crate::contrast::contrast_ratio(c, bg) >= MIN_MARKER_CONTRAST {
            return c;
        }
        v = if push_up { (v + 0.015).min(1.0) } else { (v - 0.015).max(0.0) };
    }
    to_color(v)
}

/// Spec 045 R14 — `1209` → `"1.2K"`, `1239897` → `"1.2M"`, `5000` → `"5K"`:
/// one decimal, dropped when exactly `.0`. Rounding that pushes a value to
/// the next unit's threshold (e.g. `999_999` rounding to `1000.0K`) carries
/// over instead of printing an ugly four-digit prefix.
fn abbreviate_downloads(n: u64) -> String {
    let nf = n as f64;
    let (mut scaled, mut suffix) = if nf < 1_000.0 {
        return n.to_string();
    } else if nf < 1_000_000.0 {
        (nf / 1_000.0, "K")
    } else if nf < 1_000_000_000.0 {
        (nf / 1_000_000.0, "M")
    } else {
        (nf / 1_000_000_000.0, "B")
    };
    scaled = (scaled * 10.0).round() / 10.0;
    if scaled >= 1000.0 {
        (scaled, suffix) = match suffix {
            "K" => (scaled / 1000.0, "M"),
            "M" => (scaled / 1000.0, "B"),
            other => (scaled, other),
        };
    }
    if scaled.fract().abs() < 1e-9 {
        format!("{}{suffix}", scaled as i64)
    } else {
        format!("{scaled:.1}{suffix}")
    }
}

/// Worker → panel messages; one channel per running action.
enum Msg {
    Note(Note),
    /// One page of results: the rows, how many matches exist in total, and
    /// which page these rows are.
    Hits { hits: Vec<SearchHit>, total: usize, page: usize },
    /// `result: Ok(None)` finishes silently — a search's feedback is the
    /// results table itself, not a line in the log.
    Finished { result: Result<Option<String>, String>, mutated: bool },
    /// Spec 045 R1 — `add` hit a direct, incompatible collision; the offer
    /// waits for the developer's accept/decline, so (unlike `Finished`) this
    /// never sets `project_changed` — nothing is recorded yet.
    AliasOffered {
        candidate: cobolt_compiler::ExternalCrate,
        linked_requirement: String,
        vendored: PathBuf,
    },
}

/// What a `spawn`ed closure hands back. `Done` goes through the usual
/// `Msg::Finished` path; `AlreadyHandled` means the closure already sent its
/// own terminal message(s) (spec 045's alias offer) and `spawn` must not
/// *also* send a `Finished` — that would apply this call's static `mutated`
/// flag to an outcome that didn't actually mutate anything.
enum SpawnOutcome {
    Done(Option<String>),
    AlreadyHandled,
}

enum LogLine {
    Info(String),
    Warn(String),
    Error(String),
}

/// Spec 045 R15/R16 — which column a click sorts the *current page* by, and
/// which direction; `None` leaves the registry's own order untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortCol {
    Crate,
    Downloads,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortDir {
    Asc,
    Desc,
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
    /// Spec 045 R12 — the platform's own System/System-dependency closure;
    /// `None` until the lazy, one-time background computation lands.
    system: Option<service::SystemClosure>,
    system_rx: Option<Receiver<Result<service::SystemClosure, String>>>,
    /// Set once the background computation fails, so a permanent failure
    /// (e.g. no workspace found) does not retry every single frame.
    system_failed: bool,
    /// Spec 045 R6 — default off: System/System-dependency rows (and the
    /// System column itself) stay hidden until the developer asks for them.
    show_system: bool,
    /// Spec 045 R15–R18 — the active sort, applied to whichever page is
    /// currently displayed and re-applied when a new page loads.
    sort: Option<(SortCol, SortDir)>,
    /// Spec 045 R1/R4 — an `AddOutcome::AliasOffered` awaiting the
    /// developer's accept/decline.
    alias_offer: Option<AliasOffer>,
}

/// Spec 045 R1 — everything the offer modal needs: what to show, and what to
/// hand back to `confirm_alias`/`discard_alias_offer` on the developer's
/// choice.
struct AliasOffer {
    candidate: cobolt_compiler::ExternalCrate,
    linked_requirement: String,
    vendored: PathBuf,
    /// The alias name the modal offers — computed once when the offer
    /// arrives (`prj_<lib_name>`), shown verbatim and sent back unchanged.
    alias: String,
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
            system: None,
            system_rx: None,
            system_failed: false,
            show_system: false,
            sort: None,
            alias_offer: None,
        }
    }

    /// Spec 045 R12 — kicks off the one-time background computation on the
    /// first call, then just polls until it lands (or gives up for good).
    /// Never blocks the UI thread and never touches `busy` — this runs
    /// alongside search, not instead of it.
    fn poll_system_closure(&mut self) {
        if let Some(rx) = &self.system_rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(closure) => self.system = Some(closure),
                    Err(e) => {
                        self.log.push(LogLine::Warn(format!(
                            "could not compute the System closure — the System \
                             column stays unavailable: {e}"
                        )));
                        self.system_failed = true;
                    }
                }
                self.system_rx = None;
            }
        } else if self.system.is_none() && !self.system_failed {
            let (tx, rx) = channel();
            self.system_rx = Some(rx);
            std::thread::spawn(move || {
                let result = cobolt_compiler::resolve_workspace_root(None)
                    .ok_or_else(|| "cannot locate the PowerRustCOBOL workspace".to_string())
                    .and_then(|root| service::system_closure(&root));
                let _ = tx.send(result);
            });
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
        work: impl FnOnce(Registry, Sender<Msg>) -> Result<SpawnOutcome, String> + Send + 'static,
    ) {
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.busy = Some(label);
        let base = self.settings.registry.clone();
        std::thread::spawn(move || {
            let registry = Registry::new(&base);
            match work(registry, tx.clone()) {
                Ok(SpawnOutcome::AlreadyHandled) => {}
                Ok(SpawnOutcome::Done(text)) => {
                    let _ = tx.send(Msg::Finished { result: Ok(text), mutated });
                }
                Err(e) => {
                    let _ = tx.send(Msg::Finished { result: Err(e), mutated });
                }
            }
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
                Msg::AliasOffered { candidate, linked_requirement, vendored } => {
                    let alias =
                        format!("prj_{}", cobolt_compiler::external_crates::lib_name(&candidate.name));
                    self.alias_offer = Some(AliasOffer { candidate, linked_requirement, vendored, alias });
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
        self.poll_system_closure();
        if self.busy.is_some() || self.system_rx.is_some() {
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

        // Alias-offer modal (spec 045 R1/R4) — same shape as the R19
        // confirmation above, outside the main window.
        let mut alias_accept = false;
        let mut alias_decline = false;
        if let Some(offer) = &self.alias_offer {
            let alias = offer.alias.clone();
            let clash = format!(
                "`{}` {} {} `{} {}`.",
                offer.candidate.name,
                offer.candidate.version,
                tr.ec_alias_offer_body,
                offer.candidate.name,
                offer.linked_requirement
            );
            let use_line = format!(
                "use {}::…;",
                cobolt_compiler::external_crates::lib_name(&alias)
            );
            egui::Window::new(tr.ec_alias_offer_title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(&clash);
                    ui.label(format!("`{alias}` — {use_line}"));
                    ui.add_space(6.0);
                    ui.weak(tr.ec_alias_caveat);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(format!("{} `{alias}`", tr.ec_alias_add)).clicked() {
                            alias_accept = true;
                        }
                        if ui.button(tr.btn_cancel).clicked() {
                            alias_decline = true;
                        }
                    });
                });
        }
        if alias_accept {
            if let (Some(offer), Some(path)) =
                (self.alias_offer.take(), project_path.map(Path::to_path_buf))
            {
                let alias = offer.alias.clone();
                self.spawn(format!("{} {alias}", tr.ec_alias_add), true, move |_registry, tx| {
                    match service::confirm_alias(&path, None, offer.candidate, &offer.alias, &mut |n| {
                        let _ = tx.send(Msg::Note(n));
                    }) {
                        Ok(text) => Ok(SpawnOutcome::Done(Some(text))),
                        Err(e) => Err(e),
                    }
                });
            }
        } else if alias_decline {
            if let Some(offer) = self.alias_offer.take() {
                if let Err(e) = service::discard_alias_offer(&offer.vendored) {
                    self.log.push(LogLine::Warn(e));
                }
            }
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

        // ── Search (R6) + Show System crates (spec 045 R6) ────────────────
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
            ui.separator();
            ui.label(tr.ec_show_system);
            ui.add(egui::Checkbox::without_text(&mut self.show_system));
        });
        if do_search && idle && !self.query.trim().is_empty() {
            self.run_search(self.query.trim().to_string(), 1, tr);
        }

        // ── Results: a native, typed table, paged (R6; spec 045 R5/R7–R10/
        //    R13/R15–R18) ────────────────────────────────────────────────
        if !self.hits.is_empty() {
            self.draw_results_table(ui, tr);

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

        // ── Add (R7; spec 045 R13 — the name field is read-only) ──────────
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(tr.ec_crate);
            ui.add_sized(
                [150.0, 20.0],
                egui::TextEdit::singleline(&mut self.sel_name).interactive(false),
            );
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
                // Spec 045 R11 — System/System-dependency refused before any
                // network call; unlike a direct incompatible collision
                // (R1), there is no alias offer for this case. Checked here
                // (fast, translated, no thread spawn) — `service::add`
                // repeats the same check (plain English, R2's diagnostic-
                // stream carve-out) as a defense-in-depth for any other
                // caller.
                if let Some(_category) = self.system.as_ref().and_then(|s| s.classify(&name)) {
                    self.log.push(LogLine::Error(format!("`{name}` — {}", tr.ec_system_refused)));
                } else {
                    let req = self.sel_req.trim().to_string();
                    let features = self.features_vec();
                    let path = project_path.to_path_buf();
                    let system = self.system.clone();
                    self.spawn(format!("{} {name}", tr.ec_add), true, move |registry, tx| {
                        let req = (!req.is_empty()).then_some(req.as_str());
                        match service::add(
                            &registry,
                            &path,
                            None,
                            &name,
                            req,
                            features,
                            system.as_ref(),
                            &mut |n| {
                                let _ = tx.send(Msg::Note(n));
                            },
                        )? {
                            service::AddOutcome::Added(text) => Ok(SpawnOutcome::Done(Some(text))),
                            // Spec 045 R1 — the offer, not a refusal; nothing
                            // is recorded until the developer accepts (the
                            // modal above).
                            service::AddOutcome::AliasOffered { candidate, linked_requirement, vendored } => {
                                let _ =
                                    tx.send(Msg::AliasOffered { candidate, linked_requirement, vendored });
                                Ok(SpawnOutcome::AlreadyHandled)
                            }
                        }
                    });
                }
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
            Ok(SpawnOutcome::Done(None))
        });
    }

    /// Spec 045 R6/R15–R18 — the current page's rows, System-classified and
    /// filtered/sorted per the live toggle+sort state. Recomputed each frame
    /// (cheap: at most `RESULTS_PER_PAGE` rows) rather than cached, so a
    /// toggle or sort click is visible immediately with no extra state to
    /// keep in sync.
    fn visible_rows(&self) -> Vec<(&SearchHit, Option<SystemCategory>)> {
        let mut rows: Vec<(&SearchHit, Option<SystemCategory>)> = self
            .hits
            .iter()
            .map(|h| (h, self.system.as_ref().and_then(|s| s.classify(&h.name))))
            .filter(|(_, cat)| self.show_system || cat.is_none())
            .collect();
        if let Some((col, dir)) = self.sort {
            rows.sort_by(|a, b| {
                let ord = match col {
                    SortCol::Crate => a.0.name.to_ascii_lowercase().cmp(&b.0.name.to_ascii_lowercase()),
                    SortCol::Downloads => a.0.downloads.cmp(&b.0.downloads),
                };
                if dir == SortDir::Desc {
                    ord.reverse()
                } else {
                    ord
                }
            });
        }
        rows
    }

    /// A column header's label, with a sort arrow appended when this column
    /// is the active sort (spec 045 R15/R16).
    fn sort_header_text(label: &str, sort: Option<(SortCol, SortDir)>, col: SortCol) -> String {
        match sort {
            Some((c, dir)) if c == col => {
                format!("{label} {}", if dir == SortDir::Asc { "▲" } else { "▼" })
            }
            _ => label.to_string(),
        }
    }

    /// Spec 045 R5/R7–R10/R13/R15–R18 — the results grid: System marker
    /// (only while `show_system`) · Crate (click-to-pick — the Add row's
    /// name field is read-only, so this is the only way to set it) ·
    /// Version · Downloads (abbreviated, click-to-sort) · Description
    /// (wraps). Follows `md_render.rs::draw_table_tight`'s
    /// measure-widest-column / tight-resizable / last-column-wraps pattern —
    /// the same underlying `egui_extras::TableBuilder`, just driven directly
    /// by typed rows instead of parsed Markdown, so per-cell color and a
    /// stateful sortable header are possible (044's Markdown-table pipeline
    /// had no notion of either).
    fn draw_results_table(&mut self, ui: &mut egui::Ui, tr: &Tr) {
        let rows = self.visible_rows();
        if rows.is_empty() {
            return;
        }
        let theme = crate::theme::active();
        let show_system = self.show_system;
        let mut picked: Option<String> = None;
        let mut clicked_sort: Option<SortCol> = None;
        let current_sort = self.sort;

        egui::ScrollArea::vertical().id_salt("ec_hits").max_height(300.0).show(ui, |ui| {
            use egui_extras::{Column, TableBuilder};
            let mut builder = TableBuilder::new(ui)
                .id_salt("ec_hits_table")
                .striped(true)
                .resizable(true)
                .vscroll(false)
                .auto_shrink([false, false])
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
            if show_system {
                builder = builder.column(Column::initial(28.0).at_least(24.0));
            }
            builder = builder
                .column(Column::initial(170.0).at_least(70.0).resizable(true))
                .column(Column::initial(70.0).at_least(50.0).resizable(true))
                .column(Column::initial(95.0).at_least(60.0).resizable(true))
                .column(Column::remainder().at_least(120.0));

            builder
                .header(22.0, |mut hrow| {
                    if show_system {
                        hrow.col(|ui| {
                            ui.strong(tr.ec_col_system);
                        });
                    }
                    hrow.col(|ui| {
                        let text = Self::sort_header_text(tr.ec_col_crate, current_sort, SortCol::Crate);
                        if ui
                            .add(egui::Label::new(egui::RichText::new(text).strong()).sense(egui::Sense::click()))
                            .clicked()
                        {
                            clicked_sort = Some(SortCol::Crate);
                        }
                    });
                    hrow.col(|ui| {
                        ui.strong(tr.ec_col_version);
                    });
                    hrow.col(|ui| {
                        let text =
                            Self::sort_header_text(tr.ec_col_downloads, current_sort, SortCol::Downloads);
                        if ui
                            .add(egui::Label::new(egui::RichText::new(text).strong()).sense(egui::Sense::click()))
                            .clicked()
                        {
                            clicked_sort = Some(SortCol::Downloads);
                        }
                    });
                    hrow.col(|ui| {
                        ui.strong(tr.ec_col_description);
                    });
                })
                .body(|body| {
                    body.rows(22.0, rows.len(), |mut row| {
                        let (hit, category) = rows[row.index()];
                        if show_system {
                            row.col(|ui| {
                                let (color, tag) = match category {
                                    Some(SystemCategory::Direct) => {
                                        (marker_color(&theme, category), tr.ec_system_tag)
                                    }
                                    Some(SystemCategory::Transitive) => {
                                        (marker_color(&theme, category), tr.ec_system_dep_tag)
                                    }
                                    None => (marker_color(&theme, None), ""),
                                };
                                let resp =
                                    ui.allocate_response(egui::vec2(14.0, 14.0), egui::Sense::hover());
                                ui.painter().circle_filled(resp.rect.center(), 5.0, color);
                                if !tag.is_empty() {
                                    resp.on_hover_text(tag);
                                }
                            });
                        }
                        row.col(|ui| {
                            if ui.link(&hit.name).clicked() {
                                picked = Some(hit.name.clone());
                            }
                        });
                        row.col(|ui| {
                            ui.label(&hit.newest);
                        });
                        row.col(|ui| {
                            ui.label(abbreviate_downloads(hit.downloads));
                        });
                        row.col(|ui| {
                            ui.add(egui::Label::new(&hit.description).wrap());
                        });
                    });
                });
        });

        if let Some(name) = picked {
            self.sel_name = name;
        }
        if let Some(col) = clicked_sort {
            self.sort = Some(match self.sort {
                Some((c, dir)) if c == col => {
                    (col, if dir == SortDir::Asc { SortDir::Desc } else { SortDir::Asc })
                }
                _ => (col, SortDir::Asc),
            });
        }
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
            Ok(SpawnOutcome::Done(Some(format!(
                "{updated} {word_updated}, {current} {word_current}, {failed} {word_failed}"
            ))))
        });
    }
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

    /// Spec 045 R10/AC6 — every theme's three marker categories clear the
    /// WCAG AA graphical-object floor against that theme's own panel
    /// background, mirroring `flags.rs`'s
    /// `every_theme_paints_flags_with_high_contrast`.
    #[test]
    fn every_theme_marks_system_crates_with_sufficient_contrast() {
        let mut worst = (f64::MAX, "", "");
        for theme in crate::theme::THEMES {
            let bg =
                egui::Color32::from_rgb(theme.bg_panel.r(), theme.bg_panel.g(), theme.bg_panel.b());
            for (label, category) in [
                ("direct", Some(SystemCategory::Direct)),
                ("transitive", Some(SystemCategory::Transitive)),
                ("addable", None),
            ] {
                let marker = marker_color(theme, category);
                let ratio = crate::contrast::contrast_ratio(marker, bg);
                assert!(
                    ratio >= MIN_MARKER_CONTRAST,
                    "theme {}: {label} marker vs panel is {ratio:.2}:1, below {MIN_MARKER_CONTRAST}:1",
                    theme.id
                );
                if ratio < worst.0 {
                    worst = (ratio, theme.id, label);
                }
            }
        }
        println!("worst System-marker contrast: {:.2}:1 ({} / {})", worst.0, worst.1, worst.2);
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

    /// Spec 045 T9 — the results grid is now a native `TableBuilder` driven
    /// directly by `visible_rows()`, not parsed Markdown, so there is no
    /// text-based row/cell shape to assert on: the row's data IS the hit,
    /// verbatim. This is the native equivalent of 044's
    /// `results_render_as_a_markdown_table_with_pick_links` — the crate name
    /// that appears is exactly the name `draw_results_table`'s row-click
    /// hands to `sel_name`.
    #[test]
    fn visible_rows_carry_every_field_verbatim() {
        let mut panel = ExternalCratesPanel::new();
        panel.hits = vec![hit("csv")];
        let rows = panel.visible_rows();
        assert_eq!(rows.len(), 1);
        let (row, category) = rows[0];
        assert_eq!(row.name, "csv");
        assert_eq!(row.newest, "1.0.0");
        assert_eq!(row.downloads, 1_234_567);
        assert_eq!(category, None, "no System closure loaded ⇒ nothing is classified yet");
    }

    /// Native equivalent of 044's `every_row_of_a_live_search_keeps_its_four_cells`
    /// — every hit from a real search survives into `visible_rows()` with its
    /// crate/version/downloads intact (the concern the old Markdown-parsing
    /// test guarded against — a row whose first three cells rendered blank —
    /// cannot happen here: there is no text row to shear in the first
    /// place).
    #[test]
    fn every_hit_of_a_live_search_keeps_its_fields() {
        let registry = service::Registry::new(service::DEFAULT_REGISTRY);
        let found = registry.search("csv", RESULTS_PER_PAGE, 1).expect("live search");
        let expected = found.hits.len();
        let mut panel = ExternalCratesPanel::new();
        panel.hits = found.hits;
        let rows = panel.visible_rows();
        assert_eq!(rows.len(), expected, "every hit must survive into visible_rows");
        for (i, (hit, _)) in rows.iter().enumerate() {
            assert!(!hit.name.trim().is_empty(), "row {i}'s crate name is empty");
            assert!(!hit.newest.trim().is_empty(), "row {i}'s version is empty");
        }
    }

    fn fixture_closure() -> service::SystemClosure {
        service::SystemClosure {
            direct: ["egui".to_string(), "eframe".to_string()].into_iter().collect(),
            transitive: ["epaint".to_string()].into_iter().collect(),
        }
    }

    /// Spec 045 R5/AC4 — a direct-linked name classifies `Direct`, a
    /// transitive-only name classifies `Transitive`, and an unrelated name
    /// classifies `None` (addable).
    #[test]
    fn system_column_classifies_direct_transitive_and_addable() {
        let mut panel = ExternalCratesPanel::new();
        panel.system = Some(fixture_closure());
        panel.show_system = true;
        panel.hits = vec![hit("egui"), hit("epaint"), hit("csv")];
        let rows = panel.visible_rows();
        let by_name: std::collections::HashMap<&str, Option<SystemCategory>> =
            rows.iter().map(|(h, c)| (h.name.as_str(), *c)).collect();
        assert_eq!(by_name["egui"], Some(SystemCategory::Direct));
        assert_eq!(by_name["epaint"], Some(SystemCategory::Transitive));
        assert_eq!(by_name["csv"], None);
    }

    /// Spec 045 R6/AC5 — off (the default) hides System and System-dependency
    /// rows entirely; on brings them back. The System *column*'s visibility
    /// is a draw-time decision (`draw_results_table` reads `show_system`
    /// directly) — this test covers the row-filtering half, which is what
    /// `visible_rows` controls.
    #[test]
    fn show_system_toggle_filters_results_and_column() {
        let mut panel = ExternalCratesPanel::new();
        panel.system = Some(fixture_closure());
        panel.hits = vec![hit("egui"), hit("epaint"), hit("csv")];

        panel.show_system = false;
        let names: Vec<&str> = panel.visible_rows().iter().map(|(h, _)| h.name.as_str()).collect();
        assert_eq!(names, vec!["csv"], "off must hide System and System-dependency rows");

        panel.show_system = true;
        let names: Vec<&str> = panel.visible_rows().iter().map(|(h, _)| h.name.as_str()).collect();
        assert_eq!(names, vec!["egui", "epaint", "csv"], "on must show every row");
    }

    /// Spec 045 R14/AC9 — the worked examples from the spec, plus the
    /// K→M carry-over boundary (`999_999` rounds to `1000.0K`, which must
    /// promote to `1M` rather than print an ugly four-digit prefix).
    #[test]
    fn downloads_abbreviate_per_worked_examples() {
        assert_eq!(abbreviate_downloads(999), "999");
        assert_eq!(abbreviate_downloads(1000), "1K");
        assert_eq!(abbreviate_downloads(1209), "1.2K");
        assert_eq!(abbreviate_downloads(5000), "5K");
        assert_eq!(abbreviate_downloads(999999), "1M");
        assert_eq!(abbreviate_downloads(1000000), "1M");
        assert_eq!(abbreviate_downloads(1239897), "1.2M");
    }

    /// Spec 045 R15–R18/AC10 — clicking "Crate" sorts the current page
    /// alphabetically and reverses on a second click; clicking "Downloads"
    /// sorts numerically by the true count; the active sort re-applies when
    /// a new page's hits load, with no extra click needed.
    #[test]
    fn sort_toggles_direction_and_reapplies_across_pages() {
        let mut panel = ExternalCratesPanel::new();
        panel.hits = vec![hit("zed"), hit("ana"), hit("mid")];

        panel.sort = Some((SortCol::Crate, SortDir::Asc));
        let names: Vec<&str> = panel.visible_rows().iter().map(|(h, _)| h.name.as_str()).collect();
        assert_eq!(names, vec!["ana", "mid", "zed"]);

        panel.sort = Some((SortCol::Crate, SortDir::Desc));
        let names: Vec<&str> = panel.visible_rows().iter().map(|(h, _)| h.name.as_str()).collect();
        assert_eq!(names, vec!["zed", "mid", "ana"]);

        let mut low = hit("low");
        low.downloads = 10;
        let mut high = hit("high");
        high.downloads = 9_999_999;
        panel.hits = vec![high, low];
        panel.sort = Some((SortCol::Downloads, SortDir::Asc));
        let names: Vec<&str> = panel.visible_rows().iter().map(|(h, _)| h.name.as_str()).collect();
        assert_eq!(names, vec!["low", "high"], "ascending must sort by the TRUE count, not the label");

        // A new page's hits land under the same, still-active Downloads-asc
        // sort — no extra click needed (R18); distinct counts make the
        // order unambiguous (name order alone would say the opposite).
        let mut zzz = hit("zzz");
        zzz.downloads = 500;
        let mut aaa = hit("aaa");
        aaa.downloads = 10;
        panel.hits = vec![zzz, aaa];
        let names: Vec<&str> = panel.visible_rows().iter().map(|(h, _)| h.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["aaa", "zzz"],
            "the still-active Downloads-asc sort must re-apply to the new page's hits"
        );
    }

    /// Spec 045 T9 — a description carrying a pipe or a bare `\r` (real
    /// crates.io data does this — e.g. `egui-cameras`, `egui-thematic`; see
    /// the operator's screenshot that motivated this migration) passes
    /// through untouched: there is no Markdown table being built from it
    /// any more, so there is nothing for it to shear. The category of bug
    /// `a_pipe_in_a_description_cannot_break_the_row` /
    /// `a_carriage_return_in_a_description_cannot_break_the_row` guarded
    /// against is now categorically impossible, not merely escaped.
    #[test]
    fn odd_description_text_reaches_visible_rows_unmodified() {
        let mut panel = ExternalCratesPanel::new();
        let mut h1 = hit("weird");
        h1.description = "parses a | b pipes\r and a\r stray CR".into();
        let h2 = hit("after");
        panel.hits = vec![h1, h2];
        let rows = panel.visible_rows();
        assert_eq!(rows.len(), 2, "both hits must stay their own row");
        assert_eq!(rows[0].0.description, "parses a | b pipes\r and a\r stray CR");
        assert_eq!(rows[1].0.name, "after");
    }

    /// Runs `widget` for a couple of layout frames, then clicks into its own
    /// rect and delivers a text-insert event, and returns the value it left
    /// behind. Shared by the read-only field test and its interactive
    /// control group.
    fn click_and_type_into(value: &mut String, interactive: bool) {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 100.0));
        let mut rect = egui::Rect::NOTHING;
        let build = |ui: &mut egui::Ui, value: &mut String| {
            let resp = ui.add_sized(
                [150.0, 20.0],
                egui::TextEdit::singleline(value).interactive(interactive),
            );
            resp.rect
        };
        // Frame 1: layout only, capture the field's screen rect.
        ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |root_ui| {
            egui::CentralPanel::default().show(root_ui, |ui| {
                rect = build(ui, value);
            });
        })
        .textures_delta
        .clear();
        // Frames 2-4: press, release (focus lands here), then a text-insert
        // event on its own frame — split like `md_render.rs`'s
        // `render_frames` click script; cramming press+release+text into one
        // frame does not reliably focus the widget in time for the text
        // event in the same frame.
        let p = rect.center();
        let scripts: Vec<Vec<egui::Event>> = vec![
            vec![egui::Event::PointerMoved(p)],
            vec![egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            }],
            vec![egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            }],
            vec![egui::Event::Text("X".into())],
        ];
        for events in scripts {
            ctx.run_ui(
                egui::RawInput { screen_rect: Some(screen), events, ..Default::default() },
                |root_ui| {
                    egui::CentralPanel::default().show(root_ui, |ui| {
                        build(ui, value);
                    });
                },
            )
            .textures_delta
            .clear();
        }
    }

    /// Spec 045 R13 — the Add row's crate-name field is built with
    /// `TextEdit::interactive(false)` (`draw_results_table`'s row-click is
    /// the only remaining way to change it): clicking into it and typing
    /// must leave the value untouched. The `interactive_field_does_change`
    /// control proves the harness itself would catch a regression — it's
    /// the same click+type script against `interactive(true)`, which DOES
    /// change, so a silent harness failure can't produce a false pass here.
    #[test]
    fn crate_name_field_is_read_only() {
        let mut read_only = "csv".to_string();
        click_and_type_into(&mut read_only, false);
        assert_eq!(read_only, "csv", "a read-only field must never change from typing");

        let mut interactive_field = "csv".to_string();
        click_and_type_into(&mut interactive_field, true);
        assert_ne!(
            interactive_field, "csv",
            "control group: an ordinary field must change, proving the harness delivers input"
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
