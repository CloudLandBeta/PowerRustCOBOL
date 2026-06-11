// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Properties inspector panel — rich, categorised property editor.
//!
//! Groups shown for any selected control:
//!   • Identity    — ID, Type (read-only)
//!   • Geometry    — X, Y, Width, Height
//!   • Appearance  — BackColor, ForeColor, Caption/Text, font, Visible, Enabled, TabOrder
//!   • Layout      — Dock, Anchor, Padding, Opacity
//!   • Data Binding— COBOL data item + format
//!   • Type-specific sections
//!   • Advanced    — Tooltip, Cursor
//!   • Events      — existing bindings + add-new
//!
//! When nothing is selected the panel shows Form Properties.

use egui::{Color32, DragValue, RichText, ScrollArea, Ui};
use cobolt_forms::{Control, ControlType, Form};
use cobolt_forms::model::{PropValue, AnimTrigger, AnimKind, EasingKind, AnimRepeat, BgImageMode};
use crate::i18n::Tr;

/// A colour-swatch button that opens egui's colour picker — but, unlike the stock
/// `ui.color_edit_button_srgba`, the popup closes as soon as the user clicks **and
/// releases** the mouse on a colour inside the picker (no need to click outside).
fn color_edit_button_closing(ui: &mut Ui, color: &mut Color32) -> egui::Response {
    use egui::{Area, Frame, Key, Order, Sense, Stroke, UiKind, Vec2};
    use egui::color_picker::{color_picker_color32, show_color_at, Alpha};

    // ── Colour swatch button ──────────────────────────────────────────────────
    let size = Vec2::new(35.0, ui.spacing().interact_size.y.max(16.0));
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        show_color_at(ui.painter(), *color, rect);
        ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_gray(120)));
    }

    let popup_id = resp.id.with("__closing_color_popup");
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }

    if ui.memory(|m| m.is_popup_open(popup_id)) {
        let area = Area::new(popup_id)
            .kind(UiKind::Picker)
            .order(Order::Foreground)
            .fixed_pos(resp.rect.max)
            .show(ui.ctx(), |ui| {
                ui.spacing_mut().slider_width = 275.0;
                let inner = Frame::popup(ui.style())
                    .show(ui, |ui| color_picker_color32(ui, color, Alpha::BlendOrAdditive));
                (inner.inner, inner.response.rect)
            });
        let (changed, popup_rect) = area.inner;
        if changed {
            resp.mark_changed();
        }

        // Close as soon as the pointer is released inside the picker (a colour was
        // picked), or via Escape / a click outside. The `!resp.clicked()` guard
        // prevents the opening click from immediately closing it.
        let released_inside = ui.input(|i| {
            i.pointer.any_released()
                && i.pointer.interact_pos().map_or(false, |p| popup_rect.contains(p))
        });
        if !resp.clicked()
            && (released_inside
                || ui.input(|i| i.key_pressed(Key::Escape))
                || area.response.clicked_elsewhere())
        {
            ui.memory_mut(|m| m.close_popup());
        }
    }

    resp
}

// ── Action ────────────────────────────────────────────────────────────────────

/// Actions the inspector wants the designer to perform this frame.
#[derive(Default)]
pub struct InspectorAction {
    pub set_props:      Vec<(String, String, PropValue)>,
    pub form_props:     Vec<(String, String)>,
    /// `(ctrl_id, event_name)` — emitted when the user clicks an event row to open the modal editor.
    /// `ctrl_id` is empty for form-level events.
    pub open_event_editor: Option<(String, String)>,
    /// `(ctrl_id, event_name)` — emitted when the user **double-clicks** an event row to
    /// jump to that event's paragraph in the generated COBOL code editor.
    /// `ctrl_id` is empty for form-level events.
    pub open_event_in_code: Option<(String, String)>,
}

// ── Panel ─────────────────────────────────────────────────────────────────────

pub struct PropertiesPanel {
    text_bufs:     std::collections::HashMap<String, String>,
    form_bufs:     std::collections::HashMap<String, String>,
    /// animation editor state per control: selected animation index
    anim_sel:      std::collections::HashMap<String, usize>,
    /// new-animation staging fields
    new_anim_name: String,
}

impl PropertiesPanel {
    pub fn new() -> Self {
        Self {
            text_bufs:       Default::default(),
            form_bufs:       Default::default(),
            anim_sel:        Default::default(),
            new_anim_name:   String::new(),
        }
    }

