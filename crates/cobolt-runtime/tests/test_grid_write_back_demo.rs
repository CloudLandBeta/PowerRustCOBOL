// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 094, AC11 — PowerDemo3's Datagrid demo saves an edit into `actors.idx`.
//!
//! The real form, the real definition and a copy of the real data file: the
//! grid is seeded from the form exactly as the host seeds it (every designed
//! property, as text), loaded the way the form opens (`COBOL-INIT-FORM`, a
//! `DataSource` quick-bind), edited as the operator edits it, and the copy of
//! the file is read back. The project's own file is never written.

use std::path::Path;
use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};

/// The demo project, built at run time: a `concat!(env!(…))` of a path outside the
/// crate would be read as a compile-time asset the staged SDK must ship.
fn demo() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerDemo3")
}

const LOOP: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 EV-ID      PIC X(64).
       01 CTL-ID     PIC X(64).
       01 COBOL-QUIT PIC X VALUE "0".
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-INIT-FORM" USING "DATAGRID-FORM"
           PERFORM UNTIL COBOL-QUIT = "1"
               CALL "COBOL-WAIT-EVENT" USING EV-ID CTL-ID
           END-PERFORM
           STOP RUN.
"#;

/// A copy of the demo's definition and data in a scratch project folder.
fn scratch_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for relative in ["indexed/actors.cidx", "data/idxfiles/actors.idx"] {
        let to = dir.path().join(relative);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(demo().join(relative), to).unwrap();
    }
    dir
}

/// The grid's design properties, as text, the way the host seeds them.
fn grid_seed() -> Vec<(String, String)> {
    let form = cobolt_forms::load_form(&demo().join("forms/Data/datagrid-form.cfrm")).expect("the demo form loads");
    let grid = form.controls.iter().find(|c| c.id == "DataGrid-1").expect("the demo's grid");
    grid.properties
        .iter()
        .map(|(k, v)| (k.clone(), cobolt_forms::model::property_runtime_text("DataGrid", k, &v.to_xml_string())))
        .collect()
}

struct Ran {
    published: Vec<(String, String)>,
}

impl Ran {
    fn last(&self, prop: &str) -> Option<&str> {
        self.published.iter().rev().find(|(p, _)| p == prop).map(|(_, v)| v.as_str())
    }
}

/// Run the demo's grid; `edit` is `(row, column, new text, rows after the edit)`.
fn run(edit: Option<(usize, usize, &str, String)>, previous: &str) -> Ran {
    let parsed = parse(tokenize(LOOP, SourceFormat::Free));
    assert!(parsed.diagnostics.iter().all(|d| d.severity != Severity::Error), "{:?}", parsed.diagnostics);
    let (event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, _display_rx) = mpsc::channel();
    let (input_tx, input_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(parsed.program.expect("program"), event_rx, state_tx, display_tx);
    interp.set_input_channel(input_rx);
    interp.seed_objects([("DataGrid-1".to_owned(), "DataGrid".to_owned(), grid_seed())]);
    if let Some((row, col, text, rows_after)) = edit {
        for (prop, value) in [
            ("Rows", rows_after),
            ("EditedRow", row.to_string()),
            ("EditedColumn", col.to_string()),
            ("EditedValue", text.to_owned()),
            ("PreviousValue", previous.to_owned()),
        ] {
            input_tx.send(StateUpdate::new("DataGrid-1", prop, value)).unwrap();
        }
        event_tx.send(FormEvent::new("DataGrid-1", "onCellEdited")).unwrap();
    }
    event_tx.send(FormEvent::quit()).unwrap();
    interp.run().expect("the form runs");
    Ran {
        published: state_rx
            .try_iter()
            .filter(|u| u.ctrl_id.eq_ignore_ascii_case("DataGrid-1"))
            .map(|u| (u.prop, u.value))
            .collect(),
    }
}

#[test]
fn the_datagrid_demo_saves_an_edited_actor_name_into_its_file() {
    let project = scratch_project();
    cobolt_forms::assets::set_base(project.path());

    // The grid as the form opens: every actor in the file.
    let loaded = run(None, "");
    let rows: Vec<Vec<String>> = loaded.last("Rows").expect("the demo loads its actors").lines().map(|l| l.split('\t').map(str::to_owned).collect()).collect();
    assert!(rows.len() > 1000, "{} actors", rows.len());
    assert_eq!(rows[1][2], "Joe Pesci", "the second actor, in the column `Actor Caption`");

    // The operator renames him, in the cell, and the form keeps the edit.
    let mut edited = rows.clone();
    edited[1][2] = "Joseph Pesci".into();
    let text: String = edited.iter().map(|r| r.join("\t")).collect::<Vec<_>>().join("\n");
    let saved = run(Some((2, 3, "Joseph Pesci", text)), "Joe Pesci");
    assert_eq!(saved.last("CommitStatus"), Some("00"), "LastError: {:?}", saved.last("LastError"));

    // Opening the form again — "Load data" does the same read — shows the edit.
    let again = run(None, "");
    let reread: Vec<Vec<String>> = again.last("Rows").unwrap().lines().map(|l| l.split('\t').map(str::to_owned).collect()).collect();
    assert_eq!(reread.len(), rows.len(), "no record was added or lost");
    assert_eq!(reread[1][2], "Joseph Pesci", "the edit is in the file");
    let changed = rows.iter().zip(&reread).filter(|(a, b)| a != b).count();
    assert_eq!(changed, 1, "exactly one record differs from before");
    println!("\n  PowerDemo3 Datagrid demo: {} actors loaded; row 2 'Joe Pesci' -> 'Joseph Pesci', CommitStatus 00; reopened: {} actors, {changed} record changed\n", rows.len(), reread.len());
}
