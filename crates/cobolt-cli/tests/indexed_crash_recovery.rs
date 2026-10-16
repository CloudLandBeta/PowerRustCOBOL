// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 001 (AC1, AC2, AC4, AC5b, AC6): **a real crash leaves an INDEXED file at its last
//! `COMMIT`.**
//!
//! Every test starts a real `rcrun` process on a COBOL program that opens a `STORAGE IS DISK`
//! file, commits some records, changes more without committing, and then waits for input. The
//! test reads the program's own `READY` line, kills the process with `SIGKILL` — no `CLOSE`, no
//! destructor, nothing flushed that the program did not flush itself — and runs a second
//! process that opens the same file and reports what it holds. Nothing is asserted about
//! recovery without the kill having been performed.
//!
//! The same program runs on both engines a `STORAGE IS DISK` file can use (`rust`, the
//! `PRCIDXD1` container, and `redb`); the second is the reference — it is crash-safe by its own
//! design — so a difference between them is a defect of the first.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let d = std::env::temp_dir().join(format!("prc-001-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Dir(d)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const ENGINES: [&str; 2] = ["rust", "redb"];

/// The environment of the program: a file of 8-character names under a 5-digit key, 1020 bytes a
/// record — four to a page — so that a few thousand of them fill hundreds of pages.
const HEAD: &str = "       IDENTIFICATION DIVISION.
       PROGRAM-ID. CRASH.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO \"crash.idx\"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS R-COD
               STORAGE IS DISK
               FILE STATUS IS WS-ST.
       DATA DIVISION.
       FILE SECTION.
       FD F.
       01 R-REC.
          05 R-COD  PIC 9(5).
          05 R-NOME PIC X(8).
          05 R-FILL PIC X(1007).
       WORKING-STORAGE SECTION.
       01 WS-ST   PIC XX.
       01 WS-EOF  PIC 9 VALUE 0.
       01 WS-I    PIC 9(5).
       01 WS-N    PIC 9(5) VALUE 0.
       01 WS-HOLD PIC X(10).
       PROCEDURE DIVISION.
       MAIN.
";

fn program(body: &str) -> String {
    format!("{HEAD}{body}           STOP RUN.\n")
}

/// Creates the file: records 1 and 2, closed cleanly. Both engines build their container here.
const SEED: &str = "           OPEN OUTPUT F
           MOVE 1 TO R-COD MOVE \"ALPHA\" TO R-NOME WRITE R-REC
           MOVE 2 TO R-COD MOVE \"BETA\" TO R-NOME WRITE R-REC
           CLOSE F
           DISPLAY \"SEEDED \" WS-ST
";

/// What the reader reports: every record in key order, then a probe for each key the writers touch.
const READER: &str = "           OPEN I-O F
           DISPLAY \"OPEN \" WS-ST
           MOVE 0 TO R-COD
           START F KEY IS GREATER THAN R-COD END-START
           PERFORM UNTIL WS-EOF = 1
               READ F NEXT
                   AT END MOVE 1 TO WS-EOF
                   NOT AT END
                       ADD 1 TO WS-N
                       IF WS-N < 12
                           DISPLAY \"REC \" R-COD \" \" R-NOME
                       END-IF
               END-READ
               IF WS-ST NOT = \"00\" AND WS-ST NOT = \"10\"
                   MOVE 1 TO WS-EOF
               END-IF
           END-PERFORM
           DISPLAY \"COUNT \" WS-N
           CLOSE F
           DISPLAY \"CLOSE \" WS-ST
";

/// Run a program to its end and answer its output lines.
fn run(dir: &Dir, engine: &str, name: &str, src: &str) -> Vec<String> {
    let file = dir.path(name);
    std::fs::write(&file, src).unwrap();
    let out = Command::new(rcrun())
        .args(["run", "--indexed-engine", engine, file.to_str().unwrap()])
        .current_dir(&dir.0)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "{name} on {engine} failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    stdout.lines().map(|l| l.trim_end().to_owned()).collect()
}

