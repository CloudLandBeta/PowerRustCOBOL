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
    /// Every accessibility node egui has reported so far (after `ctx.enable_accesskit()`):
    /// where a button is, found by its label rather than by guessing.
    nodes: std::collections::HashMap<egui::accesskit::NodeId, egui::accesskit::Node>,
    /// The clipboard the designer's Cmd+C / Cmd+X / Cmd+V use, kept between frames.
    clipboard: Option<DesignerClipboard>,
}

impl Rig {
    fn new(form: Form) -> Self {
        Self {
            ctx: egui::Context::default(),
            llm: LlmConfig::load_defaults_for_test(),
            d: DesignerPanel::new(form),
            used: Vec2::ZERO,
            mods: egui::Modifiers::NONE,
            nodes: Default::default(),
            clipboard: None,
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
        let clipboard = &mut self.clipboard;
        let mut out = self.ctx.run_ui(input, |root| {
            egui::CentralPanel::default().show_inside(root, |ui| {
                result = d.show(ui, clipboard, &[], llm, None, None);
                used = ui.min_rect().size();
            });
        });
        out.textures_delta.clear();
        if let Some(update) = out.platform_output.accesskit_update.take() {
            self.nodes.extend(update.nodes);
        }
        self.used = used;
        result
    }

    /// The middle of the button labelled `label`, from the accessibility tree.
    fn button_centre(&self, label: &str) -> Option<Pos2> {
        self.nodes
            .values()
            .find(|n| n.role() == egui::accesskit::Role::Button && n.label() == Some(label))
            .and_then(|n| n.bounds())
            .map(|b| Pos2::new(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32))
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

// ── T23, T24: deleting a layer ───────────────────────────────────────────────

mod delete_layer {
    use super::*;
    use cobolt_forms::{BindingSourceDescriptor, BindingTargetDescriptor, DataBindingDef};

    fn handler(event: &str, para: &str, code: &str) -> cobolt_forms::EventBinding {
        let mut ev = cobolt_forms::EventBinding::new(event, para);
        ev.code = code.into();
        ev
    }

    /// Three layers. `Layer-2` holds a Panel with a child, a Button, a ListBox bound to
    /// data, and a Label with no handler; `Layer-1` and the base hold one Button each,
    /// and the base a procedure that mentions one of `Layer-2`'s controls.
    fn fixture() -> Form {
        let mut f = form_with_layers(3);
        let mut push = |id: &str, ct: ControlType, x: i32, layer: Option<&str>, parent: Option<&str>, z: i32| {
            let mut c = Control::new(id, ct, x, 20);
            c.layer = layer.map(str::to_owned);
            c.parent = parent.map(str::to_owned);
            c.z_order = z;
            f.controls.push(c);
        };
        push("Base-Btn", ControlType::Button, 10, None, None, 1);
        push("L1-Btn", ControlType::Button, 20, Some("Layer-1"), None, 2);
        push("L2-Panel", ControlType::Panel, 30, Some("Layer-2"), None, 3);
        push("L2-Child", ControlType::Button, 40, None, Some("L2-Panel"), 4);
        push("L2-Btn", ControlType::Button, 50, Some("Layer-2"), None, 5);
        push("L2-Label", ControlType::Label, 60, Some("Layer-2"), None, 6);
        push("L2-List", ControlType::ListBox, 70, Some("Layer-2"), None, 7);
        push("L3-Btn", ControlType::Button, 80, Some("Layer-3"), None, 8);
        f.find_control_mut("L2-Btn").unwrap().events.push(handler("onClick", "L2-BTN--ONCLICK", "DISPLAY \"two\"."));
        f.find_control_mut("L2-Child").unwrap().events.push(handler("onClick", "L2-CHILD--ONCLICK", "DISPLAY \"child\"."));
        f.find_control_mut("L2-Child").unwrap().events.push(handler("onFocus", "L2-CHILD--ONFOCUS", ""));
        f.find_control_mut("L1-Btn").unwrap().events.push(handler("onClick", "L1-BTN--ONCLICK", "DISPLAY \"one\"."));
        f.user_procedures.push(cobolt_forms::model::UserProcedure {
            name: "SHOW-IT".into(),
            code: "SET L2-Btn::Visible TO TRUE.".into(),
        });
        // …and one that also addresses a control that stays.
        f.user_procedures.push(cobolt_forms::model::UserProcedure {
            name: "SHOW-BOTH".into(),
            code: "SET L2-Btn::Visible TO TRUE. SET Base-Btn::Visible TO TRUE.".into(),
        });
        f.data_bindings.push(DataBindingDef::new(
            "b1",
            "B",
            BindingSourceDescriptor::IndexedFile {
                definition_path: "x.cidx".into(),
                record_name: "R".into(),
                fields: Vec::new(),
                key_field: None,
                writable: false,
            },
            BindingTargetDescriptor::ListBox { control_id: "L2-List".into() },
        ));
        f
    }

    /// AC31 (R63) — the ✕ selects the layer, turns the others off and asks, naming
    /// the layer and the counts; also for a layer with nothing in it.
    #[test]
    fn the_cross_selects_the_layer_and_asks_with_the_counts() {
        let mut d = DesignerPanel::new(fixture());
        d.tabs.select_form();
        d.tabs.set_shown("Layer-1", true);
        d.tabs.set_shown("Layer-3", true);

        d.apply_tab_actions(vec![TabAction::RequestDelete("Layer-2".into())]);
        assert_eq!(d.pending_layer_delete.as_deref(), Some("Layer-2"), "the question is up");
        assert_eq!(d.tabs.active(), &ActiveTab::Layer("Layer-2".into()), "the layer is the active tab");
        assert!(d.tabs.is_shown("Layer-2") && !d.tabs.is_shown("Layer-1") && !d.tabs.is_shown("Layer-3"), "the others are off (R61)");
        assert!(d.has_blocking_modal(), "nothing behind it reacts");
        assert_eq!(d.form.layers.len(), 3, "and nothing is deleted yet");
        // Panel, its child, Button, Label, ListBox = 5; two handlers with code (the third has none).
        assert_eq!(d.layer_delete_summary("Layer-2"), (5, 2));

        let mut empty = DesignerPanel::new(form_with_layers(1));
        empty.apply_tab_actions(vec![TabAction::RequestDelete("Layer-1".into())]);
        assert_eq!(empty.pending_layer_delete.as_deref(), Some("Layer-1"), "asked even for an empty layer");
        assert_eq!(empty.layer_delete_summary("Layer-1"), (0, 0), "both numbers read zero");
    }

    /// AC31 — the window is one size in every language and on every frame, and its
    /// text fits inside it; cancelling leaves it all as it was.
    #[test]
    fn the_confirmation_window_is_one_size_in_every_language_and_cancel_changes_nothing() {
        let mut sizes: Vec<(String, egui::Vec2)> = Vec::new();
        for lang in crate::i18n::Language::ALL {
            let mut r = Rig::new(fixture());
            crate::i18n::set_language(&r.ctx, *lang);
            let before = saved(&r.d);
            r.d.apply_tab_actions(vec![TabAction::RequestDelete("Layer-2".into())]);
            let win_id = egui::Id::new("designer_layer_delete_confirm");
            let mut seen: Vec<egui::Vec2> = Vec::new();
            for _ in 0..6 {
                r.frame(vec![]);
                let rect = r.ctx.memory(|m| m.area_rect(win_id)).expect("the window is open");
                seen.push(rect.size());
            }
            assert!(seen.windows(2).all(|w| w[0] == w[1]), "{lang:?}: no frame changes it: {seen:?}");
            sizes.push((format!("{lang:?}"), seen[0]));

            // Cancel (the question goes away) and nothing is touched.
            r.d.pending_layer_delete = None;
            r.settle(2);
            assert_eq!(saved(&r.d), before, "{lang:?}: cancelling deletes nothing");
            assert_eq!(r.d.tabs.active(), &ActiveTab::Layer("Layer-2".into()), "{lang:?}: and the layer stays active");
            assert!(r.d.undo_stack.is_empty());
        }
        let first = sizes[0].1;
        println!("  AC31: the window is {first:?} in {} languages", sizes.len());
        assert!(sizes.iter().all(|(_, s)| *s == first), "the same size in every language: {sizes:?}");

        // …and for a layer with a name far longer than the window is wide.
        let mut r = Rig::new(fixture());
        let long = "X".repeat(160);
        r.d.rename_layer_by_bar("Layer-2", &long);
        assert_eq!(r.d.form.layers[1].name, long, "the long name is a layer's name");
        r.d.apply_tab_actions(vec![TabAction::RequestDelete(long.clone())]);
        r.settle(4);
        let win = r.ctx.memory(|m| m.area_rect(egui::Id::new("designer_layer_delete_confirm"))).unwrap();
        assert_eq!(win.size(), first, "a 160-character name does not change the window: {:?} against {first:?}", win.size());
    }

    /// AC32 (R64) — confirming deletes the layer, its controls (a Panel's contents
    /// too) and the handlers bound to them; a procedure that only mentions one
    /// stays and is reported; Form is active; the other layers' state is as it was.
    #[test]
    fn confirming_deletes_the_layer_its_controls_and_their_handlers_and_keeps_the_procedures() {
        let mut d = DesignerPanel::new(fixture());
        d.apply_tab_actions(vec![TabAction::RequestDelete("Layer-2".into())]);
        d.delete_layer_now("Layer-2");
        d.pending_layer_delete = None;

        let layers: Vec<&str> = d.form.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(layers, ["Layer-1", "Layer-3"], "the layer is gone, the others keep their order");
        for gone in ["L2-Panel", "L2-Child", "L2-Btn", "L2-Label", "L2-List"] {
            assert!(d.form.find_control(gone).is_none(), "{gone} went with the layer");
        }
        for stays in ["Base-Btn", "L1-Btn", "L3-Btn"] {
            assert!(d.form.find_control(stays).is_some(), "{stays} is another tab's and stays");
        }
        assert!(d.form.deleted_code.iter().any(|c| c.control_id == "L2-Btn"), "its handler is in the recycle bin");
        assert!(d.form.deleted_code.iter().any(|c| c.control_id == "L2-Child"), "…and so is the child's");
        assert!(d.form.data_bindings.is_empty(), "the binding that named the ListBox is pruned");
        // The procedure that only mentions a deleted control is kept — and reported.
        assert!(d.form.user_procedures.iter().any(|p| p.name == "SHOW-IT"), "SHOW-IT is kept");
        assert!(d.orphan_notices.iter().any(|n| n.contains("SHOW-IT") && n.contains("KEPT")), "{:?}", d.orphan_notices);
        // One that also addresses a control that stays is kept too, and named.
        assert!(d.form.user_procedures.iter().any(|p| p.name == "SHOW-BOTH"), "SHOW-BOTH is kept");
        assert!(
            d.notices.contains(&DesignerNotice::ProcedureStillRefers { procedure: "SHOW-BOTH".into(), control: "L2-Btn".into() }),
            "the Output panel names it: {:?}",
            d.notices
        );
        assert!(
            !d.notices.iter().any(|n| matches!(n, DesignerNotice::ProcedureStillRefers { procedure, .. } if procedure == "SHOW-IT")),
            "SHOW-IT, which addresses nothing that exists, has the stronger report and not this one"
        );
        for lang in crate::i18n::Language::ALL {
            let text = DesignerNotice::ProcedureStillRefers { procedure: "SHOW-BOTH".into(), control: "L2-Btn".into() }.text(&lang.tr());
            assert!(text.contains("SHOW-BOTH") && text.contains("L2-Btn") && !text.contains("{}"), "{lang:?}: {text}");
        }
        assert_eq!(d.tabs.active(), &ActiveTab::Form, "Form is the active tab afterwards");
        assert!(d.selected_ids.is_empty());
        // The next generation has no layer and no deleted control.
        let cbl = cobolt_codegen::generate(&d.form);
        assert!(!cbl.contains("L2-BTN") && !cbl.to_ascii_uppercase().contains("L2-BTN--ONCLICK"), "the handler is not generated any more");
    }

    /// AC33 (R65) — one Undo brings back everything, so the COBOL generated is what
    /// it was; one Redo deletes it again; the stack grew by one step.
    #[test]
    fn one_undo_brings_everything_back_and_one_redo_deletes_it_again() {
        let mut d = DesignerPanel::new(fixture());
        let form_before = d.form.clone();
        let cobol_before = cobolt_codegen::generate(&d.form);
        let saved_before = saved(&d);

        d.delete_layer_now("Layer-2");
        assert_eq!(d.undo_stack.len(), 1, "one step for the whole deletion");

        d.undo();
        d.tabs.reconcile(&d.form);
        assert_eq!(d.form.layers, form_before.layers, "the layer is back at its place, with its backdrop");
        for before in &form_before.controls {
            let after = d.form.find_control(&before.id).unwrap_or_else(|| panic!("{} is back", before.id));
            assert_eq!(after.rect, before.rect, "{}: rect", before.id);
            assert_eq!(after.z_order, before.z_order, "{}: ZOrder", before.id);
            assert_eq!((&after.parent, &after.layer), (&before.parent, &before.layer), "{}: container and layer", before.id);
            assert_eq!(after.events.len(), before.events.len(), "{}: handlers", before.id);
            assert!(
                after.events.iter().zip(&before.events).all(|(a, b)| a.code == b.code),
                "{}: handler code is as it was",
                before.id
            );
        }
        assert_eq!(d.form.controls.len(), form_before.controls.len());
        assert_eq!(d.form.data_bindings, form_before.data_bindings, "the binding is back too");
        assert!(d.form.deleted_code.is_empty(), "and the recycle bin holds none of it");
        assert_eq!(saved(&d), saved_before, "the saved form is what it was");
        assert_eq!(cobolt_codegen::generate(&d.form), cobol_before, "so is the generated COBOL");

        d.redo();
        assert_eq!(d.form.layers.len(), 2);
        assert!(d.form.find_control("L2-Btn").is_none() && d.form.data_bindings.is_empty());
        assert_eq!(d.undo_stack.len(), 1);
    }

    /// The window's buttons are real: Cancel keeps everything, Delete deletes.
    #[test]
    fn the_windows_buttons_cancel_and_delete() {
        let tr = crate::i18n::Language::English.tr();
        for press_delete in [false, true] {
            let mut r = Rig::new(fixture());
            r.ctx.enable_accesskit();
            r.settle(2);
            r.d.apply_tab_actions(vec![TabAction::RequestDelete("Layer-2".into())]);
            r.settle(3);
            let label = if press_delete { tr.delete_confirm_ok } else { tr.delete_confirm_cancel };
            let at = r.button_centre(label).unwrap_or_else(|| panic!("a {label:?} button is on the window"));
            r.click(at);
            assert!(r.d.pending_layer_delete.is_none(), "{label}: the question is closed");
            if press_delete {
                assert_eq!(r.d.form.layers.len(), 2, "Delete deleted the layer");
                assert_eq!(r.d.undo_stack.len(), 1);
                assert_eq!(r.d.tabs.active(), &ActiveTab::Form);
            } else {
                assert_eq!(r.d.form.layers.len(), 3, "Cancel left it");
                assert!(r.d.undo_stack.is_empty());
                assert_eq!(r.d.tabs.active(), &ActiveTab::Layer("Layer-2".into()), "…and its tab active");
            }
        }
    }

    /// The red ✕ on the bar is a real button: a click selects the layer and asks.
    #[test]
    fn clicking_the_red_cross_on_a_tab_asks() {
        let mut r = Rig::new(fixture());
        r.settle(3);
        let cross = layer_tab_parts(r.slot_rect(Slot::Layer(1))).cross.center();
        r.click(cross);
        assert_eq!(r.d.pending_layer_delete.as_deref(), Some("Layer-2"));
        assert_eq!(r.d.tabs.active(), &ActiveTab::Layer("Layer-2".into()));
        assert_eq!(r.d.form.layers.len(), 3, "and nothing is deleted until the developer confirms");
    }

    /// A layer that vanishes under the question (an undo) leaves nothing to ask.
    #[test]
    fn a_layer_that_vanishes_under_the_question_closes_it() {
        let mut r = Rig::new(fixture());
        r.d.apply_tab_actions(vec![TabAction::RequestDelete("Layer-2".into())]);
        r.d.form.layers.remove(1);
        r.settle(2);
        assert!(r.d.pending_layer_delete.is_none(), "nothing left to ask about");
    }
}

// ── T28: what layers cost (AC15) ─────────────────────────────────────────────

/// A measurement, not a check: 3,200 controls in the base against the same 3,200
/// spread over 64 layers of 50, the designer's frame and its hit-test timed. Run
/// with `cargo test -p cobolt-ide --release --bin cobolt-ide layers_091::bench --
/// --ignored --nocapture` — a debug build reports the same ratios, slower.
mod bench {
    use super::*;
    use std::time::Instant;

    const LAYERS: usize = 64;
    const PER_LAYER: usize = 50;
    const FRAMES: usize = 12;
    const HITS: usize = 4000;

    fn button(id: String, x: i32, y: i32, layer: Option<&str>) -> Control {
        let mut c = Control::new(id, ControlType::Button, x, y);
        c.rect.w = 40;
        c.rect.h = 14;
        c.layer = layer.map(str::to_owned);
        c
    }

    /// 3,200 buttons in the base.
    fn all_in_the_base() -> Form {
        let mut f = Form::new("F", "F", 1200, 800);
        for i in 0..LAYERS * PER_LAYER {
            f.controls.push(button(format!("B-{i}"), (i % 64) as i32 * 18, (i / 64) as i32 * 15, None));
        }
        f
    }

    /// The same 3,200, 50 to a layer, in 64 layers.
    fn spread_over_layers() -> Form {
        let mut f = form_with_layers(LAYERS);
        for l in 0..LAYERS {
            let name = format!("Layer-{}", l + 1);
            for j in 0..PER_LAYER {
                f.controls.push(button(format!("L{l}-{j}"), (j % 25) as i32 * 46, (j / 25) as i32 * 16 + l as i32 * 3, Some(&name)));
            }
        }
        f
    }

    struct Timing {
        frame_avg_ms: f64,
        frame_max_ms: f64,
        hit_avg_us: f64,
    }

    fn measure(rig: &mut Rig) -> Timing {
        rig.settle(3); // warm-up: textures, fonts, caches
        let mut frames = Vec::with_capacity(FRAMES);
        for _ in 0..FRAMES {
            let t = Instant::now();
            rig.frame(vec![]);
            frames.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        // Pseudo-random points over the form, the same sequence for every case.
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let points: Vec<(i32, i32)> = (0..HITS).map(|_| ((next() % 1200) as i32, (next() % 800) as i32)).collect();
        let t = Instant::now();
        let mut found = 0usize;
        for &(x, y) in &points {
            if rig.d.hit_top_id(x, y).is_some() {
                found += 1;
            }
        }
        let hit_total = t.elapsed().as_secs_f64() * 1e6;
        std::hint::black_box(found);
        Timing {
            frame_avg_ms: frames.iter().sum::<f64>() / frames.len() as f64,
            frame_max_ms: frames.iter().cloned().fold(0.0, f64::max),
            hit_avg_us: hit_total / HITS as f64,
        }
    }

    #[test]
    #[ignore = "a measurement: run with --release -- --ignored --nocapture"]
    fn sixty_four_layers_of_fifty_controls_against_three_thousand_two_hundred_in_the_base() {
        let mut base = Rig::new(all_in_the_base());
        base.d.tabs.select_form();
        let t_base = measure(&mut base);

        // All 64 layers shown over an empty `Form`, the active tab `Form`: the whole
        // 3,200 are drawn; the pointer asks about the base.
        let mut shown = Rig::new(spread_over_layers());
        for l in 0..LAYERS {
            shown.d.tabs.set_shown(&format!("Layer-{}", l + 1), true);
        }
        shown.d.tabs.select_form();
        let t_shown = measure(&mut shown);

        // One layer being edited, as the designer is used: it alone is drawn, and the
        // pointer asks about its 50.
        let mut one = Rig::new(spread_over_layers());
        one.d.apply_tab_actions(vec![TabAction::SelectLayer("Layer-32".into())]);
        let t_one = measure(&mut one);

        println!("  AC15 — what 64 layers cost, {FRAMES} frames and {HITS} hit-tests each");
        println!("  {:<46} {:>9} {:>12} {:>12} {:>12}", "case", "controls", "frame avg ms", "frame max ms", "hit avg us");
        for (case, n, t) in [
            ("3,200 controls in the base (Form active)", 3200, &t_base),
            ("64 layers x 50, all shown (Form active)", 3200, &t_shown),
            ("64 layers x 50, Layer-32 active (edited alone)", 50, &t_one),
        ] {
            println!("  {case:<46} {n:>9} {:>12.2} {:>12.2} {:>12.2}", t.frame_avg_ms, t.frame_max_ms, t.hit_avg_us);
        }
        println!(
            "  all 64 shown against the base: frame x{:.2}; one layer edited against the base: frame x{:.2}, hit-test x{:.3}",
            t_shown.frame_avg_ms / t_base.frame_avg_ms,
            t_one.frame_avg_ms / t_base.frame_avg_ms,
            t_one.hit_avg_us / t_base.hit_avg_us,
        );
        // A guard, not a budget: the first measurement was x4.47 (the expand-icon pass
        // ordered every control once per layer), and 3 is far enough above the x1.0
        // that was measured after the fix to be steady under noise and below that.
        assert!(
            t_shown.frame_avg_ms < 3.0 * t_base.frame_avg_ms,
            "64 layers must not cost three times what the same controls cost in the base: {:.1} ms against {:.1} ms",
            t_shown.frame_avg_ms,
            t_base.frame_avg_ms
        );
    }
}

// ── T31, T32: the bar as the mock-up, the eye ────────────────────────────────

mod look {
    use super::*;
    use crate::panels::layer_tabs::{LayerTabs, ACTIVE_FACE, STRIP};
    use egui::{Color32, ColorImage};

    const WIDTH: f32 = 560.0;

    /// The bar rendered to pixels: a form with `Layer-1` and `Layer-2`, the tab
    /// `active` selected, `Layer-1` shown or not.
    fn picture(active: &ActiveTab, layer_one_shown: bool) -> ColorImage {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::fonts::base_font_definitions());
        let tr = crate::i18n::Language::English.tr();
        let form = form_with_layers(2);
        let mut tabs = LayerTabs::default();
        match active {
            ActiveTab::NonVisuals => tabs.select_non_visuals(),
            ActiveTab::Form => tabs.select_form(),
            ActiveTab::Layer(n) => tabs.select_layer(n),
        }
        tabs.set_shown("Layer-1", layer_one_shown);
        let mut raster = cobolt_forms::raster::Rasterizer::new();
        let mut img = None;
        for time in [0.0, 0.5, 1.0] {
            img = Some(cobolt_forms::raster::render_frame(
                &ctx,
                &mut raster,
                Vec2::new(WIDTH, BAR_H),
                Color32::from_rgb(20, 24, 34),
                time,
                |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let _ = tabs.show_bar(ui, &form, &tr);
                },
            ));
        }
        img.unwrap()
    }

    fn at(img: &ColorImage, x: usize, y: usize) -> Color32 {
        img.pixels[y * img.size[0] + x]
    }

    fn near(a: Color32, b: Color32) -> bool {
        (a.r() as i32 - b.r() as i32).abs() < 12 && (a.g() as i32 - b.g() as i32).abs() < 12 && (a.b() as i32 - b.b() as i32).abs() < 12
    }

    /// The leftmost and rightmost pixel of `face` in a row.
    fn extent(img: &ColorImage, y: usize, face: Color32) -> Option<(usize, usize)> {
        let xs: Vec<usize> = (0..img.size[0]).filter(|&x| near(at(img, x, y), face)).collect();
        Some((*xs.first()?, *xs.last()?))
    }

    fn save(img: &ColorImage, name: &str) -> std::path::PathBuf {
        let out = std::env::temp_dir().join(name);
        std::fs::write(&out, cobolt_forms::raster::to_png(img).unwrap()).unwrap();
        out
    }

    /// AC34 (R66) — the mock-up's colours and lean, measured on the picture.
    #[test]
    fn the_bar_is_drawn_as_the_mock_up() {
        let img = picture(&ActiveTab::NonVisuals, true);
        let out = save(&img, "prc-091-tab-bar-non-visuals.png");
        let (w, h) = (img.size[0], img.size[1]);
        println!("tab bar picture {w}x{h} -> {}", out.display());

        // The strip, in the far corner past the tabs and the plus.
        assert!(near(at(&img, w - 3, h - 3), STRIP), "the strip is #50504E: {:?}", at(&img, w - 3, h - 3));
        // The white rule along the top, with black above and below it.
        assert!(near(at(&img, w - 3, 0), Color32::BLACK), "black above the rule");
        assert!(near(at(&img, w - 3, 1), Color32::WHITE) && near(at(&img, w - 3, 2), Color32::WHITE), "the white rule, two points thick");
        assert!(near(at(&img, w - 3, 3), Color32::BLACK), "black below the rule");

        // The first tab: active blue, a trapezoid — wider at the top than at its foot,
        // its left edge leaning `\` and its right edge `/`.
        let row_top = 5;
        let row_foot = (BAR_H as usize) - 5;
        let (tl, tr) = extent(&img, row_top, ACTIVE_FACE).expect("blue at the top of the first tab");
        let (bl, br) = extent(&img, row_foot, ACTIVE_FACE).expect("blue at the foot of the first tab");
        assert!(bl > tl + 4, "the left edge leans \\ : x {tl} at the top, {bl} at the foot");
        assert!(br + 4 < tr, "the right edge leans / : x {tr} at the top, {br} at the foot");

        // A tab after the first leans `/` on both edges: with `Form` active, its blue
        // starts further left at its foot than at its top.
        let form_active = picture(&ActiveTab::Form, true);
        save(&form_active, "prc-091-tab-bar-form.png");
        let (fl_top, fr_top) = extent(&form_active, row_top, ACTIVE_FACE).expect("blue at the top of Form");
        let (fl_foot, fr_foot) = extent(&form_active, row_foot, ACTIVE_FACE).expect("blue at the foot of Form");
        assert!(fl_foot + 4 < fl_top, "Form's left edge leans / : {fl_top} at the top, {fl_foot} at the foot");
        assert!(fr_foot + 4 < fr_top, "Form's right edge leans / : {fr_top} at the top, {fr_foot} at the foot");
        // …and it starts where the first tab ends: the tabs touch.
        let (_, nv_right_top) = extent(&img, row_top, ACTIVE_FACE).unwrap();
        assert!(fl_top.abs_diff(nv_right_top) <= 3, "Form's top starts where Non-Visuals' ends: {fl_top} against {nv_right_top}");

        // Inactive tabs are white, and the `+` is white on the strip.
        assert!(
            (0..w).any(|x| near(at(&img, x, BAR_H as usize / 2), Color32::WHITE)),
            "white faces on the inactive tabs"
        );
        let plus_x = (w / 2..w).filter(|&x| near(at(&img, x, BAR_H as usize / 2), Color32::WHITE)).count();
        assert!(plus_x >= 8, "the plus is white outside the tabs: {plus_x} px in the middle row to the right of the middle");
    }

    /// AC34 — the same pixels under every theme in the registry.
    #[test]
    fn the_bar_is_the_same_under_every_theme() {
        let reference = picture(&ActiveTab::Layer("Layer-1".into()), true).pixels;
        for theme in crate::theme::THEMES {
            crate::theme::set_active(theme);
            let img = picture(&ActiveTab::Layer("Layer-1".into()), true);
            assert!(img.pixels == reference, "theme {} changed the bar", theme.id);
        }
        crate::theme::set_active(crate::theme::default_theme());
        println!("tab bar: identical under all {} themes", crate::theme::THEMES.len());
    }

    /// AC35 (R67) — an open eye while the layer is shown, a closed one while it is
    /// hidden; the two differ, and neither is the old box.
    #[test]
    fn the_eye_is_open_when_the_layer_is_shown_and_closed_when_it_is_hidden() {
        let shown = picture(&ActiveTab::Form, true);
        let hidden = picture(&ActiveTab::Form, false);
        save(&shown, "prc-091-tab-bar-eye-open.png");
        save(&hidden, "prc-091-tab-bar-eye-closed.png");
        // Where Layer-1's eye is, from the layout the painter used.
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::fonts::base_font_definitions());
        ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear(); // the fonts exist after a first frame
        let tr = crate::i18n::Language::English.tr();
        let form = form_with_layers(2);
        let layout = layout_for(&ctx, &form, &tr, WIDTH, 0.0);
        let p = layout.placed.iter().find(|p| p.slot == Slot::Layer(0)).unwrap();
        let bar = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(WIDTH, BAR_H));
        let eye = layer_tab_parts(placed_rect(bar, p)).check;
        let (x0, x1, y0, y1) = (eye.min.x as usize, eye.max.x as usize + 1, eye.min.y as usize, eye.max.y as usize + 1);
        let differing = (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&(x, y)| !near(at(&shown, x, y), at(&hidden, x, y)))
            .count();
        let dark = |img: &ColorImage| (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).filter(|&(x, y)| at(img, x, y).r() < 100).count();
        println!("eye: {differing} px differ between open and closed; ink px {} open, {} closed", dark(&shown), dark(&hidden));
        assert!(differing >= 15, "the open and the closed eye are different drawings: {differing} px");
        assert!(dark(&shown) >= 10 && dark(&hidden) >= 8, "both are drawn in dark ink on the white tab");
        assert!(dark(&shown) > dark(&hidden) + 10, "the open eye (almond and iris) carries more ink than the closed lid: {} against {}", dark(&shown), dark(&hidden));
        // Nothing of the old box: its black frame ran along the eye's whole outline.
        let corner_dark = [(x0, y0), (x1 - 1, y0), (x0, y1 - 1), (x1 - 1, y1 - 1)]
            .iter()
            .filter(|&&(x, y)| at(&shown, x, y).r() < 100)
            .count();
        assert_eq!(corner_dark, 0, "no square frame: the corners of the eye's box are empty");
    }
}

