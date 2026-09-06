// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

use cobolt_codegen::generate;
use cobolt_forms::{
    BindingDataType, BindingField, BindingMode, BindingSourceDescriptor, BindingTargetDescriptor,
    BindingTargetPath, Control, ControlType, DataBindingDef, FieldMapping, Form,
};
use cobolt_indexed::{
    AccessMode, FieldUsage, IndexedDefinition, IndexedField, KeyDef, KeyEncodingDef,
    KeyOrderingDef, KeyPartDef, RecordFormatDef, StorageMode,
};
use cobolt_runtime::indexed::{status, KeySpec, OpenMode};
use cobolt_runtime::indexed_disk::DiskIndexedFile;
use cobolt_runtime::FormEvent;

fn run_capture(src: &str) -> Vec<String> {
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
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

#[test]
fn data_binding_runtime_initial_load_does_not_mark_dirty() {
    let out = run_capture(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BIND-LOAD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-STATUS PIC X(64) VALUE SPACES.
       01 WS-DIRTY  PIC 9 VALUE 9.
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-BINDING-LOAD" USING "BIND-1" WS-STATUS.
           CALL "COBOL-BINDING-POPULATE" USING "BIND-1" WS-STATUS.
           CALL "COBOL-BINDING-MARK-CLEAN" USING "BIND-1" WS-DIRTY.
           DISPLAY WS-STATUS.
           DISPLAY WS-DIRTY.
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["", "0"]);
}

#[test]
fn data_binding_runtime_writable_update_preserves_identity_and_clears_dirty() {
    let out = run_capture(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BIND-WRITE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-STATUS PIC X(64) VALUE SPACES.
       01 WS-DIRTY  PIC 9 VALUE 0.
       01 WS-KEY    PIC X(16) VALUE "C001".
       01 WS-VALUE  PIC X(16) VALUE "Alice".
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-BINDING-SET-READ-ONLY" USING "BIND-1" "0".
           CALL "COBOL-BINDING-SET-PENDING" USING "BIND-1" WS-KEY WS-VALUE WS-DIRTY.
           DISPLAY WS-DIRTY.
           CALL "COBOL-BINDING-UPDATE" USING "BIND-1" WS-KEY WS-STATUS.
           CALL "COBOL-BINDING-MARK-CLEAN" USING "BIND-1" WS-DIRTY.
           DISPLAY WS-STATUS.
           DISPLAY WS-DIRTY.
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["1", "", "0"]);
}

#[test]
fn data_binding_runtime_read_only_never_writes_back() {
    let out = run_capture(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BIND-RO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-STATUS PIC X(64) VALUE SPACES.
       01 WS-DIRTY  PIC 9 VALUE 0.
       01 WS-KEY    PIC X(16) VALUE "C001".
       01 WS-VALUE  PIC X(16) VALUE "Alice".
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-BINDING-SET-READ-ONLY" USING "BIND-1" "1".
           CALL "COBOL-BINDING-SET-PENDING" USING "BIND-1" WS-KEY WS-VALUE WS-DIRTY.
           CALL "COBOL-BINDING-UPDATE" USING "BIND-1" WS-KEY WS-STATUS.
           DISPLAY WS-STATUS.
           DISPLAY WS-DIRTY.
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["READ-ONLY", "1"]);
}

#[test]
fn data_binding_runtime_failed_update_keeps_pending_edits_recoverable() {
    let out = run_capture(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BIND-FAIL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-STATUS PIC X(64) VALUE SPACES.
       01 WS-DIRTY  PIC 9 VALUE 0.
       01 WS-KEY    PIC X(16) VALUE SPACES.
       01 WS-VALUE  PIC X(16) VALUE "Alice".
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-BINDING-SET-READ-ONLY" USING "BIND-1" "0".
           CALL "COBOL-BINDING-SET-PENDING" USING "BIND-1" WS-KEY WS-VALUE WS-DIRTY.
           CALL "COBOL-BINDING-UPDATE" USING "BIND-1" WS-KEY WS-STATUS.
           DISPLAY WS-STATUS.
           DISPLAY WS-DIRTY.
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["MISSING-ROW-KEY", "1"]);
}

// ── IndexedFile-sourced DataGrid binding ────────────────────────────────────
//
// A DataGrid bound to an `IndexedFile` source used to never populate: codegen
// only seeded `_BindingKind`/`_BindingFields` (what `refresh_datagrid_binding`
// needs to write `Rows`) for a `CobolTable` source, so an `IndexedFile`
// binding's grid never got past an empty registry entry. These mirror
// `datagrid_refresh_binding_updates_rows_from_cobol_table` (in
// `cobolt_runtime::interpreter`'s own `#[cfg(test)]` module) but end to end:
// codegen builds the real `COBOL-DATA-BINDINGS-LOAD`/`-POPULATE` paragraphs
// from an actual `DataBindingDef`, the parser accepts the generated source,
// and the interpreter runs it — the same path a `.cfrm` saved by the Designer
// goes through via `rcrun run-form`.

fn actor_leaf(name: &str, pic: &str, offset: u32, length: u32) -> cobolt_indexed::IndexedField {
    cobolt_indexed::IndexedField {
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

/// `ACTOR-ID PIC 9(9)` (key) / `ACTOR-NAME PIC X(20)` / `ACTOR-SALARY PIC
/// 9(7)V99` — a smaller stand-in for PowerDemo3's `ACTORS-RECORD` (the form
/// that surfaced this bug), covering the same field categories: a numeric
/// key, plain text and a `V`-scaled numeric. Record length 38.
fn actors_definition(assign_path: &str) -> (IndexedDefinition, [IndexedField; 3]) {
    let id = actor_leaf("ACTOR-ID", "9(9)", 0, 9);
    let name = actor_leaf("ACTOR-NAME", "X(20)", 9, 20);
    let salary = actor_leaf("ACTOR-SALARY", "9(7)V99", 29, 9);
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
        parts: vec![KeyPartDef {
            field_name: "ACTOR-ID".into(),
            offset: 0,
            length: 9,
            encoding: KeyEncodingDef::Bytes,
        }],
        duplicates_allowed: false,
        ordering: KeyOrderingDef::Ascending,
    };
    def.fields = vec![root];
    def.finalized = true;
    (def, [id, name, salary])
}

fn encode_actor(fields: &[IndexedField; 3], id: &str, name: &str, salary: &str) -> Vec<u8> {
    let mut rec = vec![b' '; 38];
    rec[0..9]
        .copy_from_slice(&cobolt_indexed::encode_field_display(&fields[0], id, 9).unwrap());
    rec[9..29]
        .copy_from_slice(&cobolt_indexed::encode_field_display(&fields[1], name, 20).unwrap());
    rec[29..38]
        .copy_from_slice(&cobolt_indexed::encode_field_display(&fields[2], salary, 9).unwrap());
    rec
}

/// A `DataGrid` bound to `IndexedFile` at `cidx_path` — same shape the
/// Designer's data-binding editor saves (see PowerDemo3's
/// `datagrid-form.cfrm`), read-only like that binding, with one `GridColumn`
/// mapping per field.
fn indexed_binding_form(cidx_path: &str, grid_id: &str) -> Form {
    let mut form = Form::new("BIND-FORM", "Bindings", 800, 600);
    form.add_control(Control::new(grid_id, ControlType::DataGrid, 0, 0));

    let fields = vec![
        BindingField::new("ACTOR-ID", BindingDataType::Integer).key(),
        BindingField::new("ACTOR-NAME", BindingDataType::Text).required(),
        BindingField::new("ACTOR-SALARY", BindingDataType::Decimal).required(),
    ];
    let mappings: Vec<FieldMapping> = fields
        .iter()
        .map(|f| {
            FieldMapping::new(
                f.name.clone(),
                BindingTargetPath::GridColumn {
                    control_id: grid_id.to_owned(),
                    column_id: f.name.clone(),
                },
            )
        })
        .collect();

    let mut binding = DataBindingDef::new(
        "BIND-ACTORS-GRID",
        "Actors Grid",
        BindingSourceDescriptor::IndexedFile {
            definition_path: cidx_path.to_owned(),
            record_name: "ACTOR-RECORD".into(),
            fields,
            key_field: Some("ACTOR-ID".into()),
            writable: false,
        },
        BindingTargetDescriptor::DataGrid {
            control_id: grid_id.to_owned(),
        },
    )
    .with_mappings(mappings);
    binding.mode = BindingMode::ReadOnly;
    form.data_bindings.push(binding);
    form
}

/// Generate, parse and run `form`, returning the LAST `Rows` value published
/// for `grid_id` (panics if none was ever published — every path through
/// `refresh_indexed_file_binding`, success or graceful failure, sets `Rows`).
fn run_and_capture_rows(form: &Form, grid_id: &str) -> String {
    let src = generate(form);
    let parsed = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}\n{}",
        parsed.diagnostics,
        src
    );
    let program = parsed.program.expect("generated program should parse");
    let (event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, _display_rx) = mpsc::channel();
    // A generated form program unconditionally ends in `PERFORM
    // COBOL-EVENT-LOOP`, which calls `COBOL-WAIT-EVENT`; with a real
    // `event_rx` attached (needed so `state_tx` can be wired up too, since
    // that field has no other public setter) this genuinely blocks on
    // `recv()`. Nothing here drives a UI, so queue the same "window closed"
    // sentinel a real form send on close — queued before `run()` starts, so
    // the very first `COBOL-WAIT-EVENT` sees it immediately and the event
    // loop exits at once. `COBOL-DATA-BINDINGS-POPULATE` (and so the `Rows`
    // publish under test) already ran by then — it is unconditionally ahead
    // of `COBOL-EVENT-LOOP` in the generated `COBOL-MAIN`.
    event_tx.send(FormEvent::quit()).expect("queue quit event");
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("generated binding form should run");
    // Control ids are canonicalised to uppercase in the object registry
    // (`ActorGrid` publishes as `ACTORGRID`), same as every other COBOL
    // identifier in this codebase — compare case-insensitively rather than
    // assume the seed's own spelling survives.
    state_rx
        .try_iter()
        .filter(|update| update.ctrl_id.eq_ignore_ascii_case(grid_id) && update.prop == "Rows")
        .map(|update| update.value)
        .last()
        .expect("COBOL-DATA-BINDINGS-POPULATE should publish Rows for an IndexedFile binding")
}

