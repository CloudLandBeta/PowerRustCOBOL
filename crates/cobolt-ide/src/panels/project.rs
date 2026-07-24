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

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
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

const INTERNAL_AGENT_DIR: &str = "agentic_ai";

fn is_hidden_tree_entry(depth: usize, name: &str) -> bool {
    depth == 0 && name.eq_ignore_ascii_case(INTERNAL_AGENT_DIR)
}

// ── Events ────────────────────────────────────────────────────────────────────

/// Actions emitted by the project panel for `CoboltApp` to handle.
#[derive(Clone)]
pub enum ProjectPanelEvent {
    /// Open the project-wide Grace chatbot in the IDE main pane.
    OpenGraceChat,
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
    /// Open the create-folder dialog for this project-relative Knowledge Base folder.
    CreateKnowledgeFolder(PathBuf),
    /// Confirm recursive deletion of this project-relative Knowledge Base folder.
    ConfirmDeleteKnowledgeFolder(PathBuf),
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
    /// Create a new folder inside `parent_rel` (project-relative). `category_root`
    /// is the category's root subdir, used to protect it (spec 033).
    CreateFolder {
        parent_rel: PathBuf,
        category_root: String,
    },
    /// Rename the project-relative folder `folder_rel` (guarded by `category_root`).
    RenameFolder {
        folder_rel: PathBuf,
        category_root: String,
    },
    /// Confirm recursive deletion of the project-relative folder `folder_rel`.
    DeleteFolder {
        folder_rel: PathBuf,
        category_root: String,
    },
    /// Drag-and-drop: move the tracked file `src_rel` into the folder
    /// `dest_dir_rel` (both project-relative).
    MoveInternal { src_rel: String, dest_dir_rel: String },
    /// OS file-manager drop: import `paths` into the project-relative folder
    /// `dest_dir_rel`.
    ImportOs {
        paths: Vec<PathBuf>,
        dest_dir_rel: String,
    },
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
    /// Project-relative folder currently under the pointer (updated each frame),
    /// used as the destination for an OS file-manager drop (spec 033, R10).
    hovered_dir: Option<String>,
    /// Ordered list of navigable rows for the current frame, in visible order,
    /// driving arrow-key navigation (spec 033, R15–R18).
    nav_rows: Vec<NavRow>,
    /// A row key that keyboard navigation asked to scroll into view; consumed on
    /// the next frame's render, keeping a one-row margin from the edge (R15).
    scroll_to_key: Option<String>,
}

