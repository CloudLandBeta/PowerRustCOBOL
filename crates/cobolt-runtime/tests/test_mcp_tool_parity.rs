// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 065 — the two front doors, and the line R30 draws.
//!
//! One tool definition is reachable two ways: a COBOL `CALL` in process, and
//! JSON-RPC over a transport. [`the_two_front_doors_agree`] is the guard that
//! stops them drifting apart (R22/AC14) — the moment one grows a special case,
//! it fails.
//!
//! [`a_tampered_definition_cannot_change_how_records_are_read`] is the other
//! half: a delivered `.cidx` is editable by whoever runs the application, so a
//! doctored one must be able to change a *description* and nothing else
//! (R30/AC20).

use std::path::Path;
use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::indexed::{KeySpec, OpenMode, status};
use cobolt_runtime::mcp_tool::{
    read_description_at, ColumnLayout, FileAccess, IndexedToolSet,
};
use cobolt_runtime::Interpreter;

const RECORD_LEN: usize = 111;

fn temp(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = std::env::temp_dir().join(format!("prc-mcp-it-{tag}-{nanos}"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A `.cidx` carrying the developer's descriptions, and the layout facts that
/// — in a delivery — must NOT be believed.
fn cidx(purpose: &str, id_note: &str, declared_offset: u32, declared_len: u32) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><IndexedFile name="ACTORS-FILE" finalized="true" version="1.0"><assign-path>actors.idx</assign-path><access-mode>dynamic</access-mode><record-format fixed-length="111"/><storage mode="disk" compression="false" persistence="false"/><comment><![CDATA[{purpose}]]></comment><keys><primary duplicates="false" ordering="ascending"><part field="ACTOR-ID" offset="0" length="9" encoding="bytes"/></primary></keys><fields><Field level="1" name="ACTORS-RECORD" usage="display"><Field level="5" name="ACTOR-ID" pic="9(9)" usage="display" offset="{declared_offset}" length="{declared_len}"><comment><![CDATA[{id_note}]]></comment></Field><Field level="5" name="ACTOR-SALARY" pic="9(9)V99" usage="display" offset="99" length="11"><comment><![CDATA[Annual salary]]></comment></Field></Field></fields></IndexedFile>"#
    )
}

fn build_data(path: &Path, rows: &[(&str, &str)]) {
    let primary = KeySpec {
        offset: 0,
        len: 9,
        duplicates: false,
    };
    let mut f = cobolt_runtime::indexed_disk::DiskIndexedFile::new(
        path,
        RECORD_LEN,
        primary,
        Vec::new(),
    );
    assert_eq!(f.open(OpenMode::Output), status::OK);
    for (id, salary) in rows {
        let mut rec = vec![b' '; RECORD_LEN];
        rec[0..9].copy_from_slice(format!("{id:>9}").as_bytes());
        rec[99..110].copy_from_slice(format!("{salary:>11}").as_bytes());
        assert_eq!(f.write(&rec), status::OK);
    }
    f.close();
}

/// The layout the COMPILED PROGRAM knows — always this, never the `.cidx`.
fn access(path: &Path) -> FileAccess {
    FileAccess {
        path: path.to_path_buf(),
        record_len: RECORD_LEN,
        primary: KeySpec {
            offset: 0,
            len: 9,
            duplicates: false,
        },
        columns: vec![
            ColumnLayout {
                name: "ACTOR-ID".into(),
                offset: 0,
                len: 9,
            },
            ColumnLayout {
                name: "ACTOR-SALARY".into(),
                offset: 99,
                len: 11,
            },
        ],
    }
}

/// Run a COBOL program whose tool set has been primed, returning its DISPLAYs.
fn run_cobol_with_tools(src: &str, set: impl Fn(&mut Interpreter)) -> Vec<String> {
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
    set(&mut interp);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

const SEARCH_PROGRAM: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ASKTOOL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-TOOL   PIC X(40) VALUE "search_actors_file".
       01 WS-ARGS   PIC X(80) VALUE '{"ACTOR-SALARY": "100000"}'.
       01 WS-RESULT PIC X(500).
       PROCEDURE DIVISION.
           CALL "COBOL-MCP-SEARCH" USING WS-TOOL WS-ARGS WS-RESULT.
           DISPLAY WS-RESULT.
           STOP RUN.
"#;

/// AC14 — the same question, both doors, one answer.
#[test]
fn the_two_front_doors_agree() {
    let dir = temp("parity");
    let def = dir.join("actors.cidx");
    std::fs::write(&def, cidx("One row per performer", "The key", 0, 9)).unwrap();
    let data = dir.join("actors.idx");
    build_data(&data, &[("1", "100000"), ("2", "250000"), ("3", "100000")]);

    let description = read_description_at(&def).expect("the definition reads");
    let acc = access(&data);

    // Door one: a COBOL program, in process.
    let displays = run_cobol_with_tools(SEARCH_PROGRAM, |interp| {
        interp.allow_mcp_file(description.clone(), acc.clone());
    });
    let in_process = displays.join("\n");

    // Door two: the same tool, through the server loop.
    let mut set = IndexedToolSet::new();
    set.allow(description.clone(), acc.clone());
    let over_the_wire = match &set
        .call(
            "search_actors_file",
            &serde_json::json!({"ACTOR-SALARY": "100000"}),
        )
        .content[0]
    {
        cobolt_mcp::Content::Text { text } => text.clone(),
    };

    // The COBOL side lands in a PIC X(500), so compare on content rather than
    // on trailing spaces the receiving item pads with.
    assert!(
        in_process.contains("ACTORS-FILE"),
        "the COBOL door named the file: {in_process}"
    );
    for fragment in ["2 record(s)", "ACTOR-ID=1", "ACTOR-ID=3"] {
        assert!(
            in_process.contains(fragment),
            "in-process answer missing {fragment}: {in_process}"
        );
        assert!(
            over_the_wire.contains(fragment),
            "over-the-wire answer missing {fragment}: {over_the_wire}"
        );
    }
    assert!(
        !in_process.contains("ACTOR-ID=2"),
        "a non-matching record leaked: {in_process}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// AC20 — R30 under attack.
///
/// The delivered `.cidx` is rewritten to claim `ACTOR-ID` lives at a different
/// offset with a different length. The description it reports must change; the
/// records must read exactly as before. If this ever fails, a file in a
/// delivery folder has become able to change what a record *means*.
#[test]
fn a_tampered_definition_cannot_change_how_records_are_read() {
    let dir = temp("tamper");
    let def = dir.join("actors.cidx");
    let data = dir.join("actors.idx");
    build_data(&data, &[("1", "100000"), ("2", "250000")]);

    // Honest definition first.
    std::fs::write(&def, cidx("Performers", "Unique performer number", 0, 9)).unwrap();
    let mut honest = IndexedToolSet::new();
    honest.allow(read_description_at(&def).unwrap(), access(&data));
    let before = match &honest
        .call("search_actors_file", &serde_json::json!({"ACTOR-ID": "1"}))
        .content[0]
    {
        cobolt_mcp::Content::Text { text } => text.clone(),
    };

    // Now doctor it: a wrong offset, a wrong length, and a rewritten note.
    std::fs::write(&def, cidx("Performers", "TAMPERED NOTE", 40, 3)).unwrap();
    let doctored = read_description_at(&def).expect("still reads");
    let mut after_set = IndexedToolSet::new();
    // The layout still comes from the program — the point of R30.
    after_set.allow(doctored.clone(), access(&data));
    let after = match &after_set
        .call("search_actors_file", &serde_json::json!({"ACTOR-ID": "1"}))
        .content[0]
    {
        cobolt_mcp::Content::Text { text } => text.clone(),
    };

    assert_eq!(
        before, after,
        "a tampered .cidx changed how records read — R30 is broken"
    );
    assert_eq!(
        doctored.columns[0].description, "TAMPERED NOTE",
        "the description DID change, which is the only thing that may"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// An application that marks nothing exposes nothing, even to its own COBOL.
#[test]
fn an_application_that_marks_no_file_answers_nothing() {
    let displays = run_cobol_with_tools(SEARCH_PROGRAM, |_| {});
    let text = displays.join("\n");
    assert!(
        text.contains("no consultable file"),
        "an unprimed application must not answer: {text}"
    );
}
