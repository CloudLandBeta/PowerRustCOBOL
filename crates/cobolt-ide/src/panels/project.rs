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

use egui::{Color32, Context, Panel, RichText, ScrollArea, Ui};

use cobolt_forms::model::Form;
use cobolt_indexed::{IndexedDefinition, IndexedField};

use crate::i18n::Tr;
use crate::panels::toolbox;
use crate::project_model::{relative_to, Category, CoboltProject, ElementStatus, FileKind};

/// Icon size in the tree — 80 % larger than the default body text (~12 px).
const ICON_SIZE: f32 = 21.6;

/// Fixed width of the expand/collapse arrow column on a control row. Reserved
/// on *every* control (blank when there is nothing to expand) so the status dot
/// and label always align in a single column regardless of the arrow.
const ARROW_GUTTER: f32 = 14.0;

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
    /// Open an indexed-file editor viewport (double-click a `.cidx` node).
    OpenIndexedEditor(PathBuf),
    /// Show indexed-file properties in the Main Pane (single click).
    InspectIndexedFile(PathBuf),
    /// Show a field's properties (click a field under an indexed file).
    InspectIndexedField { cidx: PathBuf, field_id: String },
    /// Open a widget event's handler — its nested COBOL program — in the
    /// editor (click an Events entry). `paragraph` is the nested PROGRAM-ID
    /// (the name is historical; see `EventBinding::paragraph`).
    OpenEventCode { form: PathBuf, paragraph: String },
    /// Internal: a tree element was selected (consumed by the panel, not the app).
    Select(String),
    /// User clicked `[+]` on a category — **create** a new item of this kind.
    Create(FileKind),
    /// User chose "Import existing…" — add an existing file of this kind.
    Add(FileKind),
    /// User chose "Remove from project" — contains the relative path string.
    Remove(String),
    /// User requested deleting a form file from the project tree.
    ConfirmRemoveForm(PathBuf),
    /// User requested deleting generated COBOL from the project tree.
    ConfirmRemoveGenerated(PathBuf),
    /// User requested deleting an asset from the project tree.
    ConfirmRemoveAsset(PathBuf),
    /// User requested removing an indexed file — prompts confirmation.
    ConfirmRemoveIndexed(String),
    /// User clicked the top/root project node in the tree (📁 ProjectName).
    /// Shows the project Settings form (parameters) in the main work area.
    ShowProjectSettings,
}

// ── ProjectPanel ──────────────────────────────────────────────────────────────

pub struct ProjectPanel {
    /// Root directory of the open project / directory (if any).
    pub root: Option<PathBuf>,
    /// Expanded directories (tree mode only).
    expanded: HashSet<PathBuf>,
    /// mtime-keyed cache of loaded forms (for the controls sub-tree).
    forms: HashMap<PathBuf, (SystemTime, Form)>,
    /// mtime-keyed cache of loaded `.cidx` definitions (field sub-tree).
    indexed: HashMap<PathBuf, (SystemTime, IndexedDefinition)>,
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
            indexed: HashMap::new(),
            status: HashMap::new(),
            selected: None,
        }
    }
}

/// Selection keys (unique per tree element).
fn sel_file(rel: &str) -> String {
    format!("file:{rel}")
}
fn sel_ctrl(rel: &str, id: &str) -> String {
    format!("ctrl:{rel}#{id}")
}
fn sel_event(rel: &str, id: &str, ev: &str) -> String {
    format!("event:{rel}#{id}@{ev}")
}
fn sel_idx_field(rel: &str, id: &str) -> String {
    format!("idxfld:{rel}#{id}")
}

/// A selectable tree row that fills the remaining width: a full-width rounded
/// **pill** (selection / hover) painted behind a **left-aligned** label. (Using
/// `add_sized` centred the text and made it shift while resizing.)
fn full_width_select(
    ui: &mut Ui,
    selected: bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    let theme = crate::theme::active();
    let text: egui::WidgetText = text.into();
    let full_w = ui.available_width();
    let galley = text.into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        (full_w - 14.0).max(0.0),
        egui::TextStyle::Body,
    );
    let h = (galley.size().y + 8.0).max(24.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(full_w, h), egui::Sense::click());

    // Full-width rounded pill for selection / hover.
    let fill = if selected {
        theme.selection
    } else if resp.hovered() {
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, if theme.dark { 14 } else { 22 })
    } else {
        egui::Color32::TRANSPARENT
    };
    if fill != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(7), fill);
    }

    // Left-aligned, vertically-centred label. RichText colours (e.g. generated
    // blue) are preserved; plain text uses the fallback colour. Selected rows
    // keep theme-appropriate contrast: white on dark themes (dark selection
    // pill), the theme's dark bright-text on light ones (light selection pill).
    let fallback = if selected {
        if theme.dark {
            egui::Color32::WHITE
        } else {
            theme.text_bright
        }
    } else {
        ui.visuals().text_color()
    };
    let text_pos = egui::pos2(rect.left() + 7.0, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(text_pos, galley, fallback);
    resp
}

