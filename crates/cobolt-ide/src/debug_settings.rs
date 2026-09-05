// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! IDE-wide **debug switches** — the single home for every diagnostic that used
//! to be reachable only through an environment variable.
//!
//! # Scope
//!
//! These are developer-machine settings, not project data: they live in
//! `<data_dir>/cobolt/debug_settings.toml` (next to `doc_viewer.toml`), so they
//! survive across projects, never reach `cobolt.toml`, and the Help → Debug
//! Settings modal works even with no project open.
//!
//! # How a switch reaches the thing it controls
//!
//! | Consumer                    | Mechanism                                        |
//! |-----------------------------|--------------------------------------------------|
//! | Design canvas / IDE process | [`DebugSettings::apply_in_process`], every frame  |
//! | `rcrun run-form` child      | [`DebugSettings::child_env`], at spawn            |
//!
//! The env vars are still read (by `rcrun`, and by `cobolt-forms` as a one-shot
//! seed) — they are simply no longer the *user interface*. A developer who
//! exports one by hand still gets it in a standalone `rcrun` run.
//!
//! # Adding a switch
//!
//! Add the field, then one [`Switch`] entry to the right [`Section`] in
//! [`SECTIONS`]. The tab strip is generated from that table and skips sections
//! with no switches, so an empty section can never appear.

use serde::{Deserialize, Serialize};

use crate::i18n::Tr;

/// Every debug switch the IDE exposes, with the env var each one mirrors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DebugSettings {
    // ── User interface ────────────────────────────────────────────────────
    /// Explode each control's composite layers into coloured, offset frames.
    pub frame_diagnostics: bool,
    /// Outline every internal DataGrid sub-component.
    pub datagrid_diagnostics: bool,
    // ── Data binding ──────────────────────────────────────────────────────
    /// Run-form data-binding trace: `databinding.log` in the platform's
    /// diagnostics directory, plus the state-key mismatch dump.
    pub databind_trace: bool,
    /// One line per UI event at BOTH ends of the channel, plus the program's
    /// own DISPLAY output, on one timeline in `prc-event-trace.log`.
    pub event_trace: bool,
    // ── Agentic AI ────────────────────────────────────────────────────────
    /// 038 R14 — machine-wide window-effects kill-switch: every entrance/exit
    /// effect is skipped (instant open/close/restore) without touching any
    /// project or form. Accessibility / weak-GPU escape hatch, NOT a
    /// diagnostic: it does not count toward `any_enabled`.
    pub no_window_fx: bool,
    /// F12 captures the focused window into a `📷 Screenshot needed` slot of the
    /// English guide (see [`crate::doc_shots`]). An authoring tool for this
    /// checkout, NOT a diagnostic: it does not count toward `any_enabled`.
    pub doc_screenshots: bool,
    // ── Indexed files ─────────────────────────────────────────────────────
    /// INDEXED transaction log level: `off` | `basic` | `full` (redb engine).
    pub indexed_log: String,
    /// INDEXED log line format: `text` (logfmt) | `json` (NDJSON).
    pub indexed_log_format: String,
    // ── Logging ───────────────────────────────────────────────────────────
    /// `tracing` env-filter, e.g. `debug`, `cobolt-runtime=trace`,
    /// `databinding=debug`. Empty ⇒ the built-in `warn` default.
    pub log_filter: String,
}

impl Default for DebugSettings {
    fn default() -> Self {
        Self {
            frame_diagnostics: false,
            datagrid_diagnostics: false,
            databind_trace: false,
            event_trace: false,
            no_window_fx: false,
            doc_screenshots: false,
            indexed_log: "off".into(),
            indexed_log_format: "text".into(),
            log_filter: String::new(),
        }
    }
}

fn settings_path() -> std::path::PathBuf {
    crate::llm::base_dir().join("debug_settings.toml")
}

impl DebugSettings {
    /// Load the persisted switches, falling back to "everything off" on any
    /// error (a corrupt file must never keep the IDE from starting).
    pub fn load() -> Self {
        std::fs::read_to_string(settings_path())
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Write the switches to disk (best-effort).
    pub fn save(&self) {
        if let Ok(text) = toml::to_string_pretty(self) {
            let path = settings_path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, text);
        }
    }

