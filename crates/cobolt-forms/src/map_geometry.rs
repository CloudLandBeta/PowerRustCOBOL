// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The geometry a `Maps` control draws on top of its tiles: traced routes and
//! filled regions.
//!
//! Pure pieces, none of which need egui, so every one is testable without a
//! screen:
//!
//! 1. [`decode_polyline`] — Google's encoded-polyline algorithm. A route comes
//!    back from Directions as one such string, and it is the only compact way
//!    to carry a few thousand points through a COBOL property.
//! 2. [`encode_polyline`] — its inverse, because a Directions answer carries the
//!    road one polyline per navigation step and those pieces have to be decoded,
//!    joined and encoded once to become a single line.
//! 3. [`simplify_polyline`] / [`encode_polyline_within`] — spending a fixed
//!    character budget on the closest line it can buy, since the field this
//!    travels through has a size the developer declared.
//! 4. [`parse_points`] — the other accepted spelling, `lat,lng;lat,lng;…`, for
//!    geometry a developer wrote or computed rather than fetched.
//! 5. [`triangulate`] — ear clipping, so a filled region may be **concave**.
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

/// Encode points back into Google's encoded-polyline form — the inverse of
/// [`decode_polyline`].
///
/// Needed because a Directions answer carries its road geometry in **pieces**:
/// `overview_polyline` is Google's *simplified* line, while the shape that
/// actually follows the road is one polyline per navigation step. Each piece is
/// delta-encoded from its own origin, so the pieces cannot be concatenated as
/// text — they have to be decoded, joined, and encoded once. Without this the
/// route drawn over the map is a smoothed approximation of the road rather than
/// the road.
///
/// Coordinates are rounded to the format's 1e-5 degree grid (about a metre),
/// which is the precision the encoding has and all a map at any usable zoom can
/// show.
pub fn encode_polyline(points: &[LatLng]) -> String {
    let mut out = String::with_capacity(points.len() * 6);
    let (mut prev_lat, mut prev_lng) = (0i64, 0i64);
    for p in points {
        let lat = (p.lat * 1e5).round() as i64;
        let lng = (p.lng * 1e5).round() as i64;
        push_value(&mut out, lat - prev_lat);
        push_value(&mut out, lng - prev_lng);
        prev_lat = lat;
        prev_lng = lng;
    }
    out
}

/// Drop the points that do not change a line's shape — Ramer-Douglas-Peucker,
/// with `epsilon` in degrees.
///
/// Route geometry is delivered through a COBOL field of a size the developer
/// declared, so fidelity is not free: it is spent, and a line that overflows the
/// field is truncated into a route that stops in the middle of nowhere. This is
/// how a long route is fitted to a budget while keeping its **shape** — every
/// bend that matters survives, and it is only the straight runs that give up
/// their redundant points. Decimating "every Nth point" instead is what makes a
/// motorway look hand-drawn: it discards curves and straights alike.
///
/// Fewer than three points, or a non-positive epsilon, returns the input.
pub fn simplify_polyline(points: &[LatLng], epsilon: f64) -> Vec<LatLng> {
    if points.len() < 3 || !(epsilon > 0.0) {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    // Explicit stack rather than recursion: a full-detail intercity route is
    // thousands of points, and the worst case recurses once per point.
    let mut stack = vec![(0usize, points.len() - 1)];
    while let Some((first, last)) = stack.pop() {
        if last <= first + 1 {
            continue;
        }
        let mut worst = (0.0f64, first);
        for (i, p) in points.iter().enumerate().take(last).skip(first + 1) {
            let d = perpendicular_distance(*p, points[first], points[last]);
            if d > worst.0 {
                worst = (d, i);
            }
        }
        if worst.0 > epsilon {
            keep[worst.1] = true;
            stack.push((first, worst.1));
            stack.push((worst.1, last));
        }
    }
    points
        .iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(*p))
        .collect()
}

