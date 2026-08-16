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

use cobolt_forms::toolbar::{ToolbarAction, ToolbarButton, ToolbarDef, ToolbarGroup};
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
        let Some(group) = self.def.groups.get_mut(gi) else {
            return;
        };
        let n = group.buttons.len() + 1;
        group
            .buttons
            .push(ToolbarButton::new(id, format!("Button {n}")));
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
    // A CONSTANT opening size, not a share of the monitor — the same rule the
    // event-handler modal follows, and for the same reason.
    let default_size = egui::vec2(900.0, 560.0);

    egui::Window::new(tr.toolbar_editor_title)
        .id(egui::Id::new("toolbar_editor_modal"))
        .collapsible(false)
        .resizable(true)
        .default_size(default_size)
        .max_size(egui::vec2(
            default_size.x.max(screen.width() - 40.0),
            default_size.y.max(screen.height() - 40.0),
        ))
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
            let total_w = ui.available_width();
            let tree_w = (total_w * modal.split_ratio).clamp(180.0, total_w - 260.0);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(tree_w);
                    ui.label(egui::RichText::new(tr.toolbar_groups).strong());
                    egui::ScrollArea::vertical()
                        .id_salt("toolbar_tree")
                        .max_height(300.0)
                        .show(ui, |ui| show_tree(modal, ui));
                });
                ui.separator();
                ui.vertical(|ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("toolbar_props")
                        .max_height(300.0)
                        .show(ui, |ui| show_props(modal, ui, &tr));
                });
            });

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
            text_row(ui, "Name:", &mut g.label);
            ui.add_space(4.0);
            ui.label(egui::RichText::new(tr.sec_appearance).small().weak());
            combo_row(ui, "Border:", &mut g.border_style, &["Single", "None", "Fixed3D"]);
            color_row(ui, "Border colour:", &mut g.border_color);
            int_row(ui, "Border width:", &mut g.border_width, 0, 40);
            int_row(ui, "Corner radius:", &mut g.corner_radius, 0, 60);
            int_row(ui, "Padding:", &mut g.padding, 0, 60);
            color_row(ui, "Background:", &mut g.background_color);
            ui.add_space(4.0);
            ui.checkbox(&mut g.separator_after, tr.toolbar_separator_after);
            if g.separator_after {
                int_row(ui, "Separator width:", &mut g.separator_width, 0, 200);
            }
        }
        Selection::Button(gi, bi) => {
            ui.label(egui::RichText::new(tr.toolbar_button_props).strong());
            // The icon picker needs the button, but so does everything else, so
            // take the flags out first and borrow the button once.
            let mut open_picker = false;
            {
                let Some(b) = modal
                    .def
                    .groups
                    .get_mut(gi)
                    .and_then(|g| g.buttons.get_mut(bi))
                else {
                    return;
                };
                text_row(ui, "Label:", &mut b.label);
                text_row(ui, "Tooltip:", &mut b.tooltip);
                ui.checkbox(&mut b.enabled, "Enabled");

                ui.add_space(4.0);
                ui.label(egui::RichText::new("Icon").small().weak());
                ui.horizontal(|ui| {
                    ui.label("Icon:");
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
                int_row(ui, "Icon size:", &mut b.icon_size, 4, 128);
                color_row(ui, "Icon colour:", &mut b.icon_color);

                ui.add_space(4.0);
                ui.label(egui::RichText::new(tr.sec_geometry).small().weak());
                int_row(ui, "Width:", &mut b.width, 8, 400);
                int_row(ui, "Height:", &mut b.height, 8, 400);
                int_row(ui, "Corner radius:", &mut b.corner_radius, 0, 60);

                ui.add_space(4.0);
                ui.label(egui::RichText::new(tr.sec_colors).small().weak());
                color_row(ui, "Background:", &mut b.background_color);
                color_row(ui, "Foreground:", &mut b.foreground_color);
                ui.checkbox(&mut b.gradient, "Gradient background");
                if b.gradient {
                    color_row(ui, "From:", &mut b.gradient_start_color);
                    color_row(ui, "To:", &mut b.gradient_end_color);
                    combo_row(
                        ui,
                        "Direction:",
                        &mut b.gradient_direction,
                        &["Vertical", "Horizontal", "Diagonal"],
                    );
                }

                ui.add_space(4.0);
                ui.label(egui::RichText::new(tr.sec_shadow).small().weak());
                ui.checkbox(&mut b.shadow, "Drop shadow");
                if b.shadow {
                    color_row(ui, "Shadow colour:", &mut b.shadow_color);
                    int_row(ui, "Opacity %:", &mut b.shadow_opacity, 0, 100);
                    int_row(ui, "Distance:", &mut b.shadow_distance, 0, 40);
                    int_row(ui, "Blur:", &mut b.shadow_blur_strength, 0, 20);
                }

                ui.add_space(4.0);
                ui.label(egui::RichText::new(tr.toolbar_action).small().weak());
                action_row(ui, b, tr);
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
            b.icon = name;
        }
        close = true;
    }
    if close {
        modal.icon_picker_open = false;
    }
}

// ── Small rows, matching the properties pane's conventions ────────────────────
//
// Field labels are plain text here, as they are in `properties.rs` — only the
// modal's chrome and section headers go through `Tr`.

fn text_row(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.text_edit_singleline(value);
    });
}

fn int_row(ui: &mut egui::Ui, label: &str, value: &mut i64, lo: i64, hi: i64) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).range(lo..=hi).speed(0.4));
    });
}

fn combo_row(ui: &mut egui::Ui, label: &str, value: &mut String, options: &[&str]) {
    // An empty stored value means "the first option", which is the documented
    // default for every one of these.
    let shown = if value.trim().is_empty() {
        options.first().copied().unwrap_or("").to_owned()
    } else {
        value.clone()
    };
    ui.horizontal(|ui| {
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
    });
}

/// A colour, with "unset" as a real state — that is what defers to the theme.
fn color_row(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut rgba = if value.trim().is_empty() {
            [0.5_f32, 0.5, 0.5, 1.0]
        } else {
            let c = cobolt_forms::paint::parse_color(value);
            [
                c.r() as f32 / 255.0,
                c.g() as f32 / 255.0,
                c.b() as f32 / 255.0,
                c.a() as f32 / 255.0,
            ]
        };
        if ui.color_edit_button_rgba_unmultiplied(&mut rgba).changed() {
            *value = format!(
                "#{:02X}{:02X}{:02X}{:02X}",
                (rgba[0] * 255.0) as u8,
                (rgba[1] * 255.0) as u8,
                (rgba[2] * 255.0) as u8,
                (rgba[3] * 255.0) as u8
            );
        }
        if value.trim().is_empty() {
            ui.label(egui::RichText::new("theme").small().weak());
        } else if ui.small_button("✕").clicked() {
            // Back to unset — the theme decides again.
            value.clear();
        }
    });
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
