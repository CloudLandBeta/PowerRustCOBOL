// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Paragraphs written BEFORE the first section header.
//!
//! COBOL-85 allows a PROCEDURE DIVISION to open with paragraphs and only later
//! introduce sections. **Every CCVS85 program has that shape** — the test
//! paragraphs come first, the SORT/merge output procedures are sections further
//! down — so if those leading paragraphs are lost, no program in the suite can
//! run at all.

use cobolt_ast::program::ProcedureBody;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;

fn body(src: &str) -> ProcedureBody {
    let r = parse(tokenize(src, SourceFormat::Free));
    let errs: Vec<&String> = r
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| &d.message)
        .collect();
    assert!(errs.is_empty(), "parse errors: {errs:?}");
    r.program.expect("no program").procedure.body
}

const LEADING_PARAS_THEN_SECTION: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. S4.
PROCEDURE DIVISION.
MAIN-PARA.
    DISPLAY \"ran\".
    STOP RUN.
OUTP-SEC SECTION.
OUTP-PARA.
    DISPLAY \"other\".
";

/// The leading paragraph must survive, and it must be **first** — execution
/// starts at the top of the PROCEDURE DIVISION.
#[test]
fn a_paragraph_before_the_first_section_is_kept_and_comes_first() {
    match body(LEADING_PARAS_THEN_SECTION) {
        ProcedureBody::Sections(secs) => {
            let all: Vec<&str> = secs
                .iter()
                .flat_map(|s| s.paragraphs.iter())
                .map(|p| p.name.as_str())
                .collect();
            assert_eq!(
                all,
                vec!["MAIN-PARA", "OUTP-PARA"],
                "the leading paragraph must be kept, in source order: {all:?}"
            );
            let first = secs
                .iter()
                .flat_map(|s| s.paragraphs.iter())
                .next()
                .expect("at least one paragraph");
            assert!(
                !first.stmts.is_empty(),
                "MAIN-PARA must carry its statements — an empty first paragraph \
                 means the program starts by doing nothing"
            );
        }
        ProcedureBody::Paragraphs(paras) => {
            // Also acceptable, as long as nothing was lost.
            let all: Vec<&str> = paras.iter().map(|p| p.name.as_str()).collect();
            assert!(all.contains(&"MAIN-PARA"), "{all:?}");
            assert!(all.contains(&"OUTP-PARA"), "{all:?}");
        }
    }
}

/// A `GO TO` may target a paragraph declared later in the SAME section —
/// forward references inside a section are ordinary COBOL, and CCVS85's
/// `GO TO OUTP3-EXIT` relies on it (ST101A).
#[test]
fn a_paragraph_declared_inside_a_section_is_a_valid_target() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. S6.
PROCEDURE DIVISION.
MAIN-PARA.
    PERFORM OUTP3 THRU OUTP3-EXIT.
    STOP RUN.
OUTP3 SECTION.
OUTP3-START.
    GO TO OUTP3-EXIT.
OUTP3-EXIT.
    EXIT.
";
    let r = parse(tokenize(src, SourceFormat::Free));
    let program = r.program.expect("no program");
    let diags = cobolt_semantic::analyze(&program).diagnostics;
    let errs: Vec<&String> = diags
        .iter()
        .filter(|d| d.severity == cobolt_semantic::Severity::Error)
        .map(|d| &d.message)
        .collect();
    assert!(
        errs.is_empty(),
        "a paragraph inside a section must be reachable: {errs:?}"
    );
}

// ── `[AT] END` — the `AT` is optional ────────────────────────────────────────
//
// COBOL-85 writes the phrase as `[AT] END`, so `READ f END …` and
// `RETURN f END …` mean the same as the `AT END` spelling. CCVS85 tests both
// deliberately: ST101A writes one `RETURN` "WITH ALL OPTIONAL WORDS" and the
// next "WITHOUT OPTIONAL WORDS".
//
// The bare form was not consumed, so the phrase ran on and swallowed the
// **next paragraph header** — which is why `OUTP3-EXIT` then looked undeclared
// to every `GO TO` that targeted it. This was the single largest cause of
// failure left in the suite.

fn errors_of(src: &str) -> Vec<String> {
    let r = parse(tokenize(src, SourceFormat::Free));
    let mut out: Vec<String> = r
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| d.message.clone())
        .collect();
    if let Some(p) = r.program.as_ref() {
        out.extend(
            cobolt_semantic::analyze(p)
                .diagnostics
                .iter()
                .filter(|d| d.severity == cobolt_semantic::Severity::Error)
                .map(|d| d.message.clone()),
        );
    }
    out
}

/// The paragraph after a bare-`END` phrase must survive as a paragraph.
#[test]
fn a_bare_end_phrase_does_not_swallow_the_next_paragraph() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. ATEND.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT INF ASSIGN TO \"in.dat\"
        ORGANIZATION IS LINE SEQUENTIAL.
DATA DIVISION.
FILE SECTION.
FD  INF.
01  IN-REC PIC X(10).
PROCEDURE DIVISION.
MAIN-PARA.
    PERFORM RET-2.
    GO TO TARGET-PARA.
RET-2.
    READ INF END GO TO TARGET-PARA.
TARGET-PARA.
    STOP RUN.
";
    assert!(
        errors_of(src).is_empty(),
        "bare END must be the AT END phrase: {:?}",
        errors_of(src)
    );
}

/// Both spellings parse, and to the same thing.
#[test]
fn at_end_and_bare_end_are_the_same_phrase() {
    let with_at = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. A1.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT INF ASSIGN TO \"in.dat\" ORGANIZATION IS LINE SEQUENTIAL.
DATA DIVISION.
FILE SECTION.
FD  INF.
01  IN-REC PIC X(10).
PROCEDURE DIVISION.
MAIN-PARA.
    READ INF AT END CONTINUE END-READ.
    STOP RUN.
";
    let bare = with_at.replace("AT END CONTINUE", "END CONTINUE");
    assert!(errors_of(with_at).is_empty(), "{:?}", errors_of(with_at));
    assert!(errors_of(&bare).is_empty(), "{:?}", errors_of(&bare));

    // Compare the STRUCTURE, not the spans: dropping the word `AT` shortens
    // the text, so every later byte offset legitimately differs.
    let shape = |s: &str| {
        let dbg = format!("{:?}", parse(tokenize(s, SourceFormat::Free)).program);
        let mut out = String::with_capacity(dbg.len());
        let mut rest = dbg.as_str();
        while let Some(i) = rest.find("Span {") {
            out.push_str(&rest[..i]);
            match rest[i..].find('}') {
                Some(j) => rest = &rest[i + j + 1..],
                None => {
                    rest = "";
                    break;
                }
            }
        }
        out.push_str(rest);
        out
    };
    assert_eq!(
        shape(with_at),
        shape(&bare),
        "the two spellings must produce the same statement"
    );
}

/// A bare `END` must NOT swallow `END PROGRAM` — the only other thing it can
/// begin. Without this guard the fix would break every nested program.
#[test]
fn a_bare_end_does_not_swallow_end_program() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. OUTERP.
PROCEDURE DIVISION.
MAIN-PARA.
    DISPLAY \"outer\".
    STOP RUN.
END PROGRAM OUTERP.
";
    assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
}
