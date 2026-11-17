// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! SORT runtime: procedure-based (RELEASE / RETURN via INPUT/OUTPUT PROCEDURE)
//! and file-based (USING / GIVING).

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

#[test]
fn sort_input_output_procedure() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SRT.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT SORT-FILE ASSIGN TO "sortwork".
       DATA DIVISION.
       FILE SECTION.
       SD SORT-FILE.
       01 SORT-REC.
          05 SORT-KEY  PIC 9(2).
          05 SORT-DATA PIC X(3).
       WORKING-STORAGE SECTION.
       01 DONE-FLAG PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           SORT SORT-FILE ON DESCENDING KEY SORT-KEY
               INPUT PROCEDURE IS FILL-PROC
               OUTPUT PROCEDURE IS SHOW-PROC
           STOP RUN.
       FILL-PROC.
           MOVE 30 TO SORT-KEY MOVE "CCC" TO SORT-DATA RELEASE SORT-REC
           MOVE 10 TO SORT-KEY MOVE "AAA" TO SORT-DATA RELEASE SORT-REC
           MOVE 20 TO SORT-KEY MOVE "BBB" TO SORT-DATA RELEASE SORT-REC.
       SHOW-PROC.
           PERFORM UNTIL DONE-FLAG = 1
               RETURN SORT-FILE
                   AT END MOVE 1 TO DONE-FLAG
                   NOT AT END DISPLAY SORT-DATA
               END-RETURN
           END-PERFORM.
    "#;
    // DESCENDING by key 30,20,10 → CCC, BBB, AAA
    assert_eq!(run_capture(src), vec!["CCC", "BBB", "AAA"]);
}

#[test]
fn sort_using_giving_files() {
    let dir = std::env::temp_dir();
    let inp = dir.join("prc_sort_in.dat");
    let out = dir.join("prc_sort_out.dat");
    std::fs::write(&inp, "30CCC\n10AAA\n20BBB\n").unwrap();
    let _ = std::fs::remove_file(&out);

    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUG.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT IN-FILE ASSIGN TO "{inp}"
               ORGANIZATION IS LINE SEQUENTIAL.
           SELECT OUT-FILE ASSIGN TO "{out}"
               ORGANIZATION IS LINE SEQUENTIAL.
           SELECT SORT-FILE ASSIGN TO "{wk}".
       DATA DIVISION.
       FILE SECTION.
       FD IN-FILE.
       01 IN-REC PIC X(5).
       FD OUT-FILE.
       01 OUT-REC PIC X(5).
       SD SORT-FILE.
       01 SORT-REC.
          05 SORT-KEY  PIC 9(2).
          05 SORT-DATA PIC X(3).
       WORKING-STORAGE SECTION.
       01 EOF-FLAG PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           SORT SORT-FILE ON ASCENDING KEY SORT-KEY
               USING IN-FILE GIVING OUT-FILE
           OPEN INPUT OUT-FILE
           PERFORM UNTIL EOF-FLAG = 1
               READ OUT-FILE
                   AT END MOVE 1 TO EOF-FLAG
                   NOT AT END DISPLAY OUT-REC
               END-READ
           END-PERFORM
           CLOSE OUT-FILE
           STOP RUN.
    "#,
        inp = inp.display(),
        out = out.display(),
        wk = dir.join("prc_sort_wk.dat").display(),
    );
    assert_eq!(run_capture(&src), vec!["10AAA", "20BBB", "30CCC"]);
}

/// CCVS85 **ST119A** (XI-19 4.4.4 GR(10)): a sort's INPUT PROCEDURE may
/// `GO TO` a paragraph *outside* its THRU range ("external code") which
/// jumps straight back in. The ordinary range runner escapes on such a jump,
/// and the escape unwound through the SORT itself: the sort ran on an empty
/// buffer, the releases landed later by top-level fall-through, and every
/// ordering test read back the release order. The procedure must instead
/// follow the jump and end when its last paragraph completes.
#[test]
fn sort_input_procedure_goto_external_code_and_back() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SRTGO.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT SORT-FILE ASSIGN TO "sortwork2".
       DATA DIVISION.
       FILE SECTION.
       SD SORT-FILE.
       01 SORT-REC.
          05 SORT-KEY  PIC 9(2).
          05 SORT-DATA PIC X(3).
       WORKING-STORAGE SECTION.
       01 DONE-FLAG PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           SORT SORT-FILE ON ASCENDING KEY SORT-KEY
               INPUT PROCEDURE IS IN-1 THRU IN-EXIT
               OUTPUT PROCEDURE IS SHOW-PROC
           STOP RUN.
       IN-1.
           MOVE 30 TO SORT-KEY MOVE "CCC" TO SORT-DATA RELEASE SORT-REC
           GO TO EXTERNAL-CODE.
       IN-2.
           MOVE 10 TO SORT-KEY MOVE "AAA" TO SORT-DATA RELEASE SORT-REC.
       IN-EXIT.
           EXIT.
       SHOW-PROC.
           PERFORM UNTIL DONE-FLAG = 1
               RETURN SORT-FILE
                   AT END MOVE 1 TO DONE-FLAG
                   NOT AT END DISPLAY SORT-DATA
               END-RETURN
           END-PERFORM.
       NEVER-HERE.
           DISPLAY "WRONG".
       EXTERNAL-CODE.
           MOVE 20 TO SORT-KEY MOVE "BBB" TO SORT-DATA RELEASE SORT-REC
           GO TO IN-2.
    "#;
    // All three releases belong to the input procedure — including the one
    // made in the "external" paragraph — and the sort must see them all.
    assert_eq!(run_capture(src), vec!["AAA", "BBB", "CCC"]);
}