#[test]
fn data_binding_indexed_file_populates_grid_rows_from_disk() {
    let dir = tempfile::tempdir().unwrap();
    let data_path = dir.path().join("actors.idx");
    let cidx_path = dir.path().join("actors.cidx");

    let (def, fields) = actors_definition(data_path.to_str().unwrap());
    cobolt_indexed::save_indexed(&cidx_path, &def).expect("write .cidx");

    let primary = KeySpec {
        offset: 0,
        len: 9,
        duplicates: false,
    };
    let mut file = DiskIndexedFile::new(&data_path, 38, primary, Vec::new());
    assert_eq!(file.open(OpenMode::Output), status::OK);
    assert_eq!(
        file.write(&encode_actor(&fields, "1", "Leonardo DiCaprio", "30000000")),
        status::OK
    );
    assert_eq!(
        file.write(&encode_actor(&fields, "2", "Joe Pesci", "12000000")),
        status::OK
    );
    assert_eq!(file.close(), status::OK);

    // What the shared `.cidx` PIC formatter actually produces for these same
    // bytes — the grid must show exactly this, not a hand re-derived string:
    // cell formatting (e.g. no decimal point re-inserted at `V`) is owned by
    // `cobolt_indexed::format_field_display`, not by this binding.
    let row = |id: &str, name: &str, salary: &str| {
        let rec = encode_actor(&fields, id, name, salary);
        [
            cobolt_indexed::format_field_display(&fields[0], &rec[0..9]),
            cobolt_indexed::format_field_display(&fields[1], &rec[9..29]),
            cobolt_indexed::format_field_display(&fields[2], &rec[29..38]),
        ]
        .join("\t")
    };
    let expected = format!(
        "{}\n{}",
        row("1", "Leonardo DiCaprio", "30000000"),
        row("2", "Joe Pesci", "12000000")
    );

    let form = indexed_binding_form(cidx_path.to_str().unwrap(), "ActorGrid");
    let rows = run_and_capture_rows(&form, "ActorGrid");
    assert_eq!(rows, expected);
    assert_eq!(
        rows.lines().count(),
        2,
        "one row per record, in primary-key order"
    );
}

