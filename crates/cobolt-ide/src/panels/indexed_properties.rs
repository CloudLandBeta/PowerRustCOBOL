// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Properties pane for the Indexed File Editor (R7, R10, R11).

use cobolt_forms::ControlType;
use cobolt_indexed::{AccessMode, FieldUsage, IndexedDefinition, IndexedField, RecordFormatDef};

use crate::i18n::Tr;

/// The comment is a text area now (5 lines by 60 columns to begin with), so it can hold more than the
/// single line it used to.
const COMMENT_MAX_LEN: usize = 500;
const COMMENT_COLS: f32 = 60.0;
const COMMENT_ROWS: f32 = 5.0;
/// Room around the text inside the box and below it, in points.
const COMMENT_PAD: f32 = 10.0;
const NAME_MAX_CHARS: usize = 30;
const PROP_ROW_H: f32 = 30.0;

fn label_with_colon(text: &str) -> String {
    let t = text.trim_end_matches(':').trim();
    format!("{t}:")
}

/// The box's size when the user has not dragged it: 60 columns by 5 lines of the body font.
fn comment_box_default(ui: &egui::Ui) -> egui::Vec2 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let (cw, rh) = ui.fonts_mut(|f| (f.glyph_width(&font, '0'), f.row_height(&font)));
    egui::vec2(COMMENT_COLS * cw + COMMENT_PAD, COMMENT_ROWS * rh + COMMENT_PAD)
}

/// A size the user dragged the box to, kept between a third of the default and a few times it, so a
/// slip of the mouse can neither hide the box nor swallow the pane.
fn clamp_comment_box(size: egui::Vec2, default: egui::Vec2) -> egui::Vec2 {
    egui::vec2(
        size.x.clamp(default.x * 0.33, default.x * 2.5),
        size.y.clamp(default.y * 0.4, default.y * 6.0),
    )
}

fn comment_box_id() -> egui::Id {
    egui::Id::new("indexed-comment-box-size")
}

/// The comment box's current size. One size for the file, record and field editors, kept for the session.
fn comment_box_size(ui: &egui::Ui) -> egui::Vec2 {
    let default = comment_box_default(ui);
    let stored = ui.ctx().data(|d| d.get_temp::<egui::Vec2>(comment_box_id()));
    clamp_comment_box(stored.unwrap_or(default), default)
}

/// The height of the Comments row in BOTH columns: the box plus a little air. The label column and the
/// value column are laid out apart, so each reads this to stay level.
fn comment_row_h(ui: &egui::Ui) -> f32 {
    comment_box_size(ui).y + COMMENT_PAD
}

/// The text area: 5 lines by 60 columns until the user drags its bottom-right corner, which resizes it
/// (and the Comments row with it). Longer text scrolls inside. Returns whether the text changed.
fn comment_editor(ui: &mut egui::Ui, text: &mut String) -> bool {
    let default = comment_box_default(ui);
    let size = comment_box_size(ui);
    let row_h = ui.fonts_mut(|f| f.row_height(&egui::TextStyle::Body.resolve(ui.style())));
    let rows = (((size.y - COMMENT_PAD) / row_h).floor() as usize).max(1);
    let width = size.x.min(ui.available_width());
    let mut changed = false;
    let out = egui::ScrollArea::vertical()
        .max_width(width)
        .max_height(size.y)
        .min_scrolled_height(size.y)
        .auto_shrink([true, false])
        .show(ui, |ui| {
            changed = ui
                .add(
                    egui::TextEdit::multiline(text)
                        .desired_width(width - COMMENT_PAD)
                        .desired_rows(rows)
                        .char_limit(COMMENT_MAX_LEN)
                        .lock_focus(true),
                )
                .changed();
        });
    // The grip: a small square over the box's bottom-right corner.
    let corner = out.inner_rect.right_bottom();
    let grip = egui::Rect::from_min_size(corner - egui::vec2(14.0, 14.0), egui::vec2(14.0, 14.0));
    let resp = ui.interact(grip, comment_box_id().with("grip"), egui::Sense::drag());
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
    }
    if resp.dragged() {
        let new = clamp_comment_box(size + resp.drag_delta(), default);
        ui.ctx().data_mut(|d| d.insert_temp(comment_box_id(), new));
    }
    let ink = ui.visuals().weak_text_color();
    for k in [4.0_f32, 8.0, 12.0] {
        ui.painter().line_segment([corner - egui::vec2(k, 2.0), corner - egui::vec2(2.0, k)], egui::Stroke::new(1.0, ink));
    }
    changed
}

fn label_row(ui: &mut egui::Ui, text: &str) {
    label_row_h(ui, text, PROP_ROW_H, None);
}

