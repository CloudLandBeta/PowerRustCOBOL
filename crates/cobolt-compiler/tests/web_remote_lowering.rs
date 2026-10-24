// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-005 T14: the web build's lowering of class C member calls
//! (`cobolt_compiler::web_remote::lower_class_c_calls`). A form program in the
//! shape `cobolt-codegen` writes is lowered, then:
//!
//! - analysed by `cobolt-semantic` (no error);
//! - run by the interpreter with a scripted transport — the endpoint and the
//!   arguments it sends, the answer's value in the receiver, the properties
//!   the backend reported on the control, and its events raised after the
//!   handler; a failed call setting `LastError` and raising `onError`;
//! - compiled into a browser module and run on the generator's test host — the
//!   same frame, then `SET_NAMED` and `RAISE` (ABI A14) from the answer;
//!
//! and what the pass refuses, and that only the web build calls it.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use cobolt_ast::program::Program;
use cobolt_compiler::web_remote::lower_class_c_calls;
use cobolt_forms::{Control, ControlType, Form};
use cobolt_runtime::Interpreter;
use cobolt_wire::call::{ArgMode, CallReply, CallRequest, Transport};
use cobolt_wire::error::{codes, CallError};

fn parse(src: &str) -> Program {
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<_> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
    assert!(errors.is_empty(), "{errors:?}");
    parsed.program.expect("a program")
}

fn shop() -> Form {
    let mut f = Form::new("SHOP", "Shop", 400, 300);
    f.controls.push(Control::new("REST-1", ControlType::RestClient, 0, 0));
    f.controls.push(Control::new("BTN-GO", ControlType::Button, 0, 0));
    f
}

/// A form program as `cobolt-codegen` shapes it: the data, the event loop
/// dispatching to the handlers, the handlers as nested COMMON programs. `main`
/// runs before the loop (the interpreter tests click by calling the handler).
fn form_program(main: &str, handlers: &[(&str, &str)], data: &str) -> String {
    let mut s = format!(
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. SHOP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-QUIT PIC 9 VALUE 0.
       01 COBOL-EVENT-ID PIC X(64) VALUE SPACES.
       01 COBOL-CONTROL-ID PIC X(64) VALUE SPACES.
       01 FORM-NAME PIC X(64) VALUE \"SHOP\".
       01 WS-ANSWER PIC X(80) VALUE SPACES GLOBAL.
       01 WS-BODY PIC X(40) VALUE \"two lamps\" GLOBAL.
{data}       PROCEDURE DIVISION.
       COBOL-MAIN.
{main}           PERFORM COBOL-EVENT-LOOP
           DISPLAY \"ANSWER=[\" WS-ANSWER::Trim() \"]\"
           STOP RUN.
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               MOVE SPACES TO COBOL-EVENT-ID COBOL-CONTROL-ID
               COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
"
    );
    // (The ids are cleared first: these tests end the loop by closing the
    // event channel, which leaves them as the last event left them; a real
    // host ends it with `__QUIT__`.)
    // One WHEN per control, its events inside, as codegen writes the loop.
    let mut controls: Vec<&str> = Vec::new();
    for (p, _) in handlers {
        let c = p.split_once("--").unwrap().0;
        if !controls.contains(&c) {
            controls.push(c);
        }
    }
    for c in controls {
        s.push_str(&format!("                   WHEN \"{c}\"\n                       EVALUATE COBOL-EVENT-ID\n"));
        for (p, _) in handlers.iter().filter(|(p, _)| p.split_once("--").unwrap().0 == c) {
            let e = p.split_once("--").unwrap().1;
            let ev = format!("on{}{}", &e[2..3], e[3..].to_ascii_lowercase());
            s.push_str(&format!("                           WHEN \"{ev}\"\n                               CALL \"{p}\"\n"));
        }
        s.push_str("                       END-EVALUATE\n");
    }
    s.push_str("               END-EVALUATE\n           END-PERFORM.\n");
    for (p, body) in handlers {
        s.push_str(&format!(
            "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. {p} IS COMMON PROGRAM.\n       PROCEDURE DIVISION.\n{body}           GOBACK.\n       END PROGRAM {p}.\n"
        ));
    }
    s.push_str("       END PROGRAM SHOP.\n");
    s
}