#[test]
fn data_binding_indexed_file_empty_file_yields_zero_rows_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let data_path = dir.path().join("actors.idx");
    let cidx_path = dir.path().join("actors.cidx");

    let (def, _fields) = actors_definition(data_path.to_str().unwrap());
    cobolt_indexed::save_indexed(&cidx_path, &def).expect("write .cidx");

    // An indexed file that exists on disk but holds zero records — the
    // legitimate "nothing written yet" case FILE STATUS 23/35 covers, not a
    // fault. Created the same way a plain `OPEN OUTPUT ... CLOSE` would leave
    // it.
    let primary = KeySpec {
        offset: 0,
        len: 9,
        duplicates: false,
    };
    let mut file = DiskIndexedFile::new(&data_path, 38, primary, Vec::new());
    assert_eq!(file.open(OpenMode::Output), status::OK);
    assert_eq!(file.close(), status::OK);

    let form = indexed_binding_form(cidx_path.to_str().unwrap(), "ActorGrid");
    let rows = run_and_capture_rows(&form, "ActorGrid");
    assert_eq!(rows, "", "zero records must yield an empty grid, not an error");
}

#[test]
fn data_binding_indexed_file_missing_data_file_yields_zero_rows_not_an_error() {
    // The `.cidx` exists (the Designer saved the binding) but nothing has
    // written the actual data file yet — a fresh project, before any record
    // is ever created. Must not create the file as a side effect of a grid
    // render (see `refresh_indexed_file_binding`'s `data_path.exists()`
    // guard) and must not fail.
    let dir = tempfile::tempdir().unwrap();
    let data_path = dir.path().join("actors.idx");
    let cidx_path = dir.path().join("actors.cidx");

    let (def, _fields) = actors_definition(data_path.to_str().unwrap());
    cobolt_indexed::save_indexed(&cidx_path, &def).expect("write .cidx");

    let form = indexed_binding_form(cidx_path.to_str().unwrap(), "ActorGrid");
    let rows = run_and_capture_rows(&form, "ActorGrid");
    assert_eq!(
        rows, "",
        "a not-yet-created data file must yield an empty grid, not an error"
    );
    assert!(
        !data_path.exists(),
        "a read-only grid refresh must never create the data file as a side effect"
    );
}