/// `tip` is shown when the pointer rests on the label (wrapped, at most 380 pt wide).
fn label_row_h(ui: &mut egui::Ui, text: &str, h: f32, tip: Option<&str>) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), h),
        egui::Layout::left_to_right(egui::Align::TOP),
        |ui| {
            // Force the row to occupy exactly PROP_ROW_H. `allocate_ui_with_layout`
            // otherwise advances by *content* height, so the label column (~18px rows)
            // and the value column (taller textboxes/combos) drift apart row-by-row.
            // Pinning every row to the same height keeps the two columns aligned.
            ui.set_min_height(h);
            let label = ui.label(label_with_colon(text));
            if let Some(tip) = tip {
                label.on_hover_ui(|ui| {
                    ui.set_max_width(380.0);
                    ui.label(tip);
                });
            }
        },
    );
}

/// Result of an edit in the properties pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyEdit {
    None,
    Changed,
    Renamed { from: String, to: String },
}

pub struct IndexedPropertiesPanel;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PicTemplate {
    X,
    Xn,
    Nine,
    NineN,
    A,
    An,
    Custom,
}

impl PicTemplate {
    fn from_pic(pic: &str) -> Self {
        // If the PIC value is prefixed with the zero-width marker, it was explicitly
        // placed into "Custom" entry mode by the user (even for template-shaped values
        // like "X(10)"). This forces the properties pane to show the direct textbox
        // and disables template auto-build/len controls.
        if pic.starts_with('\u{200B}') {
            return PicTemplate::Custom;
        }
        let p = pic.trim().to_ascii_uppercase();
        if p == "X" {
            PicTemplate::X
        } else if p.starts_with("X(") {
            PicTemplate::Xn
        } else if p == "9" {
            PicTemplate::Nine
        } else if p.starts_with("9(") || p.starts_with("S9(") {
            PicTemplate::NineN
        } else if p == "A" {
            PicTemplate::A
        } else if p.starts_with("A(") {
            PicTemplate::An
        } else {
            PicTemplate::Custom
        }
    }

    fn label(self, tr: &Tr) -> &'static str {
        match self {
            PicTemplate::X => tr.pic_tpl_x,
            PicTemplate::Xn => tr.pic_tpl_xn,
            PicTemplate::Nine => tr.pic_tpl_9,
            PicTemplate::NineN => tr.pic_tpl_9n,
            PicTemplate::A => tr.pic_tpl_a,
            PicTemplate::An => tr.pic_tpl_an,
            PicTemplate::Custom => tr.pic_tpl_custom,
        }
    }

    fn build(self, len: u32, signed: bool) -> String {
        match self {
            PicTemplate::X => "X".into(),
            PicTemplate::Xn => format!("X({len})"),
            PicTemplate::Nine => "9".into(),
            PicTemplate::NineN => {
                if signed {
                    format!("S9({len})")
                } else {
                    format!("9({len})")
                }
            }
            PicTemplate::A => "A".into(),
            PicTemplate::An => format!("A({len})"),
            PicTemplate::Custom => String::new(),
        }
    }
}

impl IndexedPropertiesPanel {
    pub fn show_file_labels(ui: &mut egui::Ui, tr: &Tr) {
        label_row(ui, tr.lbl_idx_file_name);
        label_row(ui, tr.dlg_assign_path);
        label_row(ui, tr.dlg_access_mode);
        label_row(ui, tr.dlg_record_format);
        label_row(ui, tr.lbl_idx_finalized);
        label_row_h(ui, tr.lbl_field_comments, comment_row_h(ui), Some(tip_for(DefinitionKind::File, tr)));
    }

