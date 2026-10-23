// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The project module (WASM-010 R1, R2, R10, R11, R12; ABI v1 §4, A1–A5): forms
//! compiled with `compile_project`, driven on the test host the way the player
//! drives them — open, events, close — in both memory variants.

mod common;

use cobolt_wasm::host::{FormHost, HostOptions};
use cobolt_wasm::module::{compile_project, FormSpec};
use cobolt_wasm::{Compiled, Options};
use cobolt_wasm_abi::codes::{PRODUCTION_IMPORTS, REQUIRED_EXPORTS};
use cobolt_wasm_abi::manifest::Manifest;
use cobolt_wasm_abi::scan::{bundle_problems, scan_module};

/// A form program in the shape `cobolt-codegen` writes (spec 049): the form's
/// data, `INIT-FORM`, the `WAIT-EVENT` loop dispatching to handlers, and the
/// handlers as nested `COMMON` programs. `handlers` is `(control, event, body)`.
fn form_source(name: &str, data: &str, handlers: &[(&str, &str, &str)]) -> String {
    let mut s = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. {name}.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 COBOL-QUIT PIC 9 VALUE 0.\n01 COBOL-EVENT-ID PIC X(64) VALUE SPACES.\n\
         01 COBOL-CONTROL-ID PIC X(64) VALUE SPACES.\n01 FORM-NAME PIC X(64) VALUE '{name}'.\n{data}\
         PROCEDURE DIVISION.\nCOBOL-MAIN.\n    COBOL::\"INIT-FORM\" ( FORM-NAME )\n    PERFORM COBOL-EVENT-LOOP\n    STOP RUN.\n\
         COBOL-EVENT-LOOP.\n    PERFORM UNTIL COBOL-QUIT = 1\n        COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )\n\
         \x20       EVALUATE COBOL-CONTROL-ID\n"
    );
    for (c, e, _) in handlers {
        s.push_str(&format!(
            "            WHEN \"{c}\"\n                EVALUATE COBOL-EVENT-ID\n                    WHEN \"{e}\"\n                        CALL \"{c}--{}\"\n                END-EVALUATE\n",
            e.to_ascii_uppercase()
        ));
    }
    s.push_str("        END-EVALUATE\n    END-PERFORM.\n");
    for (c, e, body) in handlers {
        let p = format!("{c}--{}", e.to_ascii_uppercase());
        s.push_str(&format!("IDENTIFICATION DIVISION.\nPROGRAM-ID. {p} IS COMMON PROGRAM.\nPROCEDURE DIVISION.\n{body}    GOBACK.\nEND PROGRAM {p}.\n"));
    }
    s.push_str(&format!("END PROGRAM {name}.\n"));
    s
}

struct Project {
    programs: Vec<cobolt_ast::program::Program>,
    specs: Vec<(String, bool, u32, Vec<(String, String)>, Vec<(String, String)>, Vec<String>)>,
}

impl Project {
    fn new() -> Self {
        Project { programs: vec![], specs: vec![] }
    }
    fn form(mut self, name: &str, main: bool, instances: u32, controls: &[(&str, &str)], src: &str) -> Self {
        self.programs.push(common::parse(src));
        let bindings: Vec<(String, String)> = src
            .lines()
            .filter_map(|l| l.trim().strip_prefix("PROGRAM-ID. ").and_then(|r| r.strip_suffix(" IS COMMON PROGRAM.")))
            .filter_map(|p| p.split_once("--").map(|(c, e)| (c.to_string(), e.to_string())))
            .map(|(c, e)| (c, format!("on{}{}", &e[2..3], e[3..].to_ascii_lowercase())))
            .collect();
        self.specs.push((
            name.into(),
            main,
            instances,
            controls.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect(),
            bindings,
            vec![],
        ));
        self
    }
    /// The last form's layers, in stack order.
    fn layers(mut self, names: &[&str]) -> Self {
        self.specs.last_mut().unwrap().5 = names.iter().map(|s| s.to_string()).collect();
        self
    }
    /// An extra binding on the last form (one with no handler program).
    fn bind(mut self, ctrl: &str, event: &str) -> Self {
        self.specs.last_mut().unwrap().4.push((ctrl.into(), event.into()));
        self
    }
    fn try_compile(&self, multi: bool) -> Result<Compiled, cobolt_wasm::GenError> {
        let specs = self.form_specs();
        compile_project(&specs, &Options { multi_memory: multi, name: "demo".into(), ..Options::default() })
    }
    fn form_specs(&self) -> Vec<FormSpec<'_>> {
        self.specs
            .iter()
            .zip(&self.programs)
            .map(|((name, main, inst, controls, bindings, layers), program)| FormSpec {
                name: name.clone(),
                main: *main,
                instances: *inst,
                program,
                controls: controls.clone(),
                bindings: bindings.clone(),
                layers: layers.clone(),
                lines: vec![],
            })
            .collect()
    }
    fn compile(&self, multi: bool) -> Compiled {
        self.try_compile(multi).unwrap_or_else(|e| panic!("compile_project failed: {e}"))
    }
}

