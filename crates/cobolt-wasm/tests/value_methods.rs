// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Value methods of data items (WASM-010 T11, R7): every spelling of
//! `cobolt_ast::methods::DATA_ITEM_METHODS` gives the interpreter's answer on
//! the module, in both memory variants.

mod common;

fn program(stmts: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. VM.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 WS-NAME PIC X(12) VALUE \"  Ana Maria \".\n01 WS-CSV PIC X(16) VALUE \"red,green,blue\".\n\
         01 WS-NUM PIC 9(4) VALUE 42.\n01 WS-OUT PIC X(24).\n01 WS-N PIC 9(4).\n\
         PROCEDURE DIVISION.\n{stmts}    STOP RUN.\n"
    )
}

/// `(spelling or case, statements)`: each prints what it computed.
fn rows() -> Vec<(&'static str, String)> {
    let show = |e: &str| format!("    MOVE {e} TO WS-OUT\n    DISPLAY \"[\" WS-OUT \"]\"\n");
    let count = |e: &str| format!("    MOVE {e} TO WS-N\n    DISPLAY WS-N\n");
    vec![
        ("Trim", show("WS-NAME::Trim")),
        ("UpperCase", show("WS-NAME::UpperCase")),
        ("ToUpperCase", show("WS-NAME::ToUpperCase")),
        ("Upper", show("WS-NAME::Upper")),
        ("LowerCase", show("WS-NAME::LowerCase")),
        ("ToLowerCase", show("WS-NAME::ToLowerCase")),
        ("Lower", show("WS-NAME::Lower")),
        ("Replace", show("WS-NAME::Replace(\"a\", \"o\")")),
        ("Len", count("WS-NAME::Len")),
        ("Length", count("WS-NAME::Length")),
        ("Split", show("WS-CSV::Split(\",\")")),
        // Beyond the eleven spellings: a chain, a computation, a number, DISPLAY.
        ("Trim::Upper", show("WS-NAME::Trim::Upper")),
        ("Len in COMPUTE", "    COMPUTE WS-N = WS-NAME::Trim::Len + 100\n    DISPLAY WS-N\n".to_string()),
        ("Len of a number", count("WS-NUM::Len")),
        ("DISPLAY Upper", "    DISPLAY WS-CSV::Upper\n".to_string()),
    ]
}

#[test]
fn every_value_method_spelling_matches_the_interpreter() {
    for m in cobolt_ast::methods::DATA_ITEM_METHODS {
        assert!(rows().iter().any(|(n, _)| n == m), "no row for {m}");
    }
    let rows = rows();
    let mut equal = 0;
    for (name, stmts) in &rows {
        let src = program(stmts);
        let out = common::same_output(&src);
        assert!(!out.is_empty(), "{name}: printed nothing");
        eprintln!("  {name:<16} {out:?}");
        equal += 1;
    }
    eprintln!("value methods: {} rows, {equal} equal on both engines and both memory variants ({} spellings)", rows.len(), cobolt_ast::methods::DATA_ITEM_METHODS.len());
}

#[test]
fn a_method_a_data_item_does_not_have_is_refused_naming_the_list() {
    let src = program("    MOVE WS-NAME::Reverse TO WS-OUT\n");
    let prog = common::parse(&src);
    let err = match cobolt_wasm::compile_program(&prog, &cobolt_wasm::Options::default()) {
        Ok(_) => panic!("compiled"),
        Err(e) => e.to_string(),
    };
    assert!(err.to_ascii_uppercase().contains("REVERSE") && err.contains("the methods Trim, Upper, Lower, Replace and Split"), "{err}");
}

#[test]
fn len_written_as_a_call_is_refused_by_the_generator() {
    for call in ["Len()", "Trim()::Length()"] {
        let src = program(&format!("    COMPUTE WS-N = WS-NAME::{call}\n"));
        let prog = common::parse(&src);
        let err = match cobolt_wasm::compile_program(&prog, &cobolt_wasm::Options::default()) {
            Ok(_) => panic!("{call} compiled"),
            Err(e) => e.to_string(),
        };
        assert!(err.contains("is a property of a data item") && err.contains("without the parentheses"), "{call}: {err}");
    }
}