    /// `true` when any switch is on — the trigger for the per-control
    /// diagnostics dump at `<diagnostics dir>/<project>_diagnostics_dump.log`.
    pub fn any_enabled(&self) -> bool {
        self.frame_diagnostics
            || self.datagrid_diagnostics
            || self.databind_trace
            || self.event_trace
            || !self.log_filter.trim().is_empty()
            || self.indexed_log != "off"
    }

    /// Push the switches the IDE process itself honours into their runtime
    /// flags. Called every frame, so a toggle applies to the design canvas
    /// without a restart.
    pub fn apply_in_process(&self) {
        cobolt_forms::paint::set_frame_diagnostics(self.frame_diagnostics);
        cobolt_forms::paint::set_datagrid_diagnostics(self.datagrid_diagnostics);
    }

    /// Env pairs for a spawned `rcrun` child. The booleans are passed
    /// explicitly (including `"0"`) so these settings are authoritative over
    /// anything the IDE inherited from its own environment; `COBOLT_LOG` is
    /// passed only when set, leaving the child's own default in place.
    ///
    /// AI-pane debug is IDE-only (it sizes the IDE's own AI pane) and is
    /// deliberately not forwarded.
    pub fn child_env(&self) -> Vec<(&'static str, String)> {
        let onoff = |b: bool| if b { "1" } else { "0" }.to_string();
        let mut env = vec![
            ("COBOLT_FRAME_DIAGNOSTICS", onoff(self.frame_diagnostics)),
            (
                "COBOLT_DATAGRID_DIAGNOSTICS",
                onoff(self.datagrid_diagnostics),
            ),
            ("COBOLT_DATABIND_TRACE", onoff(self.databind_trace)),
            ("COBOLT_EVENT_TRACE", onoff(self.event_trace)),
            ("PRC_NO_WINDOW_FX", onoff(self.no_window_fx)),
            ("COBOL_INDEXED_LOG", self.indexed_log.clone()),
            ("COBOL_INDEXED_LOG_FORMAT", self.indexed_log_format.clone()),
        ];
        let filter = self.log_filter.trim();
        if !filter.is_empty() {
            env.push(("COBOLT_LOG", filter.to_string()));
        }
        env
    }
}

// ── Switch catalogue ──────────────────────────────────────────────────────────

/// One row in the modal. `env` is the variable the switch mirrors, shown in the
/// hover so a developer can still reproduce the setting from a shell.
enum Switch {
    Flag {
        label: &'static str,
        hint: &'static str,
        env: &'static str,
        get: fn(&mut DebugSettings) -> &mut bool,
    },
    /// A fixed set of values, rendered as a combo box.
    Choice {
        label: &'static str,
        hint: &'static str,
        env: &'static str,
        options: &'static [&'static str],
        get: fn(&mut DebugSettings) -> &mut String,
    },
    /// Free text, rendered as a single-line edit.
    Text {
        label: &'static str,
        hint: &'static str,
        env: &'static str,
        get: fn(&mut DebugSettings) -> &mut String,
    },
}

/// One tab. The caption is localized; the switches decide whether the tab
/// exists at all.
struct Section {
    tab: fn(&Tr) -> &'static str,
    switches: &'static [Switch],
}

