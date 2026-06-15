// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Documentation viewer — a separate window (egui viewport) that renders the
//! embedded PowerRustCOBOL documentation (Markdown + Mermaid diagrams) with a
//! custom theme-aware renderer ([`crate::panels::md_render`]).
//!
//! Each document is a separate entry in the left-hand list; selecting one loads
//! it, and `Cmd+O` adds an external `.md` file to the list. Mermaid blocks are
//! rendered to images via the pure-Rust `mermaid-rs-renderer` (→ SVG) + `resvg`.
//! The window is theme- and I18N-aware and offers File/View/Help menus, zoom, a
//! font-size control, an outline (table of contents), in-document search with
//! highlighting, a view-source modal, keyboard shortcuts, and PDF print.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use egui::{Context, Key, ViewportBuilder, ViewportCommand, ViewportId};

use crate::docs_embed::{self, DocEntry};
use crate::i18n::{Language, Tr};
use crate::panels::md_render::{self, RenderOpts};
use crate::version::VERSION;

/// A rendered Mermaid diagram (or the error that prevented it).
enum MermaidTex {
    Ok { tex: egui::TextureHandle, size: egui::Vec2 },
    Err(String),
}

/// The documentation viewer window state.
pub struct DocViewer {
    pub open: bool,

    lang: Language,
    docs: Vec<DocEntry>,
    /// Docs opened from disk (`Cmd+O`), preserved across language rebuilds.
    extra: Vec<DocEntry>,
    selected: Option<usize>,
    outline: Vec<(u8, String, usize)>,
    mermaid: HashMap<u64, MermaidTex>,

    // Left "Search": filters the list.
    list_filter: String,
    // Right "Search": in-document find (highlights matches).
    find_query: String,
    /// Currently-focused match (0-based) for the prev/next controls.
    find_idx: usize,
    /// Total matches found in the last render pass.
    find_total: usize,
    /// Scroll the focused match into view on the next render pass.
    find_scroll: bool,
    /// Pending explicit vertical scroll offset to apply to the viewer next frame
    /// (drives match/heading jumps deterministically).
    pending_offset: Option<f32>,

    show_outline: bool,
    /// Heading index to scroll to next frame (outline click).
    scroll_to_heading: Option<usize>,

    // View state.
    font_pt: f32,
    zoom: f32,
    fullscreen: bool,
    on_top: bool,
    show_source: bool,
    show_shortcuts: bool,
    focus_find: bool,
    /// One-shot guard: install the broad fallback font into the child viewport
    /// context once (so glyphs egui's default font lacks render, not as tofu).
    fonts_ready: bool,
    /// Procedural "uneven frosted glass" overlay, built once for the window.
    fog_tex: Option<egui::TextureHandle>,
}

impl Default for DocViewer {
    fn default() -> Self {
        Self {
            open: false,
            lang: Language::English,
            docs: Vec::new(),
            extra: Vec::new(),
            selected: None,
            outline: Vec::new(),
            mermaid: HashMap::new(),
            list_filter: String::new(),
            find_query: String::new(),
            find_idx: 0,
            find_total: 0,
            find_scroll: false,
            pending_offset: None,
            show_outline: false,
            scroll_to_heading: None,
            font_pt: default_font_pt(),
            zoom: 1.0,
            fullscreen: false,
            on_top: false,
            show_source: false,
            show_shortcuts: false,
            focus_find: false,
            fonts_ready: false,
            fog_tex: None,
        }
    }
}

/// Default body font size (points) for the documentation viewer.
fn default_font_pt() -> f32 {
    16.0
}

/// User preferences that survive across sessions (currently just the font size).
#[derive(serde::Serialize, serde::Deserialize)]
struct DocPrefs {
    #[serde(default = "default_font_pt")]
    font_pt: f32,
}

impl Default for DocPrefs {
    fn default() -> Self {
        Self { font_pt: default_font_pt() }
    }
}

fn prefs_path() -> std::path::PathBuf {
    crate::llm::base_dir().join("doc_viewer.toml")
}

/// Load the persisted viewer preferences, falling back to defaults on any error.
fn load_prefs() -> DocPrefs {
    std::fs::read_to_string(prefs_path())
        .ok()
        .and_then(|t| toml::from_str(&t).ok())
        .unwrap_or_default()
}

