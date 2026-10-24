// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! In-memory INDEXED files and SORT in a browser module (WASM-010 T14, R7, Q8,
//! Q-new-1): status codes, key order, duplicates, START and SORT procedures
//! held to the interpreter's in-memory engine; memory full is FILE STATUS 89
//! with every record kept; the records go with the form instance.
//!
//! The interpreter refuses to write to a `STORAGE IS MEMORY` file without
//! `WITH PERSISTENCE` (37, a read-only copy, operator 2026-10-01), and a
//! browser module refuses `WITH PERSISTENCE` (it writes to disk). So the oracle
//! runs the same program with `WITH PERSISTENCE` and a fresh file in a
//! temporary directory: a file that lives from `OPEN` to `CLOSE` and back, as
//! the module's lives with its form instance.

mod common;

use cobolt_wasm::host::{run_program, FormHost};
use cobolt_wasm::module::{compile_project, FormSpec};
use cobolt_wasm::Options;

/// The program with `@STORAGE@` and `@PATH@` filled in for one engine.
fn variant(src: &str, persistent: bool, path: &str) -> String {
    let storage = if persistent {
        "STORAGE IS MEMORY WITH PERSISTENCE"
    } else {
        "STORAGE IS MEMORY"
    };
    src.replace("@STORAGE@", storage).replace("@PATH@", path)
}