fn manifest_of(c: &Compiled) -> Manifest {
    scan_module(&c.wasm).unwrap().embedded_manifest().unwrap()
}

fn name_id(m: &Manifest, name: &str) -> u32 {
    m.names.iter().position(|n| n.eq_ignore_ascii_case(name)).unwrap_or_else(|| panic!("{name} is not in names {:?}", m.names)) as u32
}

fn calculator() -> Project {
    let main = form_source(
        "MAIN-FORM",
        "01 WS-TOTAL PIC 9(4) VALUE 5 GLOBAL.\n01 WS-SHOWN PIC Z(3)9 GLOBAL.\n",
        &[
            (
                "BTN-OK",
                "onClick",
                "    ADD 37 TO WS-TOTAL\n    MOVE WS-TOTAL TO WS-SHOWN\n    DISPLAY \"TOTAL \" WS-SHOWN\n    MOVE WS-TOTAL TO LBL-OUT::Caption\n",
            ),
            ("BTN-OPEN", "onClick", "    DISPLAY \"OPEN PRESSED\"\n"),
        ],
    );
    let other = form_source("SECOND-FORM", "", &[("BTN-BACK", "onClick", "    DISPLAY \"BACK\"\n")]);
    Project::new()
        .form("MAIN-FORM", true, 1, &[("BTN-OK", "Button"), ("LBL-OUT", "Label"), ("BTN-OPEN", "Button")], &main)
        .form("SECOND-FORM", false, 2, &[("BTN-BACK", "Button")], &other)
}

/// T8: the manifest is the ABI crate's, it validates, and both variants are
/// acceptable bundle members.
#[test]
fn a_two_form_manifest_validates_and_both_variants_bundle() {
    let p = calculator();
    let mut checked = 0;
    for multi in [true, false] {
        let c = p.compile(multi);
        let m = manifest_of(&c);
        m.validate().unwrap();
        assert_eq!(m.forms.len(), 2);
        assert_eq!((m.forms[0].name.as_str(), m.forms[0].main, m.forms[1].instances), ("MAIN-FORM", true, 2));
        assert_eq!(m.forms[0].controls[1].name, "LBL-OUT");
        assert_eq!(m.forms[0].controls[1].id, 4);
        assert!(m.forms[0].has_handler(3, m.event_index("onClick").unwrap()), "{:?}", m.forms[0].handlers);
        assert!(m.forms[0].region_size > 0 && m.limits.tasks == 3, "{:?}", m.limits);
        assert_eq!(m.variant.multi_memory, multi);
        let problems = bundle_problems(&c.wasm, !multi);
        assert!(problems.is_empty(), "multi={multi}: {problems:#?}");
        checked += 1;
    }
    eprintln!("manifest: {checked} variants validated, 0 bundle problems");
}

