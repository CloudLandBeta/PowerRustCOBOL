// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Forms list panel — shows all `.cfrm` files in the project directory.
//!
//! Displayed in the left sidebar of the form designer.  Double-clicking a form
//! opens it in a new designer tab.  Currently open forms are highlighted.

use std::path::{Path, PathBuf};

use egui::{Color32, RichText, ScrollArea, Ui};
use crate::i18n::Tr;

// ── FormsListPanel ────────────────────────────────────────────────────────────

pub struct FormsListPanel {
    /// Project root to scan.
    root:         Option<PathBuf>,
    /// Cached list of discovered .cfrm paths.
    found:        Vec<PathBuf>,
    /// Selected entry (for single-click highlight).
    selected:     Option<PathBuf>,
    /// True when we need to re-scan (root changed or Refresh pressed).
    needs_rescan: bool,
}

impl FormsListPanel {
    pub fn new() -> Self {
        Self {
            root:         None,
            found:        Vec::new(),
            selected:     None,
            needs_rescan: false,
        }
    }

    /// Notify the panel that the project root has changed.
    pub fn set_root(&mut self, root: &Path) {
        if self.root.as_deref() != Some(root) {
            self.root         = Some(root.to_owned());
            self.needs_rescan = true;
        }
    }

    /// Force a rescan even if the root hasn't changed (e.g. after a Save).
    pub fn refresh(&mut self) {
        self.needs_rescan = true;
    }

    /// Clear the project root (no project open).
    pub fn clear_root(&mut self) {
        self.root         = None;
        self.found        = Vec::new();
        self.needs_rescan = false;
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    /// Draw the forms list.
    ///
    /// * `open_paths` — paths of forms currently open in designer tabs.
    ///
    /// Returns the path that was double-clicked (to open), if any.
    pub fn show(&mut self, ui: &mut Ui, open_paths: &[&Path], tr: &Tr) -> Option<PathBuf> {
        if self.needs_rescan {
            self.rescan();
        }

        let mut to_open: Option<PathBuf> = None;

        ui.horizontal(|ui| {
            ui.strong(tr.forms_list_title);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("⟳").on_hover_text("Refresh form list").clicked() {
                    self.needs_rescan = true;
                }
            });
        });
        ui.separator();

        if self.found.is_empty() {
            if self.root.is_none() {
                ui.label(
                    RichText::new(tr.forms_no_project)
                        .color(Color32::GRAY)
                        .small(),
                );
            } else {
                ui.label(
                    RichText::new(tr.forms_no_cfrm)
                        .color(Color32::GRAY)
                        .small(),
                );
            }
            return None;
        }

        ScrollArea::vertical()
            .id_salt("forms_list_scroll")
            .max_height(200.0)
            .show(ui, |ui| {
                for path in &self.found {
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("?");

                    let is_open    = open_paths.iter().any(|p| *p == path.as_path());
                    let is_selected = self.selected.as_deref() == Some(path.as_path());

                    // Icon: open tab marker vs plain form icon
                    let icon = if is_open { "🖊" } else { "🗔" };

                    let label = RichText::new(format!("{icon} {stem}"))
                        .color(if is_open {
                            Color32::from_rgb(100, 200, 100)
                        } else {
                            crate::theme::active().text_bright
                        });

                    let resp = ui.selectable_label(is_selected, label);

                    if resp.clicked() {
                        self.selected = Some(path.clone());
                    }

                    if resp.double_clicked() {
                        to_open = Some(path.clone());
                    }

                    // Tooltip: full path
                    resp.on_hover_text(path.display().to_string());
                }
            });

        to_open
    }

    // ── Directory scan ────────────────────────────────────────────────────────

    fn rescan(&mut self) {
        self.found        = Vec::new();
        self.needs_rescan = false;

        let Some(ref root) = self.root else { return };

        scan_dir(root, &mut self.found);
        self.found.sort();
    }
}

/// Recursively collect .cfrm files under `dir`.
fn scan_dir(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            // Recurse but skip hidden dirs
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.starts_with('.') {
                scan_dir(&path, out);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("cfrm") {
            out.push(path);
        }
    }
}
