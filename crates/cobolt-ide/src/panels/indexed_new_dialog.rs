// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! New Indexed File wizard dialog (R3, R8).

use cobolt_indexed::{
    AccessMode, IndexedDefinition, IndexedField, KeyDef, KeyEncodingDef, KeyOrderingDef,
    KeyPartDef, KeySchema, RecordFormatDef, StorageMode,
};

use crate::i18n::Tr;

/// Wizard state for creating a new `.cidx` definition.
///
/// Supports two modes:
/// - Form (properties pane): the original wizard fields.
/// - Code (COBOL text editor): raw COBOL-85-like record definition text.
///
/// Once the user switches to Code mode for a file, the form is no longer
/// offered for that creation (locked to editor as requested).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewAlternateKey {
    pub name: String,
    pub offset: String,
    pub length: String,
    pub duplicates: bool,
}

pub struct NewIndexedDialog {
    pub open: bool,
    pub name: String,
    pub assign_path: String,
    pub variable: bool,
    pub access_mode: AccessMode,
    pub storage: StorageMode,
    pub compression: bool,
    pub persistence: bool,
    pub key_field: String,
    pub key_offset: String,
    pub key_length: String,
    pub alternates: Vec<NewAlternateKey>,

    /// When true, the user is editing via the built-in COBOL text editor
    /// instead of the properties form (Step 2 of the wizard).
    pub raw_mode: bool,
    /// The COBOL-like record text (for raw_mode).
    pub raw_text: String,
    /// Live validation error for the raw text (populated on changes / Create check).
    pub raw_error: Option<String>,
}

impl NewIndexedDialog {
    pub fn new() -> Self {
        Self {
            open: false,
            name: "CUSTOMER-FILE".into(),
            assign_path: "data/customers.idx".into(),
            variable: false,
            access_mode: AccessMode::Dynamic,
            storage: StorageMode::Disk,
            compression: false,
            persistence: false,
            key_field: "RECORD-KEY".into(),
            key_offset: "0".into(),
            key_length: "8".into(),
            alternates: Vec::new(),
            raw_mode: false,
            raw_text: String::new(),
            raw_error: None,
        }
    }

