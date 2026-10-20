// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `CALL … WITH WEBSOCKET LINKAGE` at Check time (WASM-005 R2, R4, R7).
//!
//! Without an imported interface description only the arguments are judged
//! (they are groups of WORKING-STORAGE and have a wire form). With one, the
//! name must be exported — `ON EXCEPTION` does not excuse a name the contract
//! does not list — the argument count must match, and each group must fit the
//! parameter it lands on. A WEBSOCKET target is never judged as a local
//! program: the "no program has that name" error of local calls does not fire.

use std::collections::HashSet;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze_with, AnalyzeOptions, Severity};
use cobolt_wire::contract::{Deprecation, Direction, InterfaceDescription, Param, ProgramSig};
use cobolt_wire::layout::{Item, Kind, Occurs};

fn caller(data: &str, body: &str) -> String {
    format!(
        "\
IDENTIFICATION DIVISION.
PROGRAM-ID. REMOTE-TEST.
DATA DIVISION.
FILE SECTION.
FD  CUST-FILE.
01  CUST-FILE-REC.
    05 FR-ID PIC 9(4).
WORKING-STORAGE SECTION.
01 CUST.
   05 CUST-ID   PIC 9(6).
   05 CUST-NAME PIC X(12).
   05 BALANCE   PIC S9(5)V99 COMP-3.
01 ORDERS-GRP.
   05 N PIC 99.
   05 LINE-NO PIC 9(4) OCCURS 1 TO 5 DEPENDING ON N.
01 RES.
   05 RC PIC 99.
01 SCALAR PIC S9(4).
01 FLOATY.
   05 F PIC 9(4)V99 COMP-1.
{data}
PROCEDURE DIVISION.
MAIN.
{body}
    STOP RUN.
"
    )
}

fn run(src: &str, opts: AnalyzeOptions) -> Vec<(Severity, String)> {
    let parsed = parse(tokenize(src, SourceFormat::Free));
    assert!(!parsed.has_errors(), "the source must parse cleanly: {:?}", parsed.diagnostics);
    let program = parsed.program.expect("program");
    analyze_with(&program, &opts)
        .diagnostics
        .into_iter()
        .filter(|d| d.message.contains("WEBSOCKET") || d.message.contains("no program, paragraph"))
        .map(|d| (d.severity, d.message))
        .collect()
}

fn errors(src: &str, opts: AnalyzeOptions) -> Vec<String> {
    run(src, opts).into_iter().filter(|(s, _)| *s == Severity::Error).map(|(_, m)| m).collect()
}

// ── layouts for an interface description ─────────────────────────────────────

fn alpha(name: &str, len: usize) -> Item {
    Item { name: name.into(), filler: false, redefines: false, occurs: None, kind: Kind::Alphanumeric { len, justified: false }, value: None }
}
fn num(name: &str, digits: u16, scale: u16, signed: bool) -> Item {
    Item { name: name.into(), filler: false, redefines: false, occurs: None, kind: Kind::Numeric { digits, scale, signed }, value: None }
}
fn group(name: &str, children: Vec<Item>) -> Item {
    Item { name: name.into(), filler: false, redefines: false, occurs: None, kind: Kind::Group { children }, value: None }
}
fn cust_layout() -> Item {
    group("CUST", vec![num("CUST-ID", 6, 0, false), alpha("CUST-NAME", 12), num("BALANCE", 5, 2, true)])
}
fn interface() -> InterfaceDescription {
    let mut d = InterfaceDescription::new("1.0.0");
    let mut get = ProgramSig::new("GET-CUSTOMER");
    get.params.push(Param { name: "LK-CUST".into(), direction: Direction::InOut, layout: cust_layout() });
    get.returning = Some(Param { name: "LK-RES".into(), direction: Direction::InOut, layout: group("RES", vec![num("RC", 2, 0, false)]) });
    d.programs.push(get);
    let mut ping = ProgramSig::new("PING");
    ping.deprecated = Some(Deprecation { message: "use HEALTH".into(), removal: Some("2.0.0".into()) });
    d.programs.push(ping);
    d
}
fn with_interface(d: InterfaceDescription) -> AnalyzeOptions {
    AnalyzeOptions { remote_interface: Some(d), ..Default::default() }
}

