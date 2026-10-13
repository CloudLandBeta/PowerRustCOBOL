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
    /// Another process deletes record `n` of [`PEOPLE`].
    fn delete_record(&self, n: usize) {
        let mut session = cobolt_runtime::indexed_ide::GridSession::open(&self.def, &self.data).expect("open the file");
        let at = session.rows().iter().position(|r| row(&self.fields, r)[0] == row(&self.fields, &encode(&self.fields, PEOPLE[n - 1].0, "", "0"))[0]).expect("the record is there");
        session.select_row(at).unwrap();
        session.delete_current().unwrap();
        session.commit();
    }

    /// Another process renames record `n` of [`PEOPLE`].
    fn rename_record(&self, n: usize, name: &str) {
        let mut session = cobolt_runtime::indexed_ide::GridSession::open(&self.def, &self.data).expect("open the file");
        let at = session.rows().iter().position(|r| row(&self.fields, r)[0] == row(&self.fields, &encode(&self.fields, PEOPLE[n - 1].0, "", "0"))[0]).expect("the record is there");
        let mut record = session.rows()[at].clone();
        record[9..29].copy_from_slice(&cobolt_indexed::encode_field_display(&self.fields[1], name, 20).unwrap());
        session.select_row(at).unwrap();
        session.rewrite_row(&record).unwrap();
        session.commit();
    }

    /// Another process changes the salary of record `n` of [`PEOPLE`].
    fn reprice_record(&self, n: usize, salary: &str) {
        let mut session = cobolt_runtime::indexed_ide::GridSession::open(&self.def, &self.data).expect("open the file");
        let at = session.rows().iter().position(|r| row(&self.fields, r)[0] == row(&self.fields, &encode(&self.fields, PEOPLE[n - 1].0, "", "0"))[0]).expect("the record is there");
        let mut record = session.rows()[at].clone();
        record[29..38].copy_from_slice(&cobolt_indexed::encode_field_display(&self.fields[2], salary, 9).unwrap());
        session.select_row(at).unwrap();
        session.rewrite_row(&record).unwrap();
        session.commit();
    }

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
                   WHEN EV-ID = "onSaved"
                       DISPLAY "SAVED " GRID-1::SavedRows
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
    /// The statements sent on the `SqlDatabase` connections, in order.
    sql: Vec<String>,
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
            // A bound grid is loaded by its binding — that is when it learns what the file holds.
            init: "           SET WS-STATUS TO GRID-1::RefreshBinding()\n           DISPLAY \"LOADED\"".into(),
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

/// The channels that talk to the interpreter of one run.
struct Io {
    event_tx: mpsc::Sender<FormEvent>,
    input_tx: mpsc::Sender<StateUpdate>,
    display_rx: mpsc::Receiver<String>,
    state_rx: mpsc::Receiver<StateUpdate>,
}

/// The interpreter of one run, and its channels.
struct Rig {
    interp: Interpreter,
    io: Io,
}

/// Parse the loop, build the interpreter and seed the grid. With `sync` the
/// `onCellEdited` handler first displays `SEEN`, so a test that sends several
/// edits can wait for each to be noted before it sends the next.
fn rig(setup: Setup, props: &[(&str, &str)], handlers: &Handlers, sync: bool) -> Rig {
    let cell = if sync { format!("DISPLAY \"SEEN\"\n{}", handlers.cell) } else { handlers.cell.to_owned() };
    let source = LOOP
        .replace("{DATA}", &setup.data)
        .replace("{INIT}", &setup.init)
        .replace("{CELL}", &indent(&cell))
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
    interp.record_sql_statements();
    let mut seed = setup.seed;
    for (k, v) in props {
        seed.retain(|(name, _)| name != k);
        seed.push(((*k).to_owned(), (*v).to_owned()));
    }
    let mut objects = vec![("GRID-1".to_owned(), "DataGrid".to_owned(), seed)];
    objects.extend(setup.objects);
    interp.seed_objects(objects);
    Rig { interp, io: Io { event_tx, input_tx, display_rx, state_rx } }
}

/// One operator edit, delivered the way the host delivers it: the grid's report,
/// then `onCellEdited`.
fn send_edit(io: &Io, edit: &Edit) {
    for (prop, value) in [
        ("Rows", edit.rows_after.clone()),
        ("EditedRow", edit.row.to_string()),
        ("EditedColumn", edit.col.to_string()),
        ("EditedValue", edit.value.clone()),
        ("PreviousValue", edit.previous.clone()),
    ] {
        io.input_tx.send(StateUpdate::new("GRID-1", prop, value)).unwrap();
    }
    io.event_tx.send(FormEvent::new("GRID-1", "onCellEdited")).unwrap();
}

fn run_setup(setup: Setup, props: &[(&str, &str)], handlers: &Handlers, edit: Option<Edit>, events: &[(&str, &str)]) -> Run {
    let mut rig = rig(setup, props, handlers, false);
    if let Some(edit) = &edit {
        send_edit(&rig.io, edit);
    }
    for (control, event) in events {
        rig.io.event_tx.send(FormEvent::new(*control, *event)).unwrap();
    }
    rig.io.event_tx.send(FormEvent::quit()).unwrap();
    rig.interp.run().expect("the loop runs");
    Run {
        display: rig.io.display_rx.try_iter().map(|s| s.trim().to_owned()).collect(),
        published: rig
            .io
            .state_rx
            .try_iter()
            .filter(|u| u.ctrl_id.eq_ignore_ascii_case("GRID-1"))
            .map(|u| (u.prop, u.value))
            .collect(),
        sql: rig.interp.recorded_sql_statements(),
    }
}

/// Several edits, each delivered after the interpreter has noted the one before
/// (the interpreter runs on its own thread; a handler that shows `SEEN` is the
/// signal), then `events`. The host does the same: an edit's report is folded in
/// when its own event arrives, never early.
fn run_many(setup: Setup, props: &[(&str, &str)], handlers: &Handlers, edits: &[Edit], events: &[(&str, &str)]) -> Run {
    run_many_with(setup, props, handlers, edits, events, &|| {})
}

/// [`run_many`], with `between` run once the form has loaded its grid (its `init` has
/// shown `LOADED`) and before the first edit arrives — the moment someone else can change
/// the source behind the grid's back.
fn run_many_with(
    setup: Setup,
    props: &[(&str, &str)],
    handlers: &Handlers,
    edits: &[Edit],
    events: &[(&str, &str)],
    between: &dyn Fn(),
) -> Run {
    // The interpreter is not `Send` (its file engines are trait objects), so it
    // is built on its own thread; only the channels come back.
    let props: Vec<(String, String)> = props.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
    let handlers = Handlers { cell: handlers.cell, click: handlers.click };
    let (io_tx, io_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let props: Vec<(&str, &str)> = props.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let Rig { mut interp, io } = rig(setup, &props, &handlers, true);
        io_tx.send(io).unwrap();
        interp.run().expect("the loop runs");
        interp.recorded_sql_statements()
    });
    let io = io_rx.recv().expect("the interpreter was built");
    let mut display = Vec::new();
    // Wait for the load, whatever the hook: an edit must not overtake it.
    loop {
        let line = io.display_rx.recv_timeout(std::time::Duration::from_secs(20)).expect("the form loaded its grid").trim().to_owned();
        if line.starts_with("LOADED") {
            display.push(line);
            break;
        }
        display.push(line);
    }
    between();
    for edit in edits {
        send_edit(&io, edit);
        loop {
            let line = io.display_rx.recv_timeout(std::time::Duration::from_secs(20)).expect("the edit was noted").trim().to_owned();
            if line == "SEEN" {
                break;
            }
            display.push(line);
        }
    }
    for (control, event) in events {
        io.event_tx.send(FormEvent::new(*control, *event)).unwrap();
    }
    io.event_tx.send(FormEvent::quit()).unwrap();
    let sql = worker.join().expect("the interpreter thread finished");
    display.extend(io.display_rx.try_iter().map(|s| s.trim().to_owned()).filter(|l| l != "SEEN"));
    Run {
        display,
        published: io
            .state_rx
            .try_iter()
            .filter(|u| u.ctrl_id.eq_ignore_ascii_case("GRID-1"))
            .map(|u| (u.prop, u.value))
            .collect(),
        sql,
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
    // The program sorts the grid by salary: it shows 2, 3, 1 — so its second row is Robert De Niro.
    let mut setup = Setup::indexed(&w);
    setup.init.push_str("\n           GRID-1::Sort(3)");
    let mut rows = w.grid_rows(&[2, 3, 1]);
    let edit = edit_cell(&mut rows, 2, 2, "Roberto De Niro");
    run_setup(setup, &[], &Handlers::NONE, Some(edit), &[]);
    let mut expected = w.grid_rows(&[1, 2, 3]);
    expected[2][1] = "Roberto De Niro".into();
    assert_eq!(w.rows_in_file(), expected, "record 3 changed, record 1 did not");
}

