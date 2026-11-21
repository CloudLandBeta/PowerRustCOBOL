// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The subset table (WASM-010 R7, R8, AC6): every built-in of the interpreter
//! is classified for a browser module, and what is outside the subset is
//! refused at its line with the alternative.

mod common;

use cobolt_wasm::support::{builtin_available, builtin_entry, builtin_support, Native, Support, BUILTIN_TABLE};
use cobolt_wasm::{compile_program, ErrorKind, Options};

/// The names that have no row in the table.
fn unclassified<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    names.into_iter().filter(|n| builtin_entry(n).is_none()).collect()
}

#[test]
fn every_builtin_of_the_interpreter_is_classified() {
    let names: Vec<&str> = cobolt_runtime::builtins::BUILTINS.iter().map(|b| b.name).collect();
    let missing = unclassified(names.iter().copied());
    let (mut module, mut backend, mut unsupported) = (0, 0, 0);
    for n in &names {
        match builtin_support(n) {
            Support::Module | Support::Conformance => module += 1,
            Support::BackendOnly(_) | Support::MemoryFiles(_) => backend += 1,
            Support::Unsupported(_) => unsupported += 1,
        }
    }
    eprintln!(
        "subset table: {} of {} classified ({module} browser, {backend} backend only, {unsupported} unsupported)",
        names.len() - missing.len(),
        names.len()
    );
    assert!(missing.is_empty(), "built-ins with no entry in cobolt_wasm::support::BUILTIN_TABLE: {missing:?}");
    // And no row names a built-in that does not exist.
    let stale: Vec<&str> = BUILTIN_TABLE.iter().map(|b| b.name).filter(|n| !names.contains(n)).collect();
    assert!(stale.is_empty(), "rows for built-ins the interpreter does not have: {stale:?}");
    assert_eq!(BUILTIN_TABLE.len(), names.len(), "one row per built-in");
}

#[test]
fn a_planted_unknown_builtin_is_caught() {
    let names = ["OPEN-DB", "NO-SUCH-BUILTIN"];
    assert_eq!(unclassified(names), vec!["NO-SUCH-BUILTIN"]);
    assert!(matches!(builtin_support("NO-SUCH-BUILTIN"), Support::Unsupported(_)));
}

#[test]
fn engine_columns_follow_the_q7_rule() {
    // Every browser entry that names an API is in both engines, or it would not be
    // available outside iOS.
    for b in BUILTIN_TABLE {
        if let (Support::Module, Some((api, e))) = (b.support, b.api) {
            assert!(e.safari, "{} needs {api}, which an iOS build reads from Safari", b.name);
            assert!(builtin_available(b.name, true) == e.safari && builtin_available(b.name, false) == (e.chrome && e.safari));
        }
    }
    assert!(builtin_available("AUDIO-PLAY", false) && builtin_available("AUDIO-PLAY", true));
    assert!(!builtin_available("OPEN-DB", false) && !builtin_available("FOLDER-DIALOG", false));
}