impl ProjectPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the root directory shown in tree mode.
    pub fn set_root(&mut self, root: impl Into<PathBuf>) {
        let root = root.into();
        self.expanded.clear();
        self.expand_first_level_dirs(&root);
        self.root = Some(root);
    }

    fn expand_first_level_dirs(&mut self, root: &Path) {
        if let Ok(entries) = std::fs::read_dir(root) {
            for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if path.is_dir() && !name.starts_with('.') {
                    self.expanded.insert(path);
                }
            }
        }
    }

    /// Drop the cached copy of a form so the controls sub-tree reloads it (after
    /// an inline-inspector edit / designer save).
    pub fn refresh_form(&mut self, path: &Path) {
        self.forms.remove(path);
    }

    /// Drop cached `.cidx` so the field sub-tree reloads after a save.
    pub fn refresh_indexed(&mut self, path: &Path) {
        self.indexed.remove(path);
    }

    /// Set the semaphore status for a tracked element (relative path).
    pub fn set_status(&mut self, rel: &str, s: ElementStatus) {
        self.status.insert(rel.replace('\\', "/"), s);
    }

    /// The status for `rel` — defaults to `Changed` (yellow / not yet tested).
    fn status_for(&self, rel: &str) -> ElementStatus {
        self.status
            .get(&rel.replace('\\', "/"))
            .copied()
            .unwrap_or_default()
    }

    /// The relative path of the currently selected *file* element, if any
    /// (used by the toolbar to gate Debug on a Generated Code selection).
    pub fn selected_file(&self) -> Option<&str> {
        self.selected
            .as_deref()
            .and_then(|s| s.strip_prefix("file:"))
    }

    /// Render the project panel and return all events that occurred this frame.
    ///
    /// * `project` — `Some(&project)` to render in project mode, `None` for
    ///   the raw file-tree fallback.
    pub fn show(
        &mut self,
        panel_ui: &mut egui::Ui,
        project: Option<&CoboltProject>,
        tr: &Tr,
    ) -> Vec<ProjectPanelEvent> {
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        let mut events = Vec::new();

        let frame = crate::theme::glass_panel_frame(
            ctx.global_style().visuals.panel_fill,
            &crate::theme::active(),
        );
        Panel::left("project_panel")
            .resizable(true)
            .default_size(410.0)
            .min_size(140.0)
            .frame(frame)
            .show(panel_ui, |ui| match project {
                Some(proj) => self.show_project_mode(ui, proj, &mut events, tr),
                None => self.show_tree_mode(ui, &mut events, tr),
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
        ui: &mut Ui,
        proj: &CoboltProject,
        events: &mut Vec<ProjectPanelEvent>,
        tr: &Tr,
    ) {
        // Current selection (read-only snapshot for highlighting); clicks emit a
        // `Select` event that `show()` applies after rendering.
        let cur = self.selected.clone();

        // Tree guide lines connecting nodes: egui draws a vertical line on the
        // left of each indented (collapsed) block from the noninteractive
        // bg_stroke. Enable it here (it is off globally) and colour it with the
        // theme's line tone — light-grey on dark themes, dark-grey on light.
        ui.visuals_mut().indent_has_left_vline = true;
        ui.visuals_mut().widgets.noninteractive.bg_stroke =
            egui::Stroke::new(1.0, crate::theme::active().line());

        // Expand/collapse arrows 50 % larger than egui's default (14 → 21) so
        // they are comfortable to spot and hit.
        ui.spacing_mut().icon_width = 21.0;
        ui.spacing_mut().icon_width_inner = 12.0;

        ScrollArea::vertical()
            .id_salt("project_panel_scroll")
            .show(ui, |ui| {
                // L1 — the project itself is the root node; categories live under it.
                let root_id = ui.make_persistent_id("project_root");
                let mut root_clicked = false;
                egui::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    root_id,
                    true,
                )
                .show_header(ui, |ui| {
                    tree_icon(ui, draw_folder_icon);
                    let name_label = egui::Label::new(RichText::new(&proj.project.name).strong())
                        .sense(egui::Sense::click());
                    let name_resp = ui
                        .add(name_label)
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if name_resp.clicked() {
                        root_clicked = true;
                    }
                    ui.label(
                        RichText::new(format!("v{}", proj.project.version))
                            .color(crate::theme::active().text_dim)
                            .small(),
                    );
                })
                .body(|ui| {
                    // L2 — editable, project-local agent prompts/skills first,
                    // then the fixed IDE-owned project categories.
                    self.show_agentic_ai_category(ui, &cur, events, tr);
                    for cat in Category::TOP {
                        self.show_category(ui, cat, proj, &cur, events, tr);
                    }
                });
                if root_clicked {
                    events.push(ProjectPanelEvent::ShowProjectSettings);
                    // Highlight the root as selected (the Select will be consumed after show()).
                    events.push(ProjectPanelEvent::Select("project:root".to_owned()));
                }
            });
    }

    // ── Tree mode ─────────────────────────────────────────────────────────────

    fn show_tree_mode(&mut self, ui: &mut Ui, events: &mut Vec<ProjectPanelEvent>, tr: &Tr) {
        ui.heading(tr.panel_project);
        ui.separator();

        ScrollArea::vertical()
            .id_salt("project_tree_scroll")
            .show(ui, |ui| match self.root.clone() {
                Some(root) => {
                    if let Some(path) = self.show_dir(ui, &root, 0) {
                        events.push(ProjectPanelEvent::Open(path));
                    }
                }
                None => {
                    ui.label(
                        RichText::new(tr.no_project_open).color(crate::theme::active().text_dim),
                    );
                }
            });
    }

    fn show_dir(&mut self, ui: &mut Ui, dir: &Path, depth: usize) -> Option<PathBuf> {
        let mut opened: Option<PathBuf> = None;

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return None,
        };

        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();

        for path in &paths {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");

            if name.starts_with('.') {
                continue;
            }

            let indent = depth as f32 * 14.0;

            if path.is_dir() {
                let expanded = self.expanded.contains(path);
                let arrow = if expanded { "▾" } else { "▸" };
                ui.horizontal(|ui| {
                    ui.add_space(indent);
                    tree_icon(ui, draw_folder_icon);
                    if ui
                        .selectable_label(false, format!("{arrow} {name}"))
                        .clicked()
                    {
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
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if !matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "cbl" | "cob" | "cpy" | "cfrm" | "toml" | "txt"
                ) {
                    continue;
                }
                ui.horizontal(|ui| {
                    ui.add_space(indent + 14.0);
                    tree_icon(ui, draw_document_icon);
                    if ui.selectable_label(false, name).double_clicked() {
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
    let color = Color32::from_rgb(r, g, b);
    // A crisp, solid filled knob (painted, not a font glyph) for clear
    // green/yellow/red semaphore visibility.
    let d = 13.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(d, d), egui::Sense::hover());
    let center = rect.center();
    let radius = d * 0.42;
    let painter = ui.painter();
    painter.circle_filled(center, radius, color);
    // Subtle dark ring so the knob reads on any background.
    painter.circle_stroke(
        center,
        radius,
        egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 110)),
    );
    resp.on_hover_text(status.tooltip());
}

/// Allocate a fixed square for a  tree icon and invoke the draw closure with
/// (painter, rect, color). Keeps icons crisp vector strokes (no emoji/glyphs)
/// so they render identically on every OS.
pub(crate) fn tree_icon(ui: &mut Ui, draw: impl FnOnce(&egui::Painter, egui::Rect, Color32)) {
    let size = egui::vec2(ICON_SIZE, ICON_SIZE);
    let (rect, _resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let color = ui.visuals().text_color();
        draw(painter, rect.shrink(1.8), color);
    }
}

/// Classic folder icon (used for project root and generic categories).
fn draw_folder_icon(p: &egui::Painter, r: egui::Rect, c: Color32) {
    let s = egui::Stroke::new(1.5, c);
    let tab_h = r.height() * 0.28;
    let tab_w = r.width() * 0.42;
    // Tab (filled)
    let tab = egui::Rect::from_min_size(r.min, egui::vec2(tab_w, tab_h));
    p.rect_filled(tab, 1.5, c);
    // Body (stroke only for "open" feel)
    let body = egui::Rect::from_min_size(
        r.min + egui::vec2(0.0, tab_h * 0.45),
        egui::vec2(r.width(), r.height() - tab_h * 0.45),
    );
    p.rect_stroke(body, 1.8, s, egui::StrokeKind::Middle);
}

/// Indexed file / data cabinet (replacement for 🗂️). Two record lines + index accent tab.
/// Used both for the "Indexed Files" category header and for each .cidx entry (e.g. CUSTOMER-FILE).
pub(crate) fn draw_indexed_icon(p: &egui::Painter, r: egui::Rect, c: Color32) {
    let s = egui::Stroke::new(1.55, c);
    let body = r.shrink(1.5);
    p.rect_stroke(body, 1.4, s, egui::StrokeKind::Middle);
    // Record / row lines inside the "file"
    for i in 0..3 {
        let y = body.min.y + body.height() * (0.30 + i as f32 * 0.18);
        p.line_segment(
            [
                egui::pos2(body.min.x + 3.0, y),
                egui::pos2(body.max.x - 3.0, y),
            ],
            egui::Stroke::new(1.0, c),
        );
    }
    // Small "index key" or tab accent (right side, near top)
    let ax = body.max.x - 4.5;
    let ay = body.min.y + body.height() * 0.20;
    p.rect_filled(
        egui::Rect::from_center_size(egui::pos2(ax, ay), egui::vec2(4.2, 2.8)),
        0.8,
        c,
    );
}

/// Simple document / file icon for forms, sources, assets, docs etc.
fn draw_document_icon(p: &egui::Painter, r: egui::Rect, c: Color32) {
    let s = egui::Stroke::new(1.5, c);
    let body = r.shrink(2.0);
    p.rect_stroke(body, 1.5, s, egui::StrokeKind::Middle);
    // Folded corner suggestion (small diagonal)
    let fold = egui::pos2(body.max.x - 5.0, body.min.y + 2.0);
    p.line_segment(
        [egui::pos2(body.max.x - 2.0, body.min.y + 5.0), fold],
        egui::Stroke::new(1.0, c),
    );
    // Three text lines
    for i in 0..3 {
        let y = body.min.y + body.height() * (0.35 + i as f32 * 0.16);
        p.line_segment(
            [
                egui::pos2(body.min.x + 3.0, y),
                egui::pos2(body.max.x - 4.0, y),
            ],
            egui::Stroke::new(0.9, c),
        );
    }
}

/// Tiny padlock for generated / locked items (replaces 🔒).
fn draw_lock_icon(p: &egui::Painter, r: egui::Rect, c: Color32) {
    let s = egui::Stroke::new(1.4, c);
    let cx = r.center().x;
    let cy = r.center().y;
    // Shackle as three line segments (avoids PathShape differences)
    let sw = 1.3;
    // left vertical
    p.line_segment(
        [
            egui::pos2(cx - 3.2, cy + 0.5),
            egui::pos2(cx - 3.2, cy - 2.8),
        ],
        egui::Stroke::new(sw, c),
    );
    // top arc (approx with short horiz)
    p.line_segment(
        [
            egui::pos2(cx - 3.2, cy - 2.8),
            egui::pos2(cx + 3.2, cy - 2.8),
        ],
        egui::Stroke::new(sw, c),
    );
    // right vertical
    p.line_segment(
        [
            egui::pos2(cx + 3.2, cy - 2.8),
            egui::pos2(cx + 3.2, cy + 0.5),
        ],
        egui::Stroke::new(sw, c),
    );
    // Body
    let body = egui::Rect::from_center_size(
        egui::pos2(cx, cy + 3.2),
        egui::vec2(r.width() * 0.70, r.height() * 0.40),
    );
    p.rect_stroke(body, 1.2, s, egui::StrokeKind::Middle);
}

// ── Category tree node (L2) ─────────────────────────────────────────────────────

impl ProjectPanel {
    fn show_agentic_ai_category(
        &mut self,
        ui: &mut Ui,
        cur: &Option<String>,
        events: &mut Vec<ProjectPanelEvent>,
        tr: &Tr,
    ) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let ensure_error = crate::agent::ensure_project_agentic_files(&root).err();

        let id = ui.make_persistent_id("project_cat_agentic_ai");
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
            .show_header(ui, |ui| {
                tree_icon(ui, draw_folder_icon);
                ui.label(RichText::new(tr.cat_agentic_ai).strong());
            })
            .body(|ui| {
                if let Some(err) = ensure_error {
                    ui.label(
                        RichText::new(format!("  {err}"))
                            .small()
                            .color(Color32::from_rgb(220, 120, 120)),
                    );
                    return;
                }
                self.show_agent_node(
                    ui,
                    &root,
                    crate::agent::FORM_DESIGNER_AGENT_DIR,
                    tr.agent_form_designer,
                    cur,
                    events,
                    tr,
                );
                self.show_agent_node(
                    ui,
                    &root,
                    crate::agent::EVENT_HANDLER_AGENT_DIR,
                    tr.agent_event_handler,
                    cur,
                    events,
                    tr,
                );
            });
        ui.add_space(2.0);
    }

    fn show_agent_node(
        &mut self,
        ui: &mut Ui,
        root: &Path,
        agent_dir: &str,
        label: &str,
        cur: &Option<String>,
        events: &mut Vec<ProjectPanelEvent>,
        tr: &Tr,
    ) {
        let id = ui.make_persistent_id(("project_agent", agent_dir));
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
            .show_header(ui, |ui| {
                ui.add_space(8.0);
                tree_icon(ui, draw_folder_icon);
                ui.label(RichText::new(label).strong());
            })
            .body(|ui| {
                self.agent_file_row(
                    ui,
                    root,
                    &format!("agentic_ai/{agent_dir}/system-prompt.md"),
                    "system-prompt.md",
                    cur,
                    events,
                );
                self.agent_file_row(
                    ui,
                    root,
                    &format!("agentic_ai/{agent_dir}/steering.md"),
                    tr.agent_steering,
                    cur,
                    events,
                );

                let skills_id = ui.make_persistent_id(("project_agent_skills", agent_dir));
                egui::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    skills_id,
                    false,
                )
                .show_header(ui, |ui| {
                    ui.add_space(18.0);
                    tree_icon(ui, draw_folder_icon);
                    ui.label(RichText::new(tr.agent_skills).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("+")
                            .on_hover_text(format!("{}: {}", tr.tree_create_hover, tr.agent_skills))
                            .clicked()
                        {
                            if let Some((rel, path)) = self.create_agent_skill_file(root, agent_dir)
                            {
                                events.push(ProjectPanelEvent::Select(sel_file(&rel)));
                                events.push(ProjectPanelEvent::Open(path));
                            }
                        }
                    });
                })
                .body(|ui| {
                    for skill in self.agent_skill_files(root, agent_dir) {
                        let rel = format!("agentic_ai/{agent_dir}/skills/{skill}");
                        self.agent_file_row(ui, root, &rel, &skill, cur, events);
                    }
                });
            });
    }

    fn agent_skill_files(&self, root: &Path, agent_dir: &str) -> Vec<String> {
        let dir = root.join("agentic_ai").join(agent_dir).join("skills");
        let mut files: Vec<String> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                (path.extension().and_then(|e| e.to_str()) == Some("md"))
                    .then(|| path.file_name()?.to_str().map(|s| s.to_string()))
                    .flatten()
            })
            .collect();
        files.sort();
        files
    }

    fn create_agent_skill_file(&self, root: &Path, agent_dir: &str) -> Option<(String, PathBuf)> {
        let skill_dir = root.join("agentic_ai").join(agent_dir).join("skills");
        std::fs::create_dir_all(&skill_dir).ok()?;
        for idx in 1..1000 {
            let name = format!("custom-skill-{idx}.md");
            let path = skill_dir.join(&name);
            if !path.exists() {
                let title = name.trim_end_matches(".md");
                let text = format!(
                    "# {title}\n\nDescribe the project-specific guidance this agent should follow.\n"
                );
                std::fs::write(&path, text).ok()?;
                let rel = format!("agentic_ai/{agent_dir}/skills/{name}");
                return Some((rel, path));
            }
        }
        None
    }

    fn agent_file_row(
        &mut self,
        ui: &mut Ui,
        root: &Path,
        rel: &str,
        label: &str,
        cur: &Option<String>,
        events: &mut Vec<ProjectPanelEvent>,
    ) {
        let key = sel_file(rel);
        let is_sel = cur.as_deref() == Some(key.as_str());
        let resp = ui
            .horizontal(|ui| {
                ui.add_space(28.0);
                tree_icon(ui, draw_document_icon);
                full_width_select(ui, is_sel, RichText::new(label)).on_hover_text(rel)
            })
            .inner;
        if resp.clicked() || resp.double_clicked() {
            events.push(ProjectPanelEvent::Select(key));
            events.push(ProjectPanelEvent::Open(root.join(rel)));
        }
    }

    /// mtime-cached load of a `.cidx` for the field sub-tree.
    fn indexed_for(&mut self, abs: &Path) -> Option<IndexedDefinition> {
        let mtime = std::fs::metadata(abs).and_then(|m| m.modified()).ok()?;
        if let Some((t, d)) = self.indexed.get(abs) {
            if *t == mtime {
                return Some(d.clone());
            }
        }
        let def = cobolt_indexed::load_indexed(abs).ok()?;
        self.indexed.insert(abs.to_path_buf(), (mtime, def.clone()));
        Some(def)
    }

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
        ui: &mut Ui,
        cat: Category,
        proj: &CoboltProject,
        cur: &Option<String>,
        events: &mut Vec<ProjectPanelEvent>,
        tr: &Tr,
    ) {
        let (label, kind): (&str, Option<FileKind>) = match cat {
            Category::Forms => (tr.panel_forms, Some(FileKind::Form)),
            Category::IndexedFiles => (tr.cat_indexed_files, Some(FileKind::Indexed)),
            Category::CommonCode => (tr.cat_common_code, Some(FileKind::Source)),
            Category::Generated => (tr.cat_generated_code, None),
            Category::Assets => (tr.panel_assets, Some(FileKind::Asset)),
            Category::Documentation => (tr.cat_documentation, Some(FileKind::Documentation)),
        };
        let is_generated = cat == Category::Generated;
        let is_forms = cat == Category::Forms;
        let is_indexed = cat == Category::IndexedFiles;
        let is_assets = cat == Category::Assets;
        let root = self.root.clone();

        let id = ui.make_persistent_id(("project_cat", label));
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
            .show_header(ui, |ui| {
                match cat {
                    Category::IndexedFiles => tree_icon(ui, draw_indexed_icon),
                    _ => tree_icon(ui, draw_folder_icon),
                }
                ui.label(RichText::new(label).strong());
                // Generated Code is IDE-owned (forms populate it) — no [+].
                if let Some(kind) = kind {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let plus = ui
                            .small_button("+")
                            .on_hover_text(format!("{}: {label}", tr.tree_create_hover));
                        if plus.clicked() {
                            events.push(ProjectPanelEvent::Create(kind));
                        }
                        // Right-click → import an existing file into this category.
                        plus.context_menu(|ui| {
                            if ui.button(tr.tree_import_existing).clicked() {
                                events.push(ProjectPanelEvent::Add(kind));
                                ui.close();
                            }
                        });
                    });
                }
            })
            .body(|ui| {
                if is_assets {
                    if let Some(root) = &root {
                        self.show_assets_folder(ui, root, cur, events);
                    }
                    return;
                }
                let files: Vec<String> = proj.files_in(cat).to_vec();
                if files.is_empty() {
                    let hint = if is_generated {
                        tr.tree_generated_empty
                    } else {
                        tr.tree_empty
                    };
                    ui.label(
                        RichText::new(format!("  {hint}"))
                            .color(crate::theme::active().text_dim)
                            .small(),
                    );
                    return;
                }
                for rel in &files {
                    let st = self.status_for(rel);
                    if is_forms {
                        self.show_form_item(ui, rel, &root, cur, events, tr);
                    } else if is_indexed {
                        self.show_indexed_item(ui, rel, &root, cur, events, tr);
                    } else if is_generated {
                        file_row(
                            ui,
                            rel,
                            "🔒",
                            Some(crate::theme::active().ed_generated),
                            false,
                            true,
                            st,
                            cur,
                            &root,
                            events,
                        );
                    } else {
                        // The icon string is only used as a selector for vector draw
                        // (see file_row); real drawing no longer depends on FileKind::icon().
                        file_row(ui, rel, "doc", None, true, false, st, cur, &root, events);
                    }
                }
            });
        ui.add_space(2.0);
    }

    fn assets_dir(root: &Path) -> PathBuf {
        let preferred = root.join("Assets");
        if preferred.exists() {
            preferred
        } else {
            let legacy = root.join("assets");
            if legacy.exists() {
                legacy
            } else {
                preferred
            }
        }
    }

    fn show_assets_folder(
        &mut self,
        ui: &mut Ui,
        root: &Path,
        cur: &Option<String>,
        events: &mut Vec<ProjectPanelEvent>,
    ) {
        let dir = Self::assets_dir(root);
        let _ = std::fs::create_dir_all(&dir);
        if !dir.exists() {
            ui.label(
                RichText::new("  Could not create Assets folder.")
                    .color(Color32::from_rgb(220, 120, 120))
                    .small(),
            );
            return;
        }

        let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        entries.sort_by(|a, b| {
            let ad = a.is_dir();
            let bd = b.is_dir();
            bd.cmp(&ad).then_with(|| {
                a.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .cmp(
                        &b.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_ascii_lowercase(),
                    )
            })
        });

        if entries.is_empty() {
            ui.label(
                RichText::new("  Drop/import files into Assets.")
                    .color(crate::theme::active().text_dim)
                    .small(),
            );
            return;
        }

        for path in entries {
            self.show_asset_path(ui, root, &path, 0, cur, events);
        }
    }

    fn show_asset_path(
        &mut self,
        ui: &mut Ui,
        root: &Path,
        path: &Path,
        depth: usize,
        cur: &Option<String>,
        events: &mut Vec<ProjectPanelEvent>,
    ) {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        if path.is_dir() {
            let id = ui.make_persistent_id(("asset_dir", path));
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
                .show_header(ui, |ui| {
                    ui.add_space(8.0 + depth as f32 * 14.0);
                    tree_icon(ui, draw_folder_icon);
                    ui.label(RichText::new(name).strong());
                })
                .body(|ui| {
                    let mut children: Vec<PathBuf> = std::fs::read_dir(path)
                        .into_iter()
                        .flatten()
                        .flatten()
                        .map(|e| e.path())
                        .collect();
                    children.sort_by(|a, b| {
                        let ad = a.is_dir();
                        let bd = b.is_dir();
                        bd.cmp(&ad).then_with(|| {
                            a.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("")
                                .to_ascii_lowercase()
                                .cmp(
                                    &b.file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("")
                                        .to_ascii_lowercase(),
                                )
                        })
                    });
                    for child in children {
                        self.show_asset_path(ui, root, &child, depth + 1, cur, events);
                    }
                });
            return;
        }

        let rel = relative_to(path, root).unwrap_or_else(|| path.display().to_string());
        let key = sel_file(&rel);
        let is_sel = cur.as_deref() == Some(key.as_str());
        let resp = ui
            .horizontal(|ui| {
                ui.add_space(8.0 + depth as f32 * 14.0);
                status_dot(ui, self.status_for(&rel));
                tree_icon(ui, draw_document_icon);
                if ui
                    .small_button("🗑")
                    .on_hover_text(format!("Delete asset {rel}"))
                    .clicked()
                {
                    events.push(ProjectPanelEvent::ConfirmRemoveAsset(path.to_path_buf()));
                }
                full_width_select(ui, is_sel, RichText::new(name)).on_hover_text(&rel)
            })
            .inner;
        if resp.clicked() || resp.double_clicked() {
            events.push(ProjectPanelEvent::Select(key));
            events.push(ProjectPanelEvent::Open(path.to_path_buf()));
        }
    }

    /// A form item (L3) that expands to its controls grouped by toolbox category;
    /// each control with handlers expands to an "Events" group.
    fn show_form_item(
        &mut self,
        ui: &mut Ui,
        rel: &str,
        root: &Option<PathBuf>,
        cur: &Option<String>,
        events: &mut Vec<ProjectPanelEvent>,
        tr: &Tr,
    ) {
        let name = Path::new(rel)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(rel);
        let abs = root.as_ref().map(|d| d.join(rel));
        let form = abs.as_ref().and_then(|p| self.form_for(p));
        let form_status = self.status_for(rel);
        let form_key = sel_file(rel);
        let form_selected = cur.as_deref() == Some(form_key.as_str());

        let id = ui.make_persistent_id(("form_item", rel));
        // Only the root project node starts open. User-expanded form nodes keep
        // their egui memory state after startup.
        let (_toggle, header_inner, _body) =
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
                .show_header(ui, |ui| {
                    ui.add_space(8.0);
                    status_dot(ui, form_status);
                    if let Some(p) = &abs {
                        let delete_resp = ui
                            .small_button("🗑")
                            .on_hover_text(format!("Delete form {rel}"));
                        if delete_resp.clicked() {
                            events.push(ProjectPanelEvent::ConfirmRemoveForm(p.clone()));
                        }
                    }
                    tree_icon(ui, draw_document_icon);
                    full_width_select(ui, form_selected, RichText::new(name)).on_hover_text(rel)
                })
                .body(|ui| {
                    let Some(form) = &form else {
                        ui.label(
                            RichText::new("  (could not read form)")
                                .color(crate::theme::active().text_dim)
                                .small(),
                        );
                        return;
                    };
                    let Some(form_path) = &abs else {
                        return;
                    };
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
                        egui::collapsing_header::CollapsingState::load_with_default_open(
                            ui.ctx(),
                            gid,
                            false,
                        )
                        .show_header(ui, |ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new(format!(
                                    "{} ({})",
                                    toolbox::category_display(cat_key),
                                    in_cat.len()
                                ))
                                .color(crate::theme::active().text_dim),
                            );
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

    /// An indexed-file item (L3) expanding to its record fields.
    fn show_indexed_item(
        &mut self,
        ui: &mut Ui,
        rel: &str,
        root: &Option<PathBuf>,
        cur: &Option<String>,
        events: &mut Vec<ProjectPanelEvent>,
        tr: &Tr,
    ) {
        let name = Path::new(rel)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or(rel);
        let abs = root.as_ref().map(|d| d.join(rel));
        let def = abs.as_ref().and_then(|p| self.indexed_for(p));
        let status = self.status_for(rel);
        let file_key = sel_file(rel);
        let file_selected = cur.as_deref() == Some(file_key.as_str());

        let mut remove_clicked = false;
        let id = ui.make_persistent_id(("indexed_item", rel));
        let (_toggle, header_inner, _body) =
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
                .show_header(ui, |ui| {
                    ui.add_space(8.0);
                    status_dot(ui, status);
                    tree_icon(ui, draw_indexed_icon);
                    if ui
                        .small_button("🗑")
                        .on_hover_text("Remove indexed file from project")
                        .clicked()
                    {
                        remove_clicked = true;
                    }
                    full_width_select(ui, file_selected, RichText::new(name)).on_hover_text(rel)
                })
                .body(|ui| {
                    let Some(def) = &def else {
                        ui.label(
                            RichText::new("  (could not read .cidx)")
                                .color(crate::theme::active().text_dim)
                                .small(),
                        );
                        return;
                    };
                    let Some(cidx_path) = &abs else {
                        return;
                    };
                    for field in &def.fields {
                        indexed_field_node(ui, rel, field, 0, status, cur, events, cidx_path);
                    }
                });
        let resp = header_inner.inner;
        resp.context_menu(|ui| {
            if ui.button("Remove from project").clicked() {
                remove_clicked = true;
                ui.close();
            }
        });
        if remove_clicked {
            events.push(ProjectPanelEvent::ConfirmRemoveIndexed(rel.to_string()));
        } else if let Some(p) = &abs {
            if resp.double_clicked() {
                events.push(ProjectPanelEvent::OpenIndexedEditor(p.clone()));
            } else if resp.clicked() {
                events.push(ProjectPanelEvent::Select(file_key));
                events.push(ProjectPanelEvent::InspectIndexedFile(p.clone()));
            }
        }
        ui.add_space(1.0);
    }
}

fn indexed_field_node(
    ui: &mut Ui,
    rel: &str,
    field: &IndexedField,
    depth: usize,
    status: ElementStatus,
    cur: &Option<String>,
    events: &mut Vec<ProjectPanelEvent>,
    cidx_abs: &Path,
) {
    let indent = 20.0 + depth as f32 * 14.0;
    let key = sel_idx_field(rel, &field.name);
    let selected = cur.as_deref() == Some(key.as_str());
    let row_resp = ui
        .horizontal(|ui| {
            ui.add_space(indent);
            status_dot(ui, status);
            full_width_select(
                ui,
                selected,
                RichText::new(format!("{:02} {}", field.level, field.name)).monospace(),
            )
        })
        .inner;
    if row_resp.clicked() {
        events.push(ProjectPanelEvent::Select(key));
        events.push(ProjectPanelEvent::InspectIndexedField {
            cidx: cidx_abs.to_path_buf(),
            field_id: field.name.clone(),
        });
    }
    for child in &field.children {
        indexed_field_node(ui, rel, child, depth + 1, status, cur, events, cidx_abs);
    }
}

/// One control (L5). A leaf row, unless it has event handlers — then it expands
/// to an "Events" group listing them (click → open the event's COBOL paragraph).
#[allow(clippy::too_many_arguments)]
fn control_node(
    ui: &mut Ui,
    rel: &str,
    form_path: &Path,
    c: &cobolt_forms::model::Control,
    status: ElementStatus,
    cur: &Option<String>,
    events: &mut Vec<ProjectPanelEvent>,
    tr: &Tr,
) {
    let ckey = sel_ctrl(rel, &c.id);
    let csel = cur.as_deref() == Some(ckey.as_str());
    let hint = format!("{:?}", c.control_type);
    let has_events = !c.events.is_empty();

    // Open-state for the (optional) Events subtree. Persisted per control (the
    // same way CollapsingState stores its openness), so the expansion survives
    // frames and app restarts; collapsed by default.
    let id = ui.make_persistent_id(("ctrl_open", rel, &c.id));
    let mut open = has_events && ui.data_mut(|d| d.get_persisted::<bool>(id).unwrap_or(false));

    // Every control row reserves the SAME leading layout — a fixed indent plus a
    // fixed-width arrow gutter — so the status dot and label line up in one
    // column whether or not the control has an expandable Events node.
    let crow = ui
        .horizontal(|ui| {
            ui.add_space(20.0);
            let (arrow_rect, arrow_resp) =
                ui.allocate_exact_size(egui::vec2(ARROW_GUTTER, 24.0), egui::Sense::click());
            // Test probe: expose the arrow's screen rect under a reconstructable
            // global id so headless tests can click it wherever layout puts it.
            #[cfg(test)]
            ui.data_mut(|d| {
                d.insert_temp(
                    egui::Id::new(("arrow_probe", rel, c.id.as_str())),
                    arrow_rect,
                )
            });
            if has_events {
                // Paint the triangle as a filled path (like egui's own collapsing
                // icon) — a text glyph here depends on the loaded fonts and can
                // render invisibly faint or missing. Use the standard interact
                // foreground colour so it is clearly visible on every theme.
                let color = ui.style().interact(&arrow_resp).fg_stroke.color;
                let c = arrow_rect.center();
                let r = 6.75;
                let points = if open {
                    vec![
                        // ▾
                        egui::pos2(c.x - r, c.y - r * 0.55),
                        egui::pos2(c.x + r, c.y - r * 0.55),
                        egui::pos2(c.x, c.y + r * 0.80),
                    ]
                } else {
                    vec![
                        // ▸
                        egui::pos2(c.x - r * 0.55, c.y - r),
                        egui::pos2(c.x + r * 0.80, c.y),
                        egui::pos2(c.x - r * 0.55, c.y + r),
                    ]
                };
                ui.painter().add(egui::Shape::convex_polygon(
                    points,
                    color,
                    egui::Stroke::NONE,
                ));
                if arrow_resp
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    open = !open;
                }
            }
            status_dot(ui, status);
            full_width_select(ui, csel, c.id.as_str()).on_hover_text(hint)
        })
        .inner;

    // Double-clicking the row is a second way to expand/collapse the Events
    // subtree (the single click still selects + inspects the control).
    if has_events && crow.double_clicked() {
        open = !open;
    }
    if has_events {
        ui.data_mut(|d| d.insert_persisted(id, open));
    }
    #[cfg(test)]
    ui.data_mut(|d| d.insert_temp(egui::Id::new(("open_probe", rel, c.id.as_str())), open));
    if crow.clicked() {
        events.push(ProjectPanelEvent::Select(ckey));
        events.push(ProjectPanelEvent::InspectControl {
            form: form_path.to_path_buf(),
            ctrl_id: c.id.clone(),
        });
    }

    if open {
        // The Events group sits one indent step under the control row, and the
        // event entries one further step under it — the same visual nesting the
        // controls have under their category header.
        let events_indent = 20.0 + ARROW_GUTTER + 16.0;
        ui.horizontal(|ui| {
            ui.add_space(events_indent);
            ui.label(
                RichText::new(format!("⚡ {}", tr.tree_events))
                    .color(crate::theme::active().text_dim),
            );
        });
        for ev in &c.events {
            let ekey = sel_event(rel, &c.id, &ev.event);
            let esel = cur.as_deref() == Some(ekey.as_str());
            let erow = ui
                .horizontal(|ui| {
                    ui.add_space(events_indent + 28.0);
                    full_width_select(ui, esel, ev.event.as_str()).on_hover_text(&ev.paragraph)
                })
                .inner;
            if erow.clicked() {
                events.push(ProjectPanelEvent::Select(ekey));
                events.push(ProjectPanelEvent::OpenEventCode {
                    form: form_path.to_path_buf(),
                    paragraph: ev.paragraph.clone(),
                });
            }
        }
    }
}

/// One file row (L3) inside a non-form category. Single click opens it in the
/// Main Pane; `color` tints the label; `removable` adds a remove context menu.
#[allow(clippy::too_many_arguments)]
fn file_row(
    ui: &mut Ui,
    rel: &str,
    icon: &str,
    color: Option<Color32>,
    removable: bool,
    delete_generated: bool,
    status: ElementStatus,
    cur: &Option<String>,
    root: &Option<PathBuf>,
    events: &mut Vec<ProjectPanelEvent>,
) {
    let name = Path::new(rel)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(rel);
    let key = sel_file(rel);
    let is_sel = cur.as_deref() == Some(key.as_str());
    let mut text = RichText::new(name);
    if let Some(c) = color {
        text = text.color(c);
    }
    let resp = ui
        .horizontal(|ui| {
            ui.add_space(8.0);
            status_dot(ui, status);
            if icon == "🔒" {
                tree_icon(ui, draw_lock_icon);
            } else {
                tree_icon(ui, draw_document_icon);
            }
            if delete_generated {
                if let Some(dir) = root {
                    if ui
                        .small_button("🗑")
                        .on_hover_text(format!("Delete generated COBOL {rel}"))
                        .clicked()
                    {
                        events.push(ProjectPanelEvent::ConfirmRemoveGenerated(dir.join(rel)));
                    }
                }
            }
            full_width_select(ui, is_sel, text).on_hover_text(rel)
        })
        .inner;

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
                ui.close();
            }
        });
    }
}

