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

use crate::i18n::Tr;
use crate::panels::data_binding::{
    default_mappings_for_target, default_member_property, visibility_for_control,
    BindingEditorSourceKind, DataBindingVisibility,
};
use cobolt_ast::data::{DataDecl, PicKind};
use cobolt_ast::program::DataSection;
use cobolt_forms::model::{
    AnimKind, AnimRepeat, AnimTrigger, BgImageMode, DataGridAdvanced, DataGridCellFrame,
    DataGridGauge, DataGridGridLineStyle, DataGridTextAlignment, DataGridValueStyleRule,
    EasingKind, PropValue, DATAGRID_ADVANCED_PROP,
};
use cobolt_forms::{
    BindingDataType, BindingField, BindingSourceDescriptor, BindingTargetDescriptor,
    BindingTargetPath, Control, ControlType, DataBindingDef, FieldMapping, Form,
};
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use egui::{Color32, DragValue, RichText, ScrollArea, Ui};

/// A colour-swatch button that opens egui's colour picker in a pinned popup with a
/// fixed-size header (live swatch + editable `RRGGBBAA` hex) and fixed-width R/G/B/A
/// fields. The popup stays open while the user picks and closes only on a click
/// **outside** its area (or Escape).
fn color_edit_button_closing(ui: &mut Ui, color: &mut Color32) -> egui::Response {
    use egui::color_picker::{color_picker_color32, show_color_at, Alpha};
    use egui::{Area, Frame, Key, Order, Pos2, Sense, Stroke, UiKind, Vec2};

    // ── Colour swatch button ──────────────────────────────────────────────────
    let size = Vec2::new(35.0, ui.spacing().interact_size.y.max(16.0));
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        show_color_at(ui.painter(), *color, rect);
        ui.painter()
            .rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_gray(120)));
    }

    let popup_id = resp.id.with("__closing_color_popup");
    let anchor_id = resp.id.with("__closing_color_anchor");
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
        // Pin the popup to where the swatch is *at open time*. Using the live
        // swatch rect each frame makes the popup drift when the panel reflows or
        // scrolls during a drag (e.g. dragging the 2-D picker to its border).
        ui.memory_mut(|m| m.data.insert_temp(anchor_id, resp.rect.left_bottom()));
    }

    if ui.memory(|m| m.is_popup_open(popup_id)) {
        let anchor: Pos2 = ui
            .memory(|m| m.data.get_temp(anchor_id))
            .unwrap_or_else(|| resp.rect.left_bottom());
        let area = Area::new(popup_id)
            .kind(UiKind::Picker)
            .order(Order::Foreground)
            .fixed_pos(anchor)
            .constrain(true)
            .show(ui.ctx(), |ui| {
                // 25 % smaller than the previous 275 px picker (square + sliders all
                // size off `slider_width`).
                let slider_w = 206.0_f32;
                ui.spacing_mut().slider_width = slider_w;
                let inner = Frame::popup(ui.style()).show(ui, |ui| {
                    // Fix the width of every numeric field (R/G/B/A) so the row can't
                    // reflow as values change. egui sizes a DragValue from
                    // `interact_size.x`; the default (40) is just under the width of
                    // "R 255", so 3-digit values overflowed it and 1-digit values
                    // didn't — that digit-dependent jitter was the swaying. A value
                    // comfortably wider than "A 255" pins all four fields.
                    ui.spacing_mut().interact_size.x = 54.0;

                    let mut changed = false;

                    // ── Fixed header: live swatch + editable HTML hex (RRGGBBAA) ──
                    // Fixed-size widgets, so this readout never moves as the colour
                    // changes. The last two hex digits are the alpha channel.
                    ui.horizontal(|ui| {
                        let (sw_rect, _) =
                            ui.allocate_exact_size(Vec2::new(46.0, 20.0), Sense::hover());
                        show_color_at(ui.painter(), *color, sw_rect);
                        ui.painter().rect_stroke(
                            sw_rect,
                            2.0,
                            Stroke::new(1.0, Color32::from_gray(120)),
                        );

                        ui.label("#");

                        // Editable hex buffer kept in memory so partial typing works;
                        // re-synced from `color` whenever the 2-D picker changes it.
                        let buf_id = popup_id.with("__hexbuf");
                        let last_id = popup_id.with("__hexlast");
                        let canonical = color32_to_hex(*color)[1..].to_string(); // drop '#'
                        let mut buf = ui
                            .memory(|m| m.data.get_temp::<String>(buf_id))
                            .unwrap_or_default();
                        let last = ui.memory(|m| m.data.get_temp::<Color32>(last_id));
                        if buf.is_empty() || last != Some(*color) {
                            buf = canonical;
                        }

                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut buf)
                                .font(egui::TextStyle::Monospace)
                                .char_limit(8)
                                .desired_width(78.0)
                                .hint_text("RRGGBBAA"),
                        );
                        buf.retain(|c| c.is_ascii_hexdigit());
                        buf.make_ascii_uppercase();
                        if resp.changed() && (buf.len() == 8 || buf.len() == 6) {
                            let new_c = hex_to_color32(&buf);
                            if new_c != *color {
                                *color = new_c;
                                changed = true;
                            }
                        }
                        ui.memory_mut(|m| {
                            m.data.insert_temp(buf_id, buf);
                            m.data.insert_temp(last_id, *color);
                        });
                    });
                    ui.separator();

                    changed |= color_picker_color32(ui, color, Alpha::BlendOrAdditive);
                    changed
                });
                (inner.inner, inner.response.rect)
            });
        let (changed, _popup_rect) = area.inner;
        if changed {
            resp.mark_changed();
        }

        // Stay open while the user picks; close only on a click **outside** the
        // popup (or Escape). The `!resp.clicked()` guard stops the opening click
        // from immediately closing it.
        if !resp.clicked()
            && (ui.input(|i| i.key_pressed(Key::Escape)) || area.response.clicked_elsewhere())
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
    pub set_props: Vec<(String, String, PropValue)>,
    pub form_props: Vec<(String, String)>,
    /// `(ctrl_id, event_name)` — emitted when the user clicks an event row to open the modal editor.
    /// `ctrl_id` is empty for form-level events.
    pub open_event_editor: Option<(String, String)>,
    /// `(ctrl_id, event_name)` — emitted when the user **double-clicks** an event row to
    /// jump to that event's paragraph in the generated COBOL code editor.
    /// `ctrl_id` is empty for form-level events.
    pub open_event_in_code: Option<(String, String)>,
    /// Set when a COBOL Structure section / procedure row is clicked — the caller
    /// opens the popup editor for that single block (spec 005).
    pub cs_open: Option<super::cobol_structure::CsTarget>,
    /// Set when the "Add procedure" button is clicked.
    pub cs_add_proc: bool,
    /// Set with the index when a user-procedure's delete button is clicked.
    pub cs_del_proc: Option<usize>,
    /// Set when the "Edit Menu..." button is clicked for a MenuBar control.
    pub open_menu_editor: Option<String>,
    /// Set when the data-binding editor creates a new form-level binding.
    pub create_data_binding: Option<DataBindingDef>,
    /// `(old_id, new_id)` — set when the user renames the selected control in the
    /// Identity header. The caller renames it throughout the form.
    pub rename_control: Option<(String, String)>,
}

/// A member control of a repeating-group target that a source field can map to.
#[derive(Debug, Clone)]
struct MemberTarget {
    id: String,
    property: String,
    type_label: String,
}

struct BindingEditorState {
    target_control_id: String,
    /// Non-empty only for a control-array (repeating GroupBox) target: the member
    /// controls a source field can be mapped onto.
    member_controls: Vec<MemberTarget>,
    selected_source: Option<BindingEditorSourceKind>,
    binding_id: String,
    display_name: String,
    indexed_files: Vec<String>,
    cobol_tables: Vec<CobolTableBindingMeta>,
    selected_indexed_file: String,
    selected_sql_control: String,
    selected_cobol_table: String,
    selected_cobol_field_to_add: String,
    rest_endpoint: String,
    rest_method: RestMethod,
    rest_headers: Vec<HttpHeaderRow>,
    rest_auth: RestAuth,
    show_jsonpath_help: bool,
    page: i64,
    page_size: i64,
    rows: Vec<BindingFieldRow>,
    removed_rows: Vec<BindingFieldRow>,
    dropdown_config_row: Option<usize>,
    validation_error: Option<String>,
    confirm_clear: bool,
}

impl BindingEditorState {
    fn new(
        form: &Form,
        control: &Control,
        source_kind: BindingEditorSourceKind,
        indexed_files: &[String],
    ) -> Option<Self> {
        // Validate the control is a bindable target, but key the editor by the
        // *control id*, not the target's primary id. For a ControlArray the
        // primary id is the array **name** (not a control id), which broke both
        // the "is this editor for the selected control?" match and the
        // target-descriptor lookup at apply time — so the source button opened a
        // modal that instantly closed and nothing happened.
        let target = form.binding_target_descriptor_for_control(&control.id)?;
        let target_id = control.id.to_owned();
        // For a control array, gather the member controls a source field can map
        // onto (with each control's default bindable property).
        let member_controls = if let BindingTargetDescriptor::ControlArray {
            member_control_ids,
            ..
        } = &target
        {
            member_control_ids
                .iter()
                .map(|member_id| MemberTarget {
                    id: member_id.clone(),
                    property: default_member_property(form, member_id),
                    type_label: form
                        .find_control(member_id)
                        .map(|c| c.control_type.as_str().to_owned())
                        .unwrap_or_default(),
                })
                .collect()
        } else {
            Vec::new()
        };
        let binding_id = next_panel_binding_id(form, source_kind, &target_id);
        let cobol_tables = cobol_table_binding_metadata(form);
        let selected_source =
            if source_kind == BindingEditorSourceKind::IndexedFile && indexed_files.is_empty() {
                None
            } else {
                Some(source_kind)
            };
        let selected_cobol_table = String::new();
        let rows = if source_kind == BindingEditorSourceKind::CobolTable {
            Vec::new()
        } else {
            sample_rows_for_source(source_kind)
        };
        Some(Self {
            target_control_id: target_id.clone(),
            member_controls,
            selected_source,
            binding_id,
            display_name: format!("{} -> {}", source_kind.id_fragment(), target_id),
            indexed_files: indexed_files.to_vec(),
            cobol_tables,
            selected_indexed_file: indexed_files
                .first()
                .cloned()
                .unwrap_or_else(|| "CUSTOMER-DATA (CUSTDAT)".to_owned()),
            selected_sql_control: "SQL-CUSTOMERS".to_owned(),
            selected_cobol_table,
            selected_cobol_field_to_add: String::new(),
            rest_endpoint: "https://api.example.com/v1/products".to_owned(),
            rest_method: RestMethod::Get,
            rest_headers: Vec::new(),
            rest_auth: RestAuth::default(),
            show_jsonpath_help: true,
            page: 1,
            page_size: 50,
            rows,
            removed_rows: Vec::new(),
            dropdown_config_row: None,
            validation_error: None,
            confirm_clear: false,
        })
    }

    /// Open the editor pre-filled from an **existing** binding so a databound
    /// control's settings can be edited instead of re-entered from scratch. Starts
    /// from [`new`] (to seed member-control targets, indexed files, cobol-table
    /// metadata) then overrides the source selection, id/name, field rows, and
    /// control-array member mappings from the persisted binding.
    fn from_existing(
        form: &Form,
        control: &Control,
        binding: &DataBindingDef,
        indexed_files: &[String],
    ) -> Option<Self> {
        use cobolt_forms::BindingSourceDescriptor as Src;
        let source_kind = match &binding.source {
            Src::IndexedFile { .. } => BindingEditorSourceKind::IndexedFile,
            Src::Sql { .. } => BindingEditorSourceKind::Sql,
            Src::CobolTable { .. } => BindingEditorSourceKind::CobolTable,
            Src::RestApi { .. } => BindingEditorSourceKind::RestApi,
            Src::AgentAi { .. } => BindingEditorSourceKind::AgentAi,
        };
        let mut s = Self::new(form, control, source_kind, indexed_files)?;
        s.binding_id = binding.id.clone();
        if !binding.display_name.trim().is_empty() {
            s.display_name = binding.display_name.clone();
        }
        match &binding.source {
            Src::IndexedFile {
                definition_path, ..
            } if !definition_path.trim().is_empty() => {
                s.selected_indexed_file = definition_path.clone();
            }
            Src::Sql {
                source_control_id, ..
            } if !source_control_id.trim().is_empty() => {
                s.selected_sql_control = source_control_id.clone();
            }
            Src::CobolTable { table_name, .. } => {
                s.selected_cobol_table = table_name.clone();
            }
            Src::RestApi { endpoint_name, .. } if !endpoint_name.trim().is_empty() => {
                s.rest_endpoint = endpoint_name.clone();
            }
            _ => {}
        }
        // Rebuild the field rows from the persisted source fields.
        s.rows = binding
            .source
            .fields()
            .iter()
            .map(|f| {
                let friendly = if f.display_name.trim().is_empty() {
                    f.name.clone()
                } else {
                    f.display_name.clone()
                };
                let mut row = BindingFieldRow::new(
                    f.name.clone(),
                    f.cobol_mask.clone(),
                    f.data_type.clone(),
                    friendly,
                    BindingEditControl::from_label(&f.edit_control),
                    DropdownConfig::empty(),
                );
                row.required = !f.nullable;
                row.key = f.key;
                row.visible = true;
                row.enabled = true;
                row
            })
            .collect();
        // Restore which source field maps onto which member control (arrays).
        for mapping in &binding.mappings {
            if let BindingTargetPath::ControlProperty { control_id, .. } = &mapping.target {
                if let Some(row) = s
                    .rows
                    .iter_mut()
                    .find(|r| r.source_field.eq_ignore_ascii_case(&mapping.source_field))
                {
                    row.target_member = control_id.clone();
                }
            }
        }
        Some(s)
    }

    fn select_source(&mut self, source_kind: BindingEditorSourceKind) {
        if self.selected_source == Some(source_kind) {
            return;
        }
        self.selected_source = Some(source_kind);
        self.page = 1;
        self.validation_error = None;
        self.rows = if source_kind == BindingEditorSourceKind::CobolTable {
            self.rows_for_selected_cobol_table(&[])
        } else {
            sample_rows_for_source(source_kind)
        };
        self.removed_rows.clear();
        self.dropdown_config_row = None;
        if source_kind == BindingEditorSourceKind::IndexedFile
            && self.selected_indexed_file.trim().is_empty()
        {
            self.selected_indexed_file = self
                .indexed_files
                .first()
                .cloned()
                .unwrap_or_else(|| "CUSTOMER-DATA (CUSTDAT)".to_owned());
        }
        if source_kind == BindingEditorSourceKind::Sql
            && self.selected_sql_control.trim().is_empty()
        {
            self.selected_sql_control = "SQL-CUSTOMERS".to_owned();
        }
        if source_kind == BindingEditorSourceKind::RestApi && self.rest_endpoint.trim().is_empty() {
            self.rest_endpoint = "https://api.example.com/v1/products".to_owned();
        }
    }

    /// Field → member-property mappings from the "Map fields to controls" grid
    /// (control-array targets only). Unmapped fields are skipped.
    fn control_array_mappings(&self, array_id: &str) -> Vec<FieldMapping> {
        self.rows
            .iter()
            .filter(|row| !row.target_member.trim().is_empty())
            .map(|row| {
                let property = self
                    .member_controls
                    .iter()
                    .find(|member| member.id == row.target_member)
                    .map(|member| member.property.clone())
                    .unwrap_or_else(|| "Text".to_owned());
                FieldMapping::new(
                    row.source_field.clone(),
                    BindingTargetPath::ControlProperty {
                        array_id: array_id.to_owned(),
                        control_id: row.target_member.clone(),
                        property_name: property,
                    },
                )
            })
            .collect()
    }

    fn to_binding(&self, form: &Form) -> Option<DataBindingDef> {
        let source_kind = self.selected_source?;
        let target = form.binding_target_descriptor_for_control(&self.target_control_id)?;
        let fields = self.binding_fields();
        let source = self.to_source_descriptor(source_kind, fields.clone());
        let mappings = match &target {
            BindingTargetDescriptor::ControlArray { array_id, .. } => {
                self.control_array_mappings(array_id)
            }
            _ => default_mappings_for_target(form, &target, &fields),
        };
        Some(
            DataBindingDef::new(
                clean_or_default(&self.binding_id, "BINDING"),
                clean_or_default(&self.display_name, "Data binding"),
                source,
                target,
            )
            .with_mappings(mappings),
        )
    }

    fn to_source_descriptor(
        &self,
        source_kind: BindingEditorSourceKind,
        fields: Vec<BindingField>,
    ) -> BindingSourceDescriptor {
        let key_fields: Vec<String> = fields
            .iter()
            .filter(|field| field.key)
            .map(|field| field.name.clone())
            .collect();
        match source_kind {
            BindingEditorSourceKind::IndexedFile => BindingSourceDescriptor::IndexedFile {
                definition_path: clean_or_default(
                    &self.selected_indexed_file,
                    "indexed/source.cidx",
                ),
                record_name: "CUSTOMER-RECORD".to_owned(),
                key_field: key_fields.first().cloned(),
                fields,
                writable: true,
            },
            BindingEditorSourceKind::Sql => BindingSourceDescriptor::Sql {
                source_control_id: clean_or_default(&self.selected_sql_control, "SQL-CUSTOMERS"),
                query_name: "DEFAULT-QUERY".to_owned(),
                result_set_name: "RESULT-SET".to_owned(),
                fields,
                key_fields,
                writable: true,
            },
            BindingEditorSourceKind::CobolTable => BindingSourceDescriptor::CobolTable {
                table_name: self.selected_cobol_table.trim().to_owned(),
                occurs_item: self.selected_cobol_occurs_item(),
                fields,
                key_fields,
                writable: true,
            },
            BindingEditorSourceKind::RestApi => BindingSourceDescriptor::RestApi {
                source_control_id: "REST-API".to_owned(),
                endpoint_name: clean_or_default(&self.rest_endpoint, "PRODUCTS-ENDPOINT"),
                response_data_item: "REST-RESPONSE".to_owned(),
                fields,
                update: None,
            },
            BindingEditorSourceKind::AgentAi => BindingSourceDescriptor::AgentAi {
                source_control_id: "AGENT-1".to_owned(),
                output_name: "DEFAULT-OUTPUT".to_owned(),
                fields,
                update: None,
            },
        }
    }

    fn binding_fields(&self) -> Vec<BindingField> {
        self.rows
            .iter()
            .map(|row| {
                let mut field = BindingField::new(row.source_field.clone(), row.data_type.clone());
                field.display_name = row.friendly_name.clone();
                field.cobol_mask = row.cobol_mask.clone();
                field.edit_control = row.edit_control.label().to_owned();
                field.nullable = !row.required;
                field.key = row.key;
                field
            })
            .collect()
    }

    fn validate(&self) -> Result<(), String> {
        let source = self
            .selected_source
            .ok_or_else(|| "A binding source must be selected.".to_owned())?;
        if source == BindingEditorSourceKind::IndexedFile
            && self.selected_indexed_file.trim().is_empty()
        {
            return Err("Indexed file must be selected.".to_owned());
        }
        if source == BindingEditorSourceKind::Sql && self.selected_sql_control.trim().is_empty() {
            return Err("SQL control must be selected.".to_owned());
        }
        if source == BindingEditorSourceKind::CobolTable {
            self.validate_cobol_table_source()?;
        }
        if source == BindingEditorSourceKind::RestApi {
            self.validate_rest_source()?;
        }
        if self.rows.is_empty() {
            return Err("At least one source field must remain visible.".to_owned());
        }
        if !self.rows.iter().any(|row| row.visible) {
            return Err("At least one source field must be visible.".to_owned());
        }
        for row in &self.rows {
            if row.friendly_name.trim().is_empty() {
                return Err(format!("{} needs a friendly name.", row.source_field));
            }
            if source == BindingEditorSourceKind::RestApi && row.source_field.trim().is_empty() {
                return Err("Every REST source field needs a JSONPath expression.".to_owned());
            }
            if source == BindingEditorSourceKind::RestApi && !is_valid_jsonpath(&row.source_field) {
                return Err(format!(
                    "{} is not a valid JSONPath expression.",
                    row.source_field
                ));
            }
            if row.data_type != BindingDataType::Boolean && row.cobol_mask.trim().is_empty() {
                return Err(format!("{} needs a COBOL mask.", row.source_field));
            }
            if source != BindingEditorSourceKind::RestApi
                && row.edit_control == BindingEditControl::Dropdown
            {
                row.dropdown.validate(&row.source_field)?;
            }
        }
        Ok(())
    }

    fn validate_rest_source(&self) -> Result<(), String> {
        if self.rest_endpoint.trim().is_empty() {
            return Err("REST API endpoint must be selected.".to_owned());
        }
        if !is_valid_url(&self.rest_endpoint) {
            return Err("REST API endpoint must be a valid URL.".to_owned());
        }
        for header in &self.rest_headers {
            if header.name.trim().is_empty() && !header.value.trim().is_empty() {
                return Err("Header names must be filled when a value is provided.".to_owned());
            }
        }
        self.rest_auth.validate()
    }

    fn validate_cobol_table_source(&self) -> Result<(), String> {
        if self.selected_cobol_table.trim().is_empty() {
            return Err("COBOL table must be selected.".to_owned());
        }
        if !self
            .cobol_tables
            .iter()
            .any(|table| table.name == self.selected_cobol_table)
        {
            return Err(
                "Selected COBOL table must resolve to a 01-level GLOBAL item with OCCURS."
                    .to_owned(),
            );
        }
        if self.selected_cobol_occurs_item().trim().is_empty() {
            return Err("COBOL table occurs item must be resolved.".to_owned());
        }
        Ok(())
    }

    fn selected_cobol_occurs_item(&self) -> String {
        self.cobol_tables
            .iter()
            .find(|table| table.name == self.selected_cobol_table)
            .map(|table| table.occurs_item.clone())
            .unwrap_or_default()
    }

    fn selected_cobol_table_meta(&self) -> Option<&CobolTableBindingMeta> {
        self.cobol_tables
            .iter()
            .find(|table| table.name == self.selected_cobol_table)
    }

    fn rows_for_selected_cobol_table(
        &self,
        previous_rows: &[BindingFieldRow],
    ) -> Vec<BindingFieldRow> {
        let Some(table) = self.selected_cobol_table_meta() else {
            return Vec::new();
        };
        table
            .fields
            .iter()
            .map(|field| {
                previous_rows
                    .iter()
                    .find(|row| row.source_field == field.name)
                    .cloned()
                    .unwrap_or_else(|| field.to_row())
            })
            .collect()
    }

    fn missing_cobol_table_fields(&self) -> Vec<CobolTableFieldMeta> {
        let Some(table) = self.selected_cobol_table_meta() else {
            return Vec::new();
        };
        table
            .fields
            .iter()
            .filter(|field| {
                !self
                    .rows
                    .iter()
                    .any(|row| row.source_field.eq_ignore_ascii_case(&field.name))
            })
            .cloned()
            .collect()
    }

    fn clear_selection(&mut self) {
        self.selected_source = None;
        self.selected_indexed_file.clear();
        self.selected_sql_control.clear();
        self.selected_cobol_table.clear();
        self.rest_endpoint.clear();
        self.rest_headers.clear();
        self.selected_cobol_field_to_add.clear();
        self.rows.clear();
        self.removed_rows.clear();
        self.dropdown_config_row = None;
        self.validation_error = None;
        self.confirm_clear = false;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BindingEditControl {
    Textbox,
    Dropdown,
    Checkbox,
}

impl BindingEditControl {
    const ALL: [Self; 3] = [Self::Textbox, Self::Dropdown, Self::Checkbox];

    fn label(&self) -> &'static str {
        match self {
            Self::Textbox => "Textbox",
            Self::Dropdown => "Dropdown",
            Self::Checkbox => "Checkbox",
        }
    }

    /// Reverse of [`label`] — parse a persisted `BindingField::edit_control` string
    /// back into the editor's enum (defaults to Textbox for anything unknown).
    fn from_label(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "dropdown" => Self::Dropdown,
            "checkbox" => Self::Checkbox,
            _ => Self::Textbox,
        }
    }
}

const DATA_BINDING_MODAL_SOURCES: [BindingEditorSourceKind; 4] = [
    BindingEditorSourceKind::IndexedFile,
    BindingEditorSourceKind::Sql,
    BindingEditorSourceKind::CobolTable,
    BindingEditorSourceKind::RestApi,
];