/// CCVS85 **ST131A**: `RELEASE S2 FROM R2` where the SD record covers the
/// low-order sort digits with FILLER. Materializing the release from named
/// fields alone blanked those bytes, `GIVING` wrote the blanks to disk, and
/// the third sort's expected keys never existed. The release must carry the
/// record's whole image — named fields over the stored bytes, as WRITE does.
#[test]
fn release_from_keeps_bytes_only_filler_covers() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SRTFIL.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT SORT-FILE ASSIGN TO "sortwork3".
       DATA DIVISION.
       FILE SECTION.
       SD SORT-FILE.
       01 S-REC.
          05 S-KEY  PIC 9(2).
          05 FILLER PIC X(3).
       WORKING-STORAGE SECTION.
       01 W-REC.
          05 W-KEY PIC 9(2).
          05 W-TAG PIC X(3).
       01 W-BACK.
          05 B-KEY PIC 9(2).
          05 B-TAG PIC X(3).
       01 DONE-FLAG PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           SORT SORT-FILE ON ASCENDING KEY S-KEY
               INPUT PROCEDURE IS FILL-PROC
               OUTPUT PROCEDURE IS SHOW-PROC
           STOP RUN.
       FILL-PROC.
           MOVE 20 TO W-KEY MOVE "XYZ" TO W-TAG RELEASE S-REC FROM W-REC
           MOVE 10 TO W-KEY MOVE "ABC" TO W-TAG RELEASE S-REC FROM W-REC.
       SHOW-PROC.
           PERFORM UNTIL DONE-FLAG = 1
               RETURN SORT-FILE INTO W-BACK
                   AT END MOVE 1 TO DONE-FLAG
                   NOT AT END DISPLAY B-KEY "/" B-TAG
               END-RETURN
           END-PERFORM.
    "#;
    // The tag bytes live under the SD record's FILLER; they must survive the
    // release and come back in sorted order.
    assert_eq!(run_capture(src), vec!["10/ABC", "20/XYZ"]);
}

/// CCVS85 **ST139A/ST140A**: `SORT … [COLLATING] SEQUENCE [IS] alphabet-name`
/// orders the sort's alphanumeric keys by a SPECIAL-NAMES alphabet. A literal
/// alphabet listing "C" "B" "A" puts C lowest, so an ASCENDING sort on that
/// sequence returns C, B, A.
#[test]
fn sort_collating_sequence_orders_keys_by_the_named_alphabet() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SRTCOLL.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           ALPHABET BACKWARDS IS "C" "B" "A".
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT SORT-FILE ASSIGN TO "sortwork4".
       DATA DIVISION.
       FILE SECTION.
       SD SORT-FILE.
       01 S-REC.
          05 S-KEY  PIC X.
          05 S-TAG  PIC X(2).
       WORKING-STORAGE SECTION.
       01 DONE-FLAG PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           SORT SORT-FILE ON ASCENDING KEY S-KEY
               COLLATING SEQUENCE IS BACKWARDS
               INPUT PROCEDURE IS FILL-PROC
               OUTPUT PROCEDURE IS SHOW-PROC
           STOP RUN.
       FILL-PROC.
           MOVE "A" TO S-KEY MOVE "1A" TO S-TAG RELEASE S-REC
           MOVE "C" TO S-KEY MOVE "3C" TO S-TAG RELEASE S-REC
           MOVE "B" TO S-KEY MOVE "2B" TO S-TAG RELEASE S-REC.
       SHOW-PROC.
           PERFORM UNTIL DONE-FLAG = 1
               RETURN SORT-FILE
                   AT END MOVE 1 TO DONE-FLAG
                   NOT AT END DISPLAY S-KEY "/" S-TAG
               END-RETURN
           END-PERFORM.
    "#;
    // ASCENDING under the BACKWARDS alphabet: C first, then B, then A.
    assert_eq!(run_capture(src), vec!["C/3C", "B/2B", "A/1A"]);
}