    pub fn show_file_values(
        ui: &mut egui::Ui,
        def: &mut IndexedDefinition,
        tr: &Tr,
    ) -> PropertyEdit {
        let mut changed = false;
        let locked = def.finalized;
        let name_w = field_width(ui, NAME_MAX_CHARS);

        value_row(ui, PROP_ROW_H, |ui| {
            ui.add_enabled_ui(!locked, |ui| {
                changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut def.name)
                            .desired_width(name_w)
                            .char_limit(NAME_MAX_CHARS),
                    )
                    .changed();
            });
        });
        value_row(ui, PROP_ROW_H, |ui| {
            ui.add_enabled_ui(!locked, |ui| {
                changed |= ui
                    .add(egui::TextEdit::singleline(&mut def.assign_path).desired_width(name_w))
                    .changed();
            });
        });
        value_row(ui, PROP_ROW_H, |ui| {
            ui.add_enabled_ui(!locked, |ui| {
                egui::ComboBox::from_id_salt("idx_prop_access")
                    .width(name_w)
                    .selected_text(access_label(def.access_mode, tr))
                    .show_ui(ui, |ui| {
                        for mode in [
                            AccessMode::Dynamic,
                            AccessMode::Sequential,
                            AccessMode::Random,
                        ] {
                            if ui
                                .selectable_value(
                                    &mut def.access_mode,
                                    mode,
                                    access_label(mode, tr),
                                )
                                .clicked()
                            {
                                changed = true;
                            }
                        }
                    });
            });
        });
        value_row(ui, PROP_ROW_H, |ui| {
            ui.add_enabled_ui(!locked, |ui| {
                changed |= show_record_format_values(ui, def, tr);
            });
        });
        value_row(ui, PROP_ROW_H, |ui| {
            if ui.checkbox(&mut def.finalized, "").changed() {
                changed = true;
            }
        });
        value_row(ui, comment_row_h(ui), |ui| {
            if comment_editor(ui, &mut def.comment) {
                changed = true;
            }
        });

        if changed {
            PropertyEdit::Changed
        } else {
            PropertyEdit::None
        }
    }

    pub fn show_field_labels(ui: &mut egui::Ui, def: &IndexedDefinition, field_id: &str, tr: &Tr) {
        let tip = find_field(def, field_id).map_or(tr.tip_def_ai_field, |f| definition_tip(def, f, tr));
        let is_leaf = find_field(def, field_id)
            .map(|f| f.is_leaf())
            .unwrap_or(false);
        if is_leaf {
            leaf_labels(ui, tr, tip);
        } else {
            group_labels(ui, tr, tip);
        }
    }

    pub fn show_field_values(
        ui: &mut egui::Ui,
        def: &mut IndexedDefinition,
        field_id: &str,
        tr: &Tr,
    ) -> PropertyEdit {
        let locked = def.finalized;
        let old_name = field_id.to_string();
        let is_leaf = find_field(def, field_id)
            .map(|f| f.is_leaf())
            .unwrap_or(false);
        if is_leaf {
            def.recompute_offsets();
        }

        let sibling_names = sibling_field_names(def, field_id);

        let Some(field) = find_field_mut(def, field_id) else {
            ui.label(tr.lbl_idx_field_missing);
            return PropertyEdit::None;
        };

        // Enforce REDEFINES rule: target must be a prior sibling (cannot refer to item
        // after the current / "above" in source order). Clear if the loaded value violates.
        let mut cleared_redef = false;
        if let Some(r) = &field.redefines {
            if !sibling_names.contains(r) {
                field.redefines = None;
                cleared_redef = true;
            }
        }

        let (rename, mut changed) = if is_leaf {
            show_leaf_values(ui, field, field_id, &sibling_names, locked, tr)
        } else {
            show_group_values(ui, field, field_id, &sibling_names, locked, tr)
        };
        if cleared_redef {
            changed = true;
        }
        finish_field_edit(def, &old_name, rename, changed, is_leaf)
    }
}

/// What kind of thing the Comments row describes, which decides what its tooltip asks the developer to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefinitionKind {
    File,
    /// The `01` group: one record of the file.
    Record,
    /// A field the primary key or an alternate key is made of or named after.
    Key,
    /// Any other field, leaf or group item.
    Field,
}

/// Whether `field` is part of the primary key or an alternate key (the same test the structure tree uses
/// to colour a key field).
fn is_key_field(def: &IndexedDefinition, field: &IndexedField) -> bool {
    let named = |k: &cobolt_indexed::KeyDef| {
        k.name.as_deref() == Some(field.name.as_str()) || k.parts.iter().any(|p| p.field_name == field.name)
    };
    named(&def.keys.primary) || def.keys.alternates.iter().any(named)
}

fn definition_kind(def: &IndexedDefinition, field: &IndexedField) -> DefinitionKind {
    if is_key_field(def, field) {
        DefinitionKind::Key
    } else if field.is_group() && field.level == 1 {
        DefinitionKind::Record
    } else {
        DefinitionKind::Field
    }
}

/// The tooltip text of one kind, in the language of the IDE.
fn tip_for(kind: DefinitionKind, tr: &Tr) -> &str {
    match kind {
        DefinitionKind::File => tr.tip_def_ai_file,
        DefinitionKind::Record => tr.tip_def_ai_record,
        DefinitionKind::Key => tr.tip_def_ai_key,
        DefinitionKind::Field => tr.tip_def_ai_field,
    }
}

/// The tooltip of the Comments label for the selected field, in the language of the IDE.
fn definition_tip<'a>(def: &IndexedDefinition, field: &IndexedField, tr: &'a Tr) -> &'a str {
    tip_for(definition_kind(def, field), tr)
}

fn leaf_labels(ui: &mut egui::Ui, tr: &Tr, tip: &str) {
    label_row(ui, tr.lbl_field_name);
    label_row(ui, tr.lbl_group_redefines);
    label_row(ui, tr.lbl_field_pic);
    label_row(ui, tr.lbl_field_length);
    label_row(ui, tr.lbl_field_usage);
    label_row(ui, tr.lbl_field_offset);
    label_row_h(ui, tr.lbl_field_comments, comment_row_h(ui), Some(tip));
    label_row(ui, tr.lbl_field_control);
}

