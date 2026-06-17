// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Indexed structure tree + properties (used by the inline inspector in the main IDE pane).
//! The separate pop-up editor window has been removed.

use cobolt_forms::ControlType;
use cobolt_indexed::{
    apply_flat, flatten_record, indent_entry, outdent_entry, record_to_text, text_to_record,
    FieldUsage, FlatEntry, IndexedDefinition, IndexedField,
};
use crate::theme;

use crate::i18n::Tr;
use super::indexed_properties::{IndexedPropertiesPanel, PropertyEdit};

const INDENT_PX: f32 = 18.0;

#[derive(Clone, PartialEq, Eq)]
pub enum IndexedSelection {
    File,
    Field(String),
}

pub struct IndexedEditorPanel {
    pub selection: IndexedSelection,
    pub flat: Vec<FlatEntry>,
    pub raw_text: String,
    pub show_raw_dialog: bool,
    /// Set by the app when we want the raw dialog to open on the next inspector show
    /// (e.g. file was created via the COBOL editor; we want the editor "visible"
    /// instead of the properties form).
    pub request_raw_dialog: bool,
    pub show_indent_alert: bool,
    pub indent_error: Option<String>,
}

impl IndexedEditorPanel {
    pub fn new() -> Self {
        Self {
            selection: IndexedSelection::File,
            flat: Vec::new(),
            raw_text: String::new(),
            show_raw_dialog: false,
            request_raw_dialog: false,
            show_indent_alert: false,
            indent_error: None,
        }
    }

    pub fn select_field(&mut self, id: impl Into<String>) {
        self.selection = IndexedSelection::Field(id.into());
    }

    pub fn sync_from_def(&mut self, def: &IndexedDefinition) {
        self.flat = flatten_record(def);
        if !matches!(self.selection, IndexedSelection::File) {
            if let IndexedSelection::Field(ref name) = self.selection {
                if !self.flat.iter().any(|e| e.field.name == *name) {
                    self.selection = IndexedSelection::File;
                }
            }
        }
    }



