// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 077 M5 — national data in files: an INDEXED file keyed on a
//! `PIC N(20)` item (AC9), a record SEQUENTIAL file, and a LINE SEQUENTIAL
//! file that carries national fields as readable UTF-8 text.

use std::path::PathBuf;
use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp_dir(tag: &str) -> Dir {
    let d = std::env::temp_dir().join(format!("prc-077-files-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    Dir(d)
}

fn run(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    let errors: Vec<_> = result.diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(result.program.unwrap(), event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect()
}

fn indexed_program(path: &str, procedure: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.
PROGRAM-ID. NATIDX.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT CUSTOMERS ASSIGN TO \"{path}\"
        ORGANIZATION IS INDEXED
        ACCESS MODE IS DYNAMIC
        RECORD KEY IS CUST-NAME
        FILE STATUS IS WS-FS.
DATA DIVISION.
FILE SECTION.
FD CUSTOMERS.
01 CUST-REC.
   05 CUST-NAME PIC N(20).
   05 CUST-CITY PIC N(40).
WORKING-STORAGE SECTION.
01 WS-FS PIC XX.
01 WS-L  PIC 9(4).
PROCEDURE DIVISION.
MAIN-PARA.
{procedure}
    STOP RUN.
"
    )
}

/// AC9: written, read back by key and rewritten in one run; reopened by a
/// second run with every character intact. The record is 120 bytes: 2 × 20 +
/// 2 × 40.
#[test]
fn ac9_an_indexed_file_keyed_on_national_data() {
    let dir = temp_dir("idx");
    let path = dir.0.join("customers.dat");
    let path = path.to_string_lossy();

    let first = run(&indexed_program(
        &path,
        "    OPEN OUTPUT CUSTOMERS
    MOVE N\"João\" TO CUST-NAME
    MOVE N\"São Paulo – Brasil\" TO CUST-CITY
    WRITE CUST-REC
    MOVE N\"Åsa\" TO CUST-NAME
    MOVE N\"Göteborg\" TO CUST-CITY
    WRITE CUST-REC
    MOVE N\"渡辺\" TO CUST-NAME
    MOVE N\"東京\" TO CUST-CITY
    WRITE CUST-REC
    CLOSE CUSTOMERS
    OPEN I-O CUSTOMERS
    MOVE N\"João\" TO CUST-NAME
    READ CUSTOMERS
    DISPLAY WS-FS \" [\" CUST-CITY \"]\"
    MOVE N\"Florianópolis\" TO CUST-CITY
    REWRITE CUST-REC
    DISPLAY WS-FS
    MOVE FUNCTION BYTE-LENGTH(CUST-REC) TO WS-L
    DISPLAY WS-L
    CLOSE CUSTOMERS",
    ));
    assert_eq!(first[0], format!("00 [{:<40}]", "São Paulo – Brasil"));
    assert_eq!(first[1], "00");
    assert_eq!(first[2], "0120");

    // A second run: keys and fields come back as the same characters, in key
    // order — UTF-16 big-endian, which is code-point order here: J U+004A,
    // Å U+00C5, 渡 U+6E21.
    let second = run(&indexed_program(
        &path,
        "    OPEN INPUT CUSTOMERS
    MOVE N\"渡辺\" TO CUST-NAME
    READ CUSTOMERS
    DISPLAY WS-FS \" [\" CUST-CITY \"]\"
    MOVE LOW-VALUES TO CUST-NAME
    START CUSTOMERS KEY IS NOT LESS THAN CUST-NAME
    READ CUSTOMERS NEXT
    DISPLAY \"[\" CUST-NAME \"][\" CUST-CITY \"]\"
    READ CUSTOMERS NEXT
    DISPLAY \"[\" CUST-NAME \"][\" CUST-CITY \"]\"
    READ CUSTOMERS NEXT
    DISPLAY \"[\" CUST-NAME \"][\" CUST-CITY \"]\"
    CLOSE CUSTOMERS",
    ));
    assert_eq!(second[0], format!("00 [{}]", pad("東京", 40)));
    assert_eq!(second[1], format!("[{}][{}]", pad("João", 20), pad("Florianópolis", 40)));
    assert_eq!(second[2], format!("[{}][{}]", pad("Åsa", 20), pad("Göteborg", 40)));
    assert_eq!(second[3], format!("[{}][{}]", pad("渡辺", 20), pad("東京", 40)));
    // The file holds UTF-16: the key "João" is there as 00 4A 00 6F 00 E3 00 6F.
    let bytes = std::fs::read(dir.0.join("customers.dat")).unwrap();
    assert!(bytes.windows(8).any(|w| w == [0x00, 0x4A, 0x00, 0x6F, 0x00, 0xE3, 0x00, 0x6F]));
}

/// `text` padded with spaces to `n` characters.
fn pad(text: &str, n: usize) -> String {
    let mut s = text.to_owned();
    s.extend(std::iter::repeat_n(' ', n - text.chars().count()));
    s
}

fn seq_program(path: &str, org: &str, procedure: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.
PROGRAM-ID. NATSEQ.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT NOTES ASSIGN TO \"{path}\"
        ORGANIZATION IS {org}.
DATA DIVISION.
FILE SECTION.
FD NOTES.
01 NOTE-REC.
   05 NOTE-ID   PIC 9(3).
   05 NOTE-TEXT PIC N(12).
   05 NOTE-TAG  PIC X(4).
WORKING-STORAGE SECTION.
PROCEDURE DIVISION.
MAIN-PARA.
{procedure}
    STOP RUN.
"
    )
}

const WRITE_TWO: &str = "    OPEN OUTPUT NOTES
    MOVE 1 TO NOTE-ID
    MOVE N\"Atenção!\" TO NOTE-TEXT
    MOVE \"ok\" TO NOTE-TAG
    WRITE NOTE-REC
    MOVE 2 TO NOTE-ID
    MOVE N\"日本語のメモ\" TO NOTE-TEXT
    MOVE \"jp\" TO NOTE-TAG
    WRITE NOTE-REC
    CLOSE NOTES";

const READ_TWO: &str = "    OPEN INPUT NOTES
    READ NOTES
    DISPLAY NOTE-ID \"[\" NOTE-TEXT \"][\" NOTE-TAG \"]\"
    READ NOTES
    DISPLAY NOTE-ID \"[\" NOTE-TEXT \"][\" NOTE-TAG \"]\"
    CLOSE NOTES";

#[test]
fn a_record_sequential_round_trip() {
    let dir = temp_dir("seq");
    let path = dir.0.join("notes.dat").to_string_lossy().into_owned();
    run(&seq_program(&path, "SEQUENTIAL", WRITE_TWO));
    // 3 + 24 + 4 bytes a record.
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 62);
    let out = run(&seq_program(&path, "SEQUENTIAL", READ_TWO));
    assert_eq!(out, vec![format!("001[{}][ok  ]", pad("Atenção!", 12)), format!("002[{}][jp  ]", pad("日本語のメモ", 12))]);
}

/// A LINE SEQUENTIAL file is text: the national field is written as its
/// characters in UTF-8, and read back from them.
#[test]
fn a_line_sequential_round_trip_is_readable_text() {
    let dir = temp_dir("line");
    let path = dir.0.join("notes.txt").to_string_lossy().into_owned();
    run(&seq_program(&path, "LINE SEQUENTIAL", WRITE_TWO));
    let text = std::fs::read_to_string(&path).expect("the file is UTF-8 text");
    assert_eq!(
        text.lines().collect::<Vec<_>>(),
        vec![format!("001{}ok", pad("Atenção!", 12)), format!("002{}jp", pad("日本語のメモ", 12))]
    );
    let out = run(&seq_program(&path, "LINE SEQUENTIAL", READ_TWO));
    assert_eq!(out, vec![format!("001[{}][ok  ]", pad("Atenção!", 12)), format!("002[{}][jp  ]", pad("日本語のメモ", 12))]);
}