/// Spec 095: a batch waits. An edit noted before the program sorts the grid
/// still belongs to the row it was made on, not to whatever now sits at that
/// position.
#[test]
fn a_sort_between_the_edit_and_the_confirm_writes_the_right_record() {
    let w = world(3);
    let mut rows = w.grid_rows(&[1, 2, 3]);
    // Record 3 becomes "Bob De Niro", so sorting by name moves it from last to first.
    let edit = edit_cell(&mut rows, 3, 2, "Bob De Niro");
    let sort_then_confirm = Handlers {
        cell: "CONTINUE",
        click: "GRID-1::Sort(2)\nSET WS-STATUS TO GRID-1::CommitEdit()\nDISPLAY \"CONFIRMED \" WS-STATUS",
    };
    let ran = run(&w, &[("AutoCommit", "false")], &sort_then_confirm, Some(edit), &[("BTN", "onClick")]);
    let mut expected = w.grid_rows(&[1, 2, 3]);
    expected[2][1] = "Bob De Niro".into();
    assert_eq!(w.rows_in_file(), expected, "record 3 changed, and only record 3");
    assert!(ran.display.iter().any(|l| l.starts_with("CONFIRMED") && l.ends_with("00")), "{:?}", ran.display);
    let shown = ran.last("Rows").unwrap();
    assert!(shown.starts_with("3\tBob De Niro"), "the sorted grid shows the edit on its row: {shown:?}");
    println!("\n  write-back, sort between edit and confirm: record 3 -> \"Bob De Niro\" in the file; the other two untouched\n");
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
    let w = world(3);
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 3, 2, "Ghost");
    // Someone else deletes record 3 after the grid has loaded it.
    let ran = run_many_with(Setup::indexed(&w), &[], &Handlers::NONE, &[edit], &[], &|| w.delete_record(3));
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

// ── Spec 095: a save is one unit ─────────────────────────────────────────────

/// What a grid row looks like for record `n` of [`PEOPLE`] with `name` and `salary`.
fn person(w: &World, n: usize, name: &str, salary: &str) -> Vec<String> {
    row(&w.fields, &encode(&w.fields, PEOPLE[n - 1].0, name, salary))
}

/// The three edits of AC2: record 1's name and salary, and record 3's name.
fn three_edits(w: &World, third_name: &str) -> Vec<Edit> {
    let mut rows = w.grid_rows(&[1, 2, 3]);
    vec![edit_cell(&mut rows, 1, 2, "Leonardo D."), edit_cell(&mut rows, 1, 3, "12345"), edit_cell(&mut rows, 3, 2, third_name)]
}

/// A working-storage item a handler can copy a cell into, to read it back.
fn with_cell_item(mut setup: Setup) -> Setup {
    setup.data = "       01 WS-CELL PIC X(60).".to_owned();
    setup
}

/// Press the button: `Save()` what is pending, say how it went, and read a cell back.
const SAVE_AND_READ: Handlers = Handlers {
    cell: "CONTINUE",
    click: "SET WS-STATUS TO GRID-1::Save()\nDISPLAY \"SAVE \" WS-STATUS\nSET WS-CELL TO GRID-1::GetCellValue(3, 2)\nDISPLAY \"CELL \" WS-CELL",
};

/// AC2: three cells in two records are written by one save — both records hold
/// the new values, `SavedRows` counts the two rows, and `onSaved` fires once.
#[test]
fn three_cells_in_two_records_are_one_save() {
    let w = world(3);
    let edits = three_edits(&w, "Bob De Niro");
    let ran = run_many(with_cell_item(Setup::indexed(&w)), &[("AutoCommit", "false")], &SAVE_AND_READ, &edits, &[("BTN", "onClick")]);
    let expected = vec![person(&w, 1, "Leonardo D.", "12345"), person(&w, 2, "Joe Pesci", "12000000"), person(&w, 3, "Bob De Niro", "20000000")];
    assert_eq!(w.rows_in_file(), expected, "records 1 and 3 changed, record 2 did not");
    assert!(ran.display.iter().any(|l| l == "SAVE 00"), "{:?}", ran.display);
    assert_eq!(ran.last("SavedRows"), Some("2"), "two records were written, three cells");
    assert_eq!(ran.display.iter().filter(|l| l.starts_with("SAVED")).count(), 1, "onSaved fired once: {:?}", ran.display);
    assert!(ran.display.iter().any(|l| l == "SAVED 2"), "{:?}", ran.display);
    assert_eq!(ran.last("_PendingCells"), Some(""), "nothing is pending after the save");
    println!("\n  save, indexed: 3 cells in 2 records written together; SavedRows 2; onSaved once; nothing pending\n");
}

/// AC2 / Q6: a value the second record cannot hold writes **neither** record, puts
/// the old values back in every cell, marks the failing cell, and raises
/// `onCommitError` once.
#[test]
fn a_bad_value_in_the_second_record_changes_neither() {
    let w = world(3);
    let edits = three_edits(&w, "A name that is far longer than twenty characters");
    let ran = run_many(with_cell_item(Setup::indexed(&w)), &[("AutoCommit", "false")], &SAVE_AND_READ, &edits, &[("BTN", "onClick")]);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]), "neither record changed");
    assert!(ran.display.iter().any(|l| l == "SAVE 92"), "{:?}", ran.display);
    assert_eq!(ran.last("CommitStatus"), Some("92"));
    assert!(ran.last("LastError").unwrap().contains("ACTOR-NAME"), "{:?}", ran.last("LastError"));
    assert_eq!(ran.last("SavedRows"), Some("0"));
    assert_eq!(ran.last("_PendingCells"), Some(""), "nothing is left pending: the old values are back");
    assert_eq!(ran.last("_FailedCells"), Some("3,2"), "the cell that could not be written is marked");
    assert_eq!(ran.last("Rows").unwrap(), text(&w.grid_rows(&[1, 2, 3])), "all three cells show what the file holds");
    assert_eq!(ran.display.iter().filter(|l| l.starts_with("ERROR")).count(), 1, "onCommitError fired once: {:?}", ran.display);
    assert!(!ran.display.iter().any(|l| l.starts_with("SAVED")), "no onSaved: {:?}", ran.display);
    assert!(ran.display.iter().any(|l| l == "CELL Robert De Niro"), "the cell has its old value back: {:?}", ran.display);
    println!("\n  save, indexed, rolled back: bad value in record 3 -> neither record written; all 3 cells back to the file's values; cell 3,2 marked; 1 onCommitError\n");
}

/// R12: an edit to a key column fails the whole unit, like any other bad edit.
#[test]
fn a_key_column_edit_fails_the_whole_unit() {
    let w = world(3);
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edits = vec![edit_cell(&mut rows, 1, 2, "Fine Name"), edit_cell(&mut rows, 2, 1, "000000099")];
    let ran = run_many(with_cell_item(Setup::indexed(&w)), &[("AutoCommit", "false")], &SAVE_AND_READ, &edits, &[("BTN", "onClick")]);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]), "the valid edit was not written either");
    assert_eq!(ran.last("CommitStatus"), Some("21"));
    assert_eq!(ran.last("_FailedCells"), Some("2,1"));
    assert_eq!(ran.last("_PendingCells"), Some(""));
    assert_eq!(ran.last("Rows").unwrap(), text(&w.grid_rows(&[1, 2, 3])), "both cells are back, the valid edit too");
    println!("\n  save, indexed, key edited: status 21, nothing written, both cells back to the file's values\n");
}

/// AC4: with `AutoCommit` off a table edit changes nothing until the save.
#[test]
fn with_auto_commit_off_nothing_is_stored_in_the_table_until_save() {
    let mut rows = table_rows();
    let edit = edit_cell(&mut rows, 2, 3, "25");
    let ran = run_setup(
        table_setup(),
        &[("AutoCommit", "false")],
        &Handlers {
            cell: "CONTINUE",
            click: "DISPLAY \"BEFORE \" T-QTY(2)\nSET WS-STATUS TO GRID-1::CommitEdit()\nDISPLAY \"AFTER \" T-QTY(2)",
        },
        Some(edit),
        &[("BTN", "onClick")],
    );
    assert!(ran.display.iter().any(|l| l == "BEFORE 020"), "{:?}", ran.display);
    assert!(ran.display.iter().any(|l| l == "AFTER 025"), "{:?}", ran.display);
    assert_eq!(ran.last("SavedRows"), Some("3"), "the table was regenerated: three rows written");
    println!("\n  save, COBOL table, AutoCommit off: T-QTY(2) 020 before the save, 025 after; SavedRows 3 (the table is regenerated whole)\n");
}

/// AC4: one value the table cannot hold stores **none** of a multi-edit save.
#[test]
fn one_non_numeric_value_stores_none_of_a_multi_edit_save() {
    let mut rows = table_rows();
    let edits = vec![edit_cell(&mut rows, 1, 2, "Screw"), edit_cell(&mut rows, 2, 3, "lots")];
    let ran = run_many(
        table_setup(),
        &[("AutoCommit", "false")],
        &Handlers {
            cell: "CONTINUE",
            click: "SET WS-STATUS TO GRID-1::CommitEdit()\nDISPLAY \"SAVE \" WS-STATUS\nDISPLAY \"NAME1 \" T-NAME(1)\nDISPLAY \"QTY2 \" T-QTY(2)",
        },
        &edits,
        &[("BTN", "onClick")],
    );
    assert!(ran.display.iter().any(|l| l == "SAVE 92"), "{:?}", ran.display);
    assert!(ran.display.iter().any(|l| l == "NAME1 Bolt"), "the valid edit was not stored either: {:?}", ran.display);
    assert!(ran.display.iter().any(|l| l == "QTY2 020"), "{:?}", ran.display);
    assert_eq!(ran.last("_FailedCells"), Some("2,3"));
    assert_eq!(ran.last("_PendingCells"), Some(""), "the old values are back");
    assert_eq!(ran.last("Rows"), Some("1\tBolt\t10\n2\tNut\t20\n3\tWasher\t30"), "the grid shows the table as it is");
    println!("\n  save, COBOL table, one non-numeric value: status 92, T-NAME(1) still Bolt, T-QTY(2) still 020, the grid back to the table\n");
}