/// AC1: a button handler doing MOVE, ADD and DISPLAY gives the expected
/// property write and output, and the module imports only the host API.
#[test]
fn button_handler_move_add_display() {
    let p = calculator();
    for multi in [true, false] {
        let c = p.compile(multi);
        let scan = scan_module(&c.wasm).unwrap();
        for imp in &scan.imports {
            assert_eq!(imp.module, "pr");
            assert!(PRODUCTION_IMPORTS.contains(&imp.name.as_str()), "multi={multi}: imports pr.{}", imp.name);
        }
        for e in REQUIRED_EXPORTS {
            assert!(scan.has_export(e), "multi={multi}: no export {e}");
        }
        let m = manifest_of(&c);
        let click = m.event_index("onClick").unwrap() as i32;
        let caption = name_id(&m, "Caption");
        let mut host = FormHost::new(&c.wasm).unwrap();
        let (inst, status) = host.open_form(0, -1);
        assert_eq!((inst, status), (0, 0), "multi={multi}: {:?}", host.outcome());
        assert_eq!(host.dbg_live(), 1);
        assert_eq!(host.event(inst, 3, click, &[]), 0, "{:?}", host.outcome());
        assert_eq!(host.event(inst, 3, click, &[]), 0, "{:?}", host.outcome());
        let o = host.outcome();
        assert!(o.trapped.is_none() && o.errors.is_empty(), "{o:?}");
        assert_eq!(common::wasm_lines(&o), vec!["TOTAL   42", "TOTAL   79"], "multi={multi}");
        assert_eq!(o.sets, vec![(0, 4, vec![caption], "42".to_string()), (0, 4, vec![caption], "79".to_string())]);
        assert_eq!(host.property(0, 4, caption).as_deref(), Some("79"));
        assert_eq!(host.tick(), 0, "the last task was instance 0's");
        eprintln!(
            "button handler (multi={multi}): {} commands, {} host calls ({} flushes, {} queries) for 2 clicks",
            o.sets.len() + o.calls.len() + 2,
            o.flushes + o.queries,
            o.flushes,
            o.queries
        );
    }
}

/// R11: a run-time error ends the handler with an ERROR carrying the COBOL
/// line, the form and the event, and the application keeps running.
#[test]
fn runtime_error_has_line() {
    let src = form_source(
        "ERR-FORM",
        "01 WS-A PIC 9(4) VALUE 7 GLOBAL.\n01 WS-Z PIC 9(4) VALUE 0 GLOBAL.\n01 WS-R PIC 9(4) VALUE 0 GLOBAL.\n",
        &[("BTN-BAD", "onClick", "    COMPUTE WS-R = FUNCTION MOD(WS-A, WS-Z)\n    DISPLAY \"NOT REACHED\"\n"), ("BTN-OK", "onClick", "    DISPLAY \"STILL HERE\"\n")],
    );
    let line = src.lines().position(|l| l.contains("FUNCTION MOD")).unwrap() as u32 + 1;
    let p = Project::new().form("ERR-FORM", true, 1, &[("BTN-BAD", "Button"), ("BTN-OK", "Button")], &src);
    for multi in [true, false] {
        let c = p.compile(multi);
        let m = manifest_of(&c);
        let click = m.event_index("onClick").unwrap() as i32;
        let mut host = FormHost::new(&c.wasm).unwrap();
        let (inst, _) = host.open_form(0, -1);
        assert_eq!(host.event(inst, 3, click, &[]), -10, "the module traps after the ERROR");
        let o = host.outcome();
        assert_eq!(o.error_positions.len(), 1, "{o:?}");
        let (code, unit, l, form, event) = o.error_positions[0];
        assert!(code >= 1000, "a COBOL run-time error is numbered from 1000: {code}");
        assert_eq!((unit, l, form, event), (0, line, 0, click as u32), "multi={multi}");
        assert!(!o.output.contains("NOT REACHED"));
        // The application keeps running: the next event is delivered.
        assert_eq!(host.event(inst, 4, click, &[]), 0, "{:?}", host.outcome());
        assert!(host.outcome().output.contains("STILL HERE"), "multi={multi}: {:?}", host.outcome());
        eprintln!("runtime error (multi={multi}): code {code} at line {l} of form {form}, event {event}; the next event ran");
    }
}

/// WASM-017 R2: opening more instances than the form allows is a run-time
/// error naming the form, not a trap; closing one makes room again.
#[test]
fn instances_exhausted_is_an_error() {
    let p = calculator();
    for multi in [true, false] {
        let c = p.compile(multi);
        let mut host = FormHost::new(&c.wasm).unwrap();
        let used0 = host.dbg_region_used();
        let (a, _) = host.open_form(1, -1);
        let (b, _) = host.open_form(1, -1);
        assert_eq!((a, b), (1, 2), "SECOND-FORM's two instances follow MAIN-FORM's one");
        let (c3, st) = host.open_form(1, -1);
        assert_eq!((c3, st), (-3, -3));
        let o = host.outcome();
        assert!(o.trapped.is_none());
        assert_eq!(o.error_positions.last().map(|e| e.0), Some(3), "{o:?}");
        assert_eq!(host.dbg_live(), 2);
        assert_eq!(host.close_form(a), 0, "{:?}", host.outcome());
        assert_eq!(host.dbg_live(), 1);
        let (again, _) = host.open_form(1, -1);
        assert_eq!(again, a, "the released instance is reused");
        assert_eq!(host.close_form(again), 0);
        assert_eq!(host.close_form(b), 0);
        assert_eq!(host.dbg_region_used(), used0, "every region is released");
        // An unknown form is refused.
        assert_eq!(host.open_form(9, -1).0, -4);
        eprintln!("instances (multi={multi}): 2 of 2 opened, the third refused with -3, regions back to {used0} bytes");
    }
}

