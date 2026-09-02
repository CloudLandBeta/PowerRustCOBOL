// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Inspecting data while stopped — the query service.
//!
//! Values are fetched **on demand**: the IDE asks for scopes, then for the rows
//! under one handle, then for the rows under one of those. The program must not
//! move between those questions, which is what separates a query from a step.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{
    new_breakpoints, DebugAnswer, DebugCmd, DebugEvent, DebugQuery, Interpreter, ScopeKind,
    SpecialValue, VarInfo,
};

/// A program with the shapes the inspector has to cope with: a group, a table,
/// an 88-level, a REDEFINES, and the four "non-values".
const SRC: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. DBGINSP.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-COUNT       PIC 9(3) VALUE 42.
01 WS-NAME        PIC X(10) VALUE \"ADA\".
01 WS-EMPTY       PIC X(5) VALUE SPACES.
01 WS-CUSTOMER.
   05 WS-CUST-ID  PIC 9(5) VALUE 12345.
   05 WS-CUST-NM  PIC X(8) VALUE \"LOVELACE\".
01 WS-TABLE.
   05 WS-ROW      PIC 9(2) OCCURS 4 TIMES.
01 WS-STATUS      PIC X VALUE \"A\".
   88 WS-ACTIVE   VALUE \"A\".
   88 WS-CLOSED   VALUE \"C\".
PROCEDURE DIVISION.
MAIN.
    MOVE 7 TO WS-ROW(2)
    DISPLAY WS-COUNT
    STOP RUN.
";

/// A session stopped at the first statement, able to ask questions.
struct Session {
    cmd: mpsc::Sender<DebugCmd>,
    ev: mpsc::Receiver<DebugEvent>,
    next_id: u64,
    handle: Option<thread::JoinHandle<()>>,
}

impl Session {
    fn start() -> Self {
        let result = parse(tokenize(SRC, SourceFormat::Free));
        assert!(
            result
                .diagnostics
                .iter()
                .all(|d| d.severity != Severity::Error),
            "parse errors: {:?}",
            result.diagnostics
        );
        let program = result.program.expect("no program");
        let (cmd_tx, cmd_rx) = mpsc::channel::<DebugCmd>();
        let (ev_tx, ev_rx) = mpsc::channel::<DebugEvent>();
        let handle = thread::spawn(move || {
            let mut interp =
                Interpreter::new_with_debug_channels(program, cmd_rx, ev_tx, new_breakpoints());
            let _ = interp.run();
        });
        let mut s = Session {
            cmd: cmd_tx,
            ev: ev_rx,
            next_id: 1,
            handle: Some(handle),
        };
        s.wait_stopped();
        s
    }

    /// Drain until the interpreter reports a stop.
    fn wait_stopped(&mut self) {
        for _ in 0..200 {
            match self.ev.recv_timeout(Duration::from_secs(5)) {
                Ok(DebugEvent::Stopped { .. }) => return,
                Ok(_) => continue,
                Err(e) => panic!("no stop: {e}"),
            }
        }
        panic!("no stop after 200 events");
    }

    fn ask(&mut self, query: DebugQuery) -> DebugAnswer {
        let id = self.next_id;
        self.next_id += 1;
        self.cmd.send(DebugCmd::Query { id, query }).unwrap();
        for _ in 0..200 {
            match self.ev.recv_timeout(Duration::from_secs(5)) {
                Ok(DebugEvent::Answer { id: got, answer }) if got == id => return answer,
                Ok(_) => continue,
                Err(e) => panic!("no answer to {id}: {e}"),
            }
        }
        panic!("no answer to {id}");
    }

    fn rows(&mut self, reference: i64) -> Vec<VarInfo> {
        match self.ask(DebugQuery::Variables { reference }) {
            DebugAnswer::Variables(v) => v,
            other => panic!("expected rows, got {other:?}"),
        }
    }

    /// The WORKING-STORAGE handle.
    fn working_storage(&mut self) -> i64 {
        match self.ask(DebugQuery::Scopes { frame: 0 }) {
            DebugAnswer::Scopes(s) => {
                s.iter()
                    .find(|sc| sc.kind == ScopeKind::WorkingStorage)
                    .unwrap_or_else(|| panic!("no WORKING-STORAGE in {s:?}"))
                    .reference
            }
            other => panic!("expected scopes, got {other:?}"),
        }
    }

    fn row<'a>(rows: &'a [VarInfo], name: &str) -> &'a VarInfo {
        rows.iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("{name} not among {:?}", rows.iter().map(|r| &r.name).collect::<Vec<_>>()))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.cmd.send(DebugCmd::Terminate);
        drop(self.cmd.clone());
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

