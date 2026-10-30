// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 109 R8: what a FORMAT / LOCALE clause or an interpolated string gets
//! wrong is said at build time — an unknown locale variable, a locale item that is
//! not alphanumeric, a non-numeric item formatted, an invalid format name in an
//! interpolation — and a correct program says nothing.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze, Severity};

fn errors_of(items: &str, proc_: &str) -> Vec<String> {
    let src = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{items}\nPROCEDURE DIVISION.\nMAIN.\n{proc_}\n    STOP RUN.\n"
    );
    let parsed = parse(tokenize(&src, SourceFormat::Free));
    let mut msgs: Vec<String> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.severity == cobolt_parser::Severity::Error)
        .map(|d| d.message.clone())
        .collect();
    let program = parsed.program.expect("program should parse");
    msgs.extend(
        analyze(&program)
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| d.message.clone()),
    );
    msgs
}

#[test]
fn a_correct_declaration_and_interpolation_say_nothing() {
    let e = errors_of(
        "01 WS-LOC PIC X(16) VALUE \"pt-BR\".\n01 WS-A PIC 9(5)V99 FORMAT IS \"currency BRL\" LOCALE IS WS-LOC.\n01 WS-B PIC 9(4) FORMAT \"number\" LOCALE \"fr-FR\".\n01 FORMAT PIC X.\n01 LOCALE PIC X.",
        "    DISPLAY F\"{WS-A} / {WS-B:percent} / {{ok}}\"",
    );
    assert!(e.is_empty(), "{e:?}");
}

#[test]
fn the_faults_of_r8_are_named_at_build_time() {
    let cases: [(&str, &str, &str, &str); 7] = [
        ("an unknown locale item", "01 WS-A PIC 9(3) FORMAT \"number\" LOCALE IS NOPE.", "", "'NOPE' is not declared"),
        ("a numeric locale item", "01 WS-N PIC 9(3).\n01 WS-A PIC 9(3) FORMAT \"number\" LOCALE IS WS-N.", "", "must be an alphanumeric item"),
        ("an invalid format name", "01 WS-A PIC 9(3) FORMAT \"bogus\".", "", "unknown format 'bogus'"),
        ("a malformed locale tag", "01 WS-A PIC 9(3) FORMAT \"number\" LOCALE \"xx-XX\".", "", "unknown locale 'xx-XX'"),
        ("a text item formatted", "01 WS-T PIC X(5) FORMAT \"number\".", "", "only a numeric item"),
        ("LOCALE with no FORMAT", "01 WS-A PIC 9(3) LOCALE \"pt-BR\".", "", "needs a FORMAT clause"),
        ("a bad format in an interpolation", "01 WS-A PIC 9(3).", "    DISPLAY F\"{WS-A:bogus}\"", "unknown format 'bogus'"),
    ];
    for (name, items, proc_, want) in cases {
        let e = errors_of(items, proc_);
        assert!(e.iter().any(|m| m.contains(want)), "{name}: wanted [{want}], got {e:?}");
    }
}

#[test]
fn unmatched_braces_and_an_invalid_expression_are_reported() {
    for (src, want) in [
        ("    DISPLAY F\"a { b\"", "unmatched '{'"),
        ("    DISPLAY F\"a } b\"", "unmatched '}'"),
        ("    DISPLAY F\"{WS-A +}\"", "invalid expression"),
    ] {
        let e = errors_of("01 WS-A PIC 9(3).", src);
        assert!(e.iter().any(|m| m.contains(want)), "[{src}] wanted [{want}], got {e:?}");
    }
}