// ── T33: the Objects list follows the tab ────────────────────────────────────

mod objects {
    use super::*;
    use crate::panels::objects_list::{object_rows_in, show};

    fn fixture() -> Form {
        let mut f = form_with_layers(2);
        let mut add = |id: &str, ct: ControlType, layer: Option<&str>, parent: Option<&str>| {
            let mut c = Control::new(id, ct, 10, 10);
            c.layer = layer.map(str::to_owned);
            c.parent = parent.map(str::to_owned);
            f.controls.push(c);
        };
        add("Base-Btn", ControlType::Button, None, None);
        add("L1-Panel", ControlType::Panel, Some("Layer-1"), None);
        add("L1-Child", ControlType::Button, None, Some("L1-Panel"));
        add("L2-Btn", ControlType::Button, Some("Layer-2"), None);
        add("Timer-1", ControlType::Timer, None, None);
        f
    }

    fn ids(d: &DesignerPanel) -> Vec<(String, usize)> {
        object_rows_in(&d.form, d.active_tab_ids().as_ref()).into_iter().map(|r| (r.id, r.depth)).collect()
    }

    /// AC36 (R68) — each tab lists its own controls, containers indented as before.
    #[test]
    fn each_tab_lists_only_its_own_controls() {
        let mut d = DesignerPanel::new(fixture());
        d.tabs.select_form();
        assert_eq!(ids(&d), [("Base-Btn".to_string(), 0)], "Form: the base's control, not the layers' nor the Timer");
        d.tabs.select_layer("Layer-1");
        assert_eq!(
            ids(&d),
            [("L1-Panel".to_string(), 0), ("L1-Child".to_string(), 1)],
            "Layer-1: its Panel and the child inside it, indented"
        );
        d.tabs.select_layer("Layer-2");
        assert_eq!(ids(&d), [("L2-Btn".to_string(), 0)]);
        d.tabs.select_non_visuals();
        assert_eq!(ids(&d), [("Timer-1".to_string(), 0)], "Non-Visuals: the cards");
        // A layer whose box is off cannot be reached, so it lists nothing.
        d.tabs.select_layer("Layer-2");
        d.tabs.set_shown("Layer-2", false);
        assert!(ids(&d).is_empty());
        // A form with no layers and no non-visual control: every control, as before.
        let mut plain = Form::new("F", "F", 640, 480);
        plain.controls.push(Control::new("A", ControlType::Button, 0, 0));
        plain.controls.push(Control::new("B", ControlType::Label, 0, 0));
        let p = DesignerPanel::new(plain);
        assert_eq!(object_rows_in(&p.form, p.active_tab_ids().as_ref()).len(), 2);
    }

