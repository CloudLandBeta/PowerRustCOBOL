// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 091 — the designer's tab bar, the `Non-Visuals` tab and the layer
//! operations, driven through real frames of the real `DesignerPanel::show`.
//!
//! A child module of `designer`, so it reads the designer's private state the way
//! the other test modules there do. Each sub-module is one slice of the task list
//! (`bar` T11, `gating` T12, …) and is named so `layers_091::<name>` selects it.

use super::*;
use crate::llm::LlmConfig;
use crate::panels::layer_tabs::{layer_tab_parts, layout_for, placed_rect, ActiveTab, Slot, TabAction, BAR_H};
use egui::{Event, PointerButton};

const SCREEN: Vec2 = Vec2::new(1400.0, 1000.0);

fn press(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }
}

/// A form of 1200 × 800 with `n` layers, `Layer-1` … `Layer-n`.
fn form_with_layers(n: usize) -> Form {
    let mut f = Form::new("F", "F", 1200, 800);
    for _ in 0..n {
        f.add_layer().expect("a layer is added");
    }
    f
}

/// What the file would hold: used to prove a gesture changed nothing saved.
fn saved(d: &DesignerPanel) -> String {
    cobolt_forms::form_to_string(&d.form).expect("the form serialises")
}

/// A designer in a window of its own, one real frame at a time.
struct Rig {
    ctx: egui::Context,
    llm: LlmConfig,
    d: DesignerPanel,
    /// The size the designer's panel used last frame — what "the window did not
    /// grow" is measured against.
    used: Vec2,
    /// The modifier keys held down: egui reads them from a `ModifiersChanged`
    /// event, not from the events that carry them.
    mods: egui::Modifiers,
}

impl Rig {
    fn new(form: Form) -> Self {
        Self {
            ctx: egui::Context::default(),
            llm: LlmConfig::load_defaults_for_test(),
            d: DesignerPanel::new(form),
            used: Vec2::ZERO,
            mods: egui::Modifiers::NONE,
        }
    }

    fn frame(&mut self, events: Vec<Event>) -> DesignerShowResult {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(Pos2::ZERO, SCREEN));
        input.max_texture_side = Some(8192);
        // egui learns the modifier keys from this event, frame by frame.
        input.events = std::iter::once(Event::ModifiersChanged(self.mods)).chain(events).collect();
        let mut result = DesignerShowResult::default();
        let mut used = Vec2::ZERO;
        let d = &mut self.d;
        let llm = &self.llm;
        let mut out = self.ctx.run_ui(input, |root| {
            egui::CentralPanel::default().show_inside(root, |ui| {
                result = d.show(ui, &mut None, &[], llm, None, None);
                used = ui.min_rect().size();
            });
        });
        out.textures_delta.clear();
        self.used = used;
        result
    }

    fn settle(&mut self, frames: usize) {
        for _ in 0..frames {
            self.frame(vec![]);
        }
    }

    /// A click: the pointer arrives, goes down, comes up, and a frame settles.
    fn click(&mut self, at: Pos2) {
        self.frame(vec![Event::PointerMoved(at)]);
        self.frame(vec![press(at, true)]);
        self.frame(vec![press(at, false)]);
        self.frame(vec![]);
    }

    /// Where a tab of the bar sits, from the layout the painter used.
    fn slot_rect(&self, slot: Slot) -> egui::Rect {
        let bar = self.d.tab_bar_rect.expect("the bar was drawn");
        let tr = crate::i18n::current_tr(&self.ctx);
        let layout = layout_for(&self.ctx, &self.d.form, &tr, bar.width(), 0.0);
        let p = layout.placed.iter().find(|p| p.slot == slot).expect("that tab is on the bar");
        placed_rect(bar, p)
    }
}

impl Rig {
    /// A drag with the primary button: arrive, press, move in two steps, release.
    pub(super) fn drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![press(from, true)]);
        self.frame(vec![Event::PointerMoved(from + (to - from) * 0.5)]);
        self.frame(vec![Event::PointerMoved(to)]);
        self.frame(vec![press(to, false)]);
        self.frame(vec![]);
    }

    pub(super) fn select_tab(&mut self, action: TabAction) {
        self.d.apply_tab_actions(vec![action]);
        self.settle(3);
    }

    /// The middle of a control on the screen, from its designed rectangle (the
    /// `Form` and layer tabs draw a control where its `X` and `Y` say).
    pub(super) fn centre_of(&self, id: &str) -> Pos2 {
        let o = self.d.canvas_origin.expect("the canvas was drawn");
        let r = self.d.form.find_control(id).expect("a control of that name").rect;
        o + Vec2::new(r.x as f32 + r.w as f32 * 0.5, r.y as f32 + r.h as f32 * 0.5)
    }

    /// The middle of a `Non-Visuals` card on the screen.
    pub(super) fn card_centre(&self, id: &str) -> Pos2 {
        let o = self.d.canvas_origin.expect("the canvas was drawn");
        let cell = self.d.non_visuals_view().into_iter().find(|c| c.id == id).expect("a card for it").rect;
        o + Vec2::new(cell.x as f32 + cell.w as f32 * 0.5, cell.y as f32 + cell.h as f32 * 0.5)
    }
}

/// The text an inspector pane draws for `ctrl` (or the form) on its first frame:
/// the widgets' labels from the accessibility tree, and the text painted straight
/// onto the canvas, as the tab strip's is.
fn pane_text(
    panel: &mut crate::panels::properties::PropertiesPanel,
    form: &Form,
    ctrl: Option<&Control>,
) -> Vec<String> {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let tr = crate::i18n::current_tr(&ctx);
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(520.0, 2400.0)));
    let mut out = ctx.run_ui(input, |root| {
        egui::CentralPanel::default().show_inside(root, |ui| {
            let _ = panel.show(ui, form, ctrl, &[], &tr);
        });
    });
    out.textures_delta.clear();
    let update = out.platform_output.accesskit_update.expect("the first frame reports every node");
    let mut text: Vec<String> = update
        .nodes
        .iter()
        .flat_map(|(_, n)| [n.label().map(str::to_owned), n.value().map(str::to_owned)])
        .flatten()
        .collect();
    fn painted(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => out.push(t.galley.text().to_owned()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| painted(s, out)),
            _ => {}
        }
    }
    for clipped in &out.shapes {
        painted(&clipped.shape, &mut text);
    }
    text
}

// ── T11: the bar, and what it does ───────────────────────────────────────────

mod bar {
    use super::*;

    #[test]
    fn a_form_with_no_layers_has_a_bar_32_px_high_with_non_visuals_form_and_the_plus() {
        let mut r = Rig::new(form_with_layers(0));
        r.settle(4);
        let bar = r.d.tab_bar_rect.expect("the bar is drawn on a form with no layers (R23)");
        assert_eq!(BAR_H, 32.0);
        assert_eq!(bar.height(), BAR_H, "the bar's height is fixed");
        assert!(bar.width() > 600.0, "it spans the canvas: {bar:?}");
        assert!(bar.min.y > 100.0 && bar.max.y < SCREEN.y - 10.0, "under the canvas and above the AI pane: {bar:?}");
        let tr = crate::i18n::current_tr(&r.ctx);
        let layout = layout_for(&r.ctx, &r.d.form, &tr, bar.width(), 0.0);
        let order: Vec<Slot> = layout.placed.iter().map(|p| p.slot).collect();
        assert_eq!(order, vec![Slot::NonVisuals, Slot::Form, Slot::Add]);
        assert!(!layout.overflow, "three tabs fit");
    }

    #[test]
    fn sixty_four_layers_change_neither_the_bar_nor_the_room_it_leaves() {
        let mut none = Rig::new(form_with_layers(0));
        none.settle(4);
        let mut many = Rig::new(form_with_layers(64));
        many.settle(4);
        let bar = many.d.tab_bar_rect.expect("drawn");
        let tr = crate::i18n::current_tr(&many.ctx);
        assert!(
            layout_for(&many.ctx, &many.d.form, &tr, bar.width(), 0.0).overflow,
            "64 tabs do not fit, so this case exercises the scroll arrows"
        );
        assert_eq!(many.d.tab_bar_rect, none.d.tab_bar_rect, "the bar is the same strip with 64 layers as with none");
        assert_eq!(many.used, none.used, "and the panel uses the same room");
        let settled = (many.d.tab_bar_rect, many.used);
        for frame in 0..8 {
            many.frame(vec![]);
            assert_eq!((many.d.tab_bar_rect, many.used), settled, "frame {frame}: nothing creeps");
        }
    }

    #[test]
    fn clicking_a_tab_selects_it_and_its_box_never_changes_the_tab() {
        let mut r = Rig::new(form_with_layers(2));
        r.settle(3);
        let before = saved(&r.d);

        let layer1 = layer_tab_parts(r.slot_rect(Slot::Layer(0)));
        r.click(layer1.label.center());
        assert_eq!(r.d.tabs.active(), &ActiveTab::Layer("Layer-1".into()));
        assert!(r.d.tabs.is_shown("Layer-1"), "selecting a layer shows it (R60)");

        // Layer-2's box: ticks it, and the active tab stays Layer-1 (R62).
        let box2 = layer_tab_parts(r.slot_rect(Slot::Layer(1))).check.center();
        r.click(box2);
        assert_eq!(r.d.tabs.active(), &ActiveTab::Layer("Layer-1".into()), "a box never changes the tab");
        assert!(r.d.tabs.is_shown("Layer-2") && r.d.tabs.is_shown("Layer-1"));
        r.click(box2);
        assert!(!r.d.tabs.is_shown("Layer-2"), "and unticks it");
        assert_eq!(r.d.tabs.active(), &ActiveTab::Layer("Layer-1".into()));

        // Its own box, while active: it is hidden, and it is still the active tab.
        r.click(layer1.check.center());
        assert!(!r.d.tabs.is_shown("Layer-1"));
        assert_eq!(r.d.tabs.active(), &ActiveTab::Layer("Layer-1".into()));

        let form_tab = r.slot_rect(Slot::Form).center();
        r.click(form_tab);
        assert_eq!(r.d.tabs.active(), &ActiveTab::Form);
        let nv_tab = r.slot_rect(Slot::NonVisuals).center();
        r.click(nv_tab);
        assert_eq!(r.d.tabs.active(), &ActiveTab::NonVisuals);

        assert!(!r.d.dirty, "choosing a tab and ticking a box are not edits");
        assert!(r.d.undo_stack.is_empty(), "…and not undo steps");
        assert_eq!(saved(&r.d), before, "…and change nothing the file holds");
    }

