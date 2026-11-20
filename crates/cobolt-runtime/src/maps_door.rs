// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Maps control's own rules**, in one place for every host that runs one: the interpreter,
//! and the desktop runner of a program built on the WebAssembly path (DESKTOP-WASM-001 T48).
//!
//! Its places and directions (`Geocode`, `ReverseGeocode`, `Directions`, `DistanceMatrix`,
//! `PlacesSearch`) ask Google's Maps services with the key the host resolved for the project
//! (`_ResolvedMapsApiKey`, never a literal on the control, spec 039 R31); `TraceRoad` asks
//! OpenRouteService with the key the program passes as its first argument. Each runs on a thread of
//! its own, one at a time per control, whatever the control's `Mode`, with the RestClient's `Busy`,
//! `TimeoutMs` and endings ([`crate::rest_door`]): the answer lands in `ResponseBody` and raises
//! `onComplete`, a failure writes `LastError` and raises `onError`. One with no key sends nothing
//! and raises `onError` at once.
//!
//! Its markers, routes and regions are TAB-separated lines in plain properties (`Markers`, `Routes`,
//! `Regions`), which `AddMarker`, `RemoveMarker`, `AddRoute`, `RemoveRoute`, `ClearRoutes`,
//! `AddRegion`, `RemoveRegion` and `ClearRegions` edit.
//!
//! The control's properties are read through a getter, so neither host's storage leaks in; the
//! host keeps the one-at-a-time rule ([`is_service`]), as it holds the running requests.

use crate::rest_door::{self, Effects, Prop};

