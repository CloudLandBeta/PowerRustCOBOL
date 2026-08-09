// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Indexed File Grid Browser viewport (R16–R21, AC7/AC8).

use std::path::Path;

use cobolt_indexed::{IndexedDefinition, IndexedField};
use cobolt_runtime::indexed_ide::GridSession;

use super::indexed_field_control::{build_record, format_cell, show_edit_control, EditCell};
use crate::i18n::Tr;

const PAGE_SIZE: usize = 200;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GridAction {
    None,
    Commit,
    Rollback,
    AddRow,
    DeleteRow,
    Refresh,
}

pub struct IndexedGridPanel {
    pub session: Option<GridSession>,
    pub error: Option<String>,
    pub selected_row: Option<usize>,
    pub page_start: usize,
    pub drift_mismatch: Option<String>,
    pub edit_cells: Vec<EditCell>,
    pub new_row_mode: bool,
}

impl IndexedGridPanel {
    /// Show an error, and record it in the IDE console (operator, 2026-08-09).
    /// This one clears on the next successful operation, so without the console
    /// entry there would be no trace of it at all.
    pub(crate) fn set_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        crate::error_log::record(&message);
        self.error = Some(message);
    }

    pub fn new() -> Self {
        Self {
            session: None,
            error: None,
            selected_row: None,
            page_start: 0,
            drift_mismatch: None,
            edit_cells: Vec::new(),
            new_row_mode: false,
        }
    }

    pub fn open(
        &mut self,
        def: &IndexedDefinition,
        data_path: &Path,
        drift_mismatch: Option<String>,
    ) {
        self.drift_mismatch = drift_mismatch;
        match GridSession::open(def, data_path) {
            Ok(s) => {
                self.session = Some(s);
                self.error = None;
            }
            Err(e) => {
                self.session = None;
                self.set_error(e);
            }
        }
        self.selected_row = None;
        self.page_start = 0;
        self.edit_cells.clear();
        self.new_row_mode = false;
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        def: &IndexedDefinition,
        tr: &Tr,
    ) -> (GridAction, Option<String>) {
        let mut action = GridAction::None;
        let mut status: Option<String> = None;

        if let Some(err) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, err);
        }
        if let Some(detail) = &self.drift_mismatch {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!("{} ({})", tr.grid_writes_blocked, detail),
            );
        }

        ui.horizontal(|ui| {
            if ui.button(tr.grid_btn_refresh).clicked() {
                action = GridAction::Refresh;
            }
            if self.drift_mismatch.is_none() {
                if ui.button(tr.grid_btn_add).clicked() {
                    action = GridAction::AddRow;
                }
                if ui.button(tr.grid_btn_delete).clicked() {
                    action = GridAction::DeleteRow;
                }
                if ui.button(tr.grid_btn_commit).clicked() {
                    action = GridAction::Commit;
                }
                if ui.button(tr.grid_btn_rollback).clicked() {
                    action = GridAction::Rollback;
                }
            }
        });
        ui.separator();

        let leaves: Vec<&IndexedField> = def
            .fields
            .first()
            .map(|r| r.all_leaves())
            .unwrap_or_default();

        let page_rows: Vec<(usize, Vec<u8>)> = if let Some(session) = &self.session {
            let rows = session.rows();
            let total = rows.len();
            if self.page_start >= total.saturating_sub(1) && self.page_start > 0 {
                self.page_start = total.saturating_sub(PAGE_SIZE);
            }
            let page_end = (self.page_start + PAGE_SIZE).min(total);
            ui.horizontal(|ui| {
                if ui.button(tr.grid_page_prev).clicked() && self.page_start >= PAGE_SIZE {
                    self.page_start -= PAGE_SIZE;
                }
                ui.label(format!(
                    "{} {}–{} / {}",
                    tr.grid_rows_label,
                    self.page_start + 1,
                    page_end,
                    total
                ));
                if ui.button(tr.grid_page_next).clicked() && page_end < total {
                    self.page_start += PAGE_SIZE;
                }
            });
            rows[self.page_start..page_end]
                .iter()
                .enumerate()
                .map(|(ri, r)| (self.page_start + ri, r.clone()))
                .collect()
        } else {
            return (action, status);
        };

        let has_edit =
            self.drift_mismatch.is_none() && (self.new_row_mode || self.selected_row.is_some());

        // Existing rows. While the edit form is open the grid is capped to a short
        // strip so the form can fill the rest of the window, and its column-name
        // header row is hidden — the form below already labels every field, so that
        // header is redundant clutter during entry. (When just browsing, the grid
        // fills the window and keeps its header for column identification.)
        let grid_max = if has_edit {
            160.0
        } else {
            ui.available_height()
        };
        egui::ScrollArea::both()
            .id_salt("grid_scroll")
            .max_height(grid_max)
            .show(ui, |ui| {
                egui::Grid::new("idx_grid").striped(true).show(ui, |ui| {
                    if !has_edit {
                        for field in &leaves {
                            ui.strong(&field.name);
                        }
                        ui.end_row();
                    }
                    for (abs, row) in &page_rows {
                        let selected = self.selected_row == Some(*abs);
                        for field in &leaves {
                            let text = format_cell(field, row, tr);
                            if ui.selectable_label(selected, text).clicked() {
                                self.selected_row = Some(*abs);
                                self.new_row_mode = false;
                                self.fill_edit_cells(row, &leaves);
                                self.error = None;
                            }
                        }
                        ui.end_row();
                    }
                });
            });

        // The dynamic edit form ("New row" / "Edit row") fills the whole area beneath
        // the toolbar/grid: the field list expands to take the remaining height while
        // the Apply button stays pinned at the bottom. Sizing the list from the (now
        // fixed) available height is a plain fill, not a resizable panel, so there is
        // no self-inflation feedback (see the egui resize guardrail).
        if has_edit {
            let record_len = def.record_length() as usize;
            if self.edit_cells.len() != leaves.len() {
                self.edit_cells = leaves
                    .iter()
                    .map(|_| EditCell::Text(String::new()))
                    .collect();
            }
            ui.separator();
            ui.push_id("edit_form_container", |ui| {
                ui.heading(if self.new_row_mode {
                    tr.grid_new_row
                } else {
                    tr.grid_edit_row
                });
                // Reserve room for the Apply row, then let the field list fill the
                // rest of the height so the form occupies the full available space.
                let apply_reserve = 46.0;
                let list_h = (ui.available_height() - apply_reserve).max(80.0);
                egui::ScrollArea::vertical()
                    .id_salt("edit_scroll")
                    .max_height(list_h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (i, field) in leaves.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.add_sized([160.0, 20.0], egui::Label::new(&field.name));
                                show_edit_control(ui, field, &mut self.edit_cells[i], tr);
                            });
                        }
                    });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(10.0); // left padding
                    if ui.button(tr.grid_btn_apply).clicked() {
                        let field_refs: Vec<&IndexedField> = leaves.iter().copied().collect();
                        match build_record(&self.edit_cells, &field_refs, record_len, tr) {
                            Ok(rec) => {
                                if self.new_row_mode {
                                    if let Some(s) = &mut self.session {
                                        match s.write_row(&rec) {
                                            Ok(()) => {
                                                status = Some(tr.grid_status_write_ok.into());
                                                self.error = None;
                                                self.new_row_mode = false;
                                            }
                                            Err(e) => {
                                                status = Some(e.clone());
                                                self.set_error(e);
                                            }
                                        }
                                    }
                                } else if let Some(idx) = self.selected_row {
                                    if let Some(s) = &mut self.session {
                                        match s.select_row(idx).and_then(|_| s.rewrite_row(&rec)) {
                                            Ok(()) => {
                                                status = Some(tr.grid_status_write_ok.into());
                                                self.error = None;
                                            }
                                            Err(e) => {
                                                status = Some(e.clone());
                                                self.set_error(e);
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                status = Some(e.clone());
                                self.set_error(e);
                            }
                        }
                    }
                });
            });
        }

        (action, status)
    }

    fn fill_edit_cells(&mut self, row: &[u8], fields: &[&IndexedField]) {
        self.edit_cells = fields
            .iter()
            .map(|f| EditCell::from_field(f, row))
            .collect();
    }

    pub fn apply_action(&mut self, action: GridAction, tr: &Tr) -> Option<String> {
        let Some(session) = &mut self.session else {
            return Some(self.error.clone().unwrap_or_default());
        };
        match action {
            GridAction::Commit => {
                session.commit();
                Some(tr.grid_status_commit.into())
            }
            GridAction::Rollback => {
                session.rollback();
                self.selected_row = None;
                Some(tr.grid_status_rollback.into())
            }
            GridAction::AddRow => {
                self.new_row_mode = true;
                self.selected_row = None;
                self.edit_cells.clear();
                None
            }
            GridAction::DeleteRow => {
                let idx = self.selected_row?;
                match session
                    .select_row(idx)
                    .and_then(|_| session.delete_current())
                {
                    Ok(()) => {
                        self.selected_row = None;
                        Some(tr.grid_status_delete_ok.into())
                    }
                    Err(e) => Some(e),
                }
            }
            GridAction::Refresh => {
                session.rollback();
                self.selected_row = None;
                None
            }
            GridAction::None => None,
        }
    }
}