/// The tab strip, in display order. A section with an empty `switches` slice is
/// never rendered.
static SECTIONS: &[Section] = &[
    Section {
        tab: |tr| tr.debug_tab_ui,
        switches: &[
            Switch::Flag {
                label: "Frame diagnostics overlay",
                hint: "Explode each control's composite layers (shadow/face/border/content/\
                       outline) into coloured, offset frames so rounded-corner bleed and \
                       mis-rounded layers are visible.",
                env: "COBOLT_FRAME_DIAGNOSTICS",
                get: |s| &mut s.frame_diagnostics,
            },
            Switch::Flag {
                label: "DataGrid component frames",
                hint: "Private to the DataGrid: outline every internal sub-component (whole \
                       grid, header, body, each column, each visible row and cell, frozen \
                       panes, scrollbar) in a distinct colour, so a mis-sized or mis-placed \
                       part is obvious.",
                env: "COBOLT_DATAGRID_DIAGNOSTICS",
                get: |s| &mut s.datagrid_diagnostics,
            },
            Switch::Flag {
                label: "Disable window effects",
                hint: "Skip every project window entrance/exit effect (instant open, close \
                       and restore) without changing any project or form — for motion \
                       sensitivity, weak GPUs, or automation (spec 038). The same variable \
                       works for a bare `rcrun run-form`.",
                env: "PRC_NO_WINDOW_FX",
                get: |s| &mut s.no_window_fx,
            },
            Switch::Flag {
                label: "Doc screenshot capture (F12)",
                hint: "Authoring tool for this checkout, not a product feature: F12 captures \
                       the focused window (Shift+F12 waits 3 s, for states that must be held \
                       with the mouse; Ctrl+F12 starts a screen RECORDING and any F12 stops \
                       it), then a popup places the shot into a `📷 Screenshot needed` slot \
                       of ANY English Markdown document — the guide, README.md, or anything \
                       under docs/ — saving the PNG under assets/images/screenshots/ and \
                       writing the markdown. A recording is saved as an animated PNG, so it \
                       fills an ordinary screenshot slot; it runs at about 8 frames a second \
                       with a smoothed pointer, and stops itself after 90 seconds. Capture \
                       runs on its own thread, so a busy IDE does not delay it. English \
                       documents only; the translated guides reference the same images.",
                env: "PRC_DOC_SCREENSHOTS",
                get: |s| &mut s.doc_screenshots,
            },
        ],
    },
    Section {
        tab: |tr| tr.debug_tab_databind,
        switches: &[Switch::Flag {
            label: "Data-bind trace",
            hint: "Run Form writes databinding.log to the diagnostics folder (/tmp on Linux \
                   and macOS, %TEMP% on Windows): repeating-group seeding and per-row \
                   control-array binding, plus — once — the mismatch between the state keys \
                   the interpreter populated and the instanced ids the renderer looks up. \
                   Decisive for \"cards show designed defaults in run-form but not in \
                   preview\".",
            env: "COBOLT_DATABIND_TRACE",
            get: |s| &mut s.databind_trace,
        }],
    },
    Section {
        tab: |tr| tr.debug_tab_events,
        switches: &[Switch::Flag {
            label: "Event trace",
            hint: "One line per UI event at BOTH ends of the channel — the host's `send` and \
                   the interpreter's `dispatch` — interleaved with the program's own DISPLAY \
                   output in prc-event-trace.log (/tmp on Linux and macOS, %TEMP% on \
                   Windows). Their ORDER is the point: it tells a handler that ran twice \
                   because the queue delivered twice apart from one that ran twice from a \
                   single delivery.",
            env: "COBOLT_EVENT_TRACE",
            get: |s| &mut s.event_trace,
        }],
    },
    Section {
        tab: |tr| tr.debug_tab_indexed,
        switches: &[
            Switch::Choice {
                label: "INDEXED transaction log",
                hint: "Per-file INDEXED transaction log written to <assign-path>.log (redb \
                       engine only). basic = one line per operation; full = adds key and \
                       record detail.",
                env: "COBOL_INDEXED_LOG",
                options: &["off", "basic", "full"],
                get: |s| &mut s.indexed_log,
            },
            Switch::Choice {
                label: "INDEXED log format",
                hint: "Line format for the transaction log: text (logfmt) or json (NDJSON, \
                       ready for Grafana/Loki).",
                env: "COBOL_INDEXED_LOG_FORMAT",
                options: &["text", "json"],
                get: |s| &mut s.indexed_log_format,
            },
        ],
    },
    Section {
        tab: |tr| tr.debug_tab_logging,
        switches: &[Switch::Text {
            label: "Tracing filter",
            hint: "tracing env-filter applied to Run Form. Examples: debug · \
                   cobolt-runtime=trace · databinding=debug. Empty keeps the built-in warn \
                   level. The IDE's own log level is fixed at startup, so changing this \
                   affects the IDE only after a restart.",
            env: "COBOLT_LOG",
            get: |s| &mut s.log_filter,
        }],
    },
];

// ── Modal ─────────────────────────────────────────────────────────────────────

