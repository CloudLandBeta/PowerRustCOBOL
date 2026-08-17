// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Toolbar Editor — where a toolbar is actually built.
//!
//! A toolbar has far too many knobs for the properties pane: groups, and inside
//! each group buttons, and on each button an icon, a size, four colours, a
//! gradient, a shadow and an action. So none of it lives there. The pane offers
//! one button, and everything is set here (operator decision, 2026-08-16).
//!
//! Two panes, the way the menu editor is laid out: the tree of groups and their
//! buttons on the left, the properties of whatever is selected on the right.
//!
//! Nothing is written to the control until **Save**. The modal edits its own copy
//! of the definition, so Cancel really cancels — a developer can open it, try
//! three arrangements, and walk away with the toolbar they started with.

use cobolt_forms::toolbar::{
    ButtonStyle, ToolbarAction, ToolbarButton, ToolbarDef, ToolbarGroup,
};
use eframe::egui;
use egui::Color32;

/// What the right-hand pane is editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    /// Nothing picked yet.
    None,
    Group(usize),
    Button(usize, usize),
}

impl Default for Selection {
    fn default() -> Self {
        Self::None
    }
}

pub struct ToolbarEditorModal {
    /// The ToolBar control being edited.
    pub ctrl_id: String,
    /// The working copy. Only Save puts it back on the control.
    pub def: ToolbarDef,
    pub selected: Selection,
    /// Icon picker state, mirroring the menu editor's.
    icon_picker_open: bool,
    icon_search: String,
    icon_picker_gen: u32,
    /// Split between the tree and the properties pane, 0.0-1.0. The developer's
    /// own drag; never derived from the available width (that is the feedback
    /// loop that makes a pane inflate).
    split_ratio: f32,
    /// A button whose handler the developer asked to edit: `(button id, event)`.
    ///
    /// Editing code happens in the COBOL editor, not in here — so this closes the
    /// modal, saving the definition on the way out, and the designer opens the
    /// handler. Nesting a code editor inside this window would put two modals on
    /// screen at once and leave the developer with two Saves to reason about.
    edit_event: Option<(String, String)>,
}

impl ToolbarEditorModal {
    pub fn new(ctrl_id: String, def: ToolbarDef) -> Self {
        let selected = if def.groups.is_empty() {
            Selection::None
        } else {
            Selection::Group(0)
        };
        Self {
            ctrl_id,
            def,
            selected,
            icon_picker_open: false,
            icon_search: String::new(),
            icon_picker_gen: 0,
            split_ratio: 0.38,
            edit_event: None,
        }
    }

    fn group(&self, i: usize) -> Option<&ToolbarGroup> {
        self.def.groups.get(i)
    }

    fn add_group(&mut self) {
        let id = self.def.next_group_id();
        let n = self.def.groups.len() + 1;
        self.def
            .groups
            .push(ToolbarGroup::new(id, format!("Group {n}")));
        self.selected = Selection::Group(self.def.groups.len() - 1);
    }

    /// Add a button to the selected group — or to the last group, since "add a
    /// button" with a button selected obviously means "another one here".
    fn add_button(&mut self) {
        let gi = match self.selected {
            Selection::Group(g) => g,
            Selection::Button(g, _) => g,
            Selection::None => match self.def.groups.len() {
                0 => {
                    self.add_group();
                    0
                }
                n => n - 1,
            },
        };
        let id = self.def.next_button_id();
        // Carry the LAST button's settings over (operator, 2026-08-17). Building
        // a toolbar means six buttons that differ only in icon and action, and
        // re-entering the size and colours on each was the work.
        //
        // The last button anywhere on the bar, not just in this group, so the
        // run of buttons a developer is adding keeps its look as they move on to
        // the next group. Its identity is NOT carried: no label, no icon, no
        // tooltip, no action — those are what makes it a different button.
        let inherited = self
            .def
            .buttons()
            .last()
            .map(|(_, b)| b.style.clone())
            .unwrap_or_default();
        let Some(group) = self.def.groups.get_mut(gi) else {
            return;
        };
        let n = group.buttons.len() + 1;
        let mut button = ToolbarButton::new(id, format!("Button {n}"));
        button.style = inherited;
        group.buttons.push(button);
        self.selected = Selection::Button(gi, group.buttons.len() - 1);
    }

    /// Delete whatever is selected. A group takes its buttons with it, which is
    /// what deleting a group means — and it is the developer pressing an explicit
    /// Delete on a selection they made, then still able to Cancel the whole
    /// modal, so the code they wrote is never lost to a stray click.
    fn delete_selected(&mut self) {
        match self.selected {
            Selection::None => {}
            Selection::Group(g) => {
                if g < self.def.groups.len() {
                    self.def.groups.remove(g);
                    self.selected = if self.def.groups.is_empty() {
                        Selection::None
                    } else {
                        Selection::Group(g.min(self.def.groups.len() - 1))
                    };
                }
            }
            Selection::Button(g, b) => {
                if let Some(group) = self.def.groups.get_mut(g) {
                    if b < group.buttons.len() {
                        group.buttons.remove(b);
                        self.selected = if group.buttons.is_empty() {
                            Selection::Group(g)
                        } else {
                            Selection::Button(g, b.min(group.buttons.len() - 1))
                        };
                    }
                }
            }
        }
    }

    /// Move the selection one place earlier (`-1`) or later (`+1`) among its
    /// siblings. Order is what a toolbar IS, so this is not a nicety.
    fn nudge(&mut self, delta: i64) {
        match self.selected {
            Selection::None => {}
            Selection::Group(g) => {
                let to = g as i64 + delta;
                if to >= 0 && (to as usize) < self.def.groups.len() {
                    self.def.groups.swap(g, to as usize);
                    self.selected = Selection::Group(to as usize);
                }
            }
            Selection::Button(g, b) => {
                if let Some(group) = self.def.groups.get_mut(g) {
                    let to = b as i64 + delta;
                    if to >= 0 && (to as usize) < group.buttons.len() {
                        group.buttons.swap(b, to as usize);
                        self.selected = Selection::Button(g, to as usize);
                    }
                }
            }
        }
    }
}

/// What the caller must do when the modal closes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorOutcome {
    /// Still open.
    Open,
    /// Closed without changing the control.
    Cancelled,
    /// Save the JSON onto the control's `ToolbarLayout` property.
    Save(String),
    /// Save the JSON, then open the COBOL editor on this button's handler.
    /// `(json, button id, event)`.
    SaveAndEditEvent(String, String, String),
}