#[test]
fn data_binding_indexed_file_resolves_designer_paths_against_the_project_anchor() {
    // The regression PowerDemo3's `datagrid-form.cfrm` hit: the Designer
    // stores BOTH paths project-relative — the `.cidx` in the binding and the
    // data file in the `.cidx`'s own `assign-path` — and the runtime resolved
    // them against the PROCESS WORKING DIRECTORY. The IDE spawns `rcrun
    // run-form` with its own directory and a built application runs from
    // `bin/`, so neither is ever the project root: the grid that reads
    // perfectly in the Indexed File Browser (which has always resolved
    // against the project) came up empty. Every other test here uses absolute
    // paths, which is exactly why none of them caught it.
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(project.path().join("indexed")).unwrap();
    std::fs::create_dir_all(project.path().join("data")).unwrap();

    // Stored exactly as the Designer and the `.cidx` editor store them.
    let stored_cidx = "indexed/actors.cidx";
    let stored_assign = "data/actors.idx";
    let cidx_path = project.path().join(stored_cidx);
    let data_path = project.path().join(stored_assign);

    let (def, fields) = actors_definition(stored_assign);
    cobolt_indexed::save_indexed(&cidx_path, &def).expect("write .cidx");

    let primary = KeySpec {
        offset: 0,
        len: 9,
        duplicates: false,
    };
    let mut file = DiskIndexedFile::new(&data_path, 38, primary, Vec::new());
    assert_eq!(file.open(OpenMode::Output), status::OK);
    assert_eq!(
        file.write(&encode_actor(&fields, "1", "Leonardo DiCaprio", "30000000")),
        status::OK
    );
    assert_eq!(
        file.write(&encode_actor(&fields, "2", "Joe Pesci", "12000000")),
        status::OK
    );
    assert_eq!(file.close(), status::OK);

    // The whole point of the test: neither stored path can be found from the
    // working directory this test runs in. If the rows arrive, they arrived
    // through the anchor.
    assert!(
        !std::path::Path::new(stored_cidx).exists(),
        "the stored .cidx path must not resolve against the test's own cwd, \
         or this test proves nothing"
    );

    cobolt_forms::assets::set_base(project.path());

    let row = |id: &str, name: &str, salary: &str| {
        let rec = encode_actor(&fields, id, name, salary);
        [
            cobolt_indexed::format_field_display(&fields[0], &rec[0..9]),
            cobolt_indexed::format_field_display(&fields[1], &rec[9..29]),
            cobolt_indexed::format_field_display(&fields[2], &rec[29..38]),
        ]
        .join("\t")
    };
    let expected = format!(
        "{}\n{}",
        row("1", "Leonardo DiCaprio", "30000000"),
        row("2", "Joe Pesci", "12000000")
    );

    let form = indexed_binding_form(stored_cidx, "ActorGrid");
    let rows = run_and_capture_rows(&form, "ActorGrid");
    assert_eq!(
        rows, expected,
        "a project-relative binding must populate from the anchored project, \
         not from the process working directory"
    );
}