/// What a Maps method asks for.
#[derive(Clone, Debug)]
pub enum Start {
    /// Not one of the control's own methods (`Cancel`, `IsBusy`, a property): the host's own.
    NotOwn,
    /// A collection method: the property it writes, and the property's new value.
    Set(&'static str, String),
    /// A service method that has no key: nothing is sent, and this is what follows.
    NotConfigured(Effects),
    /// A service method: what [`run`] does on a thread of its own.
    Request(Request),
}

/// Whom a service method asks.
#[derive(Clone, Debug)]
enum Service {
    /// Google's Maps services: the verb (upper case) and the project's key.
    Google { verb: &'static str, key: String },
    /// OpenRouteService's road route (`TraceRoad`): the key is the first argument.
    Ors,
}

/// A service method's request, run by [`run`].
#[derive(Clone, Debug)]
pub struct Request {
    service: Service,
    args: Vec<String>,
    /// The control's `TimeoutMs` (0: none), as a RestClient's ([`rest_door::timeout_ms`]).
    pub timeout_ms: u64,
}

/// The service methods, with the number of arguments each reads.
const SERVICES: [(&str, usize); 6] = [("GEOCODE", 1), ("REVERSEGEOCODE", 2), ("DIRECTIONS", 2), ("DISTANCEMATRIX", 2), ("PLACESSEARCH", 2), ("TRACEROAD", 5)];

/// Is `method` (upper case) one of the methods that ask a service? While one of the control's runs,
/// a second is ignored and answers nothing (spec 032 R6), which the host checks before [`start`].
pub fn is_service(method: &str) -> bool {
    SERVICES.iter().any(|(m, _)| *m == method)
}

/// What the Maps control asks for with the method `method` (upper case) and the argument texts
/// `args` (each trimmed; one the call does not pass reads as empty).
pub fn start(prop: Prop<'_>, method: &str, args: &[String]) -> Start {
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
    if let Some(&(verb, n)) = SERVICES.iter().find(|(m, _)| *m == method) {
        let args: Vec<String> = (0..n).map(arg).collect();
        let service = if verb == "TRACEROAD" {
            if args[0].trim().is_empty() {
                // Same shape as a missing Maps key: no thread and no request that would certainly be
                // refused.
                return not_configured("TraceRoad: no OpenRouteService key was supplied");
            }
            Service::Ors
        } else {
            let key = prop("_ResolvedMapsApiKey");
            if key.trim().is_empty() {
                // R33: "not configured", rather than a request that would answer 400.
                return not_configured("Maps API key not configured");
            }
            Service::Google { verb, key }
        };
        return Start::Request(Request { service, args, timeout_ms: rest_door::timeout_ms(prop) });
    }
    match method {
        "ADDMARKER" => {
            let line = format!("{}\t{}\t{}\t{}\t{}", arg(0), arg(1), arg(2), arg(3), arg(4));
            let existing = prop("Markers");
            Start::Set("Markers", if existing.trim().is_empty() { line } else { format!("{existing}\n{line}") })
        }
        "REMOVEMARKER" => {
            let id = arg(0);
            // Unlike a route or a region, a marker keeps the blank lines around it.
            let markers = prop("Markers");
            let kept: Vec<&str> = markers.lines().filter(|l| l.split('\t').next().unwrap_or("") != id).collect();
            Start::Set("Markers", kept.join("\n"))
        }
        // `geometry` is either an encoded polyline, exactly what `Directions` answers in its sixth
        // field, or an explicit `lat,lng;lat,lng;…` list; the renderer tells them apart.
        "ADDROUTE" => {
            let line = format!("{}\t{}\t{}\t{}", arg(0), arg(1), arg(2), arg(3));
            Start::Set("Routes", append_collection_line(&prop("Routes"), &line))
        }
        "REMOVEROUTE" => Start::Set("Routes", remove_collection_line(&prop("Routes"), &arg(0))),
        "CLEARROUTES" => Start::Set("Routes", String::new()),
        // The label and the info come after the geometry and are optional, so the five-argument
        // call that shipped first still works and has no info window.
        "ADDREGION" => {
            let line = format!("{}\t{}\t{}\t{}\t{}\t{}\t{}", arg(0), arg(1), arg(2), arg(3), arg(4), arg(5), arg(6));
            Start::Set("Regions", append_collection_line(&prop("Regions"), &line))
        }
        "REMOVEREGION" => Start::Set("Regions", remove_collection_line(&prop("Regions"), &arg(0))),
        "CLEARREGIONS" => Start::Set("Regions", String::new()),
        _ => Start::NotOwn,
    }
}

/// A service method that cannot start: `LastError` says why and `onError` is raised.
fn not_configured(why: &str) -> Start {
    Start::NotConfigured(Effects { props: vec![("LastError".into(), why.to_owned())], events: vec!["onError".into()], answer: String::new() })
}

/// Run `request` (blocking: on the thread the host starts for it), answering the body and the status
/// for [`rest_door::delivered`]: the answer and 200, or why it failed and 0.
pub fn run(request: &Request) -> (String, u16) {
    let a = |i: usize| request.args.get(i).map(String::as_str).unwrap_or("");
    let outcome = match &request.service {
        Service::Google { verb, key } => crate::maps_bridge::run(key, verb, &request.args),
        Service::Ors => crate::ors_bridge::trace_road(a(0), a(1), a(2), a(3), a(4), request.timeout_ms),
    };
    match outcome {
        Ok(body) => (body, 200),
        Err(message) => (message, 0),
    }
}

/// `existing` with `line` in place of the line with the same id (its first field), or after the
/// others; blank lines go.
fn append_collection_line(existing: &str, line: &str) -> String {
    let id = line.split('\t').next().unwrap_or("");
    let mut out: Vec<&str> = existing.lines().filter(|l| !l.trim().is_empty() && l.split('\t').next().unwrap_or("") != id).collect();
    out.push(line);
    out.join("\n")
}

/// `existing` without the line with this id; blank lines go.
fn remove_collection_line(existing: &str, id: &str) -> String {
    existing.lines().filter(|l| !l.trim().is_empty() && l.split('\t').next().unwrap_or("") != id).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn props(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_ascii_uppercase(), (*v).to_owned())).collect()
    }