/// Draw the modal. Returns what the designer should do about it.
pub fn show(modal: &mut ToolbarEditorModal, ctx: &egui::Context) -> EditorOutcome {
    let tr = crate::i18n::current_tr(ctx);
    let screen = ctx.content_rect();

    // The scrim: this is modal, and it should look it.
    egui::Area::new(egui::Id::new("toolbar_editor_scrim"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.painter().rect_filled(
                screen,
                0.0,
                Color32::from_rgba_premultiplied(0, 0, 0, 140),
            );
        });

    let mut outcome = EditorOutcome::Open;
    // 70 % of the HOST window's height (operator, 2026-08-17) — a toolbar is
    // arranged while looking at it, and the panes need the room.
    let modal_h = modal_height(screen.height());
    // `default_size`/`max_size` are the CONTENT box; the title bar is added on
    // top, so 70 % of the host means 70 % minus the bar.
    let default_size = egui::vec2(
        900.0_f32.min(screen.width() - 40.0).max(520.0),
        content_height(modal_h),
    );
    // The ceiling IS the opening size, exactly as the event-handler modal does
    // it. A `Window` takes its size from its CONTENT, so a ceiling looser than
    // the default is an invitation to grow: the previous one was
    // `screen.height() - 40`, which let the panes push the window out to the
    // edges. The developer's own drag still governs everything below this.
    let panes_h = panes_height(modal_h);

    egui::Window::new(tr.toolbar_editor_title)
        .id(egui::Id::new("toolbar_editor_modal"))
        .collapsible(false)
        .resizable(true)
        .default_size(default_size)
        .max_size(default_size)
        .default_pos(screen.center() - default_size * 0.5)
        .frame(egui::Frame::window(&ctx.global_style()).inner_margin(egui::Margin::same(12)))
        .show(ctx, |ui| {
            // ── The row of actions ────────────────────────────────────────
            ui.horizontal_wrapped(|ui| {
                if ui.small_button(tr.toolbar_add_group).clicked() {
                    modal.add_group();
                }
                if ui.small_button(tr.toolbar_add_button).clicked() {
                    modal.add_button();
                }
                ui.separator();
                let has_selection = modal.selected != Selection::None;
                if ui
                    .add_enabled(has_selection, egui::Button::new("▲").small())
                    .clicked()
                {
                    modal.nudge(-1);
                }
                if ui
                    .add_enabled(has_selection, egui::Button::new("▼").small())
                    .clicked()
                {
                    modal.nudge(1);
                }
                if ui
                    .add_enabled(has_selection, egui::Button::new(tr.toolbar_delete).small())
                    .clicked()
                {
                    modal.delete_selected();
                }
            });
            ui.add_space(6.0);
            ui.separator();

            // ── A live preview of the bar being built ─────────────────────
            //    Through the same renderer the running form uses, so what is
            //    arranged here is what ships.
            let preview_h = 56.0;
            let (preview_rect, _) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), preview_h), egui::Sense::hover());
            ui.painter().rect_filled(
                preview_rect,
                6.0,
                ui.visuals().extreme_bg_color,
            );
            let inner = preview_rect.shrink(6.0);
            if modal.def.is_empty() {
                ui.painter().text(
                    inner.center(),
                    egui::Align2::CENTER_CENTER,
                    tr.toolbar_empty_hint,
                    egui::FontId::proportional(12.0),
                    ui.visuals().weak_text_color(),
                );
            } else {
                cobolt_forms::toolbar_paint::draw(
                    ui.painter(),
                    inner,
                    &modal.def,
                    1.0,
                    cobolt_forms::toolbar_paint::Interaction::inert(),
                );
            }
            ui.add_space(8.0);

            // ── Tree | properties ─────────────────────────────────────────
            //
            // Both panes are given the room the modal reserved for them —
            // `panes_h`, derived from the window's own opening height and NEVER
            // from `available_height()`, which is the feedback loop that makes a
            // pane and its window inflate each other.
            //
            // `auto_shrink([false, false])` is what makes the properties pane
            // FILL that box. Without it a ScrollArea shrinks to its content, so
            // a button with few properties drew a pane a fraction of the size of
            // the space it had (operator screenshot, 2026-08-17).
            let total_w = ui.available_width();
            let tree_w = tree_pane_width(total_w, modal.split_ratio);
            let tree_scroll_h = (panes_h - 22.0).max(PANE_MIN_H - 22.0);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(tree_w);
                    ui.set_min_height(panes_h);
                    ui.label(egui::RichText::new(tr.toolbar_groups).strong());
                    egui::ScrollArea::vertical()
                        .id_salt("toolbar_tree")
                        .auto_shrink([false, false])
                        .min_scrolled_height(tree_scroll_h)
                        .max_height(tree_scroll_h)
                        .show(ui, |ui| show_tree(modal, ui));
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_min_height(panes_h);
                    egui::ScrollArea::vertical()
                        .id_salt("toolbar_props")
                        .auto_shrink([false, false])
                        .min_scrolled_height(panes_h)
                        .max_height(panes_h)
                        .show(ui, |ui| show_props(modal, ui, &tr));
                });
            });

            // A click on a button's "Edit code" closes the editor, keeping the
            // definition, and hands over to the COBOL editor.
            if let Some((button_id, event)) = modal.edit_event.take() {
                match modal.def.to_json() {
                    Ok(json) => {
                        outcome = EditorOutcome::SaveAndEditEvent(json, button_id, event)
                    }
                    Err(e) => {
                        tracing::error!(target: "toolbar", "toolbar save failed: {e}");
                        outcome = EditorOutcome::Cancelled;
                    }
                }
            }

            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(tr.btn_save).clicked() {
                    match modal.def.to_json() {
                        Ok(json) => outcome = EditorOutcome::Save(json),
                        // Serialising a definition we just built cannot really
                        // fail, but a silent no-op on Save would be the worst
                        // possible answer if it did.
                        Err(e) => {
                            tracing::error!(target: "toolbar", "toolbar save failed: {e}");
                            outcome = EditorOutcome::Cancelled;
                        }
                    }
                }
                if ui.button(tr.btn_cancel).clicked() {
                    outcome = EditorOutcome::Cancelled;
                }
                ui.separator();
                ui.label(
                    egui::RichText::new(format!(
                        "{} group(s), {} button(s), {}px wide",
                        modal.def.groups.len(),
                        modal.def.buttons().count(),
                        modal.def.layout_width()
                    ))
                    .small()
                    .weak(),
                );
            });
        });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && outcome == EditorOutcome::Open {
        outcome = EditorOutcome::Cancelled;
    }
    outcome
}

