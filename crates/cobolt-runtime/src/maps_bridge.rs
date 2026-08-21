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
                route.overview_polyline.points,
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
