// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Behavioral parity for the crash-safe **redb** INDEXED engine.
//!
//! The redb engine (`IndexedEngine::Redb`, opt-in via `--indexed-engine redb`)
//! must present *identical* observable COBOL behavior to the default disk engine.
//! These tests run the exact same versioned fixtures used for the PRCIDXD1 engine
//! (`tests/cobol/fileio/idx_*.cbl`) through the interpreter with the redb engine
//! selected, and assert the same DISPLAY output — CRUD with a primary + an
//! alternate key WITH DUPLICATES, persistence across a reopen, and COMMIT/ROLLBACK.
//!
//! It also exercises the `IndexedStore` surface directly for transaction
//! durability (`COMMIT` survives, `ROLLBACK` undoes) on the redb backend.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::indexed::{status, IndexedEngine, IndexedStore, KeySpec, OpenMode, ReadDir, StartOp};
use cobolt_runtime::indexed_redb::RedbIndexedFile;
use cobolt_runtime::Interpreter;

/// Run a program with the **redb** indexed engine selected, capturing trimmed
/// DISPLAY lines. The fixture's `/tmp/<file>` ASSIGN paths are redirected into a
/// unique temp dir.
fn run_fixture_redb(tag: &str, raw: &str) -> Vec<String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("prc-redb-{tag}-{nanos}"));
    std::fs::create_dir_all(&base).unwrap();
    let src = raw.replace("\"/tmp/", &format!("\"{}/", base.display()));

    let tokens = tokenize(&src, SourceFormat::Free);
    let result = parse(tokens);
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
    interp.set_indexed_engine(IndexedEngine::Redb);
    interp.run().expect("run failed");
    let out: Vec<String> = display_rx.try_iter().map(|l| l.trim_end().to_string()).collect();
    let _ = std::fs::remove_dir_all(&base);
    out
}

const IDX_CRUD: &str = include_str!("../../../tests/cobol/fileio/idx_crud.cbl");
const IDX_PERSIST: &str = include_str!("../../../tests/cobol/fileio/idx_persist.cbl");
const IDX_TX: &str = include_str!("../../../tests/cobol/fileio/idx_tx.cbl");

#[test]
fn redb_crud_matches_disk_fixture() {
    let out = run_fixture_redb("crud", IDX_CRUD);
    assert_eq!(
        out,
        vec![
            "RANDOM 0002 BRUNO",
            "MISSING ST 23",
            "SEQ 0001 ANA",
            "SEQ 0002 BRUNINHO",
            "SEQ 0003 CARLOS",
            "ALT 1 CARLOS",
            "ALT 2 BRUNINHO",
            "DELETE ST 00",
            "DELETED ST 23",
        ]
    );
}

#[test]
fn redb_persistence_across_reopen() {
    let out = run_fixture_redb("persist", IDX_PERSIST);
    assert_eq!(
        out,
        vec![
            "REC 0010 DEZ",
            "REC 0020 VINTE",
            "REC 0030 TRINTA",
            "TOTAL 03",
        ]
    );
}

#[test]
fn redb_commit_rollback() {
    let out = run_fixture_redb("tx", IDX_TX);
    assert_eq!(out, vec!["TX 0001 ALPHA", "TX 0002 BETA", "TX 0003 GAMMA"]);
}

// ── Direct IndexedStore-level checks ────────────────────────────────────────

fn tmp_path(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("prc-redb-unit-{tag}-{nanos}.rdb"))
}

/// `rec(id, name)` = a 9-byte record: 4-digit id + 5-char name.
fn rec(id: &str, name: &str) -> Vec<u8> {
    format!("{id:0>4}{name:<5}").into_bytes()
}

