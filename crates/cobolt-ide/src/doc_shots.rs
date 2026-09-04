// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Documentation screenshot capture — an **internal authoring tool**.
//!
//! # What it is for
//!
//! PowerRustCOBOL's English Markdown documentation — the Developer's Guide,
//! `README.md`, and anything else under `docs/` — carries
//! `📷 Screenshot needed — \`name.png\`` placeholders. Filling them by hand
//! means hunting a window id, shelling out to
//! a capture utility and pasting markdown. Worse, the shots that matter most —
//! an open combo popup, the DateTimePicker calendar, a hover highlight — vanish
//! the moment anything is clicked.
//!
//! This module turns that into one keystroke: arrange the view, press **F12**
//! (or **Shift+F12** for a [`DELAY`] countdown when the state must be held with
//! the mouse), then pick the slot to fill from a popup.
//!
//! # Why a key and not the title bar
//!
//! The IDE runs with the platform's own window decorations, so the title bar is
//! drawn by the OS and its double-click is already bound there — no egui event
//! ever arrives. A key press also leaves focus, popups and hover states intact,
//! which is exactly what most of the pending shots are *of*. And because every
//! designer and every running form is its own viewport, the capture follows
//! whichever window has focus rather than one fixed title bar.
//!
//! # Contract with `/doc-shots`
//!
//! The marker, the image directory and the inserted HTML are deliberately the
//! ones the `/doc-shots` skill already reads and writes, so a shot taken here
//! and a shot taken by the agent are indistinguishable in the document:
//!
//! ```text
//! <!-- 📷 install-launch.png — <the original instruction> -->
//! <p align="center"><img src="../assets/images/screenshots/install-launch.png" alt="…" width="900"></p>
//! ```
//!
//! The instruction survives as an HTML comment on purpose: a filled slot stays
//! findable, so a shot invalidated by a UI change can be retaken without the
//! author having to remember what was supposed to be on screen.
//!
//! # Scope
//!
//! English documents only (GOLDEN RULE #3) — the translated guides reference the
//! same language-neutral images. Hidden behind Help → Debug Settings; it is not
//! part of the shipped product.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Color32, ColorImage, Context, Key, ViewportId};

/// Image directory, relative to the repository root. Fixed by the `/doc-shots`
/// contract — both writers must agree or the guide ends up with two conventions.
pub const SHOTS_REL: &str = "assets/images/screenshots";

/// Widest the saved PNG may be. A 1280×800 window captures at 2560×1600 on a
/// HiDPI display; storing that verbatim would put multi-megabyte images in a
/// repository for no gain, since the guide renders them under 900 px.
const MAX_WIDTH: u32 = 1600;

/// The `width=` attribute written into the document.
const DOC_WIDTH: u32 = 900;

/// Shift+F12 countdown — long enough to grab the mouse and open a menu.
pub const DELAY: Duration = Duration::from_secs(3);

/// Longest instruction kept in the HTML comment.
const MAX_COMMENT: usize = 400;

/// Longest `alt` text.
const MAX_ALT: usize = 120;

// ── Document discovery ────────────────────────────────────────────────────────

/// Root of the PowerRustCOBOL checkout this binary was built from.
///
/// Baked in at compile time so the tool works from `cargo run` and from a copied
/// `.app` bundle alike (both were built from the same tree). `PRC_DOCS_ROOT`
/// overrides it. Returns `None` when the tree is gone — the feature then simply
/// has nothing to offer.
pub fn repo_root() -> Option<PathBuf> {
    if let Ok(over) = std::env::var("PRC_DOCS_ROOT") {
        let path = PathBuf::from(over);
        return path.join("docs").is_dir().then_some(path);
    }
    // <root>/crates/cobolt-ide → <root>
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.parent()?;
    root.join("docs").is_dir().then(|| root.to_path_buf())
}

/// `true` for an English document. The translated guides carry the same
/// placeholders, and filling those is the operator's call, not this tool's.
pub fn is_english_doc(path: &Path) -> bool {
    if path.extension().and_then(|e| e.to_str()) != Some("md") {
        return false;
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    !["-es", "-pt", "-jp", "-cn", "-fr", "-ja", "-zh"]
        .iter()
        .any(|suffix| stem.ends_with(suffix))
}

/// Every English Markdown document the capture tool may fill, sorted.
///
/// **The repository's own top-level files — `README.md` among them — and
/// everything under `docs/`, at any depth.** It used to be a single
/// non-recursive `read_dir` of `docs/`, which left `README.md` unreachable
/// because it does not live there, and skipped anything in a `docs/`
/// subdirectory (operator, 2026-09-03: "only update the user guide … I need it
/// to be able to update any PowerRustCOBOL documentation in Markdown format
/// (.md), including the README.md file").
///
/// In practice it *looked* guide-only for a second reason: the picker lists a
/// document only when it holds at least one `📷` slot, and the guide was the
/// only one that did. Widening the scan is therefore safe — a document with no
/// slots never appears — so this errs towards offering more rather than
/// guessing which files count as documentation.
///
/// The top level is deliberately NOT recursed: that would drag in `target/`,
/// `crates/`, the agent skill files and the NIST corpus, none of which are
/// documentation a screenshot belongs in.
pub fn english_docs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_english_md(root, false, &mut out);
    collect_english_md(&root.join("docs"), true, &mut out);
    out.sort();
    out.dedup();
    out
}

/// English `.md` files in `dir`, descending into subdirectories when asked.
fn collect_english_md(dir: &Path, recurse: bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.is_dir() {
            if recurse {
                collect_english_md(&path, true, out);
            }
        } else if is_english_doc(&path) {
            out.push(path);
        }
    }
}

// ── Window capture ────────────────────────────────────────────────────────────

/// Which window to photograph, in screen coordinates.
///
/// **The handle every viewport can supply for itself.** egui knows where its own
/// OS window sits, so the main window and a Form Designer window hand over the
/// same kind of value and call the same capture — one implementation, no branch
/// on "am I the root".
///
/// That symmetry is the whole point. The previous design asked egui for the
/// picture (`ViewportCommand::Screenshot`), and eframe only services that where
/// a viewport runs its own paint loop. A designer window is an *immediate*
/// viewport — rendered inside the root's frame — so its request was queued and
/// silently never serviced: the key arrived, the command went out, no image ever
/// came back (operator's trace, 2026-09-04: `F12 seen … viewport="2134"`
/// repeatedly, `capture landed` never once).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowTarget {
    /// The window's outer rectangle, in logical screen points.
    pub rect: egui::Rect,
    /// Points → pixels on the display it is on.
    pub scale: f32,
}