    pub fn show(
        &mut self,
        ui:   &mut Ui,
        form: &Form,
        ctrl: Option<&Control>,
        tr:   &Tr,
    ) -> InspectorAction {
        let mut action = InspectorAction::default();
        // Prevent any single property section from blowing out the panel width.
        // This is especially important for SqlDatabase which has many long fields.
        ui.set_max_width(ui.available_width());
        ScrollArea::vertical()
            .id_salt("properties_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                if let Some(ctrl) = ctrl {
                    self.show_control(ui, ctrl, &mut action, tr);
                } else {
                    self.show_form(ui, form, &mut action, tr);
                }
            });
        action
    }

    // ── Control inspector ─────────────────────────────────────────────────────

    fn show_control(&mut self, ui: &mut Ui, ctrl: &Control, action: &mut InspectorAction, tr: &Tr) {
        let id = ctrl.id.clone();

        // ── Identity ──────────────────────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.strong(&ctrl.id);
            ui.label(
                RichText::new(format!("[{}]", ctrl.control_type.as_str()))
                    .color(Color32::GRAY).small(),
            );
        });
        // LabelFor association
        {
            let cur = ctrl.get_prop("LabelFor").map(|v| v.as_str().to_owned()).unwrap_or_default();
            text_row_hint(ui, &mut self.text_bufs, &id, "LabelFor", &cur,
                tr.lbl_label_for, "LBL-1 (auto-arrange)", action);
        }
        ui.separator();

        // ── Geometry ──────────────────────────────────────────────────────────
        section_header(ui, tr.sec_geometry);
        egui::Grid::new(format!("geo_{id}"))
            .num_columns(4).spacing([4.0, 4.0])
            .show(ui, |ui| {
                ui.label("X:"); let mut x = ctrl.rect.x;
                if ui.add(DragValue::new(&mut x).speed(1)).changed() {
                    action.set_props.push((id.clone(), "X".into(), PropValue::Int(x as i64)));
                }
                ui.label("W:"); let mut w = ctrl.rect.w;
                if ui.add(DragValue::new(&mut w).speed(1).range(1..=9999)).changed() {
                    action.set_props.push((id.clone(), "Width".into(), PropValue::Int(w as i64)));
                }
                ui.end_row();

                ui.label("Y:"); let mut y = ctrl.rect.y;
                if ui.add(DragValue::new(&mut y).speed(1)).changed() {
                    action.set_props.push((id.clone(), "Y".into(), PropValue::Int(y as i64)));
                }
                ui.label("H:"); let mut h = ctrl.rect.h;
                if ui.add(DragValue::new(&mut h).speed(1).range(1..=9999)).changed() {
                    action.set_props.push((id.clone(), "Height".into(), PropValue::Int(h as i64)));
                }
                ui.end_row();

                ui.label("Z:"); let mut z = ctrl.z_order as i64;
                if ui.add(DragValue::new(&mut z).speed(1).prefix("z=")
                    .range(-9999..=9999)).changed()
                {
                    action.set_props.push((id.clone(), "ZOrder".into(), PropValue::Int(z)));
                }
                ui.label(RichText::new("(z-order)").small().color(Color32::GRAY));
                ui.end_row();
            });
        ui.add_space(4.0);

        // Non-visual controls (Timer, AgentObject, RestClient) only show
        // geometry + their own type-specific settings — no style, no animations.
        if ctrl.control_type.is_non_visual() {
            self.show_type_specific(ui, ctrl, &id, action);
            return;
        }

        // ── Appearance ────────────────────────────────────────────────────────
        section_header(ui, tr.sec_appearance);

        egui::Grid::new(format!("appearance_{id}")).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            // Caption (controls with an intrinsic text label) / Text (TextBox only)
            let text_key: Option<&str> = match ctrl.control_type {
                ControlType::Label
                | ControlType::Button
                | ControlType::CheckBox
                | ControlType::RadioButton
                | ControlType::GroupBox  => Some("Caption"),
                ControlType::TextBox     => Some("Text"),
                _                        => None,
            };
            if let Some(cap_key) = text_key {
                let cur = ctrl.get_prop(cap_key).map(|v| v.as_str().to_owned()).unwrap_or_default();
                ui.label(cap_key);
                {
                    let buf_key = format!("{id}-{cap_key}");
                    let wid = egui::Id::new(&buf_key);
                    let buf = self.text_bufs.entry(buf_key).or_insert_with(|| cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur.clone(); }
                    if ui.add(egui::TextEdit::singleline(buf).id(wid).desired_width(f32::INFINITY)).lost_focus() {
                        action.set_props.push((id.clone(), cap_key.into(), PropValue::String(buf.clone())));
                    }
                }
                ui.end_row();
            }

            // BackColor
            if ctrl.get_prop("BackColor").is_some() {
                let hex = ctrl.get_prop("BackColor").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "#F0F0F0".into());
                let mut color = hex_to_color32(&hex);
                ui.label(tr.lbl_back_color);
                ui.horizontal(|ui| {
                    if color_edit_button_closing(ui, &mut color).changed() {
                        action.set_props.push((id.clone(), "BackColor".into(), PropValue::String(color32_to_hex(color))));
                    }
                    ui.label(RichText::new(color32_to_hex(color)).monospace().small().color(Color32::GRAY));
                });
                ui.end_row();
            }

            // ForeColor
            if ctrl.get_prop("ForeColor").is_some() {
                let hex = ctrl.get_prop("ForeColor").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "#000000".into());
                let mut color = hex_to_color32(&hex);
                ui.label(tr.lbl_fore_color);
                ui.horizontal(|ui| {
                    if color_edit_button_closing(ui, &mut color).changed() {
                        action.set_props.push((id.clone(), "ForeColor".into(), PropValue::String(color32_to_hex(color))));
                    }
                    ui.label(RichText::new(color32_to_hex(color)).monospace().small().color(Color32::GRAY));
                });
                ui.end_row();
            }

            // Font name — dropdown of installed system fonts (fallback: Arial).
            ui.label(tr.lbl_font);
            {
                let cur = ctrl.get_prop("FontName").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Arial".into());
                let mut sel = cur.clone();
                let fonts = crate::fonts::system_fonts();
                // Show the selected font's name rendered in that font.
                let sel_fid = crate::fonts::font_id(ui.ctx(), &cur, 14.0);
                egui::ComboBox::from_id_salt(format!("{id}-FontName"))
                    .selected_text(egui::RichText::new(&cur).font(sel_fid))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        // If the saved font isn't installed here, still list it so it
                        // stays selectable (it falls back to Arial on a target lacking it).
                        if !fonts.iter().any(|f| f == &cur) {
                            ui.selectable_value(&mut sel, cur.clone(), format!("{cur}  (not installed)"));
                        }
                        // Virtualised list: only visible rows are laid out, so only the
                        // fonts you actually scroll past get loaded into egui (loading all
                        // ~400 system fonts up-front would be far too costly).
                        let row_h = ui.text_style_height(&egui::TextStyle::Button);
                        egui::ScrollArea::vertical()
                            .max_height(320.0)
                            .show_rows(ui, row_h, fonts.len(), |ui, range| {
                                for i in range {
                                    let fam = &fonts[i];
                                    let fid = crate::fonts::font_id(ui.ctx(), fam, 14.0);
                                    ui.selectable_value(
                                        &mut sel,
                                        fam.clone(),
                                        egui::RichText::new(fam).font(fid),
                                    );
                                }
                            });
                    });
                if sel != cur {
                    action.set_props.push((id.clone(), "FontName".into(), PropValue::String(sel)));
                }
            }
            ui.end_row();

            // Font size
            ui.label(tr.lbl_font_size);
            {
                let mut fs = ctrl.get_prop("FontSize").map(|v| v.as_i64()).unwrap_or(10);
                if ui.add(DragValue::new(&mut fs).speed(0.5).range(4..=200)).changed() {
                    action.set_props.push((id.clone(), "FontSize".into(), PropValue::Int(fs)));
                }
            }
            ui.end_row();

            // Font style
            ui.label(tr.lbl_style);
            ui.horizontal(|ui| {
                let mut bold = ctrl.get_prop("Bold").map(|v| v.as_bool()).unwrap_or(false);
                if ui.checkbox(&mut bold, "B").changed() {
                    action.set_props.push((id.clone(), "Bold".into(), PropValue::Bool(bold)));
                }
                let mut italic = ctrl.get_prop("Italic").map(|v| v.as_bool()).unwrap_or(false);
                if ui.checkbox(&mut italic, "I").changed() {
                    action.set_props.push((id.clone(), "Italic".into(), PropValue::Bool(italic)));
                }
                let mut under = ctrl.get_prop("Underline").map(|v| v.as_bool()).unwrap_or(false);
                if ui.checkbox(&mut under, "U").changed() {
                    action.set_props.push((id.clone(), "Underline".into(), PropValue::Bool(under)));
                }
                let mut strike = ctrl.get_prop("Strikethrough").map(|v| v.as_bool()).unwrap_or(false);
                if ui.checkbox(&mut strike, "S̶").changed() {
                    action.set_props.push((id.clone(), "Strikethrough".into(), PropValue::Bool(strike)));
                }
            });
            ui.end_row();

            // Visible / Enabled
            ui.label(tr.lbl_visible);
            {
                let mut vis = ctrl.visible;
                if ui.checkbox(&mut vis, "").changed() {
                    action.set_props.push((id.clone(), "Visible".into(), PropValue::Bool(vis)));
                }
            }
            ui.end_row();

            ui.label(tr.lbl_enabled);
            {
                let mut ena = ctrl.enabled;
                if ui.checkbox(&mut ena, "").changed() {
                    action.set_props.push((id.clone(), "Enabled".into(), PropValue::Bool(ena)));
                }
            }
            ui.end_row();

            // Tab order
            ui.label(tr.lbl_tab_order);
            {
                let mut to = ctrl.tab_order as i64;
                if ui.add(DragValue::new(&mut to).speed(1).range(0..=999)).changed() {
                    action.set_props.push((id.clone(), "TabOrder".into(), PropValue::Int(to)));
                }
            }
            ui.end_row();

            // Opacity
            if let Some(op) = ctrl.get_prop("Opacity") {
                ui.label(tr.lbl_opacity);
                let mut v = op.as_i64();
                if ui.add(DragValue::new(&mut v).speed(1).range(0..=100).suffix("%")).changed() {
                    action.set_props.push((id.clone(), "Opacity".into(), PropValue::Int(v)));
                }
                ui.end_row();
            }
        });

        // ── Drop Shadow ───────────────────────────────────────────────────────
        section_header(ui, tr.sec_shadow);
        egui::Grid::new(format!("shadow_{id}")).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            // Enabled toggle
            ui.label(tr.lbl_shadow_enabled);
            {
                let mut on = ctrl.get_prop("ShadowEnabled").map(|v| v.as_bool()).unwrap_or(false);
                if ui.checkbox(&mut on, "").changed() {
                    action.set_props.push((id.clone(), "ShadowEnabled".into(), PropValue::Bool(on)));
                }
            }
            ui.end_row();

            // Opacity
            ui.label(tr.lbl_shadow_opacity);
            {
                let mut op = ctrl.get_prop("ShadowOpacity").map(|v| v.as_i64()).unwrap_or(20);
                if ui.add(DragValue::new(&mut op).speed(1).range(0..=100).suffix("%")).changed() {
                    action.set_props.push((id.clone(), "ShadowOpacity".into(), PropValue::Int(op)));
                }
            }
            ui.end_row();

            // Color
            ui.label(tr.lbl_shadow_color);
            {
                let hex = ctrl.get_prop("ShadowColor").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "#000000".into());
                let mut color = hex_to_color32(&hex);
                ui.horizontal(|ui| {
                    if color_edit_button_closing(ui, &mut color).changed() {
                        action.set_props.push((id.clone(), "ShadowColor".into(), PropValue::String(color32_to_hex(color))));
                    }
                    ui.label(RichText::new(color32_to_hex(color)).monospace().small().color(Color32::GRAY));
                });
            }
            ui.end_row();

            // Direction
            ui.label(tr.lbl_shadow_direction);
            {
                let cur_dir = ctrl.get_prop("ShadowDirection").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "South".into());
                egui::ComboBox::from_id_salt(format!("shadow_dir_{id}"))
                    .selected_text(&cur_dir).width(120.0)
                    .show_ui(ui, |ui| {
                        for opt in &["North","NorthEast","East","SouthEast","South","SouthWest","West","NorthWest"] {
                            if ui.selectable_label(cur_dir == *opt, *opt).clicked() {
                                action.set_props.push((id.clone(), "ShadowDirection".into(),
                                    PropValue::String(opt.to_string())));
                            }
                        }
                    });
            }
            ui.end_row();

            // Distance
            ui.label(tr.lbl_shadow_distance);
            {
                let mut dist = ctrl.get_prop("ShadowDistance").map(|v| v.as_i64()).unwrap_or(7);
                if ui.add(DragValue::new(&mut dist).speed(1).range(0..=60).suffix("px")).changed() {
                    action.set_props.push((id.clone(), "ShadowDistance".into(), PropValue::Int(dist)));
                }
            }
            ui.end_row();

            // Blur enabled
            ui.label(tr.lbl_shadow_blur);
            {
                let mut blur_on = ctrl.get_prop("ShadowBlur").map(|v| v.as_bool()).unwrap_or(true);
                if ui.checkbox(&mut blur_on, "").changed() {
                    action.set_props.push((id.clone(), "ShadowBlur".into(), PropValue::Bool(blur_on)));
                }
            }
            ui.end_row();

            // Blur strength
            ui.label(tr.lbl_shadow_blur_strength);
            {
                let mut bs = ctrl.get_prop("ShadowBlurStrength").map(|v| v.as_i64()).unwrap_or(8);
                if ui.add(DragValue::new(&mut bs).speed(1).range(0..=20)).changed() {
                    action.set_props.push((id.clone(), "ShadowBlurStrength".into(), PropValue::Int(bs)));
                }
            }
            ui.end_row();
        });
        ui.add_space(4.0);

        // ── Layout ────────────────────────────────────────────────────────────
        section_header(ui, tr.sec_layout);
        egui::Grid::new(format!("layout_{id}")).num_columns(2).spacing([4.0, 3.0]).show(ui, |ui| {
            // Dock
            let cur_dock = ctrl.get_prop("Dock")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_else(|| "None".into());
            ui.label(tr.lbl_dock);
            egui::ComboBox::from_id_salt(format!("dock_{id}"))
                .selected_text(&cur_dock).width(120.0)
                .show_ui(ui, |ui| {
                    for opt in &["None", "Top", "Bottom", "Left", "Right", "Fill"] {
                        if ui.selectable_label(cur_dock == *opt, *opt).clicked() {
                            action.set_props.push((id.clone(), "Dock".into(),
                                PropValue::String(opt.to_string())));
                        }
                    }
                });
            ui.end_row();

            // Anchor
            let cur_anc = ctrl.get_prop("Anchor")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_else(|| "Top,Left".into());
            ui.label(tr.lbl_anchor);
            {
                let buf_key = format!("{id}-Anchor");
                let wid = egui::Id::new(&buf_key);
                let buf = self.text_bufs.entry(buf_key).or_insert(cur_anc.clone());
                if *buf != cur_anc && !ui.memory(|m| m.has_focus(wid)) {
                    *buf = cur_anc;
                }
                if ui.add(egui::TextEdit::singleline(buf)
                    .id(wid)
                    .hint_text("Top,Left")
                    .desired_width(120.0)).lost_focus()
                {
                    action.set_props.push((id.clone(), "Anchor".into(),
                        PropValue::String(buf.clone())));
                }
            }
            ui.end_row();

            // Padding
            ui.label(tr.lbl_padding);
            let mut pad = ctrl.get_prop("Padding").map(|v| v.as_i64()).unwrap_or(0);
            if ui.add(DragValue::new(&mut pad).speed(1).range(0..=200)).changed() {
                action.set_props.push((id.clone(), "Padding".into(), PropValue::Int(pad)));
            }
            ui.end_row();
        });
        ui.add_space(4.0);

        // ── Data Binding ──────────────────────────────────────────────────────
        section_header(ui, tr.sec_data_binding);
        egui::Grid::new(format!("binding_{id}")).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label(tr.lbl_cobol_data_item);
            {
                let cur = ctrl.get_prop("DataItem").map(|v| v.as_str().to_owned()).unwrap_or_default();
                let buf_key = format!("{id}-DataItem");
                let wid = egui::Id::new(&buf_key);
                let buf = self.text_bufs.entry(buf_key).or_insert_with(|| cur.clone());
                if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                if ui.add(egui::TextEdit::singleline(buf).id(wid)
                    .hint_text("WS-FIELD-NAME").desired_width(f32::INFINITY)).lost_focus()
                {
                    action.set_props.push((id.clone(), "DataItem".into(), PropValue::String(buf.clone())));
                }
            }
            ui.end_row();

            ui.label(tr.lbl_format_mask);
            {
                let cur = ctrl.get_prop("DataFormat").map(|v| v.as_str().to_owned()).unwrap_or_default();
                let buf_key = format!("{id}-DataFormat");
                let wid = egui::Id::new(&buf_key);
                let buf = self.text_bufs.entry(buf_key).or_insert_with(|| cur.clone());
                if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                if ui.add(egui::TextEdit::singleline(buf).id(wid)
                    .hint_text("e.g. 99/99/9999").desired_width(f32::INFINITY)).lost_focus()
                {
                    action.set_props.push((id.clone(), "DataFormat".into(), PropValue::String(buf.clone())));
                }
            }
            ui.end_row();
        });
        ui.label(
            RichText::new("Control value syncs when the named COBOL data item changes at runtime.")
                .color(Color32::GRAY).small().italics(),
        );
        ui.add_space(4.0);

        // ── Type-specific ─────────────────────────────────────────────────────
        self.show_type_specific(ui, ctrl, &id, action);

        // ── Advanced ──────────────────────────────────────────────────────────
        section_header(ui, tr.sec_advanced);
        egui::Grid::new(format!("adv_{id}")).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            // Tooltip
            ui.label(tr.lbl_tooltip_lbl);
            {
                let cur = ctrl.get_prop("Tooltip").map(|v| v.as_str().to_owned()).unwrap_or_default();
                let buf_key = format!("{id}-Tooltip");
                let wid = egui::Id::new(&buf_key);
                let buf = self.text_bufs.entry(buf_key).or_insert_with(|| cur.clone());
                if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                if ui.add(egui::TextEdit::singleline(buf).id(wid)
                    .hint_text("(shown on hover)").desired_width(f32::INFINITY)).lost_focus()
                {
                    action.set_props.push((id.clone(), "Tooltip".into(), PropValue::String(buf.clone())));
                }
            }
            ui.end_row();

            // Cursor
            ui.label(tr.lbl_cursor_lbl);
            {
                let cur_cursor = ctrl.get_prop("Cursor")
                    .map(|v| v.as_str().to_owned())
                    .unwrap_or_else(|| "Default".into());
                egui::ComboBox::from_id_salt(format!("cur_{id}"))
                    .selected_text(&cur_cursor).width(130.0)
                    .show_ui(ui, |ui| {
                        for opt in &["Default","Hand","Text","Wait","Crosshair",
                                     "No","SizeAll","SizeNS","SizeWE","Help"] {
                            if ui.selectable_label(cur_cursor == *opt, *opt).clicked() {
                                action.set_props.push((id.clone(), "Cursor".into(),
                                    PropValue::String(opt.to_string())));
                            }
                        }
                    });
            }
            ui.end_row();
        });
        ui.add_space(4.0);

        // ── Events ────────────────────────────────────────────────────────────
        section_header(ui, tr.sec_events);
        ui.label(RichText::new(tr.hint_click_event)
            .small().color(Color32::GRAY).italics());
        ui.add_space(4.0);

        for ev in ctrl.control_type.supported_events() {
            let ev_str   = ev.to_string();
            let binding  = ctrl.events.iter().find(|e| e.event == ev_str);
            let has_code = binding.map(|e| e.has_code()).unwrap_or(false);
            let lines    = binding.map(|e| e.code_line_count()).unwrap_or(0);

            let row_resp = ui.horizontal(|ui| {
                let dot_color = if has_code {
                    Color32::from_rgb(100, 220, 100)
                } else {
                    Color32::from_rgb(120, 120, 120)
                };
                ui.label(RichText::new(if has_code { "●" } else { "○" }).color(dot_color));
                let lbl = ui.add(
                    egui::Label::new(
                        RichText::new(format!("⚙ {ev_str}"))
                            .color(Color32::from_rgb(200, 200, 100))
                    ).sense(egui::Sense::click())
                ).on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(tr.hint_dblclick_event);
                if has_code {
                    ui.label(RichText::new(format!("({lines} {})", tr.hint_lines))
                        .small().color(Color32::GRAY));
                } else {
                    ui.label(RichText::new(tr.hint_click_to_add)
                        .small().color(Color32::from_rgb(100, 100, 100)).italics());
                }
                (lbl.clicked(), lbl.double_clicked())
            });

            let (clicked, double_clicked) = row_resp.inner;
            if double_clicked {
                action.open_event_in_code = Some((id.clone(), ev_str));
            } else if clicked {
                action.open_event_editor = Some((id.clone(), ev_str));
            }
        }

        // ── Animations ────────────────────────────────────────────────────────
        self.show_animations(ui, ctrl, &id, action, tr);
    }

    // ── Animation editor ─────────────────────────────────────────────────────

    fn show_animations(
        &mut self, ui: &mut Ui, ctrl: &Control, id: &str, action: &mut InspectorAction, tr: &Tr,
    ) {
        section_header(ui, tr.sec_animations);

        ui.label(RichText::new(
            "Each animation is a named effect triggered by an event or from COBOL.\n\
             COBOL: INVOKE ctrl-id 'PlayAnimation' USING BY VALUE 'fly-in'")
            .small().color(Color32::GRAY).italics());
        ui.add_space(2.0);

        // List existing animations
        let anim_count = ctrl.animations.len();
        let sel = *self.anim_sel.entry(id.to_owned()).or_insert(0);

        if anim_count == 0 {
            ui.label(RichText::new("(no animations — add one below)").small().color(Color32::GRAY));
        } else {
            // Selector tabs for each animation
            ui.horizontal_wrapped(|ui| {
                for (i, anim) in ctrl.animations.iter().enumerate() {
                    let active = i == sel;
                    if ui.selectable_label(active,
                        RichText::new(&anim.name).small()
                    ).clicked() {
                        *self.anim_sel.entry(id.to_owned()).or_insert(0) = i;
                    }
                }
            });
            ui.add_space(2.0);

            // Edit the selected animation
            if let Some(anim) = ctrl.animations.get(sel) {
                let anim_id = format!("{id}-anim{sel}");

                egui::Grid::new(format!("anim_grid_{anim_id}"))
                    .num_columns(2).spacing([4.0,3.0])
                    .show(ui, |ui| {

                    // Name
                    ui.label("Name:");
                    let cur_name = anim.name.clone();
                    let bk = format!("{anim_id}-name");
                    let wid = egui::Id::new(&bk);
                    let buf = self.text_bufs.entry(bk).or_insert(cur_name.clone());
                    if *buf != cur_name && !ui.memory(|m| m.has_focus(wid)) { *buf = cur_name.clone(); }
                    if ui.add(egui::TextEdit::singleline(buf).id(wid).desired_width(f32::INFINITY)).lost_focus() {
                        action.set_props.push((id.to_owned(), format!("Anim{sel}_Name"), PropValue::String(buf.clone())));
                    }
                    ui.end_row();

                    // Trigger
                    let cur_t = anim.trigger.as_str().to_owned();
                    ui.label("Trigger:");
                    egui::ComboBox::from_id_salt(format!("anim_trigger_{anim_id}"))
                        .selected_text(&cur_t).width(140.0)
                        .show_ui(ui, |ui| {
                            for &opt in AnimTrigger::ALL {
                                if ui.selectable_label(cur_t == opt, opt).clicked() {
                                    action.set_props.push((id.to_owned(), format!("Anim{sel}_Trigger"), PropValue::String(opt.to_owned())));
                                }
                            }
                        });
                    ui.end_row();

                    // Kind
                    let cur_k = anim.kind.as_str().to_owned();
                    ui.label("Effect:");
                    egui::ComboBox::from_id_salt(format!("anim_kind_{anim_id}"))
                        .selected_text(&cur_k).width(140.0)
                        .show_ui(ui, |ui| {
                            for &opt in AnimKind::ALL {
                                if ui.selectable_label(cur_k == opt, opt).clicked() {
                                    action.set_props.push((id.to_owned(), format!("Anim{sel}_Kind"), PropValue::String(opt.to_owned())));
                                }
                            }
                        });
                    ui.end_row();

                    // Duration
                    ui.label("Duration (ms):");
                    let mut dur = anim.duration_ms as i64;
                    if ui.add(DragValue::new(&mut dur).speed(10).range(50..=30_000)).changed() {
                        action.set_props.push((id.to_owned(), format!("Anim{sel}_Duration"), PropValue::Int(dur)));
                    }
                    ui.end_row();

                    // Delay
                    ui.label("Delay (ms):");
                    let mut delay = anim.delay_ms as i64;
                    if ui.add(DragValue::new(&mut delay).speed(10).range(0..=10_000)).changed() {
                        action.set_props.push((id.to_owned(), format!("Anim{sel}_Delay"), PropValue::Int(delay)));
                    }
                    ui.end_row();

                    // Easing
                    let cur_e = anim.easing.as_str().to_owned();
                    ui.label("Easing:");
                    egui::ComboBox::from_id_salt(format!("anim_ease_{anim_id}"))
                        .selected_text(&cur_e).width(140.0)
                        .show_ui(ui, |ui| {
                            for &opt in EasingKind::ALL {
                                if ui.selectable_label(cur_e == opt, opt).clicked() {
                                    action.set_props.push((id.to_owned(), format!("Anim{sel}_Easing"), PropValue::String(opt.to_owned())));
                                }
                            }
                        });
                    ui.end_row();

                    // Repeat
                    let cur_r = anim.repeat.as_str().to_owned();
                    ui.label("Repeat:");
                    egui::ComboBox::from_id_salt(format!("anim_rep_{anim_id}"))
                        .selected_text(&cur_r).width(140.0)
                        .show_ui(ui, |ui| {
                            for &opt in AnimRepeat::ALL {
                                if ui.selectable_label(cur_r == opt, opt).clicked() {
                                    action.set_props.push((id.to_owned(), format!("Anim{sel}_Repeat"), PropValue::String(opt.to_owned())));
                                }
                            }
                        });
                    ui.end_row();

                    // Slide offsets (shown only for Slide kind)
                    if anim.kind.as_str() == "Slide" {
                        ui.label("Slide DX:");
                        let mut sdx = anim.slide_dx as i64;
                        if ui.add(DragValue::new(&mut sdx).speed(4)).changed() {
                            action.set_props.push((id.to_owned(), format!("Anim{sel}_SlideDX"), PropValue::Int(sdx)));
                        }
                        ui.end_row();
                        ui.label("Slide DY:");
                        let mut sdy = anim.slide_dy as i64;
                        if ui.add(DragValue::new(&mut sdy).speed(4)).changed() {
                            action.set_props.push((id.to_owned(), format!("Anim{sel}_SlideDY"), PropValue::Int(sdy)));
                        }
                        ui.end_row();
                    }
                });

                // Preview + Remove buttons
                ui.horizontal(|ui| {
                    if ui.button("▶ Preview").clicked() {
                        action.set_props.push((
                            id.to_owned(),
                            format!("_PreviewAnim{sel}"),
                            PropValue::String(anim.name.clone()),
                        ));
                    }
                    if ui.button("🗑 Remove").clicked() {
                        action.set_props.push((
                            id.to_owned(),
                            format!("_RemoveAnim{sel}"),
                            PropValue::String(anim.name.clone()),
                        ));
                    }
                });
            }
        }

        // Add new animation
        ui.add_space(4.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("New animation name:");
            ui.add(egui::TextEdit::singleline(&mut self.new_anim_name)
                .hint_text("fly-in").desired_width(120.0));
            if ui.button("➕ Add").clicked() && !self.new_anim_name.is_empty() {
                action.set_props.push((
                    id.to_owned(),
                    "_AddAnimation".to_owned(),
                    PropValue::String(std::mem::take(&mut self.new_anim_name)),
                ));
            }
        });
        ui.add_space(4.0);
    }

    // ── Type-specific sections ────────────────────────────────────────────────

    fn show_type_specific(
        &mut self, ui: &mut Ui, ctrl: &Control, id: &str, action: &mut InspectorAction,
    ) {
        match ctrl.control_type {

            // ── Button ────────────────────────────────────────────────────────
            ControlType::Button => {
                section_header(ui, "Button");
                egui::Grid::new(format!("btn_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    bool_row(ui, id, "IsDefault", "Default button", ctrl, action); ui.end_row();
                    bool_row(ui, id, "IsCancel",  "Cancel button",  ctrl, action); ui.end_row();
                    bool_row(ui, id, "FlatStyle", "Flat style",     ctrl, action); ui.end_row();

                    ui.label("CornerRadius:");
                    let mut r = ctrl.get_prop("CornerRadius").map(|v| v.as_i64()).unwrap_or(3);
                    if ui.add(DragValue::new(&mut r).speed(1).range(0..=50)).changed() {
                        action.set_props.push((id.to_owned(), "CornerRadius".into(), PropValue::Int(r)));
                    }
                    ui.end_row();

                    combo_row(ui, id, "ModalResult", ctrl, action,
                        &["None","OK","Cancel","Yes","No","Abort","Retry","Ignore"]);
                    ui.end_row();

                    combo_row(ui, id, "TextAlign", ctrl, action,
                        &["MiddleCenter","TopLeft","TopCenter","TopRight",
                          "MiddleLeft","MiddleRight","BottomLeft","BottomCenter","BottomRight"]);
                    ui.end_row();
                });
                // Image
                image_browse_row(ui, id, "ImagePath", ctrl, action, &mut self.text_bufs);
                combo_row_inline(ui, id, "ImageAlign", ctrl, action,
                    &["MiddleLeft","MiddleCenter","MiddleRight","TopLeft","BottomLeft"]);
                border_rows(ui, id, ctrl, action, &mut self.text_bufs);
                ui.add_space(4.0);
            }

            // ── Label ─────────────────────────────────────────────────────────
            ControlType::Label => {
                section_header(ui, "Label");
                combo_row_inline(ui, id, "TextAlign", ctrl, action, &["Left","Center","Right"]);
                bool_row_inline(ui, id, "WordWrap", "WordWrap", ctrl, action);
                bool_row_inline(ui, id, "AutoSize", "AutoSize", ctrl, action);
                combo_row_inline(ui, id, "BorderStyle", ctrl, action, &["None","Single","Fixed3D"]);
                ui.add_space(4.0);
            }

            // ── TextBox ───────────────────────────────────────────────────────
            ControlType::TextBox => {
                section_header(ui, "TextBox");
                {
                    let cur = ctrl.get_prop("HintText").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "HintText", &cur,
                        "Hint text:", "(placeholder)", action);
                }
                egui::Grid::new(format!("tbx_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    ui.label("MaxLength:");
                    let mut ml = ctrl.get_prop("MaxLength").map(|v| v.as_i64()).unwrap_or(0);
                    if ui.add(DragValue::new(&mut ml).speed(1).range(0..=32767)).changed() {
                        action.set_props.push((id.to_owned(), "MaxLength".into(), PropValue::Int(ml)));
                    }
                    ui.end_row();
                    bool_row(ui, id, "Multiline", "Multiline", ctrl, action); ui.end_row();
                    bool_row(ui, id, "WordWrap",  "WordWrap",  ctrl, action); ui.end_row();
                    bool_row(ui, id, "ReadOnly",  "ReadOnly",  ctrl, action); ui.end_row();

                    ui.label("PwdChar:");
                    let buf_key = format!("{id}-PasswordChar");
                    let wid = egui::Id::new(&buf_key);
                    let cur = ctrl.get_prop("PasswordChar").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    let buf = self.text_bufs.entry(buf_key).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                    if ui.add(egui::TextEdit::singleline(buf).id(wid).desired_width(30.0)).lost_focus() {
                        buf.truncate(1);
                        action.set_props.push((id.to_owned(), "PasswordChar".into(), PropValue::String(buf.clone())));
                    }
                    ui.end_row();

                    combo_row(ui, id, "ScrollBars", ctrl, action,
                        &["None","Horizontal","Vertical","Both"]);
                    ui.end_row();
                });
                border_rows(ui, id, ctrl, action, &mut self.text_bufs);
                ui.add_space(4.0);
            }

            // ── CheckBox / RadioButton ────────────────────────────────────────
            ControlType::CheckBox | ControlType::RadioButton => {
                section_header(ui, "Check Options");
                bool_row_inline(ui, id, "Checked",  "Checked (default)", ctrl, action);
                combo_row_inline(ui, id, "CheckAlign", ctrl, action, &["Left","Center","Right"]);
                color_row(ui, id, "CheckColor", ctrl, action);
                if matches!(ctrl.control_type, ControlType::RadioButton) {
                    let cur = ctrl.get_prop("GroupName").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "GroupName", &cur,
                        "Group:", "group-name", action);
                }
                ui.add_space(4.0);
            }

            // ── PictureBox ────────────────────────────────────────────────────
            ControlType::PictureBox => {
                section_header(ui, "Image");
                image_browse_row(ui, id, "ImagePath", ctrl, action, &mut self.text_bufs);
                combo_row_inline(ui, id, "SizeMode", ctrl, action,
                    &["Normal","Stretch","Zoom","CenterImage","AutoSize"]);
                combo_row_inline(ui, id, "ImageAlign", ctrl, action,
                    &["TopLeft","TopCenter","TopRight",
                      "MiddleLeft","MiddleCenter","MiddleRight",
                      "BottomLeft","BottomCenter","BottomRight"]);
                {
                    // Frame toggle — when off, only the image is shown (transparent
                    // PNG areas reveal whatever is behind the control).
                    let mut show = ctrl.get_prop("ShowFrame").map(|v| v.as_bool()).unwrap_or(true);
                    if ui.checkbox(&mut show, "Show frame (uncheck = image only)").changed() {
                        action.set_props.push((id.to_owned(), "ShowFrame".into(), PropValue::Bool(show)));
                    }
                }
                border_rows(ui, id, ctrl, action, &mut self.text_bufs);
                ui.add_space(4.0);
            }

            // ── Animator ──────────────────────────────────────────────────────
            ControlType::Animator => {
                section_header(ui, "Animation");
                image_browse_row(ui, id, "Source", ctrl, action, &mut self.text_bufs);
                combo_row_inline(ui, id, "SizeMode", ctrl, action,
                    &["Fit","Fill","Stretch","Center"]);
                egui::Grid::new(format!("anim_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    bool_row(ui, id, "AutoPlay", "Auto-play", ctrl, action); ui.end_row();
                    bool_row(ui, id, "Loop",     "Loop",      ctrl, action); ui.end_row();
                });
                border_rows(ui, id, ctrl, action, &mut self.text_bufs);
                ui.add_space(4.0);
            }

            // ── ListBox ───────────────────────────────────────────────────────
            ControlType::ListBox => {
                section_header(ui, "ListBox");
                items_multiline(ui, id, ctrl, action, &mut self.text_bufs);
                egui::Grid::new(format!("lb_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    bool_row(ui, id, "MultiSelect", "Multi-select", ctrl, action); ui.end_row();
                    bool_row(ui, id, "Sorted",      "Sorted",       ctrl, action); ui.end_row();
                });
                border_rows(ui, id, ctrl, action, &mut self.text_bufs);
                ui.add_space(4.0);
            }

            // ── ComboBox ─────────────────────────────────────────────────────
            ControlType::ComboBox => {
                section_header(ui, "ComboBox");
                items_multiline(ui, id, ctrl, action, &mut self.text_bufs);
                egui::Grid::new(format!("cb_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    bool_row(ui, id, "Sorted",   "Sorted",   ctrl, action); ui.end_row();
                    bool_row(ui, id, "Editable", "Editable", ctrl, action); ui.end_row();
                    combo_row(ui, id, "DropDownStyle", ctrl, action,
                        &["DropDown","DropDownList","Simple"]);
                    ui.end_row();
                    ui.label("DropDownHeight:");
                    let mut ddh = ctrl.get_prop("DropDownHeight").map(|v| v.as_i64()).unwrap_or(200);
                    if ui.add(DragValue::new(&mut ddh).speed(1).range(50..=600)).changed() {
                        action.set_props.push((id.to_owned(), "DropDownHeight".into(), PropValue::Int(ddh)));
                    }
                    ui.end_row();
                });
                ui.add_space(4.0);
            }

            // ── Slider ───────────────────────────────────────────────────────
            ControlType::Slider => {
                section_header(ui, "Slider");
                egui::Grid::new(format!("sld_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    ui.label("Min:"); let mut min = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0);
                    if ui.add(DragValue::new(&mut min).speed(1)).changed() {
                        action.set_props.push((id.to_owned(), "Minimum".into(), PropValue::Int(min)));
                    }
                    ui.end_row();
                    ui.label("Max:"); let mut max = ctrl.get_prop("Maximum").map(|v| v.as_i64()).unwrap_or(100);
                    if ui.add(DragValue::new(&mut max).speed(1)).changed() {
                        action.set_props.push((id.to_owned(), "Maximum".into(), PropValue::Int(max)));
                    }
                    ui.end_row();
                    ui.label("Value:"); let mut val = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0);
                    if ui.add(DragValue::new(&mut val).speed(1)).changed() {
                        action.set_props.push((id.to_owned(), "Value".into(), PropValue::Int(val)));
                    }
                    ui.end_row();
                    ui.label("Step:"); let mut step = ctrl.get_prop("Step").map(|v| v.as_i64()).unwrap_or(10);
                    if ui.add(DragValue::new(&mut step).speed(1).range(1..=9999)).changed() {
                        action.set_props.push((id.to_owned(), "Step".into(), PropValue::Int(step)));
                    }
                    ui.end_row();
                    ui.label("Large change:"); let mut lc = ctrl.get_prop("LargeChange").map(|v| v.as_i64()).unwrap_or(20);
                    if ui.add(DragValue::new(&mut lc).speed(1).range(1..=9999)).changed() {
                        action.set_props.push((id.to_owned(), "LargeChange".into(), PropValue::Int(lc)));
                    }
                    ui.end_row();
                    ui.label("Tick frequency:"); let mut tf = ctrl.get_prop("TickFrequency").map(|v| v.as_i64()).unwrap_or(10);
                    if ui.add(DragValue::new(&mut tf).speed(1).range(1..=9999)).changed() {
                        action.set_props.push((id.to_owned(), "TickFrequency".into(), PropValue::Int(tf)));
                    }
                    ui.end_row();
                    combo_row(ui, id, "Orientation", ctrl, action, &["Horizontal","Vertical"]); ui.end_row();
                    combo_row(ui, id, "TickStyle",   ctrl, action, &["Bottom","Top","Both","None"]); ui.end_row();
                    bool_row(ui, id, "ShowValue", "Show value label", ctrl, action); ui.end_row();
                });
                color_row(ui, id, "TrackColor", ctrl, action);
                color_row(ui, id, "ThumbColor", ctrl, action);
                color_row(ui, id, "FillColor",  ctrl, action);
                {
                    let cur = ctrl.get_prop("ChangePara").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "ChangePara", &cur,
                        "On change PERFORM:", "SLIDER-CHANGED-PARA", action);
                }
                ui.add_space(4.0);
            }

            // ── ProgressBar ───────────────────────────────────────────────────
            ControlType::ProgressBar => {
                section_header(ui, "Progress");
                egui::Grid::new(format!("pb_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    ui.label("Min:"); let mut min = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0);
                    if ui.add(DragValue::new(&mut min).speed(1)).changed() {
                        action.set_props.push((id.to_owned(), "Minimum".into(), PropValue::Int(min)));
                    }
                    ui.end_row();
                    ui.label("Max:"); let mut max = ctrl.get_prop("Maximum").map(|v| v.as_i64()).unwrap_or(100);
                    if ui.add(DragValue::new(&mut max).speed(1)).changed() {
                        action.set_props.push((id.to_owned(), "Maximum".into(), PropValue::Int(max)));
                    }
                    ui.end_row();
                    ui.label("Value:"); let mut val = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0);
                    if ui.add(DragValue::new(&mut val).speed(1)).changed() {
                        action.set_props.push((id.to_owned(), "Value".into(), PropValue::Int(val)));
                    }
                    ui.end_row();
                    combo_row(ui, id, "Orientation", ctrl, action, &["Horizontal","Vertical"]); ui.end_row();
                    combo_row(ui, id, "Style",       ctrl, action, &["Continuous","Blocks"]);   ui.end_row();
                    bool_row(ui, id, "ShowValue",    "Show value text", ctrl, action);          ui.end_row();
                });
                color_row(ui, id, "BarColor", ctrl, action);
                ui.add_space(4.0);
            }

            // ── DataGrid ─────────────────────────────────────────────────────
            ControlType::DataGrid => {
                section_header(ui, "DataGrid");
                {
                    let cur = ctrl.get_prop("Columns").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    let buf_key = format!("{id}-Columns");
                    let wid = egui::Id::new(&buf_key);
                    let buf = self.text_bufs.entry(buf_key).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                    ui.label("Columns (comma-sep):");
                    if ui.add(egui::TextEdit::singleline(buf).id(wid).desired_width(f32::INFINITY)).lost_focus() {
                        action.set_props.push((id.to_owned(), "Columns".into(), PropValue::String(buf.clone())));
                    }
                }
                egui::Grid::new(format!("dg_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    bool_row(ui, id, "ReadOnly",          "ReadOnly",           ctrl, action); ui.end_row();
                    bool_row(ui, id, "AllowSorting",      "Allow sorting",      ctrl, action); ui.end_row();
                    bool_row(ui, id, "AllowColumnResize", "Allow col resize",   ctrl, action); ui.end_row();
                    bool_row(ui, id, "ShowRowNumbers",    "Show row numbers",   ctrl, action); ui.end_row();
                    combo_row(ui, id, "SelectionMode", ctrl, action, &["Row","Cell","Column"]); ui.end_row();
                    ui.label("Row height:");
                    let mut rh = ctrl.get_prop("RowHeight").map(|v| v.as_i64()).unwrap_or(22);
                    if ui.add(DragValue::new(&mut rh).speed(1).range(14..=120)).changed() {
                        action.set_props.push((id.to_owned(), "RowHeight".into(), PropValue::Int(rh)));
                    }
                    ui.end_row();
                });
                color_row(ui, id, "HeaderBackColor",     ctrl, action);
                color_row(ui, id, "HeaderForeColor",     ctrl, action);
                color_row(ui, id, "AlternatingRowColor", ctrl, action);
                color_row(ui, id, "GridLineColor",       ctrl, action);

                section_header(ui, "📄 CSV Export");
                egui::Grid::new(format!("dg_csv_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    bool_row(ui, id, "ExportCSV", "Enable CSV export", ctrl, action); ui.end_row();
                    ui.label("Delimiter:");
                    let cur_d = ctrl.get_prop("CSVDelimiter").map(|v| v.as_str().to_owned()).unwrap_or_else(|| ",".into());
                    let buf_key = format!("{id}-CSVDelimiter");
                    let wid = egui::Id::new(&buf_key);
                    let buf = self.text_bufs.entry(buf_key).or_insert(cur_d.clone());
                    if *buf != cur_d && !ui.memory(|m| m.has_focus(wid)) { *buf = cur_d; }
                    if ui.add(egui::TextEdit::singleline(buf).id(wid).desired_width(30.0)).lost_focus() {
                        action.set_props.push((id.to_owned(), "CSVDelimiter".into(), PropValue::String(buf.clone())));
                    }
                    ui.end_row();
                });
                {
                    let cur = ctrl.get_prop("CSVParagraph").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "CSVParagraph", &cur,
                        "After export PERFORM:", "CSV-EXPORTED-PARA", action);
                }
                ui.label(RichText::new("COBOL: INVOKE grid-id 'ExportCSV' USING WS-CSV-PATH")
                    .small().color(Color32::GRAY).italics());
                ui.add_space(4.0);
            }

            // ── TabControl ────────────────────────────────────────────────────
            ControlType::TabControl => {
                section_header(ui, "TabControl");
                {
                    let cur = ctrl.get_prop("Tabs").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    let buf_key = format!("{id}-Tabs");
                    let wid = egui::Id::new(&buf_key);
                    let buf = self.text_bufs.entry(buf_key).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                    ui.label("Tabs (one per line):");
                    let resp = ui.add(egui::TextEdit::multiline(buf)
                        .id(wid).desired_rows(3).desired_width(f32::INFINITY));
                    if resp.lost_focus() {
                        action.set_props.push((id.to_owned(), "Tabs".into(), PropValue::String(buf.clone())));
                    }
                }
                combo_row_inline(ui, id, "TabPosition", ctrl, action, &["Top","Bottom","Left","Right"]);
                ui.add_space(4.0);
            }

            // ── Panel / GroupBox ──────────────────────────────────────────────
            ControlType::Panel | ControlType::GroupBox => {
                section_header(ui, "Container");
                bool_row_inline(ui, id, "Scrollable", "Scrollable", ctrl, action);
                border_rows(ui, id, ctrl, action, &mut self.text_bufs);
                ui.add_space(4.0);
            }

            // ── Line ─────────────────────────────────────────────────────────
            ControlType::Line => {
                section_header(ui, "Line");
                color_row(ui, id, "LineColor", ctrl, action);
                egui::Grid::new(format!("ln_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    ui.label("Thickness:");
                    let mut t = ctrl.get_prop("LineThickness").map(|v| v.as_i64()).unwrap_or(1);
                    if ui.add(DragValue::new(&mut t).speed(1).range(1..=32)).changed() {
                        action.set_props.push((id.to_owned(), "LineThickness".into(), PropValue::Int(t)));
                    }
                    ui.end_row();
                    combo_row(ui, id, "LineDirection", ctrl, action,
                        &["Horizontal","Vertical","Diagonal"]); ui.end_row();
                    combo_row(ui, id, "DashStyle",     ctrl, action,
                        &["Solid","Dash","Dot","DashDot"]); ui.end_row();
                });
                ui.add_space(4.0);
            }

            // ── DateTimePicker ────────────────────────────────────────────────
            ControlType::DateTimePicker => {
                section_header(ui, "DateTimePicker");
                {
                    let cur = ctrl.get_prop("Value").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "Value", &cur,
                        "Value:", "YYYY-MM-DD", action);
                }
                egui::Grid::new(format!("dtp_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    combo_row(ui, id, "Format", ctrl, action,
                        &["Short","Long","Time","Custom"]); ui.end_row();
                    bool_row(ui, id, "ShowUpDown", "Show up/down", ctrl, action); ui.end_row();
                });
                {
                    let cur = ctrl.get_prop("CustomFormat").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "CustomFormat", &cur,
                        "Custom fmt:", "dd/MM/yyyy HH:mm", action);
                }
                {
                    let cur = ctrl.get_prop("MinDate").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "MinDate", &cur,
                        "Min date:", "YYYY-MM-DD", action);
                }
                {
                    let cur = ctrl.get_prop("MaxDate").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "MaxDate", &cur,
                        "Max date:", "YYYY-MM-DD", action);
                }
                color_row(ui, id, "BorderColor", ctrl, action);
                ui.add_space(4.0);
            }

            // ── NumericUpDown ─────────────────────────────────────────────────
            ControlType::NumericUpDown => {
                section_header(ui, "NumericUpDown");
                egui::Grid::new(format!("nud_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    ui.label("Value:");
                    let mut v = ctrl.get_prop("Value").map(|vv| vv.as_i64()).unwrap_or(0);
                    if ui.add(DragValue::new(&mut v).speed(1)).changed() {
                        action.set_props.push((id.to_owned(), "Value".into(), PropValue::Int(v)));
                    }
                    ui.end_row();
                    ui.label("Min:");
                    let mut mn = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0);
                    if ui.add(DragValue::new(&mut mn).speed(1)).changed() {
                        action.set_props.push((id.to_owned(), "Minimum".into(), PropValue::Int(mn)));
                    }
                    ui.end_row();
                    ui.label("Max:");
                    let mut mx = ctrl.get_prop("Maximum").map(|v| v.as_i64()).unwrap_or(100);
                    if ui.add(DragValue::new(&mut mx).speed(1)).changed() {
                        action.set_props.push((id.to_owned(), "Maximum".into(), PropValue::Int(mx)));
                    }
                    ui.end_row();
                    ui.label("Step:");
                    let mut st = ctrl.get_prop("Step").map(|v| v.as_i64()).unwrap_or(1);
                    if ui.add(DragValue::new(&mut st).speed(1).range(1..=1000)).changed() {
                        action.set_props.push((id.to_owned(), "Step".into(), PropValue::Int(st)));
                    }
                    ui.end_row();
                    ui.label("Decimals:");
                    let mut dp = ctrl.get_prop("DecimalPlaces").map(|v| v.as_i64()).unwrap_or(0);
                    if ui.add(DragValue::new(&mut dp).speed(1).range(0..=10)).changed() {
                        action.set_props.push((id.to_owned(), "DecimalPlaces".into(), PropValue::Int(dp)));
                    }
                    ui.end_row();
                    bool_row(ui, id, "ThousandsSep", "Thousands sep", ctrl, action); ui.end_row();
                    bool_row(ui, id, "ReadOnly",     "ReadOnly",      ctrl, action); ui.end_row();
                });
                color_row(ui, id, "BorderColor", ctrl, action);
                ui.add_space(4.0);
            }

            // ── TreeView ──────────────────────────────────────────────────────
            ControlType::TreeView => {
                section_header(ui, "TreeView");
                {
                    let cur = ctrl.get_prop("Items").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    let buf_key = format!("{id}-Items");
                    let wid = egui::Id::new(&buf_key);
                    let buf = self.text_bufs.entry(buf_key).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                    ui.label("Nodes (indent = child):");
                    let resp = ui.add(egui::TextEdit::multiline(buf)
                        .id(wid).desired_rows(5).desired_width(f32::INFINITY));
                    if resp.lost_focus() {
                        action.set_props.push((id.to_owned(), "Items".into(), PropValue::String(buf.clone())));
                    }
                }
                egui::Grid::new(format!("tv_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    bool_row(ui, id, "AllowEdit",    "Allow edit",   ctrl, action); ui.end_row();
                    bool_row(ui, id, "CheckBoxes",   "Checkboxes",   ctrl, action); ui.end_row();
                    bool_row(ui, id, "ShowLines",    "Show lines",   ctrl, action); ui.end_row();
                    bool_row(ui, id, "ShowRootLines","Root lines",   ctrl, action); ui.end_row();
                    bool_row(ui, id, "Sorted",       "Sorted",       ctrl, action); ui.end_row();
                    bool_row(ui, id, "HotTracking",  "Hot tracking", ctrl, action); ui.end_row();
                });
                color_row(ui, id, "LineColor",   ctrl, action);
                color_row(ui, id, "BorderColor", ctrl, action);
                ui.add_space(4.0);
            }

            // ── Splitter ──────────────────────────────────────────────────────
            ControlType::Splitter => {
                section_header(ui, "Splitter");
                egui::Grid::new(format!("sp_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    combo_row(ui, id, "Orientation", ctrl, action, &["Horizontal","Vertical"]); ui.end_row();
                    ui.label("MinSize:");
                    let mut ms = ctrl.get_prop("MinSize").map(|v| v.as_i64()).unwrap_or(25);
                    if ui.add(DragValue::new(&mut ms).speed(1).range(0..=500)).changed() {
                        action.set_props.push((id.to_owned(), "MinSize".into(), PropValue::Int(ms)));
                    }
                    ui.end_row();
                    ui.label("SplitPosition:");
                    let mut sp = ctrl.get_prop("SplitPosition").map(|v| v.as_i64()).unwrap_or(100);
                    if ui.add(DragValue::new(&mut sp).speed(1).range(0..=9999)).changed() {
                        action.set_props.push((id.to_owned(), "SplitPosition".into(), PropValue::Int(sp)));
                    }
                    ui.end_row();
                });
                color_row(ui, id, "BorderColor", ctrl, action);
                ui.add_space(4.0);
            }

            // ── Timer ─────────────────────────────────────────────────────────
            ControlType::Timer => {
                section_header(ui, "⏱ Timer");
                egui::Grid::new(format!("tmr_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    ui.label("Interval (ms):");
                    let mut iv = ctrl.get_prop("Interval").map(|v| v.as_i64()).unwrap_or(1000);
                    if ui.add(DragValue::new(&mut iv).speed(10).range(1..=3_600_000)).changed() {
                        action.set_props.push((id.to_owned(), "Interval".into(), PropValue::Int(iv)));
                    }
                    ui.end_row();
                    bool_row(ui, &id, "Enabled", "Enabled at start", ctrl, action); ui.end_row();
                });
                {
                    let cur = ctrl.get_prop("Paragraph").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, &id, "Paragraph", &cur,
                        "PERFORM:", "TICK-HANDLER", action);
                }
                ui.label(RichText::new("PERFORM is called every Interval ms. Non-visual at runtime.")
                    .color(Color32::GRAY).small().italics());
                ui.add_space(4.0);
            }

            // ── Shape ─────────────────────────────────────────────────────────
            ControlType::Shape => {
                section_header(ui, "Shape");
                egui::Grid::new(format!("shp_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    combo_row(ui, id, "ShapeType", ctrl, action,
                        &["Rectangle","Circle","RoundRect","Triangle"]); ui.end_row();
                    combo_row(ui, id, "FillStyle", ctrl, action,
                        &["Solid","None","Hatched"]); ui.end_row();
                    ui.label("LineThickness:");
                    let mut t = ctrl.get_prop("LineThickness").map(|v| v.as_i64()).unwrap_or(1);
                    if ui.add(DragValue::new(&mut t).speed(1).range(1..=32)).changed() {
                        action.set_props.push((id.to_owned(), "LineThickness".into(), PropValue::Int(t)));
                    }
                    ui.end_row();
                    combo_row(ui, id, "LineStyle", ctrl, action,
                        &["Solid","Dash","Dot","DashDot"]); ui.end_row();
                });
                color_row(ui, id, "FillColor", ctrl, action);
                color_row(ui, id, "LineColor", ctrl, action);
                ui.add_space(4.0);
            }

            // ── MenuBar / ToolBar / StatusBar ─────────────────────────────────
            ControlType::MenuBar | ControlType::ToolBar | ControlType::StatusBar => {
                section_header(ui, "Items");
                let cur = ctrl.get_prop("Items").map(|v| v.as_str().to_owned()).unwrap_or_default();
                let buf_key = format!("{id}-Items");
                let wid = egui::Id::new(&buf_key);
                let buf = self.text_bufs.entry(buf_key).or_insert(cur.clone());
                if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                ui.label("Items (one per line):");
                let resp = ui.add(egui::TextEdit::multiline(buf)
                    .id(wid).desired_rows(4).desired_width(f32::INFINITY));
                if resp.lost_focus() {
                    action.set_props.push((id.to_owned(), "Items".into(), PropValue::String(buf.clone())));
                }
                ui.add_space(4.0);
            }

            // ── Agent Object ──────────────────────────────────────────────────
            ControlType::AgentObject => {
                section_header(ui, "🤖 AI Agent — Network");
                egui::Grid::new(format!("agt_net_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    combo_row(ui, id, "AgentAPI", ctrl, action,
                        &["Ollama","LMStudio","OpenAI","Anthropic","Custom"]); ui.end_row();
                    ui.label("URL:");
                    let cur = ctrl.get_prop("AgentURL").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    let bk = format!("{id}-AgentURL");
                    let wid = egui::Id::new(&bk);
                    let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                    if ui.add(egui::TextEdit::singleline(buf).id(wid).hint_text("http://localhost:11434").desired_width(f32::INFINITY)).lost_focus() {
                        action.set_props.push((id.to_owned(), "AgentURL".into(), PropValue::String(buf.clone())));
                    }
                    ui.end_row();
                    let cur = ctrl.get_prop("AgentModel").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    let bk = format!("{id}-AgentModel");
                    let wid = egui::Id::new(&bk);
                    let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                    ui.label("Model:");
                    if ui.add(egui::TextEdit::singleline(buf).id(wid).hint_text("llama3.2").desired_width(f32::INFINITY)).lost_focus() {
                        action.set_props.push((id.to_owned(), "AgentModel".into(), PropValue::String(buf.clone())));
                    }
                    ui.end_row();
                    let cur = ctrl.get_prop("AgentEndpoint").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    let bk = format!("{id}-AgentEndpoint");
                    let wid = egui::Id::new(&bk);
                    let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                    ui.label("Endpoint:");
                    if ui.add(egui::TextEdit::singleline(buf).id(wid).hint_text("/api/chat (override)").desired_width(f32::INFINITY)).lost_focus() {
                        action.set_props.push((id.to_owned(), "AgentEndpoint".into(), PropValue::String(buf.clone())));
                    }
                    ui.end_row();
                    let cur = ctrl.get_prop("AgentAPIKey").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    let bk = format!("{id}-AgentAPIKey");
                    let wid = egui::Id::new(&bk);
                    let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                    ui.label("API Key:");
                    if ui.add(egui::TextEdit::singleline(buf).id(wid).password(true).desired_width(f32::INFINITY)).lost_focus() {
                        action.set_props.push((id.to_owned(), "AgentAPIKey".into(), PropValue::String(buf.clone())));
                    }
                    ui.end_row();
                });

                section_header(ui, "🤖 AI Agent — Behaviour");
                {
                    let cur = ctrl.get_prop("SystemPrompt").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    let bk = format!("{id}-SystemPrompt");
                    let wid = egui::Id::new(&bk);
                    let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                    ui.label("System prompt:");
                    let resp = ui.add(egui::TextEdit::multiline(buf).id(wid).desired_rows(3).desired_width(f32::INFINITY));
                    if resp.lost_focus() {
                        action.set_props.push((id.to_owned(), "SystemPrompt".into(), PropValue::String(buf.clone())));
                    }
                }
                egui::Grid::new(format!("agt_beh_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    ui.label("Temperature (0-100):");
                    let mut t = ctrl.get_prop("Temperature").map(|v| v.as_i64()).unwrap_or(70);
                    if ui.add(DragValue::new(&mut t).speed(1).range(0..=100).suffix("%")).changed() {
                        action.set_props.push((id.to_owned(), "Temperature".into(), PropValue::Int(t)));
                    }
                    ui.end_row();
                    ui.label("Max tokens:");
                    let mut mt = ctrl.get_prop("MaxTokens").map(|v| v.as_i64()).unwrap_or(1024);
                    if ui.add(DragValue::new(&mut mt).speed(10).range(1..=128000)).changed() {
                        action.set_props.push((id.to_owned(), "MaxTokens".into(), PropValue::Int(mt)));
                    }
                    ui.end_row();
                    ui.label("Timeout (s):");
                    let mut to = ctrl.get_prop("TimeoutSec").map(|v| v.as_i64()).unwrap_or(30);
                    if ui.add(DragValue::new(&mut to).speed(1).range(1..=300)).changed() {
                        action.set_props.push((id.to_owned(), "TimeoutSec".into(), PropValue::Int(to)));
                    }
                    ui.end_row();
                    bool_row(ui, id, "Stream", "Streaming mode", ctrl, action); ui.end_row();
                });

                section_header(ui, "🤖 AI Agent — COBOL Integration");
                {
                    let cur = ctrl.get_prop("TargetControls").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "TargetControls", &cur,
                        "Target controls:", "TXT-1,LBL-2 (comma-sep IDs)", action);
                }
                {
                    let cur = ctrl.get_prop("ResponseDataItem").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "ResponseDataItem", &cur,
                        "Response data item:", "WS-AGENT-RESPONSE", action);
                }
                {
                    let cur = ctrl.get_prop("ResponsePara").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "ResponsePara", &cur,
                        "On response PERFORM:", "AGENT-RESPONSE-HANDLER", action);
                }
                {
                    let cur = ctrl.get_prop("StreamChunkPara").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "StreamChunkPara", &cur,
                        "On stream chunk:", "AGENT-CHUNK-HANDLER", action);
                }
                {
                    let cur = ctrl.get_prop("ErrorPara").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "ErrorPara", &cur,
                        "On error PERFORM:", "AGENT-ERROR-HANDLER", action);
                }
                ui.label(RichText::new(
                    "COBOL: INVOKE agent-id 'Ask' USING WS-PROMPT RETURNING WS-RESPONSE")
                    .small().color(Color32::GRAY).italics());
                ui.add_space(4.0);
            }

            // ── Modal Window ──────────────────────────────────────────────────
            ControlType::ModalWindow => {
                section_header(ui, "⊞ Modal Window");
                egui::Grid::new(format!("mdl_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    ui.label("Form file (.cfrm):");
                    let cur = ctrl.get_prop("FormFile").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    let bk = format!("{id}-FormFile");
                    let wid = egui::Id::new(&bk);
                    let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                    let pick_key = format!("modalform:{id}");
                    ui.horizontal(|ui| {
                        if ui.button("📂").clicked() {
                            crate::file_dialog::open_file(&pick_key, "Form", &["cfrm"]);
                        }
                        if crate::file_dialog::is_open(&pick_key) { ui.ctx().request_repaint(); }
                        if let Some(Some(p)) = crate::file_dialog::take(&pick_key) {
                            let ps = p.to_string_lossy().to_string();
                            *buf = ps.clone();
                            action.set_props.push((id.to_owned(), "FormFile".into(), PropValue::String(ps)));
                        }
                        if ui.add(egui::TextEdit::singleline(buf).id(wid).desired_width(f32::INFINITY)).lost_focus() {
                            action.set_props.push((id.to_owned(), "FormFile".into(), PropValue::String(buf.clone())));
                        }
                    });
                    ui.end_row();
                    let cur = ctrl.get_prop("ProgramName").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "ProgramName", &cur,
                        "Program-ID:", "DIALOG-PROG", action);
                    let cur = ctrl.get_prop("Title").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Dialog".into());
                    ui.end_row();
                    let bk = format!("{id}-Title");
                    let wid = egui::Id::new(&bk);
                    let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                    ui.label("Title:");
                    if ui.add(egui::TextEdit::singleline(buf).id(wid).desired_width(f32::INFINITY)).lost_focus() {
                        action.set_props.push((id.to_owned(), "Title".into(), PropValue::String(buf.clone())));
                    }
                    ui.end_row();
                    ui.label("Width:");
                    let mut mw = ctrl.get_prop("Width").map(|v| v.as_i64()).unwrap_or(400);
                    if ui.add(DragValue::new(&mut mw).speed(1).range(100..=9999)).changed() {
                        action.set_props.push((id.to_owned(), "Width".into(), PropValue::Int(mw)));
                    }
                    ui.end_row();
                    ui.label("Height:");
                    let mut mh = ctrl.get_prop("Height").map(|v| v.as_i64()).unwrap_or(300);
                    if ui.add(DragValue::new(&mut mh).speed(1).range(60..=9999)).changed() {
                        action.set_props.push((id.to_owned(), "Height".into(), PropValue::Int(mh)));
                    }
                    ui.end_row();
                    bool_row(ui, id, "Resizable", "Resizable", ctrl, action); ui.end_row();
                    combo_row(ui, id, "StartPosition", ctrl, action,
                        &["CenterParent","CenterScreen","Manual"]); ui.end_row();
                    combo_row(ui, id, "ModalResult", ctrl, action,
                        &["None","OK","Cancel","Yes","No"]); ui.end_row();
                });
                {
                    let cur = ctrl.get_prop("SharedDataItems").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "SharedDataItems", &cur,
                        "Shared data items:", "WS-A,WS-B (comma-sep)", action);
                }
                ui.label(RichText::new("Shared items are passed by reference to the modal COBOL program.")
                    .small().color(Color32::GRAY).italics());
                section_header(ui, "⊞ Modal COBOL Events");
                for (key, hint) in [
                    ("OpenPara",      "MODAL-OPEN-PARA"),
                    ("ClosedPara",    "MODAL-CLOSED-PARA"),
                    ("ConfirmedPara", "MODAL-OK-PARA"),
                    ("CancelledPara", "MODAL-CANCEL-PARA"),
                ] {
                    let cur = ctrl.get_prop(key).map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, key, &cur,
                        &format!("{key}:"), hint, action);
                }
                ui.add_space(4.0);
            }

            // ── REST Client ───────────────────────────────────────────────────
            ControlType::RestClient => {
                section_header(ui, "🌐 REST Client — Connection");
                egui::Grid::new(format!("rst_con_{id}")).num_columns(2).spacing([4.0,3.0]).show(ui, |ui| {
                    let cur = ctrl.get_prop("BaseURL").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    let bk = format!("{id}-BaseURL");
                    let wid = egui::Id::new(&bk);
                    let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                    ui.label("Base URL:");
                    if ui.add(egui::TextEdit::singleline(buf).id(wid)
                        .hint_text("https://api.example.com").desired_width(f32::INFINITY)).lost_focus() {
                        action.set_props.push((id.to_owned(), "BaseURL".into(), PropValue::String(buf.clone())));
                    }
                    ui.end_row();
                    combo_row(ui, id, "DefaultMethod", ctrl, action,
                        &["GET","POST","PUT","PATCH","DELETE","HEAD","OPTIONS"]); ui.end_row();
                    combo_row(ui, id, "AuthType", ctrl, action,
                        &["None","Bearer","Basic","APIKey"]); ui.end_row();
                    let cur = ctrl.get_prop("AuthToken").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    let bk = format!("{id}-AuthToken");
                    let wid = egui::Id::new(&bk);
                    let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                    ui.label("Auth token:");
                    if ui.add(egui::TextEdit::singleline(buf).id(wid).password(true).desired_width(f32::INFINITY)).lost_focus() {
                        action.set_props.push((id.to_owned(), "AuthToken".into(), PropValue::String(buf.clone())));
                    }
                    ui.end_row();
                    ui.label("Timeout (s):");
                    let mut to = ctrl.get_prop("TimeoutSec").map(|v| v.as_i64()).unwrap_or(30);
                    if ui.add(DragValue::new(&mut to).speed(1).range(1..=300)).changed() {
                        action.set_props.push((id.to_owned(), "TimeoutSec".into(), PropValue::Int(to)));
                    }
                    ui.end_row();
                    bool_row(ui, id, "FollowRedirects", "Follow redirects", ctrl, action); ui.end_row();
                    bool_row(ui, id, "VerifyTLS",       "Verify TLS cert",  ctrl, action); ui.end_row();
                });
                {
                    let cur = ctrl.get_prop("DefaultHeaders").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    let bk = format!("{id}-DefaultHeaders");
                    let wid = egui::Id::new(&bk);
                    let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
                    ui.label("Default headers (Key: Value, one per line):");
                    let resp = ui.add(egui::TextEdit::multiline(buf).id(wid).desired_rows(3).desired_width(f32::INFINITY));
                    if resp.lost_focus() {
                        action.set_props.push((id.to_owned(), "DefaultHeaders".into(), PropValue::String(buf.clone())));
                    }
                }

                section_header(ui, "🌐 REST Client — COBOL Integration");
                {
                    let cur = ctrl.get_prop("RequestDataItem").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "RequestDataItem", &cur,
                        "Request body item:", "WS-REQUEST-JSON", action);
                }
                {
                    let cur = ctrl.get_prop("ResponseDataItem").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "ResponseDataItem", &cur,
                        "Response item:", "WS-RESPONSE-JSON", action);
                }
                {
                    let cur = ctrl.get_prop("StatusDataItem").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "StatusDataItem", &cur,
                        "HTTP status item:", "WS-HTTP-STATUS", action);
                }
                {
                    let cur = ctrl.get_prop("ResponsePara").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "ResponsePara", &cur,
                        "On response PERFORM:", "REST-RESPONSE-HANDLER", action);
                }
                {
                    let cur = ctrl.get_prop("ErrorPara").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "ErrorPara", &cur,
                        "On error PERFORM:", "REST-ERROR-HANDLER", action);
                }
                ui.label(RichText::new(
                    "COBOL: SET WS-RESP TO rst-id::call('GET', 'https://...')\n\
                     Or:   INVOKE rst-id 'call' USING BY VALUE 'GET' BY VALUE WS-URL\n\
                           RETURNING WS-RESPONSE-JSON")
                    .small().color(Color32::GRAY).italics());
                ui.add_space(4.0);
            }

            // ── SQL Database ──────────────────────────────────────────────────
            ControlType::SqlDatabase => {
                section_header(ui, "🗄 SQL Database — Connection");
                egui::Grid::new(format!("sql_conn_{id}")).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                    ui.label("Driver:");
                    {
                        let cur = ctrl.get_prop("Driver").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "sqlite".into());
                        egui::ComboBox::from_id_salt(format!("sql_driver_{id}"))
                            .selected_text(&cur).width(120.0)
                            .show_ui(ui, |ui| {
                                for opt in &["sqlite", "postgres", "mysql", "mssql"] {
                                    if ui.selectable_label(&cur == opt, *opt).clicked() {
                                        action.set_props.push((id.to_owned(), "Driver".into(), PropValue::String(opt.to_string())));
                                    }
                                }
                            });
                    }
                    ui.end_row();

                    let cur_cs = ctrl.get_prop("ConnectionString").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "ConnectionString", &cur_cs,
                        "Connection string:", "sqlite::memory:", action);
                    ui.end_row();

                    ui.label("Auto-connect:");
                    {
                        let mut v = ctrl.get_prop("AutoConnect").map(|p| p.as_bool()).unwrap_or(false);
                        if ui.checkbox(&mut v, "").changed() {
                            action.set_props.push((id.to_owned(), "AutoConnect".into(), PropValue::Bool(v)));
                        }
                    }
                    ui.end_row();

                    ui.label("Max connections:");
                    {
                        let mut n = ctrl.get_prop("MaxConnections").map(|v| v.as_i64()).unwrap_or(5);
                        if ui.add(DragValue::new(&mut n).range(1..=100)).changed() {
                            action.set_props.push((id.to_owned(), "MaxConnections".into(), PropValue::Int(n)));
                        }
                    }
                    ui.end_row();
                });

                section_header(ui, "🗄 SQL Database — COBOL Data Items");
                egui::Grid::new(format!("sql_items_{id}")).num_columns(1).spacing([8.0, 4.0]).show(ui, |ui| {
                    let cur = ctrl.get_prop("ConnDataItem").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "ConnDataItem", &cur,
                        "Connection item:", "conn1", action);
                    ui.end_row();

                    let cur = ctrl.get_prop("ResultSetDataItem").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "ResultSetDataItem", &cur,
                        "Result set item:", "resultset1", action);
                    ui.end_row();

                    let cur = ctrl.get_prop("ConnectPara").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "ConnectPara", &cur,
                        "After connect:", "DB-CONNECTED", action);
                    ui.end_row();

                    let cur = ctrl.get_prop("QueryCompletePara").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "QueryCompletePara", &cur,
                        "After query:", "DB-QUERY-DONE", action);
                    ui.end_row();

                    let cur = ctrl.get_prop("ErrorPara").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "ErrorPara", &cur,
                        "On SQL error:", "DB-ERROR-HANDLER", action);
                    ui.end_row();
                });

                ui.add_space(4.0);
                ui.label(RichText::new(
                    "Usage:\n  SET conn1 TO sql1::openConnection()\n\
                     SET resultset1 TO conn1::exec('SELECT ...')\n\
                     PERFORM UNTIL resultset1::Next() = sql1::None\n\
                        COMPUTE total = total + resultset1::fetch()::col\n\
                     END-PERFORM")
                    .small().color(Color32::GRAY).italics());
                ui.add_space(4.0);
            }

            // ── Charts ───────────────────────────────────────────────────────
            ControlType::BarChart
            | ControlType::LineChart
            | ControlType::PieChart
            | ControlType::AreaChart
            | ControlType::ScatterChart
            | ControlType::DonutChart => {
                let type_label = match ctrl.control_type {
                    ControlType::BarChart     => "📊 Bar Chart",
                    ControlType::LineChart    => "📈 Line Chart",
                    ControlType::PieChart     => "🥧 Pie Chart",
                    ControlType::AreaChart    => "📉 Area Chart",
                    ControlType::ScatterChart => "✦ Scatter Chart",
                    ControlType::DonutChart   => "🍩 Donut Chart",
                    _                         => "📊 Chart",
                };
                section_header(ui, type_label);

                // ── Visual ────────────────────────────────────────────────────
                egui::Grid::new(format!("chart_vis_{id}")).num_columns(2).spacing([8.0,4.0]).show(ui, |ui| {
                    // Title
                    let cur_title = ctrl.get_prop("Title").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "Title", &cur_title,
                        "Title:", "Sales by Region", action);
                    bool_row(ui, id, "ShowLegend",   "Show legend",    ctrl, action); ui.end_row();
                    bool_row(ui, id, "ShowGridLines", "Show grid lines", ctrl, action); ui.end_row();
                    bool_row(ui, id, "ShowTooltips", "Show tooltips",  ctrl, action); ui.end_row();
                    bool_row(ui, id, "AnimateOnLoad","Animate on load",ctrl, action); ui.end_row();
                    // Axis labels (not for pie/donut)
                    if !matches!(ctrl.control_type, ControlType::PieChart | ControlType::DonutChart) {
                        let cx = ctrl.get_prop("XAxisLabel").map(|v| v.as_str().to_owned()).unwrap_or_default();
                        text_row_hint(ui, &mut self.text_bufs, id, "XAxisLabel", &cx, "X-axis label:", "Month", action);
                        let cy = ctrl.get_prop("YAxisLabel").map(|v| v.as_str().to_owned()).unwrap_or_default();
                        text_row_hint(ui, &mut self.text_bufs, id, "YAxisLabel", &cy, "Y-axis label:", "Amount", action);
                    }
                });

                // ── Data Binding ──────────────────────────────────────────────
                section_header(ui, "🔗 Data Binding — COBOL Table");
                egui::Grid::new(format!("chart_data_{id}")).num_columns(2).spacing([8.0,4.0]).show(ui, |ui| {
                    // DataSource: COBOL working-storage table item name
                    let ds = ctrl.get_prop("DataSource").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "DataSource", &ds,
                        "Table item:", "WS-SALES-TABLE", action);
                    // Row count
                    let dc = ctrl.get_prop("DataCount").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "DataCount", &dc,
                        "Row count item:", "WS-SALES-COUNT", action);
                    // Field for X labels
                    let lf = ctrl.get_prop("LabelField").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "LabelField", &lf,
                        "Label field:", "SALES-MONTH", action);
                    // Y series fields (comma-separated)
                    let vf = ctrl.get_prop("ValueFields").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "ValueFields", &vf,
                        "Value field(s):", "SALES-AMOUNT,SALES-BUDGET", action);
                    // Series display labels
                    let sl = ctrl.get_prop("SeriesLabels").map(|v| v.as_str().to_owned()).unwrap_or_default();
                    text_row_hint(ui, &mut self.text_bufs, id, "SeriesLabels", &sl,
                        "Series labels:", "Actual,Budget", action);
                });

                // ── Type-specific ─────────────────────────────────────────────
                if matches!(ctrl.control_type, ControlType::BarChart) {
                    section_header(ui, "Bar Chart Options");
                    egui::Grid::new(format!("chart_bar_{id}")).num_columns(2).spacing([8.0,4.0]).show(ui, |ui| {
                        bool_row(ui, id, "Horizontal", "Horizontal bars", ctrl, action); ui.end_row();
                        bool_row(ui, id, "Stacked",    "Stacked",         ctrl, action); ui.end_row();
                        ui.label("Corner radius:"); {
                            let mut v = ctrl.get_prop("BarCornerRadius").map(|v| v.as_i64()).unwrap_or(3);
                            if ui.add(DragValue::new(&mut v).speed(1).range(0..=20)).changed() {
                                action.set_props.push((id.to_owned(),"BarCornerRadius".into(),PropValue::Int(v)));
                            }
                        } ui.end_row();
                    });
                }
                if matches!(ctrl.control_type, ControlType::LineChart | ControlType::AreaChart) {
                    section_header(ui, "Line / Area Options");
                    egui::Grid::new(format!("chart_line_{id}")).num_columns(2).spacing([8.0,4.0]).show(ui, |ui| {
                        bool_row(ui, id, "Smooth",     "Smooth curve", ctrl, action); ui.end_row();
                        bool_row(ui, id, "ShowPoints", "Show points",  ctrl, action); ui.end_row();
                        ui.label("Point radius:"); {
                            let mut v = ctrl.get_prop("PointRadius").map(|v| v.as_i64()).unwrap_or(4);
                            if ui.add(DragValue::new(&mut v).speed(1).range(0..=20)).changed() {
                                action.set_props.push((id.to_owned(),"PointRadius".into(),PropValue::Int(v)));
                            }
                        } ui.end_row();
                        if matches!(ctrl.control_type, ControlType::AreaChart) {
                            ui.label("Fill alpha (%):"); {
                                let mut v = ctrl.get_prop("FillAlpha").map(|v| v.as_i64()).unwrap_or(40);
                                if ui.add(DragValue::new(&mut v).speed(1).range(0..=100).suffix("%")).changed() {
                                    action.set_props.push((id.to_owned(),"FillAlpha".into(),PropValue::Int(v)));
                                }
                            } ui.end_row();
                            bool_row(ui, id, "Stacked", "Stacked areas", ctrl, action); ui.end_row();
                        }
                    });
                }
                if matches!(ctrl.control_type, ControlType::PieChart | ControlType::DonutChart) {
                    section_header(ui, "Pie / Donut Options");
                    egui::Grid::new(format!("chart_pie_{id}")).num_columns(2).spacing([8.0,4.0]).show(ui, |ui| {
                        bool_row(ui, id, "ShowLabels", "Show labels", ctrl, action); ui.end_row();
                        ui.label("Label format:");
                        let cur_lf = ctrl.get_prop("LabelFormat").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "percent".into());
                        egui::ComboBox::from_id_salt(format!("chart_lf_{id}"))
                            .selected_text(&cur_lf).width(90.0)
                            .show_ui(ui, |ui| {
                                for opt in &["percent","value","label"] {
                                    if ui.selectable_label(&cur_lf == opt, *opt).clicked() {
                                        action.set_props.push((id.to_owned(),"LabelFormat".into(),PropValue::String(opt.to_string())));
                                    }
                                }
                            });
                        ui.end_row();
                        if matches!(ctrl.control_type, ControlType::DonutChart) {
                            ui.label("Inner radius (%):"); {
                                let mut v = ctrl.get_prop("InnerRadius").map(|v| v.as_i64()).unwrap_or(40);
                                if ui.add(DragValue::new(&mut v).speed(1).range(10..=80).suffix("%")).changed() {
                                    action.set_props.push((id.to_owned(),"InnerRadius".into(),PropValue::Int(v)));
                                }
                            } ui.end_row();
                        }
                    });
                }
                if matches!(ctrl.control_type, ControlType::ScatterChart) {
                    section_header(ui, "Scatter / Bubble Options");
                    egui::Grid::new(format!("chart_sct_{id}")).num_columns(2).spacing([8.0,4.0]).show(ui, |ui| {
                        let bb = ctrl.get_prop("BubbleField").map(|v| v.as_str().to_owned()).unwrap_or_default();
                        text_row_hint(ui, &mut self.text_bufs, id, "BubbleField", &bb,
                            "Bubble size field:", "SALES-VOLUME", action);
                        ui.label("Max bubble (px):"); {
                            let mut v = ctrl.get_prop("BubbleScale").map(|v| v.as_i64()).unwrap_or(20);
                            if ui.add(DragValue::new(&mut v).speed(1).range(4..=60)).changed() {
                                action.set_props.push((id.to_owned(),"BubbleScale".into(),PropValue::Int(v)));
                            }
                        } ui.end_row();
                    });
                }

                ui.add_space(4.0);
                ui.label(RichText::new(
                    "Table binding:\n  INVOKE CHART1 SET-TABLE\n    USING WS-SALES-TABLE WS-SALES-COUNT\n\
                     \nDirect point:\n  INVOKE CHART1 ADD-POINT\n    USING 'January' WS-VALUE\n\
                     \n  INVOKE CHART1 CLEAR\n  INVOKE CHART1 REFRESH")
                    .small().color(Color32::GRAY).italics());
                ui.add_space(4.0);
            }

            _ => {}
        }
    }

    // ── Form inspector ────────────────────────────────────────────────────────

    fn show_form(&mut self, ui: &mut Ui, form: &Form, action: &mut InspectorAction, tr: &Tr) {
        section_card(ui, "form-sec-props", tr.sec_form_props, true, |ui| {
        // ── Identity (read-only) ──────────────────────────────────────────────
        egui::Grid::new("form_identity").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label(tr.lbl_name);  ui.label(&form.name);  ui.end_row();
            ui.label(tr.lbl_size);  ui.label(format!("{} × {}", form.width, form.height)); ui.end_row();
        });
        });

        // ── Target device ─────────────────────────────────────────────────────
        section_card(ui, "form-sec-target", tr.sec_target, true, |ui| {
        egui::Grid::new("form_target").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            use super::designer::TARGET_PRESETS;

            ui.label(tr.lbl_target_label);
            {
                let cur = form.target.as_str();
                // Show current selection + dimensions hint
                let display = if cur == "Custom" {
                    format!("Custom ({}×{})", form.width, form.height)
                } else {
                    // Find preset dims
                    TARGET_PRESETS.iter()
                        .find(|(l, ..)| *l == cur)
                        .map(|(l, w, h)| format!("{l}  ({w}×{h})"))
                        .unwrap_or_else(|| cur.to_owned())
                };
                egui::ComboBox::from_id_salt("form_target_combo")
                    .selected_text(&display)
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        // Group headers — rendered as disabled labels between items
                        let groups: &[(&str, &[&str])] = &[
                            ("— Custom —", &["Custom"]),
                            ("— Apple iPhone —", &[
                                "iPhone 16 Pro Max",
                                "iPhone 16 / 15 Pro",
                                "iPhone 15 / 14",
                                "iPhone SE (3rd gen)",
                            ]),
                            ("— Apple iPad —", &[
                                "iPad Pro 13\" (M4)",
                                "iPad Pro 11\" (M4)",
                                "iPad Air 13\" (M2)",
                                "iPad (10th gen)",
                                "iPad mini (7th gen)",
                            ]),
                            ("— Apple Watch —", &[
                                "Apple Watch Ultra 2 (49mm)",
                                "Apple Watch Series 10 (46mm)",
                                "Apple Watch Series 10 (42mm)",
                            ]),
                            ("— Android Phone —", &[
                                "Samsung Galaxy S24 Ultra",
                                "Samsung Galaxy S24",
                                "Google Pixel 9 Pro",
                                "Android Phone (generic 1080p)",
                            ]),
                            ("— Android Tablet —", &[
                                "Samsung Galaxy Tab S9 Ultra",
                                "Samsung Galaxy Tab S9",
                                "Lenovo Tab P12",
                                "Android Tablet (generic)",
                            ]),
                            ("— Android SmartWatch —", &[
                                "Samsung Galaxy Watch 7 (44mm)",
                                "Samsung Galaxy Watch 7 (40mm)",
                                "Wear OS (generic round)",
                                "Wear OS (generic square)",
                            ]),
                        ];
                        for (header, items) in groups {
                            ui.add_enabled(false,
                                egui::Label::new(RichText::new(*header).small().color(Color32::from_rgb(140, 160, 200))));
                            for &item in *items {
                                let dims = TARGET_PRESETS.iter()
                                    .find(|(l, ..)| *l == item)
                                    .map(|(_, w, h)| format!("  {w}×{h}"))
                                    .unwrap_or_default();
                                let label = format!("{item}{dims}");
                                if ui.selectable_label(cur == item, &label).clicked() {
                                    action.form_props.push(("Target".into(), item.to_owned()));
                                }
                            }
                        }
                    });
            }
            ui.end_row();

            // Orientation hint (Portrait / Landscape swap button)
            ui.label(tr.lbl_orientation);
            ui.horizontal(|ui| {
                let portrait  = form.width <= form.height;
                if ui.selectable_label( portrait, tr.lbl_portrait).clicked()  && !portrait {
                    action.form_props.push(("Width".into(),  form.height.to_string()));
                    action.form_props.push(("Height".into(), form.width.to_string()));
                }
                if ui.selectable_label(!portrait, tr.lbl_landscape).clicked() && portrait {
                    action.form_props.push(("Width".into(),  form.height.to_string()));
                    action.form_props.push(("Height".into(), form.width.to_string()));
                }
            });
            ui.end_row();
        });
        });

        // ── Appearance ────────────────────────────────────────────────────────
        section_card(ui, "form-sec-appearance", tr.sec_appearance, true, |ui| {
        egui::Grid::new("form_appearance").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            // Title
            ui.label(tr.lbl_title);
            {
                const K: &str = "form-Title";
                let wid = egui::Id::new(K);
                let buf = self.form_bufs.entry(K.into()).or_insert(form.title.clone());
                if *buf != form.title && !ui.memory(|m| m.has_focus(wid)) {
                    *buf = form.title.clone();
                }
                if ui.add(egui::TextEdit::singleline(buf).id(wid).desired_width(f32::INFINITY)).lost_focus() {
                    action.form_props.push(("Title".into(), buf.clone()));
                }
            }
            ui.end_row();

            // BackColor
            ui.label(tr.lbl_back_color);
            {
                let hex = format!("#{}", form.background_color.trim_start_matches('#'));
                let mut color = hex_to_color32(&hex);
                ui.horizontal(|ui| {
                    if color_edit_button_closing(ui, &mut color).changed() {
                        action.form_props.push(("BackColor".into(), color32_to_hex(color)));
                    }
                    ui.label(RichText::new(color32_to_hex(color)).monospace().small().color(Color32::GRAY));
                });
            }
            ui.end_row();

            // Transparency
            ui.label(tr.lbl_transparency);
            {
                let mut trans = form.transparency as i64;
                if ui.add(DragValue::new(&mut trans).speed(1).range(0..=100).suffix("%")).changed() {
                    action.form_props.push(("Transparency".into(), trans.to_string()));
                }
            }
            ui.end_row();

            // Grid dot spacing
            ui.label(tr.lbl_grid_size);
            {
                let mut gs = form.grid_size as i64;
                if ui.add(DragValue::new(&mut gs).speed(1).range(4..=64).suffix("px")).changed() {
                    action.form_props.push(("GridSize".into(), gs.to_string()));
                }
            }
            ui.end_row();

            // Snap-to-grid toggle
            ui.label(tr.lbl_snap_to_grid);
            {
                let mut snapping = form.snap_to_grid;
                if ui.checkbox(&mut snapping, "").changed() {
                    action.form_props.push(("SnapToGrid".into(), if snapping { "true" } else { "false" }.to_string()));
                }
            }
            ui.end_row();
        });
        });

        // ── Background Image ──────────────────────────────────────────────────
        section_card(ui, "form-sec-bgimage", tr.sec_bg_image, true, |ui| {
        egui::Grid::new("form_bgimage").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            // Image path + browse button
            ui.label(tr.lbl_image_path);
            {
                const K: &str = "form-BgImage";
                let wid = egui::Id::new(K);
                let buf = self.form_bufs.entry(K.into()).or_insert(form.background_image.clone());
                if *buf != form.background_image && !ui.memory(|m| m.has_focus(wid)) {
                    *buf = form.background_image.clone();
                }
                ui.horizontal(|ui| {
                    const PICK_K: &str = "form-BgImage-pick";
                    if ui.button("📂").on_hover_text("Browse for image…").clicked() {
                        crate::file_dialog::open_file(
                            PICK_K, "Images",
                            &["png","jpg","jpeg","bmp","gif","ico","webp","svg"]);
                    }
                    if crate::file_dialog::is_open(PICK_K) { ui.ctx().request_repaint(); }
                    if let Some(Some(p)) = crate::file_dialog::take(PICK_K) {
                        let path_str = p.to_string_lossy().to_string();
                        *buf = path_str.clone();
                        action.form_props.push(("BackgroundImage".into(), path_str));
                    }
                    if ui.add(egui::TextEdit::singleline(buf).id(wid)
                        .hint_text("/path/to/image.png")
                        .desired_width(f32::INFINITY)).lost_focus()
                    {
                        action.form_props.push(("BackgroundImage".into(), buf.clone()));
                    }
                });
            }
            ui.end_row();

            // Sizing mode
            ui.label(tr.lbl_img_mode);
            {
                let cur_mode = form.bg_image_mode.as_str();
                egui::ComboBox::from_id_salt("form_bgimage_mode")
                    .selected_text(cur_mode).width(130.0)
                    .show_ui(ui, |ui| {
                        for &opt in BgImageMode::all() {
                            if ui.selectable_label(cur_mode == opt, opt).clicked() {
                                action.form_props.push(("BgImageMode".into(), opt.to_owned()));
                            }
                        }
                    });
            }
            ui.end_row();
        });
        ui.label(RichText::new(
            "Stretch = fill exactly  •  Fill = crop to fill  •  Fit = letterbox\n\
             Center = original size  •  Tile = repeat"
        ).small().color(Color32::GRAY).italics());
        });

        // ── Size ──────────────────────────────────────────────────────────────
        section_card(ui, "form-sec-size", tr.sec_size, true, |ui| {
        egui::Grid::new("form_size").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label(tr.lbl_width);
            {
                const K: &str = "form-Width";
                let wid = egui::Id::new(K);
                let buf = self.form_bufs.entry(K.into()).or_insert(form.width.to_string());
                if !ui.memory(|m| m.has_focus(wid)) { *buf = form.width.to_string(); }
                if ui.add(egui::TextEdit::singleline(buf).id(wid).desired_width(70.0)).lost_focus() {
                    action.form_props.push(("Width".into(), buf.clone()));
                }
            }
            ui.end_row();

            ui.label(tr.lbl_height);
            {
                const K: &str = "form-Height";
                let wid = egui::Id::new(K);
                let buf = self.form_bufs.entry(K.into()).or_insert(form.height.to_string());
                if !ui.memory(|m| m.has_focus(wid)) { *buf = form.height.to_string(); }
                if ui.add(egui::TextEdit::singleline(buf).id(wid).desired_width(70.0)).lost_focus() {
                    action.form_props.push(("Height".into(), buf.clone()));
                }
            }
            ui.end_row();
        });
        });

        // ── Form-level Events ─────────────────────────────────────────────────
        section_card(ui, "form-sec-events", tr.sec_form_events, true, |ui| {
        ui.label(RichText::new(tr.hint_click_event)
            .small().color(Color32::GRAY).italics());
        ui.add_space(4.0);

        for ev_name in &["OnLoad", "OnClose"] {
            let binding  = form.form_events.iter().find(|e| e.event == *ev_name);
            let has_code = binding.map(|e| e.has_code()).unwrap_or(false);
            let lines    = binding.map(|e| e.code_line_count()).unwrap_or(0);

            let row_resp = ui.horizontal(|ui| {
                let dot_color = if has_code {
                    Color32::from_rgb(100, 220, 100)
                } else {
                    Color32::from_rgb(120, 120, 120)
                };
                ui.label(RichText::new(if has_code { "●" } else { "○" }).color(dot_color));
                let lbl = ui.add(
                    egui::Label::new(
                        RichText::new(format!("⚙ {ev_name}"))
                            .color(Color32::from_rgb(200, 200, 100))
                    ).sense(egui::Sense::click())
                ).on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(tr.hint_dblclick_event);
                if has_code {
                    ui.label(RichText::new(format!("({lines} {})", tr.hint_lines))
                        .small().color(Color32::GRAY));
                } else {
                    ui.label(RichText::new(tr.hint_click_to_add)
                        .small().color(Color32::from_rgb(100, 100, 100)).italics());
                }
                (lbl.clicked(), lbl.double_clicked())
            });

            let (clicked, double_clicked) = row_resp.inner;
            // ctrl_id = "" signals form-level event to the designer.
            if double_clicked {
                action.open_event_in_code = Some((String::new(), ev_name.to_string()));
            } else if clicked {
                action.open_event_editor = Some((String::new(), ev_name.to_string()));
            }
        }
        });

        ui.add_space(8.0);
        ui.label(
            RichText::new(tr.hint_click_control)
                .italics().color(Color32::GRAY),
        );
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn section_header(ui: &mut Ui, title: &str) {
    let theme = crate::theme::active();
    // Same dark-blue card-style header bar as `section_card`, so the widget
    // inspector's sections look consistent with the form inspector's cards.
    let fill = if theme.dark {
        Color32::from_rgba_unmultiplied(10, 11, 14, 150)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 150)
    };
    ui.add_space(8.0);
    egui::Frame::none()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, theme.panel_border()))
        .rounding(egui::Rounding::same(8.0))
        .inner_margin(egui::Margin::symmetric(10.0, 6.0))
        .outer_margin(egui::Margin { left: 0.0, right: 0.0, top: 0.0, bottom: 4.0 })
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(3.5, 17.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, egui::Rounding::same(2.0), theme.accent);
                ui.add_space(7.0);
                ui.label(RichText::new(title).size(16.0).strong().color(theme.accent));
            });
        });
    ui.add_space(2.0);
}