/// A table with no key, and an occurrence (the 3rd) left blank, so that the 3rd
/// line of the grid is table occurrence 4.
fn keyless_setup() -> Setup {
    Setup {
        data: "       01 T-CODE PIC X(3) OCCURS 4.\n       01 T-NAME PIC X(10) OCCURS 4.\n       01 T-NOTE PIC X(10) OCCURS 4.".into(),
        init: [
            "           MOVE \"A\" TO T-CODE(1)",
            "           MOVE \"Washer\" TO T-NAME(1)",
            "           MOVE \"x\" TO T-NOTE(1)",
            "           MOVE \"B\" TO T-CODE(2)",
            "           MOVE \"Nut\" TO T-NAME(2)",
            "           MOVE \"y\" TO T-NOTE(2)",
            "           MOVE \"D\" TO T-CODE(4)",
            "           MOVE \"Bolt\" TO T-NAME(4)",
            "           MOVE \"z\" TO T-NOTE(4)",
            "           SET WS-STATUS TO GRID-1::RefreshBinding()",
        ]
        .join("\n"),
        objects: Vec::new(),
        seed: vec![
            ("_BindingKind".into(), "CobolTable".into()),
            ("_BindingFields".into(), "T-CODE,T-NAME,T-NOTE".into()),
            ("_BindingWritable".into(), "1".into()),
            ("AllowCellEditing".into(), "true".into()),
        ],
    }
}

fn keyless_rows() -> Vec<Vec<String>> {
    [["A", "Washer", "x"], ["B", "Nut", "y"], ["D", "Bolt", "z"]].iter().map(|r| r.iter().map(|c| (*c).to_owned()).collect()).collect()
}

/// Q2: a COBOL table is **regenerated from the grid** — whatever the grid holds, in
/// the grid's order, goes into the occurrences and the rest are blanked. Here the
/// table has no key and an occurrence (the 3rd) left blank, so the 3rd line of the
/// grid is occurrence 4; after the save it is occurrence 3.
#[test]
fn a_table_is_regenerated_from_the_grid() {
    let cases = [
        ("no change", "CONTINUE", ["Washer", "Nut", "Anchor", ""], "3"),
        ("a Sort", "GRID-1::Sort(2)", ["Anchor", "Nut", "Washer", ""], "3"),
        ("a DeleteRow", "GRID-1::DeleteRow(1)", ["Nut", "Anchor", "", ""], "2"),
        ("an AddRow", "GRID-1::AddRow()\nGRID-1::SetCellValue(4, 1, \"E\")\nGRID-1::SetCellValue(4, 2, \"Spacer\")\nGRID-1::SetCellValue(4, 3, \"w\")", ["Washer", "Nut", "Anchor", "Spacer"], "4"),
    ];
    for (what, change, names, saved) in cases {
        let mut rows = keyless_rows();
        let edit = edit_cell(&mut rows, 3, 2, "Anchor"); // line 3 is occurrence 4
        let click = format!(
            "{change}\nSET WS-STATUS TO GRID-1::Save()\nDISPLAY \"SAVE \" WS-STATUS\nDISPLAY \"N1 \" T-NAME(1)\nDISPLAY \"N2 \" T-NAME(2)\nDISPLAY \"N3 \" T-NAME(3)\nDISPLAY \"N4 \" T-NAME(4)"
        );
        let handlers = Handlers { cell: "CONTINUE", click: Box::leak(click.into_boxed_str()) };
        let ran = run_setup(keyless_setup(), &[("AutoCommit", "false")], &handlers, Some(edit), &[("BTN", "onClick")]);
        assert!(ran.display.iter().any(|l| l == "SAVE 00"), "after {what}: {:?}", ran.display);
        for (i, name) in names.iter().enumerate() {
            let want = format!("N{} {name}", i + 1);
            assert!(ran.display.iter().any(|l| l.trim_end() == want.trim_end()), "after {what}: {want:?} not in {:?}", ran.display);
        }
        assert_eq!(ran.last("SavedRows"), Some(saved), "after {what}");
    }
    println!("\n  save, COBOL table: regenerated from the grid -- the edit, then a Sort, a DeleteRow and an AddRow each become the table; the occurrences after the last row are blank\n");
}

/// More rows in the grid than the table has room for are refused (`92`) and the grid
/// goes back to the table.
#[test]
fn a_grid_with_more_rows_than_the_table_holds_is_refused() {
    let click = Handlers {
        cell: "CONTINUE",
        click: "GRID-1::AddRow()\nGRID-1::SetCellValue(4, 2, \"Fourth\")\nGRID-1::AddRow()\nGRID-1::SetCellValue(5, 2, \"Fifth\")\nSET WS-STATUS TO GRID-1::Save()\nDISPLAY \"SAVE \" WS-STATUS\nDISPLAY \"N4 \" T-NAME(4)",
    };
    let ran = run_setup(keyless_setup(), &[("AutoCommit", "false")], &click, None, &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l == "SAVE 92"), "{:?}", ran.display);
    assert!(ran.display.iter().any(|l| l.trim_end() == "N4 Bolt"), "the table kept its row: {:?}", ran.display);
    assert!(ran.last("LastError").unwrap().contains("room for 4"), "{:?}", ran.last("LastError"));
    assert_eq!(ran.last("Rows").unwrap().lines().count(), 3, "the grid shows the table again");
}

/// The transaction statements and the `UPDATE`s of a run, in the order they were
/// sent — what a save puts on the wire, without the setup's `CREATE`s and `SELECT`s.
fn writes(ran: &Run) -> Vec<String> {
    // Not the form's own setup: everything after the grid loaded its rows.
    let loaded = ran.sql.iter().position(|s| s.starts_with("SELECT id")).map_or(0, |at| at + 1);
    ran.sql[loaded..]
        .iter()
        .filter(|s| ["BEGIN", "UPDATE", "INSERT", "DELETE", "COMMIT", "ROLLBACK", "SAVEPOINT", "RELEASE"].iter().any(|k| s.starts_with(k)))
        .cloned()
        .collect()
}

/// AC3: one `BEGIN`, one `UPDATE` per changed row (carrying all that row's
/// columns), one `COMMIT`.
#[test]
fn a_save_sends_begin_one_update_per_row_and_commit() {
    let mut rows = sql_rows();
    let edits = vec![edit_cell(&mut rows, 1, 2, "Bolt2"), edit_cell(&mut rows, 1, 3, "11"), edit_cell(&mut rows, 3, 2, "Wing")];
    let save = Handlers {
        cell: "CONTINUE",
        click: "SET WS-STATUS TO GRID-1::Save()\nDISPLAY \"SAVE \" WS-STATUS",
    };
    let ran = run_many(sql_setup(), &[("AutoCommit", "false")], &save, &edits, &[("BTN", "onClick")]);
    assert_eq!(
        writes(&ran),
        [
            "BEGIN",
            "UPDATE items SET name = 'Bolt2', qty = 11 WHERE id = 1",
            "UPDATE items SET name = 'Wing' WHERE id = 3",
            "COMMIT"
        ]
    );
    assert!(ran.display.iter().any(|l| l == "SAVE 00"), "{:?}", ran.display);
    assert_eq!(ran.last("SavedRows"), Some("2"));
    assert!(ran.display.iter().any(|l| l == "SAVED 2"), "{:?}", ran.display);
    println!("\n  save, SQL: BEGIN, 2 UPDATEs (one per row, 3 cells), COMMIT; SavedRows 2; onSaved once\n");
}

/// AC3: a constraint failure on the second row leaves the first row's value
/// unchanged in the table — one transaction, rolled back.
#[test]
fn a_failing_second_row_leaves_the_first_unchanged() {
    let mut setup = sql_setup();
    // (`CONTINUE` ends the `DISPLAY` above it: a bare method call after one would be another operand.)
    setup.init.push_str("\n           CONTINUE\n           SQL-1::Execute(\"CREATE UNIQUE INDEX items_name ON items(name)\")");
    let mut rows = sql_rows();
    // Row 1's quantity is fine; row 2 wants the name row 1 already has.
    let edits = vec![edit_cell(&mut rows, 1, 3, "99"), edit_cell(&mut rows, 2, 2, "Bolt")];
    let save = Handlers {
        cell: "CONTINUE",
        click: "SET WS-STATUS TO GRID-1::CommitEdit()\nDISPLAY \"SAVE \" WS-STATUS\nCONTINUE\nSQL-1::Query(\"SELECT qty FROM items WHERE id = 1\")\nSET WS-ROW TO SQL-1::Fetch()\nDISPLAY \"ROW \" WS-ROW",
    };
    let ran = run_many(setup, &[("AutoCommit", "false")], &save, &edits, &[("BTN", "onClick")]);
    assert_eq!(
        writes(&ran),
        [
            "BEGIN",
            "UPDATE items SET qty = 99 WHERE id = 1",
            "UPDATE items SET name = 'Bolt' WHERE id = 2",
            "ROLLBACK"
        ]
    );
    assert!(ran.display.iter().any(|l| l == "SAVE 90"), "{:?}", ran.display);
    assert_eq!(the_row(&ran), "10", "row 1 still holds its old quantity");
    assert_eq!(ran.last("_FailedCells"), Some("2,2"));
    assert_eq!(ran.last("_PendingCells"), Some(""), "the old values are back");
    assert_eq!(ran.last("Rows"), Some("1\tBolt\t10\n2\tNut\t20\n3\tO'Brien\t30"), "both cells show what the table holds");
    assert!(!ran.display.iter().any(|l| l.starts_with("SAVED")), "{:?}", ran.display);
    println!("\n  save, SQL, rolled back: BEGIN, UPDATE ok, UPDATE fails (unique), ROLLBACK; row 1 still 10; both cells back to the table's values\n");
}

