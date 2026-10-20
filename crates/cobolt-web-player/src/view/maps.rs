// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Maps control** (WASM-004 class B for the picture, C for geocoding and directions).
//!
//! The picture is a grid of `<img>` tiles placed by the same Web-Mercator arithmetic the desktop
//! painter uses (`cobolt_forms::map_tiles`), with the markers, routes and regions laid over it. The
//! tile address is a template (`{z}`, `{x}`, `{y}`) from the player's configuration, so an
//! application can serve its own tiles; the default is the OpenStreetMap server, whose origin the
//! bundle's `img-src` must then allow (the build prints it, WASM-013). Geocoding, directions and
//! places go through the backend (class C); this module draws what the program holds.

use cobolt_forms::map_geometry::{parse_geometry, LatLng};
use cobolt_forms::model::{parse_map_regions, parse_map_routes};
use cobolt_forms::parse_map_markers;

use super::controls::root;
use super::{VNode, ViewCtx};
use crate::value::{fmt_number, parse_color, parse_f64};

// ── Web Mercator ────────────────────────────────────────────────────────────
//
// The same projection the desktop painter uses (`cobolt_forms::map_tiles`, which sits behind the
// painter's `render` feature and so is not built into the player): the world is `2^zoom` tiles of
// 256 pixels across, and a coordinate's tile is `x = (lng + 180) / 360 * n`,
// `y = (1 - ln(tan(lat) + sec(lat)) / pi) / 2 * n`. A test below checks known tile numbers.

pub const TILE_SIZE: f64 = 256.0;
pub const MIN_ZOOM: u8 = 0;
pub const MAX_ZOOM: u8 = 19;

/// lat/lng (degrees) to fractional tile coordinates at a (continuous) zoom.
pub fn lat_lng_to_tile_frac_at(lat: f64, lng: f64, zoom: f64) -> (f64, f64) {
    let lat = lat.clamp(-85.051_128, 85.051_128);
    let lat_rad = lat.to_radians();
    let n = 2f64.powf(zoom);
    let x = (lng + 180.0) / 360.0 * n;
    let y = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI) / 2.0 * n;
    (x, y)
}

/// The inverse of [`lat_lng_to_tile_frac_at`].
pub fn tile_frac_to_lat_lng_at(x: f64, y: f64, zoom: f64) -> (f64, f64) {
    let n = 2f64.powf(zoom);
    let lng = x / n * 360.0 - 180.0;
    let lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * y / n)).sinh().atan();
    (lat_rad.to_degrees(), lng)
}

/// The pixel offset of a coordinate from a viewport centred on `(center_lat, center_lng)`.
pub fn lat_lng_to_offset_at(lat: f64, lng: f64, center_lat: f64, center_lng: f64, zoom: f64) -> (f32, f32) {
    let (tx, ty) = lat_lng_to_tile_frac_at(lat, lng, zoom);
    let (ctx, cty) = lat_lng_to_tile_frac_at(center_lat, center_lng, zoom);
    (((tx - ctx) * TILE_SIZE) as f32, ((ty - cty) * TILE_SIZE) as f32)
}

/// The viewport of the map: its size in pixels and where it is centred.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub w: f64,
    pub h: f64,
    pub lat: f64,
    pub lng: f64,
    pub zoom: u8,
}

impl Viewport {
    pub fn read(cx: &ViewCtx, i: usize) -> Viewport {
        let get = |k: &str, d: f64| parse_f64(&cx.tree.get(i, k)).unwrap_or(d);
        Viewport {
            w: get("Width", 400.0).max(16.0),
            h: get("Height", 300.0).max(16.0),
            lat: get("CenterLat", 0.0).clamp(-85.0, 85.0),
            lng: get("CenterLng", 0.0).clamp(-180.0, 180.0),
            zoom: (get("Zoom", 2.0).round() as i64).clamp(MIN_ZOOM as i64, MAX_ZOOM as i64) as u8,
        }
    }

    /// The pixel position of a coordinate inside the viewport (top-left origin).
    pub fn to_px(&self, lat: f64, lng: f64) -> (f64, f64) {
        let (dx, dy) = lat_lng_to_offset_at(lat, lng, self.lat, self.lng, self.zoom as f64);
        (self.w / 2.0 + dx as f64, self.h / 2.0 + dy as f64)
    }

