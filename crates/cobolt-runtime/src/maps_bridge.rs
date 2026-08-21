// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `google_maps` half of the Maps control (spec 039 T11) — Directions,
//! Geocoding, Distance-Matrix, and Places data, never the OpenStreetMap
//! basemap tiles (that is `cobolt-forms::map_tiles`, entirely independent
//! of this module and of any API key).
//!
//! `google_maps` is `reqwest`+async; this crate's interpreter is
//! synchronous and stays that way (plan.md §4 Decision 5) — [`run`] is
//! called from inside the background worker thread `Interpreter::
//! spawn_maps_op` already spawns, and privately builds a minimal
//! current-thread `tokio::Runtime` just for the one blocking `.block_on()`
//! call this function makes. Nothing here ever runs on the interpreter's
//! own thread.
//!
//! Every verb formats its result as a single tab-separated line (or
//! newline-separated lines for a multi-result verb like `PLACESSEARCH`) —
//! the same convention `MapMarkerRecord` and other multi-field properties
//! already use — rather than handing raw JSON back to COBOL, so a
//! developer's handler can `UNSTRING` it directly.

/// Run one Maps data verb synchronously (blocking the calling — background
/// — thread only). `args` are the verb's own parameters, already
/// COBOL-value-to-string converted by the caller.
pub fn run(api_key: &str, verb: &str, args: &[String]) -> Result<String, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("could not start the Maps async runtime: {e}"))?;
    runtime.block_on(run_async(api_key, verb, args))
}

async fn run_async(api_key: &str, verb: &str, args: &[String]) -> Result<String, String> {
    let client = google_maps::Client::try_new(api_key)
        .map_err(|e| format!("Maps client: {e}"))?;
    let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");

    match verb {
        "GEOCODE" => {
            let address = arg(0);
            let response = client
                .geocoding()
                .with_address(address)
                .execute()
                .await
                .map_err(|e| format!("Geocode: {e}"))?;
            let first = response
                .results
                .first()
                .ok_or_else(|| "Geocode: no results".to_owned())?;
            Ok(format!(
                "{}\t{}\t{}",
                first.geometry.location.lat, first.geometry.location.lng, first.formatted_address
            ))
        }
        "REVERSEGEOCODE" => {
            let lat: f64 = arg(0).trim().parse().map_err(|_| "bad latitude".to_owned())?;
            let lng: f64 = arg(1).trim().parse().map_err(|_| "bad longitude".to_owned())?;
            let latlng = google_maps::LatLng::try_from_f64(lat, lng)
                .map_err(|e| format!("ReverseGeocode: {e}"))?;
            let response = client
                .reverse_geocoding(latlng)
                .execute()
                .await
                .map_err(|e| format!("ReverseGeocode: {e}"))?;
            let first = response
                .results
                .first()
                .ok_or_else(|| "ReverseGeocode: no results".to_owned())?;
            Ok(first.formatted_address.clone())
        }
        "DIRECTIONS" => {
            let origin = arg(0);
            let destination = arg(1);
            // `departure_time: now` is what makes Google answer with CURRENT
            // TRAFFIC as well as free-flow. Without it there is no
            // `duration_in_traffic` at all — the field simply does not come
            // back — so "how long will this delivery take, leaving now" was
            // unanswerable. Traffic is not available to us as a map overlay
            // (Google exposes that only through its own JS/mobile SDKs, never
            // as tiles), but as a NUMBER it is, and a number is what a business
            // program can act on anyway.
            let response = client
                .directions(
                    google_maps::directions::request::location::Location::from_address(origin),
                    google_maps::directions::request::location::Location::from_address(
                        destination,
                    ),
                )
                .with_departure_time(google_maps::directions::request::departure_time::DepartureTime::Now)
                .execute()
                .await
                .map_err(|e| format!("Directions: {e}"))?;
            let route = response
                .routes
                .first()
                .ok_or_else(|| "Directions: no route".to_owned())?;
            let leg = route
                .legs
                .first()
                .ok_or_else(|| "Directions: route has no legs".to_owned())?;
            // Six fields, and the first three are the ones this always
            // returned — appended to, never reordered, so COBOL that already
            // UNSTRINGs three keeps working.
            //
            // The numbers matter as much as the text: `72,4 km` is something to
            // display, `72400` is something to COMPUTE with, and a business
            // language that can only print a distance cannot charge for it. The
            // encoded polyline is the route itself, for `AddRoute` to trace.
            // Field 7 is the drive time WITH current traffic, in seconds — 0
            // when Google did not supply one (some routes and modes have no
            // traffic model). Appended, like the others, so nothing that reads
            // the earlier fields has to change.
            let traffic_seconds = leg
                .duration_in_traffic
                .as_ref()
                .map(|d| d.value.num_seconds())
                .unwrap_or(0);
            Ok(format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                leg.distance.text,
                leg.duration.text,
                route.summary,
                leg.distance.value,
                leg.duration.value.num_seconds(),
                road_polyline(route),
                traffic_seconds,
            ))
        }
        "DISTANCEMATRIX" => {
            let origin = arg(0);
            let destination = arg(1);
            let response = client
                .distance_matrix(
                    vec![google_maps::directions::request::waypoint::Waypoint::from_address(
                        origin,
                    )],
                    vec![google_maps::directions::request::waypoint::Waypoint::from_address(
                        destination,
                    )],
                )
                .execute()
                .await
                .map_err(|e| format!("DistanceMatrix: {e}"))?;
            let element = response
                .rows
                .first()
                .and_then(|row| row.elements.first())
                .ok_or_else(|| "DistanceMatrix: no result".to_owned())?;
            let distance = element
                .distance
                .as_ref()
                .map(|d| d.text.clone())
                .unwrap_or_default();
            let duration = element
                .duration
                .as_ref()
                .map(|d| d.text.clone())
                .unwrap_or_default();
            // Metres and seconds appended after the two display strings — see
            // DIRECTIONS above for why the numbers are the useful half.
            let meters = element.distance.as_ref().map(|d| d.value).unwrap_or(0);
            let seconds = element
                .duration
                .as_ref()
                .map(|d| d.value.num_seconds())
                .unwrap_or(0);
            Ok(format!("{distance}\t{duration}\t{meters}\t{seconds}"))
        }
        "PLACESSEARCH" => {
            let query = arg(0);
            let radius: u32 = arg(1).trim().parse().unwrap_or(5_000);
            let response = client
                .text_search(query, radius)
                .execute()
                .await
                .map_err(|e| format!("PlacesSearch: {e}"))?;
            let lines: Vec<String> = response
                .results
                .iter()
                .map(|p| {
                    let name = p.name.clone().unwrap_or_default();
                    let address = p.formatted_address.clone().unwrap_or_default();
                    let place_id = p.place_id.clone().unwrap_or_default();
                    let (lat, lng) = p
                        .geometry
                        .as_ref()
                        .map(|g| (g.location.lat.to_string(), g.location.lng.to_string()))
                        .unwrap_or_default();
                    format!("{place_id}\t{name}\t{address}\t{lat}\t{lng}")
                })
                .collect();
            Ok(lines.join("\n"))
        }
        other => Err(format!("Maps: unknown verb '{other}'")),
    }
}