fn group_labels(ui: &mut egui::Ui, tr: &Tr, tip: &str) {
    label_row(ui, tr.lbl_field_name);
    label_row(ui, tr.lbl_group_occurs);
    label_row(ui, tr.lbl_group_redefines);
    label_row(ui, tr.lbl_group_synchronized);
    label_row_h(ui, tr.lbl_field_comments, comment_row_h(ui), Some(tip));
}

fn value_row(ui: &mut egui::Ui, h: f32, add: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), h),
        egui::Layout::left_to_right(egui::Align::TOP),
        |ui| {
            // Pin the row to exactly `h` so it lines up with the matching label row
            // (see `label_row`). Without this the column advances by content height
            // and the label/value pairs drift out of alignment.
            ui.set_min_height(h);
            add(ui);
        },
    );
}

fn show_leaf_values(
    ui: &mut egui::Ui,
    field: &mut IndexedField,
    field_id: &str,
    sibling_names: &[String],
    locked: bool,
    tr: &Tr,
) -> (Option<String>, bool) {
    let mut rename: Option<String> = None;
    let mut changed = false;
    let name_w = field_width(ui, NAME_MAX_CHARS);
    let pic_w = field_width(ui, 22);
    let num_w = field_width(ui, 5);

    let mut tpl = PicTemplate::from_pic(&field.pic);
    let mut len = field.length.unwrap_or(1);

    value_row(ui, PROP_ROW_H, |ui| {
        if locked {
            ui.monospace(&field.name);
        } else {
            let mut name = field.name.clone();
            if ui
                .add(
                    egui::TextEdit::singleline(&mut name)
                        .desired_width(name_w)
                        .char_limit(NAME_MAX_CHARS),
                )
                .changed()
            {
                let new_name = name.trim().to_ascii_uppercase().replace(' ', "-");
                if !new_name.is_empty() && new_name != field.name {
                    rename = Some(new_name.clone());
                    field.name = new_name;
                    changed = true;
                }
            }
        }
    });

    // REDEFINES support for leaves (including FILLER) - every data item under the main record group
    value_row(ui, PROP_ROW_H, |ui| {
        ui.add_enabled_ui(!locked, |ui| {
            let mut r = field.redefines.clone().unwrap_or_default();
            egui::ComboBox::from_id_salt(("idx_redefines", field_id))
                .width(name_w)
                .selected_text(if r.is_empty() { "—" } else { &r })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(r.is_empty(), "—").clicked() {
                        field.redefines = None;
                        changed = true;
                    }
                    for name in sibling_names {
                        if ui.selectable_label(r == *name, name.as_str()).clicked() {
                            r = name.clone();
                            field.redefines = Some(name.clone());
                            changed = true;
                        }
                    }
                });
        });
    });

    // PIC row: show ComboBox of templates, or replace entirely with a TextEdit when
    // the user selects "Custom" (or when the stored pic does not match a known template).
    // We use the \u{200B} (zero-width space) prefix as an invisible marker on field.pic
    // so that from_pic() will return Custom even for strings that look like X(n)/9(n)/A(n).
    // This makes the textbox the sole control for custom PIC entry and persists the choice
    // across frames / save/load. The marker is stripped on COBOL source emission.
    let mut force_custom_entry = false;

    value_row(ui, PROP_ROW_H, |ui| {
        ui.add_enabled_ui(!locked, |ui| {
            let is_custom = tpl == PicTemplate::Custom || force_custom_entry;
            if !is_custom {
                egui::ComboBox::from_id_salt(("idx_pic_tpl", field_id))
                    .width(pic_w)
                    .selected_text(tpl.label(tr))
                    .show_ui(ui, |ui| {
                        for t in [
                            PicTemplate::Xn,
                            PicTemplate::X,
                            PicTemplate::NineN,
                            PicTemplate::Nine,
                            PicTemplate::An,
                            PicTemplate::A,
                            PicTemplate::Custom,
                        ] {
                            if ui.selectable_value(&mut tpl, t, t.label(tr)).clicked() {
                                changed = true;
                                if t == PicTemplate::Custom {
                                    force_custom_entry = true;
                                    // Copy current (clean) PIC value into the edit buffer so user
                                    // can see/adjust the exact definition they are taking over.
                                    let effective = if field
                                        .pic
                                        .trim_start_matches('\u{200B}')
                                        .trim()
                                        .is_empty()
                                    {
                                        "X(1)".to_string()
                                    } else {
                                        field.pic.trim_start_matches('\u{200B}').to_string()
                                    };
                                    field.pic = format!("\u{200B}{}", effective.trim());
                                }
                            }
                        }
                    });
            } else {
                // Replace the dropdown entirely with a textbox for direct custom PIC entry.
                let mut custom_pic = field.pic.trim_start_matches('\u{200B}').to_string();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut custom_pic)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(220.0),
                    )
                    .changed()
                {
                    // Store with marker so the custom mode "sticks" (textbox stays sole control)
                    // even if the user types a value that would otherwise match a template.
                    field.pic = format!("\u{200B}{}", custom_pic.trim());
                    changed = true;
                }
            }
        });
    });

    value_row(ui, PROP_ROW_H, |ui| {
        ui.add_enabled_ui(!locked, |ui| {
            let show_len = tpl != PicTemplate::X
                && tpl != PicTemplate::Nine
                && tpl != PicTemplate::A
                && tpl != PicTemplate::Custom;
            if show_len {
                let mut len_s = format!("{:05}", len);
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut len_s)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(num_w),
                    )
                    .changed()
                {
                    if let Ok(v) = len_s.trim().parse::<u32>() {
                        let v = v.clamp(1, 99_999);
                        if field.length != Some(v) {
                            field.length = Some(v);
                            len = v;
                            changed = true;
                        }
                    }
                }
            } else if tpl != PicTemplate::Custom {
                ui.monospace("00001");
            }
        });
    });

    if tpl != PicTemplate::Custom {
        let clean_for_signed = field.pic.trim_start_matches('\u{200B}');
        let new_pic = tpl.build(len, clean_for_signed.to_ascii_uppercase().starts_with('S'));
        if field.pic != new_pic {
            field.pic = new_pic;
            changed = true;
        }
        if tpl == PicTemplate::X || tpl == PicTemplate::Nine || tpl == PicTemplate::A {
            field.length = Some(1);
        }
    }

    value_row(ui, PROP_ROW_H, |ui| {
        ui.add_enabled_ui(!locked, |ui| {
            let mut usage = field.usage;
            egui::ComboBox::from_id_salt(("idx_usage", field_id))
                .width(pic_w)
                .selected_text(usage_label(usage))
                .show_ui(ui, |ui| {
                    for u in usage_all() {
                        if ui.selectable_value(&mut usage, u, usage_label(u)).clicked() {
                            changed = true;
                        }
                    }
                });
            if usage != field.usage {
                field.usage = usage;
                changed = true;
            }
        });
    });

    value_row(ui, PROP_ROW_H, |ui| {
        ui.monospace(format!("{:05}", field.offset.unwrap_or(0)));
    });

    value_row(ui, comment_row_h(ui), |ui| {
        truncate_comment(field);
        if comment_editor(ui, &mut field.comment) {
            truncate_comment(field);
            changed = true;
        }
    });

    value_row(ui, PROP_ROW_H, |ui| {
        let mut control = field
            .grid_control
            .clone()
            .unwrap_or_else(|| cobolt_indexed::default_control_for_field(field));
        egui::ComboBox::from_id_salt(("idx_control", field_id))
            .width(pic_w)
            .selected_text(control_label(&control))
            .show_ui(ui, |ui| {
                for c in grid_controls() {
                    if ui
                        .selectable_value(&mut control, c.clone(), control_label(&c))
                        .clicked()
                    {
                        changed = true;
                    }
                }
            });
        if field.grid_control.as_ref() != Some(&control) {
            field.grid_control = Some(control);
            changed = true;
        }
    });

    (rename, changed)
}

