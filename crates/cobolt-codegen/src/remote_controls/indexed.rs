// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The backend end of an `IndexedFile` control on a web form (spec 111).
//!
//! On the web the file lives with the Backend. Every COBOL verb on it — `OPEN`,
//! `CLOSE`, `START`, `READ` (by key, `NEXT`, `PREVIOUS`), `WRITE`, `REWRITE`,
//! `DELETE` — is lowered by the web build (`cobolt_compiler::web_remote`) into a
//! `CALL … WITH WEBSOCKET LINKAGE` to one of the programs made here, one per
//! *(form, control, operation)*, named by [`cobolt_wire::controls::endpoint_name`].
//!
//! The programs keep nothing between calls (plan O4): each one opens the file,
//! does its verb and closes it, and the browser sends what the verb needs — the
//! record area (whose primary key is the key) and how to position
//! (`KEY-MODE`). The file, its `SELECT` and its `FD` are those of the control's
//! `.cidx`, fixed at design time; the browser never names them (R17).

use cobolt_forms::{web_class, Control, Form};
use cobolt_indexed::{AccessMode, FieldUsage, IndexedDefinition, IndexedField};
use cobolt_wire::controls::{self, Operation, IX_RECORD_BYTES};

use super::{banner_lines, endpoint, file_status_call, linkage_names, program, write_linkage, BackendProgram, PARAMS, RESULT};

/// The operations that have a program. `READ-FIRST`, `READ-LAST`, `COMMIT` and
/// `ROLLBACK` are the facade's compositions of these, or nothing to do once
/// each call is durable.
pub const SERVED: &[&str] = &["OPEN", "CLOSE", "START", "READ-NEXT", "READ-PREVIOUS", "READ-INVALID", "WRITE", "REWRITE", "DELETE"];

/// The `.cidx` an IndexedFile control names, when it names one.
pub fn definition_path(ctrl: &Control) -> Option<String> {
    ctrl.get_prop("IndexedFile").map(|v| v.as_str().trim().to_owned()).filter(|s| !s.is_empty())
}

/// Is `ctrl` an IndexedFile control the web build serves?
pub fn is_served(ctrl: &Control) -> bool {
    ctrl.control_type.as_str() == "IndexedFile" && definition_path(ctrl).is_some()
}

/// What the programs need from one control's `.cidx`.
pub struct Source {
    pub def: IndexedDefinition,
    /// The record's 01 name in the `.cidx`.
    pub record: String,
    /// The primary key's field.
    pub key: String,
}

impl Source {
    pub fn new(def: &IndexedDefinition) -> Result<Source, String> {
        let root = def.record_root().ok_or_else(|| format!("{} declares no record", def.name))?;
        if def.record_length() as usize > IX_RECORD_BYTES {
            return Err(format!(
                "the record of {} is {} bytes; a call to the backend carries at most {IX_RECORD_BYTES}",
                def.name,
                def.record_length()
            ));
        }
        if !matches!(def.record_format, cobolt_indexed::RecordFormatDef::Fixed { .. }) {
            return Err(format!("{} has variable-length records; the backend serves fixed-length ones", def.name));
        }
        if def.keys.primary.parts.len() != 1 {
            return Err(format!(
                "{} has {} fields in its primary key; the backend serves a file whose primary key is one field",
                def.name,
                def.keys.primary.parts.len()
            ));
        }
        // The record travels as text: a binary field would not survive it.
        fn binary(f: &IndexedField) -> Option<&IndexedField> {
            if !matches!(f.usage, FieldUsage::Display) && f.offset.is_some() {
                return Some(f);
            }
            f.children.iter().find_map(binary)
        }
        if let Some(f) = def.fields.iter().find_map(binary) {
            return Err(format!(
                "field {} of {} is {:?}; the record travels as text, so the backend serves DISPLAY fields only",
                f.name, def.name, f.usage
            ));
        }
        let mut taken: Vec<String> = Vec::new();
        fn names(f: &IndexedField, out: &mut Vec<String>) {
            out.push(f.name.to_ascii_uppercase());
            f.children.iter().for_each(|c| names(c, out));
        }
        def.fields.iter().for_each(|f| names(f, &mut taken));
        taken.push(def.name.to_ascii_uppercase());
        let reserved: Vec<String> = SERVED
            .iter()
            .filter_map(|op| controls::find("IndexedFile", op))
            .flat_map(linkage_names)
            .collect();
        for name in &taken {
            if name.starts_with("RC-") || reserved.iter().any(|n| n.eq_ignore_ascii_case(name)) {
                return Err(format!(
                    "{} names an item {name}, which the generated backend program declares itself; rename it in the .cidx",
                    def.name
                ));
            }
        }
        Ok(Source { def: def.clone(), record: root.name.clone(), key: def.keys.primary.parts[0].field_name.clone() })
    }
}