    /// Build an unfinalized definition from the wizard fields.
    pub fn build_definition(&self) -> Option<IndexedDefinition> {
        let name = self.name.trim();
        if name.is_empty() {
            return None;
        }
        let key_off: u32 = self.key_offset.parse().ok()?;
        let key_len: u32 = self.key_length.parse().ok()?;
        if key_len == 0 {
            return None;
        }
        let record_len = key_off + key_len;
        let record_format = if self.variable {
            RecordFormatDef::Variable {
                min_length: key_len,
                max_length: record_len,
            }
        } else {
            RecordFormatDef::Fixed { length: record_len }
        };
        let key_name = if self.key_field.trim().is_empty() {
            "RECORD-KEY".into()
        } else {
            self.key_field.trim().to_ascii_uppercase().replace(' ', "-")
        };
        let mut alternates = Vec::new();
        for alt in &self.alternates {
            let alt_name = alt.name.trim();
            if alt_name.is_empty() {
                continue;
            }
            let alt_off = alt.offset.parse().unwrap_or(0);
            let alt_len = alt.length.parse().unwrap_or(1);
            let alt_key_name = alt_name.to_ascii_uppercase().replace(' ', "-");
            alternates.push(KeyDef {
                name: Some(alt_key_name.clone()),
                parts: vec![KeyPartDef {
                    field_name: alt_key_name.clone(),
                    offset: alt_off,
                    length: alt_len,
                    encoding: KeyEncodingDef::Bytes,
                }],
                duplicates_allowed: alt.duplicates,
                ordering: KeyOrderingDef::Ascending,
            });
        }
        let logical = name.to_ascii_uppercase().replace(' ', "-");
        let mut def = IndexedDefinition {
            name: logical.clone(),
            assign_path: self.assign_path.trim().into(),
            access_mode: self.access_mode,
            record_format,
            storage: self.storage,
            compression: self.compression,
            persistence: self.persistence,
            comment: String::new(),
            keys: KeySchema {
                primary: KeyDef {
                    name: Some(key_name.clone()),
                    parts: vec![KeyPartDef {
                        field_name: key_name.clone(),
                        offset: key_off,
                        length: key_len,
                        encoding: KeyEncodingDef::Bytes,
                    }],
                    duplicates_allowed: false,
                    ordering: KeyOrderingDef::Ascending,
                },
                alternates,
            },
            fields: vec![IndexedField {
                level: 1,
                name: format!("{logical}-RECORD"),
                pic: String::new(),
                usage: cobolt_indexed::FieldUsage::Display,
                offset: None,
                length: None,
                comment: String::new(),
                grid_control: None,
                occurs: None,
                redefines: None,
                synchronized: false,
                children: {
                    let mut children = vec![IndexedField {
                        level: 5,
                        name: key_name,
                        pic: format!("X({key_len})"),
                        usage: cobolt_indexed::FieldUsage::Display,
                        offset: Some(key_off),
                        length: Some(key_len),
                        comment: String::new(),
                        grid_control: None,
                        occurs: None,
                        redefines: None,
                        synchronized: false,
                        children: Vec::new(),
                    }];
                    for alt in &self.alternates {
                        let alt_name = alt.name.trim();
                        if alt_name.is_empty() {
                            continue;
                        }
                        let alt_off = alt.offset.parse().unwrap_or(0);
                        let alt_len = alt.length.parse().unwrap_or(1);
                        let alt_key_name = alt_name.to_ascii_uppercase().replace(' ', "-");
                        children.push(IndexedField {
                            level: 5,
                            name: alt_key_name.clone(),
                            pic: format!("X({alt_len})"),
                            usage: cobolt_indexed::FieldUsage::Display,
                            offset: Some(alt_off),
                            length: Some(alt_len),
                            comment: String::new(),
                            grid_control: None,
                            occurs: None,
                            redefines: None,
                            synchronized: false,
                            children: Vec::new(),
                        });
                    }
                    children
                },
            }],
            finalized: true,
        };
        cobolt_indexed::apply_default_controls(&mut def.fields);
        def.recompute_offsets();
        Some(def)
    }

    /// Returns a valid definition if the current form (or raw text) is complete
    /// and compliant (has record group + RECORD KEY, passes COBOL-85 record syntax
    /// via the indexed parser + structural validation).
    pub fn get_definition(&self) -> Option<IndexedDefinition> {
        if self.raw_mode {
            self.get_definition_from_raw()
        } else {
            self.build_definition()
        }
    }