fn show_group_values(
    ui: &mut egui::Ui,
    field: &mut IndexedField,
    field_id: &str,
    sibling_names: &[String],
    locked: bool,
    tr: &Tr,
) -> (Option<String>, bool) {
    let mut rename: Option<String> = None;
    let mut changed = false;
    let name_w = field_width(ui, NAME_MAX_CHARS);

    value_row(ui, PROP_ROW_H, |ui| {
        if locked {
            ui.monospace(&field.name);
        } else {
            let mut name = field.name.clone();
            if ui
                .add(
                    egui::TextEdit::singleline(&mut name)
                        .desired_width(name_w)
                        .char_limit(NAME_MAX_CHARS),
                )
                .changed()
            {
                let new_name = name.trim().to_ascii_uppercase().replace(' ', "-");
                if !new_name.is_empty() && new_name != field.name {
                    rename = Some(new_name.clone());
                    field.name = new_name;
                    changed = true;
                }
            }
        }
    });

    value_row(ui, PROP_ROW_H, |ui| {
        ui.add_enabled_ui(!locked, |ui| {
            let occ = field.occurs.unwrap_or(0);
            let mut n_s = if occ == 0 {
                String::new()
            } else {
                format!("{:05}", occ)
            };
            if ui
                .add(
                    egui::TextEdit::singleline(&mut n_s)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(field_width(ui, 5)),
                )
                .changed()
            {
                let trimmed = n_s.trim();
                if trimmed.is_empty() {
                    if field.occurs.is_some() {
                        field.occurs = None;
                        changed = true;
                    }
                } else if let Ok(v) = trimmed.parse::<u32>() {
                    let v = v.min(9999);
                    let new_occ = if v == 0 { None } else { Some(v) };
                    if field.occurs != new_occ {
                        field.occurs = new_occ;
                        changed = true;
                    }
                }
            }
        });
    });

    value_row(ui, PROP_ROW_H, |ui| {
        ui.add_enabled_ui(!locked, |ui| {
            let mut r = field.redefines.clone().unwrap_or_default();
            egui::ComboBox::from_id_salt(("idx_redefines", field_id))
                .width(name_w)
                .selected_text(if r.is_empty() { "—" } else { &r })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(r.is_empty(), "—").clicked() {
                        field.redefines = None;
                        changed = true;
                    }
                    for name in sibling_names {
                        if ui.selectable_label(r == *name, name.as_str()).clicked() {
                            r = name.clone();
                            field.redefines = Some(name.clone());
                            changed = true;
                        }
                    }
                });
        });
    });

    value_row(ui, PROP_ROW_H, |ui| {
        ui.add_enabled_ui(!locked, |ui| {
            if ui.checkbox(&mut field.synchronized, "").changed() {
                changed = true;
            }
        });
    });

    value_row(ui, comment_row_h(ui), |ui| {
        truncate_comment(field);
        if comment_editor(ui, &mut field.comment) {
            truncate_comment(field);
            changed = true;
        }
    });

    (rename, changed)
}