impl WindowTarget {
    /// What this viewport knows about its own window. `None` when the platform
    /// does not report a rectangle, which is the honest answer — better than
    /// photographing the wrong part of the screen.
    pub fn of(ctx: &Context) -> Option<Self> {
        // egui hands out every rectangle in *its own* points: screen points
        // divided by the context-wide zoom factor (egui-winit
        // `outer_rect_in_points` is `outer_rect_px / (zoom × native)`).
        // `screencapture -R` wants screen points, so the zoom is multiplied
        // back. At 1.0 that is the identity; at anything else it is the
        // difference between the window and a slab of desktop around it —
        // and two things in the IDE move it: egui's own Cmd+= / Cmd+-, and the
        // doc viewer's Zoom In/Out, which set it for the whole context.
        let zoom = ctx.zoom_factor();
        if !(zoom > 0.0) {
            return None;
        }
        ctx.input(|input| {
            let viewport = input.viewport();
            let rect = viewport.outer_rect? * zoom;
            let scale = viewport
                .native_pixels_per_point
                .unwrap_or_else(|| input.pixels_per_point() / zoom);
            (rect.width() >= 1.0 && rect.height() >= 1.0 && scale > 0.0)
                .then_some(Self { rect, scale })
        })
    }

    /// The `x,y,w,h` region argument, rounded outwards so a fractional window
    /// edge never shaves a pixel off the shot.
    ///
    /// A hair under a whole number is treated as that whole number: the zoom
    /// round trip in [`Self::of`] can turn 100 into 99.99999, and flooring
    /// that would move the region one point left of the window.
    pub fn region(&self) -> String {
        const HAIR: f32 = 1.0 / 1024.0;
        let x = (self.rect.min.x + HAIR).floor() as i64;
        let y = (self.rect.min.y + HAIR).floor() as i64;
        let w = (self.rect.width() - HAIR).ceil().max(1.0) as i64;
        let h = (self.rect.height() - HAIR).ceil().max(1.0) as i64;
        format!("{x},{y},{w},{h}")
    }
}

/// Photograph one window — the single implementation every surface calls.
///
/// macOS only, deliberately: `screencapture` is the same tool the `/doc-shots`
/// skill drives, so a shot taken by hand and one taken here come out of the same
/// pipe. Capturing a *region* rather than a window id keeps this free of
/// CoreGraphics bindings — the rectangle is what egui already knows.
/// What macOS says when the permission is missing — the whole remedy, in the
/// popup, where the operator is.
#[cfg(target_os = "macos")]
const NO_SCREEN_RECORDING: &str = "macOS is not letting PowerRustCOBOL see \
     other windows, so the picture would have been the desktop wallpaper with \
     every window missing. Turn on System Settings → Privacy & Security → \
     Screen Recording for the app that STARTED PowerRustCOBOL — Terminal, \
     iTerm, VS Code — not for “cobolt-ide”, which will not be in the list. \
     Then quit that app and start the IDE again.";

/// Does this process have macOS's **Screen Recording** permission?
///
/// Without it `screencapture` still exits 0 and still writes a perfectly valid
/// PNG — of the desktop wallpaper, with every window omitted. The exit code,
/// the file size and the image all look like success, which is how two
/// photographs of a redwood forest reached `assets/images/screenshots/`
/// (operator, 2026-09-04: "F12 is capturing the desktop instead the window
/// where it was called"). Both were exactly 1280×828 points — the window's own
/// outer rectangle — so the region was never the fault.
///
/// The permission belongs to the **responsible** process, not to this binary:
/// a `cobolt-ide` started from a shell inherits the grant of the terminal
/// application that owns that shell. That is why the app to switch on is
/// Terminal (or iTerm, or VS Code), and why "cobolt-ide" never appears in the
/// Screen Recording list.
///
/// `CGPreflightScreenCaptureAccess` reads the answer without prompting;
/// `CGRequestScreenCaptureAccess` asks, which is what puts the responsible app
/// INTO that list the first time. Neither one grants anything — only the
/// operator can, in System Settings.
#[cfg(target_os = "macos")]
fn screen_recording_allowed() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }
    // SAFETY: both take no arguments and return a C `bool`; they are the
    // documented way to read and to ask for this permission (CoreGraphics,
    // macOS 10.15+), and neither touches memory this side owns.
    unsafe {
        if CGPreflightScreenCaptureAccess() {
            return true;
        }
        CGRequestScreenCaptureAccess()
    }
}

#[cfg(target_os = "macos")]
pub fn capture_window(target: &WindowTarget) -> Result<ColorImage, String> {
    capture_window_when(screen_recording_allowed(), target)
}

/// [`capture_window`] with the permission answer supplied, so the refusal can
/// be tested without depending on how the test machine is configured.
#[cfg(target_os = "macos")]
fn capture_window_when(allowed: bool, target: &WindowTarget) -> Result<ColorImage, String> {
    if !allowed {
        tracing::warn!(
            target: "doc_shots",
            "no Screen Recording permission: refusing to photograph the desktop"
        );
        return Err(NO_SCREEN_RECORDING.to_owned());
    }
    let temp = std::env::temp_dir().join(format!(
        "prc-doc-shot-{}.png",
        std::process::id() as u64 * 31 + Instant::now().elapsed().subsec_nanos() as u64
    ));
    let status = std::process::Command::new("/usr/sbin/screencapture")
        .args(["-x", "-o", "-R", &target.region()])
        .arg(&temp)
        .status()
        .map_err(|error| format!("could not run screencapture: {error}"))?;
    if !status.success() {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("screencapture exited with {status}"));
    }
    let decoded = image::open(&temp);
    let _ = std::fs::remove_file(&temp);
    let decoded = decoded.map_err(|error| {
        format!(
            "screencapture wrote nothing readable ({error}). On macOS this is \
             usually Screen Recording permission: System Settings → Privacy & \
             Security → Screen Recording, then restart the IDE"
        )
    })?;
    let rgba = decoded.to_rgba8();
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);
    if width == 0 || height == 0 {
        return Err("the capture came back empty".into());
    }
    Ok(ColorImage {
        size: [width, height],
        pixels: rgba
            .pixels()
            .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect(),
        source_size: egui::Vec2::new(width as f32, height as f32),
    })
}

/// Every other platform: say so rather than pretend.
#[cfg(not(target_os = "macos"))]
pub fn capture_window(_target: &WindowTarget) -> Result<ColorImage, String> {
    Err("window capture is implemented for macOS only".into())
}

// ── Slots ─────────────────────────────────────────────────────────────────────

