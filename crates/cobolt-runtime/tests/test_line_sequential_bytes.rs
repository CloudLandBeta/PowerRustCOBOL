// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A LINE SEQUENTIAL file carries the bytes the record holds — the Cnd_PRC report, defect 9. A
//! program that writes a DOS exchange file (code page 850) puts bytes above 127 in a `PIC X` item
//! with `FUNCTION CHAR`; the file must hold those bytes, and a `READ` of such a file must give them
//! back. Before, `WRITE` ran the record through a UTF-8 conversion that replaced every lone high byte
//! with `EF BF BD`, and `READ` took a line that was not UTF-8 for the end of the file.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

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
    let d = std::env::temp_dir().join(format!("prc-linebytes-{tag}-{}", std::process::id()));
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

fn program(path: &Path, working: &str, procedure: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.
PROGRAM-ID. LINEBYTES.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT OUT-FILE ASSIGN TO \"{}\"
        ORGANIZATION IS LINE SEQUENTIAL
        FILE STATUS IS WS-FS.
DATA DIVISION.
FILE SECTION.
FD OUT-FILE.
01 OUT-REC.
   05 OUT-A  PIC X(2).
   05 OUT-HI PIC X.
   05 OUT-Z  PIC X(9).
WORKING-STORAGE SECTION.
01 WS-FS PIC XX.
{working}
PROCEDURE DIVISION.
MAIN-PARA.
{procedure}
    STOP RUN.
",
        path.display()
    )
}

/// One line per entry of `bytes_of`: the byte of each `CHAR(n)` the cases put between two letters.
const WORKING: &str = "01 WS-HI PIC X.
01 WS-ORD PIC 9(3).
01 WS-N PIC 9(2).";

/// The bytes a DOS exchange file holds: C7 (a letter of code page 850), FF, 80 and 9B, each between
/// two letters, written one per line; the last line has a trailing high byte, which is not a space.
#[test]
fn a_record_with_bytes_above_127_is_written_as_those_bytes() {
    let dir = temp_dir("write");
    let path = dir.0.join("morador.txt");
    let t = Instant::now();
    let out = run(&program(
        &path,
        WORKING,
        "    OPEN OUTPUT OUT-FILE
    MOVE FUNCTION CHAR(200) TO WS-HI
    MOVE SPACES TO OUT-REC
    STRING \"AB\" WS-HI \"CD\" DELIMITED BY SIZE INTO OUT-REC
    WRITE OUT-REC
    MOVE FUNCTION CHAR(256) TO WS-HI
    MOVE SPACES TO OUT-REC
    STRING \"E\" WS-HI \"F\" DELIMITED BY SIZE INTO OUT-REC
    WRITE OUT-REC
    MOVE FUNCTION CHAR(129) TO WS-HI
    MOVE SPACES TO OUT-REC
    STRING \"G\" WS-HI WS-HI \"H\" DELIMITED BY SIZE INTO OUT-REC
    WRITE OUT-REC
    MOVE FUNCTION CHAR(156) TO WS-HI
    MOVE SPACES TO OUT-REC
    STRING \"I\" WS-HI DELIMITED BY SIZE INTO OUT-REC
    WRITE OUT-REC
    MOVE \"ção\" TO OUT-REC
    WRITE OUT-REC
    DISPLAY WS-FS
    CLOSE OUT-FILE",
    ));
    let write_ms = t.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(out, vec!["00"]);
    let bytes = std::fs::read(&path).unwrap();
    let want: Vec<u8> = [
        &b"AB\xC7CD\n"[..],
        &b"E\xFFF\n"[..],
        &b"G\x80\x80H\n"[..],
        &b"I\x9B\n"[..],
        "ção\n".as_bytes(),
    ]
    .concat();
    assert_eq!(bytes, want, "the file holds the bytes the records held: {bytes:02X?}");
    assert!(!bytes.windows(3).any(|w| w == [0xEF, 0xBF, 0xBD]), "no replacement character");
    println!("LINE SEQUENTIAL write: 5 records (C7, FF, 80 80, 9B and a UTF-8 line) came out byte for byte in {write_ms:.2} ms");
}

/// A file a DOS program wrote: its lines are not UTF-8. `READ` gives every record, and the bytes
/// are in the item as they were in the file.
#[test]
fn a_line_that_is_not_utf8_is_read_as_a_record() {
    let dir = temp_dir("read");
    let path = dir.0.join("dos.txt");
    std::fs::write(&path, b"AB\xC7CD\r\nEF\xFFG\nplain\nGH\x80\x80I\n").unwrap();
    let t = Instant::now();
    let out = run(&program(
        &path,
        WORKING,
        "    OPEN INPUT OUT-FILE
    PERFORM UNTIL WS-FS NOT = \"00\"
        READ OUT-FILE
            AT END CONTINUE
            NOT AT END
                ADD 1 TO WS-N
                MOVE FUNCTION ORD(OUT-HI) TO WS-ORD
                DISPLAY WS-N \" \" OUT-A \" \" WS-ORD
        END-READ
    END-PERFORM
    DISPLAY WS-FS
    CLOSE OUT-FILE",
    ));
    let read_ms = t.elapsed().as_secs_f64() * 1000.0;
    // ORD gives the position in the collating sequence: the byte plus one.
    assert_eq!(out, vec!["01 AB 200", "02 EF 256", "03 pl 098", "04 GH 129", "10"], "{out:?}");
    println!("LINE SEQUENTIAL read: 4 lines, 3 of them not UTF-8, came back as records (the end is status 10) in {read_ms:.2} ms");
}
