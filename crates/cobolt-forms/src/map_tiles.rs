// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Hand-rolled OpenStreetMap slippy-map tile rendering (spec 039).
//!
//! `egui-map-view` — the crate first chosen for this — turned out to be
//! hard-pinned to an exact, non-unifiable `egui`/`eframe` version (T1's
//! spike, recorded in `specs/039-…/plan.md` §4 Decision 1): its `Map`
//! widget cannot type-check against this workspace's egui 0.35 at all.
//! Rather than run a second, nested egui context just to host that one
//! widget, this module renders OpenStreetMap tiles directly: the standard
//! Web Mercator projection (the same formula every slippy-map library
//! uses — <https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames>), a
//! background-thread tile fetcher, and an `egui::TextureHandle` cache.
//!
//! The cache is process-global and keyed by `(zoom, x, y)` only — not by
//! which `Maps` control asked for a tile — so two Maps controls (or the
//! designer canvas and a live preview of the same form) showing overlapping
//! territory share one download and one texture, the same way a browser's
//! tile cache does.

use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::sync::{Mutex, OnceLock};

/// OSM's own tile pixel size — fixed by the protocol, not configurable.
pub const TILE_SIZE: f64 = 256.0;
pub const MIN_ZOOM: u8 = 0;
pub const MAX_ZOOM: u8 = 19;

/// A descriptive User-Agent, per OSM's tile usage policy
/// (<https://operations.osmfoundation.org/policies/tiles/>) — the default
/// User-Agent a bare HTTP client sends is explicitly disallowed there.
const USER_AGENT: &str = "PowerRustCOBOL-IDE/1.0 (+https://github.com/CloudLandBeta/PowerRustCOBOL)";

/// lat/lng (degrees) + zoom -> fractional tile coordinates. The integer part
/// is the tile index; the fractional part is the pixel offset within it —
/// both are needed to place a marker or a viewport with sub-tile precision.
pub fn lat_lng_to_tile_frac(lat: f64, lng: f64, zoom: u8) -> (f64, f64) {
    lat_lng_to_tile_frac_at(lat, lng, zoom as f64)
}

/// The same projection at a **continuous** zoom.
///
/// Smooth zooming lives between whole levels, so the geometry has to: at
/// zoom 12.4 the world is `2^12.4` tiles across, not 2^12. Every integer-zoom
/// helper is a thin wrapper over this one, so a fractional viewport and a whole
/// one cannot drift apart.
pub fn lat_lng_to_tile_frac_at(lat: f64, lng: f64, zoom: f64) -> (f64, f64) {
    let lat = lat.clamp(-85.051_128, 85.051_128); // Web Mercator's own valid range
    let lat_rad = lat.to_radians();
    let n = 2f64.powf(zoom);
    let x = (lng + 180.0) / 360.0 * n;
    let y = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI) / 2.0 * n;
    (x, y)
}

/// The inverse of [`lat_lng_to_tile_frac`] — a fractional tile coordinate
/// back to lat/lng. Used to turn a drag delta (in tiles) back into a new
/// `Center`.
pub fn tile_frac_to_lat_lng(x: f64, y: f64, zoom: u8) -> (f64, f64) {
    tile_frac_to_lat_lng_at(x, y, zoom as f64)
}

/// [`tile_frac_to_lat_lng`] at a continuous zoom.
pub fn tile_frac_to_lat_lng_at(x: f64, y: f64, zoom: f64) -> (f64, f64) {
    let n = 2f64.powf(zoom);
    let lng = x / n * 360.0 - 180.0;
    let lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * y / n)).sinh().atan();
    (lat_rad.to_degrees(), lng)
}

/// Screen-pixel offset of lat/lng from a viewport centred on `center_*` at
/// `zoom`, with (0,0) at the viewport's own centre and +y down — the same
/// convention `egui::Rect` uses, so a caller can add this directly to
/// `rect.center()`.
pub fn lat_lng_to_offset(
    lat: f64,
    lng: f64,
    center_lat: f64,
    center_lng: f64,
    zoom: u8,
) -> (f32, f32) {
    lat_lng_to_offset_at(lat, lng, center_lat, center_lng, zoom as f64)
}

/// [`lat_lng_to_offset`] at a continuous zoom — what a marker, a route vertex
/// or a region outline needs while the map is between levels, so the overlay
/// stays pinned to the ground as the tiles scale under it.
pub fn lat_lng_to_offset_at(
    lat: f64,
    lng: f64,
    center_lat: f64,
    center_lng: f64,
    zoom: f64,
) -> (f32, f32) {
    let (tx, ty) = lat_lng_to_tile_frac_at(lat, lng, zoom);
    let (ctx, cty) = lat_lng_to_tile_frac_at(center_lat, center_lng, zoom);
    (
        ((tx - ctx) * TILE_SIZE) as f32,
        ((ty - cty) * TILE_SIZE) as f32,
    )
}

/// Scroll pixels that make up one whole zoom level.
///
/// Zoom used to be one level per scroll EVENT, with no accumulation: a mouse
/// notch and the dozens of small deltas a trackpad flick emits counted the
/// same, so one gesture crossed five or six levels and the map was impossible
/// to aim (operator, 2026-08-20). Counting pixels instead makes both devices
/// agree — a notch is ~50 px, a flick is the sum of its parts.
pub const SCROLL_PER_ZOOM: f32 = 90.0;

/// How many zoom levels a single frame may apply, however hard the wheel is
/// spun. The accumulator survives to the next frame, so nothing is lost — but
/// the map never leaps, which is the whole complaint.
pub const MAX_ZOOM_STEP_PER_FRAME: i32 = 1;

/// Fold this frame's scroll into the accumulator and report how many whole
/// zoom levels come out of it.
///
/// Returns `(levels, remaining accumulator)`. The remainder is kept, so slow
/// scrolling still gets there and a fast one does not overshoot.
pub fn zoom_steps(accumulated: f32, scroll: f32) -> (i32, f32) {
    if !accumulated.is_finite() || !scroll.is_finite() {
        return (0, 0.0);
    }
    // A reversal starts from scratch. Credit built up scrolling one way must
    // not have to be spent before the other way answers — pushing back should
    // zoom back, not first undo an invisible balance.
    let base = if scroll != 0.0 && accumulated != 0.0 && accumulated.signum() != scroll.signum() {
        0.0
    } else {
        accumulated
    };
    let mut acc = base + scroll;
    let mut levels = (acc / SCROLL_PER_ZOOM).trunc() as i32;
    if levels == 0 {
        return (0, acc);
    }
    levels = levels.clamp(-MAX_ZOOM_STEP_PER_FRAME, MAX_ZOOM_STEP_PER_FRAME);
    acc -= levels as f32 * SCROLL_PER_ZOOM;
    (levels, acc)
}

/// The most zoom, in whole levels, one frame may apply while gliding.
///
/// At 60 fps this crosses a level in about eight frames (~0.13 s) — fast enough
/// to feel immediate, slow enough that the eye follows the scale instead of
/// being handed a new picture.
pub const MAX_ZOOM_PER_FRAME: f32 = 0.125;

/// Below this the pending zoom is spent: a remainder too small to see would
/// otherwise keep requesting repaints forever.
const ZOOM_GLIDE_EPSILON: f32 = 0.0005;

/// Fold this frame's scroll into the pending zoom and release a slice of it.
///
/// Returns `(levels to apply now, still pending)`, both **fractional**. This is
/// the continuous counterpart of [`zoom_steps`], which could only ever hand back
/// whole levels — and a whole level is a factor of two, so every notch of the
/// wheel replaced the picture rather than growing it. Nothing about the input
/// changes: the same `SCROLL_PER_ZOOM` pixels still buy the same one level, and
/// the same reversal rule still applies. What changes is that the level arrives
/// in slices the eye can follow.
///
/// The caller keeps repainting while the returned remainder is non-zero; that
/// is what makes a single flick glide to a stop instead of landing in one jump.
pub fn zoom_glide(pending: f32, scroll: f32) -> (f32, f32) {
    if !pending.is_finite() || !scroll.is_finite() {
        return (0.0, 0.0);
    }
    // Same rule as `zoom_steps`: pushing the other way answers immediately
    // rather than first spending credit built up in the first direction.
    let base = if scroll != 0.0 && pending != 0.0 && pending.signum() != scroll.signum() {
        0.0
    } else {
        pending
    };
    let want = base + scroll / SCROLL_PER_ZOOM;
    if want.abs() < ZOOM_GLIDE_EPSILON {
        return (0.0, 0.0);
    }
    let step = want.clamp(-MAX_ZOOM_PER_FRAME, MAX_ZOOM_PER_FRAME);
    let left = want - step;
    (
        step,
        if left.abs() < ZOOM_GLIDE_EPSILON {
            0.0
        } else {
            left
        },
    )
}

