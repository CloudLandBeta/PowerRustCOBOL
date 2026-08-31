// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A road route from **OpenRouteService**, for programs with no Google
//! credential.
//!
//! The Maps control draws whatever geometry it is handed and invents none, so a
//! hand-written waypoint list is exactly as close to the road as its points —
//! eight waypoints cut every curve between them. Road geometry has to come from
//! a routing service. `Directions` is Google's, and needs the project's
//! `google_maps` key; this is the other door.
//!
//! # The key is an argument, not a setting
//!
//! `TraceRoad`'s first argument is the OpenRouteService key, and the platform
//! **never stores it** — not in the project manifest, not in a control property,
//! not in a file. The program asks the operator for it (a `TextBox` on the form)
//! and passes it in. That is deliberate for now: PowerRustCOBOL has no vault
//! yet, and a credential written into a project file travels to everyone the
//! project is shared with. When local/cloud vaults land, this argument becomes
//! the fallback rather than the only way.
//!
//! It travels in an **`Authorization` header**, which is why this POSTs rather
//! than using the simpler GET form OpenRouteService also publishes: that one
//! takes the key as `?api_key=…`, and a credential in a URL ends up in proxy
//! logs, crash reports and any error text that quotes the address it failed to
//! reach. Nothing here logs the key, and no failure path formats it.
//!
//! # What comes back
//!
//! Three TAB-separated fields — `distance_METRES`, `duration_SECONDS`, and the
//! encoded polyline — delivered in `ResponseBody` on `onComplete`, like every
//! other async Maps answer. The numbers are what a program computes with; the
//! polyline goes straight into `AddRoute`.
//!
//! The geometry is fitted to the same [`GEOMETRY_BUDGET`] a `Directions` answer
//! is, and for the same reason: it lands in a COBOL item whose size the
//! developer declared, and a line that overflows it is truncated into a route
//! that stops in the middle of nowhere.

// OpenStreetMap's routing door, reached over plain HTTP rather than through the
// `google_maps` client — so it rides the `http` feature, not `maps`.
#[cfg(feature = "http")]
use cobolt_forms::map_geometry::{decode_polyline, encode_polyline_within};

/// Where the request goes. The `driving-car` profile is the road network a
/// business route means; ORS also publishes walking and cycling profiles, which
/// are a different question and not offered here.
const ENDPOINT: &str = "https://api.openrouteservice.org/v2/directions/driving-car";

/// How many characters of geometry the answer may carry — the same contract a
/// `Directions` answer keeps, so one COBOL field of `PIC X(4096)` holds either.
pub const GEOMETRY_BUDGET: usize = 4_000;

/// Ask OpenRouteService for the road from one point to another.
///
/// Blocking: called from the background worker thread the interpreter already
/// spawns for async Maps operations, never from the interpreter's own thread.
#[cfg(feature = "http")]
pub fn trace_road(
    api_key: &str,
    from_lat: &str,
    from_lng: &str,
    to_lat: &str,
    to_lng: &str,
    timeout_ms: u64,
) -> Result<String, String> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err("TraceRoad: no OpenRouteService key was supplied".to_owned());
    }
    let from = point(from_lat, from_lng, "origin")?;
    let to = point(to_lat, to_lng, "destination")?;

    let body = request_body(from, to);
    let response = crate::http_runtime::agent(timeout_ms)
        .post(ENDPOINT)
        .set("Authorization", key)
        .set("Content-Type", "application/json")
        .send_string(&body);

    let text = match response {
        Ok(r) => r.into_string().map_err(|e| format!("TraceRoad: {e}"))?,
        // A 4xx/5xx carries ORS's own explanation, which is far more useful
        // than the status number: a bad key says so in words.
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_default();
            return Err(match error_message(&body) {
                Some(m) => format!("TraceRoad: {m}"),
                None => format!("TraceRoad: OpenRouteService returned {code}"),
            });
        }
        Err(e) => return Err(format!("TraceRoad: {e}")),
    };
    parse_route(&text)
}

