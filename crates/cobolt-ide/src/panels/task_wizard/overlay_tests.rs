// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The overlay and the keyboard policy, rendered (spec 104 tasks.md T12, T13): what reaches
//! the IDE through a hole, what a blocker stops, and what `Esc` does.

use std::cell::{Cell, RefCell};

use eframe::egui::{
    self, pos2, vec2, Button, Event, Id, Key, Modifiers, Order, Pos2, RawInput, Rect, Ui,
    ViewportId,
};

use super::anchors::{self, AnchorId as A};
use super::input;
use super::overlay::{Overlay, Probe, PROBE_ID};
use super::{Card, KeyPolicy, Nav, Plan, StepCard};
use crate::i18n::{Language, Tr};
use crate::panels::spotlight::{balloon_palette, Palette};

const SCREEN: Rect = Rect {
    min: Pos2 { x: 0.0, y: 0.0 },
    max: Pos2 { x: 1280.0, y: 800.0 },
};

struct Rig {
    ctx: egui::Context,
    overlay: Overlay,
    tr: Tr,
    palette: Palette,
    nav: Option<Nav>,
    /// Frames run; time advances with them, or egui's smoothed wheel never arrives.
    frames: u32,
}

fn plan(ring: A, live: Option<A>, keys: KeyPolicy) -> Plan {
    Plan {
        serial: 1,
        viewport: ViewportId::ROOT,
        ring: Some(ring),
        live,
        dim_only: false,
        keys,
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
    }
}

impl Rig {
    fn new() -> Self {
        Rig {
            ctx: egui::Context::default(),
            overlay: Overlay::default(),
            tr: Language::English.tr(),
            palette: balloon_palette(&crate::theme::THEMES[0]),
            nav: None,
            frames: 0,
        }
    }

    fn frame(&mut self, events: Vec<Event>, plan: Option<&Plan>, body: &mut dyn FnMut(&mut Ui)) {
        let mut input = RawInput::default();
        input.screen_rect = Some(SCREEN);
        input.max_texture_side = Some(8192);
        input.events = events;
        self.frames += 1;
        input.time = Some(f64::from(self.frames) * 0.1);
        anchors::install(None);
        let overlay = &mut self.overlay;
        let tr = &self.tr;
        let p = &self.palette;
        let nav = &mut self.nav;
        let mut out = self.ctx.run_ui(input, |ui| {
            let vp = ViewportId::ROOT;
            if let Some(n) = input::filter(ui.ctx(), plan, overlay.live_rect(vp), overlay.field_focus(vp), false) {
                *nav = Some(n);
            }
            egui::CentralPanel::default().show(ui, |ui| body(ui));
            let marks = anchors::snapshot();
            if let Some(n) = overlay.paint(ui.ctx(), plan, &marks, tr, p, &|_| "X".into()) {
                *nav = Some(n);
            }
        });
        out.textures_delta.clear();
        let _ = anchors::take();
    }

    fn settle(&mut self, plan: Option<&Plan>, body: &mut dyn FnMut(&mut Ui)) {
        for _ in 0..4 {
            self.frame(vec![], plan, body);
        }
    }