/// Split a continuous zoom into the level whose TILES are fetched and the
/// fractional offset the painter scales them by.
///
/// The nearest whole level, so the tiles on screen are never scaled by more
/// than √2 in either direction and stay sharp: at 12.4 the map draws level 12's
/// tiles 32 % larger, and at 12.6 it draws level 13's 30 % smaller.
pub fn split_zoom(zoom: f32) -> (u8, f32) {
    let z = zoom.clamp(MIN_ZOOM as f32, MAX_ZOOM as f32);
    let level = z.round().clamp(MIN_ZOOM as f32, MAX_ZOOM as f32) as u8;
    (level, z - level as f32)
}

/// Re-centre so the coordinate under `anchor` (a pixel offset from the
/// viewport centre) stays under it across a zoom change.
///
/// Zooming used to leave the centre alone, so whatever you were pointing at
/// slid away as the scale changed and you had to chase it — the other half of
/// "impossible to control". This is the standard behaviour of every slippy map:
/// the cursor is the fixed point.
pub fn zoom_about(
    center_lat: f64,
    center_lng: f64,
    from_zoom: u8,
    to_zoom: u8,
    anchor_dx: f32,
    anchor_dy: f32,
) -> (f64, f64) {
    zoom_about_at(
        center_lat,
        center_lng,
        from_zoom as f64,
        to_zoom as f64,
        anchor_dx,
        anchor_dy,
    )
}

/// [`zoom_about`] between continuous zooms — the one a glide actually uses,
/// since every frame of it lands between levels.
pub fn zoom_about_at(
    center_lat: f64,
    center_lng: f64,
    from_zoom: f64,
    to_zoom: f64,
    anchor_dx: f32,
    anchor_dy: f32,
) -> (f64, f64) {
    if from_zoom == to_zoom {
        return (center_lat, center_lng);
    }
    // What is under the cursor now…
    let (anchor_lat, anchor_lng) =
        offset_to_lat_lng_at(anchor_dx, anchor_dy, center_lat, center_lng, from_zoom);
    // …and the centre that keeps it there at the new scale.
    let (ax, ay) = lat_lng_to_tile_frac_at(anchor_lat, anchor_lng, to_zoom);
    let cx = ax - anchor_dx as f64 / TILE_SIZE;
    let cy = ay - anchor_dy as f64 / TILE_SIZE;
    tile_frac_to_lat_lng_at(cx, cy, to_zoom)
}

/// The inverse of [`lat_lng_to_offset`] — a screen-pixel offset from the
/// viewport centre back to lat/lng. Used to resolve a click/marker-overlay
/// hit test into a real coordinate.
pub fn offset_to_lat_lng(
    dx: f32,
    dy: f32,
    center_lat: f64,
    center_lng: f64,
    zoom: u8,
) -> (f64, f64) {
    offset_to_lat_lng_at(dx, dy, center_lat, center_lng, zoom as f64)
}

/// [`offset_to_lat_lng`] at a continuous zoom, so a drag or a click resolves
/// against the scale the map is actually drawn at rather than the level whose
/// tiles it borrowed.
pub fn offset_to_lat_lng_at(
    dx: f32,
    dy: f32,
    center_lat: f64,
    center_lng: f64,
    zoom: f64,
) -> (f64, f64) {
    let (ctx, cty) = lat_lng_to_tile_frac_at(center_lat, center_lng, zoom);
    let tx = ctx + dx as f64 / TILE_SIZE;
    let ty = cty + dy as f64 / TILE_SIZE;
    tile_frac_to_lat_lng_at(tx, ty, zoom)
}

type TileKey = (u8, i64, i64);

enum TileSlot {
    /// A background thread is fetching this tile; poll the receiver.
    Loading(Receiver<Option<Vec<u8>>>),
    Ready(egui::TextureHandle),
    Failed,
}

fn cache() -> &'static Mutex<HashMap<TileKey, TileSlot>> {
    static CACHE: OnceLock<Mutex<HashMap<TileKey, TileSlot>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// How many levels up a missing tile may borrow from. Five is a 32× blow-up —
/// past that the stand-in is a smear of four pixels and the honest backdrop
/// reads better.
const MAX_ANCESTOR_LEVELS: u8 = 5;

/// Which part of an ancestor `n` levels up this tile covers, as UV.
///
/// Each level halves the square: one level up, a tile is one of the ancestor's
/// four quadrants; two levels up, one of sixteen. Which one is written in the
/// low bits of its own x/y — `x & (2^n − 1)` is its column inside the ancestor.
fn ancestor_uv(x: i64, y: i64, n: u8) -> egui::Rect {
    let span = 1i64 << n;
    let step = 1.0 / span as f32;
    let cx = x.rem_euclid(span) as f32 * step;
    let cy = y.rem_euclid(span) as f32 * step;
    egui::Rect::from_min_max(
        egui::pos2(cx, cy),
        egui::pos2(cx + step, cy + step),
    )
}

/// Paint a stand-in for a tile that has not arrived, out of the tiles that
/// **have** — the way every map client covers the same gap.
///
/// A zoom used to blank to a flat grey block and then snap to the new image.
/// The map already holds a picture of that ground, at another scale (operator,
/// 2026-08-22), so:
///
/// * **Zooming in**, the nearest loaded ANCESTOR is magnified and cropped to
///   the quadrant this tile covers — the ground stays where the eye left it and
///   simply sharpens when the real tile lands.
/// * **Zooming out**, the four CHILDREN are drawn shrunk into their quarters —
///   the level being left is still in the cache, so the new one fills in over a
///   picture rather than over grey. Whichever children are ready are drawn;
///   partial cover still beats none.
///
/// Ancestors are tried first: one draw, and one level up is only a 2× blow-up,
/// while children cost four draws and are often only partly present.
///
/// Returns whether anything was painted.
fn draw_tile_stand_in(
    map: &HashMap<TileKey, TileSlot>,
    painter: &egui::Painter,
    dest: egui::Rect,
    (z, x, y): TileKey,
) -> bool {
    let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    for n in 1..=MAX_ANCESTOR_LEVELS.min(z) {
        let key = (z - n, x >> n, y >> n);
        if let Some(TileSlot::Ready(tex)) = map.get(&key) {
            painter.image(tex.id(), dest, ancestor_uv(x, y, n), egui::Color32::WHITE);
            return true;
        }
    }
    if z >= MAX_ZOOM {
        return false;
    }
    let half = dest.size() * 0.5;
    let mut drew = false;
    for (i, j) in [(0i64, 0i64), (1, 0), (0, 1), (1, 1)] {
        let key = (z + 1, x * 2 + i, y * 2 + j);
        if let Some(TileSlot::Ready(tex)) = map.get(&key) {
            let quarter = egui::Rect::from_min_size(
                dest.min + egui::vec2(i as f32 * half.x, j as f32 * half.y),
                half,
            );
            painter.image(tex.id(), quarter, full, egui::Color32::WHITE);
            drew = true;
        }
    }
    drew
}

/// Start a background fetch for one tile, unless one is already in flight
/// or has already resolved. Mirrors `file_dialog.rs`'s `begin`/`take`
/// shape: never blocks the calling (paint) frame, wakes the context when
/// the download finishes so the next frame picks up the result.
fn request_tile(ctx: &egui::Context, key: TileKey) {
    let mut map = cache().lock().unwrap();
    if map.contains_key(&key) {
        return;
    }
    let (zoom, x, y) = key;
    let (tx, rx) = std::sync::mpsc::channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let url = format!("https://tile.openstreetmap.org/{zoom}/{x}/{y}.png");
        let result = agent()
            .get(&url)
            .set("User-Agent", USER_AGENT)
            .call()
            .map_err(|e| report_tile_failure(&e))
            .ok()
            .and_then(|resp| {
                let mut bytes = Vec::new();
                resp.into_reader().read_to_end(&mut bytes).ok()?;
                Some(bytes)
            });
        let _ = tx.send(result);
        ctx.request_repaint();
    });
    map.insert(key, TileSlot::Loading(rx));
}

