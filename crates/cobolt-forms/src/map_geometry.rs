// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The geometry a `Maps` control draws on top of its tiles: traced routes and
//! filled regions.
//!
//! Three pure pieces, none of which need egui, so all three are testable
//! without a screen:
//!
//! 1. [`decode_polyline`] — Google's encoded-polyline algorithm. A route comes
//!    back from Directions as one such string, and it is the only compact way
//!    to carry a few thousand points through a COBOL property.
//! 2. [`parse_points`] — the other accepted spelling, `lat,lng;lat,lng;…`, for
//!    geometry a developer wrote or computed rather than fetched.
//! 3. [`triangulate`] — ear clipping, so a filled region may be **concave**.
//!    A delivery zone, a municipal boundary or a sales territory almost never
//!    is convex, and epaint's `convex_polygon` silently renders those wrong.

/// A geographic point, in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LatLng {
    pub lat: f64,
    pub lng: f64,
}

/// Decode Google's [encoded polyline algorithm][alg] into points.
///
/// [alg]: https://developers.google.com/maps/documentation/utilities/polylinealgorithm
///
/// Written here rather than taken from `google_maps`'s own decoder, which is
/// behind that crate's `polyline` + `geo` features — pulling `geo` in for
/// twenty lines of varint arithmetic is a poor trade, and this is needed in
/// `cobolt-forms`, which does not depend on `google_maps` at all.
///
/// Malformed input yields the points decoded so far rather than an error: a
/// truncated route should draw the part that survived, not vanish.
pub fn decode_polyline(encoded: &str) -> Vec<LatLng> {
    let bytes = encoded.as_bytes();
    let mut out = Vec::new();
    let (mut lat, mut lng) = (0i64, 0i64);
    let mut i = 0;
    while i < bytes.len() {
        let Some(dlat) = next_value(bytes, &mut i) else {
            break;
        };
        let Some(dlng) = next_value(bytes, &mut i) else {
            break;
        };
        lat += dlat;
        lng += dlng;
        out.push(LatLng {
            lat: lat as f64 / 1e5,
            lng: lng as f64 / 1e5,
        });
    }
    out
}

/// One zig-zag-encoded varint from `bytes`, advancing `i` past it.
fn next_value(bytes: &[u8], i: &mut usize) -> Option<i64> {
    let mut result = 0i64;
    let mut shift = 0u32;
    loop {
        if *i >= bytes.len() || shift > 63 {
            return None;
        }
        let b = bytes[*i] as i64 - 63;
        *i += 1;
        if !(0..=0x3f).contains(&b) {
            return None; // not a chunk of this encoding
        }
        result |= (b & 0x1f) << shift;
        shift += 5;
        if b < 0x20 {
            break;
        }
    }
    // Zig-zag: the low bit is the sign.
    Some(if result & 1 != 0 {
        !(result >> 1)
    } else {
        result >> 1
    })
}

/// Parse `lat,lng;lat,lng;…` (semicolons, spaces or newlines between pairs).
///
/// Returns `None` when the text is not that shape at all, which is how
/// [`parse_geometry`] tells a hand-written point list from an encoded polyline
/// without needing a prefix or a mode flag on the property.
pub fn parse_points(text: &str) -> Option<Vec<LatLng>> {
    let mut out = Vec::new();
    for pair in text
        .split([';', ' ', '\n', '\r', '\t'])
        .filter(|s| !s.trim().is_empty())
    {
        let (a, b) = pair.split_once(',')?;
        let lat: f64 = a.trim().parse().ok()?;
        let lng: f64 = b.trim().parse().ok()?;
        if !lat.is_finite() || !lng.is_finite() {
            return None;
        }
        out.push(LatLng { lat, lng });
    }
    (!out.is_empty()).then_some(out)
}

/// The geometry of one route or region, however it was written.
///
/// A point list is tried first because it is unambiguous — every token must be
/// two finite numbers around a comma. Anything else is handed to the polyline
/// decoder, which is what a Directions result carries.
pub fn parse_geometry(text: &str) -> Vec<LatLng> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    parse_points(text).unwrap_or_else(|| decode_polyline(text))
}