impl DocViewer {
    pub fn open(&mut self, lang: Language) {
        self.open = true;
        // Restore the last font size the user chose.
        self.font_pt = load_prefs().font_pt.clamp(8.0, 28.0);
        self.ensure_lang(lang);
    }

    /// Change the body font size and persist the choice for next time.
    fn set_font_pt(&mut self, v: f32) {
        let v = v.clamp(8.0, 28.0);
        if (v - self.font_pt).abs() > f32::EPSILON {
            self.font_pt = v;
            self.save_prefs();
        }
    }

    /// Run the search: jump to the first match in the document. Used by the Go
    /// button and the Enter key.
    fn commit_find(&mut self) {
        self.find_idx = 0;
        self.find_scroll = true;
    }

    /// Move the focused match to the next one (wrapping) and request a scroll.
    fn find_next(&mut self) {
        if self.find_total > 0 {
            self.find_idx = (self.find_idx + 1) % self.find_total;
            self.find_scroll = true;
        }
    }

    /// Move the focused match to the previous one (wrapping) and request a scroll.
    fn find_prev(&mut self) {
        if self.find_total > 0 {
            self.find_idx = (self.find_idx + self.find_total - 1) % self.find_total;
            self.find_scroll = true;
        }
    }

    /// Write the current preferences to disk (best-effort).
    fn save_prefs(&self) {
        let prefs = DocPrefs { font_pt: self.font_pt };
        if let Ok(text) = toml::to_string_pretty(&prefs) {
            let path = prefs_path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, text);
        }
    }

    /// Install a broad system font as a fallback in the doc window's context so
    /// glyphs egui's default font lacks (e.g. the U+2011 non-breaking hyphen in
    /// "RustCOBOL‑85") render properly instead of as tofu boxes. Runs once.
    fn install_doc_fonts(ctx: &Context) {
        ctx.set_fonts(crate::fonts::base_font_definitions());
    }

    /// Paint the "uneven frosted glass" overlay across the whole window on the
    /// background layer (below the panels, which are transparent). The fog uses
    /// the theme background colour with a per-pixel alpha that varies ~20% from
    /// the clearest to the foggiest area.
    fn paint_frost(&mut self, ctx: &Context, rgb: egui::Color32) {
        let tex = self
            .fog_tex
            .get_or_insert_with(|| build_fog_texture(ctx, rgb));
        let screen = ctx.screen_rect();
        let painter = ctx.layer_painter(egui::LayerId::background());
        painter.image(
            tex.id(),
            screen,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }

    fn ensure_lang(&mut self, lang: Language) {
        if self.docs.is_empty() || self.lang != lang {
            self.lang = lang;
            let prev_id = self.selected.and_then(|i| self.docs.get(i)).map(|d| d.id.clone());
            self.docs = docs_embed::doc_list(lang);
            self.docs.extend(self.extra.iter().cloned());
            self.selected = prev_id.and_then(|id| self.docs.iter().position(|d| d.id == id));
            self.rebuild_outline();
        }
    }

    fn select(&mut self, idx: usize) {
        if self.selected != Some(idx) {
            self.selected = Some(idx);
            self.find_query.clear();
            self.rebuild_outline();
        }
    }

    fn rebuild_outline(&mut self) {
        self.outline = self
            .selected
            .and_then(|i| self.docs.get(i))
            .map(|d| build_outline(&d.source))
            .unwrap_or_default();
    }

    /// Open a Markdown file from disk (`Cmd+O`) and add it to the list.
    fn open_external(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Markdown", &["md", "markdown", "txt"])
            .pick_file()
        {
            if let Ok(source) = std::fs::read_to_string(&path) {
                let id = path.to_string_lossy().to_string();
                if let Some(i) = self.docs.iter().position(|d| d.id == id) {
                    self.select(i);
                    return;
                }
                let title = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("document")
                    .to_string();
                let entry = DocEntry { id, title, source };
                self.extra.push(entry.clone());
                self.docs.push(entry);
                self.select(self.docs.len() - 1);
            }
        }
    }

    // ── Window ────────────────────────────────────────────────────────────────

    pub fn show(&mut self, parent: &Context, lang: Language, tr: &Tr) {
        if !self.open {
            return;
        }
        self.ensure_lang(lang);

        let parent_style = parent.style();
        let vp_id = ViewportId::from_hash_of("powerrustcobol_doc_viewer");
        let title = format!("PowerRustCOBOL — {}  v{VERSION}", tr.doc_win_title);

        parent.show_viewport_immediate(
            vp_id,
            ViewportBuilder::default()
                .with_title(title)
                .with_inner_size([1100.0, 760.0])
                .with_min_inner_size([640.0, 420.0])
                .with_transparent(true),
            |ctx, _class| {
                ctx.set_style((*parent_style).clone());
                if !self.fonts_ready {
                    Self::install_doc_fonts(ctx);
                    self.fonts_ready = true;
                }
                ctx.set_zoom_factor(self.zoom);

                // Translucent "frosted glass": the window is transparent and the
                // panels paint nothing, so an uneven fog overlay (built from the
                // theme background colour) is all that sits over the desktop.
                let fog_rgb = parent_style.visuals.panel_fill;
                self.paint_frost(ctx, fog_rgb);
                {
                    let mut s = (*ctx.style()).clone();
                    s.visuals.panel_fill = egui::Color32::TRANSPARENT;
                    s.visuals.window_fill = fog_rgb.gamma_multiply(0.92);
                    s.visuals.extreme_bg_color =
                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 70);
                    ctx.set_style(s);
                }

                if ctx.input(|i| i.viewport().close_requested()) {
                    self.open = false;
                }
                self.handle_shortcuts(ctx);
                self.menu_bar(ctx, tr);
                self.toolbar(ctx, tr);
                self.left_pane(ctx, tr);
                if self.show_outline {
                    self.outline_pane(ctx);
                }
                self.viewer_pane(ctx, tr);
                self.modals(ctx, tr);
            },
        );
    }

    fn handle_shortcuts(&mut self, ctx: &Context) {
        let (cmd, alt, f, o, w, t, u, plus, minus, prev, next) = ctx.input(|i| {
            let m = i.modifiers;
            (
                m.command,
                m.alt,
                i.key_pressed(Key::F),
                i.key_pressed(Key::O),
                i.key_pressed(Key::W),
                i.key_pressed(Key::T),
                i.key_pressed(Key::U),
                i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals),
                i.key_pressed(Key::Minus),
                i.key_pressed(Key::Comma),
                i.key_pressed(Key::Period),
            )
        });
        // `,` / `.` jump between matches — but only when not typing in a field,
        // so they don't swallow punctuation in the search box.
        let typing = ctx.memory(|m| m.focused().is_some());
        if !typing && prev {
            self.find_prev();
        }
        if !typing && next {
            self.find_next();
        }
        if cmd && f {
            self.focus_find = true;
        }
        if cmd && o {
            self.open_external();
        }
        if cmd && w {
            self.open = false;
        }
        if cmd && t {
            self.on_top = !self.on_top;
            ctx.send_viewport_cmd(ViewportCommand::WindowLevel(if self.on_top {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            }));
        }
        if cmd && alt && u {
            self.show_source = !self.show_source;
        }
        if cmd && plus {
            self.set_font_pt(self.font_pt + 1.0);
        }
        if cmd && minus {
            self.set_font_pt(self.font_pt - 1.0);
        }
    }

    fn menu_bar(&mut self, ctx: &Context, tr: &Tr) {
        egui::TopBottomPanel::top("doc_menubar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button(tr.doc_menu_file, |ui| {
                    if ui.button(tr.doc_print).clicked() {
                        self.print();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(tr.doc_close).clicked() {
                        self.open = false;
                        ui.close_menu();
                    }
                });
                ui.menu_button(tr.doc_menu_view, |ui| {
                    if ui.button(tr.doc_zoom_in).clicked() {
                        self.zoom = (self.zoom * 1.1).min(3.0);
                        ui.close_menu();
                    }
                    if ui.button(tr.doc_zoom_out).clicked() {
                        self.zoom = (self.zoom / 1.1).max(0.5);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.checkbox(&mut self.fullscreen, tr.doc_fullscreen).changed() {
                        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self.fullscreen));
                    }
                    ui.checkbox(&mut self.show_outline, tr.doc_miniatures);
                });
                ui.menu_button(tr.doc_menu_help, |ui| {
                    if ui.button(tr.doc_shortcuts).clicked() {
                        self.show_shortcuts = true;
                        ui.close_menu();
                    }
                });
            });
        });
    }

    /// Icon toolbar mirroring the keyboard shortcuts (open / view-source / on-top
    /// / print / close). Icons are drawn as vectors so they are theme-aware and
    /// need no image assets.
    fn toolbar(&mut self, ctx: &Context, tr: &Tr) {
        egui::TopBottomPanel::top("doc_toolbar").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                if icon_button(ui, Icon::Open, false, &format!("{}  (⌘O)", tr.doc_open_file)) {
                    self.open_external();
                }
                if icon_button(
                    ui,
                    Icon::Source,
                    self.show_source,
                    &format!("{}  (⌥⌘U)", tr.doc_view_source),
                ) {
                    self.show_source = !self.show_source;
                }
                if icon_button(ui, Icon::Pin, self.on_top, &format!("{}  (⌘T)", tr.doc_on_top)) {
                    self.on_top = !self.on_top;
                    ctx.send_viewport_cmd(ViewportCommand::WindowLevel(if self.on_top {
                        egui::WindowLevel::AlwaysOnTop
                    } else {
                        egui::WindowLevel::Normal
                    }));
                }
                ui.separator();
                if icon_button(ui, Icon::Print, false, &format!("{}  (⌘P)", tr.doc_print)) {
                    self.print();
                }
                ui.separator();
                if icon_button(ui, Icon::Close, false, &format!("{}  (⌘W)", tr.doc_close)) {
                    self.open = false;
                }
            });
            ui.add_space(2.0);
        });
    }

    fn left_pane(&mut self, ctx: &Context, tr: &Tr) {
        egui::SidePanel::left("doc_list_panel")
            .resizable(true)
            .default_width(260.0)
            .min_width(170.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr.doc_search);
                    ui.text_edit_singleline(&mut self.list_filter);
                });
                ui.separator();
                let filter = self.list_filter.trim().to_lowercase();
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let mut to_select = None;
                    for (i, doc) in self.docs.iter().enumerate() {
                        if !filter.is_empty() && !doc.title.to_lowercase().contains(&filter) {
                            continue;
                        }
                        if ui.selectable_label(self.selected == Some(i), &doc.title).clicked() {
                            to_select = Some(i);
                        }
                    }
                    if let Some(i) = to_select {
                        self.select(i);
                    }
                });
            });
    }

    fn outline_pane(&mut self, ctx: &Context) {
        egui::SidePanel::right("doc_outline_panel")
            .resizable(true)
            .default_width(220.0)
            .min_width(150.0)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("☰").size(self.font_pt + 2.0));
                ui.separator();
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let mut jump = None;
                    for (idx, (level, title, _off)) in self.outline.iter().enumerate() {
                        let indent = (level.saturating_sub(1)) as f32 * 12.0;
                        ui.horizontal(|ui| {
                            ui.add_space(indent);
                            if ui.link(title).clicked() {
                                jump = Some(idx);
                            }
                        });
                    }
                    if let Some(idx) = jump {
                        self.scroll_to_heading = Some(idx);
                    }
                });
            });
    }

    fn viewer_pane(&mut self, ctx: &Context, tr: &Tr) {
        // Search bar + nav + font-size control (right-aligned).
        egui::TopBottomPanel::top("doc_viewer_search").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(tr.doc_search);
                let resp = ui.text_edit_singleline(&mut self.find_query);
                if self.focus_find {
                    resp.request_focus();
                    self.focus_find = false;
                }
                if resp.changed() {
                    // New query: restart at the first match (commit on Go/Enter).
                    self.find_idx = 0;
                }
                let entered =
                    resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                // Go runs the search (jump to the current match); Enter does too.
                if ui.button(tr.doc_find_go).clicked() || entered {
                    self.commit_find();
                }

                // Previous / next match controls + counter.
                let has = self.find_total > 0;
                if ui.add_enabled(has, egui::Button::new("◀").small()).clicked() {
                    self.find_prev();
                }
                if ui.add_enabled(has, egui::Button::new("▶").small()).clicked() {
                    self.find_next();
                }
                let counter = if has {
                    format!("{}/{}", self.find_idx + 1, self.find_total)
                } else if self.find_query.trim().is_empty() {
                    String::new()
                } else {
                    "0/0".to_string()
                };
                if !counter.is_empty() {
                    ui.label(counter);
                }

                // Font-size control, right-aligned.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("A+").clicked() {
                        self.set_font_pt(self.font_pt + 1.0);
                    }
                    if ui.small_button("A−").clicked() {
                        self.set_font_pt(self.font_pt - 1.0);
                    }
                    ui.label(format!("{}px", self.font_pt as i32));
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.selected.is_none() {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.35);
                    ui.label(egui::RichText::new(tr.doc_placeholder).size(16.0).weak());
                });
                return;
            }

            let source = self.docs[self.selected.unwrap()].source.clone();
            let search = self.find_query.trim().to_lowercase();
            let base = self.font_pt;
            let scroll_to = self.scroll_to_heading.take();
            let active = (!search.is_empty()).then_some(self.find_idx);
            let scroll_active = self.find_scroll;
            self.find_scroll = false;
            let mtr = tr.doc_mermaid_error;
            // GitHub-style anchors so in-document ToC links can jump to sections.
            let anchors: Vec<(String, usize)> = self
                .outline
                .iter()
                .enumerate()
                .map(|(i, (_, title, _))| (slugify(title), i))
                .collect();

            let want_heading_jump = scroll_to.is_some();
            let mut sa = egui::ScrollArea::vertical().auto_shrink([false, false]);
            if let Some(off) = self.pending_offset.take() {
                sa = sa.vertical_scroll_offset(off);
            }
            let sa_out = sa.show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                let mermaid = &mut self.mermaid;
                let opts = RenderOpts {
                    search: &search,
                    base,
                    scroll_to_heading: scroll_to,
                    active_match: active,
                    scroll_to_active: scroll_active,
                    anchors: &anchors,
                };
                md_render::render(ui, &source, &opts, &mut |ui, code| {
                    draw_mermaid(mermaid, ui, code, mtr);
                })
            });
            let out = sa_out.inner;

            // Convert the target block's screen-Y into a scroll offset and apply
            // it next frame (deterministic; `scroll_to_me` is unreliable here).
            if scroll_active || want_heading_jump {
                if let Some(ty) = out.scroll_target_y {
                    let vp = sa_out.inner_rect;
                    let cur = sa_out.state.offset.y;
                    let target_content_y = ty - vp.top() + cur;
                    let desired = if want_heading_jump {
                        (target_content_y - 8.0).max(0.0)
                    } else {
                        (target_content_y - vp.height() * 0.5).max(0.0)
                    };
                    self.pending_offset = Some(desired);
                    ctx.request_repaint();
                }
            }

            // A clicked ToC link scrolls to its heading on the next frame.
            if let Some(idx) = out.clicked_heading {
                self.scroll_to_heading = Some(idx);
                ctx.request_repaint();
            }

            // Record the match count so the prev/next controls know the range.
            self.find_total = out.match_count;
            if self.find_total == 0 {
                self.find_idx = 0;
            } else if self.find_idx >= self.find_total {
                self.find_idx = self.find_total - 1;
            }
        });
    }

    fn modals(&mut self, ctx: &Context, tr: &Tr) {
        if self.show_source {
            let src = self
                .selected
                .and_then(|i| self.docs.get(i))
                .map(|d| d.source.clone())
                .unwrap_or_default();
            let mut open = self.show_source;
            egui::Window::new(format!("{} — Markdown", tr.doc_win_title))
                .open(&mut open)
                .default_size([720.0, 560.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::both().show(ui, |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(&src).monospace()).wrap());
                    });
                });
            self.show_source = open;
        }

        if self.show_shortcuts {
            let mut open = self.show_shortcuts;
            egui::Window::new(tr.doc_shortcuts)
                .open(&mut open)
                .default_size([460.0, 320.0])
                .show(ctx, |ui| {
                    let rows = [
                        ("⌘F", tr.doc_search),
                        ("⌘O", tr.doc_open_file),
                        ("⌥⌘U", tr.doc_view_source),
                        ("⌘W", tr.doc_close),
                        ("⌘T", tr.doc_on_top),
                        ("⌘P", tr.doc_print),
                        ("⌘+ / ⌘-", tr.doc_font_size),
                    ];
                    egui::Grid::new("doc_shortcuts_grid").striped(true).show(ui, |ui| {
                        for (k, d) in rows {
                            ui.strong(k);
                            ui.label(d);
                            ui.end_row();
                        }
                    });
                });
            self.show_shortcuts = open;
        }
    }

    /// Render the current document to a PDF and open it in the OS viewer.
    fn print(&mut self) {
        let Some(doc) = self.selected.and_then(|i| self.docs.get(i)) else {
            return;
        };
        let out = std::env::temp_dir().join(format!("{}.pdf", sanitize_filename(&doc.title)));
        if crate::pdf_export::export(&doc.title, &doc.source, &out).is_ok() {
            open_in_os(&out);
        }
    }
}