/// One shared OS-TLS connector for the tile fetch.
///
/// `ureq`'s `native-tls` feature is an **adapter only**: the crate-level
/// helpers (`ureq::get`, …) and a bare `AgentBuilder` never pick it up, so
/// every HTTPS call has to go through an agent carrying this connector or it
/// fails with "no TLS backend". This file used `ureq::get` directly, so every
/// tile request failed before it left the machine and the map drew as a grey
/// square with nothing on it but the markers (operator, 2026-08-20).
///
/// The same rule, and the same reason for choosing native-tls over rustls, as
/// `cobolt_runtime::http_runtime` — which documented it and was the only place
/// obeying it.
fn tls_connector() -> Option<std::sync::Arc<native_tls::TlsConnector>> {
    static CONNECTOR: OnceLock<Option<std::sync::Arc<native_tls::TlsConnector>>> = OnceLock::new();
    CONNECTOR
        .get_or_init(|| native_tls::TlsConnector::new().ok().map(std::sync::Arc::new))
        .clone()
}

/// The agent every tile request runs through.
fn agent() -> ureq::Agent {
    let mut builder = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(15));
    if let Some(connector) = tls_connector() {
        builder = builder.tls_connector(connector);
    }
    builder.build()
}

/// Say — **once** — why the basemap is not appearing.
///
/// A tile that cannot be fetched is indistinguishable from a map centred on
/// open water: both are a flat grey square. That silence is what made a
/// misconfigured HTTP client look like a working control with nothing to show,
/// so the first failure of a session is reported. Once, because a map covers
/// its viewport in tiles and every one of them fails together — a line each
/// would be a flood saying one thing.
fn report_tile_failure(err: &ureq::Error) {
    static SAID: OnceLock<()> = OnceLock::new();
    if SAID.set(()).is_ok() {
        eprintln!(
            "[prc] map basemap unavailable: {err}\n\
             [prc]   tiles come from tile.openstreetmap.org over HTTPS and need no API key;\n\
             [prc]   markers and the centre still work, but the map draws blank without them."
        );
    }
}

/// Poll every in-flight download, decoding+uploading any that finished this
/// frame. Cheap when nothing is pending (a non-blocking `try_recv` per key).
fn poll_tiles(ctx: &egui::Context) {
    let mut map = cache().lock().unwrap();
    let mut resolved: Vec<(TileKey, TileSlot)> = Vec::new();
    for (&key, slot) in map.iter() {
        if let TileSlot::Loading(rx) = slot {
            match rx.try_recv() {
                Ok(Some(bytes)) => {
                    let decoded = image::load_from_memory(&bytes).ok().map(|img| {
                        let rgba = img.to_rgba8();
                        let (w, h) = rgba.dimensions();
                        egui::ColorImage::from_rgba_unmultiplied(
                            [w as usize, h as usize],
                            rgba.as_raw(),
                        )
                    });
                    let slot = match decoded {
                        Some(color_image) => {
                            let tex = ctx.load_texture(
                                format!("osm-tile-{}-{}-{}", key.0, key.1, key.2),
                                color_image,
                                egui::TextureOptions::LINEAR,
                            );
                            TileSlot::Ready(tex)
                        }
                        None => TileSlot::Failed,
                    };
                    resolved.push((key, slot));
                }
                Ok(None) => resolved.push((key, TileSlot::Failed)),
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    resolved.push((key, TileSlot::Failed))
                }
            }
        }
    }
    for (key, slot) in resolved {
        map.insert(key, slot);
    }
}

/// One marker overlaid on the map: a real-world position plus whatever the
/// caller wants back from a hit test (an id, typically).
pub struct MapMarker<'a> {
    pub lat: f64,
    pub lng: f64,
    pub label: &'a str,
    /// Which marker this is — needed to keep a card open across frames, since
    /// the open one is remembered by id (`SelectedMarkerId`), not by index.
    pub id: &'a str,
    /// The body of the click card. Carried in the `Markers` property since it
    /// was written and, until the info window existed, never displayed.
    pub info: &'a str,
}

/// Where the pointer is over the map this frame.
#[derive(Clone, Copy, Default)]
pub struct MapPointer {
    /// The pointer's position while it is over the map, for the hover tooltip.
    pub hover: Option<egui::Pos2>,
    /// Where a click landed this frame, if one did.
    pub click: Option<egui::Pos2>,
}

/// What the pointer was over. Markers win ties with regions — a pin is small,
/// deliberately aimed at, and usually sits inside a territory.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct MapHit {
    pub hovered_marker: Option<usize>,
    pub hovered_region: Option<usize>,
    pub clicked_marker: Option<usize>,
    pub clicked_region: Option<usize>,
}

/// How the info window is painted.
///
/// Every colour defaults to the form's own theme and each can be overridden on
/// its own (`InfoBackgroundColor` and friends), so a map matches the form it
/// sits on without being told to, and can still be restyled when it has to be.
#[derive(Clone, Copy)]
pub struct InfoStyle {
    pub bg: egui::Color32,
    pub fg: egui::Color32,
    pub border: egui::Color32,
    pub corner: f32,
    pub shadow: bool,
    pub font_size: f32,
}

impl Default for InfoStyle {
    fn default() -> Self {
        Self {
            bg: egui::Color32::from_rgb(252, 252, 253),
            fg: egui::Color32::from_rgb(28, 30, 34),
            border: egui::Color32::from_rgb(206, 210, 218),
            corner: 8.0,
            shadow: true,
            font_size: 13.0,
        }
    }
}

/// Ink that is readable ON `bg` — near-black over a light card, near-white over
/// a dark one.
///
/// The info window used to take its text colour from the control's
/// ForegroundColor and its background from the control's BackgroundColor,
/// INDEPENDENTLY — so nothing made the two contrast. On a dark-themed form
/// whose map carried a white foreground, the card came out white-on-light and
/// could not be read at all (operator, 2026-08-21).
///
/// Deriving the ink from the background that was actually chosen makes the
/// window legible whatever that background turns out to be, which no amount of
/// inheriting two colours separately can promise. An explicit
/// `InfoForegroundColor` still wins — this is the DEFAULT, not a clamp.
pub fn readable_ink(bg: egui::Color32) -> egui::Color32 {
    // PURE black and white, not a softened near-black and near-white.
    //
    // Softened ink cannot keep the promise. The hardest background to write on
    // is the one where black and white are equally bad, at luminance ≈ 0.179;
    // with pure ink both sit at 4.58:1 there, clearing WCAG's 4.5:1 floor for
    // body text by a hair. Backing the ink off even to #181A1E drops that worst
    // case to 4.41:1 — below the floor, which the mid-grey case in the tests
    // caught. Legibility over a map earns the harder tone.
    const DARK: egui::Color32 = egui::Color32::BLACK;
    const LIGHT: egui::Color32 = egui::Color32::WHITE;
    // Compare the candidates properly rather than guessing from a threshold:
    // mid-tones are exactly where a fixed cut-off picks the worse one.
    if crate::paint::contrast_ratio(DARK, bg) >= crate::paint::contrast_ratio(LIGHT, bg) {
        DARK
    } else {
        LIGHT
    }
}

/// Draw the info window near `anchor`, kept inside `bounds`.
///
/// `title` alone is the hover tooltip; `title` + `body` is the click card. The
/// card is nudged back inside the viewport and flipped above the anchor when it
/// would hang off the bottom, so a marker near an edge is still readable —
/// which is exactly where a naive popup becomes useless.
fn draw_info_window(
    painter: &egui::Painter,
    bounds: egui::Rect,
    anchor: egui::Pos2,
    title: &str,
    body: &str,
    style: &InfoStyle,
) {
    if title.trim().is_empty() && body.trim().is_empty() {
        return;
    }
    const PAD: f32 = 8.0;
    const GAP: f32 = 12.0; // clear of the marker itself
    let max_w = (bounds.width() * 0.6).clamp(120.0, 320.0);

    let title_font = egui::FontId::proportional(style.font_size + 1.0);
    let body_font = egui::FontId::proportional(style.font_size);
    let title_galley = (!title.trim().is_empty())
        .then(|| painter.layout(title.to_owned(), title_font, style.fg, max_w));
    // The body carries the SAME ink as the title, and is told apart by size
    // alone. It used to be dimmed toward the background, which spends the very
    // contrast this window exists to have — and the smaller text is the part
    // that can least afford it.
    let body_galley = (!body.trim().is_empty())
        .then(|| painter.layout(body.to_owned(), body_font, style.fg, max_w));

    let w = title_galley
        .as_ref()
        .map(|g| g.size().x)
        .unwrap_or(0.0)
        .max(body_galley.as_ref().map(|g| g.size().x).unwrap_or(0.0))
        + PAD * 2.0;
    let inner_h = title_galley.as_ref().map(|g| g.size().y).unwrap_or(0.0)
        + body_galley.as_ref().map(|g| g.size().y + 4.0).unwrap_or(0.0);
    let h = inner_h + PAD * 2.0;

    // Prefer above the anchor, the way a map pin's callout sits; flip below
    // when there is no room up there.
    let mut min = egui::pos2(anchor.x - w * 0.5, anchor.y - GAP - h);
    if min.y < bounds.top() + 2.0 {
        min.y = anchor.y + GAP;
    }
    min.x = min
        .x
        .clamp(bounds.left() + 2.0, (bounds.right() - w - 2.0).max(bounds.left() + 2.0));
    min.y = min
        .y
        .clamp(bounds.top() + 2.0, (bounds.bottom() - h - 2.0).max(bounds.top() + 2.0));
    let card = egui::Rect::from_min_size(min, egui::vec2(w, h));

    if style.shadow {
        painter.rect_filled(
            card.translate(egui::vec2(0.0, 2.0)),
            style.corner,
            egui::Color32::from_black_alpha(40),
        );
    }
    painter.rect_filled(card, style.corner, style.bg);
    painter.rect_stroke(
        card,
        style.corner,
        egui::Stroke::new(1.0, style.border),
        egui::StrokeKind::Inside,
    );

    let mut y = card.top() + PAD;
    if let Some(g) = title_galley {
        let size = g.size();
        painter.galley(egui::pos2(card.left() + PAD, y), g, style.fg);
        y += size.y + 4.0;
    }
    if let Some(g) = body_galley {
        painter.galley(egui::pos2(card.left() + PAD, y), g, style.fg);
    }
}

