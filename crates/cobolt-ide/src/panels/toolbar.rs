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
/// the palette's selected-tab pill under Aurora. The ▾ always works (R3): while off it
/// turns the wizard on, which opens the catalogue. Both report their rect to the wizard,
/// so they are always holes in its dim (R16), and the switch remembers its rect for the
/// catalogue to hang under.
fn task_wizard_toggle(
    ui: &mut egui::Ui,
    tr: &Tr,
    on: bool,
    aurora: bool,
) -> Option<ToolbarAction> {
    use crate::panels::task_wizard::{anchors, catalogue, AnchorId};
    let mut action = None;
    let more = ui
        .add(Button::new("▾").min_size(egui::vec2(18.0, 0.0)))
        .on_hover_text(tr.task_wizard_catalogue_tip);
    if more.clicked() {
        action = Some(arrow_action(on));
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

/// What a click on the Task Wizard's ▾ asks for: the catalogue — and, while the wizard is
/// off, the wizard first, since turning it on is what opens the catalogue (R3). The ▾ was
/// dead while off, so it looked broken (operator, 2026-10-17: "if the switch is not
/// selected the arrow does not work; it should select the switch and open the options").
fn arrow_action(on: bool) -> ToolbarAction {
    if on {
        ToolbarAction::OpenTaskCatalogue
    } else {
        ToolbarAction::ToggleTaskWizard
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panels::task_wizard::{catalogue, TASKS};
    use egui::{Event, PointerButton, Pos2, Rect};

    /// One frame of the IDE as far as the switch goes: the toolbar's right-to-left group,
    /// the app acting on what it reported, then the catalogue at the end of the frame —
    /// the order `app.rs` runs them in. Answers whether the catalogue painted its title.
    fn frame(ctx: &Context, aurora: bool, on: &mut bool, events: Vec<Event>, t: &mut f64) -> bool {
        let tr = Language::English.tr();
        *t += 0.1;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280.0, 800.0))),
            time: Some(*t),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |root| {
            let mut action = None;
            egui::Panel::top("toolbar").show_inside(root, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    action = task_wizard_toggle(ui, &tr, *on, aurora);
                });
            });
            match action {
                Some(ToolbarAction::ToggleTaskWizard) => {
                    *on = !*on;
                    if *on {
                        catalogue::open(root.ctx());
                    }
                }
                Some(ToolbarAction::OpenTaskCatalogue) => catalogue::open(root.ctx()),
                _ => {}
            }
            if *on {
                if let Some(anchor) = catalogue::toggle_rect(root.ctx()) {
                    let _ = catalogue::show(root.ctx(), anchor, TASKS, &|_| catalogue::TaskState::New, true, &tr);
                }
            }
        });
        out.textures_delta.clear();
        fn walk(s: &egui::Shape, title: &str) -> bool {
            match s {
                egui::Shape::Text(t) => t.galley.text() == title,
                egui::Shape::Vec(v) => v.iter().any(|s| walk(s, title)),
                _ => false,
            }
        }
        out.shapes.iter().any(|c| walk(&c.shape, tr.task_wizard_catalogue_title))
    }

    fn click(at: Pos2) -> [Vec<Event>; 2] {
        let press = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        [vec![Event::PointerMoved(at), press(true)], vec![press(false)]]
    }

    /// Operator, 2026-10-17: "the Task Wizard button has an arrow to open the options, but
    /// it does not work — you have to click the button a few times to show the options",
    /// then: "if the switch is selected the arrow works; if it is not, the arrow does not
    /// work, but then it should select the switch and open the options". One click on the
    /// ▾ opens the catalogue and it stays open — with the wizard on, and with it off, which
    /// the click turns on.
    #[test]
    fn one_click_on_the_arrow_opens_the_catalogue() {
        for (start_on, aurora) in [(true, false), (true, true), (false, false), (false, true)] {
            for inset in [3.0, 9.0, 15.0] {
                let ctx = Context::default();
                let (mut on, mut t) = (start_on, 0.0);
                for _ in 0..3 {
                    frame(&ctx, aurora, &mut on, vec![], &mut t);
                }
                let both = catalogue::toggle_rect(&ctx).expect("the toolbar remembered the switch");
                // Right to left: the ▾ is the right end of the pair.
                let arrow = Pos2::new(both.right() - inset, both.center().y);
                let mut shown = Vec::new();
                for events in click(arrow) {
                    shown.push(frame(&ctx, aurora, &mut on, events, &mut t));
                }
                for _ in 0..4 {
                    shown.push(frame(&ctx, aurora, &mut on, vec![], &mut t));
                }
                println!("started on {start_on}, aurora {aurora}, {inset}px in from {both:?}: on {on}, catalogue per frame {shown:?}");
                assert!(on, "started on {start_on}, aurora {aurora}, {inset}px: the ▾ leaves the wizard on");
                assert!(
                    shown[shown.len() - 4..].iter().all(|s| *s),
                    "started on {start_on}, aurora {aurora}, {inset}px: one click on ▾ opens the catalogue and it stays open: {shown:?}"
                );
            }
        }
    }

    /// While a task runs: the IDE dimmed, a step's balloon up, and the switch and its ▾
    /// always a hole in the dim (R16). One click on the ▾ still opens the catalogue.
    #[test]
    fn one_click_on_the_arrow_opens_the_catalogue_while_a_task_runs() {
        use crate::panels::task_wizard::overlay::Overlay;
        use crate::panels::task_wizard::{anchors, input, AnchorId, Card, KeyPolicy, Plan, StepCard};
        let tr = Language::English.tr();
        let palette = crate::panels::spotlight::balloon_palette(&crate::theme::THEMES[0]);
        let plan = Plan {
            serial: 1,
            viewport: egui::ViewportId::ROOT,
            ring: Some(AnchorId::ProjectRoot),
            live: None,
            dim_only: false,
            keys: KeyPolicy::LiveRegion,
            card: Card::Step(StepCard {
                task: "Task",
                title: "Step",
                body: "Do the thing.",
                n: 1,
                m: 2,
                already: false,
                did: None,
                back: false,
                do_for_me: false,
                reopen: false,
                last: false,
            }),
        };
        let mut overlay = Overlay::default();
        let ctx = Context::default();
        let (mut on, mut t) = (true, 0.0);
        let mut run = |events: Vec<Event>, t: &mut f64, on: &mut bool| -> bool {
            *t += 0.1;
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280.0, 800.0))),
                time: Some(*t),
                events,
                ..Default::default()
            };
            anchors::install(None);
            let mut out = ctx.run_ui(input, |root| {
                let vp = egui::ViewportId::ROOT;
                let _ = input::filter(root.ctx(), Some(&plan), overlay.live_rect(vp), overlay.field_focus(vp), false);
                let mut action = None;
                egui::Panel::top("toolbar").show_inside(root, |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        action = task_wizard_toggle(ui, &tr, *on, false);
                    });
                });
                egui::CentralPanel::default().show_inside(root, |ui| {
                    let r = Rect::from_min_size(Pos2::new(40.0, 200.0), egui::vec2(200.0, 24.0));
                    ui.put(r, egui::Label::new("Project"));
                    anchors::mark(ui, AnchorId::ProjectRoot, r);
                });
                match action {
                    Some(ToolbarAction::ToggleTaskWizard) => *on = !*on,
                    Some(ToolbarAction::OpenTaskCatalogue) => catalogue::open(root.ctx()),
                    _ => {}
                }
                let marks = anchors::snapshot();
                let _ = overlay.paint(root.ctx(), Some(&plan), &marks, &tr, &palette, &|_| String::new());
                if let Some(anchor) = catalogue::toggle_rect(root.ctx()) {
                    let _ = catalogue::show(root.ctx(), anchor, TASKS, &|_| catalogue::TaskState::New, true, &tr);
                }
            });
            let _ = anchors::take();
            out.textures_delta.clear();
            fn walk(s: &egui::Shape, title: &str) -> bool {
                match s {
                    egui::Shape::Text(t) => t.galley.text() == title,
                    egui::Shape::Vec(v) => v.iter().any(|s| walk(s, title)),
                    _ => false,
                }
            }
            out.shapes.iter().any(|c| walk(&c.shape, tr.task_wizard_catalogue_title))
        };
        for _ in 0..4 {
            run(vec![], &mut t, &mut on);
        }
        let both = catalogue::toggle_rect(&ctx).expect("the toolbar remembered the switch");
        let arrow = Pos2::new(both.right() - 9.0, both.center().y);
        let mut shown = Vec::new();
        for events in click(arrow) {
            shown.push(run(events, &mut t, &mut on));
        }
        for _ in 0..4 {
            shown.push(run(vec![], &mut t, &mut on));
        }
        println!("a task running: on {on}, catalogue per frame {shown:?}");
        assert!(on);
        assert!(shown[shown.len() - 4..].iter().all(|s| *s), "one click on ▾ opens it while a task runs: {shown:?}");
    }

    /// A person's click: the pointer rests on the ▾ until its tooltip shows, then a quick
    /// click whose press and release land in one frame.
    #[test]
    fn a_resting_quick_click_on_the_arrow_opens_the_catalogue() {
        for aurora in [false, true] {
            let ctx = Context::default();
            let (mut on, mut t) = (true, 0.0);
            for _ in 0..3 {
                frame(&ctx, aurora, &mut on, vec![], &mut t);
            }
            let both = catalogue::toggle_rect(&ctx).expect("the toolbar remembered the switch");
            let arrow = Pos2::new(both.right() - 9.0, both.center().y);
            frame(&ctx, aurora, &mut on, vec![Event::PointerMoved(arrow)], &mut t);
            for _ in 0..12 {
                frame(&ctx, aurora, &mut on, vec![], &mut t);
            }
            let [press, release] = click(arrow);
            let mut shown = vec![frame(&ctx, aurora, &mut on, press.into_iter().chain(release).collect(), &mut t)];
            for _ in 0..4 {
                shown.push(frame(&ctx, aurora, &mut on, vec![], &mut t));
            }
            println!("aurora {aurora}, resting quick click: on {on}, catalogue per frame {shown:?}");
            assert!(on && shown[shown.len() - 4..].iter().all(|s| *s), "aurora {aurora}: {shown:?}");
        }
    }

    /// The Form Designer's ▾ — the one the operator reported, dead while the wizard was off.
    /// A designer is its own OS window: an immediate viewport drawn inside the IDE window's
    /// pass, with its own input. So it is drawn here the way eframe draws it, with a renderer
    /// like eframe's, laid out as `show_designer_window` lays it out, and the click arrives
    /// at the designer. On or off, one click opens that window's catalogue.
    #[test]
    fn one_click_on_the_arrow_in_a_designer_window_opens_its_catalogue() {
        use crate::panels::designer::{draw_icon_toolbar, DesignerToolbarAction as D};
        use std::cell::{Cell, RefCell};
        use std::rc::Rc;
        let designer_vp = egui::ViewportId::from_hash_of("designer");
        let inbox: Rc<RefCell<Vec<Event>>> = Rc::default();
        let clock = Rc::new(Cell::new(0.0_f64));
        let painted = Rc::new(Cell::new(false));
        let title = Language::English.tr().task_wizard_catalogue_title;
        {
            let (inbox, clock, painted) = (inbox.clone(), clock.clone(), painted.clone());
            Context::set_immediate_viewport_renderer(move |ctx, mut vp| {
                let mut input = egui::RawInput {
                    viewport_id: vp.ids.this,
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1400.0, 800.0))),
                    time: Some(clock.get()),
                    events: inbox.borrow_mut().drain(..).collect(),
                    ..Default::default()
                };
                input.viewports.insert(vp.ids.this, Default::default());
                let mut out = ctx.run_ui(input, |ui| (vp.viewport_ui_cb)(ui));
                out.textures_delta.clear();
                fn walk(s: &egui::Shape, title: &str) -> bool {
                    match s {
                        egui::Shape::Text(t) => t.galley.text() == title,
                        egui::Shape::Vec(v) => v.iter().any(|s| walk(s, title)),
                        _ => false,
                    }
                }
                painted.set(out.shapes.iter().any(|c| walk(&c.shape, title)));
            });
        }
        let tr = Language::English.tr();
        for start_on in [true, false] {
            let ctx = Context::default();
            ctx.set_embed_viewports(false);
            let on = Cell::new(start_on);
            let toggle = Cell::new(None::<Rect>);
            let run = |events: Vec<Event>| -> bool {
                clock.set(clock.get() + 0.1);
                *inbox.borrow_mut() = events;
                let input = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280.0, 800.0))),
                    time: Some(clock.get()),
                    ..Default::default()
                };
                let mut out = ctx.run_ui(input, |root| {
                    // The IDE window: its own switch, and its catalogue at the end of its pass.
                    egui::Panel::top("toolbar").show_inside(root, |ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let _ = task_wizard_toggle(ui, &tr, on.get(), false);
                        });
                    });
                    root.ctx().show_viewport_immediate(designer_vp, egui::ViewportBuilder::default(), |vp_ui, _| {
                        let mut action = D::None;
                        // A 50-point frameless panel, one centred row, the language selector at
                        // its right end; then the toolbox and the canvas.
                        egui::Panel::top("dtb_0").exact_size(50.0).frame(egui::Frame::NONE).show_separator_line(false).show_inside(vp_ui, |ui| {
                            ui.horizontal_centered(|ui| {
                                action = draw_icon_toolbar(
                                    ui, true, true, true, false, true, "Cut", "Copy", "Paste", "Duplicate", "Format painter", "Tab order", "Tab list",
                                    false, false, true, true, false, false, false, false, false, false, false, "Magnifier", true, "Show Grid",
                                    on.get(), "Task Wizard", "Choose a task",
                                );
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    egui::ComboBox::from_id_salt("designer_lang_selector")
                                        .selected_text(Language::English.native_name())
                                        .width(130.0)
                                        .show_ui(ui, |ui| {
                                            for &l in Language::ALL {
                                                let _ = crate::flags::language_row(ui, l, l == Language::English);
                                            }
                                        });
                                    crate::flags::flag_widget(ui, Language::English);
                                });
                            });
                        });
                        egui::Panel::left("dl_0").show_inside(vp_ui, |ui| {
                            ui.label("Toolbox");
                        });
                        egui::CentralPanel::default().show_inside(vp_ui, |ui| {
                            ui.label("Canvas");
                        });
                        // As the app dispatches them: `toggle_task_wizard` opens the catalogue
                        // in the window whose switch turned it on.
                        match action {
                            D::ToggleTaskWizard => {
                                on.set(!on.get());
                                if on.get() {
                                    catalogue::open(vp_ui.ctx());
                                }
                            }
                            D::OpenTaskCatalogue => catalogue::open(vp_ui.ctx()),
                            _ => {}
                        }
                        toggle.set(catalogue::toggle_rect(vp_ui.ctx()));
                        // `task_wizard_designer_end`: this window's catalogue, while on.
                        if on.get() {
                            if let Some(anchor) = catalogue::toggle_rect(vp_ui.ctx()) {
                                let _ = catalogue::show(vp_ui.ctx(), anchor, TASKS, &|_| catalogue::TaskState::New, true, &tr);
                            }
                        }
                    });
                    // `task_wizard_end_frame`: the IDE window's catalogue, while on.
                    if on.get() {
                        if let Some(anchor) = catalogue::toggle_rect(root.ctx()) {
                            let _ = catalogue::show(root.ctx(), anchor, TASKS, &|_| catalogue::TaskState::New, true, &tr);
                        }
                    }
                });
                out.textures_delta.clear();
                painted.get()
            };
            for _ in 0..3 {
                run(vec![]);
            }
            let both = toggle.get().expect("the designer remembered its switch");
            let arrow = Pos2::new(both.right() - 7.0, both.center().y);
            let mut shown = Vec::new();
            for events in click(arrow) {
                shown.push(run(events));
            }
            for _ in 0..4 {
                shown.push(run(vec![]));
            }
            println!("designer window, started on {start_on}: on {}, its catalogue per frame {shown:?}", on.get());
            assert!(on.get(), "started on {start_on}: the ▾ leaves the wizard on");
            assert!(
                shown[shown.len() - 4..].iter().all(|s| *s),
                "started on {start_on}: one click on the designer's ▾ opens its catalogue: {shown:?}"
            );
        }
    }

    /// The same once the catalogue has been used: turned on, it opens; a click elsewhere
    /// closes it; then the ▾ must bring it back with one click.
    #[test]
    fn the_arrow_reopens_the_catalogue_after_it_closed() {
        for aurora in [false, true] {
            let ctx = Context::default();
            let (mut on, mut t) = (false, 0.0);
            for _ in 0..3 {
                frame(&ctx, aurora, &mut on, vec![], &mut t);
            }
            let both = catalogue::toggle_rect(&ctx).expect("the toolbar remembered the switch");
            let mut log = Vec::new();
            // The switch, at its left end.
            for events in click(Pos2::new(both.left() + 12.0, both.center().y)) {
                log.push(("switch", frame(&ctx, aurora, &mut on, events, &mut t)));
            }
            log.push(("idle", frame(&ctx, aurora, &mut on, vec![], &mut t)));
            assert!(on && log.last().unwrap().1, "aurora {aurora}: turned on, the catalogue opens: {log:?}");
            // Elsewhere.
            for events in click(Pos2::new(200.0, 600.0)) {
                log.push(("elsewhere", frame(&ctx, aurora, &mut on, events, &mut t)));
            }
            log.push(("idle", frame(&ctx, aurora, &mut on, vec![], &mut t)));
            assert!(!log.last().unwrap().1, "aurora {aurora}: a click elsewhere closes it: {log:?}");
            for events in click(Pos2::new(both.right() - 9.0, both.center().y)) {
                log.push(("arrow", frame(&ctx, aurora, &mut on, events, &mut t)));
            }
            for _ in 0..3 {
                log.push(("idle", frame(&ctx, aurora, &mut on, vec![], &mut t)));
            }
            println!("aurora {aurora}: {log:?}");
            assert!(on && log.last().unwrap().1, "aurora {aurora}: one click on ▾ reopens it: {log:?}");
        }
    }
}