/// What kind of marker a slot came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotKind {
    /// `📷 **Screenshot needed — \`name.png\`.** …` — carries name and instruction.
    Needed,
    /// A bare `[SCREENSHOT]` shorthand; the file name is asked for on insert.
    Bare,
    /// Already filled — kept listed so a stale shot can be retaken in place.
    Filled,
}

/// One screenshot position inside a document.
#[derive(Debug, Clone)]
pub struct Slot {
    pub kind: SlotKind,
    /// 0-based index of the first line the marker occupies.
    pub line: usize,
    /// How many lines it occupies (the whole block quote, or comment + image).
    pub span: usize,
    /// Target file name, when the marker names one.
    pub name: Option<String>,
    /// What must be on screen — the placeholder's own words, or the surrounding
    /// text for a bare `[SCREENSHOT]`.
    pub context: String,
}

impl Slot {
    /// Short label for the popup list.
    pub fn label(&self) -> String {
        match (&self.name, self.kind) {
            (Some(name), SlotKind::Filled) => format!("{name}  (filled)"),
            (Some(name), _) => name.clone(),
            (None, _) => format!("[SCREENSHOT] · line {}", self.line + 1),
        }
    }
}

/// Find every screenshot slot in a document.
pub fn scan(text: &str) -> Vec<Slot> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];

        // A filled slot: our own comment marker, followed by the image line.
        if let Some(rest) = line.trim_start().strip_prefix("<!-- 📷 ") {
            if let Some(end) = rest.find("-->") {
                let (name, context) = split_filled(&rest[..end]);
                let span = if i + 1 < lines.len() && lines[i + 1].contains("<img") {
                    2
                } else {
                    1
                };
                out.push(Slot { kind: SlotKind::Filled, line: i, span, name, context });
                i += span;
                continue;
            }
        }

        // An unfilled placeholder, possibly spanning a whole block quote.
        if line.contains("Screenshot needed") {
            let mut span = 1;
            if line.trim_start().starts_with('>') {
                while i + span < lines.len() && lines[i + span].trim_start().starts_with('>') {
                    span += 1;
                }
            } else {
                while i + span < lines.len() && continues_marker(lines[i + span]) {
                    span += 1;
                }
            }
            let (name, context) = split_needed(&join_block(&lines[i..i + span]));
            out.push(Slot { kind: SlotKind::Needed, line: i, span, name, context });
            i += span;
            continue;
        }

        // The shorthand.
        if line.contains("[SCREENSHOT]") {
            out.push(Slot {
                kind: SlotKind::Bare,
                line: i,
                span: 1,
                name: None,
                context: bare_context(&lines, i),
            });
            i += 1;
            continue;
        }

        i += 1;
    }
    out
}

/// Whether an unquoted marker's instruction carries on into this line. Prose
/// continues; a blank line or the start of any other markdown block ends it.
fn continues_marker(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with('#')
        && !trimmed.starts_with('>')
        && !trimmed.starts_with("<!--")
        && !trimmed.starts_with("<p ")
        && !trimmed.starts_with("```")
        && !trimmed.starts_with('|')
        && !trimmed.contains("Screenshot needed")
        && !trimmed.contains("[SCREENSHOT]")
}

/// Strip the block-quote markers and fold a marker block into one line.
fn join_block(lines: &[&str]) -> String {
    let mut out = String::new();
    for line in lines {
        let text = line.trim_start();
        let text = text.strip_prefix('>').unwrap_or(text).trim();
        if !out.is_empty() && !text.is_empty() {
            out.push(' ');
        }
        out.push_str(text);
    }
    out
}

/// Split a `📷 Screenshot needed — name.png …` block into its parts.
///
/// The guide writes this marker five different ways, and all of them are real:
/// bold inside a block quote with the period in or out of the backticks, bold
/// followed by an em dash, an unquoted definition list whose instruction starts
/// on the next line behind a `:`, unquoted with the instruction simply flowing
/// on, and one with the file name carrying no backticks at all.
fn split_needed(joined: &str) -> (Option<String>, String) {
    let start = joined.find("Screenshot needed").unwrap_or(0);
    let rest = &joined[start..];
    // Whether the marker itself is bold decides how the instruction begins. Any
    // other `**` in the block belongs to the prose (`press **Edit Toolbar…**`)
    // and must not be mistaken for the end of the marker.
    let bold = joined[..start].trim_end().ends_with("**");

    let mut name = None;
    let mut after_name = 0;
    if let Some(open) = rest.find('`') {
        if let Some(len) = rest[open + 1..].find('`') {
            let candidate = &rest[open + 1..open + 1 + len];
            if candidate.ends_with(".png") {
                name = Some(candidate.to_string());
                after_name = open + 1 + len + 1;
            }
        }
    }
    // One placeholder names its file without backticks.
    if name.is_none() {
        if let Some(dot) = rest.find(".png") {
            let end = dot + ".png".len();
            let from = rest[..dot]
                .rfind(char::is_whitespace)
                .map_or(0, |at| at + 1);
            let candidate = rest[from..end].trim_matches(['`', '*', '"']);
            if !candidate.is_empty() {
                name = Some(candidate.to_string());
                after_name = end;
            }
        }
    }

    // A bold marker's instruction starts once the bold run closes; an unbolded
    // one starts right after the name. Either way the separator left over —
    // a period, a colon, an em dash — is punctuation, not prose.
    let tail = &rest[after_name..];
    let instruction = match bold.then(|| tail.find("**")).flatten() {
        Some(at) => &tail[at + 2..],
        None => tail,
    };
    (
        name,
        normalize_ws(instruction.trim_start_matches(['.', ':', '—', '-', ' '])),
    )
}

/// Read back our own `<!-- 📷 name.png — instruction -->` marker.
fn split_filled(body: &str) -> (Option<String>, String) {
    let body = body.trim();
    match body.split_once(" — ") {
        Some((name, rest)) => (Some(name.trim().to_string()), normalize_ws(rest)),
        None => (Some(body.to_string()), String::new()),
    }
}

/// Ten words before and five after a bare `[SCREENSHOT]`, so the popup can say
/// where in the document it sits.
fn bare_context(lines: &[&str], index: usize) -> String {
    const MARKER: &str = "[SCREENSHOT]";
    let line = lines[index];
    let at = line.find(MARKER).unwrap_or(0);

    let head_start = index.saturating_sub(3);
    let mut head = lines[head_start..index].join(" ");
    head.push(' ');
    head.push_str(&line[..at]);

    let tail_end = (index + 4).min(lines.len());
    let mut tail = line[at + MARKER.len()..].to_string();
    tail.push(' ');
    tail.push_str(&lines[index + 1..tail_end].join(" "));

    let head_words: Vec<&str> = head.split_whitespace().collect();
    let head = head_words[head_words.len().saturating_sub(10)..].join(" ");
    let tail = tail.split_whitespace().take(5).collect::<Vec<_>>().join(" ");

    normalize_ws(&format!("{head} […] {tail}"))
}

