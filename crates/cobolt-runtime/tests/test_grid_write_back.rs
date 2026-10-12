// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 094 — a cell edited in a bound DataGrid is written back to its source.
//!
//! Each test runs a real event loop (`COBOL-WAIT-EVENT`) against a real indexed
//! file: the operator's edit arrives as the grid's own report (`Rows`,
//! `EditedRow`, `EditedColumn`, `EditedValue`, `PreviousValue`, then
//! `onCellEdited`), the handler runs, and the file is read back afterwards.

use std::sync::mpsc;

use cobolt_indexed::{
    AccessMode, FieldUsage, IndexedDefinition, IndexedField, KeyDef, KeyEncodingDef, KeyOrderingDef, KeyPartDef,
    RecordFormatDef, StorageMode,
};
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::indexed::{status, KeySpec, OpenMode};
use cobolt_runtime::indexed_disk::DiskIndexedFile;
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};

// ── The file ─────────────────────────────────────────────────────────────────

fn leaf(name: &str, pic: &str, offset: u32, length: u32) -> IndexedField {
    IndexedField {
        level: 5,
        name: name.to_owned(),
        pic: pic.to_owned(),
        usage: FieldUsage::Display,
        offset: Some(offset),
        length: Some(length),
        comment: String::new(),
        grid_control: None,
        occurs: None,
        redefines: None,
        synchronized: false,
        children: Vec::new(),
    }
}

/// `ACTOR-ID 9(9)` (the key) · `ACTOR-NAME X(20)` · `ACTOR-SALARY 9(7)V99`.
fn definition(assign_path: &str) -> (IndexedDefinition, [IndexedField; 3]) {
    let (id, name, salary) = (leaf("ACTOR-ID", "9(9)", 0, 9), leaf("ACTOR-NAME", "X(20)", 9, 20), leaf("ACTOR-SALARY", "9(7)V99", 29, 9));
    let root = IndexedField {
        level: 1,
        name: "ACTOR-RECORD".to_owned(),
        pic: String::new(),
        usage: FieldUsage::Display,
        offset: None,
        length: None,
        comment: String::new(),
        grid_control: None,
        occurs: None,
        redefines: None,
        synchronized: false,
        children: vec![id.clone(), name.clone(), salary.clone()],
    };
    let mut def = IndexedDefinition::new("ACTORS-FILE", assign_path);
    def.record_format = RecordFormatDef::Fixed { length: 38 };
    def.storage = StorageMode::Disk;
    def.access_mode = AccessMode::Dynamic;
    def.keys.primary = KeyDef {
        name: Some("ACTOR-ID".into()),
        parts: vec![KeyPartDef { field_name: "ACTOR-ID".into(), offset: 0, length: 9, encoding: KeyEncodingDef::Bytes }],
        duplicates_allowed: false,
        ordering: KeyOrderingDef::Ascending,
    };
    def.fields = vec![root];
    def.finalized = true;
    (def, [id, name, salary])
}

fn encode(fields: &[IndexedField; 3], id: &str, name: &str, salary: &str) -> Vec<u8> {
    let mut record = vec![b' '; 38];
    record[0..9].copy_from_slice(&cobolt_indexed::encode_field_display(&fields[0], id, 9).unwrap());
    record[9..29].copy_from_slice(&cobolt_indexed::encode_field_display(&fields[1], name, 20).unwrap());
    record[29..38].copy_from_slice(&cobolt_indexed::encode_field_display(&fields[2], salary, 9).unwrap());
    record
}

/// One row of the grid for a record, as the loader writes it.
fn row(fields: &[IndexedField; 3], record: &[u8]) -> Vec<String> {
    vec![
        cobolt_indexed::format_field_display(&fields[0], &record[0..9]),
        cobolt_indexed::format_field_display(&fields[1], &record[9..29]),
        cobolt_indexed::format_field_display(&fields[2], &record[29..38]),
    ]
}

