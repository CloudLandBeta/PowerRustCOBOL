// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The designer's **tab bar** (spec 091 R23–R26, R43, R46): the fixed
//! `Non-Visuals` tab, the base's `Form` tab, one tab per layer, and a `+` — a
//! strip of fixed height directly under the canvas.
//!
//! Two halves, kept apart so the rules can be tested without a window:
//!
//! * [`LayerTabs`] — which tab is active and which layers the designer is
//!   *showing*. Pure state. Selecting a layer shows it and hides the others
//!   (R60, R61); the visibility box never changes the tab (R62). The shown set
//!   is a **design aid, never saved** (R25, R35; operator, 2026-10-09): it lives
//!   here and not in the form, so choosing a tab neither marks the form modified
//!   nor makes an undo step.
//! * [`bar_layout`] and [`LayerTabs::show_bar`] — where each tab sits, and the
//!   painting and pointer handling. The layout is a pure function of the tab
//!   widths and the width available; the bar never asks for more height or
//!   width than it was given (the window never resizes itself, R23).
//!
//! The bar reports what the developer did as [`TabAction`]s; the designer
//! applies them — the undoable ones (add, rename, re-stack, delete) as commands.

use std::collections::HashSet;

use egui::{pos2, vec2, Align2, Color32, FontId, Rect, Sense, Shape, Stroke, Ui};

use cobolt_forms::Form;

/// Which tab is selected. Exactly one at any time (R43).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum ActiveTab {
    /// The fixed grid of non-visual controls (R46).
    NonVisuals,
    /// The base: the form as it exists without layers. The default.
    #[default]
    Form,
    /// A layer, by its name.
    Layer(String),
}

/// What the developer did on the bar. The designer decides what each means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TabAction {
    SelectNonVisuals,
    SelectForm,
    SelectLayer(String),
    /// The visibility box of a layer was clicked (R25, R62): the tab does not change.
    ToggleShown(String),
    /// The `+` (R26, Q30).
    Add,
    /// The red ✕ of a layer (R63): select it, then ask.
    RequestDelete(String),
    /// A layer's new name, typed after a double-click (R26).
    Rename { layer: String, to: String },
    /// A layer tab dropped at another place among the layer tabs (R26): positions
    /// count layers above the base, 0 nearest it.
    Move { from: usize, to: usize },
}

// ── The state: which tab, which layers shown ─────────────────────────────────

/// The designer's layer state: the active tab, what is shown, and the bar's own
/// interaction state. Never saved and never part of the form.
#[derive(Default)]
pub struct LayerTabs {
    active: ActiveTab,
    /// The layers shown on the canvas, lower-cased. The base is always shown and
    /// is not in here.
    shown: HashSet<String>,
    /// How far the scrollable tabs are shifted left, in pixels (the ◀ ▶ arrows).
    scroll: f32,
    /// A layer tab being dragged: where it was, and the pointer's offset from
    /// its left edge.
    drag: Option<TabDrag>,
    /// A layer being renamed in place: its name, and the text typed so far.
    rename: Option<(String, String)>,
}

#[derive(Clone, Copy, Debug)]
struct TabDrag {
    from: usize,
    grab: f32,
    moved: bool,
}

impl LayerTabs {
    pub fn active(&self) -> &ActiveTab {
        &self.active
    }

    pub fn is_non_visuals(&self) -> bool {
        self.active == ActiveTab::NonVisuals
    }

    pub fn is_form(&self) -> bool {
        self.active == ActiveTab::Form
    }

    /// The active layer's name, when a layer is selected.
    pub fn active_layer(&self) -> Option<&str> {
        match &self.active {
            ActiveTab::Layer(name) => Some(name),
            _ => None,
        }
    }

    /// Whether the designer shows the layer called `name` (case-insensitive). The
    /// base is not a layer and is always shown.
    pub fn is_shown(&self, name: &str) -> bool {
        self.shown.contains(&name.to_ascii_lowercase())
    }

    /// Select the `Non-Visuals` tab. It hides no layer (R59, R61).
    pub fn select_non_visuals(&mut self) {
        self.active = ActiveTab::NonVisuals;
    }

    /// Select the base. It hides no layer (R61): the layers the developer left
    /// showing stay showing, over the form.
    pub fn select_form(&mut self) {
        self.active = ActiveTab::Form;
    }

    /// Select a layer (R60, R61): it becomes active, it is shown — if its box was
    /// off, selecting it turns it on — and **every other layer is hidden**,
    /// because one layer is edited at a time. The base is never hidden. The
    /// developer may tick any box afterwards; nothing hides it again until
    /// another layer is selected.
    pub fn select_layer(&mut self, name: &str) {
        self.active = ActiveTab::Layer(name.to_owned());
        self.shown.clear();
        self.shown.insert(name.to_ascii_lowercase());
    }