    fn click(&mut self, at: Pos2, plan: Option<&Plan>, body: &mut dyn FnMut(&mut Ui)) {
        self.frame(vec![Event::PointerMoved(at)], plan, body);
        self.frame(
            vec![Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
            plan,
            body,
        );
        self.frame(
            vec![Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
            plan,
            body,
        );
        self.settle(plan, body);
    }

    fn key(&mut self, key: Key, plan: Option<&Plan>, body: &mut dyn FnMut(&mut Ui)) {
        for pressed in [true, false] {
            self.frame(
                vec![Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
                plan,
                body,
            );
        }
    }

    fn probe(&self) -> Probe {
        self.ctx
            .data(|d| d.get_temp::<Probe>(Id::new(PROBE_ID)))
            .expect("the overlay painted")
    }
}

const LIT: Rect = Rect {
    min: Pos2 { x: 100.0, y: 100.0 },
    max: Pos2 { x: 200.0, y: 130.0 },
};
const DARK: Rect = Rect {
    min: Pos2 { x: 700.0, y: 600.0 },
    max: Pos2 { x: 800.0, y: 630.0 },
};

#[test]
fn a_click_in_the_hole_reaches_the_ide_and_one_on_the_dim_does_not() {
    let lit = Cell::new(0);
    let dark = Cell::new(0);
    let mut body = |ui: &mut Ui| {
        if ui.put(LIT, Button::new("lit")).clicked() {
            lit.set(lit.get() + 1);
        }
        anchors::mark(ui, A::ModelsSave, LIT);
        if ui.put(DARK, Button::new("dark")).clicked() {
            dark.set(dark.get() + 1);
        }
    };
    let p = plan(A::ModelsSave, None, KeyPolicy::LiveRegion);
    let mut rig = Rig::new();
    rig.settle(Some(&p), &mut body);
    assert_eq!(rig.probe().ring, Some(LIT), "the ring is the painted anchor (AC6)");
    assert!(
        rig.probe().blockers.iter().any(|b| b.contains(DARK.center())),
        "the other button is under the dim"
    );
    rig.click(LIT.center(), Some(&p), &mut body);
    assert_eq!(lit.get(), 1, "the lit widget is live (R13)");
    rig.click(DARK.center(), Some(&p), &mut body);
    assert_eq!(dark.get(), 0, "a stray click must not wander off the task (R13)");

    // The same click with the wizard off reaches it: the blocker was the only obstacle.
    rig.click(DARK.center(), None, &mut body);
    assert_eq!(dark.get(), 1);
}

#[test]
fn the_toggle_is_always_a_hole() {
    let toggled = Cell::new(0);
    let mut body = |ui: &mut Ui| {
        ui.put(LIT, Button::new("lit"));
        anchors::mark(ui, A::ModelsSave, LIT);
        if ui.put(DARK, Button::new("toggle")).clicked() {
            toggled.set(toggled.get() + 1);
        }
        anchors::mark(ui, A::Toggle, DARK);
    };
    let p = plan(A::ModelsSave, None, KeyPolicy::LiveRegion);
    let mut rig = Rig::new();
    rig.settle(Some(&p), &mut body);
    rig.click(DARK.center(), Some(&p), &mut body);
    assert_eq!(toggled.get(), 1, "the wizard can always be turned off (R16)");
}

#[test]
fn the_wheel_scrolls_the_lit_pane_and_nothing_under_the_dim() {
    let lit_pane = Rect::from_min_size(pos2(100.0, 100.0), vec2(300.0, 200.0));
    let dark_pane = Rect::from_min_size(pos2(700.0, 400.0), vec2(300.0, 200.0));
    let offsets = RefCell::new([0.0f32; 2]);
    let mut body = |ui: &mut Ui| {
        for (i, r) in [lit_pane, dark_pane].into_iter().enumerate() {
            ui.scope_builder(egui::UiBuilder::new().max_rect(r), |ui| {
                let out = egui::ScrollArea::vertical()
                    .id_salt(("pane", i))
                    // Fill the pane: a vertical list otherwise shrinks to its text's width.
                    .auto_shrink([false, false])
                    .animated(false)
                    .show(ui, |ui| {
                        for n in 0..80 {
                            ui.label(format!("row {n}"));
                        }
                    });
                offsets.borrow_mut()[i] = out.state.offset.y;
            });
        }
        anchors::mark(ui, A::PropertiesPane, lit_pane);
    };
    let p = plan(A::PropertiesPane, None, KeyPolicy::LiveRegion);
    let wheel = |rig: &mut Rig, at: Pos2, plan: Option<&Plan>, body: &mut dyn FnMut(&mut Ui)| {
        rig.frame(vec![Event::PointerMoved(at)], plan, body);
        for _ in 0..3 {
            rig.frame(
                vec![Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -120.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: Modifiers::NONE,
                }],
                plan,
                body,
            );
        }
        rig.settle(plan, body);
    };
    // The harness can scroll: with the wizard off, the wheel moves the pane it is over.
    let mut off = Rig::new();
    off.settle(None, &mut body);
    wheel(&mut off, dark_pane.center(), None, &mut body);
    assert!(offsets.borrow()[1] > 0.0, "the wheel scrolls a pane with the wizard off");
    *offsets.borrow_mut() = [0.0; 2];

    let mut rig = Rig::new();
    rig.settle(Some(&p), &mut body);
    wheel(&mut rig, dark_pane.center(), Some(&p), &mut body);
    assert_eq!(offsets.borrow()[1], 0.0, "the wheel over the dim scrolls nothing beneath");
    wheel(&mut rig, lit_pane.center(), Some(&p), &mut body);
    assert!(offsets.borrow()[0] > 0.0, "the lit pane scrolls (R13)");
}

#[test]
fn a_list_opened_from_the_lit_region_is_live_over_the_dim() {
    let combo = Rect::from_min_size(pos2(100.0, 100.0), vec2(160.0, 24.0));
    let picked = RefCell::new(String::from("a"));
    let mut body = |ui: &mut Ui| {
        ui.scope_builder(egui::UiBuilder::new().max_rect(combo), |ui| {
            let shown = picked.borrow().clone();
            let r = egui::ComboBox::from_id_salt("pick")
                .selected_text(shown)
                .width(150.0)
                .show_ui(ui, |ui| {
                    for v in ["a", "b", "c", "d", "e", "f"] {
                        let mut cur = picked.borrow_mut();
                        ui.selectable_value(&mut *cur, v.to_owned(), v);
                    }
                });
            anchors::mark(ui, A::AgentsScope, r.response.rect);
        });
    };
    let p = plan(A::AgentsScope, None, KeyPolicy::LiveRegion);
    let mut rig = Rig::new();
    rig.settle(Some(&p), &mut body);
    let ring = rig.probe().ring.expect("ringed");
    rig.click(ring.center(), Some(&p), &mut body);
    // The list hangs below the combo, over the dim; pick its last row.
    let list = rig.ctx.memory(|m| {
        m.areas()
            .visible_layer_ids()
            .into_iter()
            .filter(|l| l.order == Order::Foreground && !format!("{:?}", l.id).is_empty())
            .filter_map(|l| m.area_rect(l.id).map(|r| (l, r)))
            .filter(|(_, r)| r.min.y >= ring.max.y - 2.0 && r.height() > 60.0)
            .map(|(_, r)| r)
            .next()
    });
    let list = list.expect("the list opened");
    let at = pos2(list.center().x, list.max.y - 12.0);
    assert!(
        rig.probe().blockers.iter().any(|b| b.contains(at)),
        "the row clicked lies over the dim"
    );
    rig.click(at, Some(&p), &mut body);
    assert_eq!(picked.borrow().as_str(), "f", "a popup from the lit region is live (R14)");
}

#[test]
fn keys_reach_a_field_in_the_hole_and_nowhere_else() {
    let lit_text = RefCell::new(String::new());
    let dark_text = RefCell::new(String::new());
    let dark_id = Id::new("dark-field");
    let focus_dark = Cell::new(false);
    let mut body = |ui: &mut Ui| {
        let r = ui.put(LIT, egui::TextEdit::singleline(&mut *lit_text.borrow_mut()));
        anchors::mark(ui, A::ModelsEndpoint, r.rect);
        ui.put(DARK, egui::TextEdit::singleline(&mut *dark_text.borrow_mut()).id(dark_id));
        if focus_dark.replace(false) {
            ui.memory_mut(|m| m.request_focus(dark_id));
        }
    };
    let p = plan(A::ModelsEndpoint, None, KeyPolicy::LiveRegion);
    let mut rig = Rig::new();
    rig.settle(Some(&p), &mut body);
    rig.click(LIT.center(), Some(&p), &mut body);
    rig.frame(vec![Event::Text("hi".into())], Some(&p), &mut body);
    assert_eq!(lit_text.borrow().as_str(), "hi", "typing reaches the lit field (R13)");

    // Focus moved outside by code (a shortcut, a stale request): the keys stop, and the
    // focus is let go.
    focus_dark.set(true);
    rig.frame(vec![], Some(&p), &mut body);
    rig.frame(vec![Event::Text("no".into())], Some(&p), &mut body);
    rig.frame(vec![Event::Text("no".into())], Some(&p), &mut body);
    assert_eq!(dark_text.borrow().as_str(), "", "no key reaches a field under the dim (R23)");
    assert!(
        !rig.ctx.memory(|m| m.has_focus(dark_id)),
        "focus outside the live region is surrendered"
    );
}

#[test]
fn escape_leaves_a_field_then_closes_a_popup_then_exits() {
    let text = RefCell::new(String::new());
    let combo = Rect::from_min_size(pos2(100.0, 160.0), vec2(160.0, 24.0));
    let mut pick = String::from("a");
    // Whether the IDE beneath saw the Esc: a field's own Esc (its cancel) must reach it.
    let saw_esc = Cell::new(false);
    let mut body = |ui: &mut Ui| {
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            saw_esc.set(true);
        }
        let live = Rect::from_min_max(LIT.min, combo.max);
        let r = ui.put(LIT, egui::TextEdit::singleline(&mut *text.borrow_mut()));
        ui.scope_builder(egui::UiBuilder::new().max_rect(combo), |ui| {
            egui::ComboBox::from_id_salt("esc-pick")
                .selected_text(pick.clone())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut pick, "b".to_owned(), "b");
                });
        });
        anchors::mark(ui, A::ModelsWindow, live);
        anchors::mark(ui, A::ModelsEndpoint, r.rect);
    };
    let p = plan(A::ModelsEndpoint, Some(A::ModelsWindow), KeyPolicy::LiveRegion);
    let mut rig = Rig::new();
    rig.settle(Some(&p), &mut body);
    rig.click(LIT.center(), Some(&p), &mut body);
    assert!(rig.ctx.memory(|m| m.focused().is_some()));

    rig.key(Key::Escape, Some(&p), &mut body);
    assert!(rig.ctx.memory(|m| m.focused().is_none()), "stage one leaves the field");
    assert_eq!(rig.nav, None, "…and does nothing else (R22)");
    assert!(saw_esc.replace(false), "the field's Esc reaches the IDE, so a cell editor can cancel");

    rig.click(combo.center(), Some(&p), &mut body);
    assert!(egui::Popup::is_any_open(&rig.ctx), "the list is open");
    rig.key(Key::Escape, Some(&p), &mut body);
    assert!(!egui::Popup::is_any_open(&rig.ctx), "stage two closes the popup");
    assert_eq!(rig.nav, None, "…and does nothing else (R22)");
    assert!(!saw_esc.replace(false), "the wizard kept that Esc");

    rig.key(Key::Escape, Some(&p), &mut body);
    assert_eq!(rig.nav, Some(Nav::Exit), "with nothing else to close, Esc is Exit");
    assert!(!saw_esc.get(), "…and kept that one too");
}

