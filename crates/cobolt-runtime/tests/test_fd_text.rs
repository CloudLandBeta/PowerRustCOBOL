// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The text an indexed file carries for its record layout is the `FD` itself,
//! and it **reads back** to the same descriptions: every level, name, PICTURE,
//! USAGE, REDEFINES, OCCURS, SIGN, VALUE and condition name that the parser
//! produced from the original comes out of the parser again from the text.

use cobolt_ast::data::FileDescription;
use cobolt_ast::fd_text::render_file_description;
use cobolt_ast::program::DataSection;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};

/// A program around `fd_source`, which is the text of one or more FDs.
fn program_around(fd_source: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. WRAP.\nENVIRONMENT DIVISION.\n\
         INPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT F ASSIGN TO \"x\"\n\
         \x20       ORGANIZATION IS INDEXED RECORD KEY IS CUST-ID.\n\
         DATA DIVISION.\nFILE SECTION.\n{fd_source}\nPROCEDURE DIVISION.\n    STOP RUN.\n"
    )
}

fn fds_of(source: &str) -> Vec<FileDescription> {
    let result = parse(tokenize(source, SourceFormat::Free));
    let errors: Vec<_> = result.diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}\n--- source ---\n{source}");
    let program = result.program.expect("a program");
    let mut out = Vec::new();
    for section in program.data.expect("a DATA DIVISION").sections {
        if let DataSection::FileSection(fds) = section {
            out.extend(fds);
        }
    }
    out
}

/// The structure of `fd` with every source position removed — what the text
/// has to reproduce, and nothing about where it was written.
fn shape(fd: &FileDescription) -> serde_json::Value {
    fn strip(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(m) => {
                m.remove("span");
                m.values_mut().for_each(strip);
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut v = serde_json::to_value(fd).expect("the AST serialises");
    strip(&mut v);
    v
}

const RICH: &str = r#"FD  CUSTOMER-FILE
    RECORD CONTAINS 120 CHARACTERS.
01  CUSTOMER-REC.
    05  CUST-ID        PIC 9(6).
    05  CUST-NAME      PIC X(30) JUSTIFIED RIGHT.
    05  BALANCE        PIC S9(7)V99 COMP-3.
    05  BAL-TEXT       REDEFINES BALANCE PIC X(5).
    05  PRICE          PIC S9(4)V99 SIGN IS LEADING SEPARATE CHARACTER.
    05  QTY            PIC 9(4) COMP.
    05  QTY-BIN        PIC S9(9) BINARY.
    05  QTY-5          PIC 9(9) COMP-5.
    05  PACKED         PIC 9(5) PACKED-DECIMAL.
    05  RATE           COMP-1.
    05  RATE-2         COMP-2.
    05  FLAG           PIC X.
        88  IS-ACTIVE  VALUE "A".
        88  IS-GRADE   VALUE "A" THRU "C" "X".
    05  MONTHS         OCCURS 12 TIMES.
        10  M-AMT      PIC 9(5)V99.
    05  LINES-CNT      PIC 99.
    05  LINE-ITEM      OCCURS 1 TO 5 TIMES DEPENDING ON LINES-CNT
                       ASCENDING KEY IS LI-ID INDEXED BY LX.
        10  LI-ID      PIC 9(3).
    05  FILLER         PIC X(4) VALUE SPACES.
    05  ZERO-FIELD     PIC 9(3) VALUE ZERO BLANK WHEN ZERO.
    05  NEG-VAL        PIC S9(3) VALUE -5.
    05  DEC-VAL        PIC 9V99 VALUE 1.50.
    05  STARS          PIC X(3) VALUE ALL "*".
    05  QUOTED         PIC X(9) VALUE "say ""hi""".
    05  WORD           PIC N(5).
    05  TAG            PIC U(4).
    05  EDITED         PIC ZZ,ZZ9.99-.
    66  WHOLE          RENAMES CUST-ID THRU CUST-NAME.
01  CUSTOMER-ALT.
    05  ALT-A          PIC X(60).
"#;

#[test]
fn the_text_of_an_fd_reads_back_to_the_same_descriptions() {
    let original = fds_of(&program_around(RICH));
    assert_eq!(original.len(), 1);
    let text = render_file_description(&original[0]);

    let again = fds_of(&program_around(&text));
    assert_eq!(again.len(), 1, "the text is one FD:\n{text}");
    assert_eq!(shape(&original[0]), shape(&again[0]), "the text lost something:\n{text}");

    // …and writing what was read back is writing the same text: it is canonical.
    assert_eq!(render_file_description(&again[0]), text);
    println!("{text}");
}

#[test]
fn the_text_names_every_clause_a_reader_needs() {
    let fd = &fds_of(&program_around(RICH))[0];
    let text = render_file_description(fd);
    for needle in [
        "FD CUSTOMER-FILE\n    RECORD CONTAINS 120 CHARACTERS.",
        "05 BALANCE PIC S9(7)V99 COMP-3.",
        "05 BAL-TEXT REDEFINES BALANCE PIC X(5).",
        "05 PRICE PIC S9(4)V99 SIGN IS LEADING SEPARATE CHARACTER.",
        "05 RATE COMP-1.",
        "OCCURS 1 TO 5 TIMES DEPENDING ON LINES-CNT ASCENDING KEY IS LI-ID INDEXED BY LX",
        "05 FILLER PIC X(4) VALUE SPACE.",
        "88 IS-GRADE VALUE \"A\" THRU \"C\" \"X\".",
        "66 WHOLE RENAMES CUST-ID THRU CUST-NAME.",
        "01 CUSTOMER-ALT.",
    ] {
        assert!(text.contains(needle), "missing `{needle}` in:\n{text}");
    }
}

#[test]
fn a_variable_length_record_clause_reads_back() {
    for clause in [
        "RECORD CONTAINS 10 TO 80 CHARACTERS",
        "RECORD VARYING FROM 10 TO 80 CHARACTERS DEPENDING ON LEN",
        "RECORD CONTAINS 80 CHARACTERS",
    ] {
        let src = format!("FD CUSTOMER-FILE\n    {clause}.\n01 CUSTOMER-REC.\n    05 CUST-ID PIC 9(6).\n    05 LEN PIC 99.\n");
        let original = fds_of(&program_around(&src));
        let text = render_file_description(&original[0]);
        let again = fds_of(&program_around(&text));
        assert_eq!(shape(&original[0]), shape(&again[0]), "{clause}\n{text}");
    }
}