/// Width of the tree pane, given the room available and the developer's split.
///
/// The tree wants at least [`TREE_MIN_W`] and the properties pane at least
/// [`PROPS_MIN_W`]. A modal narrower than both cannot honour both, and the tree
/// gives way — the properties pane is where the work happens.
///
/// This exists as a function because it was a `clamp(180.0, total_w - 260.0)`
/// inline, and `f32::clamp` PANICS when its low bound exceeds its high one. Any
/// modal under 440 px wide killed the event loop (operator, 2026-08-16). Never
/// hand `clamp` a computed maximum without pinning it above the minimum first.
fn tree_pane_width(total_w: f32, split_ratio: f32) -> f32 {
    let high = (total_w - PROPS_MIN_W).max(TREE_MIN_W);
    // `clamp` PROPAGATES NaN rather than clamping it, so a bad ratio would come
    // straight back out as a NaN width and land in a layout call.
    let ratio = if split_ratio.is_finite() {
        split_ratio.clamp(0.0, 1.0)
    } else {
        0.38
    };
    let want = total_w.max(0.0) * ratio;
    if want.is_finite() {
        want.clamp(TREE_MIN_W, high)
    } else {
        TREE_MIN_W
    }
}

const TREE_MIN_W: f32 = 180.0;
const PROPS_MIN_W: f32 = 260.0;

/// Smallest the modal opens at, however short the host window is.
const MODAL_MIN_H: f32 = 380.0;
/// The window's title bar, which `Window::max_size` does NOT cover.
///
/// `max_size` bounds the CONTENT area; the title bar is added on top of it. So a
/// content ceiling of 70 % produced a window of 70 % + 38 px, at every host size
/// tested — a constant offset, which is what identified it. Asking for 70 % of
/// the host therefore means giving the content 70 % **minus this**.
const TITLE_BAR_H: f32 = 38.0;

/// Everything inside the content area that is NOT the two panes: the inner
/// margins, the action row, the live preview strip, the separators and spacing,
/// and the Save/Cancel footer.
///
/// Deliberately generous. The panes are sized as `content − this`, and if that
/// comes out too large the content wants more room than the ceiling allows — at
/// which point the footer, being last, is what gets pushed out of view.
const MODAL_CHROME_H: f32 = 212.0;
/// Smallest either pane is given, whatever is left over.
const PANE_MIN_H: f32 = 160.0;

/// The window's CONTENT height for a modal whose total is `modal_h` — what goes
/// to `default_size`/`max_size`, the title bar taken off.
fn content_height(modal_h: f32) -> f32 {
    (modal_h - TITLE_BAR_H).max(MODAL_MIN_H - TITLE_BAR_H)
}

/// Height reserved for the two panes in a modal of `modal_h`.
///
/// Kept as a function so the regression test can assert the arithmetic without
/// standing up a window: the panes must grow with the host and must never be
/// computed from the space they are currently sitting in.
fn panes_height(modal_h: f32) -> f32 {
    (content_height(modal_h) - MODAL_CHROME_H).max(PANE_MIN_H)
}

/// The modal's opening height for a host window of `host_h` — 70 % of it.
fn modal_height(host_h: f32) -> f32 {
    let host_h = if host_h.is_finite() { host_h.max(0.0) } else { MODAL_MIN_H };
    (host_h * 0.70).clamp(MODAL_MIN_H, (host_h - 40.0).max(MODAL_MIN_H))
}

fn show_tree(modal: &mut ToolbarEditorModal, ui: &mut egui::Ui) {
    let groups = modal.def.groups.len();
    for gi in 0..groups {
        let (label, buttons, framed, sep) = {
            let g = &modal.def.groups[gi];
            (
                if g.label.trim().is_empty() {
                    g.id.clone()
                } else {
                    g.label.clone()
                },
                g.buttons.len(),
                g.draws_frame(),
                g.separator_after,
            )
        };
        let selected = modal.selected == Selection::Group(gi);
        let text = format!(
            "{} {label}  ({buttons}){}",
            if framed { "▣" } else { "▢" },
            if sep { "  ┆" } else { "" }
        );
        if ui.selectable_label(selected, text).clicked() {
            modal.selected = Selection::Group(gi);
        }
        for bi in 0..buttons {
            let b = &modal.def.groups[gi].buttons[bi];
            let icon = if b.icon.trim().is_empty() { "·" } else { "◆" };
            let name = if b.label.trim().is_empty() {
                b.id.clone()
            } else {
                b.label.clone()
            };
            let verb = b.action().verb();
            let text = format!("      {icon} {name}   [{verb}]");
            let text = if b.enabled {
                egui::RichText::new(text)
            } else {
                egui::RichText::new(text).weak()
            };
            if ui
                .selectable_label(modal.selected == Selection::Button(gi, bi), text)
                .clicked()
            {
                modal.selected = Selection::Button(gi, bi);
            }
        }
    }
}

