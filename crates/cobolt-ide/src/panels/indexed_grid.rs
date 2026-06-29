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
    pub writes_blocked: bool,
    pub edit_cells: Vec<EditCell>,
    pub new_row_mode: bool,
}

impl IndexedGridPanel {
    pub fn new() -> Self {
        Self {
            session: None,
            error: None,
            selected_row: None,
            page_start: 0,
            writes_blocked: false,
            edit_cells: Vec::new(),
            new_row_mode: false,
        }
    }

    pub fn open(&mut self, def: &IndexedDefinition, data_path: &Path, writes_blocked: bool) {
        self.writes_blocked = writes_blocked;
        match GridSession::open(def, data_path) {
            Ok(s) => {
                self.session = Some(s);
                self.error = None;
            }
            Err(e) => {
                self.session = None;
                self.error = Some(e);
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
        if self.writes_blocked {
            ui.colored_label(ui.visuals().warn_fg_color, tr.grid_writes_blocked);
        }

        ui.horizontal(|ui| {
            if ui.button(tr.grid_btn_refresh).clicked() {
                action = GridAction::Refresh;
            }
            if !self.writes_blocked {
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

        egui::ScrollArea::both().show(ui, |ui| {
            egui::Grid::new("idx_grid").striped(true).show(ui, |ui| {
                for field in &leaves {
                    ui.strong(&field.name);
                }
                ui.end_row();
                for (abs, row) in &page_rows {
                    let selected = self.selected_row == Some(*abs);
                    for field in &leaves {
                        let text = format_cell(field, row, tr);
                        if ui.selectable_label(selected, text).clicked() {
                            self.selected_row = Some(*abs);
                            self.new_row_mode = false;
                            self.fill_edit_cells(row, &leaves);
                        }
                    }
                    ui.end_row();
                }
            });
        });

        if !self.writes_blocked && (self.new_row_mode || self.selected_row.is_some()) {
            ui.separator();
            ui.heading(if self.new_row_mode {
                tr.grid_new_row
            } else {
                tr.grid_edit_row
            });
            let record_len = def.record_length() as usize;
            if self.edit_cells.len() != leaves.len() {
                self.edit_cells = leaves
                    .iter()
                    .map(|f| EditCell::Text(String::new()))
                    .collect();
            }
            for (i, field) in leaves.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(&field.name);
                    show_edit_control(ui, field, &mut self.edit_cells[i], tr);
                });
            }
            if ui.button(tr.grid_btn_apply).clicked() {
                let field_refs: Vec<&IndexedField> = leaves.iter().copied().collect();
                match build_record(&self.edit_cells, &field_refs, record_len, tr) {
                    Ok(rec) => {
                        if self.new_row_mode {
                            if let Some(s) = &mut self.session {
                                match s.write_row(&rec) {
                                    Ok(()) => status = Some(tr.grid_status_write_ok.into()),
                                    Err(e) => status = Some(e),
                                }
                            }
                            self.new_row_mode = false;
                        } else if let Some(idx) = self.selected_row {
                            if let Some(s) = &mut self.session {
                                match s.select_row(idx).and_then(|_| s.rewrite_row(&rec)) {
                                    Ok(()) => status = Some(tr.grid_status_write_ok.into()),
                                    Err(e) => status = Some(e),
                                }
                            }
                        }
                    }
                    Err(e) => status = Some(e),
                }
            }
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