/// Triangulate a simple polygon by ear clipping — indices into `pts`, three per
/// triangle.
///
/// Regions have to be fillable when they are **concave**, which is nearly
/// always: a delivery zone follows streets and a municipal boundary follows a
/// river. epaint fills a path by assuming convexity, so a concave region drawn
/// that way spills colour across its own notches. Clipping to triangles first
/// costs ~80 lines and renders any simple polygon correctly.
///
/// Self-intersecting input has no correct filling, and none is attempted: the
/// loop gives up rather than spin, returning what it has.
pub fn triangulate(pts: &[(f32, f32)]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    // Work counter-clockwise so the ear test has one orientation to reason
    // about, and remember the mapping back to the caller's indices.
    let ccw = signed_area(pts) > 0.0;
    let mut remaining: Vec<usize> = if ccw {
        (0..n).collect()
    } else {
        (0..n).rev().collect()
    };

    let mut out = Vec::with_capacity(n.saturating_sub(2));
    // Each successful clip removes one vertex; the guard bounds the pathological
    // (self-intersecting) case instead of looping forever.
    let mut guard = n * n;
    while remaining.len() > 3 && guard > 0 {
        guard -= 1;
        let m = remaining.len();
        let mut clipped = false;
        for k in 0..m {
            let (ia, ib, ic) = (
                remaining[(k + m - 1) % m],
                remaining[k],
                remaining[(k + 1) % m],
            );
            if !is_ear(pts, &remaining, ia, ib, ic) {
                continue;
            }
            out.push([ia, ib, ic]);
            remaining.remove(k);
            clipped = true;
            break;
        }
        if !clipped {
            break; // not a simple polygon; stop with what we have
        }
    }
    if remaining.len() == 3 {
        out.push([remaining[0], remaining[1], remaining[2]]);
    }
    out
}

/// Twice the signed area — positive when the ring is counter-clockwise in a
/// y-up sense. (Screen space is y-down, so this reads "clockwise on screen";
/// only the sign's consistency matters here, not which name it goes by.)
fn signed_area(pts: &[(f32, f32)]) -> f32 {
    let n = pts.len();
    let mut sum = 0.0;
    for i in 0..n {
        let (x1, y1) = pts[i];
        let (x2, y2) = pts[(i + 1) % n];
        sum += x1 * y2 - x2 * y1;
    }
    sum
}