fn show_props(modal: &mut ToolbarEditorModal, ui: &mut egui::Ui, tr: &crate::i18n::Tr) {
    match modal.selected {
        Selection::None => {
            ui.label(egui::RichText::new(tr.toolbar_empty_hint).weak());
        }
        Selection::Group(gi) => {
            ui.label(egui::RichText::new(tr.toolbar_group_props).strong());
            let Some(g) = modal.def.groups.get_mut(gi) else {
                return;
            };
            prop_grid(ui, "group", |ui| {
                text_row(ui, "Name:", &mut g.label);
                section(ui, tr.sec_appearance);
                combo_row(ui, "Border:", &mut g.border_style, &["Single", "None", "Fixed3D"]);
                color_row(ui, "Border colour:", &mut g.border_color, "theme");
                int_row(ui, "Border width:", &mut g.border_width, 0, 40);
                int_row(ui, "Corner radius:", &mut g.corner_radius, 0, 60);
                int_row(ui, "Padding:", &mut g.padding, 0, 60);
                color_row(ui, "Background:", &mut g.background_color, "theme");
                ui.label(tr.toolbar_separator_after);
                ui.checkbox(&mut g.separator_after, "");
                ui.end_row();
                if g.separator_after {
                    int_row(ui, "Separator width:", &mut g.separator_width, 0, 200);
                }
                // The same appearance a button has, applied to every button in
                // the group unless that button says otherwise.
                section(ui, "Defaults for this group's buttons");
                style_rows(ui, &mut g.button_defaults, &ButtonStyle::default(), "theme");
            });
        }
        Selection::Button(gi, bi) => {
            ui.label(egui::RichText::new(tr.toolbar_button_props).strong());
            let mut open_picker = false;
            let mut edit_event: Option<(String, String)> = None;
            // What the group would give this button, so every inheritable row can
            // show the value it is actually inheriting.
            let group_defaults = modal
                .def
                .groups
                .get(gi)
                .map(|g| g.button_defaults.clone())
                .unwrap_or_default();
            {
                let Some(b) = modal
                    .def
                    .groups
                    .get_mut(gi)
                    .and_then(|g| g.buttons.get_mut(bi))
                else {
                    return;
                };
                prop_grid(ui, "button", |ui| {
                    // A button shows a label OR an icon, never both. Editing one
                    // clears the other, and the row says so.
                    let mut label = b.label.clone();
                    if text_row(ui, "Label:", &mut label) {
                        b.set_label(label);
                    }
                    ui.label("Icon:");
                    ui.horizontal(|ui| {
                        let shown = if b.icon.trim().is_empty() {
                            "(none)".to_owned()
                        } else {
                            b.icon.clone()
                        };
                        if ui.button(shown).clicked() {
                            open_picker = true;
                        }
                        if !b.icon.trim().is_empty() && ui.small_button("✕").clicked() {
                            b.icon.clear();
                        }
                    });
                    ui.end_row();
                    ui.label("");
                    ui.label(
                        egui::RichText::new(if b.icon.trim().is_empty() {
                            "A label or an icon — setting one clears the other."
                        } else {
                            "Icon set; a label would replace it."
                        })
                        .small()
                        .weak(),
                    );
                    ui.end_row();
                    text_row(ui, "Tooltip:", &mut b.tooltip);
                    bool_row(ui, "Enabled:", &mut b.enabled);

                    section(ui, tr.toolbar_action);
                    action_row(ui, b, tr);

                    section(ui, tr.sec_events);
                    if let Some(event) = events_rows(ui, b, tr) {
                        edit_event = Some((b.id.clone(), event));
                    }

                    section(ui, tr.sec_appearance);
                    style_rows(ui, &mut b.style, &group_defaults, "group");
                });
            }
            if let Some(target) = edit_event {
                modal.edit_event = Some(target);
            }
            if open_picker {
                modal.icon_picker_open = true;
                modal.icon_picker_gen += 1;
                modal.icon_search.clear();
            }
            show_icon_picker(modal, ui.ctx(), gi, bi);
        }
    }
}

/// The appearance rows, shared by a group's defaults and a button's overrides.
///
/// ONE function for both levels, so the two can never offer different fields —
/// which is the whole point of a group's settings being the buttons' defaults.
/// `inherited` is what the level above would give, shown greyed on a row the
/// developer has not set; `unset_hint` names that level ("group" on a button,
/// "theme" on a group).
fn style_rows(
    ui: &mut egui::Ui,
    style: &mut ButtonStyle,
    inherited: &ButtonStyle,
    unset_hint: &str,
) {
    use cobolt_forms::toolbar::{DEFAULT_BUTTON_SIZE, DEFAULT_CORNER_RADIUS, DEFAULT_ICON_SIZE};

    opt_int_row(
        ui,
        "Icon size:",
        &mut style.icon_size,
        4,
        128,
        inherited.icon_size.unwrap_or(DEFAULT_ICON_SIZE),
    );
    color_row(ui, "Icon colour:", &mut style.icon_color, unset_hint);
    opt_int_row(
        ui,
        "Width:",
        &mut style.width,
        8,
        400,
        inherited.width.unwrap_or(DEFAULT_BUTTON_SIZE.0),
    );
    opt_int_row(
        ui,
        "Height:",
        &mut style.height,
        8,
        400,
        inherited.height.unwrap_or(DEFAULT_BUTTON_SIZE.1),
    );
    opt_int_row(
        ui,
        "Corner radius:",
        &mut style.corner_radius,
        0,
        60,
        inherited.corner_radius.unwrap_or(DEFAULT_CORNER_RADIUS),
    );
    color_row(ui, "Background:", &mut style.background_color, unset_hint);
    color_row(ui, "Foreground:", &mut style.foreground_color, unset_hint);

    opt_bool_row(
        ui,
        "Gradient:",
        &mut style.gradient,
        inherited.gradient.unwrap_or(false),
    );
    if style.gradient.or(inherited.gradient).unwrap_or(false) {
        color_row(ui, "From:", &mut style.gradient_start_color, unset_hint);
        color_row(ui, "To:", &mut style.gradient_end_color, unset_hint);
        combo_row(
            ui,
            "Direction:",
            &mut style.gradient_direction,
            &["Vertical", "Horizontal", "Diagonal"],
        );
    }

    opt_bool_row(
        ui,
        "Drop shadow:",
        &mut style.shadow,
        inherited.shadow.unwrap_or(false),
    );
    if style.shadow.or(inherited.shadow).unwrap_or(false) {
        color_row(ui, "Shadow colour:", &mut style.shadow_color, unset_hint);
        opt_int_row(
            ui,
            "Opacity %:",
            &mut style.shadow_opacity,
            0,
            100,
            inherited.shadow_opacity.unwrap_or(25),
        );
        opt_int_row(
            ui,
            "Distance:",
            &mut style.shadow_distance,
            0,
            40,
            inherited.shadow_distance.unwrap_or(2),
        );
        opt_int_row(
            ui,
            "Blur:",
            &mut style.shadow_blur_strength,
            0,
            20,
            inherited.shadow_blur_strength.unwrap_or(0),
        );
    }
}