/// A STRING whose sources read more values than the module's four character temporaries hold is
/// lowered in parts that share one pointer: PowerCobolCraft's and PowerCloud's long STRINGs of
/// `::Trim()` sources. Every case gives the interpreter's receiver and pointer; with an OVERFLOW
/// phrase, which a part could answer differently, the statement is still refused.
#[test]
fn a_string_of_more_than_four_reads_is_built_in_parts() {
    let six = |m: &str| format!("WS-A::{m}() \"/\" WS-B::{m}() \"/\" WS-C::{m}() \"/\" WS-D::{m}() \"/\" WS-E::{m}() \"/\" WS-F::{m}()");
    let src = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. SP.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 WS-A PIC X(8) VALUE \"  alpha \".\n01 WS-B PIC X(8) VALUE \" beta   \".\n\
         01 WS-C PIC X(8) VALUE \"gamma   \".\n01 WS-D PIC X(8) VALUE \"  delta \".\n\
         01 WS-E PIC X(8) VALUE \"eps     \".\n01 WS-F PIC X(8) VALUE \" zeta   \".\n\
         01 WS-OUT PIC X(48).\n01 WS-SMALL PIC X(12).\n01 WS-P PIC 99.\n\
         PROCEDURE DIVISION.\n\
         \x20   MOVE SPACES TO WS-OUT\n\
         \x20   STRING {trim} DELIMITED BY SIZE INTO WS-OUT\n\
         \x20   DISPLAY \"[\" WS-OUT \"]\"\n\
         \x20   MOVE ALL \".\" TO WS-OUT\n\
         \x20   MOVE 4 TO WS-P\n\
         \x20   STRING {upper} DELIMITED BY SIZE INTO WS-OUT WITH POINTER WS-P\n\
         \x20   DISPLAY \"[\" WS-OUT \"] \" WS-P\n\
         \x20   MOVE SPACES TO WS-OUT\n\
         \x20   STRING FUNCTION TRIM(WS-A) DELIMITED BY SIZE FUNCTION UPPER-CASE(WS-B) DELIMITED BY SPACE\n\
         \x20          \"+\" DELIMITED BY SIZE FUNCTION TRIM(WS-C) DELIMITED BY FUNCTION LOWER-CASE(\"M\")\n\
         \x20          FUNCTION REVERSE(WS-D) DELIMITED BY SIZE WS-E::Trim() DELIMITED BY SIZE\n\
         \x20          WS-F DELIMITED BY \"e\" INTO WS-OUT\n\
         \x20   DISPLAY \"[\" WS-OUT \"]\"\n\
         \x20   MOVE ALL \"-\" TO WS-SMALL\n\
         \x20   STRING {trim} DELIMITED BY SIZE INTO WS-SMALL\n\
         \x20   DISPLAY \"[\" WS-SMALL \"]\"\n\
         \x20   MOVE ALL \"-\" TO WS-SMALL\n\
         \x20   MOVE 3 TO WS-P\n\
         \x20   STRING {trim} DELIMITED BY SIZE INTO WS-SMALL WITH POINTER WS-P\n\
         \x20   DISPLAY \"[\" WS-SMALL \"] \" WS-P\n\
         \x20   MOVE ALL \"-\" TO WS-SMALL\n\
         \x20   MOVE 13 TO WS-P\n\
         \x20   STRING {trim} DELIMITED BY SIZE INTO WS-SMALL WITH POINTER WS-P\n\
         \x20   DISPLAY \"[\" WS-SMALL \"] \" WS-P\n\
         \x20   STOP RUN.\n",
        trim = six("Trim"),
        upper = six("Upper"),
    );
    let out = common::same_output(&src);
    assert_eq!(out[0], format!("[alpha/beta/gamma/delta/eps/zeta{}]", " ".repeat(17)), "{out:?}");
    assert_eq!(out.len(), 6, "{out:?}");
    for (i, l) in out.iter().enumerate() {
        eprintln!("  case {}: {l}", i + 1);
    }

    // With an OVERFLOW phrase the statement keeps its refusal, at its line.
    let guarded = src.replacen(
        &format!("STRING {} DELIMITED BY SIZE INTO WS-SMALL\n", six("Trim")),
        &format!("STRING {} DELIMITED BY SIZE INTO WS-SMALL\n        ON OVERFLOW DISPLAY \"full\"\n    END-STRING\n", six("Trim")),
        1,
    );
    assert_ne!(guarded, src);
    let err = match cobolt_wasm::compile_program(&common::parse(&guarded), &cobolt_wasm::Options::default()) {
        Ok(_) => panic!("a long STRING with ON OVERFLOW compiled"),
        Err(e) => e,
    };
    let refused = err.all_kinded().into_iter().filter(|e| e.kind == cobolt_wasm::ErrorKind::TooManyHostReads).count();
    assert_eq!(refused, 1, "{err:?}");
    eprintln!("STRING in parts: 6 statements of 6 reads each, {} lines equal on both engines and both memory variants; with ON OVERFLOW, 1 of 1 refused", out.len());
}

