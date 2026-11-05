// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-014 T11 (R8): `status = "<group>"` in `exports.toml` names the group of the program's `RETURNING` item that reports a business
//! failure. The loader refuses a status that names no group, or a group that is not laid out as `WS-STATUS`.

use cobolt_server::backend::Backend;

fn program(returning_body: &str) -> String {
    format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. PAY.\n       DATA DIVISION.\n       LINKAGE SECTION.\n       01 PAY-REQ.\n          05 AMOUNT PIC 9(4).\n       01 PAY-RESULT.\n{returning_body}       PROCEDURE DIVISION USING PAY-REQ RETURNING PAY-RESULT.\n           GOBACK.\n"
    )
}

const GOOD: &str = "          05 REF PIC X(8).\n          05 WS-STATUS.\n             10 WS-STATUS-CODE PIC 9(4).\n             10 WS-STATUS-MESSAGE PIC X(200).\n             10 WS-STATUS-DETAIL PIC X(1000).\n";

fn load(tag: &str, returning_body: &str, status: &str) -> Result<Backend, Vec<String>> {
    let dir = std::env::temp_dir().join(format!("prc-status-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src").join("pay.cbl"), program(returning_body)).unwrap();
    std::fs::write(dir.join("exports.toml"), format!("[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"PAY\"\n{status}\n")).unwrap();
    let r = Backend::load(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    r
}

#[test]
fn a_status_group_of_the_right_shape_is_on_the_contract() {
    let b = load("ok", GOOD, "status = \"WS-STATUS\"").unwrap_or_else(|e| panic!("{e:#?}"));
    let sig = &b.interface.programs[0];
    println!("PAY reports failures in the status group {:?}", sig.status);
    assert_eq!(sig.status.as_deref(), Some("WS-STATUS"));
    let none = load("none", GOOD, "").unwrap();
    assert_eq!(none.interface.programs[0].status, None, "no status declared: none on the contract");
}

#[test]
fn status_naming_no_group_is_refused_with_program_and_item() {
    let e = load("nogroup", GOOD, "status = \"WS-OOPS\"").err().expect("refused").join("\n");
    println!("{e}");
    assert!(e.contains("export PAY") && e.contains("WS-OOPS") && e.contains("PAY-RESULT"), "{e}");
}

#[test]
fn status_of_the_wrong_shape_is_refused() {
    let wrong = "          05 WS-STATUS.\n             10 WS-STATUS-CODE PIC 9(3).\n             10 WS-STATUS-MESSAGE PIC X(200).\n             10 WS-STATUS-DETAIL PIC X(1000).\n";
    let e = load("shape", wrong, "status = \"WS-STATUS\"").err().expect("refused").join("\n");
    println!("{e}");
    assert!(e.contains("export PAY") && e.contains("not laid out as WS-STATUS") && e.contains("PAY-RESULT"), "{e}");
}

/// The scaffolded copybook is what a program copies into its RETURNING record (the text is the scaffold's; the
/// project-tools test keeps the constant itself).
#[test]
fn the_scaffolded_copybook_copied_into_a_returning_record_is_a_valid_status_group() {
    let dir = std::env::temp_dir().join(format!("prc-status-copy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::create_dir_all(dir.join("COPYBOOKS")).unwrap();
    std::fs::write(
        dir.join("COPYBOOKS").join("WS-STATUS.cpy"),
        "           05 WS-STATUS.\n               10 WS-STATUS-CODE                   PIC 9(4)    VALUE 0.\n               10 WS-STATUS-MESSAGE                PIC X(200)  VALUE SPACES.\n               10 WS-STATUS-DETAIL                 PIC X(1000) VALUE SPACES.\n",
    )
    .unwrap();
    let src = program("          05 REF PIC X(8).\n          COPY \"../COPYBOOKS/WS-STATUS\".\n");
    std::fs::write(dir.join("src").join("pay.cbl"), src).unwrap();
    std::fs::write(dir.join("exports.toml"), "[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"PAY\"\nstatus = \"WS-STATUS\"\n").unwrap();
    let r = Backend::load(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    let b = r.unwrap_or_else(|e| panic!("{e:#?}"));
    println!("COPY \"WS-STATUS\" inside RETURNING: status {:?}", b.interface.programs[0].status);
    assert_eq!(b.interface.programs[0].status.as_deref(), Some("WS-STATUS"));
}