    /// AC29 (R60, R61), with the pure state applied through the designer.
    #[test]
    fn selecting_a_layer_turns_it_on_and_the_others_off_and_saves_nothing() {
        let mut d = DesignerPanel::new(form_with_layers(3));
        let before = saved(&d);
        d.tabs.set_shown("Layer-1", true);
        d.tabs.set_shown("Layer-3", true);

        d.apply_tab_actions(vec![TabAction::SelectLayer("Layer-2".into())]);
        let view = d.tabs.view();
        assert!(view.is_shown("Layer-2"), "the layer whose box was off is turned on");
        assert!(!view.is_shown("Layer-1") && !view.is_shown("Layer-3"), "the others are turned off");

        // The state the canvas answers from, not only the tab bar's own.
        let st = DesignerState { anim: &Default::default(), tabs: Some(&view) };
        use cobolt_forms::render::FormState;
        assert!(st.layer_visible("Layer-2") && !st.layer_visible("Layer-1") && !st.layer_visible("Layer-3"));

        d.apply_tab_actions(vec![TabAction::ToggleShown("Layer-3".into())]);
        assert!(d.tabs.is_shown("Layer-3") && d.tabs.is_shown("Layer-2"), "ticking a box afterwards sticks");
        assert_eq!(d.tabs.active(), &ActiveTab::Layer("Layer-2".into()), "and the tab stays Layer-2");

        d.apply_tab_actions(vec![TabAction::SelectLayer("Layer-1".into())]);
        assert!(d.tabs.is_shown("Layer-1") && !d.tabs.is_shown("Layer-2") && !d.tabs.is_shown("Layer-3"));

        d.apply_tab_actions(vec![TabAction::SelectForm]);
        assert!(d.tabs.is_shown("Layer-1"), "Form hides nothing");
        d.apply_tab_actions(vec![TabAction::SelectNonVisuals]);
        assert!(d.tabs.is_shown("Layer-1"), "Non-Visuals hides nothing");

        assert!(!d.dirty && d.undo_stack.is_empty(), "none of this is an edit");
        assert_eq!(saved(&d), before, "the file is the same whatever the designer was left showing");
    }

    /// AC30 (R62): a box never moves the tab, from any of the three kinds of tab.
    #[test]
    fn ticking_boxes_leaves_form_non_visuals_and_a_layer_where_they_were() {
        for (start, action) in [
            (ActiveTab::Form, TabAction::SelectForm),
            (ActiveTab::NonVisuals, TabAction::SelectNonVisuals),
            (ActiveTab::Layer("Layer-1".into()), TabAction::SelectLayer("Layer-1".into())),
        ] {
            let mut d = DesignerPanel::new(form_with_layers(2));
            d.apply_tab_actions(vec![action]);
            for layer in ["Layer-2", "Layer-1", "Layer-2", "Layer-1"] {
                d.apply_tab_actions(vec![TabAction::ToggleShown(layer.into())]);
                assert_eq!(d.tabs.active(), &start, "after toggling {layer}");
            }
        }
    }

    /// R44: what the developer can no longer reach leaves the selection.
    #[test]
    fn a_control_outside_the_new_active_tab_leaves_the_selection() {
        let mut form = form_with_layers(1);
        form.controls.push(Control::new("Button-1", ControlType::Button, 20, 20));
        form.controls.push(Control::new("Button-2", ControlType::Button, 200, 20));
        form.controls.push(Control::new("Timer-1", ControlType::Timer, 0, 0));
        form.set_control_layer("Button-2", Some("Layer-1")).expect("moved to the layer");
        let mut d = DesignerPanel::new(form);
        d.selected_ids = vec!["Button-1".into(), "Button-2".into(), "Timer-1".into()];

        let changed = d.apply_tab_actions(vec![TabAction::SelectLayer("Layer-1".into())]);
        assert!(changed);
        assert_eq!(d.selected_ids, vec!["Button-2".to_string()], "only the layer's own control stays");

        d.selected_ids = vec!["Button-1".into(), "Button-2".into(), "Timer-1".into()];
        let changed = d.apply_tab_actions(vec![TabAction::SelectNonVisuals]);
        assert!(changed);
        assert_eq!(d.selected_ids, vec!["Timer-1".to_string()], "on Non-Visuals only the cards");

        d.selected_ids = vec!["Button-1".into(), "Button-2".into(), "Timer-1".into()];
        let changed = d.apply_tab_actions(vec![TabAction::SelectForm]);
        assert!(changed);
        assert_eq!(d.selected_ids, vec!["Button-1".to_string()], "on Form only the base's visual controls");

        // Selecting the tab that is already active changes nothing.
        let changed = d.apply_tab_actions(vec![TabAction::SelectForm]);
        assert!(!changed);
    }
}

// ── T12: what the active tab lets the pointer do ─────────────────────────────

mod gating {
    use super::*;
    use crate::panels::properties::PropertiesPanel;

    fn timers() -> Form {
        let mut f = form_with_layers(1);
        for (i, (x, y)) in [(500, 400), (700, 100), (300, 600)].into_iter().enumerate() {
            f.controls.push(Control::new(format!("Timer-{}", i + 1), ControlType::Timer, x, y));
        }
        f
    }

    /// R30, R57 — the form's right edge is a grip only on `Form`.
    #[test]
    fn the_form_is_resized_from_its_edge_only_while_form_is_the_active_tab() {
        for (tab, resizable) in [
            (TabAction::SelectForm, true),
            (TabAction::SelectLayer("Layer-1".into()), false),
            (TabAction::SelectNonVisuals, false),
        ] {
            let mut r = Rig::new(form_with_layers(1));
            r.select_tab(tab.clone());
            let o = r.d.canvas_origin.expect("drawn");
            let edge = o + Vec2::new(1200.0 - 2.0, 300.0);
            // On `Non-Visuals` the canvas is the grid, so its edge is the form's too.
            r.drag(edge, edge + Vec2::new(100.0, 0.0));
            if resizable {
                assert!(r.d.form.width > 1200, "{tab:?}: the edge drag resizes the form, width {}", r.d.form.width);
            } else {
                assert_eq!(r.d.form.width, 1200, "{tab:?}: no grip, no resize");
                assert!(!r.d.dirty, "{tab:?}: and nothing was edited");
            }
            assert_eq!(r.d.form_resizable(), resizable);
        }
    }

    /// R30, R57 — and the inspector's size rows are read only there.
    #[test]
    fn the_inspector_s_width_and_height_are_read_only_off_form() {
        // Width, Height, the target device and the orientation: the rows that size the form.
        fn disabled_spin_buttons(locked: bool) -> (usize, usize) {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut panel = PropertiesPanel::new();
            panel.form_size_locked = locked;
            let form = Form::new("F", "F", 1200, 800);
            let tr = crate::i18n::current_tr(&ctx);
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 1600.0)));
            let mut out = ctx.run_ui(input, |root| {
                egui::CentralPanel::default().show_inside(root, |ui| {
                    let _ = panel.show(ui, &form, None, &[], &tr);
                });
            });
            out.textures_delta.clear();
            let update = out.platform_output.accesskit_update.expect("the first frame reports every node");
            let spins: Vec<bool> = update
                .nodes
                .iter()
                .filter(|(_, n)| n.role() == egui::accesskit::Role::SpinButton)
                .map(|(_, n)| n.is_disabled())
                .collect();
            (spins.iter().filter(|d| **d).count(), spins.len())
        }
        let (off_disabled, off_total) = disabled_spin_buttons(false);
        let (on_disabled, on_total) = disabled_spin_buttons(true);
        assert!(off_total >= 4, "X, Y, Width and Height are drag values: {off_total}");
        assert_eq!(off_total, on_total, "locking changes no row, only whether it takes input");
        assert_eq!(off_disabled, 0, "on Form every drag value takes input");
        assert_eq!(on_disabled, 2, "off Form exactly Width and Height are read only");
    }

    /// R52 — a card is selected by the press, never moved by a drag.
    #[test]
    fn a_drag_on_a_card_selects_it_and_moves_nothing() {
        let mut r = Rig::new(timers());
        r.select_tab(TabAction::SelectNonVisuals);
        let before: Vec<(String, i32, i32)> = r.d.form.controls.iter().map(|c| (c.id.clone(), c.rect.x, c.rect.y)).collect();
        let cells_before: Vec<(String, cobolt_forms::Rect)> =
            r.d.non_visuals_view().into_iter().map(|c| (c.id, c.rect)).collect();

        let from = r.card_centre("Timer-2");
        r.drag(from, from + Vec2::new(180.0, 150.0));

        assert_eq!(r.d.selected_ids, vec!["Timer-2".to_string()], "the press selects the card");
        let after: Vec<(String, i32, i32)> = r.d.form.controls.iter().map(|c| (c.id.clone(), c.rect.x, c.rect.y)).collect();
        assert_eq!(after, before, "no control's X or Y changed");
        let cells_after: Vec<(String, cobolt_forms::Rect)> =
            r.d.non_visuals_view().into_iter().map(|c| (c.id, c.rect)).collect();
        assert_eq!(cells_after, cells_before, "and no card changed cell");
        assert!(matches!(r.d.drag, DragState::None), "no drag is left under way");
        assert!(!r.d.dirty, "the form was not edited");
        assert!(r.d.undo_stack.is_empty());
    }

    /// R52 — a card has no resize knobs; a control on `Form` does.
    #[test]
    fn a_selected_card_has_no_knobs_where_a_control_on_form_has_them() {
        let mut form = timers();
        form.controls.push(Control::new("Button-1", ControlType::Button, 100, 100));
        let mut r = Rig::new(form);
        r.d.select_only("Button-1");
        r.settle(3);
        assert!(r.d.sel_handle_rect.is_some(), "on Form a selected control shows its knobs");

        r.select_tab(TabAction::SelectNonVisuals);
        r.d.select_only("Timer-1");
        r.settle(3);
        assert!(r.d.sel_handle_rect.is_none(), "on Non-Visuals a selected card shows none");
    }

    /// R52 — the arrow keys nudge a control on `Form` and never a card.
    #[test]
    fn the_arrow_keys_do_not_nudge_a_card() {
        fn right() -> Vec<Event> {
            vec![Event::Key {
                key: egui::Key::ArrowRight,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]
        }
        // Control: on Form the same key moves the selected button.
        let mut form = timers();
        form.controls.push(Control::new("Button-1", ControlType::Button, 100, 100));
        let mut r = Rig::new(form);
        r.d.select_only("Button-1");
        r.settle(2);
        r.frame(right());
        assert!(r.d.form.find_control("Button-1").unwrap().rect.x > 100, "on Form the arrow key nudges");

        let mut r = Rig::new(timers());
        r.select_tab(TabAction::SelectNonVisuals);
        r.d.select_only("Timer-1");
        r.settle(2);
        r.frame(right());
        assert_eq!(r.d.form.find_control("Timer-1").unwrap().rect.x, 500, "on Non-Visuals it does not");
        assert!(!r.d.dirty);
    }
}