    /// The visibility box of a layer (R25, R62): shows or hides that layer and
    /// **never changes the active tab** — not when the box is another layer's,
    /// and not when it is the active layer's own.
    pub fn set_shown(&mut self, name: &str, on: bool) {
        let key = name.to_ascii_lowercase();
        if on {
            self.shown.insert(key);
        } else {
            self.shown.remove(&key);
        }
    }

    /// A layer was renamed: the tab and the shown set follow it.
    pub fn retarget(&mut self, old: &str, new: &str) {
        if self.shown.remove(&old.to_ascii_lowercase()) {
            self.shown.insert(new.to_ascii_lowercase());
        }
        if matches!(&self.active, ActiveTab::Layer(n) if n.eq_ignore_ascii_case(old)) {
            self.active = ActiveTab::Layer(new.to_owned());
        }
        if matches!(&self.rename, Some((n, _)) if n.eq_ignore_ascii_case(old)) {
            self.rename = None;
        }
    }

    /// A layer is gone (deleted, or an add undone). It is no longer shown, and if
    /// it was the active tab the base is (Q27).
    pub fn forget(&mut self, name: &str) {
        self.shown.remove(&name.to_ascii_lowercase());
        if matches!(&self.active, ActiveTab::Layer(n) if n.eq_ignore_ascii_case(name)) {
            self.active = ActiveTab::Form;
        }
        if matches!(&self.rename, Some((n, _)) if n.eq_ignore_ascii_case(name)) {
            self.rename = None;
        }
    }

    /// Drop everything that names a layer the form no longer has — after an undo,
    /// a redo, a load or an edit the agent made. A tab that points at nothing
    /// falls back to the base.
    pub fn reconcile(&mut self, form: &Form) {
        let exists = |n: &str| form.layer_index(n).is_some();
        self.shown.retain(|k| exists(k));
        if matches!(&self.active, ActiveTab::Layer(n) if !exists(n)) {
            self.active = ActiveTab::Form;
        }
        if matches!(&self.rename, Some((n, _)) if !exists(n)) {
            self.rename = None;
        }
    }
}

/// A frame's snapshot of what the canvas needs to know of the tabs: whether the
/// `Non-Visuals` tab is active, and which layers are shown. Owned, so the
/// canvas's `FormState` can hold it while the designer is borrowed mutably.
#[derive(Clone, Debug, Default)]
pub struct TabView {
    non_visuals: bool,
    shown: HashSet<String>,
}

impl TabView {
    pub fn is_non_visuals(&self) -> bool {
        self.non_visuals
    }

    pub fn is_shown(&self, name: &str) -> bool {
        self.shown.contains(&name.to_ascii_lowercase())
    }
}

impl LayerTabs {
    /// The snapshot the canvas paints from this frame.
    pub fn view(&self) -> TabView {
        TabView { non_visuals: self.is_non_visuals(), shown: self.shown.clone() }
    }
}

// ── The layout: where each tab sits ──────────────────────────────────────────

/// The bar's fixed height (R23): it asks for no more, ever.
pub const BAR_H: f32 = 32.0;
/// A tab's height, and how far its edges lean.
pub const TAB_H: f32 = 26.0;
pub const SLANT: f32 = 9.0;
/// The scroll arrows' width.
pub const ARROW_W: f32 = 22.0;
const GAP: f32 = 2.0;
const LEFT_PAD: f32 = 6.0;
/// A layer tab's two controls: the visibility box and the red ✕.
pub const BOX: f32 = 13.0;
pub const CROSS: f32 = 11.0;
const INNER_PAD: f32 = 6.0;

/// What sits at a place in the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    NonVisuals,
    Form,
    /// A layer, by its place among the layers (0 nearest the base).
    Layer(usize),
    Add,
}

/// One tab's place: `x` from the bar's left edge, and its width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub slot: Slot,
    pub x: f32,
    pub w: f32,
}

/// The whole bar.
#[derive(Clone, Debug, PartialEq)]
pub struct BarLayout {
    pub placed: Vec<Placed>,
    /// The tabs do not all fit: the arrows are shown, and the scrollable tabs are
    /// cut to the room between the pinned tab and the arrows.
    pub overflow: bool,
    /// The most the scrollable tabs can be shifted, in pixels.
    pub max_scroll: f32,
    /// Where the scrollable tabs are clipped, as `(left, right)` from the bar's
    /// left edge.
    pub clip: (f32, f32),
}

/// A tab's width: its text and what it carries, plus the lean of both edges.
pub fn tab_width(text_w: f32, layer: bool) -> f32 {
    let carried = if layer { BOX + CROSS + 2.0 * INNER_PAD } else { 0.0 };
    text_w + carried + 2.0 * INNER_PAD + SLANT
}

