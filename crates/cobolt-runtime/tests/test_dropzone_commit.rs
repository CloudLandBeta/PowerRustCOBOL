// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `FDZ::CommitFiles()` — the confirmation half of a staged FileDropZone drop,
//! driven from real COBOL.
//!
//! A default zone copies a dropped file the instant it lands, which gives the
//! operator no chance to change their mind. With `StageOnly` on, the drop only
//! HOLDS the files and lists them with a tick box each; the form's own COBOL
//! calls `CommitFiles()` when the operator is happy, and only then is anything
//! written. These tests stand in for the GUI: they seed the properties a drop
//! would have set, run the COBOL, and look at the folder.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Run `src`, having seeded `(control, property, value)` the way a drop does.
fn run_seeded(src: &str, seed: &[(&str, &str, &str)]) -> Vec<String> {
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

    // Group the seed by control, as `seed_objects` takes it.
    let mut objects: Vec<(String, String, Vec<(String, String)>)> = Vec::new();
    for (id, key, value) in seed {
        match objects.iter_mut().find(|(oid, _, _)| oid == id) {
            Some((_, _, props)) => props.push((key.to_string(), value.to_string())),
            None => objects.push((
                id.to_string(),
                "Control".to_owned(),
                vec![(key.to_string(), value.to_string())],
            )),
        }
    }
    interp.seed_objects(objects);
    interp.run().expect("run failed");
    display_rx.try_iter().collect()
}

const COMMIT_SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-SUMMARY PIC X(60).
       PROCEDURE DIVISION.
           MOVE FDZ-1::CommitFiles() TO WS-SUMMARY
           DISPLAY "summary=[" FUNCTION TRIM(WS-SUMMARY) "]".
           DISPLAY "landed=[" FDZ-1::DroppedFiles "]".
           DISPLAY "stored=[" FUNCTION TRIM(FDZ-1::CommitSummary) "]".
           DISPLAY "rows=[" LST-1::Items "]".
           STOP RUN.
"#;

/// The whole flow: two files staged, one unticked, one copied.
#[test]
fn committing_copies_only_the_ticked_files_and_reports_what_it_did() {
    let root = std::env::temp_dir().join(format!("prc-commit-{}", std::process::id()));
    let from = root.join("from");
    let inbox = root.join("inbox");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&from).expect("scratch");
    let write = |name: &str, bytes: usize| {
        let p = from.join(name);
        std::fs::write(&p, vec![b'x'; bytes]).expect("write");
        p.display().to_string()
    };
    let alpha = write("alpha.csv", 1_500_000);
    let beta = write("beta.csv", 2_000_000);

    // What a staged drop leaves behind: both files held, both rows ticked.
    let rows = format!(
        "{}\n{}",
        cobolt_forms::dropzone::row_label(&alpha, Some(1_500_000)),
        cobolt_forms::dropzone::row_label(&beta, Some(2_000_000))
    );
    // …then the operator unticks beta.
    let ticked = cobolt_forms::dropzone::row_label(&alpha, Some(1_500_000));

    let out = run_seeded(
        COMMIT_SRC,
        &[
            ("FDZ-1", "StagedFiles", &format!("{alpha}\n{beta}")),
            ("FDZ-1", "DestinationFolder", &inbox.display().to_string()),
            ("FDZ-1", "FileListControl", "LST-1"),
            ("LST-1", "Items", &rows),
            ("LST-1", "CheckedItems", &ticked),
        ],
    )
    .join("\n");

    assert!(
        out.contains("summary=[1 of 1 copied, 1.500 MB]"),
        "CommitFiles must return the summary: {out}"
    );
    assert!(
        out.contains("stored=[1 of 1 copied, 1.500 MB]"),
        "…and leave it in CommitSummary: {out}"
    );
    // The ticked file was copied; the unticked one was not.
    assert!(
        inbox.join("alpha.csv").exists(),
        "the ticked file must be copied"
    );
    assert_eq!(
        std::fs::read(inbox.join("alpha.csv")).unwrap().len(),
        1_500_000
    );
    assert!(
        !inbox.join("beta.csv").exists(),
        "the UNTICKED file must not be copied: {out}"
    );
    // DroppedFiles is now the copy's path, and only the included file. Read
    // that line alone: the excluded file is still in `rows=[…]` on purpose.
    let landed = out
        .lines()
        .find(|l| l.starts_with("landed=["))
        .unwrap_or_else(|| panic!("no landed line in:\n{out}"));
    assert!(
        landed.contains(&inbox.join("alpha.csv").display().to_string()),
        "DroppedFiles must carry the new path: {landed}"
    );
    assert!(
        !landed.contains(&beta),
        "the excluded file must not appear in DroppedFiles: {landed}"
    );
    // The rows now say what happened, and the unticked one is still listed.
    assert!(out.contains("✓ "), "the copied row must be ticked off: {out}");
    assert!(
        out.contains("(excluded)"),
        "the unticked row must stay in the list, marked: {out}"
    );

    println!(
        "\n  CommitFiles — 2 staged, 1 unticked ⇒ \"1 of 1 copied, 1.500 MB\"; \
         alpha.csv copied to the destination, beta.csv left where it was and its \
         row kept as (excluded); DroppedFiles carries only the copy's new path\n"
    );
    let _ = std::fs::remove_dir_all(&root);
}

const EMPTY_SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           DISPLAY "empty=[" FDZ-1::CommitFiles() "]".
           STOP RUN.
"#;

/// Confirming a zone that is holding nothing is not an error.
#[test]
fn committing_an_empty_zone_reports_nothing_copied() {
    let out = run_seeded(EMPTY_SRC, &[("FDZ-1", "DestinationFolder", "/tmp")]).join("\n");
    assert!(
        out.contains("empty=[0 of 0 copied, 0.000 MB]"),
        "an empty zone must report 0 of 0, not fail: {out}"
    );
    println!("\n  CommitFiles — a zone holding nothing reports \"0 of 0 copied, 0.000 MB\"\n");
}

const NO_LIST_SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           DISPLAY "nolist=[" FDZ-1::CommitFiles() "]".
           STOP RUN.
"#;

/// With no review list wired up there is nothing to untick, so everything
/// staged is included — a zone without a list still works.
#[test]
fn with_no_review_list_every_staged_file_is_included() {
    let root = std::env::temp_dir().join(format!("prc-commit-nolist-{}", std::process::id()));
    let from = root.join("from");
    let inbox = root.join("inbox");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&from).expect("scratch");
    let one = from.join("one.csv");
    let two = from.join("two.csv");
    std::fs::write(&one, vec![b'x'; 1_000_000]).expect("write");
    std::fs::write(&two, vec![b'x'; 1_000_000]).expect("write");

    let out = run_seeded(
        NO_LIST_SRC,
        &[
            (
                "FDZ-1",
                "StagedFiles",
                &format!("{}\n{}", one.display(), two.display()),
            ),
            ("FDZ-1", "DestinationFolder", &inbox.display().to_string()),
        ],
    )
    .join("\n");

    assert!(
        out.contains("nolist=[2 of 2 copied, 2.000 MB]"),
        "with no list, both staged files are included: {out}"
    );
    assert!(inbox.join("one.csv").exists() && inbox.join("two.csv").exists());

    println!(
        "\n  CommitFiles — no FileListControl ⇒ nothing to untick, so both staged \
         files copied: \"2 of 2 copied, 2.000 MB\"\n"
    );
    let _ = std::fs::remove_dir_all(&root);
}
