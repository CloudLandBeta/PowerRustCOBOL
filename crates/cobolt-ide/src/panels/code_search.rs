// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Project-wide code search (spec 053, layer 3b).
//!
//! Enumerates every [`cobolt_forms::code_site`] of every form in the project —
//! live in-memory text for forms open in the RAD (R22), `.cfrm` files on disk
//! for the rest — plus every Common Code file, and lists the occurrences of a
//! plain-text query grouped by form then site (R17–R20). Double-clicking a
//! result navigates through the same `goto_code_location` a diagnostic uses
//! (R21). Generated `.cbl` files and the recycle bin are never scanned (R24),
//! and no path here writes to developer code (R23).
//!
//! The window is the IDE's third instance of the proven resizable-box shape
//! (`app::resizable_tool_box`): `Resize` seeded once, bounded, re-allocating
//! its own box, with the interior partitioned by embedded panels and **no
//! estimated heights anywhere**. That shape is why the window can hold its
//! size (R34) with a result list that overflows — egui ≥0.35's `Resize`
//! ratchets to measured content min every frame, and a fresh layout here is
//! how this codebase has twice shipped self-inflating windows.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};

use cobolt_forms::code_site::{code_sites, CodeSite};
use cobolt_forms::Form;

use crate::i18n::Tr;

// ── Matching ─────────────────────────────────────────────────────────────────

/// Is the byte at the edge of `range` a COBOL word boundary? COBOL words are
/// letters, digits and hyphens — so whole-word `BAL` must not match inside
/// `CUST-BAL` (spec 053 AC9).
fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

/// Byte ranges of every occurrence of `query` in `line`.
///
/// Case-insensitivity is ASCII (COBOL identifiers and keywords); `whole_word`
/// requires non-word bytes (or the line edge) on both sides.
pub fn find_matches(
    line: &str,
    query: &str,
    case_sensitive: bool,
    whole_word: bool,
) -> Vec<(usize, usize)> {
    let qlen = query.len();
    let mut out = Vec::new();
    if qlen == 0 || line.len() < qlen {
        return out;
    }
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i + qlen <= line.len() {
        if !line.is_char_boundary(i) || !line.is_char_boundary(i + qlen) {
            i += 1;
            continue;
        }
        let cand = &line[i..i + qlen];
        let hit = if case_sensitive {
            cand == query
        } else {
            cand.eq_ignore_ascii_case(query)
        };
        if hit && whole_word {
            let left_ok = i == 0 || !is_word_byte(bytes[i - 1]);
            let right_ok = i + qlen == line.len() || !is_word_byte(bytes[i + qlen]);
            if !(left_ok && right_ok) {
                i += 1;
                continue;
            }
        }
        if hit {
            out.push((i, i + qlen));
            i += qlen;
        } else {
            i += 1;
        }
    }
    out
}

// ── Results ──────────────────────────────────────────────────────────────────

/// One occurrence of the query.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    /// Owning form name — empty for a Common Code file.
    pub form_name: String,
    /// The owning `.cfrm` — `None` for a Common Code file.
    pub form_path: Option<PathBuf>,
    pub site: CodeSite,
    /// 1-based line within the site's own text.
    pub line: u32,
    /// Byte range of the matched span within `line_text`.
    pub span: (usize, usize),
    pub line_text: String,
}

impl SearchHit {
    pub fn display_path(&self) -> String {
        self.site.display_path(&self.form_name)
    }
}

/// What a finished scan measured (R27 — numbers the run produced).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanStats {
    pub forms_scanned: usize,
    pub sites_scanned: usize,
    pub occurrences: usize,
    pub elapsed_ms: u128,
}

enum ScanMsg {
    Hit(SearchHit),
    Done(ScanStats),
}

// ── The scan ─────────────────────────────────────────────────────────────────

/// Everything the worker needs, snapshotted on the UI thread — open forms are
/// cloned (their live, possibly unsaved text — R22), the rest go as paths.
pub struct ScanInputs {
    pub live_forms: Vec<(PathBuf, Form)>,
    pub disk_forms: Vec<PathBuf>,
    /// Common Code files: absolute path + project-relative path.
    pub common_files: Vec<(PathBuf, String)>,
    /// Fingerprint of the live forms at snapshot time — a mismatch later
    /// marks the result set stale.
    pub fingerprint: u64,
}