/// The programs of every served IndexedFile control of `form`.
pub fn programs(
    form: &Form,
    load_cidx: &dyn Fn(&str) -> Result<IndexedDefinition, String>,
    serves: &dyn Fn(&Form, &Control) -> bool,
) -> Result<Vec<BackendProgram>, Vec<String>> {
    let mut out = Vec::new();
    let mut errs = Vec::new();
    for ctrl in crate::collect_all_controls(&form.controls) {
        if !is_served(ctrl) || !serves(form, ctrl) {
            continue;
        }
        let at = format!("{}: IndexedFile {}", form.name, ctrl.id);
        let path = definition_path(ctrl).unwrap_or_default();
        let source = load_cidx(&path).map_err(|e| format!("cannot read {path}: {e}")).and_then(|d| Source::new(&d));
        let source = match source {
            Ok(s) => s,
            Err(e) => {
                errs.push(format!("{at}: {e}"));
                continue;
            }
        };
        let roles = web_class::backend_roles(ctrl);
        for name in SERVED {
            let Some(op) = controls::find("IndexedFile", name) else { continue };
            let id = match endpoint(form, &ctrl.id, name) {
                Ok(id) => id,
                Err(e) => {
                    errs.push(format!("{at}: {e}"));
                    break;
                }
            };
            let text = source_text(form, ctrl, &path, &id, op, &source);
            // A read can be repeated safely; a verb that changes the file cannot.
            let idempotent = !matches!(*name, "WRITE" | "REWRITE" | "DELETE");
            out.push(program(id, text, &roles, idempotent));
        }
    }
    if errs.is_empty() {
        Ok(out)
    } else {
        Err(errs)
    }
}

fn source_text(form: &Form, ctrl: &Control, path: &str, program_id: &str, op: &Operation, ix: &Source) -> String {
    let file = ix.def.name.as_str();
    let mut out = String::new();
    for line in banner_lines() {
        out.push_str(&format!("      *>{}{line}\n", if line.is_empty() { "" } else { " " }));
    }
    out.push_str("      *> ───────────────────────────────────────────────────────────\n\n");
    out.push_str("       IDENTIFICATION DIVISION.\n");
    out.push_str(&format!("       PROGRAM-ID. {program_id}.\n"));
    out.push_str(&format!("      *> {} of IndexedFile {} on form {},\n", op.operation, ctrl.id, form.name));
    out.push_str(&format!("      *> run on the backend over the indexed file {file} ({path}).\n"));
    write_file_sections(&mut out, ix);
    out.push_str("       WORKING-STORAGE SECTION.\n");
    out.push_str("       01 RC-FILE-STATUS PIC XX.\n");
    out.push_str("       01 RC-NEW PIC X(4096).\n");
    out.push_str("       01 RC-NONE PIC 9 VALUE 0.\n");
    write_linkage(&mut out, op);
    out.push_str(&format!("\n       PROCEDURE DIVISION USING {PARAMS} RETURNING {RESULT}.\n"));
    out.push_str("       RC-MAIN.\n");
    out.push_str("           MOVE \"00\" TO FILE-STATUS\n");
    out.push_str("           MOVE SPACES TO RESULT-IMAGE\n");
    out.push_str("           MOVE 0 TO PAST-END\n");
    out.push_str(&body(op.operation, ix));
    out
}