/// Draw one Mermaid diagram into `ui`, rendering+caching it on first use.
fn draw_mermaid(
    cache: &mut HashMap<u64, MermaidTex>,
    ui: &mut egui::Ui,
    code: &str,
    err_label: &str,
) {
    let key = fnv1a(code);
    let entry = cache.entry(key).or_insert_with(|| render_mermaid(ui.ctx(), code));
    match entry {
        MermaidTex::Ok { tex, size } => {
            let avail = ui.available_width().max(1.0);
            let w = size.x.min(avail);
            let h = if size.x > 0.0 { w * size.y / size.x } else { size.y };
            ui.add_space(6.0);
            ui.add(egui::Image::new((tex.id(), egui::vec2(w, h))));
            ui.add_space(6.0);
        }
        MermaidTex::Err(e) => {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(format!("⚠ {err_label}: {e}")).weak());
            ui.add(egui::Label::new(egui::RichText::new(code).monospace()).wrap());
            ui.add_space(4.0);
        }
    }
}

/// Headings of a document for the outline: `(level, title, char offset)`.
/// GitHub-style heading slug: lower-cased, punctuation dropped, spaces and
/// hyphens become `-` (so `## 14. Indexed files — a resource` →
/// `14-indexed-files--a-resource`, matching the `[…](#…)` links in the docs).
fn slugify(title: &str) -> String {
    let mut out = String::new();
    for c in title.trim().chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if c == ' ' || c == '-' {
            out.push('-');
        } else if c == '_' {
            out.push('_');
        }
        // all other characters (.,:—'…) are dropped, like GitHub
    }
    out
}