    fn start_with(p: &HashMap<String, String>, method: &str, args: &[&str]) -> Start {
        let get = |k: &str| p.get(&k.to_ascii_uppercase()).cloned().unwrap_or_default();
        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        start(&get, method, &args)
    }

    fn set(s: Start) -> (&'static str, String) {
        match s {
            Start::Set(p, v) => (p, v),
            other => panic!("expected a collection write, got {other:?}"),
        }
    }

    #[test]
    fn markers_append_without_dedup_and_remove_by_id() {
        let p = props(&[("Markers", "A\t1\t2\tone\t")]);
        assert_eq!(set(start_with(&p, "ADDMARKER", &["A", "3", "4", "again"])), ("Markers", "A\t1\t2\tone\t\nA\t3\t4\tagain\t".into()));
        assert_eq!(set(start_with(&props(&[]), "ADDMARKER", &["B", "5", "6"])), ("Markers", "B\t5\t6\t\t".into()));
        let p = props(&[("Markers", "A\t1\t2\n\nB\t3\t4")]);
        assert_eq!(set(start_with(&p, "REMOVEMARKER", &["A"])), ("Markers", "\nB\t3\t4".into()));
    }

    #[test]
    fn routes_and_regions_replace_by_id_and_drop_blank_lines() {
        let p = props(&[("Routes", "R1\tred\t2\tabc\n\nR2\tblue\t3\tdef")]);
        assert_eq!(set(start_with(&p, "ADDROUTE", &["R1", "green", "4", "xyz"])), ("Routes", "R2\tblue\t3\tdef\nR1\tgreen\t4\txyz".into()));
        assert_eq!(set(start_with(&p, "REMOVEROUTE", &["R2"])), ("Routes", "R1\tred\t2\tabc".into()));
        assert_eq!(set(start_with(&p, "CLEARROUTES", &[])), ("Routes", String::new()));
        let five = set(start_with(&props(&[]), "ADDREGION", &["Z", "#fff", "#000", "1", "1,2;3,4"]));
        assert_eq!(five, ("Regions", "Z\t#fff\t#000\t1\t1,2;3,4\t\t".into()));
        assert_eq!(set(start_with(&props(&[("Regions", "Z\ta")]), "CLEARREGIONS", &[])), ("Regions", String::new()));
    }

    #[test]
    fn a_service_without_a_key_sends_nothing() {
        for (method, args, why) in [("GEOCODE", vec!["Paris"], "Maps API key not configured"), ("TRACEROAD", vec!["", "1", "2", "3", "4"], "TraceRoad: no OpenRouteService key was supplied"), ("TRACEROAD", vec![], "TraceRoad: no OpenRouteService key was supplied")] {
            match start_with(&props(&[("_ResolvedMapsApiKey", "  ")]), method, &args) {
                Start::NotConfigured(fx) => {
                    assert_eq!(fx.props, vec![("LastError".to_owned(), why.to_owned())]);
                    assert_eq!(fx.events, vec!["onError".to_owned()]);
                }
                other => panic!("{method}: {other:?}"),
            }
        }
    }

    #[test]
    fn a_service_reads_its_own_arguments_and_the_timeout() {
        let p = props(&[("_ResolvedMapsApiKey", "k"), ("TimeoutMs", "2500")]);
        match start_with(&p, "REVERSEGEOCODE", &["1", "2", "ignored"]) {
            Start::Request(r) => {
                assert_eq!(r.args, vec!["1".to_owned(), "2".to_owned()]);
                assert_eq!(r.timeout_ms, 2500);
            }
            other => panic!("{other:?}"),
        }
        match start_with(&props(&[]), "TRACEROAD", &["key", "1", "2"]) {
            Start::Request(r) => assert_eq!(r.args, vec!["key", "1", "2", "", ""].iter().map(|s| s.to_string()).collect::<Vec<_>>()),
            other => panic!("{other:?}"),
        }
        assert!(matches!(start_with(&p, "CANCEL", &[]), Start::NotOwn));
        assert!(is_service("DIRECTIONS") && !is_service("ADDMARKER"));
    }
}