const MOCK_SQL_CONTROLS: &[&str] = &[
    "SQL-CUSTOMERS",
    "SQL-COUNTRIES",
    "SQL-ORDERS",
    "SQL-INVOICES",
];
const MOCK_COBOL_TABLES: &[&str] = &[
    "CUSTOMER_TIERS",
    "PAYMENT_TERMS",
    "ORDER_STATUS",
    "WS-CATEGORY-TABLE",
];
const MOCK_INDEXED_LOOKUPS: &[&str] = &["COUNTRY-CODES (CNTRYIDX)", "REGION-CODES (REGIONST)"];
const MOCK_REST_CONTROLS: &[&str] = &["REST-LOOKUP", "REST-COUNTRIES", "REST-STATUS"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct CobolTableBindingMeta {
    name: String,
    occurs_item: String,
    fields: Vec<CobolTableFieldMeta>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CobolTableFieldMeta {
    name: String,
    picture: String,
    data_type: BindingDataType,
}

impl CobolTableFieldMeta {
    fn to_row(&self) -> BindingFieldRow {
        let mut row = BindingFieldRow::new(
            self.name.clone(),
            self.picture.clone(),
            self.data_type.clone(),
            friendly_name_from_cobol_name(&self.name),
            BindingEditControl::Textbox,
            DropdownConfig::empty(),
        );
        row.cobol_mask = format!("PIC {}", normalize_pic_template(&self.picture));
        row
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RestMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl RestMethod {
    const ALL: [Self; 5] = [Self::Get, Self::Post, Self::Put, Self::Patch, Self::Delete];

    fn label(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone, Default)]
struct HttpHeaderRow {
    name: String,
    value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RestAuthMode {
    None,
    ApiKey,
    BearerToken,
    BasicAuth,
}

impl RestAuthMode {
    const ALL: [Self; 4] = [Self::None, Self::ApiKey, Self::BearerToken, Self::BasicAuth];

    fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::ApiKey => "API key",
            Self::BearerToken => "Bearer token",
            Self::BasicAuth => "Basic auth",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApiKeyLocation {
    Header,
    Query,
}

impl ApiKeyLocation {
    const ALL: [Self; 2] = [Self::Header, Self::Query];

    fn label(self) -> &'static str {
        match self {
            Self::Header => "Header",
            Self::Query => "Query",
        }
    }
}

#[derive(Debug, Clone)]
struct RestAuth {
    mode: RestAuthMode,
    api_key_name: String,
    api_key_value: String,
    api_key_location: ApiKeyLocation,
    bearer_token: String,
    username: String,
    password: String,
}

impl Default for RestAuth {
    fn default() -> Self {
        Self {
            mode: RestAuthMode::None,
            api_key_name: String::new(),
            api_key_value: String::new(),
            api_key_location: ApiKeyLocation::Header,
            bearer_token: String::new(),
            username: String::new(),
            password: String::new(),
        }
    }
}

impl RestAuth {
    fn validate(&self) -> Result<(), String> {
        match self.mode {
            RestAuthMode::None => Ok(()),
            RestAuthMode::ApiKey => {
                if self.api_key_name.trim().is_empty() || self.api_key_value.trim().is_empty() {
                    Err("API key authentication needs key name and key value.".to_owned())
                } else {
                    Ok(())
                }
            }
            RestAuthMode::BearerToken => {
                if self.bearer_token.trim().is_empty() {
                    Err("Bearer token authentication needs a token.".to_owned())
                } else {
                    Ok(())
                }
            }
            RestAuthMode::BasicAuth => {
                if self.username.trim().is_empty() || self.password.trim().is_empty() {
                    Err("Basic auth needs username and password.".to_owned())
                } else {
                    Ok(())
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DropdownOrigin {
    IndexedFile,
    SqlControl,
    CobolTable,
    RestApi,
    StaticValues,
}

impl DropdownOrigin {
    const ALL: [Self; 5] = [
        Self::IndexedFile,
        Self::SqlControl,
        Self::CobolTable,
        Self::RestApi,
        Self::StaticValues,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::IndexedFile => "Indexed file",
            Self::SqlControl => "SQL control (result set)",
            Self::CobolTable => "COBOL table",
            Self::RestApi => "REST API",
            Self::StaticValues => "Static values",
        }
    }

    fn summary(self) -> &'static str {
        match self {
            Self::IndexedFile => "Indexed file",
            Self::SqlControl => "SQL control",
            Self::CobolTable => "COBOL table",
            Self::RestApi => "REST API",
            Self::StaticValues => "Static values",
        }
    }

    fn source_label(self) -> Option<&'static str> {
        match self {
            Self::IndexedFile => Some("Select indexed file"),
            Self::SqlControl => Some("Select SQL control"),
            Self::CobolTable => Some("Select COBOL table"),
            Self::RestApi => Some("Select REST API control"),
            Self::StaticValues => None,
        }
    }
}

#[derive(Debug, Clone)]
struct DropdownConfig {
    origin: Option<DropdownOrigin>,
    source_ref: String,
    display_field: String,
    value_field: String,
    line_limit: i64,
    static_options: String,
}

impl DropdownConfig {
    fn city() -> Self {
        Self {
            origin: Some(DropdownOrigin::IndexedFile),
            source_ref: "CITY-MASTER (CITYMAST)".to_owned(),
            display_field: "CITY-NAME (X(30))".to_owned(),
            value_field: "CITY-ID (9(04))".to_owned(),
            line_limit: 1000,
            static_options: String::new(),
        }
    }

    fn status() -> Self {
        Self {
            origin: Some(DropdownOrigin::IndexedFile),
            source_ref: "STATUS-CODES (STATCDS)".to_owned(),
            display_field: "STATUS-DESC (X(20))".to_owned(),
            value_field: "STATUS-CODE (X(10))".to_owned(),
            line_limit: 1000,
            static_options: String::new(),
        }
    }

    fn country_sql() -> Self {
        Self {
            origin: Some(DropdownOrigin::SqlControl),
            source_ref: "SQL-COUNTRIES".to_owned(),
            display_field: "COUNTRY_NAME (X(100))".to_owned(),
            value_field: "COUNTRY_ID (9(5))".to_owned(),
            line_limit: 1000,
            static_options: String::new(),
        }
    }

    fn tier_cobol_table() -> Self {
        Self {
            origin: Some(DropdownOrigin::CobolTable),
            source_ref: "CUSTOMER_TIERS".to_owned(),
            display_field: "TIER_NAME (X(30))".to_owned(),
            value_field: "TIER_ID (9(2))".to_owned(),
            line_limit: 1000,
            static_options: String::new(),
        }
    }

    fn category_cobol_table() -> Self {
        Self {
            origin: Some(DropdownOrigin::CobolTable),
            source_ref: "WS-CATEGORY-TABLE".to_owned(),
            display_field: "CATEGORY-NAME (X(30))".to_owned(),
            value_field: "CATEGORY-ID (9(04))".to_owned(),
            line_limit: 1000,
            static_options: String::new(),
        }
    }

    fn region_indexed_file() -> Self {
        Self {
            origin: Some(DropdownOrigin::IndexedFile),
            source_ref: "REGION-CODES (REGIONST)".to_owned(),
            display_field: "REGION-NAME (X(30))".to_owned(),
            value_field: "REGION-ID (9(04))".to_owned(),
            line_limit: 1000,
            static_options: String::new(),
        }
    }

    fn empty() -> Self {
        Self {
            origin: Some(DropdownOrigin::IndexedFile),
            source_ref: "CITY-MASTER (CITYMAST)".to_owned(),
            display_field: "CITY-NAME (X(30))".to_owned(),
            value_field: "CITY-ID (9(04))".to_owned(),
            line_limit: 1000,
            static_options: String::new(),
        }
    }

    fn validate(&self, field_name: &str) -> Result<(), String> {
        let origin = self
            .origin
            .ok_or_else(|| format!("{field_name} needs a dropdown origin."))?;
        match origin {
            DropdownOrigin::StaticValues => {
                if self.static_options.trim().is_empty() {
                    return Err(format!("{field_name} needs at least one static option."));
                }
            }
            DropdownOrigin::IndexedFile
            | DropdownOrigin::SqlControl
            | DropdownOrigin::CobolTable
            | DropdownOrigin::RestApi => {
                if self.line_limit <= 0 {
                    return Err(format!(
                        "{field_name} needs a positive dropdown line limit."
                    ));
                }
                if self.source_ref.trim().is_empty() {
                    return Err(format!("{field_name} needs a dropdown source."));
                }
                if self.display_field.trim().is_empty() {
                    return Err(format!("{field_name} needs a dropdown display field."));
                }
                if self.value_field.trim().is_empty() {
                    return Err(format!("{field_name} needs a dropdown value field."));
                }
            }
        }
        Ok(())
    }

    fn summary(&self) -> &'static str {
        match self.origin {
            Some(origin) => origin.summary(),
            None => "-",
        }
    }

    fn reset_for_origin(&mut self, origin: DropdownOrigin) {
        self.origin = Some(origin);
        self.source_ref.clear();
        self.display_field.clear();
        self.value_field.clear();
        match origin {
            DropdownOrigin::IndexedFile => {
                self.source_ref = "COUNTRY-CODES (CNTRYIDX)".to_owned();
                self.display_field = "COUNTRY_NAME (X(100))".to_owned();
                self.value_field = "COUNTRY_ID (9(5))".to_owned();
            }
            DropdownOrigin::SqlControl => {
                self.source_ref = "SQL-COUNTRIES".to_owned();
                self.display_field = "COUNTRY_NAME (X(100))".to_owned();
                self.value_field = "COUNTRY_ID (9(5))".to_owned();
            }
            DropdownOrigin::CobolTable => {
                self.source_ref = "CUSTOMER_TIERS".to_owned();
                self.display_field = "TIER_NAME (X(30))".to_owned();
                self.value_field = "TIER_ID (9(2))".to_owned();
            }
            DropdownOrigin::RestApi => {
                self.source_ref = "REST-LOOKUP".to_owned();
                self.display_field = "NAME (X(100))".to_owned();
                self.value_field = "ID (9(9))".to_owned();
            }
            DropdownOrigin::StaticValues => {
                self.static_options = "Y=Yes\nN=No".to_owned();
            }
        }
    }
}

#[derive(Debug, Clone)]
struct BindingFieldRow {
    source_field: String,
    picture: String,
    cobol_mask: String,
    data_type: BindingDataType,
    key: bool,
    required: bool,
    visible: bool,
    enabled: bool,
    friendly_name: String,
    edit_control: BindingEditControl,
    dropdown: DropdownConfig,
    /// For a control-array target: the member control id this field maps to
    /// (empty = unmapped). Ignored for other target kinds.
    target_member: String,
}

impl BindingFieldRow {
    fn new(
        source_field: impl Into<String>,
        picture: impl Into<String>,
        data_type: BindingDataType,
        friendly_name: impl Into<String>,
        edit_control: BindingEditControl,
        dropdown: DropdownConfig,
    ) -> Self {
        let picture = picture.into();
        Self {
            source_field: source_field.into(),
            picture: picture.clone(),
            cobol_mask: picture,
            data_type,
            key: false,
            required: true,
            visible: true,
            enabled: true,
            friendly_name: friendly_name.into(),
            edit_control,
            dropdown,
            target_member: String::new(),
        }
    }
}

fn sample_indexed_field_rows() -> Vec<BindingFieldRow> {
    let mut rows = vec![
        BindingFieldRow::new(
            "CUSTOMER-ID",
            "9(06)",
            BindingDataType::Integer,
            "Customer ID",
            BindingEditControl::Textbox,
            DropdownConfig::empty(),
        ),
        BindingFieldRow::new(
            "CUSTOMER-NAME",
            "X(40)",
            BindingDataType::Text,
            "Customer Name",
            BindingEditControl::Textbox,
            DropdownConfig::empty(),
        ),
        BindingFieldRow::new(
            "BALANCE",
            "S9(9)V99",
            BindingDataType::Decimal,
            "Balance",
            BindingEditControl::Textbox,
            DropdownConfig::empty(),
        ),
        BindingFieldRow::new(
            "ACTIVE",
            "X",
            BindingDataType::Boolean,
            "Active",
            BindingEditControl::Checkbox,
            DropdownConfig::empty(),
        ),
        BindingFieldRow::new(
            "CITY-ID",
            "9(04)",
            BindingDataType::Integer,
            "City",
            BindingEditControl::Dropdown,
            DropdownConfig::city(),
        ),
        BindingFieldRow::new(
            "STATUS",
            "X(10)",
            BindingDataType::Text,
            "Status",
            BindingEditControl::Dropdown,
            DropdownConfig::status(),
        ),
    ];
    if let Some(id) = rows.get_mut(0) {
        id.key = true;
    }
    rows
}

fn sample_sql_field_rows() -> Vec<BindingFieldRow> {
    let mut rows = vec![
        BindingFieldRow::new(
            "CUSTOMER_ID",
            "9(9)",
            BindingDataType::Integer,
            "Customer ID",
            BindingEditControl::Textbox,
            DropdownConfig::empty(),
        ),
        BindingFieldRow::new(
            "CUSTOMER_NAME",
            "X(100)",
            BindingDataType::Text,
            "Customer Name",
            BindingEditControl::Textbox,
            DropdownConfig::empty(),
        ),
        BindingFieldRow::new(
            "CREDIT_LIMIT",
            "9(15)V99",
            BindingDataType::Decimal,
            "Credit Limit",
            BindingEditControl::Textbox,
            DropdownConfig::empty(),
        ),
        BindingFieldRow::new(
            "IS_ACTIVE",
            "-",
            BindingDataType::Boolean,
            "Is Active",
            BindingEditControl::Checkbox,
            DropdownConfig::empty(),
        ),
        BindingFieldRow::new(
            "COUNTRY_ID",
            "9(5)",
            BindingDataType::Integer,
            "Country",
            BindingEditControl::Dropdown,
            DropdownConfig::country_sql(),
        ),
        BindingFieldRow::new(
            "TIER_ID",
            "9(2)",
            BindingDataType::Integer,
            "Customer Tier",
            BindingEditControl::Dropdown,
            DropdownConfig::tier_cobol_table(),
        ),
    ];
    if let Some(id) = rows.get_mut(0) {
        id.key = true;
    }
    if let Some(limit) = rows.get_mut(2) {
        limit.required = false;
    }
    if let Some(tier) = rows.get_mut(5) {
        tier.required = false;
    }
    rows
}

fn sample_rest_field_rows() -> Vec<BindingFieldRow> {
    let mut rows = vec![
        BindingFieldRow::new(
            "$.[*].title",
            "PIC X(60)",
            BindingDataType::Text,
            "Title",
            BindingEditControl::Textbox,
            DropdownConfig::empty(),
        ),
        BindingFieldRow::new(
            "$.[*].price",
            "PIC S9(9)V99",
            BindingDataType::Decimal,
            "Price",
            BindingEditControl::Textbox,
            DropdownConfig::empty(),
        ),
        BindingFieldRow::new(
            "$.[*].category",
            "PIC X(30)",
            BindingDataType::Text,
            "Category",
            BindingEditControl::Dropdown,
            DropdownConfig::empty(),
        ),
        BindingFieldRow::new(
            "$.[*].available",
            "PIC X",
            BindingDataType::Boolean,
            "Available",
            BindingEditControl::Checkbox,
            DropdownConfig::empty(),
        ),
    ];
    rows[0].cobol_mask = "X(60)".to_owned();
    rows[1].cobol_mask = "S9(9)V99".to_owned();
    rows[2].cobol_mask = "X(30)".to_owned();
    rows[2].required = false;
    rows[3].cobol_mask = "X".to_owned();
    rows[3].required = false;
    rows
}

fn sample_rows_for_source(source_kind: BindingEditorSourceKind) -> Vec<BindingFieldRow> {
    match source_kind {
        BindingEditorSourceKind::Sql => sample_sql_field_rows(),
        BindingEditorSourceKind::CobolTable => Vec::new(),
        BindingEditorSourceKind::RestApi => sample_rest_field_rows(),
        _ => sample_indexed_field_rows(),
    }
}

fn cobol_table_binding_metadata(form: &Form) -> Vec<CobolTableBindingMeta> {
    let ws = form.user_ws_source.trim();
    if ws.is_empty() {
        return Vec::new();
    }
    let source = format!(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. BINDING-METADATA.\n\
         DATA DIVISION.\n\
         WORKING-STORAGE SECTION.\n\
         {ws}\n\
         PROCEDURE DIVISION.\n\
         MAIN.\n\
             STOP RUN.\n"
    );
    let result = parse(tokenize(&source, SourceFormat::Free));
    let Some(program) = result.program else {
        return Vec::new();
    };
    let Some(data) = program.data else {
        return Vec::new();
    };
    data.sections
        .iter()
        .filter_map(|section| match section {
            DataSection::WorkingStorage(items) => Some(items),
            _ => None,
        })
        .flat_map(|items| items.iter())
        .filter_map(cobol_table_meta_from_root)
        .collect()
}

fn cobol_table_meta_from_root(root: &DataDecl) -> Option<CobolTableBindingMeta> {
    if root.level != 1 || !root.is_global {
        return None;
    }
    let name = root.name.as_ref()?.clone();
    let occurs_item = if root.occurs.is_some() {
        root
    } else {
        root.children.iter().find(|child| child.occurs.is_some())?
    };
    let occurs_name = occurs_item.name.as_ref()?.clone();
    let mut fields = Vec::new();
    collect_cobol_table_fields(occurs_item, &mut fields);
    if fields.is_empty() {
        return None;
    }
    Some(CobolTableBindingMeta {
        name,
        occurs_item: occurs_name,
        fields,
    })
}

fn collect_cobol_table_fields(item: &DataDecl, fields: &mut Vec<CobolTableFieldMeta>) {
    for child in &item.children {
        if let (Some(name), Some(pic)) = (&child.name, &child.picture) {
            fields.push(CobolTableFieldMeta {
                name: name.clone(),
                picture: pic.template.clone(),
                data_type: binding_data_type_from_pic(pic.kind, &pic.template),
            });
        }
        if !child.children.is_empty() {
            collect_cobol_table_fields(child, fields);
        }
    }
}

fn binding_data_type_from_pic(kind: PicKind, template: &str) -> BindingDataType {
    match kind {
        PicKind::Numeric | PicKind::NumericEdited => {
            if template.contains('V') || template.contains('v') {
                BindingDataType::Decimal
            } else {
                BindingDataType::Integer
            }
        }
        PicKind::Alphabetic | PicKind::Alphanumeric | PicKind::AlphanumericEdited => {
            BindingDataType::Text
        }
    }
}

fn normalize_pic_template(template: &str) -> String {
    let mut normalized = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '(' {
            normalized.push(ch);
            let mut digits = String::new();
            while let Some(next) = chars.peek().copied() {
                if next == ')' {
                    break;
                }
                digits.push(next);
                chars.next();
            }
            if digits.chars().all(|ch| ch.is_ascii_digit()) {
                let trimmed = digits.trim_start_matches('0');
                normalized.push_str(if trimmed.is_empty() { "0" } else { trimmed });
            } else {
                normalized.push_str(&digits);
            }
        } else {
            normalized.push(ch);
        }
    }
    normalized
}

fn friendly_name_from_cobol_name(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            let mut word = String::new();
            word.push(first.to_ascii_uppercase());
            for ch in chars {
                word.push(ch.to_ascii_lowercase());
            }
            word
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn binding_data_type_label(data_type: &BindingDataType) -> &'static str {
    match data_type {
        BindingDataType::Text => "Text",
        BindingDataType::Integer => "Integer",
        BindingDataType::Decimal => "Decimal",
        BindingDataType::Boolean => "Boolean",
        BindingDataType::Date => "Date",
        BindingDataType::DateTime => "DateTime",
        BindingDataType::Json => "Json",
        BindingDataType::Unknown => "Unknown",
    }
}

fn rest_detected_type_label(data_type: &BindingDataType) -> &'static str {
    match data_type {
        BindingDataType::Text => "String",
        BindingDataType::Integer | BindingDataType::Decimal => "Number",
        BindingDataType::Boolean => "Boolean",
        BindingDataType::Json => "Object",
        BindingDataType::Date | BindingDataType::DateTime => "String",
        BindingDataType::Unknown => "Unknown",
    }
}

fn is_valid_url(value: &str) -> bool {
    let trimmed = value.trim();
    let Some(rest) = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
    else {
        return false;
    };
    let host = rest.split('/').next().unwrap_or_default();
    !host.is_empty() && host.contains('.')
}

fn is_valid_jsonpath(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with('$') && !trimmed.contains(' ')
}

fn rest_response_preview_json() -> &'static str {
    r#"[
  {
    "title": "Wireless Headphones",
    "price": 129.99,
    "category": "Electronics",
    "available": true
  },
  {
    "title": "Bluetooth Speaker",
    "price": 59.50,
    "category": "Audio",
    "available": true
  }
]"#
}

fn rest_help_sample_json() -> &'static str {
    r#"[
  {
    "title": "Wireless Headphones",
    "price": 129.99,
    "category": "Electronics",
    "available": true
  }
]"#
}

fn clean_or_default(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn next_panel_binding_id(
    form: &Form,
    source_kind: BindingEditorSourceKind,
    target_control_id: &str,
) -> String {
    let mut normalized = String::with_capacity(target_control_id.len());
    for ch in target_control_id.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_uppercase());
        } else {
            normalized.push('-');
        }
    }
    let normalized = normalized.trim_matches('-');
    let base = format!("BIND-{}-{}", source_kind.id_fragment(), normalized);
    if !form
        .data_bindings
        .iter()
        .any(|binding| binding.id.eq_ignore_ascii_case(&base))
    {
        return base;
    }
    for n in 2.. {
        let candidate = format!("{base}-{n}");
        if !form
            .data_bindings
            .iter()
            .any(|binding| binding.id.eq_ignore_ascii_case(&candidate))
        {
            return candidate;
        }
    }
    unreachable!("unbounded suffix search must return")
}

fn show_binding_source_selector(ui: &mut Ui, editor: &mut BindingEditorState, tr: &Tr) {
    let selected_fill = Color32::from_rgba_unmultiplied(44, 111, 210, 70);
    let idle_fill = Color32::from_rgba_unmultiplied(16, 18, 22, 210);
    let selected_stroke = egui::Stroke::new(1.5, Color32::from_rgb(70, 145, 255));
    let idle_stroke = egui::Stroke::new(1.0, Color32::from_rgb(75, 80, 88));

    ui.horizontal(|ui| {
        for source_kind in DATA_BINDING_MODAL_SOURCES {
            let enabled = source_kind != BindingEditorSourceKind::IndexedFile
                || !editor.indexed_files.is_empty();
            let selected = editor.selected_source == Some(source_kind);
            let icon = match source_kind {
                BindingEditorSourceKind::IndexedFile => "[file]",
                BindingEditorSourceKind::Sql => "(db)",
                BindingEditorSourceKind::CobolTable => "[grid]",
                BindingEditorSourceKind::RestApi => "(cloud)",
                BindingEditorSourceKind::AgentAi => "(ai)",
            };
            let text_color = if selected {
                Color32::from_rgb(210, 230, 255)
            } else {
                Color32::from_rgb(205, 208, 216)
            };
            let button = egui::Button::new(
                RichText::new(format!("{icon} {}", source_kind.label(tr))).color(text_color),
            )
            .min_size(egui::vec2(170.0, 42.0))
            .fill(if selected { selected_fill } else { idle_fill })
            .stroke(if selected {
                selected_stroke
            } else {
                idle_stroke
            });
            if ui.add_enabled(enabled, button).clicked() {
                editor.select_source(source_kind);
            }
        }
    });
}

fn show_clear_selection_banner(ui: &mut Ui, editor: &mut BindingEditorState) {
    egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(78, 31, 30, 130))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(112, 58, 56)))
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(Color32::from_rgb(255, 115, 100), "!");
                ui.label("Clearing the selection will delete the previous binding configuration.");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(
                            RichText::new("Clear selection")
                                .color(Color32::from_rgb(255, 120, 110)),
                        )
                        .clicked()
                    {
                        editor.confirm_clear = true;
                    }
                });
            });
        });

    if editor.confirm_clear {
        let mut confirm_open = true;
        egui::Window::new("Clear binding configuration?")
            .collapsible(false)
            .resizable(false)
            .open(&mut confirm_open)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.label("The previous binding configuration will be deleted.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        editor.confirm_clear = false;
                    }
                    if ui
                        .button(
                            RichText::new("Confirm clear").color(Color32::from_rgb(255, 120, 110)),
                        )
                        .clicked()
                    {
                        editor.clear_selection();
                    }
                });
            });
        if !confirm_open {
            editor.confirm_clear = false;
        }
    }
}

fn show_indexed_source_section(ui: &mut Ui, editor: &mut BindingEditorState) {
    ui.heading("Indexed file source");
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label("Indexed file:");
        egui::ComboBox::from_id_salt("data_binding_indexed_file")
            .selected_text(clean_or_default(
                &editor.selected_indexed_file,
                "CUSTOMER-DATA (CUSTDAT)",
            ))
            .width(330.0)
            .show_ui(ui, |ui| {
                let files: Vec<String> = if editor.indexed_files.is_empty() {
                    vec!["CUSTOMER-DATA (CUSTDAT)".to_owned()]
                } else {
                    editor.indexed_files.clone()
                };
                for file in files {
                    ui.selectable_value(&mut editor.selected_indexed_file, file.clone(), file);
                }
            });
    });
    ui.add_space(8.0);
    ui.label(
        RichText::new("File records are rows. Browse records below to preview data.")
            .small()
            .color(Color32::GRAY),
    );
    ui.add_space(12.0);
    show_preview_pagination(ui, editor, "indexed", 42);
    ui.add_space(10.0);
    show_preview_grid(ui);
}

fn show_sql_source_section(ui: &mut Ui, editor: &mut BindingEditorState) {
    ui.heading("SQL control");
    ui.add_space(8.0);
    egui::ComboBox::from_id_salt("data_binding_sql_control")
        .selected_text(clean_or_default(
            &editor.selected_sql_control,
            "SQL-CUSTOMERS",
        ))
        .width(330.0)
        .show_ui(ui, |ui| {
            for &control in MOCK_SQL_CONTROLS {
                if ui
                    .selectable_value(
                        &mut editor.selected_sql_control,
                        control.to_owned(),
                        control,
                    )
                    .clicked()
                {
                    editor.page = 1;
                    editor.rows = sample_sql_field_rows();
                    editor.removed_rows.clear();
                }
            }
        });
    ui.add_space(8.0);
    ui.label(
        RichText::new(
            "Records are fetched from the selected SQL control result set with pagination.",
        )
        .small()
        .color(Color32::GRAY),
    );
    ui.add_space(16.0);
    show_preview_pagination(ui, editor, "sql", 24);
}

fn show_preview_pagination(
    ui: &mut Ui,
    editor: &mut BindingEditorState,
    id_suffix: &str,
    total_pages: i64,
) {
    editor.page = editor.page.clamp(1, total_pages);
    ui.horizontal(|ui| {
        if ui.button("«").clicked() {
            editor.page = 1;
        }
        if ui.button("‹").clicked() {
            editor.page = (editor.page - 1).max(1);
        }
        ui.label("Page");
        ui.add(
            DragValue::new(&mut editor.page)
                .speed(1)
                .range(1..=total_pages)
                .fixed_decimals(0),
        );
        ui.label(format!("of {total_pages}"));
        if ui.button("›").clicked() {
            editor.page = (editor.page + 1).min(total_pages);
        }
        if ui.button("»").clicked() {
            editor.page = total_pages;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let old_size = editor.page_size;
            egui::ComboBox::from_id_salt(format!("data_binding_page_size_{id_suffix}"))
                .selected_text(editor.page_size.to_string())
                .width(90.0)
                .show_ui(ui, |ui| {
                    for size in [25, 50, 100, 250] {
                        ui.selectable_value(&mut editor.page_size, size, size.to_string());
                    }
                });
            if editor.page_size != old_size {
                editor.page = 1;
            }
            ui.label("Page size:");
        });
    });
}

fn show_preview_grid(ui: &mut Ui) {
    egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(15, 18, 22, 210))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(58, 64, 72)))
        .rounding(egui::Rounding::same(5.0))
        .show(ui, |ui| {
            egui::Grid::new("data_binding_preview_grid")
                .num_columns(6)
                .spacing([34.0, 8.0])
                .striped(true)
                .show(ui, |ui| {
                    for header in [
                        "CUSTOMER-ID",
                        "CUSTOMER-NAME",
                        "BALANCE",
                        "ACTIVE",
                        "CITY-ID",
                        "STATUS",
                    ] {
                        ui.label(RichText::new(header).small().color(Color32::from_gray(190)));
                    }
                    ui.end_row();
                    for value in [
                        "100001",
                        "Acme Corporation",
                        "12500.75",
                        "Y",
                        "10",
                        "ACTIVE",
                    ] {
                        ui.label(value);
                    }
                    ui.end_row();
                });
        });
}

fn show_rest_source_section(ui: &mut Ui, editor: &mut BindingEditorState) {
    ui.heading("REST API source");
    ui.add_space(10.0);
    egui::Grid::new("rest_api_source_settings")
        .num_columns(2)
        .spacing([18.0, 10.0])
        .show(ui, |ui| {
            ui.label("API endpoint");
            ui.add(
                egui::TextEdit::singleline(&mut editor.rest_endpoint)
                    .desired_width(560.0)
                    .hint_text("https://api.example.com/v1/products"),
            );
            ui.end_row();

            ui.label("Method");
            egui::ComboBox::from_id_salt("rest_api_method")
                .selected_text(editor.rest_method.label())
                .width(150.0)
                .show_ui(ui, |ui| {
                    for method in RestMethod::ALL {
                        ui.selectable_value(&mut editor.rest_method, method, method.label());
                    }
                });
            ui.end_row();

            ui.label("Headers (optional)");
            ui.horizontal(|ui| {
                if ui.button("+ Add header").clicked() {
                    editor.rest_headers.push(HttpHeaderRow::default());
                }
            });
            ui.end_row();

            if !editor.rest_headers.is_empty() {
                ui.label("");
                ui.vertical(|ui| {
                    let mut remove_header = None;
                    for (index, header) in editor.rest_headers.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut header.name)
                                    .desired_width(190.0)
                                    .hint_text("Header name"),
                            );
                            ui.add(
                                egui::TextEdit::singleline(&mut header.value)
                                    .desired_width(250.0)
                                    .hint_text("Header value"),
                            );
                            if ui.small_button("X").clicked() {
                                remove_header = Some(index);
                            }
                        });
                    }
                    if let Some(index) = remove_header {
                        editor.rest_headers.remove(index);
                    }
                });
                ui.end_row();
            }

            ui.label("Authentication (optional)");
            egui::ComboBox::from_id_salt("rest_api_auth")
                .selected_text(editor.rest_auth.mode.label())
                .width(150.0)
                .show_ui(ui, |ui| {
                    for mode in RestAuthMode::ALL {
                        ui.selectable_value(&mut editor.rest_auth.mode, mode, mode.label());
                    }
                });
            ui.end_row();
        });

    show_rest_auth_fields(ui, &mut editor.rest_auth);
    ui.add_space(12.0);
    ui.label("API response preview");
    ui.add_space(4.0);
    show_json_code_preview(ui, rest_response_preview_json(), 220.0);
    ui.add_space(8.0);
    show_jsonpath_hint_row(ui, editor);
    ui.add_space(16.0);
    show_rest_source_fields_section(ui, editor);
    ui.add_space(12.0);
    show_rest_field_actions(ui, editor);
    if editor.show_jsonpath_help {
        ui.add_space(14.0);
        show_jsonpath_help_panel(ui);
    }
}

fn show_cobol_table_source_section(ui: &mut Ui, editor: &mut BindingEditorState) {
    ui.heading("Configure COBOL table binding");
    ui.add_space(10.0);
    ui.label(
        RichText::new("Table (01-level GLOBAL item with OCCURS)")
            .small()
            .color(Color32::GRAY),
    );
    let before_table = editor.selected_cobol_table.clone();
    egui::ComboBox::from_id_salt("cobol_binding_table")
        .selected_text(clean_or_default(&editor.selected_cobol_table, "None"))
        .width(380.0)
        .show_ui(ui, |ui| {
            if editor.cobol_tables.is_empty() {
                ui.label(RichText::new("None").color(Color32::GRAY));
            }
            for table in &editor.cobol_tables {
                ui.selectable_value(
                    &mut editor.selected_cobol_table,
                    table.name.clone(),
                    table.name.as_str(),
                );
            }
        });
    if before_table != editor.selected_cobol_table {
        let previous_rows = std::mem::take(&mut editor.rows);
        editor.rows = editor.rows_for_selected_cobol_table(&previous_rows);
        editor.removed_rows.clear();
        editor.dropdown_config_row = None;
        editor.selected_cobol_field_to_add.clear();
    }
    ui.add_space(10.0);
    // The occurs item is derived from the selected 01 automatically — a 01-level
    // table with OCCURS is enough to bind to, so we no longer ask the user for it.
    let helper = if editor.selected_cobol_table.trim().is_empty() {
        if editor.cobol_tables.is_empty() {
            "No eligible 01-level GLOBAL working-storage table with OCCURS was found.".to_owned()
        } else {
            "Select a 01-level GLOBAL working-storage table with OCCURS. Pagination is not required."
                .to_owned()
        }
    } else {
        format!(
            "Binding occurs to the COBOL table structure ({}). Pagination is not required.",
            editor.selected_cobol_table,
        )
    };
    ui.label(RichText::new(helper).small().color(Color32::GRAY));
}

fn show_rest_auth_fields(ui: &mut Ui, auth: &mut RestAuth) {
    match auth.mode {
        RestAuthMode::None => {}
        RestAuthMode::ApiKey => {
            egui::Grid::new("rest_auth_api_key")
                .num_columns(2)
                .spacing([18.0, 8.0])
                .show(ui, |ui| {
                    ui.label("");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut auth.api_key_name)
                                .desired_width(160.0)
                                .hint_text("Key name"),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut auth.api_key_value)
                                .desired_width(220.0)
                                .hint_text("Key value"),
                        );
                        egui::ComboBox::from_id_salt("rest_auth_api_key_location")
                            .selected_text(auth.api_key_location.label())
                            .width(100.0)
                            .show_ui(ui, |ui| {
                                for location in ApiKeyLocation::ALL {
                                    ui.selectable_value(
                                        &mut auth.api_key_location,
                                        location,
                                        location.label(),
                                    );
                                }
                            });
                    });
                    ui.end_row();
                });
        }
        RestAuthMode::BearerToken => {
            egui::Grid::new("rest_auth_bearer")
                .num_columns(2)
                .spacing([18.0, 8.0])
                .show(ui, |ui| {
                    ui.label("");
                    ui.add(
                        egui::TextEdit::singleline(&mut auth.bearer_token)
                            .desired_width(360.0)
                            .hint_text("Bearer token"),
                    );
                    ui.end_row();
                });
        }
        RestAuthMode::BasicAuth => {
            egui::Grid::new("rest_auth_basic")
                .num_columns(2)
                .spacing([18.0, 8.0])
                .show(ui, |ui| {
                    ui.label("");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut auth.username)
                                .desired_width(180.0)
                                .hint_text("Username"),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut auth.password)
                                .password(true)
                                .desired_width(180.0)
                                .hint_text("Password"),
                        );
                    });
                    ui.end_row();
                });
        }
    }
}

fn show_json_code_preview(ui: &mut Ui, json: &str, max_height: f32) {
    egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(7, 10, 13, 235))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(55, 65, 78)))
        .rounding(egui::Rounding::same(5.0))
        .inner_margin(egui::Margin::symmetric(8.0, 8.0))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(max_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (index, line) in json.lines().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [28.0, 16.0],
                                egui::Label::new(
                                    RichText::new(format!("{:>2}", index + 1))
                                        .monospace()
                                        .color(Color32::from_gray(120)),
                                ),
                            );
                            show_json_highlighted_line(ui, line);
                        });
                    }
                });
        });
}

fn show_json_highlighted_line(ui: &mut Ui, line: &str) {
    ui.horizontal_wrapped(|ui| {
        for token in line.split_inclusive([' ', ',', ':']) {
            let trimmed =
                token.trim_matches(|ch: char| ch == ',' || ch == ':' || ch.is_whitespace());
            let color = if trimmed.starts_with('"') {
                Color32::from_rgb(130, 220, 130)
            } else if matches!(trimmed, "true" | "false") || trimmed.parse::<f64>().is_ok() {
                Color32::from_rgb(135, 145, 255)
            } else {
                Color32::from_gray(210)
            };
            ui.label(RichText::new(token).monospace().color(color));
        }
    });
}

fn show_jsonpath_hint_row(ui: &mut Ui, editor: &mut BindingEditorState) {
    ui.horizontal(|ui| {
        ui.colored_label(Color32::from_rgb(70, 145, 255), "i");
        ui.label(
            RichText::new(
                "JSON keys are mapped to COBOL types using JSONPath syntax. Use the help panel for examples and guidance.",
            )
            .small()
            .color(Color32::GRAY),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("ⓘ JSONPath help").clicked() {
                editor.show_jsonpath_help = !editor.show_jsonpath_help;
            }
        });
    });
}

fn show_rest_source_fields_section(ui: &mut Ui, editor: &mut BindingEditorState) {
    ui.heading("Source fields");
    ui.add_space(8.0);
    egui::ScrollArea::horizontal()
        .id_salt("rest_data_binding_source_fields_scroll")
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Frame::none()
                .fill(Color32::from_rgba_unmultiplied(18, 21, 25, 215))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(50, 57, 64)))
                .rounding(egui::Rounding::same(6.0))
                .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                .show(ui, |ui| {
                    let mut remove_at = None;
                    egui::Grid::new("rest_data_binding_source_fields")
                        .num_columns(11)
                        .spacing([8.0, 7.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for header in [
                                "",
                                "",
                                "Source field / JSONPath",
                                "Detected type\nCOBOL type",
                                "COBOL mask",
                                "Required",
                                "Visible",
                                "Enabled",
                                "Friendly name",
                                "Edit control",
                                "Extra notes",
                            ] {
                                ui.label(
                                    RichText::new(header).small().color(Color32::from_gray(170)),
                                );
                            }
                            ui.end_row();

                            for row_index in 0..editor.rows.len() {
                                let row = &mut editor.rows[row_index];
                                ui.label(RichText::new("::").color(Color32::from_gray(130)));
                                if ui.small_button("X").clicked() {
                                    remove_at = Some(row_index);
                                }
                                ui.add(
                                    egui::TextEdit::singleline(&mut row.source_field)
                                        .desired_width(120.0),
                                );
                                ui.label(format!(
                                    "{}\n{}",
                                    rest_detected_type_label(&row.data_type),
                                    row.picture
                                ));
                                ui.add(
                                    egui::TextEdit::singleline(&mut row.cobol_mask)
                                        .desired_width(85.0),
                                );
                                ui.checkbox(&mut row.required, "");
                                ui.checkbox(&mut row.visible, "");
                                ui.checkbox(&mut row.enabled, "");
                                ui.add(
                                    egui::TextEdit::singleline(&mut row.friendly_name)
                                        .desired_width(95.0),
                                );
                                egui::ComboBox::from_id_salt(format!(
                                    "rest_data_binding_edit_control_{}",
                                    row_index
                                ))
                                .selected_text(row.edit_control.label())
                                .width(90.0)
                                .show_ui(ui, |ui| {
                                    for control in BindingEditControl::ALL {
                                        ui.selectable_value(
                                            &mut row.edit_control,
                                            control.clone(),
                                            control.label(),
                                        );
                                    }
                                });
                                ui.label("-");
                                ui.end_row();
                            }
                        });
                    if let Some(index) = remove_at {
                        if index < editor.rows.len() {
                            let removed = editor.rows.remove(index);
                            editor.removed_rows.push(removed);
                        }
                    }
                });
        });
}

