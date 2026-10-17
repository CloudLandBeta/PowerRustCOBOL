// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Top toolbar panel — Run ▶, Stop ■, Build ⚙ buttons + language selector.

use egui::{Button, Color32, Context, Panel, RichText};

use crate::i18n::{Language, Tr};
use crate::runner::Runner;

/// Render the toolbar and return the user's action (if any).
///
/// `lang` is updated in-place when the user picks a different language.
/// `compilable` gates the Run / Debug / Build actions (a project needs at least
/// one COBOL program or one form).
/// `has_unsaved` gates Save: the button is live only while something in the
/// project is actually unsaved, so its state reports whether the project is clean.
pub fn show(
    panel_ui: &mut egui::Ui,
    ctx: &Context,
    runner: &Runner,
    tr: &Tr,
    lang: &mut Language,
    compilable: bool,
    debuggable: bool,
    // `debug_hint`: why Debug is greyed when it is — the project has not been
    // built yet, or no COBOL file is open. Chosen by the app, which knows which.
    debug_hint: &'static str,
    // `has_active`: true when there is an active project or open file — gates Save/Check.
    has_active: bool,
    // `has_unsaved`: true when a form designer, an editor tab, or the Settings
    // form has pending edits — gates Save only.
    has_unsaved: bool,
    // `building`: a build is already running. Every build of a project stages
    // into one folder and a full build empties it first, so a second build
    // wrecks the first — the File menu has always greyed its Build item out
    // while one runs, and this is the same rule on the button beside it.
    building: bool,
    // `wizard_on`: the Task Wizard switch (spec 104), shown engaged while on.
    wizard_on: bool,
) -> ToolbarAction {
    let mut action = ToolbarAction::None;
    let busy = runner.is_running();
    // Aurora Pastel draws each button as its own raised gradient pill
    // (`crate::aurora::pill`); every other theme keeps egui's buttons.
    let aurora = crate::aurora::active();
    let pills = crate::aurora::pills();

    Panel::top("toolbar").show(panel_ui, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(4.0);

            // Order: Open | Save | Check | Build | Debug | Run | Stop | ⚙

            // Every button flashes briefly on click (crate::theme::flash_on_click)
            // so a press is always visibly acknowledged.

            // ── Open file ─────────────────────────────────────────────────────
            let open_resp = if aurora {
                crate::aurora::pill(ui, true, tr.tb_open, pills.open)
            } else {
                ui.button(tr.tb_open)
            };
            if open_resp.clicked() {
                action = ToolbarAction::Open;
            }
            crate::theme::flash_on_click(ui, &open_resp);

            // ── Save ──────────────────────────────────────────────────────────
            // Live only when something is genuinely unsaved (dirty form designer,
            // dirty editor tab, or unsaved project Settings). A clean project
            // shows a greyed Save, so the button itself is the dirty indicator.
            let save_enabled = has_active && has_unsaved;
            let save_resp = if aurora {
                crate::aurora::pill(ui, save_enabled, tr.tb_save, pills.open)
            } else {
                ui.add_enabled(save_enabled, Button::new(tr.tb_save))
            };
            if save_resp.clicked() {
                action = ToolbarAction::Save;
            }
            crate::theme::flash_on_click(ui, &save_resp);
            if has_active && !has_unsaved {
                save_resp.on_disabled_hover_text(tr.tb_nothing_to_save);
            }

            ui.separator();

            // ── Check (parse/analyse only) ────────────────────────────────────
            let check_resp = if aurora {
                crate::aurora::pill(ui, has_active, tr.tb_check, pills.check)
            } else {
                ui.add_enabled(has_active, Button::new(tr.tb_check))
            };
            if check_resp.clicked() {
                action = ToolbarAction::Check;
            }
            crate::theme::flash_on_click(ui, &check_resp);

            // ── Project-wide code search (spec 053) ───────────────────────────
            let search_resp = if aurora {
                crate::aurora::pill(ui, has_active, tr.tb_search, pills.search)
            } else {
                ui.add_enabled(has_active, Button::new(tr.tb_search))
            }
            .on_hover_text(tr.tb_search_hint);
            if search_resp.clicked() {
                action = ToolbarAction::Search;
            }
            crate::theme::flash_on_click(ui, &search_resp);

            // ── Build binary ──────────────────────────────────────────────────
            let build_resp = if aurora {
                crate::aurora::pill(ui, compilable && !building, tr.tb_build, pills.build)
            } else {
                ui.add_enabled(compilable && !building, Button::new(tr.tb_build))
            };
            if build_resp.clicked() {
                action = ToolbarAction::Build;
            }
            crate::theme::flash_on_click(ui, &build_resp);
            // `on_disabled_hover_text`, not `on_hover_text`: the button is
            // disabled in both branches, and egui shows the plain tooltip only
            // on an ENABLED widget — so the "add a program first" hint has
            // never once appeared. Save, three buttons to the left, has had it
            // right all along.
            if !compilable {
                build_resp.on_disabled_hover_text(tr.tb_need_program);
            } else if building {
                build_resp.on_disabled_hover_text(tr.tb_build_busy);
            }

            ui.separator();

            // ── Run (interpreted) ─────────────────────────────────────────────
            let run_btn = Button::new(RichText::new(tr.tb_run).color(if busy || !compilable {
                Color32::GRAY
            } else {
                Color32::from_rgb(80, 200, 80)
            }));
            let run_resp = if aurora {
                crate::aurora::pill(ui, !busy && compilable, tr.tb_run, pills.run)
            } else {
                ui.add_enabled(!busy && compilable, run_btn)
            };
            if run_resp.clicked() {
                action = ToolbarAction::Run;
            }
            crate::theme::flash_on_click(ui, &run_resp);
            if !compilable {
                run_resp.on_hover_text(tr.tb_need_program);
            }

            // ── Debug (right of Run; in a project, enabled once it has been built) ─
            let dbg_resp = if aurora {
                crate::aurora::pill(ui, !busy && debuggable, tr.tb_debug, pills.debug)
            } else {
                ui.add_enabled(
                !busy && debuggable,
                Button::new(RichText::new(tr.tb_debug).color(if busy || !debuggable {
                    Color32::GRAY
                } else {
                    Color32::from_rgb(200, 150, 80)
                })),
            )
            };
            if dbg_resp.clicked() {
                action = ToolbarAction::Debug;
            }
            crate::theme::flash_on_click(ui, &dbg_resp);
            if !debuggable {
                dbg_resp.on_hover_text(debug_hint);
            }

            // ── Stop ─────────────────────────────────────────────────────────
            let stop_btn = Button::new(RichText::new(tr.tb_stop).color(if busy {
                Color32::from_rgb(220, 80, 80)
            } else {
                Color32::GRAY
            }));
            let stop_resp = if aurora {
                crate::aurora::pill(ui, busy, tr.tb_stop, pills.stop)
            } else {
                ui.add_enabled(busy, stop_btn)
            };
            if stop_resp.clicked() {
                action = ToolbarAction::Stop;
            }
            crate::theme::flash_on_click(ui, &stop_resp);

            // Settings now live in the Main Pane (click the project node); the
            // toolbar no longer has a gear button.

            // ── Right side: spinner + language selector ────────────────────────
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Language selector ────────────────────────────────────────────
                ui.add_space(4.0);
                egui::ComboBox::from_id_salt("ide_lang_selector")
                    .selected_text(lang.native_name())
                    .width(140.0)
                    .show_ui(ui, |ui| {
                        for &l in Language::ALL {
                            if crate::flags::language_row(ui, l, *lang == l).clicked() {
                                *lang = l;
                            }
                        }
                    });
                // The flag sits left of the closed combo (this is a right-to-left
                // layout, so it is added after it).
                crate::flags::flag_widget(ui, *lang);
                ui.add_space(4.0);
                ui.label("🌐").on_hover_text(tr.lang_btn_tooltip);
                ui.separator();

                // ── Task Wizard: the switch, and its ▾ (spec 104 R1–R3) ─────────
                // Right to left, so the ▾ is added first and lands right of the
                // switch.
                if let Some(a) = task_wizard_toggle(ui, tr, wizard_on, aurora) {
                    action = a;
                }
                ui.separator();

                if runner.is_running() {
                    ui.spinner();
                    ui.label(RichText::new(tr.tb_running).color(Color32::YELLOW));
                }
            });
        });
    });

    action
}