/// A collapsible **section card**: a rounded, subtly-bordered, padded container
/// with a blue ▸/▾ header (the reference's property-section style). `body` runs
/// inside the card when expanded.
fn section_card(
    ui: &mut Ui,
    id_salt: &str,
    title: &str,
    default_open: bool,
    body: impl FnOnce(&mut Ui),
) {
    let theme = crate::theme::active();
    // A translucent **dark-blue** card (not a semi-white lift): it darkens the
    // backdrop into a "dark glass" panel while staying partly see-through.
    let card_fill = if theme.dark {
        Color32::from_rgba_unmultiplied(10, 11, 14, 150)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 150)
    };
    egui::Frame::none()
        .fill(card_fill)
        .stroke(egui::Stroke::new(1.0, theme.panel_border()))
        .rounding(egui::Rounding::same(8.0))
        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
        .outer_margin(egui::Margin { left: 0.0, right: 0.0, top: 0.0, bottom: 8.0 })
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let id = ui.make_persistent_id(id_salt);
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open)
                .show_header(ui, |ui| {
                    ui.label(RichText::new(title).size(16.0).strong().color(theme.accent));
                })
                .body_unindented(|ui| {
                    ui.add_space(4.0);
                    body(ui);
                });
        });
}

fn color_row(ui: &mut Ui, id: &str, key: &str, ctrl: &Control, action: &mut InspectorAction) {
    let hex = ctrl.get_prop(key).map(|v| v.as_str().to_owned())
        .unwrap_or_else(|| "#F0F0F0".to_owned());
    let mut color = hex_to_color32(&hex);
    ui.horizontal(|ui| {
        ui.label(key);
        if color_edit_button_closing(ui, &mut color).changed() {
            let new_hex = color32_to_hex(color);
            action.set_props.push((id.to_owned(), key.to_owned(), PropValue::String(new_hex)));
        }
        ui.label(RichText::new(color32_to_hex(color)).monospace().small().color(Color32::GRAY));
    });
}

