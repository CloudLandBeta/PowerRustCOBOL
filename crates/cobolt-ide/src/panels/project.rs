// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Project explorer panel.
//!
//! Two display modes:
//!
//! **Project mode** (when a `CoboltProject` is loaded):
//!   Shows three collapsible sections — Sources, Forms, Assets — each with a
//!   `[+]` button to add files and a right-click context menu to remove them.
//!
//! **Tree mode** (no project loaded):
//!   Shows the raw directory tree for the current root, just like before.
//!
//! The panel returns a `Vec<ProjectPanelEvent>` every frame; the caller
//! processes those events against the application state.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use egui::{Color32, Context, RichText, ScrollArea, SidePanel, Ui};

use crate::project_model::{CoboltProject, FileKind};
use crate::i18n::Tr;

// ── Events ────────────────────────────────────────────────────────────────────

/// Actions emitted by the project panel for `CoboltApp` to handle.
#[derive(Clone)]
pub enum ProjectPanelEvent {
    /// User double-clicked a file — open it in the editor / designer.
    Open(PathBuf),
    /// User clicked `[+]` on a section — show a file-picker for this kind.
    Add(FileKind),
    /// User chose "Remove from project" — contains the relative path string.
    Remove(String),
}

// ── ProjectPanel ──────────────────────────────────────────────────────────────

pub struct ProjectPanel {
    /// Root directory of the open project / directory (if any).
    pub root: Option<PathBuf>,
    /// Expanded directories (tree mode only).
    expanded: HashSet<PathBuf>,
}

impl Default for ProjectPanel {
    fn default() -> Self {
        Self { root: None, expanded: HashSet::new() }
    }
}

impl ProjectPanel {
    pub fn new() -> Self { Self::default() }

    /// Set the root directory shown in tree mode.
    pub fn set_root(&mut self, root: impl Into<PathBuf>) {
        self.root = Some(root.into());
        self.expanded.clear();
    }

    /// Render the project panel and return all events that occurred this frame.
    ///
    /// * `project` — `Some(&project)` to render in project mode, `None` for
    ///   the raw file-tree fallback.
    pub fn show(
        &mut self,
        ctx:     &Context,
        project: Option<&CoboltProject>,
        tr:      &Tr,
    ) -> Vec<ProjectPanelEvent> {
        let mut events = Vec::new();

        SidePanel::left("project_panel")
            .resizable(true)
            .default_width(200.0)
            .min_width(120.0)
            .show(ctx, |ui| {
                match project {
                    Some(proj) => self.show_project_mode(ui, proj, &mut events, tr),
                    None       => self.show_tree_mode(ui, &mut events, tr),
                }
            });

        events
    }

    // ── Project mode ──────────────────────────────────────────────────────────