/// One row per event a button can raise, with a dot for "has code" and a link to
/// write it. Returns the event the developer asked to edit, if any.
///
/// This is what makes a button first-class: its own handler instead of one
/// `onClick` on the toolbar sorting out which button was pressed. Both still
/// work — the toolbar's `onClick` fires either way, and `LastButton` still names
/// the button — so a toolbar built the old way keeps behaving the old way.
///
/// The visual is the properties inspector's event row deliberately: a developer
/// who has bound a Button's `onClick` should recognise this on sight.
fn events_rows(
    ui: &mut egui::Ui,
    b: &ToolbarButton,
    tr: &crate::i18n::Tr,
) -> Option<String> {
    let mut asked: Option<String> = None;
    for event in cobolt_forms::toolbar::BUTTON_EVENTS {
        let bound = b.event(event);
        let has_code = bound.map(|e| e.has_code()).unwrap_or(false);
        let lines = bound.map(|e| e.code_line_count()).unwrap_or(0);
        ui.label(*event);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(if has_code { "●" } else { "○" }).color(if has_code {
                    Color32::from_rgb(100, 220, 100)
                } else {
                    Color32::from_rgb(120, 120, 120)
                }),
            );
            let link = ui
                .add(
                    egui::Label::new(
                        egui::RichText::new(tr.toolbar_edit_code)
                            .color(Color32::from_rgb(200, 200, 100)),
                    )
                    .sense(egui::Sense::click()),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(tr.toolbar_edit_code_hint);
            if has_code {
                ui.label(
                    egui::RichText::new(format!("({lines} {})", tr.hint_lines))
                        .small()
                        .weak(),
                );
            }
            if link.clicked() {
                asked = Some((*event).to_owned());
            }
        });
        ui.end_row();
    }
    asked
}

/// The action picker: a verb, and the target the verb needs.
fn action_row(ui: &mut egui::Ui, b: &mut ToolbarButton, _tr: &crate::i18n::Tr) {
    let current = b.action();
    let mut verb = current.verb().to_owned();
    let mut target = match &current {
        ToolbarAction::Procedure(t)
        | ToolbarAction::OpenModal(t)
        | ToolbarAction::Print(t)
        | ToolbarAction::RunApp(t)
        | ToolbarAction::OpenTerminal(t) => t.clone(),
        _ => String::new(),
    };
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label("Does:");
        egui::ComboBox::from_id_salt(("tb-action", b.id.clone()))
            .selected_text(verb.clone())
            .show_ui(ui, |ui| {
                for v in ToolbarAction::VERBS {
                    if ui.selectable_label(verb == *v, *v).clicked() {
                        verb = (*v).to_owned();
                        changed = true;
                    }
                }
            });
    });
    if ToolbarAction::takes_target(&verb) {
        let hint = match verb.as_str() {
            "procedure" => "the procedure to PERFORM",
            "open-modal" => "a STANDALONE form's name",
            "print" => "file path, or a data item holding one",
            "run-app" => "path to the application, then any arguments",
            "open-terminal" => "folder to open in (blank = the project's)",
            _ => "",
        };
        ui.horizontal(|ui| {
            ui.label("With:");
            if ui.text_edit_singleline(&mut target).changed() {
                changed = true;
            }
        });
        if !hint.is_empty() {
            ui.label(egui::RichText::new(hint).small().weak());
        }
    }
    // What this action means, in one line, so the developer is not guessing.
    let explain = match verb.as_str() {
        "event" => "Fires this button's onClick handler; your COBOL decides.",
        "procedure" => "PERFORMs one of the form's procedures.",
        "open-modal" => "Opens a standalone form as a modal window.",
        "print" => "Sends the file to the OS print dialog.",
        "share" => "Offers an image of this form's window to the OS share sheet.",
        "screenshot" => "Puts an image of this form's window on the clipboard.",
        "copy" => "Copies from the focused control to the clipboard.",
        "cut" => "Cuts from the focused control to the clipboard.",
        "paste" => "Pastes the clipboard into the focused control.",
        "run-app" => "Launches another application.",
        "open-terminal" => "Opens a terminal window.",
        _ => "",
    };
    if !explain.is_empty() {
        ui.label(egui::RichText::new(explain).small().weak());
    }
    if changed {
        let action = match verb.as_str() {
            "procedure" => ToolbarAction::Procedure(target),
            "open-modal" => ToolbarAction::OpenModal(target),
            "print" => ToolbarAction::Print(target),
            "run-app" => ToolbarAction::RunApp(target),
            "open-terminal" => ToolbarAction::OpenTerminal(target),
            "share" => ToolbarAction::Share,
            "screenshot" => ToolbarAction::Screenshot,
            "copy" => ToolbarAction::Copy,
            "cut" => ToolbarAction::Cut,
            "paste" => ToolbarAction::Paste,
            _ => ToolbarAction::Event,
        };
        b.action = action.to_action_string();
    }
}

fn show_icon_picker(modal: &mut ToolbarEditorModal, ctx: &egui::Context, gi: usize, bi: usize) {
    if !modal.icon_picker_open {
        return;
    }
    let mut chosen: Option<String> = None;
    let mut close = false;
    egui::Window::new("Icon")
        .id(egui::Id::new(("toolbar_icon_picker", modal.icon_picker_gen)))
        .collapsible(false)
        .resizable(true)
        .default_size([420.0, 380.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Find:");
                ui.text_edit_singleline(&mut modal.icon_search);
            });
            ui.separator();
            let needle = modal.icon_search.trim().to_ascii_lowercase();
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    egui::Grid::new("toolbar_icon_grid").show(ui, |ui| {
                        let mut n = 0;
                        for name in cobolt_forms::icons::menu_icon_names() {
                            if !needle.is_empty() && !name.to_ascii_lowercase().contains(&needle) {
                                continue;
                            }
                            let (rect, resp) = ui.allocate_exact_size(
                                egui::vec2(72.0, 56.0),
                                egui::Sense::click(),
                            );
                            if resp.hovered() {
                                ui.painter().rect_filled(
                                    rect,
                                    6.0,
                                    ui.visuals().widgets.hovered.bg_fill,
                                );
                            }
                            let icon_rect = egui::Rect::from_center_size(
                                egui::pos2(rect.center().x, rect.top() + 20.0),
                                egui::Vec2::splat(24.0),
                            );
                            cobolt_forms::icons::draw_menu_icon(
                                ui.painter(),
                                icon_rect,
                                name,
                                ui.visuals().text_color(),
                            );
                            ui.painter().text(
                                egui::pos2(rect.center().x, rect.bottom() - 4.0),
                                egui::Align2::CENTER_BOTTOM,
                                name,
                                egui::FontId::proportional(9.0),
                                ui.visuals().weak_text_color(),
                            );
                            if resp.clicked() {
                                chosen = Some(name.to_owned());
                            }
                            n += 1;
                            if n % 5 == 0 {
                                ui.end_row();
                            }
                        }
                    });
                });
            ui.separator();
            if ui.button("Close").clicked() {
                close = true;
            }
        });
    if let Some(name) = chosen {
        if let Some(b) = modal
            .def
            .groups
            .get_mut(gi)
            .and_then(|g| g.buttons.get_mut(bi))
        {
            // Through the setter, so choosing an icon takes the label away.
            b.set_icon(name);
        }
        close = true;
    }
    if close {
        modal.icon_picker_open = false;
    }
}