fn reader() -> Project {
    let src = form_source(
        "READ-FORM",
        "01 WS-X PIC X(10) GLOBAL.\n",
        &[("BTN-GO", "onClick", "    MOVE \"abc\" TO TXT-NAME::Text\n    MOVE TXT-NAME::Text TO WS-X\n    DISPLAY \"READ \" WS-X\n")],
    );
    Project::new().form("READ-FORM", true, 1, &[("BTN-GO", "Button"), ("TXT-NAME", "TextBox")], &src)
}

/// R12 / WASM-017 R11: a read sees the handler's own earlier write (the write
/// travels with the query), and the handler's commands reach the host in one
/// flush when it ends.
#[test]
fn host_calls_per_handler() {
    let p = reader();
    for multi in [true, false] {
        let c = p.compile(multi);
        let m = manifest_of(&c);
        let click = m.event_index("onClick").unwrap() as i32;
        let mut host = FormHost::new(&c.wasm).unwrap();
        let (inst, _) = host.open_form(0, -1);
        let before = host.outcome();
        assert_eq!(host.event(inst, 3, click, &[]), 0, "{:?}", host.outcome());
        let o = host.outcome();
        assert!(o.trapped.is_none() && o.errors.is_empty(), "{o:?}");
        assert_eq!(common::wasm_lines(&o), vec!["READ abc       "], "multi={multi}: WS-X is PIC X(10)");
        let (flushes, queries) = (o.flushes - before.flushes, o.queries - before.queries);
        assert_eq!((flushes, queries), (1, 1), "multi={multi}: one query (carrying the SET), one flush at the end");
        eprintln!("host calls (multi={multi}): {flushes} flush + {queries} query for a handler that writes, reads and displays");
    }
}

/// AC9 part 2: the manifest lists the imports, and a host that lacks one the
/// module needs refuses to start, naming it.
#[test]
fn manifest_and_imports() {
    let c = reader().compile(true);
    let m = manifest_of(&c);
    let scan = scan_module(&c.wasm).unwrap();
    let imported: Vec<&str> = scan.imports.iter().map(|i| i.name.as_str()).collect();
    assert!(imported.contains(&"query"), "{imported:?}");
    assert_eq!(m.imports, imported, "the manifest lists exactly the module's imports, in order");
    let err = FormHost::with(&c.wasm, HostOptions { omit_imports: vec!["query".into()], ..HostOptions::default() }).err().expect("refused");
    assert!(err.contains("query"), "{err}");
    assert!(scan.exports.iter().any(|e| e.name == "pr_line"), "$line is exported for diagnostics");
    eprintln!("imports: {} declared ({}), a host without query refused: {err}", imported.len(), imported.join(", "));
}