fn build_outline(md: &str) -> Vec<(u8, String, usize)> {
    let mut out = Vec::new();
    let mut char_off = 0usize;
    let mut in_fence = false;
    for line in md.lines() {
        let t = line.trim_start();
        if t.starts_with("```") {
            in_fence = !in_fence;
        } else if !in_fence && t.starts_with('#') {
            let level = t.chars().take_while(|&c| c == '#').count();
            if (1..=6).contains(&level) {
                let title = t[level..].trim().to_string();
                if !title.is_empty() {
                    out.push((level as u8, title, char_off));
                }
            }
        }
        char_off += line.chars().count() + 1;
    }
    out
}

// ── Mermaid → texture ────────────────────────────────────────────────────────────

fn mermaid_fontdb() -> Arc<resvg::usvg::fontdb::Database> {
    static DB: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    })
    .clone()
}

fn render_mermaid(ctx: &Context, code: &str) -> MermaidTex {
    match render_mermaid_image(code) {
        Ok((image, logical)) => {
            let tex = ctx.load_texture("mermaid_diagram", image, egui::TextureOptions::LINEAR);
            MermaidTex::Ok { tex, size: logical }
        }
        Err(e) => MermaidTex::Err(e),
    }
}

fn render_mermaid_image(code: &str) -> Result<(egui::ColorImage, egui::Vec2), String> {
    let pixmap = render_mermaid_pixmap(code, 2.0)?;
    let (w, h) = (pixmap.width(), pixmap.height());
    let pixels: Vec<egui::Color32> = pixmap
        .pixels()
        .iter()
        .map(|p| egui::Color32::from_rgba_premultiplied(p.red(), p.green(), p.blue(), p.alpha()))
        .collect();
    let image = egui::ColorImage { size: [w as usize, h as usize], pixels };
    let logical = egui::vec2(w as f32 / 2.0, h as f32 / 2.0);
    Ok((image, logical))
}