fn cross(o: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

/// Is `b` the tip of an ear? Convex there, and no other remaining vertex inside
/// the triangle it would cut off.
fn is_ear(pts: &[(f32, f32)], remaining: &[usize], ia: usize, ib: usize, ic: usize) -> bool {
    let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
    if cross(a, b, c) <= 0.0 {
        return false; // reflex (or degenerate) — not an ear
    }
    for &i in remaining {
        if i == ia || i == ib || i == ic {
            continue;
        }
        if point_in_triangle(pts[i], a, b, c) {
            return false;
        }
    }
    true
}

fn point_in_triangle(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let d1 = cross(a, b, p);
    let d2 = cross(b, c, p);
    let d3 = cross(c, a, p);
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Google's own worked example from the algorithm's documentation.
    #[test]
    fn decodes_the_reference_polyline() {
        let pts = decode_polyline("_p~iF~ps|U_ulLnnqC_mqNvxq`@");
        assert_eq!(pts.len(), 3);
        let near = |a: f64, b: f64| (a - b).abs() < 1e-5;
        assert!(near(pts[0].lat, 38.5) && near(pts[0].lng, -120.2), "{:?}", pts[0]);
        assert!(near(pts[1].lat, 40.7) && near(pts[1].lng, -120.95), "{:?}", pts[1]);
        assert!(near(pts[2].lat, 43.252) && near(pts[2].lng, -126.453), "{:?}", pts[2]);
    }

    /// A truncated route draws the part that survived rather than vanishing.
    #[test]
    fn a_truncated_polyline_keeps_what_decoded() {
        let full = "_p~iF~ps|U_ulLnnqC_mqNvxq`@";
        let cut = &full[..full.len() - 3];
        let pts = decode_polyline(cut);
        assert!(!pts.is_empty(), "the first points are still good");
        assert!(pts.len() < 3);
    }

    #[test]
    fn empty_and_junk_decode_to_nothing_rather_than_panicking() {
        assert!(decode_polyline("").is_empty());
        assert!(parse_geometry("").is_empty());
        // Every byte below the encoding's own range.
        assert!(decode_polyline("\u{1}\u{2}\u{3}").is_empty());
    }

    /// The two accepted spellings are told apart without a prefix or a flag.
    #[test]
    fn a_point_list_is_not_mistaken_for_a_polyline() {
        let pts = parse_geometry("-23.5614,-46.6558; -23.5505,-46.6333");
        assert_eq!(pts.len(), 2);
        assert!((pts[0].lat + 23.5614).abs() < 1e-9);
        assert!((pts[1].lng + 46.6333).abs() < 1e-9);

        // …and an encoded route is not mistaken for a point list.
        assert_eq!(parse_geometry("_p~iF~ps|U_ulLnnqC").len(), 2);
    }

    #[test]
    fn a_malformed_pair_is_not_half_parsed() {
        assert!(parse_points("1,2;notapair").is_none());
        assert!(parse_points("1,2;3").is_none());
        assert!(parse_points("").is_none());
    }

    /// A square: two triangles, every vertex used.
    #[test]
    fn a_convex_ring_triangulates_into_n_minus_two() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let tris = triangulate(&sq);
        assert_eq!(tris.len(), 2, "n-2 triangles for n=4");
    }

    /// **The reason this exists.** An L — concave — must still fill correctly,
    /// which means every one of its n-2 triangles, and none of them straying
    /// outside the shape.
    #[test]
    fn a_concave_ring_is_filled_correctly() {
        // An L-shape, six vertices, one reflex corner.
        let l = [
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 4.0),
            (4.0, 4.0),
            (4.0, 10.0),
            (0.0, 10.0),
        ];
        let tris = triangulate(&l);
        assert_eq!(tris.len(), 4, "n-2 triangles for n=6, got {tris:?}");

        // The notch — outside the L, inside its bounding box — must be covered
        // by NO triangle. A convex fill would swallow it, which is the bug.
        let notch = (7.0f32, 7.0f32);
        let covered = tris.iter().any(|t| {
            point_in_triangle(notch, l[t[0]], l[t[1]], l[t[2]])
        });
        assert!(!covered, "the concave notch was filled in: {tris:?}");

        // …while a point genuinely inside the L is covered.
        let inside = (2.0f32, 2.0f32);
        assert!(
            tris.iter()
                .any(|t| point_in_triangle(inside, l[t[0]], l[t[1]], l[t[2]])),
            "a point inside the region was left unfilled"
        );
    }

    /// Winding order is the caller's business, not ours — a ring given
    /// clockwise fills the same as the same ring given counter-clockwise.
    #[test]
    fn winding_order_does_not_change_the_result() {
        let cw = [(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)];
        let ccw = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        assert_eq!(triangulate(&cw).len(), triangulate(&ccw).len());
        assert_eq!(triangulate(&cw).len(), 2);
    }

    /// A Directions answer traces straight onto the map: the sixth field of
    /// `ResponseBody` is an encoded polyline, and `AddRoute` takes it verbatim.
    #[test]
    fn a_directions_answer_is_route_geometry_as_it_stands() {
        // The shape maps_bridge returns: text, text, summary, metres, seconds,
        // polyline.
        let response = "72,4 km\t1 hour 5 mins\tRodovia dos Imigrantes\t72400\t3900\t_p~iF~ps|U_ulLnnqC";
        let polyline = response.split('\t').nth(5).expect("six fields");
        let pts = parse_geometry(polyline);
        assert_eq!(pts.len(), 2, "the route decoded without any massaging");
    }

    /// Degenerate input is not a crash and not an infinite loop.
    #[test]
    fn degenerate_rings_terminate() {
        assert!(triangulate(&[]).is_empty());
        assert!(triangulate(&[(0.0, 0.0)]).is_empty());
        assert!(triangulate(&[(0.0, 0.0), (1.0, 1.0)]).is_empty());
        // A bow-tie has no correct filling; it must stop, not spin.
        let bowtie = [(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0)];
        let _ = triangulate(&bowtie);
    }
}
