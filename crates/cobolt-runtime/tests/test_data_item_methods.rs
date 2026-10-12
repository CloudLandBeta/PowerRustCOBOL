// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The value methods of a data item at run time: the worked examples in
//! `tests/cobol/string-ops/test-data-item-methods.cbl` all pass, every name in
//! `cobolt_ast::methods::DATA_ITEM_METHODS` evaluates, and any other name after
//! a data item is a run-time error instead of an empty string (operator,
//! 2026-10-09). A control's chain and an `OBJECT REFERENCE` are not touched.

use std::path::PathBuf;
use std::sync::mpsc;

use cobolt_lexer::{expand_copybooks, tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn detect_format(src: &str) -> SourceFormat {
    let looks_fixed = src.lines().any(|line| {
        let b = line.as_bytes();
        b.len() > 6 && b[6] != b' ' && b[..6].iter().all(|&c| c == b' ' || c.is_ascii_digit())
    });
    if looks_fixed {
        SourceFormat::Fixed
    } else {
        SourceFormat::Free
    }
}

fn run_source(src: &str, fmt: SourceFormat, dir: &std::path::Path) -> Result<Vec<String>, String> {
    let expanded = expand_copybooks(src, dir, fmt);
    assert!(expanded.errors.is_empty(), "copybook errors: {:?}", expanded.errors);
    let result = parse(tokenize(&expanded.text, SourceFormat::Free));
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

/// A free-form program: the declarations, then `procedure` between MAIN and STOP RUN.
fn run(procedure: &str) -> Result<Vec<String>, String> {
    let src = format!(
        "\
IDENTIFICATION DIVISION.
PROGRAM-ID. DIM.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
REPOSITORY.
    CLASS RUST-STRING IS \"Rust.String\"
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-TEXT PIC X(20) VALUE \"  Hello  \".
01 WS-OUT  PIC X(40).
01 WS-N    PIC 9(4).
01 WS-TAB.
   05 WS-ROW OCCURS 2 TIMES PIC X(10) VALUE \"abc\".
01 S USAGE IS OBJECT REFERENCE RUST-STRING VALUE \"hello\".
PROCEDURE DIVISION.
MAIN.
{procedure}
    STOP RUN.
"
    );
    run_source(&src, SourceFormat::Free, std::path::Path::new("."))
}

#[test]
fn the_worked_examples_all_pass() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/string-ops");
    let src = std::fs::read_to_string(dir.join("test-data-item-methods.cbl")).expect("read .cbl");
    let out = run_source(&src, detect_format(&src), &dir).expect("the examples run").join("\n");
    println!("{out}");
    // 32 in the outermost program, 8 for the two canonical-spelling forms (20 and 21),
    // then the 13 forms from contained programs in three situations (one program,
    // the same name in two, an outer GLOBAL).
    assert!(out.contains("TESTS RUN       : 0092"), "{out}");
    assert!(out.contains("TESTS PASSED    : 0092"), "{out}");
    assert!(out.contains("ALIAS CHECKS    : 0018 passed of 0018 run"), "{out}");
    assert!(out.contains("TESTS FAILED    : 0000"), "{out}");
    assert!(out.contains("OVERALL RESULT  : PASS"), "{out}");
}

#[test]
fn every_listed_value_method_evaluates() {
    for m in cobolt_ast::methods::DATA_ITEM_METHODS {
        let args = if m.eq_ignore_ascii_case("Replace") { "\"a\" \"b\"" } else { "" };
        let code = format!("    MOVE WS-TEXT::{m}({args}) TO WS-OUT.");
        run(&code).unwrap_or_else(|e| panic!("{m}: {e}"));
    }
    run("    MOVE WS-TEXT::Split(\"l\")(2) TO WS-OUT.").expect("Split(sep)(n)");
    run("    COMPUTE WS-N = WS-TEXT::Length.").expect("the bare Length property");
}

/// The single spelling (`Upper`, `Lower`, `Len`) must work in every FORM the
/// older spelling did — call, chain, subscript, slice, bare property, statement,
/// condition — and give the identical answer, so rewriting one into the other
/// can never change a program (operator, 2026-10-10).
#[test]
fn the_canonical_spellings_work_in_every_form_the_aliases_did() {
    let pairs: [(&str, &[&str]); 3] = [
        ("Upper", &["UpperCase", "ToUpperCase", "toUpperCase", "UPPERCASE"]),
        ("Lower", &["LowerCase", "ToLowerCase", "toLowerCase", "lowercase"]),
        ("Len", &["Length", "LENGTH", "length"]),
    ];
    let mut compared = 0;
    for (canonical, aliases) in pairs {
        for alias in aliases {
            for form in [
                // (a program body with a `{m}` hole, shown by DISPLAY)
                "    DISPLAY WS-TEXT::{m}().",
                "    DISPLAY WS-TEXT::Trim()::{m}().",
                "    DISPLAY WS-ROW(2)::{m}().",
                "    DISPLAY WS-TEXT(3:4)::{m}().",
                "    MOVE WS-TEXT::{m}() TO WS-OUT. DISPLAY WS-OUT.",
                "    IF WS-TEXT::{m}() = WS-TEXT::{m}() DISPLAY \"same\" END-IF.",
                "    STRING WS-TEXT::{m}() DELIMITED BY SIZE INTO WS-OUT. DISPLAY WS-OUT.",
                "    INVOKE WS-TEXT::{m}(). DISPLAY \"ran\".",
            ] {
                let with_alias = run(&form.replace("{m}", alias)).unwrap_or_else(|e| panic!("{alias} in {form}: {e}"));
                let with_canonical = run(&form.replace("{m}", canonical)).unwrap_or_else(|e| panic!("{canonical} in {form}: {e}"));
                assert_eq!(with_alias, with_canonical, "{alias} vs {canonical} in {form}");
                assert!(!with_canonical.is_empty(), "{canonical} in {form} showed nothing");
                compared += 1;
            }
        }
    }
    // The bare property form: no parentheses at all.
    for (alias, canonical) in [("Length", "Len"), ("LENGTH", "len")] {
        let a = run(&format!("    COMPUTE WS-N = WS-TEXT::{alias}. DISPLAY WS-N.")).expect(alias);
        let c = run(&format!("    COMPUTE WS-N = WS-TEXT::{canonical}. DISPLAY WS-N.")).expect(canonical);
        assert_eq!(a, c, "{alias} vs {canonical} as a property");
        assert_eq!(c, vec!["0020".to_string()], "WS-TEXT is PIC X(20)");
        compared += 1;
    }
    println!("value-method spellings — {compared} alias/canonical pairs gave identical answers in 8 call forms + the bare property");
}

#[test]
fn a_name_a_data_item_does_not_answer_is_a_run_time_error() {
    let mut shown = Vec::new();
    for (code, method) in [
        ("    MOVE WS-TEXT::Contains(\"x\") TO WS-OUT.", "CONTAINS"),
        ("    MOVE WS-TEXT::Reverse() TO WS-OUT.", "REVERSE"),
        ("    MOVE WS-TEXT::Value TO WS-OUT.", "VALUE"),
        ("    MOVE WS-TEXT::Trim()::Foo() TO WS-OUT.", "FOO"),
        ("    MOVE WS-ROW(2)::Bar() TO WS-OUT.", "BAR"),
        ("    MOVE WS-TEXT(1:3)::Baz() TO WS-OUT.", "BAZ"),
        ("    DISPLAY WS-TEXT::StartsWith(\"x\").", "STARTSWITH"),
        ("    INVOKE WS-TEXT::Show().", "SHOW"),
    ] {
        let e = run(code).expect_err(code);
        assert!(
            e.to_ascii_uppercase().contains(&format!("NO METHOD OR PROPERTY '{method}'")),
            "{code}: {e}"
        );
        assert!(e.contains("A data item answers: Trim, Upper, Lower, Replace, Len and Split"), "{code}: {e}");
        assert!(e.contains("older spellings that still work"), "{code}: {e}");
        shown.push(e);
    }
    println!("run-time data-item method check — {} unknown names stopped the run, e.g.:\n  {}", shown.len(), shown[0]);
}

#[test]
fn an_object_reference_still_answers_its_own_class() {
    // `S` is a Rust `String` handle: its methods are the class's, not a data item's.
    let out = run("    DISPLAY S::len().").expect("a handle's method still dispatches");
    assert!(out.iter().any(|l| l.contains('5')), "{out:?}");
}

// ── Items of a CONTAINED program ──────────────────────────────────────────────
//
// A form's event handlers are contained programs, each with a WORKING-STORAGE of
// its own (`LK-PATH`, `LK-FILE`, `WS-LINE` …). What such a program declares is
// stored under its own key and reached through the name's resolution; the
// value methods used to test the bare name against the store, found nothing,
// and answered an EMPTY value — while FUNCTION TRIM on the same item was right.

fn run_whole(src: &str) -> Result<Vec<String>, String> {
    run_source(src, SourceFormat::Free, std::path::Path::new("."))
}

/// `FUNCTION TRIM` and `::Trim()` of a name only a contained program declares.
const LOCAL_UNIQUE: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. LOCP.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-TOP  PIC X(20) VALUE \"  top level  \".
PROCEDURE DIVISION.
    DISPLAY \"main new: [\" WS-TOP::Trim() \"]\"
    CALL \"SUB-ONE\"
    STOP RUN.
IDENTIFICATION DIVISION.
PROGRAM-ID. SUB-ONE.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 LK-FILE PIC X(30).
01 WS-DEST PIC X(30).
PROCEDURE DIVISION.
    MOVE SPACES TO LK-FILE
    MOVE \"   file one   \" TO LK-FILE
    DISPLAY \"old: [\" FUNCTION TRIM(LK-FILE) \"]\"
    DISPLAY \"new: [\" LK-FILE::Trim() \"]\"
    MOVE LK-FILE::Trim() TO WS-DEST
    DISPLAY \"dest: [\" WS-DEST \"]\"
    GOBACK.
END PROGRAM SUB-ONE.
END PROGRAM LOCP.
";

#[test]
fn a_unique_name_only_a_contained_program_declares_answers_like_the_function() {
    let out = run_whole(LOCAL_UNIQUE).expect("runs");
    assert_eq!(
        out,
        [
            "main new: [top level]".to_string(),
            "old: [file one]".to_string(),
            "new: [file one]".to_string(),
            format!("dest: [{:<30}]", "file one"),
        ]
    );
}

#[test]
fn one_name_declared_by_two_contained_programs_answers_in_each() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. DUPP.
DATA DIVISION.
WORKING-STORAGE SECTION.
PROCEDURE DIVISION.
    CALL \"SUB-A\"
    CALL \"SUB-B\"
    STOP RUN.
IDENTIFICATION DIVISION.
PROGRAM-ID. SUB-A.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-LINE PIC X(30) VALUE \"   from a   \".
PROCEDURE DIVISION.
    DISPLAY \"a old: [\" FUNCTION TRIM(WS-LINE) \"]\"
    DISPLAY \"a new: [\" WS-LINE::Trim() \"]\"
    DISPLAY \"a len: \" WS-LINE::Len()
    GOBACK.
END PROGRAM SUB-A.
IDENTIFICATION DIVISION.
PROGRAM-ID. SUB-B.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-LINE PIC X(12) VALUE \"  from b\".
PROCEDURE DIVISION.
    DISPLAY \"b old: [\" FUNCTION TRIM(WS-LINE) \"]\"
    DISPLAY \"b new: [\" WS-LINE::Trim() \"]\"
    DISPLAY \"b len: \" WS-LINE::Len()
    GOBACK.
END PROGRAM SUB-B.
END PROGRAM DUPP.
";
    let out = run_whole(src).expect("runs");
    assert_eq!(
        out,
        ["a old: [from a]", "a new: [from a]", "a len: 30", "b old: [from b]", "b new: [from b]", "b len: 12"]
    );
}

