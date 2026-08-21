// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `AddRoute` / `AddRegion` and their companions, driven from real COBOL.
//!
//! Routes and regions are collections on the `Maps` control, stored the way
//! `Markers` always was: one TAB-separated record per line in a plain string
//! property. None of it needs an API key — the geometry comes from the program,
//! and the basemap underneath is OpenStreetMap.
//!
//! What these pin is the collection *semantics*, which is where a map drawing
//! itself repeatedly goes wrong: re-adding an id must UPDATE that record, not
//! stack a second copy under the first.
//!
//! (The COBOL below is in `r##"…"##` strings on purpose — a colour literal like
//! `"#FF0000"` contains `"#`, which closes a plain `r#"…"#`.)

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Run `body` against a seeded `MAP-1` and return that control's final
/// published value for `prop`.
fn run_map(body: &str, prop: &str) -> String {
    let src = format!(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. MAPTEST.\n\
         PROCEDURE DIVISION.\n\
         {body}\n\
             STOP RUN.\n"
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
    let errors: Vec<String> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("{}:{}: {}", d.span.line, d.span.col, d.message))
        .collect();
    assert!(errors.is_empty(), "parse errors: {errors:#?}");
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, _display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.seed_objects(vec![("MAP-1".to_owned(), "Maps".to_owned(), vec![])]);
    interp.run().expect("run failed");
    drop(interp);
    let mut last = String::new();
    for u in state_rx.try_iter() {
        if u.ctrl_id.eq_ignore_ascii_case("MAP-1") && u.prop.eq_ignore_ascii_case(prop) {
            last = u.value;
        }
    }
    last
}

/// A route is one record: id, colour, width, geometry.
#[test]
fn add_route_stores_one_record_per_line() {
    let routes = run_map(
        r##"    INVOKE MAP-1 "AddRoute" USING "R1" "#FF0000" "5" "40.4168,-3.7038;37.1773,-3.5986"
    INVOKE MAP-1 "AddRoute" USING "R2" "#0000FF" "3" "41.3874,2.1686;39.4699,-0.3763""##,
        "Routes",
    );
    let lines: Vec<&str> = routes.lines().collect();
    assert_eq!(lines.len(), 2, "two routes, two lines: {routes:?}");
    assert!(lines[0].starts_with("R1\t#FF0000\t5\t"), "{:?}", lines[0]);
    assert!(lines[1].starts_with("R2\t#0000FF\t3\t"), "{:?}", lines[1]);
}

/// **Re-adding an id updates it.** A program that redraws a route as its data
/// changes would otherwise pile duplicates on the map, each drawn over the
/// last, with no way to move the original.
#[test]
fn adding_the_same_route_id_replaces_rather_than_duplicates() {
    let routes = run_map(
        r##"    INVOKE MAP-1 "AddRoute" USING "R1" "#FF0000" "5" "40.0,-3.0;37.0,-3.5"
    INVOKE MAP-1 "AddRoute" USING "R1" "#00AA00" "8" "41.0,2.0;39.0,-0.3""##,
        "Routes",
    );
    assert_eq!(routes.lines().count(), 1, "still one route: {routes:?}");
    assert!(
        routes.starts_with("R1\t#00AA00\t8\t"),
        "the newer one wins: {routes:?}"
    );
}

#[test]
fn remove_route_drops_only_that_one() {
    let routes = run_map(
        r##"    INVOKE MAP-1 "AddRoute" USING "R1" "#FF0000" "5" "40.0,-3.0;37.0,-3.5"
    INVOKE MAP-1 "AddRoute" USING "R2" "#0000FF" "3" "41.0,2.0;39.0,-0.3"
    INVOKE MAP-1 "RemoveRoute" USING "R1""##,
        "Routes",
    );
    assert_eq!(routes.lines().count(), 1);
    assert!(routes.starts_with("R2\t"), "{routes:?}");
}

/// Sales territories, each its own colour — the operator's case.
#[test]
fn regions_carry_a_fill_and_a_stroke_per_territory() {
    let regions = run_map(
        r##"    INVOKE MAP-1 "AddRegion" USING "NORTE" "#E5484D80" "#E5484D" "2" "43.4,-8.4;43.5,-5.7;42.6,-5.6;42.5,-8.5"
    INVOKE MAP-1 "AddRegion" USING "CENTRO" "#3E63DD80" "#3E63DD" "2" "41.0,-4.7;41.1,-2.9;40.2,-2.8;40.1,-4.6""##,
        "Regions",
    );
    let lines: Vec<&str> = regions.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(
        lines[0].starts_with("NORTE\t#E5484D80\t#E5484D\t2\t"),
        "{:?}",
        lines[0]
    );
    assert!(
        lines[1].starts_with("CENTRO\t#3E63DD80\t#3E63DD\t2\t"),
        "{:?}",
        lines[1]
    );
}

#[test]
fn clearing_empties_the_collection() {
    let routes = run_map(
        r##"    INVOKE MAP-1 "AddRoute" USING "R1" "#FF0000" "5" "40.0,-3.0;37.0,-3.5"
    INVOKE MAP-1 "ClearRoutes""##,
        "Routes",
    );
    assert!(routes.is_empty(), "expected nothing, got {routes:?}");
}

/// Markers, routes and regions are three independent collections — adding to
/// one must not disturb the others.
#[test]
fn the_three_collections_do_not_interfere() {
    let body = r##"    INVOKE MAP-1 "AddMarker" USING "M1" "40.4168" "-3.7038" "Madrid" "HQ"
    INVOKE MAP-1 "AddRoute" USING "R1" "#FF0000" "5" "40.0,-3.0;37.0,-3.5"
    INVOKE MAP-1 "AddRegion" USING "Z1" "#00000040" "#000000" "1" "40.0,-3.0;41.0,-3.0;41.0,-2.0"
    INVOKE MAP-1 "RemoveRoute" USING "R1""##;
    assert_eq!(run_map(body, "Routes"), "", "the route went");
    assert!(
        run_map(body, "Markers").starts_with("M1\t"),
        "the marker stayed"
    );
    assert!(
        run_map(body, "Regions").starts_with("Z1\t"),
        "the region stayed"
    );
}