/// The Risk of the spec: a transaction the *program* began is **joined** under a
/// savepoint — the save does not begin or end one of its own, and the program's
/// `Commit()` is what makes the write permanent.
#[test]
fn a_save_joins_the_programs_transaction_and_leaves_the_commit_to_it() {
    let mut setup = sql_setup();
    setup.init.push_str("\n           CONTINUE\n           SQL-1::BeginTransaction()");
    let mut rows = sql_rows();
    let edit = edit_cell(&mut rows, 2, 3, "77");
    let save = Handlers {
        cell: "CONTINUE",
        click: "SET WS-STATUS TO GRID-1::Save()\nDISPLAY \"SAVE \" WS-STATUS\nCONTINUE\nSQL-1::Rollback()\nSQL-1::Query(\"SELECT qty FROM items WHERE id = 2\")\nSET WS-ROW TO SQL-1::Fetch()\nDISPLAY \"ROW \" WS-ROW",
    };
    let ran = run_setup(setup, &[("AutoCommit", "false")], &save, Some(edit), &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l == "SAVE 00"), "{:?}", ran.display);
    assert_eq!(
        writes(&ran),
        ["BEGIN", "SAVEPOINT prc_save", "UPDATE items SET qty = 77 WHERE id = 2", "RELEASE SAVEPOINT prc_save", "ROLLBACK"],
        "no BEGIN or COMMIT of the save's own: {:?}",
        ran.sql
    );
    assert_eq!(the_row(&ran), "20", "the program rolled its transaction back, and the save's write went with it");
    println!("\n  save, SQL, inside BeginTransaction(): SAVEPOINT .. UPDATE .. RELEASE, no COMMIT of its own; the program's Rollback() took the write away\n");
}

/// A save that fails inside the program's transaction undoes **its own** writes and
/// nothing else: what the program wrote before it is still there to commit.
#[test]
fn a_failed_save_inside_the_programs_transaction_undoes_only_its_own_writes() {
    let mut setup = sql_setup();
    setup.init.push_str("\n           CONTINUE\n           SQL-1::Execute(\"CREATE UNIQUE INDEX items_name ON items(name)\")\n           SQL-1::BeginTransaction()\n           SQL-1::Execute(\"UPDATE items SET qty = 5 WHERE id = 3\")");
    let mut rows = sql_rows();
    let edits = vec![edit_cell(&mut rows, 1, 3, "99"), edit_cell(&mut rows, 2, 2, "Bolt")];
    let save = Handlers {
        cell: "CONTINUE",
        click: "SET WS-STATUS TO GRID-1::Save()\nDISPLAY \"SAVE \" WS-STATUS\nCONTINUE\nSQL-1::Commit()\nSQL-1::Query(\"SELECT qty FROM items WHERE id = 3\")\nSET WS-ROW TO SQL-1::Fetch()\nDISPLAY \"ROW \" WS-ROW",
    };
    let ran = run_many(setup, &[("AutoCommit", "false")], &save, &edits, &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l == "SAVE 90"), "{:?}", ran.display);
    assert_eq!(the_row(&ran), "5", "the program's own update, made before the save, survived it and was committed");
    let log = writes(&ran);
    assert!(log.contains(&"ROLLBACK TO SAVEPOINT prc_save".to_owned()) && !log.contains(&"ROLLBACK".to_owned()), "{log:?}");
    println!("\n  save, SQL, fails inside BeginTransaction(): ROLLBACK TO SAVEPOINT only; the program's earlier write (qty 5) committed\n");
}

/// Q5: a connection in `Async` mode is refused with `95`.
#[test]
fn an_async_connection_is_refused() {
    let mut rows = sql_rows();
    let edit = edit_cell(&mut rows, 2, 3, "77");
    let save = Handlers {
        cell: "CONTINUE",
        click: "MOVE \"Async\" TO SQL-1::Mode\nSET WS-STATUS TO GRID-1::CommitEdit()\nDISPLAY \"SAVE \" WS-STATUS",
    };
    let ran = run_setup(sql_setup(), &[("AutoCommit", "false")], &save, Some(edit), &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l == "SAVE 95"), "{:?}", ran.display);
    assert!(writes(&ran).is_empty(), "{:?}", writes(&ran));
    assert_eq!(ran.last("_PendingCells"), Some("2,3"));
    println!("\n  save, SQL: a connection in Async mode -> 95, nothing sent\n");
}

/// A connection that was never opened cannot take a save: `90`, edits kept.
#[test]
fn a_connection_that_is_not_open_answers_90() {
    let mut setup = sql_setup();
    setup.init = String::new();
    let mut rows = sql_rows();
    let edit = edit_cell(&mut rows, 2, 3, "77");
    let save = Handlers { cell: "CONTINUE", click: "SET WS-STATUS TO GRID-1::CommitEdit()\nDISPLAY \"SAVE \" WS-STATUS" };
    let ran = run_setup(setup, &[("AutoCommit", "false")], &save, Some(edit), &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l == "SAVE 90"), "{:?}", ran.display);
    assert_eq!(ran.last("_PendingCells"), Some(""), "the old value is back");
    assert!(ran.last("LastError").unwrap().contains("not open"), "{:?}", ran.last("LastError"));
}

/// R11: `CommitEdit()` and `Save()` are one action — the same pending set gives
/// the same file, the same status and the same count.
#[test]
fn commit_edit_and_save_give_the_same_result() {
    let results: Vec<_> = ["CommitEdit", "Save"]
        .iter()
        .map(|method| {
            let w = world(3);
            let edits = three_edits(&w, "Bob De Niro");
            let click = format!("SET WS-STATUS TO GRID-1::{method}()\nDISPLAY \"SAVE \" WS-STATUS");
            let handlers = Handlers { cell: "CONTINUE", click: Box::leak(click.into_boxed_str()) };
            let ran = run_many(Setup::indexed(&w), &[("AutoCommit", "false")], &handlers, &edits, &[("BTN", "onClick")]);
            (w.rows_in_file(), ran.display.clone(), ran.last("CommitStatus").map(str::to_owned), ran.last("SavedRows").map(str::to_owned))
        })
        .collect();
    assert_eq!(results[0], results[1], "the two names do one thing");
    assert_eq!(results[1].3.as_deref(), Some("2"));
    println!("\n  CommitEdit() and Save() on the same 3 pending edits: identical file, display, status 00, SavedRows 2\n");
}

/// Spec 095 R13/R19: a Checkbox cell's toggle reaches the runtime as the same
/// `onCellEdited` report a typed edit makes, so it is written back the same way.
/// A SQL flag loads as `0`/`1`; the value a click writes is the renderer's own
/// (`checkbox_next`), and a `boolean` field takes it.
#[test]
fn a_checkbox_toggle_on_a_bound_grid_is_written_back_like_a_typed_edit() {
    let mut setup = sql_setup();
    setup.init = [
        "           SET WS-H TO SQL-1::Open(\":memory:\")",
        "           SQL-1::Execute(\"CREATE TABLE items (id INTEGER PRIMARY KEY, done INTEGER)\")",
        "           SQL-1::Execute(\"INSERT INTO items VALUES (1, 0), (2, 1)\")",
        "           SET WS-STATUS TO GRID-1::RefreshBinding()",
    ]
    .join("\n");
    setup.seed.retain(|(k, _)| !["_BindingSql", "_BindingFields", "_BindingFieldTypes"].contains(&k.as_str()));
    setup.seed.push(("_BindingSql".into(), "SELECT id, done FROM items ORDER BY id".into()));
    setup.seed.push(("_BindingFields".into(), "id\ndone".into()));
    setup.seed.push(("_BindingFieldTypes".into(), "number,boolean".into()));
    let mut rows = vec![vec!["1".to_owned(), "0".to_owned()], vec!["2".to_owned(), "1".to_owned()]];
    // What a click on row 1's box writes: the renderer's own next value for "0".
    let toggled = cobolt_forms::datagrid::checkbox_next("0", "", "");
    assert_eq!(toggled, "1", "an unchecked 0 becomes the 1 of its pair");
    let edit = edit_cell(&mut rows, 1, 2, &toggled);
    let ran = run_setup(setup, &[], &read_back_flag(1), Some(edit), &[("BTN", "onClick")]);
    assert_eq!(the_row(&ran), "1", "the database holds the toggled flag");
    assert_eq!(writes(&ran), ["BEGIN", "UPDATE items SET done = 1 WHERE id = 1", "COMMIT"], "{:?}", ran.sql);
    assert_eq!(ran.last("CommitStatus"), Some("00"));
    println!("\n  checkbox toggle on a SQL boolean: renderer value {toggled:?} -> UPDATE items SET done = 1, read back 1\n");
}