/// Start a program that prints `READY` when it has done its work and then waits on stdin; kill it
/// the moment the line arrives. Answers how long the kill took to be possible, in milliseconds.
fn crash(dir: &Dir, engine: &str, name: &str, src: &str) -> u128 {
    let file = dir.path(name);
    std::fs::write(&file, src).unwrap();
    let started = Instant::now();
    let mut child: Child = Command::new(rcrun())
        .args(["run", "--indexed-engine", engine, file.to_str().unwrap()])
        .current_dir(&dir.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap(); // held open: the program is blocked in its ACCEPT
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut seen = Vec::new();
    loop {
        assert!(Instant::now() < deadline, "{name} on {engine} never said READY; saw {seen:?}");
        match lines.next() {
            Some(Ok(l)) if l.trim() == "READY" => break,
            Some(Ok(l)) => seen.push(l),
            _ => panic!("{name} on {engine} ended before READY; saw {seen:?}"),
        }
    }
    let took = started.elapsed().as_millis();
    child.kill().unwrap(); // SIGKILL
    let _ = child.wait();
    drop(stdin);
    took
}

/// The records a reader printed, `(key, name)`, and the total it counted.
fn held(out: &[String]) -> (Vec<(u32, String)>, u32) {
    let recs = out
        .iter()
        .filter_map(|l| l.strip_prefix("REC "))
        .map(|l| {
            let (k, n) = l.split_once(' ').unwrap();
            (k.parse().unwrap(), n.trim().to_owned())
        })
        .collect();
    let count = out.iter().find_map(|l| l.strip_prefix("COUNT ")).and_then(|c| c.trim().parse().ok()).expect("a COUNT line");
    (recs, count)
}

fn names(recs: &[(u32, String)]) -> Vec<(u32, &str)> {
    recs.iter().map(|(k, n)| (*k, n.as_str())).collect()
}

fn status_of(out: &[String], what: &str) -> String {
    out.iter().find_map(|l| l.strip_prefix(what)).map(|s| s.trim().to_owned()).unwrap_or_else(|| panic!("no `{what}` line in {out:?}"))
}

/// AC1, AC4, AC6 — what was committed is there after the kill, what was not is not, and the keys
/// still walk in order. The uncommitted work is a `WRITE`, a `REWRITE` and a `DELETE`: one of each
/// verb, so a half-applied transaction would show.
#[test]
fn a_kill_leaves_the_file_at_its_last_commit() {
    let mut rows = Vec::new();
    for engine in ENGINES {
        let dir = Dir::new(&format!("commit-{engine}"));
        run(&dir, engine, "seed.cbl", &program(SEED));
        let took = crash(
            &dir,
            engine,
            "writer.cbl",
            &program(
                "           OPEN I-O F
           MOVE 3 TO R-COD MOVE \"GAMMA\" TO R-NOME WRITE R-REC
           COMMIT
           MOVE 4 TO R-COD MOVE \"DELTA\" TO R-NOME WRITE R-REC
           MOVE 1 TO R-COD READ F END-READ
           MOVE \"CHANGED\" TO R-NOME REWRITE R-REC
           MOVE 2 TO R-COD DELETE F
           DISPLAY \"READY\"
           ACCEPT WS-HOLD
",
            ),
        );
        let out = run(&dir, engine, "reader.cbl", &program(READER));
        let (recs, count) = held(&out);
        assert_eq!(status_of(&out, "OPEN "), "00", "{engine}: {out:?}");
        assert_eq!(names(&recs), [(1, "ALPHA"), (2, "BETA"), (3, "GAMMA")], "{engine}: {out:?}");
        assert_eq!(count, 3, "{engine}");
        assert_eq!(status_of(&out, "CLOSE "), "00", "{engine}");
        rows.push(format!("{engine}: killed after {took} ms; reopened 00; 3 records, keys 1 2 3 in order, 4 absent, 1 unchanged, 2 present"));
    }
    println!("\n  001 AC1/AC4/AC6 — a SIGKILL between COMMIT and the next COMMIT\n    {}", rows.join("\n    "));
}

/// The transaction the program abandoned with `ROLLBACK` leaves the file in the same state as the
/// crash did (AC2) — the two ways of not committing agree.
#[test]
fn a_rollback_and_a_kill_leave_the_same_file() {
    let rolled = {
        let dir = Dir::new("rollback-rust");
        run(&dir, "rust", "seed.cbl", &program(SEED));
        run(
            &dir,
            "rust",
            "writer.cbl",
            &program(
                "           OPEN I-O F
           MOVE 3 TO R-COD MOVE \"GAMMA\" TO R-NOME WRITE R-REC
           COMMIT
           MOVE 4 TO R-COD MOVE \"DELTA\" TO R-NOME WRITE R-REC
           MOVE 1 TO R-COD READ F END-READ
           MOVE \"CHANGED\" TO R-NOME REWRITE R-REC
           MOVE 2 TO R-COD DELETE F
           ROLLBACK
           CLOSE F
",
            ),
        );
        held(&run(&dir, "rust", "reader.cbl", &program(READER)))
    };
    let killed = {
        let dir = Dir::new("kill-rust");
        run(&dir, "rust", "seed.cbl", &program(SEED));
        crash(
            &dir,
            "rust",
            "writer.cbl",
            &program(
                "           OPEN I-O F
           MOVE 3 TO R-COD MOVE \"GAMMA\" TO R-NOME WRITE R-REC
           COMMIT
           MOVE 4 TO R-COD MOVE \"DELTA\" TO R-NOME WRITE R-REC
           MOVE 1 TO R-COD READ F END-READ
           MOVE \"CHANGED\" TO R-NOME REWRITE R-REC
           MOVE 2 TO R-COD DELETE F
           DISPLAY \"READY\"
           ACCEPT WS-HOLD
",
            ),
        );
        held(&run(&dir, "rust", "reader.cbl", &program(READER)))
    };
    assert_eq!(rolled, killed, "a ROLLBACK and a crash leave the same records");
    println!("\n  001 AC2 — ROLLBACK and SIGKILL give the same {} records", killed.1);
}

/// A transaction too big for the engine's write set — 2500 records of 1020 bytes, about 630 pages,
/// against a checkpoint every 256 — that is still not committed when the process is killed. The
/// committed records are the only ones that may be there.
#[test]
fn a_big_uncommitted_transaction_does_not_survive_a_kill() {
    let mut rows = Vec::new();
    for engine in ENGINES {
        let dir = Dir::new(&format!("big-{engine}"));
        run(&dir, engine, "seed.cbl", &program(SEED));
        let took = crash(
            &dir,
            engine,
            "writer.cbl",
            &program(
                "           OPEN I-O F
           MOVE 3 TO R-COD MOVE \"GAMMA\" TO R-NOME WRITE R-REC
           COMMIT
           PERFORM VARYING WS-I FROM 100 BY 1 UNTIL WS-I > 2599
               MOVE WS-I TO R-COD
               MOVE \"BULK\" TO R-NOME
               MOVE ALL \"X\" TO R-FILL
               WRITE R-REC
           END-PERFORM
           DISPLAY \"READY\"
           ACCEPT WS-HOLD
",
            ),
        );
        let out = run(&dir, engine, "reader.cbl", &program(READER));
        let (recs, count) = held(&out);
        assert_eq!(status_of(&out, "OPEN "), "00", "{engine}: {out:?}");
        assert_eq!(names(&recs), [(1, "ALPHA"), (2, "BETA"), (3, "GAMMA")], "{engine}: {out:?}");
        assert_eq!(count, 3, "{engine}: the 2500 uncommitted records are not there");
        rows.push(format!("{engine}: 2500 uncommitted WRITEs, killed after {took} ms; reopened with {count} records"));
    }
    println!("\n  001 R1/R3 — an uncommitted transaction larger than one checkpoint\n    {}", rows.join("\n    "));
}

/// The file the kill left is complete: a writer can carry on and commit, and what it added is read
/// back with what survived. Nothing is held over from the crashed run.
#[test]
fn a_file_after_a_kill_takes_new_work_and_commits_it() {
    for engine in ENGINES {
        let dir = Dir::new(&format!("resume-{engine}"));
        run(&dir, engine, "seed.cbl", &program(SEED));
        crash(
            &dir,
            engine,
            "writer.cbl",
            &program(
                "           OPEN I-O F
           MOVE 7 TO R-COD MOVE \"LOST\" TO R-NOME WRITE R-REC
           DISPLAY \"READY\"
           ACCEPT WS-HOLD
",
            ),
        );
        run(
            &dir,
            engine,
            "more.cbl",
            &program(
                "           OPEN I-O F
           MOVE 7 TO R-COD MOVE \"KEPT\" TO R-NOME WRITE R-REC
           DISPLAY \"WRITE \" WS-ST
           COMMIT
           CLOSE F
",
            ),
        );
        let out = run(&dir, engine, "reader.cbl", &program(READER));
        assert_eq!(names(&held(&out).0), [(1, "ALPHA"), (2, "BETA"), (7, "KEPT")], "{engine}: {out:?}");
    }
}

/// AC7 — a recovery journal that has been cut below what it certified is refused: the `OPEN`
/// answers `90`, and neither the container nor the journal is touched. Put back, the same `OPEN`
/// recovers. Only the default engine has a journal to damage.
#[test]
fn a_damaged_recovery_journal_fails_the_open_with_90_and_touches_nothing() {
    let dir = Dir::new("journal-cut");
    run(&dir, "rust", "seed.cbl", &program(SEED));
    crash(
        &dir,
        "rust",
        "writer.cbl",
        &program(
            "           OPEN I-O F
           PERFORM VARYING WS-I FROM 100 BY 1 UNTIL WS-I > 2599
               MOVE WS-I TO R-COD
               MOVE \"BULK\" TO R-NOME
               MOVE ALL \"X\" TO R-FILL
               WRITE R-REC
           END-PERFORM
           DISPLAY \"READY\"
           ACCEPT WS-HOLD
",
        ),
    );
    let journal = dir.path("crash.idx.jrn");
    let whole = std::fs::read(&journal).expect("a killed transaction that spilled leaves its journal");
    let container = std::fs::read(dir.path("crash.idx")).unwrap();
    assert!(whole.len() > 72 + 3 * 4108, "the journal holds the pages the spills changed ({} bytes)", whole.len());

    // Cut in the middle of the second page image: fewer valid images than the journal certified.
    std::fs::write(&journal, &whole[..72 + 4108 + 2000]).unwrap();
    let out = run(&dir, "rust", "reader.cbl", &program(READER));
    assert_eq!(status_of(&out, "OPEN "), "90", "{out:?}");
    assert_eq!(std::fs::read(dir.path("crash.idx")).unwrap(), container, "the container was not touched");
    assert_eq!(std::fs::read(&journal).unwrap(), &whole[..72 + 4108 + 2000], "and neither was the journal");

    // Put back, the same open recovers the file to its last commit.
    std::fs::write(&journal, &whole).unwrap();
    let out = run(&dir, "rust", "reader.cbl", &program(READER));
    assert_eq!(status_of(&out, "OPEN "), "00", "{out:?}");
    assert_eq!(names(&held(&out).0), [(1, "ALPHA"), (2, "BETA")], "{out:?}");
    assert!(!journal.exists(), "recovery removed the journal it replayed");
    println!(
        "\n  001 AC7 — journal of {} bytes cut to {}: OPEN 90, both files byte-identical; restored: OPEN 00, 2 records",
        whole.len(),
        72 + 4108 + 2000
    );
}

/// AC9 — a `STORAGE IS MEMORY WITH PERSISTENCE` file is written at `CLOSE` only. A run that changes
/// it and is killed before closing leaves the snapshot of the last `CLOSE` on disk: the next run
/// finds that, and not the work of the run that died.
#[test]
fn a_memory_file_with_persistence_reloads_its_last_close_after_a_kill() {
    let dir = Dir::new("memory-persist");
    let memory = |body: &str| {
        program(body).replace("STORAGE IS DISK", "STORAGE IS MEMORY WITH PERSISTENCE")
    };
    // The snapshot: records 1 and 2, written by a run that closes.
    run(&dir, "rust", "seed.cbl", &memory(&format!("{SEED}")));
    // A run that adds and changes records and is killed without closing.
    crash(
        &dir,
        "rust",
        "writer.cbl",
        &memory(
            "           OPEN I-O F
           MOVE 3 TO R-COD MOVE \"GAMMA\" TO R-NOME WRITE R-REC
           MOVE 1 TO R-COD READ F END-READ
           MOVE \"CHANGED\" TO R-NOME REWRITE R-REC
           COMMIT
           DISPLAY \"READY\"
           ACCEPT WS-HOLD
",
        ),
    );
    let out = run(&dir, "rust", "reader.cbl", &memory(READER));
    assert_eq!(names(&held(&out).0), [(1, "ALPHA"), (2, "BETA")], "{out:?}");
}