fn finish_field_edit(
    def: &mut IndexedDefinition,
    old_name: &str,
    rename: Option<String>,
    changed: bool,
    is_leaf: bool,
) -> PropertyEdit {
    if changed && is_leaf {
        def.recompute_offsets();
    }
    if let Some(new_name) = rename {
        sync_key_field_name(def, old_name, &new_name);
        return PropertyEdit::Renamed {
            from: old_name.into(),
            to: new_name,
        };
    }
    if changed {
        PropertyEdit::Changed
    } else {
        PropertyEdit::None
    }
}

fn field_width(ui: &egui::Ui, max_chars: usize) -> f32 {
    let id = egui::TextStyle::Body.resolve(ui.style());
    let cw = ui.fonts_mut(|f| f.glyph_width(&id, 'M'));
    let cap = max_chars as f32 * cw + 10.0;
    ui.available_width().min(cap).max(cw * 3.0)
}

fn sibling_field_names(def: &IndexedDefinition, field_id: &str) -> Vec<String> {
    let flat = cobolt_indexed::flatten_record(def);
    let current_idx = flat.iter().position(|e| e.field.name == field_id);
    let current_depth = current_idx
        .and_then(|i| flat.get(i).map(|e| e.depth))
        .unwrap_or(0);
    let mut names = Vec::new();
    if let Some(idx) = current_idx {
        for (i, e) in flat.iter().enumerate() {
            if i >= idx {
                break;
            }
            if e.depth == current_depth && e.field.name != field_id {
                names.push(e.field.name.clone());
            }
        }
    }
    names
}

fn usage_all() -> [FieldUsage; 8] {
    [
        FieldUsage::Display,
        FieldUsage::Comp,
        FieldUsage::Comp3,
        FieldUsage::Comp4,
        FieldUsage::Binary,
        FieldUsage::PackedDecimal,
        FieldUsage::Index,
        FieldUsage::Pointer,
    ]
}

fn truncate_comment(field: &mut IndexedField) {
    truncate_text(&mut field.comment);
}

/// By characters, not bytes: a byte cut could land inside an accented letter and panic.
fn truncate_text(text: &mut String) {
    if text.chars().count() > COMMENT_MAX_LEN {
        *text = text.chars().take(COMMENT_MAX_LEN).collect();
    }
}

#[cfg(test)]
mod comment_box_tests {
    use super::*;