/// R14: the line a run-time error reports is the developer's line in the
/// handler, as Check reports it (through codegen's source map).
#[test]
fn error_line_maps_to_handler() {
    use cobolt_forms::model::{Control, ControlType, EventBinding, Form};
    let mut form = Form::new("ERR-FORM", "Errors", 400, 300);
    let mut btn = Control::new("BTN-BAD", ControlType::Button, 10, 10);
    let mut ev = EventBinding::for_control("BTN-BAD", "onClick");
    ev.code = "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 WS-A PIC 9(4) VALUE 7.\n       01 WS-Z PIC 9(4) VALUE 0.\n       01 WS-R PIC 9(4) VALUE 0.\n       PROCEDURE DIVISION.\n           COMPUTE WS-R = FUNCTION MOD(WS-A, WS-Z).".to_string();
    btn.events.push(ev);
    form.controls.push(btn);
    let (src, map) = cobolt_codegen::generate_with_map(&form);
    let program = common::parse(&src);
    let n_lines = src.lines().count() as u32;
    let lines: Vec<cobolt_wasm::module::SourceLine> = (1..=n_lines)
        .filter_map(|g| map.resolve(g).map(|(site, l)| cobolt_wasm::module::SourceLine { gen_line: g, site: site.display_path(&form.name), line: l, column: 0 }))
        .collect();
    let spec = FormSpec {
        name: form.name.clone(),
        main: true,
        instances: 1,
        program: &program,
        controls: form.controls.iter().map(|c| (c.id.clone(), format!("{:?}", c.control_type))).collect(),
        bindings: vec![("BTN-BAD".into(), "onClick".into())],
        layers: vec![],
        lines,
    };
    for multi in [true, false] {
        let c = compile_project(std::slice::from_ref(&spec), &Options { multi_memory: multi, ..Options::default() }).unwrap_or_else(|e| panic!("{e}"));
        let m = manifest_of(&c);
        let mut host = FormHost::new(&c.wasm).unwrap();
        let (inst, st) = host.open_form(0, -1);
        assert_eq!(st, 0, "{:?}", host.outcome());
        assert_eq!(host.event(inst, 3, m.event_index("onClick").unwrap() as i32, &[]), -10, "{:?}", host.outcome());
        let (code, unit, gen_line, ..) = host.outcome().error_positions[0];
        assert!(code >= 1000);
        let entry = c.lines.iter().find(|e| e.unit == unit && e.gen_line == gen_line).expect("the line is in the table");
        assert_eq!((entry.site.as_str(), entry.line), ("ERR-FORM ▸ BTN-BAD ▸ onClick", 8), "{entry:?}");
        // The same table is in the module, as the custom section `prlines`.
        let scan = scan_module(&c.wasm).unwrap();
        let embedded: Vec<cobolt_wasm::module::LineEntry> = serde_json::from_slice(scan.custom("prlines").expect("prlines")).unwrap();
        assert_eq!(embedded, c.lines);
        eprintln!("line table (multi={multi}): {} rows; the error at generated line {gen_line} is {} line {}", c.lines.len(), entry.site, entry.line);
    }
}

fn layered(body: &str) -> Project {
    let src = form_source("LAYER-FORM", "", &[("BTN-GO", "onClick", body)]);
    Project::new().form("LAYER-FORM", true, 1, &[("BTN-GO", "Button"), ("LBL-A", "Label")], &src).layers(&["LAYER-1"])
}

/// AC11 (R19, spec 091 R37): a property of a layer the form does not have is
/// refused at build time, naming the layer the form has and what a layer answers.
#[test]
fn unknown_layer_refused() {
    let err = layered("    MOVE 1 TO LAYER-9::Visible\n").try_compile(true).err().expect("refused");
    let all = err.all_kinded();
    let e = all.iter().find(|e| e.kind == cobolt_wasm::ErrorKind::UnknownLayer).unwrap_or_else(|| panic!("{all:?}"));
    assert!(e.message.contains("LAYER-9") && e.message.contains("LAYER-1") && e.message.contains("Visible") && e.message.contains("Name (read only)"), "{}", e.message);
    assert_eq!(e.subject, "LAYER-9");
    // A property a layer does not have, and a write to a control's Layer.
    let err = layered("    MOVE 1 TO LAYER-1::Caption\n").try_compile(true).err().expect("refused");
    assert!(err.all_kinded().iter().any(|e| e.kind == cobolt_wasm::ErrorKind::UnknownLayerProperty && e.subject == "CAPTION"), "{err:?}");
    let err = layered("    MOVE \"LAYER-1\" TO LBL-A::Layer\n").try_compile(true).err().expect("refused");
    assert!(err.all_kinded().iter().any(|e| e.kind == cobolt_wasm::ErrorKind::LayerWrite), "{err:?}");
    let err = layered("    MOVE \"X\" TO LAYER-1::Name\n").try_compile(true).err().expect("refused");
    assert!(err.to_string().contains("read but not written"), "{err}");
    eprintln!("layers: unknown layer, unknown property, a write to Layer and to Name refused at build time");
}