// ── T13: the toolbox chooses the tab, and what the tab cannot take ───────────

mod toolbox {
    use super::*;

    fn mixed_form() -> Form {
        let mut f = form_with_layers(2);
        f.controls.push(Control::new("Button-1", ControlType::Button, 100, 100));
        f.controls.push(Control::new("Timer-1", ControlType::Timer, 300, 300));
        f
    }

    /// A clipboard holding `ids` of `mixed_form`, as a copy would leave it.
    fn clipboard_of(ids: &[&str]) -> Option<DesignerClipboard> {
        let mut source = DesignerPanel::new(mixed_form());
        source.selected_ids = ids.iter().map(|s| s.to_string()).collect();
        let mut clip = None;
        source.copy_selected(&mut clip);
        assert!(clip.is_some(), "something was copied");
        clip
    }

    fn starts(tab: &ActiveTab, d: &mut DesignerPanel) {
        match tab {
            ActiveTab::Form => d.tabs.select_form(),
            ActiveTab::NonVisuals => d.tabs.select_non_visuals(),
            ActiveTab::Layer(n) => d.tabs.select_layer(n),
        }
    }

    /// AC28, R59 — a press on a non-visual entry selects `Non-Visuals` from any
    /// tab, and no layer's `Visible` changes.
    #[test]
    fn a_non_visual_press_selects_non_visuals_from_any_tab_and_hides_nothing() {
        for start in [ActiveTab::Form, ActiveTab::Layer("Layer-1".into()), ActiveTab::NonVisuals] {
            let mut d = DesignerPanel::new(mixed_form());
            starts(&start, &mut d);
            d.tabs.set_shown("Layer-2", true);
            let shown = (d.tabs.is_shown("Layer-1"), d.tabs.is_shown("Layer-2"));

            d.on_toolbox_press(&ControlType::Timer);
            assert_eq!(d.tabs.active(), &ActiveTab::NonVisuals, "from {start:?}");
            assert_eq!((d.tabs.is_shown("Layer-1"), d.tabs.is_shown("Layer-2")), shown, "from {start:?}: no Visible changed");

            d.add_control(ControlType::Timer, 600, 400);
            let timer = d.form.controls.last().expect("the Timer was created").id.clone();
            assert!(
                d.active_tab_ids().is_some_and(|ids| ids.contains(&timer)),
                "from {start:?}: and it is a card on the tab that is showing"
            );
        }
    }

    /// AC28, R59 — a visual press leaves the tab as it is.
    #[test]
    fn a_visual_press_leaves_the_active_tab_alone() {
        for start in [ActiveTab::Form, ActiveTab::Layer("Layer-1".into())] {
            let mut d = DesignerPanel::new(mixed_form());
            starts(&start, &mut d);
            d.on_toolbox_press(&ControlType::Button);
            assert_eq!(d.tabs.active(), &start);
            let before = d.form.controls.len();
            d.add_control(ControlType::Button, 600, 400);
            assert_eq!(d.form.controls.len(), before + 1, "and the Button is created");
            assert_eq!(d.tabs.active(), &start, "the tab is still {start:?}");
        }
    }

    /// R58 — a visual control that reaches the canvas while `Non-Visuals` is
    /// active is refused with a message, and never rerouted to another tab.
    #[test]
    fn a_visual_control_is_refused_on_non_visuals_with_a_message() {
        let mut d = DesignerPanel::new(mixed_form());
        d.tabs.select_non_visuals();
        let before = d.form.controls.len();
        d.add_control(ControlType::Button, 600, 400);
        assert_eq!(d.form.controls.len(), before, "created nowhere");
        assert_eq!(d.tabs.active(), &ActiveTab::NonVisuals, "and no tab was changed");
        assert_eq!(d.notices, vec![DesignerNotice::VisualControlRefused]);
        assert!(!d.dirty && d.undo_stack.is_empty());
        // The notice reads in every language, and it is the toolbox's own hint.
        for lang in crate::i18n::Language::ALL {
            assert!(!DesignerNotice::VisualControlRefused.text(&lang.tr()).is_empty());
        }
        // A non-visual one still works there.
        d.add_control(ControlType::Timer, 0, 0);
        assert_eq!(d.form.controls.len(), before + 1);
    }

    /// AC21 (R58, Q29) — a paste holding any visual control is refused whole on
    /// `Non-Visuals`; one of non-visual controls only is pasted.
    #[test]
    fn a_paste_with_a_visual_control_is_refused_whole_on_non_visuals() {
        for (ids, allowed) in [(&["Button-1"][..], false), (&["Button-1", "Timer-1"][..], false), (&["Timer-1"][..], true)] {
            let mut d = DesignerPanel::new(mixed_form());
            d.tabs.select_non_visuals();
            let clip = clipboard_of(ids);
            let before = d.form.controls.len();
            d.paste_from_clipboard(&clip);
            if allowed {
                assert_eq!(d.form.controls.len(), before + ids.len(), "{ids:?} is pasted");
                assert!(d.notices.is_empty());
            } else {
                assert_eq!(d.form.controls.len(), before, "{ids:?}: nothing is pasted — not even the Timer");
                assert_eq!(d.notices, vec![DesignerNotice::PasteRefused]);
                assert!(!d.dirty && d.undo_stack.is_empty());
            }
            assert_eq!(d.tabs.active(), &ActiveTab::NonVisuals);
        }
    }

    /// R48 — pasting or duplicating non-visual controls brings `Non-Visuals` up; a
    /// mixed paste stays where it was and selects what is reachable there.
    #[test]
    fn pasting_and_duplicating_a_non_visual_control_select_non_visuals() {
        // Paste from the Form tab.
        let mut d = DesignerPanel::new(mixed_form());
        d.tabs.select_form();
        d.paste_from_clipboard(&clipboard_of(&["Timer-1"]));
        assert_eq!(d.tabs.active(), &ActiveTab::NonVisuals, "a pasted Timer");

        // Duplicate from a layer.
        let mut d = DesignerPanel::new(mixed_form());
        d.tabs.select_layer("Layer-1");
        d.selected_ids = vec!["Timer-1".into()];
        let mut clip = None;
        d.duplicate_selected(&mut clip);
        assert_eq!(d.tabs.active(), &ActiveTab::NonVisuals, "a duplicated Timer");
        assert_eq!(d.form.controls.iter().filter(|c| c.control_type == ControlType::Timer).count(), 2);

        // Mixed: stays on Form, and only the Button is selected.
        let mut d = DesignerPanel::new(mixed_form());
        d.tabs.select_form();
        d.paste_from_clipboard(&clipboard_of(&["Button-1", "Timer-1"]));
        assert_eq!(d.tabs.active(), &ActiveTab::Form, "a mixed paste does not move the tab");
        assert_eq!(d.selected_ids.len(), 1, "and selects what the tab can reach: {:?}", d.selected_ids);
        assert!(d.selected_ids[0].starts_with("Button"));
    }
}

// ── T14: a new control lands in the active tab ───────────────────────────────

mod placement {
    use super::*;

    /// AC28, R28 — a Button is created in the active layer, or on `Form`; a Timer
    /// on `Non-Visuals`, with no layer.
    #[test]
    fn a_new_control_is_created_in_the_active_tab() {
        let mut d = DesignerPanel::new(form_with_layers(2));

        d.tabs.select_form();
        d.add_control(ControlType::Button, 40, 40);
        let on_form = d.form.controls.last().unwrap().clone();
        assert_eq!(on_form.layer, None, "on Form a Button names no layer");

        d.tabs.select_layer("Layer-2");
        d.add_control(ControlType::Button, 200, 40);
        let in_layer = d.form.controls.last().unwrap().clone();
        assert_eq!(in_layer.layer.as_deref(), Some("Layer-2"), "on Layer-2 a Button is created in it");
        assert!(d.active_tab_ids().unwrap().contains(&in_layer.id));
        assert!(!d.active_tab_ids().unwrap().contains(&on_form.id), "and the base's Button is not in the tab");

        d.on_toolbox_press(&ControlType::Timer);
        d.add_control(ControlType::Timer, 300, 300);
        let timer = d.form.controls.last().unwrap().clone();
        assert_eq!(timer.layer, None, "a Timer names no layer, even pressed from Layer-2");
        assert!(d.active_tab_ids().unwrap().contains(&timer.id), "it is a card on Non-Visuals");
    }

    /// AC20, R47 — every control type lands where `is_non_visual()` says, so a
    /// type added to the catalogue later is covered with no change here.
    #[test]
    fn every_control_type_lands_in_non_visuals_if_and_only_if_it_is_non_visual() {
        let mut checked = 0;
        let mut non_visual = Vec::new();
        for ct in ControlType::ALL {
            if matches!(ct, ControlType::Custom { .. }) {
                continue;
            }
            let mut d = DesignerPanel::new(form_with_layers(1));
            // Created from a layer, to prove a non-visual type ignores it.
            d.tabs.select_layer("Layer-1");
            if ct.is_non_visual() {
                d.on_toolbox_press(ct);
            }
            d.add_control(ct.clone(), 60, 60);
            let made = d
                .form
                .controls
                .iter()
                .find(|c| c.control_type == *ct)
                .unwrap_or_else(|| panic!("a {} was created", ct.as_str()))
                .clone();

            d.tabs.select_non_visuals();
            let in_nv = d.active_tab_ids().is_some_and(|s| s.contains(&made.id));
            assert_eq!(in_nv, ct.is_non_visual(), "{}: on Non-Visuals iff non-visual", ct.as_str());
            if ct.is_non_visual() {
                assert_eq!(made.layer, None, "{}: a non-visual control names no layer (R47)", ct.as_str());
                non_visual.push(ct.as_str().to_string());
            } else {
                assert_eq!(made.layer.as_deref(), Some("Layer-1"), "{}: created in the active layer", ct.as_str());
                d.tabs.select_layer("Layer-1");
                assert!(d.active_tab_ids().unwrap().contains(&made.id), "{}: and it is in that tab", ct.as_str());
            }
            checked += 1;
        }
        // MenuBar, SideMenu and StatusBar are visual (Q4): they stay on Form/layers.
        for ct in [ControlType::MenuBar, ControlType::SideMenu, ControlType::StatusBar] {
            assert!(!ct.is_non_visual(), "{} is a visual control", ct.as_str());
        }
        println!(
            "  AC20: {checked} control types created; {} land in Non-Visuals, in the catalogue's own words: {}",
            non_visual.len(),
            non_visual.join(", ")
        );
        assert!(checked >= 40 && !non_visual.is_empty());
    }