/// The route geometry that actually follows the road.
///
/// `route.overview_polyline` is Google's **simplified** line: it is meant for a
/// thumbnail, and over a few hundred kilometres it cuts corners the road does
/// not — a trace drawn from it visibly leaves the motorway, which is exactly
/// what "a lousy approximation" looks like on screen.
///
/// The shape that follows the road is per navigation **step**. Each step is
/// delta-encoded from its own origin, so the encoded strings cannot be joined
/// as text: they are decoded, joined, and encoded once. Consecutive steps share
/// an endpoint — the end of one is the start of the next — and that duplicate
/// is dropped, because a repeated point is a zero-length segment that widens
/// the join under a thick stroke.
///
/// The geometry is then fitted to [`GEOMETRY_BUDGET`] — full detail when it
/// fits, and otherwise simplified to the closest line that does, because this
/// field lands in a COBOL item of a size the developer declared and a line that
/// overflows it is truncated into a route that stops halfway.
///
/// Falls back to the overview line when a response carries no steps at all: a
/// coarse route still beats no route.
fn road_polyline(route: &google_maps::directions::response::route::Route) -> String {
    let points = join_steps(
        route
            .legs
            .iter()
            .flat_map(|leg| leg.steps.iter().map(|step| step.polyline.points.as_str())),
    );
    if points.len() < 2 {
        return route.overview_polyline.points.clone();
    }
    cobolt_forms::map_geometry::encode_polyline_within(&points, GEOMETRY_BUDGET)
}