const PEOPLE: [(&str, &str, &str); 3] = [("1", "Leonardo DiCaprio", "30000000"), ("2", "Joe Pesci", "12000000"), ("3", "Robert De Niro", "20000000")];

struct World {
    dir: tempfile::TempDir,
    cidx: String,
    data: std::path::PathBuf,
    fields: [IndexedField; 3],
    def: IndexedDefinition,
}

/// An indexed file with the first `how_many` of [`PEOPLE`].
fn world(how_many: usize) -> World {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("actors.idx");
    let cidx = dir.path().join("actors.cidx");
    let (def, fields) = definition(data.to_str().unwrap());
    cobolt_indexed::save_indexed(&cidx, &def).expect("write .cidx");
    let mut file = DiskIndexedFile::new(&data, 38, KeySpec { parts: Vec::new(), offset: 0, len: 9, duplicates: false }, Vec::new());
    assert_eq!(file.open(OpenMode::Output), status::OK);
    for (id, name, salary) in PEOPLE.iter().take(how_many) {
        assert_eq!(file.write(&encode(&fields, id, name, salary)), status::OK);
    }
    assert_eq!(file.close(), status::OK);
    World { cidx: cidx.to_str().unwrap().to_owned(), dir, data, fields, def }
}

impl World {
    /// What the file holds now, as grid rows in key order.
    fn rows_in_file(&self) -> Vec<Vec<String>> {
        let session = cobolt_runtime::indexed_ide::GridSession::open(&self.def, &self.data).expect("open the file");
        session.rows().iter().map(|record| row(&self.fields, record)).collect()
    }

    /// The grid's `Rows` for people `ids` (1-based into [`PEOPLE`]), in that order.
    fn grid_rows(&self, ids: &[usize]) -> Vec<Vec<String>> {
        ids.iter()
            .map(|n| {
                let (id, name, salary) = PEOPLE[n - 1];
                row(&self.fields, &encode(&self.fields, id, name, salary))
            })
            .collect()
    }
}

fn text(rows: &[Vec<String>]) -> String {
    rows.iter().map(|r| r.join("\t")).collect::<Vec<_>>().join("\n")
}

// ── The event loop ───────────────────────────────────────────────────────────

/// The form's event loop, with one handler per event the tests care about.
const LOOP: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GRIDLOOP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 EV-ID      PIC X(64).
       01 CTL-ID     PIC X(64).
       01 COBOL-QUIT PIC X VALUE "0".
       01 WS-STATUS  PIC X(8).
{DATA}
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-INIT-FORM" USING "T"
{INIT}
           PERFORM UNTIL COBOL-QUIT = "1"
               CALL "COBOL-WAIT-EVENT" USING EV-ID CTL-ID
               EVALUATE TRUE
                   WHEN EV-ID = "onCellEdited"
{CELL}
                   WHEN EV-ID = "onClick"
{CLICK}
                   WHEN EV-ID = "onCommitError"
{ERROR}
                   WHEN OTHER
                       CONTINUE
               END-EVALUATE
           END-PERFORM
           STOP RUN.
"#;

struct Handlers {
    cell: &'static str,
    click: &'static str,
}

impl Handlers {
    const NONE: Handlers = Handlers { cell: "CONTINUE", click: "CONTINUE" };
}

/// What a run left: what the program displayed, and every property the runtime
/// published for the grid, in order.
struct Run {
    display: Vec<String>,
    published: Vec<(String, String)>,
}

impl Run {
    /// The last value published for `prop`.
    fn last(&self, prop: &str) -> Option<&str> {
        self.published.iter().rev().find(|(p, _)| p == prop).map(|(_, v)| v.as_str())
    }
}

/// One operator edit, as the grid reports it.
struct Edit {
    rows_after: String,
    row: usize,
    col: usize,
    value: String,
    previous: String,
}