    /// R8 — a child follows its container, so it names no layer.
    #[test]
    fn a_control_dropped_into_a_container_of_its_layer_follows_it_and_undo_restores_the_layer() {
        let mut d = DesignerPanel::new(form_with_layers(1));
        d.tabs.select_layer("Layer-1");
        d.add_control(ControlType::Panel, 100, 100);
        let panel = d.form.controls.last().unwrap().id.clone();
        assert_eq!(d.form.find_control(&panel).unwrap().layer.as_deref(), Some("Layer-1"));
        let (px, py) = {
            let r = d.form.find_control(&panel).unwrap().rect;
            (r.x + r.w / 2, r.y + r.h / 2)
        };

        d.add_control(ControlType::Button, px - 20, py - 10);
        let button = d.form.controls.last().unwrap().id.clone();
        let b = d.form.find_control(&button).unwrap();
        assert_eq!(b.parent.as_deref(), Some(panel.as_str()), "the Button was taken into the Panel");
        assert_eq!(b.layer, None, "and, a child, it names no layer (R8)");

        // The Reparent is its own undo step: undoing it makes the Button a root of
        // the layer again, not of the base.
        d.undo();
        let b = d.form.find_control(&button).unwrap();
        assert_eq!((b.parent.clone(), b.layer.clone()), (None, Some("Layer-1".to_string())));
        d.redo();
        assert_eq!(d.form.find_control(&button).unwrap().layer, None);
    }

    /// R8, R44 — only the active tab's containers can adopt a control.
    #[test]
    fn a_container_of_another_tab_does_not_adopt_a_control() {
        let mut form = form_with_layers(1);
        let mut panel = Control::new("Panel-1", ControlType::Panel, 100, 100);
        panel.rect.w = 300;
        panel.rect.h = 200;
        form.controls.push(panel); // a Panel in the base
        let mut d = DesignerPanel::new(form);
        d.tabs.select_layer("Layer-1");
        d.add_control(ControlType::Button, 150, 150); // over the base's Panel
        let b = d.form.controls.last().unwrap();
        assert_eq!(b.parent, None, "the base's Panel is not there to be dropped on");
        assert_eq!(b.layer.as_deref(), Some("Layer-1"));

        // Control: on Form the same drop is adopted.
        let mut d = DesignerPanel::new({
            let mut f = form_with_layers(1);
            let mut p = Control::new("Panel-1", ControlType::Panel, 100, 100);
            p.rect.w = 300;
            p.rect.h = 200;
            f.controls.push(p);
            f
        });
        d.tabs.select_form();
        d.add_control(ControlType::Button, 150, 150);
        assert_eq!(d.form.controls.last().unwrap().parent.as_deref(), Some("Panel-1"));
    }

    /// R8 — a child dragged out of its container becomes a root of the layer it was in.
    #[test]
    fn a_child_dragged_out_of_its_container_becomes_a_root_of_the_layer() {
        let mut d = DesignerPanel::new(form_with_layers(1));
        d.tabs.select_layer("Layer-1");
        d.add_control(ControlType::Panel, 100, 100);
        let panel = d.form.controls.last().unwrap().id.clone();
        let r = d.form.find_control(&panel).unwrap().rect;
        d.add_control(ControlType::Button, r.x + 10, r.y + 10);
        let button = d.form.controls.last().unwrap().id.clone();
        assert_eq!(d.form.find_control(&button).unwrap().parent.as_deref(), Some(panel.as_str()));

        // Move it well clear of the Panel and let it settle where it was dropped.
        d.form.find_control_mut(&button).unwrap().rect.x = r.x + r.w + 200;
        d.form.find_control_mut(&button).unwrap().rect.y = r.y + r.h + 200;
        d.reparent_to_drop(&button);
        let b = d.form.find_control(&button).unwrap();
        assert_eq!(b.parent, None);
        assert_eq!(b.layer.as_deref(), Some("Layer-1"), "it stays in the layer it was dragged out in");
    }

    /// R5 — a generated control name is never one a layer is called.
    #[test]
    fn a_generated_control_name_skips_a_name_a_layer_has() {
        let mut form = Form::new("F", "F", 1200, 800);
        form.add_layer().unwrap();
        form.rename_layer("Layer-1", "Button-1").expect("a layer may be called that while no control is");
        let mut d = DesignerPanel::new(form);
        d.add_control(ControlType::Button, 20, 20);
        assert_eq!(d.form.controls.last().unwrap().id, "Button-2", "Button-1 is a layer's name");
    }

    /// Pasting lands in the active tab too: a root takes the layer, a child none.
    #[test]
    fn a_paste_lands_in_the_active_layer() {
        let mut source = DesignerPanel::new(form_with_layers(2));
        source.tabs.select_layer("Layer-1");
        source.add_control(ControlType::Button, 20, 20);
        source.selected_ids = vec![source.form.controls.last().unwrap().id.clone()];
        let mut clip = None;
        source.copy_selected(&mut clip);
        assert_eq!(source.form.controls[0].layer.as_deref(), Some("Layer-1"));

        let mut d = DesignerPanel::new(form_with_layers(2));
        d.tabs.select_layer("Layer-2");
        d.paste_from_clipboard(&clip);
        assert_eq!(d.form.controls.last().unwrap().layer.as_deref(), Some("Layer-2"), "pasted into the layer on show");
        d.tabs.select_form();
        d.paste_from_clipboard(&clip);
        assert_eq!(d.form.controls.last().unwrap().layer, None, "and on Form it names none");
    }
}

// ── T15: the pointer, the lasso and Select All, per tab ──────────────────────

mod selection {
    use super::*;

    /// One control in the base, one in `Layer-1`, two in `Layer-2` — the second
    /// drawn over `Layer-1`'s.
    fn three_tabs() -> Form {
        let mut f = form_with_layers(2);
        let mut add = |id: &str, x: i32, y: i32, layer: Option<&str>| {
            let mut c = Control::new(id, ControlType::Button, x, y);
            c.rect.w = 80;
            c.rect.h = 30;
            c.layer = layer.map(str::to_owned);
            f.controls.push(c);
        };
        add("Base-A", 40, 40, None);
        add("L1-A", 200, 40, Some("Layer-1"));
        add("L2-A", 360, 40, Some("Layer-2"));
        add("L2-Over", 210, 45, Some("Layer-2")); // covers L1-A
        f
    }

    fn rig(active: TabAction) -> Rig {
        let mut r = Rig::new(three_tabs());
        r.select_tab(active);
        r
    }

    fn ctrl_click(r: &mut Rig, at: Pos2) {
        r.mods = egui::Modifiers::COMMAND;
        r.click(at);
        r.mods = egui::Modifiers::NONE;
    }

    fn xy(r: &Rig) -> Vec<(String, i32, i32)> {
        r.d.form.controls.iter().map(|c| (c.id.clone(), c.rect.x, c.rect.y)).collect()
    }

    /// AC17 (R44) — with `Layer-1` active, only its control answers the pointer.
    #[test]
    fn with_a_layer_active_only_its_controls_answer_the_pointer() {
        let mut r = rig(TabAction::SelectLayer("Layer-1".into()));
        let before = xy(&r);

        // A click on the layer's own control, which a Layer-2 control is drawn over.
        let l1 = r.centre_of("L1-A") + Vec2::new(-30.0, -10.0); // the part L2-Over does not cover
        r.click(l1);
        assert_eq!(r.d.selected_ids, vec!["L1-A".to_string()], "a click selects the active layer's control");

        // …and under the Layer-2 control that covers it: the inactive one blocks nothing.
        let under = r.centre_of("L2-Over");
        r.click(under);
        assert_eq!(r.d.selected_ids, vec!["L1-A".to_string()], "L2-Over is not there; L1-A is hit beneath it");

        // The base's control and Layer-2's: a click is a click on empty canvas.
        for other in ["Base-A", "L2-A"] {
            r.click(r.centre_of(other));
            assert!(r.d.selected_ids.is_empty(), "a click on {other} selects nothing: {:?}", r.d.selected_ids);
        }

        // A drag on one moves nothing and starts a lasso instead.
        let from = r.centre_of("Base-A");
        r.frame(vec![Event::PointerMoved(from)]);
        r.frame(vec![press(from, true)]);
        r.frame(vec![Event::PointerMoved(from + Vec2::new(40.0, 40.0))]);
        assert!(matches!(r.d.drag, DragState::RubberBand { .. }), "a lasso is under way, not a move");
        r.frame(vec![press(from + Vec2::new(40.0, 40.0), false)]);
        r.frame(vec![]);
        assert_eq!(xy(&r), before, "no control moved");

        // A lasso over all four reaches the active layer's alone.
        let o = r.d.canvas_origin.unwrap();
        r.drag(o + Vec2::new(10.0, 10.0), o + Vec2::new(520.0, 200.0));
        assert_eq!(r.d.selected_ids, vec!["L1-A".to_string()], "a lasso selects only the active layer's controls");
        assert_eq!(xy(&r), before);
    }

    /// AC17 — the same with the base active.
    #[test]
    fn with_form_active_only_the_bases_controls_answer_the_pointer() {
        let mut r = rig(TabAction::SelectForm);
        r.d.tabs.set_shown("Layer-1", true); // drawn over the base, and still inactive
        r.settle(2);
        let before = xy(&r);

        r.click(r.centre_of("Base-A"));
        assert_eq!(r.d.selected_ids, vec!["Base-A".to_string()]);
        r.click(r.centre_of("L1-A"));
        assert!(r.d.selected_ids.is_empty(), "a layer's control is empty canvas to the pointer: {:?}", r.d.selected_ids);

        let o = r.d.canvas_origin.unwrap();
        r.drag(o + Vec2::new(10.0, 10.0), o + Vec2::new(520.0, 200.0));
        assert_eq!(r.d.selected_ids, vec!["Base-A".to_string()], "the lasso reaches the base's control only");
        assert_eq!(xy(&r), before, "and moved nothing");
    }