/// Decode each step's polyline and join them into one path, dropping the point
/// a step shares with the one before it.
///
/// Split out from [`road_polyline`] so the joining rule can be tested: the rest
/// of that function needs a live `Directions` response, and a rule only
/// exercised by a credentialed network call is a rule nothing checks.
fn join_steps<'a>(steps: impl Iterator<Item = &'a str>) -> Vec<cobolt_forms::map_geometry::LatLng> {
    let mut points: Vec<cobolt_forms::map_geometry::LatLng> = Vec::new();
    for step in steps {
        for p in cobolt_forms::map_geometry::decode_polyline(step) {
            let repeats_previous = points
                .last()
                .map(|last: &cobolt_forms::map_geometry::LatLng| {
                    (last.lat - p.lat).abs() < 1e-7 && (last.lng - p.lng).abs() < 1e-7
                })
                .unwrap_or(false);
            if !repeats_previous {
                points.push(p);
            }
        }
    }
    points
}

/// How many characters of route geometry a `Directions` answer may carry.
///
/// Field 6 is UNSTRINGed into a COBOL item, so its size is a contract, not an
/// implementation detail: 4,000 characters fit the `PIC X(4096)` the guide's
/// worked example declares, with room for the field's own trailing spaces.
/// Before this cap existed the field carried `overview_polyline`, whose length
/// is bounded by nothing at all — so a long enough route could already
/// overflow whatever the developer declared and truncate silently.
const GEOMETRY_BUDGET: usize = 4_000;

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::map_geometry::{decode_polyline, encode_polyline, LatLng};

    fn pt(lat: f64, lng: f64) -> LatLng {
        LatLng { lat, lng }
    }

    /// Google ends each step where the next one begins. Keeping both copies
    /// puts a zero-length segment at every turn, which a thick stroke draws as
    /// a blob — so the shared point is dropped exactly once per join.
    #[test]
    fn the_point_two_steps_share_is_kept_once() {
        let a = encode_polyline(&[pt(40.4168, -3.7038), pt(40.3000, -3.6800)]);
        let b = encode_polyline(&[pt(40.3000, -3.6800), pt(40.0300, -3.6000)]);
        let joined = join_steps([a.as_str(), b.as_str()].into_iter());
        assert_eq!(joined.len(), 3, "four points, one shared: {joined:?}");
        let near = |x: f64, y: f64| (x - y).abs() < 1e-5;
        assert!(near(joined[1].lat, 40.3000) && near(joined[1].lng, -3.6800));
        assert!(near(joined[2].lat, 40.0300) && near(joined[2].lng, -3.6000));
    }

    /// Two *different* points that merely sit close together are both kept —
    /// the rule is "the same point twice in a row", not "points near each
    /// other", or a hairpin bend would lose its apex.
    #[test]
    fn distinct_neighbouring_points_both_survive() {
        let steps = encode_polyline(&[pt(37.1773, -3.5986), pt(37.1774, -3.5987)]);
        assert_eq!(join_steps([steps.as_str()].into_iter()).len(), 2);
    }

    /// No steps at all yields nothing, which is what makes `road_polyline`
    /// fall back to the overview line instead of returning an empty route.
    #[test]
    fn no_steps_means_no_points() {
        assert!(join_steps(std::iter::empty()).is_empty());
        assert!(join_steps(["".as_ref()].into_iter()).is_empty());
    }

    /// The whole point of joining: the result is one line through every step's
    /// geometry, in order, and it survives the encode the caller performs.
    #[test]
    fn the_joined_route_is_one_line_in_step_order() {
        let steps: Vec<String> = vec![
            encode_polyline(&[pt(40.4168, -3.7038), pt(40.2000, -3.6500)]),
            encode_polyline(&[pt(40.2000, -3.6500), pt(38.7600, -3.3800)]),
            encode_polyline(&[pt(38.7600, -3.3800), pt(37.1773, -3.5986)]),
        ];
        let joined = join_steps(steps.iter().map(String::as_str));
        assert_eq!(joined.len(), 4);
        let back = decode_polyline(&cobolt_forms::map_geometry::encode_polyline_within(
            &joined,
            GEOMETRY_BUDGET,
        ));
        assert_eq!(back.len(), 4, "well under the budget, so nothing is dropped");
        let near = |x: f64, y: f64| (x - y).abs() < 1e-5;
        assert!(near(back[0].lat, 40.4168), "starts in Madrid: {:?}", back[0]);
        assert!(near(back[3].lat, 37.1773), "ends in Granada: {:?}", back[3]);
    }
}