/// The click handler: read the flag of row `id` back from the database.
fn read_back_flag(id: u32) -> Handlers {
    Handlers {
        cell: "CONTINUE",
        click: Box::leak(
            format!("SQL-1::Query(\"SELECT done FROM items WHERE id = {id}\")\nSET WS-ROW TO SQL-1::Fetch()\nDISPLAY \"ROW \" WS-ROW").into_boxed_str(),
        ),
    }
}

/// R9: a save with nothing pending opens no transaction, answers `00`, writes no
/// row and raises no `onSaved`.
#[test]
fn an_empty_save_opens_no_transaction() {
    let save = Handlers { cell: "CONTINUE", click: "SET WS-STATUS TO GRID-1::Save()\nDISPLAY \"SAVE \" WS-STATUS" };
    let ran = run_setup(sql_setup(), &[("AutoCommit", "false")], &save, None, &[("BTN", "onClick")]);
    assert!(writes(&ran).is_empty(), "no BEGIN, no COMMIT: {:?}", writes(&ran));
    assert!(ran.display.iter().any(|l| l == "SAVE 00"), "{:?}", ran.display);
    assert_eq!(ran.last("SavedRows"), Some("0"));
    assert!(!ran.display.iter().any(|l| l.starts_with("SAVED")), "{:?}", ran.display);
    println!("\n  empty Save(): 00, SavedRows 0, no BEGIN sent, no onSaved\n");
}

/// R10: what `AutoCommit` writes after a handler is one unit — here the edits the
/// grid had kept while it was off, plus the one whose handler turned it on.
#[test]
fn auto_commit_writes_the_edits_kept_so_far_as_one_unit() {
    let w = world(3);
    let edits = three_edits(&w, "Bob De Niro");
    let handlers = Handlers {
        cell: "IF GRID-1::EditedRow = 3\n    MOVE \"true\" TO GRID-1::AutoCommit\nEND-IF",
        click: "CONTINUE",
    };
    let ran = run_many(Setup::indexed(&w), &[("AutoCommit", "false")], &handlers, &edits, &[]);
    let expected = vec![person(&w, 1, "Leonardo D.", "12345"), person(&w, 2, "Joe Pesci", "12000000"), person(&w, 3, "Bob De Niro", "20000000")];
    assert_eq!(w.rows_in_file(), expected);
    assert_eq!(ran.last("SavedRows"), Some("2"), "one unit, two records");
    assert_eq!(ran.display.iter().filter(|l| l.starts_with("SAVED")).count(), 1, "onSaved once: {:?}", ran.display);
    println!("\n  AutoCommit turned on by the 3rd edit's handler: the 3 kept edits are written as one unit (2 records, onSaved once)\n");
}

/// A grid that is not writable keeps nothing, so a save has nothing to do.
#[test]
fn a_save_on_a_read_only_grid_writes_nothing() {
    let w = world(3);
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 2, 2, "Nobody");
    let save = Handlers { cell: "CONTINUE", click: "SET WS-STATUS TO GRID-1::Save()\nDISPLAY \"SAVE \" WS-STATUS" };
    let ran = run(&w, &[("_BindingWritable", "0")], &save, Some(edit), &[("BTN", "onClick")]);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]));
    assert!(ran.display.iter().any(|l| l == "SAVE 00"), "{:?}", ran.display);
    assert_eq!(ran.last("SavedRows"), Some("0"));
}

// ── Spec 095 §8.1: Save() writes inserts and deletes, and checks first ───────

/// What a grid row looks like for a person who is not in [`PEOPLE`].
fn newcomer(w: &World, id: &str, name: &str, salary: &str) -> Vec<String> {
    row(&w.fields, &encode(&w.fields, id, name, salary))
}

/// A click handler that runs `statements` and then `Save()`, showing how it went.
fn then_save(statements: &str) -> Handlers {
    let click = format!("{statements}\nSET WS-STATUS TO GRID-1::Save()\nDISPLAY \"SAVE \" WS-STATUS");
    Handlers { cell: "CONTINUE", click: Box::leak(click.into_boxed_str()) }
}

/// The statements that add a row to the grid: id, name, salary.
fn add_row(row: usize, id: &str, name: &str, salary: &str) -> String {
    format!("GRID-1::AddRow()\nGRID-1::SetCellValue({row}, 1, \"{id}\")\nGRID-1::SetCellValue({row}, 2, \"{name}\")\nGRID-1::SetCellValue({row}, 3, \"{salary}\")")
}

/// D3: an explicit `Save()` writes the rows added and the rows removed, with the cells
/// edited, in one unit: record 2 is deleted, record 1 renamed, a newcomer added.
#[test]
fn a_save_deletes_updates_and_inserts_in_one_unit() {
    let w = world(3);
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 1, 2, "Leo");
    let click = format!("GRID-1::DeleteRow(2)\n{}", add_row(3, "4", "Al Pacino", "5000000"));
    let ran = run(&w, &[("AutoCommit", "false")], &then_save(Box::leak(click.into_boxed_str())), Some(edit), &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l == "SAVE 00"), "{:?}", ran.display);
    assert_eq!(
        w.rows_in_file(),
        vec![person(&w, 1, "Leo", "30000000"), person(&w, 3, "Robert De Niro", "20000000"), newcomer(&w, "4", "Al Pacino", "5000000")],
        "record 2 gone, record 1 renamed, record 4 added"
    );
    assert_eq!(ran.last("SavedRows"), Some("3"), "one delete, one update, one insert");
    assert_eq!(ran.display.iter().filter(|l| l.starts_with("SAVED")).count(), 1, "onSaved once: {:?}", ran.display);
    assert_eq!(ran.last("_PendingCells"), Some(""));
    println!("\n  save, indexed: DeleteRow + an edit + AddRow -> record 2 deleted, 1 updated, 4 inserted; SavedRows 3; onSaved once\n");
}

/// Adding and deleting rows is for the explicit save: `AutoCommit` settles cell edits only.
#[test]
fn nothing_inserts_or_deletes_until_save_is_called() {
    let w = world(3);
    let click = format!("GRID-1::DeleteRow(1)\n{}", add_row(3, "4", "Al Pacino", "5000000"));
    let handlers = Handlers { cell: "CONTINUE", click: Box::leak(click.into_boxed_str()) };
    // An ordinary edit as well, so the settle after the handler has something to write.
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 3, 2, "Roberto");
    let ran = run(&w, &[], &handlers, Some(edit), &[("BTN", "onClick"), ("BTN", "onClick")]);
    let mut expected = w.grid_rows(&[1, 2, 3]);
    expected[2][1] = "Roberto".into();
    assert_eq!(w.rows_in_file(), expected, "the edit was written; the delete and the insert were not");
    assert_eq!(ran.last("CommitStatus"), Some("00"));
}

/// A blank line is never inserted.
#[test]
fn a_blank_line_is_never_inserted() {
    let w = world(3);
    // Two lines the operator opened and left empty (a lone space is still blank).
    let blanks = "GRID-1::AddRow()\nGRID-1::SetCellValue(4, 3, \" \")\nGRID-1::AddRow()\nGRID-1::SetCellValue(5, 3, \" \")";
    let ran = run(&w, &[], &then_save(blanks), None, &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l == "SAVE 00"), "{:?}", ran.display);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]));
    assert_eq!(ran.last("SavedRows"), Some("0"));
    assert!(!ran.display.iter().any(|l| l.starts_with("SAVED")), "{:?}", ran.display);
}

/// A new row needs its key; a key the file already has fails the whole unit and the
/// grid goes back to what the file holds — the deleted row returns, the added one goes.
#[test]
fn a_new_row_needs_its_key_and_a_duplicate_key_undoes_the_whole_unit() {
    let w = world(3);
    // No key typed.
    let ran = run(&w, &[], &then_save("GRID-1::AddRow()\nGRID-1::SetCellValue(4, 2, \"Nobody\")"), None, &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l == "SAVE 92"), "{:?}", ran.display);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]));
    assert_eq!(ran.last("Rows").unwrap(), text(&w.grid_rows(&[1, 2, 3])), "the added row is gone again");

    // Key 2 is taken; record 1 was deleted in the same unit.
    let click = format!("GRID-1::DeleteRow(1)\n{}", add_row(3, "2", "Twin", "1"));
    let ran = run(&w, &[], &then_save(Box::leak(click.into_boxed_str())), None, &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l == "SAVE 22"), "FILE STATUS 22, duplicate key: {:?}", ran.display);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]), "the delete was rolled back with the insert");
    assert_eq!(ran.last("Rows").unwrap(), text(&w.grid_rows(&[1, 2, 3])), "the deleted row is back, the added one gone");
    assert_eq!(ran.display.iter().filter(|l| l.starts_with("ERROR")).count(), 1, "{:?}", ran.display);
    println!("\n  save, indexed: a new row with no key -> 92; a duplicate key -> 22 with the delete rolled back; the grid restored both times\n");
}