    /// The drawn list: the names of this tab's controls and nothing of another's.
    #[test]
    fn the_drawn_list_names_only_the_active_tabs_controls() {
        let mut d = DesignerPanel::new(fixture());
        let tr = crate::i18n::Language::English.tr();
        let drawn = |d: &DesignerPanel| -> String {
            let ctx = egui::Context::default();
            ctx.set_fonts(crate::fonts::base_font_definitions());
            let members = d.active_tab_ids();
            let mut raster = cobolt_forms::raster::Rasterizer::new();
            let mut texts: Vec<String> = Vec::new();
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 300.0)));
            for time in [0.0, 0.5] {
                input.time = Some(time);
                let mut out = ctx.run_ui(input.clone(), |root| {
                    egui::CentralPanel::default().show_inside(root, |ui| {
                        let _ = show(ui, &d.form, members.as_ref(), &d.selected_ids, 250.0, &tr);
                    });
                });
                raster.apply(&out.textures_delta);
                out.textures_delta.clear();
                texts.clear();
                fn painted(shape: &egui::Shape, out: &mut Vec<String>) {
                    match shape {
                        egui::Shape::Text(t) => out.push(t.galley.text().to_owned()),
                        egui::Shape::Vec(v) => v.iter().for_each(|s| painted(s, out)),
                        _ => {}
                    }
                }
                for c in &out.shapes {
                    painted(&c.shape, &mut texts);
                }
            }
            texts.join("|")
        };
        d.tabs.select_layer("Layer-1");
        let layer1 = drawn(&d);
        assert!(layer1.contains("L1-Panel") && layer1.contains("L1-Child"), "Layer-1's own: {layer1}");
        assert!(!layer1.contains("Base-Btn") && !layer1.contains("L2-Btn") && !layer1.contains("Timer-1"), "none of the others: {layer1}");
        d.tabs.select_form();
        let form_tab = drawn(&d);
        assert!(form_tab.contains("Base-Btn") && !form_tab.contains("L1-Panel"), "{form_tab}");
    }
}