#[test]
fn store_crud_and_alternate_duplicates() {
    let path = tmp_path("crud");
    let primary = KeySpec { offset: 0, len: 4, duplicates: false };
    let city = KeySpec { offset: 4, len: 5, duplicates: true }; // alt WITH DUPLICATES
    let mut f = RedbIndexedFile::new(&path, 9, primary, vec![city]);

    assert_eq!(f.open(OpenMode::Output), status::OK);
    assert_eq!(f.write(&rec("3", "SP")), status::OK);
    assert_eq!(f.write(&rec("1", "RIO")), status::OK);
    assert_eq!(f.write(&rec("2", "SP")), status::OK); // duplicate alt = still 00
    assert_eq!(f.close(), status::OK);

    // Reopen and read back in primary order.
    assert_eq!(f.open(OpenMode::Io), status::OK);
    let (r, st) = f.read_key(b"0002");
    assert_eq!(st, status::OK);
    assert_eq!(r.unwrap(), rec("2", "SP"));

    // Sequential scan by primary key.
    f.start(StartOp::Gt, b"0000");
    let mut ids = Vec::new();
    loop {
        let (r, st) = f.read_seq(ReadDir::Next);
        if st == status::EOF {
            break;
        }
        ids.push(String::from_utf8_lossy(&r.unwrap()[..4]).to_string());
    }
    assert_eq!(ids, vec!["0001", "0002", "0003"]);

    // Alternate key "SP" has duplicates → first is the one written first (0003),
    // i.e. insertion order, matching the disk engine's RecordId ordering.
    f.set_key_of_reference(1);
    let (r, st) = f.read_key(b"SP");
    assert_eq!(st, status::OK);
    assert_eq!(&r.unwrap()[..4], b"0003");

    // Delete and confirm gone.
    f.set_key_of_reference(0);
    let (_r, _st) = f.read_key(b"0001");
    assert_eq!(f.delete(None), status::OK);
    let (_r, st) = f.read_key(b"0001");
    assert_eq!(st, status::NOT_FOUND);
    assert_eq!(f.close(), status::OK);
}

#[test]
fn store_commit_survives_rollback_undoes() {
    let path = tmp_path("tx");
    let primary = KeySpec { offset: 0, len: 4, duplicates: false };
    let mut f = RedbIndexedFile::new(&path, 9, primary, vec![]);

    assert_eq!(f.open(OpenMode::Io), status::OK);
    assert_eq!(f.write(&rec("1", "ALPHA")), status::OK);
    assert_eq!(f.write(&rec("2", "BETA")), status::OK);
    f.commit(); // ALPHA + BETA durable

    assert_eq!(f.write(&rec("3", "GAMMA")), status::OK); // uncommitted
    f.rollback(); // undo GAMMA

    // Scan: only the committed ALPHA + BETA remain.
    f.start(StartOp::Gt, b"0000");
    let mut names = Vec::new();
    loop {
        let (r, st) = f.read_seq(ReadDir::Next);
        if st == status::EOF {
            break;
        }
        let r = r.unwrap();
        names.push(String::from_utf8_lossy(&r[4..]).trim_end().to_string());
    }
    assert_eq!(names, vec!["ALPHA", "BETA"]);
    assert_eq!(f.close(), status::OK);
}