    /// The coordinate under a pixel of the viewport.
    pub fn to_lat_lng(&self, px: f64, py: f64) -> (f64, f64) {
        let (cx_, cy) = lat_lng_to_tile_frac_at(self.lat, self.lng, self.zoom as f64);
        tile_frac_to_lat_lng_at(cx_ + (px - self.w / 2.0) / TILE_SIZE, cy + (py - self.h / 2.0) / TILE_SIZE, self.zoom as f64)
    }

    /// The centre after the map is dragged by `(dx, dy)` pixels.
    pub fn panned(&self, dx: f64, dy: f64) -> (f64, f64) {
        let (cx_, cy) = lat_lng_to_tile_frac_at(self.lat, self.lng, self.zoom as f64);
        let (lat, lng) = tile_frac_to_lat_lng_at(cx_ - dx / TILE_SIZE, cy - dy / TILE_SIZE, self.zoom as f64);
        (lat.clamp(-85.0, 85.0), ((lng + 540.0).rem_euclid(360.0)) - 180.0)
    }
}

/// One tile to draw: its address and where it goes.
#[derive(Debug, Clone, PartialEq)]
pub struct Tile {
    pub z: u8,
    pub x: u64,
    pub y: u64,
    pub left: f64,
    pub top: f64,
}

/// The tiles that cover the viewport. `x` wraps around the world; a `y` outside the map is skipped.
pub fn tiles(v: &Viewport) -> Vec<Tile> {
    let n = 1u64 << v.zoom;
    let (cx_, cy) = lat_lng_to_tile_frac_at(v.lat, v.lng, v.zoom as f64);
    let (x0, x1) = (((cx_ - v.w / 2.0 / TILE_SIZE).floor()) as i64, ((cx_ + v.w / 2.0 / TILE_SIZE).floor()) as i64);
    let (y0, y1) = (((cy - v.h / 2.0 / TILE_SIZE).floor()) as i64, ((cy + v.h / 2.0 / TILE_SIZE).floor()) as i64);
    let mut out = Vec::new();
    for ty in y0..=y1 {
        if ty < 0 || ty as u64 >= n {
            continue;
        }
        for tx in x0..=x1 {
            out.push(Tile {
                z: v.zoom,
                x: (tx.rem_euclid(n as i64)) as u64,
                y: ty as u64,
                left: v.w / 2.0 + (tx as f64 - cx_) * TILE_SIZE,
                top: v.h / 2.0 + (ty as f64 - cy) * TILE_SIZE,
            });
        }
    }
    out
}

/// A tile's address from the template.
pub fn tile_url(template: &str, t: &Tile) -> String {
    template.replace("{z}", &t.z.to_string()).replace("{x}", &t.x.to_string()).replace("{y}", &t.y.to_string())
}

fn px(v: f64) -> String {
    format!("{}px", fmt_number((v * 100.0).round() / 100.0))
}

fn css_color(c: &str, default: &str) -> String {
    match parse_color(c) {
        Some(a) if a[3] == 255 => format!("#{:02x}{:02x}{:02x}", a[0], a[1], a[2]),
        Some(a) => format!("rgb({} {} {} / {})", a[0], a[1], a[2], fmt_number((a[3] as f64 / 255.0 * 1000.0).round() / 1000.0)),
        None => default.to_owned(),
    }
}

fn points_path(v: &Viewport, pts: &[LatLng], close: bool) -> String {
    let mut d = String::new();
    for (k, p) in pts.iter().enumerate() {
        let (x, y) = v.to_px(p.lat, p.lng);
        d.push_str(&format!("{}{} {}", if k == 0 { "M" } else { " L" }, fmt_number((x * 10.0).round() / 10.0), fmt_number((y * 10.0).round() / 10.0)));
    }
    if close && !pts.is_empty() {
        d.push_str(" Z");
    }
    d
}