/// Route colour when the `Routes` line does not name one.
const DEFAULT_ROUTE_COLOR: egui::Color32 = egui::Color32::from_rgb(30, 110, 220);
/// Region fill when the `Regions` line does not name one — translucent on
/// purpose, so the streets underneath stay readable through a territory.
const DEFAULT_REGION_FILL: egui::Color32 =
    egui::Color32::from_rgba_premultiplied(60, 120, 200, 70);

/// Every colour the map paints that the overlay data does not carry itself.
///
/// These were literals in the painting code, so a pin was red on a form whose
/// every other colour the developer had chosen, and no property could say
/// otherwise (operator, 2026-08-22). Each is now a property on the Maps
/// control, and each **defaults to exactly what was painted before**, so a form
/// that sets none of them looks as it always did.
///
/// Where the data DOES carry a colour — `AddRoute`'s colour argument,
/// `AddRegion`'s fill and stroke — that colour still wins. These are the
/// defaults it falls back to, not a clamp over it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapColors {
    /// The pin itself. Was `#C82828` in the code and nowhere else.
    pub marker: egui::Color32,
    /// The ring drawn around a pin so it reads on a busy basemap.
    pub marker_border: egui::Color32,
    /// A route whose `Routes` line names no colour.
    pub route: egui::Color32,
    /// The casing under every route — the bright halo that makes a thin line
    /// readable over mixed terrain, the way every road map draws one.
    pub route_casing: egui::Color32,
    /// A region whose `Regions` line names no fill.
    pub region_fill: egui::Color32,
    /// A region whose `Regions` line names no stroke. `None` — the default —
    /// draws no border at all, which is what such a region has always done;
    /// naming a colour gives every unstyled region an outline.
    pub region_border: Option<egui::Color32>,
    /// Painted under the whole map before any tile has arrived.
    pub tile_backdrop: egui::Color32,
    /// A tile that has not arrived yet, in its own place.
    pub tile_loading: egui::Color32,
}

impl Default for MapColors {
    fn default() -> Self {
        Self {
            marker: egui::Color32::from_rgb(200, 40, 40),
            marker_border: egui::Color32::WHITE,
            route: DEFAULT_ROUTE_COLOR,
            route_casing: egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180),
            region_fill: DEFAULT_REGION_FILL,
            region_border: None,
            tile_backdrop: egui::Color32::from_gray(200),
            tile_loading: egui::Color32::from_gray(210),
        }
    }
}

impl MapColors {
    /// Read the colour properties off a Maps control. An empty or malformed
    /// value leaves that colour at its default, so one bad hex string costs
    /// only its own colour.
    ///
    /// ONE reader, called by both painting paths: the designer canvas draws
    /// markers, routes and regions too, and a pin that is one colour while you
    /// lay the form out and another while it runs is not a design surface.
    pub fn from_control(ctrl: &crate::model::Control) -> Self {
        let mut c = Self::default();
        let prop = |key: &str| -> Option<egui::Color32> {
            ctrl.get_prop(key)
                .map(|v| v.as_str().to_owned())
                .filter(|s| !s.trim().is_empty())
                .and_then(|s| parse_hex_color(&s))
        };
        if let Some(v) = prop("MarkerColor") {
            c.marker = v;
        }
        if let Some(v) = prop("MarkerBorderColor") {
            c.marker_border = v;
        }
        if let Some(v) = prop("RouteColor") {
            c.route = v;
        }
        if let Some(v) = prop("RouteCasingColor") {
            c.route_casing = v;
        }
        if let Some(v) = prop("RegionFillColor") {
            c.region_fill = v;
        }
        // The one colour whose absence MEANS something: no border.
        c.region_border = prop("RegionBorderColor");
        if let Some(v) = prop("TileBackgroundColor") {
            c.tile_backdrop = v;
        }
        if let Some(v) = prop("TileLoadingColor") {
            c.tile_loading = v;
        }
        c
    }
}

/// `#RGB`, `#RRGGBB` or `#RRGGBBAA` → a colour. `None` for empty or malformed
/// text, which is how a caller says "use the default" without a sentinel.
///
/// A region fill given without alpha is made translucent rather than opaque:
/// an area that hides the map under it is not what anybody drawing a sales
/// territory wants, and requiring eight digits to discover that would be a
/// poor default.
fn parse_hex_color(text: &str) -> Option<egui::Color32> {
    let h = text.trim().trim_start_matches('#');
    let val = |i: usize, n: usize| u8::from_str_radix(&h[i..i + n], 16).ok();
    match h.len() {
        3 => {
            let c = |i: usize| val(i, 1).map(|v| v * 17);
            Some(egui::Color32::from_rgb(c(0)?, c(1)?, c(2)?))
        }
        6 => Some(egui::Color32::from_rgb(val(0, 2)?, val(2, 2)?, val(4, 2)?)),
        8 => Some(egui::Color32::from_rgba_unmultiplied(
            val(0, 2)?,
            val(2, 2)?,
            val(4, 2)?,
            val(6, 2)?,
        )),
        _ => None,
    }
}

/// Paint the OpenStreetMap basemap plus a marker overlay into `rect`,
/// clipped to it. Pure rendering — no interaction, no `Ui`; a `Painter`
/// (which carries its own `Context` — `Painter::ctx()`) is all texture
/// upload needs, which is what lets this same function serve both the
/// designer canvas's static face (`paint::draw_control`, `Painter`-only)
/// and the interactive surfaces (`render_interactive`, which has a full
/// `Ui` but calls this the same way).
///
/// Returns the marker (if any) whose screen position is nearest `hit_test`
/// within a small radius — the caller does the actual click-vs-marker
/// decision; this function only offers the geometry.
pub fn paint_map(
    painter: &egui::Painter,
    rect: egui::Rect,
    center_lat: f64,
    center_lng: f64,
    zoom: u8,
    markers: &[MapMarker],
    routes: &[crate::model::MapRouteRecord],
    regions: &[crate::model::MapRegionRecord],
    pointer: MapPointer,
    // `open_marker_id` / `open_region_id`: whose CLICK card is open, by id —
    // empty for none. By id rather than index because the open one has to
    // survive the collection changing between frames.
    open_marker_id: &str,
    open_region_id: &str,
    info_style: &InfoStyle,
    colors: &MapColors,
) -> MapHit {
    paint_map_at(
        painter,
        rect,
        center_lat,
        center_lng,
        zoom,
        0.0,
        markers,
        routes,
        regions,
        pointer,
        open_marker_id,
        open_region_id,
        info_style,
        colors,
    )
}