/// Lay the bar out for `avail` pixels of width (R23, R26).
///
/// The `Non-Visuals` tab is **pinned** at the left and does not scroll (Q21). The
/// others — `Form`, each layer in stack order, and the `+` — follow it, shifted
/// left by `scroll` when they do not all fit; the arrows then appear at the right
/// edge and the tabs are cut to the room between. Pure: no fonts, no window.
///
/// `layer_text_w` are the layers' label widths in stack order; `nv_w` and
/// `form_w` the two fixed tabs'.
pub fn bar_layout(nv_text_w: f32, form_text_w: f32, layer_text_w: &[f32], avail: f32, scroll: f32) -> BarLayout {
    let nv_w = tab_width(nv_text_w, false);
    let form_w = tab_width(form_text_w, false);
    let add_w = TAB_H;
    let tabs_w: f32 = form_w
        + layer_text_w.iter().map(|w| GAP + tab_width(*w, true)).sum::<f32>()
        + GAP
        + add_w;
    let pinned_end = LEFT_PAD + nv_w + GAP;
    let room_without_arrows = (avail - pinned_end).max(0.0);
    let overflow = tabs_w > room_without_arrows;
    let room = if overflow { (room_without_arrows - 2.0 * ARROW_W).max(0.0) } else { room_without_arrows };
    let max_scroll = (tabs_w - room).max(0.0);
    let scroll = scroll.clamp(0.0, max_scroll);

    let mut placed = vec![Placed { slot: Slot::NonVisuals, x: LEFT_PAD, w: nv_w }];
    let mut x = pinned_end - scroll;
    placed.push(Placed { slot: Slot::Form, x, w: form_w });
    x += form_w;
    for (i, tw) in layer_text_w.iter().enumerate() {
        x += GAP;
        let w = tab_width(*tw, true);
        placed.push(Placed { slot: Slot::Layer(i), x, w });
        x += w;
    }
    x += GAP;
    placed.push(Placed { slot: Slot::Add, x, w: add_w });
    BarLayout { placed, overflow, max_scroll, clip: (pinned_end, pinned_end + room) }
}

/// Where a layer tab's three parts sit inside its rectangle: the label, the
/// visibility box and the red ✕ (R23). One function for the painter, the pointer
/// and the tests, so the three cannot disagree about where a click lands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TabParts {
    pub label: Rect,
    pub check: Rect,
    pub cross: Rect,
}

pub fn layer_tab_parts(rect: Rect) -> TabParts {
    let cross = Rect::from_center_size(pos2(rect.max.x - SLANT - INNER_PAD - CROSS * 0.5, rect.center().y), vec2(CROSS, CROSS));
    let check = Rect::from_center_size(pos2(cross.min.x - INNER_PAD - BOX * 0.5, rect.center().y), vec2(BOX, BOX));
    let label = Rect::from_min_max(pos2(rect.min.x + SLANT + INNER_PAD * 0.5, rect.min.y), pos2(check.min.x - 2.0, rect.max.y));
    TabParts { label, check, cross }
}

/// Where a layer tab dropped with its left edge at `x` lands among the layer
/// tabs (R26): the number of layer tabs whose middle is left of `x`'s centre.
/// Never before `Form`: the result counts layers, so 0 is the one nearest it.
pub fn drop_position(layout: &BarLayout, dragged_centre: f32, from: usize) -> usize {
    let mut to = 0;
    for p in &layout.placed {
        if let Slot::Layer(i) = p.slot {
            if i == from {
                continue;
            }
            if p.x + p.w * 0.5 < dragged_centre {
                to += 1;
            }
        }
    }
    to
}

// ── The colours: fixed, not the IDE theme's (R43) ────────────────────────────

/// The active tab's face and its text; the inactive's are the reverse. The IDE
/// ships 33 themes, glass ones among them, and a tab read from the theme would
/// be dark on dark on some — the colours are the same on every one.
pub const ACTIVE_FACE: Color32 = Color32::from_rgb(0x2F, 0x63, 0xB3);
pub const ACTIVE_TEXT: Color32 = Color32::WHITE;
pub const INACTIVE_FACE: Color32 = Color32::WHITE;
pub const INACTIVE_TEXT: Color32 = ACTIVE_FACE;
const EDGE: Color32 = Color32::from_rgb(0x1B, 0x1B, 0x1B);
/// The red of the ✕ — it reads on both faces.
pub const CROSS_RED: Color32 = Color32::from_rgb(0xD8, 0x22, 0x22);

// ── The bar itself ───────────────────────────────────────────────────────────

fn tab_font() -> FontId {
    FontId::proportional(13.0)
}