/// Render a Mermaid diagram to a `tiny_skia` pixmap at `scale`. Shared by the
/// on-screen viewer and the PDF export.
pub(crate) fn render_mermaid_pixmap(
    code: &str,
    scale: f32,
) -> Result<resvg::tiny_skia::Pixmap, String> {
    let svg = mermaid_rs_renderer::render(code).map_err(|e| e.to_string())?;
    let opt = resvg::usvg::Options { fontdb: mermaid_fontdb(), ..Default::default() };
    let tree = resvg::usvg::Tree::from_str(&svg, &opt).map_err(|e| e.to_string())?;
    let isize = tree.size().to_int_size();
    let w = ((isize.width() as f32) * scale).ceil().max(1.0) as u32;
    let h = ((isize.height() as f32) * scale).ceil().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h).ok_or("pixmap allocation failed")?;
    pixmap.fill(resvg::tiny_skia::Color::WHITE);
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap)
}

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn sanitize_filename(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() { "document".into() } else { s }
}

fn open_in_os(path: &std::path::Path) {
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "windows")]
    let cmd = "explorer";
    #[cfg(all(unix, not(target_os = "macos")))]
    let cmd = "xdg-open";
    let _ = std::process::Command::new(cmd).arg(path).spawn();
}

// ── Vector toolbar icons ────────────────────────────────────────────────────