/// `ENVIRONMENT DIVISION` and `FILE SECTION`: the facade's `SELECT` and `FD`
/// with dynamic access.
fn write_file_sections(out: &mut String, ix: &Source) {
    let mut def = ix.def.clone();
    def.access_mode = AccessMode::Dynamic;
    out.push_str("\n       ENVIRONMENT DIVISION.\n");
    out.push_str("       INPUT-OUTPUT SECTION.\n");
    out.push_str("       FILE-CONTROL.\n");
    out.push_str(&crate::generate_indexed_select(&def));
    out.push_str("\n       DATA DIVISION.\n");
    out.push_str("       FILE SECTION.\n");
    let fd = crate::generate_indexed_fd(&def);
    out.push_str(&fd.replacen(&format!("       FD  {}.\n", def.name), &format!("       FD  {}\n", def.name), 1));
}

/// `OPEN <mode> file`, its status, and the early `GOBACK` of a failed open.
fn open_then_check(out: &mut String, file: &str, mode: &str) {
    out.push_str(&format!("           OPEN {mode} {file}\n"));
    out.push_str(&format!("           {}\n", file_status_call(file)));
    out.push_str("           IF RC-FILE-STATUS(1:1) NOT = \"0\"\n");
    out.push_str("               MOVE RC-FILE-STATUS TO FILE-STATUS\n");
    out.push_str("               GOBACK\n");
    out.push_str("           END-IF\n");
}

/// The `START` that positions a read or answers a `START`, by `KEY-MODE`.
/// `RC-NONE` is 1 when no record satisfies it.
fn start_by_mode(out: &mut String, ix: &Source) {
    let file = ix.def.name.as_str();
    let key = ix.key.as_str();
    out.push_str("           EVALUATE KEY-MODE\n");
    for (mode, rel) in [
        ("EQ", "EQUAL TO"),
        ("GT", "GREATER THAN"),
        ("GE", "GREATER THAN OR EQUAL TO"),
        ("LT", "LESS THAN"),
        ("LE", "LESS THAN OR EQUAL TO"),
    ] {
        out.push_str(&format!("               WHEN \"{mode}\"\n"));
        out.push_str(&format!("                   START {file} KEY IS {rel} {key}\n"));
        out.push_str("                       INVALID KEY MOVE 1 TO RC-NONE\n");
        out.push_str("                   END-START\n");
    }
    out.push_str("           END-EVALUATE\n");
}