/// The bar's layout for `form` in a strip `width` wide, scrolled by `scroll`: the
/// label widths are measured with the bar's own font. What the painter draws from
/// and what a test aims a click with.
pub fn layout_for(ctx: &egui::Context, form: &Form, tr: &crate::i18n::Tr, width: f32, scroll: f32) -> BarLayout {
    let font = tab_font();
    let text_w = |s: &str| ctx.fonts_mut(|f| f.layout_no_wrap(s.to_owned(), font.clone(), Color32::BLACK).size().x);
    let lw: Vec<f32> = form.layers.iter().map(|l| text_w(&l.name)).collect();
    bar_layout(text_w(tr.layer_tab_non_visuals), text_w(cobolt_forms::model::BASE_LAYER_NAME), &lw, width, scroll)
}

/// The rectangle of a placed tab inside the bar `bar`.
pub fn placed_rect(bar: Rect, p: &Placed) -> Rect {
    Rect::from_min_size(pos2(bar.min.x + p.x, bar.min.y + (BAR_H - TAB_H) * 0.5), vec2(p.w, TAB_H))
}

fn slanted(rect: Rect) -> Vec<egui::Pos2> {
    vec![
        pos2(rect.min.x, rect.max.y),
        pos2(rect.min.x + SLANT, rect.min.y),
        pos2(rect.max.x, rect.min.y),
        pos2(rect.max.x - SLANT, rect.max.y),
    ]
}