    fn get_definition_from_raw(&self) -> Option<IndexedDefinition> {
        if self.raw_text.trim().is_empty() {
            return None;
        }
        // Start from skeleton using the dialog's top-level settings (name, assign,
        // access, storage, record format, keys). The raw text supplies the detailed
        // field layout (the 01 group and children).
        let name = self.name.trim();
        if name.is_empty() {
            return None;
        }
        let record_format = if self.variable {
            RecordFormatDef::Variable {
                min_length: 0,
                max_length: 0,
            }
        } else {
            RecordFormatDef::Fixed { length: 0 }
        };
        let key_off = self.key_offset.parse().ok()?;
        let key_len = self.key_length.parse().ok()?;
        if key_len == 0 {
            return None;
        }
        let key_name = if self.key_field.trim().is_empty() {
            "RECORD-KEY".into()
        } else {
            self.key_field.trim().to_ascii_uppercase().replace(' ', "-")
        };
        let mut alternates = Vec::new();
        for alt in &self.alternates {
            let alt_name = alt.name.trim();
            if alt_name.is_empty() {
                continue;
            }
            let alt_off = alt.offset.parse().unwrap_or(0);
            let alt_len = alt.length.parse().unwrap_or(1);
            let alt_key_name = alt_name.to_ascii_uppercase().replace(' ', "-");
            alternates.push(KeyDef {
                name: Some(alt_key_name.clone()),
                parts: vec![KeyPartDef {
                    field_name: alt_key_name.clone(),
                    offset: alt_off,
                    length: alt_len,
                    encoding: KeyEncodingDef::Bytes,
                }],
                duplicates_allowed: alt.duplicates,
                ordering: KeyOrderingDef::Ascending,
            });
        }
        let logical = name.to_ascii_uppercase().replace(' ', "-");
        let mut def = IndexedDefinition {
            name: logical.clone(),
            assign_path: self.assign_path.trim().into(),
            access_mode: self.access_mode,
            record_format,
            storage: self.storage,
            compression: self.compression,
            persistence: self.persistence,
            comment: String::new(),
            keys: KeySchema {
                primary: KeyDef {
                    name: Some(key_name.clone()),
                    parts: vec![KeyPartDef {
                        field_name: key_name.clone(),
                        offset: key_off,
                        length: key_len,
                        encoding: KeyEncodingDef::Bytes,
                    }],
                    duplicates_allowed: false,
                    ordering: KeyOrderingDef::Ascending,
                },
                alternates,
            },
            fields: vec![],
            finalized: true,
        };
        // Parse the user's raw COBOL-85-like record text into the fields.
        // This enforces the record description syntax (levels, PIC, REDEFINES,
        // OCCURS, USAGE etc.).
        if let Err(e) = cobolt_indexed::raw_text::text_to_record(&mut def, &self.raw_text) {
            return None;
        }
        // Structural validation (indent, REDEFINES order per prior COBOL-85 rules, etc.).
        if cobolt_indexed::validate_definition(&def).is_err() {
            return None;
        }
        // Must have at least a data item group (01-level root) and a record key.
        let has_group = def.record_root().is_some();
        let has_key = def.keys.primary.name.is_some() && !def.keys.primary.parts.is_empty();
        if !has_group || !has_key {
            return None;
        }
        // Ensure the declared primary and alternate key fields actually exist in the record layout
        let mut keys_exist = true;
        if let Some(kn) = &def.keys.primary.name {
            let exists = def.fields.iter().any(|f| f.name.eq_ignore_ascii_case(kn))
                || def
                    .fields
                    .iter()
                    .flat_map(|f| &f.children)
                    .any(|c| c.name.eq_ignore_ascii_case(kn));
            if !exists {
                keys_exist = false;
            }
        }
        for alt in &def.keys.alternates {
            if let Some(kn) = &alt.name {
                let exists = def.fields.iter().any(|f| f.name.eq_ignore_ascii_case(kn))
                    || def
                        .fields
                        .iter()
                        .flat_map(|f| &f.children)
                        .any(|c| c.name.eq_ignore_ascii_case(kn));
                if !exists {
                    keys_exist = false;
                    break;
                }
            }
        }
        if !keys_exist {
            return None;
        }
        cobolt_indexed::apply_default_controls(&mut def.fields);
        def.recompute_offsets();
        Some(def)
    }

    /// Live (or on-demand) validation for raw mode. Sets self.raw_error.
    pub fn validate_raw(&mut self) {
        if !self.raw_mode {
            self.raw_error = None;
            return;
        }
        if let Some(_) = self.get_definition_from_raw() {
            self.raw_error = None;
        } else if self.raw_text.trim().is_empty() {
            self.raw_error = Some("Enter a COBOL-85 record description (01 group + key).".into());
        } else {
            // Provide a generic or more specific error by attempting parse.
            let mut temp = IndexedDefinition {
                name: "TEMP".into(),
                assign_path: String::new(),
                access_mode: self.access_mode,
                record_format: if self.variable {
                    RecordFormatDef::Variable {
                        min_length: 0,
                        max_length: 0,
                    }
                } else {
                    RecordFormatDef::Fixed { length: 0 }
                },
                storage: self.storage,
                compression: self.compression,
                persistence: self.persistence,
                comment: String::new(),
                keys: KeySchema {
                    primary: KeyDef {
                        name: Some(self.key_field.clone()),
                        parts: vec![],
                        duplicates_allowed: false,
                        ordering: KeyOrderingDef::Ascending,
                    },
                    alternates: vec![],
                },
                fields: vec![],
                finalized: true,
            };
            match cobolt_indexed::raw_text::text_to_record(&mut temp, &self.raw_text) {
                Ok(()) => {
                    temp.recompute_offsets();
                    if let Err(e) = cobolt_indexed::validate_definition(&temp) {
                        self.raw_error = Some(e);
                    } else {
                        self.raw_error = Some("Definition parsed but is missing a record group (01) or compliant RECORD KEY.".into());
                    }
                }
                Err(e) => {
                    self.raw_error = Some(e);
                }
            }
        }
    }