/// AC6: a file verb, `EXEC SQL`, `EXEC RUST`, `SET` of a pointer and a backend
/// built-in each fail at their line, naming the construct and the alternative; each refusal carries
/// its kind (T5, D6), and so does the limit on host reads.
#[test]
fn constructs_outside_the_subset_are_refused_at_their_line_with_the_alternative() {
    let cases: &[(&str, u32, ErrorKind, &[&str])] = &[
        (
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. F1.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT IN-FILE ASSIGN TO \"in.txt\".\nDATA DIVISION.\nFILE SECTION.\nFD IN-FILE.\n01 IN-REC PIC X(10).\nPROCEDURE DIVISION.\n    OPEN INPUT IN-FILE.\n    STOP RUN.\n",
            12,
            ErrorKind::BackendOnly,
            &["browser module", "backend", "CALL … WITH WEBSOCKET LINKAGE"],
        ),
        (
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. S1.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 9(4).\nPROCEDURE DIVISION.\n    EXEC SQL SELECT COUNT(*) INTO :N FROM T END-EXEC.\n    STOP RUN.\n",
            7,
            ErrorKind::BackendOnly,
            &["EXEC SQL", "backend", "CALL … WITH WEBSOCKET LINKAGE"],
        ),
        (
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. R1.\nPROCEDURE DIVISION.\n    EXEC RUST\n        println!(\"hi\");\n    END-EXEC.\n    STOP RUN.\n",
            4,
            ErrorKind::Unsupported,
            &["EXEC RUST", "native build", "backend"],
        ),
        (
            // DESKTOP-WASM-001 C13: nothing lowers SET of a pointer; it was classed as
            // compiled and refused with the generic "not yet supported".
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. P1.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-PTR USAGE POINTER.\n01 WS-CODE PIC X(4).\nPROCEDURE DIVISION.\n    SET WS-PTR TO ADDRESS OF WS-CODE.\n    STOP RUN.\n",
            8,
            ErrorKind::BackendOnly,
            &["SET (pointer)", "browser module", "backend", "CALL … WITH WEBSOCKET LINKAGE"],
        ),
        (
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. B1.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 H PIC 9(4).\n01 ST PIC X(10).\nPROCEDURE DIVISION.\n    CALL \"COBOL-OPEN-DB\" USING \"x.db\" H ST.\n    STOP RUN.\n",
            8,
            ErrorKind::BackendOnly,
            &["OPEN-DB", "backend", "CALL … WITH WEBSOCKET LINKAGE"],
        ),
        (
            // WASM-009 D15: the identity built-in of the generated AUTH-* programs is the backend's.
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. A1.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 AUTH-REQUEST.\n   05 USERNAME PIC X(10).\n01 AUTH-RESULT.\n   05 OUTCOME PIC X(10).\nPROCEDURE DIVISION.\n    COBOL::\"AUTH-OP\" ( \"SIGN-IN\" AUTH-REQUEST AUTH-RESULT ).\n    STOP RUN.\n",
            10,
            ErrorKind::BackendOnly,
            &["AUTH-OP", "identity operations run in the Backend"],
        ),
        (
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. T1.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A PIC X(3) VALUE \"abc\".\nPROCEDURE DIVISION.\n    DISPLAY FUNCTION UPPER-CASE(A) FUNCTION UPPER-CASE(A) FUNCTION UPPER-CASE(A)\n            FUNCTION UPPER-CASE(A) FUNCTION UPPER-CASE(A).\n    STOP RUN.\n",
            // the line of the fifth function, where the limit is crossed
            8,
            ErrorKind::TooManyHostReads,
            &["more than four", "split it"],
        ),
    ];
    let mut refused = 0;
    for (src, line, kind, words) in cases {
        let prog = common::parse(src);
        let err = match compile_program(&prog, &Options::default()) {
            Ok(_) => panic!("compiled, but should be refused:\n{src}"),
            Err(e) => e,
        };
        let all = err.all_kinded();
        let hit = all.iter().find(|e| e.line == *line).unwrap_or_else(|| panic!("no diagnostic at line {line}: {all:?}"));
        for w in *words {
            assert!(hit.message.contains(w), "line {line}: {:?} lacks {w:?}", hit.message);
        }
        assert_eq!(hit.kind, *kind, "line {line}: {}", hit.message);
        assert!(!hit.subject.is_empty(), "line {line}: a kinded refusal names its subject");
        refused += 1;
    }
    eprintln!("subset refusals: {refused} of {} constructs refused at their line with the alternative", cases.len());
}

/// The faults of one Desktop class (DESKTOP-WASM-001 R12): a `Host` that names no
/// capability of ABI A27, a `Cargo` with no reason.
fn native_fault(name: &str, n: Option<Native>) -> Option<String> {
    match n {
        None => Some(format!("{name} has no Desktop class")),
        Some(Native::Host(c)) if !cobolt_wasm_abi::codes::cap::ALL.contains(&c) => {
            Some(format!("{name} names the capability {c:?}, which ABI A27 does not list"))
        }
        Some(Native::Cargo(why)) if why.trim().is_empty() => Some(format!("{name} takes the Cargo path with no reason")),
        Some(_) => None,
    }
}

/// AC10: every statement and every built-in has a Desktop class, as the
/// WebAssembly path stands at M2 (console programs, and forms since T33).
#[test]
fn every_statement_and_builtin_has_a_native_class() {
    use cobolt_wasm::support::{statement_native, STATEMENT_TABLE};
    let t = std::time::Instant::now();
    let (mut module, mut host, mut form, mut cargo) = (0, 0, 0, 0);
    let mut caps = std::collections::BTreeSet::new();
    let mut faults = Vec::new();
    let mut tally = |name: &str, n: Option<Native>| {
        if let Some(f) = native_fault(name, n) {
            faults.push(f);
        }
        match n {
            Some(Native::Module) => module += 1,
            Some(Native::Host(c)) => {
                host += 1;
                caps.insert(c);
            }
            // A file verb: the capability of the file's organisation (T38, ABI A34).
            Some(Native::Files) => {
                host += 1;
                caps.insert("files.*");
            }
            Some(Native::Form) => form += 1,
            Some(Native::Cargo(_)) => cargo += 1,
            None => {}
        }
    };
    for (name, _, _) in STATEMENT_TABLE {
        tally(name, statement_native(name));
    }
    let builtins: Vec<&str> = cobolt_runtime::builtins::BUILTINS.iter().map(|b| b.name).collect();
    for name in &builtins {
        tally(name, builtin_entry(name).map(|b| b.native));
    }
    let checked = STATEMENT_TABLE.len() + builtins.len();
    // The fixed points of M1: EXEC RUST is Rust, so it never leaves the Cargo path (R12);
    // the console and the environment are the runner's; STOP RUN and GOBACK end the run in
    // the module (T12); RETURN-CODE is a whole-number register the run hands to the runner's
    // `exit` (T15, tests/native_return_code.rs).
    let fixed = [
        ("EXEC RUST", matches!(statement_native("EXEC RUST"), Some(Native::Cargo(_)))),
        ("ACCEPT", statement_native("ACCEPT") == Some(Native::Host(cobolt_wasm_abi::codes::cap::CONSOLE))),
        ("STOP", statement_native("STOP") == Some(Native::Module)),
        ("GOBACK", statement_native("GOBACK") == Some(Native::Module)),
        ("MOVE", statement_native("MOVE") == Some(Native::Module)),
        // T38: the file verbs are the runner's, by the file's organisation.
        ("READ", statement_native("READ") == Some(Native::Files)),
        // T33: a form's program is served the controls and the application object.
        ("INVOKE", statement_native("INVOKE") == Some(Native::Form)),
        ("SET-PROPERTY", builtin_entry("SET-PROPERTY").map(|b| b.native) == Some(Native::Form)),
    ];
    let fixed_ok = fixed.iter().filter(|(_, ok)| *ok).count();
    // A Cargo reason names the milestone of DESKTOP-WASM-001 that moves the entry; only
    // EXEC RUST never moves (R11's end state at M6 leaves it alone on the Cargo path).
    let undated: Vec<&str> = STATEMENT_TABLE
        .iter()
        .map(|(n, _, c)| (*n, *c))
        .chain(BUILTIN_TABLE.iter().map(|b| (b.name, b.native)))
        .filter(|(n, c)| matches!(c, Native::Cargo(why) if *n != "EXEC RUST" && !why.contains("(DESKTOP-WASM-001 M")))
        .map(|(n, _)| n)
        .collect();
    // The M6 close (T51): `EXEC RUST` and these families are every entry the native column still
    // sends to the Cargo path. Each family needs a host the desktop runner does not have: a
    // connection to a backend and the identity that goes with it (`pr.remote`: `CALL … WITH
    // WEBSOCKET LINKAGE`, the `WS-*` verbs and `AUTH-OP`), the Data Binding engine (`BINDING-*`),
    // and the phone's secure store, where a desktop program stops (`SECURE*`). The list is exact: a
    // new entry on the Cargo path, or one of these that moves to the module, fails here until the
    // list says so.
    let residual_at_t51: std::collections::BTreeSet<&str> = [
        "AUTH-OP", "CALL … WITH WEBSOCKET LINKAGE", "BINDING-LOAD", "BINDING-MARK-CLEAN", "BINDING-POPULATE", "BINDING-SET-PENDING", "BINDING-SET-READ-ONLY", "BINDING-UPDATE",
        "SECUREDELETE", "SECURELOAD", "SECURESTORE", "WS-CANCELLED", "WS-CLAIM", "WS-DOWNLOAD", "WS-EMIT", "WS-FAIL", "WS-FILE-PATH",
        "WS-FILE-PUBLISH", "WS-LAST-ERROR", "WS-PAUSE", "WS-PROGRESS", "WS-SHOW-PROGRESS", "WS-STREAM-ITEM", "WS-SUBSCRIBE", "WS-UNSUBSCRIBE",
        "WS-UPLOAD", "WS-VERSION", "WS-WARNING",
    ]
    .into_iter()
    .collect();
    let on_cargo: std::collections::BTreeSet<&str> = STATEMENT_TABLE
        .iter()
        .map(|(n, _, c)| (*n, *c))
        .chain(BUILTIN_TABLE.iter().map(|b| (b.name, b.native)))
        .filter(|(n, c)| matches!(c, Native::Cargo(_)) && *n != "EXEC RUST")
        .map(|(n, _)| n)
        .collect();
    let unlisted: Vec<&&str> = on_cargo.difference(&residual_at_t51).collect();
    let moved: Vec<&&str> = residual_at_t51.difference(&on_cargo).collect();
    eprintln!(
        "native column (DESKTOP-WASM-001 T10, AC10)\n  \
         entries: {} statements + {} built-ins = {checked}\n  \
         classes: {module} in the module, {host} through the runner ({caps:?}), {form} in a form's program, {cargo} on the Cargo path\n  \
         fixed points: {fixed_ok} of {} (EXEC RUST, ACCEPT, STOP, GOBACK, MOVE, READ; INVOKE, SET-PROPERTY in a form)\n  \
         faults: {}; Cargo reasons with no milestone: {}\n  \
         Cargo entries other than EXEC RUST: {} of the {} the M6 close lists (new: {}, moved to the module: {})\n  \
         time: {:.3} ms\n  \
         result: {}",
        STATEMENT_TABLE.len(),
        builtins.len(),
        fixed.len(),
        faults.len(),
        undated.len(),
        on_cargo.len(),
        residual_at_t51.len(),
        unlisted.len(),
        moved.len(),
        t.elapsed().as_secs_f64() * 1e3,
        if faults.is_empty() && undated.is_empty() && unlisted.is_empty() && moved.is_empty() && fixed_ok == fixed.len() { "PASS" } else { "FAIL" }
    );
    assert!(unlisted.is_empty(), "entries on the Cargo path the M6 close does not list: {unlisted:?}");
    assert!(moved.is_empty(), "entries the M6 close lists that the native column no longer sends to Cargo: {moved:?}");
    assert!(faults.is_empty(), "{faults:#?}");
    assert!(undated.is_empty(), "Cargo reasons that name no milestone: {undated:?}");
    assert_eq!(fixed_ok, fixed.len(), "{fixed:?}");
    assert_eq!(module + host + form + cargo, checked, "one class per entry");
}

/// T33 (R13, plan §4.3): every control type of the toolbox has a Desktop class. The runner serves
/// the controls of classes A, B and E (HTML, drawn, canvases and graphics objects), the DataGrid and
/// the service controls it carries out (since T48 Maps, since T50 the Viewer); the service controls
/// left (the sign-in control) and an unknown type send the Build to the Cargo path with a reason naming
/// the milestone that moves it. So does a control bound to data, for each source.
#[test]
fn every_control_has_a_desktop_class() {
    use cobolt_forms::model::{BindingSourceDescriptor as S, ControlType};
    use cobolt_wasm::support::{desktop_binding_gap, desktop_control_gap, DESKTOP_BINDING_GAPS};
    let t = std::time::Instant::now();
    let mut served = 0usize;
    let mut by_reason: std::collections::BTreeMap<&str, Vec<&str>> = std::collections::BTreeMap::new();
    for ct in ControlType::ALL {
        match desktop_control_gap(ct) {
            None => served += 1,
            Some(why) => by_reason.entry(why).or_default().push(ct.as_str()),
        }
    }
    let gaps: usize = by_reason.values().map(Vec::len).sum();
    let undated: Vec<&&str> = by_reason.keys().filter(|why| !why.contains("(DESKTOP-WASM-001 M")).collect();
    let fixed = [
        ("Button served", desktop_control_gap(&ControlType::Button).is_none()),
        ("Label served", desktop_control_gap(&ControlType::Label).is_none()),
        ("SideMenu served", desktop_control_gap(&ControlType::SideMenu).is_none()),
        ("Timer served", desktop_control_gap(&ControlType::Timer).is_none()),
        ("DataGrid served", desktop_control_gap(&ControlType::DataGrid).is_none()),
        ("Maps served (T48)", desktop_control_gap(&ControlType::Maps).is_none()),
        ("Viewer served (T50)", desktop_control_gap(&ControlType::Viewer).is_none()),
        ("Canvas2D served (T50)", desktop_control_gap(&ControlType::Canvas2D).is_none()),
        ("Canvas3D served (T50)", desktop_control_gap(&ControlType::Canvas3D).is_none()),
        ("FileDropZone served (T39c)", desktop_control_gap(&ControlType::FileDropZone).is_none()),
        ("IndexedFile served (T39c)", desktop_control_gap(&ControlType::IndexedFile).is_none()),
        ("SqlDatabase served (T41c)", desktop_control_gap(&ControlType::SqlDatabase).is_none()),
        ("an unknown type refused", desktop_control_gap(&ControlType::from_str("NoSuchControl")).is_some()),
        // T41c: a SqlDatabase that names a data item, by its design or by a member, is refused.
        ("a SqlDatabase naming no data item is served", {
            let c = cobolt_forms::Control::new("DB-1", ControlType::SqlDatabase, 0, 0);
            cobolt_wasm::support::desktop_design_gap(&c).is_none()
        }),
        ("a SqlDatabase with ResultSetDataItem set is refused (M6)", {
            let mut c = cobolt_forms::Control::new("DB-1", ControlType::SqlDatabase, 0, 0);
            c.set_prop("ResultSetDataItem", cobolt_forms::PropValue::String("WS-ROW".into()));
            cobolt_wasm::support::desktop_design_gap(&c).is_some_and(|w| w.contains("M6"))
        }),
        ("a SqlDatabase with ConnectionDataItem set is refused", {
            let mut c = cobolt_forms::Control::new("DB-1", ControlType::SqlDatabase, 0, 0);
            c.set_prop("ConnectionDataItem", cobolt_forms::PropValue::String("WS-H".into()));
            cobolt_wasm::support::desktop_design_gap(&c).is_some()
        }),
        ("a Button's design is never refused", {
            let mut c = cobolt_forms::Control::new("BTN-1", ControlType::Button, 0, 0);
            c.set_prop("ResultSetDataItem", cobolt_forms::PropValue::String("WS-ROW".into()));
            cobolt_wasm::support::desktop_design_gap(&c).is_none()
        }),
        ("the members naming a data item: ResultSetDataItem, SET-ConnectionDataItem, SetProperty", {
            use cobolt_wasm::support::is_sql_item_member as m;
            m("ResultSetDataItem") && m("set-connectiondataitem") && m("GET-ResultSetDataItem") && m("SetProperty")
        }),
        ("and not the database's own: Query, Fetch, GET-LastError", {
            use cobolt_wasm::support::is_sql_item_member as m;
            !m("Query") && !m("Fetch") && !m("GET-LastError") && !m("SET-")
        }),
        // T42b: the RestClient is served; one that names a data item, by its design or by a member, is not.
        ("RestClient served (T42b)", desktop_control_gap(&ControlType::RestClient).is_none()),
        // T47a: the WebSearch is served the same way; one that names a data item is not.
        ("WebSearch served (T47a)", desktop_control_gap(&ControlType::WebSearch).is_none()),
        // T47b: the KnowledgeBase is served; its built-in semantic model is the Build's to judge.
        ("KnowledgeBase served (T47b)", desktop_control_gap(&ControlType::KnowledgeBase).is_none()),
        // T47c: the AgentObject is served, its file and Knowledge Base tools too; one that names a data item is
        // not. The sign-in control is still refused.
        ("AgentObject served (T47c)", desktop_control_gap(&ControlType::AgentObject).is_none()),
        ("Authentication still refused (M6)", desktop_control_gap(&ControlType::Authentication).is_some_and(|w| w.contains("M6"))),
        ("an AgentObject with ResponseDataItem set is refused (M6); one naming none is served", {
            let mut c = cobolt_forms::Control::new("AGT-1", ControlType::AgentObject, 0, 0);
            let none = cobolt_wasm::support::desktop_design_gap(&c).is_none();
            c.set_prop("ResponseDataItem", cobolt_forms::PropValue::String("WS-REPLY".into()));
            none && cobolt_wasm::support::desktop_design_gap(&c).is_some_and(|w| w.contains("ResponseDataItem") && w.contains("M6"))
        }),
        ("the AgentObject members refused: SET-ResponseDataItem, SetProperty; not Ask, AddTool, SetToolResult, GET-LastReply, the file tools or the Knowledge Base tools", {
            use cobolt_wasm::support::{agent_member_gap as m, D_AGENT_ITEMS};
            m("SET-ResponseDataItem") == Some(D_AGENT_ITEMS)
                && m("SetProperty") == Some(D_AGENT_ITEMS)
                && ["Ask", "AddTool", "AddToolParameter", "SetToolResult", "GET-LastReply", "Cancel", "AllowFile", "DenyFile", "registerfile", "UnregisterFile", "AllowKnowledgeBase", "denyknowledgebase"]
                    .iter()
                    .all(|x| m(x).is_none())
        }),
        ("a RestClient naming no data item is served", {
            let c = cobolt_forms::Control::new("REST-1", ControlType::RestClient, 0, 0);
            cobolt_wasm::support::desktop_design_gap(&c).is_none()
        }),
        ("a RestClient with ResponseDataItem, StatusDataItem or RequestDataItem set is refused (M6)", {
            ["ResponseDataItem", "StatusDataItem", "RequestDataItem"].iter().all(|p| {
                let mut c = cobolt_forms::Control::new("REST-1", ControlType::RestClient, 0, 0);
                c.set_prop(*p, cobolt_forms::PropValue::String("WS-ITEM".into()));
                cobolt_wasm::support::desktop_design_gap(&c).is_some_and(|w| w.contains("names the program's data items") && w.contains("M6"))
            })
        }),
        ("a WebSearch with ResponseDataItem set is refused (M6); one naming none is served", {
            let mut c = cobolt_forms::Control::new("WEB-1", ControlType::WebSearch, 0, 0);
            let none = cobolt_wasm::support::desktop_design_gap(&c).is_none();
            c.set_prop("ResponseDataItem", cobolt_forms::PropValue::String("WS-ITEM".into()));
            none && cobolt_wasm::support::desktop_design_gap(&c).is_some_and(|w| w.contains("names the program's data items") && w.contains("M6"))
        }),
        ("the RestClient members naming a data item: SET-ResponseDataItem, StatusDataItem, SetProperty; not Get or GET-StatusCode", {
            use cobolt_wasm::support::is_rest_item_member as m;
            m("SET-ResponseDataItem") && m("statusdataitem") && m("RequestDataItem") && m("SetProperty") && !m("Get") && !m("GET-StatusCode")
        }),
    ];
    let fixed_ok = fixed.iter().filter(|(_, ok)| *ok).count();
    let sources = [
        S::IndexedFile { definition_path: String::new(), record_name: String::new(), fields: Vec::new(), key_field: None, writable: false },
        S::Sql {
            source_control_id: String::new(),
            query_name: String::new(),
            result_set_name: String::new(),
            fields: Vec::new(),
            key_fields: Vec::new(),
            writable: false,
            query: String::new(),
            table_name: String::new(),
        },
        S::RestApi { source_control_id: String::new(), endpoint_name: String::new(), response_data_item: String::new(), fields: Vec::new(), update: None },
        S::CobolTable { table_name: String::new(), occurs_item: String::new(), fields: Vec::new(), key_fields: Vec::new(), writable: false },
        S::AgentAi { source_control_id: String::new(), output_name: String::new(), fields: Vec::new(), update: None },
        S::PartiQl { source_control_id: String::new(), query: String::new(), parameters: Vec::new(), fields: Vec::new(), key_fields: Vec::new(), writable: false },
    ];
    let bindings: Vec<&str> = sources.iter().map(desktop_binding_gap).collect();
    let binding_rows_ok = bindings.iter().zip(DESKTOP_BINDING_GAPS).all(|(why, (_, row))| why == row) && bindings.len() == DESKTOP_BINDING_GAPS.len();
    let bindings_dated = bindings.iter().all(|w| w.contains("(DESKTOP-WASM-001 M"));
    eprintln!("controls on the Desktop (DESKTOP-WASM-001 T33, R13)");
    eprintln!("  control types: {} = {served} served + {gaps} on the Cargo path", ControlType::ALL.len());
    for (why, names) in &by_reason {
        eprintln!("    {:>2} {}: {}", names.len(), why.split(" (DESKTOP-WASM-001").next().unwrap_or(why), names.join(", "));
    }
    eprintln!("  fixed points: {fixed_ok} of {}", fixed.len());
    eprintln!("  bound to data: {} sources, one row each: {binding_rows_ok}; every reason names its milestone: {bindings_dated}", sources.len());
    eprintln!("  reasons with no milestone: {}", undated.len());
    eprintln!("  time: {:.3} ms", t.elapsed().as_secs_f64() * 1e3);
    let ok = fixed_ok == fixed.len() && undated.is_empty() && binding_rows_ok && bindings_dated && served + gaps == ControlType::ALL.len();
    eprintln!("  result: {}", if ok { "PASS" } else { "FAIL" });
    assert_eq!(fixed_ok, fixed.len(), "{fixed:?}");
    assert!(undated.is_empty(), "control reasons that name no milestone: {undated:?}");
    assert!(binding_rows_ok && bindings_dated, "{bindings:?}");
}

/// AC10's twin: a name with no row has no Desktop class, and the check names it;
/// a planted capability outside ABI A27 and a planted empty reason are caught too.
#[test]
fn a_planted_entry_with_no_native_class_is_caught() {
    use cobolt_wasm::support::statement_native;
    let t = std::time::Instant::now();
    let cases: [(&str, Option<Native>, &str); 5] = [
        ("NO-SUCH-STATEMENT", statement_native("NO-SUCH-STATEMENT"), "has no Desktop class"),
        ("NO-SUCH-BUILTIN", builtin_entry("NO-SUCH-BUILTIN").map(|b| b.native), "has no Desktop class"),
        ("PLANTED-HOST", Some(Native::Host("teleport")), "which ABI A27 does not list"),
        ("PLANTED-CARGO", Some(Native::Cargo("  ")), "with no reason"),
        ("MOVE", statement_native("MOVE"), ""),
    ];
    let mut caught = 0;
    for (name, n, words) in &cases {
        match (native_fault(name, *n), words.is_empty()) {
            (None, true) => {}
            (Some(f), false) => {
                assert!(f.contains(words) && f.contains(name), "{f}");
                caught += 1;
            }
            (got, _) => panic!("{name}: {got:?}"),
        }
    }
    eprintln!(
        "native column, planted faults (DESKTOP-WASM-001 T10)\n  \
         cases: {} (2 unknown names, 1 unknown capability, 1 empty reason, 1 sound control)\n  \
         caught: {caught} of 4 faults; the sound entry passed\n  \
         time: {:.3} ms\n  \
         result: PASS",
        cases.len(),
        t.elapsed().as_secs_f64() * 1e3
    );
    assert_eq!(caught, 4);
}

/// Q9: `ACCEPT` from the operator is ignored in a browser module, with one
/// warning at its line; the item keeps its value; `FROM DATE` still reads the
/// host clock and warns about nothing.
#[test]
fn accept_from_the_operator_is_ignored_with_a_warning() {
    let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. A1.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 X PIC X(4) VALUE \"KEEP\".\n01 D PIC 9(6).\nPROCEDURE DIVISION.\n    ACCEPT X.\n    ACCEPT D FROM DATE.\n    DISPLAY X.\n    STOP RUN.\n";
    for multi in [true, false] {
        let opts = Options { multi_memory: multi, ..Options::default() };
        let c = common::compile(src, &opts);
        assert_eq!(c.warnings.len(), 1, "{:?}", c.warnings);
        let w = &c.warnings[0];
        assert_eq!((w.line, w.kind), (8, cobolt_wasm::WarningKind::AcceptIgnored), "{w}");
        assert!(w.message.contains("ignored") && w.message.contains("keeps its value"), "{w}");
        let o = cobolt_wasm::host::run_program(&c.wasm).unwrap();
        assert!(o.trapped.is_none() && o.errors.is_empty(), "{o:?}");
        assert_eq!(common::wasm_lines(&o), vec!["KEEP".to_string()], "multi={multi}");
    }
    eprintln!("accept: 2 variants, 1 warning each, the item kept its value");
}

/// D7: the statement table the System KB is written from has one row per
/// statement name `stmt_name` gives (read from its source, so a new `Stmt`
/// variant with a name and no row fails here), and agrees with the exhaustive
/// `support()` on every statement of the sample programs.
#[test]
fn the_statement_table_is_the_support_match() {
    use cobolt_wasm::support::{statement_entry, stmt_name, support, STATEMENT_TABLE};
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/support.rs")).unwrap();
    let body = src.split("pub fn stmt_name").nth(1).unwrap().split("\n}\n").next().unwrap();
    let names: Vec<&str> = body.lines().filter_map(|l| l.split("=> \"").nth(1)).map(|r| r.trim_end_matches("\",")).collect();
    assert!(names.len() > 40, "{names:?}");
    for n in &names {
        assert!(statement_entry(n).is_some(), "statement {n} has no row in STATEMENT_TABLE");
    }
    assert_eq!(STATEMENT_TABLE.len(), names.len(), "a row names no statement");
    // Agreement with support(), statement by statement.
    let programs = [
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. S1.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT F ASSIGN TO \"f\" ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC RECORD KEY IS K STORAGE IS MEMORY.\n    SELECT W ASSIGN TO \"w\".\n\
         DATA DIVISION.\nFILE SECTION.\nFD F.\n01 R.\n    05 K PIC 9(4).\nSD W.\n01 WR PIC X(4).\nWORKING-STORAGE SECTION.\n01 A PIC 9(4).\n01 B PIC 9(4).\n01 T PIC X(10).\n01 G1.\n    05 A1 PIC 9.\n01 G2.\n    05 A1 PIC 9.\n01 TB.\n    05 E PIC 9 OCCURS 3 INDEXED BY IX.\n\
         PROCEDURE DIVISION.\nP1.\n    MOVE 1 TO A.\n    ADD 1 TO A.\n    SUBTRACT 1 FROM A.\n    MULTIPLY 2 BY A.\n    DIVIDE 2 INTO A.\n    COMPUTE A = A + 1.\n\
         \x20   ADD CORRESPONDING G1 TO G2.\n    SUBTRACT CORRESPONDING G1 FROM G2.\n    MOVE CORRESPONDING G1 TO G2.\n    IF A = 1 CONTINUE END-IF.\n    EVALUATE A WHEN 1 CONTINUE END-EVALUATE.\n    PERFORM P2.\n    INITIALIZE A.\n\
         \x20   STRING \"A\" DELIMITED BY SIZE INTO T.\n    UNSTRING T INTO B.\n    INSPECT T TALLYING A FOR ALL \"A\".\n    SEARCH E AT END CONTINUE WHEN E (IX) = 1 CONTINUE END-SEARCH.\n    ALTER P2 TO PROCEED TO P3.\n    GO TO P2 DEPENDING ON A.\n\
         \x20   OPEN I-O F.\n    READ F.\n    WRITE R.\n    REWRITE R.\n    DELETE F.\n    START F.\n    CLOSE F.\n    DISPLAY A.\n    ACCEPT A.\n    CALL \"X\".\n    CANCEL \"X\".\n\
         \x20   SORT W ON ASCENDING KEY WR INPUT PROCEDURE P2 OUTPUT PROCEDURE P3.\n    GO TO P3.\nP2.\n    RELEASE WR.\n    NEXT SENTENCE.\nP3.\n    RETURN W AT END CONTINUE END-RETURN.\n    EXIT.\nP4.\n    STOP RUN.\n",
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. S2.\nPROCEDURE DIVISION.\n    EXEC SQL COMMIT END-EXEC.\n    EXEC RUST\n        let x = 1;\n    END-EXEC.\n    GOBACK.\n",
    ];
    let mut checked = 0;
    let mut seen = std::collections::BTreeSet::new();
    for p in programs {
        let prog = common::parse(p);
        let cobolt_ast::program::ProcedureBody::Paragraphs(ps) = &prog.procedure.body else { panic!("paragraphs") };
        for para in ps {
            for s in &para.stmts {
                s.walk(&mut |s| {
                    let name = stmt_name(s);
                    assert_eq!(statement_entry(name), Some(support(s)), "{name}");
                    seen.insert(name);
                    checked += 1;
                });
            }
        }
    }
    eprintln!("statement table: {} rows, one per statement name; {checked} parsed statements ({} kinds) agree with support(): {seen:?}", STATEMENT_TABLE.len(), seen.len());
}