    /// R62, Q2b — a layer whose box is off is not there to be reached.
    #[test]
    fn the_controls_of_an_active_layer_whose_box_is_off_cannot_be_reached() {
        let mut r = rig(TabAction::SelectLayer("Layer-1".into()));
        r.d.apply_tab_actions(vec![TabAction::ToggleShown("Layer-1".into())]);
        r.settle(2);
        assert_eq!(r.d.tabs.active(), &ActiveTab::Layer("Layer-1".into()), "it is still the active tab");
        r.click(r.centre_of("L1-A") + Vec2::new(-30.0, -10.0));
        assert!(r.d.selected_ids.is_empty(), "its control is out of reach: {:?}", r.d.selected_ids);
        r.d.apply_tab_actions(vec![TabAction::ToggleShown("Layer-1".into())]);
        r.settle(2);
        r.click(r.centre_of("L1-A") + Vec2::new(-30.0, -10.0));
        assert_eq!(r.d.selected_ids, vec!["L1-A".to_string()], "ticking the box brings it back");
    }

    fn select_all(r: &mut Rig) {
        r.mods = egui::Modifiers::COMMAND;
        r.frame(vec![Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }]);
        r.mods = egui::Modifiers::NONE;
    }

    /// AC18 (R45) — Cmd/Ctrl+A selects the active tab's controls, children included.
    #[test]
    fn select_all_selects_exactly_the_active_tabs_controls() {
        let mut f = three_tabs();
        // A Panel with a child in the base: the child is selected with it, as it was.
        let mut panel = Control::new("Panel-1", ControlType::Panel, 40, 200);
        panel.rect.w = 200;
        panel.rect.h = 100;
        f.controls.push(panel);
        let mut child = Control::new("Child-1", ControlType::Button, 60, 220);
        child.parent = Some("Panel-1".into());
        f.controls.push(child);
        let mut r = Rig::new(f);

        r.select_tab(TabAction::SelectForm);
        select_all(&mut r);
        let mut got = r.d.selected_ids.clone();
        got.sort();
        assert_eq!(got, vec!["Base-A", "Child-1", "Panel-1"], "the base's, a container's child included");

        r.select_tab(TabAction::SelectLayer("Layer-2".into()));
        select_all(&mut r);
        let mut got = r.d.selected_ids.clone();
        got.sort();
        assert_eq!(got, vec!["L2-A", "L2-Over"], "a layer's own, and nothing of the base's or Layer-1's");

        // A layer with nothing in it: the selection becomes empty.
        let mut empty = Rig::new(form_with_layers(1));
        empty.select_tab(TabAction::SelectLayer("Layer-1".into()));
        empty.d.selected_ids = vec!["stale".into()];
        select_all(&mut empty);
        assert!(empty.d.selected_ids.is_empty());

        // A form with no layers: every control, as before the feature.
        let mut plain = Rig::new({
            let mut f = Form::new("F", "F", 1200, 800);
            f.controls.push(Control::new("Button-1", ControlType::Button, 10, 10));
            f.controls.push(Control::new("Label-1", ControlType::Label, 10, 60));
            f
        });
        plain.settle(2);
        select_all(&mut plain);
        assert_eq!(plain.d.selected_ids.len(), 2);
    }

    /// AC22 (R49) — the cells of 1, 5, 6 and 11 cards; deleting one closes the gap.
    #[test]
    fn the_cards_fill_five_columns_row_by_row_and_a_deletion_moves_the_rest_up() {
        use cobolt_forms::nv_grid::{CELL_H, CELL_W, COLUMNS, GAP, MARGIN};
        fn cell_of(r: cobolt_forms::Rect) -> (i32, i32) {
            ((r.y - MARGIN) / (CELL_H + GAP), (r.x - MARGIN) / (CELL_W + GAP))
        }
        fn timers(n: usize) -> Form {
            let mut f = Form::new("F", "F", 1200, 800);
            for i in 0..n {
                f.controls.push(Control::new(format!("Timer-{:02}", i + 1), ControlType::Timer, 700, 700));
            }
            f
        }
        for (n, last) in [(1usize, (0, 0)), (5, (0, 4)), (6, (1, 0)), (11, (2, 0))] {
            let d = DesignerPanel::new(timers(n));
            let cells: Vec<(i32, i32)> = d.non_visuals_view().iter().map(|c| cell_of(c.rect)).collect();
            assert_eq!(cells.len(), n);
            assert_eq!(*cells.last().unwrap(), last, "{n} cards: the last is at (row, column) {last:?}");
            let mut unique = cells.clone();
            unique.sort();
            unique.dedup();
            assert_eq!(unique.len(), n, "{n} cards: no two share a cell");
            assert!(cells.iter().all(|(_, c)| *c >= 0 && (*c as usize) < COLUMNS), "none is outside the five columns");
        }
        // Delete the third of six: the fourth, fifth and sixth each move up one cell, no gap.
        let mut d = DesignerPanel::new(timers(6));
        d.tabs.select_non_visuals();
        d.selected_ids = vec!["Timer-03".into()];
        d.delete_selected();
        let cells: Vec<(i32, i32)> = d.non_visuals_view().iter().map(|c| cell_of(c.rect)).collect();
        assert_eq!(cells, vec![(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)], "five cards fill the first row");
    }

    /// AC22 — a long list scrolls; the form and the panel do not grow.
    #[test]
    fn many_cards_scroll_and_neither_the_form_nor_the_panel_grows() {
        let mut one = Rig::new({
            let mut f = Form::new("F", "F", 1200, 800);
            f.controls.push(Control::new("Timer-1", ControlType::Timer, 10, 10));
            f
        });
        one.select_tab(TabAction::SelectNonVisuals);
        let mut many = Rig::new({
            let mut f = Form::new("F", "F", 1200, 800);
            for i in 0..120 {
                f.controls.push(Control::new(format!("Timer-{:03}", i + 1), ControlType::Timer, 10, 10));
            }
            f
        });
        many.select_tab(TabAction::SelectNonVisuals);
        assert!(many.d.canvas_size().1 > 800.0, "120 cards need more than the form's height: {:?}", many.d.canvas_size());
        assert_eq!(many.d.form.height, 800, "the form is not changed");
        assert_eq!(many.used, one.used, "and the panel uses the same room");
        assert_eq!(many.d.tab_bar_rect, one.d.tab_bar_rect);
    }

    /// AC23 (R50, R51) — types A-Z by their English names, names A-Z within a
    /// type; renaming moves a card, undo and redo move it back; every language
    /// gives the same order.
    #[test]
    fn the_cards_read_types_a_to_z_then_names_a_to_z_in_every_language() {
        let mut form = Form::new("F", "F", 1200, 800);
        // Created in scrambled order, with names that sort differently from it.
        for (id, ct) in [
            ("tm-b", ControlType::Timer),
            ("sql-1", ControlType::SqlDatabase),
            ("TM-a", ControlType::Timer),
            ("rest-1", ControlType::RestClient),
            ("kb-1", ControlType::KnowledgeBase),
            ("agent-2", ControlType::AgentObject),
            ("agent-1", ControlType::AgentObject),
        ] {
            form.controls.push(Control::new(id, ct, 5, 5));
        }
        let want = ["agent-1", "agent-2", "kb-1", "rest-1", "sql-1", "TM-a", "tm-b"];
        let order = |d: &DesignerPanel| d.non_visuals_view().into_iter().map(|c| c.id).collect::<Vec<_>>();

        for lang in crate::i18n::Language::ALL {
            let mut r = Rig::new(form.clone());
            crate::i18n::set_language(&r.ctx, *lang);
            r.select_tab(TabAction::SelectNonVisuals);
            assert_eq!(order(&r.d), want, "{lang:?}: the same order in every language");
        }

        // A rename moves the card; undo and redo put it back and forth.
        let mut d = DesignerPanel::new(form);
        d.apply(Cmd::Rename { old: "agent-1".into(), new: "zz-agent".into() });
        assert_eq!(order(&d), ["agent-2", "zz-agent", "kb-1", "rest-1", "sql-1", "TM-a", "tm-b"], "renamed: still among AgentObjects");
        d.apply(Cmd::Rename { old: "tm-b".into(), new: "A-timer".into() });
        assert_eq!(order(&d).last().map(String::as_str), Some("TM-a"), "a renamed Timer re-sorts inside Timer");
        d.undo();
        assert_eq!(order(&d).last().map(String::as_str), Some("tm-b"));
        d.redo();
        assert_eq!(order(&d)[order(&d).len() - 2], "A-timer");
    }

    /// AC24 (R52) — click, Ctrl-click, the lasso and Select All reach only the
    /// cards; a card is a real control, so its inspector is that control's.
    #[test]
    fn click_ctrl_click_lasso_and_select_all_reach_only_the_cards() {
        let mut f = three_tabs();
        for i in 0..4 {
            f.controls.push(Control::new(format!("Timer-{}", i + 1), ControlType::Timer, 40, 40));
        }
        let mut r = Rig::new(f);
        r.select_tab(TabAction::SelectNonVisuals);

        r.click(r.card_centre("Timer-2"));
        assert_eq!(r.d.selected_ids, vec!["Timer-2".to_string()]);
        let third = r.card_centre("Timer-3");
        ctrl_click(&mut r, third);
        let mut got = r.d.selected_ids.clone();
        got.sort();
        assert_eq!(got, vec!["Timer-2", "Timer-3"], "Ctrl-click adds a card");

        // The base's Button sits at (40, 40) on the form — and is not there on this tab.
        r.click(r.centre_of("Base-A"));
        assert!(r.d.selected_ids.iter().all(|id| id.starts_with("Timer")), "{:?}", r.d.selected_ids);

        // A lasso across the first row's first two cards.
        let o = r.d.canvas_origin.unwrap();
        let a = r.card_centre("Timer-1");
        let b = r.card_centre("Timer-2");
        r.drag(o + Vec2::new(2.0, 2.0), Pos2::new(b.x + 10.0, a.y + 10.0));
        let mut got = r.d.selected_ids.clone();
        got.sort();
        assert_eq!(got, vec!["Timer-1", "Timer-2"], "the lasso takes the cards it covers");

        select_all(&mut r);
        assert_eq!(r.d.selected_ids.len(), 4, "Select All takes the four cards and nothing else");
        // …and the inspector is the control's own: its Properties, Events and Procs.
        let primary = r.d.selected_ids[0].clone();
        assert!(r.d.form.find_control(&primary).is_some_and(|c| c.control_type == ControlType::Timer));
    }
}

// ── T17: add, rename and re-stack a layer, each one undo step ────────────────