/// Distance from `p` to the segment `a`-`b`, in degrees.
///
/// Longitude is scaled by cos(latitude) so a degree of longitude counts for
/// what it is worth at that latitude — without it, simplification is far more
/// aggressive east-west than north-south, and at Spanish latitudes that is a
/// 25 % error in one axis only, which shows as a route that straightens across
/// its horizontal bends but not its vertical ones.
fn perpendicular_distance(p: LatLng, a: LatLng, b: LatLng) -> f64 {
    let scale = ((a.lat + b.lat) * 0.5).to_radians().cos().abs().max(1e-6);
    let (px, py) = ((p.lng - a.lng) * scale, p.lat - a.lat);
    let (bx, by) = ((b.lng - a.lng) * scale, b.lat - a.lat);
    let len_sq = bx * bx + by * by;
    if len_sq <= f64::EPSILON {
        return (px * px + py * py).sqrt();
    }
    let t = ((px * bx + py * by) / len_sq).clamp(0.0, 1.0);
    let (dx, dy) = (px - t * bx, py - t * by);
    (dx * dx + dy * dy).sqrt()
}

/// Encode `points`, simplifying only as much as it takes to fit `max_chars`.
///
/// Full detail when it fits — a city trip's road geometry is a few hundred
/// points and costs a couple of kilobytes. A long route gives up its redundant
/// straight-run points, doubling the tolerance until the encoding fits, so the
/// caller always gets the closest line the budget can hold rather than either an
/// arbitrary thumbnail or a truncated one.
pub fn encode_polyline_within(points: &[LatLng], max_chars: usize) -> String {
    let full = encode_polyline(points);
    if full.len() <= max_chars || points.len() < 3 {
        return full;
    }
    // ~1 m, then doubling. Twenty steps reaches ~1 000 km, so the loop always
    // ends on shape rather than on its own iteration count.
    let mut epsilon = 1e-5;
    for _ in 0..20 {
        let encoded = encode_polyline(&simplify_polyline(points, epsilon));
        if encoded.len() <= max_chars {
            return encoded;
        }
        epsilon *= 2.0;
    }
    // Nothing simplified far enough (a degenerate input): the two ends still
    // describe the journey, and they always fit.
    encode_polyline(&[points[0], points[points.len() - 1]])
}