fn edit_cell(rows: &mut [Vec<String>], row: usize, col: usize, value: &str) -> Edit {
    let previous = rows[row - 1][col - 1].clone();
    rows[row - 1][col - 1] = value.to_owned();
    Edit { rows_after: text(rows), row, col, value: value.to_owned(), previous }
}

/// What the form declares and how its grid is bound.
struct Setup {
    data: String,
    init: String,
    seed: Vec<(String, String)>,
    /// Other controls the form has: `(id, class, properties)`.
    objects: Vec<(String, String, Vec<(String, String)>)>,
}

impl Setup {
    /// A grid bound (writable) to the indexed file of `world`.
    fn indexed(world: &World) -> Self {
        Setup {
            data: String::new(),
            init: String::new(),
            objects: Vec::new(),
            seed: vec![
                ("_BindingKind".into(), "IndexedFile".into()),
                ("_BindingIndexedPath".into(), world.cidx.clone()),
                ("_BindingFields".into(), "ACTOR-ID\nACTOR-NAME\nACTOR-SALARY".into()),
                ("_BindingWritable".into(), "1".into()),
                ("AllowCellEditing".into(), "true".into()),
            ],
        }
    }
}

fn run(world: &World, props: &[(&str, &str)], handlers: &Handlers, edit: Option<Edit>, events: &[(&str, &str)]) -> Run {
    run_setup(Setup::indexed(world), props, handlers, edit, events)
}

fn run_setup(setup: Setup, props: &[(&str, &str)], handlers: &Handlers, edit: Option<Edit>, events: &[(&str, &str)]) -> Run {
    let source = LOOP
        .replace("{DATA}", &setup.data)
        .replace("{INIT}", &setup.init)
        .replace("{CELL}", &indent(handlers.cell))
        .replace("{CLICK}", &indent(handlers.click))
        .replace("{ERROR}", &indent("DISPLAY \"ERROR \" GRID-1::CommitStatus"));
    let parsed = parse(tokenize(&source, SourceFormat::Free));
    assert!(parsed.diagnostics.iter().all(|d| d.severity != Severity::Error), "parse errors: {:?}\n{source}", parsed.diagnostics);
    let (event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let (input_tx, input_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(parsed.program.expect("program"), event_rx, state_tx, display_tx);
    interp.set_input_channel(input_rx);
    let mut seed = setup.seed;
    for (k, v) in props {
        seed.retain(|(name, _)| name != k);
        seed.push(((*k).to_owned(), (*v).to_owned()));
    }
    let mut objects = vec![("GRID-1".to_owned(), "DataGrid".to_owned(), seed)];
    objects.extend(setup.objects);
    interp.seed_objects(objects);
    if let Some(edit) = &edit {
        for (prop, value) in [
            ("Rows", edit.rows_after.clone()),
            ("EditedRow", edit.row.to_string()),
            ("EditedColumn", edit.col.to_string()),
            ("EditedValue", edit.value.clone()),
            ("PreviousValue", edit.previous.clone()),
        ] {
            input_tx.send(StateUpdate::new("GRID-1", prop, value)).unwrap();
        }
        event_tx.send(FormEvent::new("GRID-1", "onCellEdited")).unwrap();
    }
    for (control, event) in events {
        event_tx.send(FormEvent::new(*control, *event)).unwrap();
    }
    event_tx.send(FormEvent::quit()).unwrap();
    interp.run().expect("the loop runs");
    Run {
        display: display_rx.try_iter().map(|s| s.trim().to_owned()).collect(),
        published: state_rx
            .try_iter()
            .filter(|u| u.ctrl_id.eq_ignore_ascii_case("GRID-1"))
            .map(|u| (u.prop, u.value))
            .collect(),
    }
}

fn indent(statements: &str) -> String {
    statements.lines().map(|l| format!("                       {l}")).collect::<Vec<_>>().join("\n")
}

// ── The cases ────────────────────────────────────────────────────────────────

/// AutoCommit on (the default): the edit is in the file after the handler.
#[test]
fn an_edit_is_written_to_the_file_when_auto_commit_is_on() {
    let w = world(3);
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 2, 2, "Joseph Pesci");
    let ran = run(&w, &[], &Handlers::NONE, Some(edit), &[]);
    let mut expected = w.grid_rows(&[1, 2, 3]);
    expected[1][1] = "Joseph Pesci".into();
    assert_eq!(w.rows_in_file(), expected, "only the edited cell changed");
    assert_eq!(ran.last("CommitStatus"), Some("00"));
    println!("\n  write-back, auto-commit on: row 2 name -> {:?} in the file, CommitStatus 00, other records untouched\n", w.rows_in_file()[1][1]);
}

/// A handler that restores `PreviousValue` has refused the edit: nothing is written.
#[test]
fn a_handler_that_puts_the_old_value_back_refuses_the_edit() {
    let w = world(3);
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 2, 2, "Refuse Me");
    let refuse = Handlers {
        cell: "GRID-1::SetCellValue(GRID-1::EditedRow, GRID-1::EditedColumn, GRID-1::PreviousValue)",
        click: "CONTINUE",
    };
    let ran = run(&w, &[], &refuse, Some(edit), &[]);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]), "the file is unchanged");
    let shown = ran.last("Rows").unwrap();
    assert!(shown.contains("Joe Pesci") && !shown.contains("Refuse Me"), "the grid shows the old value again: {shown:?}");
}