/// `SORT` and `MERGE` with a RELATIVE or INDEXED file in `USING` or `GIVING`
/// (ISO 1989:1985 XI-4.4 GR 9 and 10): each `USING` file is read as if by
/// `READ … NEXT … AT END`, so a relative file gives its records in record
/// number order, skipping the empty slots, and an indexed file in ascending
/// RECORD KEY order; each `GIVING` record is written as if by `WRITE`, so a
/// relative file's records go to slots 1, 2, 3, … and its RELATIVE KEY item
/// is left on the last of them. The sort read only sequential files: from
/// a relative or indexed `USING` file it took no record, and it wrote none
/// to such a `GIVING` file.
#[test]
fn sort_and_merge_use_relative_and_indexed_files() {
    let dir = std::env::temp_dir().join(format!("prc_sort_ri_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = |n: &str| dir.join(n).display().to_string();
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SRTRI.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT IDX-IN ASSIGN TO "{idx_in}"
               ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC
               RECORD KEY IS II-KEY.
           SELECT REL-IN ASSIGN TO "{rel_in}"
               ORGANIZATION IS RELATIVE ACCESS MODE IS RANDOM
               RELATIVE KEY IS WS-RIN.
           SELECT SEQ-OUT ASSIGN TO "{seq_out}"
               ORGANIZATION IS LINE SEQUENTIAL.
           SELECT REL-OUT ASSIGN TO "{rel_out}"
               ORGANIZATION IS RELATIVE ACCESS MODE IS SEQUENTIAL
               RELATIVE KEY IS WS-ROUT.
           SELECT IDX-OUT ASSIGN TO "{idx_out}"
               ORGANIZATION IS INDEXED ACCESS MODE IS SEQUENTIAL
               RECORD KEY IS IO-KEY.
           SELECT REL-WIDE ASSIGN TO "{rel_nine}"
               ORGANIZATION IS RELATIVE ACCESS MODE IS RANDOM
               RELATIVE KEY IS WS-RWIDE.
           SELECT REL-NARROW ASSIGN TO "{rel_nine}"
               ORGANIZATION IS RELATIVE ACCESS MODE IS SEQUENTIAL
               RELATIVE KEY IS WS-RNARROW.
           SELECT SORT-FILE ASSIGN TO "{wk}".
       DATA DIVISION.
       FILE SECTION.
       FD IDX-IN.
       01 II-REC.
          05 II-KEY  PIC 9(2).
          05 II-NAME PIC X(3).
       FD REL-IN.
       01 RI-REC PIC X(5).
       FD SEQ-OUT.
       01 SO-REC PIC X(5).
       FD REL-OUT.
       01 RO-REC PIC X(5).
       FD REL-WIDE.
       01 RW-REC PIC X(5).
       FD REL-NARROW.
       01 RN-REC PIC X(5).
       FD IDX-OUT.
       01 IO-REC.
          05 IO-KEY  PIC 9(2).
          05 IO-NAME PIC X(3).
       SD SORT-FILE.
       01 SORT-REC.
          05 S-KEY  PIC 9(2).
          05 S-NAME PIC X(3).
       WORKING-STORAGE SECTION.
       01 WS-RIN  PIC 9(3).
       01 WS-ROUT PIC 9(3).
       01 WS-RWIDE   PIC 9(2).
       01 WS-RNARROW PIC 9.
       01 EOF-FLAG PIC 9.
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT IDX-IN
           MOVE "30CCC" TO II-REC WRITE II-REC
           MOVE "10AAA" TO II-REC WRITE II-REC
           MOVE "20BBB" TO II-REC WRITE II-REC
           CLOSE IDX-IN
           OPEN OUTPUT REL-IN
           MOVE 5 TO WS-RIN MOVE "50EEE" TO RI-REC WRITE RI-REC
           MOVE 2 TO WS-RIN MOVE "60FFF" TO RI-REC WRITE RI-REC
           MOVE 7 TO WS-RIN MOVE "40DDD" TO RI-REC WRITE RI-REC
           CLOSE REL-IN
           *> An indexed file in, by descending name.
           SORT SORT-FILE ON DESCENDING KEY S-NAME
               USING IDX-IN GIVING SEQ-OUT
           PERFORM SHOW-SEQ
           *> A relative file in and a relative file out.
           MOVE 0 TO WS-ROUT
           SORT SORT-FILE ON ASCENDING KEY S-KEY
               USING REL-IN GIVING REL-OUT
           DISPLAY "REL-OUT KEY AFTER SORT " WS-ROUT
           PERFORM SHOW-REL
           *> Both in, an indexed file out in its key order.
           SORT SORT-FILE ON ASCENDING KEY S-KEY
               USING REL-IN IDX-IN GIVING IDX-OUT
           PERFORM SHOW-IDX
           *> MERGE: two files already in key order, a relative file out.
           MERGE SORT-FILE ON ASCENDING KEY S-KEY
               USING IDX-IN IDX-OUT GIVING REL-OUT
           DISPLAY "MERGED KEY " WS-ROUT
           PERFORM SHOW-REL
           *> A record number wider than the RELATIVE KEY ends the reading.
           OPEN OUTPUT REL-WIDE
           MOVE 3 TO WS-RWIDE MOVE "70GGG" TO RW-REC WRITE RW-REC
           MOVE 12 TO WS-RWIDE MOVE "80HHH" TO RW-REC WRITE RW-REC
           CLOSE REL-WIDE
           SORT SORT-FILE ON ASCENDING KEY S-KEY
               USING REL-NARROW GIVING SEQ-OUT
           PERFORM SHOW-SEQ
           STOP RUN.
       SHOW-SEQ.
           MOVE 0 TO EOF-FLAG
           OPEN INPUT SEQ-OUT
           PERFORM UNTIL EOF-FLAG = 1
               READ SEQ-OUT
                   AT END MOVE 1 TO EOF-FLAG
                   NOT AT END DISPLAY "SEQ " SO-REC
               END-READ
           END-PERFORM
           CLOSE SEQ-OUT.
       SHOW-REL.
           MOVE 0 TO EOF-FLAG
           OPEN INPUT REL-OUT
           PERFORM UNTIL EOF-FLAG = 1
               READ REL-OUT NEXT
                   AT END MOVE 1 TO EOF-FLAG
                   NOT AT END
                       DISPLAY "REL " WS-ROUT "=" RO-REC
               END-READ
           END-PERFORM
           CLOSE REL-OUT.
       SHOW-IDX.
           MOVE 0 TO EOF-FLAG
           OPEN INPUT IDX-OUT
           PERFORM UNTIL EOF-FLAG = 1
               READ IDX-OUT NEXT
                   AT END MOVE 1 TO EOF-FLAG
                   NOT AT END DISPLAY "IDX " IO-REC
               END-READ
           END-PERFORM
           CLOSE IDX-OUT.
    "#,
        idx_in = p("idx_in.dat"),
        rel_in = p("rel_in.dat"),
        seq_out = p("seq_out.dat"),
        rel_out = p("rel_out.dat"),
        idx_out = p("idx_out.dat"),
        wk = p("sortwork.dat"),
        rel_nine = p("rel_nine.dat"),
    );
    let t = std::time::Instant::now();
    let got = run_capture(&src);
    let want = vec![
        // An indexed file in, by descending name.
        "SEQ 30CCC",
        "SEQ 20BBB",
        "SEQ 10AAA",
        // A relative file in (slots 2, 5, 7) and out (slots 1, 2, 3).
        "REL-OUT KEY AFTER SORT 003",
        "REL 001=40DDD",
        "REL 002=50EEE",
        "REL 003=60FFF",
        // Both in, an indexed file out.
        "IDX 10AAA",
        "IDX 20BBB",
        "IDX 30CCC",
        "IDX 40DDD",
        "IDX 50EEE",
        "IDX 60FFF",
        // MERGE of the two indexed files into the relative file.
        "MERGED KEY 009",
        "REL 001=10AAA",
        "REL 002=10AAA",
        "REL 003=20BBB",
        "REL 004=20BBB",
        "REL 005=30CCC",
        "REL 006=30CCC",
        "REL 007=40DDD",
        "REL 008=50EEE",
        "REL 009=60FFF",
        // Slot 12 has two digits; the RELATIVE KEY item holds one.
        "SEQ 70GGG",
    ];
    let equal = got.iter().zip(&want).filter(|(g, w)| g == w).count();
    for i in 0..got.len().max(want.len()) {
        let (g, w) = (got.get(i).map(String::as_str), want.get(i).copied());
        let mark = if g == w { "ok " } else { "BAD" };
        eprintln!("  {mark} {g:?}{}", if g == w { String::new() } else { format!("  (want {w:?})") });
    }
    eprintln!("== sort_and_merge_use_relative_and_indexed_files ==");
    eprintln!(
        "cases: indexed USING, relative USING and GIVING (RELATIVE KEY left on the last record), both USING into indexed GIVING, MERGE of two indexed files into relative GIVING, a record number wider than the RELATIVE KEY ending the reading (14); lines {} of {} as expected; {:.0} ms",
        equal,
        want.len(),
        t.elapsed().as_secs_f64() * 1000.0
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(got, want);
}