// ── T34, T35: a tab order for each tab; copy here, paste there ───────────────

mod tab_order {
    use super::*;

    fn button(id: &str, layer: Option<&str>, order: u32) -> Control {
        let mut c = Control::new(id, ControlType::Button, 10, 10);
        c.layer = layer.map(str::to_owned);
        c.tab_order = order;
        c
    }

    /// Base 1–3, `Layer-1` 1–2, `Layer-2` 1: each tab numbered from 1 on its own.
    fn fixture() -> Form {
        let mut f = form_with_layers(2);
        for c in [
            button("Base-A", None, 1),
            button("Base-B", None, 2),
            button("Base-C", None, 3),
            button("L1-A", Some("Layer-1"), 1),
            button("L1-B", Some("Layer-1"), 2),
            button("L2-A", Some("Layer-2"), 1),
        ] {
            f.controls.push(c);
        }
        f
    }

    fn number(d: &DesignerPanel, id: &str) -> u32 {
        d.form.find_control(id).unwrap().tab_order
    }

    fn numbers(d: &DesignerPanel, ids: &[&str]) -> Vec<u32> {
        ids.iter().map(|id| number(d, id)).collect()
    }

    /// AC37 (R69) — a new control takes the next number in ITS tab.
    #[test]
    fn a_new_control_takes_the_next_number_in_its_own_tab() {
        let mut d = DesignerPanel::new(fixture());
        d.tabs.select_layer("Layer-2");
        d.add_control(ControlType::Button, 100, 100);
        let made = d.form.controls.last().unwrap().id.clone();
        assert_eq!(number(&d, &made), 2, "Layer-2 held 1, so its next is 2 — not 7");
        d.tabs.select_form();
        d.add_control(ControlType::Button, 200, 100);
        assert_eq!(number(&d, &d.form.controls.last().unwrap().id.clone()), 4, "Form held 1–3");
        d.tabs.select_layer("Layer-1");
        d.add_control(ControlType::Button, 300, 100);
        assert_eq!(number(&d, &d.form.controls.last().unwrap().id.clone()), 3);
    }