/// Scan one form's code sites into `out`. Returns the number of sites
/// scanned. The recycle bin is not part of `code_sites` (R24).
pub fn scan_form(
    form: &Form,
    form_path: Option<&Path>,
    query: &str,
    case_sensitive: bool,
    whole_word: bool,
    out: &mut Vec<SearchHit>,
) -> usize {
    let sites = code_sites(form);
    let n = sites.len();
    for (site, text) in sites {
        for (idx, line_text) in text.lines().enumerate() {
            for span in find_matches(line_text, query, case_sensitive, whole_word) {
                out.push(SearchHit {
                    form_name: form.name.clone(),
                    form_path: form_path.map(|p| p.to_path_buf()),
                    site: site.clone(),
                    line: idx as u32 + 1,
                    span,
                    line_text: line_text.to_string(),
                });
            }
        }
    }
    n
}

/// Scan one Common Code file's text into `out`.
pub fn scan_common_text(
    rel_path: &str,
    text: &str,
    query: &str,
    case_sensitive: bool,
    whole_word: bool,
    out: &mut Vec<SearchHit>,
) {
    let site = CodeSite::CommonCode {
        rel_path: rel_path.to_string(),
    };
    for (idx, line_text) in text.lines().enumerate() {
        for span in find_matches(line_text, query, case_sensitive, whole_word) {
            out.push(SearchHit {
                form_name: String::new(),
                form_path: None,
                site: site.clone(),
                line: idx as u32 + 1,
                span,
                line_text: line_text.to_string(),
            });
        }
    }
}

/// The whole scan, synchronously — the worker thread's body, also called
/// directly by tests. Streams hits over `tx` and finishes with `Done(stats)`.
fn run_scan(
    inputs: ScanInputs,
    query: String,
    case_sensitive: bool,
    whole_word: bool,
    tx: Sender<ScanMsg>,
) {
    let start = std::time::Instant::now();
    let mut stats = ScanStats::default();
    let mut hits: Vec<SearchHit> = Vec::new();

    for (path, form) in &inputs.live_forms {
        stats.sites_scanned += scan_form(
            form,
            Some(path),
            &query,
            case_sensitive,
            whole_word,
            &mut hits,
        );
        stats.forms_scanned += 1;
    }
    for path in &inputs.disk_forms {
        if let Ok(form) = cobolt_forms::load_form(path) {
            stats.sites_scanned += scan_form(
                &form,
                Some(path),
                &query,
                case_sensitive,
                whole_word,
                &mut hits,
            );
            stats.forms_scanned += 1;
        }
    }
    for (abs, rel) in &inputs.common_files {
        if let Ok(text) = std::fs::read_to_string(abs) {
            scan_common_text(rel, &text, &query, case_sensitive, whole_word, &mut hits);
            stats.sites_scanned += 1;
        }
    }

    stats.occurrences = hits.len();
    stats.elapsed_ms = start.elapsed().as_millis();
    for hit in hits {
        if tx.send(ScanMsg::Hit(hit)).is_err() {
            return; // window closed mid-scan; nothing to deliver to
        }
    }
    let _ = tx.send(ScanMsg::Done(stats));
}

// ── The panel ────────────────────────────────────────────────────────────────

/// What the window asks the app to do this frame.
#[derive(Debug, Clone, PartialEq)]
pub enum CodeSearchAction {
    None,
    /// Snapshot the project and call [`CodeSearchPanel::start_scan`].
    StartScan,
    /// Navigate to a double-clicked result (R21).
    Jump(SearchHit),
    /// The Cancel button — resolved inside [`CodeSearchPanel::show`] (it
    /// clears the open flag there), so the caller only ever sees the other
    /// three.
    Cancel,
}

/// Seed size of the search window (R33). A seed only: after the first frame
/// the size lives in egui's `Resize` state and changes exclusively through
/// the user's grip drag (R34).
pub const SEARCH_WINDOW_SEED: [f32; 2] = [860.0, 520.0];

pub struct CodeSearchPanel {
    pub query: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    hits: Vec<SearchHit>,
    stats: Option<ScanStats>,
    scanning: bool,
    rx: Option<Receiver<ScanMsg>>,
    /// Fingerprint of the live forms when the current results were produced.
    scan_fingerprint: u64,
    /// Distinguishes "no query run yet" from a genuine zero-match result.
    searched_once: bool,
}

impl Default for CodeSearchPanel {
    fn default() -> Self {
        Self {
            query: String::new(),
            case_sensitive: false,
            whole_word: false,
            hits: Vec::new(),
            stats: None,
            scanning: false,
            rx: None,
            scan_fingerprint: 0,
            searched_once: false,
        }
    }
}