fn text_row_hint(
    ui: &mut Ui,
    bufs: &mut std::collections::HashMap<String, String>,
    ctrl_id: &str,
    prop_key: &str,
    cur: &str,
    label: &str,
    hint: &str,
    action: &mut InspectorAction,
) {
    let buf_key = format!("{ctrl_id}-{prop_key}");
    let widget_id = egui::Id::new(&buf_key);
    let buf = bufs.entry(buf_key).or_insert_with(|| cur.to_owned());
    if *buf != cur && !ui.memory(|m| m.has_focus(widget_id)) {
        *buf = cur.to_owned();
    }
    ui.horizontal(|ui| {
        ui.label(label);
        let resp = ui.add(
            egui::TextEdit::singleline(buf)
                .id(widget_id)
                .hint_text(hint)
                .desired_width(f32::INFINITY),
        );
        if resp.lost_focus() {
            action.set_props.push((ctrl_id.to_owned(), prop_key.to_owned(), PropValue::String(buf.clone())));
        }
    });
}

/// Bool property — grid cell style (label in left col, checkbox in right).
fn bool_row(ui: &mut Ui, ctrl_id: &str, key: &str, label: &str, ctrl: &Control, action: &mut InspectorAction) {
    ui.label(label);
    let mut v = ctrl.get_prop(key).map(|p| p.as_bool()).unwrap_or(false);
    if ui.checkbox(&mut v, "").changed() {
        action.set_props.push((ctrl_id.to_owned(), key.to_owned(), PropValue::Bool(v)));
    }
}