pub fn view(cx: &ViewCtx, i: usize) -> VNode {
    let v = Viewport::read(cx, i);
    let mut n = root(cx, i, "div").attr("role", "application").attr("aria-label", "Map").attr("tabindex", "0").on("pointerdown").on("pointermove").on("pointerup").on("pointercancel").on("wheel").on("dblclick").on("keydown").on("click");
    let tile_bg = css_color(&cx.tree.get(i, "TileBackgroundColor"), "#1c1f24");
    n = n.style("background-color", tile_bg);
    let mut layer = VNode::new("div").key("tiles").class("pr-tiles");
    for t in tiles(&v) {
        layer = layer.child(
            VNode::new("img")
                .key(format!("t:{}/{}/{}", t.z, t.x, t.left.round() as i64))
                .attr("src", tile_url(cx.tile_url, &t))
                .attr("alt", "")
                .attr("draggable", "false")
                .attr("decoding", "async")
                .attr("width", "256")
                .attr("height", "256")
                .style("left", px(t.left))
                .style("top", px(t.top)),
        );
    }
    n = n.child(layer);

    // Regions and routes: one SVG over the tiles, in pixel space of the viewport.
    let regions = parse_map_regions(&cx.tree.get(i, "Regions"));
    let routes = parse_map_routes(&cx.tree.get(i, "Routes"));
    if !regions.is_empty() || !routes.is_empty() {
        let mut svg = VNode::svg("svg").key("overlay").class("pr-map-overlay").attr("viewBox", format!("0 0 {} {}", fmt_number(v.w), fmt_number(v.h))).attr("width", fmt_number(v.w)).attr("height", fmt_number(v.h));
        let region_fill = css_color(&cx.tree.get(i, "RegionFillColor"), "rgb(70 130 255 / 0.25)");
        let region_border = cx.tree.get(i, "RegionBorderColor");
        for r in &regions {
            let pts = parse_geometry(&r.geometry);
            if pts.len() < 3 {
                continue;
            }
            let mut p = VNode::svg("path").key(format!("reg:{}", r.id)).attr("d", points_path(&v, &pts, true)).attr("fill", css_color(&r.fill, &region_fill)).attr("data-id", r.id.clone()).class("pr-region");
            let stroke = if r.stroke.is_empty() { region_border.clone() } else { r.stroke.clone() };
            if !stroke.is_empty() {
                p = p.attr("stroke", css_color(&stroke, "#4682ff")).attr("stroke-width", fmt_number(if r.width > 0.0 { r.width as f64 } else { 2.0 }));
            }
            if !r.label.is_empty() {
                p = p.child(VNode::svg("title").text(r.label.clone()));
            }
            svg = svg.child(p);
        }
        let route_default = css_color(&cx.tree.get(i, "RouteColor"), "#2f6fed");
        for r in &routes {
            let pts = parse_geometry(&r.geometry);
            if pts.len() < 2 {
                continue;
            }
            let d = points_path(&v, &pts, false);
            let casing = css_color(&cx.tree.get(i, "RouteCasingColor"), "#ffffff");
            svg = svg.child(VNode::svg("path").key(format!("rc:{}", r.id)).attr("d", d.clone()).attr("fill", "none").attr("stroke", casing).attr("stroke-width", fmt_number(if r.width > 0.0 { r.width as f64 + 3.0 } else { 7.0 })).attr("stroke-linejoin", "round").attr("stroke-linecap", "round"));
            svg = svg.child(
                VNode::svg("path")
                    .key(format!("rt:{}", r.id))
                    .attr("d", d)
                    .attr("fill", "none")
                    .attr("stroke", if r.color.is_empty() { route_default.clone() } else { css_color(&r.color, &route_default) })
                    .attr("stroke-width", fmt_number(if r.width > 0.0 { r.width as f64 } else { 4.0 }))
                    .attr("stroke-linejoin", "round")
                    .attr("stroke-linecap", "round")
                    .class("pr-route"),
            );
        }
        n = n.child(svg);
    }

    // Markers.
    let markers = parse_map_markers(&cx.tree.get(i, "Markers"));
    let open = cx.tree.get(i, "SelectedMarkerId");
    let marker_color = css_color(&cx.tree.get(i, "MarkerColor"), "#e5484d");
    let mut layer = VNode::new("div").key("markers").class("pr-markers");
    let mut card: Option<VNode> = None;
    for (k, m) in markers.iter().enumerate() {
        let (x, y) = v.to_px(m.lat, m.lng);
        if x < -20.0 || y < -40.0 || x > v.w + 20.0 || y > v.h + 20.0 {
            continue;
        }
        let selected = !open.is_empty() && open == m.id;
        let mut b = VNode::new("button")
            .key(format!("m:{k}"))
            .class("pr-marker")
            .attr("type", "button")
            .attr("data-id", m.id.clone())
            .attr("aria-label", if m.label.is_empty() { m.id.clone() } else { m.label.clone() })
            .attr("aria-pressed", if selected { "true" } else { "false" })
            .style("left", px(x))
            .style("top", px(y))
            .style("--pr-marker", marker_color.clone());
        if selected {
            b = b.class("pr-selected");
        }
        layer = layer.child(b);
        if selected && (!m.label.is_empty() || !m.info.is_empty()) {
            let mut c = VNode::new("div").key("card").class("pr-info").attr("role", "dialog").style("left", px(x)).style("top", px(y - 8.0));
            if !m.label.is_empty() {
                c = c.child(VNode::new("strong").key("card-title").text(m.label.clone()));
            }
            if !m.info.is_empty() {
                c = c.child(VNode::new("p").key("card-body").text(m.info.clone()));
            }
            card = Some(c);
        }
    }
    n = n.child(layer);
    if let Some(c) = card {
        n = n.child(c);
    }
    n.child(
        VNode::new("div")
            .key("zoom")
            .class("pr-map-zoom")
            .child(VNode::new("button").key("zin").attr("type", "button").attr("data-part", "zoom-in").attr("aria-label", "Zoom in").text("+"))
            .child(VNode::new("button").key("zout").attr("type", "button").attr("data-part", "zoom-out").attr("aria-label", "Zoom out").text("\u{2212}")),
    )
    .child(VNode::new("small").key("attribution").class("pr-attribution").text("\u{00A9} OpenStreetMap contributors"))
}