impl CodeSearchPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Results in their stable presentation order (grouped by form, then
    /// site, then line — R20).
    pub fn hits(&self) -> &[SearchHit] {
        &self.hits
    }

    pub fn stats(&self) -> Option<ScanStats> {
        self.stats
    }

    pub fn is_scanning(&self) -> bool {
        self.scanning
    }

    /// Number of distinct sites among the current hits (R20).
    pub fn distinct_sites(&self) -> usize {
        let mut keys: Vec<String> = self.hits.iter().map(SearchHit::display_path).collect();
        keys.sort_unstable();
        keys.dedup();
        keys.len()
    }

    /// Launch the scan on a worker thread (R26 — never on the paint path).
    pub fn start_scan(&mut self, inputs: ScanInputs) {
        let (tx, rx) = channel();
        self.hits.clear();
        self.stats = None;
        self.scanning = true;
        self.searched_once = true;
        self.scan_fingerprint = inputs.fingerprint;
        self.rx = Some(rx);
        let query = self.query.clone();
        let (case, whole) = (self.case_sensitive, self.whole_word);
        std::thread::spawn(move || run_scan(inputs, query, case, whole, tx));
    }

    /// Drain whatever the worker has produced so far. Called every frame the
    /// window is open; the UI never blocks on the scan (R26).
    fn pump(&mut self) {
        let Some(rx) = &self.rx else { return };
        let mut done = false;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                ScanMsg::Hit(hit) => self.hits.push(hit),
                ScanMsg::Done(stats) => {
                    self.stats = Some(stats);
                    done = true;
                }
            }
        }
        if done {
            self.rx = None;
            self.scanning = false;
            // Stable presentation order (R20): form, then site, then line.
            self.hits.sort_by(|a, b| {
                (&a.form_name, a.display_path(), a.line, a.span.0).cmp(&(
                    &b.form_name,
                    b.display_path(),
                    b.line,
                    b.span.0,
                ))
            });
        }
    }

    /// Render the window. `open` is the plain bool on the app (R35): only the
    /// window's `✕` (via egui's `open` handling) and the Cancel button clear
    /// it — never a jump, a click elsewhere, or a rebuild. The window never
    /// touches egui's popup manager, and it does not block input to the rest
    /// of the IDE (R36).
    ///
    /// `live_fingerprint` is the current live-forms fingerprint; when it
    /// differs from the one the results were produced under, the results are
    /// marked stale rather than silently trusted.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        open: &mut bool,
        tr: &Tr,
        live_fingerprint: u64,
    ) -> CodeSearchAction {
        if !*open {
            return CodeSearchAction::None;
        }
        self.pump();
        if self.scanning {
            ctx.request_repaint(); // keep draining while the worker runs
        }
        let mut action = CodeSearchAction::None;
        let stale = self.searched_once && self.scan_fingerprint != live_fingerprint;

        let mut still_open = *open;
        egui::Window::new(tr.search_title)
            .id(egui::Id::new("code_search_window"))
            .collapsible(false)
            .resizable(false) // the inner Resize box is the single size authority
            .open(&mut still_open)
            .show(ctx, |ui| {
                crate::app::resizable_tool_box(
                    ui,
                    "code_search_resize",
                    egui::Vec2::from(SEARCH_WINDOW_SEED),
                    |ui| {
                        action = self.body_ui(ui, tr, stale);
                    },
                );
            });
        if !still_open {
            *open = false; // ✕ — the one closer besides Cancel (R35)
        }
        if action == CodeSearchAction::Cancel {
            *open = false;
            action = CodeSearchAction::None;
        }
        action
    }

    /// The interior: query panel on top, button row at the bottom, the result
    /// list scrolling INSIDE the central panel — embedded panels partition the
    /// box exactly, no estimated heights anywhere (R34).
    fn body_ui(&mut self, ui: &mut egui::Ui, tr: &Tr, stale: bool) -> CodeSearchAction {
        let mut action = CodeSearchAction::None;

        egui::Panel::top(ui.id().with("code_search_query"))
            .resizable(false)
            .show_separator_line(true)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let field = egui::TextEdit::singleline(&mut self.query)
                        .hint_text(tr.search_placeholder)
                        .desired_width(ui.available_width() - 110.0);
                    let resp = ui.add(field);
                    let submitted =
                        resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (ui.button(tr.search_button).clicked() || submitted)
                        && !self.query.trim().is_empty()
                    {
                        action = CodeSearchAction::StartScan;
                    }
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.case_sensitive, tr.search_case);
                    ui.checkbox(&mut self.whole_word, tr.search_whole_word);
                    if self.scanning {
                        ui.spinner();
                        ui.label(tr.search_scanning);
                    } else if stale {
                        ui.colored_label(
                            egui::Color32::from_rgb(240, 170, 90),
                            tr.search_stale,
                        );
                    }
                });
                ui.add_space(4.0);
            });

        egui::Panel::bottom(ui.id().with("code_search_footer"))
            .resizable(false)
            .show_separator_line(true)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(tr.btn_cancel).clicked() {
                        action = CodeSearchAction::Cancel;
                    }
                    ui.separator();
                    if self.stats.is_some() {
                        let totals = tr
                            .search_totals
                            .replacen("{}", &self.hits.len().to_string(), 1)
                            .replacen("{}", &self.distinct_sites().to_string(), 1);
                        ui.label(totals);
                    }
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                // The list scrolls INSIDE the panel; the window never sizes
                // itself to the results (R34).
                egui::ScrollArea::both()
                    .id_salt("code_search_results")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if let Some(act) = self.results_ui(ui, tr) {
                            action = act;
                        }
                    });
            });

        action
    }

    /// R25: a zero-match query shows an explicit no-matches state, never a
    /// silent empty list.
    pub fn empty_state_visible(&self) -> bool {
        self.searched_once && !self.scanning && self.hits.is_empty()
    }

    /// The grouped result rows (R19/R20/R25). Returns a Jump action when a
    /// row is double-clicked.
    fn results_ui(&self, ui: &mut egui::Ui, tr: &Tr) -> Option<CodeSearchAction> {
        let mut action = None;
        if self.empty_state_visible() {
            ui.add_space(12.0);
            ui.label(egui::RichText::new(tr.search_no_matches).italics());
            return None;
        }
        let mut last_form: Option<&str> = None;
        let mut last_site: Option<String> = None;
        for hit in &self.hits {
            let form_label: &str = if hit.form_name.is_empty() {
                tr.search_common_code
            } else {
                &hit.form_name
            };
            if last_form != Some(form_label) {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(form_label).strong());
                last_form = Some(form_label);
                last_site = None;
            }
            let site_path = hit.display_path();
            if last_site.as_deref() != Some(site_path.as_str()) {
                ui.label(egui::RichText::new(&site_path).small().strong());
                last_site = Some(site_path);
            }
            // One row: line number + the matching line, matched span
            // highlighted (R19).
            let (pre, rest) = hit.line_text.split_at(hit.span.0.min(hit.line_text.len()));
            let span_len = (hit.span.1 - hit.span.0).min(rest.len());
            let (matched, post) = rest.split_at(span_len);
            let mut job = egui::text::LayoutJob::default();
            let font = egui::FontId::monospace(13.0);
            let dim = ui.visuals().text_color();
            job.append(
                &format!("{:>5} │ ", hit.line),
                0.0,
                egui::TextFormat::simple(font.clone(), egui::Color32::from_gray(130)),
            );
            job.append(pre, 0.0, egui::TextFormat::simple(font.clone(), dim));
            job.append(
                matched,
                0.0,
                egui::TextFormat {
                    font_id: font.clone(),
                    color: egui::Color32::from_rgb(255, 210, 90),
                    background: egui::Color32::from_rgba_unmultiplied(255, 210, 90, 40),
                    ..Default::default()
                },
            );
            job.append(post, 0.0, egui::TextFormat::simple(font, dim));
            // A result row is a link, and reads as one: the pointing hand on
            // hover (the diagnostic rows already do this — R16's convention).
            let resp = ui
                .add(egui::Label::new(job).sense(egui::Sense::click()))
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.double_clicked() {
                action = Some(CodeSearchAction::Jump(hit.clone()));
            }
        }
        action
    }
}