/// AutoCommit off: the edit waits; nothing is written until the code confirms.
#[test]
fn with_auto_commit_off_the_edit_stays_pending_until_confirmed() {
    let w = world(3);
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 3, 2, "Bob De Niro");
    let ran = run(&w, &[("AutoCommit", "false")], &Handlers::NONE, Some(edit), &[]);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]), "nothing written without CommitEdit()");
    assert_eq!(ran.last("_PendingCells"), Some("3,2"), "the cell is marked pending");

    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 3, 2, "Bob De Niro");
    let confirm = Handlers { cell: "SET WS-STATUS TO GRID-1::CommitEdit()\nDISPLAY \"CONFIRMED \" WS-STATUS", click: "CONTINUE" };
    let ran = run(&w, &[("AutoCommit", "false")], &confirm, Some(edit), &[]);
    let mut expected = w.grid_rows(&[1, 2, 3]);
    expected[2][1] = "Bob De Niro".into();
    assert_eq!(w.rows_in_file(), expected);
    assert!(ran.display.iter().any(|l| l.starts_with("CONFIRMED") && l.ends_with("00")), "{:?}", ran.display);
    assert_eq!(ran.last("_PendingCells"), Some(""), "nothing is pending after the confirm");
    println!("\n  write-back, auto-commit off: not written until CommitEdit(); then written, status 00, no cell pending\n");
}

/// AutoCommit off, discarded: the cell comes back and the file never changed.
#[test]
fn discarding_a_pending_edit_restores_the_cell() {
    let w = world(3);
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 1, 2, "Somebody Else");
    let discard = Handlers { cell: "SET WS-STATUS TO GRID-1::DiscardEdit()", click: "CONTINUE" };
    let ran = run(&w, &[("AutoCommit", "false")], &discard, Some(edit), &[]);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]));
    assert!(ran.last("Rows").unwrap().contains("Leonardo DiCaprio"), "{:?}", ran.last("Rows"));
    assert_eq!(ran.last("_PendingCells"), Some(""));
}

/// The record is found by its key, not by where the row sits in the grid.
#[test]
fn a_sorted_grid_writes_the_record_its_row_belongs_to() {
    let w = world(3);
    // The grid is showing 3, 1, 2 — so its first row is Robert De Niro.
    let mut rows = w.grid_rows(&[3, 1, 2]);
    let edit = edit_cell(&mut rows, 1, 2, "Roberto De Niro");
    run(&w, &[], &Handlers::NONE, Some(edit), &[]);
    let mut expected = w.grid_rows(&[1, 2, 3]);
    expected[2][1] = "Roberto De Niro".into();
    assert_eq!(w.rows_in_file(), expected, "record 3 changed, record 1 did not");
}