    /// AC37 — the Tab Order list shows and renumbers the active tab's controls alone.
    #[test]
    fn the_tab_order_list_covers_the_active_tab_only() {
        let mut d = DesignerPanel::new(fixture());
        d.tabs.select_layer("Layer-1");
        d.open_tab_order_list();
        let modal = d.tab_order_modal.as_mut().expect("the list is open");
        assert_eq!(modal.order(), ["L1-A", "L1-B"], "Layer-1's two, not the form's six");
        modal.move_row(0, 1); // L1-B first
        let order = modal.order();
        d.tab_order_modal = None;
        d.apply_tab_order(&order);
        assert_eq!(numbers(&d, &["L1-B", "L1-A"]), [1, 2]);
        assert_eq!(numbers(&d, &["Base-A", "Base-B", "Base-C"]), [1, 2, 3], "the base is as it was");
        assert_eq!(number(&d, "L2-A"), 1, "and so is Layer-2");
        assert_eq!(d.undo_stack.len(), 1, "one undo step");
        d.undo();
        assert_eq!(numbers(&d, &["L1-A", "L1-B"]), [1, 2]);
    }

    /// AC37 — and so does Visual Tab Order.
    #[test]
    fn visual_tab_order_covers_the_active_tab_only() {
        let mut d = DesignerPanel::new(fixture());
        d.tabs.select_form();
        d.toggle_visual_tab_order();
        {
            let v = d.tab_order_visual.as_mut().expect("the mode is on");
            assert_eq!(v.order, ["Base-A", "Base-B", "Base-C"], "the base's three");
            assert!(!v.click("L1-A"), "a control of another tab is not part of this tab's order");
            v.click("Base-C");
            v.click("Base-A");
        }
        d.toggle_visual_tab_order(); // off: written
        assert_eq!(numbers(&d, &["Base-C", "Base-A", "Base-B"]), [1, 2, 3]);
        assert_eq!(numbers(&d, &["L1-A", "L1-B", "L2-A"]), [1, 2, 1], "no other tab's number moved");
    }

    /// AC37 (R69), AC38 — what is pasted takes the next places in the target tab, in
    /// the order it had.
    #[test]
    fn a_paste_takes_the_next_places_in_the_target_tab() {
        let mut d = DesignerPanel::new(fixture());
        d.tabs.select_layer("Layer-1");
        d.selected_ids = vec!["L1-B".into(), "L1-A".into()];
        let mut clip = None;
        d.copy_selected(&mut clip);
        d.tabs.select_layer("Layer-2");
        d.paste_from_clipboard(&clip);
        let pasted: Vec<&Control> = d.form.controls.iter().filter(|c| c.id.starts_with("Button-")).collect();
        assert_eq!(pasted.len(), 2);
        // L1-A (1) then L1-B (2) in the order they had, after Layer-2's own 1: 2 then 3.
        let by_number: Vec<u32> = pasted.iter().map(|c| c.tab_order).collect();
        assert_eq!({ let mut v = by_number.clone(); v.sort(); v }, [2, 3], "after Layer-2's 1: {by_number:?}");
        assert_eq!(d.form.layer_of(&pasted[0].id), Some("Layer-2"));
    }

    /// AC37 — sending controls to a layer gives them the next places there, and
    /// undo gives the old numbers back.
    #[test]
    fn controls_sent_to_a_layer_take_its_next_places_and_undo_restores_them() {
        let mut d = DesignerPanel::new(fixture());
        d.tabs.select_form();
        d.selected_ids = vec!["Base-B".into(), "Base-A".into()];
        d.move_selected_to_layer("Layer-1");
        assert_eq!(numbers(&d, &["Base-A", "Base-B"]), [3, 4], "after Layer-1's 1 and 2, in the order they had");
        assert_eq!(number(&d, "Base-C"), 3, "the base keeps its own");
        d.undo();
        assert_eq!(numbers(&d, &["Base-A", "Base-B"]), [1, 2]);
        assert_eq!(d.form.layer_of("Base-A"), None);
    }
}

mod clipboard {
    use super::*;

    impl Rig {
        fn cmd_key(&mut self, key: egui::Key) {
            self.mods = egui::Modifiers::COMMAND;
            self.frame(vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND }]);
            self.mods = egui::Modifiers::NONE;
            self.frame(vec![]);
        }
    }

    fn fixture() -> Form {
        let mut f = form_with_layers(2);
        let mut panel = Control::new("L1-Panel", ControlType::Panel, 20, 20);
        panel.rect.w = 200;
        panel.rect.h = 120;
        panel.layer = Some("Layer-1".into());
        f.controls.push(panel);
        let mut child = Control::new("L1-Child", ControlType::Button, 30, 40);
        child.parent = Some("L1-Panel".into());
        f.controls.push(child);
        let mut lone = Control::new("L1-Btn", ControlType::Button, 300, 20);
        lone.layer = Some("Layer-1".into());
        f.controls.push(lone);
        f.controls.push(Control::new("Base-Btn", ControlType::Button, 20, 300));
        f
    }

    fn layer_of_new(d: &DesignerPanel, before: usize) -> Vec<(String, Option<String>)> {
        d.form.controls[before..].iter().map(|c| (c.id.clone(), d.form.layer_of(&c.id).map(str::to_owned))).collect()
    }

    /// AC38 (R70) — with the real shortcuts: copy on one tab, select another, paste.
    #[test]
    fn copy_on_one_layer_and_paste_on_another_lands_in_the_active_one() {
        let mut r = Rig::new(fixture());
        r.d.apply_tab_actions(vec![TabAction::SelectLayer("Layer-1".into())]);
        r.settle(2);
        r.d.selected_ids = vec!["L1-Panel".into()]; // the Panel takes its child
        r.cmd_key(egui::Key::C);
        assert!(r.clipboard.is_some(), "Cmd+C filled the clipboard");

        // Layer-1 -> Layer-2.
        r.d.apply_tab_actions(vec![TabAction::SelectLayer("Layer-2".into())]);
        r.settle(2);
        let before = r.d.form.controls.len();
        r.cmd_key(egui::Key::V);
        let made = layer_of_new(&r.d, before);
        assert_eq!(made.len(), 2, "the Panel and its child: {made:?}");
        assert!(made.iter().all(|(_, l)| l.as_deref() == Some("Layer-2")), "both in Layer-2: {made:?}");
        let panel = r.d.form.controls[before..].iter().find(|c| c.control_type == ControlType::Panel).unwrap();
        assert_eq!(panel.layer.as_deref(), Some("Layer-2"), "the root names the layer");
        assert!(r.d.form.controls[before..].iter().any(|c| c.parent.as_deref() == Some(panel.id.as_str()) && c.layer.is_none()), "the child follows its container");
        assert!(r.d.form.controls[before..].iter().all(|c| !["L1-Panel", "L1-Child"].contains(&c.id.as_str())), "with names of their own");

        // Layer-2 -> Form.
        r.d.apply_tab_actions(vec![TabAction::SelectForm]);
        r.settle(2);
        let before = r.d.form.controls.len();
        r.cmd_key(egui::Key::V);
        let made = layer_of_new(&r.d, before);
        assert!(made.iter().all(|(_, l)| l.is_none()), "on Form they name no layer: {made:?}");

        // Form -> Layer-1.
        r.d.selected_ids = vec!["Base-Btn".into()];
        r.cmd_key(egui::Key::C);
        r.d.apply_tab_actions(vec![TabAction::SelectLayer("Layer-1".into())]);
        r.settle(2);
        let before = r.d.form.controls.len();
        r.cmd_key(egui::Key::V);
        let made = layer_of_new(&r.d, before);
        assert_eq!(made.len(), 1);
        assert_eq!(made[0].1.as_deref(), Some("Layer-1"), "Form to a layer: {made:?}");
        assert!(r.d.selected_ids.contains(&made[0].0), "and the copy is selected, in the active tab");
    }

    /// AC38 — cut moves a control to the active tab, one undo step each way, and
    /// duplicate stays where it is.
    #[test]
    fn cut_and_paste_moves_across_tabs_and_duplicate_stays_put() {
        let mut r = Rig::new(fixture());
        r.d.apply_tab_actions(vec![TabAction::SelectLayer("Layer-1".into())]);
        r.settle(2);
        r.d.selected_ids = vec!["L1-Btn".into()];
        r.cmd_key(egui::Key::X);
        assert!(r.d.form.find_control("L1-Btn").is_none(), "cut removed it");
        r.d.apply_tab_actions(vec![TabAction::SelectLayer("Layer-2".into())]);
        r.settle(2);
        let before = r.d.form.controls.len();
        r.cmd_key(egui::Key::V);
        assert_eq!(layer_of_new(&r.d, before)[0].1.as_deref(), Some("Layer-2"));

        r.d.apply_tab_actions(vec![TabAction::SelectLayer("Layer-1".into())]);
        r.settle(2);
        r.d.selected_ids = vec!["L1-Panel".into()];
        let before = r.d.form.controls.len();
        r.cmd_key(egui::Key::D);
        let made = layer_of_new(&r.d, before);
        assert!(made.iter().all(|(_, l)| l.as_deref() == Some("Layer-1")), "a duplicate stays in its layer: {made:?}");
    }

    /// AC38 — between two designers (two forms), the copy lands in the target's active tab.
    #[test]
    fn a_copy_from_one_form_pastes_into_the_active_tab_of_another() {
        let mut source = Rig::new(fixture());
        source.d.apply_tab_actions(vec![TabAction::SelectLayer("Layer-1".into())]);
        source.settle(2);
        source.d.selected_ids = vec!["L1-Btn".into()];
        source.cmd_key(egui::Key::C);

        let mut target = Rig::new(form_with_layers(3));
        target.clipboard = source.clipboard.take();
        target.d.apply_tab_actions(vec![TabAction::SelectLayer("Layer-3".into())]);
        target.settle(2);
        target.cmd_key(egui::Key::V);
        let c = target.d.form.controls.last().expect("pasted");
        assert_eq!(c.layer.as_deref(), Some("Layer-3"), "the active layer of the OTHER form");
    }
}