#[cfg(test)]
mod control_node_tests {
    use super::*;
    use cobolt_forms::model::{Control, ControlType, EventBinding};

    /// Render one `control_node` frame headlessly. Returns the height the node
    /// occupied — the collapsed row is one ~24 px line; an expanded Events
    /// subtree makes it strictly taller.
    fn frame(
        ctx: &egui::Context,
        at: f64,
        events_in: Vec<egui::Event>,
        c: &Control,
        out: &mut Vec<ProjectPanelEvent>,
    ) -> f32 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(800.0, 600.0),
            )),
            time: Some(at),
            events: events_in,
            ..Default::default()
        };
        let mut height = 0.0;
        let _ = ctx.run_ui(input, |root_ui| {
            let ctx = root_ui.ctx().clone();
            let ctx = &ctx;
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(root_ui, |ui| {
                    let tr = crate::i18n::Language::English.tr();
                    let used = ui
                        .vertical(|ui| {
                            control_node(
                                ui,
                                "forms/f.cfrm",
                                Path::new("/tmp/f.cfrm"),
                                c,
                                ElementStatus::Changed,
                                &None,
                                out,
                                &tr,
                            );
                        })
                        .response
                        .rect
                        .height();
                    height = used;
                });
        });
        height
    }

    #[test]
    fn arrow_click_expands_events_subtree() {
        let mut c = Control::new("Button-1", ControlType::Button, 10, 10);
        c.events
            .push(EventBinding::new("onClick", "BUTTON-1--ONCLICK"));

        let ctx = egui::Context::default();
        let mut out = Vec::new();
        let arrow = egui::pos2(27.0, 12.0); // indent 20 + gutter 14 → centre ≈ x 27

        let collapsed = frame(&ctx, 0.00, vec![], &c, &mut out);
        frame(
            &ctx,
            0.05,
            vec![
                egui::Event::PointerMoved(arrow),
                egui::Event::PointerButton {
                    pos: arrow,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
            ],
            &c,
            &mut out,
        );
        let on_release = frame(
            &ctx,
            0.10,
            vec![egui::Event::PointerButton {
                pos: arrow,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            }],
            &c,
            &mut out,
        );
        let settled = frame(&ctx, 0.15, vec![], &c, &mut out);

        assert!(
            collapsed > 0.0 && collapsed < 40.0,
            "collapsed row should be a single line, got {collapsed}"
        );
        assert!(
            on_release > collapsed + 20.0,
            "clicking the arrow must expand the Events subtree \
             (collapsed {collapsed}, after click {on_release})"
        );
        assert!(
            settled > collapsed + 20.0,
            "expansion must persist on the next frame (got {settled})"
        );
    }

    #[test]
    fn control_without_events_is_single_row() {
        let c = Control::new("Label-1", ControlType::Label, 10, 10);
        let ctx = egui::Context::default();
        let mut out = Vec::new();
        let h = frame(&ctx, 0.0, vec![], &c, &mut out);
        assert!(
            h > 0.0 && h < 40.0,
            "event-less control must stay one row, got {h}"
        );
    }
}