/// The toolbar icons, drawn procedurally (no image assets, theme-aware).
#[derive(Clone, Copy)]
enum Icon {
    Open,
    Source,
    Pin,
    Print,
    Close,
}

/// A 30×26 toolbar button with a vector icon and a hover tooltip. Returns whether
/// it was clicked. `selected` draws the pressed/active background (used for the
/// keep-on-top and view-source toggles).
fn icon_button(ui: &mut egui::Ui, icon: Icon, selected: bool, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(30.0, 26.0), egui::Sense::click());
    let v = ui.style().interact_selectable(&resp, selected);
    if selected || resp.hovered() {
        ui.painter()
            .rect(rect.shrink(1.0), egui::Rounding::same(4.0), v.bg_fill, egui::Stroke::NONE);
    }
    paint_icon(ui.painter(), rect, v.fg_stroke.color, icon);
    resp.on_hover_text(tip).clicked()
}

/// Draw one icon as vector strokes inside `rect`.
fn paint_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32, icon: Icon) {
    use egui::{pos2, Shape, Stroke};
    let r = rect.shrink(7.0);
    let (l, t, w, h) = (r.left(), r.top(), r.width(), r.height());
    let (cx, cy) = (r.center().x, r.center().y);
    let s = Stroke::new(1.6, color);
    match icon {
        Icon::Open => {
            // A folder with a tab.
            let pts = vec![
                pos2(l, t + h * 0.95),
                pos2(l, t + h * 0.30),
                pos2(l + w * 0.10, t + h * 0.30),
                pos2(l + w * 0.16, t + h * 0.12),
                pos2(l + w * 0.46, t + h * 0.12),
                pos2(l + w * 0.52, t + h * 0.30),
                pos2(l + w, t + h * 0.30),
                pos2(l + w, t + h * 0.95),
            ];
            painter.add(Shape::closed_line(pts, s));
        }
        Icon::Source => {
            // "</>": two chevrons and a slash.
            painter.add(Shape::line(
                vec![
                    pos2(l + w * 0.34, t + h * 0.18),
                    pos2(l + w * 0.10, cy),
                    pos2(l + w * 0.34, t + h * 0.82),
                ],
                s,
            ));
            painter.add(Shape::line(
                vec![
                    pos2(l + w * 0.66, t + h * 0.18),
                    pos2(l + w * 0.90, cy),
                    pos2(l + w * 0.66, t + h * 0.82),
                ],
                s,
            ));
            painter.line_segment(
                [pos2(cx + w * 0.07, t + h * 0.14), pos2(cx - w * 0.07, t + h * 0.86)],
                s,
            );
        }
        Icon::Pin => {
            // A thumbtack: round head, cap line, and needle.
            painter.circle_stroke(pos2(cx, t + h * 0.30), w * 0.22, s);
            painter.line_segment(
                [pos2(cx - w * 0.30, t + h * 0.52), pos2(cx + w * 0.30, t + h * 0.52)],
                s,
            );
            painter.line_segment([pos2(cx, t + h * 0.52), pos2(cx, t + h * 0.95)], s);
        }
        Icon::Print => {
            // Paper out the top, printer body, output sheet at the bottom.
            painter.add(Shape::closed_line(
                vec![
                    pos2(l + w * 0.20, t + h * 0.32),
                    pos2(l + w * 0.20, t),
                    pos2(l + w * 0.80, t),
                    pos2(l + w * 0.80, t + h * 0.32),
                ],
                s,
            ));
            painter.rect_stroke(
                egui::Rect::from_min_max(pos2(l, t + h * 0.30), pos2(l + w, t + h * 0.74)),
                egui::Rounding::same(2.0),
                s,
            );
            painter.add(Shape::closed_line(
                vec![
                    pos2(l + w * 0.22, t + h * 0.60),
                    pos2(l + w * 0.78, t + h * 0.60),
                    pos2(l + w * 0.78, t + h),
                    pos2(l + w * 0.22, t + h),
                ],
                s,
            ));
        }
        Icon::Close => {
            let q = r.shrink2(egui::vec2(w * 0.12, h * 0.08));
            painter.line_segment([q.left_top(), q.right_bottom()], s);
            painter.line_segment([q.right_top(), q.left_bottom()], s);
        }
    }
}