/// R19, A9: a layer is addressed by the id after the controls; its property is
/// written and read like a control's, and the manifest lists it.
#[test]
fn layer_visible_round_trip() {
    let p = layered("    MOVE 1 TO LAYER-1::Visible\n    DISPLAY \"SHOWN \" LAYER-1::Visible\n");
    for multi in [true, false] {
        let c = p.compile(multi);
        let m = manifest_of(&c);
        assert_eq!(m.forms[0].layers.len(), 1);
        let l = &m.forms[0].layers[0];
        assert_eq!((l.id, l.name.as_str(), l.index), (5, "LAYER-1", 0), "3 + 2 controls");
        let visible = name_id(&m, "Visible");
        let mut host = FormHost::new(&c.wasm).unwrap();
        let (inst, _) = host.open_form(0, -1);
        assert_eq!(host.event(inst, 3, m.event_index("onClick").unwrap() as i32, &[]), 0, "{:?}", host.outcome());
        assert_eq!(host.property(0, 5, visible).as_deref(), Some("1"));
        assert_eq!(common::wasm_lines(&host.outcome()), vec!["SHOWN 1"], "multi={multi}");
    }
}

/// R20: a layer has no handler — none is listed for it, and an event aimed at
/// it is not delivered.
#[test]
fn no_layer_handler() {
    let p = layered("    DISPLAY \"GO\"\n").bind("LAYER-1", "onClick");
    let c = p.compile(true);
    let m = manifest_of(&c);
    assert!(m.forms[0].handlers.iter().all(|h| h.ctrl != 5), "{:?}", m.forms[0].handlers);
    m.validate().unwrap();
    let mut host = FormHost::new(&c.wasm).unwrap();
    let (inst, _) = host.open_form(0, -1);
    assert_eq!(host.event(inst, 5, m.event_index("onClick").unwrap() as i32, &[]), -4);
    assert_eq!(host.outcome().output, "");
}

/// A form whose BTN-GET asks the backend for a customer and whose BTN-COUNT
/// counts clicks (T16).
fn remote_form() -> Project {
    let src = form_source(
        "REMOTE-FORM",
        "01 CUST GLOBAL.\n    05 CUST-ID PIC 9(4) VALUE 7.\n    05 NAME PIC X(10).\n    05 BAL PIC S9(5)V99.\n\
         01 WS-STATE PIC X(4) VALUE SPACES GLOBAL.\n01 WS-COUNT PIC 9(2) VALUE 0 GLOBAL.\n01 WS-BAL-ED PIC -(5)9.99 GLOBAL.\n",
        &[
            (
                "BTN-GET",
                "onClick",
                "    CALL \"get-customer\" WITH WEBSOCKET LINKAGE USING BY REFERENCE CUST\n        ON EXCEPTION MOVE \"ERR\" TO WS-STATE\n        NOT ON EXCEPTION MOVE \"OK\" TO WS-STATE\n    END-CALL\n\
                 \x20   MOVE BAL TO WS-BAL-ED\n    DISPLAY \"STATE \" WS-STATE NAME WS-BAL-ED\n",
            ),
            ("BTN-COUNT", "onClick", "    ADD 1 TO WS-COUNT\n    DISPLAY \"COUNT \" WS-COUNT\n"),
        ],
    );
    Project::new().form("REMOTE-FORM", true, 1, &[("BTN-GET", "Button"), ("BTN-COUNT", "Button")], &src)
}

