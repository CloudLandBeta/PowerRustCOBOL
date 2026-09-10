// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Documentation viewer — a separate window (egui viewport) that renders the
//! embedded PowerRustCOBOL documentation (Markdown, Mermaid diagrams and
//! screenshots) with a custom theme-aware renderer
//! ([`crate::panels::md_render`]).
//!
//! Each document is a separate entry in the left-hand list; selecting one loads
//! it, and `Cmd+O` adds an external `.md` file to the list. The window is theme-
//! and I18N-aware and offers File/View/Help menus, zoom, a font-size control, an
//! outline (table of contents), in-document search with highlighting, a
//! view-source modal, keyboard shortcuts, and PDF print.
//!
//! # Keeping the window responsive
//!
//! Three things used to happen on the UI thread, and the Developer's Guide is
//! big enough — 499 KB, 1 900 blocks, a dozen screenshots, ten diagrams — that
//! all three showed:
//!
//! 1. **Diagrams were rendered when first scrolled to**, stalling the frame
//!    that reached them; the guide's ten cost **189 ms** between them, and the
//!    first also paid for loading the system font database. Now a background
//!    thread ([`prepare_thread`]) decodes *everything the document embeds* the
//!    moment the document is selected — for the guide, 189 ms of diagrams and
//!    163 ms of screenshots, none of it on the UI thread any more.
//! 2. **Every block was laid out on every frame** — 6.9 ms for the guide, on
//!    top of whatever the IDE's own window costs, since an immediate viewport
//!    shares its frame. Now [`md_render::BlockCache`] remembers each block's
//!    height and the renderer draws only what is near the viewport, plus
//!    [`READ_AHEAD`] points either side: **2.0 ms**, and 38 blocks of 1 900.
//! 3. **The whole source string was copied every frame.** It is an [`Arc`] now.
//!
//! The window itself still draws on the application's single egui context —
//! `show_viewport_immediate` shares it — so "its own thread" is where the *work*
//! lives, which is what the frame rate actually depends on.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, OnceLock};

use egui::{Context, Key, ViewportBuilder, ViewportCommand, ViewportId};

use crate::docs_embed::{self, DocEntry};
use crate::i18n::{Language, Tr};
use crate::panels::md_render::{self, ImageRef, RenderOpts};
use crate::version::VERSION;

/// How fast a thrown document has to be still moving to keep gliding, in
/// points per second. Below this it stops rather than creeping.
const THROW_STOP_SPEED: f32 = 20.0;
/// How hard a thrown document is slowed, in points per second squared.
const THROW_FRICTION: f32 = 1000.0;
/// How much of the recent gesture a throw's speed is measured over, in seconds.
/// Long enough to be steady, short enough that a drag which stopped before the
/// release throws nothing — letting go of a document you have stopped moving
/// should leave it where it is.
const THROW_SAMPLE: f64 = 0.12;

/// A grab-and-throw in flight: where the pointer has been, and when.
///
/// The gesture is tracked here rather than left to egui's own drag-to-scroll
/// because egui measures the throw's speed from `pointer.velocity()` at the
/// moment it sees the button released — and that history belongs to the
/// pointer, not to the drag. Release the button anywhere but over the document
/// and the speed read back is nothing, so the page stopped dead instead of
/// gliding (operator, 2026-09-10: the release "should occur anywhere in the
/// screen, not only over the document").
///
/// Ours is measured from the samples taken while the button was down, which is
/// the gesture the developer actually made, and it does not care where the
/// button came up.
#[derive(Default)]
struct Throw {
    /// `(time, pointer y)` for the last [`THROW_SAMPLE`] seconds of the drag.
    samples: Vec<(f64, f32)>,
    /// Where the pointer was last frame, for this frame's drag delta.
    last_y: f32,
}

/// Points per second a held scroll key moves the document, before acceleration.
const BASE_SPEED: f32 = 320.0;
/// The ceiling on the acceleration ramp: four times the starting pace.
const MAX_FACTOR: f32 = 4.0;
/// Seconds of holding it takes to reach [`MAX_FACTOR`].
const ACCEL_TIME: f32 = 2.0;
/// A held key scrolls continuously only after this long, so that a tap stays a
/// tap — one line — rather than launching a glide.
const REPEAT_DELAY: f32 = 0.25;

/// How fast the page was moving when it was let go, in points per second.
///
/// Measured across the samples the drag left behind — oldest to newest — so a
/// hand that slowed to a stop before releasing throws nothing, and one still
/// moving throws at the speed it was moving. Positive is a throw UP the
/// document (the pointer moving up), which scrolls forward.
fn throw_speed(samples: &[(f64, f32)]) -> f32 {
    let (Some((t0, y0)), Some((t1, y1))) = (samples.first(), samples.last()) else {
        return 0.0;
    };
    let dt = (t1 - t0) as f32;
    if dt <= 0.0 {
        return 0.0;
    }
    (y1 - y0) / dt
}

/// Speed multiplier for a scroll key held `held` seconds: `None` while it is
/// still a tap, then `1.0` rising to [`MAX_FACTOR`] over [`ACCEL_TIME`] and
/// never past it.
///
/// It starts at exactly 1 — the reader's normal pace — because a key that
/// begins fast cannot be used to move one paragraph.
fn accel_factor(held: f32) -> Option<f32> {
    if held <= REPEAT_DELAY {
        return None;
    }
    let ramp = (held - REPEAT_DELAY) / ACCEL_TIME;
    Some((1.0 + ramp * (MAX_FACTOR - 1.0)).min(MAX_FACTOR))
}

/// How far beyond the viewport the renderer keeps drawing for real, in points.
/// Roughly two screens either way: far enough that a fast drag never overtakes
/// the drawn band, near enough that a long document still costs little.
const READ_AHEAD: f32 = 1600.0;

/// Widest a decoded screenshot is kept, in pixels. The guide displays them at
/// 900 points, so this is still sharp on a 2× display while keeping a dozen
/// images from turning into hundreds of megabytes of texture.
const MAX_IMAGE_WIDTH: u32 = 1600;

/// A decoded document asset — a Mermaid diagram or an embedded image — or the
/// error that prevented it.
enum Asset {
    /// Being decoded on the preparer thread.
    Pending,
    Ok {
        tex: egui::TextureHandle,
        /// Logical (point) size, which for a diagram is half the pixel size
        /// because it is rendered at 2×.
        size: egui::Vec2,
    },
    Err(String),
}