#[cfg(test)]
mod tests {
    use super::super::Empty;
    use super::*;
    use crate::tree::testing::tree_with;

    fn map(props: &[(&str, &str)]) -> VNode {
        let mut all = vec![("Width", "512"), ("Height", "512")];
        all.extend_from_slice(props);
        let (t, i) = tree_with("Maps", "MAP-1", &all);
        let e = Empty::default();
        view(&e.cx(&t), i)
    }

    #[test]
    fn the_projection_gives_the_tiles_the_world_knows() {
        let (x, y) = lat_lng_to_tile_frac_at(0.0, 0.0, 1.0);
        assert!((x - 1.0).abs() < 1e-9 && (y - 1.0).abs() < 1e-9, "the origin is the corner of four tiles at zoom 1");
        let (x, y) = lat_lng_to_tile_frac_at(51.5074, -0.1278, 10.0);
        assert_eq!((x.floor() as u32, y.floor() as u32), (511, 340), "London at zoom 10 is tile 511/340");
        let (lat, lng) = tile_frac_to_lat_lng_at(x, y, 10.0);
        assert!((lat - 51.5074).abs() < 1e-9 && (lng + 0.1278).abs() < 1e-9);
        let (_, y) = lat_lng_to_tile_frac_at(89.9, 0.0, 3.0);
        assert!(y >= 0.0 && y < 0.1, "latitude is clamped to Mercator's range");
    }

    #[test]
    fn the_centre_of_the_viewport_is_the_centre_coordinate() {
        let v = Viewport { w: 400.0, h: 300.0, lat: 51.5, lng: -0.12, zoom: 12 };
        let (x, y) = v.to_px(51.5, -0.12);
        assert!((x - 200.0).abs() < 1e-6 && (y - 150.0).abs() < 1e-6);
        let (lat, lng) = v.to_lat_lng(x, y);
        assert!((lat - 51.5).abs() < 1e-6 && (lng + 0.12).abs() < 1e-6);
    }