/// AC5 (R9, R10; ABI A5, A6): a handler that calls the backend stops at the
/// call (SUSPENDED); two events raised meanwhile are queued; the answer frame
/// resumes it and its group takes the answer; the queued events then run, the
/// second one calling again, and an error frame takes it to ON EXCEPTION
/// with the group untouched.
#[test]
fn suspend_resume_with_queued_events() {
    let p = remote_form();
    for multi in [true, false] {
        let c = p.compile(multi);
        let m = manifest_of(&c);
        assert!(m.imports.iter().any(|i| i == "remote"), "{:?}", m.imports);
        let click = m.event_index("onClick").unwrap() as i32;
        let mut host = FormHost::new(&c.wasm).unwrap();
        let (inst, _) = host.open_form(0, -1);
        assert_eq!(host.event(inst, 3, click, &[]), 1, "SUSPENDED at the call: {:?}", host.outcome());
        let task = host.tick();
        assert_eq!(task, inst);
        let frames = host.remote_frames();
        assert_eq!(
            String::from_utf8(frames[0].clone()).unwrap(),
            r#"{"program":"GET-CUSTOMER","args":[{"mode":"ref","json":{"CUST-ID":7,"NAME":"","BAL":0.00}}],"returning":false}"#
        );
        // Two events while the task waits: queued, nothing runs.
        assert_eq!(host.post(inst, 4, click, &[]), None);
        assert_eq!(host.post(inst, 3, click, &[]), None);
        assert_eq!(host.queued(), 2);
        assert!(host.outcome().output.is_empty(), "{:?}", host.outcome());
        // The answer.
        let answer = br#"{"id":0,"result":{"args":[{"CUST-ID":7,"NAME":"Ana","BAL":-12.5}]}}"#;
        assert_eq!(host.resume(task, answer), 0, "{:?}", host.outcome());
        // The queued events: COUNT runs, GET calls again and waits.
        assert_eq!(host.pump(), Some(1));
        assert_eq!(host.queued(), 0);
        assert_eq!(host.remote_frames().len(), 2);
        let refused = br#"{"id":1,"error":{"class":"transport","code":7,"message":"not connected","may_have_been_received":false}}"#;
        let again = host.tick();
        assert_eq!(host.resume(again, refused), 0, "{:?}", host.outcome());
        let o = host.outcome();
        assert!(o.trapped.is_none() && o.errors.is_empty(), "{o:?}");
        assert_eq!(
            common::wasm_lines(&o),
            vec!["STATE OK  Ana          -12.50", "COUNT 01", "STATE ERR Ana          -12.50"],
            "multi={multi}"
        );
        eprintln!(
            "suspend/resume (multi={multi}): SUSPENDED, 2 events queued, result frame then error frame; {} remote frames, {} lines",
            host.remote_frames().len(),
            o.output.lines().count()
        );
    }
}

