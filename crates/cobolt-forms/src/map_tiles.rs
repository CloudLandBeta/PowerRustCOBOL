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
                    painter.image(tex.id(), dest.intersect(rect), uv, egui::Color32::WHITE);
                }
                Some(TileSlot::Failed) => {
                    painter.rect_filled(
                        dest.intersect(rect),
                        0.0,
                        egui::Color32::from_gray(210),
                    );
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
}