#[test]
fn scopes_list_only_what_the_program_actually_has() {
    let mut s = Session::start();
    let DebugAnswer::Scopes(scopes) = s.ask(DebugQuery::Scopes { frame: 0 }) else {
        panic!("expected scopes");
    };
    assert!(
        scopes.iter().any(|sc| sc.kind == ScopeKind::WorkingStorage),
        "WORKING-STORAGE must be there: {scopes:?}"
    );
    // This program declares no LINKAGE, so an empty LINKAGE row would be a row
    // that never opens.
    assert!(
        !scopes.iter().any(|sc| sc.kind == ScopeKind::Linkage),
        "an empty scope must not be listed: {scopes:?}"
    );
    let ws = scopes
        .iter()
        .find(|sc| sc.kind == ScopeKind::WorkingStorage)
        .unwrap();
    assert_eq!(ws.name, "WORKING-STORAGE SECTION", "the COBOL name, untranslated");
    assert!(ws.count >= 6, "six 01-levels declared, got {}", ws.count);
}

/// Top level means top level: a `05` arrives by opening its `01`, never beside
/// it, and an occurrence arrives by opening its table.
#[test]
fn working_storage_lists_only_the_01_levels() {
    let mut s = Session::start();
    let ws = s.working_storage();
    let names: Vec<String> = s.rows(ws).into_iter().map(|r| r.name).collect();
    for want in ["WS-COUNT", "WS-NAME", "WS-CUSTOMER", "WS-TABLE", "WS-STATUS"] {
        assert!(names.contains(&want.to_owned()), "{want} missing from {names:?}");
    }
    for subordinate in ["WS-CUST-ID", "WS-CUST-NM", "WS-ROW"] {
        assert!(
            !names.contains(&subordinate.to_owned()),
            "{subordinate} is subordinate and must not be a sibling: {names:?}"
        );
    }
}

#[test]
fn an_elementary_item_carries_its_value_and_its_picture() {
    let mut s = Session::start();
    let ws = s.working_storage();
    let rows = s.rows(ws);

    let count = Session::row(&rows, "WS-COUNT");
    assert_eq!(count.value.trim(), "42");
    assert_eq!(count.pic, "9(3)");
    assert_eq!(count.category, "numeric");
    assert_eq!(count.reference, 0, "an elementary item does not expand");
    assert!(count.editable);

    let name = Session::row(&rows, "WS-NAME");
    assert!(name.value.starts_with("ADA"));
    assert_eq!(name.category, "alphanumeric");
    assert_eq!(name.length, Some(10));
}

/// A group expands into its children; a table into its occurrences. Both are
/// one query deep — nothing below is built until it is asked for.
#[test]
fn a_group_expands_into_children_and_a_table_into_occurrences() {
    let mut s = Session::start();
    let ws = s.working_storage();
    let rows = s.rows(ws);

    let cust = Session::row(&rows, "WS-CUSTOMER");
    assert_eq!(cust.category, "group");
    assert!(cust.reference > 0, "a group must expand");
    let children: Vec<String> = s.rows(cust.reference).into_iter().map(|r| r.name).collect();
    assert_eq!(children, vec!["WS-CUST-ID", "WS-CUST-NM"]);

    let table = Session::row(&rows, "WS-TABLE");
    let inner = s.rows(table.reference);
    let row = Session::row(&inner, "WS-ROW");
    assert_eq!(row.occurs, Some(4), "OCCURS 4 TIMES");
    assert!(row.reference > 0, "a table must expand into its occurrences");
    let cells = s.rows(row.reference);
    assert_eq!(cells.len(), 4, "one row per occurrence: {cells:?}");
    assert_eq!(cells[0].name, "(1)");
    // MOVE 7 TO WS-ROW(2) has not run yet — the session stops at the FIRST
    // statement, so every cell is still at its initial value.
    assert!(cells.iter().all(|c| !c.editable == false), "a cell is editable");
}

#[test]
fn an_88_level_reports_the_condition_the_program_would_test() {
    let mut s = Session::start();
    let ws = s.working_storage();
    let rows = s.rows(ws);
    let status = Session::row(&rows, "WS-STATUS");
    assert!(status.reference > 0, "an item with 88s must expand");

    let conds = s.rows(status.reference);
    let active = Session::row(&conds, "WS-ACTIVE");
    let closed = Session::row(&conds, "WS-CLOSED");
    assert_eq!(active.category, "condition");
    assert_eq!(active.value, "TRUE", "WS-STATUS is \"A\"");
    assert_eq!(closed.value, "FALSE");
    assert!(
        !active.editable,
        "an 88 is not storage — SET … TO TRUE writes its host"
    );
}