    fn frame(f: impl FnOnce(&mut egui::Ui)) {
        let ctx = egui::Context::default();
        let mut f = Some(f);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            if let Some(f) = f.take() {
                f(ui);
            }
        });
        out.textures_delta.clear();
    }

    #[test]
    fn the_box_starts_at_60_columns_by_5_lines_of_the_body_font_and_the_row_is_as_tall() {
        frame(|ui| {
            let font = egui::TextStyle::Body.resolve(ui.style());
            let (cw, rh) = ui.fonts_mut(|f| (f.glyph_width(&font, '0'), f.row_height(&font)));
            let d = comment_box_default(ui);
            assert_eq!(d, egui::vec2(60.0 * cw + COMMENT_PAD, 5.0 * rh + COMMENT_PAD));
            assert_eq!(comment_box_size(ui), d, "nothing dragged yet: the default");
            assert_eq!(comment_row_h(ui), d.y + COMMENT_PAD, "label and value rows read the same height");
            println!("indexed comment box: default {:.0} x {:.0} pt ({} columns x {} lines), row {:.0} pt", d.x, d.y, COMMENT_COLS, COMMENT_ROWS, comment_row_h(ui));
        });
    }

    #[test]
    fn a_dragged_size_is_kept_for_the_session_and_bounded() {
        frame(|ui| {
            let d = comment_box_default(ui);
            ui.ctx().data_mut(|m| m.insert_temp(comment_box_id(), d * 1.5));
            assert_eq!(comment_box_size(ui), d * 1.5, "a size inside the bounds is kept");
            assert_eq!(comment_row_h(ui), d.y * 1.5 + COMMENT_PAD, "and the row follows it");
            for (asked, want) in [
                (egui::vec2(1.0, 1.0), egui::vec2(d.x * 0.33, d.y * 0.4)),
                (egui::vec2(1.0e6, 1.0e6), egui::vec2(d.x * 2.5, d.y * 6.0)),
            ] {
                ui.ctx().data_mut(|m| m.insert_temp(comment_box_id(), asked));
                assert_eq!(comment_box_size(ui), want, "{asked:?} is bounded");
            }
        });
    }

    #[test]
    fn the_editor_draws_in_the_row_it_was_given_and_lets_text_of_many_lines_in() {
        frame(|ui| {
            let mut text = "line one\nline two\nline three".to_owned();
            let before = ui.cursor().min.y;
            let changed = comment_editor(ui, &mut text);
            let used = ui.min_rect().max.y - before;
            assert!(!changed, "drawing it changes nothing");
            assert_eq!(text, "line one\nline two\nline three");
            assert!(used >= comment_box_size(ui).y - 1.0, "the box takes its height: {used}");
        });
    }

    #[test]
    fn truncation_is_by_characters_so_an_accented_letter_at_the_edge_cannot_panic() {
        let mut s = "é".repeat(COMMENT_MAX_LEN + 40);
        truncate_text(&mut s);
        assert_eq!(s.chars().count(), COMMENT_MAX_LEN);
        s = "short".into();
        truncate_text(&mut s);
        assert_eq!(s, "short");
    }
}

fn sync_key_field_name(def: &mut IndexedDefinition, old: &str, new: &str) {
    for part in &mut def.keys.primary.parts {
        if part.field_name == old {
            part.field_name = new.to_string();
        }
    }
    if def.keys.primary.name.as_deref() == Some(old) {
        def.keys.primary.name = Some(new.to_string());
    }
}

fn show_record_format_values(ui: &mut egui::Ui, def: &IndexedDefinition, tr: &Tr) -> bool {
    match &def.record_format {
        RecordFormatDef::Fixed { length } => {
            ui.horizontal(|ui| {
                ui.label(tr.dlg_record_fixed);
                ui.label(length.to_string());
            });
        }
        RecordFormatDef::Variable {
            min_length,
            max_length,
        } => {
            ui.horizontal(|ui| {
                ui.label(tr.dlg_record_variable);
                ui.label(tr.dlg_record_min_len);
                ui.label(min_length.to_string());
                ui.label(tr.dlg_record_max_len);
                ui.label(max_length.to_string());
            });
        }
    }
    false
}

fn find_field<'a>(def: &'a IndexedDefinition, id: &str) -> Option<&'a IndexedField> {
    fn walk<'a>(fields: &'a [IndexedField], id: &str) -> Option<&'a IndexedField> {
        for f in fields {
            if f.name == id {
                return Some(f);
            }
            if let Some(found) = walk(&f.children, id) {
                return Some(found);
            }
        }
        None
    }
    walk(&def.fields, id)
}

fn find_field_mut<'a>(def: &'a mut IndexedDefinition, id: &str) -> Option<&'a mut IndexedField> {
    fn walk<'a>(fields: &'a mut [IndexedField], id: &str) -> Option<&'a mut IndexedField> {
        for f in fields {
            if f.name == id {
                return Some(f);
            }
            if let Some(found) = walk(&mut f.children, id) {
                return Some(found);
            }
        }
        None
    }
    walk(&mut def.fields, id)
}

fn access_label(mode: AccessMode, tr: &Tr) -> &'static str {
    match mode {
        AccessMode::Dynamic => tr.idx_access_dynamic,
        AccessMode::Sequential => tr.idx_access_sequential,
        AccessMode::Random => tr.idx_access_random,
    }
}

fn usage_label(u: FieldUsage) -> &'static str {
    match u {
        FieldUsage::Display => "DISPLAY",
        FieldUsage::Comp => "COMP",
        FieldUsage::Comp3 => "COMP-3",
        FieldUsage::Comp4 => "COMP-4",
        FieldUsage::Binary => "BINARY",
        FieldUsage::PackedDecimal => "PACKED-DECIMAL",
        FieldUsage::Index => "INDEX",
        FieldUsage::Pointer => "POINTER",
    }
}