/// A REWRITE cannot change a key.
#[test]
fn an_edit_to_the_key_column_is_refused() {
    let w = world(3);
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 2, 1, "000000099");
    let ran = run(&w, &[], &Handlers::NONE, Some(edit), &[]);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]));
    assert_eq!(ran.last("CommitStatus"), Some("21"));
    assert!(ran.display.iter().any(|l| l == "ERROR 21"), "onCommitError reached the handler: {:?}", ran.display);
    assert_eq!(ran.last("Rows").unwrap(), text(&w.grid_rows(&[1, 2, 3])), "the key shows as before");
}

/// Text that does not fit the field is refused and the cell restored.
#[test]
fn a_value_too_long_for_its_field_is_refused() {
    let w = world(3);
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 2, 2, "A name that is far longer than twenty characters");
    let ran = run(&w, &[], &Handlers::NONE, Some(edit), &[]);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]));
    assert_eq!(ran.last("CommitStatus"), Some("92"));
    assert!(ran.last("LastError").unwrap().contains("ACTOR-NAME"), "{:?}", ran.last("LastError"));
    assert!(ran.last("Rows").unwrap().contains("Joe Pesci"));
}

/// A number is checked by the field's PICTURE too.
#[test]
fn a_number_is_encoded_by_the_pictures_of_its_field() {
    let w = world(3);
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 2, 3, "ABC");
    let ran = run(&w, &[], &Handlers::NONE, Some(edit), &[]);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]));
    assert_eq!(ran.last("CommitStatus"), Some("92"));
}

/// The record another process deleted since the load is not replaced by a neighbour.
#[test]
fn a_record_that_is_gone_is_not_written_to_another() {
    let w = world(2); // the file holds 1 and 2 only …
    let mut rows = w.grid_rows(&[1, 2, 3]); // … but the grid still shows 3
    let edit = edit_cell(&mut rows, 3, 2, "Ghost");
    let ran = run(&w, &[], &Handlers::NONE, Some(edit), &[]);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2]), "no record was touched");
    assert_eq!(ran.last("CommitStatus"), Some("23"));
}

/// A read-only binding, or a grid whose editing is off, writes nothing.
#[test]
fn a_read_only_binding_or_editing_off_writes_nothing() {
    let w = world(3);
    for props in [[("_BindingWritable", "0")], [("AllowCellEditing", "false")]] {
        let mut rows = w.grid_rows(&[1, 2, 3]);
        let edit = edit_cell(&mut rows, 2, 2, "Nope");
        let ran = run(&w, &props, &Handlers::NONE, Some(edit), &[]);
        assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]), "{props:?}");
        assert_eq!(ran.last("CommitStatus"), None, "{props:?}: the runtime did not even try");
    }
}

/// A refresh while an edit is pending keeps it, and says so.
#[test]
fn a_refresh_keeps_a_pending_edit_and_leaves_a_status() {
    let w = world(3);
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 2, 2, "Joseph Pesci");
    let reload = Handlers { cell: "CONTINUE", click: "SET WS-STATUS TO GRID-1::RefreshBinding()" };
    let ran = run(&w, &[("AutoCommit", "false")], &reload, Some(edit), &[("BTN", "onClick")]);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]), "still not written");
    assert!(ran.last("Rows").unwrap().contains("Joseph Pesci"), "the cell still shows the pending edit: {:?}", ran.last("Rows"));
    assert_eq!(ran.last("CommitStatus"), Some("98"));
}