/// A value method's answer in arithmetic reads as the number its text spells, as the interpreter
/// reads any text (`CobolValue::as_exact`), and as zero when it spells none. The module read it as
/// zero unless the method was `::Len` (WASM-PARITY #73).
#[test]
fn a_value_methods_answer_in_arithmetic_is_the_number_it_spells() {
    let program = |stmts: &str| {
        format!(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. VMARITH.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
             01 WS-T20 PIC X(6) VALUE \"  20  \".\n01 WS-TDEC PIC X(6) VALUE \"2.5\".\n\
             01 WS-TNEG PIC X(6) VALUE \"-3\".\n01 WS-TABC PIC X(6) VALUE \"abc\".\n\
             01 WS-CSV PIC X(8) VALUE \"10,20,30\".\n01 WS-N PIC 9(4).\n01 WS-S PIC S9(4).\n\
             01 WS-D PIC 9(2)V9.\nPROCEDURE DIVISION.\n{stmts}    STOP RUN.\n"
        )
    };
    let rows: Vec<(&str, &str, &str)> = vec![
        ("Trim plus 1", "    COMPUTE WS-N = WS-T20::Trim + 1\n    DISPLAY WS-N\n", "0021"),
        ("Upper times 2", "    COMPUTE WS-N = WS-T20::Upper * 2\n    DISPLAY WS-N\n", "0040"),
        ("two answers added", "    COMPUTE WS-N = WS-T20::Trim + WS-T20::Lower\n    DISPLAY WS-N\n", "0040"),
        ("a decimal text keeps its scale", "    COMPUTE WS-D = WS-TDEC::Trim * 3\n    DISPLAY WS-D\n", "075"),
        ("a negative text", "    COMPUTE WS-S = WS-TNEG::Trim + 10\n    DISPLAY WS-S\n", "0007"),
        ("a text that is no number reads as 0", "    COMPUTE WS-N = WS-TABC::Trim + 5\n    DISPLAY WS-N\n", "0005"),
        ("Split's first part", "    COMPUTE WS-N = WS-CSV::Split(\",\") + 1\n    DISPLAY WS-N\n", "0011"),
        ("Len is unchanged", "    COMPUTE WS-N = WS-T20::Trim::Len + 100\n    DISPLAY WS-N\n", "0102"),
    ];
    let t = std::time::Instant::now();
    let mut expected = 0;
    for (name, stmts, want) in &rows {
        let out = common::same_output(&program(stmts));
        let line = out.first().cloned().unwrap_or_default();
        eprintln!("  {name:<38} {line:?}");
        if line.starts_with(want) {
            expected += 1;
        } else {
            eprintln!("    expected to start with {want:?}");
        }
    }
    eprintln!("== a_value_methods_answer_in_arithmetic_is_the_number_it_spells (WASM-PARITY #73) ==");
    eprintln!(
        "rows {}: equal on both engines and both memory variants {}, as expected {expected}; {:.0} ms",
        rows.len(),
        rows.len(),
        t.elapsed().as_secs_f64() * 1000.0
    );
    assert_eq!(expected, rows.len(), "a row printed what it should not");
}