// ── Frosted-glass fog overlay ───────────────────────────────────────────────

/// Build the procedural fog texture: the theme background colour with a
/// per-pixel alpha driven by two octaves of value noise, so the window reads as
/// uneven frosted glass (some areas clearer, some foggier — about a 20% spread).
fn build_fog_texture(ctx: &Context, rgb: egui::Color32) -> egui::TextureHandle {
    const W: usize = 160;
    const H: usize = 110;
    // High mean opacity so the desktop is only barely visible, with an uneven
    // ~20% swing between the clearest and foggiest patches.
    let base = 0.89_f32;
    let spread = 0.09_f32; // ±9% → ~18-20% range
    let mut pixels = Vec::with_capacity(W * H);
    for y in 0..H {
        for x in 0..W {
            let (fx, fy) = (x as f32, y as f32);
            let n = 0.65 * vnoise(fx / 38.0, fy / 30.0)
                + 0.35 * vnoise(fx / 14.0 + 11.3, fy / 12.0 + 7.1);
            let a = ((base + (n - 0.5) * 2.0 * spread).clamp(0.80, 0.98) * 255.0) as u8;
            pixels.push(egui::Color32::from_rgba_unmultiplied(
                rgb.r(),
                rgb.g(),
                rgb.b(),
                a,
            ));
        }
    }
    let img = egui::ColorImage { size: [W, H], pixels };
    ctx.load_texture("doc_frost", img, egui::TextureOptions::LINEAR)
}

