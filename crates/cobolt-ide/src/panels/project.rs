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

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use egui::{Color32, Context, RichText, ScrollArea, SidePanel, Ui};

use cobolt_forms::model::Form;

use crate::project_model::{CoboltProject, Category, ElementStatus, FileKind};
use crate::i18n::Tr;
use crate::panels::toolbox;

/// Blue for read-only RAD-generated COBOL in the tree (matches the editor).
const GENERATED_BLUE: Color32 = Color32::from_rgb(96, 160, 240);
/// Icon size in the tree — 80 % larger than the default body text (~12 px).
const ICON_SIZE: f32 = 21.6;

// ── Events ────────────────────────────────────────────────────────────────────

/// Actions emitted by the project panel for `CoboltApp` to handle.
#[derive(Clone)]
pub enum ProjectPanelEvent {
    /// Open a code/doc/asset file in the Main Pane editor (single click).
    Open(PathBuf),
    /// Open a form in the RAD designer (double-click a form node).
    OpenDesigner(PathBuf),
    /// Show a form's properties in the Main Pane (click a form node).
    InspectForm(PathBuf),
    /// Show a control's properties in the Main Pane (click a control in a form).
    InspectControl { form: PathBuf, ctrl_id: String },
    /// User clicked `[+]` on a category — show a file-picker for this kind.
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
    /// mtime-keyed cache of loaded forms (for the controls sub-tree).
    forms: HashMap<PathBuf, (SystemTime, Form)>,
    /// Per-element "semaphore" status, keyed by relative path.
    status: HashMap<String, ElementStatus>,
}

impl Default for ProjectPanel {
    fn default() -> Self {
        Self {
            root: None,
            expanded: HashSet::new(),
            forms: HashMap::new(),
            status: HashMap::new(),
        }
    }
}

impl ProjectPanel {
    pub fn new() -> Self { Self::default() }

    /// Set the root directory shown in tree mode.
    pub fn set_root(&mut self, root: impl Into<PathBuf>) {
        self.root = Some(root.into());
        self.expanded.clear();
    }

    /// Drop the cached copy of a form so the controls sub-tree reloads it (after
    /// an inline-inspector edit / designer save).
    pub fn refresh_form(&mut self, path: &Path) {
        self.forms.remove(path);
    }

    /// Set the semaphore status for a tracked element (relative path).
    pub fn set_status(&mut self, rel: &str, s: ElementStatus) {
        self.status.insert(rel.replace('\\', "/"), s);
    }

    /// The status for `rel` — defaults to `Changed` (yellow / not yet tested).
    fn status_for(&self, rel: &str) -> ElementStatus {
        self.status.get(&rel.replace('\\', "/")).copied().unwrap_or_default()
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
        ScrollArea::vertical()
            .id_salt("project_panel_scroll")
            .show(ui, |ui| {
                // L1 — the project itself is the root node; categories live under it.
                let root_id = ui.make_persistent_id("project_root");
                egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), root_id, true)
                    .show_header(ui, |ui| {
                        ui.label(RichText::new("📁").size(ICON_SIZE));
                        ui.label(RichText::new(&proj.project.name).strong());
                        ui.label(RichText::new(format!("v{}", proj.project.version))
                            .color(Color32::GRAY).small());
                    })
                    .body(|ui| {
                        // L2 — the five fixed, IDE-owned categories.
                        for cat in Category::TOP {
                            self.show_category(ui, cat, proj, events, tr);
                        }
                    });
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

/// A small "semaphore" dot to the left of an element's icon.
fn status_dot(ui: &mut Ui, status: ElementStatus) {
    let (r, g, b) = status.rgb();
    ui.label(RichText::new("●").color(Color32::from_rgb(r, g, b)).size(12.0))
        .on_hover_text(status.tooltip());
}

// ── Category tree node (L2) ─────────────────────────────────────────────────────

impl ProjectPanel {
    /// mtime-cached load of a form for the controls sub-tree (returns a clone).
    fn form_for(&mut self, abs: &Path) -> Option<Form> {
        let mtime = std::fs::metadata(abs).and_then(|m| m.modified()).ok()?;
        if let Some((t, f)) = self.forms.get(abs) {
            if *t == mtime {
                return Some(f.clone());
            }
        }
        let form = cobolt_forms::load_form(abs).ok()?;
        self.forms.insert(abs.to_path_buf(), (mtime, form.clone()));
        Some(form)
    }

    /// Draw one fixed category node (L2) and its items (L3).
    fn show_category(
        &mut self,
        ui:     &mut Ui,
        cat:    Category,
        proj:   &CoboltProject,
        events: &mut Vec<ProjectPanelEvent>,
        tr:     &Tr,
    ) {
        let (label, kind): (&str, Option<FileKind>) = match cat {
            Category::Forms         => (tr.panel_forms, Some(FileKind::Form)),
            Category::CommonCode    => (tr.cat_common_code, Some(FileKind::Source)),
            Category::Generated     => (tr.cat_generated_code, None),
            Category::Assets        => (tr.panel_assets, Some(FileKind::Asset)),
            Category::Documentation => (tr.cat_documentation, Some(FileKind::Documentation)),
        };
        let is_generated = cat == Category::Generated;
        let is_forms = cat == Category::Forms;
        let root = self.root.clone();

        let id = ui.make_persistent_id(("project_cat", label));
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
            .show_header(ui, |ui| {
                ui.label(RichText::new(cat.icon()).size(ICON_SIZE));
                ui.label(RichText::new(label).strong());
                // Generated Code is IDE-owned (forms populate it) — no [+].
                if let Some(kind) = kind {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("➕")
                            .on_hover_text(format!("{}: {label}", tr.tree_add_hover))
                            .clicked()
                        {
                            events.push(ProjectPanelEvent::Add(kind));
                        }
                    });
                }
            })
            .body(|ui| {
                let files: Vec<String> = proj.files_in(cat).to_vec();
                if files.is_empty() {
                    let hint = if is_generated { tr.tree_generated_empty } else { tr.tree_empty };
                    ui.label(RichText::new(format!("  {hint}")).color(Color32::GRAY).small());
                    return;
                }
                for rel in &files {
                    let st = self.status_for(rel);
                    if is_forms {
                        self.show_form_item(ui, rel, &root, events, tr);
                    } else if is_generated {
                        file_row(ui, rel, "🔒", Some(GENERATED_BLUE), false, st, &root, events);
                    } else {
                        let icon = kind.map(|k| k.icon()).unwrap_or("📄");
                        file_row(ui, rel, icon, None, true, st, &root, events);
                    }
                }
            });
        ui.add_space(2.0);
    }