/// One zig-zag-encoded varint appended to `out` — the inverse of [`next_value`].
fn push_value(out: &mut String, value: i64) {
    let mut v = if value < 0 { !(value << 1) } else { value << 1 };
    while v >= 0x20 {
        out.push((((0x20 | (v & 0x1f)) + 63) as u8) as char);
        v >>= 5;
    }
    out.push(((v + 63) as u8) as char);
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

/// Is `p` inside triangle `abc` (edges included)?
///
/// Public because hit-testing a filled region is the same question the ear
/// test asks: a region is its triangles, so "is the pointer in this territory"
/// costs nothing beyond the fill that is already computed.
pub fn point_in_triangle(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
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

    /// The encoder is the decoder's inverse, byte for byte, on Google's own
    /// worked example.
    #[test]
    fn encodes_the_reference_polyline() {
        let reference = "_p~iF~ps|U_ulLnnqC_mqNvxq`@";
        assert_eq!(encode_polyline(&decode_polyline(reference)), reference);
    }

    /// What the encoder exists for: joining the per-step geometry of a
    /// Directions answer into one line. Encoded pieces cannot be concatenated
    /// as text — each is delta-encoded from its own origin — so they are
    /// decoded, joined and encoded once, and the result must decode back to
    /// exactly the points that went in.
    #[test]
    fn joined_steps_survive_a_round_trip() {
        let steps = ["_p~iF~ps|U_ulLnnqC", "_mqNvxq`@", "_p~iF~ps|U"];
        let joined: Vec<LatLng> = steps.iter().flat_map(|s| decode_polyline(s)).collect();
        let back = decode_polyline(&encode_polyline(&joined));
        assert_eq!(back.len(), joined.len());
        for (a, b) in joined.iter().zip(back.iter()) {
            assert!(
                (a.lat - b.lat).abs() < 1e-5 && (a.lng - b.lng).abs() < 1e-5,
                "{a:?} != {b:?}"
            );
        }
    }

    /// Southern/western hemispheres and sub-metre deltas: the sign bit and the
    /// rounding are where a hand-written varint encoder goes wrong.
    #[test]
    fn negative_and_tiny_deltas_round_trip() {
        let pts = vec![
            LatLng {
                lat: -33.86785,
                lng: 151.20732,
            },
            LatLng {
                lat: -33.86786,
                lng: 151.20731,
            },
            LatLng {
                lat: 37.17730,
                lng: -3.59860,
            },
            LatLng { lat: 0.0, lng: 0.0 },
        ];
        let back = decode_polyline(&encode_polyline(&pts));
        assert_eq!(back.len(), pts.len());
        for (a, b) in pts.iter().zip(back.iter()) {
            assert!(
                (a.lat - b.lat).abs() < 1e-5 && (a.lng - b.lng).abs() < 1e-5,
                "{a:?} != {b:?}"
            );
        }
        assert_eq!(encode_polyline(&[]), "");
    }

    /// Simplification drops redundant points and keeps the corners — the whole
    /// difference between fitting a budget and hand-drawing a motorway.
    #[test]
    fn simplification_keeps_the_bend_and_drops_the_straight_run() {
        // A straight run east, then a sharp turn north.
        let pts = vec![
            LatLng {
                lat: 40.0,
                lng: -4.0,
            },
            LatLng {
                lat: 40.0,
                lng: -3.9,
            },
            LatLng {
                lat: 40.0,
                lng: -3.8,
            },
            LatLng {
                lat: 40.0,
                lng: -3.7,
            }, // the corner
            LatLng {
                lat: 40.1,
                lng: -3.7,
            },
            LatLng {
                lat: 40.2,
                lng: -3.7,
            },
        ];
        let simple = simplify_polyline(&pts, 1e-4);
        assert_eq!(simple.len(), 3, "ends plus the corner: {simple:?}");
        assert_eq!(simple[0], pts[0]);
        assert_eq!(simple[1], pts[3], "the corner must survive");
        assert_eq!(simple[2], pts[5]);
        // Ends are never dropped, and a line with nothing to drop is unchanged.
        assert_eq!(simplify_polyline(&pts, 0.0), pts);
        assert_eq!(simplify_polyline(&pts[..2], 1.0).len(), 2);
    }

    /// The budget is a promise: whatever the route, the encoded field fits.
    #[test]
    fn geometry_is_fitted_to_its_budget() {
        // A long, wiggly route — every point genuinely off the straight line,
        // so nothing can be dropped for free.
        let pts: Vec<LatLng> = (0..4000)
            .map(|i| LatLng {
                lat: 40.0 + i as f64 * 1e-3,
                lng: -3.7 + if i % 2 == 0 { 1e-3 } else { -1e-3 },
            })
            .collect();
        let full = encode_polyline(&pts);
        assert!(full.len() > 4_000, "the test route must exceed the budget");

        let fitted = encode_polyline_within(&pts, 4_000);
        assert!(fitted.len() <= 4_000, "fitted to {} chars", fitted.len());
        let back = decode_polyline(&fitted);
        assert!(back.len() >= 2);
        // It is still the same journey: the ends are where they were.
        let near = |a: f64, b: f64| (a - b).abs() < 1e-4;
        assert!(near(back[0].lat, pts[0].lat) && near(back[0].lng, pts[0].lng));
        let (last_in, last_out) = (pts[pts.len() - 1], back[back.len() - 1]);
        assert!(near(last_out.lat, last_in.lat) && near(last_out.lng, last_in.lng));

        // And a route that already fits is handed over untouched — full detail
        // is the normal case, not the exception.
        assert_eq!(
            encode_polyline_within(&pts[..10], 4_000),
            encode_polyline(&pts[..10])
        );
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
