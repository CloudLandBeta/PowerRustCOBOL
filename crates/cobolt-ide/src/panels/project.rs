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
    /// Open a widget event's generated COBOL paragraph (click an Events entry).
    OpenEventCode { form: PathBuf, paragraph: String },
    /// Internal: a tree element was selected (consumed by the panel, not the app).
    Select(String),
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
    /// The currently selected tree element (a unique key — see `sel_*` helpers).
    selected: Option<String>,
}

impl Default for ProjectPanel {
    fn default() -> Self {
        Self {
            root: None,
            expanded: HashSet::new(),
            forms: HashMap::new(),
            status: HashMap::new(),
            selected: None,
        }
    }
}

/// Selection keys (unique per tree element).
fn sel_file(rel: &str) -> String { format!("file:{rel}") }
fn sel_ctrl(rel: &str, id: &str) -> String { format!("ctrl:{rel}#{id}") }
fn sel_event(rel: &str, id: &str, ev: &str) -> String { format!("event:{rel}#{id}@{ev}") }

/// A selectable tree row that fills the remaining width, so the selection
/// highlight reads as a full-width rounded **pill** (matching the reference
/// design) rather than hugging the text.
fn full_width_select(
    ui: &mut Ui,
    selected: bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    ui.add_sized([ui.available_width(), h], egui::SelectableLabel::new(selected, text))
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

    /// The relative path of the currently selected *file* element, if any
    /// (used by the toolbar to gate Debug on a Generated Code selection).
    pub fn selected_file(&self) -> Option<&str> {
        self.selected.as_deref().and_then(|s| s.strip_prefix("file:"))
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

        let frame = crate::theme::glass_panel_frame(
            ctx.style().visuals.panel_fill, &crate::theme::active());
        SidePanel::left("project_panel")
            .resizable(true)
            .default_width(220.0)
            .min_width(140.0)
            .frame(frame)
            .show(ctx, |ui| {
                match project {
                    Some(proj) => self.show_project_mode(ui, proj, &mut events, tr),
                    None       => self.show_tree_mode(ui, &mut events, tr),
                }
            });

        // Consume Select events internally (update the highlighted element).
        events.retain(|e| {
            if let ProjectPanelEvent::Select(key) = e {
                self.selected = Some(key.clone());
                false
            } else {
                true
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
        // Current selection (read-only snapshot for highlighting); clicks emit a
        // `Select` event that `show()` applies after rendering.
        let cur = self.selected.clone();
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
                            self.show_category(ui, cat, proj, &cur, events, tr);
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
        cur:    &Option<String>,
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
                        self.show_form_item(ui, rel, &root, cur, events, tr);
                    } else if is_generated {
                        file_row(ui, rel, "🔒", Some(crate::theme::active().ed_generated), false, st, cur, &root, events);
                    } else {
                        let icon = kind.map(|k| k.icon()).unwrap_or("📄");
                        file_row(ui, rel, icon, None, true, st, cur, &root, events);
                    }
                }
            });
        ui.add_space(2.0);
    }

    /// A form item (L3) that expands to its controls grouped by toolbox category;
    /// each control with handlers expands to an "Events" group.
    fn show_form_item(
        &mut self,
        ui:     &mut Ui,
        rel:    &str,
        root:   &Option<PathBuf>,
        cur:    &Option<String>,
        events: &mut Vec<ProjectPanelEvent>,
        tr:     &Tr,
    ) {
        let name = Path::new(rel).file_name().and_then(|n| n.to_str()).unwrap_or(rel);
        let abs = root.as_ref().map(|d| d.join(rel));
        let form = abs.as_ref().and_then(|p| self.form_for(p));
        let form_status = self.status_for(rel);
        let form_key = sel_file(rel);
        let form_selected = cur.as_deref() == Some(form_key.as_str());

        let id = ui.make_persistent_id(("form_item", rel));
        // L3 form node is open by default (collapse only kicks in below it).
        let (_toggle, header_inner, _body) =
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
            .show_header(ui, |ui| {
                ui.add_space(8.0);
                status_dot(ui, form_status);
                ui.label(RichText::new(FileKind::Form.icon()).size(ICON_SIZE));
                full_width_select(ui, form_selected, RichText::new(name)).on_hover_text(rel)
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
                                .color(Color32::from_gray(120)));
                        })
                        .body(|ui| {
                            for c in &in_cat {
                                control_node(ui, rel, form_path, c, form_status, cur, events, tr);
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
                events.push(ProjectPanelEvent::Select(form_key));
                events.push(ProjectPanelEvent::InspectForm(p.clone()));
            }
        }
        ui.add_space(1.0);
    }
}