/// One asset to decode, as the preparer sees it.
enum AssetJob {
    /// A ```mermaid fence, by its code.
    Mermaid(String),
    /// An embedded image, already resolved to a file on disk.
    Image(PathBuf),
}

/// Pixels ready for the UI thread to upload as a texture.
enum Decoded {
    Ok {
        image: egui::ColorImage,
        /// Logical size in points (half the pixel size for a 2× diagram).
        size: egui::Vec2,
    },
    Err(String),
}

/// The preparer thread and its two channels.
struct Prep {
    jobs: Sender<Vec<(u64, AssetJob)>>,
    done: Receiver<(u64, Decoded)>,
}

/// Decode every asset of a document, newest request first, off the UI thread.
///
/// Each batch is one document's worth of work in reading order, so the top of
/// the document is ready first. A batch is abandoned as soon as a newer one
/// arrives — the reader has moved on, and finishing the old one only delays
/// what they are actually looking at.
fn prepare_thread(
    ctx: Context,
    jobs: Receiver<Vec<(u64, AssetJob)>>,
    done: Sender<(u64, Decoded)>,
) {
    while let Ok(batch) = jobs.recv() {
        // Skip straight to the newest queued document.
        let mut batch = batch;
        while let Ok(newer) = jobs.try_recv() {
            batch = newer;
        }
        for (key, job) in batch {
            let decoded = match job {
                AssetJob::Mermaid(code) => match render_mermaid_image(&code) {
                    Ok((image, size)) => Decoded::Ok { image, size },
                    Err(e) => Decoded::Err(e),
                },
                AssetJob::Image(path) => decode_image_file(&path),
            };
            if done.send((key, decoded)).is_err() {
                return; // the viewer is gone
            }
            ctx.request_repaint();
            // A newer document is waiting: drop this one and take it.
            if !matches!(jobs.try_recv(), Err(TryRecvError::Empty)) {
                break;
            }
        }
    }
}