/// Bool property — inline horizontal style.
fn bool_row_inline(ui: &mut Ui, ctrl_id: &str, key: &str, label: &str, ctrl: &Control, action: &mut InspectorAction) {
    let mut v = ctrl.get_prop(key).map(|p| p.as_bool()).unwrap_or(false);
    if ui.checkbox(&mut v, label).changed() {
        action.set_props.push((ctrl_id.to_owned(), key.to_owned(), PropValue::Bool(v)));
    }
}

/// Combo row — grid cell style.
fn combo_row(ui: &mut Ui, ctrl_id: &str, key: &str, ctrl: &Control, action: &mut InspectorAction, opts: &[&str]) {
    let cur = ctrl.get_prop(key).map(|v| v.as_str().to_owned()).unwrap_or_else(|| opts[0].to_owned());
    ui.label(key);
    egui::ComboBox::from_id_salt(format!("cb_{ctrl_id}_{key}"))
        .selected_text(&cur).width(140.0)
        .show_ui(ui, |ui| {
            for &opt in opts {
                if ui.selectable_label(cur == opt, opt).clicked() {
                    action.set_props.push((ctrl_id.to_owned(), key.to_owned(), PropValue::String(opt.to_owned())));
                }
            }
        });
}

/// Combo row — inline horizontal style.
fn combo_row_inline(ui: &mut Ui, ctrl_id: &str, key: &str, ctrl: &Control, action: &mut InspectorAction, opts: &[&str]) {
    let cur = ctrl.get_prop(key).map(|v| v.as_str().to_owned()).unwrap_or_else(|| opts[0].to_owned());
    ui.horizontal(|ui| {
        ui.label(key);
        egui::ComboBox::from_id_salt(format!("cbi_{ctrl_id}_{key}"))
            .selected_text(&cur).width(140.0)
            .show_ui(ui, |ui| {
                for &opt in opts {
                    if ui.selectable_label(cur == opt, opt).clicked() {
                        action.set_props.push((ctrl_id.to_owned(), key.to_owned(), PropValue::String(opt.to_owned())));
                    }
                }
            });
    });
}