/// `X::Split(sep)(i)`, part `i` of a value method's answer (DESKTOP-WASM-001 T39c): the index written
/// after a `Split`, in every form the interpreter reads it, and what follows it in a chain. PowerCloud's
/// main form reads a role from a JSON answer with two of them in one chain.
#[test]
fn split_with_an_index_matches_the_interpreter() {
    let program = |stmts: &str| {
        format!(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. VMSPLIT.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
             01 WS-CSV PIC X(16) VALUE \"red,green,blue\".\n01 WS-NUMS PIC X(8) VALUE \"10,20,30\".\n\
             01 WS-WORDS PIC X(16) VALUE \"one two  three\".\n01 WS-PAIRS PIC X(10) VALUE \"a, b,c\".\n\
             01 WS-SEP PIC X(2) VALUE \",\".\n01 WS-I PIC 9(2) VALUE 2.\n01 WS-NEG PIC S9(2) VALUE -3.\n\
             01 WS-DEC PIC 9V9 VALUE 2.7.\n01 WS-TXT-I PIC X(3) VALUE \"3\".\n01 WS-TXT-X PIC X(3) VALUE \"x\".\n\
             01 WS-JSON PIC X(80) VALUE '[{{\"OutputKey\":\"FunctionRoleArn\",\"OutputValue\":\"arn:aws:iam::role/deployer\"}}]'.\n\
             01 WS-OUT PIC X(32).\n01 WS-N PIC 9(4).\nPROCEDURE DIVISION.\n{stmts}    STOP RUN.\n"
        )
    };
    let show = |e: &str| format!("    MOVE {e} TO WS-OUT\n    DISPLAY \"[\" WS-OUT \"]\"\n");
    let rows: Vec<(&str, String, &str)> = vec![
        ("part 1", show("WS-CSV::Split(\",\")(1)"), "[red"),
        ("part 2", show("WS-CSV::Split(\",\")(2)"), "[green"),
        ("part 3, the last", show("WS-CSV::Split(\",\")(3)"), "[blue"),
        ("part 4, past the last", show("WS-CSV::Split(\",\")(4)"), "[ "),
        ("part 0 reads as 1", show("WS-CSV::Split(\",\")(0)"), "[red"),
        ("a data item's index", show("WS-CSV::Split(\",\")(WS-I)"), "[green"),
        ("a computed index", show("WS-CSV::Split(\",\")(WS-I + 1)"), "[blue"),
        ("a negative index reads as 1", show("WS-CSV::Split(\",\")(WS-NEG)"), "[red"),
        ("a decimal index, truncated", show("WS-CSV::Split(\",\")(WS-DEC)"), "[green"),
        ("a text index", show("WS-CSV::Split(\",\")(WS-TXT-I)"), "[blue"),
        ("a text that is no number reads as 1", show("WS-CSV::Split(\",\")(WS-TXT-X)"), "[red"),
        ("a separator item keeps its spaces", show("WS-PAIRS::Split(WS-SEP)(2)"), "[b,c"),
        ("no separator: a space", show("WS-WORDS::Split()(2)"), "[two"),
        ("two spaces leave an empty part", show("WS-WORDS::Split()(3)"), "[ "),
        ("a method after the part", show("WS-CSV::Split(\",\")(2)::Upper"), "[GREEN"),
        ("two Splits in one chain (PowerCloud)", show("WS-JSON::Split( '\"OutputKey\":\"FunctionRoleArn\",\"OutputValue\":\"' )(2)::Split( '\"' )(1)"), "[arn:aws:iam::role/deployer"),
        ("in COMPUTE", "    COMPUTE WS-N = WS-NUMS::Split(\",\")(2) + 1\n    DISPLAY WS-N\n".to_string(), "0021"),
        ("in IF", "    IF WS-CSV::Split(\",\")(2) = \"green\"\n        DISPLAY \"yes\"\n    ELSE\n        DISPLAY \"no\"\n    END-IF\n".to_string(), "yes"),
        ("in DISPLAY", "    DISPLAY WS-CSV::Split(\",\")(3)\n".to_string(), "blue"),
    ];
    let t = std::time::Instant::now();
    let mut equal = 0;
    let mut expected = 0;
    for (name, stmts, want) in &rows {
        let out = common::same_output(&program(stmts));
        let line = out.first().cloned().unwrap_or_default();
        eprintln!("  {name:<38} {line:?}");
        equal += 1;
        if line.starts_with(want) {
            expected += 1;
        } else {
            eprintln!("    expected to start with {want:?}");
        }
    }
    eprintln!("== split_with_an_index_matches_the_interpreter (DESKTOP-WASM-001 T39c) ==");
    eprintln!(
        "rows {}: equal on both engines and both memory variants {equal}, as expected {expected}; {:.0} ms",
        rows.len(),
        t.elapsed().as_secs_f64() * 1000.0
    );
    assert_eq!(expected, rows.len(), "a row printed what it should not");
}