/// A row whose key cells were changed is never updated, deleted or inserted.
#[test]
fn a_row_whose_key_was_changed_is_never_written() {
    let w = world(3);
    // Changing the key of a loaded row is neither an insert of a new key nor a delete of the old.
    let ran = run(&w, &[], &then_save("GRID-1::SetCellValue(1, 1, \"000000077\")"), None, &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l == "SAVE 00"), "{:?}", ran.display);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]));
    assert_eq!(ran.last("SavedRows"), Some("0"));

    // …and an edit of another cell of that row does not write it either.
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 1, 2, "Leo");
    let ran = run(&w, &[("AutoCommit", "false")], &then_save("GRID-1::SetCellValue(1, 1, \"000000077\")"), Some(edit), &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l == "SAVE 21"), "{:?}", ran.display);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]));
    println!("\n  save, indexed: a loaded row whose key was changed is not inserted, not deleted, and its edit is refused (21)\n");
}

/// `ClearRows()` empties the display; it is not a request to delete the table.
#[test]
fn clear_rows_is_not_a_delete() {
    let w = world(3);
    let ran = run(&w, &[], &then_save("GRID-1::ClearRows()"), None, &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l == "SAVE 00"), "{:?}", ran.display);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]));
    assert_eq!(ran.last("SavedRows"), Some("0"));
}

/// Q4: a row someone else changed since the grid loaded it is not overwritten.
#[test]
fn a_row_changed_by_someone_else_is_not_overwritten() {
    let w = world(3);
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 2, 2, "Joseph Pesci");
    let ran = run_many_with(Setup::indexed(&w), &[], &Handlers::NONE, &[edit], &[], &|| w.rename_record(2, "Someone Else"));
    assert_eq!(ran.last("CommitStatus"), Some("96"));
    assert!(ran.last("LastError").unwrap().contains("ACTOR-NAME"), "{:?}", ran.last("LastError"));
    let mut expected = w.grid_rows(&[1, 2, 3]);
    expected[1][1] = "Someone Else".into();
    assert_eq!(w.rows_in_file(), expected, "the other person's name stands");
    assert_eq!(ran.last("_FailedCells"), Some("2,2"));
    assert_eq!(ran.last("Rows").unwrap(), text(&w.grid_rows(&[1, 2, 3])), "the old value is back (RefreshBinding() shows the new one)");
    println!("\n  save, indexed: record 2 renamed by someone else after the load -> 96, their name kept, nothing written\n");
}

/// Q4: a change to a *different* column of the same record survives the save.
#[test]
fn a_change_to_another_column_of_the_row_survives() {
    let w = world(3);
    let mut rows = w.grid_rows(&[1, 2, 3]);
    let edit = edit_cell(&mut rows, 2, 2, "Joseph Pesci");
    let ran = run_many_with(Setup::indexed(&w), &[], &Handlers::NONE, &[edit], &[], &|| w.reprice_record(2, "99"));
    assert_eq!(ran.last("CommitStatus"), Some("00"));
    assert_eq!(w.rows_in_file()[1], person(&w, 2, "Joseph Pesci", "99"), "the new name and the other person's salary");
}

/// Q4: a row someone else changed is not deleted either.
#[test]
fn a_row_changed_by_someone_else_is_not_deleted() {
    let w = world(3);
    let save = then_save("GRID-1::DeleteRow(2)");
    let ran = run_many_with(Setup::indexed(&w), &[("AutoCommit", "false")], &save, &[], &[("BTN", "onClick")], &|| w.reprice_record(2, "1"));
    assert!(ran.display.iter().any(|l| l == "SAVE 96"), "{:?}", ran.display);
    assert_eq!(w.rows_in_file()[1], person(&w, 2, "Joe Pesci", "1"), "the record is still there, as the other person left it");
    assert_eq!(ran.last("Rows").unwrap(), text(&w.grid_rows(&[1, 2, 3])), "the deleted row is back in the grid");
}

/// D3 over a SQL table: `DELETE`, `UPDATE`, `INSERT`, in that order, between one `BEGIN`
/// and one `COMMIT`; the rows are read back and compared first.
#[test]
fn a_sql_save_deletes_updates_and_inserts_in_one_transaction() {
    let mut rows = sql_rows();
    let edit = edit_cell(&mut rows, 1, 3, "11");
    let click = format!(
        "GRID-1::DeleteRow(2)\n{}",
        "GRID-1::AddRow()\nGRID-1::SetCellValue(3, 1, \"4\")\nGRID-1::SetCellValue(3, 2, \"Washer\")\nGRID-1::SetCellValue(3, 3, \"40\")"
    );
    let ran = run_setup(sql_setup(), &[("AutoCommit", "false")], &then_save(Box::leak(click.into_boxed_str())), Some(edit), &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l == "SAVE 00"), "{:?}", ran.display);
    assert_eq!(
        writes(&ran),
        [
            "BEGIN",
            "DELETE FROM items WHERE id = 2",
            "UPDATE items SET qty = 11 WHERE id = 1",
            "INSERT INTO items (id, name, qty) VALUES (4, 'Washer', 40)",
            "COMMIT"
        ]
    );
    assert_eq!(ran.last("SavedRows"), Some("3"));
    // The rows were read back first, each by its key.
    let selects: Vec<&String> = ran.sql.iter().filter(|s| s.starts_with("SELECT") && s.contains("WHERE")).collect();
    assert_eq!(selects.len(), 2, "one read for the delete, one for the update: {selects:?}");
    println!("\n  save, SQL: BEGIN, DELETE, UPDATE, INSERT, COMMIT -- with each changed or deleted row read back first; SavedRows 3\n");
}

/// Q4 over SQL: a row someone else changed since the load fails the unit (`96`) and
/// nothing is written; the grid goes back to the loaded values.
#[test]
fn a_sql_row_changed_by_someone_else_fails_the_save() {
    let mut rows = sql_rows();
    let edit = edit_cell(&mut rows, 2, 2, "Joseph");
    // The program changes the row behind the grid's back, then saves.
    let click = then_save("SQL-1::Execute(\"UPDATE items SET name = 'Zed' WHERE id = 2\")");
    let ran = run_setup(sql_setup(), &[("AutoCommit", "false")], &click, Some(edit), &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l == "SAVE 96"), "{:?}", ran.display);
    let log = writes(&ran);
    assert_eq!(log.last().map(String::as_str), Some("ROLLBACK"), "{log:?}");
    assert!(!log.iter().any(|s| s.contains("'Joseph'")), "the edit was never sent: {log:?}");
    assert!(ran.last("LastError").unwrap().contains("name"), "{:?}", ran.last("LastError"));
    assert_eq!(ran.last("_FailedCells"), Some("2,2"));
    assert_eq!(ran.last("Rows"), Some("1\tBolt\t10\n2\tNut\t20\n3\tO'Brien\t30"), "the old value is back");
    // A row someone changed is not deleted either.
    let click = then_save("SQL-1::Execute(\"UPDATE items SET qty = 5 WHERE id = 2\")\nGRID-1::DeleteRow(2)");
    let ran = run_setup(sql_setup(), &[("AutoCommit", "false")], &click, None, &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l == "SAVE 96"), "{:?}", ran.display);
    assert!(!writes(&ran).iter().any(|s| s.starts_with("DELETE")), "the delete was never sent: {:?}", writes(&ran));
    assert_eq!(ran.last("Rows"), Some("1\tBolt\t10\n2\tNut\t20\n3\tO'Brien\t30"), "the deleted row is back");
}

/// A new row needs its key; a duplicate key undoes the whole unit, delete included.
#[test]
fn a_sql_insert_needs_its_key_and_a_duplicate_undoes_the_whole_unit() {
    let ran = run_setup(
        sql_setup(),
        &[("AutoCommit", "false")],
        &then_save("GRID-1::AddRow()\nGRID-1::SetCellValue(4, 2, \"Nobody\")"),
        None,
        &[("BTN", "onClick")],
    );
    assert!(ran.display.iter().any(|l| l == "SAVE 92"), "{:?}", ran.display);
    assert!(!writes(&ran).iter().any(|s| s.starts_with("INSERT")), "nothing was sent: {:?}", writes(&ran));

    let click = then_save("GRID-1::DeleteRow(1)\nGRID-1::AddRow()\nGRID-1::SetCellValue(3, 1, \"2\")\nGRID-1::SetCellValue(3, 2, \"Twin\")\nGRID-1::SetCellValue(3, 3, \"1\")");
    let ran = run_setup(sql_setup(), &[("AutoCommit", "false")], &click, None, &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l.starts_with("SAVE 90")), "a duplicate primary key: {:?}", ran.display);
    let log = writes(&ran);
    assert_eq!(log.first().map(String::as_str), Some("BEGIN"));
    assert_eq!(log.last().map(String::as_str), Some("ROLLBACK"), "{log:?}");
    assert_eq!(ran.last("Rows"), Some("1\tBolt\t10\n2\tNut\t20\n3\tO'Brien\t30"), "the deleted row is back, the added one gone");
}

/// A loaded row whose key was changed, and `ClearRows()`, send nothing.
#[test]
fn a_sql_key_change_and_clear_rows_send_nothing() {
    for statements in ["GRID-1::SetCellValue(1, 1, \"99\")", "GRID-1::ClearRows()"] {
        let ran = run_setup(sql_setup(), &[("AutoCommit", "false")], &then_save(statements), None, &[("BTN", "onClick")]);
        assert!(ran.display.iter().any(|l| l == "SAVE 00"), "{statements}: {:?}", ran.display);
        assert!(writes(&ran).is_empty(), "{statements}: {:?}", writes(&ran));
        assert_eq!(ran.last("SavedRows"), Some("0"));
    }
}