/// A quick-bound grid (a `DataSource`, no binding) is writable because its file has a key.
#[test]
fn a_quick_bound_grid_writes_back_through_the_files_primary_key() {
    let w = world(3);
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 1, 2, "Leo DiCaprio");
    let ran = run(
        &w,
        &[("_BindingKind", ""), ("_BindingWritable", ""), ("_BindingIndexedPath", ""), ("_BindingFields", ""), ("DataSource", &format!("{} / ACTOR-RECORD", w.cidx))],
        &Handlers::NONE,
        Some(edit),
        &[],
    );
    let mut expected = w.grid_rows(&[1, 2, 3]);
    expected[0][1] = "Leo DiCaprio".into();
    assert_eq!(w.rows_in_file(), expected);
    assert_eq!(ran.last("CommitStatus"), Some("00"));
    drop(w.dir);
}

// ── A COBOL table ────────────────────────────────────────────────────────────

fn table_setup() -> Setup {
    Setup {
        data: "       01 T-ID   PIC 9(3) OCCURS 3.\n       01 T-NAME PIC X(10) OCCURS 3.\n       01 T-QTY  PIC 9(3) OCCURS 3.".into(),
        init: "           MOVE 1 TO T-ID(1)\n           MOVE \"Bolt\" TO T-NAME(1)\n           MOVE 10 TO T-QTY(1)\n           MOVE 2 TO T-ID(2)\n           MOVE \"Nut\" TO T-NAME(2)\n           MOVE 20 TO T-QTY(2)\n           MOVE 3 TO T-ID(3)\n           MOVE \"Washer\" TO T-NAME(3)\n           MOVE 30 TO T-QTY(3)\n           SET WS-STATUS TO GRID-1::RefreshBinding()\n           DISPLAY \"LOADED \" WS-STATUS\n           DISPLAY \"QTY2 \" T-QTY(2)\n           DISPLAY \"NAME2 \" T-NAME(2)".into(),
        objects: Vec::new(),
        seed: vec![
            ("_BindingKind".into(), "CobolTable".into()),
            ("_BindingFields".into(), "T-ID,T-NAME,T-QTY".into()),
            ("_BindingKeyFields".into(), "T-ID".into()),
            ("_BindingWritable".into(), "1".into()),
            ("AllowCellEditing".into(), "true".into()),
        ],
    }
}

/// The rows a table binding loads, as the first run publishes them.
fn table_rows() -> Vec<Vec<String>> {
    let first = run_setup(table_setup(), &[], &Handlers::NONE, None, &[]);
    first.last("Rows").expect("the table loads").lines().map(|l| l.split('\t').map(str::to_owned).collect()).collect()
}

/// The program's own view of the table after the run (what it DISPLAYed last).
fn qty2(ran: &Run) -> String {
    ran.display.iter().rev().find(|l| l.starts_with("QTY2")).cloned().unwrap_or_default()
}

#[test]
fn a_table_binding_stores_the_edit_in_the_table_and_picture_rules_apply() {
    let mut rows = table_rows();
    assert_eq!(rows.len(), 3, "{rows:?}");
    let edit = edit_cell(&mut rows, 2, 3, "25");
    let after = {
        let mut setup = table_setup();
        setup.init.push_str("\n");
        // Show the table at the end of the loop, after the settle.
        run_setup(setup, &[], &Handlers { cell: "CONTINUE", click: "DISPLAY \"QTY2 \" T-QTY(2)" }, Some(edit), &[("BTN", "onClick")])
    };
    assert_eq!(qty2(&after), "QTY2 025", "{:?}", after.display);
    assert_eq!(after.last("CommitStatus"), Some("00"));
}

#[test]
fn a_table_edit_that_is_not_a_number_is_refused_and_the_table_keeps_its_value() {
    let mut rows = table_rows();
    let edit = edit_cell(&mut rows, 2, 3, "lots");
    let after = run_setup(table_setup(), &[], &Handlers { cell: "CONTINUE", click: "DISPLAY \"QTY2 \" T-QTY(2)" }, Some(edit), &[("BTN", "onClick")]);
    assert_eq!(qty2(&after), "QTY2 020");
    assert_eq!(after.last("CommitStatus"), Some("92"));
}