fn show_rest_field_actions(ui: &mut Ui, editor: &mut BindingEditorState) {
    ui.horizontal(|ui| {
        if ui.button("+ Add field").clicked() {
            let mut row = BindingFieldRow::new(
                "",
                "PIC X(255)",
                BindingDataType::Unknown,
                "",
                BindingEditControl::Textbox,
                DropdownConfig::empty(),
            );
            row.cobol_mask = "X(255)".to_owned();
            row.required = false;
            row.visible = true;
            row.enabled = true;
            editor.rows.push(row);
        }
        if ui.button("Restore removed fields").clicked() {
            editor.rows.append(&mut editor.removed_rows);
        }
    });
}

fn show_cobol_table_field_actions(ui: &mut Ui, editor: &mut BindingEditorState) {
    ui.horizontal(|ui| {
        let missing_fields = editor.missing_cobol_table_fields();
        if !missing_fields.is_empty() {
            if !missing_fields
                .iter()
                .any(|field| field.name == editor.selected_cobol_field_to_add)
            {
                editor.selected_cobol_field_to_add = missing_fields[0].name.clone();
            }
            egui::ComboBox::from_id_salt("cobol_binding_add_field")
                .selected_text(clean_or_default(
                    &editor.selected_cobol_field_to_add,
                    missing_fields[0].name.as_str(),
                ))
                .width(220.0)
                .show_ui(ui, |ui| {
                    for field in &missing_fields {
                        ui.selectable_value(
                            &mut editor.selected_cobol_field_to_add,
                            field.name.clone(),
                            field.name.as_str(),
                        );
                    }
                });
            if ui.button("+ Add field").clicked() {
                if let Some(field) = missing_fields
                    .iter()
                    .find(|field| field.name == editor.selected_cobol_field_to_add)
                    .or_else(|| missing_fields.first())
                {
                    editor.rows.push(field.to_row());
                    editor.selected_cobol_field_to_add.clear();
                }
            }
        }
        if ui.button("Restore removed fields").clicked() {
            let available_names = editor
                .selected_cobol_table_meta()
                .map(|table| {
                    table
                        .fields
                        .iter()
                        .map(|field| field.name.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let mut restored = Vec::new();
            editor.removed_rows.retain(|row| {
                let can_restore = available_names
                    .iter()
                    .any(|name| row.source_field.eq_ignore_ascii_case(name))
                    && !editor
                        .rows
                        .iter()
                        .any(|active| active.source_field.eq_ignore_ascii_case(&row.source_field));
                if can_restore {
                    restored.push(row.clone());
                    false
                } else {
                    true
                }
            });
            editor.rows.append(&mut restored);
        }
    });
}

fn show_jsonpath_help_panel(ui: &mut Ui) {
    egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(14, 17, 21, 230))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(50, 57, 64)))
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            ui.heading("JSONPath help");
            ui.add_space(8.0);
            ui.columns(2, |columns| {
                columns[0].label(
                    RichText::new(
                        "Use JSONPath expressions to extract values from the JSON response. JSON keys are mapped to COBOL types based on the detected data type.",
                    )
                    .small()
                    .color(Color32::from_gray(205)),
                );
                columns[0].add_space(8.0);
                columns[0].label("Sample JSON response");
                show_json_code_preview(&mut columns[0], rest_help_sample_json(), 130.0);

                columns[1].label("Examples");
                egui::Grid::new("jsonpath_examples")
                    .num_columns(2)
                    .spacing([28.0, 6.0])
                    .striped(true)
                    .show(&mut columns[1], |ui| {
                        ui.label(RichText::new("Source field (JSONPath)").small());
                        ui.label(RichText::new("Friendly name").small());
                        ui.end_row();
                        ui.monospace("$.[*].title");
                        ui.label("Title");
                        ui.end_row();
                        ui.monospace("$.[*].price");
                        ui.label("Price");
                        ui.end_row();
                    });
                columns[1].add_space(8.0);
                columns[1].label(RichText::new("- $ : Root of the JSON document").small());
                columns[1].label(RichText::new("- [*] : All items in the root array").small());
                columns[1].label(RichText::new("- .key : Select the key from each item").small());
                columns[1].add_space(8.0);
                let _ = columns[1].button(
                    RichText::new("Learn more about JSONPath ↗")
                        .color(Color32::from_rgb(85, 160, 255)),
                );
            });
        });
}

fn show_source_fields_section(ui: &mut Ui, editor: &mut BindingEditorState) {
    ui.heading("Source fields");
    ui.add_space(8.0);
    let show_sql_type = editor.selected_source == Some(BindingEditorSourceKind::Sql);
    let show_cobol_table = editor.selected_source == Some(BindingEditorSourceKind::CobolTable);
    egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(18, 21, 25, 215))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(50, 57, 64)))
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::symmetric(8.0, 6.0))
        .show(ui, |ui| {
            let mut remove_at = None;
            egui::Grid::new("data_binding_source_fields")
                .num_columns(12)
                .spacing([10.0, 7.0])
                .striped(true)
                .show(ui, |ui| {
                    for header in [
                        "",
                        "",
                        "Source field",
                        if show_sql_type { "Type" } else { "Picture" },
                        "COBOL mask",
                        "Key",
                        "Req.",
                        "Visible",
                        "Enabled",
                        "Friendly name",
                        "Edit control",
                        "Extra configuration",
                    ] {
                        ui.label(RichText::new(header).small().color(Color32::from_gray(170)));
                    }
                    ui.end_row();

                    for row_index in 0..editor.rows.len() {
                        let row = &mut editor.rows[row_index];
                        let mut row_clicked = false;
                        let mut row_became_dropdown = false;
                        let mut row_stopped_being_dropdown = false;

                        if ui
                            .add(egui::Label::new(
                                RichText::new("::").color(Color32::from_gray(130)),
                            ))
                            .clicked()
                        {
                            row_clicked = true;
                        }
                        if ui.small_button("X").clicked() {
                            remove_at = Some(row_index);
                        }
                        if ui
                            .add(egui::Label::new(&row.source_field).sense(egui::Sense::click()))
                            .clicked()
                        {
                            row_clicked = true;
                        }
                        let detail = if show_sql_type {
                            binding_data_type_label(&row.data_type)
                        } else {
                            &row.picture
                        };
                        if ui
                            .add(egui::Label::new(detail).sense(egui::Sense::click()))
                            .clicked()
                        {
                            row_clicked = true;
                        }
                        ui.add(egui::TextEdit::singleline(&mut row.cobol_mask).desired_width(92.0));
                        ui.checkbox(&mut row.key, "");
                        ui.checkbox(&mut row.required, "");
                        ui.checkbox(&mut row.visible, "");
                        ui.checkbox(&mut row.enabled, "");
                        ui.add(
                            egui::TextEdit::singleline(&mut row.friendly_name).desired_width(112.0),
                        );
                        let before_control = row.edit_control.clone();
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt(format!(
                                "data_binding_edit_control_{}",
                                row.source_field
                            ))
                            .selected_text(row.edit_control.label())
                            .width(98.0)
                            .show_ui(ui, |ui| {
                                for control in BindingEditControl::ALL {
                                    ui.selectable_value(
                                        &mut row.edit_control,
                                        control.clone(),
                                        control.label(),
                                    );
                                }
                            });
                            if row.edit_control == BindingEditControl::Dropdown
                                && ui.small_button("Edit...").clicked()
                            {
                                row_clicked = true;
                            }
                        });
                        if before_control != row.edit_control {
                            if row.edit_control == BindingEditControl::Dropdown {
                                row.dropdown = default_dropdown_for_field(&row.source_field);
                                row_became_dropdown = true;
                            } else {
                                row_stopped_being_dropdown = true;
                            }
                        }
                        let summary = if show_cobol_table {
                            "-"
                        } else if row.edit_control == BindingEditControl::Dropdown {
                            row.dropdown.summary()
                        } else {
                            "-"
                        };
                        if ui
                            .add(egui::Label::new(summary).sense(egui::Sense::click()))
                            .clicked()
                        {
                            row_clicked = true;
                        }
                        ui.end_row();

                        let is_dropdown = row.edit_control == BindingEditControl::Dropdown;
                        if row_stopped_being_dropdown
                            && editor.dropdown_config_row == Some(row_index)
                        {
                            editor.dropdown_config_row = None;
                        }
                        if remove_at != Some(row_index)
                            && is_dropdown
                            && (row_became_dropdown || row_clicked)
                        {
                            editor.dropdown_config_row = Some(row_index);
                        }
                    }
                });

            if let Some(index) = remove_at {
                if index < editor.rows.len() {
                    let removed = editor.rows.remove(index);
                    editor.removed_rows.push(removed);
                    if let Some(open_index) = editor.dropdown_config_row {
                        editor.dropdown_config_row = if open_index == index {
                            None
                        } else if open_index > index {
                            Some(open_index - 1)
                        } else {
                            Some(open_index)
                        };
                    }
                }
            }
        });
    show_control_array_mapping_section(ui, editor);
}

/// For a repeating-group (control-array) target only: a compact "field → control"
/// map. Each source field can be assigned to one member control; the control's
/// default bindable property is shown and used on apply. Unmapped fields are
/// skipped.
fn show_control_array_mapping_section(ui: &mut Ui, editor: &mut BindingEditorState) {
    if editor.member_controls.is_empty() {
        return;
    }
    let members = editor.member_controls.clone();
    ui.add_space(16.0);
    ui.heading("Map fields to controls");
    ui.label(
        RichText::new(
            "Each mapped source field fills a control's property in every repeated item.",
        )
        .small()
        .color(Color32::GRAY),
    );
    ui.add_space(8.0);
    egui::Grid::new("data_binding_member_map")
        .num_columns(3)
        .spacing([12.0, 6.0])
        .striped(true)
        .show(ui, |ui| {
            for header in ["Source field", "Target control", "Property"] {
                ui.label(RichText::new(header).small().color(Color32::from_gray(170)));
            }
            ui.end_row();

            for row_index in 0..editor.rows.len() {
                let field_name = editor.rows[row_index].source_field.clone();
                ui.label(&field_name);

                let current = editor.rows[row_index].target_member.clone();
                let selected_text = if current.trim().is_empty() {
                    "(none)".to_owned()
                } else {
                    current.clone()
                };
                egui::ComboBox::from_id_salt(format!("member_map_{row_index}"))
                    .selected_text(selected_text)
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut editor.rows[row_index].target_member,
                            String::new(),
                            "(none)",
                        );
                        for member in &members {
                            ui.selectable_value(
                                &mut editor.rows[row_index].target_member,
                                member.id.clone(),
                                format!("{} — {}", member.id, member.type_label),
                            );
                        }
                    });

                let property = members
                    .iter()
                    .find(|member| member.id == editor.rows[row_index].target_member)
                    .map(|member| member.property.clone())
                    .unwrap_or_default();
                ui.label(
                    RichText::new(property)
                        .small()
                        .color(Color32::from_gray(150)),
                );
                ui.end_row();
            }
        });
}

fn show_dropdown_config_modal(ctx: &egui::Context, editor: &mut BindingEditorState) {
    let Some(row_index) = editor.dropdown_config_row else {
        return;
    };
    if row_index >= editor.rows.len()
        || editor.rows[row_index].edit_control != BindingEditControl::Dropdown
    {
        editor.dropdown_config_row = None;
        return;
    }

    let show_cobol_table = editor.selected_source == Some(BindingEditorSourceKind::CobolTable);
    let title = format!(
        "Dropdown configuration - {}",
        editor.rows[row_index].source_field
    );
    let mut open = true;
    egui::Window::new(title)
        .id(egui::Id::new((
            "data_binding_dropdown_config",
            &editor.target_control_id,
            row_index,
        )))
        .collapsible(false)
        .resizable(true)
        .default_size(egui::vec2(760.0, 340.0))
        .max_size(egui::vec2(860.0, 560.0))
        .open(&mut open)
        .show(ctx, |ui| {
            if show_cobol_table {
                show_cobol_table_dropdown_config_body(ui, &mut editor.rows[row_index]);
            } else {
                show_dropdown_config_body(ui, &mut editor.rows[row_index]);
            }
        });
    if !open {
        editor.dropdown_config_row = None;
    }
}

fn show_cobol_table_dropdown_config_body(ui: &mut Ui, row: &mut BindingFieldRow) {
    egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(10, 13, 16, 230))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(60, 67, 76)))
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::symmetric(14.0, 10.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new("Dropdown configuration")
                    .small()
                    .color(Color32::from_gray(190)),
            );
            ui.separator();
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Origin of data").small().color(Color32::GRAY));
                    for origin in [DropdownOrigin::CobolTable, DropdownOrigin::IndexedFile] {
                        let selected = row.dropdown.origin == Some(origin);
                        if ui.radio(selected, origin.summary()).clicked() && !selected {
                            row.dropdown = match origin {
                                DropdownOrigin::CobolTable => {
                                    DropdownConfig::category_cobol_table()
                                }
                                DropdownOrigin::IndexedFile => {
                                    DropdownConfig::region_indexed_file()
                                }
                                _ => DropdownConfig::empty(),
                            };
                        }
                    }
                });
                ui.add_space(30.0);
                ui.vertical(|ui| {
                    let origin = row.dropdown.origin.unwrap_or(DropdownOrigin::CobolTable);
                    let source_label = match origin {
                        DropdownOrigin::IndexedFile => "Select source indexed file",
                        _ => "Select source COBOL table",
                    };
                    ui.label(RichText::new(source_label).small().color(Color32::GRAY));
                    egui::ComboBox::from_id_salt(format!(
                        "cobol_dropdown_source_{}",
                        row.source_field
                    ))
                    .selected_text(clean_or_default(
                        &row.dropdown.source_ref,
                        dropdown_source_options(origin)[0],
                    ))
                    .width(235.0)
                    .show_ui(ui, |ui| {
                        for &source in dropdown_source_options(origin) {
                            ui.selectable_value(
                                &mut row.dropdown.source_ref,
                                source.to_owned(),
                                source,
                            );
                        }
                    });
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Display / name field (shown in dropdown)")
                            .small()
                            .color(Color32::GRAY),
                    );
                    egui::ComboBox::from_id_salt(format!(
                        "cobol_dropdown_display_{}",
                        row.source_field
                    ))
                    .selected_text(&row.dropdown.display_field)
                    .width(235.0)
                    .show_ui(ui, |ui| {
                        for &field in
                            dropdown_field_options(row.dropdown.origin, &row.dropdown.source_ref)
                        {
                            ui.selectable_value(
                                &mut row.dropdown.display_field,
                                field.to_owned(),
                                field,
                            );
                        }
                    });
                    ui.label(
                        RichText::new("Displayed to the user.")
                            .small()
                            .color(Color32::GRAY),
                    );
                });
                ui.add_space(24.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("ID / value field (stored as bound value)")
                            .small()
                            .color(Color32::GRAY),
                    );
                    egui::ComboBox::from_id_salt(format!(
                        "cobol_dropdown_value_{}",
                        row.source_field
                    ))
                    .selected_text(&row.dropdown.value_field)
                    .width(235.0)
                    .show_ui(ui, |ui| {
                        for &field in
                            dropdown_field_options(row.dropdown.origin, &row.dropdown.source_ref)
                        {
                            ui.selectable_value(
                                &mut row.dropdown.value_field,
                                field.to_owned(),
                                field,
                            );
                        }
                    });
                    ui.label(
                        RichText::new("Stored as the selected value.")
                            .small()
                            .color(Color32::GRAY),
                    );
                    ui.add_space(8.0);
                    ui.label(RichText::new("Line limit").small().color(Color32::GRAY));
                    egui::ComboBox::from_id_salt(format!(
                        "cobol_dropdown_line_limit_{}",
                        row.source_field
                    ))
                    .selected_text(row.dropdown.line_limit.to_string())
                    .width(92.0)
                    .show_ui(ui, |ui| {
                        for limit in [100, 500, 1000, 5000, 10000] {
                            ui.selectable_value(
                                &mut row.dropdown.line_limit,
                                limit,
                                limit.to_string(),
                            );
                        }
                    });
                });
            });
        });
}

fn show_dropdown_config_body(ui: &mut Ui, row: &mut BindingFieldRow) {
    egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(10, 13, 16, 230))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(60, 67, 76)))
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new("Dropdown configuration")
                    .small()
                    .color(Color32::from_gray(190)),
            );
            ui.separator();
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Origin of data").small().color(Color32::GRAY));
                    egui::ComboBox::from_id_salt(format!("dropdown_origin_{}", row.source_field))
                        .selected_text(
                            row.dropdown
                                .origin
                                .map(|origin| origin.label())
                                .unwrap_or("Select origin"),
                        )
                        .width(220.0)
                        .show_ui(ui, |ui| {
                            for origin in DropdownOrigin::ALL {
                                if ui
                                    .selectable_value(
                                        &mut row.dropdown.origin,
                                        Some(origin),
                                        origin.label(),
                                    )
                                    .clicked()
                                {
                                    row.dropdown.reset_for_origin(origin);
                                }
                            }
                        });
                });
                ui.add_space(36.0);
                if let Some(origin) = row.dropdown.origin {
                    if let Some(source_label) = origin.source_label() {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(source_label).small().color(Color32::GRAY));
                            egui::ComboBox::from_id_salt(format!(
                                "dropdown_source_{}",
                                row.source_field
                            ))
                            .selected_text(clean_or_default(
                                &row.dropdown.source_ref,
                                dropdown_source_options(origin)[0],
                            ))
                            .width(350.0)
                            .show_ui(ui, |ui| {
                                for &source in dropdown_source_options(origin) {
                                    ui.selectable_value(
                                        &mut row.dropdown.source_ref,
                                        source.to_owned(),
                                        source,
                                    );
                                }
                            });
                        });
                    } else {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new("Static value editor")
                                    .small()
                                    .color(Color32::GRAY),
                            );
                            ui.add(
                                egui::TextEdit::multiline(&mut row.dropdown.static_options)
                                    .desired_rows(2)
                                    .desired_width(350.0)
                                    .hint_text("CODE=Label"),
                            );
                        });
                    }
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Display / name field (shown in dropdown)")
                            .small()
                            .color(Color32::GRAY),
                    );
                    egui::ComboBox::from_id_salt(format!("dropdown_display_{}", row.source_field))
                        .selected_text(&row.dropdown.display_field)
                        .width(320.0)
                        .show_ui(ui, |ui| {
                            for &field in dropdown_field_options(
                                row.dropdown.origin,
                                &row.dropdown.source_ref,
                            ) {
                                ui.selectable_value(
                                    &mut row.dropdown.display_field,
                                    field.to_owned(),
                                    field,
                                );
                            }
                        });
                    ui.label(
                        RichText::new(format!(
                            "{} will be displayed in the dropdown list.",
                            field_name_only(&row.dropdown.display_field)
                        ))
                        .small()
                        .color(Color32::GRAY),
                    );
                });
                ui.add_space(20.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("ID / value field (stored as option value)")
                            .small()
                            .color(Color32::GRAY),
                    );
                    egui::ComboBox::from_id_salt(format!("dropdown_value_{}", row.source_field))
                        .selected_text(&row.dropdown.value_field)
                        .width(320.0)
                        .show_ui(ui, |ui| {
                            for &field in dropdown_field_options(
                                row.dropdown.origin,
                                &row.dropdown.source_ref,
                            ) {
                                ui.selectable_value(
                                    &mut row.dropdown.value_field,
                                    field.to_owned(),
                                    field,
                                );
                            }
                        });
                    ui.label(
                        RichText::new(format!(
                            "{} will be stored as the selected value.",
                            field_name_only(&row.dropdown.value_field)
                        ))
                        .small()
                        .color(Color32::GRAY),
                    );
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Line limit").small().color(Color32::GRAY));
                    egui::ComboBox::from_id_salt(format!(
                        "dropdown_line_limit_{}",
                        row.source_field
                    ))
                    .selected_text(row.dropdown.line_limit.to_string())
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        for limit in [100, 500, 1000, 5000, 10000] {
                            ui.selectable_value(
                                &mut row.dropdown.line_limit,
                                limit,
                                limit.to_string(),
                            );
                        }
                    });
                });
                ui.add_space(10.0);
                ui.label(
                    RichText::new("Maximum number of items to retrieve.")
                        .small()
                        .color(Color32::GRAY),
                );
            });
        });
}

fn default_dropdown_for_field(source_field: &str) -> DropdownConfig {
    match source_field {
        "COUNTRY_ID" => DropdownConfig::country_sql(),
        "TIER_ID" => DropdownConfig::tier_cobol_table(),
        "CITY-ID" => DropdownConfig::city(),
        "STATUS" => DropdownConfig::status(),
        "CATEGORY-ID" => DropdownConfig::category_cobol_table(),
        "REGION-ID" => DropdownConfig::region_indexed_file(),
        _ => DropdownConfig::empty(),
    }
}

fn dropdown_source_options(origin: DropdownOrigin) -> &'static [&'static str] {
    match origin {
        DropdownOrigin::IndexedFile => MOCK_INDEXED_LOOKUPS,
        DropdownOrigin::SqlControl => MOCK_SQL_CONTROLS,
        DropdownOrigin::CobolTable => MOCK_COBOL_TABLES,
        DropdownOrigin::RestApi => MOCK_REST_CONTROLS,
        DropdownOrigin::StaticValues => &[""],
    }
}

fn dropdown_field_options(
    origin: Option<DropdownOrigin>,
    source_ref: &str,
) -> &'static [&'static str] {
    match (origin, source_ref) {
        (Some(DropdownOrigin::SqlControl), "SQL-COUNTRIES") => &[
            "COUNTRY_ID (9(5))",
            "COUNTRY_NAME (X(100))",
            "COUNTRY_CODE (X(2))",
        ],
        (Some(DropdownOrigin::CobolTable), "CUSTOMER_TIERS") => &[
            "TIER_ID (9(2))",
            "TIER_NAME (X(30))",
            "DISCOUNT_RATE (9(3)V99)",
        ],
        (Some(DropdownOrigin::CobolTable), "WS-CATEGORY-TABLE") => {
            &["CATEGORY-ID (9(04))", "CATEGORY-NAME (X(30))"]
        }
        (Some(DropdownOrigin::IndexedFile), "REGION-CODES (REGIONST)") => {
            &["REGION-ID (9(04))", "REGION-NAME (X(30))"]
        }
        (Some(DropdownOrigin::IndexedFile), _) => &[
            "COUNTRY_ID (9(5))",
            "COUNTRY_NAME (X(100))",
            "COUNTRY_CODE (X(2))",
        ],
        (Some(DropdownOrigin::RestApi), _) => &["ID (9(9))", "NAME (X(100))"],
        _ => &["VALUE (X(30))", "LABEL (X(100))"],
    }
}

fn field_name_only(field: &str) -> &str {
    field.split_whitespace().next().unwrap_or("Field")
}

fn source_placeholder(ui: &mut Ui, message: &str) {
    egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(18, 21, 25, 215))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(50, 57, 64)))
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::symmetric(14.0, 12.0))
        .show(ui, |ui| {
            ui.label(RichText::new(message).color(Color32::GRAY));
        });
}

// ── Panel ─────────────────────────────────────────────────────────────────────

pub struct PropertiesPanel {
    text_bufs: std::collections::HashMap<String, String>,
    form_bufs: std::collections::HashMap<String, String>,
    /// animation editor state per control: selected animation index
    anim_sel: std::collections::HashMap<String, usize>,
    /// new-animation staging fields
    new_anim_name: String,
    binding_editor: Option<BindingEditorState>,
    datagrid_editor: Option<String>,
}

impl PropertiesPanel {
    pub fn new() -> Self {
        Self {
            text_bufs: Default::default(),
            form_bufs: Default::default(),
            anim_sel: Default::default(),
            new_anim_name: String::new(),
            binding_editor: None,
            datagrid_editor: None,
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        form: &Form,
        ctrl: Option<&Control>,
        indexed_files: &[String],
        tr: &Tr,
    ) -> InspectorAction {
        let mut action = InspectorAction::default();
        // Pin the inspector to the panel's *current* width (both min and max). egui's
        // SidePanel records the content's resulting rect width as the panel width every
        // frame, so otherwise the pane would shrink to fit a control with few/short
        // properties and grow for one with long values — i.e. resize itself on every
        // selection. Forcing the content to fill exactly the available width keeps the
        // recorded width constant; resizing stays the developer's job (drag the edge).
        let panel_w = ui.available_width();
        ui.set_width(panel_w);
        // `both()` (not just vertical): the pane stays bounded to its own width, but
        // if a single control's properties are genuinely wider than the pane (e.g. a
        // long label beside a fixed-width combo at a narrow pane), the overflow becomes
        // horizontally scrollable instead of bleeding past the window border or being
        // clipped out of reach. Normal content still fits and never shows an h-bar.
        ScrollArea::both()
            .id_salt("properties_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Clamp the scroll content to the (finite) panel width. A vertical
                // ScrollArea can report an unbounded content width, which lets
                // `desired_width(f32::INFINITY)` fields inside Grids (Advanced,
                // Events, Animations, …) expand past the panel and drag the SidePanel
                // wider. `.min(panel_w)` keeps every section within the border while
                // still tracking the pane as the developer resizes it.
                ui.set_max_width(ui.available_width().min(panel_w));
                if let Some(ctrl) = ctrl {
                    self.show_control(ui, form, ctrl, indexed_files, &mut action, tr);
                } else {
                    self.show_form(ui, form, &mut action, tr);
                }
            });
        if let Some(ctrl) = ctrl {
            if ctrl.control_type == ControlType::DataGrid {
                self.show_datagrid_editor_modal(ui.ctx(), ctrl, &mut action);
            }
        }
        action
    }

    // ── Control inspector ─────────────────────────────────────────────────────