/// One coordinate pair, or a message naming which end was wrong.
/// TraceRoad was left out of the build — it is an HTTP call like any other.
#[cfg(not(feature = "http"))]
pub fn trace_road(
    _api_key: &str,
    _from_lat: &str,
    _from_lng: &str,
    _to_lat: &str,
    _to_lng: &str,
    _timeout_ms: u64,
) -> Result<String, String> {
    Err("TraceRoad is not linked into this program: it reaches OpenRouteService \
         over HTTP, and the build found nothing in this program that uses the \
         HTTP bridge"
        .to_owned())
}

#[cfg(feature = "http")]
fn point(lat: &str, lng: &str, which: &str) -> Result<(f64, f64), String> {
    let lat: f64 = lat
        .trim()
        .parse()
        .map_err(|_| format!("TraceRoad: {which} latitude '{}' is not a number", lat.trim()))?;
    let lng: f64 = lng
        .trim()
        .parse()
        .map_err(|_| format!("TraceRoad: {which} longitude '{}' is not a number", lng.trim()))?;
    Ok((lat, lng))
}

/// The request body. **Coordinates are `[lng, lat]`** — ORS follows GeoJSON's
/// x,y order, which is the reverse of how everything else in this platform (and
/// every COBOL program a developer writes) says a position. Getting it backwards
/// asks for a route between two points in the sea, so it is done in exactly one
/// place.
#[cfg(feature = "http")]
fn request_body(from: (f64, f64), to: (f64, f64)) -> String {
    format!(
        "{{\"coordinates\":[[{},{}],[{},{}]],\"elevation\":false,\"instructions\":false}}",
        from.1, from.0, to.1, to.0
    )
}

/// ORS's own words for a failure, when the body carries them.
#[cfg(feature = "http")]
fn error_message(body: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(body).ok()?;
    let error = parsed.get("error")?;
    // The shape differs by failure: an object with a message, or a bare string.
    error
        .get("message")
        .and_then(|m| m.as_str())
        .or_else(|| error.as_str())
        .map(str::to_owned)
}

/// Pull the answer out of a successful response.
///
/// `elevation: false` keeps the geometry a plain 2-D encoded polyline at the
/// same 1e-5 precision Google uses, so the platform's own decoder reads it and
/// `AddRoute` traces it with no conversion at all.
#[cfg(feature = "http")]
fn parse_route(body: &str) -> Result<String, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("TraceRoad: unreadable answer ({e})"))?;
    let route = parsed
        .get("routes")
        .and_then(|r| r.as_array())
        .and_then(|r| r.first())
        .ok_or_else(|| "TraceRoad: OpenRouteService found no route".to_owned())?;
    let geometry = route
        .get("geometry")
        .and_then(|g| g.as_str())
        .ok_or_else(|| "TraceRoad: the answer carried no geometry".to_owned())?;
    let summary = route.get("summary");
    let number = |key: &str| {
        summary
            .and_then(|s| s.get(key))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
            .round() as i64
    };
    // Re-encoded through the budget rather than passed through: the field this
    // lands in is sized by the developer, and full intercity geometry is
    // comfortably larger than it.
    let points = decode_polyline(geometry);
    if points.len() < 2 {
        return Err("TraceRoad: the answer's geometry decoded to nothing".to_owned());
    }
    Ok(format!(
        "{}\t{}\t{}",
        number("distance"),
        number("duration"),
        encode_polyline_within(&points, GEOMETRY_BUDGET)
    ))
}

#[cfg(all(test, feature = "http"))]
mod tests {
    use super::*;

    /// GeoJSON order is longitude first, and getting it wrong routes between
    /// two points in the sea. Pinned because nothing downstream would notice.
    #[test]
    fn the_request_puts_longitude_first() {
        let body = request_body((40.4168, -3.7038), (37.1773, -3.5986));
        assert_eq!(
            body,
            "{\"coordinates\":[[-3.7038,40.4168],[-3.5986,37.1773]],\
             \"elevation\":false,\"instructions\":false}"
        );
    }