/// Scale / instant-OPEN smoke test. Ignored by default (writes N records).
/// Run: `cargo test -p cobolt-runtime --test test_indexed_redb -- --ignored --nocapture scale`.
#[test]
#[ignore = "scale smoke test; run explicitly"]
fn scale_open_is_instant_and_reads_are_fast() {
    use std::time::Instant;
    let n: u32 = std::env::var("PRC_SCALE_N").ok().and_then(|s| s.parse().ok()).unwrap_or(200_000);
    let path = tmp_path("scale");
    let primary = KeySpec { offset: 0, len: 9, duplicates: false };
    let mut f = RedbIndexedFile::new(&path, 14, primary, vec![]);

    let t0 = Instant::now();
    assert_eq!(f.open(OpenMode::Output), status::OK);
    for i in 0..n {
        // 9-digit id + 5-char name.
        let r = format!("{i:09}{:<5}", "X");
        assert_eq!(f.write(r.as_bytes()), status::OK);
    }
    assert_eq!(f.close(), status::OK);
    let load = t0.elapsed();

    // OPEN must be O(1): time it on the now-large file.
    let t1 = Instant::now();
    assert_eq!(f.open(OpenMode::Input), status::OK);
    let open = t1.elapsed();

    // Random reads across the keyspace.
    let t2 = Instant::now();
    for i in (0..n).step_by((n / 1000).max(1) as usize) {
        let key = format!("{i:09}");
        let (r, st) = f.read_key(key.as_bytes());
        assert_eq!(st, status::OK);
        assert_eq!(&r.unwrap()[..9], key.as_bytes());
    }
    let reads = t2.elapsed();

    // Sequential READ NEXT scan of the whole file (exercises opt 1: one descent
    // per record instead of two).
    let t3 = Instant::now();
    assert_eq!(f.start(StartOp::Gt, b"\x00"), status::OK);
    let mut scanned = 0u32;
    loop {
        let (r, st) = f.read_seq(ReadDir::Next);
        if st == status::EOF {
            break;
        }
        assert!(r.is_some());
        scanned += 1;
    }
    assert_eq!(scanned, n);
    let scan = t3.elapsed();
    assert_eq!(f.close(), status::OK);
    let _ = std::fs::remove_file(&path);

    eprintln!(
        "redb scale: n={n}  load={load:?}  OPEN={open:?}  1000 random reads={reads:?}  \
         READ NEXT x{n}={scan:?} ({:.2}us/rec)",
        scan.as_micros() as f64 / n as f64
    );
    // OPEN must not scale with record count — generously bounded.
    assert!(open.as_millis() < 250, "OPEN was not instant: {open:?}");
}

#[test]
fn open_input_missing_file_is_35() {
    let path = tmp_path("missing");
    let primary = KeySpec { offset: 0, len: 4, duplicates: false };
    let mut f = RedbIndexedFile::new(&path, 9, primary, vec![]);
    assert_eq!(f.open(OpenMode::Input), status::FILE_NOT_FOUND);
}

#[test]
fn observability_log_records_transactions() {
    use cobolt_runtime::indexed_log::LogLevel;
    let path = tmp_path("log");
    let log_path = {
        let mut os = path.as_os_str().to_owned();
        os.push(".log");
        std::path::PathBuf::from(os)
    };
    let primary = KeySpec { offset: 0, len: 4, duplicates: false };
    let mut f = RedbIndexedFile::new(&path, 9, primary, vec![]);
    f.set_log_level(LogLevel::Full);

    assert_eq!(f.open(OpenMode::Io), status::OK);
    assert_eq!(f.write(&rec("1", "A")), status::OK);
    assert_eq!(f.write(&rec("2", "B")), status::OK); // two ordered writes
    f.commit();
    assert_eq!(f.write(&rec("3", "C")), status::OK); // one write, then undone
    f.rollback();
    assert_eq!(f.close(), status::OK);

    let log = std::fs::read_to_string(&log_path).expect("log file written");
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&log_path);

    // One line per transaction event.
    assert!(log.contains("kind=OPEN"), "missing OPEN:\n{log}");
    assert!(log.contains("kind=ROLLBACK"), "missing ROLLBACK:\n{log}");
    // The COMMIT recorded two writes, in ascending key order.
    assert!(
        log.lines().any(|l| l.contains("kind=COMMIT")
            && l.contains("writes=2")
            && l.contains("order=ordered")
            && l.contains("out_of_order=0")),
        "COMMIT line wrong:\n{log}"
    );
    // The ROLLBACK recorded the single (undone) write.
    assert!(
        log.lines().any(|l| l.contains("kind=ROLLBACK") && l.contains("writes=1")),
        "ROLLBACK line wrong:\n{log}"
    );
    // The full-level CLOSE line carries redb index statistics.
    assert!(
        log.lines().any(|l| l.contains("kind=CLOSE")
            && l.contains("tree_height=")
            && l.contains("leaf_pages=")
            && l.contains("allocated_pages=")),
        "CLOSE stats missing:\n{log}"
    );
    // Every line is timestamped and names the file.
    assert!(log.lines().all(|l| l.starts_with("ts=") && l.contains("file=")));
}