/// Deterministic 0..1 hash for integer lattice points.
fn hash01(x: i32, y: i32) -> f32 {
    let mut h = (x.wrapping_mul(374_761_393).wrapping_add(y.wrapping_mul(668_265_263))) as u32;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Bilinearly-interpolated value noise in 0..1.
fn vnoise(x: f32, y: f32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (tx, ty) = (smoothstep(x - x0), smoothstep(y - y0));
    let (ix, iy) = (x0 as i32, y0 as i32);
    let v00 = hash01(ix, iy);
    let v10 = hash01(ix + 1, iy);
    let v01 = hash01(ix, iy + 1);
    let v11 = hash01(ix + 1, iy + 1);
    let a = v00 + (v10 - v00) * tx;
    let b = v01 + (v11 - v01) * tx;
    a + (b - a) * ty
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mermaid_renders_to_a_non_empty_image() {
        let (img, size) =
            render_mermaid_image("flowchart LR\n    A-->B-->C").expect("mermaid render");
        assert!(img.size[0] > 0 && img.size[1] > 0);
        assert!(size.x > 0.0 && size.y > 0.0);
        assert!(img.pixels.iter().any(|p| p.a() > 0));
    }

    #[test]
    fn slugify_matches_github_anchor_style() {
        assert_eq!(
            slugify("1. What PowerRustCOBOL is, and why it exists"),
            "1-what-powerrustcobol-is-and-why-it-exists"
        );
        // Em dash drops out, leaving a double hyphen — like the doc's ToC links.
        assert_eq!(
            slugify("14. Indexed files — a first-class resource"),
            "14-indexed-files--a-first-class-resource"
        );
    }

    #[test]
    fn outline_collects_headings_and_skips_fences() {
        let md = "# A\n\n## B\n\n```\n# not a heading\n```\n\n### C\n";
        let o = build_outline(md);
        let titles: Vec<&str> = o.iter().map(|(_, t, _)| t.as_str()).collect();
        assert_eq!(titles, vec!["A", "B", "C"]);
    }

    #[test]
    fn doc_list_is_non_empty_and_has_the_guide() {
        let list = docs_embed::doc_list(Language::English);
        assert!(!list.is_empty());
        assert!(list.iter().any(|d| d.id.starts_with("developers-guide")));
    }
}