// ── R2: the arguments ────────────────────────────────────────────────────────

#[test]
fn groups_and_elementary_items_of_working_storage_are_accepted() {
    let src = caller("", "    CALL \"ANYTHING\" WITH WEBSOCKET LINKAGE USING BY REFERENCE CUST BY VALUE SCALAR ORDERS-GRP RETURNING RES");
    assert!(errors(&src, AnalyzeOptions::default()).is_empty());
}

#[test]
fn a_literal_an_expression_a_table_element_and_a_file_record_are_each_refused() {
    for (arg, needle) in [
        ("\"text\"", "must be a data item"),
        ("LINE-NO (1)", "whole group"),
        ("CUST-FILE-REC", "file record"),
        ("FLOATY", "COMP-1"),
    ] {
        let src = caller("", &format!("    CALL \"X\" WITH WEBSOCKET LINKAGE USING {arg}"));
        let e = errors(&src, AnalyzeOptions::default());
        assert_eq!(e.len(), 1, "{arg}: {e:?}");
        assert!(e[0].contains(needle), "{arg}: {}", e[0]);
    }
}

#[test]
fn a_depending_on_item_outside_the_group_is_named() {
    let data = "01 BAD.\n   05 ITEMS PIC 9 OCCURS 1 TO 3 DEPENDING ON OUTSIDE.\n01 OUTSIDE PIC 9.";
    let src = caller(data, "    CALL \"X\" WITH WEBSOCKET LINKAGE USING BAD");
    let e = errors(&src, AnalyzeOptions::default());
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].contains("DEPENDING ON OUTSIDE"), "{}", e[0]);
}

#[test]
fn the_returning_receiver_is_judged_like_an_argument() {
    let src = caller("", "    CALL \"X\" WITH WEBSOCKET LINKAGE USING CUST RETURNING FLOATY");
    let e = errors(&src, AnalyzeOptions::default());
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].contains("RETURNING"), "{}", e[0]);
}

// ── a remote name is not a local program ─────────────────────────────────────

#[test]
fn the_local_unknown_program_error_does_not_fire_for_a_websocket_target() {
    let known = Some(HashSet::new());
    let opts = AnalyzeOptions { known_programs: known, ..Default::default() };
    let local = caller("", "    CALL \"NOT-HERE\" USING CUST");
    assert_eq!(errors(&local, opts.clone()).len(), 1, "a local call to nothing is still an error");
    let remote = caller("", "    CALL \"NOT-HERE\" WITH WEBSOCKET LINKAGE USING CUST");
    assert!(errors(&remote, opts).is_empty());
}

#[test]
fn returning_from_a_websocket_call_is_not_judged_as_a_local_one() {
    let src = caller("", "    CALL \"NOT-HERE\" WITH WEBSOCKET LINKAGE USING CUST RETURNING RES");
    let all = run(&src, AnalyzeOptions::default());
    assert!(all.iter().all(|(_, m)| !m.contains("RETURNING:")), "{all:?}");
}

// ── R4 + R7: against the interface description ───────────────────────────────

#[test]
fn an_exported_name_with_a_fitting_group_is_accepted_without_on_exception() {
    let src = caller("", "    CALL \"get-customer\" WITH WEBSOCKET LINKAGE USING CUST RETURNING RES");
    assert!(errors(&src, with_interface(interface())).is_empty());
}

#[test]
fn a_name_the_contract_does_not_export_is_an_error_even_with_on_exception() {
    let src = caller(
        "",
        "    CALL \"GET-CUSTOMR\" WITH WEBSOCKET LINKAGE USING CUST\n        ON EXCEPTION DISPLAY \"x\"\n    END-CALL",
    );
    let e = errors(&src, with_interface(interface()));
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].contains("exports no program") && e[0].contains("GET-CUSTOMER"), "{}", e[0]);
}