#[test]
fn a_table_edit_pending_is_confirmed_or_discarded_by_the_code() {
    let mut rows = table_rows();
    let edit = edit_cell(&mut rows, 3, 2, "Spacer");
    let confirm = Handlers { cell: "CONTINUE", click: "SET WS-STATUS TO GRID-1::CommitEdit()\nDISPLAY \"NAME3 \" T-NAME(3)" };
    let ran = run_setup(table_setup(), &[("AutoCommit", "false")], &confirm, Some(edit), &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l.starts_with("NAME3 Spacer")), "{:?}", ran.display);

    let mut rows = table_rows();
    let edit = edit_cell(&mut rows, 3, 2, "Spacer");
    let discard = Handlers { cell: "CONTINUE", click: "SET WS-STATUS TO GRID-1::DiscardEdit()\nDISPLAY \"NAME3 \" T-NAME(3)" };
    let ran = run_setup(table_setup(), &[("AutoCommit", "false")], &discard, Some(edit), &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l.starts_with("NAME3 Washer")), "{:?}", ran.display);
    println!("\n  write-back, COBOL table: stored by key with the PICTURE applied; not-a-number refused (92); confirm stores, discard keeps the old value\n");
}

// ── A SQL table ──────────────────────────────────────────────────────────────

fn sql_setup() -> Setup {
    let quote = "'";
    let init = [
        "           SET WS-H TO SQL-1::Open(\":memory:\")".to_owned(),
        "           SQL-1::Execute(\"CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, qty INTEGER)\")".to_owned(),
        format!("           SQL-1::Execute(\"INSERT INTO items VALUES (1, {quote}Bolt{quote}, 10), (2, {quote}Nut{quote}, 20), (3, {quote}O{quote}{quote}Brien{quote}, 30)\")"),
        "           SET WS-STATUS TO GRID-1::RefreshBinding()".to_owned(),
        "           DISPLAY \"LOADED \" WS-STATUS".to_owned(),
    ]
    .join("\n");
    Setup {
        data: "       01 WS-H   PIC 9(4).\n       01 WS-ROW PIC X(80).".into(),
        init,
        objects: vec![("SQL-1".into(), "SqlDatabase".into(), Vec::new())],
        seed: vec![
            ("_BindingKind".into(), "Sql".into()),
            ("_BindingSqlControl".into(), "SQL-1".into()),
            ("_BindingSql".into(), "SELECT id, name, qty FROM items ORDER BY id".into()),
            ("_BindingTable".into(), "items".into()),
            ("_BindingFields".into(), "id\nname\nqty".into()),
            ("_BindingKeyFields".into(), "id".into()),
            ("_BindingFieldTypes".into(), "number,text,number".into()),
            ("_BindingWritable".into(), "1".into()),
            ("AllowCellEditing".into(), "true".into()),
        ],
    }
}

fn sql_rows() -> Vec<Vec<String>> {
    vec![
        vec!["1".into(), "Bolt".into(), "10".into()],
        vec!["2".into(), "Nut".into(), "20".into()],
        vec!["3".into(), "O'Brien".into(), "30".into()],
    ]
}

/// The click handler: read row `id` back from the database.
fn read_back(id: u32) -> Handlers {
    Handlers {
        cell: "CONTINUE",
        click: Box::leak(
            format!("SQL-1::Query(\"SELECT name, qty FROM items WHERE id = {id}\")\nSET WS-ROW TO SQL-1::Fetch()\nDISPLAY \"ROW \" WS-ROW").into_boxed_str(),
        ),
    }
}

fn the_row(ran: &Run) -> String {
    ran.display.iter().rev().find(|l| l.starts_with("ROW ")).map(|l| l[4..].to_owned()).unwrap_or_default()
}