// ── Rows, in a two-column grid: label | value ─────────────────────────────────
//
// Every row is one `Grid` row, so the labels line up down the left and the
// widgets down the right (operator, 2026-08-17) — `ui.horizontal` per row left
// each widget wherever its own label ended, which is what the screenshot showed.
//
// Field labels are plain text here, as they are in `properties.rs` — only the
// modal's chrome and section headers go through `Tr`.

/// Open the two-column grid the rows below expect to be inside.
fn prop_grid<R>(ui: &mut egui::Ui, salt: &str, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Grid::new(format!("tb-grid-{salt}"))
        .num_columns(2)
        .spacing([10.0, 4.0])
        .striped(false)
        .show(ui, body)
        .inner
}

/// A section heading that spans both columns.
fn section(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).small().weak());
    ui.end_row();
}

fn text_row(ui: &mut egui::Ui, label: &str, value: &mut String) -> bool {
    ui.label(label);
    let changed = ui.text_edit_singleline(value).changed();
    ui.end_row();
    changed
}

fn bool_row(ui: &mut egui::Ui, label: &str, value: &mut bool) {
    ui.label(label);
    ui.checkbox(value, "");
    ui.end_row();
}

fn int_row(ui: &mut egui::Ui, label: &str, value: &mut i64, lo: i64, hi: i64) {
    ui.label(label);
    ui.add(egui::DragValue::new(value).range(lo..=hi).speed(0.4));
    ui.end_row();
}

/// An inheritable number: blank means "whatever my group says".
fn opt_int_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<i64>,
    lo: i64,
    hi: i64,
    inherited: i64,
) {
    ui.label(label);
    ui.horizontal(|ui| {
        let mut shown = value.unwrap_or(inherited);
        if ui
            .add(egui::DragValue::new(&mut shown).range(lo..=hi).speed(0.4))
            .changed()
        {
            *value = Some(shown);
        }
        match value {
            None => {
                ui.label(egui::RichText::new("group").small().weak());
            }
            Some(_) => {
                if ui.small_button("✕").clicked() {
                    *value = None;
                }
            }
        }
    });
    ui.end_row();
}

/// An inheritable switch: blank means "whatever my group says".
fn opt_bool_row(ui: &mut egui::Ui, label: &str, value: &mut Option<bool>, inherited: bool) {
    ui.label(label);
    ui.horizontal(|ui| {
        let mut shown = value.unwrap_or(inherited);
        if ui.checkbox(&mut shown, "").changed() {
            *value = Some(shown);
        }
        match value {
            None => {
                ui.label(egui::RichText::new("group").small().weak());
            }
            Some(_) => {
                if ui.small_button("✕").clicked() {
                    *value = None;
                }
            }
        }
    });
    ui.end_row();
}

fn combo_row(ui: &mut egui::Ui, label: &str, value: &mut String, options: &[&str]) {
    // An empty stored value means "the first option", which is the documented
    // default for every one of these.
    let shown = if value.trim().is_empty() {
        options.first().copied().unwrap_or("").to_owned()
    } else {
        value.clone()
    };
    ui.label(label);
    egui::ComboBox::from_id_salt((label, options.len()))
        .selected_text(shown.clone())
        .show_ui(ui, |ui| {
            for opt in options {
                if ui.selectable_label(shown == *opt, *opt).clicked() {
                    *value = (*opt).to_owned();
                }
            }
        });
    ui.end_row();
}