/// Border colour + style rows.
fn border_rows(
    ui:    &mut Ui,
    ctrl_id: &str,
    ctrl:  &Control,
    action: &mut InspectorAction,
    _bufs: &mut std::collections::HashMap<String, String>,
) {
    if ctrl.get_prop("BorderColor").is_some() {
        color_row(ui, ctrl_id, "BorderColor", ctrl, action);
    }
    if ctrl.get_prop("BorderStyle").is_some() {
        combo_row_inline(ui, ctrl_id, "BorderStyle", ctrl, action, &["None","Single","Fixed3D","Raised","Sunken"]);
    }
}

/// Browse button + text field for an image path property.
fn image_browse_row(
    ui:    &mut Ui,
    ctrl_id: &str,
    key:   &str,
    ctrl:  &Control,
    action: &mut InspectorAction,
    bufs:  &mut std::collections::HashMap<String, String>,
) {
    let cur = ctrl.get_prop(key).map(|v| v.as_str().to_owned()).unwrap_or_default();
    let buf_key = format!("{ctrl_id}-{key}");
    let wid = egui::Id::new(&buf_key);
    let buf = bufs.entry(buf_key).or_insert(cur.clone());
    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
    let pick_key = format!("imgpick:{ctrl_id}:{key}");
    ui.horizontal(|ui| {
        ui.label(key);
        // Open the native picker asynchronously — a synchronous dialog nests the
        // OS event loop and aborts winit 0.30.
        if ui.button("📂").on_hover_text("Browse for image…").clicked() {
            crate::file_dialog::open_file(
                &pick_key,
                "Images",
                &["png","jpg","jpeg","bmp","gif","ico","webp","svg"],
            );
        }
        // Keep repainting while the dialog is open so the result is collected.
        if crate::file_dialog::is_open(&pick_key) {
            ui.ctx().request_repaint();
        }
        if let Some(Some(p)) = crate::file_dialog::take(&pick_key) {
            let path_str = p.to_string_lossy().to_string();
            *buf = path_str.clone();
            action.set_props.push((ctrl_id.to_owned(), key.to_owned(), PropValue::String(path_str)));
        }
        if ui.add(egui::TextEdit::singleline(buf)
            .id(wid)
            .hint_text("(none)")
            .desired_width(f32::INFINITY)).lost_focus()
        {
            action.set_props.push((ctrl_id.to_owned(), key.to_owned(), PropValue::String(buf.clone())));
        }
    });
}