#[test]
fn the_argument_count_must_match() {
    let src = caller("", "    CALL \"GET-CUSTOMER\" WITH WEBSOCKET LINKAGE USING CUST RES");
    let e = errors(&src, with_interface(interface()));
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].contains("takes 1 argument"), "{}", e[0]);
}

#[test]
fn a_group_that_does_not_fit_its_parameter_is_named_member_by_member() {
    let data = "01 WIDE.\n   05 CUST-ID   PIC 9(7).\n   05 CUST-NAME PIC X(40).\n   05 EXTRA     PIC X.\n";
    let src = caller(data, "    CALL \"GET-CUSTOMER\" WITH WEBSOCKET LINKAGE USING BY VALUE WIDE");
    let e = errors(&src, with_interface(interface()));
    let joined = e.join("\n");
    assert!(joined.contains("CUST-ID") && joined.contains("narrowed"), "{joined}");
    assert!(joined.contains("CUST-NAME"), "{joined}");
    assert!(joined.contains("EXTRA") && joined.contains("member-missing"), "{joined}");
}

#[test]
fn the_answer_direction_is_checked_for_by_reference_and_returning() {
    // The backend answers with a 12-byte name; the caller's group holds 6.
    let data = "01 SMALL.\n   05 CUST-ID PIC 9(6).\n   05 CUST-NAME PIC X(6).\n   05 BALANCE PIC S9(5)V99.\n";
    let by_ref = caller(data, "    CALL \"GET-CUSTOMER\" WITH WEBSOCKET LINKAGE USING BY REFERENCE SMALL");
    assert!(errors(&by_ref, with_interface(interface())).join("\n").contains("CUST-NAME"));
    // BY VALUE never reads the answer, so the narrower group is fine.
    let by_value = caller(data, "    CALL \"GET-CUSTOMER\" WITH WEBSOCKET LINKAGE USING BY VALUE SMALL");
    assert!(errors(&by_value, with_interface(interface())).is_empty());
}

#[test]
fn a_caller_that_knows_fewer_members_than_the_backend_sends_is_fine() {
    // The old client: it has no BALANCE. The backend projects its answer to what the client knows.
    let data = "01 OLD.\n   05 CUST-ID PIC 9(6).\n   05 CUST-NAME PIC X(12).\n";
    let src = caller(data, "    CALL \"GET-CUSTOMER\" WITH WEBSOCKET LINKAGE USING BY REFERENCE OLD");
    assert!(errors(&src, with_interface(interface())).is_empty());
}

#[test]
fn returning_from_a_program_that_returns_nothing_is_an_error() {
    let src = caller("", "    CALL \"PING\" WITH WEBSOCKET LINKAGE RETURNING RES");
    let e = errors(&src, with_interface(interface()));
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].contains("returns nothing"), "{}", e[0]);
}

#[test]
fn a_deprecated_program_is_a_warning_naming_the_replacement() {
    let src = caller("", "    CALL \"PING\" WITH WEBSOCKET LINKAGE");
    let all = run(&src, with_interface(interface()));
    let warn: Vec<_> = all.iter().filter(|(s, _)| *s == Severity::Warning).collect();
    assert_eq!(warn.len(), 1, "{all:?}");
    assert!(warn[0].1.contains("deprecated") && warn[0].1.contains("use HEALTH") && warn[0].1.contains("2.0.0"), "{}", warn[0].1);
}

#[test]
fn a_table_that_outgrows_the_parameters_occurs_is_named() {
    let mut d = interface();
    let mut t = alpha("TAGS", 4);
    t.occurs = Some(Occurs { min: 3, max: 3, depending_on: None });
    d.programs.push({
        let mut p = ProgramSig::new("TAG-IT");
        p.params.push(Param { name: "LK-T".into(), direction: Direction::In, layout: group("T", vec![t]) });
        p
    });
    let data = "01 T.\n   05 TAGS PIC X(4) OCCURS 5.\n";
    let src = caller(data, "    CALL \"TAG-IT\" WITH WEBSOCKET LINKAGE USING BY VALUE T");
    let e = errors(&src, with_interface(d));
    assert!(e.join("\n").contains("5 occurrences") || e.join("\n").contains("sends up to 5"), "{e:?}");
}