#[test]
fn with_the_wizard_off_nothing_is_drawn_and_no_layer_exists() {
    let mut rig = Rig::new();
    let mut body = |ui: &mut Ui| {
        ui.put(LIT, Button::new("lit"));
    };
    rig.settle(None, &mut body);
    let ours = rig.ctx.memory(|m| {
        m.areas()
            .visible_layer_ids()
            .into_iter()
            .filter(|l| {
                (0..crate::panels::spotlight::MAX_BLOCKERS)
                    .any(|i| l.id == Id::new(("task_wizard_block", i)))
                    || l.id == Id::new("task_wizard_balloon")
            })
            .count()
    });
    assert_eq!(ours, 0, "off, the wizard creates no layer (R35)");
}

#[test]
fn the_balloon_keeps_off_the_live_region_on_a_crowded_screen() {
    let pane = Rect::from_min_size(pos2(40.0, 80.0), vec2(700.0, 600.0));
    let mut body = |ui: &mut Ui| {
        anchors::mark(ui, A::ModelsWindow, pane);
        anchors::mark(ui, A::ModelsEndpoint, Rect::from_min_size(pos2(300.0, 300.0), vec2(200.0, 24.0)));
    };
    let p = plan(A::ModelsEndpoint, Some(A::ModelsWindow), KeyPolicy::LiveRegion);
    let mut rig = Rig::new();
    rig.settle(Some(&p), &mut body);
    let probe = rig.probe();
    let balloon = probe.balloon.expect("a balloon");
    assert!(!balloon.intersects(pane.shrink(1.0)), "the balloon never covers the window being worked in");
    assert!(SCREEN.contains_rect(balloon));
}