#[test]
fn a_contained_program_still_reads_an_outer_global_item() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. GLOP.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-G GLOBAL PIC X(20) VALUE \"  global one  \".
PROCEDURE DIVISION.
    CALL \"SUB-ONE\"
    STOP RUN.
IDENTIFICATION DIVISION.
PROGRAM-ID. SUB-ONE.
PROCEDURE DIVISION.
    DISPLAY \"glob: [\" WS-G::Trim() \"]\"
    GOBACK.
END PROGRAM SUB-ONE.
END PROGRAM GLOP.
";
    assert_eq!(run_whole(src).expect("runs"), ["glob: [global one]"]);
}

/// Every value method, on an item a contained program declares and on an
/// outermost item holding the same text: the two answers are the same, and
/// neither is empty.
#[test]
fn every_value_method_answers_the_same_for_a_contained_programs_item() {
    let calls = [
        ("Trim", ""),
        ("UpperCase", ""),
        ("ToUpperCase", ""),
        ("Upper", ""),
        ("LowerCase", ""),
        ("ToLowerCase", ""),
        ("Lower", ""),
        ("Replace", "\"l\" \"L\""),
        ("Len", ""),
        ("Length", ""),
        ("Split", "\"l\""),
    ];
    // The list under test and the list above must not drift apart.
    assert_eq!(calls.len(), cobolt_ast::methods::DATA_ITEM_METHODS.len());
    for m in cobolt_ast::methods::DATA_ITEM_METHODS {
        assert!(calls.iter().any(|(c, _)| c.eq_ignore_ascii_case(m)), "{m} is not exercised");
    }
    for (m, args) in calls {
        let src = format!(
            "\
IDENTIFICATION DIVISION.
PROGRAM-ID. EVERY.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-TOP GLOBAL PIC X(20) VALUE \"  Hello  \".
PROCEDURE DIVISION.
    CALL \"SUB-ONE\"
    STOP RUN.
IDENTIFICATION DIVISION.
PROGRAM-ID. SUB-ONE.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 LK-TEXT PIC X(20) VALUE \"  Hello  \".
01 LK-OUT  PIC X(60).
PROCEDURE DIVISION.
    MOVE LK-TEXT::{m}({args}) TO LK-OUT
    DISPLAY \"local: [\" LK-OUT \"]\"
    MOVE WS-TOP::{m}({args}) TO LK-OUT
    DISPLAY \"outer: [\" LK-OUT \"]\"
    GOBACK.
END PROGRAM SUB-ONE.
END PROGRAM EVERY.
"
        );
        let out = run_whole(&src).unwrap_or_else(|e| panic!("{m}: {e}"));
        assert_eq!(out.len(), 2, "{m}: {out:?}");
        let local = out[0].strip_prefix("local: [").expect("local line");
        let outer = out[1].strip_prefix("outer: [").expect("outer line");
        assert_eq!(local, outer, "{m}: a contained program's item answers like the outermost one's");
        assert!(!local.trim_end_matches(']').trim().is_empty(), "{m}: the answer is empty: {out:?}");
    }
}