// ── Tests (spec 053 T18–T25) ─────────────────────────────────────────────────

#[cfg(test)]
mod code_search_tests {
    use super::*;
    use cobolt_forms::code_site::{all_sites_fixture, fixture_markers, StructureSection};

    // ── Matching (AC9) ───────────────────────────────────────────────────────

    /// Case and whole-word options change the result set exactly as
    /// specified, with a query that distinguishes them: `bal` / `BAL` /
    /// `CUST-BAL` (COBOL words include hyphens, so whole-word `BAL` must NOT
    /// match inside `CUST-BAL`).
    #[test]
    fn options_distinguish_bal_variants() {
        let line = "           MOVE CUST-BAL TO BAL OF SUMMARY, bal-total.";

        let loose = find_matches(line, "bal", false, false);
        assert_eq!(loose.len(), 3, "insensitive substring: CUST-BAL, BAL, bal-total");

        let case = find_matches(line, "BAL", true, false);
        assert_eq!(case.len(), 2, "case-sensitive: CUST-BAL and BAL only");

        let whole = find_matches(line, "BAL", false, true);
        assert_eq!(whole.len(), 1, "whole-word: only the standalone BAL");
        let (a, b) = whole[0];
        assert_eq!(&line[a..b], "BAL");

        let whole_qualified = find_matches(line, "CUST-BAL", false, true);
        assert_eq!(whole_qualified.len(), 1, "a hyphenated word matches whole");

        println!(
            "bal-matrix: loose={} case={} whole={} qualified={}",
            loose.len(),
            case.len(),
            whole.len(),
            whole_qualified.len()
        );
    }