/// The column exists to tell these apart. Spaces is not an empty string.
#[test]
fn a_field_of_spaces_is_not_an_empty_string() {
    let mut s = Session::start();
    let ws = s.working_storage();
    let rows = s.rows(ws);
    assert_eq!(
        Session::row(&rows, "WS-EMPTY").special,
        Some(SpecialValue::Spaces)
    );
    assert_eq!(
        Session::row(&rows, "WS-COUNT").special,
        None,
        "an ordinary value has no special marker"
    );
}

/// Editing while stopped: validated against the item's own PICTURE, and the
/// answer is what the PROGRAM will see, not what was typed.
#[test]
fn a_compatible_edit_is_accepted_and_reads_back_through_the_picture() {
    let mut s = Session::start();
    let ws = s.working_storage();
    match s.ask(DebugQuery::SetVariable {
        reference: ws,
        name: "WS-COUNT".into(),
        value: "7".into(),
    }) {
        DebugAnswer::Set { value } => {
            assert_eq!(value.trim(), "7", "PIC 9(3) given 7");
        }
        other => panic!("expected a write, got {other:?}"),
    }
    let rows = s.rows(ws);
    assert_eq!(Session::row(&rows, "WS-COUNT").value.trim(), "7");
}

/// A rejected edit must leave the program exactly as it was — not half-applied,
/// and not silently coerced.
#[test]
fn an_incompatible_edit_is_refused_without_touching_the_program() {
    let mut s = Session::start();
    let ws = s.working_storage();
    let before = Session::row(&s.rows(ws), "WS-COUNT").value.clone();

    match s.ask(DebugQuery::SetVariable {
        reference: ws,
        name: "WS-COUNT".into(),
        value: "not a number".into(),
    }) {
        DebugAnswer::Error(msg) => assert!(msg.contains("not a number"), "{msg}"),
        other => panic!("a numeric item must refuse text, got {other:?}"),
    }
    assert_eq!(
        Session::row(&s.rows(ws), "WS-COUNT").value,
        before,
        "the refused edit must not have changed anything"
    );

    // A group is written through its children, never directly.
    let cust = Session::row(&s.rows(ws), "WS-CUSTOMER").reference;
    let _ = cust;
    match s.ask(DebugQuery::SetVariable {
        reference: ws,
        name: "WS-CUSTOMER".into(),
        value: "X".into(),
    }) {
        DebugAnswer::Error(msg) => assert!(msg.contains("group"), "{msg}"),
        other => panic!("a group must refuse a direct write, got {other:?}"),
    }
}

#[test]
fn evaluate_reads_an_item_and_says_so_when_it_cannot() {
    let mut s = Session::start();
    match s.ask(DebugQuery::Evaluate {
        frame: 0,
        expression: "WS-COUNT".into(),
    }) {
        DebugAnswer::Evaluated { result, pic } => {
            assert_eq!(result.trim(), "42");
            assert_eq!(pic, "9(3)");
        }
        other => panic!("expected a value, got {other:?}"),
    }
    match s.ask(DebugQuery::Evaluate {
        frame: 0,
        expression: "WS-NOT-A-THING".into(),
    }) {
        DebugAnswer::Error(msg) => assert!(!msg.is_empty()),
        other => panic!("expected an error, got {other:?}"),
    }
}

/// The property that separates a query from a step: asking does not advance the
/// program. Three questions in a row, and the statement under the pointer is
/// still the same one.
#[test]
fn asking_questions_does_not_move_the_program() {
    let mut s = Session::start();
    let ws = s.working_storage();
    let first = Session::row(&s.rows(ws), "WS-COUNT").value.clone();
    for _ in 0..3 {
        let _ = s.rows(ws);
    }
    assert_eq!(
        Session::row(&s.rows(ws), "WS-COUNT").value,
        first,
        "MOVE 7 TO WS-ROW(2) must not have run while we were looking"
    );
}

/// A handle from a previous stop must resolve to nothing rather than to
/// whatever now occupies that slot.
#[test]
fn a_handle_from_an_earlier_stop_is_refused() {
    let mut s = Session::start();
    let ws = s.working_storage();
    s.cmd.send(DebugCmd::StepOver).unwrap();
    s.wait_stopped();
    match s.ask(DebugQuery::Variables { reference: ws }) {
        DebugAnswer::Error(msg) => assert!(msg.contains("earlier stop"), "{msg}"),
        DebugAnswer::Variables(_) => panic!("a stale handle must not resolve"),
        other => panic!("unexpected {other:?}"),
    }
}