/// Multiline text field for list items.
fn items_multiline(
    ui: &mut Ui,
    ctrl_id: &str,
    ctrl: &Control,
    action: &mut InspectorAction,
    bufs: &mut std::collections::HashMap<String, String>,
) {
    let cur = ctrl.get_prop("Items").map(|v| v.as_str().to_owned()).unwrap_or_default();
    let buf_key = format!("{ctrl_id}-Items");
    let wid = egui::Id::new(&buf_key);
    let buf = bufs.entry(buf_key).or_insert(cur.clone());
    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) { *buf = cur; }
    ui.label("Items (one per line):");
    let resp = ui.add(egui::TextEdit::multiline(buf).id(wid).desired_rows(4).desired_width(f32::INFINITY));
    if resp.lost_focus() {
        action.set_props.push((ctrl_id.to_owned(), "Items".into(), PropValue::String(buf.clone())));
    }
}

/// Parse an RGB or RGBA hex colour string (`#RRGGBB` or `#RRGGBBAA`).
/// The alpha component is stored as straight alpha (0 = transparent, FF = opaque).
pub fn hex_to_color32(s: &str) -> Color32 {
    let s = s.trim_start_matches('#');
    // 8-char: RRGGBBAA — straight alpha
    if s.len() == 8 {
        if let (Ok(r), Ok(g), Ok(b), Ok(a)) = (
            u8::from_str_radix(&s[0..2], 16),
            u8::from_str_radix(&s[2..4], 16),
            u8::from_str_radix(&s[4..6], 16),
            u8::from_str_radix(&s[6..8], 16),
        ) {
            return Color32::from_rgba_unmultiplied(r, g, b, a);
        }
    }
    // 6-char: RRGGBB — fully opaque
    if s.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&s[0..2], 16),
            u8::from_str_radix(&s[2..4], 16),
            u8::from_str_radix(&s[4..6], 16),
        ) {
            return Color32::from_rgb(r, g, b);
        }
    }
    Color32::from_rgb(240, 240, 240)
}

/// Serialise a colour to `#RRGGBBAA` (always includes the alpha channel so
/// transparency round-trips correctly through the properties panel).
pub fn color32_to_hex(c: Color32) -> String {
    // Color32 stores premultiplied alpha; unmultiply to get straight-alpha RGB.
    let a = c.a();
    let (r, g, b) = if a == 0 {
        (0u8, 0u8, 0u8)
    } else if a == 255 {
        (c.r(), c.g(), c.b())
    } else {
        let af = a as f32 / 255.0;
        (
            (c.r() as f32 / af).round().clamp(0.0, 255.0) as u8,
            (c.g() as f32 / af).round().clamp(0.0, 255.0) as u8,
            (c.b() as f32 / af).round().clamp(0.0, 255.0) as u8,
        )
    };
    format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
}