/// AC39 (R71) — the AI agents work with layers: the operations parse, validate
/// against the form each earlier one leaves, apply as ONE undo step, and every
/// refusal leaves the form exactly as it was.
mod agent {
    use super::*;
    use crate::agent::{parse_change_set, AgentChangeSet, AgentOp};

    fn cs(json: &str) -> AgentChangeSet {
        parse_change_set(&format!("```json\n{json}\n```")).expect("the change-set parses")
    }

    /// A form with a Button and a Panel (with a child) on the base, one layer
    /// `Overlay` holding a Label that has a handler, and a Timer.
    fn fixture() -> Form {
        let mut f = Form::new("F", "F", 800, 600);
        f.add_layer().expect("a layer");
        f.rename_layer("Layer-1", "Overlay").expect("renamed");
        f.controls.push(Control::new("SAVE", ControlType::Button, 20, 20));
        let mut p = Control::new("BOX", ControlType::Panel, 200, 20);
        p.rect.w = 200;
        p.rect.h = 120;
        f.controls.push(p);
        let mut c = Control::new("INNER", ControlType::Button, 210, 40);
        c.parent = Some("BOX".into());
        f.controls.push(c);
        let mut l = Control::new("NOTE", ControlType::Label, 20, 200);
        l.layer = Some("Overlay".into());
        l.events.push(cobolt_forms::model::EventBinding {
            event: "onClick".into(),
            paragraph: "NOTE--ONCLICK".into(),
            code: "       PROCEDURE DIVISION.\n           CONTINUE.".into(),
        });
        f.controls.push(l);
        f.controls.push(Control::new("TMR", ControlType::Timer, 0, 0));
        f
    }