mod ops {
    use super::*;
    use cobolt_forms::model::MAX_LAYERS;

    fn names(d: &DesignerPanel) -> Vec<String> {
        d.form.layers.iter().map(|l| l.name.clone()).collect()
    }

    /// The `+`: a layer on top, named, selected, one undo step; undo and redo.
    #[test]
    fn the_plus_adds_a_layer_selects_it_and_is_one_undo_step() {
        let mut d = DesignerPanel::new(form_with_layers(0));
        d.apply_tab_actions(vec![TabAction::Add]);
        assert_eq!(names(&d), ["Layer-1"]);
        assert_eq!(d.tabs.active(), &ActiveTab::Layer("Layer-1".into()), "the new layer is selected (Q30)");
        assert!(d.tabs.is_shown("Layer-1"), "…and, selected, shown (R60)");
        assert!(d.dirty, "adding is an edit");
        assert_eq!(d.undo_stack.len(), 1, "one undo step");

        d.apply_tab_actions(vec![TabAction::Add]);
        assert_eq!(names(&d), ["Layer-1", "Layer-2"], "the next one is above the first");
        assert!(!d.tabs.is_shown("Layer-1"), "and selecting it hid the other (R61)");

        d.undo();
        d.tabs.reconcile(&d.form);
        assert_eq!(names(&d), ["Layer-1"], "undo removes the second");
        assert_eq!(d.tabs.active(), &ActiveTab::Form, "the tab that was selected is gone, so Form is");
        d.undo();
        d.tabs.reconcile(&d.form);
        assert!(d.form.layers.is_empty());
        assert_eq!(d.tabs.active(), &ActiveTab::Form, "a tab that points at nothing falls back to Form");
        d.redo();
        assert_eq!(names(&d), ["Layer-1"], "redo brings it back");
    }

    /// AC3 (R7) — the 64th layer is added; the 65th is refused, with a message.
    #[test]
    fn the_sixty_fourth_layer_is_added_and_the_sixty_fifth_refused() {
        let mut d = DesignerPanel::new(form_with_layers(0));
        for _ in 0..MAX_LAYERS {
            d.apply_tab_actions(vec![TabAction::Add]);
        }
        assert_eq!(d.form.layers.len(), 64, "the 64th is added");
        assert_eq!(d.undo_stack.len(), 64);
        d.apply_tab_actions(vec![TabAction::Add]);
        assert_eq!(d.form.layers.len(), 64, "the 65th is refused");
        assert_eq!(d.undo_stack.len(), 64, "and is no undo step");
        assert_eq!(d.notices, vec![DesignerNotice::LayerLimit]);
        for lang in crate::i18n::Language::ALL {
            let text = DesignerNotice::LayerLimit.text(&lang.tr());
            assert!(text.contains("64") && !text.contains("{}"), "{lang:?}: the limit is filled in: {text}");
        }
    }

    /// AC11 (R23) — adding 64 layers through the bar changes neither the bar nor the room.
    #[test]
    fn adding_sixty_four_layers_does_not_change_the_bar_or_the_window() {
        let mut r = Rig::new(form_with_layers(0));
        r.settle(4);
        let (bar, used) = (r.d.tab_bar_rect, r.used);
        for _ in 0..MAX_LAYERS {
            r.d.apply_tab_actions(vec![TabAction::Add]);
        }
        r.settle(6);
        assert_eq!(r.d.form.layers.len(), 64);
        assert_eq!((r.d.tab_bar_rect, r.used), (bar, used), "the same strip and the same room");
    }

    /// The `+` is a real button on the bar.
    #[test]
    fn clicking_the_plus_adds_a_layer() {
        let mut r = Rig::new(form_with_layers(0));
        r.settle(3);
        let plus = r.slot_rect(Slot::Add).center();
        r.click(plus);
        assert_eq!(names(&r.d), ["Layer-1"]);
        assert_eq!(r.d.tabs.active(), &ActiveTab::Layer("Layer-1".into()));
    }

    /// AC2 (R4–R6) — a rename that would collide, or is not a name, is refused.
    #[test]
    fn a_name_that_is_reserved_in_use_or_not_a_name_is_refused() {
        let mut form = form_with_layers(2);
        form.controls.push(Control::new("Button-1", ControlType::Button, 10, 10));
        let mut d = DesignerPanel::new(form);
        let before = saved(&d);
        for bad in ["Form", "form", "FORM", "Non-Visuals", "non-visuals", "NON-VISUALS", "Button-1", "button-1", "Layer-2", "1abc", "a b", "", "   "] {
            d.notices.clear();
            d.apply_tab_actions(vec![TabAction::Rename { layer: "Layer-1".into(), to: bad.into() }]);
            assert_eq!(names(&d), ["Layer-1", "Layer-2"], "{bad:?} is refused");
            assert_eq!(d.notices, vec![DesignerNotice::LayerNameRefused], "{bad:?}: with a message");
        }
        assert!(d.undo_stack.is_empty() && !d.dirty, "no refusal is an undo step or an edit");
        assert_eq!(saved(&d), before);
    }

    /// R26, R32 — a rename follows into the controls and the code, retargets the
    /// tab, and undo and redo put it all back.
    #[test]
    fn a_rename_follows_into_controls_and_code_and_is_undoable() {
        let mut form = form_with_layers(2);
        let mut b = Control::new("Button-1", ControlType::Button, 10, 10);
        b.layer = Some("Layer-1".into());
        let mut ev = cobolt_forms::EventBinding::new("onClick", "BUTTON-1--ONCLICK");
        ev.code = "SET LAYER-1::Visible TO TRUE.".into();
        b.events.push(ev);
        form.controls.push(b);
        let mut d = DesignerPanel::new(form);
        d.tabs.select_layer("Layer-1");

        d.apply_tab_actions(vec![TabAction::Rename { layer: "Layer-1".into(), to: "Detail".into() }]);
        assert_eq!(names(&d), ["Detail", "Layer-2"]);
        let b = d.form.find_control("Button-1").unwrap();
        assert_eq!(b.layer.as_deref(), Some("Detail"), "the control that named it follows");
        assert!(b.events[0].code.to_ascii_uppercase().contains("DETAIL::VISIBLE"), "and so does the code: {}", b.events[0].code);
        assert_eq!(d.tabs.active(), &ActiveTab::Layer("Detail".into()), "the active tab follows the new name");
        assert!(d.tabs.is_shown("Detail"));
        assert_eq!(d.undo_stack.len(), 1);

        d.undo();
        assert_eq!(names(&d), ["Layer-1", "Layer-2"]);
        let b = d.form.find_control("Button-1").unwrap();
        assert_eq!(b.layer.as_deref(), Some("Layer-1"));
        assert!(b.events[0].code.to_ascii_uppercase().contains("LAYER-1::VISIBLE"));
        assert_eq!(d.tabs.active(), &ActiveTab::Layer("Layer-1".into()));
        d.redo();
        assert_eq!(names(&d), ["Detail", "Layer-2"]);
        assert_eq!(d.tabs.active(), &ActiveTab::Layer("Detail".into()));

        // A change of letter case alone is a rename.
        d.apply_tab_actions(vec![TabAction::Rename { layer: "Detail".into(), to: "detail".into() }]);
        assert_eq!(names(&d), ["detail", "Layer-2"]);
    }

    /// R11, R26 — a tab dropped among the layers re-stacks them, one undo step.
    #[test]
    fn dropping_a_tab_restacks_the_layers_and_changes_the_paint_order() {
        let mut d = DesignerPanel::new(form_with_layers(3));
        let rank = |d: &DesignerPanel, n: &str| d.form.layer_rank(Some(n));
        assert!(rank(&d, "Layer-1") < rank(&d, "Layer-3"), "Layer-3 is above Layer-1");

        d.apply_tab_actions(vec![TabAction::Move { from: 0, to: 2 }]);
        assert_eq!(names(&d), ["Layer-2", "Layer-3", "Layer-1"]);
        assert!(rank(&d, "Layer-1") > rank(&d, "Layer-3"), "Layer-1 now paints above Layer-3");
        assert_eq!(d.undo_stack.len(), 1);
        d.undo();
        assert_eq!(names(&d), ["Layer-1", "Layer-2", "Layer-3"]);
        d.redo();
        assert_eq!(names(&d), ["Layer-2", "Layer-3", "Layer-1"]);

        // Out of range, or to the same place: nothing happens, no step.
        let steps = d.undo_stack.len();
        d.apply_tab_actions(vec![TabAction::Move { from: 9, to: 0 }, TabAction::Move { from: 1, to: 1 }]);
        assert_eq!(d.undo_stack.len(), steps);
    }

    /// The same, with the pointer: dragging a layer tab to the right of the others.
    #[test]
    fn dragging_a_layer_tab_with_the_pointer_restacks_and_form_cannot_be_dragged() {
        let mut r = Rig::new(form_with_layers(3));
        r.settle(3);
        let grab = |r: &Rig, i: usize| layer_tab_parts(r.slot_rect(Slot::Layer(i))).label.center();

        // Layer-1 dragged well past Layer-3.
        let from = grab(&r, 0);
        r.drag(from, from + Vec2::new(400.0, 0.0));
        assert_eq!(names(&r.d), ["Layer-2", "Layer-3", "Layer-1"], "dropped past the others, it is the top layer");

        // Layer-1 (now last) dragged far to the left: no further than the layer nearest Form.
        let from = grab(&r, 2);
        r.drag(from, from - Vec2::new(1500.0, 0.0));
        assert_eq!(names(&r.d), ["Layer-1", "Layer-2", "Layer-3"], "never before Form: it lands next to it");

        // The Form tab and the Non-Visuals tab cannot be dragged.
        let steps = r.d.undo_stack.len();
        for slot in [Slot::Form, Slot::NonVisuals] {
            let c = r.slot_rect(slot).center();
            r.drag(c, c + Vec2::new(300.0, 0.0));
        }
        assert_eq!(names(&r.d), ["Layer-1", "Layer-2", "Layer-3"]);
        assert_eq!(r.d.undo_stack.len(), steps, "no step");
    }
}

// ── T18: a layer's own properties in the inspector ───────────────────────────

mod layer_props {
    use super::*;
    use crate::panels::properties::PropertiesPanel;
    use cobolt_forms::model::LAYER_TRANSPARENT_COLOR;

    fn backdrop(d: &DesignerPanel, name: &str) -> cobolt_forms::model::MenuPaneBackground {
        d.form.layers[d.form.layer_index(name).unwrap()].backdrop.clone()
    }