#[test]
fn a_name_declared_twice_in_one_program_is_found_by_its_qualifier() {
    // `WS-FLD OF REC-A` and `OF REC-B` are two items under one leaf name, stored
    // by their path; the bare leaf is stored nowhere.
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. QUAL.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 REC-A.
   05 WS-FLD PIC X(10) VALUE \"  aa  \".
01 REC-B.
   05 WS-FLD PIC X(10) VALUE \"  bb  \".
PROCEDURE DIVISION.
    DISPLAY \"a: [\" WS-FLD OF REC-A::Trim() \"]\"
    DISPLAY \"b: [\" WS-FLD OF REC-B::Trim() \"]\"
    STOP RUN.
";
    assert_eq!(run_whole(src).expect("runs"), ["a: [aa]", "b: [bb]"]);
}

#[test]
fn a_linkage_parameter_of_a_contained_program_still_answers() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. LNK.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-PATH PIC X(20) VALUE \"  /tmp/x  \".
PROCEDURE DIVISION.
    CALL \"SUB-ONE\" USING WS-PATH
    STOP RUN.
IDENTIFICATION DIVISION.
PROGRAM-ID. SUB-ONE.
DATA DIVISION.
LINKAGE SECTION.
01 LK-PATH PIC X(20).
PROCEDURE DIVISION USING LK-PATH.
    DISPLAY \"param: [\" LK-PATH::Trim() \"]\"
    GOBACK.