    // ── Scan correctness (AC8, AC10, AC11) ───────────────────────────────────

    /// AC8: a query present in all NINE site kinds — the fixture's eight
    /// in-form sites plus a Common Code file — is found in every one, each
    /// with the right site path and the right line number.
    #[test]
    fn a_query_in_all_nine_site_kinds_finds_all_nine() {
        let form = all_sites_fixture();
        println!("── nine-site result table ───────────────────────────────");
        // Each site's unique marker is found at exactly its site and line.
        for (site, marker, line) in fixture_markers() {
            let mut hits: Vec<SearchHit> = Vec::new();
            let sites = scan_form(
                &form,
                Some(Path::new("/proj/forms/all-sites.cfrm")),
                marker,
                true,
                false,
                &mut hits,
            );
            assert_eq!(sites, 8, "eight in-form sites scanned");
            assert_eq!(hits.len(), 1, "{marker} occurs exactly once");
            assert_eq!(hits[0].site, site, "{marker} owned by the wrong site");
            assert_eq!(hits[0].line, line, "{marker} at the wrong line");
            println!(
                "  {:<46} line {:>2}  {}",
                hits[0].display_path(),
                hits[0].line,
                hits[0].line_text.trim()
            );
        }
        // The ninth kind: a Common Code file.
        let mut hits: Vec<SearchHit> = Vec::new();
        scan_common_text(
            "common/billing.cbl",
            "       01  WS-X PIC X.\n           DISPLAY \"MARK-COMMON-053\".",
            "MARK-COMMON-053",
            true,
            false,
            &mut hits,
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 2);
        assert!(matches!(
            &hits[0].site,
            CodeSite::CommonCode { rel_path } if rel_path == "common/billing.cbl"
        ));
        println!(
            "  {:<46} line {:>2}  {}",
            hits[0].display_path(),
            hits[0].line,
            hits[0].line_text.trim()
        );
    }

    /// AC10: the scan reads the form MODEL — a string typed into a live,
    /// unsaved form is found, because live forms are snapshotted from memory,
    /// never re-read from disk.
    #[test]
    fn unsaved_live_text_is_found() {
        let mut form = all_sites_fixture();
        // "Type" into the open form without saving anything anywhere.
        if let Some(ev) = form.controls[0].events.first_mut() {
            ev.code.push_str("\n           DISPLAY \"ONLY-IN-MEMORY-053\".");
        }
        let mut hits = Vec::new();
        scan_form(&form, None, "ONLY-IN-MEMORY-053", true, false, &mut hits);
        assert_eq!(hits.len(), 1, "the unsaved line is found");
        assert_eq!(
            hits[0].site,
            CodeSite::ControlEvent {
                control_id: "BTN-GO".into(),
                event: "onClick".into()
            }
        );
    }

    /// AC11: a string that exists ONLY in a generated `.cbl` or in recycled
    /// deleted code returns nothing — the scan never reads a generated
    /// artifact, and `code_sites` never yields the recycle bin.
    #[test]
    fn generated_and_recycled_text_is_never_found() {
        let mut form = all_sites_fixture();
        form.deleted_code
            .push(cobolt_forms::model::DeletedControlCode {
                control_id: "BTN-GONE".into(),
                deleted_at: String::new(),
                events: vec![cobolt_forms::model::EventBinding {
                    event: "onClick".into(),
                    paragraph: "BTN-GONE--ONCLICK".into(),
                    code: "           DISPLAY \"ONLY-IN-RECYCLE-BIN-053\".".into(),
                }],
            });
        // A string that exists only in the generated artifact:
        let generated = cobolt_codegen::generate(&form);
        assert!(
            generated.contains("END PROGRAM"),
            "generated artifact contains its own scaffolding"
        );
        let mut hits = Vec::new();
        scan_form(&form, None, "ONLY-IN-RECYCLE-BIN-053", true, false, &mut hits);
        assert!(hits.is_empty(), "recycled code is never searched (R24)");
        let mut hits = Vec::new();
        scan_form(&form, None, "END PROGRAM", true, false, &mut hits);
        assert!(
            hits.is_empty(),
            "generated scaffolding is never searched (R24)"
        );
    }