const CLICK: &str = "           MOVE REST-1::Post(\"orders\" WS-BODY) TO WS-ANSWER\n           DISPLAY \"CLICK-END\"\n";
const ON_COMPLETE: &str = "           DISPLAY \"COMPLETE \" REST-1::StatusCode \" [\" REST-1::LastError \"]\"\n";
const ON_ERROR: &str = "           DISPLAY \"ERROR [\" REST-1::LastError \"]\"\n";

fn lowered(main: &str) -> Program {
    let src = form_program(main, &[("BTN-GO--ONCLICK", CLICK), ("REST-1--ONCOMPLETE", ON_COMPLETE), ("REST-1--ONERROR", ON_ERROR)], "");
    let mut program = parse(&src);
    let n = lower_class_c_calls(&mut program, &shop()).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(n, 1, "one site lowered");
    program
}

type Handler = Box<dyn FnMut(&CallRequest) -> Result<CallReply, CallError> + Send>;

struct Double {
    handler: Handler,
    seen: Arc<Mutex<Vec<CallRequest>>>,
}

impl Transport for Double {
    fn call(&mut self, request: &CallRequest) -> Result<CallReply, CallError> {
        self.seen.lock().unwrap().push(request.clone());
        (self.handler)(request)
    }
}

/// Run `program` as a form's interpreter with no UI events: the raised events
/// are what the loop is given; then the channel is closed and the loop ends.
fn run(program: Program, handler: Handler) -> (Vec<String>, Vec<CallRequest>) {
    let (event_tx, event_rx) = mpsc::channel();
    drop(event_tx);
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.seed_objects(vec![("REST-1".to_string(), "RestClient".to_string(), Vec::<(String, String)>::new())]);
    interp.set_control_ids(["REST-1", "BTN-GO"]);
    let seen = Arc::new(Mutex::new(Vec::new()));
    interp.set_remote_transport(Box::new(Double { handler, seen: seen.clone() }));
    interp.run().unwrap_or_else(|e| panic!("run: {e}"));
    let shown = display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect();
    let seen = seen.lock().unwrap().clone();
    (shown, seen)
}

const ENDPOINT: &str = "RC-4-SHOP-6-REST-1-POST-1";
const ANSWER: &str = r#"{"RESULT-VALUE":"order 9 made","LAST-ERROR":"","PROP-COUNT":1,"PROPS":[{"PROP-NAME":"StatusCode","PROP-VALUE":"201"}],"EVENT-COUNT":1,"EVENTS":[{"EVENT-NAME":"onComplete"}],"ROWS-RETURNED":0,"ROWS":[]}"#;

