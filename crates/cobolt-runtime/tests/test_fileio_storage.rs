// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! End-to-end File I/O suite from the storage/compression test pack.
//!
//! Runs the baseline `fileiot.cbl` plus the six INDEXED storage/compression
//! variants (`STORAGE IS DISK|MEMORY [WITH COMPRESSION]`, and the no-clause
//! defaults) through the full pipeline. Each program is self-checking and prints
//! `RESULT : PASS`. The on-disk ASSIGN paths are redirected to a unique temp
//! directory and the 1,000,000-record performance loop is shrunk so the suite
//! runs fast; the original `tests/cobol/fileio/*.cbl` keep the full 1M profile
//! for manual benchmarking via `rcrun`.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Run a program source, returning captured DISPLAY lines.
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
    display_rx.try_iter().collect()
}

/// Each program references these subdirectories under `tests/cobol/fileio/`.
fn run_variant(tag: &str, raw: &str) {
    // The programs write to `/tmp/<file>`. Redirect that into a unique temp dir
    // so parallel test threads never collide, and shrink the 1M perf loop.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("prc-fileio-{tag}-{nanos}"));
    std::fs::create_dir_all(&base).unwrap();
    let src = raw
        .replace("\"/tmp/", &format!("\"{}/", base.display()))
        .replace("VALUE 1000000", "VALUE 200    ");

    let out = run_capture(&src).join("\n");
    let _ = std::fs::remove_dir_all(&base);

    assert!(out.contains("RESULT       : PASS"), "{tag} did not pass:\n{out}");
    assert!(!out.contains("\nFAIL "), "{tag} reported failures:\n{out}");
}

macro_rules! variant_test {
    ($name:ident, $file:literal) => {
        #[test]
        fn $name() {
            run_variant(
                stringify!($name),
                include_str!(concat!("../../../tests/cobol/fileio/", $file)),
            );
        }
    };
}

variant_test!(fileio_baseline, "fileiot.cbl");
variant_test!(fileio_storage_disk, "fileiot_storage_disk.cbl");
variant_test!(fileio_storage_disk_compression, "fileiot_storage_disk_compression.cbl");
variant_test!(fileio_storage_memory, "fileiot_storage_memory.cbl");
variant_test!(fileio_storage_memory_compression, "fileiot_storage_memory_compression.cbl");
variant_test!(fileio_default_disk, "fileiot_default_disk.cbl");
variant_test!(fileio_default_compression, "fileiot_default_compression.cbl");