/// Run on the module (both memory variants) and on the interpreter; the lines
/// must be equal.
fn same(name: &str, src: &str) -> Vec<String> {
    let dir = std::env::temp_dir().join(format!("wasm-memfiles-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("data.idx").to_string_lossy().replace('\\', "/");
    let want = common::run_interpreter(&variant(src, true, &path));
    let _ = std::fs::remove_dir_all(&dir);
    let module_src = variant(src, false, "data.idx");
    for multi in [true, false] {
        let o = common::run_wasm(
            &module_src,
            &Options {
                multi_memory: multi,
                ..Options::default()
            },
        );
        assert!(
            o.trapped.is_none() && o.errors.is_empty(),
            "{name} multi={multi}: trapped {:?} errors {:?}",
            o.trapped,
            o.errors
        );
        assert_eq!(
            common::wasm_lines(&o),
            want,
            "{name}: multi_memory={multi}\n{module_src}"
        );
    }
    assert!(!want.is_empty(), "{name}: printed nothing");
    want
}

const FILE_HEAD: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. MF.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n";

/// A program over `CUST`: a 5-digit id, a 10-character name (alternate key,
/// duplicates allowed) and a 5-character city.
fn customers(access: &str, optional: bool, procedure: &str) -> String {
    format!(
        "{FILE_HEAD}    SELECT {opt}CUST ASSIGN TO \"@PATH@\"\n        ORGANIZATION IS INDEXED\n        ACCESS MODE IS {access}\n\
         \x20       RECORD KEY IS C-ID\n        ALTERNATE RECORD KEY IS C-NAME WITH DUPLICATES\n        @STORAGE@\n        FILE STATUS IS FS.\n\
         DATA DIVISION.\nFILE SECTION.\nFD CUST.\n01 C-REC.\n    05 C-ID PIC 9(5).\n    05 C-NAME PIC X(10).\n    05 C-CITY PIC X(5).\n\
         WORKING-STORAGE SECTION.\n01 FS PIC XX.\n01 N PIC 99 VALUE 0.\n01 EOF PIC 9 VALUE 0.\n01 W-ID PIC 9(5).\n\
         PROCEDURE DIVISION.\n{procedure}    STOP RUN.\n",
        opt = if optional { "OPTIONAL " } else { "" }
    )
}

/// `WRITE` one record and show the status.
fn put(id: u32, name: &str, city: &str) -> String {
    format!(
        "    MOVE {id} TO C-ID MOVE \"{name}\" TO C-NAME MOVE \"{city}\" TO C-CITY\n    WRITE C-REC INVALID KEY DISPLAY \"DUP \" C-ID \" \" FS NOT INVALID KEY DISPLAY \"PUT \" C-ID \" \" FS END-WRITE\n"
    )
}

/// Read to the end and show every record.
const LIST: &str = "    MOVE 0 TO EOF\n    PERFORM UNTIL EOF = 1\n        READ CUST NEXT RECORD AT END MOVE 1 TO EOF DISPLAY \"END \" FS\n            NOT AT END DISPLAY C-ID \" \" C-NAME \" \" C-CITY\n        END-READ\n    END-PERFORM\n";

fn load() -> String {
    let mut s = String::from("    OPEN OUTPUT CUST\n    DISPLAY \"OPEN \" FS\n");
    for (id, name, city) in [
        (30, "ANA", "RIO"),
        (10, "BRUNO", "SAO"),
        (50, "ANA", "POA"),
        (20, "CARLA", "RIO"),
        (40, "ANA", "BH"),
    ] {
        s.push_str(&put(id, name, city));
    }
    s.push_str(&put(20, "DUPE", "X"));
    s.push_str("    CLOSE CUST\n    DISPLAY \"CLOSE \" FS\n");
    s
}

fn cases() -> Vec<(&'static str, String)> {
    let mut v = Vec::new();
    // Written out of order, read back in key order; a duplicate primary is 22.
    v.push((
        "write-and-list",
        customers(
            "DYNAMIC",
            false,
            &format!("{}    OPEN INPUT CUST\n{LIST}    CLOSE CUST\n", load()),
        ),
    ));
    // Random reads by the primary key, found and not found.
    v.push((
        "random-read",
        customers(
            "DYNAMIC",
            false,
            &format!(
                "{}    OPEN INPUT CUST\n    MOVE 40 TO C-ID\n    READ CUST INVALID KEY DISPLAY \"NO \" FS NOT INVALID KEY DISPLAY \"GOT \" C-NAME C-CITY \" \" FS END-READ\n\
                 \x20   MOVE 41 TO C-ID\n    READ CUST INVALID KEY DISPLAY \"NO \" FS NOT INVALID KEY DISPLAY \"GOT \" C-NAME END-READ\n    CLOSE CUST\n",
                load()
            ),
        ),
    ));
    // The alternate key: duplicates come back in the order they were written.
    v.push((
        "alternate-duplicates",
        customers(
            "DYNAMIC",
            false,
            &format!(
                "{}    OPEN INPUT CUST\n    MOVE \"ANA\" TO C-NAME\n    READ CUST KEY IS C-NAME INVALID KEY DISPLAY \"NO\" END-READ\n    DISPLAY \"FIRST ANA \" C-ID \" \" FS\n\
                 \x20   MOVE \"ANA\" TO C-NAME\n    START CUST KEY IS EQUAL TO C-NAME INVALID KEY DISPLAY \"NO START\" END-START\n{LIST}    CLOSE CUST\n",
                load()
            ),
        ),
    ));
    // START with every relation, on the primary key; generic (shorter) keys.
    let mut starts = load();
    starts.push_str("    OPEN INPUT CUST\n");
    for (rel, id) in [
        ("EQUAL TO", 20),
        ("GREATER THAN", 20),
        ("NOT LESS THAN", 25),
        ("LESS THAN", 25),
        ("NOT GREATER THAN", 30),
        ("GREATER THAN", 50),
        ("EQUAL TO", 21),
    ] {
        starts.push_str(&format!(
            "    MOVE {id} TO C-ID\n    START CUST KEY IS {rel} C-ID INVALID KEY DISPLAY \"START {rel} {id} INVALID \" FS\n\
             \x20       NOT INVALID KEY READ CUST NEXT RECORD AT END DISPLAY \"AT END\" NOT AT END DISPLAY \"START {rel} {id} -> \" C-ID END-READ\n    END-START\n"
        ));
    }
    starts.push_str("    CLOSE CUST\n");
    v.push(("start", customers("DYNAMIC", false, &starts)));
    // REWRITE moving an alternate value (the record joins its new set last),
    // DELETE, then the alternate order again.
    v.push((
        "rewrite-delete",
        customers(
            "DYNAMIC",
            false,
            &format!(
                "{}    OPEN I-O CUST\n    MOVE 30 TO C-ID\n    READ CUST\n    MOVE \"BRUNO\" TO C-NAME\n    REWRITE C-REC INVALID KEY DISPLAY \"RW BAD \" FS END-REWRITE\n    DISPLAY \"RW \" FS\n\
                 \x20   MOVE 10 TO C-ID\n    DELETE CUST INVALID KEY DISPLAY \"DEL BAD \" FS END-DELETE\n    DISPLAY \"DEL \" FS\n\
                 \x20   MOVE 99 TO C-ID\n    DELETE CUST INVALID KEY DISPLAY \"DEL 99 \" FS END-DELETE\n\
                 \x20   MOVE 77 TO C-ID\n    REWRITE C-REC INVALID KEY DISPLAY \"RW 77 \" FS END-REWRITE\n\
                 \x20   MOVE LOW-VALUES TO C-NAME\n    START CUST KEY IS NOT LESS THAN C-NAME END-START\n{LIST}    CLOSE CUST\n",
                load()
            ),
        ),
    ));
    // Sequential access: out-of-order WRITE is 21; REWRITE/DELETE need a READ
    // (43); reading past the end is 46.
    v.push((
        "sequential-rules",
        customers(
            "SEQUENTIAL",
            false,
            &format!(
                "    OPEN OUTPUT CUST\n{}{}{}    CLOSE CUST\n    OPEN I-O CUST\n    REWRITE C-REC\n    DISPLAY \"RW NO READ \" FS\n    DELETE CUST RECORD\n    DISPLAY \"DEL NO READ \" FS\n\
                 {LIST}    READ CUST NEXT RECORD AT END DISPLAY \"AGAIN AT END\" END-READ\n    DISPLAY \"AFTER END \" FS\n    CLOSE CUST\n",
                put(10, "A", "X"),
                put(30, "B", "Y"),
                put(20, "C", "Z")
            ),
        ),
    ));
    // OPEN and CLOSE statuses: 35 (never created), 41 (open twice), 42 (not
    // open), 48 (WRITE when INPUT), 47 (READ when not open), 49 (REWRITE when
    // INPUT), 38 (reopen after CLOSE WITH LOCK).
    v.push((
        "open-close-statuses",
        customers(
            "DYNAMIC",
            false,
            &format!(
                "    OPEN INPUT CUST\n    DISPLAY \"OPEN NEW \" FS\n    CLOSE CUST\n    DISPLAY \"CLOSE NOT OPEN \" FS\n    READ CUST NEXT RECORD END-READ\n    DISPLAY \"READ NOT OPEN \" FS\n\
                 {}    OPEN INPUT CUST\n    DISPLAY \"OPEN \" FS\n    OPEN INPUT CUST\n    DISPLAY \"OPEN AGAIN \" FS\n{}    REWRITE C-REC\n    DISPLAY \"REWRITE INPUT \" FS\n\
                 \x20   CLOSE CUST WITH LOCK\n    DISPLAY \"CLOSE LOCK \" FS\n    OPEN INPUT CUST\n    DISPLAY \"REOPEN \" FS\n",
                load(),
                put(60, "Z", "Z")
            ),
        ),
    ));
    // SELECT OPTIONAL: a file that is not there opens with 05 and is empty.
    v.push((
        "optional",
        customers(
            "DYNAMIC",
            true,
            &format!("    OPEN INPUT CUST\n    DISPLAY \"OPEN \" FS\n{LIST}    CLOSE CUST\n"),
        ),
    ));
    v
}

#[test]
fn in_memory_indexed_files_match_the_interpreter() {
    let cases = cases();
    let mut lines = 0;
    for (name, src) in &cases {
        let out = same(name, src);
        eprintln!("  {name:<22} {} lines: {}", out.len(), out.join(" | "));
        lines += out.len();
    }
    eprintln!("in-memory INDEXED: {} cases, {} cases equal on both engines and both memory variants ({lines} lines compared)", cases.len(), cases.len());
}

/// SORT with INPUT and OUTPUT PROCEDUREs: ascending and descending keys,
/// numeric and alphanumeric, and equal keys kept in the order released.
#[test]
fn sort_procedures_match_the_interpreter() {
    let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. SRT.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT WORK-FILE ASSIGN TO \"work.tmp\".\n\
               DATA DIVISION.\nFILE SECTION.\nSD WORK-FILE.\n01 W-REC.\n    05 W-CITY PIC X(5).\n    05 W-AMT PIC S9(5)V99.\n    05 W-SEQ PIC 99.\n\
               WORKING-STORAGE SECTION.\n01 K PIC 99.\n01 EOF PIC 9.\n01 T-DATA.\n    05 FILLER PIC X(14) VALUE \"RIO  0001050  \".\n    05 FILLER PIC X(14) VALUE \"SAO  0000990  \".\n\
               \x20   05 FILLER PIC X(14) VALUE \"RIO  0001050  \".\n    05 FILLER PIC X(14) VALUE \"BH   0012000  \".\n    05 FILLER PIC X(14) VALUE \"SAO  0000100  \".\n    05 FILLER PIC X(14) VALUE \"RIO  0000005  \".\n\
               01 T-ROWS REDEFINES T-DATA.\n    05 T-ROW OCCURS 6.\n        10 T-CITY PIC X(5).\n        10 T-AMT PIC 9(5)V99.\n        10 FILLER PIC XX.\n\
               PROCEDURE DIVISION.\nMAIN-PARA.\n    SORT WORK-FILE ON ASCENDING KEY W-CITY ON DESCENDING KEY W-AMT\n        INPUT PROCEDURE IS GIVE-RECORDS\n        OUTPUT PROCEDURE IS TAKE-RECORDS\n\
               \x20   SORT WORK-FILE ON DESCENDING KEY W-CITY\n        INPUT PROCEDURE IS GIVE-RECORDS\n        OUTPUT PROCEDURE IS TAKE-RECORDS\n    STOP RUN.\n\
               GIVE-RECORDS.\n    PERFORM VARYING K FROM 1 BY 1 UNTIL K > 6\n        MOVE T-CITY (K) TO W-CITY\n        MOVE T-AMT (K) TO W-AMT\n        MOVE K TO W-SEQ\n        RELEASE W-REC\n    END-PERFORM.\n\
               TAKE-RECORDS.\n    MOVE 0 TO EOF\n    PERFORM UNTIL EOF = 1\n        RETURN WORK-FILE AT END MOVE 1 TO EOF DISPLAY \"--\"\n            NOT AT END DISPLAY W-CITY \" \" W-AMT \" \" W-SEQ\n        END-RETURN\n    END-PERFORM.\n";
    let want = common::run_interpreter(src);
    for multi in [true, false] {
        let o = common::run_wasm(
            src,
            &Options {
                multi_memory: multi,
                ..Options::default()
            },
        );
        assert!(
            o.trapped.is_none() && o.errors.is_empty(),
            "multi={multi}: {:?} {:?}",
            o.trapped,
            o.errors
        );
        assert_eq!(common::wasm_lines(&o), want, "multi={multi}");
    }
    eprintln!(
        "sort: 2 sorts x 6 records, {} lines equal on both engines and both memory variants",
        want.len()
    );
}

/// Q-new-1: no record limit. A module whose file heap is small fills it; the
/// WRITE that does not fit ends with 89, and every record written before it is
/// read back.
#[test]
fn memory_full_is_status_89_and_every_record_is_kept() {
    let src = format!(
        "{FILE_HEAD}    SELECT BIG ASSIGN TO \"big\"\n        ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC\n        RECORD KEY IS B-ID\n        STORAGE IS MEMORY\n        FILE STATUS IS FS.\n\
         DATA DIVISION.\nFILE SECTION.\nFD BIG.\n01 B-REC.\n    05 B-ID PIC 9(6).\n    05 B-DATA PIC X(94).\n\
         WORKING-STORAGE SECTION.\n01 FS PIC XX.\n01 K PIC 9(6) VALUE 0.\n01 WRITTEN PIC 9(6) VALUE 0.\n01 READ-BACK PIC 9(6) VALUE 0.\n01 BAD PIC 9(6) VALUE 0.\n01 EOF PIC 9 VALUE 0.\n\
         PROCEDURE DIVISION.\n    OPEN OUTPUT BIG\n    PERFORM UNTIL FS NOT = \"00\" OR K > 999990\n        ADD 1 TO K\n        MOVE K TO B-ID\n        MOVE ALL \"X\" TO B-DATA\n        WRITE B-REC\n        IF FS = \"00\" ADD 1 TO WRITTEN END-IF\n    END-PERFORM\n\
         \x20   DISPLAY \"STATUS \" FS \" AFTER \" WRITTEN\n    CLOSE BIG\n    OPEN INPUT BIG\n    PERFORM UNTIL EOF = 1\n        READ BIG NEXT RECORD AT END MOVE 1 TO EOF\n            NOT AT END ADD 1 TO READ-BACK\n                IF B-ID NOT = READ-BACK ADD 1 TO BAD END-IF\n        END-READ\n    END-PERFORM\n\
         \x20   DISPLAY \"READ \" READ-BACK \" OUT OF ORDER \" BAD\n    CLOSE BIG\n    STOP RUN.\n"
    );
    for multi in [true, false] {
        let opts = Options {
            multi_memory: multi,
            file_heap: 64 * 1024,
            ..Options::default()
        };
        let c = common::compile(&src, &opts);
        let o = run_program(&c.wasm).unwrap();
        assert!(
            o.trapped.is_none() && o.errors.is_empty(),
            "multi={multi}: {:?} {:?}",
            o.trapped,
            o.errors
        );
        let lines = common::wasm_lines(&o);
        let written: u32 = lines[0].rsplit(' ').next().unwrap().parse().unwrap();
        assert!(lines[0].starts_with("STATUS 89 AFTER "), "{lines:?}");
        assert!(
            written > 100,
            "a 64 KiB heap holds more than 100 records of 100 bytes: {lines:?}"
        );
        assert_eq!(
            lines[1],
            format!("READ {written:06} OUT OF ORDER 000000"),
            "multi={multi}"
        );
        eprintln!("memory full (multi={multi}): 64 KiB heap, {written} records written, status 89 on the next, {written} read back in order");
    }
}

/// A file that is never in memory is refused at the verb's line, naming the
/// alternative; WITH PERSISTENCE is refused because it writes to disk.
#[test]
fn files_the_module_cannot_hold_are_refused() {
    let disk = customers("DYNAMIC", false, "    OPEN INPUT CUST\n")
        .replace("@STORAGE@", "STORAGE IS DISK")
        .replace("@PATH@", "x");
    let persistent = customers("DYNAMIC", false, "    OPEN INPUT CUST\n")
        .replace("@STORAGE@", "STORAGE IS MEMORY WITH PERSISTENCE")
        .replace("@PATH@", "x");
    let using = "IDENTIFICATION DIVISION.\nPROGRAM-ID. SU.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT W ASSIGN TO \"w\".\n    SELECT IN-F ASSIGN TO \"in\".\n\
                 DATA DIVISION.\nFILE SECTION.\nSD W.\n01 W-REC PIC X(5).\nFD IN-F.\n01 IN-REC PIC X(5).\nPROCEDURE DIVISION.\n    SORT W ON ASCENDING KEY W-REC USING IN-F GIVING IN-F.\n    STOP RUN.\n";
    for (src, words) in [
        (
            disk.as_str(),
            &["file CUST", "STORAGE IS MEMORY", "backend"][..],
        ),
        (persistent.as_str(), &["WITH PERSISTENCE", "disk"][..]),
        (using, &["USING", "INPUT PROCEDURE"][..]),
    ] {
        let prog = common::parse(src);
        let err = match cobolt_wasm::compile_program(&prog, &Options::default()) {
            Ok(_) => panic!("compiled:\n{src}"),
            Err(e) => e,
        };
        assert_eq!(err.kind, cobolt_wasm::ErrorKind::BackendOnly, "{err}");
        for w in words {
            assert!(err.message.contains(w), "{err} lacks {w:?}");
        }
    }
    eprintln!("refusals: 3 of 3 (disk file, WITH PERSISTENCE, SORT USING/GIVING)");
}

/// The records a form instance wrote are given back with the instance: the
/// debug count of its data is back to where it started once the form closes.
#[test]
fn the_records_go_with_the_form_instance() {
    let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. F1.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT T ASSIGN TO \"t\" ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC\n        RECORD KEY IS T-ID STORAGE IS MEMORY.\n\
               DATA DIVISION.\nFILE SECTION.\nFD T.\n01 T-REC.\n    05 T-ID PIC 9(4).\n    05 T-X PIC X(60).\n\
               WORKING-STORAGE SECTION.\n01 COBOL-QUIT PIC 9 VALUE 0.\n01 COBOL-EVENT-ID PIC X(64) VALUE SPACES.\n01 COBOL-CONTROL-ID PIC X(64) VALUE SPACES.\n01 FORM-NAME PIC X(64) VALUE 'F1'.\n01 K PIC 9(4) VALUE 0.\n\
               PROCEDURE DIVISION.\nCOBOL-MAIN.\n    COBOL::\"INIT-FORM\" ( FORM-NAME )\n    OPEN OUTPUT T\n    PERFORM UNTIL COBOL-QUIT = 1\n        COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )\n\
               \x20       IF COBOL-CONTROL-ID = \"BTN\"\n            PERFORM 50 TIMES\n                ADD 1 TO K\n                MOVE K TO T-ID\n                WRITE T-REC\n            END-PERFORM\n        END-IF\n    END-PERFORM\n    STOP RUN.\n";
    let prog = common::parse(src);
    for multi in [true, false] {
        let spec = FormSpec {
            name: "F1".into(),
            main: true,
            instances: 1,
            program: &prog,
            controls: vec![("BTN".into(), "Button".into())],
            bindings: vec![("BTN".into(), "onClick".into())],
            layers: vec![],
            lines: vec![],
        };
        let c = compile_project(
            std::slice::from_ref(&spec),
            &Options {
                multi_memory: multi,
                ..Options::default()
            },
        )
        .unwrap();
        let mut h = FormHost::new(&c.wasm).unwrap();
        let before = h.dbg_region_used();
        let (inst, _) = h.open_form(0, -1);
        assert!(inst >= 0, "open: {inst}");
        let open = h.dbg_region_used();
        assert_eq!(h.event(inst, 3, 0, &[]), 0, "{:?}", h.outcome());
        let full = h.dbg_region_used();
        assert!(
            full > open + 50 * 64,
            "fifty records live in the heap: {open} -> {full}"
        );
        h.close_form(inst);
        assert_eq!(
            h.dbg_region_used(),
            before,
            "multi={multi}: the instance's region and records are given back"
        );
        eprintln!("release (multi={multi}): {before} bytes before, {open} open, {full} with 50 records, {} after close", h.dbg_region_used());
    }
}