/// A colour, with "unset" as a real state — that is what defers to the group,
/// and then to the theme.
///
/// The swatch is [`crate::panels::properties::color_edit_button_closing`], the
/// SAME picker every RAD control property uses (operator, 2026-08-17). Rolling a
/// second one here meant egui's default button, which has none of the theme's
/// palette grid — a developer picking a group's colour got a different tool from
/// the one they use everywhere else in the IDE.
///
/// Unset survives it: the picker needs a concrete colour, so an unset row seeds
/// it with a neutral and only writes when the developer actually picks, and the
/// ✕ puts the row back to inheriting.
fn color_row(ui: &mut egui::Ui, label: &str, value: &mut String, unset_hint: &str) {
    use crate::panels::properties::{color32_to_hex, color_edit_button_closing, hex_to_color32};

    ui.label(label);
    ui.horizontal(|ui| {
        let unset = value.trim().is_empty();
        let mut color = if unset {
            egui::Color32::from_gray(160)
        } else {
            hex_to_color32(value)
        };
        if color_edit_button_closing(ui, &mut color).changed() {
            *value = color32_to_hex(color);
        }
        if value.trim().is_empty() {
            ui.label(egui::RichText::new(unset_hint).small().weak());
        } else {
            ui.label(
                egui::RichText::new(color32_to_hex(color))
                    .small()
                    .color(egui::Color32::GRAY),
            );
            if ui.small_button("✕").clicked() {
                // Back to unset — the group, then the theme, decides again.
                value.clear();
            }
        }
    });
    ui.end_row();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modal() -> ToolbarEditorModal {
        ToolbarEditorModal::new("TB-1".into(), ToolbarDef::default())
    }

    /// Building a toolbar in the editor: groups, buttons into the right group,
    /// reordering, and deleting without taking the wrong thing.
    #[test]
    fn the_editor_builds_groups_and_buttons_and_can_reorder_them() {
        let mut m = modal();
        assert_eq!(m.selected, Selection::None);

        // "Add button" with nothing at all creates the group it needs.
        m.add_button();
        assert_eq!(m.def.groups.len(), 1, "a button needs a group to live in");
        assert_eq!(m.selected, Selection::Button(0, 0));

        m.add_button();
        assert_eq!(m.def.groups[0].buttons.len(), 2, "…and the next joins it");

        // A second group, and a button lands in THAT one now.
        m.add_group();
        assert_eq!(m.selected, Selection::Group(1));
        m.add_button();
        assert_eq!(m.selected, Selection::Button(1, 0));
        assert_eq!(m.def.groups[0].buttons.len(), 2);
        assert_eq!(m.def.groups[1].buttons.len(), 1);

        // Ids are unique across the whole toolbar.
        let ids: Vec<&str> = m.def.buttons().map(|(_, b)| b.id.as_str()).collect();
        let mut u = ids.clone();
        u.sort_unstable();
        u.dedup();
        assert_eq!(u.len(), 3, "button ids collide: {ids:?}");

        // Reordering groups.
        let first_id = m.def.groups[0].id.clone();
        m.selected = Selection::Group(0);
        m.nudge(1);
        assert_eq!(m.def.groups[1].id, first_id, "the group moved down");
        assert_eq!(m.selected, Selection::Group(1), "…and stays selected");
        m.nudge(-1);
        assert_eq!(m.def.groups[0].id, first_id);
        // Nudging past the end does nothing rather than panicking.
        m.selected = Selection::Group(0);
        m.nudge(-1);
        assert_eq!(m.def.groups[0].id, first_id);

        // Reordering buttons within a group.
        m.selected = Selection::Button(0, 0);
        let b0 = m.def.groups[0].buttons[0].id.clone();
        m.nudge(1);
        assert_eq!(m.def.groups[0].buttons[1].id, b0);
        assert_eq!(m.selected, Selection::Button(0, 1));

        // Deleting a button leaves its group and its siblings.
        m.delete_selected();
        assert_eq!(m.def.groups[0].buttons.len(), 1);
        assert_eq!(m.def.groups.len(), 2, "the group survives its button");

        // Deleting a group takes its buttons — that is what it means.
        m.selected = Selection::Group(0);
        m.delete_selected();
        assert_eq!(m.def.groups.len(), 1);
        assert_eq!(m.selected, Selection::Group(0), "selection follows");
        m.delete_selected();
        assert!(m.def.groups.is_empty());
        assert_eq!(m.selected, Selection::None, "nothing left to select");
        // Deleting with nothing selected is a no-op, not a panic.
        m.delete_selected();

        println!(
            "\n  Toolbar editor — add button with no group creates one; buttons land in \
             the selected group; groups and buttons reorder with the selection \
             following; deleting a button keeps its group, deleting a group takes its \
             buttons; empty selection is a no-op\n"
        );
    }

    /// The split must never panic, at ANY width.
    ///
    /// It did: `clamp(180.0, total_w - 260.0)` hands `f32::clamp` a maximum
    /// below its minimum as soon as the modal is under 440 px, and `clamp`
    /// panics on that — which killed the whole event loop, not just the modal.
    #[test]
    fn the_split_never_panics_however_narrow_the_modal_gets() {
        // Every width from nothing to a very wide window, and the whole range of
        // splits including the degenerate ends.
        let mut narrowest_ok = f32::MAX;
        for w in 0..2000 {
            let total = w as f32;
            for ratio in [0.0_f32, 0.05, 0.38, 0.5, 0.95, 1.0] {
                let got = tree_pane_width(total, ratio);
                assert!(
                    got.is_finite() && got > 0.0,
                    "width {total} ratio {ratio} gave {got}"
                );
                assert!(
                    got >= TREE_MIN_W,
                    "the tree must keep its minimum: {got} at width {total}"
                );
                if total >= TREE_MIN_W + PROPS_MIN_W {
                    assert!(
                        total - got >= PROPS_MIN_W - 0.001,
                        "with room for both, the properties pane must keep its \
                         minimum: tree {got} of {total}"
                    );
                    narrowest_ok = narrowest_ok.min(total);
                }
            }
        }
        // Degenerate inputs must not produce a NaN either.
        for ratio in [f32::NAN, f32::INFINITY, -1.0] {
            let got = tree_pane_width(900.0, ratio);
            assert!(got.is_finite(), "ratio {ratio} gave {got}");
        }

        println!(
            "\n  Toolbar editor split — no panic across widths 0..2000 × 6 ratios; the \
             tree never drops below {TREE_MIN_W}px, and from {narrowest_ok}px up the \
             properties pane keeps its {PROPS_MIN_W}px; NaN/inf ratios stay finite\n"
        );
    }

    /// A new button keeps the last one's appearance, and nothing of its identity.
    ///
    /// Building a toolbar is six buttons that differ only in icon and action;
    /// re-entering the size and colours on each was the work (operator,
    /// 2026-08-17).
    #[test]
    fn a_new_button_inherits_the_previous_ones_look_but_not_its_identity() {
        let mut m = modal();
        m.add_button();

        // Dress the first button.
        {
            let b = &mut m.def.groups[0].buttons[0];
            b.style.icon_size = Some(30);
            b.style.width = Some(44);
            b.style.background_color = "#204080FF".into();
            b.style.shadow = Some(true);
            b.set_icon("folder-open");
            b.tooltip = "Open".into();
            b.action = "print:/tmp/a.pdf".into();
        }

        m.add_button();
        let (first, second) = {
            let bs = &m.def.groups[0].buttons;
            (bs[0].clone(), bs[1].clone())
        };

        // The look carries over, in full.
        assert_eq!(second.style, first.style, "the appearance must carry over");
        assert_eq!(second.style.icon_size, Some(30));
        assert_eq!(second.style.background_color, "#204080FF");

        // The identity does NOT — that is what makes it a different button.
        assert_ne!(second.id, first.id);
        assert!(second.icon.is_empty(), "not the same icon");
        assert!(second.tooltip.is_empty(), "not the same tooltip");
        assert!(second.action.is_empty(), "not the same action");
        assert_eq!(second.action().verb(), "event", "…so it runs its own onClick");

        // It carries across groups too: the run of buttons keeps its look when
        // the developer moves on.
        m.add_group();
        m.add_button();
        let third = m.def.groups[1].buttons[0].clone();
        assert_eq!(
            third.style.icon_size,
            Some(30),
            "the look follows into the next group"
        );

        // And the very first button of an empty toolbar inherits nothing, so it
        // is the theme's.
        let mut fresh = modal();
        fresh.add_button();
        assert_eq!(
            fresh.def.groups[0].buttons[0].style,
            ButtonStyle::default(),
            "with nothing to inherit from, a button starts plain"
        );

        println!(
            "\n  Toolbar editor — a new button inherits the previous one's style \
             (icon 30, width 44, #204080, shadow) across groups, and none of its \
             identity (id, icon, tooltip, action); the first button of an empty \
             toolbar starts plain\n"
        );
    }

    /// The modal opens at 70 % of the host window, the panes take everything
    /// that is not chrome, and neither is ever derived from the space it happens
    /// to be sitting in.
    #[test]
    fn the_modal_takes_seventy_percent_of_the_host_and_gives_the_rest_to_the_panes() {
        // 70 % on any host tall enough for it to mean anything.
        for host in [800.0_f32, 1000.0, 1200.0, 1600.0, 2160.0] {
            let h = modal_height(host);
            assert!(
                (h - host * 0.70).abs() < 0.01,
                "host {host} should open at {}, got {h}",
                host * 0.70
            );
            assert!(h <= host - 40.0, "the modal must fit the host: {h} of {host}");
        }
        // A short host cannot give 70 % and still be usable, so the floor wins —
        // and the modal never exceeds the host either way.
        for host in [0.0_f32, 200.0, 400.0, 540.0] {
            let h = modal_height(host);
            assert!(h >= MODAL_MIN_H, "host {host} gave {h}, below the floor");
            assert!(h.is_finite());
        }
        assert!(modal_height(f32::NAN).is_finite(), "a NaN host must not spread");

        // The panes grow WITH the host: taller window, taller panes.
        let small = panes_height(modal_height(800.0));
        let large = panes_height(modal_height(1600.0));
        assert!(
            large > small + 200.0,
            "the panes must take the extra room: {small} then {large}"
        );
        assert!(
            panes_height(MODAL_MIN_H) >= PANE_MIN_H,
            "even at the floor the panes get their minimum"
        );
        // Chrome plus panes must fit inside the modal, or the content pushes
        // against the window's ceiling.
        for host in [800.0_f32, 1200.0, 2160.0] {
            let modal = modal_height(host);
            assert!(
                panes_height(modal) + MODAL_CHROME_H <= modal + 0.01,
                "panes + chrome ({} + {MODAL_CHROME_H}) overflow a {modal}px modal",
                panes_height(modal)
            );
        }

        println!(
            "\n  Toolbar editor size — 70% of the host (800⇒{:.0}, 1200⇒{:.0}, 2160⇒{:.0}); \
             panes take the rest ({:.0}⇒{:.0}px of panes at a 1200px host); floor {MODAL_MIN_H} \
             on a short host; NaN stays finite\n",
            modal_height(800.0),
            modal_height(1200.0),
            modal_height(2160.0),
            modal_height(1200.0),
            panes_height(modal_height(1200.0))
        );
    }

    /// The pane boxes must be STABLE across frames.
    ///
    /// This is the failure this project keeps re-learning: a pane sized from its
    /// available space and a window sized from its content grow each other, one
    /// frame at a time, until the window is at the screen edges. Rendering the
    /// real modal repeatedly is the only way to catch it — the arithmetic above
    /// looks fine either way.
    #[test]
    fn rendering_the_modal_repeatedly_never_grows_it() {
        let ctx = egui::Context::default();
        let mut m = modal();
        // A toolbar with enough content that both panes have something to scroll.
        m.add_group();
        for _ in 0..12 {
            m.add_button();
        }
        m.selected = Selection::Button(0, 0);

        // Two host heights, so a modal that ignores the host shows up as a
        // height that does not move when the host does.
        let mut report = String::new();
        for host_h in [900.0_f32, 1400.0] {
            let host = egui::vec2(1440.0, host_h);
            let mut seen: Vec<f32> = Vec::new();
            for _ in 0..10 {
                let mut input = egui::RawInput::default();
                input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, host));
                let mut full = ctx.run_ui(input, |ui| {
                    let _ = super::show(&mut m, ui.ctx());
                });
                full.textures_delta.clear();
                if let Some(rect) =
                    ctx.memory(|mem| mem.area_rect(egui::Id::new("toolbar_editor_modal")))
                {
                    seen.push(rect.height());
                }
            }
            assert!(
                seen.len() >= 4,
                "the modal should have been laid out on most frames, got {seen:?}"
            );
            let settled = *seen.last().unwrap();
            let want = modal_height(host_h);

            // The one thing that must never happen: growing frame after frame.
            // egui's `Resize` steps toward its target over the first few frames,
            // which is a settle, not creep — so the tail is what must be flat.
            let tail = &seen[4..];
            let lo = tail.iter().cloned().fold(f32::MAX, f32::min);
            let hi = tail.iter().cloned().fold(f32::MIN, f32::max);
            assert!(
                hi - lo < 1.0,
                "the modal kept growing after settling on a {host_h}px host: {seen:?}"
            );
            // And it must actually follow the host, at 70 % give or take the
            // chrome slack — not sit at some size of its own.
            assert!(
                (settled - want).abs() <= 4.0,
                "a {host_h}px host should give a {want:.0}px modal, got {settled:.0} \
                 (content {:.0}, panes {:.0}) — check TITLE_BAR_H/MODAL_CHROME_H: {seen:?}",
                content_height(want),
                panes_height(want)
            );
            report.push_str(&format!(
                "{host_h:.0}px host ⇒ wanted {want:.0}, settled {settled:.0}; "
            ));
        }

        println!("\n  Toolbar editor stability — 10 frames each, 12 buttons: {report}no frame-over-frame growth.\n");
    }

    /// The action picker writes the stored string the model reads back.
    #[test]
    fn the_editor_writes_actions_the_model_can_read() {
        for (verb, target, want) in [
            ("event", "", ToolbarAction::Event),
            ("procedure", "UPDATE-TOTAL", ToolbarAction::Procedure("UPDATE-TOTAL".into())),
            ("open-modal", "CUST", ToolbarAction::OpenModal("CUST".into())),
            ("print", "/tmp/r.pdf", ToolbarAction::Print("/tmp/r.pdf".into())),
            ("screenshot", "", ToolbarAction::Screenshot),
            ("run-app", "/usr/bin/vi", ToolbarAction::RunApp("/usr/bin/vi".into())),
        ] {
            let action = match verb {
                "procedure" => ToolbarAction::Procedure(target.into()),
                "open-modal" => ToolbarAction::OpenModal(target.into()),
                "print" => ToolbarAction::Print(target.into()),
                "run-app" => ToolbarAction::RunApp(target.into()),
                "screenshot" => ToolbarAction::Screenshot,
                _ => ToolbarAction::Event,
            };
            let mut b = ToolbarButton::new("b1", "X");
            b.action = action.to_action_string();
            assert_eq!(b.action(), want, "{verb} did not round-trip");
        }
        // Every verb the picker offers is one the model knows.
        for verb in ToolbarAction::VERBS {
            assert!(
                ToolbarAction::VERBS.contains(verb),
                "the picker offers a verb the model has never heard of: {verb}"
            );
        }
        println!(
            "\n  Toolbar editor — the action picker's {} verbs all round-trip through \
             the stored string\n",
            ToolbarAction::VERBS.len()
        );
    }
}