/// One keyboard-navigable tree row (spec 033).
struct NavRow {
    /// Selection key (`sel_file(rel)` for leaves, a synthetic key for folders).
    key: String,
    depth: usize,
    /// `Some(collapsing_id)` for a folder row (used to expand/collapse it).
    folder_id: Option<egui::Id>,
    /// Whether a folder row is currently expanded.
    expanded: bool,
    /// The event to emit when the row is activated with Enter (leaf rows).
    activate: Option<ProjectPanelEvent>,
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
            hovered_dir: None,
            nav_rows: Vec::new(),
            scroll_to_key: None,
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
    // `click_and_drag` so file rows can act as drag sources for folder moves
    // (spec 033, R9); a plain click still reports `clicked()`.
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(full_w, h), egui::Sense::click_and_drag());

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
                if path.is_dir() && !name.starts_with('.') && !is_hidden_tree_entry(0, name) {
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

    /// The project-relative folder currently under the pointer, if any — the
    /// destination for an OS file-manager drop (spec 033, R10).
    pub fn hovered_dir(&self) -> Option<&str> {
        self.hovered_dir.as_deref()
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
        // Recomputed every frame from the folder headers under the pointer.
        self.hovered_dir = None;
        self.nav_rows.clear();

        let frame = crate::theme::glass_panel_frame(
            ctx.global_style().visuals.panel_fill,
            &crate::theme::active(),
        );
        let panel_resp = Panel::left("project_panel")
            .resizable(true)
            .default_size(410.0)
            // Keeps the project-wide Grace command at its requested 150 px
            // minimum after the panel's inner margins are accounted for.
            .min_size(170.0)
            .frame(frame)
            .show(panel_ui, |ui| match project {
                Some(proj) => {
                    let button_width = ui.available_width().max(150.0);
                    if ui
                        .add_sized([button_width, 34.0], egui::Button::new("👑 Grace"))
                        .on_hover_text("Open the project-wide Grace chatbot")
                        .clicked()
                    {
                        events.push(ProjectPanelEvent::OpenGraceChat);
                    }
                    ui.separator();
                    self.show_project_mode(ui, proj, &mut events, tr);
                }
                None => self.show_tree_mode(ui, &mut events, tr),
            });

        // Arrow-key navigation, scoped to when the pointer is over the tree so we
        // never hijack arrows from the editor or other panels (spec 033, R15–R18).
        if panel_resp.response.contains_pointer() {
            self.handle_tree_keys(ctx, &mut events);
        }

        // While a file is being dragged, paint a small document icon riding the
        // cursor (over the grabbing hand) so the drag reads as "moving a file".
        paint_drag_ghost(ctx);

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

    /// Handle Up/Down/Left/Right/Enter over the tree's visible rows (spec 033,
    /// R15–R18). Right expands (or descends into) a folder; Left always ascends to
    /// the parent (it never collapses). Expansion is toggled through egui's
    /// `CollapsingState` memory so the change is reflected on the next frame.
    fn handle_tree_keys(&mut self, ctx: &egui::Context, events: &mut Vec<ProjectPanelEvent>) {
        if self.nav_rows.is_empty() {
            return;
        }
        // Never steal keys while a text field (rename dialog, search, …) is focused.
        if ctx.memory(|m| m.focused()).is_some() {
            return;
        }
        let (up, down, left, right, enter) = ctx.input_mut(|i| {
            use egui::Key::{ArrowDown, ArrowLeft, ArrowRight, ArrowUp, Enter};
            let m = egui::Modifiers::NONE;
            (
                i.consume_key(m, ArrowUp),
                i.consume_key(m, ArrowDown),
                i.consume_key(m, ArrowLeft),
                i.consume_key(m, ArrowRight),
                i.consume_key(m, Enter),
            )
        });
        if !(up || down || left || right || enter) {
            return;
        }

        let set_open = |ctx: &egui::Context, id: egui::Id, open: bool| {
            if let Some(mut s) = egui::collapsing_header::CollapsingState::load(ctx, id) {
                s.set_open(open);
                s.store(ctx);
            }
        };

        let cur_idx = self
            .selected
            .as_ref()
            .and_then(|k| self.nav_rows.iter().position(|r| &r.key == k));

        let Some(idx) = cur_idx else {
            // No current selection in view → any nav key lands on the first row.
            self.select_row(0, events);
            ctx.request_repaint();
            return;
        };

        let folder_id = self.nav_rows[idx].folder_id;
        let expanded = self.nav_rows[idx].expanded;
        let depth = self.nav_rows[idx].depth;
        let activate = self.nav_rows[idx].activate.clone();

        // The row to move selection to (if any), and whether to toggle a folder.
        let mut new_idx: Option<usize> = None;
        if down && idx + 1 < self.nav_rows.len() {
            new_idx = Some(idx + 1);
        } else if up && idx > 0 {
            new_idx = Some(idx - 1);
        } else if right {
            if let Some(fid) = folder_id {
                if !expanded {
                    set_open(ctx, fid, true);
                    // Keep the folder selected and in view as it opens.
                    self.scroll_to_key = self.selected.clone();
                } else if idx + 1 < self.nav_rows.len() {
                    new_idx = Some(idx + 1);
                }
            }
        } else if left {
            // Left always ascends to the parent folder (never collapses).
            if let Some(p) = self.nav_rows[..idx].iter().rposition(|r| r.depth < depth) {
                new_idx = Some(p);
            }
        } else if enter {
            if let Some(fid) = folder_id {
                set_open(ctx, fid, !expanded);
            } else if let Some(ev) = activate {
                events.push(ProjectPanelEvent::Select(self.nav_rows[idx].key.clone()));
                events.push(ev);
            }
        }

        if let Some(ni) = new_idx {
            self.select_row(ni, events);
        }
        ctx.request_repaint();
    }

    /// If `key` is the row keyboard navigation asked to reveal, scroll it into
    /// view keeping a one-row margin from the edge (egui clamps at the ends, so
    /// the first/last row simply rests against the border). Consumes the request.
    fn maybe_scroll_to(&mut self, ui: &Ui, key: &str, rect: egui::Rect) {
        if self.scroll_to_key.as_deref() == Some(key) {
            // Pad by ~one row top and bottom so the highlighted item is never the
            // very first/last visible line unless it is genuinely at an end.
            let margin = egui::vec2(0.0, 26.0);
            ui.scroll_to_rect(rect.expand2(margin), None);
            self.scroll_to_key = None;
        }
    }

    /// Select the nav row at `idx`: highlight it, request that it scroll into
    /// view, and — so navigation loads the element like a click — emit its
    /// activation (property/editor load) when it is a file row (spec 033, R15).
    fn select_row(&mut self, idx: usize, events: &mut Vec<ProjectPanelEvent>) {
        let key = self.nav_rows[idx].key.clone();
        self.selected = Some(key.clone());
        self.scroll_to_key = Some(key.clone());
        if let Some(ev) = self.nav_rows[idx].activate.clone() {
            events.push(ProjectPanelEvent::Select(key));
            events.push(ev);
        }
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
                    // L2 — fixed IDE-owned project categories. Internal agent
                    // configuration remains managed through Agents Manager.
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

            if name.starts_with('.') || is_hidden_tree_entry(depth, name) {
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
        let is_assets = cat == Category::Assets;
        let is_knowledge_base = cat == Category::Documentation;
        let root = self.root.clone();

        let id = ui.make_persistent_id(("project_cat", label));
        let (_toggle, header_inner, _body) =
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
            .show_header(ui, |ui| {
                match cat {
                    Category::IndexedFiles => tree_icon(ui, draw_indexed_icon),
                    _ => tree_icon(ui, draw_folder_icon),
                }
                let header_hover =
                    ui.interact(ui.max_rect(), id.with("cat_hover"), egui::Sense::hover());
                ui.label(RichText::new(label).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Generated Code is IDE-owned (forms populate it) — no file [+].
                    if let Some(kind) = kind {
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
                    }
                    // New-folder affordance for every category (spec 033, R1).
                    if ui
                        .small_button("📁+")
                        .on_hover_text(tr.tree_new_folder)
                        .clicked()
                    {
                        if is_knowledge_base {
                            // Documentation keeps its Knowledge-Base-aware create
                            // path (indexes + doc-sync).
                            events.push(ProjectPanelEvent::CreateKnowledgeFolder(PathBuf::from(
                                cobolt_agents::project_knowledge::KNOWLEDGE_BASE_ROOT,
                            )));
                        } else {
                            events.push(ProjectPanelEvent::CreateFolder {
                                parent_rel: PathBuf::from(cat.root_subdir()),
                                category_root: cat.root_subdir().to_string(),
                            });
                        }
                    }
                });
                header_hover
            })
            .body(|ui| {
                if is_knowledge_base {
                    if let Some(root) = &root {
                        self.show_knowledge_base(ui, root, cur, events, tr);
                    }
                    return;
                }
                if is_assets {
                    if let Some(root) = &root {
                        self.show_assets_folder(ui, root, cur, events, tr);
                    }
                    return;
                }
                let files: Vec<String> = proj.files_in(cat).to_vec();
                let structure = FolderStructure::build(root.as_deref(), cat, &files);
                if structure.is_empty() {
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
                let sub = cat.root_subdir().to_string();
                self.render_folder_children(ui, cat, &sub, &structure, 0, &root, cur, events, tr);
            });
        // Category header as a fallback OS-drop / move target = the category root
        // (dropping a file here moves it out of any subfolder). A more specific
        // subfolder under the pointer overrides this (set later in the frame).
        if self.hovered_dir.is_none() && header_inner.inner.contains_pointer() {
            self.hovered_dir = Some(cat.root_subdir().to_string());
        }
        ui.add_space(2.0);
    }

    /// Recursively render the subfolders and tracked files whose parent directory
    /// is `dir_rel`, for a flat (`cobolt.toml`-tracked) category. Folder nodes are
    /// collapsing headers with a New/Rename/Delete context menu and act as
    /// drag-drop move targets (spec 033, R1, R9).
    #[allow(clippy::too_many_arguments)]
    fn render_folder_children(
        &mut self,
        ui: &mut Ui,
        cat: Category,
        dir_rel: &str,
        structure: &FolderStructure,
        depth: usize,
        root: &Option<PathBuf>,
        cur: &Option<String>,
        events: &mut Vec<ProjectPanelEvent>,
        tr: &Tr,
    ) {
        let category_root = cat.root_subdir().to_string();
        // Subfolders first, then files (both already sorted by the builder).
        for folder_rel in structure.subfolders_of(dir_rel) {
            let name = folder_rel.rsplit('/').next().unwrap_or(folder_rel);
            let fid = ui.make_persistent_id(("cat_folder", cat.root_subdir(), folder_rel));
            let expanded = egui::collapsing_header::CollapsingState::load(ui.ctx(), fid)
                .map(|s| s.is_open())
                .unwrap_or(false);
            let folder_key = format!("catfolder:{folder_rel}");
            self.nav_rows.push(NavRow {
                key: folder_key.clone(),
                depth,
                folder_id: Some(fid),
                expanded,
                activate: None,
            });
            let (_toggle, header_inner, _body) =
                egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), fid, false)
                    .show_header(ui, |ui| {
                        ui.add_space(8.0 + depth as f32 * 14.0);
                        tree_icon(ui, draw_folder_icon);
                        ui.label(RichText::new(name).strong())
                    })
                    .body(|ui| {
                        self.render_folder_children(
                            ui,
                            cat,
                            folder_rel,
                            structure,
                            depth + 1,
                            root,
                            cur,
                            events,
                            tr,
                        );
                    });
            let header_resp = header_inner.inner;
            self.maybe_scroll_to(ui, &folder_key, header_resp.rect);
            if header_resp.contains_pointer() {
                self.hovered_dir = Some(folder_rel.to_string());
            }
            folder_context_menu(&header_resp, folder_rel, &category_root, tr, events);
            accept_file_drop(ui, &header_resp, folder_rel, events);
        }
        for rel in structure.files_of(dir_rel) {
            self.render_folder_leaf(ui, cat, rel, depth, root, cur, events, tr);
        }
    }

    /// Render one tracked file inside a folder, dispatching to the category's
    /// existing item renderer. The row is also a drag source (spec 033, R9).
    #[allow(clippy::too_many_arguments)]
    fn render_folder_leaf(
        &mut self,
        ui: &mut Ui,
        cat: Category,
        rel: &str,
        depth: usize,
        root: &Option<PathBuf>,
        cur: &Option<String>,
        events: &mut Vec<ProjectPanelEvent>,
        tr: &Tr,
    ) {
        // Record this leaf for keyboard navigation (spec 033, R15, R18).
        let abs = root.as_ref().map(|d| d.join(rel));
        let activate = abs.as_ref().map(|p| match cat {
            Category::Forms => ProjectPanelEvent::InspectForm(p.clone()),
            Category::IndexedFiles => ProjectPanelEvent::InspectIndexedFile(p.clone()),
            _ => ProjectPanelEvent::Open(p.clone()),
        });
        let key = sel_file(rel);
        // Forms and indexed files are themselves expandable (their controls/
        // events/fields subtrees), so record their CollapsingState id — computed
        // with the same source the item renderer uses — so Right expands them
        // (spec 033, R16). Other categories are true leaves.
        let (folder_id, expanded) = match cat {
            Category::Forms => {
                let id = ui.make_persistent_id(("form_item", rel));
                let open = egui::collapsing_header::CollapsingState::load(ui.ctx(), id)
                    .map(|s| s.is_open())
                    .unwrap_or(false);
                (Some(id), open)
            }
            Category::IndexedFiles => {
                let id = ui.make_persistent_id(("indexed_item", rel));
                let open = egui::collapsing_header::CollapsingState::load(ui.ctx(), id)
                    .map(|s| s.is_open())
                    .unwrap_or(false);
                (Some(id), open)
            }
            _ => (None, false),
        };
        self.nav_rows.push(NavRow {
            key: key.clone(),
            depth: depth + 1,
            folder_id,
            expanded,
            activate,
        });
        let st = self.status_for(rel);
        // Measure the vertical span the row occupies so keyboard navigation can
        // scroll it into view (the leaf renderers don't return a response).
        let top_before = ui.cursor().top();
        match cat {
            Category::Forms => self.show_form_item(ui, rel, root, cur, events, tr),
            Category::IndexedFiles => self.show_indexed_item(ui, rel, root, cur, events, tr),
            Category::Generated => file_row(
                ui,
                rel,
                "🔒",
                Some(crate::theme::active().ed_generated),
                false,
                true,
                st,
                cur,
                root,
                events,
            ),
            _ => file_row(ui, rel, "doc", None, true, false, st, cur, root, events),
        }
        if self.scroll_to_key.as_deref() == Some(key.as_str()) {
            let bottom_after = ui.cursor().top();
            let rect = egui::Rect::from_min_max(
                egui::pos2(ui.max_rect().left(), top_before),
                egui::pos2(ui.max_rect().right(), bottom_after),
            );
            self.maybe_scroll_to(ui, &key, rect);
        }
    }

    fn show_knowledge_base(
        &mut self,
        ui: &mut Ui,
        root: &Path,
        cur: &Option<String>,
        events: &mut Vec<ProjectPanelEvent>,
        tr: &Tr,
    ) {
        let directory = root.join(cobolt_agents::project_knowledge::KNOWLEDGE_BASE_ROOT);
        let _ = std::fs::create_dir_all(&directory);
        let entries = sorted_directory_entries(&directory);
        if entries.is_empty() {
            ui.label(
                RichText::new("  No Knowledge Base documents or subfolders.")
                    .color(crate::theme::active().text_dim)
                    .small(),
            );
            return;
        }
        for path in entries {
            self.show_knowledge_path(ui, root, &path, 0, cur, events, tr);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn show_knowledge_path(
        &mut self,
        ui: &mut Ui,
        root: &Path,
        path: &Path,
        depth: usize,
        cur: &Option<String>,
        events: &mut Vec<ProjectPanelEvent>,
        tr: &Tr,
    ) {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("?");
        let relative =
            PathBuf::from(relative_to(path, root).unwrap_or_else(|| path.display().to_string()));
        if path.is_dir() {
            let id = ui.make_persistent_id(("knowledge_dir", path));
            let (_toggle, header_inner, _body) =
                egui::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    id,
                    false,
                )
                .show_header(ui, |ui| {
                    ui.add_space(8.0 + depth as f32 * 14.0);
                    tree_icon(ui, draw_folder_icon);
                    ui.label(RichText::new(name).strong())
                })
                .body(|ui| {
                    for child in sorted_directory_entries(path) {
                        self.show_knowledge_path(ui, root, &child, depth + 1, cur, events, tr);
                    }
                });
            let response = header_inner.inner;
            let relative_menu = relative.clone();
            response.context_menu(|ui| {
                if ui.button(tr.tree_new_folder).clicked() {
                    events.push(ProjectPanelEvent::CreateKnowledgeFolder(relative_menu.clone()));
                    ui.close();
                }
                if ui.button(tr.tree_rename_folder).clicked() {
                    events.push(ProjectPanelEvent::RenameFolder {
                        folder_rel: relative_menu.clone(),
                        category_root: Category::Documentation.root_subdir().to_string(),
                    });
                    ui.close();
                }
                if ui.button(tr.tree_delete_folder).clicked() {
                    events.push(ProjectPanelEvent::ConfirmDeleteKnowledgeFolder(
                        relative_menu.clone(),
                    ));
                    ui.close();
                }
            });
            let folder_rel = relative.to_string_lossy().replace('\\', "/");
            if response.contains_pointer() {
                self.hovered_dir = Some(folder_rel.clone());
            }
            accept_file_drop(ui, &response, &folder_rel, events);
            return;
        }

        let relative_string = relative.to_string_lossy().replace('\\', "/");
        file_row(
            ui,
            &relative_string,
            "doc",
            None,
            true,
            false,
            self.status_for(&relative_string),
            cur,
            &Some(root.to_path_buf()),
            events,
        );
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
        tr: &Tr,
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
            self.show_asset_path(ui, root, &path, 0, cur, events, tr);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn show_asset_path(
        &mut self,
        ui: &mut Ui,
        root: &Path,
        path: &Path,
        depth: usize,
        cur: &Option<String>,
        events: &mut Vec<ProjectPanelEvent>,
        tr: &Tr,
    ) {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        if path.is_dir() {
            let folder_rel = relative_to(path, root).unwrap_or_else(|| path.display().to_string());
            let folder_rel = folder_rel.replace('\\', "/");
            let id = ui.make_persistent_id(("asset_dir", path));
            let (_toggle, header_inner, _body) =
                egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
                .show_header(ui, |ui| {
                    ui.add_space(8.0 + depth as f32 * 14.0);
                    tree_icon(ui, draw_folder_icon);
                    ui.label(RichText::new(name).strong())
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
                        self.show_asset_path(ui, root, &child, depth + 1, cur, events, tr);
                    }
                });
            let header_resp = header_inner.inner;
            if header_resp.contains_pointer() {
                self.hovered_dir = Some(folder_rel.clone());
            }
            let category_root = Category::Assets.root_subdir().to_string();
            folder_context_menu(&header_resp, &folder_rel, &category_root, tr, events);
            accept_file_drop(ui, &header_resp, &folder_rel, events);
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
        // Drag source: move the asset into another folder (spec 033, R9).
        resp.dnd_set_drag_payload(rel.clone());
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
        resp.dnd_set_drag_payload(rel.to_string());
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
        resp.dnd_set_drag_payload(rel.to_string());
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

/// A flat category's folder hierarchy, derived from the union of its on-disk
/// directories (so empty folders survive) and its `cobolt.toml`-tracked files
/// (spec 033, Q2). Keys and values are project-relative, forward-slash paths.
struct FolderStructure {
    /// parent dir rel → its direct child dir rels (sorted).
    subfolders: BTreeMap<String, Vec<String>>,
    /// parent dir rel → the tracked files directly in it (sorted).
    files: BTreeMap<String, Vec<String>>,
    /// Whether there is anything at all to show (files or subfolders).
    has_any: bool,
}

impl FolderStructure {
    fn build(root: Option<&Path>, cat: Category, tracked: &[String]) -> Self {
        let sub = cat.root_subdir().to_string();
        let mut dirs: BTreeSet<String> = BTreeSet::new();
        if let Some(root) = root {
            collect_dirs_rel(&root.join(&sub), root, &mut dirs);
        }
        let mut files: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for raw in tracked {
            let rel = raw.replace('\\', "/");
            // Bare (root-level) tracked files show under the category root.
            let parent = match rel.rfind('/') {
                Some(i) => rel[..i].to_string(),
                None => sub.clone(),
            };
            // Record the ancestor chain so folders exist even without a disk walk.
            let mut ancestor = parent.clone();
            while ancestor != sub && !ancestor.is_empty() {
                dirs.insert(ancestor.clone());
                match ancestor.rfind('/') {
                    Some(i) => ancestor.truncate(i),
                    None => break,
                }
            }
            files.entry(parent).or_default().push(rel);
        }
        let mut subfolders: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for dir in &dirs {
            if *dir == sub {
                continue;
            }
            let parent = match dir.rfind('/') {
                Some(i) => dir[..i].to_string(),
                None => String::new(),
            };
            subfolders.entry(parent).or_default().push(dir.clone());
        }
        for v in subfolders.values_mut() {
            v.sort();
            v.dedup();
        }
        for v in files.values_mut() {
            v.sort();
            v.dedup();
        }
        let has_any = !tracked.is_empty() || subfolders.values().any(|v| !v.is_empty());
        Self {
            subfolders,
            files,
            has_any,
        }
    }

    fn is_empty(&self) -> bool {
        !self.has_any
    }

    fn subfolders_of(&self, dir: &str) -> &[String] {
        self.subfolders.get(dir).map(Vec::as_slice).unwrap_or(&[])
    }

    fn files_of(&self, dir: &str) -> &[String] {
        self.files.get(dir).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// Recursively collect directory paths under `abs`, as project-relative
/// forward-slash strings. Hidden (dot) directories are skipped.
fn collect_dirs_rel(abs: &Path, root: &Path, out: &mut BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(abs) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let hidden = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('.'))
            .unwrap_or(false);
        if hidden {
            continue;
        }
        if let Some(rel) = relative_to(&path, root) {
            out.insert(rel.replace('\\', "/"));
            collect_dirs_rel(&path, root, out);
        }
    }
}

/// Attach the New/Rename/Delete-folder context menu to a folder header
/// (spec 033, R1, R4, R5).
fn folder_context_menu(
    resp: &egui::Response,
    folder_rel: &str,
    category_root: &str,
    tr: &Tr,
    events: &mut Vec<ProjectPanelEvent>,
) {
    resp.context_menu(|ui| {
        if ui.button(tr.tree_new_folder).clicked() {
            events.push(ProjectPanelEvent::CreateFolder {
                parent_rel: PathBuf::from(folder_rel),
                category_root: category_root.to_string(),
            });
            ui.close();
        }
        if ui.button(tr.tree_rename_folder).clicked() {
            events.push(ProjectPanelEvent::RenameFolder {
                folder_rel: PathBuf::from(folder_rel),
                category_root: category_root.to_string(),
            });
            ui.close();
        }
        if ui.button(tr.tree_delete_folder).clicked() {
            events.push(ProjectPanelEvent::DeleteFolder {
                folder_rel: PathBuf::from(folder_rel),
                category_root: category_root.to_string(),
            });
            ui.close();
        }
    });
}

/// While a tree file drag is in progress (a `String` payload is live), paint a
/// document icon on the cursor over the grabbing hand, so the gesture visibly
/// represents a file being moved (spec 033, R9/R11). A soft rounded backing plate
/// keeps the glyph legible on any background.
fn paint_drag_ghost(ctx: &Context) {
    if egui::DragAndDrop::payload::<String>(ctx).is_none() {
        return;
    }
    let Some(pos) = ctx.pointer_interact_pos() else {
        return;
    };
    // Sit the icon just above-right of the hand's hotspot so both read clearly.
    let center = pos + egui::vec2(11.0, -11.0);
    let size = egui::vec2(ICON_SIZE, ICON_SIZE);
    let icon_rect = egui::Rect::from_center_size(center, size);
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("prc_tree_drag_ghost"),
    ));
    let theme = crate::theme::active();
    // Backing plate for contrast, then the crisp vector document glyph.
    painter.rect_filled(
        icon_rect.expand(3.0),
        egui::CornerRadius::same(4),
        egui::Color32::from_rgba_unmultiplied(0, 0, 0, if theme.dark { 150 } else { 90 }),
    );
    let color = if theme.dark {
        egui::Color32::WHITE
    } else {
        theme.text_bright
    };
    draw_document_icon(&painter, icon_rect.shrink(1.8), color);
}

/// Make `resp` a drop target for an in-tree file drag (spec 033, R9). When a
/// `String` payload (the source rel path) is hovering and the pointer releases
/// here, emit a `MoveInternal` into `dest_dir_rel`.
fn accept_file_drop(
    ui: &mut Ui,
    resp: &egui::Response,
    dest_dir_rel: &str,
    events: &mut Vec<ProjectPanelEvent>,
) {
    if !resp.contains_pointer() {
        return;
    }
    let Some(payload) = egui::DragAndDrop::payload::<String>(ui.ctx()) else {
        return;
    };
    // Highlight the valid target while dragging over it (R11).
    ui.painter().rect_stroke(
        resp.rect,
        egui::CornerRadius::same(4),
        egui::Stroke::new(1.5, crate::theme::active().selection),
        egui::StrokeKind::Inside,
    );
    if ui.input(|i| i.pointer.any_released()) {
        let src = (*payload).clone();
        let _ = egui::DragAndDrop::take_payload::<String>(ui.ctx());
        events.push(ProjectPanelEvent::MoveInternal {
            src_rel: src,
            dest_dir_rel: dest_dir_rel.to_string(),
        });
    }
}

fn sorted_directory_entries(directory: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .collect();
    entries.sort_by(|left, right| {
        right.is_dir().cmp(&left.is_dir()).then_with(|| {
            left.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
                .cmp(
                    &right
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase(),
                )
        })
    });
    entries
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

    // Drag source: carry the tracked rel path for a folder move (spec 033, R9).
    resp.dnd_set_drag_payload(rel.to_string());

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
mod folder_structure_tests {
    use super::*;

    #[test]
    fn groups_tracked_files_by_folder_and_lists_subfolders() {
        let tracked = vec![
            "forms/login.cfrm".to_string(),
            "forms/customers/order.cfrm".to_string(),
            "forms/customers/invoice.cfrm".to_string(),
            "top.cfrm".to_string(), // bare file → shown under the category root
        ];
        // root = None → structure comes purely from tracked paths.
        let s = FolderStructure::build(None, Category::Forms, &tracked);
        assert!(!s.is_empty());
        // The category root ("forms") has the login form, the bare file, and one
        // subfolder "forms/customers".
        assert_eq!(s.subfolders_of("forms"), &["forms/customers".to_string()]);
        let top_files = s.files_of("forms");
        assert!(top_files.contains(&"forms/login.cfrm".to_string()));
        assert!(top_files.contains(&"top.cfrm".to_string()));
        // The subfolder holds its two forms.
        assert_eq!(
            s.files_of("forms/customers"),
            &[
                "forms/customers/invoice.cfrm".to_string(),
                "forms/customers/order.cfrm".to_string(),
            ]
        );
    }

    #[test]
    fn empty_category_reports_empty() {
        let s = FolderStructure::build(None, Category::Forms, &[]);
        assert!(s.is_empty());
    }
}

#[cfg(test)]
mod keyboard_nav_tests {
    use super::*;

    /// Build a panel with a synthetic nav-row list and drive `handle_tree_keys`
    /// by injecting key events, asserting the resulting selection / expansion.
    fn panel_with_rows() -> ProjectPanel {
        let mut p = ProjectPanel::new();
        // forms/ (folder, collapsed) ; forms/a.cfrm ; forms/customers/ (folder)
        p.nav_rows = vec![
            NavRow {
                key: "catfolder:forms/customers".into(),
                depth: 0,
                folder_id: Some(egui::Id::new("f_customers")),
                expanded: false,
                activate: None,
            },
            NavRow {
                key: sel_file("forms/a.cfrm"),
                depth: 1,
                folder_id: None,
                expanded: false,
                activate: Some(ProjectPanelEvent::Open(PathBuf::from("/x/forms/a.cfrm"))),
            },
        ];
        p
    }

    fn run_keys(panel: &mut ProjectPanel, keys: &[egui::Key]) -> Vec<ProjectPanelEvent> {
        let ctx = egui::Context::default();
        let mut events = Vec::new();
        let events_in: Vec<egui::Event> = keys
            .iter()
            .map(|k| egui::Event::Key {
                key: *k,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            })
            .collect();
        let input = egui::RawInput {
            events: events_in,
            ..Default::default()
        };
        let _ = ctx.run_ui(input, |root_ui| {
            let c = root_ui.ctx().clone();
            panel.handle_tree_keys(&c, &mut events);
        });
        events
    }

    #[test]
    fn down_moves_selection_to_next_visible_row() {
        let mut p = panel_with_rows();
        p.selected = Some("catfolder:forms/customers".into());
        run_keys(&mut p, &[egui::Key::ArrowDown]);
        assert_eq!(p.selected.as_deref(), Some(sel_file("forms/a.cfrm").as_str()));
    }

    #[test]
    fn navigating_onto_a_file_row_activates_it_and_requests_scroll() {
        let mut p = panel_with_rows();
        p.selected = Some("catfolder:forms/customers".into());
        let events = run_keys(&mut p, &[egui::Key::ArrowDown]);
        // Landing on a file row loads it (like a single click) …
        assert!(events
            .iter()
            .any(|e| matches!(e, ProjectPanelEvent::Open(_))));
        // … and asks to be scrolled into view.
        assert_eq!(p.scroll_to_key.as_deref(), Some(sel_file("forms/a.cfrm").as_str()));
    }

    #[test]
    fn up_moves_selection_to_previous_row() {
        let mut p = panel_with_rows();
        p.selected = Some(sel_file("forms/a.cfrm"));
        run_keys(&mut p, &[egui::Key::ArrowUp]);
        assert_eq!(
            p.selected.as_deref(),
            Some("catfolder:forms/customers")
        );
    }

    #[test]
    fn left_on_leaf_ascends_to_parent_folder() {
        let mut p = panel_with_rows();
        p.selected = Some(sel_file("forms/a.cfrm"));
        run_keys(&mut p, &[egui::Key::ArrowLeft]);
        assert_eq!(p.selected.as_deref(), Some("catfolder:forms/customers"));
    }

    #[test]
    fn left_on_expanded_folder_ascends_without_collapsing() {
        // A depth-1 expanded folder selected; Left moves to its depth-0 parent
        // instead of collapsing it.
        let mut p = ProjectPanel::new();
        p.nav_rows = vec![
            NavRow {
                key: "catfolder:forms/customers".into(),
                depth: 0,
                folder_id: Some(egui::Id::new("f_customers")),
                expanded: true,
                activate: None,
            },
            NavRow {
                key: "catfolder:forms/customers/orders".into(),
                depth: 1,
                folder_id: Some(egui::Id::new("f_orders")),
                expanded: true,
                activate: None,
            },
        ];
        p.selected = Some("catfolder:forms/customers/orders".into());
        run_keys(&mut p, &[egui::Key::ArrowLeft]);
        assert_eq!(p.selected.as_deref(), Some("catfolder:forms/customers"));
    }

    #[test]
    fn enter_on_leaf_activates_it() {
        let mut p = panel_with_rows();
        p.selected = Some(sel_file("forms/a.cfrm"));
        let events = run_keys(&mut p, &[egui::Key::Enter]);
        assert!(events
            .iter()
            .any(|e| matches!(e, ProjectPanelEvent::Open(_))));
    }
}

#[cfg(test)]
mod project_tree_visibility_tests {
    use super::*;

    #[test]
    fn hides_only_the_root_agentic_ai_directory() {
        assert!(is_hidden_tree_entry(0, "agentic_ai"));
        assert!(is_hidden_tree_entry(0, "Agentic_AI"));
        assert!(!is_hidden_tree_entry(0, "Documentation"));
        assert!(!is_hidden_tree_entry(1, "agentic_ai"));
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
