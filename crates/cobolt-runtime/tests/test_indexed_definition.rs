// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! An indexed file carries the `FD` that describes its records.
//!
//! A COBOL program creates the file; the record definition is read back from
//! the file alone — no `.cidx`, no source. What that definition says is what a
//! program that has never seen the FD needs: every field's name, PICTURE and
//! USAGE, the REDEFINES, the condition names, and the bytes each one takes.

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::indexed::IndexedEngine;
use cobolt_runtime::indexed_import::{definition_from_inspect, inspect_any_path};
use cobolt_runtime::Interpreter;

fn run(src: &str, engine: Option<IndexedEngine>) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("a program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    if let Some(e) = engine {
        interp.set_indexed_engine(e);
    }
    interp.run().expect("the program runs");
    display_rx.try_iter().collect()
}

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("prc-def-{tag}-{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A program that creates `path` with the customer FD below and writes two records.
fn creator(path: &Path, storage: &str) -> String {
    format!(
        r#"IDENTIFICATION DIVISION.
PROGRAM-ID. MK.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT CUST ASSIGN TO "{path}"
        ORGANIZATION IS INDEXED
        ACCESS MODE IS DYNAMIC
        RECORD KEY IS CUST-ID
        ALTERNATE RECORD KEY IS CUST-NAME WITH DUPLICATES
        {storage}
        FILE STATUS IS WS-FS.
DATA DIVISION.
FILE SECTION.
FD CUST.
01 CUST-REC.
    05 CUST-ID     PIC 9(6).
    05 CUST-NAME   PIC X(20).
    05 BALANCE     PIC S9(7)V99 COMP-3.
    05 BAL-TEXT    REDEFINES BALANCE PIC X(9).
    05 FLAG        PIC X.
        88 IS-VIP  VALUE "V".
    05 TAIL        PIC X(4).
WORKING-STORAGE SECTION.
01 WS-FS PIC XX.
PROCEDURE DIVISION.
MAIN-PARA.
    OPEN OUTPUT CUST
    MOVE 1 TO CUST-ID
    MOVE "ANA" TO CUST-NAME
    MOVE "V" TO FLAG
    WRITE CUST-REC
    MOVE 2 TO CUST-ID
    MOVE "BRUNO" TO CUST-NAME
    WRITE CUST-REC
    CLOSE CUST
    STOP RUN.
"#,
        path = path.display(),
        storage = storage
    )
}

/// What the customer FD's text has to say.
fn assert_customer_definition(text: &str) {
    for needle in [
        "FD CUST.\n",
        "01 CUST-REC. *> @0+40\n",
        "05 CUST-ID PIC 9(6). *> @0+6\n",
        "05 CUST-NAME PIC X(20). *> @6+20\n",
        "05 BALANCE PIC S9(7)V99 COMP-3. *> @26+9\n",
        "05 BAL-TEXT REDEFINES BALANCE PIC X(9). *> @26+9\n",
        "05 FLAG PIC X. *> @35+1\n",
        "88 IS-VIP VALUE \"V\".\n",
        "05 TAIL PIC X(4). *> @36+4\n",
    ] {
        assert!(text.contains(needle), "missing `{needle}` in:\n{text}");
    }
}

#[test]
fn a_file_a_program_creates_carries_the_fd_that_describes_its_records() {
    let dir = temp_dir("create");
    let path = dir.join("cust.idx");
    run(&creator(&path, ""), None);

    let info = inspect_any_path(&path).unwrap().expect("a container");
    let text = info.definition.expect("the file carries its definition");
    println!("{text}");
    assert_customer_definition(&text);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_memory_file_with_persistence_carries_it_too() {
    let dir = temp_dir("memory");
    let path = dir.join("cust.idx");
    run(&creator(&path, "STORAGE IS MEMORY WITH PERSISTENCE"), None);
    let text = inspect_any_path(&path).unwrap().unwrap().definition.expect("the definition");
    assert_customer_definition(&text);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_redb_file_carries_it_too() {
    let dir = temp_dir("redb");
    let path = dir.join("cust.idx");
    run(&creator(&path, ""), Some(IndexedEngine::Redb));
    let text = cobolt_runtime::indexed_redb::RedbIndexedFile::definition_of(&path).expect("the definition");
    assert_customer_definition(&text);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The import reads the field tree out of the file: names, PICTUREs, usage,
/// REDEFINES and bytes, and names the keys after their fields.
#[test]
fn the_import_names_every_field_from_the_file_alone() {
    let dir = temp_dir("import");
    let path = dir.join("cust.idx");
    run(&creator(&path, ""), None);
    let info = inspect_any_path(&path).unwrap().unwrap();
    let def = definition_from_inspect("CUST", &dir, &path, &info);

    let root = def.fields.first().expect("a record");
    assert_eq!(root.name, "CUST-REC");
    let by_name = |n: &str| root.children.iter().find(|f| f.name == n).unwrap_or_else(|| panic!("no {n}"));
    let id = by_name("CUST-ID");
    assert_eq!((id.pic.as_str(), id.offset, id.length), ("9(6)", Some(0), Some(6)));
    let bal = by_name("BALANCE");
    assert_eq!((bal.pic.as_str(), bal.usage, bal.offset, bal.length), ("S9(7)V99", cobolt_indexed::FieldUsage::Comp3, Some(26), Some(9)));
    let text = by_name("BAL-TEXT");
    assert_eq!(text.redefines.as_deref(), Some("BALANCE"));
    assert_eq!((text.offset, text.length), (Some(26), Some(9)));
    assert!(root.children.iter().all(|f| f.name != "IS-VIP"), "a condition name is no field");
    assert_eq!(root.children.len(), 6, "{:?}", root.children.iter().map(|f| &f.name).collect::<Vec<_>>());

    assert_eq!(def.keys.primary.parts[0].field_name, "CUST-ID", "the key is named after its field");
    assert_eq!(def.keys.alternates[0].parts[0].field_name, "CUST-NAME");
    assert_eq!(def.record_length(), 40);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A file with no definition — the key schema is all it has — still imports, as
/// it did before: the keys named, every other byte a `FILLER`.
#[test]
fn a_file_without_a_definition_still_imports_from_its_keys() {
    use cobolt_runtime::indexed::KeySpec;
    use cobolt_runtime::indexed_disk::DiskIndexedFile;
    let dir = temp_dir("legacy");
    let path = dir.join("old.idx");
    let key = KeySpec { parts: Vec::new(), offset: 0, len: 5, duplicates: false };
    let mut f = DiskIndexedFile::new(&path, 15, key, Vec::new());
    assert_eq!(f.open(cobolt_runtime::indexed::OpenMode::Output), "00");
    f.close();
    let info = inspect_any_path(&path).unwrap().unwrap();
    assert!(info.definition.is_none());
    let def = definition_from_inspect("OLD", &dir, &path, &info);
    assert_eq!(def.fields[0].name, "IMPORTED-RECORD");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A program that opens a file written before the definition existed converts it
/// when it opens it to write — and an `INPUT` open leaves it alone.
#[test]
fn a_program_converts_an_older_file_when_it_opens_it_for_writing() {
    use cobolt_runtime::indexed::{KeySpec, OpenMode};
    use cobolt_runtime::indexed_disk::DiskIndexedFile;
    let dir = temp_dir("convert");
    let path = dir.join("cust.idx");
    // A file with the customer's keys and 40-byte records, but no definition.
    let key = KeySpec { parts: Vec::new(), offset: 0, len: 6, duplicates: false };
    let alt = KeySpec { parts: Vec::new(), offset: 6, len: 20, duplicates: true };
    let mut f = DiskIndexedFile::new(&path, 40, key, vec![alt]);
    assert_eq!(f.open(OpenMode::Output), "00");
    let mut rec = b"000001ANA                 ".to_vec();
    rec.resize(40, b' ');
    assert_eq!(f.write(&rec), "00");
    f.close();
    assert!(inspect_any_path(&path).unwrap().unwrap().definition.is_none());

    let reader = format!(
        r#"IDENTIFICATION DIVISION.
PROGRAM-ID. RD.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT CUST ASSIGN TO "{path}" ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC
        RECORD KEY IS CUST-ID ALTERNATE RECORD KEY IS CUST-NAME WITH DUPLICATES FILE STATUS IS WS-FS.
DATA DIVISION.
FILE SECTION.
FD CUST.
01 CUST-REC.
    05 CUST-ID     PIC 9(6).
    05 CUST-NAME   PIC X(20).
    05 BALANCE     PIC S9(7)V99 COMP-3.
    05 BAL-TEXT    REDEFINES BALANCE PIC X(9).
    05 FLAG        PIC X.
        88 IS-VIP  VALUE "V".
    05 TAIL        PIC X(4).
WORKING-STORAGE SECTION.
01 WS-FS PIC XX.
PROCEDURE DIVISION.
MAIN-PARA.
    OPEN INPUT CUST
    CLOSE CUST
    STOP RUN.
"#,
        path = path.display()
    );
    let before = std::fs::read(&path).unwrap();
    run(&reader, None);
    assert_eq!(std::fs::read(&path).unwrap(), before, "an INPUT open changed the file");

    run(&reader.replace("OPEN INPUT CUST", "OPEN I-O CUST"), None);
    let text = inspect_any_path(&path).unwrap().unwrap().definition.expect("converted");
    assert_customer_definition(&text);
    // …with its record intact.
    let mut g = DiskIndexedFile::new(
        &path,
        40,
        KeySpec { parts: Vec::new(), offset: 0, len: 6, duplicates: false },
        vec![KeySpec { parts: Vec::new(), offset: 6, len: 20, duplicates: true }],
    );
    assert_eq!(g.open(OpenMode::Input), "00");
    assert!(matches!(g.read_seq(cobolt_runtime::indexed::ReadDir::Next), (Some(r), _) if r.starts_with(b"000001ANA")));
    g.close();
    let _ = std::fs::remove_dir_all(&dir);
}