/// Scrollable height of the switch list. A constant on purpose — see the note
/// at the `ScrollArea` in [`DebugSettingsModal::show`].
const SWITCH_AREA_HEIGHT: f32 = 260.0;
/// Width of the switch-name column. Fits the longest shipped label with room
/// to spare, so nothing truncates.
const SWITCH_LABEL_WIDTH: f32 = 340.0;

/// Help → Debug Settings. Edits [`DebugSettings`] live; every change is saved
/// immediately, so there is no Apply/Cancel to get wrong.
#[derive(Default)]
pub struct DebugSettingsModal {
    pub open: bool,
    /// Index into the *rendered* (non-empty) sections.
    tab: usize,
}

impl DebugSettingsModal {
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// One frame. Returns `true` when a switch changed this frame (the caller
    /// re-applies the in-process flags immediately).
    pub fn show(&mut self, ctx: &egui::Context, settings: &mut DebugSettings, tr: &Tr) -> bool {
        if !self.open {
            return false;
        }
        let sections: Vec<&Section> = SECTIONS.iter().filter(|s| !s.switches.is_empty()).collect();
        if sections.is_empty() {
            return false;
        }
        self.tab = self.tab.min(sections.len() - 1);

        let mut changed = false;
        let mut open = self.open;
        let mut close = false;
        egui::Window::new(format!("🐞 {}", tr.debug_title))
            .id(egui::Id::new("debug_settings_modal"))
            .collapsible(false)
            .resizable(true)
            .default_size([700.0, 420.0])
            .min_size([560.0, 320.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                // Tab strip — one tab per section that has switches.
                ui.horizontal_wrapped(|ui| {
                    for (i, section) in sections.iter().enumerate() {
                        if ui
                            .selectable_label(self.tab == i, (section.tab)(tr))
                            .clicked()
                        {
                            self.tab = i;
                        }
                    }
                });
                ui.separator();

                // Fixed max height, never one derived from the available space:
                // a child sized from the space it is given inflates the window a
                // little more every frame.
                egui::ScrollArea::vertical()
                    .max_height(SWITCH_AREA_HEIGHT)
                    .show(ui, |ui| {
                        egui::Grid::new("debug_switches")
                            .num_columns(2)
                            .spacing([16.0, 10.0])
                            .show(ui, |ui| {
                                for switch in sections[self.tab].switches {
                                    changed |= switch_row(ui, switch, settings);
                                    ui.end_row();
                                }
                            });
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Label::new(egui::RichText::new(tr.debug_scope_hint).small().weak())
                            .wrap(),
                    );
                });
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(tr.debug_close).clicked() {
                            close = true;
                        }
                    });
                });
            });

        self.open = open && !close;
        if changed {
            settings.save();
        }
        changed
    }
}