    /// R20: results are grouped stably — form, then site, then line — and the
    /// distinct-site count matches.
    #[test]
    fn results_sort_stably_and_count_distinct_sites() {
        let mut panel = CodeSearchPanel::new();
        let form = all_sites_fixture();
        let mut hits = Vec::new();
        scan_form(&form, None, "MARK-", true, false, &mut hits);
        // Deliver in reverse to prove the sort restores the order.
        hits.reverse();
        let (tx, rx) = channel();
        for h in hits {
            tx.send(ScanMsg::Hit(h)).unwrap();
        }
        tx.send(ScanMsg::Done(ScanStats::default())).unwrap();
        panel.rx = Some(rx);
        panel.scanning = true;
        panel.searched_once = true;
        panel.pump();
        assert!(!panel.is_scanning());
        let paths: Vec<String> = panel.hits().iter().map(SearchHit::display_path).collect();
        let mut sorted = paths.clone();
        sorted.sort();
        assert_eq!(paths, sorted, "presentation order is the stable sort");
        assert_eq!(panel.distinct_sites(), 8);
        assert!(!panel.empty_state_visible());
    }

    /// AC13: a zero-match query shows the explicit no-matches state.
    #[test]
    fn zero_matches_shows_the_empty_state() {
        let mut panel = CodeSearchPanel::new();
        assert!(!panel.empty_state_visible(), "no query yet — no empty state");
        let (tx, rx) = channel();
        tx.send(ScanMsg::Done(ScanStats::default())).unwrap();
        panel.rx = Some(rx);
        panel.scanning = true;
        panel.searched_once = true;
        panel.pump();
        assert!(panel.empty_state_visible(), "explicit no-matches state (R25)");
    }

    // ── The window (AC17, AC18, AC19) ────────────────────────────────────────

    fn overflow_hits(n: usize) -> Vec<SearchHit> {
        (0..n)
            .map(|i| SearchHit {
                form_name: format!("FORM-{:02}", i % 7),
                form_path: None,
                site: CodeSite::Section(StructureSection::WorkingStorage),
                line: i as u32 + 1,
                span: (30, 38),
                line_text: format!(
                    "       01  WS-FIELD-{i:04}  PIC X(200) VALUE \"{}\".",
                    "A-VERY-LONG-VALUE-".repeat(8)
                ),
            })
            .collect()
    }