/// R9/R11: an answer that does not fit its group is a failure of the call
/// (ON EXCEPTION, nothing written); without ON EXCEPTION a failed call is a
/// run-time error and the application keeps running.
#[test]
fn a_bad_answer_or_a_call_without_on_exception() {
    let src = form_source(
        "R2-FORM",
        "01 REC GLOBAL.\n    05 QTY PIC 9(2) VALUE 5.\n01 WS-STATE PIC X(4) VALUE SPACES GLOBAL.\n",
        &[
            (
                "BTN-A",
                "onClick",
                "    CALL \"P\" WITH WEBSOCKET LINKAGE USING BY REFERENCE REC\n        ON EXCEPTION MOVE \"ERR\" TO WS-STATE\n    END-CALL\n    DISPLAY WS-STATE QTY\n",
            ),
            ("BTN-B", "onClick", "    CALL \"P\" WITH WEBSOCKET LINKAGE USING BY REFERENCE REC\n    DISPLAY \"NOT REACHED\"\n"),
        ],
    );
    let p = Project::new().form("R2-FORM", true, 1, &[("BTN-A", "Button"), ("BTN-B", "Button")], &src);
    for multi in [true, false] {
        let c = p.compile(multi);
        let click = manifest_of(&c).event_index("onClick").unwrap() as i32;
        let mut host = FormHost::new(&c.wasm).unwrap();
        let (inst, _) = host.open_form(0, -1);
        assert_eq!(host.event(inst, 3, click, &[]), 1);
        // QTY holds two digits: 123 does not fit, so nothing is written.
        assert_eq!(host.resume(inst, br#"{"id":0,"result":{"args":[{"QTY":123}]}}"#), 0);
        assert_eq!(common::wasm_lines(&host.outcome()), vec!["ERR 05"]);
        assert_eq!(host.event(inst, 4, click, &[]), 1);
        assert_eq!(host.resume(inst, br#"{"id":1,"error":{"class":"transport","code":7,"message":"down"}}"#), -10);
        let o = host.outcome();
        assert_eq!(o.error_positions.last().map(|e| e.0), Some(1110), "{o:?}");
        assert!(!o.output.contains("NOT REACHED"));
        assert_eq!(host.event(inst, 3, click, &[]), 1, "the application keeps running");
        eprintln!("bad answer (multi={multi}): ON EXCEPTION with the group untouched; without it, run-time error 1110");
    }
}

/// ABI §12.1: a verb that waits (a message box) suspends the handler, and
/// `pr_resume` continues it after the dialog.
#[test]
fn a_message_box_suspends_the_handler() {
    let src = form_source(
        "MB-FORM",
        "",
        &[("BTN-ASK", "onClick", "    DISPLAY \"BEFORE\"\n    COBOL::MessageBox(\"Saved\", \"Title\", \"OK\")\n    DISPLAY \"AFTER\"\n")],
    );
    let p = Project::new().form("MB-FORM", true, 1, &[("BTN-ASK", "Button")], &src);
    for multi in [true, false] {
        let c = p.compile(multi);
        let click = manifest_of(&c).event_index("onClick").unwrap() as i32;
        let mut host = FormHost::new(&c.wasm).unwrap();
        let (inst, _) = host.open_form(0, -1);
        assert_eq!(host.event(inst, 3, click, &[]), 1, "{:?}", host.outcome());
        let o = host.outcome();
        assert_eq!(common::wasm_lines(&o), vec!["BEFORE"]);
        assert_eq!(o.calls.len(), 1, "the dialog's CALL was flushed before the suspension: {o:?}");
        assert_eq!(host.resume(inst, &[]), 0);
        assert_eq!(common::wasm_lines(&host.outcome()), vec!["BEFORE", "AFTER"]);
        eprintln!("message box (multi={multi}): SUSPENDED after 1 CALL, resumed to the end");
    }
}

/// R9: a handler that runs longer than the budget yields at a loop back-edge
/// and is resumed on the host's next turn; where it yielded is invisible —
/// budgets of 1, 7 and 20 000 blocks give the same data and the same commands.
#[test]
fn yield_is_invisible() {
    let src = form_source(
        "LOOP-FORM",
        "01 WS-I PIC 9(4) VALUE 0 GLOBAL.\n01 WS-TOTAL PIC 9(8) VALUE 0 GLOBAL.\n",
        &[(
            "BTN-RUN",
            "onClick",
            "    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 200\n        ADD WS-I TO WS-TOTAL\n        IF FUNCTION MOD(WS-I, 50) = 0\n            MOVE WS-TOTAL TO LBL-OUT::Caption\n            DISPLAY \"AT \" WS-I \" \" WS-TOTAL\n        END-IF\n    END-PERFORM\n    DISPLAY \"DONE \" WS-TOTAL\n",
        )],
    );
    let p = Project::new().form("LOOP-FORM", true, 1, &[("BTN-RUN", "Button"), ("LBL-OUT", "Label")], &src);
    let specs = p.form_specs();
    let mut runs: Vec<(String, Vec<String>, u32, u32)> = Vec::new();
    for multi in [true, false] {
        for budget in [1, 7, 20_000] {
            let c = compile_project(&specs, &Options { multi_memory: multi, budget, name: "loop".into(), ..Options::default() }).unwrap();
            let click = manifest_of(&c).event_index("onClick").unwrap() as i32;
            let mut host = FormHost::new(&c.wasm).unwrap();
            let (inst, mut opening) = host.open_form(0, -1);
            let mut yields = 0u32;
            // The start-up run yields too under a small budget.
            while opening == 2 {
                yields += 1;
                opening = host.resume(inst, &[]);
            }
            assert_eq!(opening, 0, "the form opened");
            let mut status = host.event(inst, 3, click, &[]);
            while status == 2 {
                yields += 1;
                assert!(host.pump().is_none(), "nothing else is delivered while the task is live");
                status = host.resume(inst, &[]);
            }
            assert_eq!(status, 0, "budget {budget}: {:?}", host.outcome());
            let o = host.outcome();
            assert!(o.trapped.is_none() && o.errors.is_empty(), "{o:?}");
            runs.push((o.output.clone(), o.commands.clone(), yields, o.flushes));
            eprintln!("yield (multi={multi}, budget {budget}): {yields} yields, {} flushes, {} commands", o.flushes, o.commands.len());
        }
    }
    let (out0, cmds0, _, _) = runs[0].clone();
    assert!(out0.ends_with("DONE 00020100\n"), "{out0}");
    assert_eq!(cmds0.len(), 4, "{cmds0:?}");
    for (out, cmds, _, _) in &runs {
        assert_eq!((out, cmds), (&out0, &cmds0));
    }
    assert!(runs[0].2 > 100 && runs[2].2 == 0, "budget 1 yields often, 20 000 never: {:?}", runs.iter().map(|r| r.2).collect::<Vec<_>>());
    eprintln!("yield: 6 runs (2 variants x 3 budgets), identical output and commands");
}