#[test]
fn a_sql_binding_loads_its_query_into_the_grid_in_the_order_of_its_fields() {
    let ran = run_setup(sql_setup(), &[], &Handlers::NONE, None, &[]);
    assert_eq!(ran.last("Rows"), Some("1\tBolt\t10\n2\tNut\t20\n3\tO'Brien\t30"));
    assert!(ran.display.iter().any(|l| l == "LOADED 0003" || l.starts_with("LOADED")), "{:?}", ran.display);
}

#[test]
fn a_sql_edit_is_an_update_and_text_with_quotes_and_backslashes_round_trips() {
    for typed in ["Joseph", "D'Arcy", "back\\slash", "x'; DROP TABLE items; --", "naïve — 日本語"] {
        let mut rows = sql_rows();
        let edit = edit_cell(&mut rows, 2, 2, typed);
        let ran = run_setup(sql_setup(), &[], &read_back(2), Some(edit), &[("BTN", "onClick")]);
        assert_eq!(ran.last("CommitStatus"), Some("00"), "{typed:?}");
        assert_eq!(the_row(&ran), format!("{typed}\t20"), "{typed:?} — the other cell of the row is untouched");
    }
    println!("\n  write-back, SQL: UPDATE by key; quotes, a backslash, an injection attempt and non-ASCII text all stored as the text typed\n");
}

#[test]
fn a_sql_number_is_written_bare_and_a_non_number_is_refused() {
    let mut rows = sql_rows();
    let edit = edit_cell(&mut rows, 1, 3, "45");
    let ran = run_setup(sql_setup(), &[], &read_back(1), Some(edit), &[("BTN", "onClick")]);
    assert_eq!(the_row(&ran), "Bolt\t45");

    let mut rows = sql_rows();
    let edit = edit_cell(&mut rows, 1, 3, "1; DELETE FROM items");
    let ran = run_setup(sql_setup(), &[], &read_back(1), Some(edit), &[("BTN", "onClick")]);
    assert_eq!(ran.last("CommitStatus"), Some("92"));
    assert_eq!(the_row(&ran), "Bolt\t10", "nothing was written");
    assert!(ran.last("Rows").unwrap().contains("Bolt\t10"), "the cell is restored");
}

#[test]
fn a_sql_edit_of_a_row_that_is_gone_or_of_the_key_writes_nothing() {
    let mut setup = sql_setup();
    setup.init.push_str("\n           SQL-1::Execute(\"DELETE FROM items WHERE id = 2\")");
    let mut rows = sql_rows();
    let edit = edit_cell(&mut rows, 2, 2, "Ghost");
    let ran = run_setup(setup, &[], &Handlers::NONE, Some(edit), &[]);
    assert_eq!(ran.last("CommitStatus"), Some("23"));

    let mut rows = sql_rows();
    let edit = edit_cell(&mut rows, 2, 1, "99");
    let ran = run_setup(sql_setup(), &[], &Handlers::NONE, Some(edit), &[]);
    assert_eq!(ran.last("CommitStatus"), Some("21"));
}

#[test]
fn a_sql_edit_pending_is_confirmed_by_the_code() {
    let mut rows = sql_rows();
    let edit = edit_cell(&mut rows, 3, 3, "99");
    let confirm = Handlers {
        cell: "CONTINUE",
        click: "SET WS-STATUS TO GRID-1::CommitEdit()\nSQL-1::Query(\"SELECT qty FROM items WHERE id = 3\")\nSET WS-ROW TO SQL-1::Fetch()\nDISPLAY \"ROW \" WS-ROW",
    };
    let ran = run_setup(sql_setup(), &[("AutoCommit", "false")], &confirm, Some(edit), &[("BTN", "onClick")]);
    assert_eq!(the_row(&ran), "99");
    assert_eq!(ran.last("CommitStatus"), Some("00"));
}