    fn run_frame(
        ctx: &egui::Context,
        panel: &mut CodeSearchPanel,
        open: &mut bool,
        events: Vec<egui::Event>,
    ) -> (Option<egui::Rect>, CodeSearchAction) {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(1600.0, 1000.0),
        ));
        input.events = events;
        let tr = crate::i18n::Language::English.tr();
        let mut action = CodeSearchAction::None;
        ctx.run_ui(input, |root_ui| {
            let ctx2 = root_ui.ctx().clone();
            action = panel.show(&ctx2, open, &tr, 0);
        })
        .textures_delta
        .clear();
        (
            ctx.memory(|m| m.area_rect(egui::Id::new("code_search_window"))),
            action,
        )
    }

    /// Operator (2026-08-23): the cursor over a search result is the pointing
    /// hand — the hyperlink convention, and the same affordance the clickable
    /// diagnostic rows already carry. Hovering the result list must set it;
    /// hovering outside the window must not.
    #[test]
    fn a_hovered_result_row_shows_the_pointing_hand() {
        let ctx = egui::Context::default();
        let mut panel = CodeSearchPanel::new();
        panel.hits = overflow_hits(300);
        panel.searched_once = true;
        let mut open = true;

        // Settle the window and learn where it is.
        let mut rect = None;
        for _ in 0..5 {
            rect = run_frame(&ctx, &mut panel, &mut open, vec![]).0;
        }
        let rect = rect.expect("window rect");

        let cursor_at = |panel: &mut CodeSearchPanel, open: &mut bool, pos: egui::Pos2| {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1600.0, 1000.0),
            ));
            input.events = vec![egui::Event::PointerMoved(pos)];
            let tr = crate::i18n::Language::English.tr();
            let full = ctx.run_ui(input, |root_ui| {
                let ctx2 = root_ui.ctx().clone();
                panel.show(&ctx2, open, &tr, 0);
            });
            full.platform_output.cursor_icon
        };

        // The list fills the central panel; with 300 rows the window's centre
        // is on a result row. Sample a few heights so a group header under
        // one probe cannot fake a failure.
        let hand = [0.45, 0.55, 0.65].iter().any(|f| {
            let pos = egui::pos2(rect.center().x, rect.min.y + rect.height() * f);
            cursor_at(&mut panel, &mut open, pos) == egui::CursorIcon::PointingHand
        });
        assert!(hand, "hovering the result list must show the pointing hand");

        // Outside the window: the default cursor, not a stuck hand.
        let outside = egui::pos2(1500.0, 950.0);
        assert_ne!(
            cursor_at(&mut panel, &mut open, outside),
            egui::CursorIcon::PointingHand,
            "the hand must not leak outside the result rows"
        );
    }

    /// AC17 — the self-inflation guard, written before the window had real
    /// content: 120 frames with a result list long and wide enough to
    /// overflow must hold the seeded size within 0.5 px once settled.
    #[test]
    fn window_holds_seeded_size_across_120_frames() {
        let ctx = egui::Context::default();
        let mut panel = CodeSearchPanel::new();
        panel.hits = overflow_hits(300);
        panel.searched_once = true;
        panel.stats = Some(ScanStats {
            occurrences: 300,
            ..Default::default()
        });
        let mut open = true;

        let mut sizes: Vec<egui::Vec2> = Vec::new();
        for _ in 0..120 {
            if let (Some(r), _) = run_frame(&ctx, &mut panel, &mut open, vec![]) {
                sizes.push(r.size());
            }
        }
        assert!(sizes.len() >= 100, "window rect missing most frames");
        let settled = sizes[4];
        let mut max_drift = 0.0f32;
        for (i, s) in sizes.iter().enumerate().skip(4) {
            max_drift = max_drift.max((s.x - settled.x).abs().max((s.y - settled.y).abs()));
            assert!(
                (s.x - settled.x).abs() < 0.5 && (s.y - settled.y).abs() < 0.5,
                "search window drifted at frame {i}: {settled:?} -> {s:?} \
                 (self-inflation regression)"
            );
        }
        assert!(
            settled.x < SEARCH_WINDOW_SEED[0] + 100.0
                && settled.y < SEARCH_WINDOW_SEED[1] + 150.0,
            "settled far above the seed: {settled:?}"
        );
        println!(
            "search window stable at {:.0}x{:.0} px across {} frames (max drift {:.3} px)",
            settled.x,
            settled.y,
            sizes.len(),
            max_drift
        );
    }

    /// AC18 — only the user's grip drag resizes the window, and the dragged
    /// size survives a result list of a very different length.
    #[test]
    fn grip_drag_resizes_and_the_size_sticks() {
        let ctx = egui::Context::default();
        let mut panel = CodeSearchPanel::new();
        panel.hits = overflow_hits(300);
        panel.searched_once = true;
        let mut open = true;

        let mut rect = None;
        for _ in 0..8 {
            rect = run_frame(&ctx, &mut panel, &mut open, vec![]).0;
        }
        let before = rect.expect("window rect").size();

        // Drag the bottom-right grip by (+140, +90).
        let grip = rect.unwrap().max - egui::vec2(6.0, 6.0);
        let end = grip + egui::vec2(140.0, 90.0);
        run_frame(
            &ctx,
            &mut panel,
            &mut open,
            vec![egui::Event::PointerMoved(grip)],
        );
        run_frame(
            &ctx,
            &mut panel,
            &mut open,
            vec![egui::Event::PointerButton {
                pos: grip,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            }],
        );
        run_frame(
            &ctx,
            &mut panel,
            &mut open,
            vec![egui::Event::PointerMoved(end)],
        );
        rect = run_frame(
            &ctx,
            &mut panel,
            &mut open,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            }],
        )
        .0;
        let after = rect.expect("window rect").size();
        assert!(
            after.x > before.x + 100.0 && after.y > before.y + 50.0,
            "the grip drag must grow the window: {before:?} -> {after:?}"
        );

        // A very different result list must NOT move the dragged size.
        panel.hits = overflow_hits(2);
        let mut sizes = Vec::new();
        for _ in 0..30 {
            if let (Some(r), _) = run_frame(&ctx, &mut panel, &mut open, vec![]) {
                sizes.push(r.size());
            }
        }
        let last = *sizes.last().unwrap();
        assert!(
            (last.x - after.x).abs() < 0.5 && (last.y - after.y).abs() < 0.5,
            "the dragged size must survive a re-query: {after:?} -> {last:?}"
        );
        println!(
            "grip drag: {:.0}x{:.0} -> {:.0}x{:.0}, held at {:.0}x{:.0} after re-query",
            before.x, before.y, after.x, after.y, last.x, last.y
        );
    }

    /// AC19 — the window stays open through everything except `✕`/Cancel:
    /// clicks elsewhere in the IDE, a changed result set (what a re-Check
    /// produces), and a live-forms change (a form opening/closing changes the
    /// fingerprint, which marks results stale — it must not close the
    /// window).
    #[test]
    fn window_survives_clicks_rechecks_and_form_changes() {
        let ctx = egui::Context::default();
        let mut panel = CodeSearchPanel::new();
        panel.hits = overflow_hits(50);
        panel.searched_once = true;
        let mut open = true;

        for _ in 0..5 {
            run_frame(&ctx, &mut panel, &mut open, vec![]);
        }
        // A click far outside the window (in the editor behind it).
        let far = egui::pos2(1500.0, 950.0);
        run_frame(
            &ctx,
            &mut panel,
            &mut open,
            vec![
                egui::Event::PointerMoved(far),
                egui::Event::PointerButton {
                    pos: far,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
                egui::Event::PointerButton {
                    pos: far,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                },
            ],
        );
        assert!(open, "a click elsewhere must not close the window (R35)");

        // A re-Check replaces the diagnostics and the result set.
        panel.hits = overflow_hits(3);
        for _ in 0..3 {
            run_frame(&ctx, &mut panel, &mut open, vec![]);
        }
        assert!(open, "a re-check must not close the window (R35)");

        // A form opened/closed elsewhere: only the fingerprint changes.
        let tr = crate::i18n::Language::English.tr();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(1600.0, 1000.0),
        ));
        ctx.run_ui(input, |root_ui| {
            let ctx2 = root_ui.ctx().clone();
            panel.show(&ctx2, &mut open, &tr, 12345 /* different fingerprint */);
        })
        .textures_delta
        .clear();
        assert!(open, "a form change must not close the window (R35)");
    }

    // ── Measured performance (AC14 / R27) ────────────────────────────────────

    /// A scan across a generated 50-form project reports measured counts and
    /// timings — forms scanned, sites scanned, occurrences, elapsed ms — all
    /// numbers this run produced.
    #[test]
    fn a_fifty_form_scan_reports_measured_numbers() {
        // Measure one form first, so the 50-form expectation is a number this
        // run produced, not a guess.
        let mut one = Vec::new();
        scan_form(&all_sites_fixture(), None, "MARK-", true, false, &mut one);
        let per_form = one.len();
        assert!(per_form > 0);

        let mut live_forms = Vec::new();
        for i in 0..50 {
            let mut form = all_sites_fixture();
            form.name = format!("FORM-{i:02}");
            live_forms.push((PathBuf::from(format!("/proj/forms/form-{i:02}.cfrm")), form));
        }
        let inputs = ScanInputs {
            live_forms,
            disk_forms: Vec::new(),
            common_files: Vec::new(),
            fingerprint: 0,
        };
        let (tx, rx) = channel();
        run_scan(inputs, "MARK-".into(), true, false, tx);
        let mut hits = 0usize;
        let mut stats = None;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                ScanMsg::Hit(_) => hits += 1,
                ScanMsg::Done(s) => stats = Some(s),
            }
        }
        let stats = stats.expect("scan finished");
        println!("── code-search scan summary (spec 053 AC14) ─────────────");
        println!("  forms scanned : {}", stats.forms_scanned);
        println!("  sites scanned : {}", stats.sites_scanned);
        println!("  occurrences   : {}", stats.occurrences);
        println!("  elapsed       : {} ms", stats.elapsed_ms);
        assert_eq!(stats.forms_scanned, 50);
        assert_eq!(stats.sites_scanned, 50 * 8);
        assert_eq!(stats.occurrences, hits);
        assert_eq!(
            stats.occurrences,
            50 * per_form,
            "fifty forms of {per_form} measured occurrences each"
        );
    }
}