    /// Every layer operation reads from the JSON an agent writes, with the names
    /// the contract teaches — and `layer` on a `deploy_control` too.
    #[test]
    fn the_layer_operations_parse_from_the_json_the_contract_teaches() {
        let set = cs(r##"{"operations":[
            {"op":"add_layer","name":"Help"},
            {"op":"add_layer"},
            {"op":"rename_layer","name":"Help","new_name":"Tips"},
            {"op":"move_layer","name":"Tips","position":1},
            {"op":"set_layer_property","layer":"Tips","key":"Transparency","value":40},
            {"op":"move_to_layer","control_ids":["SAVE"],"layer":"Tips"},
            {"op":"delete_layer","name":"Tips"},
            {"op":"deploy_control","control_type":"Label","id":"L1","layer":"Tips","properties":{}}
        ]}"##);
        assert_eq!(set.operations.len(), 8);
        assert!(matches!(&set.operations[0], AgentOp::AddLayer { name: Some(n) } if n == "Help"));
        assert!(matches!(&set.operations[1], AgentOp::AddLayer { name: None }));
        assert!(matches!(&set.operations[2], AgentOp::RenameLayer { name, new_name } if name == "Help" && new_name == "Tips"));
        assert!(matches!(&set.operations[3], AgentOp::MoveLayer { position: 1, .. }));
        assert!(matches!(&set.operations[4], AgentOp::SetLayerProperty { key, .. } if key == "Transparency"));
        assert!(matches!(&set.operations[5], AgentOp::MoveToLayer { control_ids, .. } if control_ids == &["SAVE"]));
        assert!(matches!(&set.operations[6], AgentOp::DeleteLayer { .. }));
        assert!(matches!(&set.operations[7], AgentOp::DeployControl { layer: Some(l), .. } if l == "Tips"));
    }

    /// R71 — the agent SEES the layers: the stack, what each holds, each control's
    /// layer, the reserved names, and the keys it may set.
    #[test]
    fn the_context_tells_the_agent_about_the_layers() {
        let ctx = crate::agent::build_context(&fixture());
        assert!(ctx.contains("LAYERS"), "a LAYERS section");
        assert!(ctx.contains("1. Overlay  controls=1"), "the layer and what it holds:\n{ctx}");
        assert!(ctx.contains("NOTE (Label)") && ctx.contains("layer=Overlay"), "the Label's layer is stated");
        let save = ctx.lines().find(|l| l.contains("SAVE (Button)")).expect("SAVE is listed");
        assert!(!save.contains("layer="), "a base control states no layer: {save}");
        assert!(ctx.contains("Non-Visuals"), "the reserved tab is named");
        assert!(ctx.contains("BackgroundColor") && ctx.contains("set_layer_property"), "the property keys are given");

        let none = crate::agent::build_context(&Form::new("G", "G", 400, 300));
        assert!(none.contains("(none"), "a form with no layers says so: the agent must not fake one with a Panel");
    }

    /// R71 — one change-set builds a layer, fills it and styles it, each operation
    /// judged against the form the earlier ones leave; the lot is ONE undo step.
    #[test]
    fn one_change_set_adds_fills_and_styles_a_layer_and_undoes_as_one_step() {
        let mut d = DesignerPanel::new(fixture());
        let before = saved(&d);
        let set = cs(r##"{"operations":[
            {"op":"add_layer","name":"Help"},
            {"op":"deploy_control","control_type":"Label","id":"HINT","layer":"Help","properties":{"X":40,"Y":40}},
            {"op":"deploy_control","control_type":"Timer","id":"TICK","properties":{}},
            {"op":"set_layer_property","layer":"Help","key":"BackgroundColor","value":"#10203080"},
            {"op":"move_to_layer","control_ids":["SAVE"],"layer":"Help"},
            {"op":"move_layer","name":"Help","position":1}
        ]}"##);
        let status = crate::agent::validate(&set, &d.form);
        assert!(status.iter().all(Option::is_none), "all six are valid: {status:?}");
        let applied = d.apply_agent_change_set(&set);
        assert!(applied > 0, "the change-set applied ({applied})");

        let names: Vec<&str> = d.form.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["Help", "Overlay"], "Help went to position 1, nearest Form");
        assert_eq!(d.form.layers[0].backdrop.color, "#10203080");
        assert_eq!(d.form.layer_of("HINT"), Some("Help"), "a control deployed with a layer lands in it");
        assert_eq!(d.form.layer_of("SAVE"), Some("Help"), "an existing control was moved in");
        assert_eq!(d.form.find_control("TICK").unwrap().layer, None, "a non-visual control names no layer");
        assert_eq!(d.undo_stack.len(), 1, "the whole change-set is one undo step");

        d.undo();
        assert_eq!(saved(&d), before, "one undo gives back the form exactly");
        d.redo();
        assert_eq!(d.form.layer_of("HINT"), Some("Help"), "and redo does it again");
    }

    /// R71 — a non-visual control lives on `Non-Visuals` by itself; naming a layer
    /// for one is refused with the reason, so the agent does not believe it is there.
    #[test]
    fn a_non_visual_control_takes_no_layer_and_the_agent_is_told() {
        let form = fixture();
        let set = cs(r##"{"operations":[
            {"op":"deploy_control","control_type":"Timer","id":"T2","layer":"Overlay","properties":{}},
            {"op":"deploy_control","control_type":"Timer","id":"T3","properties":{}}
        ]}"##);
        let status = crate::agent::validate(&set, &form);
        assert!(status[0].as_deref().unwrap().contains("Non-Visuals"), "{:?}", status[0]);
        assert!(status[1].is_none());
        let mut d = DesignerPanel::new(form);
        d.apply_agent_change_set(&set);
        assert!(d.form.find_control("T2").is_none(), "the refused Timer was not created");
        assert_eq!(d.form.find_control("T3").map(|c| c.layer.clone()), Some(None), "the other lands on Non-Visuals");
    }

    /// R71 — a layer can be renamed and then named by its new name in the same
    /// change-set; the controls that name it follow.
    #[test]
    fn a_rename_is_visible_to_the_operations_after_it() {
        let mut d = DesignerPanel::new(fixture());
        let set = cs(r##"{"operations":[
            {"op":"rename_layer","name":"Overlay","new_name":"Banner"},
            {"op":"set_layer_property","layer":"Banner","key":"Transparency","value":25},
            {"op":"deploy_control","control_type":"Label","id":"TITLE","layer":"Banner","properties":{}}
        ]}"##);
        assert!(crate::agent::validate(&set, &d.form).iter().all(Option::is_none));
        d.apply_agent_change_set(&set);
        assert_eq!(d.form.layers[0].name, "Banner");
        assert_eq!(d.form.layers[0].backdrop.transparency, 25);
        assert_eq!(d.form.layer_of("NOTE"), Some("Banner"), "the control that named Overlay follows");
        assert_eq!(d.form.layer_of("TITLE"), Some("Banner"));
        d.undo();
        assert_eq!(d.form.layers[0].name, "Overlay");
        assert_eq!(d.form.layer_of("NOTE"), Some("Overlay"));
        assert!(d.form.find_control("TITLE").is_none());
    }

    /// R64/R71 — deleting a layer takes its controls AND their handlers, as one
    /// undo step that gives all of it back.
    #[test]
    fn delete_layer_takes_its_controls_and_handlers_and_one_undo_returns_them() {
        let mut d = DesignerPanel::new(fixture());
        let before = saved(&d);
        d.apply_agent_change_set(&cs(r##"{"operations":[{"op":"delete_layer","name":"Overlay"}]}"##));
        assert!(d.form.layers.is_empty());
        assert!(d.form.find_control("NOTE").is_none(), "the Label went with its layer");
        assert!(d.form.find_control("SAVE").is_some(), "a control on the base is untouched");
        assert_eq!(d.undo_stack.len(), 1);
        d.undo();
        assert_eq!(saved(&d), before, "layer, control and its handler are back exactly");
        assert!(d.form.find_control("NOTE").unwrap().events.iter().any(|e| e.has_code()));
    }

    /// R71 — moving to `Form` sends a control back to the base; a container takes
    /// its children; a child alone, a non-visual control and an unknown layer are
    /// refused with a reason the agent can act on.
    #[test]
    fn move_to_layer_follows_the_rules_of_the_designer() {
        let form = fixture();
        let set = cs(r##"{"operations":[
            {"op":"move_to_layer","control_ids":["NOTE"],"layer":"Form"},
            {"op":"move_to_layer","control_ids":["BOX"],"layer":"Overlay"},
            {"op":"move_to_layer","control_ids":["INNER"],"layer":"Overlay"},
            {"op":"move_to_layer","control_ids":["TMR"],"layer":"Overlay"},
            {"op":"move_to_layer","control_ids":["SAVE"],"layer":"Nowhere"},
            {"op":"move_to_layer","control_ids":["GHOST"],"layer":"Overlay"}
        ]}"##);
        let status = crate::agent::validate(&set, &form);
        assert!(status[0].is_none(), "back to the base is fine");
        assert!(status[1].is_none(), "a container is fine");
        assert!(status[2].as_deref().unwrap().contains("BOX"), "a child names its container: {:?}", status[2]);
        assert!(status[3].as_deref().unwrap().contains("no layer"), "{:?}", status[3]);
        assert!(status[4].as_deref().unwrap().contains("Overlay"), "the missing layer lists the real ones: {:?}", status[4]);
        assert!(status[5].as_deref().unwrap().contains("GHOST"), "{:?}", status[5]);

        let mut d = DesignerPanel::new(form);
        d.apply_agent_change_set(&set);
        assert_eq!(d.form.layer_of("NOTE"), None, "NOTE is back on the base");
        assert_eq!(d.form.layer_of("BOX"), Some("Overlay"));
        assert_eq!(d.form.layer_of("INNER"), Some("Overlay"), "the child followed its Panel");
    }

    /// R71 — every refusal leaves the form exactly as it was, and says why.
    #[test]
    fn a_refused_layer_operation_changes_nothing_and_says_why() {
        let mut f = fixture();
        while f.layers.len() < cobolt_forms::model::MAX_LAYERS {
            f.add_layer().expect("a layer");
        }
        let mut d = DesignerPanel::new(f);
        let before = saved(&d);
        let set = cs(r##"{"operations":[
            {"op":"add_layer","name":"Past-The-Limit"},
            {"op":"rename_layer","name":"Overlay","new_name":"Form"},
            {"op":"rename_layer","name":"Overlay","new_name":"non-visuals"},
            {"op":"rename_layer","name":"Overlay","new_name":"SAVE"},
            {"op":"rename_layer","name":"Overlay","new_name":"has space"},
            {"op":"move_layer","name":"Overlay","position":0},
            {"op":"move_layer","name":"Overlay","position":999},
            {"op":"set_layer_property","layer":"Overlay","key":"Visible","value":false},
            {"op":"set_layer_property","layer":"Overlay","key":"Transparency","value":400},
            {"op":"set_layer_property","layer":"Overlay","key":"CornerRadius","value":8},
            {"op":"delete_layer","name":"Nowhere"}
        ]}"##);
        let status = crate::agent::validate(&set, &d.form);
        for (i, s) in status.iter().enumerate() {
            assert!(s.is_some(), "operation {i} must be refused");
        }
        assert!(status[0].as_deref().unwrap().contains("at most"), "{:?}", status[0]);
        assert!(status[7].as_deref().unwrap().contains("Visible"), "{:?}", status[7]);
        assert_eq!(d.apply_agent_change_set(&set), 0, "nothing applied");
        assert_eq!(saved(&d), before, "the form is byte-for-byte what it was");
        assert!(d.undo_stack.is_empty(), "and no undo step was made");
        assert!(d.last_change_outcome.contains("NOT applied"), "the agent is told: {}", d.last_change_outcome);
    }

    /// R71 — a layer name is a control name: it cannot take one in use, in
    /// either direction, even inside one change-set.
    #[test]
    fn layer_and_control_names_share_one_namespace_for_agents_too() {
        let form = fixture();
        let set = cs(r##"{"operations":[
            {"op":"add_layer","name":"SAVE"},
            {"op":"add_layer","name":"Help"},
            {"op":"add_layer","name":"help"},
            {"op":"deploy_control","control_type":"Label","id":"Help","properties":{}},
            {"op":"deploy_control","control_type":"Label","id":"B2","layer":"Nowhere","properties":{}}
        ]}"##);
        let status = crate::agent::validate(&set, &form);
        assert!(status[0].is_some(), "a layer cannot be named after a control");
        assert!(status[1].is_none(), "Help is free");
        assert!(status[2].is_some(), "…and then it is not, whatever the letter case");
        assert!(status[4].as_deref().unwrap().contains("Overlay"), "a deploy into a missing layer lists the real ones");
    }

    /// Two layers with controls between base controls, so that a deletion planned
    /// against the wrong list would take the wrong ones: order is SAVE (base), NOTE
    /// (Overlay), BOX, INNER (child of BOX), OLD-LBL (Old), TMR.
    fn interleaved() -> Form {
        let mut f = Form::new("F", "F", 800, 600);
        f.add_layer().expect("a layer");
        f.rename_layer("Layer-1", "Overlay").expect("renamed");
        f.add_layer().expect("a layer");
        f.rename_layer("Layer-1", "Old").expect("renamed");
        let handler = |p: &str| cobolt_forms::model::EventBinding {
            event: "onClick".into(),
            paragraph: p.into(),
            code: "       PROCEDURE DIVISION.\n           CONTINUE.".into(),
        };
        f.controls.push(Control::new("SAVE", ControlType::Button, 20, 20));
        let mut note = Control::new("NOTE", ControlType::Label, 20, 200);
        note.layer = Some("Overlay".into());
        note.events.push(handler("NOTE--ONCLICK"));
        f.controls.push(note);
        let mut p = Control::new("BOX", ControlType::Panel, 200, 20);
        p.rect.w = 200;
        p.rect.h = 120;
        f.controls.push(p);
        let mut c = Control::new("INNER", ControlType::Button, 210, 40);
        c.parent = Some("BOX".into());
        f.controls.push(c);
        let mut old = Control::new("OLD-LBL", ControlType::Label, 20, 300);
        old.layer = Some("Old".into());
        old.events.push(handler("OLD-LBL--ONCLICK"));
        f.controls.push(old);
        f.controls.push(Control::new("TMR", ControlType::Timer, 0, 0));
        f
    }

    /// AC39 (R71) — the whole scenario in ONE change-set: add a layer, rename it, set
    /// its colour, create a button in it, send a second control to it, re-stack it and
    /// delete another layer. Validated, applied as one undo step, undone as one.
    #[test]
    fn the_whole_ac39_scenario_is_one_change_and_one_undo() {
        let mut d = DesignerPanel::new(interleaved());
        let before = saved(&d);
        let set = cs(r##"{"operations":[
            {"op":"add_layer","name":"Help"},
            {"op":"rename_layer","name":"Help","new_name":"Banner"},
            {"op":"set_layer_property","layer":"Banner","key":"BackgroundColor","value":"#203040"},
            {"op":"deploy_control","control_type":"Button","id":"GO","layer":"Banner","properties":{"X":30,"Y":30}},
            {"op":"move_to_layer","control_ids":["SAVE"],"layer":"Banner"},
            {"op":"move_layer","name":"Banner","position":1},
            {"op":"delete_layer","name":"Old"}
        ]}"##);
        let status = crate::agent::validate(&set, &d.form);
        assert!(status.iter().all(Option::is_none), "all seven are valid: {status:?}");

        d.apply_agent_change_set(&set);
        let names: Vec<&str> = d.form.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["Banner", "Overlay"], "Old is gone, Banner went to position 1");
        assert_eq!(d.form.layers[0].backdrop.color, "#203040");
        assert_eq!(d.form.layer_of("GO"), Some("Banner"));
        assert_eq!(d.form.layer_of("SAVE"), Some("Banner"));
        assert!(d.form.find_control("OLD-LBL").is_none(), "the deleted layer took its control");
        for kept in ["NOTE", "BOX", "INNER", "TMR"] {
            assert!(d.form.find_control(kept).is_some(), "{kept} is untouched");
        }
        assert_eq!(d.undo_stack.len(), 1, "one undo step for the whole change-set");

        d.undo();
        assert_eq!(saved(&d), before, "one undo gives back the form exactly — layers, controls, handlers, order");
        assert!(d.form.find_control("OLD-LBL").unwrap().events.iter().any(|e| e.has_code()));
        d.redo();
        assert!(d.form.find_control("OLD-LBL").is_none());
        assert_eq!(d.form.layer_of("GO"), Some("Banner"));
    }

    /// R71 — two deletions in one change-set each take their own controls: the second
    /// is not planned against positions the first has already moved.
    #[test]
    fn two_deleted_layers_take_the_right_controls_and_one_undo_returns_all() {
        let mut d = DesignerPanel::new(interleaved());
        let before = saved(&d);
        d.apply_agent_change_set(&cs(
            r##"{"operations":[{"op":"delete_layer","name":"Overlay"},{"op":"delete_layer","name":"Old"}]}"##,
        ));
        assert!(d.form.layers.is_empty());
        let left: Vec<&str> = d.form.controls.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(left, ["SAVE", "BOX", "INNER", "TMR"], "exactly the two layers' controls went");
        assert_eq!(d.undo_stack.len(), 1);
        d.undo();
        assert_eq!(saved(&d), before, "and they come back in their places");
    }

    /// R71 — a control the change-set moved out of a layer is no longer in it when the
    /// layer is deleted; one deleted from a layer the change-set added is not a leftover.
    #[test]
    fn a_control_sent_out_of_a_layer_survives_the_deletion_of_that_layer() {
        let mut f = interleaved();
        let mut second = Control::new("NOTE2", ControlType::Label, 20, 240);
        second.layer = Some("Overlay".into());
        f.controls.push(second);
        let mut d = DesignerPanel::new(f);
        d.apply_agent_change_set(&cs(
            r##"{"operations":[
                {"op":"move_to_layer","control_ids":["NOTE"],"layer":"Form"},
                {"op":"delete_layer","name":"Overlay"}
            ]}"##,
        ));
        assert_eq!(d.form.layer_of("NOTE"), None, "NOTE was sent to the base first, so it stays");
        assert!(d.form.find_control("NOTE").is_some());
        assert!(d.form.find_control("NOTE2").is_none(), "NOTE2 was still in Overlay when it went");
        assert_eq!(d.undo_stack.len(), 1);
    }

    /// R71 — a change-set that puts controls into a layer and deletes that same layer
    /// contradicts itself: it is refused with the reason, whichever way the controls
    /// got there; deleting ANOTHER layer in the same change-set is fine.
    #[test]
    fn deleting_a_layer_the_same_change_set_fills_is_refused() {
        let form = interleaved();
        let by_deploy = cs(r##"{"operations":[
            {"op":"deploy_control","control_type":"Label","id":"N","layer":"Overlay","properties":{}},
            {"op":"delete_layer","name":"Overlay"}]}"##);
        let by_move = cs(r##"{"operations":[
            {"op":"move_to_layer","control_ids":["SAVE"],"layer":"Overlay"},
            {"op":"delete_layer","name":"Overlay"}]}"##);
        let by_container = cs(r##"{"operations":[
            {"op":"move_to_layer","control_ids":["BOX"],"layer":"Old"},
            {"op":"deploy_control","control_type":"Label","id":"KID","parent":"BOX","properties":{}},
            {"op":"delete_layer","name":"Old"}]}"##);
        let by_rename = cs(r##"{"operations":[
            {"op":"deploy_control","control_type":"Label","id":"N","layer":"Overlay","properties":{}},
            {"op":"rename_layer","name":"Overlay","new_name":"Renamed"},
            {"op":"delete_layer","name":"Renamed"}]}"##);
        for (name, set, at) in [("deploy", by_deploy, 1), ("move", by_move, 1), ("container", by_container, 2), ("rename", by_rename, 2)] {
            let status = crate::agent::validate(&set, &form);
            let why = status[at].as_deref().unwrap_or_else(|| panic!("{name}: the deletion must be refused"));
            assert!(why.contains("also puts controls"), "{name}: {why}");
        }
        let other = cs(r##"{"operations":[
            {"op":"deploy_control","control_type":"Label","id":"N","layer":"Overlay","properties":{}},
            {"op":"delete_layer","name":"Old"}]}"##);
        assert!(crate::agent::validate(&other, &form).iter().all(Option::is_none), "another layer may go");
    }

    /// R71 — a redeploy (an id of the same type already on the form) updates the control
    /// where it stands, and moves it only when it names a layer; validation and apply
    /// agree on which, so a later deletion takes what is really in the layer.
    #[test]
    fn a_redeploy_stays_in_its_layer_unless_it_names_another() {
        let mut d = DesignerPanel::new(interleaved());
        // No layer named: NOTE stays in Overlay, so deleting Overlay in the same change-set takes it.
        d.apply_agent_change_set(&cs(r##"{"operations":[
            {"op":"deploy_control","control_type":"Label","id":"NOTE","properties":{"Caption":"again"}},
            {"op":"delete_layer","name":"Overlay"}]}"##));
        assert!(d.form.find_control("NOTE").is_none(), "NOTE never left Overlay");
        d.undo();

        // A layer named: SAVE (a base Button) is moved; `Form` brings NOTE back to the base.
        d.apply_agent_change_set(&cs(r##"{"operations":[
            {"op":"deploy_control","control_type":"Button","id":"SAVE","layer":"Overlay","properties":{}},
            {"op":"deploy_control","control_type":"Label","id":"NOTE","layer":"Form","properties":{}}]}"##));
        assert_eq!(d.form.layer_of("SAVE"), Some("Overlay"));
        assert_eq!(d.form.layer_of("NOTE"), None);
        assert_eq!(d.form.controls.len(), 6, "no second control was made");
        assert_eq!(d.undo_stack.len(), 1, "the first change-set was undone; this one is a single step");
    }

    /// R71 — a control deployed with no `layer` lands on the base, and a deploy of a
    /// control that already exists keeps its layer (a redeploy is not a move).
    #[test]
    fn a_deploy_without_a_layer_lands_on_the_base() {
        let mut d = DesignerPanel::new(fixture());
        d.tabs.select_layer("Overlay"); // the designer's active tab must NOT leak into an agent's deploy
        d.apply_agent_change_set(&cs(
            r##"{"operations":[{"op":"deploy_control","control_type":"Button","id":"PLAIN","properties":{}}]}"##,
        ));
        assert_eq!(d.form.layer_of("PLAIN"), None, "no `layer` means the base, whichever tab is open");
    }

    /// R71 — the preview row and the headline carry identifiers only, in the
    /// language of the IDE, and the lint gate leaves a valid layer change alone.
    #[test]
    fn the_preview_and_the_lint_know_the_layer_operations() {
        let set = cs(r##"{"operations":[
            {"op":"add_layer","name":"Help"},
            {"op":"move_to_layer","control_ids":["SAVE","BOX"],"layer":"Help"}
        ]}"##);
        assert_eq!(crate::agent::layer_op_label(&set.operations[0]), "add_layer Help");
        assert_eq!(crate::agent::layer_op_label(&set.operations[1]), "move_to_layer SAVE,BOX -> Help");
        let ledger = crate::agent::outcome_ledger(&set, &[None, None]);
        assert!(ledger.contains("applied: add_layer Help"), "{ledger}");
        let shown = "```json\n{\"operations\":[{\"op\":\"add_layer\",\"name\":\"Help\"}]}\n```";
        assert!(
            crate::agent::lint_change_set_submission("Form Designer Agent", shown).is_none(),
            "a valid layer operation is not a lint defect"
        );
        let bad = "```json\n{\"operations\":[{\"op\":\"add_layer\",\"name\":\"Form\"}]}\n```";
        assert!(
            crate::agent::lint_change_set_submission("Form Designer Agent", bad).is_some(),
            "a reserved name is caught before a model is asked to review it"
        );
    }
}