/// Read and decode one image file, shrinking anything wider than
/// [`MAX_IMAGE_WIDTH`] so a document full of screenshots stays affordable.
fn decode_image_file(path: &Path) -> Decoded {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return Decoded::Err(format!("{}: {e}", path.display())),
    };
    let img = match image::load_from_memory(&bytes) {
        Ok(i) => i,
        Err(e) => return Decoded::Err(e.to_string()),
    };
    let mut img = img.into_rgba8();
    if img.width() > MAX_IMAGE_WIDTH {
        let h = (img.height() as f32 * MAX_IMAGE_WIDTH as f32 / img.width() as f32).round();
        img = image::imageops::resize(
            &img,
            MAX_IMAGE_WIDTH,
            (h as u32).max(1),
            image::imageops::FilterType::CatmullRom,
        );
    }
    let (w, h) = (img.width() as usize, img.height() as usize);
    let pixels: Vec<egui::Color32> = img
        .pixels()
        .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    Decoded::Ok {
        image: egui::ColorImage {
            size: [w, h],
            source_size: egui::vec2(w as f32, h as f32),
            pixels,
        },
        // Screenshots are laid out from the document's own `width=`, so the
        // logical size here is only the aspect ratio's source.
        size: egui::vec2(w as f32, h as f32),
    }
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
    /// The selected document's source, shared with the preparer rather than
    /// copied on every frame.
    current: Option<Arc<String>>,
    /// Decoded diagrams and images, keyed by content.
    assets: HashMap<u64, Asset>,
    /// Keys already handed to the preparer, so nothing is queued twice.
    queued: HashSet<u64>,
    /// The preparer thread, started on first use.
    prep: Option<Prep>,
    /// Work queued before the thread existed — the first document is usually
    /// selected before there is an egui context to hand it.
    pending_batch: Option<Vec<(u64, AssetJob)>>,
    /// Remembered block heights, so only what is near the viewport is drawn.
    blocks: md_render::BlockCache,

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
    /// Where the document is scrolled to, mirrored from the scroll area so the
    /// keyboard can move it.
    scroll_y: f32,
    /// The largest offset the document can be scrolled to (End, PageDown).
    scroll_max: f32,
    /// Height of the scrolled viewport, for page-sized jumps.
    page_h: f32,
    /// How long a scroll key has been held, which is what the acceleration
    /// ramp is a function of.
    key_hold: f32,
    /// The grab-and-throw in flight, if the developer has hold of the page.
    throw: Option<Throw>,
    /// A thrown page's remaining speed, in points per second. Zero when the
    /// page is at rest.
    glide: f32,
    /// Where the document was on screen last frame — what a grab has to start
    /// inside of. The release does not have to be anywhere near it.
    doc_rect: egui::Rect,

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
            current: None,
            assets: HashMap::new(),
            queued: HashSet::new(),
            prep: None,
            pending_batch: None,
            blocks: md_render::BlockCache::new(READ_AHEAD),
            list_filter: String::new(),
            find_query: String::new(),
            find_idx: 0,
            find_total: 0,
            find_scroll: false,
            pending_offset: None,
            scroll_y: 0.0,
            scroll_max: 0.0,
            page_h: 0.0,
            key_hold: 0.0,
            throw: None,
            glide: 0.0,
            doc_rect: egui::Rect::NOTHING,
            show_outline: false,
            scroll_to_heading: None,
            font_pt: default_font_pt(),
            zoom: 1.0,
            fullscreen: false,
            on_top: false,
            show_source: false,
            show_shortcuts: false,
            focus_find: false,
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
        Self {
            font_pt: default_font_pt(),
        }
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
        let prefs = DocPrefs {
            font_pt: self.font_pt,
        };
        if let Ok(text) = toml::to_string_pretty(&prefs) {
            let path = prefs_path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, text);
        }
    }

    // The broad-Latin/CJK fallback this window needs (the U+2011 non-breaking
    // hyphen in "RustCOBOL‑85", 日本語, 中文) is installed ONCE, at start-up, by
    // `CoboltApp::new` — on the very context this window draws on, because
    // `show_viewport_immediate` shares the application's single egui Context.
    //
    // Re-installing it here was therefore a no-op with a bite: `set_fonts`
    // REPLACES a context's definitions, so it also dropped every font family
    // `cobolt_forms::fonts` had loaded on demand for the open forms. The next
    // repaint of a designer holding a control whose Font is not a built-in —
    // "Arial Black" on the reported form — asked epaint for a family it no
    // longer had, and epaint panics on that rather than falling back:
    // `FontFamily::Name("Arial Black") is not bound to any fonts`. Opening the
    // documentation crashed the IDE (operator, 2026-08-20).
    //
    // `cobolt_forms::fonts::font_id` now also re-registers a family the context
    // has lost, so no single `set_fonts` anywhere can bring the panic back. Both
    // halves matter: this one is why nothing is dropped, that one is why a drop
    // would no longer be fatal.

    /// Paint the "uneven frosted glass" overlay across the whole window on the
    /// background layer (below the panels, which are transparent). The fog uses
    /// the theme background colour with a per-pixel alpha that varies ~20% from
    /// the clearest to the foggiest area.
    fn paint_frost(&mut self, ctx: &Context, rgb: egui::Color32) {
        let tex = self
            .fog_tex
            .get_or_insert_with(|| build_fog_texture(ctx, rgb));
        let screen = ctx.content_rect();
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
            let prev_id = self
                .selected
                .and_then(|i| self.docs.get(i))
                .map(|d| d.id.clone());
            self.docs = docs_embed::doc_list(lang);
            self.docs.extend(self.extra.iter().cloned());
            self.selected = prev_id.and_then(|id| self.docs.iter().position(|d| d.id == id));
            self.rebuild_outline();
            self.adopt_selected();
        }
    }

    fn select(&mut self, idx: usize) {
        if self.selected != Some(idx) {
            self.selected = Some(idx);
            self.find_query.clear();
            self.rebuild_outline();
            self.pending_offset = Some(0.0);
            self.scroll_y = 0.0;
            self.adopt_selected();
        }
    }

    /// Take up the selected document: hold its source without copying it, and
    /// put everything it embeds in front of the preparer thread straight away,
    /// so the diagrams and screenshots are decoded long before the reader
    /// scrolls to them.
    fn adopt_selected(&mut self) {
        let Some(doc) = self.selected.and_then(|i| self.docs.get(i)) else {
            self.current = None;
            return;
        };
        let source = Arc::new(doc.source.clone());
        self.current = Some(source.clone());
        let roots = self.asset_roots();
        let batch: Vec<(u64, AssetJob)> = scan_assets(&source, &roots)
            .into_iter()
            .filter(|(key, _)| self.queued.insert(*key))
            .collect();
        for (key, _) in &batch {
            self.assets.insert(*key, Asset::Pending);
        }
        if batch.is_empty() {
            return;
        }
        if let Some(prep) = &self.prep {
            let _ = prep.jobs.send(batch);
        } else {
            // The thread has not started yet (no Context until the first
            // frame); `show` sends this batch once it has one.
            self.pending_batch = Some(batch);
        }
    }

    /// Turn finished pixels into textures. Only the UI thread may do this, so
    /// it happens once per frame rather than in the preparer.
    fn drain_prepared(&mut self, ctx: &Context) {
        let Some(prep) = &self.prep else { return };
        // Bounded per frame: a batch of large screenshots arriving at once must
        // not spend the whole frame uploading.
        for _ in 0..4 {
            match prep.done.try_recv() {
                Ok((key, Decoded::Ok { image, size })) => {
                    let tex = ctx.load_texture("doc_asset", image, egui::TextureOptions::LINEAR);
                    self.assets.insert(key, Asset::Ok { tex, size });
                }
                Ok((key, Decoded::Err(e))) => {
                    self.assets.insert(key, Asset::Err(e));
                }
                Err(_) => break,
            }
        }
    }

    /// Directories a document's relative `src` may be resolved against.
    ///
    /// The documents are embedded from the repository's `docs/`, and their
    /// image paths are written against that directory — `../assets/images/…`.
    /// Joining that onto `<exe dir>/docs` lands on `<exe dir>/assets/images/…`,
    /// which is exactly where the release package puts the assets tree, so the
    /// same relative path works from an installed app and from a source build.
    fn asset_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        // A document opened from disk resolves against its own directory first.
        if let Some(doc) = self.selected.and_then(|i| self.docs.get(i)) {
            let p = Path::new(&doc.id);
            if p.is_file() {
                if let Some(dir) = p.parent() {
                    roots.push(dir.to_path_buf());
                }
            }
        }
        if let Some(exe_dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
        {
            roots.push(exe_dir.join("docs"));
            // macOS: the executable lives in Contents/MacOS and the assets in
            // Contents/Resources, with a symlink between them — but a bundle
            // built by hand may not have the symlink.
            roots.push(exe_dir.join("../Resources/docs"));
        }
        // The repository this binary was built from — a `cargo run` or a
        // `target/release` build straight out of the tree.
        roots.push(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../docs")
                .to_path_buf(),
        );
        roots.push(PathBuf::from("docs"));
        roots
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

        let parent_style = parent.global_style();
        let vp_id = ViewportId::from_hash_of("powerrustcobol_doc_viewer");
        let title = format!("PowerRustCOBOL — {}  v{VERSION}", tr.doc_win_title);

        parent.show_viewport_immediate(
            vp_id,
            ViewportBuilder::default()
                .with_title(title)
                .with_inner_size([1100.0, 760.0])
                .with_min_inner_size([640.0, 420.0])
                .with_transparent(true),
            |root_ui, _class| {
                let ctx = root_ui.ctx().clone();
                let ctx = &ctx;
                ctx.set_global_style((*parent_style).clone());
                ctx.set_zoom_factor(self.zoom);

                // Translucent "frosted glass": the window is transparent and the
                // panels paint nothing, so an uneven fog overlay (built from the
                // theme background colour) is all that sits over the desktop.
                let fog_rgb = parent_style.visuals.panel_fill;
                self.paint_frost(ctx, fog_rgb);
                {
                    let mut s = (*ctx.global_style()).clone();
                    s.visuals.panel_fill = egui::Color32::TRANSPARENT;
                    s.visuals.window_fill = fog_rgb.gamma_multiply(0.92);
                    s.visuals.extreme_bg_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 70);
                    ctx.set_global_style(s);
                }

                if ctx.input(|i| i.viewport().close_requested()) {
                    self.open = false;
                }
                self.start_preparer(ctx);
                self.drain_prepared(ctx);
                self.handle_shortcuts(ctx);
                self.handle_scroll_keys(ctx);
                self.handle_throw(ctx);
                self.menu_bar(root_ui, tr);
                self.toolbar(root_ui, tr);
                self.left_pane(root_ui, tr);
                if self.show_outline {
                    self.outline_pane(root_ui);
                }
                self.viewer_pane(root_ui, tr);
                self.modals(ctx, tr);
            },
        );
    }

    /// Start the preparer thread on the first frame, and hand it anything that
    /// was queued before there was a context to wake.
    fn start_preparer(&mut self, ctx: &Context) {
        if self.prep.is_none() {
            let (jobs_tx, jobs_rx) = std::sync::mpsc::channel::<Vec<(u64, AssetJob)>>();
            let (done_tx, done_rx) = std::sync::mpsc::channel::<(u64, Decoded)>();
            let thread_ctx = ctx.clone();
            std::thread::Builder::new()
                .name("doc-assets".into())
                .spawn(move || prepare_thread(thread_ctx, jobs_rx, done_tx))
                .ok();
            self.prep = Some(Prep {
                jobs: jobs_tx,
                done: done_rx,
            });
        }
        if let (Some(batch), Some(prep)) = (self.pending_batch.take(), &self.prep) {
            let _ = prep.jobs.send(batch);
        }
    }

    /// Grab the page, drag it, and throw it.
    ///
    /// The grab must start over the document — that is what makes it a grab of
    /// the document rather than of the list beside it — but from then on the
    /// gesture belongs to the developer's hand: the drag follows the pointer
    /// wherever it goes, and **the release counts wherever it happens**, over
    /// the toolbar, over the document list, or outside the window entirely.
    ///
    /// Driven here rather than by `ScrollArea`'s own drag-to-scroll (which is
    /// switched off for this area) so the throw's speed comes from the samples
    /// taken during the drag. See [`Throw`] for why egui's own measurement
    /// could not answer it.
    fn handle_throw(&mut self, ctx: &Context) {
        let (down, pos, time, dt) = ctx.input(|i| {
            (
                i.pointer.primary_down(),
                i.pointer.latest_pos(),
                i.time,
                i.stable_dt.min(0.1),
            )
        });

        match (&mut self.throw, down) {
            // Nothing in hand: a press over the document takes hold of it, and
            // stops whatever glide was still running — catching a moving page
            // is how every touch surface behaves.
            (None, true) => {
                if let Some(p) = pos.filter(|p| self.doc_rect.contains(*p)) {
                    self.glide = 0.0;
                    self.throw = Some(Throw {
                        samples: vec![(time, p.y)],
                        last_y: p.y,
                    });
                }
            }
            // In hand: drag by the frame's movement, and remember where the
            // pointer has been for the throw.
            (Some(t), true) => {
                if let Some(p) = pos {
                    let delta = p.y - t.last_y;
                    t.last_y = p.y;
                    t.samples.push((time, p.y));
                    t.samples.retain(|(s, _)| time - *s <= THROW_SAMPLE);
                    if delta != 0.0 {
                        let base = self.pending_offset.unwrap_or(self.scroll_y);
                        self.pending_offset =
                            Some((base - delta).clamp(0.0, self.scroll_max.max(0.0)));
                    }
                }
            }
            // Let go — anywhere. What the page does now is decided by how the
            // hand was moving, not by where it stopped.
            (Some(_), false) => {
                let t = self.throw.take().expect("matched Some");
                self.glide = throw_speed(&t.samples);
                ctx.request_repaint();
            }
            (None, false) => {}
        }

        // The glide, with the friction a thrown thing has.
        if self.throw.is_none() && self.glide != 0.0 {
            let friction = THROW_FRICTION * dt;
            if friction >= self.glide.abs() || self.glide.abs() < THROW_STOP_SPEED {
                self.glide = 0.0;
            } else {
                self.glide -= friction * self.glide.signum();
                let base = self.pending_offset.unwrap_or(self.scroll_y);
                let next = (base - self.glide * dt).clamp(0.0, self.scroll_max.max(0.0));
                // Run into either end and the throw is spent.
                if next == base {
                    self.glide = 0.0;
                }
                self.pending_offset = Some(next);
                ctx.request_repaint();
            }
        }
    }

    /// Scroll the document from the keyboard.
    ///
    /// A tap moves one line. Holding starts at the same reading pace and winds
    /// up to four times that over [`ACCEL_TIME`] — fast enough to cross a long
    /// guide, slow at the start so a held key never overshoots the next
    /// paragraph.
    fn handle_scroll_keys(&mut self, ctx: &Context) {
        // Never while the search box has the caret: the arrows belong to it.
        if ctx.memory(|m| m.focused().is_some()) {
            self.key_hold = 0.0;
            return;
        }
        let line = self.font_pt * 1.6;
        let page = (self.page_h - line * 2.0).max(line);
        let (down, up, pg_dn, pg_up, home, end, dt) = ctx.input(|i| {
            (
                i.key_down(Key::ArrowDown),
                i.key_down(Key::ArrowUp),
                i.key_pressed(Key::PageDown),
                i.key_pressed(Key::PageUp),
                i.key_pressed(Key::Home),
                i.key_pressed(Key::End),
                i.stable_dt.min(0.1),
            )
        });
        let (tap_down, tap_up) =
            ctx.input(|i| (i.key_pressed(Key::ArrowDown), i.key_pressed(Key::ArrowUp)));

        let mut delta = 0.0;
        // The tap: one line, the moment the key goes down.
        if tap_down {
            delta += line;
        }
        if tap_up {
            delta -= line;
        }
        if down || up {
            self.key_hold += dt;
            if let Some(factor) = accel_factor(self.key_hold) {
                let step = BASE_SPEED * factor * dt;
                if down {
                    delta += step;
                }
                if up {
                    delta -= step;
                }
            }
            // Holding a key produces no events of its own; ask for the frames.
            ctx.request_repaint();
        } else {
            self.key_hold = 0.0;
        }
        if pg_dn {
            delta += page;
        }
        if pg_up {
            delta -= page;
        }

        if home {
            self.pending_offset = Some(0.0);
            return;
        }
        if end {
            self.pending_offset = Some(self.scroll_max);
            return;
        }
        if delta != 0.0 {
            let base = self.pending_offset.unwrap_or(self.scroll_y);
            self.pending_offset = Some((base + delta).clamp(0.0, self.scroll_max.max(0.0)));
        }
    }

    fn handle_shortcuts(&mut self, ctx: &Context) {
        let (cmd, alt, f, o, p, w, t, u, plus, minus, prev, next) = ctx.input(|i| {
            let m = i.modifiers;
            (
                m.command,
                m.alt,
                i.key_pressed(Key::F),
                i.key_pressed(Key::O),
                i.key_pressed(Key::P),
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
        // ⌘P — advertised by the toolbar tooltip and the shortcut list, so it
        // has to actually fire.
        if cmd && p {
            self.print();
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

    fn menu_bar(&mut self, panel_ui: &mut egui::Ui, tr: &Tr) {
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        egui::Panel::top("doc_menubar").show(panel_ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button(tr.doc_menu_file, |ui| {
                    if ui.button(tr.doc_print).clicked() {
                        self.print();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(tr.doc_close).clicked() {
                        self.open = false;
                        ui.close();
                    }
                });
                ui.menu_button(tr.doc_menu_view, |ui| {
                    if ui.button(tr.doc_zoom_in).clicked() {
                        self.zoom = (self.zoom * 1.1).min(3.0);
                        ui.close();
                    }
                    if ui.button(tr.doc_zoom_out).clicked() {
                        self.zoom = (self.zoom / 1.1).max(0.5);
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .checkbox(&mut self.fullscreen, tr.doc_fullscreen)
                        .changed()
                    {
                        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self.fullscreen));
                    }
                    ui.checkbox(&mut self.show_outline, tr.doc_miniatures);
                });
                ui.menu_button(tr.doc_menu_help, |ui| {
                    if ui.button(tr.doc_shortcuts).clicked() {
                        self.show_shortcuts = true;
                        ui.close();
                    }
                });
            });
        });
    }

    /// Icon toolbar mirroring the keyboard shortcuts (open / view-source / on-top
    /// / print / close). Icons are drawn as vectors so they are theme-aware and
    /// need no image assets.
    fn toolbar(&mut self, panel_ui: &mut egui::Ui, tr: &Tr) {
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        egui::Panel::top("doc_toolbar").show(panel_ui, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                if icon_button(
                    ui,
                    Icon::Open,
                    false,
                    &format!("{}  (⌘O)", tr.doc_open_file),
                ) {
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
                if icon_button(
                    ui,
                    Icon::Pin,
                    self.on_top,
                    &format!("{}  (⌘T)", tr.doc_on_top),
                ) {
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

    fn left_pane(&mut self, panel_ui: &mut egui::Ui, tr: &Tr) {
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        egui::Panel::left("doc_list_panel")
            .resizable(true)
            .default_size(260.0)
            .min_size(170.0)
            .show(panel_ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr.doc_search);
                    ui.text_edit_singleline(&mut self.list_filter);
                });
                ui.separator();
                let filter = self.list_filter.trim().to_lowercase();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut to_select = None;
                        for (i, doc) in self.docs.iter().enumerate() {
                            if !filter.is_empty() && !doc.title.to_lowercase().contains(&filter) {
                                continue;
                            }
                            if ui
                                .selectable_label(self.selected == Some(i), &doc.title)
                                .clicked()
                            {
                                to_select = Some(i);
                            }
                        }
                        if let Some(i) = to_select {
                            self.select(i);
                        }
                    });
            });
    }

    fn outline_pane(&mut self, panel_ui: &mut egui::Ui) {
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        egui::Panel::right("doc_outline_panel")
            .resizable(true)
            .default_size(220.0)
            .min_size(150.0)
            .show(panel_ui, |ui| {
                ui.label(egui::RichText::new("☰").size(self.font_pt + 2.0));
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
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

    fn viewer_pane(&mut self, panel_ui: &mut egui::Ui, tr: &Tr) {
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        // Search bar + nav + font-size control (right-aligned).
        egui::Panel::top("doc_viewer_search").show(panel_ui, |ui| {
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
                let entered = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                // Go runs the search (jump to the current match); Enter does too.
                if ui.button(tr.doc_find_go).clicked() || entered {
                    self.commit_find();
                }

                // Previous / next match controls + counter.
                let has = self.find_total > 0;
                if ui
                    .add_enabled(has, egui::Button::new("◀").small())
                    .clicked()
                {
                    self.find_prev();
                }
                if ui
                    .add_enabled(has, egui::Button::new("▶").small())
                    .clicked()
                {
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

        egui::CentralPanel::default().show(panel_ui, |ui| {
            if self.selected.is_none() {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.35);
                    ui.label(egui::RichText::new(tr.doc_placeholder).size(16.0).weak());
                });
                return;
            }

            let Some(source) = self.current.clone() else {
                return;
            };
            let search = self.find_query.trim().to_lowercase();
            let base = self.font_pt;
            let scroll_to = self.scroll_to_heading.take();
            let active = (!search.is_empty()).then_some(self.find_idx);
            let scroll_active = self.find_scroll;
            self.find_scroll = false;
            let mtr = tr.doc_mermaid_error;
            let itr = tr.doc_image_error;
            let roots = self.asset_roots();
            // GitHub-style anchors so in-document ToC links can jump to sections.
            let anchors: Vec<(String, usize)> = self
                .outline
                .iter()
                .enumerate()
                .map(|(i, (_, title, _))| (slugify(title), i))
                .collect();

            let want_heading_jump = scroll_to.is_some();
            let mut sa = egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                // Grab-and-throw is `handle_throw`'s, not egui's: egui reads a
                // throw's speed from the pointer's own history at the moment
                // the button comes up, which is empty unless the release
                // happened over the document. Leaving its drag on as well
                // would move the page twice for one gesture.
                .scroll_source(egui::containers::scroll_area::ScrollSource {
                    drag: egui::containers::scroll_area::DragScroll::Never,
                    ..Default::default()
                });
            if let Some(off) = self.pending_offset.take() {
                sa = sa.vertical_scroll_offset(off);
            }
            // Heights are only valid for the layout they were measured under.
            let doc_id = self.selected.and_then(|i| self.docs.get(i)).map(|d| &d.id);
            self.blocks.retarget(layout_key(
                doc_id.map(String::as_str).unwrap_or(""),
                ui.available_width(),
                base,
            ));
            let sa_out = sa.show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                // Both drawing closures need the same asset table, and the
                // renderer holds both at once — one cell, borrowed in turn.
                let sink = std::cell::RefCell::new(AssetSink {
                    assets: &mut self.assets,
                    queued: &mut self.queued,
                    prep: self.prep.as_ref(),
                });
                let roots = &roots;
                let opts = RenderOpts {
                    search: &search,
                    base,
                    scroll_to_heading: scroll_to,
                    active_match: active,
                    scroll_to_active: scroll_active,
                    anchors: &anchors,
                    table_layout: md_render::TableLayout::Equal,
                };
                // Both closures are a safety net: the preparer was handed every
                // asset when the document was selected, so what they normally
                // find is either finished pixels or a job already running.
                let mut mermaid = |ui: &mut egui::Ui, code: &str| {
                    let key = fnv1a(code);
                    {
                        let mut s = sink.borrow_mut();
                        if !s.assets.contains_key(&key) {
                            s.queue(key, AssetJob::Mermaid(code.to_string()));
                        }
                    }
                    draw_asset(sink.borrow().assets, ui, key, None, code, mtr);
                };
                let mut image = |ui: &mut egui::Ui, img: &ImageRef<'_>| {
                    let key = asset_key(img.src);
                    {
                        let mut s = sink.borrow_mut();
                        if !s.assets.contains_key(&key) {
                            match resolve_asset(img.src, roots) {
                                Some(path) => s.queue(key, AssetJob::Image(path)),
                                // Under none of the roots: say so, rather than
                                // leaving a placeholder waiting on a job that
                                // can never finish.
                                None => {
                                    s.assets.insert(key, Asset::Err(img.src.to_string()));
                                }
                            }
                        }
                    }
                    draw_asset(sink.borrow().assets, ui, key, img.width, img.alt, itr);
                };
                md_render::render_with(
                    ui,
                    &source,
                    &opts,
                    md_render::Extras {
                        mermaid: &mut mermaid,
                        image: Some(&mut image),
                        cache: Some(&mut self.blocks),
                    },
                )
            });
            let out = sa_out.inner;
            // Mirror the scroll state so the keyboard can drive it next frame.
            self.scroll_y = sa_out.state.offset.y;
            self.page_h = sa_out.inner_rect.height();
            self.scroll_max = (sa_out.content_size.y - self.page_h).max(0.0);
            // What a grab has to start inside of, next frame. The release is
            // not tested against it — see `handle_throw`.
            self.doc_rect = sa_out.inner_rect;
            // The hand: open over the document, closed while it is held. Set
            // here because egui's own drag cursors travel with its
            // drag-to-scroll, which this area does not use.
            if self.throw.is_some() {
                ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if ctx
                .pointer_interact_pos()
                .is_some_and(|p| sa_out.inner_rect.contains(p))
            {
                ctx.set_cursor_icon(egui::CursorIcon::Grab);
            }

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
                        ("↑ ↓ ⇞ ⇟ ⇱ ⇲", tr.doc_scroll_keys),
                    ];
                    egui::Grid::new("doc_shortcuts_grid")
                        .striped(true)
                        .show(ui, |ui| {
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

/// The asset table as the two drawing closures see it.
struct AssetSink<'a> {
    assets: &'a mut HashMap<u64, Asset>,
    queued: &'a mut HashSet<u64>,
    prep: Option<&'a Prep>,
}

impl AssetSink<'_> {
    /// Hand one asset to the preparer, once.
    fn queue(&mut self, key: u64, job: AssetJob) {
        self.assets.insert(key, Asset::Pending);
        if !self.queued.insert(key) {
            return; // already in flight
        }
        if let Some(p) = self.prep {
            let _ = p.jobs.send(vec![(key, job)]);
        }
    }
}

/// Draw one prepared asset — a diagram or an image.
///
/// `declared` is the document's own `width=` for an image, in points, and
/// `fallback` is the text to show when it could not be prepared (a diagram's
/// source, an image's alt text).
fn draw_asset(
    assets: &HashMap<u64, Asset>,
    ui: &mut egui::Ui,
    key: u64,
    declared: Option<f32>,
    fallback: &str,
    err_label: &str,
) {
    let avail = ui.available_width().max(1.0);
    match assets.get(&key) {
        Some(Asset::Ok { tex, size }) => {
            // The document's width wins where it gave one, but never wider
            // than the pane; the aspect ratio is always the image's own.
            let w = declared.unwrap_or(size.x).min(avail);
            let h = if size.x > 0.0 {
                w * size.y / size.x
            } else {
                size.y
            };
            ui.add_space(6.0);
            ui.vertical_centered(|ui| {
                ui.add(egui::Image::new((tex.id(), egui::vec2(w, h))));
            });
            ui.add_space(6.0);
        }
        // Still decoding: hold a plausible amount of room so the text around it
        // does not jump far when the pixels land.
        Some(Asset::Pending) | None => {
            let w = declared.unwrap_or(360.0).min(avail);
            let h = w * 0.6;
            ui.add_space(6.0);
            ui.vertical_centered(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
                ui.painter().rect_stroke(
                    rect,
                    egui::CornerRadius::same(4),
                    egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                    egui::StrokeKind::Inside,
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "…",
                    egui::FontId::proportional(18.0),
                    ui.visuals().weak_text_color(),
                );
            });
            ui.add_space(6.0);
        }
        Some(Asset::Err(e)) => {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(format!("⚠ {err_label}: {e}")).weak());
            if !fallback.is_empty() {
                ui.add(egui::Label::new(egui::RichText::new(fallback).monospace()).wrap());
            }
            ui.add_space(4.0);
        }
    }
}

/// Identity of a layout, for the block-height cache: the same document at the
/// same width and font size measures the same heights, and nothing else does.
fn layout_key(doc_id: &str, width: f32, base: f32) -> u64 {
    let mut h = fnv1a(doc_id);
    h ^= (width.round() as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15);
    h ^= ((base * 10.0).round() as i64 as u64).wrapping_mul(0xc2b2ae3d27d4eb4f);
    h
}

/// Everything `source` embeds, in reading order, ready for the preparer.
///
/// Both spellings of an image are collected: Markdown `![alt](src)` and the raw
/// `<img src=…>` the guide writes for its screenshots. An image whose file
/// cannot be found under any root is skipped here and reported by the drawing
/// side, which knows the language to report it in.
fn scan_assets(source: &str, roots: &[PathBuf]) -> Vec<(u64, AssetJob)> {
    use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};

    let mut out: Vec<(u64, AssetJob)> = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();
    let mut push = |key: u64, job: AssetJob, out: &mut Vec<(u64, AssetJob)>| {
        if seen.insert(key) {
            out.push((key, job));
        }
    };

    let parser = Parser::new_ext(
        source,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    );
    let events: Vec<Event> = parser.collect();
    let mut i = 0;
    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang)))
                if lang.eq_ignore_ascii_case("mermaid") =>
            {
                let mut code = String::new();
                let mut j = i + 1;
                while j < events.len() {
                    match &events[j] {
                        Event::Text(t) => code.push_str(t),
                        Event::End(pulldown_cmark::TagEnd::CodeBlock) => break,
                        _ => {}
                    }
                    j += 1;
                }
                push(fnv1a(&code), AssetJob::Mermaid(code), &mut out);
                i = j + 1;
                continue;
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                if let Some(path) = resolve_asset(dest_url, roots) {
                    push(asset_key(dest_url), AssetJob::Image(path), &mut out);
                }
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                for src in html_image_srcs(html) {
                    if let Some(path) = resolve_asset(src, roots) {
                        push(asset_key(src), AssetJob::Image(path), &mut out);
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// The `src` of every `<img>` in a raw-HTML block. A deliberate twin of
/// `md_render`'s own scan: this one runs before there is a `Ui`, and only needs
/// the path.
fn html_image_srcs(html: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find("<img") {
        let after = &rest[at + 4..];
        let Some(end) = after.find('>') else { break };
        if let Some(src) = attr_value(&after[..end], "src") {
            out.push(src);
        }
        rest = &after[end + 1..];
    }
    out
}

/// Value of `name="…"` in one tag's attribute text.
fn attr_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    loop {
        let at = rest.find(name)?;
        let before_ok = at == 0
            || rest[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_whitespace());
        let after = &rest[at + name.len()..];
        let trimmed = after.trim_start();
        if before_ok && trimmed.starts_with('=') {
            let value = trimmed[1..].trim_start();
            let quote = value.chars().next()?;
            if quote == '"' || quote == '\'' {
                let inner = &value[1..];
                let end = inner.find(quote)?;
                return Some(&inner[..end]);
            }
            let end = value.find(char::is_whitespace).unwrap_or(value.len());
            return Some(&value[..end]);
        }
        rest = after;
    }
}

/// Cache key for an image: its `src` as the document wrote it. Two documents
/// naming the same screenshot share one texture.
fn asset_key(src: &str) -> u64 {
    fnv1a(src)
}

/// Find `src` under the first root that has it.
///
/// An absolute path and a `file:` URL are taken as they are; anything remote
/// (`http:`, `data:`) is not fetched — the documentation ships with the app and
/// must render with no network.
fn resolve_asset(src: &str, roots: &[PathBuf]) -> Option<PathBuf> {
    let src = src.trim();
    if src.is_empty() || src.starts_with("http://") || src.starts_with("https://") {
        return None;
    }
    let src = src.strip_prefix("file://").unwrap_or(src);
    let raw = Path::new(src);
    if raw.is_absolute() {
        return raw.is_file().then(|| raw.to_path_buf());
    }
    roots
        .iter()
        .map(|root| normalize(&root.join(raw)))
        .find(|p| p.is_file())
}

/// Resolve `.` and `..` textually, without touching the filesystem.
///
/// `canonicalize` cannot be used: `<exe dir>/docs` is the anchor the embedded
/// documents' paths are written against, and in a release package that
/// directory does not exist — only the `../assets/…` it points at does.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
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

/// Render a diagram to pixels. Runs on the preparer thread — it never touches
/// an egui context, only the returned image does.
fn render_mermaid_image(code: &str) -> Result<(egui::ColorImage, egui::Vec2), String> {
    let pixmap = render_mermaid_pixmap(code, 2.0)?;
    let (w, h) = (pixmap.width(), pixmap.height());
    let pixels: Vec<egui::Color32> = pixmap
        .pixels()
        .iter()
        .map(|p| egui::Color32::from_rgba_premultiplied(p.red(), p.green(), p.blue(), p.alpha()))
        .collect();
    let image = egui::ColorImage {
        size: [w as usize, h as usize],
        source_size: egui::vec2(w as f32, h as f32),
        pixels,
    };
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
    let opt = resvg::usvg::Options {
        fontdb: mermaid_fontdb(),
        ..Default::default()
    };
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
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "document".into()
    } else {
        s
    }
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
        ui.painter().rect(
            rect.shrink(1.0),
            egui::CornerRadius::same(4),
            v.bg_fill,
            egui::Stroke::NONE,
            egui::StrokeKind::Middle,
        );
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
                [
                    pos2(cx + w * 0.07, t + h * 0.14),
                    pos2(cx - w * 0.07, t + h * 0.86),
                ],
                s,
            );
        }
        Icon::Pin => {
            // A thumbtack: round head, cap line, and needle.
            painter.circle_stroke(pos2(cx, t + h * 0.30), w * 0.22, s);
            painter.line_segment(
                [
                    pos2(cx - w * 0.30, t + h * 0.52),
                    pos2(cx + w * 0.30, t + h * 0.52),
                ],
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
                egui::CornerRadius::same(2),
                s,
                egui::StrokeKind::Middle,
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
    let img = egui::ColorImage {
        size: [W, H],
        source_size: egui::vec2(W as f32, H as f32),
        pixels,
    };
    ctx.load_texture("doc_frost", img, egui::TextureOptions::LINEAR)
}

/// Deterministic 0..1 hash for integer lattice points.
fn hash01(x: i32, y: i32) -> f32 {
    let mut h = (x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263))) as u32;
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

    /// The guide writes its screenshots as raw HTML, and the scan that feeds
    /// the preparer has to find them — this is the shape it must handle.
    #[test]
    fn html_screenshots_are_found_in_the_guide() {
        let block = r#"<p align="center"><img src="../assets/images/screenshots/welcome.png" alt="The welcome screen" width="900"></p>"#;
        assert_eq!(
            html_image_srcs(block),
            vec!["../assets/images/screenshots/welcome.png"]
        );
        assert_eq!(attr_value(block, "alt"), Some("The welcome screen"));
        assert_eq!(attr_value(block, "width"), Some("900"));

        // Two in one block, single quotes, and no width.
        let two = "<img src='a.png'><img src=\"b.png\" width=\"120\">";
        assert_eq!(html_image_srcs(two), vec!["a.png", "b.png"]);
    }

    /// The real guide, as it ships: the scan must actually come back with the
    /// screenshots and diagrams, or the preparer has nothing to prepare.
    #[test]
    fn the_guide_scan_finds_its_screenshots_and_diagrams() {
        let docs = docs_embed::doc_list(Language::English);
        let guide = docs
            .iter()
            .find(|d| d.id.starts_with("developers-guide"))
            .expect("the guide ships");
        // No roots: images cannot resolve, so only the diagrams come back —
        // which is exactly how a scan behaves on a machine missing assets/.
        let diagrams = scan_assets(&guide.source, &[]);
        assert!(
            diagrams
                .iter()
                .all(|(_, j)| matches!(j, AssetJob::Mermaid(_))),
            "with no roots, no image job can be produced"
        );

        // With the repository root, every `<img>` in the guide resolves.
        let roots = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs")];
        let all = scan_assets(&guide.source, &roots);
        let images = all
            .iter()
            .filter(|(_, j)| matches!(j, AssetJob::Image(_)))
            .count();
        assert!(
            images >= 8,
            "the guide embeds a dozen screenshots; the scan found {images}"
        );
    }

    /// `../assets/…` is resolved against a `docs` directory that need not
    /// exist — which is the whole trick that makes one relative path work both
    /// from the repository and from an installed app.
    #[test]
    fn asset_paths_resolve_through_a_directory_that_is_not_there() {
        let root = Path::new("/opt/PowerRustCOBOL/docs"); // never created
        assert_eq!(
            normalize(&root.join("../assets/images/screenshots/x.png")),
            PathBuf::from("/opt/PowerRustCOBOL/assets/images/screenshots/x.png")
        );

        // And a real one resolves for real: the guide's own mascot.
        let roots = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs")];
        let found = resolve_asset("../assets/images/powerrustcobol-mascot.png", &roots)
            .expect("the mascot ships in the repository");
        assert!(found.is_file());

        // Remote sources are never fetched — the docs render offline.
        assert!(resolve_asset("https://example.com/a.png", &roots).is_none());
    }

    /// A throw's speed is the hand's speed, measured over the drag — and it
    /// does not know or care where the button came up, which is the whole
    /// point (operator, 2026-09-10).
    #[test]
    fn a_throw_takes_its_speed_from_the_drag_not_the_release() {
        // 100 points up in a tenth of a second: 1000 points per second.
        let samples = vec![(1.00, 400.0), (1.05, 350.0), (1.10, 300.0)];
        assert!(
            (throw_speed(&samples) - -1000.0).abs() < 1.0,
            "expected -1000 pt/s, got {}",
            throw_speed(&samples)
        );

        // A hand that stopped before letting go throws nothing, however far it
        // travelled earlier — the samples older than THROW_SAMPLE are gone by
        // then, which is what leaves a stopped page where it is.
        let stopped = vec![(2.00, 300.0), (2.05, 300.0), (2.10, 300.0)];
        assert_eq!(throw_speed(&stopped), 0.0);

        // Degenerate input is not a divide by zero.
        assert_eq!(throw_speed(&[]), 0.0);
        assert_eq!(throw_speed(&[(1.0, 10.0)]), 0.0);
        assert_eq!(throw_speed(&[(1.0, 10.0), (1.0, 90.0)]), 0.0);
    }

    /// The glide is spent by friction rather than running for ever, and it
    /// stops instead of creeping once it is slower than the eye can follow.
    #[test]
    fn a_thrown_page_comes_to_rest() {
        // Friction is THROW_FRICTION points per second per second, so a throw
        // of 1000 pt/s has about a second in it.
        let mut speed: f32 = 1000.0;
        let dt = 1.0 / 60.0;
        let mut frames = 0;
        while speed != 0.0 && frames < 600 {
            let friction = THROW_FRICTION * dt;
            if friction >= speed.abs() || speed.abs() < THROW_STOP_SPEED {
                speed = 0.0;
            } else {
                speed -= friction * speed.signum();
            }
            frames += 1;
        }
        assert!(speed == 0.0, "a throw must come to rest");
        assert!(
            (30..=90).contains(&frames),
            "a 1000 pt/s throw should settle in about a second at 60 fps, took {frames} frames"
        );
    }

    /// Holding an arrow key starts at the reading pace and winds up to exactly
    /// four times it, never further.
    #[test]
    fn held_arrow_keys_accelerate_to_four_times_and_stop() {
        // A tap is not a hold.
        assert_eq!(accel_factor(0.0), None);
        assert_eq!(accel_factor(REPEAT_DELAY), None);

        // The hold begins at the normal pace.
        let start = accel_factor(REPEAT_DELAY + 0.001).expect("holding");
        assert!(
            (start - 1.0).abs() < 0.01,
            "a hold must start at 1×, not {start}×"
        );

        // Half way up the ramp, half way up the range.
        let mid = accel_factor(REPEAT_DELAY + ACCEL_TIME / 2.0).expect("holding");
        assert!((mid - 2.5).abs() < 0.01, "expected 2.5×, got {mid}×");

        // And it stops at four, however long it is held.
        assert_eq!(accel_factor(REPEAT_DELAY + ACCEL_TIME), Some(MAX_FACTOR));
        assert_eq!(accel_factor(600.0), Some(MAX_FACTOR));
        assert!(accel_factor(f32::MAX).unwrap() <= MAX_FACTOR);
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