fn body(operation: &str, ix: &Source) -> String {
    let file = ix.def.name.as_str();
    let record = ix.record.as_str();
    let status = file_status_call(file);
    let mut out = String::new();
    match operation {
        "OPEN" => {
            out.push_str("           EVALUATE KEY-MODE\n");
            out.push_str(&format!("               WHEN \"INPUT\" OPEN INPUT {file}\n"));
            out.push_str(&format!("               WHEN \"OUTPUT\" OPEN OUTPUT {file}\n"));
            out.push_str(&format!("               WHEN \"EXTEND\" OPEN EXTEND {file}\n"));
            out.push_str(&format!("               WHEN OTHER OPEN I-O {file}\n"));
            out.push_str("           END-EVALUATE\n");
            out.push_str(&format!("           {status}\n"));
            out.push_str("           MOVE RC-FILE-STATUS TO FILE-STATUS\n");
            out.push_str("           IF RC-FILE-STATUS(1:1) = \"0\"\n");
            out.push_str(&format!("               CLOSE {file}\n"));
            out.push_str("           END-IF\n");
            out.push_str("           GOBACK.\n");
        }
        "CLOSE" => {
            out.push_str("      *>   Nothing stays open between calls: every call closed its file.\n");
            out.push_str("           GOBACK.\n");
        }
        "START" => {
            open_then_check(&mut out, file, "INPUT");
            out.push_str(&format!("           MOVE RECORD-IMAGE TO {record}\n"));
            start_by_mode(&mut out, ix);
            out.push_str(&format!("           {status}\n"));
            out.push_str("           MOVE RC-FILE-STATUS TO FILE-STATUS\n");
            out.push_str("           IF RC-NONE = 1\n");
            out.push_str("               MOVE \"23\" TO FILE-STATUS\n");
            out.push_str("           END-IF\n");
            out.push_str(&format!("           CLOSE {file}\n"));
            out.push_str("           GOBACK.\n");
        }
        "READ-NEXT" | "READ-PREVIOUS" => {
            let dir = if operation == "READ-NEXT" { "NEXT" } else { "PREVIOUS" };
            open_then_check(&mut out, file, "INPUT");
            out.push_str(&format!("           MOVE RECORD-IMAGE TO {record}\n"));
            start_by_mode(&mut out, ix);
            out.push_str("           IF RC-NONE = 1\n");
            out.push_str("               MOVE \"10\" TO FILE-STATUS\n");
            out.push_str("               MOVE 1 TO PAST-END\n");
            out.push_str("           ELSE\n");
            out.push_str(&format!("               READ {file} {dir}\n"));
            out.push_str("                   AT END MOVE 1 TO PAST-END\n");
            out.push_str(&format!("                   NOT AT END MOVE {record} TO RESULT-IMAGE\n"));
            out.push_str("               END-READ\n");
            out.push_str(&format!("               {status}\n"));
            out.push_str("               MOVE RC-FILE-STATUS TO FILE-STATUS\n");
            out.push_str("           END-IF\n");
            out.push_str(&format!("           CLOSE {file}\n"));
            out.push_str("           GOBACK.\n");
        }
        "READ-INVALID" => {
            open_then_check(&mut out, file, "INPUT");
            out.push_str(&format!("           MOVE RECORD-IMAGE TO {record}\n"));
            out.push_str(&format!("           READ {file}\n"));
            out.push_str("               INVALID KEY CONTINUE\n");
            out.push_str(&format!("               NOT INVALID KEY MOVE {record} TO RESULT-IMAGE\n"));
            out.push_str("           END-READ\n");
            out.push_str(&format!("           {status}\n"));
            out.push_str("           MOVE RC-FILE-STATUS TO FILE-STATUS\n");
            out.push_str(&format!("           CLOSE {file}\n"));
            out.push_str("           GOBACK.\n");
        }
        "WRITE" => {
            open_then_check(&mut out, file, "I-O");
            out.push_str(&format!("           MOVE RECORD-IMAGE TO {record}\n"));
            out.push_str(&format!("           WRITE {record}\n"));
            out.push_str("               INVALID KEY CONTINUE\n");
            out.push_str("           END-WRITE\n");
            out.push_str(&format!("           {status}\n"));
            out.push_str("           MOVE RC-FILE-STATUS TO FILE-STATUS\n");
            out.push_str(&format!("           MOVE {record} TO RESULT-IMAGE\n"));
            out.push_str(&format!("           CLOSE {file}\n"));
            out.push_str("           GOBACK.\n");
        }
        "REWRITE" => {
            open_then_check(&mut out, file, "I-O");
            out.push_str("           MOVE RECORD-IMAGE TO RC-NEW\n");
            out.push_str(&format!("           MOVE RECORD-IMAGE TO {record}\n"));
            out.push_str(&format!("           READ {file}\n"));
            out.push_str("               INVALID KEY MOVE 1 TO RC-NONE\n");
            out.push_str("           END-READ\n");
            out.push_str("           IF RC-NONE = 1\n");
            out.push_str("               MOVE \"23\" TO FILE-STATUS\n");
            out.push_str("           ELSE\n");
            out.push_str(&format!("               MOVE RC-NEW TO {record}\n"));
            out.push_str(&format!("               REWRITE {record}\n"));
            out.push_str("                   INVALID KEY CONTINUE\n");
            out.push_str("               END-REWRITE\n");
            out.push_str(&format!("               {status}\n"));
            out.push_str("               MOVE RC-FILE-STATUS TO FILE-STATUS\n");
            out.push_str(&format!("               MOVE {record} TO RESULT-IMAGE\n"));
            out.push_str("           END-IF\n");
            out.push_str(&format!("           CLOSE {file}\n"));
            out.push_str("           GOBACK.\n");
        }
        "DELETE" => {
            open_then_check(&mut out, file, "I-O");
            out.push_str(&format!("           MOVE RECORD-IMAGE TO {record}\n"));
            out.push_str(&format!("           READ {file}\n"));
            out.push_str("               INVALID KEY MOVE 1 TO RC-NONE\n");
            out.push_str("           END-READ\n");
            out.push_str("           IF RC-NONE = 1\n");
            out.push_str("               MOVE \"23\" TO FILE-STATUS\n");
            out.push_str("           ELSE\n");
            out.push_str(&format!("               DELETE {file}\n"));
            out.push_str("                   INVALID KEY CONTINUE\n");
            out.push_str("               END-DELETE\n");
            out.push_str(&format!("               {status}\n"));
            out.push_str("               MOVE RC-FILE-STATUS TO FILE-STATUS\n");
            out.push_str("           END-IF\n");
            out.push_str(&format!("           CLOSE {file}\n"));
            out.push_str("           GOBACK.\n");
        }
        other => out.push_str(&format!("      *>   {other} has no program.\n           GOBACK.\n")),
    }
    out
}