#[test]
fn open_with_registered_user_appears_in_log() {
    use cobolt_runtime::indexed_log::LogLevel;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("prc-redb-reguser-{nanos}"));
    std::fs::create_dir_all(&base).unwrap();

    let raw = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. REGUSER.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO "/tmp/reguser.dat"
               ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC
               RECORD KEY IS R-COD STORAGE IS DISK FILE STATUS IS WS-ST.
       DATA DIVISION.
       FILE SECTION.
       FD F.
       01 R-REC.
          05 R-COD  PIC 9(4).
          05 R-NOME PIC X(8).
       WORKING-STORAGE SECTION.
       01 WS-ST   PIC XX.
       01 WS-USER PIC X(12) VALUE "BOB-FROM-WS".
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F WITH REGISTERED USER "ALICE"
           MOVE 0001 TO R-COD MOVE "ANA" TO R-NOME WRITE R-REC
           CLOSE F
           OPEN I-O F WITH REGISTERED USER WS-USER
           MOVE 0002 TO R-COD MOVE "BRUNO" TO R-NOME WRITE R-REC
           CLOSE F
           STOP RUN.
    "#;
    let src = raw.replace("\"/tmp/", &format!("\"{}/", base.display()));

    let tokens = tokenize(&src, SourceFormat::Free);
    let result = parse(tokens);
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_e, erx) = mpsc::channel();
    let (stx, _sr) = mpsc::channel();
    let (dtx, _dr) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, erx, stx, dtx);
    interp.set_indexed_engine(IndexedEngine::Redb);
    interp.set_indexed_log_level(LogLevel::Basic);
    interp.run().expect("run failed");

    let log = std::fs::read_to_string(base.join("reguser.dat.log")).expect("log written");
    let _ = std::fs::remove_dir_all(&base);

    // The string-literal user tags the first session…
    assert!(
        log.lines().any(|l| l.contains("user=ALICE") && l.contains("kind=OPEN")),
        "missing literal user:\n{log}"
    );
    // …and the data-item user tags the second.
    assert!(
        log.lines().any(|l| l.contains("user=BOB-FROM-WS") && l.contains("kind=CLOSE")),
        "missing data-item user:\n{log}"
    );
    // Every event line in a session carries the user.
    assert!(log.lines().all(|l| l.contains("user=ALICE") || l.contains("user=BOB-FROM-WS")));
}

#[test]
fn observability_log_json_format() {
    use cobolt_runtime::indexed_log::{LogFormat, LogLevel};
    let path = tmp_path("logjson");
    let log_path = {
        let mut os = path.as_os_str().to_owned();
        os.push(".log");
        std::path::PathBuf::from(os)
    };
    let primary = KeySpec { offset: 0, len: 4, duplicates: false };
    let mut f = RedbIndexedFile::new(&path, 9, primary, vec![]);
    f.set_log_level(LogLevel::Basic);
    f.set_log_format(LogFormat::Json);

    assert_eq!(f.open(OpenMode::Io), status::OK);
    assert_eq!(f.write(&rec("1", "A")), status::OK);
    assert_eq!(f.write(&rec("2", "B")), status::OK);
    f.commit();
    assert_eq!(f.close(), status::OK);

    let log = std::fs::read_to_string(&log_path).expect("json log written");
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&log_path);

    // Every line is a JSON object (NDJSON).
    for l in log.lines() {
        assert!(l.starts_with('{') && l.ends_with('}'), "not a JSON object: {l}");
        assert!(l.contains(r#""ts":""#) && l.contains(r#""kind":""#), "missing fields: {l}");
    }
    // Numeric fields are bare numbers (graphable in Grafana), strings are quoted.
    assert!(
        log.lines().any(|l| l.contains(r#""kind":"COMMIT""#)
            && l.contains(r#""writes":2"#)
            && l.contains(r#""order":"ordered""#)),
        "COMMIT json wrong:\n{log}"
    );
}