    /// Left pane — indexed file root + record structure tree.
    pub fn show_structure(
        &mut self,
        ui: &mut egui::Ui,
        def: &IndexedDefinition,
        tr: &Tr,
    ) -> StructureAction {
        let mut action = StructureAction::None;
        if self.flat.is_empty() {
            self.sync_from_def(def);
        }

        let mut indent_cmd = IndentCommand::None;
        let mut clicked_field: Option<String> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            let file_sel = self.selection == IndexedSelection::File;
            ui.horizontal(|ui| {
                ui.monospace(if file_sel { ">" } else { " " });
                let resp = ui.selectable_label(file_sel, tr.sec_indexed_file_props);
                if resp.clicked() {
                    self.selection = IndexedSelection::File;
                }
            });

            for (idx, entry) in self.flat.iter().enumerate() {
                let selected = matches!(
                    &self.selection,
                    IndexedSelection::Field(n) if n == &entry.field.name
                );
                let pad = entry.depth as f32 * INDENT_PX;
                let label = entry.field.name.clone();

                // Color COBOL data-items using the theme's data-item color (ed_data).
                // This is the same palette used for syntax highlighting data names in the COBOL editor,
                // so it stays visible and consistent whether the active theme is lighter or darker.
                let th = theme::active();
                let is_key = def.keys.primary.name.as_deref() == Some(entry.field.name.as_str())
                    || def.keys.primary.parts.iter().any(|p| p.field_name == entry.field.name)
                    || def.keys.alternates.iter().any(|a| a.name.as_deref() == Some(entry.field.name.as_str()));
                let mut item_color = th.ed_keyword;  // blue (replaces the previous green ed_data for data-items)
                if is_key {
                    // Keys pop a bit more but remain theme-visible (use a warmer tint on dark themes).
                    if th.dark {
                        item_color = egui::Color32::from_rgb(255, 210, 130);
                    }
                }

                ui.horizontal(|ui| {
                    ui.monospace(if selected { ">" } else { " " });
                    ui.add_space(pad);
                    let mut text = egui::RichText::new(&label).color(item_color);
                    if is_key {
                        text = text.strong();
                    }
                    let resp = ui.selectable_label(selected, text);
                    let clicked = resp.clicked();
                    if !def.finalized {
                        resp.on_hover_text(tr.lbl_idx_indent_hint);
                    }
                    if clicked {
                        clicked_field = Some(entry.field.name.clone());
                    }
                });
                if selected && !def.finalized {
                    indent_cmd = read_indent_keys(ui);
                    let _ = idx;
                }
            }
        });

        if let Some(name) = clicked_field {
            self.selection = IndexedSelection::Field(name);
        }
        if indent_cmd != IndentCommand::None && !def.finalized {
            if let IndexedSelection::Field(name) = self.selection.clone() {
                if let Some(idx) = self.flat.iter().position(|e| e.field.name == name) {
                    match indent_cmd {
                        IndentCommand::Indent => {
                            indent_entry(&mut self.flat, idx);
                            action = StructureAction::StructureChanged;
                        }
                        IndentCommand::Outdent => {
                            if !outdent_entry(&mut self.flat, idx) {
                                self.indent_error = Some(tr.err_idx_illegal_indent.into());
                                self.show_indent_alert = true;
                            } else {
                                action = StructureAction::StructureChanged;
                            }
                        }
                        IndentCommand::None => {}
                    }
                }
            }
        }
        action
    }



    pub fn open_raw_dialog(&mut self, def: &IndexedDefinition) {
        self.raw_text = record_to_text(def);
        self.show_raw_dialog = true;
    }

    pub fn show_raw_dialog(&mut self, ctx: &egui::Context, def: &mut IndexedDefinition, tr: &Tr) -> RawDialogResult {
        let mut result = RawDialogResult::None;
        if !self.show_raw_dialog {
            return result;
        }
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new(tr.dlg_raw_record_title)
            .collapsible(false)
            .resizable(true)
            .default_size([520.0, 360.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(tr.lbl_raw_record_hint);
                ui.add(
                    egui::TextEdit::multiline(&mut self.raw_text)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .desired_rows(16),
                );
                ui.horizontal(|ui| {
                    if ui.button(tr.btn_apply_raw).clicked() {
                        apply = true;
                    }
                    // "Edit in panes" / switch back removed per requirement: once raw editor used for
                    // a file, the editor must remain the visible form; no going back to property pane
                    // for structural editing.
                    if ui.button(tr.btn_cancel).clicked() {
                        cancel = true;
                    }
                });
            });
        if cancel || !open {
            self.show_raw_dialog = false;
            result = RawDialogResult::Cancelled;
        }
        if apply {
            match text_to_record(def, &self.raw_text) {
                Ok(()) => {
                    self.sync_from_def(def);
                    self.show_raw_dialog = false;
                    result = RawDialogResult::Applied;
                }
                Err(e) => {
                    self.indent_error = Some(e);
                    self.show_indent_alert = true;
                }
            }
        }
        result
    }

    /// Render the raw COBOL-85 record descriptor editor directly in the main
    /// inspector pane (used when the file has been edited via the embedded
    /// editor; the property pane form is replaced by the editor per requirements).
    /// Returns true if the user clicked Apply and the parse succeeded.
    pub fn show_raw_editor_inline(
        &mut self,
        ui: &mut egui::Ui,
        def: &mut IndexedDefinition,
        tr: &Tr,
    ) -> bool {
        let mut applied = false;

        ui.label(tr.lbl_raw_record_hint);
        ui.add_space(2.0);

        let te = egui::TextEdit::multiline(&mut self.raw_text)
            .code_editor()
            .desired_rows(20)
            .desired_width(f32::INFINITY);
        ui.add(te);

        ui.horizontal(|ui| {
            if ui.button(tr.btn_apply_raw).clicked() {
                match text_to_record(def, &self.raw_text) {
                    Ok(()) => {
                        self.sync_from_def(def);
                        self.indent_error = None;
                        self.show_indent_alert = false;
                        applied = true;
                    }
                    Err(e) => {
                        self.indent_error = Some(e);
                        self.show_indent_alert = true;
                    }
                }
            }
            if ui.button("Reload from current structure").clicked() {
                self.raw_text = record_to_text(def);
                self.indent_error = None;
                self.show_indent_alert = false;
            }
        });

        if let Some(e) = &self.indent_error {
            ui.colored_label(egui::Color32::from_rgb(220, 80, 80), format!("COBOL-85 validation error: {}", e));
        }

        applied
    }

    /// Middle pane — property labels for the current selection.
    pub fn show_property_labels(
        &self,
        ui: &mut egui::Ui,
        def: &IndexedDefinition,
        tr: &Tr,
    ) {
        match &self.selection {
            IndexedSelection::File => IndexedPropertiesPanel::show_file_labels(ui, tr),
            IndexedSelection::Field(id) => {
                IndexedPropertiesPanel::show_field_labels(ui, def, id, tr);
            }
        }
    }

    /// Right pane — property value widgets for the current selection.
    pub fn show_property_values(
        &mut self,
        ui: &mut egui::Ui,
        def: &mut IndexedDefinition,
        tr: &Tr,
    ) -> PropertyEdit {
        let edit = match &self.selection {
            IndexedSelection::File => IndexedPropertiesPanel::show_file_values(ui, def, tr),
            IndexedSelection::Field(id) => {
                IndexedPropertiesPanel::show_field_values(ui, def, id, tr)
            }
        };
        if let PropertyEdit::Renamed { from, to } = &edit {
            if let Some(entry) = self.flat.iter_mut().find(|e| e.field.name == *from) {
                entry.field.name = to.clone();
            }
            self.selection = IndexedSelection::Field(to.clone());
        }
        edit
    }

    pub fn apply_structure_to_def(&mut self, def: &mut IndexedDefinition) -> Result<(), String> {
        apply_flat(def, &self.flat)
    }

    pub fn add_field(&mut self, def: &mut IndexedDefinition) {
        let Some(root) = def.fields.first_mut() else {
            return;
        };
        let (next_off, n) = next_field_slot(root);
        let len = 8u32;
        let name = format!("FIELD-{n}");
        let entry = FlatEntry {
            depth: 1,
            field: IndexedField {
                level: 5,
                name: name.clone(),
                pic: format!("X({len})"),
                usage: FieldUsage::Display,
                offset: Some(next_off),
                length: Some(len),
                comment: String::new(),
                grid_control: Some(ControlType::TextBox),
                occurs: None,
                redefines: None,
                synchronized: false,
                children: Vec::new(),
            },
        };
        self.flat.push(entry);
        let _ = self.apply_structure_to_def(def);
        self.selection = IndexedSelection::Field(name);
    }

    pub fn remove_selected_field(&mut self, def: &mut IndexedDefinition) -> bool {
        let IndexedSelection::Field(id) = self.selection.clone() else {
            return false;
        };
        let before = self.flat.len();
        let Some(idx) = self.flat.iter().position(|e| e.field.name == id) else {
            return false;
        };
        remove_subtree(&mut self.flat, idx);
        if self.flat.len() == before {
            return false;
        }
        self.selection = IndexedSelection::File;
        self.apply_structure_to_def(def).is_ok()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StructureAction {
    None,
    StructureChanged,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RawDialogResult {
    None,
    Applied,
    Cancelled,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum IndentCommand {
    None,
    Indent,
    Outdent,
}

fn remove_subtree(flat: &mut Vec<FlatEntry>, idx: usize) {
    let depth = flat[idx].depth;
    let end = flat[idx + 1..]
        .iter()
        .position(|e| e.depth <= depth)
        .map(|p| idx + 1 + p)
        .unwrap_or(flat.len());
    flat.drain(idx..end);
}

fn read_indent_keys(ui: &egui::Ui) -> IndentCommand {
    let mut cmd = IndentCommand::None;
    ui.input(|i| {
        if i.key_pressed(egui::Key::Tab) && !i.modifiers.shift {
            cmd = IndentCommand::Indent;
        } else if i.key_pressed(egui::Key::Tab) && i.modifiers.shift {
            cmd = IndentCommand::Outdent;
        } else if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) {
            cmd = IndentCommand::Indent;
        } else if i.key_pressed(egui::Key::Minus) {
            cmd = IndentCommand::Outdent;
        }
    });
    cmd
}

fn next_field_slot(root: &IndexedField) -> (u32, u32) {
    let mut max_end = 0u32;
    let mut count = 0u32;
    for leaf in root.all_leaves() {
        count += 1;
        if let (Some(o), Some(l)) = (leaf.offset, leaf.length) {
            max_end = max_end.max(o + l);
        }
    }
    (max_end, count + 1)
}