// ── The browser's end ────────────────────────────────────────────────────────

/// The group a lowered verb sends, the one its answer comes back in, the file
/// status it is read through and the failure's three items.
pub const FRONTEND_PARAMS: &str = "RC-IX-PARAMS";
pub const FRONTEND_RESULT: &str = "RC-IX-RESULT";
pub const FRONTEND_STATUS: &str = "RC-IX-STATUS";
pub const FRONTEND_ERR_CLASS: &str = "RC-IX-ERR-CLASS";
pub const FRONTEND_ERR_CODE: &str = "RC-IX-ERR-CODE";
pub const FRONTEND_ERR_MESSAGE: &str = "RC-IX-ERR-MESSAGE";

/// The cursor a served file keeps in the browser, since the backend keeps none
/// (O4): how the next sequential read positions (`GT` after a read, the
/// `START`'s own comparison after one; blank: the file's own first record) and
/// the record it positions from.
pub fn frontend_position(n: usize) -> (String, String) {
    (format!("RC-IX-POS-{n}-MODE"), format!("RC-IX-POS-{n}-IMAGE"))
}

/// The names the frontend's declarations take; a developer's item named like
/// one would be ambiguous, so the web build refuses the form.
pub fn frontend_names(files: usize) -> Vec<String> {
    let mut out: Vec<String> = [FRONTEND_PARAMS, FRONTEND_RESULT, FRONTEND_STATUS, FRONTEND_ERR_CLASS, FRONTEND_ERR_CODE, FRONTEND_ERR_MESSAGE]
        .map(str::to_owned)
        .to_vec();
    if let Some(op) = controls::find("IndexedFile", "OPEN") {
        out.extend(linkage_names(op).into_iter().filter(|n| n != PARAMS && n != RESULT));
    }
    for n in 1..=files {
        let (m, i) = frontend_position(n);
        out.push(m);
        out.push(i);
    }
    out
}

/// The WORKING-STORAGE a web form's program gains for `files` served
/// IndexedFile controls: the call's two groups under the names the backend
/// programs' LINKAGE gives their members, and a cursor per file. All `GLOBAL`.
pub fn frontend_working_storage(files: usize) -> String {
    let mut out = String::new();
    out.push_str("      *> The groups an IndexedFile's verbs travel in on the web,\n");
    out.push_str("      *> generated by PowerRustCOBOL RAD on every Build (spec 111).\n");
    if let Some(op) = controls::find("IndexedFile", "OPEN") {
        out.push_str(&format!("       01 {FRONTEND_PARAMS} GLOBAL.\n"));
        super::write_fields(&mut out, op.params, 5, 10);
        out.push_str(&format!("       01 {FRONTEND_RESULT} GLOBAL.\n"));
        super::write_fields(&mut out, op.result, 5, 10);
    }
    out.push_str(&format!("       01 {FRONTEND_STATUS} PIC XX GLOBAL.\n"));
    out.push_str(&format!("       01 {FRONTEND_ERR_CLASS} PIC X(16) GLOBAL.\n"));
    out.push_str(&format!("       01 {FRONTEND_ERR_CODE} PIC 9(5) GLOBAL.\n"));
    out.push_str(&format!("       01 {FRONTEND_ERR_MESSAGE} PIC X(512) GLOBAL.\n"));
    for n in 1..=files {
        let (m, i) = frontend_position(n);
        out.push_str(&format!("       01 {m} PIC X(2) GLOBAL.\n"));
        out.push_str(&format!("       01 {i} PIC X({IX_RECORD_BYTES}) GLOBAL.\n"));
    }
    out
}