fn normalize_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ── Insertion ─────────────────────────────────────────────────────────────────

/// Path from a document's directory to the screenshot directory.
pub fn rel_shots_path(doc: &Path, root: &Path) -> String {
    let depth = doc
        .parent()
        .and_then(|dir| dir.strip_prefix(root).ok())
        .map(|rel| rel.components().count())
        .unwrap_or(1);
    let mut out = String::new();
    for _ in 0..depth {
        out.push_str("../");
    }
    out.push_str(SHOTS_REL);
    out
}

/// The two lines that replace a marker.
pub fn render_block(name: &str, rel: &str, instruction: &str) -> String {
    format!(
        "<!-- 📷 {name} — {} -->\n<p align=\"center\"><img src=\"{rel}/{name}\" alt=\"{}\" width=\"{DOC_WIDTH}\"></p>",
        comment_safe(instruction),
        alt_text(instruction, name),
    )
}

/// Replace `slot`'s lines with the image block, leaving the rest untouched.
pub fn apply(text: &str, slot: &Slot, name: &str, rel: &str) -> String {
    let trailing_newline = text.ends_with('\n');
    let mut out = String::with_capacity(text.len() + 256);
    for (index, line) in text.lines().enumerate() {
        if index == slot.line {
            out.push_str(&render_block(name, rel, &slot.context));
            out.push('\n');
            continue;
        }
        if index > slot.line && index < slot.line + slot.span {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !trailing_newline {
        out.pop();
    }
    out
}

/// An instruction safe to sit inside an HTML comment: `--` would close it early.
fn comment_safe(instruction: &str) -> String {
    let mut text = normalize_ws(instruction).replace("--", "–");
    if text.chars().count() > MAX_COMMENT {
        text = text.chars().take(MAX_COMMENT - 1).collect::<String>() + "…";
    }
    text
}

/// First sentence of the instruction, stripped of markdown and quote-safe.
fn alt_text(instruction: &str, fallback: &str) -> String {
    let plain = normalize_ws(instruction)
        .replace(['`', '*'], "")
        .replace('"', "'");
    let sentence = plain.split_once(". ").map(|(head, _)| head).unwrap_or(&plain);
    let sentence = sentence.trim_end_matches('.').trim();
    let text = if sentence.is_empty() {
        fallback.trim_end_matches(".png").replace(['-', '_'], " ")
    } else {
        sentence.to_string()
    };
    if text.chars().count() > MAX_ALT {
        text.chars().take(MAX_ALT - 1).collect::<String>() + "…"
    } else {
        text
    }
}

// ── Image ─────────────────────────────────────────────────────────────────────

/// Flatten the captured frame onto `backdrop` and write it as a PNG.
///
/// The IDE window is deliberately transparent (`clear_color = [0,0,0,0]`), so a
/// raw capture carries alpha everywhere the desktop shows through — glass panels
/// included. Left alone, the guide's own background would bleed through the
/// screenshot. Compositing over the theme's panel colour reproduces what the
/// operator actually sees, on an opaque surface.
///
/// Returns the saved dimensions.
pub fn save_png(image: &ColorImage, backdrop: Color32, path: &Path) -> Result<(u32, u32), String> {
    let [width, height] = image.size;
    let (width, height) = (width as u32, height as u32);
    if width == 0 || height == 0 {
        return Err("the capture came back empty".into());
    }

    let mut flat = Vec::with_capacity(image.pixels.len() * 4);
    for pixel in &image.pixels {
        // Color32 is premultiplied, so `src + bg * (1 - a)` is the plain
        // "over" composite — no un-premultiply step needed.
        let inverse = 255 - pixel.a() as u32;
        let blend = |channel: u8, under: u8| {
            (channel as u32 + under as u32 * inverse / 255).min(255) as u8
        };
        flat.push(blend(pixel.r(), backdrop.r()));
        flat.push(blend(pixel.g(), backdrop.g()));
        flat.push(blend(pixel.b(), backdrop.b()));
        flat.push(255);
    }

    let buffer = image::RgbaImage::from_raw(width, height, flat)
        .ok_or_else(|| "the capture did not match its reported size".to_string())?;
    let buffer = if width > MAX_WIDTH {
        let scaled = (height as f32 * MAX_WIDTH as f32 / width as f32).round() as u32;
        image::imageops::resize(
            &buffer,
            MAX_WIDTH,
            scaled.max(1),
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        buffer
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    buffer
        .save(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    Ok((buffer.width(), buffer.height()))
}

// ── State ─────────────────────────────────────────────────────────────────────

/// One loaded document and the slots found in it.
struct DocEntry {
    path: PathBuf,
    slots: Vec<Slot>,
}

impl DocEntry {
    fn label(&self) -> String {
        let name = self
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("(document)");
        let empty = self
            .slots
            .iter()
            .filter(|s| s.kind != SlotKind::Filled)
            .count();
        format!("{name}   ({empty} open / {} total)", self.slots.len())
    }
}

/// Capture state and the placement popup. Lives on the app; [`Self::poll`] runs
/// in every viewport, [`Self::ui`] only in the main window.
#[derive(Default)]
pub struct DocShots {
    /// The frame waiting to be placed.
    pending: Option<Arc<ColorImage>>,
    /// A Shift+F12 countdown, and the viewport that asked for it.
    delay: Option<(ViewportId, Instant)>,
    open: bool,
    /// The viewport that took the pending shot — and therefore the one that
    /// draws the placement popup. The popup used to live on the main window
    /// only, because a synchronous-looking `ViewportCommand::Screenshot` could
    /// have caught it in the frame being photographed. `screencapture` has
    /// already returned the PNG by the time this is set, so the shot cannot
    /// contain a popup that did not exist when it was taken — and the operator
    /// keeps the window they were working in (operator, 2026-09-04: "draw the
    /// popup in the window that took the shot").
    popup_viewport: Option<ViewportId>,
    docs: Vec<DocEntry>,
    selected: Option<usize>,
    /// File name for a bare `[SCREENSHOT]`, which names no target itself.
    name_input: String,
    status: Option<String>,
}

impl DocShots {
    /// `true` while a countdown is running — the caller shows it in the status bar.
    pub fn countdown(&self) -> Option<Duration> {
        let (_, at) = self.delay?;
        at.checked_duration_since(Instant::now())
    }

    /// Hotkeys and the capture reply, for one viewport, for one frame.
    ///
    /// Modifiers are matched exactly: `key_pressed(F12)` alone would also fire
    /// on Shift+F12 and take the instant shot the operator wanted delayed.
    pub fn poll(&mut self, ctx: &Context, enabled: bool) {
        if !enabled {
            return;
        }

        let (mut now, mut delayed) = (false, false);
        ctx.input(|input| {
            // NO `input.focused` GATE.
            //
            // It used to return early unless the viewport reported focus, and
            // that is what made F12 dead in the Form Designer while it worked
            // in the main window (operator, 2026-09-04: "Debug settings shows
            // the F12 is enabled, but RAD does not open it"). The flag is not
            // dependable for an immediate child viewport, and it was never
            // needed: the OS routes a key press to the FOCUSED window only, so
            // an F12 event sitting in this viewport's queue is already proof
            // that this viewport had focus. Asking twice, with the weaker
            // question second, could only ever lose events.
            for event in &input.events {
                if let egui::Event::Key {
                    key: Key::F12,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } = event
                {
                    if modifiers.ctrl || modifiers.alt || modifiers.command {
                        continue;
                    }
                    if modifiers.shift {
                        delayed = true;
                    } else {
                        now = true;
                    }
                }
            }
        });

        // The key arriving, and (in `take`) the picture that came back or the
        // reason none did. Enough to place a failure without reading code, now
        // that the capture is synchronous and there is no round trip to lose an
        // answer in. `COBOLT_LOG=doc_shots=info` to see them: this binary
        // filters on COBOLT_LOG, not RUST_LOG, and defaults to WARN.
        if now || delayed {
            tracing::info!(
                target: "doc_shots",
                "F12 seen (delayed={delayed}) viewport={:?} focused={}",
                ctx.viewport_id(),
                ctx.input(|i| i.focused),
            );
        }
        if now {
            self.take(ctx);
        }
        if delayed {
            self.delay = Some((ctx.viewport_id(), Instant::now() + DELAY));
        }

        // Fire a countdown only from the viewport that started it, so the shot
        // lands on the window the operator was arranging.
        if let Some((viewport, at)) = self.delay {
            if viewport == ctx.viewport_id() {
                if Instant::now() >= at {
                    self.delay = None;
                    self.take(ctx);
                } else {
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
            }
        }

    }

    /// Photograph THIS window and open the placement popup.
    ///
    /// Called by whichever viewport saw the key — the main window and every
    /// designer window run the identical path, each passing its own window.
    fn take(&mut self, ctx: &Context) {
        let Some(target) = WindowTarget::of(ctx) else {
            // The one path that used to fail in total silence: no capture, no
            // popup (it never claimed a viewport), and no log line. If a
            // window does not report `outer_rect`, say so where it can be
            // seen — and still claim the popup, so the message reaches the
            // operator instead of dying in a field nobody reads.
            let (rect, ppp) = ctx.input(|i| (i.viewport().outer_rect, i.pixels_per_point()));
            tracing::warn!(
                target: "doc_shots",
                "no capture: viewport={:?} reports outer_rect={rect:?} ppp={ppp}",
                ctx.viewport_id(),
            );
            self.status = Some(
                "✗ this window does not report its position, so there is \
                 nothing to photograph"
                    .into(),
            );
            self.open = true;
            self.popup_viewport = Some(ctx.viewport_id());
            return;
        };
        match capture_window(&target) {
            Ok(image) => {
                tracing::info!(
                    target: "doc_shots",
                    "captured viewport={:?} region={} -> {}x{}",
                    ctx.viewport_id(),
                    target.region(),
                    image.size[0],
                    image.size[1],
                );
                self.pending = Some(Arc::new(image));
                self.status = None;
                self.name_input.clear();
                self.refresh();
                self.open = true;
                self.popup_viewport = Some(ctx.viewport_id());
            }
            Err(message) => {
                // The failure is reported in the window that tried, for the
                // same reason the popup is: that is where the operator is.
                tracing::warn!(target: "doc_shots", "capture failed: {message}");
                self.status = Some(format!("✗ {message}"));
                self.open = true;
                self.popup_viewport = Some(ctx.viewport_id());
            }
        }
    }

    /// Re-read every English document and rescan its slots.
    fn refresh(&mut self) {
        self.docs.clear();
        self.selected = None;
        let Some(root) = repo_root() else { return };
        for path in english_docs(&root) {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let slots = scan(&text);
            if !slots.is_empty() {
                self.docs.push(DocEntry { path, slots });
            }
        }
        if self.docs.len() == 1 {
            self.selected = Some(0);
        }
    }

    /// The placement popup. Main window only — it must not appear in the very
    /// frame the operator is photographing.
    pub fn ui(&mut self, ctx: &Context, backdrop: Color32) {
        // Every viewport calls this; the one that took the shot draws it.
        if self.popup_viewport != Some(ctx.viewport_id()) {
            return;
        }
        if !self.open {
            return;
        }
        // A failed capture has a status and NO image. It still has to be shown:
        // silence is what made three of these faults look identical from the
        // outside (operator, 2026-09-04: "F12 is not working anywhere").
        if self.pending.is_none() {
            let mut open = true;
            egui::Window::new("📷 Documentation screenshot")
                .id(egui::Id::new("doc_shots_popup"))
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.label(
                        self.status
                            .clone()
                            .unwrap_or_else(|| "✗ the capture produced no image".into()),
                    );
                });
            if !open {
                self.open = false;
                self.status = None;
                self.popup_viewport = None;
            }
            return;
        }
        let mut open = true;
        let mut close = false;
        let size = self.pending.as_ref().map(|i| i.size).unwrap_or([0, 0]);

        egui::Window::new("📷 Documentation screenshot")
            .id(egui::Id::new("doc_shots_popup"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([700.0, 460.0])
            .show(ctx, |ui| {
                ui.label(format!("Captured {} × {} px.", size[0], size[1]));
                if repo_root().is_none() {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        "The documentation tree this build came from is gone. \
                         Set PRC_DOCS_ROOT to point at a checkout.",
                    );
                    return;
                }
                if self.docs.is_empty() {
                    ui.label("No English document holds a screenshot slot.");
                    return;
                }
                ui.separator();

                ui.label("Document");
                egui::ScrollArea::vertical()
                    .id_salt("doc_shots_docs")
                    .max_height(110.0)
                    .show(ui, |ui| {
                        for index in 0..self.docs.len() {
                            let label = self.docs[index].label();
                            if ui
                                .selectable_label(self.selected == Some(index), label)
                                .clicked()
                            {
                                self.selected = Some(index);
                            }
                        }
                    });

                let Some(doc_index) = self.selected else {
                    return;
                };
                ui.separator();
                ui.label("Slot — pick where this shot belongs");

                let mut chosen: Option<usize> = None;
                egui::ScrollArea::vertical()
                    .id_salt("doc_shots_slots")
                    .max_height(180.0)
                    .show(ui, |ui| {
                        for (index, slot) in self.docs[doc_index].slots.iter().enumerate() {
                            ui.horizontal_wrapped(|ui| {
                                if ui.button(slot.label()).clicked() {
                                    chosen = Some(index);
                                }
                                ui.label(
                                    egui::RichText::new(truncate(&slot.context, 150))
                                        .small()
                                        .weak(),
                                );
                            });
                            ui.add_space(2.0);
                        }
                    });

                let needs_name = self.docs[doc_index]
                    .slots
                    .iter()
                    .any(|slot| slot.name.is_none());
                if needs_name {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("File name for a [SCREENSHOT] slot:");
                        ui.text_edit_singleline(&mut self.name_input);
                    });
                }

                if let Some(slot_index) = chosen {
                    match self.insert(doc_index, slot_index, backdrop) {
                        Ok(message) => {
                            self.status = Some(message);
                            close = true;
                        }
                        Err(message) => self.status = Some(format!("✗ {message}")),
                    }
                }

                if let Some(status) = &self.status {
                    ui.separator();
                    ui.label(status.clone());
                }
            });

        if !open || close {
            self.open = false;
            self.pending = None;
            self.popup_viewport = None;
        }
    }

    /// Save the image and rewrite the document. Returns the report line.
    fn insert(
        &mut self,
        doc_index: usize,
        slot_index: usize,
        backdrop: Color32,
    ) -> Result<String, String> {
        let root = repo_root().ok_or("the documentation tree is gone")?;
        let image = self.pending.clone().ok_or("nothing captured")?;
        let entry = &self.docs[doc_index];
        let slot = &entry.slots[slot_index];

        let name = match &slot.name {
            Some(name) => name.clone(),
            None => {
                let typed = self.name_input.trim();
                if typed.is_empty() {
                    return Err("this slot names no file — type one first".into());
                }
                if typed.ends_with(".png") {
                    typed.to_string()
                } else {
                    format!("{typed}.png")
                }
            }
        };

        let target = root.join(SHOTS_REL).join(&name);
        let (width, height) = save_png(&image, backdrop, &target)?;

        let text = std::fs::read_to_string(&entry.path)
            .map_err(|error| format!("{}: {error}", entry.path.display()))?;
        // Rescan: the file may have been edited since the popup opened, which
        // would put the stored line numbers on the wrong lines.
        let fresh = scan(&text);
        let slot = fresh
            .get(slot_index)
            .filter(|candidate| candidate.name == slot.name && candidate.kind == slot.kind)
            .ok_or("the document changed while the popup was open — capture again")?;

        let updated = apply(&text, slot, &name, &rel_shots_path(&entry.path, &root));
        std::fs::write(&entry.path, updated)
            .map_err(|error| format!("{}: {error}", entry.path.display()))?;

        let doc_name = entry
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("the document");
        Ok(format!("✓ {name} ({width}×{height}) → {doc_name}"))
    }
}

fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    text.chars().take(limit.saturating_sub(1)).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUIDE: &str = "\
Intro line.

> 📷 **Screenshot needed — `install-launch.png`.** Please provide a capture of
> the application icon in your OS dock **and** the empty IDE window
> immediately after first launch (no project open).

Body text.
";

    #[test]
    fn a_quoted_placeholder_yields_its_name_and_whole_instruction() {
        let slots = scan(GUIDE);
        assert_eq!(slots.len(), 1);
        let slot = &slots[0];
        assert_eq!(slot.kind, SlotKind::Needed);
        assert_eq!(slot.name.as_deref(), Some("install-launch.png"));
        assert_eq!(slot.span, 3, "the whole block quote belongs to the slot");
        assert!(slot.context.starts_with("Please provide a capture"));
        assert!(slot.context.ends_with("(no project open)."));
    }

    #[test]
    fn the_period_may_sit_outside_the_backticks() {
        // Both spellings occur in the shipped guide.
        let slots = scan("> 📷 **Screenshot needed — `project-settings.png`**. Show the tree.");
        assert_eq!(slots[0].name.as_deref(), Some("project-settings.png"));
        assert_eq!(slots[0].context, "Show the tree.");
    }

    #[test]
    fn inserting_replaces_the_marker_and_keeps_the_rest() {
        let slots = scan(GUIDE);
        let out = apply(GUIDE, &slots[0], "install-launch.png", "../assets/images/screenshots");
        assert!(out.starts_with("Intro line.\n"));
        assert!(out.ends_with("Body text.\n"));
        assert!(!out.contains("Screenshot needed"));
        assert!(out.contains(
            "<img src=\"../assets/images/screenshots/install-launch.png\""
        ));
        assert!(out.contains("width=\"900\""));
    }

    #[test]
    fn a_filled_slot_stays_findable_so_the_shot_can_be_retaken() {
        let slots = scan(GUIDE);
        let filled = apply(GUIDE, &slots[0], "install-launch.png", "../assets/images/screenshots");
        let again = scan(&filled);
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].kind, SlotKind::Filled);
        assert_eq!(again[0].name.as_deref(), Some("install-launch.png"));
        assert!(again[0].context.starts_with("Please provide a capture"));
        // And retaking it must not stack a second image block.
        let twice = apply(&filled, &again[0], "install-launch.png", "../assets/images/screenshots");
        assert_eq!(twice.matches("<img").count(), 1);
    }

    #[test]
    fn a_bare_marker_reports_ten_words_before_and_five_after() {
        let text = "one two three four five six seven eight nine ten eleven \
                    [SCREENSHOT] alpha beta gamma delta epsilon zeta";
        let slots = scan(text);
        assert_eq!(slots[0].kind, SlotKind::Bare);
        assert!(slots[0].name.is_none());
        assert!(slots[0].context.starts_with("two three four"), "{}", slots[0].context);
        assert!(slots[0].context.contains("[…]"));
        assert!(slots[0].context.ends_with("alpha beta gamma delta epsilon"));
    }

    #[test]
    fn a_double_hyphen_can_never_close_the_html_comment_early() {
        let block = render_block("x.png", "../img", "before -- after");
        let comment = block.lines().next().unwrap();
        assert!(comment.ends_with("-->"));
        assert_eq!(comment.matches("-->").count(), 1);
    }

    #[test]
    fn alt_text_takes_the_first_sentence_and_drops_markdown() {
        let block = render_block("x.png", "../img", "Capture the `Form Designer`. Then more.");
        assert!(block.contains("alt=\"Capture the Form Designer\""), "{block}");
    }

    #[test]
    fn the_relative_path_climbs_out_of_the_document_directory() {
        let root = Path::new("/repo");
        assert_eq!(
            rel_shots_path(Path::new("/repo/docs/guide.md"), root),
            "../assets/images/screenshots"
        );
        assert_eq!(
            rel_shots_path(Path::new("/repo/docs/deep/guide.md"), root),
            "../../assets/images/screenshots"
        );
    }

    #[test]
    fn translated_guides_are_never_offered() {
        assert!(is_english_doc(Path::new("docs/developers-guide-en.md")));
        assert!(is_english_doc(Path::new("docs/observability-en.md")));
        for translated in ["-es", "-pt", "-jp", "-cn", "-fr"] {
            let path = format!("docs/developers-guide{translated}.md");
            assert!(!is_english_doc(Path::new(&path)), "{path} must be skipped");
        }
    }

    #[test]
    fn an_unquoted_definition_list_marker_takes_the_lines_below_it() {
        // The prose here contains its own `**bold**`, which must not be read as
        // the end of the marker.
        let text = "\
Lead in.

📷 Screenshot needed — `toolbar-editor.png`
: Open a form with a ToolBar, press **Edit Toolbar…**, and build two groups —
  one with three icon buttons.

Next section.
";
        let slots = scan(text);
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].name.as_deref(), Some("toolbar-editor.png"));
        assert_eq!(slots[0].span, 3, "the instruction lines belong to the slot");
        assert!(
            slots[0].context.starts_with("Open a form with a ToolBar"),
            "{}",
            slots[0].context
        );

        let out = apply(text, &slots[0], "toolbar-editor.png", "../img");
        assert!(out.starts_with("Lead in.\n"));
        assert!(out.ends_with("Next section.\n"));
        assert!(!out.contains("Screenshot needed"));
        // The instruction survives only in the two generated lines (comment and
        // alt); no continuation line may be left orphaned under the image.
        let carriers: Vec<&str> = out.lines().filter(|l| l.contains("Edit Toolbar")).collect();
        assert_eq!(carriers.len(), 2, "{carriers:?}");
        assert!(carriers[0].starts_with("<!-- 📷 "), "{}", carriers[0]);
        assert!(carriers[1].starts_with("<p align=\"center\">"), "{}", carriers[1]);
    }

    #[test]
    fn an_em_dash_separator_is_not_part_of_the_instruction() {
        let slots = scan("> 📷 **Screenshot needed — `indexed.png`** — Indexed File Editor.");
        assert_eq!(slots[0].name.as_deref(), Some("indexed.png"));
        assert_eq!(slots[0].context, "Indexed File Editor.");
    }

    #[test]
    fn a_file_name_without_backticks_is_still_found() {
        let slots = scan("📷 Screenshot needed — project-crates-dialog.png (the dialog)");
        assert_eq!(slots[0].name.as_deref(), Some("project-crates-dialog.png"));
        assert_eq!(slots[0].context, "(the dialog)");
    }

    #[test]
    fn every_marker_in_the_shipped_guide_parses() {
        // The synthetic fixtures above prove the rules; this proves the rules
        // match what the guide actually contains. Vacuously true once every
        // slot is filled, so it never blocks finishing the job.
        let Some(root) = repo_root() else { return };
        let path = root.join("docs/developers-guide-en.md");
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let slots = scan(&text);
        let raw = text.matches("Screenshot needed").count();
        let parsed = slots
            .iter()
            .filter(|slot| slot.kind == SlotKind::Needed)
            .count();
        assert_eq!(parsed, raw, "every placeholder must become exactly one slot");
        for slot in slots.iter().filter(|s| s.kind == SlotKind::Needed) {
            let name = slot.name.as_deref().unwrap_or_default();
            assert!(name.ends_with(".png"), "line {}: no target name", slot.line + 1);
            assert!(
                !slot.context.is_empty(),
                "line {}: {name} has no instruction",
                slot.line + 1
            );
        }
    }

    /// **Any English Markdown document, not just the guide** (operator,
    /// 2026-09-03). `README.md` is the case that motivated it: it does not live
    /// under `docs/`, so the old single `read_dir("docs")` could never see it,
    /// and a document in a `docs/` subdirectory was missed for the same reason
    /// — the scan did not recurse.
    #[test]
    fn the_scan_reaches_the_readme_and_nested_docs_but_not_the_source_tree() {
        let dir = std::env::temp_dir().join("prc_doc_shots_scope_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("docs/deep")).unwrap();
        std::fs::create_dir_all(dir.join("crates/cobolt-ide/src")).unwrap();

        std::fs::write(dir.join("README.md"), "# readme").unwrap();
        std::fs::write(dir.join("CHANGELOG.md"), "# changelog").unwrap();
        std::fs::write(dir.join("docs/developers-guide-en.md"), "# guide").unwrap();
        std::fs::write(dir.join("docs/deep/nested-en.md"), "# nested").unwrap();
        // Translations stay out — the images are language-neutral and the
        // translated guides reference the same files (GOLDEN RULE #3).
        std::fs::write(dir.join("docs/developers-guide-pt.md"), "# pt").unwrap();
        // Source-tree markdown is not documentation a screenshot belongs in,
        // and the top level is deliberately not recursed so it stays out.
        std::fs::write(dir.join("crates/cobolt-ide/src/notes.md"), "# notes").unwrap();

        let found: Vec<String> = english_docs(&dir)
            .iter()
            .map(|p| {
                p.strip_prefix(&dir)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();

        assert!(
            found.contains(&"README.md".to_string()),
            "README.md must be reachable: {found:?}"
        );
        assert!(
            found.contains(&"docs/deep/nested-en.md".to_string()),
            "a nested doc must be reachable: {found:?}"
        );
        assert!(
            found.contains(&"docs/developers-guide-en.md".to_string()),
            "the guide must still be there: {found:?}"
        );
        assert!(
            !found.iter().any(|p| p.ends_with("-pt.md")),
            "translations must stay out: {found:?}"
        );
        assert!(
            !found.iter().any(|p| p.starts_with("crates/")),
            "the source tree must stay out: {found:?}"
        );

        // A root-level document reaches the images without climbing out of a
        // directory it is not in.
        assert_eq!(
            rel_shots_path(&dir.join("README.md"), &dir),
            "assets/images/screenshots"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **`README.md` must actually reach the picker** (operator, 2026-09-04:
    /// "I still can't see the other documents" … "README.md has [SCREENSHOT]").
    ///
    /// Two conditions have to hold together, and each failed at some point:
    /// the scan must reach a file that is **not** under `docs/` (it did a plain
    /// `read_dir("docs")` until 1.64.5), and the picker only lists a document
    /// that holds at least one slot — so this asserts the README's markers are
    /// recognised too. They are the bare `[SCREENSHOT]` spelling, not the `📷`
    /// one, which is precisely what a `grep` for the camera misses.
    #[test]
    fn the_readme_is_offered_with_its_screenshot_slots() {
        let Some(root) = repo_root() else { return };
        let docs = english_docs(&root);
        assert!(
            docs.iter()
                .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("README.md")),
            "README.md must be among the documents the picker scans: {:?}",
            docs.iter().filter_map(|p| p.file_name()).collect::<Vec<_>>()
        );

        let readme = root.join("README.md");
        let Ok(text) = std::fs::read_to_string(&readme) else {
            return;
        };
        let slots = scan(&text);
        assert!(
            !slots.is_empty(),
            "README.md carries [SCREENSHOT] markers, so the picker must offer it — \
             a document with no slots is skipped"
        );
        // Its images sit beside it, not one directory up.
        assert_eq!(rel_shots_path(&readme, &root), SHOTS_REL);
    }

    /// **No permission means no picture — never a picture of the desktop.**
    ///
    /// macOS answers a capture from a process without Screen Recording
    /// permission with a valid PNG of the wallpaper, every window omitted, and
    /// `screencapture` exits 0. Two of those reached the repository before this
    /// gate existed, both exactly the window's own size, which is why neither
    /// the exit code nor the region could have caught it.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_capture_without_permission_is_refused_rather_than_faked() {
        let target = WindowTarget {
            rect: egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(1280.0, 828.0)),
            scale: 2.0,
        };
        let message = capture_window_when(false, &target)
            .expect_err("a capture with no permission must fail, not return the desktop");
        for expected in [
            "Screen Recording",     // the setting, by its exact name
            "System Settings",      // where it lives
            "STARTED PowerRustCOBOL", // whose permission it is
            "Terminal",             // and the usual answer to that
        ] {
            assert!(
                message.contains(expected),
                "the refusal must name {expected:?}, said: {message}"
            );
        }
    }

    /// The region handed to `screencapture`, rounded **outwards**.
    ///
    /// A window edge lands on a fractional point often enough (HiDPI, a dragged
    /// window), and rounding to nearest would shave a pixel column off one side
    /// of every such shot. Growing the rectangle can only pick up a pixel of
    /// desktop, which is invisible against the window's own border; shrinking it
    /// cuts the content.
    #[test]
    fn the_capture_region_rounds_outwards() {
        let target = WindowTarget {
            rect: egui::Rect::from_min_size(egui::pos2(100.4, 50.6), egui::vec2(1200.2, 800.9)),
            scale: 2.0,
        };
        assert_eq!(target.region(), "100,50,1201,801");

        // A whole-number rectangle is passed through untouched.
        let exact = WindowTarget {
            rect: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 800.0)),
            scale: 1.0,
        };
        assert_eq!(exact.region(), "0,0,1280,800");

        // A degenerate rectangle still names at least one pixel rather than
        // asking the tool for a zero-sized region.
        let sliver = WindowTarget {
            rect: egui::Rect::from_min_size(egui::pos2(5.0, 5.0), egui::vec2(0.0, 0.0)),
            scale: 1.0,
        };
        assert_eq!(sliver.region(), "5,5,1,1");
    }

    /// egui reports window rectangles in its own points — screen points
    /// divided by the context-wide zoom factor (egui-winit
    /// `outer_rect_in_points`). Anything that moves that factor — the doc
    /// viewer's Zoom In/Out, egui's Cmd+= / Cmd+- — used to move the region
    /// F12 asked `screencapture` for: at zoom 0.5 the region was twice the
    /// window, and the shot was mostly desktop.
    #[test]
    fn the_capture_region_is_in_screen_points_whatever_the_zoom() {
        fn target_at(zoom: f32) -> WindowTarget {
            let ctx = Context::default();
            ctx.set_zoom_factor(zoom);
            // A window at screen (100, 50), 1200×800 points, on a 2× display —
            // reported the way egui-winit reports it, divided by the zoom.
            let input = || egui::RawInput {
                viewport_id: ViewportId::ROOT,
                viewports: std::iter::once((
                    ViewportId::ROOT,
                    egui::ViewportInfo {
                        outer_rect: Some(
                            egui::Rect::from_min_size(
                                egui::pos2(100.0, 50.0),
                                egui::vec2(1200.0, 800.0),
                            ) / zoom,
                        ),
                        native_pixels_per_point: Some(2.0),
                        ..Default::default()
                    },
                ))
                .collect(),
                ..Default::default()
            };
            // The zoom takes effect at the start of the next pass. Each pass
            // hands back font-texture deltas that egui insists are handled;
            // there is no painter here, so they are dropped on purpose.
            ctx.run_ui(input(), |_| {}).textures_delta.clear();
            let mut seen = None;
            ctx.run_ui(input(), |ui| seen = WindowTarget::of(ui.ctx()))
                .textures_delta
                .clear();
            seen.expect("a viewport that reports its rectangle yields a target")
        }
        for zoom in [1.0, 0.5, 1.5, 1.1_f32.powi(3)] {
            let target = target_at(zoom);
            assert_eq!(target.region(), "100,50,1200,800", "at zoom {zoom}");
            assert_eq!(target.scale, 2.0, "at zoom {zoom}");
        }
    }

    #[test]
    fn flattening_removes_every_trace_of_transparency() {
        // A fully transparent pixel must come back as the backdrop, not a hole:
        // the guide's own background would otherwise show through the glass.
        let image = ColorImage {
            size: [2, 1],
            source_size: egui::Vec2::new(2.0, 1.0),
            pixels: vec![Color32::TRANSPARENT, Color32::from_rgb(200, 100, 50)],
        };
        let dir = std::env::temp_dir().join("prc_doc_shots_test");
        let path = dir.join("flat.png");
        let backdrop = Color32::from_rgb(20, 30, 40);
        let (width, height) = save_png(&image, backdrop, &path).expect("save");
        assert_eq!((width, height), (2, 1));

        let saved = image::open(&path).expect("read back").to_rgba8();
        assert_eq!(saved.get_pixel(0, 0).0, [20, 30, 40, 255]);
        assert_eq!(saved.get_pixel(1, 0).0[3], 255);
        let _ = std::fs::remove_file(&path);
    }
}
