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
    hit_test: Option<egui::Pos2>,
) -> Option<usize> {
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

    let mut nearest: Option<(usize, f32)> = None;
    for (i, m) in markers.iter().enumerate() {
        let (dx, dy) = lat_lng_to_offset(m.lat, m.lng, center_lat, center_lng, zoom);
        let pos = rect.center() + egui::vec2(dx, dy);
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
        if let Some(hp) = hit_test {
            let d = pos.distance(hp);
            if d <= radius + 3.0 && nearest.map(|(_, best)| d < best).unwrap_or(true) {
                nearest = Some((i, d));
            }
        }
    }
    nearest.map(|(i, _)| i)
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