    #[test]
    fn a_coordinate_that_is_not_a_number_says_which_end() {
        let err = point("north", "-3.7", "origin").unwrap_err();
        assert!(err.contains("origin latitude"), "{err}");
        let err = point("40.4", "west", "destination").unwrap_err();
        assert!(err.contains("destination longitude"), "{err}");
        assert_eq!(point(" 40.4 ", " -3.7 ", "origin"), Ok((40.4, -3.7)));
    }

    #[test]
    fn a_good_answer_becomes_metres_seconds_and_geometry() {
        let body = r#"{"routes":[{"summary":{"distance":419876.4,"duration":15234.6},
                       "geometry":"_p~iF~ps|U_ulLnnqC_mqNvxq`@"}]}"#;
        let answer = parse_route(body).expect("a normal answer parses");
        let fields: Vec<&str> = answer.split('\t').collect();
        assert_eq!(fields.len(), 3, "{answer}");
        assert_eq!(fields[0], "419876", "metres, rounded");
        assert_eq!(fields[1], "15235", "seconds, rounded");
        assert_eq!(
            decode_polyline(fields[2]).len(),
            3,
            "the geometry survives the re-encode"
        );
    }

    #[test]
    fn a_missing_route_or_geometry_is_reported_not_guessed() {
        assert!(parse_route(r#"{"routes":[]}"#)
            .unwrap_err()
            .contains("no route"));
        assert!(parse_route(r#"{"routes":[{"summary":{}}]}"#)
            .unwrap_err()
            .contains("no geometry"));
        assert!(parse_route("not json").unwrap_err().contains("unreadable"));
        // A single-point geometry is not a route anyone can draw.
        assert!(parse_route(r#"{"routes":[{"geometry":"_p~iF~ps|U"}]}"#)
            .unwrap_err()
            .contains("decoded to nothing"));
    }

    /// A rejected key is the commonest failure, and ORS explains it in words.
    /// Passing its own sentence on beats reporting "403".
    #[test]
    fn the_services_own_explanation_is_used_when_it_gives_one() {
        let body = r#"{"error":{"code":2099,"message":"Access to this API has been disallowed"}}"#;
        assert_eq!(
            error_message(body).as_deref(),
            Some("Access to this API has been disallowed")
        );
        assert_eq!(
            error_message(r#"{"error":"Quota exceeded"}"#).as_deref(),
            Some("Quota exceeded")
        );
        assert_eq!(error_message("<html>502</html>"), None);
    }

    /// No key, no call: a request that would certainly be refused is refused
    /// here, without a worker thread or a network round trip.
    #[test]
    fn a_blank_key_never_reaches_the_network() {
        let err = trace_road("   ", "40.4", "-3.7", "37.1", "-3.5", 0).unwrap_err();
        assert!(err.contains("no OpenRouteService key"), "{err}");
    }

    /// Long geometry is fitted to the same budget a Directions answer keeps, so
    /// one `PIC X(4096)` field holds either provider's answer.
    #[test]
    fn the_geometry_honours_the_same_budget_as_directions() {
        assert_eq!(GEOMETRY_BUDGET, 4_000);
        let points: Vec<cobolt_forms::map_geometry::LatLng> = (0..4000)
            .map(|i| cobolt_forms::map_geometry::LatLng {
                lat: 40.0 + i as f64 * 1e-3,
                lng: -3.7 + if i % 2 == 0 { 1e-3 } else { -1e-3 },
            })
            .collect();
        let long = cobolt_forms::map_geometry::encode_polyline(&points);
        assert!(long.len() > GEOMETRY_BUDGET);
        let body = format!(
            "{{\"routes\":[{{\"summary\":{{\"distance\":1,\"duration\":1}},\"geometry\":\"{long}\"}}]}}"
        );
        let answer = parse_route(&body).expect("parses");
        let geometry = answer.split('\t').nth(2).expect("three fields");
        assert!(geometry.len() <= GEOMETRY_BUDGET, "{}", geometry.len());
    }
}