/// A grid its binding never loaded shows whatever the designer or the program put there:
/// `Save()` has nothing to compare it with, so it writes nothing, and `DiscardEdit()`
/// has nothing to go back to, so it leaves the rows alone.
#[test]
fn a_grid_its_binding_never_loaded_is_neither_saved_nor_discarded() {
    let w = world(3);
    let rows = text(&w.grid_rows(&[1, 2, 3]));
    let unloaded = || {
        let mut setup = Setup::indexed(&w);
        setup.init = "           DISPLAY \"LOADED\"".into();
        setup
    };
    let handlers = then_save("GRID-1::AddRow()\nGRID-1::SetCellValue(4, 1, \"9\")\nGRID-1::SetCellValue(4, 2, \"Sample\")\nSET WS-STATUS TO GRID-1::DiscardEdit()");
    let ran = run_setup(unloaded(), &[("Rows", &rows)], &handlers, None, &[("BTN", "onClick")]);
    assert!(ran.display.iter().any(|l| l == "SAVE 00"), "{:?}", ran.display);
    assert_eq!(w.rows_in_file(), w.grid_rows(&[1, 2, 3]), "the sample row was not inserted");
    assert!(ran.last("Rows").map_or(true, |shown| shown.lines().count() >= 3), "the rows were not wiped: {:?}", ran.last("Rows"));
}

// ── The report (Golden Rule 7) ───────────────────────────────────────────────

/// The five cases of spec 094 AC10 over an indexed file, run through the same
/// helpers as the tests above, with one result block at the end: what was
/// exercised, how long it took, and the tally. (A console COBOL program has no
/// event loop to carry a grid's report, so the block is printed from here.)
#[test]
fn write_back_report() {
    let started = std::time::Instant::now();
    let mut cases: Vec<(&str, bool, u128)> = Vec::new();
    let mut case = |name: &'static str, check: &dyn Fn() -> bool| {
        let t = std::time::Instant::now();
        let ok = check();
        cases.push((name, ok, t.elapsed().as_millis()));
    };

    case("auto-commit on: the edit is in the file", &|| {
        let w = world(3);
        let mut rows = w.grid_rows(&[1, 2, 3]);
        let edit = edit_cell(&mut rows, 2, 2, "Joseph Pesci");
        let ran = run(&w, &[], &Handlers::NONE, Some(edit), &[]);
        w.rows_in_file()[1][1] == "Joseph Pesci" && ran.last("CommitStatus") == Some("00")
    });
    case("auto-commit off + confirm: written by CommitEdit()", &|| {
        let w = world(3);
        let mut rows = w.grid_rows(&[1, 2, 3]);
        let edit = edit_cell(&mut rows, 3, 2, "Bob De Niro");
        let confirm = Handlers { cell: "SET WS-STATUS TO GRID-1::CommitEdit()", click: "CONTINUE" };
        run(&w, &[("AutoCommit", "false")], &confirm, Some(edit), &[]);
        w.rows_in_file()[2][1] == "Bob De Niro"
    });
    case("auto-commit off + discard: the file never changed", &|| {
        let w = world(3);
        let mut rows = w.grid_rows(&[1, 2, 3]);
        let edit = edit_cell(&mut rows, 1, 2, "Somebody Else");
        let discard = Handlers { cell: "SET WS-STATUS TO GRID-1::DiscardEdit()", click: "CONTINUE" };
        let ran = run(&w, &[("AutoCommit", "false")], &discard, Some(edit), &[]);
        w.rows_in_file() == w.grid_rows(&[1, 2, 3]) && ran.last("Rows").is_some_and(|r| r.contains("Leonardo DiCaprio"))
    });
    case("a value too long for its PICTURE: refused (92)", &|| {
        let w = world(3);
        let mut rows = w.grid_rows(&[1, 2, 3]);
        let edit = edit_cell(&mut rows, 2, 2, "A name that is far longer than twenty characters");
        let ran = run(&w, &[], &Handlers::NONE, Some(edit), &[]);
        w.rows_in_file() == w.grid_rows(&[1, 2, 3]) && ran.last("CommitStatus") == Some("92")
    });
    case("a record that is gone: not written to another (23)", &|| {
        let w = world(3);
        let mut rows = w.grid_rows(&[1, 2, 3]);
        let edit = edit_cell(&mut rows, 3, 2, "Ghost");
        let ran = run_many_with(Setup::indexed(&w), &[], &Handlers::NONE, &[edit], &[], &|| w.delete_record(3));
        w.rows_in_file() == w.grid_rows(&[1, 2]) && ran.last("CommitStatus") == Some("23")
    });

    let failed = cases.iter().filter(|(_, ok, _)| !ok).count();
    println!("\n  DATAGRID WRITE-BACK -- indexed file, 3 records, one edited cell per case");
    for (name, ok, ms) in &cases {
        println!("    {} {name:<58} {ms:>4} ms", if *ok { "PASS" } else { "FAIL" });
    }
    println!("  TESTS RUN      : {:04}", cases.len());
    println!("  TESTS FAILED   : {failed:04}");
    println!("  ELAPSED        : {} ms", started.elapsed().as_millis());
    println!("  OVERALL RESULT : {}\n", if failed == 0 { "PASS" } else { "FAIL" });
    assert_eq!(failed, 0);
}

/// A file of `how_many` synthetic records (ids 1 to `how_many`), for the timing cases.
fn big_world(how_many: usize) -> World {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("actors.idx");
    let cidx = dir.path().join("actors.cidx");
    let (def, fields) = definition(data.to_str().unwrap());
    cobolt_indexed::save_indexed(&cidx, &def).expect("write .cidx");
    let mut file = DiskIndexedFile::new(&data, 38, KeySpec { parts: Vec::new(), offset: 0, len: 9, duplicates: false }, Vec::new());
    assert_eq!(file.open(OpenMode::Output), status::OK);
    for n in 1..=how_many {
        assert_eq!(file.write(&encode(&fields, &n.to_string(), &format!("Actor {n}"), "1000")), status::OK);
    }
    assert_eq!(file.close(), status::OK);
    World { cidx: cidx.to_str().unwrap().to_owned(), dir, data, fields, def }
}

/// The grid's rows for [`big_world`]: record `n` on line `n`.
fn big_rows(w: &World, how_many: usize) -> Vec<Vec<String>> {
    (1..=how_many).map(|n| row(&w.fields, &encode(&w.fields, &n.to_string(), &format!("Actor {n}"), "1000"))).collect()
}

/// One `Save()` of `edited` renamed rows in a file of `records`: the milliseconds
/// the whole run took (the edits delivered one event at a time, then the save),
/// and whether exactly those records changed.
fn timed_save(records: usize, edited: usize) -> (u128, bool) {
    let w = big_world(records);
    let mut rows = big_rows(&w, records);
    let edits: Vec<Edit> = (1..=edited).map(|n| edit_cell(&mut rows, n, 2, &format!("Renamed {n}"))).collect();
    let save = Handlers { cell: "CONTINUE", click: "SET WS-STATUS TO GRID-1::Save()" };
    let started = std::time::Instant::now();
    let ran = run_many(Setup::indexed(&w), &[("AutoCommit", "false")], &save, &edits, &[("BTN", "onClick")]);
    let ms = started.elapsed().as_millis();
    let in_file = w.rows_in_file();
    let right = in_file.len() == records
        && ran.last("SavedRows") == Some(edited.to_string().as_str())
        && in_file.iter().enumerate().all(|(i, r)| r[1] == if i < edited { format!("Renamed {}", i + 1) } else { format!("Actor {}", i + 1) });
    (ms, right)
}