    /// AC8 (R15) — a new layer is fully transparent, so adding one changes nothing drawn.
    #[test]
    fn a_new_layer_is_fully_transparent() {
        let mut d = DesignerPanel::new(form_with_layers(0));
        d.apply_tab_actions(vec![TabAction::Add]);
        assert_eq!(backdrop(&d, "Layer-1").color, LAYER_TRANSPARENT_COLOR);
    }

    /// AC8, R32 — every backdrop property takes the value written, as one undo step.
    #[test]
    fn each_backdrop_property_is_one_undo_step() {
        let mut d = DesignerPanel::new(form_with_layers(1));
        let cases: [(&str, &str); 8] = [
            ("BackgroundColor", "#336699"),
            ("BackgroundGradientEnabled", "true"),
            ("BackgroundGradientStartColor", "#112233"),
            ("BackgroundGradientEndColor", "#445566"),
            ("BackgroundGradientDirection", "East"),
            ("Transparency", "35"),
            ("BackgroundImage", "assets/clouds.png"),
            ("BackgroundImageMode", "Tile"),
        ];
        let before = backdrop(&d, "Layer-1");
        for (i, (key, value)) in cases.iter().enumerate() {
            d.set_layer_prop("Layer-1", key, value);
            assert_eq!(d.undo_stack.len(), i + 1, "{key}: one step");
        }
        let after = backdrop(&d, "Layer-1");
        assert_eq!(
            (after.color.as_str(), after.gradient_enabled, after.gradient_start_color.as_str(), after.gradient_end_color.as_str()),
            ("#336699", true, "#112233", "#445566")
        );
        assert_eq!((after.gradient_direction.as_str(), after.transparency, after.image.as_str()), ("East", 35, "assets/clouds.png"));
        assert_eq!(after.image_mode.as_str(), "Tile");
        assert!(d.dirty);

        // A value the layer already has is no step.
        d.set_layer_prop("Layer-1", "Transparency", "35");
        assert_eq!(d.undo_stack.len(), cases.len());
        // Undo walks them back one at a time to the transparent layer it began as.
        for _ in 0..cases.len() {
            d.undo();
        }
        assert_eq!(backdrop(&d, "Layer-1"), before);
        d.redo();
        assert_eq!(backdrop(&d, "Layer-1").color, "#336699");
    }

    /// R35, R16 — `Visible`, `Name`, and what a layer does not have are not designed here.
    #[test]
    fn name_visible_and_unknown_properties_are_not_written_as_backdrop() {
        let mut d = DesignerPanel::new(form_with_layers(1));
        for key in ["Visible", "Name", "CornerRadius", "Title", "Colour"] {
            d.set_layer_prop("Layer-1", key, "x");
        }
        assert!(d.undo_stack.is_empty() && !d.dirty, "nothing was written");
        assert_eq!(d.form.layers[0].name, "Layer-1");
        // An unknown layer is no layer.
        d.set_layer_prop("Layer-9", "Transparency", "10");
        assert!(d.undo_stack.is_empty());
    }

    fn inspector_text(layer: Option<cobolt_forms::Layer>) -> Vec<String> {
        let mut panel = PropertiesPanel::new();
        panel.layer_view = layer;
        pane_text(&mut panel, &Form::new("F", "F", 1200, 800), None)
    }

    /// AC8 (R15, R16), Q9 — with a layer in view the pane is the layer's: its name
    /// and its background, with no corner radius, no window property, no `Visible`,
    /// and no Events or Procs tab.
    #[test]
    fn the_inspector_of_a_layer_shows_its_name_and_background_only() {
        let tr = crate::i18n::Language::English.tr();
        let layer = cobolt_forms::Layer::new("Detail");
        let with = inspector_text(Some(layer));
        let has = |t: &[String], s: &str| t.iter().any(|x| x.contains(s));

        for (what, label) in [
            ("its name", tr.lbl_name),
            ("the colour", tr.lbl_back_color),
            ("the gradient", tr.lbl_gradient),
            ("the transparency", tr.lbl_transparency),
            ("the image", tr.lbl_image_path),
            ("the image mode", tr.lbl_img_mode),
        ] {
            assert!(has(&with, label), "the layer pane shows {what} ({label:?}): {with:?}");
        }
        assert!(has(&with, "Detail"), "and the layer's name");
        assert!(has(&with, "SET Detail::Visible TO TRUE"), "and says how a program shows it");
        for (what, label) in [
            ("a corner radius", tr.lbl_corner_radius),
            ("a window title", tr.lbl_title),
            ("a start position", tr.lbl_start_position),
            ("the Events tab", tr.tab_events),
            ("the Procs tab", tr.tab_procs),
        ] {
            assert!(!has(&with, label), "the layer pane has no {what} ({label:?})");
        }
        assert!(!with.iter().any(|t| t.eq_ignore_ascii_case("Visible") || t.starts_with("Visible")), "and no Visible row");

        // Control: with no layer in view the same pane is the form's, tabs included.
        let without = inspector_text(None);
        assert!(has(&without, tr.tab_events) && has(&without, tr.lbl_title), "the form's pane keeps its tabs and window rows");
    }
}

// ── T19: a control's layer, and sending controls to another ──────────────────

mod move_to_layer {
    use super::*;
    use crate::panels::properties::PropertiesPanel;

    fn layered() -> Form {
        let mut f = form_with_layers(2);
        // A docked, anchored Button in the base, and a Panel with a docked child.
        let mut b = Control::new("Button-1", ControlType::Button, 20, 20);
        b.set_prop("Dock", PropValue::String("Left".into()));
        b.set_prop("Anchor", PropValue::String("Top,Right".into()));
        f.controls.push(b);
        let mut p = Control::new("Panel-1", ControlType::Panel, 200, 20);
        p.rect.w = 200;
        p.rect.h = 120;
        f.controls.push(p);
        let mut c = Control::new("Child-1", ControlType::Button, 210, 40);
        c.parent = Some("Panel-1".into());
        c.set_prop("Dock", PropValue::String("Top".into()));
        f.controls.push(c);
        f.controls.push(Control::new("Timer-1", ControlType::Timer, 0, 0));
        f
    }

    fn prop(d: &DesignerPanel, id: &str, key: &str) -> Option<String> {
        d.form.find_control(id).and_then(|c| c.get_prop(key)).map(|v| v.as_str().to_owned())
    }

    /// AC10 (R21, Q7) — into a layer a control loses its Dock and its Anchor
    /// returns to the default; undo gives both back.
    #[test]
    fn moving_a_docked_control_into_a_layer_resets_dock_and_anchor_and_undo_restores_them() {
        let mut d = DesignerPanel::new(layered());
        d.selected_ids = vec!["Button-1".into()];
        d.move_selected_to_layer("Layer-1");
        let b = d.form.find_control("Button-1").unwrap();
        assert_eq!(b.layer.as_deref(), Some("Layer-1"));
        assert_eq!(prop(&d, "Button-1", "Dock").as_deref(), Some("None"), "Dock becomes None");
        assert_eq!(prop(&d, "Button-1", "Anchor").as_deref(), Some("Top,Left"), "Anchor becomes the default");
        assert_eq!(d.undo_stack.len(), 1, "one undo step");

        d.undo();
        let b = d.form.find_control("Button-1").unwrap();
        assert_eq!(b.layer, None);
        assert_eq!(prop(&d, "Button-1", "Dock").as_deref(), Some("Left"), "undo gives the Dock back");
        assert_eq!(prop(&d, "Button-1", "Anchor").as_deref(), Some("Top,Right"), "…and the Anchor");
        d.redo();
        assert_eq!(prop(&d, "Button-1", "Dock").as_deref(), Some("None"));
    }

    /// AC4 (R8) — a Panel takes what is inside it; a child cannot be moved alone.
    #[test]
    fn a_container_takes_its_children_and_a_child_cannot_move_alone() {
        let mut d = DesignerPanel::new(layered());
        d.selected_ids = vec!["Child-1".into()];
        d.move_selected_to_layer("Layer-2");
        assert!(d.undo_stack.is_empty(), "a child cannot be moved on its own");
        assert_eq!(d.form.layer_of("Child-1"), None);

        d.selected_ids = vec!["Panel-1".into()];
        d.move_selected_to_layer("Layer-2");
        assert_eq!(d.form.layer_of("Panel-1"), Some("Layer-2"));
        assert_eq!(d.form.layer_of("Child-1"), Some("Layer-2"), "the child went with the Panel");
        assert_eq!(d.form.find_control("Child-1").unwrap().layer, None, "…and names no layer of its own");
        assert_eq!(prop(&d, "Child-1", "Dock").as_deref(), Some("None"), "its Dock went too: only the base is laid out");
        d.undo();
        assert_eq!(d.form.layer_of("Child-1"), None);
        assert_eq!(prop(&d, "Child-1", "Dock").as_deref(), Some("Top"));
    }

    /// R31, R44 — layer to layer, layer to base; the moved controls leave the
    /// selection; a non-visual control is never moved; the same layer is no step.
    #[test]
    fn controls_move_between_layers_and_leave_the_selection() {
        let mut d = DesignerPanel::new(layered());
        d.tabs.select_form();
        d.selected_ids = vec!["Button-1".into(), "Panel-1".into(), "Timer-1".into()];
        d.move_selected_to_layer("Layer-1");
        assert_eq!(d.form.layer_of("Button-1"), Some("Layer-1"));
        assert_eq!(d.form.layer_of("Panel-1"), Some("Layer-1"));
        assert_eq!(d.form.find_control("Timer-1").unwrap().layer, None, "a Timer belongs to no layer");
        assert_eq!(d.undo_stack.len(), 1, "one step for the lot");
        assert!(!d.selected_ids.contains(&"Button-1".to_string()), "the moved controls left the Form tab's selection");

        // Layer-1 → Layer-2, then back to Form.
        d.tabs.select_layer("Layer-1");
        d.selected_ids = vec!["Button-1".into()];
        d.move_selected_to_layer("layer-2");
        assert_eq!(d.form.layer_of("Button-1"), Some("Layer-2"), "a layer name in any letter case");
        d.tabs.select_layer("Layer-2");
        d.selected_ids = vec!["Button-1".into()];
        d.move_selected_to_layer("Form");
        assert_eq!(d.form.layer_of("Button-1"), None);

        // Nothing to do, nothing recorded.
        let steps = d.undo_stack.len();
        d.tabs.select_form();
        d.selected_ids = vec!["Button-1".into()];
        d.move_selected_to_layer("Form");
        d.move_selected_to_layer("Layer-9");
        assert_eq!(d.undo_stack.len(), steps);
    }