impl LayerTabs {
    /// Paint the bar and report what the developer did. `bar` is the strip it
    /// owns: [`BAR_H`] high and as wide as the canvas — never more.
    pub fn show_bar(&mut self, ui: &mut Ui, form: &Form, tr: &crate::i18n::Tr) -> Vec<TabAction> {
        let mut actions = Vec::new();
        let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), BAR_H), Sense::hover());
        let painter = ui.painter_at(bar);
        let font = tab_font();
        let layers: Vec<&str> = form.layers.iter().map(|l| l.name.as_str()).collect();
        let layout = layout_for(ui.ctx(), form, tr, bar.width(), self.scroll);
        let at = |p: &Placed| placed_rect(bar, p);
        let (clip_l, clip_r) = layout.clip;
        let tabs_clip = Rect::from_min_max(
            pos2(bar.min.x + clip_l, bar.min.y),
            pos2(bar.min.x + if layout.overflow { clip_r } else { bar.width() }, bar.max.y),
        );

        // The arrows, when the tabs do not all fit (R23): at the right edge.
        if layout.overflow {
            self.scroll = self.scroll.clamp(0.0, layout.max_scroll);
            for (i, (glyph, hint)) in [("◀", tr.layer_tab_scroll_left_hint), ("▶", tr.layer_tab_scroll_right_hint)]
                .into_iter()
                .enumerate()
            {
                let r = Rect::from_min_size(
                    pos2(bar.max.x - ARROW_W * (2 - i) as f32, bar.min.y + 3.0),
                    vec2(ARROW_W, BAR_H - 6.0),
                );
                let resp = ui.interact(r, ui.id().with(("layer-tab-arrow", i)), Sense::click()).on_hover_text(hint);
                painter.text(r.center(), Align2::CENTER_CENTER, glyph, font.clone(), if resp.hovered() { Color32::WHITE } else { Color32::LIGHT_GRAY });
                if resp.clicked() {
                    let step = 80.0;
                    self.scroll = if i == 0 { (self.scroll - step).max(0.0) } else { (self.scroll + step).min(layout.max_scroll) };
                }
            }
        }

        // Inactive tabs first, the active one last, so its edge is on top.
        let order: Vec<&Placed> = layout
            .placed
            .iter()
            .filter(|p| !self.is_active_slot(p.slot, &layers))
            .chain(layout.placed.iter().filter(|p| self.is_active_slot(p.slot, &layers)))
            .collect();
        for p in order {
            let rect = at(p);
            // The pinned tab is never clipped; the others are cut to the room
            // the arrows leave.
            let clip = if matches!(p.slot, Slot::NonVisuals) { bar } else { tabs_clip };
            let tp = painter.with_clip_rect(clip);
            match p.slot {
                Slot::NonVisuals => {
                    let a = self.is_non_visuals();
                    if tab_face(ui, &tp, rect, tr.layer_tab_non_visuals, a, ui.id().with("nv"), &font, tr.layer_tab_non_visuals_hint)
                        .clicked()
                    {
                        actions.push(TabAction::SelectNonVisuals);
                    }
                }
                Slot::Form => {
                    let a = self.is_form();
                    if tab_face(ui, &tp, rect, cobolt_forms::model::BASE_LAYER_NAME, a, ui.id().with("form"), &font, tr.layer_tab_form_hint)
                        .clicked()
                    {
                        actions.push(TabAction::SelectForm);
                    }
                }
                Slot::Layer(i) => {
                    let name = layers[i].to_owned();
                    self.layer_tab(ui, &tp, rect, &name, i, &font, tr, &layout, &mut actions);
                }
                Slot::Add => {
                    let resp = ui
                        .interact(rect, ui.id().with("layer-add"), Sense::click())
                        .on_hover_text(tr.layer_tab_add_hint);
                    let c = rect.center();
                    let col = if resp.hovered() { Color32::WHITE } else { Color32::LIGHT_GRAY };
                    tp.line_segment([pos2(c.x - 6.0, c.y), pos2(c.x + 6.0, c.y)], Stroke::new(2.0, col));
                    tp.line_segment([pos2(c.x, c.y - 6.0), pos2(c.x, c.y + 6.0)], Stroke::new(2.0, col));
                    if resp.clicked() {
                        actions.push(TabAction::Add);
                    }
                }
            }
        }

        // The drop marker while a layer tab is dragged (R26).
        if let Some(d) = self.drag.filter(|d| d.moved) {
            if let Some(pos) = ui.ctx().pointer_latest_pos() {
                let to = drop_position(&layout, pos.x - bar.min.x - d.grab + 40.0, d.from);
                let slot_x = layout
                    .placed
                    .iter()
                    .filter(|p| matches!(p.slot, Slot::Layer(i) if i != d.from))
                    .nth(to)
                    .map(|p| p.x)
                    .or_else(|| layout.placed.iter().find(|p| p.slot == Slot::Add).map(|p| p.x))
                    .unwrap_or(0.0);
                painter.line_segment(
                    [pos2(bar.min.x + slot_x - 1.0, bar.min.y + 2.0), pos2(bar.min.x + slot_x - 1.0, bar.max.y - 2.0)],
                    Stroke::new(2.0, Color32::from_rgb(0xFF, 0xC1, 0x07)),
                );
            }
        }
        actions
    }

    fn is_active_slot(&self, slot: Slot, layers: &[&str]) -> bool {
        match slot {
            Slot::NonVisuals => self.is_non_visuals(),
            Slot::Form => self.is_form(),
            Slot::Layer(i) => self.active_layer().is_some_and(|a| a.eq_ignore_ascii_case(layers[i])),
            Slot::Add => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn layer_tab(
        &mut self,
        ui: &mut Ui,
        painter: &egui::Painter,
        rect: Rect,
        name: &str,
        index: usize,
        font: &FontId,
        tr: &crate::i18n::Tr,
        layout: &BarLayout,
        actions: &mut Vec<TabAction>,
    ) {
        let active = self.active_layer().is_some_and(|a| a.eq_ignore_ascii_case(name));
        let (face, ink) = if active { (ACTIVE_FACE, ACTIVE_TEXT) } else { (INACTIVE_FACE, INACTIVE_TEXT) };
        painter.add(Shape::convex_polygon(slanted(rect), face, Stroke::new(1.0, EDGE)));

        // The visibility box and the red ✕, at the tab's right (R23). Their own
        // widgets, registered AFTER the tab body below so a click on them is theirs.
        let TabParts { label: label_r, check: box_r, cross: cross_r } = layer_tab_parts(rect);

        // Rename in place (R26): a double-click turns the label into a field.
        let renaming = matches!(&self.rename, Some((n, _)) if n.eq_ignore_ascii_case(name));
        let body = ui.interact(
            rect,
            ui.id().with(("layer-tab", index)),
            Sense::click_and_drag(),
        );
        if renaming {
            if let Some((_, buf)) = self.rename.as_mut() {
                let edit = egui::TextEdit::singleline(buf)
                    .font(font.clone())
                    .desired_width(label_r.width())
                    .frame(egui::Frame::NONE)
                    .text_color(ink);
                let resp = ui.put(label_r, edit);
                resp.request_focus();
                let (enter, esc) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
                if enter || (resp.lost_focus() && !esc) {
                    let to = buf.trim().to_owned();
                    self.rename = None;
                    // A change of letter case alone is a rename; so is a new name.
                    if !to.is_empty() && to != name {
                        actions.push(TabAction::Rename { layer: name.to_owned(), to });
                    }
                } else if esc {
                    self.rename = None;
                }
            }
        } else {
            painter.text(label_r.left_center(), Align2::LEFT_CENTER, name, font.clone(), ink);
            let hint = tr.layer_tab_layer_hint.replacen("{}", name, 1);
            let body = body.on_hover_text(hint);
            if body.double_clicked() {
                self.rename = Some((name.to_owned(), name.to_owned()));
            } else if body.clicked() {
                actions.push(TabAction::SelectLayer(name.to_owned()));
            }
            // Re-stacking by dragging the tab (R26).
            if body.drag_started() {
                // Where the pointer went DOWN on the tab, not where it is when egui
                // decides this is a drag: those differ by the drag threshold, and
                // a quick flick would otherwise drop the tab short of where it was
                // taken.
                let down = ui.input(|i| i.pointer.press_origin()).or_else(|| body.interact_pointer_pos());
                if let Some(p) = down {
                    self.drag = Some(TabDrag { from: index, grab: p.x - rect.min.x, moved: false });
                }
            }
            if body.dragged() {
                if let Some(d) = self.drag.as_mut() {
                    if body.drag_delta().length() > 0.0 {
                        d.moved = true;
                    }
                }
            }
            if body.drag_stopped() {
                if let (Some(d), Some(pos)) = (self.drag.take(), body.interact_pointer_pos()) {
                    if d.moved {
                        let bar_left = rect.min.x - layout.placed.iter().find(|p| p.slot == Slot::Layer(index)).map_or(0.0, |p| p.x);
                        let to = drop_position(layout, pos.x - bar_left - d.grab + rect.width() * 0.5, d.from);
                        if to != d.from {
                            actions.push(TabAction::Move { from: d.from, to });
                        }
                    }
                }
            }
        }

        // The visibility box: a boxed X when the layer is shown (R25).
        let shown = self.is_shown(name);
        painter.rect_stroke(box_r, 1.0, Stroke::new(1.0, if active { Color32::WHITE } else { Color32::BLACK }), egui::StrokeKind::Inside);
        if shown {
            let m = 3.0;
            let col = if active { Color32::WHITE } else { Color32::BLACK };
            painter.line_segment([box_r.min + vec2(m, m), box_r.max - vec2(m, m)], Stroke::new(1.6, col));
            painter.line_segment([pos2(box_r.max.x - m, box_r.min.y + m), pos2(box_r.min.x + m, box_r.max.y - m)], Stroke::new(1.6, col));
        }
        let box_resp = ui
            .interact(box_r.expand(2.0), ui.id().with(("layer-box", index)), Sense::click())
            .on_hover_text(tr.layer_tab_visible_hint);
        if box_resp.clicked() {
            actions.push(TabAction::ToggleShown(name.to_owned()));
        }

        // The red ✕ (R23, R63).
        let hover = ui.interact(cross_r.expand(3.0), ui.id().with(("layer-cross", index)), Sense::click());
        let c = cross_r.center();
        let d = CROSS * 0.5 - 1.5;
        // Red on the active tab's blue reads at barely 1.2:1, so there the ✕ sits
        // on a white chip: red on white is 5:1, as it is on an inactive tab.
        if active {
            painter.circle_filled(c, CROSS * 0.5 + 1.5, INACTIVE_FACE);
        }
        let stroke = Stroke::new(if hover.hovered() { 2.6 } else { 2.0 }, CROSS_RED);
        painter.line_segment([c + vec2(-d, -d), c + vec2(d, d)], stroke);
        painter.line_segment([c + vec2(d, -d), c + vec2(-d, d)], stroke);
        if hover.on_hover_text(tr.layer_tab_delete_hint).clicked() {
            actions.push(TabAction::RequestDelete(name.to_owned()));
        }
    }
}

/// A fixed tab (`Non-Visuals`, `Form`): face, label and one click target.
#[allow(clippy::too_many_arguments)]
fn tab_face(
    ui: &mut Ui,
    painter: &egui::Painter,
    rect: Rect,
    label: &str,
    active: bool,
    id: egui::Id,
    font: &FontId,
    hint: &str,
) -> egui::Response {
    let (face, ink) = if active { (ACTIVE_FACE, ACTIVE_TEXT) } else { (INACTIVE_FACE, INACTIVE_TEXT) };
    painter.add(Shape::convex_polygon(slanted(rect), face, Stroke::new(1.0, EDGE)));
    painter.text(rect.center(), Align2::CENTER_CENTER, label, font.clone(), ink);
    ui.interact(rect, id, Sense::click()).on_hover_text(hint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::Layer;

    fn form_with(names: &[&str]) -> Form {
        let mut f = Form::new("F", "F", 640, 480);
        f.layers = names.iter().map(|n| Layer::new(*n)).collect();
        f
    }

    // ── R60, R61: selecting a layer ─────────────────────────────────────────

    #[test]
    fn selecting_a_layer_shows_it_and_hides_every_other() {
        let mut t = LayerTabs::default();
        t.set_shown("Layer-1", true);
        t.set_shown("Layer-3", true);
        assert!(t.is_shown("Layer-1") && t.is_shown("layer-3") && !t.is_shown("Layer-2"));
        t.select_layer("Layer-2");
        assert_eq!(t.active(), &ActiveTab::Layer("Layer-2".into()));
        assert!(t.is_shown("Layer-2"), "a layer whose box was off is turned on by selecting it (R60)");
        assert!(!t.is_shown("Layer-1") && !t.is_shown("Layer-3"), "the others are hidden (R61)");
        // The box may be ticked afterwards; nothing hides it again until another layer is selected.
        t.set_shown("Layer-3", true);
        assert!(t.is_shown("Layer-3") && t.is_shown("Layer-2"));
        t.select_layer("Layer-1");
        assert!(t.is_shown("Layer-1") && !t.is_shown("Layer-2") && !t.is_shown("Layer-3"));
    }

    #[test]
    fn selecting_form_or_non_visuals_hides_nothing() {
        let mut t = LayerTabs::default();
        t.select_layer("Layer-1");
        t.select_form();
        assert!(t.is_form() && t.is_shown("Layer-1"), "Form is selected and the layer stays shown over it");
        t.select_non_visuals();
        assert!(t.is_non_visuals() && t.is_shown("Layer-1"));
        t.select_form();
        assert_eq!(t.active(), &ActiveTab::Form);
    }

    // ── R62: the visibility box never changes the tab ───────────────────────

    #[test]
    fn the_visibility_box_never_changes_the_selected_tab() {
        for start in [ActiveTab::Form, ActiveTab::NonVisuals, ActiveTab::Layer("Layer-1".into())] {
            let mut t = LayerTabs::default();
            match &start {
                ActiveTab::Form => t.select_form(),
                ActiveTab::NonVisuals => t.select_non_visuals(),
                ActiveTab::Layer(n) => t.select_layer(n),
            }
            for layer in ["Layer-1", "Layer-2"] {
                t.set_shown(layer, false);
                assert_eq!(t.active(), &start, "unticking {layer}");
                t.set_shown(layer, true);
                assert_eq!(t.active(), &start, "ticking {layer}");
            }
        }
    }

    #[test]
    fn the_shown_set_is_never_part_of_the_form() {
        // Nothing about showing a layer is in `Form`, so choosing a tab cannot
        // mark it modified or make an undo step (R25, R32, Q26).
        let form = form_with(&["Layer-1"]);
        let before = cobolt_forms::form_to_string(&form).unwrap();
        let mut t = LayerTabs::default();
        t.select_layer("Layer-1");
        t.set_shown("Layer-1", false);
        assert_eq!(cobolt_forms::form_to_string(&form).unwrap(), before);
    }

    // ── following the form ─────────────────────────────────────────────────

    #[test]
    fn a_rename_is_followed_and_a_deletion_falls_back_to_form() {
        let mut t = LayerTabs::default();
        t.select_layer("Layer-1");
        t.retarget("layer-1", "Busy");
        assert_eq!(t.active(), &ActiveTab::Layer("Busy".into()));
        assert!(t.is_shown("Busy") && !t.is_shown("Layer-1"));
        t.forget("BUSY");
        assert_eq!(t.active(), &ActiveTab::Form, "Q27: the base after a delete");
        assert!(!t.is_shown("Busy"));
        // Forgetting a layer that is not the active one leaves the tab alone.
        t.select_layer("A");
        t.forget("B");
        assert_eq!(t.active(), &ActiveTab::Layer("A".into()));
    }

    #[test]
    fn reconcile_drops_what_the_form_no_longer_has() {
        let mut t = LayerTabs::default();
        t.select_layer("Layer-2");
        t.set_shown("Layer-1", true);
        t.reconcile(&form_with(&["Layer-1"]));
        assert_eq!(t.active(), &ActiveTab::Form, "the active layer is gone");
        assert!(t.is_shown("Layer-1") && !t.is_shown("Layer-2"));
        t.reconcile(&form_with(&[]));
        assert!(!t.is_shown("Layer-1"));
    }

    // ── R23, R26: the layout ───────────────────────────────────────────────

    fn names(l: &BarLayout) -> Vec<Slot> {
        l.placed.iter().map(|p| p.slot).collect()
    }

    #[test]
    fn a_form_with_no_layers_shows_non_visuals_form_and_the_plus_in_that_order() {
        let l = bar_layout(60.0, 30.0, &[], 800.0, 0.0);
        assert_eq!(names(&l), [Slot::NonVisuals, Slot::Form, Slot::Add]);
        assert!(!l.overflow && l.max_scroll == 0.0);
        let xs: Vec<f32> = l.placed.iter().map(|p| p.x).collect();
        assert!(xs.windows(2).all(|w| w[0] < w[1]), "left to right: {xs:?}");
    }

    #[test]
    fn layer_tabs_follow_form_in_stack_order_and_the_plus_comes_last() {
        let l = bar_layout(60.0, 30.0, &[40.0, 50.0, 45.0], 1200.0, 0.0);
        assert_eq!(
            names(&l),
            [Slot::NonVisuals, Slot::Form, Slot::Layer(0), Slot::Layer(1), Slot::Layer(2), Slot::Add]
        );
        let x = |s: Slot| l.placed.iter().find(|p| p.slot == s).unwrap().x;
        assert!(x(Slot::Layer(0)) < x(Slot::Layer(1)) && x(Slot::Layer(2)) < x(Slot::Add));
    }

    #[test]
    fn when_the_tabs_do_not_fit_the_arrows_appear_and_non_visuals_stays_pinned() {
        let texts: Vec<f32> = (0..64).map(|_| 60.0).collect();
        let at0 = bar_layout(60.0, 30.0, &texts, 700.0, 0.0);
        assert!(at0.overflow, "64 layers do not fit 700 px");
        assert!(at0.max_scroll > 0.0);
        let scrolled = bar_layout(60.0, 30.0, &texts, 700.0, 500.0);
        let nv = |l: &BarLayout| l.placed[0];
        assert_eq!(nv(&at0), nv(&scrolled), "Non-Visuals does not scroll (Q21)");
        let form = |l: &BarLayout| l.placed.iter().find(|p| p.slot == Slot::Form).unwrap().x;
        assert!(form(&scrolled) < form(&at0), "the others shift left");
        // A scroll past the end is held at the end.
        let past = bar_layout(60.0, 30.0, &texts, 700.0, 1.0e9);
        let far = bar_layout(60.0, 30.0, &texts, 700.0, at0.max_scroll);
        assert_eq!(past, far);
        // The tabs are cut to the room the arrows leave.
        assert!(at0.clip.1 <= 700.0 - 2.0 * ARROW_W + 0.01, "{:?}", at0.clip);
    }

    #[test]
    fn the_bar_never_asks_for_more_than_its_fixed_height() {
        assert!(TAB_H < BAR_H);
        assert!(bar_layout(60.0, 30.0, &[], 10.0, 0.0).overflow, "even a narrow bar only scrolls");
    }

    #[test]
    fn a_dropped_tab_lands_among_the_layers_and_never_before_form() {
        let l = bar_layout(60.0, 30.0, &[50.0, 50.0, 50.0], 1200.0, 0.0);
        let centre = |i: usize| {
            let p = l.placed.iter().find(|p| p.slot == Slot::Layer(i)).unwrap();
            p.x + p.w * 0.5
        };
        // Layer 0 dragged to the far right: it ends up last (position 2 of 3).
        assert_eq!(drop_position(&l, 10_000.0, 0), 2);
        // Layer 2 dragged to the far left of everything: position 0, never before Form.
        assert_eq!(drop_position(&l, -500.0, 2), 0);
        // Dropped where it already is: no move.
        for i in 0..3 {
            assert_eq!(drop_position(&l, centre(i), i), i, "layer {i} dropped on itself");
        }
    }

    // ── R43: the colours ───────────────────────────────────────────────────

    #[test]
    fn the_active_tab_is_blue_with_white_text_and_the_others_the_reverse() {
        assert_eq!(ACTIVE_TEXT, Color32::WHITE);
        assert_eq!(INACTIVE_FACE, Color32::WHITE);
        assert_eq!(INACTIVE_TEXT, ACTIVE_FACE, "white face, blue text");
        assert_ne!(ACTIVE_FACE, INACTIVE_FACE);
        // The red ✕ must read on both faces (contrast against blue and white).
        let ratio = |a: Color32, b: Color32| {
            let l = |c: Color32| {
                let f = |v: u8| {
                    let s = v as f32 / 255.0;
                    if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
                };
                0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
            };
            let (hi, lo) = (l(a).max(l(b)), l(a).min(l(b)));
            (hi + 0.05) / (lo + 0.05)
        };
        assert!(ratio(ACTIVE_FACE, ACTIVE_TEXT) >= 4.5, "white on blue");
        assert!(ratio(INACTIVE_FACE, INACTIVE_TEXT) >= 4.5, "blue on white");
        // The ✕ must read on BOTH faces. On white it does; on the active tab's blue
        // it would not (the measurement below), which is why `layer_tab` puts it
        // on a white chip there — so what it sits on is white either way.
        assert!(ratio(CROSS_RED, INACTIVE_FACE) >= 3.0, "red ✕ on white: {}", ratio(CROSS_RED, INACTIVE_FACE));
        assert!(
            ratio(CROSS_RED, ACTIVE_FACE) < 3.0,
            "if red ever reads on the blue, the white chip behind the ✕ on the active tab is no longer needed"
        );
        println!(
            "091 R43 contrast — white on blue {:.1}:1, blue on white {:.1}:1, red ✕ on white {:.1}:1 (also on the white chip an active tab gives it); red directly on the blue would be {:.1}:1",
            ratio(ACTIVE_FACE, ACTIVE_TEXT),
            ratio(INACTIVE_FACE, INACTIVE_TEXT),
            ratio(CROSS_RED, INACTIVE_FACE),
            ratio(CROSS_RED, ACTIVE_FACE),
        );
    }
}