/// Review R2 — a window the lit window opens (the binding editor's Dropdown configuration)
/// stays above it: the lit window is brought forward once per step, not every frame, or
/// every child it opens would sink under it a frame later.
#[test]
fn a_window_opened_from_the_lit_window_stays_above_it() {
    let child_open = Cell::new(false);
    let mut body = |ui: &mut Ui| {
        let ctx = ui.ctx().clone();
        let parent = egui::Window::new("parent")
            .id(Id::new("lit-parent"))
            .fixed_pos(pos2(100.0, 100.0))
            .fixed_size(vec2(400.0, 300.0))
            .show(&ctx, |ui| {
                let r = ui.button("field");
                anchors::mark(ui, A::BindingApply, r.rect);
            });
        if let Some(p) = &parent {
            anchors::mark_window(&ctx, A::BindingWindow, &p.response);
        }
        if child_open.get() {
            egui::Window::new("child")
                .id(Id::new("its-child"))
                .fixed_pos(pos2(200.0, 150.0))
                .fixed_size(vec2(200.0, 150.0))
                .show(&ctx, |ui| {
                    ui.label("dropdown items");
                });
        }
    };
    let p = plan(A::BindingApply, Some(A::BindingWindow), KeyPolicy::LiveRegion);
    let mut rig = Rig::new();
    rig.settle(Some(&p), &mut body);
    child_open.set(true);
    rig.settle(Some(&p), &mut body);
    rig.settle(Some(&p), &mut body);
    let order: Vec<Id> = rig.ctx.memory(|m| m.layer_ids().map(|l| l.id).collect());
    let at = |id: &str| order.iter().position(|l| *l == Id::new(id)).unwrap_or_else(|| panic!("{id} not shown"));
    assert!(
        at("its-child") > at("lit-parent"),
        "the child opened by the lit window must stay on top of it"
    );
}