/// [`paint_map`] with the map held **between** whole zoom levels.
///
/// `zoom_frac` is the offset from `zoom`, normally in `-0.5..=0.5`: the tiles of
/// level `zoom` are drawn `2^zoom_frac` times their natural 256 px, and every
/// overlay is placed at the same continuous zoom, so markers, routes and
/// regions stay pinned to the ground while the basemap scales under them.
///
/// This is what makes zooming smooth. A whole level is a factor of two, so
/// stepping one at a time replaced the picture rather than growing it; holding
/// the map at 12.4 for a few frames on the way from 12 to 13 lets the eye follow
/// the scale instead (operator, 2026-08-22). Tiles are still fetched by whole
/// level — `zoom` is the one asked for — so nothing about the cache or OSM's
/// tile protocol changes.
#[allow(clippy::too_many_arguments)]
pub fn paint_map_at(
    painter: &egui::Painter,
    rect: egui::Rect,
    center_lat: f64,
    center_lng: f64,
    zoom: u8,
    zoom_frac: f32,
    markers: &[MapMarker],
    routes: &[crate::model::MapRouteRecord],
    regions: &[crate::model::MapRegionRecord],
    pointer: MapPointer,
    open_marker_id: &str,
    open_region_id: &str,
    info_style: &InfoStyle,
    colors: &MapColors,
) -> MapHit {
    let ctx = painter.ctx();
    poll_tiles(ctx);

    let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    // Clamped, not trusted: a caller that lets the fraction run past a whole
    // level would have the map borrow tiles from two levels away and scale them
    // to blur. Half a level in each direction is the most any tile is stretched.
    let zoom_frac = if zoom_frac.is_finite() {
        zoom_frac.clamp(-0.5, 0.5)
    } else {
        0.0
    };
    // The zoom the map is actually DRAWN at — what every overlay is placed by.
    let zoom_at = zoom as f64 + zoom_frac as f64;
    // …and what that does to a tile: 256 px at the level itself, larger while
    // the map is on its way up, smaller on its way down.
    let tile_px = TILE_SIZE * 2f64.powf(zoom_frac as f64);
    painter.rect_filled(rect, 0.0, colors.tile_backdrop); // pre-tile backdrop
    // Tiles are drawn whole and let the painter cut them at the viewport edge —
    // see the `TileSlot::Ready` arm for why cutting the DESTINATION instead is
    // what made the map ripple while it was dragged.
    let painter = &painter.with_clip_rect(rect.intersect(painter.clip_rect()));

    let (center_tx, center_ty) = lat_lng_to_tile_frac(center_lat, center_lng, zoom);
    let center_tile_x = center_tx.floor() as i64;
    let center_tile_y = center_ty.floor() as i64;
    let frac_x = center_tx - center_tile_x as f64;
    let frac_y = center_ty - center_tile_y as f64;

    // Screen position of tile (center_tile_x, center_tile_y)'s top-left. All of
    // this is in `tile_px`, the tile's DRAWN size, so a fractional zoom moves
    // the grid and its origin together and the seams stay closed.
    let origin_x = rect.center().x - (frac_x * tile_px) as f32;
    let origin_y = rect.center().y - (frac_y * tile_px) as f32;

    let tiles_left = ((rect.center().x - rect.left()) / tile_px as f32).ceil() as i64 + 1;
    let tiles_right = ((rect.right() - rect.center().x) / tile_px as f32).ceil() as i64 + 1;
    let tiles_up = ((rect.center().y - rect.top()) / tile_px as f32).ceil() as i64 + 1;
    let tiles_down = ((rect.bottom() - rect.center().y) / tile_px as f32).ceil() as i64 + 1;

    let n_tiles = 1i64 << zoom;

    for dy in -tiles_up..=tiles_down {
        for dx in -tiles_left..=tiles_right {
            let tx = center_tile_x + dx;
            let ty = center_tile_y + dy;
            if ty < 0 || ty >= n_tiles {
                continue; // no wrap on the poles
            }
            // Wrap longitude tiles so panning past +/-180 keeps loading.
            let wrapped_tx = tx.rem_euclid(n_tiles);
            let key: TileKey = (zoom, wrapped_tx, ty);

            let dest = egui::Rect::from_min_size(
                egui::pos2(
                    origin_x + (dx as f64 * tile_px) as f32,
                    origin_y + (dy as f64 * tile_px) as f32,
                ),
                egui::vec2(tile_px as f32, tile_px as f32),
            );
            if !rect.intersects(dest) {
                continue;
            }

            let mut map = cache().lock().unwrap();
            match map.get(&key) {
                Some(TileSlot::Ready(tex)) => {
                    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    // The WHOLE tile, at its true size, clipped by the painter.
                    //
                    // This used to draw into `dest.intersect(rect)` while leaving
                    // the UV at the full 0..1 — which does not crop an edge tile,
                    // it SQUEEZES all 256×256 pixels of it into whatever sliver is
                    // still inside the viewport. Every edge tile was distorted by
                    // a different amount, and dragging changed those amounts
                    // continuously, so the whole map rippled while it moved
                    // (operator, 2026-08-20). Clipping is the painter's job;
                    // geometry stays undistorted.
                    painter.image(tex.id(), dest, uv, egui::Color32::WHITE);
                }
                // Not here yet — and a flat grey block is the worst thing to
                // put in its place, because the map already holds a picture of
                // this ground at another scale. `draw_tile_stand_in` magnifies
                // the nearest loaded ancestor (zooming in) or shrinks the four
                // children (zooming out), so the real tile arrives OVER a
                // picture instead of over grey.
                Some(TileSlot::Failed) => {
                    if !draw_tile_stand_in(&map, painter, dest, key) {
                        // Nothing to borrow from: a tile that will never come
                        // still has to read as absent rather than as ground.
                        painter.rect_filled(dest, 0.0, colors.tile_loading);
                    }
                }
                Some(TileSlot::Loading(_)) => {
                    // A stand-in while it travels; the backdrop when there is
                    // none to borrow.
                    draw_tile_stand_in(&map, painter, dest, key);
                }
                None => {
                    draw_tile_stand_in(&map, painter, dest, key);
                    drop(map);
                    request_tile(ctx, key);
                }
            }
        }
    }

    let mut hit = MapHit::default();
    // Where each region's card would point, kept for the draw pass at the end
    // (the window goes on top of everything, so it cannot be drawn inline).
    let mut region_anchor: Vec<egui::Pos2> = vec![egui::Pos2::ZERO; regions.len()];

    // Regions first, then routes, then markers — an area is a backdrop for the
    // line crossing it, and a pin belongs on top of both.
    for (ri, region) in regions.iter().enumerate() {
        let pts: Vec<(f32, f32)> = crate::map_geometry::parse_geometry(&region.geometry)
            .iter()
            .map(|p| {
                let (dx, dy) =
                    lat_lng_to_offset_at(p.lat, p.lng, center_lat, center_lng, zoom_at);
                (rect.center().x + dx, rect.center().y + dy)
            })
            .collect();
        if pts.len() < 3 {
            continue;
        }
        // Hit-testing a filled area is free once it is triangulated: the
        // question "is the pointer in this region" is "is it in any of its
        // triangles", and the triangles already exist to draw the fill.
        let tris = crate::map_geometry::triangulate(&pts);
        let inside = |p: egui::Pos2| {
            tris.iter().any(|t| {
                crate::map_geometry::point_in_triangle(
                    (p.x, p.y),
                    pts[t[0]],
                    pts[t[1]],
                    pts[t[2]],
                )
            })
        };
        // The card points at the region's centroid, not the cursor: an area's
        // callout belongs to the area, and a card chasing the pointer across a
        // territory is hard to read.
        let cx = pts.iter().map(|p| p.0).sum::<f32>() / pts.len() as f32;
        let cy = pts.iter().map(|p| p.1).sum::<f32>() / pts.len() as f32;
        region_anchor[ri] = egui::pos2(cx, cy);
        if let Some(p) = pointer.hover {
            if inside(p) {
                hit.hovered_region = Some(ri);
            }
        }
        if let Some(p) = pointer.click {
            if inside(p) {
                hit.clicked_region = Some(ri);
            }
        }
        let fill = parse_hex_color(&region.fill).unwrap_or(colors.region_fill);
        // Triangulated, never `convex_polygon`: a sales territory or a delivery
        // zone is nearly always concave, and a convex fill floods its notches.
        for tri in crate::map_geometry::triangulate(&pts) {
            painter.add(egui::Shape::convex_polygon(
                tri.iter()
                    .map(|&i| egui::pos2(pts[i].0, pts[i].1))
                    .collect(),
                fill,
                egui::Stroke::NONE,
            ));
        }
        if let Some(stroke) = parse_hex_color(&region.stroke) {
            let mut ring: Vec<egui::Pos2> =
                pts.iter().map(|&(x, y)| egui::pos2(x, y)).collect();
            ring.push(ring[0]);
            let w = if region.width > 0.0 { region.width } else { 2.0 };
            painter.add(egui::Shape::line(ring, egui::Stroke::new(w, stroke)));
        }
    }
    for route in routes {
        let pts: Vec<egui::Pos2> = crate::map_geometry::parse_geometry(&route.geometry)
            .iter()
            .map(|p| {
                let (dx, dy) =
                    lat_lng_to_offset_at(p.lat, p.lng, center_lat, center_lng, zoom_at);
                rect.center() + egui::vec2(dx, dy)
            })
            .collect();
        if pts.len() < 2 {
            continue;
        }
        let color = parse_hex_color(&route.color).unwrap_or(colors.route);
        let width = if route.width > 0.0 { route.width } else { 4.0 };
        // A casing under the line, the way every map draws a road: a thin
        // bright route over mixed terrain is unreadable without one.
        painter.add(egui::Shape::line(
            pts.clone(),
            egui::Stroke::new(width + 2.0, colors.route_casing),
        ));
        painter.add(egui::Shape::line(pts, egui::Stroke::new(width, color)));
    }

    let mut nearest_click: Option<(usize, f32)> = None;
    let mut nearest_hover: Option<(usize, f32)> = None;
    let mut marker_pos: Vec<egui::Pos2> = vec![egui::Pos2::ZERO; markers.len()];
    for (i, m) in markers.iter().enumerate() {
        let (dx, dy) = lat_lng_to_offset_at(m.lat, m.lng, center_lat, center_lng, zoom_at);
        let pos = rect.center() + egui::vec2(dx, dy);
        marker_pos[i] = pos;
        if !rect.contains(pos) {
            continue;
        }
        let radius = 6.0;
        painter.circle_filled(pos, radius, colors.marker);
        painter.circle_stroke(
            pos,
            radius,
            egui::Stroke::new(1.5, colors.marker_border),
        );
        // A little slack around the pin: a 6px dot is hard to hit exactly, and
        // the same slack for hover and click keeps the tooltip honest about
        // what a click would select.
        let slack = radius + 4.0;
        if let Some(cp) = pointer.click {
            let d = pos.distance(cp);
            if d <= slack && nearest_click.map(|(_, best)| d < best).unwrap_or(true) {
                nearest_click = Some((i, d));
            }
        }
        if let Some(hp) = pointer.hover {
            let d = pos.distance(hp);
            if d <= slack && nearest_hover.map(|(_, best)| d < best).unwrap_or(true) {
                nearest_hover = Some((i, d));
            }
        }
    }
    hit.clicked_marker = nearest_click.map(|(i, _)| i);
    hit.hovered_marker = nearest_hover.map(|(i, _)| i);
    // A pin sits inside its territory, and is the smaller, deliberately aimed
    // target — so it takes the hover and the click from the region under it.
    if hit.hovered_marker.is_some() {
        hit.hovered_region = None;
    }
    if hit.clicked_marker.is_some() {
        hit.clicked_region = None;
    }

    // ── The info window, last, over everything ──────────────────────────────
    //
    // Google's own behaviour, which is what the operator asked for: hovering
    // gives you the name, clicking opens the card. Both draw from the item's
    // own label/info — the fields the `Markers` and `Regions` properties have
    // always carried and which, until now, nothing ever displayed.
    let open_marker = (!open_marker_id.is_empty())
        .then(|| markers.iter().position(|m| m.id == open_marker_id))
        .flatten();
    let open_region = (!open_region_id.is_empty())
        .then(|| regions.iter().position(|r| r.id == open_region_id))
        .flatten();

    if let Some(i) = open_marker {
        draw_info_window(
            painter,
            rect,
            marker_pos[i],
            markers[i].label,
            markers[i].info,
            info_style,
        );
    } else if let Some(i) = open_region {
        draw_info_window(
            painter,
            rect,
            region_anchor[i],
            &regions[i].label,
            &regions[i].info,
            info_style,
        );
    } else if let Some(i) = hit.hovered_marker {
        // Hover shows the name only — the card is what a click is for.
        draw_info_window(painter, rect, marker_pos[i], markers[i].label, "", info_style);
    } else if let Some(i) = hit.hovered_region {
        draw_info_window(
            painter,
            rect,
            region_anchor[i],
            &regions[i].label,
            "",
            info_style,
        );
    }

    hit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lat_lng_round_trips_through_tile_fractions() {
        // Greenwich, zoom 10 — a mid-range, well-behaved case.
        let (lat, lng) = (51.4779, -0.0015);
        let zoom = 10;
        let (tx, ty) = lat_lng_to_tile_frac(lat, lng, zoom);
        let (lat2, lng2) = tile_frac_to_lat_lng(tx, ty, zoom);
        assert!((lat - lat2).abs() < 1e-6, "{lat} vs {lat2}");
        assert!((lng - lng2).abs() < 1e-6, "{lng} vs {lng2}");
    }

    #[test]
    fn known_reference_tile_matches_the_osm_wiki_example() {
        // The OSM wiki's own worked example: London at zoom 10 is tile
        // (511, 340) — https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
        let (tx, ty) = lat_lng_to_tile_frac(51.5074, -0.1278, 10);
        assert_eq!(tx.floor() as i64, 511);
        assert_eq!(ty.floor() as i64, 340);
    }

    #[test]
    fn offset_is_zero_at_the_exact_center() {
        let (dx, dy) = lat_lng_to_offset(40.0, -70.0, 40.0, -70.0, 8);
        assert!(dx.abs() < 1e-6 && dy.abs() < 1e-6, "{dx},{dy}");
    }

    #[test]
    fn offset_and_its_inverse_agree() {
        let (center_lat, center_lng, zoom) = (35.0, 139.0, 12);
        let (dx, dy) = (37.0_f32, -52.0_f32);
        let (lat, lng) = offset_to_lat_lng(dx, dy, center_lat, center_lng, zoom);
        let (dx2, dy2) = lat_lng_to_offset(lat, lng, center_lat, center_lng, zoom);
        assert!((dx - dx2).abs() < 0.01, "{dx} vs {dx2}");
        assert!((dy - dy2).abs() < 0.01, "{dy} vs {dy2}");
    }

    #[test]
    fn higher_zoom_moves_a_fixed_offset_a_larger_geographic_distance_per_tile() {
        // Sanity check on the projection's scale direction: at a higher
        // zoom, the SAME pixel offset corresponds to a SMALLER change in
        // longitude (more tiles cover the same geography), not a larger one.
        let (center_lat, center_lng) = (10.0, 10.0);
        let (_, lng_low_zoom) = offset_to_lat_lng(100.0, 0.0, center_lat, center_lng, 4);
        let (_, lng_high_zoom) = offset_to_lat_lng(100.0, 0.0, center_lat, center_lng, 12);
        let delta_low = (lng_low_zoom - center_lng).abs();
        let delta_high = (lng_high_zoom - center_lng).abs();
        assert!(
            delta_high < delta_low,
            "zoom 12's 100px offset ({delta_high} deg) should cover less \
             ground than zoom 4's ({delta_low} deg)"
        );
    }

    /// **One notch, one level.** Zoom used to move a level per scroll EVENT,
    /// and a trackpad flick is dozens of events — so a single gesture crossed
    /// five or six levels and the map could not be aimed.
    #[test]
    /// **The info window is always readable.** Its ink used to be inherited
    /// from the control's ForegroundColor while its card came from the
    /// control's BackgroundColor — two colours from two places, with nothing
    /// making them contrast. On a dark-themed form carrying a white
    /// foreground, the card came out white-on-light and could not be read
    /// (operator, 2026-08-21).
    ///
    /// WCAG calls 4.5:1 the floor for body text. Deriving the ink from the
    /// background clears that for any background at all, which is the only way
    /// to promise it — a threshold guess fails exactly at the mid-tones.
    #[test]
    fn the_info_window_ink_always_contrasts_with_its_card() {
        let cases = [
            ("white card", egui::Color32::WHITE),
            ("black card", egui::Color32::BLACK),
            ("the light grey that broke", egui::Color32::from_rgb(210, 212, 215)),
            ("dark navy form", egui::Color32::from_rgb(16, 24, 38)),
            ("mid grey", egui::Color32::from_gray(128)),
            ("mid-tone teal", egui::Color32::from_rgb(70, 130, 130)),
            ("warm sand", egui::Color32::from_rgb(200, 180, 140)),
        ];
        for (name, bg) in cases {
            let ink = readable_ink(bg);
            let ratio = crate::paint::contrast_ratio(ink, bg);
            assert!(
                ratio >= 4.5,
                "{name}: ink {ink:?} on {bg:?} is {ratio:.2}:1 — below the 4.5:1 floor"
            );
        }
    }

    /// …and every grey in between, so no mid-tone slips through.
    #[test]
    fn no_shade_of_grey_produces_an_unreadable_window() {
        for v in 0..=255u8 {
            let bg = egui::Color32::from_gray(v);
            let ratio = crate::paint::contrast_ratio(readable_ink(bg), bg);
            assert!(ratio >= 4.5, "gray {v} gave only {ratio:.2}:1");
        }
    }

    #[test]
    fn a_trackpad_flick_does_not_cross_six_zoom_levels() {
        // Twelve small deltas, the shape a flick arrives in.
        let mut accum = 0.0f32;
        let mut levels = 0;
        for _ in 0..12 {
            let (step, next) = zoom_steps(accum, 12.0);
            levels += step;
            accum = next;
        }
        assert_eq!(
            levels,
            (12.0 * 12.0 / SCROLL_PER_ZOOM) as i32,
            "144px of scroll is one level at {SCROLL_PER_ZOOM}px each, not twelve"
        );
        assert!(levels <= 2, "a flick must not leap: {levels} levels");
    }

    /// However hard one frame is spun, the map moves at most one level — and
    /// the rest is kept, not thrown away, so nothing is lost.
    #[test]
    fn a_single_violent_frame_still_moves_only_one_level() {
        let (levels, accum) = zoom_steps(0.0, 10_000.0);
        assert_eq!(levels, MAX_ZOOM_STEP_PER_FRAME);
        assert!(accum > 0.0, "the surplus is carried, not discarded");
    }

    /// Slow scrolling still gets there: the accumulator is what makes a
    /// fine-grained device usable at all.
    #[test]
    fn small_deltas_accumulate_into_a_step() {
        let mut accum = 0.0f32;
        let mut levels = 0;
        for _ in 0..(SCROLL_PER_ZOOM as i32 / 5) {
            let (step, next) = zoom_steps(accum, 5.0);
            levels += step;
            accum = next;
        }
        assert_eq!(levels, 1, "enough small pushes make exactly one level");
    }

    /// Reversing direction responds at once instead of spending the old
    /// direction's credit first.
    #[test]
    fn reversing_direction_does_not_have_to_pay_off_the_old_one() {
        let (_, accum) = zoom_steps(0.0, SCROLL_PER_ZOOM * 0.9); // nearly a level in
        let (levels, _) = zoom_steps(accum, -SCROLL_PER_ZOOM * 0.9);
        assert_eq!(levels, 0, "no phantom step on the reversal itself");
        let (_, accum) = zoom_steps(0.0, SCROLL_PER_ZOOM * 0.9);
        let (levels, _) = zoom_steps(accum, -SCROLL_PER_ZOOM * 1.1);
        assert_eq!(levels, -1, "one push the other way zooms out");
    }

    // ── Smooth zoom ──────────────────────────────────────────────────────────
    //
    // A whole level is a factor of two, so stepping one at a time REPLACED the
    // picture: nothing was ever drawn between 12 and 13. The glide releases a
    // fraction per frame and the painter holds the map at 12.4 on the way
    // (operator, 2026-08-22).

    /// One notch of the wheel now arrives over several frames instead of one,
    /// and adds up to exactly the same amount of zoom it always did.
    #[test]
    fn one_notch_arrives_in_slices_and_still_totals_one_level() {
        let (mut pending, mut total, mut frames) = (0.0f32, 0.0f32, 0);
        // The whole notch in a single frame's scroll, then let it glide out.
        let (step, next) = zoom_glide(pending, SCROLL_PER_ZOOM);
        total += step;
        pending = next;
        frames += 1;
        while pending != 0.0 && frames < 200 {
            let (step, next) = zoom_glide(pending, 0.0);
            total += step;
            pending = next;
            frames += 1;
        }
        assert!(
            (total - 1.0).abs() < 1e-3,
            "a notch must still buy exactly one level, got {total}"
        );
        assert!(
            frames >= 4,
            "and it must take several frames to get there, took {frames}"
        );
        assert!(frames < 30, "but not so many it feels sluggish: {frames}");
    }

    /// No frame may move more than a slice, however hard the wheel is spun —
    /// that cap is the whole difference between gliding and jumping.
    #[test]
    fn no_single_frame_leaps() {
        let (step, pending) = zoom_glide(0.0, 10_000.0);
        assert!(
            step.abs() <= MAX_ZOOM_PER_FRAME + 1e-6,
            "one frame moved {step} levels"
        );
        assert!(pending > 0.0, "the surplus is carried, not discarded");
    }

    /// The glide ends. A remainder that never reached zero would keep asking
    /// for repaints for as long as the form was open.
    #[test]
    fn the_glide_settles() {
        let mut pending = zoom_glide(0.0, SCROLL_PER_ZOOM * 3.0).1;
        let mut frames = 0;
        while pending != 0.0 {
            pending = zoom_glide(pending, 0.0).1;
            frames += 1;
            assert!(frames < 500, "the glide never settled");
        }
    }

    /// Reversing still answers at once — the rule `zoom_steps` had, kept.
    #[test]
    fn the_glide_reverses_without_paying_off_the_old_direction() {
        let (_, pending) = zoom_glide(0.0, SCROLL_PER_ZOOM * 3.0); // a lot queued in
        let (step, _) = zoom_glide(pending, -SCROLL_PER_ZOOM);
        assert!(step < 0.0, "pushing back must zoom back, got {step}");
    }

    /// The tiles fetched are always the NEAREST whole level, so nothing on
    /// screen is ever stretched by more than √2 and the map stays sharp.
    #[test]
    fn the_fraction_always_borrows_from_the_nearest_level() {
        for (zoom, level, frac) in [
            (12.0f32, 12u8, 0.0f32),
            (12.4, 12, 0.4),
            (12.6, 13, -0.4),
            (0.0, 0, 0.0),
            (19.0, 19, 0.0),
        ] {
            let (l, f) = split_zoom(zoom);
            assert_eq!(l, level, "{zoom} should draw level {level}");
            assert!((f - frac).abs() < 1e-5, "{zoom} frac {f} != {frac}");
            assert!(f.abs() <= 0.5 + 1e-6, "{zoom} stretches too far: {f}");
        }
    }

    /// Past the ends the split stays inside the tile range rather than asking
    /// for a level OSM does not serve.
    #[test]
    fn the_split_never_leaves_the_tile_range() {
        for zoom in [-5.0f32, -0.4, 19.4, 40.0] {
            let (level, frac) = split_zoom(zoom);
            assert!((MIN_ZOOM..=MAX_ZOOM).contains(&level), "level {level}");
            assert!(frac.abs() <= 0.5 + 1e-6, "frac {frac}");
        }
    }

    /// A fractional zoom is a real scale, not a rounding: halfway between two
    /// levels the world is √2 times wider than at the lower one.
    #[test]
    fn a_fractional_zoom_scales_between_the_levels_it_sits_between() {
        let (lat, lng) = (48.8566, 2.3522);
        let at12 = lat_lng_to_offset_at(lat + 0.05, lng, lat, lng, 12.0).1.abs();
        let at12_5 = lat_lng_to_offset_at(lat + 0.05, lng, lat, lng, 12.5).1.abs();
        let at13 = lat_lng_to_offset_at(lat + 0.05, lng, lat, lng, 13.0).1.abs();
        assert!(
            at12 < at12_5 && at12_5 < at13,
            "12 < 12.5 < 13 in screen distance: {at12} {at12_5} {at13}"
        );
        assert!(
            (at12_5 / at12 - 2f32.sqrt()).abs() < 1e-3,
            "half a level is a factor of √2, got {}",
            at12_5 / at12
        );
    }

    /// The cursor stays the fixed point BETWEEN levels too — the anchor rule
    /// has to hold on every frame of a glide, not only when it lands.
    #[test]
    fn the_cursor_stays_fixed_across_a_fractional_zoom() {
        let (lat, lng) = (-23.5614, -46.6558);
        let (dx, dy) = (120.0f32, -80.0f32);
        let before = offset_to_lat_lng_at(dx, dy, lat, lng, 12.0);
        let (nlat, nlng) = zoom_about_at(lat, lng, 12.0, 12.375, dx, dy);
        let after = offset_to_lat_lng_at(dx, dy, nlat, nlng, 12.375);
        assert!(
            (before.0 - after.0).abs() < 1e-9 && (before.1 - after.1).abs() < 1e-9,
            "the anchored coordinate moved mid-glide: {before:?} -> {after:?}"
        );
    }

    /// The whole-level helpers are the continuous ones at a whole level, so a
    /// map that never zooms fractionally is untouched by any of this.
    #[test]
    fn the_whole_level_helpers_are_unchanged() {
        let (lat, lng) = (35.6762, 139.6503);
        for zoom in [0u8, 5, 12, 19] {
            assert_eq!(
                lat_lng_to_tile_frac(lat, lng, zoom),
                lat_lng_to_tile_frac_at(lat, lng, zoom as f64)
            );
            assert_eq!(
                lat_lng_to_offset(lat + 0.1, lng, lat, lng, zoom),
                lat_lng_to_offset_at(lat + 0.1, lng, lat, lng, zoom as f64)
            );
            assert_eq!(
                offset_to_lat_lng(30.0, -40.0, lat, lng, zoom),
                offset_to_lat_lng_at(30.0, -40.0, lat, lng, zoom as f64)
            );
        }
    }

    /// **The cursor is the fixed point.** Zooming used to leave the centre
    /// alone, so whatever you were pointing at slid away as the scale changed
    /// and you had to chase it.
    #[test]
    fn zooming_keeps_the_point_under_the_cursor_under_the_cursor() {
        let (lat, lng) = (-23.5614, -46.6558); // São Paulo
        let (anchor_dx, anchor_dy) = (120.0f32, -80.0f32);
        let before = offset_to_lat_lng(anchor_dx, anchor_dy, lat, lng, 12);

        let (new_lat, new_lng) = zoom_about(lat, lng, 12, 13, anchor_dx, anchor_dy);
        let after = offset_to_lat_lng(anchor_dx, anchor_dy, new_lat, new_lng, 13);

        assert!(
            (before.0 - after.0).abs() < 1e-9 && (before.1 - after.1).abs() < 1e-9,
            "the anchored coordinate moved: {before:?} -> {after:?}"
        );
    }

    /// Zooming out about a cursor is the exact inverse of zooming in about it.
    #[test]
    fn zoom_about_round_trips() {
        let (lat, lng) = (48.8566, 2.3522);
        let (dx, dy) = (-64.0f32, 200.0f32);
        let (l1, g1) = zoom_about(lat, lng, 10, 14, dx, dy);
        let (l2, g2) = zoom_about(l1, g1, 14, 10, dx, dy);
        assert!(
            (lat - l2).abs() < 1e-9 && (lng - g2).abs() < 1e-9,
            "in-then-out should return to the start: ({lat},{lng}) -> ({l2},{g2})"
        );
    }

    /// A no-op zoom leaves the centre exactly alone — no drift from repeated
    /// frames where the accumulator has not yet reached a level.
    #[test]
    fn an_unchanged_zoom_never_moves_the_centre() {
        let (lat, lng) = (10.0, 20.0);
        assert_eq!(zoom_about(lat, lng, 8, 8, 300.0, -50.0), (lat, lng));
    }

    /// **A missing tile borrows the right piece of ground.**
    ///
    /// Operator, 2026-08-22: a zoom blanked to a grey block and then snapped to
    /// the new image, where every map client shows the ground it already has,
    /// rescaled. Which PART of the ancestor a tile covers is the whole of that:
    /// get it wrong and the stand-in shows ground from somewhere else, which is
    /// worse than grey because it looks correct.
    #[test]
    fn a_tile_covers_its_own_quadrant_of_its_ancestor() {
        // One level up: the low bit of each coordinate picks the quadrant.
        assert_eq!(
            ancestor_uv(2, 3, 1),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.5), egui::pos2(0.5, 1.0)),
            "even x, odd y ⇒ bottom-left quarter"
        );
        assert_eq!(
            ancestor_uv(3, 2, 1),
            egui::Rect::from_min_max(egui::pos2(0.5, 0.0), egui::pos2(1.0, 0.5)),
            "odd x, even y ⇒ top-right quarter"
        );
        // Two levels up: one of sixteen, from the low TWO bits.
        assert_eq!(
            ancestor_uv(5, 6, 2),
            egui::Rect::from_min_max(egui::pos2(0.25, 0.5), egui::pos2(0.5, 0.75))
        );
    }

    /// The four children of one tile tile it exactly — no overlap, no gap.
    /// A stand-in that double-covers a strip would draw one band of ground
    /// twice and leave another missing.
    #[test]
    fn the_four_children_cover_their_parent_exactly() {
        let (px, py) = (11i64, 4i64);
        let quads: Vec<egui::Rect> = [(0i64, 0i64), (1, 0), (0, 1), (1, 1)]
            .iter()
            .map(|(i, j)| ancestor_uv(px * 2 + i, py * 2 + j, 1))
            .collect();
        let mut area = 0.0;
        for (n, a) in quads.iter().enumerate() {
            area += a.width() * a.height();
            for b in &quads[n + 1..] {
                let hit = a.intersect(*b);
                assert!(
                    hit.width() <= 0.0 || hit.height() <= 0.0,
                    "{a:?} and {b:?} overlap"
                );
            }
        }
        assert!(
            (area - 1.0).abs() < 1e-6,
            "the four must cover the whole parent, got {area}"
        );
    }

    /// **A map nobody restyled paints exactly what it always did.** The colours
    /// became properties (operator, 2026-08-22: "all Map's colors are hard
    /// coded"), and the whole promise of seeding them EMPTY is that an existing
    /// form is untouched — so the built-ins are pinned here against the literals
    /// they replaced.
    #[test]
    fn an_unset_map_keeps_every_built_in_colour() {
        let map = crate::model::Control::new("MAP-1", crate::ControlType::Maps, 0, 0);
        let c = MapColors::from_control(&map);
        assert_eq!(c, MapColors::default(), "a seeded Maps control must not restyle itself");
        assert_eq!(c.marker, egui::Color32::from_rgb(200, 40, 40));
        assert_eq!(c.marker_border, egui::Color32::WHITE);
        assert_eq!(c.route, egui::Color32::from_rgb(30, 110, 220));
        assert_eq!(
            c.route_casing,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180)
        );
        assert_eq!(
            c.region_fill,
            egui::Color32::from_rgba_premultiplied(60, 120, 200, 70)
        );
        assert_eq!(c.region_border, None, "an unstyled region has never had a border");
        assert_eq!(c.tile_backdrop, egui::Color32::from_gray(200));
        assert_eq!(c.tile_loading, egui::Color32::from_gray(210));
    }

    /// Every property is read, and each one moves ONLY its own colour — a
    /// blanket "colours come from properties now" is worth nothing if two of
    /// them share a code path.
    #[test]
    fn each_map_colour_property_moves_only_its_own_colour() {
        let cases: [(&str, &str, fn(&MapColors) -> egui::Color32); 7] = [
            ("MarkerColor", "#112233", |c| c.marker),
            ("MarkerBorderColor", "#445566", |c| c.marker_border),
            ("RouteColor", "#778899", |c| c.route),
            ("RouteCasingColor", "#AABBCC", |c| c.route_casing),
            ("RegionFillColor", "#DDEEFF", |c| c.region_fill),
            ("TileBackgroundColor", "#010203", |c| c.tile_backdrop),
            ("TileLoadingColor", "#040506", |c| c.tile_loading),
        ];
        for (key, hex, read) in cases {
            let mut map = crate::model::Control::new("MAP-1", crate::ControlType::Maps, 0, 0);
            map.set_prop(key, crate::PropValue::String(hex.into()));
            let c = MapColors::from_control(&map);
            assert_eq!(
                read(&c),
                parse_hex_color(hex).expect("test hex parses"),
                "{key} did not reach the painter"
            );
            // Nothing else moved.
            let base = MapColors::default();
            let moved = cases
                .iter()
                .filter(|(other, _, other_read)| *other != key && other_read(&c) != other_read(&base))
                .map(|(other, _, _)| *other)
                .collect::<Vec<_>>();
            assert!(moved.is_empty(), "{key} also changed {moved:?}");
        }
    }

    /// `RegionBorderColor` is the one whose EMPTY value means something: a
    /// region whose own line names no stroke has never had a border, so
    /// seeding one would draw an outline on every such region in every form
    /// that already exists.
    #[test]
    fn a_region_border_appears_only_when_asked_for() {
        let mut map = crate::model::Control::new("MAP-1", crate::ControlType::Maps, 0, 0);
        assert_eq!(MapColors::from_control(&map).region_border, None);
        map.set_prop("RegionBorderColor", crate::PropValue::String("#FF8800".into()));
        assert_eq!(
            MapColors::from_control(&map).region_border,
            Some(egui::Color32::from_rgb(255, 136, 0))
        );
    }

    /// A colour that cannot be parsed costs only itself. One typo in one hex
    /// string must not drag a map back to stock.
    #[test]
    fn a_malformed_colour_leaves_the_others_alone() {
        let mut map = crate::model::Control::new("MAP-1", crate::ControlType::Maps, 0, 0);
        map.set_prop("MarkerColor", crate::PropValue::String("not a colour".into()));
        map.set_prop("RouteColor", crate::PropValue::String("#00FF00".into()));
        let c = MapColors::from_control(&map);
        assert_eq!(c.marker, MapColors::default().marker, "a bad hex falls back");
        assert_eq!(c.route, egui::Color32::from_rgb(0, 255, 0));
    }
}