/// The Task Wizard's split button, in a right-to-left layout: the ▾, then the switch.
///
/// The switch reads as **engaged** while on (R2): egui's own selected face in every theme,
/// the palette's selected-tab pill under Aurora. The ▾ is live only while on (R3). Both
/// report their rect to the wizard, so they are always holes in its dim (R16), and the
/// switch remembers its rect for the catalogue to hang under.
fn task_wizard_toggle(
    ui: &mut egui::Ui,
    tr: &Tr,
    on: bool,
    aurora: bool,
) -> Option<ToolbarAction> {
    use crate::panels::task_wizard::{anchors, catalogue, AnchorId};
    let mut action = None;
    let more = ui
        .add_enabled(on, Button::new("▾").min_size(egui::vec2(18.0, 0.0)))
        .on_hover_text(tr.task_wizard_catalogue_tip);
    if more.clicked() {
        action = Some(ToolbarAction::OpenTaskCatalogue);
    }
    let switch = if aurora {
        let look = if on {
            crate::aurora::engaged_pill()
        } else {
            crate::aurora::pills().open
        };
        crate::aurora::pill(ui, true, tr.task_wizard_toggle, look)
    } else {
        ui.add(Button::new(tr.task_wizard_toggle).selected(on))
    };
    let switch = switch.on_hover_text(if on {
        tr.task_wizard_tip_on
    } else {
        tr.task_wizard_tip_off
    });
    if switch.clicked() {
        action = Some(ToolbarAction::ToggleTaskWizard);
    }
    crate::theme::flash_on_click(ui, &switch);
    anchors::mark(ui, AnchorId::Toggle, switch.rect.union(more.rect));
    catalogue::remember_toggle(ui.ctx(), switch.rect.union(more.rect));
    action
}

/// Action requested by the toolbar buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarAction {
    None,
    Run,
    Stop,
    Debug,
    Build,
    Check,
    Open,
    Save,
    /// Open the project-wide code search window (spec 053).
    Search,
    /// Turn the Task Wizard on or off (spec 104 R1).
    ToggleTaskWizard,
    /// Reopen the Task Wizard's catalogue (spec 104 R3).
    OpenTaskCatalogue,
}