/// One label + editor row. Returns `true` when the value changed.
fn switch_row(ui: &mut egui::Ui, switch: &Switch, settings: &mut DebugSettings) -> bool {
    let (label, hint, env) = match switch {
        Switch::Flag {
            label, hint, env, ..
        }
        | Switch::Choice {
            label, hint, env, ..
        }
        | Switch::Text {
            label, hint, env, ..
        } => (*label, *hint, *env),
    };
    // The env var stays visible: these switches replace it as the interface, not
    // as the mechanism, and a developer still needs it for a shell run.
    let hover = format!("{hint}\n\nEnv: {env}");

    // `truncate()` alone lets the grid column collapse to its minimum, which is
    // how every label had become "Fra…", "Da…", "Ro…". The width is stated
    // explicitly instead: wide enough for the longest switch name, and a
    // constant rather than a share of the available space, so a resizable
    // window cannot be widened by its own labels.
    ui.allocate_ui_with_layout(
        egui::vec2(SWITCH_LABEL_WIDTH, 0.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.add(egui::Label::new(label).truncate())
                .on_hover_text(hover.clone());
        },
    );

    let mut changed = false;
    match switch {
        Switch::Flag { get, .. } => {
            let value = get(settings);
            changed = ui.checkbox(value, "").on_hover_text(hover).changed();
        }
        Switch::Choice { get, options, .. } => {
            let value = get(settings);
            egui::ComboBox::from_id_salt(label)
                .selected_text(value.clone())
                .width(160.0)
                .show_ui(ui, |ui| {
                    for opt in *options {
                        if ui.selectable_label(value == opt, *opt).clicked() {
                            *value = (*opt).to_string();
                            changed = true;
                        }
                    }
                })
                .response
                .on_hover_text(hover);
        }
        Switch::Text { get, .. } => {
            let value = get(settings);
            changed = ui
                .add(
                    egui::TextEdit::singleline(value)
                        .hint_text("warn")
                        .desired_width(280.0),
                )
                .on_hover_text(hover)
                .changed();
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_empty_sections_are_declared() {
        for section in SECTIONS {
            assert!(
                !section.switches.is_empty(),
                "a section with no switches would render an empty tab"
            );
        }
    }

    #[test]
    fn defaults_are_all_quiet() {
        let d = DebugSettings::default();
        assert!(!d.any_enabled());
    }

    #[test]
    fn child_env_is_explicit_about_off() {
        let env = DebugSettings::default().child_env();
        let get = |k: &str| {
            env.iter()
                .find(|(name, _)| *name == k)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(get("COBOLT_DATABIND_TRACE"), Some("0"));
        assert_eq!(get("COBOL_INDEXED_LOG"), Some("off"));
        assert_eq!(get("PRC_NO_WINDOW_FX"), Some("0"), "fx kill-switch explicit off");
        // No filter set ⇒ the child keeps its own default.
        assert_eq!(get("COBOLT_LOG"), None);
    }

    /// 038 R14 — the window-effects kill-switch reaches children as "1" when
    /// on, and is NOT a diagnostic: it never triggers the dump condition.
    #[test]
    fn window_fx_kill_switch_forwards_without_counting_as_diagnostic() {
        let mut s = DebugSettings::default();
        s.no_window_fx = true;
        assert!(
            !s.any_enabled(),
            "kill-switch must not trigger the diagnostics dump"
        );
        let env = s.child_env();
        assert!(
            env.contains(&("PRC_NO_WINDOW_FX", "1".to_string())),
            "child env carries the kill-switch"
        );
        println!("kill-switch: child env PRC_NO_WINDOW_FX=1, any_enabled={}", s.any_enabled());
    }

    #[test]
    fn log_filter_is_forwarded_when_set() {
        let mut s = DebugSettings::default();
        s.log_filter = "  databinding=debug  ".into();
        assert!(s.any_enabled());
        let env = s.child_env();
        assert!(env.contains(&("COBOLT_LOG", "databinding=debug".to_string())));
    }

    #[test]
    fn roundtrips_through_toml() {
        let mut s = DebugSettings::default();
        s.frame_diagnostics = true;
        s.indexed_log = "full".into();
        let text = toml::to_string_pretty(&s).unwrap();
        let back: DebugSettings = toml::from_str(&text).unwrap();
        assert_eq!(s, back);
    }

    /// An older file that predates a field must still load.
    #[test]
    fn partial_file_falls_back_to_defaults() {
        let back: DebugSettings = toml::from_str("frame_diagnostics = true\n").unwrap();
        assert!(back.frame_diagnostics);
        assert_eq!(back.indexed_log, "off");
    }

    /// A file written while a since-removed switch existed must still load,
    /// with the stale key ignored and every other switch intact. Covers the
    /// rounded-corner GL clip (spec 057 AC8, glow-only on a wgpu build) and
    /// the AI-pane layout trace, whose only effect was terminal noise.
    #[test]
    fn a_file_naming_a_removed_switch_still_loads() {
        let back: DebugSettings = toml::from_str(
            "rounded_clip = true\nai_pane_debug = true\nframe_diagnostics = true\n",
        )
        .unwrap();
        assert!(back.frame_diagnostics);
        assert!(
            !SECTIONS
                .iter()
                .flat_map(|s| s.switches.iter())
                .any(|sw| matches!(sw, Switch::Flag { env, .. } if *env == "COBOLT_ROUNDED_CLIP")),
            "the switch is no longer offered"
        );
    }
}

#[cfg(test)]
mod switches_respond {
    use super::*;

    struct Harness {
        ctx: egui::Context,
        modal: DebugSettingsModal,
        settings: DebugSettings,
        tr: Tr,
    }

    impl Harness {
        fn new() -> Self {
            Self {
                ctx: egui::Context::default(),
                modal: DebugSettingsModal { open: true, tab: 0 },
                settings: DebugSettings::default(),
                tr: crate::i18n::Language::English.tr(),
            }
        }

        fn frame(&mut self, events: Vec<egui::Event>) {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            ));
            input.events = events;
            self.ctx.begin_pass(input);
            self.modal.show(&self.ctx, &mut self.settings, &self.tr);
            // epaint 0.36 asserts on dropping unapplied texture deltas; a
            // headless test pass has no renderer to apply them to.
            self.ctx.end_pass().textures_delta.clear();
        }

        fn settle(&mut self) {
            for _ in 0..6 {
                self.frame(vec![]);
            }
        }

        fn rect(&self) -> egui::Rect {
            self.ctx
                .memory(|m| m.area_rect(egui::Id::new("debug_settings_modal")))
                .expect("no debug settings window")
        }

        fn click(&mut self, pos: egui::Pos2) {
            self.frame(vec![egui::Event::PointerMoved(pos)]);
            self.frame(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            }]);
            self.frame(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }]);
            self.frame(vec![]);
        }

        fn read(&mut self, getters: &[Flag]) -> Vec<bool> {
            getters.iter().map(|(_, g)| *g(&mut self.settings)).collect()
        }
    }

    /// `(label, accessor)` for one checkbox switch.
    type Flag = (&'static str, fn(&mut DebugSettings) -> &mut bool);

    fn flags_of(section: &Section) -> Vec<Flag> {
        section
            .switches
            .iter()
            .filter_map(|sw| match sw {
                Switch::Flag { label, get, .. } => Some((*label, *get)),
                _ => None,
            })
            .collect()
    }

    /// Every checkbox in every tab must actually be clickable, and must move its
    /// own field and no other.
    ///
    /// The switches on disk had three of six flags set while the other three
    /// stayed `false`, which reads exactly like a dead row or a `get` closure
    /// pointing at the wrong field — and nothing in the suite could tell the
    /// difference, because the tests all called the accessors directly instead
    /// of going through the rendered checkbox. This one clicks.
    #[test]
    fn every_switch_moves_its_own_field() {
        for (index, section) in SECTIONS.iter().enumerate() {
            let flags = flags_of(section);
            if flags.is_empty() {
                continue;
            }
            let mut h = Harness::new();
            h.modal.tab = index;
            h.settle();
            let rect = h.rect();
            let mut found: Vec<Option<egui::Pos2>> = vec![None; flags.len()];

            // The checkbox column is wherever the label column happens to end,
            // so the click point is discovered rather than assumed. The title
            // bar and the button row are left out: a click there drags or
            // resizes the window and moves everything else under the cursor.
            let mut y = rect.top() + 60.0;
            'scan: while y < rect.bottom() - 60.0 {
                let mut x = rect.left() + 100.0;
                while x < rect.right() - 60.0 {
                    let before = h.read(&flags);
                    h.click(egui::pos2(x, y));
                    let after = h.read(&flags);
                    let moved: Vec<usize> = (0..flags.len())
                        .filter(|i| before[*i] != after[*i])
                        .collect();
                    assert!(
                        moved.len() <= 1,
                        "one click at ({x:.0},{y:.0}) moved several switches: {:?}",
                        moved.iter().map(|i| flags[*i].0).collect::<Vec<_>>()
                    );
                    if let Some(i) = moved.first() {
                        found[*i] = Some(egui::pos2(x, y));
                        h.settings = DebugSettings::default();
                        if found.iter().all(|f| f.is_some()) {
                            break 'scan;
                        }
                    }
                    if !h.modal.open {
                        h.modal.open = true;
                        h.settle();
                    }
                    x += 25.0;
                }
                y += 4.0;
            }

            for (i, (label, _)) in flags.iter().enumerate() {
                assert!(
                    found[i].is_some(),
                    "no click anywhere in the \"{}\" tab reached the \"{label}\" checkbox — \
                     the row renders but cannot be switched",
                    (section.tab)(&crate::i18n::Language::English.tr())
                );
            }
        }
    }
}