#[test]
fn the_scripted_answer_is_one_the_backend_program_can_give() {
    // The answer of the tests below decodes into the catalogue's own result
    // layout — the LINKAGE of the generated backend program.
    let row = cobolt_wire::controls::find("RestClient", "POST").unwrap();
    let layout = row.result_layout();
    cobolt_wire::mapping::decode(&layout, &cobolt_wire::node::Node::initial(&layout), ANSWER, 1 << 20).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn a_lowered_form_program_passes_the_analyser() {
    let program = lowered("");
    let errors: Vec<String> =
        cobolt_semantic::analyze(&program).errors().map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn the_interpreter_sends_the_endpoint_moves_the_answer_sets_the_properties_and_raises_the_events_after() {
    let (shown, seen) = run(
        lowered("           CALL \"BTN-GO--ONCLICK\"\n"),
        Box::new(|_| Ok(CallReply { args: vec![None], returning: Some(ANSWER.to_owned()), warnings: Vec::new() })),
    );
    assert_eq!(seen.len(), 1, "exactly one remote call");
    let r = &seen[0];
    assert_eq!(r.program, ENDPOINT);
    assert!(r.wants_returning);
    assert_eq!(r.args.len(), 1);
    assert_eq!(r.args[0].mode, ArgMode::Value);
    // The resource ("orders") is fixed at Build (R17): only the body travels.
    assert_eq!(r.args[0].json, r#"{"ARG-COUNT":1,"ARGS":[{"ARG-VALUE":"two lamps"}]}"#);
    // The handler finishes first; onComplete runs after it, as on the desktop.
    assert_eq!(shown, vec!["CLICK-END", "COMPLETE 201 []", "ANSWER=[order 9 made]"], "{shown:?}");
    eprintln!("interpreter: {} → {}; lines {shown:?}", r.program, r.args[0].json);
}

#[test]
fn a_failed_call_sets_last_error_and_raises_on_error_as_a_failing_method_does() {
    let (shown, seen) = run(
        lowered("           MOVE \"stale\" TO WS-ANSWER\n           CALL \"BTN-GO--ONCLICK\"\n"),
        Box::new(|_| Err(CallError::transport(codes::NO_CONNECTION, "the backend is down"))),
    );
    assert_eq!(seen.len(), 1);
    assert_eq!(shown, vec!["CLICK-END", "ERROR [the backend is down]", "ANSWER=[]"], "{shown:?}");
}

#[test]
fn only_the_web_build_lowers_and_a_program_without_a_served_site_is_unchanged() {
    // A property read and a non-class-C call are not sites: nothing changes.
    let src = form_program(
        "",
        &[("BTN-GO--ONCLICK", "           MOVE REST-1::BaseURL TO WS-ANSWER\n           MOVE BTN-GO::Caption TO WS-ANSWER\n")],
        "",
    );
    let mut program = parse(&src);
    let before = serde_json::to_string(&program).unwrap();
    assert_eq!(lower_class_c_calls(&mut program, &shop()).unwrap(), 0);
    assert_eq!(serde_json::to_string(&program).unwrap(), before);
    // The desktop paths never call the pass: only the web build does.
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut callers = Vec::new();
    let mut stack = vec![crates.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            if p.is_dir() {
                if !matches!(name.as_str(), "target" | "target-player" | "vendor" | "tests" | "node_modules") {
                    stack.push(p);
                }
            } else if name.ends_with(".rs") {
                let text = std::fs::read_to_string(&p).unwrap_or_default();
                if text.contains("lower_class_c_calls(") {
                    callers.push(p.strip_prefix(crates).unwrap().to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    callers.sort();
    assert_eq!(callers, vec!["cobolt-compiler/src/web_remote.rs", "cobolt-project-tools/src/web/generator.rs"]);
}

#[test]
fn a_call_the_desktop_would_make_conditionally_or_repeatedly_is_refused_at_its_line() {
    for (body, why) in [
        ("           PERFORM UNTIL REST-1::Get(\"x\") = \"done\"\n               DISPLAY \"WAIT\"\n           END-PERFORM\n", "loop"),
        ("           IF WS-ANSWER = SPACES AND REST-1::Get(\"x\") = \"y\"\n               DISPLAY \"Y\"\n           END-IF\n", "AND or OR"),
        ("           EVALUATE TRUE\n               WHEN REST-1::Get(\"x\") = \"y\"\n                   DISPLAY \"Y\"\n           END-EVALUATE\n", "WHEN"),
    ] {
        let mut program = parse(&form_program("", &[("BTN-GO--ONCLICK", body)], ""));
        let errs = lower_class_c_calls(&mut program, &shop()).expect_err(why);
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(errs[0].message.contains(why) && errs[0].message.contains("REST-1::Get"), "{}", errs[0].message);
        assert!(errs[0].line > 0);
    }
    // Hoisted where it is evaluated once: an IF's single comparison, a nested call.
    let mut program = parse(&form_program(
        "",
        &[("BTN-GO--ONCLICK", "           IF REST-1::Get(\"x\") = \"y\"\n               MOVE REST-1::Post(\"o\" REST-1::Get(\"z\")) TO WS-ANSWER\n           END-IF\n")],
        "",
    ));
    assert_eq!(lower_class_c_calls(&mut program, &shop()).unwrap(), 3);
    assert!(cobolt_semantic::analyze(&program).errors().next().is_none());
}

#[test]
fn an_item_named_like_a_member_of_the_groups_is_refused() {
    let mut program = parse(&form_program("", &[("BTN-GO--ONCLICK", CLICK)], "       01 EVENTS PIC X(10) GLOBAL.\n"));
    let errs = lower_class_c_calls(&mut program, &shop()).unwrap_err();
    assert!(errs.iter().any(|e| e.message.contains("EVENTS") && e.message.contains("rename")), "{errs:?}");
}

// ── the browser module: the same pass, the same call ─────────────────────────

#[test]
fn the_module_sends_the_same_frame_and_applies_the_answer_with_set_named_and_raise() {
    use cobolt_wasm::host::FormHost;
    use cobolt_wasm::module::{compile_project, FormSpec};
    // The module's form: INIT-FORM and the loop, the click handler bound.
    let src = form_program(
        "           COBOL::\"INIT-FORM\" ( FORM-NAME )\n",
        &[("BTN-GO--ONCLICK", CLICK), ("REST-1--ONCOMPLETE", ON_COMPLETE), ("REST-1--ONERROR", ON_ERROR)],
        "",
    );
    let mut program = parse(&src);
    assert_eq!(lower_class_c_calls(&mut program, &shop()).unwrap(), 1);
    let spec = FormSpec {
        name: "SHOP".into(),
        main: true,
        instances: 1,
        program: &program,
        controls: vec![("REST-1".into(), "RestClient".into()), ("BTN-GO".into(), "Button".into())],
        bindings: vec![("BTN-GO".into(), "onClick".into()), ("REST-1".into(), "onComplete".into()), ("REST-1".into(), "onError".into())],
        layers: vec![],
        lines: vec![],
    };
    for multi in [true, false] {
        let c = compile_project(std::slice::from_ref(&spec), &cobolt_wasm::Options { multi_memory: multi, name: "shop".into(), ..Default::default() })
            .unwrap_or_else(|e| panic!("the lowered program compiles into a module: {e}"));
        let m = cobolt_wasm_abi::scan::scan_module(&c.wasm).unwrap().embedded_manifest().unwrap();
        let click = m.event_index("onClick").unwrap() as i32;
        let mut host = FormHost::new(&c.wasm).unwrap();
        let (inst, _) = host.open_form(0, -1);
        // BTN-GO is control 4 (REST-1 is 3).
        assert_eq!(host.event(inst, 4, click, &[]), 1, "SUSPENDED at the call: {:?}", host.outcome());
        let frame = String::from_utf8(host.remote_frames()[0].clone()).unwrap();
        assert_eq!(
            frame,
            format!(r#"{{"program":"{ENDPOINT}","args":[{{"mode":"value","json":{{"ARG-COUNT":1,"ARGS":[{{"ARG-VALUE":"two lamps"}}]}}}}],"returning":true}}"#),
            "the interpreter's request, byte for byte"
        );
        let task = host.tick();
        let answer = format!(r#"{{"id":0,"result":{{"args":[null],"returning":{ANSWER}}}}}"#);
        assert_eq!(host.resume(task, answer.as_bytes()), 0, "{:?}", host.outcome());
        let o = host.outcome();
        assert!(o.trapped.is_none() && o.errors.is_empty(), "{o:?}");
        let i = inst as u32;
        assert_eq!(o.named_sets, vec![(i, 3, "StatusCode".to_string(), "201".to_string())], "{:?}", o.commands);
        assert_eq!(o.raised, vec![(i, 3, "onComplete".to_string(), String::new())]);
        assert!(o.output.contains("CLICK-END"), "{:?}", o.output);
        // A failure: LastError from the error frame, and onError.
        assert_eq!(host.event(inst, 4, click, &[]), 1);
        let task = host.tick();
        let refused = br#"{"id":1,"error":{"class":"transport","code":1000,"message":"the backend is down","may_have_been_received":false}}"#;
        assert_eq!(host.resume(task, refused), 0, "{:?}", host.outcome());
        let o = host.outcome();
        assert_eq!(o.raised.last(), Some(&(i, 3, "onError".to_string(), String::new())), "{:?}", o.commands);
        let last_error = m.names.iter().position(|n| n == "LastError").expect("LastError is a name") as u32;
        assert!(
            o.sets.iter().any(|(si, sc, segs, v)| *si == i && *sc == 3 && segs == &vec![last_error] && v == "the backend is down"),
            "{:?}",
            o.sets
        );
        eprintln!("module (multi={multi}): frame {} bytes; named sets {:?}; raised {:?}", frame.len(), o.named_sets, o.raised);
    }
}