END PROGRAM SUB-ONE.
END PROGRAM LNK.
";
    assert_eq!(run_whole(src).expect("runs"), ["param: [/tmp/x]"]);
}

#[test]
fn a_name_a_contained_programs_item_does_not_answer_is_a_run_time_error() {
    // The 1.90.47 guard reached only the outermost program's items: on a
    // handler's own item the unknown name was treated as a control member and
    // came out empty.
    for (stmt, method) in [
        ("MOVE LK-FILE::Contains(\"x\") TO WS-DEST", "CONTAINS"),
        ("MOVE LK-FILE::Value TO WS-DEST", "VALUE"),
        ("MOVE LK-FILE::Trim()::Foo() TO WS-DEST", "FOO"),
        ("DISPLAY LK-FILE::StartsWith(\"x\")", "STARTSWITH"),
        ("INVOKE LK-FILE::Show()", "SHOW"),
    ] {
        let src = format!(
            "\
IDENTIFICATION DIVISION.
PROGRAM-ID. BAD.
DATA DIVISION.
WORKING-STORAGE SECTION.
PROCEDURE DIVISION.
    CALL \"SUB-ONE\"
    STOP RUN.
IDENTIFICATION DIVISION.
PROGRAM-ID. SUB-ONE.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 LK-FILE PIC X(30) VALUE \"abc\".
01 WS-DEST PIC X(30).
PROCEDURE DIVISION.
    {stmt}
    GOBACK.
END PROGRAM SUB-ONE.
END PROGRAM BAD.
"
        );
        let e = run_whole(&src).expect_err(stmt);
        assert!(e.to_ascii_uppercase().contains(&format!("NO METHOD OR PROPERTY '{method}'")), "{stmt}: {e}");
        assert!(e.contains("'LK-FILE::"), "{stmt}: the message names the item: {e}");
    }
}

#[test]
fn a_contained_programs_object_reference_still_answers_its_own_class() {
    // The item is declared by the contained program but is a handle, not a
    // value: it must not be taken for a data item now that those are found.
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. OBJP.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
REPOSITORY.
    CLASS RUST-STRING IS \"Rust.String\"
DATA DIVISION.
WORKING-STORAGE SECTION.
PROCEDURE DIVISION.
    CALL \"SUB-ONE\"
    STOP RUN.
IDENTIFICATION DIVISION.
PROGRAM-ID. SUB-ONE.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 S USAGE IS OBJECT REFERENCE RUST-STRING VALUE \"hello\".
PROCEDURE DIVISION.
    DISPLAY S::len()
    GOBACK.
END PROGRAM SUB-ONE.
END PROGRAM OBJP.
";
    let out = run_whole(src).expect("a handle's method still dispatches");
    assert!(out.iter().any(|l| l.contains('5')), "{out:?}");
}