/// Spec 095 AC12 (Golden Rule 7): the cases of a save, over every source, with
/// one result block at the end — what was exercised, how long it took, the
/// tally. (A console COBOL program has no event loop to carry a grid's report,
/// so the block is printed from here, as 094's is.)
#[test]
fn save_report() {
    let started = std::time::Instant::now();
    let mut cases: Vec<(&str, bool, u128)> = Vec::new();
    let mut case = |name: &'static str, check: &dyn Fn() -> bool| {
        let t = std::time::Instant::now();
        let ok = check();
        cases.push((name, ok, t.elapsed().as_millis()));
    };
    let save_click = |method: &str| -> Handlers {
        let click = format!("SET WS-STATUS TO GRID-1::{method}()\nDISPLAY \"SAVE \" WS-STATUS");
        Handlers { cell: "CONTINUE", click: Box::leak(click.into_boxed_str()) }
    };

    case("indexed: 3 cells in 2 records are one save (SavedRows 2)", &|| {
        let w = world(3);
        let ran = run_many(Setup::indexed(&w), &[("AutoCommit", "false")], &save_click("Save"), &three_edits(&w, "Bob De Niro"), &[("BTN", "onClick")]);
        w.rows_in_file() == vec![person(&w, 1, "Leonardo D.", "12345"), person(&w, 2, "Joe Pesci", "12000000"), person(&w, 3, "Bob De Niro", "20000000")]
            && ran.last("SavedRows") == Some("2")
            && ran.display.iter().filter(|l| l.starts_with("SAVED")).count() == 1
    });
    case("indexed: a bad value in record 3 writes neither record (92)", &|| {
        let w = world(3);
        let ran = run_many(Setup::indexed(&w), &[("AutoCommit", "false")], &save_click("Save"), &three_edits(&w, "A name that is far longer than twenty characters"), &[("BTN", "onClick")]);
        w.rows_in_file() == w.grid_rows(&[1, 2, 3])
            && ran.last("CommitStatus") == Some("92")
            && ran.last("_PendingCells") == Some("")
            && ran.last("_FailedCells") == Some("3,2")
    });
    case("indexed: a key column edited fails the whole save (21)", &|| {
        let w = world(3);
        let mut rows = w.grid_rows(&[1, 2, 3]);
        let edits = vec![edit_cell(&mut rows, 1, 2, "Fine Name"), edit_cell(&mut rows, 2, 1, "000000099")];
        let ran = run_many(Setup::indexed(&w), &[("AutoCommit", "false")], &save_click("Save"), &edits, &[("BTN", "onClick")]);
        w.rows_in_file() == w.grid_rows(&[1, 2, 3]) && ran.last("CommitStatus") == Some("21")
    });
    case("indexed: a Sort between the edit and the save (right record)", &|| {
        let w = world(3);
        let mut rows = w.grid_rows(&[1, 2, 3]);
        let edit = edit_cell(&mut rows, 3, 2, "Bob De Niro");
        let click = Handlers { cell: "CONTINUE", click: "GRID-1::Sort(2)\nSET WS-STATUS TO GRID-1::Save()" };
        run(&w, &[("AutoCommit", "false")], &click, Some(edit), &[("BTN", "onClick")]);
        w.rows_in_file() == vec![person(&w, 1, "Leonardo DiCaprio", "30000000"), person(&w, 2, "Joe Pesci", "12000000"), person(&w, 3, "Bob De Niro", "20000000")]
    });
    case("SQL: BEGIN, one UPDATE per row, COMMIT", &|| {
        let mut rows = sql_rows();
        let edits = vec![edit_cell(&mut rows, 1, 2, "Bolt2"), edit_cell(&mut rows, 1, 3, "11"), edit_cell(&mut rows, 3, 2, "Wing")];
        let ran = run_many(sql_setup(), &[("AutoCommit", "false")], &save_click("Save"), &edits, &[("BTN", "onClick")]);
        writes(&ran) == ["BEGIN", "UPDATE items SET name = 'Bolt2', qty = 11 WHERE id = 1", "UPDATE items SET name = 'Wing' WHERE id = 3", "COMMIT"]
            && ran.last("SavedRows") == Some("2")
    });
    case("SQL: the second row fails -> ROLLBACK, the first is unchanged", &|| {
        let mut setup = sql_setup();
        setup.init.push_str("\n           CONTINUE\n           SQL-1::Execute(\"CREATE UNIQUE INDEX items_name ON items(name)\")");
        let mut rows = sql_rows();
        let edits = vec![edit_cell(&mut rows, 1, 3, "99"), edit_cell(&mut rows, 2, 2, "Bolt")];
        let click = Handlers {
            cell: "CONTINUE",
            click: "SET WS-STATUS TO GRID-1::Save()\nCONTINUE\nSQL-1::Query(\"SELECT qty FROM items WHERE id = 1\")\nSET WS-ROW TO SQL-1::Fetch()\nDISPLAY \"ROW \" WS-ROW",
        };
        let ran = run_many(setup, &[("AutoCommit", "false")], &click, &edits, &[("BTN", "onClick")]);
        writes(&ran).last().map(String::as_str) == Some("ROLLBACK") && the_row(&ran) == "10" && ran.last("CommitStatus") == Some("90")
    });
    case("SQL: inside the program's transaction the save joins it (SAVEPOINT)", &|| {
        let mut setup = sql_setup();
        setup.init.push_str("\n           CONTINUE\n           SQL-1::BeginTransaction()");
        let mut rows = sql_rows();
        let edit = edit_cell(&mut rows, 2, 3, "77");
        let ran = run_setup(setup, &[("AutoCommit", "false")], &save_click("Save"), Some(edit), &[("BTN", "onClick")]);
        ran.display.iter().any(|l| l == "SAVE 00")
            && writes(&ran) == ["BEGIN", "SAVEPOINT prc_save", "UPDATE items SET qty = 77 WHERE id = 2", "RELEASE SAVEPOINT prc_save"]
    });
    case("SQL: a connection in Async mode is refused (95)", &|| {
        let mut rows = sql_rows();
        let edit = edit_cell(&mut rows, 2, 3, "77");
        let click = Handlers { cell: "CONTINUE", click: "MOVE \"Async\" TO SQL-1::Mode\nSET WS-STATUS TO GRID-1::Save()\nDISPLAY \"SAVE \" WS-STATUS" };
        let ran = run_setup(sql_setup(), &[("AutoCommit", "false")], &click, Some(edit), &[("BTN", "onClick")]);
        ran.display.iter().any(|l| l == "SAVE 95") && writes(&ran).is_empty()
    });
    case("COBOL table: one non-numeric value stores none of the save (92)", &|| {
        let mut rows = table_rows();
        let edits = vec![edit_cell(&mut rows, 1, 2, "Screw"), edit_cell(&mut rows, 2, 3, "lots")];
        let click = Handlers {
            cell: "CONTINUE",
            click: "SET WS-STATUS TO GRID-1::Save()\nDISPLAY \"SAVE \" WS-STATUS\nDISPLAY \"NAME1 \" T-NAME(1)\nDISPLAY \"QTY2 \" T-QTY(2)",
        };
        let ran = run_many(table_setup(), &[("AutoCommit", "false")], &click, &edits, &[("BTN", "onClick")]);
        ran.display.iter().any(|l| l == "SAVE 92") && ran.display.iter().any(|l| l == "NAME1 Bolt") && ran.display.iter().any(|l| l == "QTY2 020")
    });
    case("COBOL table, AutoCommit on: the edit stores itself", &|| {
        let mut rows = table_rows();
        let edit = edit_cell(&mut rows, 2, 3, "25");
        let ran = run_setup(table_setup(), &[], &Handlers { cell: "CONTINUE", click: "DISPLAY \"QTY2 \" T-QTY(2)" }, Some(edit), &[("BTN", "onClick")]);
        qty2(&ran) == "QTY2 025"
    });
    case("COBOL table: regenerated from the grid after a Sort (Anchor first)", &|| {
        let mut rows = keyless_rows();
        let edit = edit_cell(&mut rows, 3, 2, "Anchor");
        let click = Handlers { cell: "CONTINUE", click: "GRID-1::Sort(2)\nSET WS-STATUS TO GRID-1::Save()\nDISPLAY \"N1 \" T-NAME(1)" };
        let ran = run_setup(keyless_setup(), &[("AutoCommit", "false")], &click, Some(edit), &[("BTN", "onClick")]);
        ran.display.iter().any(|l| l == "N1 Anchor")
    });
    case("an empty Save(): 00, no BEGIN sent, no onSaved", &|| {
        let ran = run_setup(sql_setup(), &[("AutoCommit", "false")], &save_click("Save"), None, &[("BTN", "onClick")]);
        writes(&ran).is_empty() && ran.display.iter().any(|l| l == "SAVE 00") && !ran.display.iter().any(|l| l.starts_with("SAVED"))
    });
    case("CommitEdit() and Save(): the same file, status and count", &|| {
        let results: Vec<_> = ["CommitEdit", "Save"]
            .iter()
            .map(|method| {
                let w = world(3);
                let ran = run_many(Setup::indexed(&w), &[("AutoCommit", "false")], &save_click(method), &three_edits(&w, "Bob De Niro"), &[("BTN", "onClick")]);
                (w.rows_in_file(), ran.last("CommitStatus").map(str::to_owned), ran.last("SavedRows").map(str::to_owned))
            })
            .collect();
        results[0] == results[1]
    });

    // How long a save takes: one renamed row, then fifty, in a file of 1,098 records
    // (the PowerDemo3 actors file's size). Whole run, edits delivered included.
    let mut timings = Vec::new();
    for edited in [1, 50] {
        let (ms, right) = timed_save(1098, edited);
        timings.push((edited, ms));
        let name = if edited == 1 { "indexed, 1,098 records: 1 renamed row saved" } else { "indexed, 1,098 records: 50 renamed rows saved" };
        cases.push((name, right, ms));
    }

    let failed = cases.iter().filter(|(_, ok, _)| !ok).count();
    println!("\n  DATAGRID SAVE -- one transaction per save, over an indexed file, a SQL table (in-memory SQLite) and a COBOL table");
    for (name, ok, ms) in &cases {
        println!("    {} {name:<66} {ms:>5} ms", if *ok { "PASS" } else { "FAIL" });
    }
    println!("  Timing (whole run: the edits delivered one event at a time, then the save):");
    for (edited, ms) in &timings {
        println!("    {edited:>3} edited row(s) of 1,098 records : {ms} ms");
    }
    println!("  TESTS RUN      : {:04}", cases.len());
    println!("  TESTS FAILED   : {failed:04}");
    println!("  ELAPSED        : {} ms", started.elapsed().as_millis());
    println!("  OVERALL RESULT : {}\n", if failed == 0 { "PASS" } else { "FAIL" });
    assert_eq!(failed, 0);
}