/// One control (L5). A leaf row, unless it has event handlers — then it expands
/// to an "Events" group listing them (click → open the event's COBOL paragraph).
#[allow(clippy::too_many_arguments)]
fn control_node(
    ui:        &mut Ui,
    rel:       &str,
    form_path: &Path,
    c:         &cobolt_forms::model::Control,
    status:    ElementStatus,
    cur:       &Option<String>,
    events:    &mut Vec<ProjectPanelEvent>,
    tr:        &Tr,
) {
    let ckey = sel_ctrl(rel, &c.id);
    let csel = cur.as_deref() == Some(ckey.as_str());
    let hint = format!("{:?}", c.control_type);

    if c.events.is_empty() {
        let crow = ui.horizontal(|ui| {
            ui.add_space(26.0);
            status_dot(ui, status);
            full_width_select(ui, csel, format!("• {}", c.id)).on_hover_text(hint)
        }).inner;
        if crow.clicked() {
            events.push(ProjectPanelEvent::Select(ckey));
            events.push(ProjectPanelEvent::InspectControl {
                form: form_path.to_path_buf(),
                ctrl_id: c.id.clone(),
            });
        }
        return;
    }

    // Has events → an expandable control node (collapsed) with an Events group.
    let id = ui.make_persistent_id(("ctrl", rel, &c.id));
    let (_t, header_inner, _b) =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
        .show_header(ui, |ui| {
            ui.add_space(20.0);
            status_dot(ui, status);
            full_width_select(ui, csel, format!("• {}", c.id)).on_hover_text(hint)
        })
        .body(|ui| {
            ui.horizontal(|ui| {
                ui.add_space(36.0);
                ui.label(RichText::new(format!("⚡ {}", tr.tree_events)).color(Color32::from_gray(120)));
            });
            for ev in &c.events {
                let ekey = sel_event(rel, &c.id, &ev.event);
                let esel = cur.as_deref() == Some(ekey.as_str());
                let erow = ui.horizontal(|ui| {
                    ui.add_space(48.0);
                    full_width_select(ui, esel, ev.event.as_str()).on_hover_text(&ev.paragraph)
                }).inner;
                if erow.clicked() {
                    events.push(ProjectPanelEvent::Select(ekey));
                    events.push(ProjectPanelEvent::OpenEventCode {
                        form: form_path.to_path_buf(),
                        paragraph: ev.paragraph.clone(),
                    });
                }
            }
        });
    if header_inner.inner.clicked() {
        events.push(ProjectPanelEvent::Select(ckey));
        events.push(ProjectPanelEvent::InspectControl {
            form: form_path.to_path_buf(),
            ctrl_id: c.id.clone(),
        });
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
    cur:       &Option<String>,
    root:      &Option<PathBuf>,
    events:    &mut Vec<ProjectPanelEvent>,
) {
    let name = Path::new(rel).file_name().and_then(|n| n.to_str()).unwrap_or(rel);
    let key = sel_file(rel);
    let is_sel = cur.as_deref() == Some(key.as_str());
    let mut text = RichText::new(name);
    if let Some(c) = color {
        text = text.color(c);
    }
    let resp = ui.horizontal(|ui| {
        ui.add_space(8.0);
        status_dot(ui, status);
        ui.label(RichText::new(icon).size(ICON_SIZE));
        full_width_select(ui, is_sel, text).on_hover_text(rel)
    }).inner;

    // Single click selects + opens the file in the Main Pane.
    if resp.clicked() {
        events.push(ProjectPanelEvent::Select(key));
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