    /// A form item (L3) that expands to its controls grouped by toolbox category.
    fn show_form_item(
        &mut self,
        ui:     &mut Ui,
        rel:    &str,
        root:   &Option<PathBuf>,
        events: &mut Vec<ProjectPanelEvent>,
        tr:     &Tr,
    ) {
        let _ = tr;
        let name = Path::new(rel).file_name().and_then(|n| n.to_str()).unwrap_or(rel);
        let abs = root.as_ref().map(|d| d.join(rel));
        let form = abs.as_ref().and_then(|p| self.form_for(p));
        let form_status = self.status_for(rel);

        let id = ui.make_persistent_id(("form_item", rel));
        // L3 form node is open by default (collapse only kicks in below it).
        let (_toggle, header_inner, _body) =
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
            .show_header(ui, |ui| {
                ui.add_space(8.0);
                status_dot(ui, form_status);
                ui.label(RichText::new(FileKind::Form.icon()).size(ICON_SIZE));
                ui.selectable_label(false, RichText::new(name)).on_hover_text(rel)
            })
            .body(|ui| {
                let Some(form) = &form else {
                    ui.label(RichText::new("  (could not read form)").color(Color32::GRAY).small());
                    return;
                };
                let Some(form_path) = &abs else { return; };
                // Group controls by toolbox category, Non-Visual first (L4, collapsed).
                for &cat_key in toolbox::TREE_CATEGORY_ORDER {
                    let in_cat: Vec<&cobolt_forms::model::Control> = form
                        .controls
                        .iter()
                        .filter(|c| toolbox::category_of(c.control_type.clone()) == cat_key)
                        .collect();
                    if in_cat.is_empty() {
                        continue;
                    }
                    let gid = ui.make_persistent_id(("form_grp", rel, cat_key));
                    // L4 — collapsed by default (everything below level 3 collapses).
                    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), gid, false)
                        .show_header(ui, |ui| {
                            ui.add_space(16.0);
                            ui.label(RichText::new(format!("{} ({})",
                                toolbox::category_display(cat_key), in_cat.len()))
                                .color(Color32::from_gray(170)));
                        })
                        .body(|ui| {
                            for c in &in_cat {
                                let crow = ui.horizontal(|ui| {
                                    ui.add_space(26.0);
                                    status_dot(ui, form_status); // control inherits its form's status
                                    ui.selectable_label(false, format!("• {}", c.id))
                                        .on_hover_text(format!("{:?}", c.control_type))
                                }).inner;
                                if crow.clicked() {
                                    events.push(ProjectPanelEvent::InspectControl {
                                        form: form_path.clone(),
                                        ctrl_id: c.id.clone(),
                                    });
                                }
                            }
                        });
                }
            });
        // Single click → inspect form properties; double click → open the designer.
        let resp = header_inner.inner;
        if let Some(p) = &abs {
            if resp.double_clicked() {
                events.push(ProjectPanelEvent::OpenDesigner(p.clone()));
            } else if resp.clicked() {
                events.push(ProjectPanelEvent::InspectForm(p.clone()));
            }
        }
        ui.add_space(1.0);
    }
}

/// One file row (L3) inside a non-form category. Single click opens it in the
/// Main Pane; `color` tints the label; `removable` adds a remove context menu.
#[allow(clippy::too_many_arguments)]
fn file_row(
    ui:        &mut Ui,
    rel:       &str,
    icon:      &str,
    color:     Option<Color32>,
    removable: bool,
    status:    ElementStatus,
    root:      &Option<PathBuf>,
    events:    &mut Vec<ProjectPanelEvent>,
) {
    let name = Path::new(rel).file_name().and_then(|n| n.to_str()).unwrap_or(rel);
    let mut text = RichText::new(name);
    if let Some(c) = color {
        text = text.color(c);
    }
    let resp = ui.horizontal(|ui| {
        ui.add_space(8.0);
        status_dot(ui, status);
        ui.label(RichText::new(icon).size(ICON_SIZE));
        ui.selectable_label(false, text).on_hover_text(rel)
    }).inner;

    // Single click opens the file in the Main Pane.
    if resp.clicked() {
        if let Some(dir) = root {
            events.push(ProjectPanelEvent::Open(dir.join(rel)));
        }
    }
    if removable {
        resp.context_menu(|ui| {
            if ui.button("Remove from project").clicked() {
                events.push(ProjectPanelEvent::Remove(rel.to_string()));
                ui.close_menu();
            }
        });
    }
}