    /// Whether the Create button should be enabled.
    /// Conditions:
    /// 1. The file at the informed assign_path does not exist on disk.
    /// 2. The definition (form or raw code) is complete + valid COBOL-85 record
    ///    (at least one data item group + compliant primary record key).
    pub fn can_create(&self) -> bool {
        let p = std::path::Path::new(self.assign_path.trim());
        if p.exists() {
            return false;
        }
        self.get_definition().is_some()
    }

    pub fn show(&mut self, ui: &mut egui::Ui, tr: &Tr) -> NewIndexedAction {
        let mut action = NewIndexedAction::None;

        ui.vertical_centered(|ui| {
            ui.heading(if self.raw_mode {
                "Step 2: Edit Record Structure (COBOL-85)"
            } else {
                "Step 1: Set File Properties"
            });
        });
        ui.add_space(4.0);

        if self.raw_mode {
            // Raw COBOL-85 record editor (no intellisense, as specified).
            // The text must define at least the 01 group + key field(s).
            ui.label(tr.lbl_raw_record_hint);
            ui.add_space(2.0);
            let te = egui::TextEdit::multiline(&mut self.raw_text)
                .code_editor()
                .desired_rows(14)
                .desired_width(ui.available_width());
            let resp = ui.add(te);
            if resp.changed() {
                self.validate_raw();
            }
            if let Some(e) = &self.raw_error {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 80, 80),
                    format!("Parse / validation error: {}", e),
                );
                ui.label("Fix the COBOL-85 record description before creating the file.");
            } else if !self.raw_text.trim().is_empty() {
                ui.colored_label(
                    egui::Color32::from_rgb(80, 180, 80),
                    "Valid COBOL-85 record definition (group + RECORD KEY present).",
                );
            }
        } else {
            // Original properties form (form mode)
            ui.label(tr.dlg_indexed_name);
            ui.text_edit_singleline(&mut self.name);
            ui.label(tr.dlg_assign_path);
            ui.text_edit_singleline(&mut self.assign_path);
            ui.label(tr.dlg_access_mode);
            egui::ComboBox::from_id_salt("idx_access")
                .selected_text(access_label(self.access_mode, tr))
                .show_ui(ui, |ui| {
                    for mode in [
                        AccessMode::Dynamic,
                        AccessMode::Sequential,
                        AccessMode::Random,
                    ] {
                        ui.selectable_value(&mut self.access_mode, mode, access_label(mode, tr));
                    }
                });
            ui.label(tr.dlg_record_format);
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.variable, false, tr.dlg_record_fixed);
                ui.radio_value(&mut self.variable, true, tr.dlg_record_variable);
            });

            ui.label(tr.dlg_storage_mode);
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.storage, StorageMode::Disk, tr.dlg_storage_disk);
                ui.radio_value(
                    &mut self.storage,
                    StorageMode::Memory,
                    tr.dlg_storage_memory,
                );
            });
            ui.checkbox(&mut self.compression, tr.dlg_compression);
            if self.storage == StorageMode::Memory {
                ui.checkbox(&mut self.persistence, tr.dlg_persistence);
            }
        }

        ui.separator();
        ui.label(tr.dlg_primary_key);
        let mut key_changed = false;
        ui.horizontal(|ui| {
            ui.label(tr.dlg_primary_key_field);
            if ui.text_edit_singleline(&mut self.key_field).changed() {
                key_changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr.dlg_primary_key_offset);
            if ui.text_edit_singleline(&mut self.key_offset).changed() {
                key_changed = true;
            }
            ui.label(tr.dlg_primary_key_length);
            if ui.text_edit_singleline(&mut self.key_length).changed() {
                key_changed = true;
            }
        });

        ui.separator();
        ui.label("Alternate Keys");
        let mut remove_idx = None;
        for (idx, alt) in self.alternates.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Alt Key #{}:", idx + 1));
                    if ui.button("🗑").clicked() {
                        remove_idx = Some(idx);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Field name:");
                    if ui.text_edit_singleline(&mut alt.name).changed() {
                        key_changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Offset:");
                    if ui.text_edit_singleline(&mut alt.offset).changed() {
                        key_changed = true;
                    }
                    ui.label("Length:");
                    if ui.text_edit_singleline(&mut alt.length).changed() {
                        key_changed = true;
                    }
                    if ui.checkbox(&mut alt.duplicates, "Duplicates").changed() {
                        key_changed = true;
                    }
                });
            });
        }
        if let Some(idx) = remove_idx {
            self.alternates.remove(idx);
            key_changed = true;
        }
        if ui.button("+ Add Alternate Key").clicked() {
            self.alternates.push(NewAlternateKey {
                name: format!("ALT-KEY-{}", self.alternates.len() + 1),
                offset: "0".into(),
                length: "8".into(),
                duplicates: true,
            });
            key_changed = true;
        }
        if key_changed && self.raw_mode {
            self.validate_raw();
        }

        ui.separator();

        // Check if file already exists at assign path
        let p = std::path::Path::new(self.assign_path.trim());
        let path_exists = p.exists();
        if path_exists {
            ui.colored_label(
                egui::Color32::from_rgb(220, 80, 80),
                "The file already exists at the informed path. Choose a different assign path.",
            );
        }

        if self.raw_mode {
            // Validation warnings in raw mode
            let can = self.can_create();
            if !can && !path_exists {
                if self.raw_error.is_some() {
                    ui.colored_label(egui::Color32::from_rgb(220, 80, 80), "Fix COBOL-85 errors in the record text (must define at least one 01 group and a compliant RECORD KEY).");
                } else {
                    ui.colored_label(
                        egui::Color32::from_rgb(200, 160, 60),
                        "Complete the definition (valid group + key) to enable Create.",
                    );
                }
            }

            ui.horizontal(|ui| {
                if ui.button("Back").clicked() {
                    self.raw_mode = false;
                }
                if ui
                    .add_enabled(can, egui::Button::new(tr.dlg_create))
                    .clicked()
                {
                    action = NewIndexedAction::Create;
                }
                if ui.button(tr.dlg_cancel).clicked() {
                    action = NewIndexedAction::Cancel;
                }
            });
        } else {
            let can_next = !self.name.trim().is_empty() && !self.assign_path.trim().is_empty() && !path_exists;
            ui.horizontal(|ui| {
                if ui.add_enabled(can_next, egui::Button::new("Next")).clicked() {
                    self.raw_mode = true;
                    // Populate initial text from current form state (so user can refine).
                    if let Some(d) = self.build_definition() {
                        self.raw_text = cobolt_indexed::raw_text::record_to_text(&d);
                    } else if self.raw_text.trim().is_empty() {
                        // Fallback minimal valid skeleton the user can edit.
                        let kname = if self.key_field.trim().is_empty() {
                            "RECORD-KEY"
                        } else {
                            &self.key_field
                        };
                        let klen = self.key_length.parse().unwrap_or(8);
                        self.raw_text = format!(
                            "01 {}-RECORD.\n    05 {} PIC X({}).\n",
                            self.name.trim().to_ascii_uppercase().replace(' ', "-"),
                            kname.to_ascii_uppercase().replace(' ', "-"),
                            klen
                        );
                    }
                    self.validate_raw();
                }
                if ui.button(tr.dlg_cancel).clicked() {
                    action = NewIndexedAction::Cancel;
                }
            });
        }

        action
    }
}

fn access_label(mode: AccessMode, tr: &Tr) -> &'static str {
    match mode {
        AccessMode::Dynamic => tr.idx_access_dynamic,
        AccessMode::Sequential => tr.idx_access_sequential,
        AccessMode::Random => tr.idx_access_random,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NewIndexedAction {
    None,
    Create,
    Cancel,
}
