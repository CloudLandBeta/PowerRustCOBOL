// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! End-to-end examples for the INDEXED file engine, exercised as versioned
//! fixtures under `tests/cobol/fileio/`:
//!
//!   * `idx_crud.cbl`    — CRUD over a DISK (compressed) INDEXED file with a
//!                         primary key and an alternate key WITH DUPLICATES:
//!                         random READ, missing-key INVALID KEY, REWRITE,
//!                         sequential START/READ NEXT, alternate-key READ, and
//!                         DELETE.
//!   * `idx_persist.cbl` — persistence: WRITE, CLOSE, reopen INPUT, and read the
//!                         records back across a file open boundary.
//!   * `idx_tx.cbl`      — COMMIT/ROLLBACK: a committed record survives while a
//!                         later WRITE/REWRITE/DELETE batch is undone by ROLLBACK.
//!
//! The `idx_tx` transaction fixture is run on both the DISK and MEMORY engines
//! (the latter via a `STORAGE IS DISK` → `STORAGE IS MEMORY` substitution) to
//! prove COMMIT/ROLLBACK behave identically on each backend.
//!
//! As in `test_fileio_storage.rs`, the on-disk `/tmp/<file>` ASSIGN paths are
//! redirected into a unique temp directory so parallel test threads can't
//! collide, and DISPLAY output is compared line-by-line (trailing PIC X fill
//! trimmed).

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Run a program source, returning captured DISPLAY lines with trailing
/// whitespace (PIC X fill) removed from each line.
fn run_capture(src: &str) -> Vec<String> {
    let tokens = tokenize(src, SourceFormat::Free);
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
    interp.run().expect("run failed");
    display_rx
        .try_iter()
        .map(|l| l.trim_end().to_string())
        .collect()
}

/// Redirect the fixture's `/tmp/<file>` ASSIGN paths into a unique temp dir,
/// run it, and return the trimmed DISPLAY lines.
fn run_fixture(tag: &str, raw: &str) -> Vec<String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("prc-idx-{tag}-{nanos}"));
    std::fs::create_dir_all(&base).unwrap();
    let src = raw.replace("\"/tmp/", &format!("\"{}/", base.display()));
    let out = run_capture(&src);
    let _ = std::fs::remove_dir_all(&base);
    out
}

const IDX_CRUD: &str = include_str!("../../../tests/cobol/fileio/idx_crud.cbl");
const IDX_PERSIST: &str = include_str!("../../../tests/cobol/fileio/idx_persist.cbl");
const IDX_TX: &str = include_str!("../../../tests/cobol/fileio/idx_tx.cbl");

#[test]
fn idx_crud_disk_compressed() {
    let out = run_fixture("crud", IDX_CRUD);
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
fn idx_persist_reopen() {
    let out = run_fixture("persist", IDX_PERSIST);
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
fn idx_tx_commit_rollback_disk() {
    let out = run_fixture("tx-disk", IDX_TX);
    assert_eq!(out, vec!["TX 0001 ALPHA", "TX 0002 BETA", "TX 0003 GAMMA"]);
}

#[test]
fn idx_tx_commit_rollback_memory() {
    // WITH PERSISTENCE so the MEMORY file survives CLOSE/reopen like the DISK
    // variant (a plain MEMORY file is now ephemeral by default).
    let mem = IDX_TX.replace("STORAGE IS DISK", "STORAGE IS MEMORY WITH PERSISTENCE");
    let out = run_fixture("tx-mem", &mem);
    assert_eq!(out, vec!["TX 0001 ALPHA", "TX 0002 BETA", "TX 0003 GAMMA"]);
}