    /// R21, R3, R47 — the inspector: a `Layer` row for a visual control, none for
    /// a non-visual one, none on a form with no layers; a layer control has no
    /// Dock, Anchor or layout rows.
    #[test]
    fn the_inspector_offers_layer_to_visual_controls_and_no_layout_to_layer_controls() {
        let tr = crate::i18n::Language::English.tr();
        let has = |t: &[String], s: &str| t.iter().any(|x| x.contains(s));
        let mut form = layered();
        form.responsive = true; // layout rows exist only on a responsive form
        let mut in_layer = form.clone();
        in_layer.set_control_layer("Button-1", Some("Layer-1")).unwrap();

        let base_button = pane_text(&mut PropertiesPanel::new(), &form, form.find_control("Button-1"));
        assert!(has(&base_button, tr.lbl_layer), "a visual control has a Layer row");
        assert!(has(&base_button, "Form"), "…showing the base's name");
        assert!(has(&base_button, tr.lbl_dock), "and, in the base, a Dock row");

        let layer_button = pane_text(&mut PropertiesPanel::new(), &in_layer, in_layer.find_control("Button-1"));
        assert!(has(&layer_button, tr.lbl_layer) && has(&layer_button, "Layer-1"));
        assert!(!has(&layer_button, tr.lbl_dock), "a layer control is offered no Dock");
        assert!(!has(&layer_button, tr.lbl_anchor), "…no Anchor");
        assert!(has(&layer_button, tr.layer_layout_hint), "…and is told why");
        assert!(has(&layer_button, "Width") && has(&layer_button, "Height"), "X, Y, Width and Height stay");

        let timer = pane_text(&mut PropertiesPanel::new(), &form, form.find_control("Timer-1"));
        assert!(!has(&timer, tr.lbl_layer), "a non-visual control has no Layer (R47)");

        let none = {
            let mut f = layered();
            f.layers.clear();
            pane_text(&mut PropertiesPanel::new(), &f, f.find_control("Button-1"))
        };
        assert!(!has(&none, tr.lbl_layer), "a form with no layers has nothing to choose");
    }
}

// ── T20: the cards — inspector, delete, collisions ───────────────────────────

mod non_visual {
    use super::*;
    use crate::panels::properties::PropertiesPanel;

    /// A Timer with a handler, and a common procedure that mentions only it.
    fn with_handler() -> Form {
        let mut f = form_with_layers(1);
        let mut t = Control::new("Timer-1", ControlType::Timer, 10, 10);
        let mut ev = cobolt_forms::EventBinding::new("onTick", "TIMER-1--ONTICK");
        ev.code = "DISPLAY \"tick\".".into();
        t.events.push(ev);
        f.controls.push(t);
        f.controls.push(Control::new("Timer-2", ControlType::Timer, 10, 10));
        f.user_procedures.push(cobolt_forms::model::UserProcedure {
            name: "RESTART-IT".into(),
            code: "MOVE 1 TO Timer-1::Interval.".into(),
        });
        f
    }

    fn key(k: egui::Key) -> Event {
        Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }
    }

    /// R49, R52, R47 — a card's inspector has Properties, Events and Procs, and no
    /// X, Y, Width, Height, Z order or Layer.
    #[test]
    fn a_card_s_inspector_has_no_geometry_and_no_layer() {
        let tr = crate::i18n::Language::English.tr();
        let has = |t: &[String], s: &str| t.iter().any(|x| x.contains(s));
        let form = with_handler();
        let timer = pane_text(&mut PropertiesPanel::new(), &form, form.find_control("Timer-1"));
        assert!(has(&timer, tr.tab_events) && has(&timer, tr.tab_procs), "the tabs are the control's own");
        for what in ["Width", "Height", "Z order"] {
            assert!(!has(&timer, what), "a card has no {what}: {timer:?}");
        }
        assert!(!has(&timer, tr.sec_geometry), "…and no Geometry section");
        assert!(!has(&timer, tr.lbl_layer), "…and no Layer");
        // Control: a visual control keeps all of it.
        let mut with_button = form.clone();
        with_button.controls.push(Control::new("Button-1", ControlType::Button, 5, 5));
        let button = pane_text(&mut PropertiesPanel::new(), &with_button, with_button.find_control("Button-1"));
        assert!(has(&button, "Width") && has(&button, "Z order") && has(&button, tr.lbl_layer));
    }

    /// AC25 (R53) — Delete on a selected card goes the way any control's Delete
    /// goes: the usual confirmation for one with code, the control and its code
    /// gone together, in the recycle bin, one undo step that brings both back.
    #[test]
    fn delete_on_a_card_asks_removes_the_control_and_its_code_and_undoes_in_one_step() {
        let mut r = Rig::new(with_handler());
        r.select_tab(TabAction::SelectNonVisuals);
        let card = r.card_centre("Timer-1");
        r.click(card);
        assert_eq!(r.d.selected_ids, vec!["Timer-1".to_string()]);

        r.frame(vec![key(egui::Key::Delete)]);
        assert!(r.d.pending_delete.is_some(), "a control that carries code asks for the usual confirmation");
        assert!(r.d.form.find_control("Timer-1").is_some(), "…and nothing is removed before the answer");

        // Confirm, as the dialog's button does.
        let ids = r.d.pending_delete.take().unwrap().control_ids;
        r.d.delete_ids_now(&ids);
        assert!(r.d.form.find_control("Timer-1").is_none(), "the card is gone");
        assert!(
            r.d.form.deleted_code.iter().any(|d| d.control_id == "Timer-1"),
            "its handler went to the form's recycle bin with it"
        );
        assert_eq!(r.d.undo_stack.len(), 1, "one undo step");
        assert!(r.d.form.find_control("Timer-2").is_some(), "the other card stays");

        // A common procedure that only mentions the control stays — and is reported.
        assert!(r.d.form.user_procedures.iter().any(|p| p.name == "RESTART-IT"), "the procedure is kept");
        assert!(
            r.d.orphan_notices.iter().any(|n| n.contains("RESTART-IT") && n.contains("KEPT")),
            "…and the developer is told: {:?}",
            r.d.orphan_notices
        );

        r.d.undo();
        let back = r.d.form.find_control("Timer-1").expect("undo brings the control back");
        assert!(back.events[0].has_code(), "…with its handler");
        assert!(!r.d.form.deleted_code.iter().any(|d| d.control_id == "Timer-1"), "and empties the bin of it");
    }

    /// A card with no code is deleted at once.
    #[test]
    fn delete_on_a_card_without_code_removes_it_at_once() {
        let mut d = DesignerPanel::new(with_handler());
        d.tabs.select_non_visuals();
        d.selected_ids = vec!["Timer-2".into()];
        d.delete_selected();
        assert!(d.pending_delete.is_none());
        assert!(d.form.find_control("Timer-2").is_none());
        assert_eq!(d.undo_stack.len(), 1);
        // The cards close the gap: only Timer-1 is left in the grid.
        assert_eq!(d.non_visuals_view().len(), 1);
    }

    /// AC2 (R5) — a name a layer has is never given to a control, and a control
    /// cannot be renamed to a layer's name.
    #[test]
    fn a_pasted_control_skips_a_layers_name_and_a_rename_to_one_is_refused() {
        let mut form = form_with_layers(0);
        form.controls.push(Control::new("Timer-1", ControlType::Timer, 5, 5));
        form.add_layer().unwrap();
        form.rename_layer("Layer-1", "Timer-2").unwrap();
        let mut d = DesignerPanel::new(form);
        d.selected_ids = vec!["Timer-1".into()];
        let mut clip = None;
        d.copy_selected(&mut clip);
        d.paste_from_clipboard(&clip);
        let ids: Vec<&str> = d.form.controls.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["Timer-1", "Timer-3"], "Timer-2 is a layer, so the copy is Timer-3");

        assert!(!d.rename_control("Timer-3", "timer-2"), "a rename to a layer's name is refused");
        assert!(!d.rename_control("Timer-3", "Timer-1"), "…and to another control's");
        assert!(d.rename_control("Timer-3", "Ticker"), "a free name is fine");
    }
}

// ── T21: what a file can carry is reported, not repaired ─────────────────────

mod load_report {
    use super::*;

    /// AC2, AC13 (R5, R40) — a form that holds a name twice, and a control that
    /// names a layer it does not define, opens with every control kept, says so
    /// once for each, and is not changed.
    #[test]
    fn a_form_with_a_collision_and_an_undefined_layer_opens_whole_and_says_so() {
        let mut form = form_with_layers(0);
        form.add_layer().unwrap();
        form.rename_layer("Layer-1", "Detail").unwrap();
        form.controls.push(Control::new("detail", ControlType::Button, 10, 10)); // a layer's name, in lower case
        let mut ghost = Control::new("Ghost-Ref", ControlType::Button, 100, 10);
        ghost.layer = Some("Ghost".into());
        form.controls.push(ghost);
        // Through the file, as it would arrive.
        let xml = cobolt_forms::form_to_string(&form).unwrap();
        let loaded = cobolt_forms::load_form_from_str(&xml).unwrap();
        assert_eq!(loaded.controls.len(), 2, "both controls came back");

        let mut d = DesignerPanel::new(loaded);
        let before = saved(&d);
        d.report_load_problems();
        assert_eq!(
            d.notices,
            vec![
                DesignerNotice::NameCollision("Detail".into()),
                DesignerNotice::UnknownLayer { control: "Ghost-Ref".into(), layer: "Ghost".into() },
            ]
        );
        assert_eq!(d.form.controls.len(), 2, "nothing was deleted");
        assert_eq!(saved(&d), before, "…and nothing was repaired");
        assert!(!d.dirty && d.undo_stack.is_empty());

        // The control with the unknown layer is on Form, where it is shown.
        d.tabs.select_form();
        assert!(d.active_tab_ids().unwrap().contains("Ghost-Ref"));

        for lang in crate::i18n::Language::ALL {
            for notice in &d.notices {
                let text = notice.text(&lang.tr());
                assert!(!text.contains("{}") && text.contains("Detail") | text.contains("Ghost"), "{lang:?}: {text}");
            }
        }
    }

    /// A form the designer wrote says nothing.
    #[test]
    fn a_clean_form_says_nothing() {
        let mut form = form_with_layers(2);
        form.controls.push(Control::new("Button-1", ControlType::Button, 10, 10));
        let mut d = DesignerPanel::new(form);
        d.report_load_problems();
        assert!(d.notices.is_empty());
    }
}