    fn show_control(
        &mut self,
        ui: &mut Ui,
        form: &Form,
        ctrl: &Control,
        indexed_files: &[String],
        action: &mut InspectorAction,
        tr: &Tr,
    ) {
        let id = ctrl.id.clone();

        // ── Identity ──────────────────────────────────────────────────────────
        // The control id is editable: rename it to a meaningful name (unique,
        // valid identifier) and every reference is updated form-wide.
        ui.horizontal(|ui| {
            let buf_key = format!("rename::{id}");
            let wid = egui::Id::new(&buf_key);
            let editing = ui.memory(|m| m.has_focus(wid));
            let buf = self
                .text_bufs
                .entry(buf_key.clone())
                .or_insert_with(|| id.clone());
            if !editing && *buf != id {
                *buf = id.clone();
            }
            let resp = ui.add(
                egui::TextEdit::singleline(buf)
                    .id(wid)
                    .desired_width(160.0)
                    .font(egui::TextStyle::Monospace),
            );
            if resp.lost_focus() {
                let new = buf.trim().to_owned();
                let unique = !form
                    .controls
                    .iter()
                    .any(|c| c.id.eq_ignore_ascii_case(&new) && !c.id.eq_ignore_ascii_case(&id));
                if new != id && cobolt_forms::model::is_valid_control_id(&new) && unique {
                    action.rename_control = Some((id.clone(), new));
                } else {
                    *buf = id.clone();
                }
            }
            ui.label(
                RichText::new(format!("[{}]", ctrl.control_type.as_str()))
                    .color(Color32::GRAY)
                    .small(),
            );
        });
        // LabelFor association
        {
            let cur = ctrl
                .get_prop("LabelFor")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_default();
            text_row_hint(
                ui,
                &mut self.text_bufs,
                &id,
                "LabelFor",
                &cur,
                tr.lbl_label_for,
                "LBL-1 (auto-arrange)",
                action,
            );
        }
        ui.separator();

        // ── Geometry ──────────────────────────────────────────────────────────
        section_header(ui, tr.sec_geometry);
        egui::Grid::new(format!("geo_{id}"))
            .num_columns(4)
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                ui.label("X:");
                let mut x = ctrl.rect.x;
                if ui.add(DragValue::new(&mut x).speed(1)).changed() {
                    action
                        .set_props
                        .push((id.clone(), "X".into(), PropValue::Int(x as i64)));
                }
                ui.label("W:");
                let mut w = ctrl.rect.w;
                if ui
                    .add(DragValue::new(&mut w).speed(1).range(1..=9999))
                    .changed()
                {
                    action
                        .set_props
                        .push((id.clone(), "Width".into(), PropValue::Int(w as i64)));
                }
                ui.end_row();

                ui.label("Y:");
                let mut y = ctrl.rect.y;
                if ui.add(DragValue::new(&mut y).speed(1)).changed() {
                    action
                        .set_props
                        .push((id.clone(), "Y".into(), PropValue::Int(y as i64)));
                }
                ui.label("H:");
                let mut h = ctrl.rect.h;
                if ui
                    .add(DragValue::new(&mut h).speed(1).range(1..=9999))
                    .changed()
                {
                    action
                        .set_props
                        .push((id.clone(), "Height".into(), PropValue::Int(h as i64)));
                }
                ui.end_row();

                ui.label("Z:");
                let mut z = ctrl.z_order as i64;
                if ui
                    .add(
                        DragValue::new(&mut z)
                            .speed(1)
                            .prefix("z=")
                            .range(-9999..=9999),
                    )
                    .changed()
                {
                    action
                        .set_props
                        .push((id.clone(), "ZOrder".into(), PropValue::Int(z)));
                }
                ui.label(RichText::new("(z-order)").small().color(Color32::GRAY));
                ui.end_row();
            });
        if ctrl.get_prop("CornerRadius").is_some() {
            int_row_inline(
                ui,
                &id,
                "CornerRadius",
                "Corner radius",
                ctrl,
                action,
                0..=400,
            );
        }
        ui.add_space(4.0);

        // Non-visual controls (Timer, AgentObject, RestClient, SqlDatabase) only
        // show geometry + type-specific settings + events — no style, no animations.
        if ctrl.control_type.is_non_visual() {
            self.show_type_specific(ui, ctrl, &id, action, tr);
            Self::show_events(ui, ctrl, &id, action, tr);
            return;
        }

        // ── Appearance ────────────────────────────────────────────────────────
        section_header(ui, tr.sec_appearance);

        egui::Grid::new(format!("appearance_{id}"))
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                // Caption (controls with an intrinsic text label) / Text (TextBox only)
                let text_key: Option<&str> = match ctrl.control_type {
                    ControlType::Label
                    | ControlType::Button
                    | ControlType::CheckBox
                    | ControlType::RadioButton
                    | ControlType::GroupBox => Some("Caption"),
                    ControlType::TextBox => Some("Text"),
                    _ => None,
                };
                if let Some(cap_key) = text_key {
                    let cur = ctrl
                        .get_prop(cap_key)
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    ui.label(cap_key);
                    {
                        let buf_key = format!("{id}-{cap_key}");
                        let wid = egui::Id::new(&buf_key);
                        let buf = self.text_bufs.entry(buf_key).or_insert_with(|| cur.clone());
                        if *buf != cur && !ui.memory(|m| m.has_focus(wid)) {
                            *buf = cur.clone();
                        }
                        if ui
                            .add(
                                egui::TextEdit::singleline(buf)
                                    .id(wid)
                                    .desired_width(f32::INFINITY),
                            )
                            .lost_focus()
                        {
                            action.set_props.push((
                                id.clone(),
                                cap_key.into(),
                                PropValue::String(buf.clone()),
                            ));
                        }
                    }
                    ui.end_row();
                }

                // BackColor
                if ctrl.get_prop("BackgroundColor").is_some() {
                    let hex = ctrl
                        .get_prop("BackgroundColor")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_else(|| "#F0F0F0".into());
                    let mut color = hex_to_color32(&hex);
                    ui.label(tr.lbl_back_color);
                    ui.horizontal(|ui| {
                        if color_edit_button_closing(ui, &mut color).changed() {
                            action.set_props.push((
                                id.clone(),
                                "BackgroundColor".into(),
                                PropValue::String(color32_to_hex(color)),
                            ));
                        }
                        ui.label(
                            RichText::new(color32_to_hex(color))
                                .monospace()
                                .small()
                                .color(Color32::GRAY),
                        );
                    });
                    ui.end_row();
                }

                // ForeColor
                if ctrl.get_prop("ForegroundColor").is_some() {
                    let hex = ctrl
                        .get_prop("ForegroundColor")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_else(|| "#000000".into());
                    let mut color = hex_to_color32(&hex);
                    ui.label(tr.lbl_fore_color);
                    ui.horizontal(|ui| {
                        if color_edit_button_closing(ui, &mut color).changed() {
                            action.set_props.push((
                                id.clone(),
                                "ForegroundColor".into(),
                                PropValue::String(color32_to_hex(color)),
                            ));
                        }
                        ui.label(
                            RichText::new(color32_to_hex(color))
                                .monospace()
                                .small()
                                .color(Color32::GRAY),
                        );
                    });
                    ui.end_row();
                }

                // Font name — dropdown of installed system fonts (fallback: Arial).
                ui.label(tr.lbl_font);
                {
                    let cur = ctrl
                        .get_prop("FontName")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_else(|| "Arial".into());
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
                                ui.selectable_value(
                                    &mut sel,
                                    cur.clone(),
                                    format!("{cur}  (not installed)"),
                                );
                            }
                            // Virtualised list: only visible rows are laid out, so only the
                            // fonts you actually scroll past get loaded into egui (loading all
                            // ~400 system fonts up-front would be far too costly).
                            let row_h = ui.text_style_height(&egui::TextStyle::Button);
                            egui::ScrollArea::vertical().max_height(320.0).show_rows(
                                ui,
                                row_h,
                                fonts.len(),
                                |ui, range| {
                                    for i in range {
                                        let fam = &fonts[i];
                                        let fid = crate::fonts::font_id(ui.ctx(), fam, 14.0);
                                        ui.selectable_value(
                                            &mut sel,
                                            fam.clone(),
                                            egui::RichText::new(fam).font(fid),
                                        );
                                    }
                                },
                            );
                        });
                    if sel != cur {
                        action.set_props.push((
                            id.clone(),
                            "FontName".into(),
                            PropValue::String(sel),
                        ));
                    }
                }
                ui.end_row();

                // Font size
                ui.label(tr.lbl_font_size);
                {
                    let mut fs = ctrl.get_prop("FontSize").map(|v| v.as_i64()).unwrap_or(10);
                    if ui
                        .add(DragValue::new(&mut fs).speed(0.5).range(4..=200))
                        .changed()
                    {
                        action
                            .set_props
                            .push((id.clone(), "FontSize".into(), PropValue::Int(fs)));
                    }
                }
                ui.end_row();

                // Font style
                ui.label(tr.lbl_style);
                ui.horizontal(|ui| {
                    let mut bold = ctrl.get_prop("Bold").map(|v| v.as_bool()).unwrap_or(false);
                    if ui.checkbox(&mut bold, "B").changed() {
                        action
                            .set_props
                            .push((id.clone(), "Bold".into(), PropValue::Bool(bold)));
                    }
                    let mut italic = ctrl
                        .get_prop("Italic")
                        .map(|v| v.as_bool())
                        .unwrap_or(false);
                    if ui.checkbox(&mut italic, "I").changed() {
                        action.set_props.push((
                            id.clone(),
                            "Italic".into(),
                            PropValue::Bool(italic),
                        ));
                    }
                    let mut under = ctrl
                        .get_prop("Underline")
                        .map(|v| v.as_bool())
                        .unwrap_or(false);
                    if ui.checkbox(&mut under, "U").changed() {
                        action.set_props.push((
                            id.clone(),
                            "Underline".into(),
                            PropValue::Bool(under),
                        ));
                    }
                    let mut strike = ctrl
                        .get_prop("Strikethrough")
                        .map(|v| v.as_bool())
                        .unwrap_or(false);
                    if ui.checkbox(&mut strike, "S̶").changed() {
                        action.set_props.push((
                            id.clone(),
                            "Strikethrough".into(),
                            PropValue::Bool(strike),
                        ));
                    }
                });
                ui.end_row();

                // Visible / Enabled
                ui.label(tr.lbl_visible);
                {
                    let mut vis = ctrl.visible;
                    if ui.checkbox(&mut vis, "").changed() {
                        action
                            .set_props
                            .push((id.clone(), "Visible".into(), PropValue::Bool(vis)));
                    }
                }
                ui.end_row();

                ui.label(tr.lbl_enabled);
                {
                    let mut ena = ctrl.enabled;
                    if ui.checkbox(&mut ena, "").changed() {
                        action
                            .set_props
                            .push((id.clone(), "Enabled".into(), PropValue::Bool(ena)));
                    }
                }
                ui.end_row();

                // Tab order
                ui.label(tr.lbl_tab_order);
                {
                    let mut to = ctrl.tab_order as i64;
                    if ui
                        .add(DragValue::new(&mut to).speed(1).range(0..=999))
                        .changed()
                    {
                        action
                            .set_props
                            .push((id.clone(), "TabOrder".into(), PropValue::Int(to)));
                    }
                }
                ui.end_row();

                // Opacity
                if let Some(op) = ctrl.get_prop("Opacity") {
                    ui.label(tr.lbl_opacity);
                    let mut v = op.as_i64();
                    if ui
                        .add(DragValue::new(&mut v).speed(1).range(0..=100).suffix("%"))
                        .changed()
                    {
                        action
                            .set_props
                            .push((id.clone(), "Opacity".into(), PropValue::Int(v)));
                    }
                    ui.end_row();
                }
            });

        // ── Drop Shadow ───────────────────────────────────────────────────────
        section_header(ui, tr.sec_shadow);
        egui::Grid::new(format!("shadow_{id}"))
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                // Enabled toggle
                ui.label(tr.lbl_shadow_enabled);
                {
                    let mut on = ctrl
                        .get_prop("ShadowEnabled")
                        .map(|v| v.as_bool())
                        .unwrap_or(false);
                    if ui.checkbox(&mut on, "").changed() {
                        action.set_props.push((
                            id.clone(),
                            "ShadowEnabled".into(),
                            PropValue::Bool(on),
                        ));
                    }
                }
                ui.end_row();

                // Opacity
                ui.label(tr.lbl_shadow_opacity);
                {
                    let mut op = ctrl
                        .get_prop("ShadowOpacity")
                        .map(|v| v.as_i64())
                        .unwrap_or(20);
                    if ui
                        .add(DragValue::new(&mut op).speed(1).range(0..=100).suffix("%"))
                        .changed()
                    {
                        action.set_props.push((
                            id.clone(),
                            "ShadowOpacity".into(),
                            PropValue::Int(op),
                        ));
                    }
                }
                ui.end_row();

                // Color
                ui.label(tr.lbl_shadow_color);
                {
                    let hex = ctrl
                        .get_prop("ShadowColor")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_else(|| "#000000".into());
                    let mut color = hex_to_color32(&hex);
                    ui.horizontal(|ui| {
                        if color_edit_button_closing(ui, &mut color).changed() {
                            action.set_props.push((
                                id.clone(),
                                "ShadowColor".into(),
                                PropValue::String(color32_to_hex(color)),
                            ));
                        }
                        ui.label(
                            RichText::new(color32_to_hex(color))
                                .monospace()
                                .small()
                                .color(Color32::GRAY),
                        );
                    });
                }
                ui.end_row();

                // Direction
                ui.label(tr.lbl_shadow_direction);
                {
                    let cur_dir = ctrl
                        .get_prop("ShadowDirection")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_else(|| "South".into());
                    egui::ComboBox::from_id_salt(format!("shadow_dir_{id}"))
                        .selected_text(&cur_dir)
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for opt in &[
                                "North",
                                "NorthEast",
                                "East",
                                "SouthEast",
                                "South",
                                "SouthWest",
                                "West",
                                "NorthWest",
                            ] {
                                if ui.selectable_label(cur_dir == *opt, *opt).clicked() {
                                    action.set_props.push((
                                        id.clone(),
                                        "ShadowDirection".into(),
                                        PropValue::String(opt.to_string()),
                                    ));
                                }
                            }
                        });
                }
                ui.end_row();

                // Distance
                ui.label(tr.lbl_shadow_distance);
                {
                    let mut dist = ctrl
                        .get_prop("ShadowDistance")
                        .map(|v| v.as_i64())
                        .unwrap_or(7);
                    if ui
                        .add(
                            DragValue::new(&mut dist)
                                .speed(1)
                                .range(0..=60)
                                .suffix("px"),
                        )
                        .changed()
                    {
                        action.set_props.push((
                            id.clone(),
                            "ShadowDistance".into(),
                            PropValue::Int(dist),
                        ));
                    }
                }
                ui.end_row();

                // Blur enabled
                ui.label(tr.lbl_shadow_blur);
                {
                    let mut blur_on = ctrl
                        .get_prop("ShadowBlur")
                        .map(|v| v.as_bool())
                        .unwrap_or(true);
                    if ui.checkbox(&mut blur_on, "").changed() {
                        action.set_props.push((
                            id.clone(),
                            "ShadowBlur".into(),
                            PropValue::Bool(blur_on),
                        ));
                    }
                }
                ui.end_row();

                // Blur strength
                ui.label(tr.lbl_shadow_blur_strength);
                {
                    let mut bs = ctrl
                        .get_prop("ShadowBlurStrength")
                        .map(|v| v.as_i64())
                        .unwrap_or(8);
                    if ui
                        .add(DragValue::new(&mut bs).speed(1).range(0..=20))
                        .changed()
                    {
                        action.set_props.push((
                            id.clone(),
                            "ShadowBlurStrength".into(),
                            PropValue::Int(bs),
                        ));
                    }
                }
                ui.end_row();
            });
        ui.add_space(4.0);

        // ── Layout ────────────────────────────────────────────────────────────
        section_header(ui, tr.sec_layout);
        egui::Grid::new(format!("layout_{id}"))
            .num_columns(2)
            .spacing([4.0, 3.0])
            .show(ui, |ui| {
                // Dock
                let cur_dock = ctrl
                    .get_prop("Dock")
                    .map(|v| v.as_str().to_owned())
                    .unwrap_or_else(|| "None".into());
                ui.label(tr.lbl_dock);
                egui::ComboBox::from_id_salt(format!("dock_{id}"))
                    .selected_text(&cur_dock)
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        for opt in &["None", "Top", "Bottom", "Left", "Right", "Fill"] {
                            if ui.selectable_label(cur_dock == *opt, *opt).clicked() {
                                action.set_props.push((
                                    id.clone(),
                                    "Dock".into(),
                                    PropValue::String(opt.to_string()),
                                ));
                            }
                        }
                    });
                ui.end_row();

                // Anchor
                let cur_anc = ctrl
                    .get_prop("Anchor")
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
                    if ui
                        .add(
                            egui::TextEdit::singleline(buf)
                                .id(wid)
                                .hint_text("Top,Left")
                                .desired_width(120.0),
                        )
                        .lost_focus()
                    {
                        action.set_props.push((
                            id.clone(),
                            "Anchor".into(),
                            PropValue::String(buf.clone()),
                        ));
                    }
                }
                ui.end_row();

                // Padding
                ui.label(tr.lbl_padding);
                let mut pad = ctrl.get_prop("Padding").map(|v| v.as_i64()).unwrap_or(0);
                if ui
                    .add(DragValue::new(&mut pad).speed(1).range(0..=200))
                    .changed()
                {
                    action
                        .set_props
                        .push((id.clone(), "Padding".into(), PropValue::Int(pad)));
                }
                ui.end_row();
            });
        ui.add_space(4.0);

        match visibility_for_control(form, ctrl) {
            DataBindingVisibility::Hidden => {}
            DataBindingVisibility::ApprovedTarget(_) => {
                section_header(ui, tr.sec_data_binding);
                ui.label(
                    RichText::new(tr.data_binding_target_ready)
                        .color(Color32::GRAY)
                        .small()
                        .italics(),
                );
                ui.add_space(2.0);
                // If this control is already bound, offer to edit the saved
                // configuration (pre-filled) instead of forcing a fresh setup.
                let existing = form.binding_for_control(&ctrl.id).cloned();
                if let Some(binding) = &existing {
                    if ui
                        .button(format!("✎ Edit current binding ({})", binding.display_name))
                        .on_hover_text("Reopen this control's saved binding configuration")
                        .clicked()
                    {
                        self.binding_editor =
                            BindingEditorState::from_existing(form, ctrl, binding, indexed_files);
                    }
                    ui.add_space(2.0);
                }
                ui.label(
                    RichText::new(tr.data_binding_choose_source)
                        .color(Color32::GRAY)
                        .small(),
                );
                ui.horizontal_wrapped(|ui| {
                    for source_kind in DATA_BINDING_MODAL_SOURCES {
                        let enabled = source_kind != BindingEditorSourceKind::IndexedFile
                            || !indexed_files.is_empty();
                        if ui
                            .add_enabled(enabled, egui::Button::new(source_kind.label(tr)))
                            .clicked()
                        {
                            // Re-selecting the SAME source as the saved binding
                            // pre-fills from it; a different source starts fresh.
                            self.binding_editor = existing
                                .as_ref()
                                .filter(|b| {
                                    matches!(
                                        (&b.source, source_kind),
                                        (cobolt_forms::BindingSourceDescriptor::IndexedFile { .. }, BindingEditorSourceKind::IndexedFile)
                                        | (cobolt_forms::BindingSourceDescriptor::Sql { .. }, BindingEditorSourceKind::Sql)
                                        | (cobolt_forms::BindingSourceDescriptor::CobolTable { .. }, BindingEditorSourceKind::CobolTable)
                                        | (cobolt_forms::BindingSourceDescriptor::RestApi { .. }, BindingEditorSourceKind::RestApi)
                                        | (cobolt_forms::BindingSourceDescriptor::AgentAi { .. }, BindingEditorSourceKind::AgentAi)
                                    )
                                })
                                .and_then(|b| {
                                    BindingEditorState::from_existing(form, ctrl, b, indexed_files)
                                })
                                .or_else(|| {
                                    BindingEditorState::new(form, ctrl, source_kind, indexed_files)
                                });
                        }
                    }
                });
                self.show_binding_editor(ui, form, ctrl, indexed_files, action, tr);
                ui.add_space(4.0);
            }
            DataBindingVisibility::ArrayMemberMapping { .. } => {
                section_header(ui, tr.sec_data_binding);
                ui.label(
                    RichText::new(tr.data_binding_array_member)
                        .color(Color32::GRAY)
                        .small()
                        .italics(),
                );
                ui.add_space(4.0);
            }
        }

        // ── Type-specific ─────────────────────────────────────────────────────
        self.show_type_specific(ui, ctrl, &id, action, tr);

        // ── Deployed User Control child properties ───────────────────────────
        self.show_user_control_children(ui, form, ctrl, action, tr);

        // ── Advanced ──────────────────────────────────────────────────────────
        section_header(ui, tr.sec_advanced);
        egui::Grid::new(format!("adv_{id}"))
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                // Tooltip
                ui.label(tr.lbl_tooltip_lbl);
                {
                    let cur = ctrl
                        .get_prop("Tooltip")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    let buf_key = format!("{id}-Tooltip");
                    let wid = egui::Id::new(&buf_key);
                    let buf = self.text_bufs.entry(buf_key).or_insert_with(|| cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) {
                        *buf = cur;
                    }
                    if ui
                        .add(
                            egui::TextEdit::singleline(buf)
                                .id(wid)
                                .hint_text("(shown on hover)")
                                .desired_width(f32::INFINITY),
                        )
                        .lost_focus()
                    {
                        action.set_props.push((
                            id.clone(),
                            "Tooltip".into(),
                            PropValue::String(buf.clone()),
                        ));
                    }
                }
                ui.end_row();

                // Cursor
                ui.label(tr.lbl_cursor_lbl);
                {
                    let cur_cursor = ctrl
                        .get_prop("Cursor")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_else(|| "Default".into());
                    egui::ComboBox::from_id_salt(format!("cur_{id}"))
                        .selected_text(&cur_cursor)
                        .width(130.0)
                        .show_ui(ui, |ui| {
                            for opt in &[
                                "Default",
                                "Hand",
                                "Text",
                                "Wait",
                                "Crosshair",
                                "No",
                                "SizeAll",
                                "SizeNS",
                                "SizeWE",
                                "Help",
                            ] {
                                if ui.selectable_label(cur_cursor == *opt, *opt).clicked() {
                                    action.set_props.push((
                                        id.clone(),
                                        "Cursor".into(),
                                        PropValue::String(opt.to_string()),
                                    ));
                                }
                            }
                        });
                }
                ui.end_row();
            });
        ui.add_space(4.0);

        // ── Events ────────────────────────────────────────────────────────────
        Self::show_events(ui, ctrl, &id, action, tr);

        // ── Animations ────────────────────────────────────────────────────────
        self.show_animations(ui, ctrl, &id, action, tr);
    }

    fn show_user_control_children(
        &mut self,
        ui: &mut Ui,
        form: &Form,
        ctrl: &Control,
        action: &mut InspectorAction,
        tr: &Tr,
    ) {
        let user_control_name = ctrl
            .get_prop("UserControl")
            .map(|v| v.as_str().trim())
            .unwrap_or_default();
        if user_control_name.is_empty() {
            return;
        }

        let children = user_control_child_controls(form, &ctrl.id);
        if children.is_empty() {
            return;
        }

        egui::CollapsingHeader::new(tr.uc_child_controls)
            .id_salt(format!("uc_child_controls_{}", ctrl.id))
            .default_open(true)
            .show(ui, |ui| {
                for child in children {
                    egui::CollapsingHeader::new(format!(
                        "{} [{}]",
                        child.id,
                        child.control_type.as_str()
                    ))
                    .id_salt(format!("uc_child_{}", child.id))
                    .default_open(false)
                    .show(ui, |ui| {
                        egui::Grid::new(format!("uc_child_props_{}", child.id))
                            .num_columns(2)
                            .spacing([8.0, 4.0])
                            .show(ui, |ui| {
                                child_prop_row(
                                    ui,
                                    &mut self.text_bufs,
                                    &child.id,
                                    "X",
                                    &PropValue::Int(child.rect.x as i64),
                                    action,
                                );
                                child_prop_row(
                                    ui,
                                    &mut self.text_bufs,
                                    &child.id,
                                    "Y",
                                    &PropValue::Int(child.rect.y as i64),
                                    action,
                                );
                                child_prop_row(
                                    ui,
                                    &mut self.text_bufs,
                                    &child.id,
                                    "Width",
                                    &PropValue::Int(child.rect.w as i64),
                                    action,
                                );
                                child_prop_row(
                                    ui,
                                    &mut self.text_bufs,
                                    &child.id,
                                    "Height",
                                    &PropValue::Int(child.rect.h as i64),
                                    action,
                                );
                                child_prop_row(
                                    ui,
                                    &mut self.text_bufs,
                                    &child.id,
                                    "Visible",
                                    &PropValue::Bool(child.visible),
                                    action,
                                );
                                child_prop_row(
                                    ui,
                                    &mut self.text_bufs,
                                    &child.id,
                                    "Enabled",
                                    &PropValue::Bool(child.enabled),
                                    action,
                                );
                                child_prop_row(
                                    ui,
                                    &mut self.text_bufs,
                                    &child.id,
                                    "TabOrder",
                                    &PropValue::Int(child.tab_order as i64),
                                    action,
                                );
                                child_prop_row(
                                    ui,
                                    &mut self.text_bufs,
                                    &child.id,
                                    "ZOrder",
                                    &PropValue::Int(child.z_order as i64),
                                    action,
                                );

                                let mut props: Vec<_> = child.properties.iter().collect();
                                props.sort_by(|(a, _), (b, _)| a.cmp(b));
                                for (key, value) in props {
                                    child_prop_row(
                                        ui,
                                        &mut self.text_bufs,
                                        &child.id,
                                        key,
                                        value,
                                        action,
                                    );
                                }
                            });
                    });
                }
            });
        ui.add_space(4.0);
    }

    fn show_datagrid_editor_modal(
        &mut self,
        ctx: &egui::Context,
        ctrl: &Control,
        action: &mut InspectorAction,
    ) {
        let Some(open_id) = self.datagrid_editor.clone() else {
            return;
        };
        if !open_id.eq_ignore_ascii_case(&ctrl.id) {
            self.datagrid_editor = None;
            return;
        }

        let mut open = true;
        egui::Window::new("Edit DataGrid settings")
            .id(egui::Id::new(("datagrid_settings", &ctrl.id)))
            .collapsible(false)
            .resizable(true)
            .default_size(egui::vec2(750.0, 550.0))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut open)
            .show(ctx, |ui| {
                let id = ctrl.id.as_str();
                ScrollArea::vertical()
                    .id_salt(format!("datagrid_settings_scroll_{}", ctrl.id))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        section_header(ui, "Grid behavior");
                        egui::Grid::new(format!("dg_modal_behavior_{id}"))
                            .num_columns(2)
                            .spacing([8.0, 4.0])
                            .show(ui, |ui| {
                                bool_row(ui, id, "ReadOnly", "Read only", ctrl, action);
                                ui.end_row();
                                bool_row(ui, id, "AllowSorting", "Allow sorting", ctrl, action);
                                ui.end_row();
                                bool_row(ui, id, "AllowColumnResize", "Allow column resize", ctrl, action);
                                ui.end_row();
                                bool_row(ui, id, "AllowColumnReorder", "Allow column reorder", ctrl, action);
                                ui.end_row();
                                bool_row(ui, id, "AllowRowResize", "Allow row resize", ctrl, action);
                                ui.end_row();
                                bool_row(ui, id, "ShowColumnFilters", "Show column filters", ctrl, action);
                                ui.end_row();
                                bool_row(ui, id, "SelectableText", "Selectable text", ctrl, action);
                                ui.end_row();
                                combo_row_labeled(ui, id, "SelectionMode", "Selection mode", ctrl, action, &["Row", "Cell", "Column"]);
                                ui.end_row();
                                datagrid_advanced_int_modal_row(ui, id, "RowHeight", "Row height", ctrl, action, 14..=120, 22);
                                ui.end_row();
                                datagrid_advanced_int_modal_row(ui, id, "FrozenColumns", "Frozen columns", ctrl, action, 0..=100, 0);
                                ui.end_row();
                                datagrid_advanced_int_modal_row(ui, id, "FrozenRows", "Frozen rows", ctrl, action, 0..=100, 0);
                                ui.end_row();
                                bool_row(ui, id, "FrozenShadow", "Frozen pane shadow", ctrl, action);
                                ui.end_row();
                                datagrid_grid_line_style_modal_row(ui, id, ctrl, action);
                                ui.end_row();
                            });

                        section_header(ui, "Grid background");
                        egui::Grid::new(format!("dg_modal_bg_{id}"))
                            .num_columns(2)
                            .spacing([8.0, 4.0])
                            .show(ui, |ui| {
                                datagrid_color_modal_row(ui, id, "BackgroundColor", "Background color", ctrl, action, "#1A203A");
                                ui.end_row();
                                datagrid_text_modal_row(ui, id, "GridBackgroundImage", "Background image", ctrl, action, "");
                                ui.end_row();
                                combo_row_labeled(ui, id, "GridBackgroundImageMode", "Image mode", ctrl, action, &["Fill", "Fit", "Stretch", "Tile", "Center"]);
                                ui.end_row();
                                combo_row_labeled(ui, id, "GridBackgroundPattern", "Pattern", ctrl, action, &["None", "Stripes", "Dots", "Cross", "X", "X Dots", "O"]);
                                ui.end_row();
                                combo_row_labeled(ui, id, "RowBackgroundPattern", "Row pattern", ctrl, action, &["None", "Stripes", "Dots", "Cross", "X", "X Dots", "O"]);
                                ui.end_row();
                                datagrid_color_modal_row(ui, id, "HeaderBackgroundColor", "Header background", ctrl, action, "#E0E0E0");
                                ui.end_row();
                                datagrid_color_modal_row(ui, id, "HeaderForegroundColor", "Header text", ctrl, action, "#000000");
                                ui.end_row();
                                datagrid_color_modal_row(ui, id, "AlternatingRowColor", "Alternating color", ctrl, action, "#F0F8FF");
                                ui.end_row();
                                datagrid_int_modal_row(ui, id, "AlternatingRowOpacity", "Alternating opacity %", ctrl, action, 0..=100, 20);
                                ui.end_row();
                                combo_row_labeled(ui, id, "AlternatingMode", "Alternating mode", ctrl, action, &["Rows", "Columns", "None"]);
                                ui.end_row();
                                // Grid-line colour now lives in the Appearance section as the
                                // DataGrid's Fore color (see ForegroundColor). Kept out of this
                                // modal so there is a single source of truth.
                            });

                        section_header(ui, "Columns");
                        let mut advanced = DataGridAdvanced::from_control(ctrl);
                        let mut changed_advanced = false;
                        if advanced.columns.is_empty() {
                            ui.label(RichText::new("No columns defined yet. Use data binding or the Columns property to create fields.").small().color(Color32::GRAY));
                        }
                        for (index, column) in advanced.columns.iter_mut().enumerate() {
                            egui::CollapsingHeader::new(format!(
                                "{} — {}",
                                index + 1,
                                if column.title.trim().is_empty() { &column.source_name } else { &column.title }
                            ))
                            .id_salt(format!("dg_column_modal_{}_{}", ctrl.id, column.id))
                            .default_open(index == 0)
                            .show(ui, |ui| {
                                egui::Grid::new(format!("dg_column_grid_{}_{}", ctrl.id, index))
                                    .num_columns(2)
                                    .spacing([8.0, 4.0])
                                    .show(ui, |ui| {
                                        ui.label("Title");
                                        changed_advanced |= ui.add(egui::TextEdit::singleline(&mut column.title).desired_width(180.0)).changed();
                                        ui.end_row();
                                        ui.label("Source field");
                                        ui.label(RichText::new(&column.source_name).monospace().color(Color32::GRAY));
                                        ui.end_row();
                                        ui.label("COBOL mask");
                                        changed_advanced |= ui.add(egui::TextEdit::singleline(&mut column.cobol_mask).desired_width(120.0)).changed();
                                        ui.end_row();
                                        ui.label("Edit control");
                                        let before_edit = column.edit_control.clone();
                                        egui::ComboBox::from_id_salt(format!("dg_col_edit_{}_{}", ctrl.id, index))
                                            .selected_text(if column.edit_control.trim().is_empty() { "Textbox" } else { &column.edit_control })
                                            .width(140.0)
                                            .show_ui(ui, |ui| {
                                                for opt in ["Textbox", "Dropdown", "Checkbox", "Button", "Gauge", "Image"] {
                                                    ui.selectable_value(&mut column.edit_control, opt.to_owned(), opt);
                                                }
                                            });
                                        changed_advanced |= before_edit != column.edit_control;
                                        ui.end_row();
                                        if column.edit_control.eq_ignore_ascii_case("Image") {
                                            ui.label("Image corner radius");
                                            changed_advanced |= ui
                                                .add(
                                                    DragValue::new(&mut column.image_corner_radius)
                                                        .speed(0.5)
                                                        .range(0.0..=200.0),
                                                )
                                                .changed();
                                            ui.end_row();
                                            ui.label("Image drop shadow");
                                            changed_advanced |= ui
                                                .checkbox(&mut column.image_shadow, "")
                                                .changed();
                                            ui.end_row();
                                        }
                                        ui.label("Column width");
                                        changed_advanced |= ui.add(DragValue::new(&mut column.width).speed(1.0).range(32.0..=1600.0)).changed();
                                        ui.end_row();
                                        ui.label("Header font size");
                                        let mut header_size = column.header_font_size as i64;
                                        if ui.add(DragValue::new(&mut header_size).speed(1).range(6..=72)).changed() {
                                            column.header_font_size = header_size as u16;
                                            changed_advanced = true;
                                        }
                                        ui.end_row();
                                        ui.label("Cell font size");
                                        let mut cell_size = column.font_size as i64;
                                        if ui.add(DragValue::new(&mut cell_size).speed(1).range(0..=72)).changed() {
                                            column.font_size = cell_size as u16;
                                            changed_advanced = true;
                                        }
                                        ui.end_row();
                                        ui.label("Cell foreground");
                                        let mut fg = hex_to_color32(&column.foreground_color);
                                        if color_edit_button_closing(ui, &mut fg).changed() {
                                            column.foreground_color = color32_to_hex(fg);
                                            changed_advanced = true;
                                        }
                                        ui.end_row();
                                        ui.label("Cell background");
                                        let mut bg = hex_to_color32(&column.background_color);
                                        if color_edit_button_closing(ui, &mut bg).changed() {
                                            column.background_color = color32_to_hex(bg);
                                            changed_advanced = true;
                                        }
                                        ui.end_row();
                                        ui.label("Background pattern");
                                        let before_pattern = column.background_pattern.clone();
                                        egui::ComboBox::from_id_salt(format!("dg_col_pattern_{}_{}", ctrl.id, index))
                                            .selected_text(if column.background_pattern.trim().is_empty() { "None" } else { &column.background_pattern })
                                            .width(120.0)
                                            .show_ui(ui, |ui| {
                                                for opt in ["None", "Stripes", "Dots", "Cross", "X", "X Dots", "O"] {
                                                    ui.selectable_value(&mut column.background_pattern, opt.to_owned(), opt);
                                                }
                                            });
                                        changed_advanced |= before_pattern != column.background_pattern;
                                        ui.end_row();
                                        ui.label("Background image");
                                        changed_advanced |= ui.add(egui::TextEdit::singleline(&mut column.background_image).desired_width(180.0)).changed();
                                        ui.end_row();
                                        ui.label("Text align");
                                        let before_align = column.text_alignment;
                                        egui::ComboBox::from_id_salt(format!("dg_col_align_{}_{}", ctrl.id, index))
                                            .selected_text(match column.text_alignment {
                                                DataGridTextAlignment::Left => "Left",
                                                DataGridTextAlignment::Center => "Center",
                                                DataGridTextAlignment::Right => "Right",
                                            })
                                            .width(120.0)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(&mut column.text_alignment, DataGridTextAlignment::Left, "Left");
                                                ui.selectable_value(&mut column.text_alignment, DataGridTextAlignment::Center, "Center");
                                                ui.selectable_value(&mut column.text_alignment, DataGridTextAlignment::Right, "Right");
                                            });
                                        changed_advanced |= before_align != column.text_alignment;
                                        ui.end_row();
                                        ui.label("Filter enabled");
                                        changed_advanced |= ui.checkbox(&mut column.filter_enabled, "").changed();
                                        ui.end_row();
                                        ui.label("Inner shape");
                                        if column.frame.is_none() {
                                            column.frame = Some(DataGridCellFrame::default());
                                        }
                                        if let Some(frame) = column.frame.as_mut() {
                                            changed_advanced |= ui.checkbox(&mut frame.enabled, "Enabled").changed();
                                            ui.end_row();
                                            ui.label("Shape padding");
                                            let mut padding = frame.padding as i64;
                                            if ui.add(DragValue::new(&mut padding).speed(1).range(0..=32)).changed() {
                                                frame.padding = padding as u16;
                                                changed_advanced = true;
                                            }
                                            ui.end_row();
                                            ui.label("Shape radius");
                                            let mut radius = frame.corner_radius as i64;
                                            if ui.add(DragValue::new(&mut radius).speed(1).range(0..=64)).changed() {
                                                frame.corner_radius = radius as u16;
                                                changed_advanced = true;
                                            }
                                            ui.end_row();
                                            ui.label("Shape color");
                                            let mut frame_bg = hex_to_color32(&frame.background_color);
                                            if color_edit_button_closing(ui, &mut frame_bg).changed() {
                                                frame.background_color = color32_to_hex(frame_bg);
                                                changed_advanced = true;
                                            }
                                            ui.end_row();
                                            ui.label("Inner shape color");
                                            ui.vertical(|ui| {
                                                let mut remove_rule = None;
                                                for (rule_index, rule) in column.value_style_rules.iter_mut().enumerate() {
                                                    ui.horizontal(|ui| {
                                                        let value_resp = ui.add(
                                                            egui::TextEdit::singleline(&mut rule.value)
                                                                .hint_text("Value")
                                                                .desired_width(120.0),
                                                        );
                                                        changed_advanced |= value_resp.changed();
                                                        let mut color = hex_to_color32(&rule.frame_background_color);
                                                        if color_edit_button_closing(ui, &mut color).changed() {
                                                            rule.frame_background_color = color32_to_hex(color);
                                                            changed_advanced = true;
                                                        }
                                                        if ui.button("X").on_hover_text("Remove value color").clicked() {
                                                            remove_rule = Some(rule_index);
                                                        }
                                                    });
                                                }
                                                if let Some(rule_index) = remove_rule {
                                                    column.value_style_rules.remove(rule_index);
                                                    changed_advanced = true;
                                                }
                                                if ui.button("New definition").clicked() {
                                                    column.value_style_rules.push(DataGridValueStyleRule {
                                                        value: String::new(),
                                                        frame_background_color: "#1BC47D".to_owned(),
                                                        frame_foreground_color: "#FFFFFF".to_owned(),
                                                        ..DataGridValueStyleRule::default()
                                                    });
                                                    changed_advanced = true;
                                                }
                                            });
                                            ui.end_row();
                                        }
                                        ui.label("Gauge");
                                        if column.gauge.is_none() {
                                            column.gauge = Some(DataGridGauge::default());
                                        }
                                        if let Some(gauge) = column.gauge.as_mut() {
                                            changed_advanced |= ui.checkbox(&mut gauge.enabled, "Enabled").changed();
                                            ui.end_row();
                                            ui.label("Gauge min / max");
                                            ui.horizontal(|ui| {
                                                changed_advanced |= ui.add(DragValue::new(&mut gauge.min).speed(1.0)).changed();
                                                changed_advanced |= ui.add(DragValue::new(&mut gauge.max).speed(1.0)).changed();
                                            });
                                            ui.end_row();
                                        }
                                    });
                            });
                            ui.add_space(6.0);
                        }

                        if changed_advanced {
                            if let Ok(json) = advanced.to_json() {
                                action.set_props.push((
                                    ctrl.id.clone(),
                                    DATAGRID_ADVANCED_PROP.to_owned(),
                                    PropValue::String(json),
                                ));
                            }
                        }

                        section_header(ui, "CSV export");
                        egui::Grid::new(format!("dg_modal_csv_{id}"))
                            .num_columns(2)
                            .spacing([8.0, 4.0])
                            .show(ui, |ui| {
                                bool_row(ui, id, "ExportCSV", "Enable CSV export", ctrl, action);
                                ui.end_row();
                                bool_row(ui, id, "ShowCSVExportButton", "Show CSV button", ctrl, action);
                                ui.end_row();
                                combo_row_labeled(ui, id, "CSVExportMode", "CSV mode", ctrl, action, &["Filtered", "AllRows"]);
                                ui.end_row();
                                datagrid_text_modal_row(ui, id, "CSVDelimiter", "Delimiter", ctrl, action, ",");
                                ui.end_row();
                            });
                    });
            });

        if !open {
            self.datagrid_editor = None;
        }
    }

    fn show_binding_editor(
        &mut self,
        ui: &mut Ui,
        form: &Form,
        ctrl: &Control,
        indexed_files: &[String],
        action: &mut InspectorAction,
        tr: &Tr,
    ) {
        let Some(editor) = self.binding_editor.as_mut() else {
            return;
        };
        if !editor.target_control_id.eq_ignore_ascii_case(&ctrl.id) {
            self.binding_editor = None;
            return;
        }

        editor.indexed_files = indexed_files.to_vec();
        if editor.selected_source == Some(BindingEditorSourceKind::IndexedFile)
            && editor.selected_indexed_file.trim().is_empty()
        {
            editor.selected_indexed_file = indexed_files.first().cloned().unwrap_or_default();
        }

        let ctx = ui.ctx().clone();
        let screen = ctx.screen_rect();
        let modal_size = egui::vec2(
            (screen.width() * 0.80).clamp(720.0, 1180.0),
            (screen.height() * 0.84).max(560.0),
        );
        let mut open = true;
        let mut close_editor = false;
        let mut apply_binding = false;
        egui::Window::new("Edit data binding settings")
            .id(egui::Id::new((
                "data_binding_settings",
                &editor.target_control_id,
            )))
            .collapsible(false)
            .resizable(true)
            .default_size(modal_size)
            .max_size(modal_size)
            .min_size(egui::vec2(680.0, 500.0))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut open)
            .show(&ctx, |ui| {
                ui.set_max_width((modal_size.x - 34.0).max(640.0));
                ui.add_space(6.0);
                ui.label("Create binding from:");
                ui.add_space(6.0);
                show_binding_source_selector(ui, editor, tr);
                ui.add_space(10.0);
                show_clear_selection_banner(ui, editor);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                let scroll_height = (modal_size.y - 190.0).max(320.0);
                egui::ScrollArea::vertical()
                    .id_salt(format!("data_binding_settings_scroll_{}", ctrl.id))
                    .max_height(scroll_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| match editor.selected_source {
                        Some(BindingEditorSourceKind::IndexedFile) => {
                            show_indexed_source_section(ui, editor);
                            ui.add_space(16.0);
                            show_source_fields_section(ui, editor);
                        }
                        Some(BindingEditorSourceKind::Sql) => {
                            show_sql_source_section(ui, editor);
                            ui.add_space(16.0);
                            show_source_fields_section(ui, editor);
                        }
                        Some(BindingEditorSourceKind::CobolTable) => {
                            show_cobol_table_source_section(ui, editor);
                            ui.add_space(16.0);
                            show_source_fields_section(ui, editor);
                            ui.add_space(12.0);
                            show_cobol_table_field_actions(ui, editor);
                        }
                        Some(BindingEditorSourceKind::RestApi) => {
                            show_rest_source_section(ui, editor);
                        }
                        Some(BindingEditorSourceKind::AgentAi) | None => {
                            source_placeholder(
                                ui,
                                "Select a binding source to configure settings.",
                            );
                        }
                    });
                show_dropdown_config_modal(&ctx, editor);

                if let Some(message) = &editor.validation_error {
                    ui.add_space(8.0);
                    ui.colored_label(Color32::from_rgb(255, 120, 110), message);
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if editor.selected_source != Some(BindingEditorSourceKind::RestApi)
                        && editor.selected_source != Some(BindingEditorSourceKind::CobolTable)
                    {
                        if ui.button("+ Add field").clicked() {
                            let (field_prefix, mask, data_type) =
                                if editor.selected_source == Some(BindingEditorSourceKind::Sql) {
                                    ("SQL_FIELD", "X(30)", BindingDataType::Text)
                                } else {
                                    ("FIELD", "X(10)", BindingDataType::Text)
                                };
                            editor.rows.push(BindingFieldRow::new(
                                format!("{field_prefix}_{}", editor.rows.len() + 1),
                                mask,
                                data_type,
                                format!("Field {}", editor.rows.len() + 1),
                                BindingEditControl::Textbox,
                                DropdownConfig::empty(),
                            ));
                        }
                        if ui.button("Restore removed fields").clicked() {
                            editor.rows.append(&mut editor.removed_rows);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Apply").color(Color32::WHITE).strong(),
                                )
                                .fill(Color32::from_rgb(45, 112, 230))
                                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(80, 145, 255))),
                            )
                            .clicked()
                        {
                            match editor.validate() {
                                Ok(()) => apply_binding = true,
                                Err(err) => editor.validation_error = Some(err),
                            }
                        }
                        if ui.button(tr.btn_cancel).clicked() {
                            close_editor = true;
                        }
                    });
                });
            });
        if !open {
            close_editor = true;
        }
        if apply_binding {
            action.create_data_binding = self
                .binding_editor
                .as_ref()
                .and_then(|editor| editor.to_binding(form));
            self.binding_editor = None;
        } else if close_editor {
            self.binding_editor = None;
        }
    }

    // ── Events section ────────────────────────────────────────────────────────

    fn show_events(ui: &mut Ui, ctrl: &Control, id: &str, action: &mut InspectorAction, tr: &Tr) {
        section_header(ui, tr.sec_events);
        ui.label(
            RichText::new(tr.hint_click_event)
                .small()
                .color(Color32::GRAY)
                .italics(),
        );
        ui.add_space(4.0);

        for ev in ctrl.control_type.supported_events() {
            let ev_str = ev.to_string();
            let binding = ctrl.events.iter().find(|e| e.event == ev_str);
            let has_code = binding.map(|e| e.has_code()).unwrap_or(false);
            let lines = binding.map(|e| e.code_line_count()).unwrap_or(0);

            let row_resp = ui.horizontal_wrapped(|ui| {
                let dot_color = if has_code {
                    Color32::from_rgb(100, 220, 100)
                } else {
                    Color32::from_rgb(120, 120, 120)
                };
                ui.label(RichText::new(if has_code { "●" } else { "○" }).color(dot_color));
                let lbl = ui
                    .add(
                        egui::Label::new(
                            RichText::new(format!("⚙ {ev_str}"))
                                .color(Color32::from_rgb(200, 200, 100)),
                        )
                        .sense(egui::Sense::click()),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(tr.hint_dblclick_event);
                if has_code {
                    ui.label(
                        RichText::new(format!("({lines} {})", tr.hint_lines))
                            .small()
                            .color(Color32::GRAY),
                    );
                } else {
                    ui.label(
                        RichText::new(tr.hint_click_to_add)
                            .small()
                            .color(Color32::from_rgb(100, 100, 100))
                            .italics(),
                    );
                }
                (lbl.clicked(), lbl.double_clicked())
            });

            let (clicked, double_clicked) = row_resp.inner;
            if double_clicked {
                action.open_event_in_code = Some((id.to_owned(), ev_str));
            } else if clicked {
                action.open_event_editor = Some((id.to_owned(), ev_str));
            }
        }
        ui.add_space(4.0);
    }

    // ── Animation editor ─────────────────────────────────────────────────────

    fn show_animations(
        &mut self,
        ui: &mut Ui,
        ctrl: &Control,
        id: &str,
        action: &mut InspectorAction,
        tr: &Tr,
    ) {
        section_header(ui, tr.sec_animations);

        ui.label(
            RichText::new(
                "Each animation is a named effect triggered by an event or from COBOL.\n\
             COBOL: INVOKE ctrl-id 'PlayAnimation' USING BY VALUE 'fly-in'",
            )
            .small()
            .color(Color32::GRAY)
            .italics(),
        );
        ui.add_space(2.0);

        // List existing animations
        let anim_count = ctrl.animations.len();
        let sel = *self.anim_sel.entry(id.to_owned()).or_insert(0);

        if anim_count == 0 {
            ui.label(
                RichText::new("(no animations — add one below)")
                    .small()
                    .color(Color32::GRAY),
            );
        } else {
            // Selector tabs for each animation
            ui.horizontal_wrapped(|ui| {
                for (i, anim) in ctrl.animations.iter().enumerate() {
                    let active = i == sel;
                    if ui
                        .selectable_label(active, RichText::new(&anim.name).small())
                        .clicked()
                    {
                        *self.anim_sel.entry(id.to_owned()).or_insert(0) = i;
                    }
                }
            });
            ui.add_space(2.0);

            // Edit the selected animation
            if let Some(anim) = ctrl.animations.get(sel) {
                let anim_id = format!("{id}-anim{sel}");

                egui::Grid::new(format!("anim_grid_{anim_id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        // Name
                        ui.label("Name:");
                        let cur_name = anim.name.clone();
                        let bk = format!("{anim_id}-name");
                        let wid = egui::Id::new(&bk);
                        let buf = self.text_bufs.entry(bk).or_insert(cur_name.clone());
                        if *buf != cur_name && !ui.memory(|m| m.has_focus(wid)) {
                            *buf = cur_name.clone();
                        }
                        if ui
                            .add(
                                egui::TextEdit::singleline(buf)
                                    .id(wid)
                                    .desired_width(f32::INFINITY),
                            )
                            .lost_focus()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                format!("Anim{sel}_Name"),
                                PropValue::String(buf.clone()),
                            ));
                        }
                        ui.end_row();

                        // Trigger
                        let cur_t = anim.trigger.as_str().to_owned();
                        ui.label("Trigger:");
                        egui::ComboBox::from_id_salt(format!("anim_trigger_{anim_id}"))
                            .selected_text(&cur_t)
                            .width(140.0)
                            .show_ui(ui, |ui| {
                                for &opt in AnimTrigger::ALL {
                                    if ui.selectable_label(cur_t == opt, opt).clicked() {
                                        action.set_props.push((
                                            id.to_owned(),
                                            format!("Anim{sel}_Trigger"),
                                            PropValue::String(opt.to_owned()),
                                        ));
                                    }
                                }
                            });
                        ui.end_row();

                        // Kind
                        let cur_k = anim.kind.as_str().to_owned();
                        ui.label("Effect:");
                        egui::ComboBox::from_id_salt(format!("anim_kind_{anim_id}"))
                            .selected_text(&cur_k)
                            .width(140.0)
                            .show_ui(ui, |ui| {
                                for &opt in AnimKind::ALL {
                                    if ui.selectable_label(cur_k == opt, opt).clicked() {
                                        action.set_props.push((
                                            id.to_owned(),
                                            format!("Anim{sel}_Kind"),
                                            PropValue::String(opt.to_owned()),
                                        ));
                                    }
                                }
                            });
                        ui.end_row();

                        // Duration
                        ui.label("Duration (ms):");
                        let mut dur = anim.duration_ms as i64;
                        if ui
                            .add(DragValue::new(&mut dur).speed(10).range(50..=30_000))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                format!("Anim{sel}_Duration"),
                                PropValue::Int(dur),
                            ));
                        }
                        ui.end_row();

                        // Delay
                        ui.label("Delay (ms):");
                        let mut delay = anim.delay_ms as i64;
                        if ui
                            .add(DragValue::new(&mut delay).speed(10).range(0..=10_000))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                format!("Anim{sel}_Delay"),
                                PropValue::Int(delay),
                            ));
                        }
                        ui.end_row();

                        // Easing
                        let cur_e = anim.easing.as_str().to_owned();
                        ui.label("Easing:");
                        egui::ComboBox::from_id_salt(format!("anim_ease_{anim_id}"))
                            .selected_text(&cur_e)
                            .width(140.0)
                            .show_ui(ui, |ui| {
                                for &opt in EasingKind::ALL {
                                    if ui.selectable_label(cur_e == opt, opt).clicked() {
                                        action.set_props.push((
                                            id.to_owned(),
                                            format!("Anim{sel}_Easing"),
                                            PropValue::String(opt.to_owned()),
                                        ));
                                    }
                                }
                            });
                        ui.end_row();

                        // Repeat
                        let cur_r = anim.repeat.as_str().to_owned();
                        ui.label("Repeat:");
                        egui::ComboBox::from_id_salt(format!("anim_rep_{anim_id}"))
                            .selected_text(&cur_r)
                            .width(140.0)
                            .show_ui(ui, |ui| {
                                for &opt in AnimRepeat::ALL {
                                    if ui.selectable_label(cur_r == opt, opt).clicked() {
                                        action.set_props.push((
                                            id.to_owned(),
                                            format!("Anim{sel}_Repeat"),
                                            PropValue::String(opt.to_owned()),
                                        ));
                                    }
                                }
                            });
                        ui.end_row();

                        // Slide offsets (shown only for Slide kind)
                        if anim.kind.as_str() == "Slide" {
                            ui.label("Slide DX:");
                            let mut sdx = anim.slide_dx as i64;
                            if ui.add(DragValue::new(&mut sdx).speed(4)).changed() {
                                action.set_props.push((
                                    id.to_owned(),
                                    format!("Anim{sel}_SlideDX"),
                                    PropValue::Int(sdx),
                                ));
                            }
                            ui.end_row();
                            ui.label("Slide DY:");
                            let mut sdy = anim.slide_dy as i64;
                            if ui.add(DragValue::new(&mut sdy).speed(4)).changed() {
                                action.set_props.push((
                                    id.to_owned(),
                                    format!("Anim{sel}_SlideDY"),
                                    PropValue::Int(sdy),
                                ));
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
            ui.add(
                egui::TextEdit::singleline(&mut self.new_anim_name)
                    .hint_text("fly-in")
                    .desired_width(120.0),
            );
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
        &mut self,
        ui: &mut Ui,
        ctrl: &Control,
        id: &str,
        action: &mut InspectorAction,
        tr: &Tr,
    ) {
        match ctrl.control_type {
            // ── Button ────────────────────────────────────────────────────────
            ControlType::Button => {
                section_header(ui, "Basic properties");
                egui::Grid::new(format!("btn_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        bool_row(ui, id, "IsDefault", "Default button", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "IsCancel", "Cancel button", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "FlatStyle", "Flat style", ctrl, action);
                        ui.end_row();

                        ui.label("CornerRadius:");
                        let mut r = ctrl
                            .get_prop("CornerRadius")
                            .map(|v| v.as_i64())
                            .unwrap_or(3);
                        if ui
                            .add(DragValue::new(&mut r).speed(1).range(0..=50))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "CornerRadius".into(),
                                PropValue::Int(r),
                            ));
                        }
                        ui.end_row();

                        combo_row(
                            ui,
                            id,
                            "ModalResult",
                            ctrl,
                            action,
                            &[
                                "None", "OK", "Cancel", "Yes", "No", "Abort", "Retry", "Ignore",
                            ],
                        );
                        ui.end_row();

                        combo_row(
                            ui,
                            id,
                            "TextAlignment",
                            ctrl,
                            action,
                            &[
                                "MiddleCenter",
                                "TopLeft",
                                "TopCenter",
                                "TopRight",
                                "MiddleLeft",
                                "MiddleRight",
                                "BottomLeft",
                                "BottomCenter",
                                "BottomRight",
                            ],
                        );
                        ui.end_row();
                    });
                // Image
                image_browse_row(ui, id, "ImagePath", ctrl, action, &mut self.text_bufs);
                combo_row_inline(
                    ui,
                    id,
                    "ImageAlignment",
                    ctrl,
                    action,
                    &[
                        "MiddleLeft",
                        "MiddleCenter",
                        "MiddleRight",
                        "TopLeft",
                        "BottomLeft",
                    ],
                );
                border_rows(ui, id, ctrl, action, &mut self.text_bufs);
                ui.add_space(4.0);
            }

            // ── Label ─────────────────────────────────────────────────────────
            ControlType::Label => {
                section_header(ui, "Basic properties");
                combo_row_inline(
                    ui,
                    id,
                    "TextAlignment",
                    ctrl,
                    action,
                    &["Left", "Center", "Right"],
                );
                bool_row_inline(ui, id, "WordWrap", "WordWrap", ctrl, action);
                bool_row_inline(ui, id, "AutoSize", "AutoSize", ctrl, action);
                combo_row_inline(
                    ui,
                    id,
                    "BorderStyle",
                    ctrl,
                    action,
                    &["None", "Single", "Fixed3D"],
                );
                ui.add_space(4.0);
            }

            // ── TextBox ───────────────────────────────────────────────────────
            ControlType::TextBox => {
                section_header(ui, "Basic properties");
                {
                    let cur = ctrl
                        .get_prop("HintText")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    text_row_hint(
                        ui,
                        &mut self.text_bufs,
                        id,
                        "HintText",
                        &cur,
                        "Hint text:",
                        "(placeholder)",
                        action,
                    );
                }
                egui::Grid::new(format!("tbx_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        ui.label("MaxLength:");
                        let mut ml = ctrl
                            .get_prop("MaximumLength")
                            .map(|v| v.as_i64())
                            .unwrap_or(0);
                        if ui
                            .add(DragValue::new(&mut ml).speed(1).range(0..=32767))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "MaximumLength".into(),
                                PropValue::Int(ml),
                            ));
                        }
                        ui.end_row();
                        bool_row(ui, id, "Multiline", "Multiline", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "WordWrap", "WordWrap", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "ReadOnly", "ReadOnly", ctrl, action);
                        ui.end_row();

                        ui.label("PwdChar:");
                        let buf_key = format!("{id}-PasswordChar");
                        let wid = egui::Id::new(&buf_key);
                        let cur = ctrl
                            .get_prop("PasswordCharacter")
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_default();
                        let buf = self.text_bufs.entry(buf_key).or_insert(cur.clone());
                        if *buf != cur && !ui.memory(|m| m.has_focus(wid)) {
                            *buf = cur;
                        }
                        if ui
                            .add(egui::TextEdit::singleline(buf).id(wid).desired_width(30.0))
                            .lost_focus()
                        {
                            buf.truncate(1);
                            action.set_props.push((
                                id.to_owned(),
                                "PasswordCharacter".into(),
                                PropValue::String(buf.clone()),
                            ));
                        }
                        ui.end_row();

                        combo_row(
                            ui,
                            id,
                            "ScrollBars",
                            ctrl,
                            action,
                            &["None", "Horizontal", "Vertical", "Both"],
                        );
                        ui.end_row();
                    });
                border_rows(ui, id, ctrl, action, &mut self.text_bufs);
                ui.add_space(4.0);
            }

            // ── CheckBox / RadioButton ────────────────────────────────────────
            ControlType::CheckBox | ControlType::RadioButton => {
                section_header(ui, "Basic properties");
                bool_row_inline(ui, id, "Checked", "Checked (default)", ctrl, action);
                combo_row_inline(
                    ui,
                    id,
                    "CheckAlignment",
                    ctrl,
                    action,
                    &["Left", "Center", "Right"],
                );
                color_row(ui, id, "CheckColor", ctrl, action);
                if matches!(ctrl.control_type, ControlType::RadioButton) {
                    let cur = ctrl
                        .get_prop("GroupName")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    text_row_hint(
                        ui,
                        &mut self.text_bufs,
                        id,
                        "GroupName",
                        &cur,
                        "Group:",
                        "group-name",
                        action,
                    );
                }
                ui.add_space(4.0);
            }

            // ── PictureBox ────────────────────────────────────────────────────
            ControlType::PictureBox => {
                section_header(ui, "Basic properties");
                image_browse_row(ui, id, "ImagePath", ctrl, action, &mut self.text_bufs);
                combo_row_inline(
                    ui,
                    id,
                    "SizeMode",
                    ctrl,
                    action,
                    &["Normal", "Stretch", "Zoom", "CenterImage", "AutoSize"],
                );
                combo_row_inline(
                    ui,
                    id,
                    "ImageAlignment",
                    ctrl,
                    action,
                    &[
                        "TopLeft",
                        "TopCenter",
                        "TopRight",
                        "MiddleLeft",
                        "MiddleCenter",
                        "MiddleRight",
                        "BottomLeft",
                        "BottomCenter",
                        "BottomRight",
                    ],
                );
                {
                    // Frame toggle — when off, only the image is shown (transparent
                    // PNG areas reveal whatever is behind the control).
                    let mut show = ctrl
                        .get_prop("ShowFrame")
                        .map(|v| v.as_bool())
                        .unwrap_or(true);
                    if ui
                        .checkbox(&mut show, "Show frame (uncheck = image only)")
                        .changed()
                    {
                        action.set_props.push((
                            id.to_owned(),
                            "ShowFrame".into(),
                            PropValue::Bool(show),
                        ));
                    }
                }
                border_rows(ui, id, ctrl, action, &mut self.text_bufs);
                ui.add_space(4.0);
            }

            // ── Animator ──────────────────────────────────────────────────────
            ControlType::Animator => {
                section_header(ui, "Basic properties");
                image_browse_row(ui, id, "Source", ctrl, action, &mut self.text_bufs);
                combo_row_inline(
                    ui,
                    id,
                    "SizeMode",
                    ctrl,
                    action,
                    &["Fit", "Fill", "Stretch", "Center"],
                );
                egui::Grid::new(format!("anim_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        bool_row(ui, id, "AutoPlay", "Auto-play", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "Loop", "Loop", ctrl, action);
                        ui.end_row();
                    });
                border_rows(ui, id, ctrl, action, &mut self.text_bufs);
                ui.add_space(4.0);
            }

            // ── ListBox ───────────────────────────────────────────────────────
            ControlType::ListBox => {
                section_header(ui, "ListBox");
                items_multiline(ui, id, ctrl, action, &mut self.text_bufs);
                egui::Grid::new(format!("lb_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        bool_row(ui, id, "MultiSelect", "Multi-select", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "Sorted", "Sorted", ctrl, action);
                        ui.end_row();
                    });
                border_rows(ui, id, ctrl, action, &mut self.text_bufs);
                ui.add_space(4.0);
            }

            // ── ComboBox ─────────────────────────────────────────────────────
            ControlType::ComboBox => {
                section_header(ui, "ComboBox");
                items_multiline(ui, id, ctrl, action, &mut self.text_bufs);
                egui::Grid::new(format!("cb_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        bool_row(ui, id, "Sorted", "Sorted", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "Editable", "Editable", ctrl, action);
                        ui.end_row();
                        combo_row(
                            ui,
                            id,
                            "DropDownStyle",
                            ctrl,
                            action,
                            &["DropDown", "DropDownList", "Simple"],
                        );
                        ui.end_row();
                        ui.label("DropDownHeight:");
                        let mut ddh = ctrl
                            .get_prop("DropDownHeight")
                            .map(|v| v.as_i64())
                            .unwrap_or(200);
                        if ui
                            .add(DragValue::new(&mut ddh).speed(1).range(50..=600))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "DropDownHeight".into(),
                                PropValue::Int(ddh),
                            ));
                        }
                        ui.end_row();
                    });
                ui.add_space(4.0);
            }

            // ── Slider ───────────────────────────────────────────────────────
            ControlType::Slider => {
                section_header(ui, "Slider");
                egui::Grid::new(format!("sld_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        ui.label("Min:");
                        let mut min = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0);
                        if ui.add(DragValue::new(&mut min).speed(1)).changed() {
                            action.set_props.push((
                                id.to_owned(),
                                "Minimum".into(),
                                PropValue::Int(min),
                            ));
                        }
                        ui.end_row();
                        ui.label("Max:");
                        let mut max = ctrl.get_prop("Maximum").map(|v| v.as_i64()).unwrap_or(100);
                        if ui.add(DragValue::new(&mut max).speed(1)).changed() {
                            action.set_props.push((
                                id.to_owned(),
                                "Maximum".into(),
                                PropValue::Int(max),
                            ));
                        }
                        ui.end_row();
                        ui.label("Value:");
                        let mut val = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0);
                        if ui.add(DragValue::new(&mut val).speed(1)).changed() {
                            action.set_props.push((
                                id.to_owned(),
                                "Value".into(),
                                PropValue::Int(val),
                            ));
                        }
                        ui.end_row();
                        ui.label("Step:");
                        let mut step = ctrl.get_prop("Step").map(|v| v.as_i64()).unwrap_or(10);
                        if ui
                            .add(DragValue::new(&mut step).speed(1).range(1..=9999))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "Step".into(),
                                PropValue::Int(step),
                            ));
                        }
                        ui.end_row();
                        ui.label("Large change:");
                        let mut lc = ctrl
                            .get_prop("LargeChange")
                            .map(|v| v.as_i64())
                            .unwrap_or(20);
                        if ui
                            .add(DragValue::new(&mut lc).speed(1).range(1..=9999))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "LargeChange".into(),
                                PropValue::Int(lc),
                            ));
                        }
                        ui.end_row();
                        ui.label("Tick frequency:");
                        let mut tf = ctrl
                            .get_prop("TickFrequency")
                            .map(|v| v.as_i64())
                            .unwrap_or(10);
                        if ui
                            .add(DragValue::new(&mut tf).speed(1).range(1..=9999))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "TickFrequency".into(),
                                PropValue::Int(tf),
                            ));
                        }
                        ui.end_row();
                        combo_row(
                            ui,
                            id,
                            "Orientation",
                            ctrl,
                            action,
                            &["Horizontal", "Vertical"],
                        );
                        ui.end_row();
                        combo_row(
                            ui,
                            id,
                            "TickStyle",
                            ctrl,
                            action,
                            &["Bottom", "Top", "Both", "None"],
                        );
                        ui.end_row();
                        bool_row(ui, id, "ShowValue", "Show value label", ctrl, action);
                        ui.end_row();
                    });
                // Slider colours are the standard Appearance Back color (track
                // body) and Fore color (knob). The legacy Track/Thumb/Fill colour
                // pickers are gone — the renderer never used them.
                ui.add_space(4.0);
            }

            // ── ProgressBar ───────────────────────────────────────────────────
            ControlType::ProgressBar => {
                section_header(ui, "Basic properties");
                egui::Grid::new(format!("pb_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        ui.label("Min:");
                        let mut min = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0);
                        if ui.add(DragValue::new(&mut min).speed(1)).changed() {
                            action.set_props.push((
                                id.to_owned(),
                                "Minimum".into(),
                                PropValue::Int(min),
                            ));
                        }
                        ui.end_row();
                        ui.label("Max:");
                        let mut max = ctrl.get_prop("Maximum").map(|v| v.as_i64()).unwrap_or(100);
                        if ui.add(DragValue::new(&mut max).speed(1)).changed() {
                            action.set_props.push((
                                id.to_owned(),
                                "Maximum".into(),
                                PropValue::Int(max),
                            ));
                        }
                        ui.end_row();
                        ui.label("Value:");
                        let mut val = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0);
                        if ui.add(DragValue::new(&mut val).speed(1)).changed() {
                            action.set_props.push((
                                id.to_owned(),
                                "Value".into(),
                                PropValue::Int(val),
                            ));
                        }
                        ui.end_row();
                        combo_row(
                            ui,
                            id,
                            "Orientation",
                            ctrl,
                            action,
                            &["Horizontal", "Vertical"],
                        );
                        ui.end_row();
                        combo_row(ui, id, "Style", ctrl, action, &["Continuous", "Blocks"]);
                        ui.end_row();
                        bool_row(ui, id, "ShowValue", "Show value text", ctrl, action);
                        ui.end_row();
                    });
                color_row(ui, id, "BarColor", ctrl, action);
                ui.add_space(4.0);
            }

            // ── DataGrid ─────────────────────────────────────────────────────
            ControlType::DataGrid => {
                section_header(ui, tr.dg_section);
                let advanced = DataGridAdvanced::from_control(ctrl);
                ui.label(
                    RichText::new(format!(
                        "{} columns, {} frozen column(s), {} frozen row(s)",
                        advanced.columns.len(),
                        advanced.frozen_columns,
                        advanced.frozen_rows
                    ))
                    .small()
                    .color(Color32::GRAY),
                );
                if ui.button("Edit DataGrid settings...").clicked() {
                    self.datagrid_editor = Some(id.to_owned());
                }
                ui.add_space(4.0);
            }

            // ── TabControl ────────────────────────────────────────────────────
            ControlType::TabControl => {
                section_header(ui, "Basic properties");
                {
                    let cur = ctrl
                        .get_prop("Tabs")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    let buf_key = format!("{id}-Tabs");
                    let wid = egui::Id::new(&buf_key);
                    let buf = self.text_bufs.entry(buf_key).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) {
                        *buf = cur;
                    }
                    ui.label("Tabs (one per line):");
                    let resp = ui.add(
                        egui::TextEdit::multiline(buf)
                            .id(wid)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                    if resp.lost_focus() {
                        action.set_props.push((
                            id.to_owned(),
                            "Tabs".into(),
                            PropValue::String(buf.clone()),
                        ));
                    }
                }
                combo_row_inline(
                    ui,
                    id,
                    "TabPosition",
                    ctrl,
                    action,
                    &["Top", "Bottom", "Left", "Right"],
                );
                // Container behaviour (spec 012).
                bool_row_inline(ui, id, "HScroll", "H-Scroll", ctrl, action);
                bool_row_inline(ui, id, "VScroll", "V-Scroll", ctrl, action);
                ui.add_space(4.0);
            }

            // ── Panel / GroupBox ──────────────────────────────────────────────
            ControlType::Panel | ControlType::GroupBox => {
                section_header(ui, "Basic properties");
                // Auto-scroll overflowing children vs clip them (spec 012).
                bool_row_inline(ui, id, "HScroll", "H-Scroll", ctrl, action);
                bool_row_inline(ui, id, "VScroll", "V-Scroll", ctrl, action);
                // Container visual properties (shared by GroupBox and Panel).
                // Caption props only for GroupBox.
                if matches!(ctrl.control_type, ControlType::GroupBox) {
                    bool_row_inline(ui, id, "HideCaption", "Hide caption", ctrl, action);
                    bool_row_inline(ui, id, "CaptionEnabled", "Caption enabled", ctrl, action);
                }
                bool_row_inline(ui, id, "HideBackground", "Hide background", ctrl, action);
                bool_row_inline(
                    ui,
                    id,
                    "BackgroundGradientEnabled",
                    "Background gradient",
                    ctrl,
                    action,
                );
                if ctrl
                    .get_prop("BackgroundGradientEnabled")
                    .map(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    color_row(ui, id, "BackgroundGradientStartColor", ctrl, action);
                    color_row(ui, id, "BackgroundGradientEndColor", ctrl, action);
                    combo_row_inline(
                        ui,
                        id,
                        "BackgroundGradientDirection",
                        ctrl,
                        action,
                        &[
                            "Vertical",
                            "Horizontal",
                            "DiagonalDown",
                            "DiagonalUp",
                            "Radial",
                        ],
                    );
                }
                border_rows(ui, id, ctrl, action, &mut self.text_bufs);
                ui.add_space(4.0);

                // GroupBox repeating-group properties (spec 015).
                if matches!(ctrl.control_type, ControlType::GroupBox) {
                    let is_rep = ctrl
                        .get_prop("IsRepeatingGroup")
                        .map(|v| v.as_bool())
                        .unwrap_or(false);
                    bool_row_inline(
                        ui,
                        id,
                        "IsRepeatingGroup",
                        "Repeating group (array)",
                        ctrl,
                        action,
                    );
                    if is_rep {
                        section_header(ui, "Repeating Group");
                        let an = ctrl
                            .get_prop("ArrayName")
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_default();
                        text_row_hint(
                            ui,
                            &mut self.text_bufs,
                            id,
                            "ArrayName",
                            &an,
                            "Array name:",
                            id,
                            action,
                        );
                        int_row_inline(
                            ui,
                            id,
                            "ItemCount",
                            "Item count",
                            ctrl,
                            action,
                            0..=100_000,
                        );
                        let ds = ctrl
                            .get_prop("DataSource")
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_default();
                        text_row_hint(
                            ui,
                            &mut self.text_bufs,
                            id,
                            "DataSource",
                            &ds,
                            "Data source:",
                            "(optional)",
                            action,
                        );
                        combo_row_inline(
                            ui,
                            id,
                            "LayoutDirection",
                            ctrl,
                            action,
                            &["Vertical", "Horizontal", "Grid"],
                        );
                        int_row_inline(
                            ui,
                            id,
                            "ItemSpacing",
                            "Item spacing",
                            ctrl,
                            action,
                            0..=400,
                        );
                        int_row_inline(
                            ui,
                            id,
                            "ItemsPerRow",
                            "Items per row",
                            ctrl,
                            action,
                            1..=100,
                        );
                        // How each card appears as its row binds.
                        combo_row_inline(
                            ui,
                            id,
                            "PlacementEffect",
                            ctrl,
                            action,
                            &["None", "Deal", "FadeIn"],
                        );
                        int_row_inline(
                            ui,
                            id,
                            "CardAppearDuration",
                            "Effect duration (ms)",
                            ctrl,
                            action,
                            0..=5000,
                        );
                        bool_row_inline(ui, id, "CloneEvents", "Clone events", ctrl, action);
                        int_row_inline(
                            ui,
                            id,
                            "PreviewItemCount",
                            "Preview items",
                            ctrl,
                            action,
                            1..=50,
                        );
                    }
                    ui.add_space(4.0);
                }
            }

            // ── Line ─────────────────────────────────────────────────────────
            ControlType::Line => {
                section_header(ui, "Basic properties");
                color_row(ui, id, "LineColor", ctrl, action);
                egui::Grid::new(format!("ln_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        ui.label("Thickness:");
                        let mut t = ctrl
                            .get_prop("LineThickness")
                            .map(|v| v.as_i64())
                            .unwrap_or(1);
                        if ui
                            .add(DragValue::new(&mut t).speed(1).range(1..=32))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "LineThickness".into(),
                                PropValue::Int(t),
                            ));
                        }
                        ui.end_row();
                        combo_row(
                            ui,
                            id,
                            "LineDirection",
                            ctrl,
                            action,
                            &["Horizontal", "Vertical", "Diagonal"],
                        );
                        ui.end_row();
                        // Free rotation angle (degrees). Setting it overrides the
                        // direction preset; drag the on-canvas knob or this value.
                        ui.label("Angle°:");
                        let mut ang = ctrl
                            .get_prop("LineAngle")
                            .map(|v| v.as_i64())
                            .unwrap_or_else(|| {
                                match ctrl
                                    .get_prop("LineDirection")
                                    .map(|v| v.as_str().to_owned())
                                    .as_deref()
                                {
                                    Some("Vertical") => 90,
                                    Some("Diagonal") => 45,
                                    _ => 0,
                                }
                            });
                        if ui
                            .add(DragValue::new(&mut ang).speed(1).range(0..=359))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "LineAngle".into(),
                                PropValue::Int(ang.rem_euclid(360)),
                            ));
                        }
                        ui.end_row();
                        combo_row(
                            ui,
                            id,
                            "DashStyle",
                            ctrl,
                            action,
                            &["Solid", "Dash", "Dot", "DashDot"],
                        );
                        ui.end_row();
                        bool_row(ui, id, "RoundedEnds", "Rounded ends", ctrl, action);
                        ui.end_row();
                    });
                ui.add_space(4.0);
            }

            // ── DateTimePicker ────────────────────────────────────────────────
            ControlType::DateTimePicker => {
                section_header(ui, "Basic properties");
                {
                    let cur = ctrl
                        .get_prop("Value")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    text_row_hint(
                        ui,
                        &mut self.text_bufs,
                        id,
                        "Value",
                        &cur,
                        "Value:",
                        "YYYY-MM-DD",
                        action,
                    );
                }
                egui::Grid::new(format!("dtp_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        combo_row(
                            ui,
                            id,
                            "Format",
                            ctrl,
                            action,
                            &["Short", "Long", "Time", "Custom"],
                        );
                        ui.end_row();
                        bool_row(ui, id, "ShowUpDown", "Show up/down", ctrl, action);
                        ui.end_row();
                    });
                {
                    let cur = ctrl
                        .get_prop("CustomFormat")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    text_row_hint(
                        ui,
                        &mut self.text_bufs,
                        id,
                        "CustomFormat",
                        &cur,
                        "Custom fmt:",
                        "dd/MM/yyyy HH:mm",
                        action,
                    );
                }
                {
                    let cur = ctrl
                        .get_prop("MinimumDate")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    text_row_hint(
                        ui,
                        &mut self.text_bufs,
                        id,
                        "MinimumDate",
                        &cur,
                        "Min date:",
                        "YYYY-MM-DD",
                        action,
                    );
                }
                {
                    let cur = ctrl
                        .get_prop("MaximumDate")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    text_row_hint(
                        ui,
                        &mut self.text_bufs,
                        id,
                        "MaximumDate",
                        &cur,
                        "Max date:",
                        "YYYY-MM-DD",
                        action,
                    );
                }
                color_row(ui, id, "BorderColor", ctrl, action);
                ui.add_space(4.0);
            }

            // ── NumericUpDown ─────────────────────────────────────────────────
            ControlType::NumericUpDown => {
                section_header(ui, "Basic properties");
                egui::Grid::new(format!("nud_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        ui.label("Value:");
                        let mut v = ctrl.get_prop("Value").map(|vv| vv.as_i64()).unwrap_or(0);
                        if ui.add(DragValue::new(&mut v).speed(1)).changed() {
                            action.set_props.push((
                                id.to_owned(),
                                "Value".into(),
                                PropValue::Int(v),
                            ));
                        }
                        ui.end_row();
                        ui.label("Min:");
                        let mut mn = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0);
                        if ui.add(DragValue::new(&mut mn).speed(1)).changed() {
                            action.set_props.push((
                                id.to_owned(),
                                "Minimum".into(),
                                PropValue::Int(mn),
                            ));
                        }
                        ui.end_row();
                        ui.label("Max:");
                        let mut mx = ctrl.get_prop("Maximum").map(|v| v.as_i64()).unwrap_or(100);
                        if ui.add(DragValue::new(&mut mx).speed(1)).changed() {
                            action.set_props.push((
                                id.to_owned(),
                                "Maximum".into(),
                                PropValue::Int(mx),
                            ));
                        }
                        ui.end_row();
                        ui.label("Step:");
                        let mut st = ctrl.get_prop("Step").map(|v| v.as_i64()).unwrap_or(1);
                        if ui
                            .add(DragValue::new(&mut st).speed(1).range(1..=1000))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "Step".into(),
                                PropValue::Int(st),
                            ));
                        }
                        ui.end_row();
                        ui.label("Decimals:");
                        let mut dp = ctrl
                            .get_prop("DecimalPlaces")
                            .map(|v| v.as_i64())
                            .unwrap_or(0);
                        if ui
                            .add(DragValue::new(&mut dp).speed(1).range(0..=10))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "DecimalPlaces".into(),
                                PropValue::Int(dp),
                            ));
                        }
                        ui.end_row();
                        bool_row(ui, id, "ThousandsSeparator", "Thousands sep", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "ReadOnly", "ReadOnly", ctrl, action);
                        ui.end_row();
                    });
                color_row(ui, id, "BorderColor", ctrl, action);
                ui.add_space(4.0);
            }

            // ── TreeView ──────────────────────────────────────────────────────
            ControlType::TreeView => {
                section_header(ui, "Basic properties");
                {
                    let cur = ctrl
                        .get_prop("Items")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    let buf_key = format!("{id}-Items");
                    let wid = egui::Id::new(&buf_key);
                    let buf = self.text_bufs.entry(buf_key).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) {
                        *buf = cur;
                    }
                    ui.label("Nodes (indent = child):");
                    let resp = ui.add(
                        egui::TextEdit::multiline(buf)
                            .id(wid)
                            .desired_rows(5)
                            .desired_width(f32::INFINITY),
                    );
                    if resp.lost_focus() {
                        action.set_props.push((
                            id.to_owned(),
                            "Items".into(),
                            PropValue::String(buf.clone()),
                        ));
                    }
                }
                egui::Grid::new(format!("tv_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        bool_row(ui, id, "AllowEdit", "Allow edit", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "CheckBoxes", "Checkboxes", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "ShowLines", "Show lines", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "ShowRootLines", "Root lines", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "Sorted", "Sorted", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "HotTracking", "Hot tracking", ctrl, action);
                        ui.end_row();
                    });
                color_row(ui, id, "LineColor", ctrl, action);
                color_row(ui, id, "BorderColor", ctrl, action);
                ui.add_space(4.0);
            }

            // ── Splitter ──────────────────────────────────────────────────────
            ControlType::Splitter => {
                section_header(ui, "Basic properties");
                egui::Grid::new(format!("sp_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        combo_row(
                            ui,
                            id,
                            "Orientation",
                            ctrl,
                            action,
                            &["Horizontal", "Vertical"],
                        );
                        ui.end_row();
                        ui.label("MinSize:");
                        let mut ms = ctrl
                            .get_prop("MinimumSize")
                            .map(|v| v.as_i64())
                            .unwrap_or(25);
                        if ui
                            .add(DragValue::new(&mut ms).speed(1).range(0..=500))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "MinimumSize".into(),
                                PropValue::Int(ms),
                            ));
                        }
                        ui.end_row();
                        ui.label("SplitPosition:");
                        let mut sp = ctrl
                            .get_prop("SplitPosition")
                            .map(|v| v.as_i64())
                            .unwrap_or(100);
                        if ui
                            .add(DragValue::new(&mut sp).speed(1).range(0..=9999))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "SplitPosition".into(),
                                PropValue::Int(sp),
                            ));
                        }
                        ui.end_row();
                    });
                color_row(ui, id, "BorderColor", ctrl, action);
                ui.add_space(4.0);
            }

            // ── Timer ─────────────────────────────────────────────────────────
            ControlType::Timer => {
                section_header(ui, "Basic properties");
                egui::Grid::new(format!("tmr_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        ui.label("Interval (ms):");
                        let mut iv = ctrl
                            .get_prop("Interval")
                            .map(|v| v.as_i64())
                            .unwrap_or(1000);
                        if ui
                            .add(DragValue::new(&mut iv).speed(10).range(1..=3_600_000))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "Interval".into(),
                                PropValue::Int(iv),
                            ));
                        }
                        ui.end_row();
                        bool_row(ui, &id, "Enabled", "Enabled at start", ctrl, action);
                        ui.end_row();
                    });
                ui.add_space(4.0);
            }

            // ── Shape ─────────────────────────────────────────────────────────
            ControlType::Shape => {
                section_header(ui, "Basic properties");
                egui::Grid::new(format!("shp_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        combo_row(
                            ui,
                            id,
                            "ShapeType",
                            ctrl,
                            action,
                            &["Rectangle", "Circle", "RoundRect", "Triangle"],
                        );
                        ui.end_row();
                        combo_row(
                            ui,
                            id,
                            "FillStyle",
                            ctrl,
                            action,
                            &["Solid", "None", "Hatched"],
                        );
                        ui.end_row();
                        ui.label("LineThickness:");
                        let mut t = ctrl
                            .get_prop("LineThickness")
                            .map(|v| v.as_i64())
                            .unwrap_or(1);
                        if ui
                            .add(DragValue::new(&mut t).speed(1).range(1..=32))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "LineThickness".into(),
                                PropValue::Int(t),
                            ));
                        }
                        ui.end_row();
                        combo_row(
                            ui,
                            id,
                            "LineStyle",
                            ctrl,
                            action,
                            &["Solid", "Dash", "Dot", "DashDot"],
                        );
                        ui.end_row();
                    });
                color_row(ui, id, "FillColor", ctrl, action);
                color_row(ui, id, "LineColor", ctrl, action);
                ui.add_space(4.0);
            }

            // ── MenuBar ───────────────────────────────────────────────────────
            ControlType::MenuBar => {
                section_header(ui, "Basic properties");
                if ui.button("Edit Menu...").clicked() {
                    action.open_menu_editor = Some(id.to_owned());
                }
                ui.add_space(4.0);
                section_header(ui, "Colors");
                color_row(ui, id, "HighlightBgColor", ctrl, action);
                color_row(ui, id, "HighlightFgColor", ctrl, action);
                color_row(ui, id, "SelectedBgColor", ctrl, action);
                color_row(ui, id, "SelectedFgColor", ctrl, action);
                ui.add_space(4.0);
            }

            // ── ToolBar / StatusBar ──────────────────────────────────────────
            ControlType::ToolBar | ControlType::StatusBar => {
                section_header(ui, "Items");
                let cur = ctrl
                    .get_prop("Items")
                    .map(|v| v.as_str().to_owned())
                    .unwrap_or_default();
                let buf_key = format!("{id}-Items");
                let wid = egui::Id::new(&buf_key);
                let buf = self.text_bufs.entry(buf_key).or_insert(cur.clone());
                if *buf != cur && !ui.memory(|m| m.has_focus(wid)) {
                    *buf = cur;
                }
                ui.label("Items (one per line):");
                let resp = ui.add(
                    egui::TextEdit::multiline(buf)
                        .id(wid)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY),
                );
                if resp.lost_focus() {
                    action.set_props.push((
                        id.to_owned(),
                        "Items".into(),
                        PropValue::String(buf.clone()),
                    ));
                }
                ui.add_space(4.0);
            }

            // ── Agent Object ──────────────────────────────────────────────────
            ControlType::AgentObject => {
                section_header(ui, "Basic properties");
                egui::Grid::new(format!("agt_net_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        combo_row(
                            ui,
                            id,
                            "AgentAPI",
                            ctrl,
                            action,
                            &["Ollama", "LMStudio", "OpenAI", "Anthropic", "Custom"],
                        );
                        ui.end_row();
                        ui.label("URL:");
                        let cur = ctrl
                            .get_prop("AgentURL")
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_default();
                        let bk = format!("{id}-AgentURL");
                        let wid = egui::Id::new(&bk);
                        let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                        if *buf != cur && !ui.memory(|m| m.has_focus(wid)) {
                            *buf = cur;
                        }
                        if ui
                            .add(
                                egui::TextEdit::singleline(buf)
                                    .id(wid)
                                    .hint_text("http://localhost:11434")
                                    .desired_width(f32::INFINITY),
                            )
                            .lost_focus()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "AgentURL".into(),
                                PropValue::String(buf.clone()),
                            ));
                        }
                        ui.end_row();
                        let cur = ctrl
                            .get_prop("AgentModel")
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_default();
                        let bk = format!("{id}-AgentModel");
                        let wid = egui::Id::new(&bk);
                        let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                        if *buf != cur && !ui.memory(|m| m.has_focus(wid)) {
                            *buf = cur;
                        }
                        ui.label("Model:");
                        if ui
                            .add(
                                egui::TextEdit::singleline(buf)
                                    .id(wid)
                                    .hint_text("llama3.2")
                                    .desired_width(f32::INFINITY),
                            )
                            .lost_focus()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "AgentModel".into(),
                                PropValue::String(buf.clone()),
                            ));
                        }
                        ui.end_row();
                        let cur = ctrl
                            .get_prop("AgentEndpoint")
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_default();
                        let bk = format!("{id}-AgentEndpoint");
                        let wid = egui::Id::new(&bk);
                        let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                        if *buf != cur && !ui.memory(|m| m.has_focus(wid)) {
                            *buf = cur;
                        }
                        ui.label("Endpoint:");
                        if ui
                            .add(
                                egui::TextEdit::singleline(buf)
                                    .id(wid)
                                    .hint_text("/api/chat (override)")
                                    .desired_width(f32::INFINITY),
                            )
                            .lost_focus()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "AgentEndpoint".into(),
                                PropValue::String(buf.clone()),
                            ));
                        }
                        ui.end_row();
                        let cur = ctrl
                            .get_prop("AgentAPIKey")
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_default();
                        let bk = format!("{id}-AgentAPIKey");
                        let wid = egui::Id::new(&bk);
                        let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                        if *buf != cur && !ui.memory(|m| m.has_focus(wid)) {
                            *buf = cur;
                        }
                        ui.label("API Key:");
                        if ui
                            .add(
                                egui::TextEdit::singleline(buf)
                                    .id(wid)
                                    .password(true)
                                    .desired_width(f32::INFINITY),
                            )
                            .lost_focus()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "AgentAPIKey".into(),
                                PropValue::String(buf.clone()),
                            ));
                        }
                        ui.end_row();
                    });

                section_header(ui, "Behaviour");
                {
                    let cur = ctrl
                        .get_prop("SystemPrompt")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    let bk = format!("{id}-SystemPrompt");
                    let wid = egui::Id::new(&bk);
                    let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) {
                        *buf = cur;
                    }
                    ui.label("System prompt:");
                    let resp = ui.add(
                        egui::TextEdit::multiline(buf)
                            .id(wid)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                    if resp.lost_focus() {
                        action.set_props.push((
                            id.to_owned(),
                            "SystemPrompt".into(),
                            PropValue::String(buf.clone()),
                        ));
                    }
                }
                egui::Grid::new(format!("agt_beh_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        ui.label("Temperature (0-100):");
                        let mut t = ctrl
                            .get_prop("Temperature")
                            .map(|v| v.as_i64())
                            .unwrap_or(70);
                        if ui
                            .add(DragValue::new(&mut t).speed(1).range(0..=100).suffix("%"))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "Temperature".into(),
                                PropValue::Int(t),
                            ));
                        }
                        ui.end_row();
                        ui.label("Max tokens:");
                        let mut mt = ctrl
                            .get_prop("MaximumTokens")
                            .map(|v| v.as_i64())
                            .unwrap_or(1024);
                        if ui
                            .add(DragValue::new(&mut mt).speed(10).range(1..=128000))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "MaximumTokens".into(),
                                PropValue::Int(mt),
                            ));
                        }
                        ui.end_row();
                        ui.label("Timeout (s):");
                        let mut to = ctrl
                            .get_prop("TimeoutSeconds")
                            .map(|v| v.as_i64())
                            .unwrap_or(30);
                        if ui
                            .add(DragValue::new(&mut to).speed(1).range(1..=300))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "TimeoutSeconds".into(),
                                PropValue::Int(to),
                            ));
                        }
                        ui.end_row();
                        bool_row(ui, id, "Stream", "Streaming mode", ctrl, action);
                        ui.end_row();
                    });

                section_header(ui, "COBOL Integration");
                {
                    let cur = ctrl
                        .get_prop("TargetControls")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    text_row_hint(
                        ui,
                        &mut self.text_bufs,
                        id,
                        "TargetControls",
                        &cur,
                        "Target controls:",
                        "TXT-1,LBL-2 (comma-sep IDs)",
                        action,
                    );
                }
                {
                    let cur = ctrl
                        .get_prop("ResponseDataItem")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    text_row_hint(
                        ui,
                        &mut self.text_bufs,
                        id,
                        "ResponseDataItem",
                        &cur,
                        "Response data item:",
                        "WS-AGENT-RESPONSE",
                        action,
                    );
                }
                ui.add_space(4.0);
            }

            // ── REST Client ───────────────────────────────────────────────────
            ControlType::RestClient => {
                section_header(ui, "Basic properties");
                egui::Grid::new(format!("rst_con_{id}"))
                    .num_columns(2)
                    .spacing([4.0, 3.0])
                    .show(ui, |ui| {
                        let cur = ctrl
                            .get_prop("BaseURL")
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_default();
                        let bk = format!("{id}-BaseURL");
                        let wid = egui::Id::new(&bk);
                        let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                        if *buf != cur && !ui.memory(|m| m.has_focus(wid)) {
                            *buf = cur;
                        }
                        ui.label("Base URL:");
                        if ui
                            .add(
                                egui::TextEdit::singleline(buf)
                                    .id(wid)
                                    .hint_text("https://api.example.com")
                                    .desired_width(f32::INFINITY),
                            )
                            .lost_focus()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "BaseURL".into(),
                                PropValue::String(buf.clone()),
                            ));
                        }
                        ui.end_row();
                        combo_row(
                            ui,
                            id,
                            "DefaultMethod",
                            ctrl,
                            action,
                            &["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"],
                        );
                        ui.end_row();
                        combo_row(
                            ui,
                            id,
                            "AuthType",
                            ctrl,
                            action,
                            &["None", "Bearer", "Basic", "APIKey"],
                        );
                        ui.end_row();
                        let cur = ctrl
                            .get_prop("AuthToken")
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_default();
                        let bk = format!("{id}-AuthToken");
                        let wid = egui::Id::new(&bk);
                        let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                        if *buf != cur && !ui.memory(|m| m.has_focus(wid)) {
                            *buf = cur;
                        }
                        ui.label("Auth token:");
                        if ui
                            .add(
                                egui::TextEdit::singleline(buf)
                                    .id(wid)
                                    .password(true)
                                    .desired_width(f32::INFINITY),
                            )
                            .lost_focus()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "AuthToken".into(),
                                PropValue::String(buf.clone()),
                            ));
                        }
                        ui.end_row();
                        ui.label("Timeout (s):");
                        let mut to = ctrl
                            .get_prop("TimeoutSeconds")
                            .map(|v| v.as_i64())
                            .unwrap_or(30);
                        if ui
                            .add(DragValue::new(&mut to).speed(1).range(1..=300))
                            .changed()
                        {
                            action.set_props.push((
                                id.to_owned(),
                                "TimeoutSeconds".into(),
                                PropValue::Int(to),
                            ));
                        }
                        ui.end_row();
                        bool_row(ui, id, "FollowRedirects", "Follow redirects", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "VerifyTLS", "Verify TLS cert", ctrl, action);
                        ui.end_row();
                    });
                {
                    let cur = ctrl
                        .get_prop("DefaultHeaders")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    let bk = format!("{id}-DefaultHeaders");
                    let wid = egui::Id::new(&bk);
                    let buf = self.text_bufs.entry(bk).or_insert(cur.clone());
                    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) {
                        *buf = cur;
                    }
                    ui.label("Default headers (Key: Value, one per line):");
                    let resp = ui.add(
                        egui::TextEdit::multiline(buf)
                            .id(wid)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                    if resp.lost_focus() {
                        action.set_props.push((
                            id.to_owned(),
                            "DefaultHeaders".into(),
                            PropValue::String(buf.clone()),
                        ));
                    }
                }

                section_header(ui, "COBOL Integration");
                {
                    let cur = ctrl
                        .get_prop("RequestDataItem")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    text_row_hint(
                        ui,
                        &mut self.text_bufs,
                        id,
                        "RequestDataItem",
                        &cur,
                        "Request body item:",
                        "WS-REQUEST-JSON",
                        action,
                    );
                }
                {
                    let cur = ctrl
                        .get_prop("ResponseDataItem")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    text_row_hint(
                        ui,
                        &mut self.text_bufs,
                        id,
                        "ResponseDataItem",
                        &cur,
                        "Response item:",
                        "WS-RESPONSE-JSON",
                        action,
                    );
                }
                {
                    let cur = ctrl
                        .get_prop("StatusDataItem")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    text_row_hint(
                        ui,
                        &mut self.text_bufs,
                        id,
                        "StatusDataItem",
                        &cur,
                        "HTTP status item:",
                        "WS-HTTP-STATUS",
                        action,
                    );
                }
                ui.add_space(4.0);
            }

            // ── SQL Database ──────────────────────────────────────────────────
            ControlType::SqlDatabase => {
                section_header(ui, "Basic properties");
                egui::Grid::new(format!("sql_conn_{id}"))
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("Driver:");
                        {
                            let cur = ctrl
                                .get_prop("Driver")
                                .map(|v| v.as_str().to_owned())
                                .unwrap_or_else(|| "sqlite".into());
                            egui::ComboBox::from_id_salt(format!("sql_driver_{id}"))
                                .selected_text(&cur)
                                .width(120.0)
                                .show_ui(ui, |ui| {
                                    for opt in &["sqlite", "postgres", "mysql", "mssql"] {
                                        if ui.selectable_label(&cur == opt, *opt).clicked() {
                                            action.set_props.push((
                                                id.to_owned(),
                                                "Driver".into(),
                                                PropValue::String(opt.to_string()),
                                            ));
                                        }
                                    }
                                });
                        }
                        ui.end_row();

                        let cur_cs = ctrl
                            .get_prop("ConnectionString")
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_default();
                        text_row_hint(
                            ui,
                            &mut self.text_bufs,
                            id,
                            "ConnectionString",
                            &cur_cs,
                            "Connection string:",
                            "sqlite::memory:",
                            action,
                        );
                        ui.end_row();

                        ui.label("Auto-connect:");
                        {
                            let mut v = ctrl
                                .get_prop("AutoConnect")
                                .map(|p| p.as_bool())
                                .unwrap_or(false);
                            if ui.checkbox(&mut v, "").changed() {
                                action.set_props.push((
                                    id.to_owned(),
                                    "AutoConnect".into(),
                                    PropValue::Bool(v),
                                ));
                            }
                        }
                        ui.end_row();

                        ui.label("Max connections:");
                        {
                            let mut n = ctrl
                                .get_prop("MaximumConnections")
                                .map(|v| v.as_i64())
                                .unwrap_or(5);
                            if ui.add(DragValue::new(&mut n).range(1..=100)).changed() {
                                action.set_props.push((
                                    id.to_owned(),
                                    "MaximumConnections".into(),
                                    PropValue::Int(n),
                                ));
                            }
                        }
                        ui.end_row();
                    });

                section_header(ui, "COBOL Integration");
                egui::Grid::new(format!("sql_items_{id}"))
                    .num_columns(1)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        let cur = ctrl
                            .get_prop("ConnectionDataItem")
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_default();
                        text_row_hint(
                            ui,
                            &mut self.text_bufs,
                            id,
                            "ConnectionDataItem",
                            &cur,
                            "Connection item:",
                            "conn1",
                            action,
                        );
                        ui.end_row();

                        let cur = ctrl
                            .get_prop("ResultSetDataItem")
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_default();
                        text_row_hint(
                            ui,
                            &mut self.text_bufs,
                            id,
                            "ResultSetDataItem",
                            &cur,
                            "Result set item:",
                            "resultset1",
                            action,
                        );
                        ui.end_row();
                    });
                ui.add_space(4.0);
            }

            // ── Charts ───────────────────────────────────────────────────────
            ControlType::BarChart
            | ControlType::LineChart
            | ControlType::PieChart
            | ControlType::AreaChart
            | ControlType::ScatterChart
            | ControlType::DonutChart => {
                section_header(ui, "Basic properties");

                // ── Visual ────────────────────────────────────────────────────
                egui::Grid::new(format!("chart_vis_{id}"))
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        // Title
                        let cur_title = ctrl
                            .get_prop("Title")
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_default();
                        text_row_hint(
                            ui,
                            &mut self.text_bufs,
                            id,
                            "Title",
                            &cur_title,
                            "Title:",
                            "Sales by Region",
                            action,
                        );
                        bool_row(ui, id, "ShowLegend", "Show legend", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "ShowGridLines", "Show grid lines", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "ShowXAxis", "Show X axis line", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "ShowYAxis", "Show Y axis line", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "ShowTooltips", "Show tooltips", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "AnimateOnLoad", "Animate on load", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "HideBackground", "Hide background", ctrl, action);
                        ui.end_row();
                        bool_row(ui, id, "Monochrome", "Monochrome", ctrl, action);
                        ui.end_row();
                        // Axis labels (not for pie/donut)
                        if !matches!(
                            ctrl.control_type,
                            ControlType::PieChart | ControlType::DonutChart
                        ) {
                            let cx = ctrl
                                .get_prop("XAxisLabel")
                                .map(|v| v.as_str().to_owned())
                                .unwrap_or_default();
                            text_row_hint(
                                ui,
                                &mut self.text_bufs,
                                id,
                                "XAxisLabel",
                                &cx,
                                "X-axis label:",
                                "Month",
                                action,
                            );
                            let cy = ctrl
                                .get_prop("YAxisLabel")
                                .map(|v| v.as_str().to_owned())
                                .unwrap_or_default();
                            text_row_hint(
                                ui,
                                &mut self.text_bufs,
                                id,
                                "YAxisLabel",
                                &cy,
                                "Y-axis label:",
                                "Amount",
                                action,
                            );
                        }
                    });

                // ── Monochrome base colour (spec 013) ─────────────────────────
                // When Monochrome is on: a Gradient toggle + a compact 16×16
                // swatch grid (1px pure-white internal lines, no external border,
                // no padding) to pick the base colour from the fixed 256 set.
                if ctrl
                    .get_prop("Monochrome")
                    .map(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    let cur = ctrl
                        .get_prop("MonochromeColor")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_else(|| "#3F6FB5".into());
                    let parse = |h: &str| -> (u8, u8, u8) {
                        let h = h.trim_start_matches('#');
                        (
                            u8::from_str_radix(h.get(0..2).unwrap_or("3F"), 16).unwrap_or(0x3F),
                            u8::from_str_radix(h.get(2..4).unwrap_or("6F"), 16).unwrap_or(0x6F),
                            u8::from_str_radix(h.get(4..6).unwrap_or("B5"), 16).unwrap_or(0xB5),
                        )
                    };
                    bool_row_inline(
                        ui,
                        id,
                        "MonochromeGradient",
                        "Gradient (light→dark)",
                        ctrl,
                        action,
                    );
                    ui.horizontal(|ui| {
                        ui.label("Base color:");
                        let (cr, cg, cb) = parse(&cur);
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(20.0, 14.0), egui::Sense::hover());
                        ui.painter()
                            .rect_filled(rect, 2.0, Color32::from_rgb(cr, cg, cb));
                        ui.monospace(&cur);
                    });
                    let pal = cobolt_forms::paint::chart_palette_256();
                    let cell = 11.0_f32;
                    let n = 16usize;
                    let (grid_rect, resp) = ui.allocate_exact_size(
                        egui::vec2(cell * n as f32, cell * n as f32),
                        egui::Sense::click(),
                    );
                    let p = ui.painter_at(grid_rect);
                    let cur_rgb = parse(&cur);
                    // Solid colour cells (butted together, no rounding/padding).
                    for (i, c) in pal.iter().enumerate() {
                        let cx = (i % n) as f32;
                        let cy = (i / n) as f32;
                        let cell_rect = egui::Rect::from_min_size(
                            grid_rect.min + egui::vec2(cx * cell, cy * cell),
                            egui::vec2(cell, cell),
                        );
                        p.rect_filled(cell_rect, 0.0, *c);
                        if (c.r(), c.g(), c.b()) == cur_rgb {
                            p.rect_stroke(
                                cell_rect.shrink(1.5),
                                0.0,
                                egui::Stroke::new(2.0, Color32::BLACK),
                            );
                        }
                    }
                    // Internal 1px pure-white grid lines only (no outer border).
                    for k in 1..n {
                        let x = grid_rect.min.x + k as f32 * cell;
                        p.line_segment(
                            [
                                egui::pos2(x, grid_rect.min.y),
                                egui::pos2(x, grid_rect.max.y),
                            ],
                            egui::Stroke::new(1.0, Color32::WHITE),
                        );
                        let y = grid_rect.min.y + k as f32 * cell;
                        p.line_segment(
                            [
                                egui::pos2(grid_rect.min.x, y),
                                egui::pos2(grid_rect.max.x, y),
                            ],
                            egui::Stroke::new(1.0, Color32::WHITE),
                        );
                    }
                    if resp.clicked() {
                        if let Some(pos) = resp.interact_pointer_pos() {
                            let rel = pos - grid_rect.min;
                            let cx = (rel.x / cell).floor().clamp(0.0, (n - 1) as f32) as usize;
                            let cy = (rel.y / cell).floor().clamp(0.0, (n - 1) as f32) as usize;
                            if let Some(c) = pal.get(cy * n + cx) {
                                let hex = format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b());
                                action.set_props.push((
                                    id.to_owned(),
                                    "MonochromeColor".into(),
                                    PropValue::String(hex),
                                ));
                            }
                        }
                    }
                    ui.add_space(4.0);
                }

                // ── Data Binding ──────────────────────────────────────────────
                section_header(ui, "🔗 Data Binding — COBOL Table");
                // `text_row_hint` is a self-contained full-width row (label +
                // stretch field); it must NOT be wrapped in an egui::Grid, or every
                // field packs onto one line and forces horizontal scrolling.
                // DataSource: COBOL working-storage table item name
                let ds = ctrl
                    .get_prop("DataSource")
                    .map(|v| v.as_str().to_owned())
                    .unwrap_or_default();
                text_row_hint(
                    ui,
                    &mut self.text_bufs,
                    id,
                    "DataSource",
                    &ds,
                    "Table item:",
                    "WS-SALES-TABLE",
                    action,
                );
                // Row count
                let dc = ctrl
                    .get_prop("DataCount")
                    .map(|v| v.as_str().to_owned())
                    .unwrap_or_default();
                text_row_hint(
                    ui,
                    &mut self.text_bufs,
                    id,
                    "DataCount",
                    &dc,
                    "Row count item:",
                    "WS-SALES-COUNT",
                    action,
                );
                // Field for X labels
                let lf = ctrl
                    .get_prop("LabelField")
                    .map(|v| v.as_str().to_owned())
                    .unwrap_or_default();
                text_row_hint(
                    ui,
                    &mut self.text_bufs,
                    id,
                    "LabelField",
                    &lf,
                    "Label field:",
                    "SALES-MONTH",
                    action,
                );
                // Y series fields (comma-separated)
                let vf = ctrl
                    .get_prop("ValueFields")
                    .map(|v| v.as_str().to_owned())
                    .unwrap_or_default();
                text_row_hint(
                    ui,
                    &mut self.text_bufs,
                    id,
                    "ValueFields",
                    &vf,
                    "Value field(s):",
                    "SALES-AMOUNT,SALES-BUDGET",
                    action,
                );
                // Series display labels
                let sl = ctrl
                    .get_prop("SeriesLabels")
                    .map(|v| v.as_str().to_owned())
                    .unwrap_or_default();
                text_row_hint(
                    ui,
                    &mut self.text_bufs,
                    id,
                    "SeriesLabels",
                    &sl,
                    "Series labels:",
                    "Actual,Budget",
                    action,
                );

                // ── Type-specific ─────────────────────────────────────────────
                if matches!(ctrl.control_type, ControlType::BarChart) {
                    section_header(ui, "Bar Chart Options");
                    egui::Grid::new(format!("chart_bar_{id}"))
                        .num_columns(2)
                        .spacing([8.0, 4.0])
                        .show(ui, |ui| {
                            bool_row(ui, id, "Horizontal", "Horizontal bars", ctrl, action);
                            ui.end_row();
                            bool_row(ui, id, "Stacked", "Stacked", ctrl, action);
                            ui.end_row();
                            ui.label("Corner radius:");
                            {
                                let mut v = ctrl
                                    .get_prop("BarCornerRadius")
                                    .map(|v| v.as_i64())
                                    .unwrap_or(3);
                                if ui
                                    .add(DragValue::new(&mut v).speed(1).range(0..=20))
                                    .changed()
                                {
                                    action.set_props.push((
                                        id.to_owned(),
                                        "BarCornerRadius".into(),
                                        PropValue::Int(v),
                                    ));
                                }
                            }
                            ui.end_row();
                        });
                }
                if matches!(
                    ctrl.control_type,
                    ControlType::LineChart | ControlType::AreaChart
                ) {
                    section_header(ui, "Line / Area Options");
                    egui::Grid::new(format!("chart_line_{id}"))
                        .num_columns(2)
                        .spacing([8.0, 4.0])
                        .show(ui, |ui| {
                            bool_row(ui, id, "Smooth", "Smooth curve", ctrl, action);
                            ui.end_row();
                            bool_row(ui, id, "ShowPoints", "Show points", ctrl, action);
                            ui.end_row();
                            ui.label("Point radius:");
                            {
                                let mut v = ctrl
                                    .get_prop("PointRadius")
                                    .map(|v| v.as_i64())
                                    .unwrap_or(4);
                                if ui
                                    .add(DragValue::new(&mut v).speed(1).range(0..=20))
                                    .changed()
                                {
                                    action.set_props.push((
                                        id.to_owned(),
                                        "PointRadius".into(),
                                        PropValue::Int(v),
                                    ));
                                }
                            }
                            ui.end_row();
                            if matches!(ctrl.control_type, ControlType::AreaChart) {
                                ui.label("Fill alpha (%):");
                                {
                                    let mut v = ctrl
                                        .get_prop("FillAlpha")
                                        .map(|v| v.as_i64())
                                        .unwrap_or(40);
                                    if ui
                                        .add(
                                            DragValue::new(&mut v)
                                                .speed(1)
                                                .range(0..=100)
                                                .suffix("%"),
                                        )
                                        .changed()
                                    {
                                        action.set_props.push((
                                            id.to_owned(),
                                            "FillAlpha".into(),
                                            PropValue::Int(v),
                                        ));
                                    }
                                }
                                ui.end_row();
                                bool_row(ui, id, "Stacked", "Stacked areas", ctrl, action);
                                ui.end_row();
                            }
                        });
                }
                if matches!(
                    ctrl.control_type,
                    ControlType::PieChart | ControlType::DonutChart
                ) {
                    section_header(ui, "Pie / Donut Options");
                    egui::Grid::new(format!("chart_pie_{id}"))
                        .num_columns(2)
                        .spacing([8.0, 4.0])
                        .show(ui, |ui| {
                            bool_row(ui, id, "ShowLabels", "Show labels", ctrl, action);
                            ui.end_row();
                            ui.label("Label format:");
                            let cur_lf = ctrl
                                .get_prop("LabelFormat")
                                .map(|v| v.as_str().to_owned())
                                .unwrap_or_else(|| "percent".into());
                            egui::ComboBox::from_id_salt(format!("chart_lf_{id}"))
                                .selected_text(&cur_lf)
                                .width(90.0)
                                .show_ui(ui, |ui| {
                                    for opt in &["percent", "value", "label"] {
                                        if ui.selectable_label(&cur_lf == opt, *opt).clicked() {
                                            action.set_props.push((
                                                id.to_owned(),
                                                "LabelFormat".into(),
                                                PropValue::String(opt.to_string()),
                                            ));
                                        }
                                    }
                                });
                            ui.end_row();
                            if matches!(ctrl.control_type, ControlType::DonutChart) {
                                ui.label("Inner radius (%):");
                                {
                                    let mut v = ctrl
                                        .get_prop("InnerRadius")
                                        .map(|v| v.as_i64())
                                        .unwrap_or(40);
                                    if ui
                                        .add(
                                            DragValue::new(&mut v)
                                                .speed(1)
                                                .range(10..=80)
                                                .suffix("%"),
                                        )
                                        .changed()
                                    {
                                        action.set_props.push((
                                            id.to_owned(),
                                            "InnerRadius".into(),
                                            PropValue::Int(v),
                                        ));
                                    }
                                }
                                ui.end_row();
                            }
                        });
                }
                if matches!(ctrl.control_type, ControlType::ScatterChart) {
                    section_header(ui, "Scatter / Bubble Options");
                    // Full-width row — outside the grid (see Data Binding note).
                    let bb = ctrl
                        .get_prop("BubbleField")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    text_row_hint(
                        ui,
                        &mut self.text_bufs,
                        id,
                        "BubbleField",
                        &bb,
                        "Bubble size field:",
                        "SALES-VOLUME",
                        action,
                    );
                    egui::Grid::new(format!("chart_sct_{id}"))
                        .num_columns(2)
                        .spacing([8.0, 4.0])
                        .show(ui, |ui| {
                            ui.label("Max bubble (px):");
                            {
                                let mut v = ctrl
                                    .get_prop("BubbleScale")
                                    .map(|v| v.as_i64())
                                    .unwrap_or(20);
                                if ui
                                    .add(DragValue::new(&mut v).speed(1).range(4..=60))
                                    .changed()
                                {
                                    action.set_props.push((
                                        id.to_owned(),
                                        "BubbleScale".into(),
                                        PropValue::Int(v),
                                    ));
                                }
                            }
                            ui.end_row();
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
            egui::Grid::new("form_identity")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.label(tr.lbl_name);
                    ui.label(&form.name);
                    ui.end_row();
                    ui.label(tr.lbl_size);
                    ui.label(format!("{} × {}", form.width, form.height));
                    ui.end_row();
                });
        });

        // ── COBOL Structure (spec 005) ────────────────────────────────────────
        // List of sections + user procedures; clicking a row opens the popup
        // editor for that single block.
        section_card(ui, "form-sec-cobol", tr.cs_open, true, |ui| {
            use super::cobol_structure::{section_text, CsTarget, SECTIONS};
            for t in SECTIONS {
                let kw = t.section_keyword().unwrap_or("");
                let filled = section_text(form, t)
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false);
                let dot = if filled { "● " } else { "○ " };
                if ui
                    .selectable_label(false, egui::RichText::new(format!("{dot}{kw}")).monospace())
                    .clicked()
                {
                    action.cs_open = Some(t);
                }
            }
            ui.add_space(6.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tr.cs_user_procedures).strong());
                if ui
                    .small_button(format!("➕ {}", tr.cs_add_procedure))
                    .clicked()
                {
                    action.cs_add_proc = true;
                }
            });
            for (i, up) in form.user_procedures.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.small_button("🗑").on_hover_text(tr.cs_delete).clicked() {
                        action.cs_del_proc = Some(i);
                    }
                    let name = if up.name.trim().is_empty() {
                        "(…)"
                    } else {
                        up.name.trim()
                    };
                    if ui
                        .selectable_label(
                            false,
                            egui::RichText::new(format!("▸ {name}")).monospace(),
                        )
                        .clicked()
                    {
                        action.cs_open = Some(CsTarget::Procedure(i));
                    }
                });
            }
            ui.add_space(2.0);
            ui.label(egui::RichText::new(tr.cs_hint).weak().italics());
        });

        // ── Target device ─────────────────────────────────────────────────────
        section_card(ui, "form-sec-target", tr.sec_target, true, |ui| {
            egui::Grid::new("form_target")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    use super::designer::TARGET_PRESETS;

                    ui.label(tr.lbl_target_label);
                    {
                        let cur = form.target.as_str();
                        // Show current selection + dimensions hint
                        let display = if cur == "Custom" {
                            format!("Custom ({}×{})", form.width, form.height)
                        } else {
                            // Find preset dims
                            TARGET_PRESETS
                                .iter()
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
                                    (
                                        "— Apple iPhone —",
                                        &[
                                            "iPhone 16 Pro Max",
                                            "iPhone 16 / 15 Pro",
                                            "iPhone 15 / 14",
                                            "iPhone SE (3rd gen)",
                                        ],
                                    ),
                                    (
                                        "— Apple iPad —",
                                        &[
                                            "iPad Pro 13\" (M4)",
                                            "iPad Pro 11\" (M4)",
                                            "iPad Air 13\" (M2)",
                                            "iPad (10th gen)",
                                            "iPad mini (7th gen)",
                                        ],
                                    ),
                                    (
                                        "— Apple Watch —",
                                        &[
                                            "Apple Watch Ultra 2 (49mm)",
                                            "Apple Watch Series 10 (46mm)",
                                            "Apple Watch Series 10 (42mm)",
                                        ],
                                    ),
                                    (
                                        "— Android Phone —",
                                        &[
                                            "Samsung Galaxy S24 Ultra",
                                            "Samsung Galaxy S24",
                                            "Google Pixel 9 Pro",
                                            "Android Phone (generic 1080p)",
                                        ],
                                    ),
                                    (
                                        "— Android Tablet —",
                                        &[
                                            "Samsung Galaxy Tab S9 Ultra",
                                            "Samsung Galaxy Tab S9",
                                            "Lenovo Tab P12",
                                            "Android Tablet (generic)",
                                        ],
                                    ),
                                    (
                                        "— Android SmartWatch —",
                                        &[
                                            "Samsung Galaxy Watch 7 (44mm)",
                                            "Samsung Galaxy Watch 7 (40mm)",
                                            "Wear OS (generic round)",
                                            "Wear OS (generic square)",
                                        ],
                                    ),
                                ];
                                for (header, items) in groups {
                                    ui.add_enabled(
                                        false,
                                        egui::Label::new(
                                            RichText::new(*header)
                                                .small()
                                                .color(Color32::from_rgb(140, 160, 200)),
                                        ),
                                    );
                                    for &item in *items {
                                        let dims = TARGET_PRESETS
                                            .iter()
                                            .find(|(l, ..)| *l == item)
                                            .map(|(_, w, h)| format!("  {w}×{h}"))
                                            .unwrap_or_default();
                                        let label = format!("{item}{dims}");
                                        if ui.selectable_label(cur == item, &label).clicked() {
                                            action
                                                .form_props
                                                .push(("Target".into(), item.to_owned()));
                                        }
                                    }
                                }
                            });
                    }
                    ui.end_row();

                    // Orientation hint (Portrait / Landscape swap button)
                    ui.label(tr.lbl_orientation);
                    ui.horizontal(|ui| {
                        let portrait = form.width <= form.height;
                        if ui.selectable_label(portrait, tr.lbl_portrait).clicked() && !portrait {
                            action
                                .form_props
                                .push(("Width".into(), form.height.to_string()));
                            action
                                .form_props
                                .push(("Height".into(), form.width.to_string()));
                        }
                        if ui.selectable_label(!portrait, tr.lbl_landscape).clicked() && portrait {
                            action
                                .form_props
                                .push(("Width".into(), form.height.to_string()));
                            action
                                .form_props
                                .push(("Height".into(), form.width.to_string()));
                        }
                    });
                    ui.end_row();
                });
        });

        // ── Appearance ────────────────────────────────────────────────────────
        section_card(ui, "form-sec-appearance", tr.sec_appearance, true, |ui| {
            egui::Grid::new("form_appearance")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    // Title
                    ui.label(tr.lbl_title);
                    {
                        const K: &str = "form-Title";
                        let wid = egui::Id::new(K);
                        let buf = self.form_bufs.entry(K.into()).or_insert(form.title.clone());
                        if *buf != form.title && !ui.memory(|m| m.has_focus(wid)) {
                            *buf = form.title.clone();
                        }
                        if ui
                            .add(
                                egui::TextEdit::singleline(buf)
                                    .id(wid)
                                    .desired_width(f32::INFINITY),
                            )
                            .lost_focus()
                        {
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
                                action
                                    .form_props
                                    .push(("BackgroundColor".into(), color32_to_hex(color)));
                            }
                            ui.label(
                                RichText::new(color32_to_hex(color))
                                    .monospace()
                                    .small()
                                    .color(Color32::GRAY),
                            );
                        });
                    }
                    ui.end_row();

                    // Transparency
                    ui.label(tr.lbl_transparency);
                    {
                        let mut trans = form.transparency as i64;
                        if ui
                            .add(
                                DragValue::new(&mut trans)
                                    .speed(1)
                                    .range(0..=100)
                                    .suffix("%"),
                            )
                            .changed()
                        {
                            action
                                .form_props
                                .push(("Transparency".into(), trans.to_string()));
                        }
                    }
                    ui.end_row();

                    // Grid dot spacing
                    ui.label(tr.lbl_grid_size);
                    {
                        let mut gs = form.grid_size as i64;
                        if ui
                            .add(DragValue::new(&mut gs).speed(1).range(4..=64).suffix("px"))
                            .changed()
                        {
                            action.form_props.push(("GridSize".into(), gs.to_string()));
                        }
                    }
                    ui.end_row();

                    // Snap-to-grid toggle
                    ui.label(tr.lbl_snap_to_grid);
                    {
                        let mut snapping = form.snap_to_grid;
                        if ui.checkbox(&mut snapping, "").changed() {
                            action.form_props.push((
                                "SnapToGrid".into(),
                                if snapping { "true" } else { "false" }.to_string(),
                            ));
                        }
                    }
                    ui.end_row();

                    // Theme / surface style. All three are procedural glass styles
                    // now — Neumorphic is shadow-based (no image pack), so selecting
                    // it just sets the glass style and clears any theme-pack override.
                    ui.label("Theme");
                    {
                        let cur = form.glass_style.as_str();
                        egui::ComboBox::from_id_salt("form_glass_style")
                            .selected_text(cur)
                            .width(120.0)
                            .show_ui(ui, |ui| {
                                for opt in &["Classic", "Enhanced", "Neumorphic"] {
                                    if ui.selectable_label(cur == *opt, *opt).clicked() {
                                        // Drop any image theme-pack override, then
                                        // set the procedural glass style.
                                        action.form_props.push(("Theme".into(), String::new()));
                                        action
                                            .form_props
                                            .push(("GlassStyle".into(), opt.to_string()));
                                    }
                                }
                            });
                    }
                    ui.end_row();

                    // Form theme override + themed-background toggle (spec 007) are
                    // **hidden for now**: only Liquid Glass ships as a finished look until
                    // the special asset packs are tuned. The model fields (`form.theme`,
                    // `form.use_theme_background`) and the renderer are retained — restoring
                    // these two rows re-enables the per-form chooser.
                });
        });

        // ── Background Image ──────────────────────────────────────────────────
        section_card(ui, "form-sec-bgimage", tr.sec_bg_image, true, |ui| {
            egui::Grid::new("form_bgimage")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    // Image path + browse button
                    ui.label(tr.lbl_image_path);
                    {
                        // Namespace the buffer, widget id, and file-dialog key by viewport
                        // so the in-window inspector and a detached Designer window (each a
                        // separate egui viewport) don't share state — otherwise whichever
                        // renders first in the frame steals the picker's result and the
                        // path never lands on the window the user clicked in.
                        let vp = ui.ctx().viewport_id();
                        let buf_key = format!("form-BgImage:{vp:?}");
                        let wid = egui::Id::new(&buf_key);
                        let buf = self
                            .form_bufs
                            .entry(buf_key)
                            .or_insert(form.background_image.clone());
                        if *buf != form.background_image && !ui.memory(|m| m.has_focus(wid)) {
                            *buf = form.background_image.clone();
                        }
                        ui.horizontal(|ui| {
                            let pick_k = format!("form-BgImage-pick:{vp:?}");
                            if ui.button("📂").on_hover_text("Browse for image…").clicked() {
                                crate::file_dialog::open_file(
                                    ui.ctx(),
                                    &pick_k,
                                    "Images",
                                    &["png", "jpg", "jpeg", "bmp", "gif", "ico", "webp", "svg"],
                                );
                            }
                            if crate::file_dialog::is_open(&pick_k) {
                                ui.ctx().request_repaint();
                            }
                            if let Some(Some(p)) = crate::file_dialog::take(&pick_k) {
                                let path_str = p.to_string_lossy().to_string();
                                *buf = path_str.clone();
                                action.form_props.push(("BackgroundImage".into(), path_str));
                            }
                            if ui
                                .add(
                                    egui::TextEdit::singleline(buf)
                                        .id(wid)
                                        .hint_text("/path/to/image.png")
                                        .desired_width(f32::INFINITY),
                                )
                                .lost_focus()
                            {
                                action
                                    .form_props
                                    .push(("BackgroundImage".into(), buf.clone()));
                            }
                        });
                    }
                    ui.end_row();

                    // Sizing mode
                    ui.label(tr.lbl_img_mode);
                    {
                        let cur_mode = form.bg_image_mode.as_str();
                        egui::ComboBox::from_id_salt("form_bgimage_mode")
                            .selected_text(cur_mode)
                            .width(130.0)
                            .show_ui(ui, |ui| {
                                for &opt in BgImageMode::all() {
                                    if ui.selectable_label(cur_mode == opt, opt).clicked() {
                                        action
                                            .form_props
                                            .push(("BgImageMode".into(), opt.to_owned()));
                                    }
                                }
                            });
                    }
                    ui.end_row();
                });
            ui.label(
                RichText::new(
                    "Stretch = fill exactly  •  Fill = crop to fill  •  Fit = letterbox\n\
             Center = original size  •  Tile = repeat",
                )
                .small()
                .color(Color32::GRAY)
                .italics(),
            );
        });

        // ── Size ──────────────────────────────────────────────────────────────
        section_card(ui, "form-sec-size", tr.sec_size, true, |ui| {
            egui::Grid::new("form_size")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.label(tr.lbl_width);
                    {
                        const K: &str = "form-Width";
                        let wid = egui::Id::new(K);
                        let buf = self
                            .form_bufs
                            .entry(K.into())
                            .or_insert(form.width.to_string());
                        if !ui.memory(|m| m.has_focus(wid)) {
                            *buf = form.width.to_string();
                        }
                        if ui
                            .add(egui::TextEdit::singleline(buf).id(wid).desired_width(70.0))
                            .lost_focus()
                        {
                            action.form_props.push(("Width".into(), buf.clone()));
                        }
                    }
                    ui.end_row();

                    ui.label(tr.lbl_height);
                    {
                        const K: &str = "form-Height";
                        let wid = egui::Id::new(K);
                        let buf = self
                            .form_bufs
                            .entry(K.into())
                            .or_insert(form.height.to_string());
                        if !ui.memory(|m| m.has_focus(wid)) {
                            *buf = form.height.to_string();
                        }
                        if ui
                            .add(egui::TextEdit::singleline(buf).id(wid).desired_width(70.0))
                            .lost_focus()
                        {
                            action.form_props.push(("Height".into(), buf.clone()));
                        }
                    }
                    ui.end_row();
                });
        });

        // ── Form-level Events ─────────────────────────────────────────────────
        section_card(ui, "form-sec-events", tr.sec_form_events, true, |ui| {
            ui.label(
                RichText::new(tr.hint_click_event)
                    .small()
                    .color(Color32::GRAY)
                    .italics(),
            );
            ui.add_space(4.0);

            // All supported form events, grouped by category (collapsible). A group
            // that has any handler-with-code starts expanded; others collapsed.
            for &(group, events) in cobolt_forms::model::FORM_EVENT_GROUPS {
                let any_code = events.iter().any(|ev| {
                    form.form_events
                        .iter()
                        .any(|e| e.event == *ev && e.has_code())
                });
                egui::CollapsingHeader::new(
                    RichText::new(group).strong().color(Color32::from_gray(170)),
                )
                .id_salt(format!("form-evgrp-{group}"))
                .default_open(any_code)
                .show(ui, |ui| {
                    for &ev_name in events {
                        let binding = form.form_events.iter().find(|e| e.event == ev_name);
                        let has_code = binding.map(|e| e.has_code()).unwrap_or(false);
                        let lines = binding.map(|e| e.code_line_count()).unwrap_or(0);

                        let row_resp = ui.horizontal(|ui| {
                            let dot_color = if has_code {
                                Color32::from_rgb(100, 220, 100)
                            } else {
                                Color32::from_rgb(120, 120, 120)
                            };
                            ui.label(
                                RichText::new(if has_code { "●" } else { "○" }).color(dot_color),
                            );
                            let lbl = ui
                                .add(
                                    egui::Label::new(
                                        RichText::new(format!("⚙ {ev_name}"))
                                            .color(Color32::from_rgb(200, 200, 100)),
                                    )
                                    .sense(egui::Sense::click()),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text(tr.hint_dblclick_event);
                            if has_code {
                                ui.label(
                                    RichText::new(format!("({lines} {})", tr.hint_lines))
                                        .small()
                                        .color(Color32::GRAY),
                                );
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
            }
        });

        ui.add_space(8.0);
        ui.label(
            RichText::new(tr.hint_click_control)
                .italics()
                .color(Color32::GRAY),
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
        .outer_margin(egui::Margin {
            left: 4.0,
            right: 0.0,
            top: 0.0,
            bottom: 4.0,
        })
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_max_width(ui.available_width());
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(3.5, 17.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, egui::Rounding::same(2.0), theme.accent);
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
        .outer_margin(egui::Margin {
            left: 4.0,
            right: 0.0,
            top: 0.0,
            bottom: 8.0,
        })
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_max_width(ui.available_width());
            let id = ui.make_persistent_id(id_salt);
            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                default_open,
            )
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
    let hex = ctrl
        .get_prop(key)
        .map(|v| v.as_str().to_owned())
        .unwrap_or_else(|| "#F0F0F0".to_owned());
    let mut color = hex_to_color32(&hex);
    ui.horizontal(|ui| {
        ui.label(key);
        if color_edit_button_closing(ui, &mut color).changed() {
            let new_hex = color32_to_hex(color);
            action
                .set_props
                .push((id.to_owned(), key.to_owned(), PropValue::String(new_hex)));
        }
        ui.label(
            RichText::new(color32_to_hex(color))
                .monospace()
                .small()
                .color(Color32::GRAY),
        );
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
            action.set_props.push((
                ctrl_id.to_owned(),
                prop_key.to_owned(),
                PropValue::String(buf.clone()),
            ));
        }
    });
}

fn datagrid_color_modal_row(
    ui: &mut Ui,
    id: &str,
    key: &str,
    label: &str,
    ctrl: &Control,
    action: &mut InspectorAction,
    fallback: &str,
) {
    ui.label(label);
    let hex = ctrl
        .get_prop(key)
        .map(|v| v.as_str().to_owned())
        .unwrap_or_else(|| fallback.to_owned());
    let mut color = hex_to_color32(&hex);
    ui.horizontal(|ui| {
        if color_edit_button_closing(ui, &mut color).changed() {
            action.set_props.push((
                id.to_owned(),
                key.to_owned(),
                PropValue::String(color32_to_hex(color)),
            ));
        }
        ui.label(
            RichText::new(color32_to_hex(color))
                .monospace()
                .small()
                .color(Color32::GRAY),
        );
    });
}

fn datagrid_text_modal_row(
    ui: &mut Ui,
    id: &str,
    key: &str,
    label: &str,
    ctrl: &Control,
    action: &mut InspectorAction,
    fallback: &str,
) {
    ui.label(label);
    let mut value = ctrl
        .get_prop(key)
        .map(|v| v.as_str().to_owned())
        .unwrap_or_else(|| fallback.to_owned());
    if ui
        .add(egui::TextEdit::singleline(&mut value).desired_width(180.0))
        .changed()
    {
        action
            .set_props
            .push((id.to_owned(), key.to_owned(), PropValue::String(value)));
    }
}

fn datagrid_int_modal_row(
    ui: &mut Ui,
    id: &str,
    key: &str,
    label: &str,
    ctrl: &Control,
    action: &mut InspectorAction,
    range: std::ops::RangeInclusive<i64>,
    fallback: i64,
) {
    ui.label(label);
    let mut value = ctrl.get_prop(key).map(|v| v.as_i64()).unwrap_or(fallback);
    if ui
        .add(DragValue::new(&mut value).speed(1).range(range))
        .changed()
    {
        action
            .set_props
            .push((id.to_owned(), key.to_owned(), PropValue::Int(value)));
    }
}

fn datagrid_advanced_int_modal_row(
    ui: &mut Ui,
    id: &str,
    key: &str,
    label: &str,
    ctrl: &Control,
    action: &mut InspectorAction,
    range: std::ops::RangeInclusive<i64>,
    fallback: i64,
) {
    ui.label(label);
    let advanced = DataGridAdvanced::from_control(ctrl);
    let mut value = match key {
        "RowHeight" => advanced.row_height as i64,
        "FrozenColumns" => advanced.frozen_columns as i64,
        "FrozenRows" => advanced.frozen_rows as i64,
        _ => ctrl.get_prop(key).map(|v| v.as_i64()).unwrap_or(fallback),
    };
    if ui
        .add(DragValue::new(&mut value).speed(1).range(range))
        .changed()
    {
        action
            .set_props
            .push((id.to_owned(), key.to_owned(), PropValue::Int(value)));
        let mut updated = DataGridAdvanced::from_control(ctrl);
        match key {
            "RowHeight" => updated.row_height = value.max(1).min(u16::MAX as i64) as u16,
            "FrozenColumns" => updated.frozen_columns = value.max(0) as usize,
            "FrozenRows" => updated.frozen_rows = value.max(0) as usize,
            _ => {}
        }
        if let Ok(json) = updated.to_json() {
            action.set_props.push((
                id.to_owned(),
                DATAGRID_ADVANCED_PROP.to_owned(),
                PropValue::String(json),
            ));
        }
    }
}

fn datagrid_grid_line_style_modal_row(
    ui: &mut Ui,
    id: &str,
    ctrl: &Control,
    action: &mut InspectorAction,
) {
    ui.label("Grid line style");
    let current = DataGridAdvanced::from_control(ctrl).grid_line_style;
    egui::ComboBox::from_id_salt(format!("cb_{id}_GridLineStyle"))
        .selected_text(current.as_str())
        .width(140.0)
        .show_ui(ui, |ui| {
            for opt in ["Solid", "Dash", "Dots", "None"] {
                if ui.selectable_label(current.as_str() == opt, opt).clicked() {
                    action.set_props.push((
                        id.to_owned(),
                        "GridLineStyle".to_owned(),
                        PropValue::String(opt.to_owned()),
                    ));
                    let mut updated = DataGridAdvanced::from_control(ctrl);
                    updated.grid_line_style = DataGridGridLineStyle::from_str(opt);
                    if let Ok(json) = updated.to_json() {
                        action.set_props.push((
                            id.to_owned(),
                            DATAGRID_ADVANCED_PROP.to_owned(),
                            PropValue::String(json),
                        ));
                    }
                }
            }
        });
}

/// Bool property — grid cell style (label in left col, checkbox in right).
fn bool_row(
    ui: &mut Ui,
    ctrl_id: &str,
    key: &str,
    label: &str,
    ctrl: &Control,
    action: &mut InspectorAction,
) {
    ui.label(label);
    let mut v = ctrl.get_prop(key).map(|p| p.as_bool()).unwrap_or(false);
    if ui.checkbox(&mut v, "").changed() {
        action
            .set_props
            .push((ctrl_id.to_owned(), key.to_owned(), PropValue::Bool(v)));
    }
}

/// Bool property — inline horizontal style.
fn bool_row_inline(
    ui: &mut Ui,
    ctrl_id: &str,
    key: &str,
    label: &str,
    ctrl: &Control,
    action: &mut InspectorAction,
) {
    let mut v = ctrl.get_prop(key).map(|p| p.as_bool()).unwrap_or(false);
    if ui.checkbox(&mut v, label).changed() {
        action
            .set_props
            .push((ctrl_id.to_owned(), key.to_owned(), PropValue::Bool(v)));
    }
}

/// Inline integer editor (label + DragValue), clamped to `range`.
fn int_row_inline(
    ui: &mut Ui,
    ctrl_id: &str,
    key: &str,
    label: &str,
    ctrl: &Control,
    action: &mut InspectorAction,
    range: std::ops::RangeInclusive<i64>,
) {
    let mut v = ctrl.get_prop(key).map(|p| p.as_i64()).unwrap_or(0);
    ui.horizontal(|ui| {
        ui.label(label);
        if ui
            .add(DragValue::new(&mut v).speed(1).range(range))
            .changed()
        {
            action
                .set_props
                .push((ctrl_id.to_owned(), key.to_owned(), PropValue::Int(v)));
        }
    });
}

/// Combo row — grid cell style.
fn combo_row(
    ui: &mut Ui,
    ctrl_id: &str,
    key: &str,
    ctrl: &Control,
    action: &mut InspectorAction,
    opts: &[&str],
) {
    let cur = ctrl
        .get_prop(key)
        .map(|v| v.as_str().to_owned())
        .unwrap_or_else(|| opts[0].to_owned());
    ui.label(key);
    egui::ComboBox::from_id_salt(format!("cb_{ctrl_id}_{key}"))
        .selected_text(&cur)
        .width(140.0)
        .show_ui(ui, |ui| {
            for &opt in opts {
                if ui.selectable_label(cur == opt, opt).clicked() {
                    action.set_props.push((
                        ctrl_id.to_owned(),
                        key.to_owned(),
                        PropValue::String(opt.to_owned()),
                    ));
                }
            }
        });
}

fn combo_row_labeled(
    ui: &mut Ui,
    ctrl_id: &str,
    key: &str,
    label: &str,
    ctrl: &Control,
    action: &mut InspectorAction,
    opts: &[&str],
) {
    let cur = ctrl
        .get_prop(key)
        .map(|v| v.as_str().to_owned())
        .unwrap_or_else(|| opts[0].to_owned());
    ui.label(label);
    egui::ComboBox::from_id_salt(format!("cb_{ctrl_id}_{key}"))
        .selected_text(&cur)
        .width(140.0)
        .show_ui(ui, |ui| {
            for &opt in opts {
                if ui.selectable_label(cur == opt, opt).clicked() {
                    action.set_props.push((
                        ctrl_id.to_owned(),
                        key.to_owned(),
                        PropValue::String(opt.to_owned()),
                    ));
                }
            }
        });
}

/// Combo row — inline horizontal style.
fn combo_row_inline(
    ui: &mut Ui,
    ctrl_id: &str,
    key: &str,
    ctrl: &Control,
    action: &mut InspectorAction,
    opts: &[&str],
) {
    let cur = ctrl
        .get_prop(key)
        .map(|v| v.as_str().to_owned())
        .unwrap_or_else(|| opts[0].to_owned());
    ui.horizontal(|ui| {
        ui.label(key);
        egui::ComboBox::from_id_salt(format!("cbi_{ctrl_id}_{key}"))
            .selected_text(&cur)
            .width(140.0)
            .show_ui(ui, |ui| {
                for &opt in opts {
                    if ui.selectable_label(cur == opt, opt).clicked() {
                        action.set_props.push((
                            ctrl_id.to_owned(),
                            key.to_owned(),
                            PropValue::String(opt.to_owned()),
                        ));
                    }
                }
            });
    });
}

fn user_control_child_controls<'a>(form: &'a Form, root_id: &str) -> Vec<&'a Control> {
    fn collect<'a>(form: &'a Form, parent_id: &str, out: &mut Vec<&'a Control>) {
        let mut children: Vec<&Control> = form
            .controls
            .iter()
            .filter(|ctrl| {
                ctrl.parent
                    .as_deref()
                    .map(|parent| parent.eq_ignore_ascii_case(parent_id))
                    .unwrap_or(false)
            })
            .collect();
        children.sort_by(|a, b| a.z_order.cmp(&b.z_order).then_with(|| a.id.cmp(&b.id)));

        for child in children {
            out.push(child);
            collect(form, &child.id, out);
        }
    }

    let mut children = Vec::new();
    collect(form, root_id, &mut children);
    children
}

fn child_prop_row(
    ui: &mut Ui,
    bufs: &mut std::collections::HashMap<String, String>,
    ctrl_id: &str,
    key: &str,
    value: &PropValue,
    action: &mut InspectorAction,
) {
    ui.label(format!("{ctrl_id}.{key} ="));
    match value {
        PropValue::Bool(current) => {
            let mut v = *current;
            if ui.checkbox(&mut v, "").changed() {
                action
                    .set_props
                    .push((ctrl_id.to_owned(), key.to_owned(), PropValue::Bool(v)));
            }
        }
        PropValue::Int(current) => {
            let mut v = *current;
            if ui.add(DragValue::new(&mut v).speed(1)).changed() {
                action
                    .set_props
                    .push((ctrl_id.to_owned(), key.to_owned(), PropValue::Int(v)));
            }
        }
        PropValue::String(current) => {
            let buf_key = format!("uc-child:{ctrl_id}:{key}");
            let widget_id = egui::Id::new(&buf_key);
            let buf = bufs.entry(buf_key).or_insert_with(|| current.clone());
            if *buf != *current && !ui.memory(|m| m.has_focus(widget_id)) {
                *buf = current.clone();
            }
            if ui
                .add(
                    egui::TextEdit::singleline(buf)
                        .id(widget_id)
                        .desired_width(f32::INFINITY),
                )
                .lost_focus()
            {
                action.set_props.push((
                    ctrl_id.to_owned(),
                    key.to_owned(),
                    PropValue::String(buf.clone()),
                ));
            }
        }
    }
    ui.end_row();
}

/// Border colour + style rows.
fn border_rows(
    ui: &mut Ui,
    ctrl_id: &str,
    ctrl: &Control,
    action: &mut InspectorAction,
    _bufs: &mut std::collections::HashMap<String, String>,
) {
    if ctrl.get_prop("BorderColor").is_some() {
        color_row(ui, ctrl_id, "BorderColor", ctrl, action);
    }
    if ctrl.get_prop("BorderStyle").is_some() {
        combo_row_inline(
            ui,
            ctrl_id,
            "BorderStyle",
            ctrl,
            action,
            &["None", "Single", "Fixed3D", "Raised", "Sunken"],
        );
    }
    if ctrl.get_prop("BorderWidth").is_some() {
        int_row_inline(
            ui,
            ctrl_id,
            "BorderWidth",
            "Border width",
            ctrl,
            action,
            0..=10,
        );
    }
}

/// Browse button + text field for an image path property.
fn image_browse_row(
    ui: &mut Ui,
    ctrl_id: &str,
    key: &str,
    ctrl: &Control,
    action: &mut InspectorAction,
    bufs: &mut std::collections::HashMap<String, String>,
) {
    let cur = ctrl
        .get_prop(key)
        .map(|v| v.as_str().to_owned())
        .unwrap_or_default();
    // Namespace by viewport so the in-window inspector and a detached Designer
    // window don't share buffer/dialog state for the same control (see the form
    // background picker for the rationale).
    let vp = ui.ctx().viewport_id();
    let buf_key = format!("{ctrl_id}-{key}:{vp:?}");
    let wid = egui::Id::new(&buf_key);
    let buf = bufs.entry(buf_key).or_insert(cur.clone());
    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) {
        *buf = cur;
    }
    let pick_key = format!("imgpick:{ctrl_id}:{key}:{vp:?}");
    ui.horizontal(|ui| {
        ui.label(key);
        // Open the native picker asynchronously — a synchronous dialog nests the
        // OS event loop and aborts winit 0.30.
        if ui.button("📂").on_hover_text("Browse for image…").clicked() {
            crate::file_dialog::open_file(
                ui.ctx(),
                &pick_key,
                "Images",
                &["png", "jpg", "jpeg", "bmp", "gif", "ico", "webp", "svg"],
            );
        }
        // Keep repainting while the dialog is open so the result is collected.
        if crate::file_dialog::is_open(&pick_key) {
            ui.ctx().request_repaint();
        }
        if let Some(Some(p)) = crate::file_dialog::take(&pick_key) {
            let path_str = p.to_string_lossy().to_string();
            *buf = path_str.clone();
            action.set_props.push((
                ctrl_id.to_owned(),
                key.to_owned(),
                PropValue::String(path_str),
            ));
        }
        if ui
            .add(
                egui::TextEdit::singleline(buf)
                    .id(wid)
                    .hint_text("(none)")
                    .desired_width(f32::INFINITY),
            )
            .lost_focus()
        {
            action.set_props.push((
                ctrl_id.to_owned(),
                key.to_owned(),
                PropValue::String(buf.clone()),
            ));
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
    let cur = ctrl
        .get_prop("Items")
        .map(|v| v.as_str().to_owned())
        .unwrap_or_default();
    let buf_key = format!("{ctrl_id}-Items");
    let wid = egui::Id::new(&buf_key);
    let buf = bufs.entry(buf_key).or_insert(cur.clone());
    if *buf != cur && !ui.memory(|m| m.has_focus(wid)) {
        *buf = cur;
    }
    ui.label("Items (one per line):");
    let resp = ui.add(
        egui::TextEdit::multiline(buf)
            .id(wid)
            .desired_rows(4)
            .desired_width(f32::INFINITY),
    );
    if resp.lost_focus() {
        action.set_props.push((
            ctrl_id.to_owned(),
            "Items".into(),
            PropValue::String(buf.clone()),
        ));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn form_with_cobol_binding_table() -> (Form, Control) {
        let mut form = Form::new("CustomerForm", "CustomerForm", 800, 600);
        form.user_ws_source = "\
01 WS-CUSTOMER-TABLE GLOBAL.
   05 WS-CUSTOMER-ROW OCCURS 100 TIMES.
      10 CUSTOMER-ID   PIC 9(06).
      10 CUSTOMER-NAME PIC X(40).
      10 BALANCE       PIC S9(9)V99.
      10 ACTIVE        PIC X.
      10 CATEGORY-ID   PIC 9(04).
      10 REGION-ID     PIC 9(04).
01 WS-SCALAR GLOBAL PIC X(10).
"
        .to_owned();
        let grid = Control::new("CustomerGrid", ControlType::DataGrid, 10, 10);
        form.controls.push(grid.clone());
        (form, grid)
    }

    fn select_customer_cobol_table(editor: &mut BindingEditorState) {
        editor.selected_cobol_table = "WS-CUSTOMER-TABLE".to_owned();
        editor.rows = editor.rows_for_selected_cobol_table(&[]);
    }

    #[test]
    fn control_array_editor_maps_fields_to_member_properties() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        let mut group = Control::new("CARD", ControlType::GroupBox, 0, 0);
        group.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        let mut label = Control::new("NAME-LBL", ControlType::Label, 10, 10);
        label.parent = Some("CARD".into());
        let mut photo = Control::new("PHOTO", ControlType::PictureBox, 10, 40);
        photo.parent = Some("CARD".into());
        form.controls = vec![group.clone(), label, photo];

        let mut editor = BindingEditorState::new(&form, &group, BindingEditorSourceKind::Sql, &[])
            .expect("editor should open for a repeating GroupBox");
        // Member controls are discovered with their default bindable property.
        assert!(
            editor
                .member_controls
                .iter()
                .any(|m| m.id == "PHOTO" && m.property == "ImagePath"),
            "PictureBox member should default to its ImagePath property"
        );
        assert!(editor.rows.len() >= 2, "sample SQL source should have rows");
        editor.rows[0].target_member = "NAME-LBL".to_owned();
        editor.rows[1].target_member = "PHOTO".to_owned();

        let binding = editor.to_binding(&form).expect("binding builds");
        // Only mapped fields produce mappings, each to the chosen member+property.
        assert_eq!(binding.mappings.len(), 2);
        let image = binding.mappings.iter().find_map(|m| match &m.target {
            BindingTargetPath::ControlProperty {
                control_id,
                property_name,
                ..
            } if control_id == "PHOTO" => Some(property_name.clone()),
            _ => None,
        });
        assert_eq!(image.as_deref(), Some("ImagePath"));
    }

    #[test]
    fn from_existing_reopens_saved_binding_config_for_editing() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        let mut group = Control::new("CARD", ControlType::GroupBox, 0, 0);
        group.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        let mut label = Control::new("NAME-LBL", ControlType::Label, 10, 10);
        label.parent = Some("CARD".into());
        form.controls = vec![group.clone(), label];

        // Configure + save a binding through the editor.
        let mut editor = BindingEditorState::new(&form, &group, BindingEditorSourceKind::Sql, &[])
            .expect("editor opens");
        editor.display_name = "Customers -> CARD".to_owned();
        editor.binding_id = "BND-CARD".to_owned();
        editor.selected_sql_control = "SQL-CUST".to_owned();
        let field = editor.rows[0].source_field.clone();
        editor.rows[0].target_member = "NAME-LBL".to_owned();
        let binding = editor.to_binding(&form).expect("binding builds");
        form.data_bindings.push(binding);

        // Reopening the editor must restore the saved config, not start blank.
        let existing = form
            .binding_for_control("CARD")
            .cloned()
            .expect("existing binding found for the bound control");
        let reopened = BindingEditorState::from_existing(&form, &group, &existing, &[])
            .expect("editor reopens from the saved binding");
        assert_eq!(reopened.binding_id, "BND-CARD");
        assert_eq!(reopened.display_name, "Customers -> CARD");
        assert_eq!(reopened.selected_source, Some(BindingEditorSourceKind::Sql));
        assert_eq!(reopened.selected_sql_control, "SQL-CUST");
        assert!(
            reopened.rows.iter().any(|r| r.source_field == field),
            "persisted source fields should be restored as rows"
        );
        assert!(
            reopened.rows.iter().any(|r| r.target_member == "NAME-LBL"),
            "the field→member mapping should be restored"
        );
    }

    #[test]
    fn source_button_opens_and_keeps_editor_for_repeating_groupbox() {
        // A repeating GroupBox whose ArrayName differs from its control id used to
        // key the editor by the array name, so the modal opened then instantly
        // cleared (the source button "did nothing") and apply couldn't resolve the
        // target. The editor must be keyed by the control id.
        let mut form = Form::new("MAIN", "Main", 800, 600);
        let mut group = Control::new("CARD", ControlType::GroupBox, 0, 0);
        group.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        group.set_prop("ArrayName", PropValue::String("CUSTOMERS".into()));
        let mut member = Control::new("NAME", ControlType::TextBox, 10, 10);
        member.parent = Some("CARD".into());
        form.controls = vec![group.clone(), member];

        let editor = BindingEditorState::new(&form, &group, BindingEditorSourceKind::Sql, &[])
            .expect("editor should open for a repeating GroupBox");
        assert_eq!(
            editor.target_control_id, "CARD",
            "editor must be keyed by the control id, not the array name"
        );
        let binding = editor
            .to_binding(&form)
            .expect("editor must build a binding at apply time");
        assert!(matches!(
            binding.target,
            cobolt_forms::BindingTargetDescriptor::ControlArray { .. }
        ));
        assert_eq!(binding.target.primary_control_id(), "CUSTOMERS");
    }

    #[test]
    fn data_binding_editor_requires_indexed_backend_for_indexed_source() {
        let mut form = Form::new("CustomerForm", "CustomerForm", 800, 600);
        let grid = Control::new("CustomerGrid", ControlType::DataGrid, 10, 10);
        form.controls.push(grid.clone());

        let without_files =
            BindingEditorState::new(&form, &grid, BindingEditorSourceKind::IndexedFile, &[])
                .expect("DataGrid should be an approved binding target");
        assert_eq!(without_files.selected_source, None);

        let with_files = BindingEditorState::new(
            &form,
            &grid,
            BindingEditorSourceKind::IndexedFile,
            &["CUSTOMER-DATA (CUSTDAT)".to_owned()],
        )
        .expect("DataGrid should be an approved binding target");
        assert_eq!(
            with_files.selected_source,
            Some(BindingEditorSourceKind::IndexedFile)
        );
        assert_eq!(with_files.selected_indexed_file, "CUSTOMER-DATA (CUSTDAT)");
    }

    #[test]
    fn data_binding_editor_validates_sample_dropdown_configuration() {
        let rows = sample_indexed_field_rows();
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[4].source_field, "CITY-ID");
        assert_eq!(rows[4].edit_control, BindingEditControl::Dropdown);
        assert_eq!(rows[4].dropdown.summary(), "Indexed file");
        assert!(rows[4].dropdown.validate("CITY-ID").is_ok());
        assert_eq!(rows[5].source_field, "STATUS");
        assert_eq!(rows[5].dropdown.source_ref, "STATUS-CODES (STATCDS)");
        assert!(rows[5].dropdown.validate("STATUS").is_ok());
    }

    #[test]
    fn data_binding_editor_initializes_sql_source_rows_and_dropdowns() {
        let mut form = Form::new("CustomerForm", "CustomerForm", 800, 600);
        let grid = Control::new("CustomerGrid", ControlType::DataGrid, 10, 10);
        form.controls.push(grid.clone());

        let editor = BindingEditorState::new(&form, &grid, BindingEditorSourceKind::Sql, &[])
            .expect("DataGrid should be an approved binding target");

        assert_eq!(editor.selected_source, Some(BindingEditorSourceKind::Sql));
        assert_eq!(editor.selected_sql_control, "SQL-CUSTOMERS");
        assert_eq!(editor.rows.len(), 6);
        assert_eq!(editor.rows[0].source_field, "CUSTOMER_ID");
        assert_eq!(editor.rows[0].cobol_mask, "9(9)");
        assert!(editor.rows[0].key);
        assert_eq!(editor.rows[4].source_field, "COUNTRY_ID");
        assert_eq!(
            editor.rows[4].dropdown.origin,
            Some(DropdownOrigin::SqlControl)
        );
        assert_eq!(editor.rows[4].dropdown.source_ref, "SQL-COUNTRIES");
        assert_eq!(editor.rows[4].dropdown.line_limit, 1000);
        assert_eq!(editor.rows[5].source_field, "TIER_ID");
        assert_eq!(
            editor.rows[5].dropdown.origin,
            Some(DropdownOrigin::CobolTable)
        );
        assert_eq!(editor.rows[5].dropdown.source_ref, "CUSTOMER_TIERS");
        assert!(editor.validate().is_ok());
    }

    #[test]
    fn data_binding_editor_initializes_cobol_table_source_rows_and_dropdowns() {
        let (form, grid) = form_with_cobol_binding_table();

        let mut editor =
            BindingEditorState::new(&form, &grid, BindingEditorSourceKind::CobolTable, &[])
                .expect("DataGrid should be an approved binding target");

        assert_eq!(
            editor.selected_source,
            Some(BindingEditorSourceKind::CobolTable)
        );
        assert_eq!(editor.selected_cobol_table, "");
        assert!(editor.rows.is_empty());
        assert_eq!(editor.cobol_tables.len(), 1);
        select_customer_cobol_table(&mut editor);
        assert_eq!(editor.selected_cobol_occurs_item(), "WS-CUSTOMER-ROW");
        assert_eq!(editor.rows.len(), 6);
        assert_eq!(editor.rows[0].source_field, "CUSTOMER-ID");
        assert_eq!(editor.rows[0].picture, "9(6)");
        assert_eq!(editor.rows[0].cobol_mask, "PIC 9(6)");
        assert_eq!(editor.rows[4].source_field, "CATEGORY-ID");
        assert_eq!(editor.rows[4].edit_control, BindingEditControl::Textbox);
        assert_eq!(editor.rows[5].source_field, "REGION-ID");
        assert!(editor.validate().is_ok());
    }

    #[test]
    fn data_binding_editor_rejects_invalid_cobol_table_settings() {
        let (form, grid) = form_with_cobol_binding_table();
        let mut editor =
            BindingEditorState::new(&form, &grid, BindingEditorSourceKind::CobolTable, &[])
                .expect("DataGrid should be an approved binding target");

        editor.selected_cobol_table.clear();
        assert_eq!(
            editor.validate().unwrap_err(),
            "COBOL table must be selected."
        );

        editor.selected_cobol_table = "WS-NOT-OCCURS".to_owned();
        assert_eq!(
            editor.validate().unwrap_err(),
            "Selected COBOL table must resolve to a 01-level GLOBAL item with OCCURS."
        );

        editor.selected_cobol_table = "WS-CUSTOMER-TABLE".to_owned();
        editor.rows = editor.rows_for_selected_cobol_table(&[]);
        editor.rows[4].edit_control = BindingEditControl::Dropdown;
        editor.rows[4].dropdown = DropdownConfig::category_cobol_table();
        editor.rows[4].dropdown.value_field.clear();
        assert_eq!(
            editor.validate().unwrap_err(),
            "CATEGORY-ID needs a dropdown value field."
        );
    }

    #[test]
    fn data_binding_editor_builds_cobol_table_descriptor_from_selected_table() {
        let (form, grid) = form_with_cobol_binding_table();
        let mut editor =
            BindingEditorState::new(&form, &grid, BindingEditorSourceKind::CobolTable, &[])
                .expect("DataGrid should be an approved binding target");
        select_customer_cobol_table(&mut editor);
        editor.rows[0].key = true;

        let binding = editor
            .to_binding(&form)
            .expect("valid COBOL table editor should build a binding");

        match binding.source {
            BindingSourceDescriptor::CobolTable {
                table_name,
                occurs_item,
                fields,
                key_fields,
                writable,
            } => {
                assert_eq!(table_name, "WS-CUSTOMER-TABLE");
                assert_eq!(occurs_item, "WS-CUSTOMER-ROW");
                assert_eq!(fields.len(), 6);
                assert_eq!(fields[0].name, "CUSTOMER-ID");
                assert_eq!(fields[4].display_name, "Category Id");
                assert_eq!(key_fields, vec!["CUSTOMER-ID".to_owned()]);
                assert!(writable);
            }
            other => panic!("expected COBOL table source, got {other:?}"),
        }
    }

    #[test]
    fn data_binding_editor_lists_only_missing_cobol_table_fields_for_add() {
        let (form, grid) = form_with_cobol_binding_table();
        let mut editor =
            BindingEditorState::new(&form, &grid, BindingEditorSourceKind::CobolTable, &[])
                .expect("DataGrid should be an approved binding target");
        select_customer_cobol_table(&mut editor);

        assert!(
            editor.missing_cobol_table_fields().is_empty(),
            "all table fields are mapped immediately after selecting the table"
        );

        let removed = editor.rows.remove(2);
        assert_eq!(removed.source_field, "BALANCE");
        editor.removed_rows.push(removed);
        let missing = editor.missing_cobol_table_fields();
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].name, "BALANCE");

        editor.rows.push(missing[0].to_row());
        assert!(
            editor.missing_cobol_table_fields().is_empty(),
            "Add field choices disappear once the missing table field is mapped again"
        );
    }

    #[test]
    fn data_binding_editor_initializes_rest_source_rows_and_preview_state() {
        let mut form = Form::new("CustomerForm", "CustomerForm", 800, 600);
        let grid = Control::new("ProductGrid", ControlType::DataGrid, 10, 10);
        form.controls.push(grid.clone());

        let editor = BindingEditorState::new(&form, &grid, BindingEditorSourceKind::RestApi, &[])
            .expect("DataGrid should be an approved binding target");

        assert_eq!(
            editor.selected_source,
            Some(BindingEditorSourceKind::RestApi)
        );
        assert_eq!(editor.rest_endpoint, "https://api.example.com/v1/products");
        assert_eq!(editor.rest_method, RestMethod::Get);
        assert!(editor.rest_headers.is_empty());
        assert_eq!(editor.rest_auth.mode, RestAuthMode::None);
        assert!(editor.show_jsonpath_help);
        assert_eq!(editor.rows.len(), 4);
        assert_eq!(editor.rows[0].source_field, "$.[*].title");
        assert_eq!(editor.rows[0].picture, "PIC X(60)");
        assert_eq!(editor.rows[0].cobol_mask, "X(60)");
        assert_eq!(editor.rows[2].source_field, "$.[*].category");
        assert_eq!(editor.rows[2].edit_control, BindingEditControl::Dropdown);
        assert_eq!(editor.rows[3].edit_control, BindingEditControl::Checkbox);
        assert!(rest_response_preview_json().contains("\"Wireless Headphones\""));
        assert!(editor.validate().is_ok());
    }

    #[test]
    fn data_binding_editor_rejects_invalid_rest_settings() {
        let mut form = Form::new("CustomerForm", "CustomerForm", 800, 600);
        let grid = Control::new("ProductGrid", ControlType::DataGrid, 10, 10);
        form.controls.push(grid.clone());
        let mut editor =
            BindingEditorState::new(&form, &grid, BindingEditorSourceKind::RestApi, &[])
                .expect("DataGrid should be an approved binding target");

        editor.rest_endpoint = "not-a-url".to_owned();
        assert_eq!(
            editor.validate().unwrap_err(),
            "REST API endpoint must be a valid URL."
        );

        editor.rest_endpoint = "https://api.example.com/v1/products".to_owned();
        editor.rest_headers.push(HttpHeaderRow {
            name: String::new(),
            value: "application/json".to_owned(),
        });
        assert_eq!(
            editor.validate().unwrap_err(),
            "Header names must be filled when a value is provided."
        );

        editor.rest_headers.clear();
        editor.rows[0].source_field = "title".to_owned();
        assert_eq!(
            editor.validate().unwrap_err(),
            "title is not a valid JSONPath expression."
        );

        editor.rows[0].source_field = "$.[*].title".to_owned();
        editor.rest_auth.mode = RestAuthMode::ApiKey;
        assert_eq!(
            editor.validate().unwrap_err(),
            "API key authentication needs key name and key value."
        );
    }

    #[test]
    fn data_binding_editor_builds_rest_descriptor_from_endpoint() {
        let mut form = Form::new("CustomerForm", "CustomerForm", 800, 600);
        let grid = Control::new("ProductGrid", ControlType::DataGrid, 10, 10);
        form.controls.push(grid.clone());
        let editor = BindingEditorState::new(&form, &grid, BindingEditorSourceKind::RestApi, &[])
            .expect("DataGrid should be an approved binding target");

        let binding = editor
            .to_binding(&form)
            .expect("valid REST editor should build a binding");

        match binding.source {
            BindingSourceDescriptor::RestApi {
                source_control_id,
                endpoint_name,
                response_data_item,
                fields,
                update,
            } => {
                assert_eq!(source_control_id, "REST-API");
                assert_eq!(endpoint_name, "https://api.example.com/v1/products");
                assert_eq!(response_data_item, "REST-RESPONSE");
                assert_eq!(fields.len(), 4);
                assert_eq!(fields[0].name, "$.[*].title");
                assert_eq!(fields[2].display_name, "Category");
                assert!(update.is_none());
            }
            other => panic!("expected REST API source, got {other:?}"),
        }
    }

    #[test]
    fn data_binding_dropdown_origin_reset_uses_latest_indexed_lookup_mock() {
        let mut config = DropdownConfig::country_sql();
        config.reset_for_origin(DropdownOrigin::IndexedFile);

        assert_eq!(config.source_ref, "COUNTRY-CODES (CNTRYIDX)");
        assert_eq!(config.display_field, "COUNTRY_NAME (X(100))");
        assert_eq!(config.value_field, "COUNTRY_ID (9(5))");
        assert_eq!(
            dropdown_source_options(DropdownOrigin::IndexedFile),
            &["COUNTRY-CODES (CNTRYIDX)", "REGION-CODES (REGIONST)"]
        );
    }

    #[test]
    fn data_binding_editor_rejects_incomplete_field_mapping() {
        let mut form = Form::new("CustomerForm", "CustomerForm", 800, 600);
        let grid = Control::new("CustomerGrid", ControlType::DataGrid, 10, 10);
        form.controls.push(grid.clone());
        let mut editor = BindingEditorState::new(
            &form,
            &grid,
            BindingEditorSourceKind::IndexedFile,
            &["CUSTOMER-DATA (CUSTDAT)".to_owned()],
        )
        .expect("DataGrid should be an approved binding target");

        editor.rows[0].friendly_name.clear();
        assert!(editor.validate().is_err());

        editor.rows[0].friendly_name = "Customer ID".to_owned();
        editor.rows[4].dropdown.value_field.clear();
        assert!(editor.validate().is_err());

        editor.rows[4].dropdown.value_field = "CITY-ID (9(04))".to_owned();
        editor.rows[4].dropdown.line_limit = 0;
        assert!(editor.validate().is_err());
    }

    #[test]
    fn user_control_child_controls_collects_nested_descendants() {
        let mut form = Form::new("Main", "Main", 640, 480);

        let mut root = Control::new("AddressBlock-1", ControlType::GroupBox, 10, 10);
        root.properties.insert(
            "UserControl".to_owned(),
            PropValue::String("AddressBlock".to_owned()),
        );

        let mut street = Control::new("AddressBlock-1-Street", ControlType::TextBox, 20, 20);
        street.parent = Some("AddressBlock-1".to_owned());
        street.z_order = 2;

        let mut phone = Control::new("AddressBlock-1-Phone", ControlType::GroupBox, 20, 48);
        phone.parent = Some("AddressBlock-1".to_owned());
        phone.z_order = 1;

        let mut phone_button =
            Control::new("AddressBlock-1-Phone-Button", ControlType::Button, 24, 52);
        phone_button.parent = Some("AddressBlock-1-Phone".to_owned());

        let mut sibling = Control::new("Other", ControlType::Button, 100, 100);
        sibling.parent = None;

        form.controls = vec![root, street, phone, phone_button, sibling];

        let ids: Vec<&str> = user_control_child_controls(&form, "addressblock-1")
            .into_iter()
            .map(|ctrl| ctrl.id.as_str())
            .collect();

        assert_eq!(
            ids,
            vec![
                "AddressBlock-1-Phone",
                "AddressBlock-1-Phone-Button",
                "AddressBlock-1-Street"
            ]
        );
    }
}