fn grid_controls() -> Vec<ControlType> {
    vec![
        ControlType::TextBox,
        ControlType::CheckBox,
        ControlType::NumericUpDown,
        ControlType::DateTimePicker,
        ControlType::ComboBox,
        ControlType::Label,
    ]
}

fn control_label(c: &ControlType) -> String {
    match c {
        ControlType::TextBox => "TextBox".into(),
        ControlType::CheckBox => "CheckBox".into(),
        ControlType::NumericUpDown => "NumericUpDown".into(),
        ControlType::DateTimePicker => "DateTimePicker".into(),
        ControlType::ComboBox => "ComboBox".into(),
        ControlType::Label => "Label".into(),
        other => other.as_str().to_string(),
    }
}

#[cfg(test)]
mod definition_tip_tests {
    use super::*;
    use cobolt_indexed::{KeyDef, KeyEncodingDef, KeyOrderingDef, KeyPartDef};

    fn field(level: u8, name: &str, leaf: bool, children: Vec<IndexedField>) -> IndexedField {
        IndexedField {
            level,
            name: name.into(),
            pic: String::new(),
            usage: FieldUsage::Display,
            offset: leaf.then_some(0),
            length: leaf.then_some(8),
            comment: String::new(),
            grid_control: None,
            occurs: None,
            redefines: None,
            synchronized: false,
            children,
        }
    }

    fn part(name: &str) -> KeyPartDef {
        KeyPartDef { field_name: name.into(), offset: 0, length: 8, encoding: KeyEncodingDef::Bytes }
    }

    fn sample() -> IndexedDefinition {
        let mut def = IndexedDefinition::new("CUSTOMER-FILE", "data/customers.idx");
        def.keys.primary = KeyDef { name: Some("CUST-ID".into()), parts: vec![part("CUST-ID")], duplicates_allowed: false, ordering: KeyOrderingDef::Ascending };
        def.keys.alternates = vec![KeyDef { name: Some("TAX-KEY".into()), parts: vec![part("TAX-NO")], duplicates_allowed: false, ordering: KeyOrderingDef::Ascending }];
        def.fields = vec![field(
            1,
            "CUSTOMER-RECORD",
            false,
            vec![
                field(5, "CUST-ID", true, vec![]),
                field(5, "TAX-NO", true, vec![]),
                field(5, "DISCOUNT", true, vec![]),
                field(5, "ADDRESS", false, vec![field(10, "STREET", true, vec![])]),
            ],
        )];
        def
    }

    /// What the Comments tooltip asks for depends on what is selected: the file, its record, a key or
    /// alternate key (the field the key is made of), and any other field.
    #[test]
    fn the_kind_follows_what_is_selected() {
        let def = sample();
        let kind = |name: &str| definition_kind(&def, find_field(&def, name).unwrap());
        assert_eq!(kind("CUSTOMER-RECORD"), DefinitionKind::Record);
        assert_eq!(kind("CUST-ID"), DefinitionKind::Key, "the primary key's field");
        assert_eq!(kind("TAX-NO"), DefinitionKind::Key, "an alternate key's part");
        assert_eq!(kind("DISCOUNT"), DefinitionKind::Field);
        assert_eq!(kind("ADDRESS"), DefinitionKind::Field, "a group item below the record is a field");
        assert_eq!(kind("STREET"), DefinitionKind::Field);
    }

    /// Four different tooltips in each of the six languages, none left in English, each with its own
    /// good and bad example, and the code name MIN-AMOUNT kept as written where the field one uses it.
    #[test]
    fn every_language_has_four_distinct_tooltips_that_say_who_reads_them() {
        let mut checked = 0;
        for &lang in crate::i18n::Language::ALL {
            let tr = lang.tr();
            let tips = [
                tip_for(DefinitionKind::File, &tr),
                tip_for(DefinitionKind::Record, &tr),
                tip_for(DefinitionKind::Key, &tr),
                tip_for(DefinitionKind::Field, &tr),
            ];
            for (i, a) in tips.iter().enumerate() {
                assert!(a.len() > 150, "{lang:?} tip {i} is too short: {a}");
                for b in &tips[i + 1..] {
                    assert_ne!(a, b, "{lang:?}: two kinds share one tooltip");
                }
                checked += 1;
            }
            assert!(tips[3].contains("MIN-AMOUNT"), "{lang:?}: the field example keeps its code name");
            assert!(!tr.lbl_field_comments.trim().is_empty());
        }
        let en = crate::i18n::Language::ALL[0].tr();
        assert_eq!(en.lbl_field_comments, "Definition for AI");
        for &lang in &crate::i18n::Language::ALL[1..] {
            let t = lang.tr();
            assert_ne!(t.tip_def_ai_file, en.tip_def_ai_file, "{lang:?} still carries the English text");
        }
        println!("definition tooltips: {} languages x 4 kinds = {checked} texts, all distinct; label \"{}\"", crate::i18n::Language::ALL.len(), en.lbl_field_comments);
    }
}