#[cfg(test)]
mod control_node_in_real_wrappers {
    use super::*;
    use cobolt_forms::model::{Control, ControlType, EventBinding};

    /// Render one frame of the REAL structure around control rows:
    /// SidePanel → ScrollArea → category CollapsingState body → control rows.
    fn frame(
        ctx: &egui::Context,
        at: f64,
        events_in: Vec<egui::Event>,
        controls: &[Control],
        out: &mut Vec<ProjectPanelEvent>,
    ) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(900.0, 700.0),
            )),
            time: Some(at),
            events: events_in,
            ..Default::default()
        };
        let _ = ctx.run_ui(input, |root_ui| {
            let ctx = root_ui.ctx().clone();
            let ctx = &ctx;
            let tr = crate::i18n::Language::English.tr();
            Panel::left("project_panel")
                .default_size(410.0)
                .show(root_ui, |ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        let gid = ui.make_persistent_id(("form_grp", "forms/f.cfrm", "common"));
                        egui::collapsing_header::CollapsingState::load_with_default_open(
                            ctx, gid, true,
                        )
                        .show_header(ui, |ui| {
                            ui.label("Common (2)");
                        })
                        .body(|ui| {
                            for c in controls {
                                control_node(
                                    ui,
                                    "forms/f.cfrm",
                                    Path::new("/tmp/f.cfrm"),
                                    c,
                                    ElementStatus::Changed,
                                    &None,
                                    out,
                                    &tr,
                                );
                            }
                        });
                    });
                });
        });
    }

    #[test]
    fn arrow_click_expands_inside_panel_scroll_and_category() {
        let label = Control::new("Label-1", ControlType::Label, 10, 10);
        let mut button = Control::new("Button-1", ControlType::Button, 10, 60);
        button
            .events
            .push(EventBinding::new("onClick", "BUTTON-1--ONCLICK"));
        let controls = vec![label, button];

        let ctx = egui::Context::default();
        let mut out = Vec::new();
        let arrow_id = egui::Id::new(("arrow_probe", "forms/f.cfrm", "Button-1"));
        let open_id = egui::Id::new(("open_probe", "forms/f.cfrm", "Button-1"));

        // Frame 1+2: settle the collapsing animation, read the arrow's rect.
        frame(&ctx, 0.00, vec![], &controls, &mut out);
        frame(&ctx, 0.40, vec![], &controls, &mut out);
        let arrow: egui::Rect = ctx
            .data(|d| d.get_temp(arrow_id))
            .expect("arrow rect probe not set — control row did not render");
        let open0: bool = ctx.data(|d| d.get_temp(open_id)).unwrap_or(false);
        assert!(!open0, "should start collapsed");

        // Click the arrow centre: move + press, then release.
        let p = arrow.center();
        frame(
            &ctx,
            0.45,
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
            ],
            &controls,
            &mut out,
        );
        frame(
            &ctx,
            0.50,
            vec![egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            }],
            &controls,
            &mut out,
        );
        frame(&ctx, 0.55, vec![], &controls, &mut out);

        let open: bool = ctx.data(|d| d.get_temp(open_id)).unwrap_or(false);
        assert!(
            open,
            "clicking the arrow at {p:?} must expand Button-1's Events subtree \
             inside SidePanel/ScrollArea/CollapsingState"
        );
    }
}
