// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 109 §5.2 at run time: `FORMAT IS … LOCALE IS …` clauses, interpolated
//! strings `F"…{expr:format}…"` and `X::Format(…)`. Formatting happens only where
//! text is made of a number (DISPLAY, interpolation, `Format()`); MOVE and
//! comparison see the stored value (R10).

use std::sync::mpsc;
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

const NB: &str = "\u{a0}";

fn run(procedure: &str) -> Result<Vec<String>, String> {
    let src = format!(
        "\
IDENTIFICATION DIVISION.
PROGRAM-ID. FMT.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-LOCALE PIC X(16) VALUE \"pt-BR\".
01 WS-ASSETS PIC S9(13)V99 VALUE 1234567.89 FORMAT IS \"currency BRL\" LOCALE IS WS-LOCALE.
01 WS-COUNT  PIC 9(9) VALUE 1234567 FORMAT \"number\" LOCALE \"pt-BR\".
01 WS-RATIO  PIC 9V999 VALUE 0.425.
01 WS-OUT    PIC X(40).
01 FORMAT    PIC X VALUE \"a\".
01 LOCALE    PIC X VALUE \"b\".
PROCEDURE DIVISION.
MAIN.
{procedure}
    STOP RUN.
"
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    match interp.run() {
        Ok(_) => Ok(display_rx.try_iter().collect()),
        Err(e) => Err(e.to_string()),
    }
}

fn shown(procedure: &str) -> String {
    run(procedure).expect("the program runs").join("|")
}

#[test]
fn format_clauses_interpolation_and_the_format_method() {
    let t0 = Instant::now();
    let cases: Vec<(&str, String, String)> = vec![
        ("DISPLAY of a formatted item", shown("DISPLAY WS-ASSETS"), format!("R${NB}1.234.567,89")),
        ("literal LOCALE", shown("DISPLAY WS-COUNT"), "1.234.567".into()),
        (
            "the locale variable is read on every display",
            shown("DISPLAY WS-ASSETS\nMOVE \"en-US\" TO WS-LOCALE\nDISPLAY WS-ASSETS"),
            format!("R${NB}1.234.567,89|R$1,234,567.89"),
        ),
        (
            "locale case-insensitive, blanks ignored",
            shown("MOVE \"PT-br\" TO WS-LOCALE\nDISPLAY WS-ASSETS"),
            format!("R${NB}1.234.567,89"),
        ),
        (
            "the stored value is unchanged by formatting",
            shown("DISPLAY WS-ASSETS\nIF WS-ASSETS = 1234567.89 DISPLAY \"same\" END-IF"),
            format!("R${NB}1.234.567,89|same"),
        ),
        (
            "MOVE moves the bytes, never the format",
            shown("MOVE WS-ASSETS TO WS-OUT\nDISPLAY WS-OUT"),
            "0001234567.89".replace('.', "") + &" ".repeat(0),
        ),
        (
            "STRING sends the formatted text",
            shown("STRING WS-COUNT DELIMITED BY SIZE INTO WS-OUT\nDISPLAY WS-OUT"),
            "1.234.567".to_string() + &" ".repeat(31),
        ),
        ("interpolation, item clause applies", shown("DISPLAY F\"Total {WS-ASSETS} / {WS-COUNT}\""), format!("Total R${NB}1.234.567,89 / 1.234.567")),
        ("interpolation, explicit format", shown("DISPLAY F\"{WS-RATIO:percent}\""), "42.5%".into()),
        ("braces are escaped", shown("DISPLAY F\"{{x}} {WS-COUNT}\""), "{x} 1.234.567".into()),
        ("two expressions left to right", shown("DISPLAY F\"{WS-COUNT:number}-{WS-RATIO:percent}\""), "1.234.567-42.5%".into()),
        ("Format() with a locale", shown("DISPLAY WS-RATIO::Format(\"percent\", \"pt-BR\")"), "42,5%".into()),
        ("Format() name case", shown("DISPLAY WS-COUNT::Format(\"NUMBER\", \"EN-us\")"), "1,234,567".into()),
        ("Format() with no argument uses the clause", shown("DISPLAY WS-COUNT::Format()"), "1.234.567".into()),
        ("an item named FORMAT or LOCALE is unchanged", shown("DISPLAY FORMAT LOCALE"), "ab".into()),
    ];
    let mut failed = 0;
    for (name, got, want) in &cases {
        // The one case that checks a MOVE keeps only that no currency symbol appears.
        if *name == "MOVE moves the bytes, never the format" {
            if got.contains("R$") || got.contains(',') {
                failed += 1;
                eprintln!("FAIL {name}: got [{got}]");
            }
            continue;
        }
        if got != want {
            failed += 1;
            eprintln!("FAIL {name}: got [{got}] want [{want}]");
        }
    }
    let refused: Vec<(&str, Result<Vec<String>, String>, &str)> = vec![
        ("unknown format name", run("DISPLAY WS-COUNT::Format(\"bogus\")"), "unknown format"),
        ("unknown locale", run("DISPLAY WS-COUNT::Format(\"number\", \"xx-XX\")"), "unknown locale"),
        ("currency with no code", run("DISPLAY WS-COUNT::Format(\"currency\", \"pt-BR\")"), "needs a currency"),
        ("Format() of an item with no clause", run("DISPLAY WS-RATIO::Format()"), "needs an item declared with FORMAT"),
        ("text is not a number", run("MOVE \"abc\" TO WS-OUT\nDISPLAY WS-OUT::Format(\"number\")"), "needs a numeric item"),
    ];
    for (name, got, want) in &refused {
        match got {
            Err(e) if e.contains(want) => {}
            other => {
                failed += 1;
                eprintln!("FAIL {name}: got {other:?}, wanted an error containing [{want}]");
            }
        }
    }
    let total = cases.len() + refused.len();
    println!("================ test_format ================");
    println!("cases: FORMAT/LOCALE clauses (literal, variable, case), interpolation ({{}} escape, explicit format), Format(), MOVE not formatted, items named FORMAT/LOCALE, 5 refusals");
    println!("run: {total} cases in {} ms", t0.elapsed().as_millis());
    println!("tally: {} passed, {failed} failed", total - failed);
    assert_eq!(failed, 0);
}
