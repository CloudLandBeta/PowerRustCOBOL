// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T20: a Backend that runs as a module is made from its parts (the interface, the released interfaces, the
//! export table, the state mode, the slots), not from source. The parts a Build writes from a Backend built from source
//! make the same Backend back, and a table that does not hold together is refused in words.

use cobolt_server::backend::{Backend, ExportEntry, SlotDecl, StateMode};
use cobolt_server::exec::{execute, ExecOptions};
use cobolt_wire::contract::InterfaceDescription;
use cobolt_wire::error::codes;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

const EXPORTS: &str =
    "[contract]\nversion = \"1.2.0\"\n\n[[export]]\nprogram = \"ADD-TWO\"\nidempotent = true\n\n[[export]]\nprogram = \"GREET\"\nroles = [\"staff\"]\n";

const ADD: &str = "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. ADD-TWO.\n       DATA DIVISION.\n       LINKAGE SECTION.\n       01 REQ.\n          05 A PIC 9(4).\n          05 B PIC 9(4).\n       01 RES.\n          05 TOTAL PIC 9(5).\n       PROCEDURE DIVISION USING REQ RETURNING RES.\n       MAIN-PARA.\n           COMPUTE TOTAL = A + B.\n           GOBACK.\n";

const GREET: &str = "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. GREET.\n       DATA DIVISION.\n       LINKAGE SECTION.\n       01 WHO PIC X(10).\n       01 MSG PIC X(20).\n       PROCEDURE DIVISION USING WHO MSG.\n       MAIN-PARA.\n           STRING \"HELLO \" DELIMITED BY SIZE WHO DELIMITED BY SPACE INTO MSG.\n           GOBACK.\n";

fn folder() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-backend-parts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("exports.toml"), EXPORTS).unwrap();
    std::fs::write(dir.join("src").join("add.cbl"), ADD).unwrap();
    std::fs::write(dir.join("src").join("greet.cbl"), GREET).unwrap();
    dir
}

#[test]
fn the_parts_of_a_backend_built_from_source_make_the_same_backend_back() {
    let started = Instant::now();
    let dir = folder();
    let source = Backend::load(&dir).unwrap_or_else(|e| panic!("the test Backend does not load: {e:?}"));

    // What a Build writes: the interface as JSON, the table as JSON, the slots as JSON.
    let interface = InterfaceDescription::from_json(&source.interface.to_json()).unwrap();
    let entries: Vec<ExportEntry> = serde_json::from_str(&serde_json::to_string(&source.export_entries()).unwrap()).unwrap();
    let slots: Vec<SlotDecl> = Vec::new();
    let parts = Backend::from_parts(interface, Vec::new(), entries.clone(), StateMode::PerCall, slots).unwrap_or_else(|e| panic!("{e:?}"));

    assert_eq!(parts.interface, source.interface);
    let mut names = 0;
    for p in source.programs() {
        let q = parts.program(&p.sig.name).unwrap_or_else(|| panic!("{} is missing", p.sig.name));
        assert_eq!((&q.sig, &q.params, &q.returning), (&p.sig, &p.params, &p.returning), "{}", p.sig.name);
        assert!(p.program.is_some() && q.program.is_none(), "the source Backend has its source, the parts one has none");
        names += 1;
    }
    assert_eq!(names, 2);
    assert_eq!(
        entries.iter().map(|e| (e.name.as_str(), e.params.len(), e.returning.is_some())).collect::<Vec<_>>(),
        vec![("ADD-TWO", 1, true), ("GREET", 2, false)]
    );

    // The interpreter has no source for such a Backend and says so, as a fault of the program, not a panic.
    let args: Vec<String> = vec![r#"{"A":1,"B":2}"#.into()];
    let e = execute(parts.program("ADD-TWO").unwrap(), &args, ExecOptions::plain(Arc::new(AtomicBool::new(false)), Duration::from_secs(1)))
        .expect_err("no source to run");
    assert_eq!(e.code, codes::PROGRAM_FAULT);
    assert!(e.message.contains("no source"), "{}", e.message);
    println!("parts of {} programs read back equal (interface, signatures, parameter names, RETURNING); no source on the parts side, refused by the interpreter in words ({} ms)", names, started.elapsed().as_millis());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_table_that_does_not_hold_together_is_refused_in_words() {
    let dir = folder();
    let source = Backend::load(&dir).unwrap();
    let interface = source.interface.clone();
    let entries = source.export_entries();

    // A contract program with no entry, and an entry no contract describes, and one entry twice.
    let short = vec![entries[0].clone()];
    let e = Backend::from_parts(interface.clone(), Vec::new(), short, StateMode::PerCall, Vec::new()).err().expect("a missing entry is refused");
    assert!(e.iter().any(|m| m.contains("GREET") && m.contains("does not list")), "{e:?}");
    let mut stray = entries.clone();
    stray.push(ExportEntry { name: "STRAY".into(), identity: false, params: vec![], returning: None, control: None });
    let e = Backend::from_parts(interface.clone(), Vec::new(), stray, StateMode::PerCall, Vec::new()).err().expect("a stray entry is refused");
    assert!(e.iter().any(|m| m.contains("STRAY") && m.contains("does not describe")), "{e:?}");
    let mut twice = entries.clone();
    twice.push(entries[0].clone());
    let e = Backend::from_parts(interface.clone(), Vec::new(), twice, StateMode::PerCall, Vec::new()).err().expect("a repeated entry is refused");
    assert!(e.iter().any(|m| m.contains("twice")), "{e:?}");
    // An identity program is in no contract: its entry alone stands for it, and it is not reachable by name as an export.
    let mut with_identity = entries.clone();
    with_identity.push(ExportEntry { name: "AUTH-SIGN-IN".into(), identity: true, params: vec!["REQ".into()], returning: None, control: None });
    let b = Backend::from_parts(interface, Vec::new(), with_identity, StateMode::PerConnection, Vec::new()).unwrap();
    assert!(b.identity_program("AUTH-SIGN-IN").is_some() && b.program("AUTH-SIGN-IN").is_none());
    assert_eq!(b.state, StateMode::PerConnection);
    println!("a missing entry, a stray entry and a repeated entry refused in words; an identity entry reachable only as identity");
    let _ = std::fs::remove_dir_all(&dir);
}