    fn show_project_mode(
        &mut self,
        ui:     &mut Ui,
        proj:   &CoboltProject,
        events: &mut Vec<ProjectPanelEvent>,
        tr:     &Tr,
    ) {
        // Header
        ui.horizontal(|ui| {
            ui.heading(tr.panel_project);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("v{}", proj.project.version))
                        .color(Color32::GRAY)
                        .small(),
                );
            });
        });
        ui.label(RichText::new(&proj.project.name).strong());

        // Main file pill
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new("main:").color(Color32::GRAY).small());
            ui.label(RichText::new(&proj.project.main).small().monospace());
        });
        ui.separator();

        ScrollArea::vertical()
            .id_salt("project_panel_scroll")
            .show(ui, |ui| {
                show_section(
                    ui, tr.panel_sources, FileKind::Source, "📄",
                    &proj.files.sources, &self.root, events,
                );
                show_section(
                    ui, tr.panel_forms, FileKind::Form, "🗔",
                    &proj.files.forms, &self.root, events,
                );
                show_section(
                    ui, tr.panel_assets, FileKind::Asset, "📦",
                    &proj.files.assets, &self.root, events,
                );
            });
    }

    // ── Tree mode ─────────────────────────────────────────────────────────────

    fn show_tree_mode(&mut self, ui: &mut Ui, events: &mut Vec<ProjectPanelEvent>, tr: &Tr) {
        ui.heading(tr.panel_project);
        ui.separator();

        ScrollArea::vertical()
            .id_salt("project_tree_scroll")
            .show(ui, |ui| {
                match self.root.clone() {
                    Some(root) => {
                        if let Some(path) = self.show_dir(ui, &root, 0) {
                            events.push(ProjectPanelEvent::Open(path));
                        }
                    }
                    None => {
                        ui.label(
                            RichText::new(tr.no_project_open)
                                .color(Color32::GRAY),
                        );
                    }
                }
            });
    }

    fn show_dir(&mut self, ui: &mut Ui, dir: &Path, depth: usize) -> Option<PathBuf> {
        let mut opened: Option<PathBuf> = None;

        let entries = match std::fs::read_dir(dir) {
            Ok(e)  => e,
            Err(_) => return None,
        };

        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        paths.sort();

        for path in &paths {
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?");

            if name.starts_with('.') { continue; }

            let indent = depth as f32 * 14.0;

            if path.is_dir() {
                let expanded = self.expanded.contains(path);
                let label = if expanded {
                    format!("▾ 📁 {name}")
                } else {
                    format!("▸ 📁 {name}")
                };
                ui.horizontal(|ui| {
                    ui.add_space(indent);
                    if ui.selectable_label(false, label).clicked() {
                        if expanded {
                            self.expanded.remove(path);
                        } else {
                            self.expanded.insert(path.clone());
                        }
                    }
                });
                if expanded {
                    if let Some(p) = self.show_dir(ui, path, depth + 1) {
                        opened = Some(p);
                    }
                }
            } else {
                let ext = path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                if !matches!(ext.to_ascii_lowercase().as_str(),
                    "cbl" | "cob" | "cpy" | "cfrm" | "toml" | "txt") {
                    continue;
                }
                let icon = match ext.to_ascii_lowercase().as_str() {
                    "cbl" | "cob" => "📄",
                    "cpy"         => "📋",
                    "cfrm"        => "🗔",
                    _             => "📃",
                };
                ui.horizontal(|ui| {
                    ui.add_space(indent + 14.0);
                    if ui.selectable_label(false, format!("{icon} {name}"))
                        .double_clicked()
                    {
                        opened = Some(path.clone());
                    }
                });
            }
        }

        opened
    }
}

// ── Section renderer ──────────────────────────────────────────────────────────

/// Draw one file-group section (Sources / Forms / Assets).
///
/// Free function to avoid borrow-checker conflicts with `&mut self`.
fn show_section(
    ui:     &mut Ui,
    title:  &str,
    kind:   FileKind,
    icon:   &str,
    files:  &[String],
    root:   &Option<PathBuf>,
    events: &mut Vec<ProjectPanelEvent>,
) {
    // Section header row with [+] button on the right
    ui.horizontal(|ui| {
        ui.strong(title);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("+")
                .on_hover_text(format!("Add file to {title}"))
                .clicked()
            {
                events.push(ProjectPanelEvent::Add(kind));
            }
        });
    });

    if files.is_empty() {
        ui.label(RichText::new("  (none)").color(Color32::GRAY).small());
    } else {
        for rel in files {
            let name = Path::new(rel)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(rel.as_str());

            let resp = ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.selectable_label(false, format!("{icon} {name}"))
                    .on_hover_text(rel.as_str())
            }).inner;

            if resp.double_clicked() {
                if let Some(dir) = root {
                    events.push(ProjectPanelEvent::Open(dir.join(rel)));
                }
            }

            resp.context_menu(|ui| {
                if ui.button("Remove from project").clicked() {
                    events.push(ProjectPanelEvent::Remove(rel.clone()));
                    ui.close_menu();
                }
            });
        }
    }

    ui.add_space(4.0);
}
