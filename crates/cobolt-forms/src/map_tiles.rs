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
    let lat = lat.clamp(-85.051_128, 85.051_128); // Web Mercator's own valid range
    let lat_rad = lat.to_radians();
    let n = 2f64.powi(zoom as i32);
    let x = (lng + 180.0) / 360.0 * n;
    let y = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI) / 2.0 * n;
    (x, y)
}

/// The inverse of [`lat_lng_to_tile_frac`] — a fractional tile coordinate
/// back to lat/lng. Used to turn a drag delta (in tiles) back into a new
/// `Center`.
pub fn tile_frac_to_lat_lng(x: f64, y: f64, zoom: u8) -> (f64, f64) {
    let n = 2f64.powi(zoom as i32);
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
    let (tx, ty) = lat_lng_to_tile_frac(lat, lng, zoom);
    let (ctx, cty) = lat_lng_to_tile_frac(center_lat, center_lng, zoom);
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
    if from_zoom == to_zoom {
        return (center_lat, center_lng);
    }
    // What is under the cursor now…
    let (anchor_lat, anchor_lng) =
        offset_to_lat_lng(anchor_dx, anchor_dy, center_lat, center_lng, from_zoom);
    // …and the centre that keeps it there at the new scale.
    let (ax, ay) = lat_lng_to_tile_frac(anchor_lat, anchor_lng, to_zoom);
    let cx = ax - anchor_dx as f64 / TILE_SIZE;
    let cy = ay - anchor_dy as f64 / TILE_SIZE;
    tile_frac_to_lat_lng(cx, cy, to_zoom)
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
    let (ctx, cty) = lat_lng_to_tile_frac(center_lat, center_lng, zoom);
    let tx = ctx + dx as f64 / TILE_SIZE;
    let ty = cty + dy as f64 / TILE_SIZE;
    tile_frac_to_lat_lng(tx, ty, zoom)
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
    let body_galley = (!body.trim().is_empty()).then(|| {
        painter.layout(
            body.to_owned(),
            body_font,
            style.fg.gamma_multiply(0.75),
            max_w,
        )
    });

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
) -> MapHit {
    let ctx = painter.ctx();
    poll_tiles(ctx);

    let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    painter.rect_filled(rect, 0.0, egui::Color32::from_gray(200)); // pre-tile backdrop
    // Tiles are drawn whole and let the painter cut them at the viewport edge —
    // see the `TileSlot::Ready` arm for why cutting the DESTINATION instead is
    // what made the map ripple while it was dragged.
    let painter = &painter.with_clip_rect(rect.intersect(painter.clip_rect()));

    let (center_tx, center_ty) = lat_lng_to_tile_frac(center_lat, center_lng, zoom);
    let center_tile_x = center_tx.floor() as i64;
    let center_tile_y = center_ty.floor() as i64;
    let frac_x = center_tx - center_tile_x as f64;
    let frac_y = center_ty - center_tile_y as f64;

    // Screen position of tile (center_tile_x, center_tile_y)'s top-left.
    let origin_x = rect.center().x - (frac_x * TILE_SIZE) as f32;
    let origin_y = rect.center().y - (frac_y * TILE_SIZE) as f32;

    let tiles_left = ((rect.center().x - rect.left()) / TILE_SIZE as f32).ceil() as i64 + 1;
    let tiles_right = ((rect.right() - rect.center().x) / TILE_SIZE as f32).ceil() as i64 + 1;
    let tiles_up = ((rect.center().y - rect.top()) / TILE_SIZE as f32).ceil() as i64 + 1;
    let tiles_down = ((rect.bottom() - rect.center().y) / TILE_SIZE as f32).ceil() as i64 + 1;

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
                    origin_x + (dx as f64 * TILE_SIZE) as f32,
                    origin_y + (dy as f64 * TILE_SIZE) as f32,
                ),
                egui::vec2(TILE_SIZE as f32, TILE_SIZE as f32),
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
                Some(TileSlot::Failed) => {
                    painter.rect_filled(dest, 0.0, egui::Color32::from_gray(210));
                }
                Some(TileSlot::Loading(_)) => {
                    // Left at the pre-tile backdrop colour; nothing to draw.
                }
                None => {
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
                let (dx, dy) = lat_lng_to_offset(p.lat, p.lng, center_lat, center_lng, zoom);
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
        let fill = parse_hex_color(&region.fill).unwrap_or(DEFAULT_REGION_FILL);
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
                let (dx, dy) = lat_lng_to_offset(p.lat, p.lng, center_lat, center_lng, zoom);
                rect.center() + egui::vec2(dx, dy)
            })
            .collect();
        if pts.len() < 2 {
            continue;
        }
        let color = parse_hex_color(&route.color).unwrap_or(DEFAULT_ROUTE_COLOR);
        let width = if route.width > 0.0 { route.width } else { 4.0 };
        // A casing under the line, the way every map draws a road: a thin
        // bright route over mixed terrain is unreadable without one.
        painter.add(egui::Shape::line(
            pts.clone(),
            egui::Stroke::new(width + 2.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180)),
        ));
        painter.add(egui::Shape::line(pts, egui::Stroke::new(width, color)));
    }

    let mut nearest_click: Option<(usize, f32)> = None;
    let mut nearest_hover: Option<(usize, f32)> = None;
    let mut marker_pos: Vec<egui::Pos2> = vec![egui::Pos2::ZERO; markers.len()];
    for (i, m) in markers.iter().enumerate() {
        let (dx, dy) = lat_lng_to_offset(m.lat, m.lng, center_lat, center_lng, zoom);
        let pos = rect.center() + egui::vec2(dx, dy);
        marker_pos[i] = pos;
        if !rect.contains(pos) {
            continue;
        }
        let radius = 6.0;
        painter.circle_filled(pos, radius, egui::Color32::from_rgb(200, 40, 40));
        painter.circle_stroke(
            pos,
            radius,
            egui::Stroke::new(1.5, egui::Color32::WHITE),
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
}