    #[test]
    fn a_coordinate_round_trips_through_a_pixel() {
        let v = Viewport { w: 640.0, h: 480.0, lat: -22.9, lng: -43.2, zoom: 10 };
        let (x, y) = v.to_px(-22.8, -43.1);
        let (lat, lng) = v.to_lat_lng(x, y);
        assert!((lat + 22.8).abs() < 1e-6 && (lng + 43.1).abs() < 1e-6);
    }

    #[test]
    fn the_tiles_cover_the_viewport_and_x_wraps_around_the_world() {
        let v = Viewport { w: 512.0, h: 512.0, lat: 0.0, lng: 179.9, zoom: 3 };
        let ts = tiles(&v);
        assert!(ts.len() >= 4 && ts.len() <= 9, "{}", ts.len());
        assert!(ts.iter().all(|t| t.x < 8 && t.y < 8), "x wraps into 0..8");
        assert!(ts.iter().any(|t| t.x == 0), "the date line shows the tiles on the other side");
        for t in &ts {
            assert!(t.left > -256.0 - 1e-6 && t.left < 512.0 + 1e-6 && t.top > -256.0 - 1e-6 && t.top < 512.0 + 1e-6, "{t:?}");
        }
        let far_north = Viewport { w: 512.0, h: 512.0, lat: 85.0, lng: 0.0, zoom: 1 };
        assert!(tiles(&far_north).iter().all(|t| t.y < 2), "tiles above the map are skipped");
    }

    #[test]
    fn the_tile_address_comes_from_the_template() {
        let t = Tile { z: 5, x: 17, y: 11, left: 0.0, top: 0.0 };
        assert_eq!(tile_url("https://tiles.example/{z}/{x}/{y}.png", &t), "https://tiles.example/5/17/11.png");
    }

    #[test]
    fn dragging_moves_the_centre_opposite_to_the_drag() {
        let v = Viewport { w: 400.0, h: 300.0, lat: 0.0, lng: 0.0, zoom: 4 };
        let (lat, lng) = v.panned(100.0, 0.0);
        assert!(lng < 0.0 && lat.abs() < 1e-9, "dragging the map right shows what lies to the west: {lat} {lng}");
        let (lat, _) = v.panned(0.0, 100.0);
        assert!(lat > 0.0, "dragging down shows the north");
    }

    #[test]
    fn markers_in_view_are_buttons_and_the_open_one_has_a_card() {
        let n = map(&[("CenterLat", "10"), ("CenterLng", "20"), ("Zoom", "6"), ("Markers", "a\t10\t20\tAlpha\tFirst place\nb\t80\t170\tFar\t")]);
        let buttons = n.count("button");
        assert_eq!(buttons, 1 + 2, "one marker in view, plus the two zoom buttons: {}", n.describe());
        assert!(n.find_tag("img").is_some(), "tiles");
        let open = map(&[("CenterLat", "10"), ("CenterLng", "20"), ("Zoom", "6"), ("Markers", "a\t10\t20\tAlpha\tFirst place"), ("SelectedMarkerId", "a")]);
        let card = open.find(&|x| x.has_class("pr-info")).expect("the info card");
        assert!(card.all_text().contains("Alpha") && card.all_text().contains("First place"));
    }

    #[test]
    fn a_route_and_a_region_are_drawn_over_the_tiles() {
        let n = map(&[("CenterLat", "0"), ("CenterLng", "0"), ("Zoom", "4"), ("Routes", "r1\t#ff0000\t3\t0,0;1,1;2,0"), ("Regions", "z1\t\t\t2\t-1,-1;-1,1;1,1;1,-1\tZone\t")]);
        assert!(n.find_tag("svg").is_some(), "{}", n.describe());
        assert_eq!(n.find_tag("svg").unwrap().count("path"), 3, "a region, and a route with its casing");
    }

    #[test]
    fn nothing_is_requested_from_a_server_the_template_does_not_name() {
        let n = map(&[]);
        let mut imgs = Vec::new();
        n.find_all(&|x| x.tag == "img", &mut imgs);
        assert!(!imgs.is_empty());
        assert!(imgs.iter().all(|i| i.attr_of("src").unwrap().starts_with("https://tile.openstreetmap.org/")));
    }
}
