// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-018 T24 (R6, R16; AC2): a Canvas2D's drawing methods in a generated module are a display
//! list in the module's memory and reach the host as one `DRAW` (ABI A23), not one call per shape.

mod common;

use cobolt_graphics_wire as wire;
use cobolt_wasm::host::FormHost;
use cobolt_wasm::module::{compile_project, FormSpec};
use cobolt_wasm::Options;
use cobolt_wasm_abi::manifest::Manifest;
use cobolt_wasm_abi::scan::scan_module;

/// A form program in the shape `cobolt-codegen` writes, with one `onClick` handler on `GO`.
fn form_source(data: &str, body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. MAIN-FORM.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 COBOL-QUIT PIC 9 VALUE 0.\n01 COBOL-EVENT-ID PIC X(64) VALUE SPACES.\n\
         01 COBOL-CONTROL-ID PIC X(64) VALUE SPACES.\n01 FORM-NAME PIC X(64) VALUE 'MAIN-FORM'.\n{data}\
         PROCEDURE DIVISION.\nCOBOL-MAIN.\n    COBOL::\"INIT-FORM\" ( FORM-NAME )\n    PERFORM COBOL-EVENT-LOOP\n    STOP RUN.\n\
         COBOL-EVENT-LOOP.\n    PERFORM UNTIL COBOL-QUIT = 1\n        COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )\n\
         \x20       EVALUATE COBOL-CONTROL-ID\n            WHEN \"GO\"\n                EVALUATE COBOL-EVENT-ID\n                    WHEN \"onClick\"\n\
         \x20                       CALL \"GO--ONCLICK\"\n                END-EVALUATE\n        END-EVALUATE\n    END-PERFORM.\n\
         IDENTIFICATION DIVISION.\nPROGRAM-ID. GO--ONCLICK IS COMMON PROGRAM.\nPROCEDURE DIVISION.\n{body}    GOBACK.\nEND PROGRAM GO--ONCLICK.\n\
         END PROGRAM MAIN-FORM.\n"
    )
}

fn compile(controls: &[(&str, &str)], data: &str, body: &str) -> (cobolt_wasm::Compiled, Manifest) {
    let program = common::parse(&form_source(data, body));
    let spec = FormSpec {
        name: "MAIN-FORM".into(),
        main: true,
        instances: 1,
        program: &program,
        controls: controls.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect(),
        bindings: vec![("GO".into(), "onClick".into())],
        layers: vec![],
        lines: vec![],
    };
    let c = compile_project(&[spec], &Options { multi_memory: true, name: "demo".into(), ..Options::default() }).unwrap_or_else(|e| panic!("compile_project failed: {e}"));
    let m = scan_module(&c.wasm).unwrap().embedded_manifest().unwrap();
    (c, m)
}

/// The version-1 list a program of N shapes must produce: `SetFillColor`, N `FillRect`, `Present`.
fn expected_list(n: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    {
        struct V<'a>(&'a mut Vec<u8>);
        impl wire::Sink for V<'_> {
            fn put(&mut self, b: &[u8]) -> Result<(), wire::Full> {
                self.0.extend_from_slice(b);
                Ok(())
            }
        }
        let mut s = V(&mut payload);
        wire::encode::set_fill_color(&mut s, 0xFF00_00FF).unwrap();
        for i in 1..=n {
            wire::encode::fill_rect(&mut s, i as f32, 10.0, 20.0, 30.0).unwrap();
        }
        wire::encode::present(&mut s).unwrap();
    }
    let mut out = Vec::new();
    out.extend_from_slice(&wire::MAGIC);
    out.extend_from_slice(&wire::VERSION.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(n + 2).to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    out
}

/// AC2: 10,000 shapes then `Present` are one `flush` that carries one `DRAW`, and the list in the
/// graphics region is, byte for byte, what the shared encoder writes for the same calls.
#[test]
fn ten_thousand_shapes_then_present_are_one_flush_with_one_draw() {
    const N: u32 = 10_000;
    let body = format!(
        "    CNV-1::SetFillColor( \"#FF0000\" )\n    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > {N}\n        CNV-1::FillRect( WS-I 10 20 30 )\n    END-PERFORM\n    CNV-1::Present()\n"
    );
    let (c, m) = compile(&[("GO", "Button"), ("CNV-1", "Canvas2D")], "01 WS-I PIC 9(5) VALUE 0 GLOBAL.\n", &body);
    assert!(m.limits.gfx_size > 0, "a module with a Canvas2D has a graphics region");
    m.validate().unwrap();
    let mut host = FormHost::new(&c.wasm).unwrap();
    let (inst, status) = host.open_form(0, -1);
    assert_eq!((inst, status), (0, 0), "{:?}", host.outcome());
    let before = host.outcome();
    let (flushes_before, draws_before) = (before.flushes, before.draws.len());
    let click = m.event_index("onClick").unwrap() as i32;
    // The loop is long: the task yields at a back-edge when its budget runs out and is resumed.
    let (mut status, mut yields) = (host.event(inst, 3, click, &[]), 0);
    while status == 2 {
        yields += 1;
        let task = host.tick();
        status = host.resume(task, &[]);
    }
    assert_eq!(status, 0, "{:?}", host.outcome());
    let o = host.outcome();
    assert!(o.trapped.is_none() && o.errors.is_empty(), "{o:?}");
    let flushes = o.flushes - flushes_before;
    let draws = &o.draws[draws_before..];
    assert_eq!((flushes, draws.len()), (1, 1), "calls: {} flushes, {} DRAW; commands {:?}", flushes, draws.len(), &o.commands[..o.commands.len().min(4)]);
    assert!(o.calls.is_empty(), "no drawing method reached the host as a CALL: {:?}", o.calls.len());
    let want = expected_list(N);
    assert_eq!(draws[0].2.len(), want.len());
    assert!(draws[0].2 == want, "the list differs from the shared encoder's");
    eprintln!(
        "AC2: {N} shapes + SetFillColor + Present = {} commands -> {flushes} flush, {} DRAW; list {} bytes (graphics region {} bytes); host CALLs {}; the task yielded {yields} times on the way",
        N + 2,
        draws.len(),
        draws[0].2.len(),
        m.limits.gfx_size,
        o.calls.len()
    );
}

/// A module without a Canvas2D has no graphics region and never emits `DRAW`.
#[test]
fn a_module_without_a_canvas_never_emits_draw() {
    let (c, m) = compile(&[("GO", "Button"), ("LBL-1", "Label")], "", "    MOVE \"hi\" TO LBL-1::Caption\n");
    assert_eq!(m.limits.gfx_size, 0);
    let mut host = FormHost::new(&c.wasm).unwrap();
    let (inst, _) = host.open_form(0, -1);
    let click = m.event_index("onClick").unwrap() as i32;
    assert_eq!(host.event(inst, 3, click, &[]), 0);
    let o = host.outcome();
    assert!(o.draws.is_empty() && o.errors.is_empty(), "{o:?}");
    eprintln!("no canvas: gfx_size 0, {} DRAW commands, {} sets", o.draws.len(), o.sets.len());
}

/// AC13: the list a generated module writes for a program is, byte for byte, the list the
/// interpreter's Canvas2D records for the same calls (the colour as text and as a number, decimals,
/// negative numbers, text, the 8-bit modes).
#[test]
fn the_module_and_the_interpreter_write_the_same_list() {
    use cobolt_graphics::canvas2d::Canvas2D;
    use cobolt_graphics::value::Value;
    use cobolt_graphics::Limits;

    let calls: Vec<(&str, Vec<Value>, &str)> = vec![
        ("Save", vec![], ""),
        ("SetFillColor", vec![Value::Text("#FF8000".into())], "\"#FF8000\""),
        ("SetStrokeColor", vec![Value::Int(4278190335)], "4278190335"),
        ("SetLineWidth", vec![Value::Num(2.5)], "2.5"),
        ("FillRect", vec![Value::Num(12.5), Value::Int(-7), Value::Int(100), Value::Num(33.25)], "12.5 -7 100 33.25"),
        ("DrawCircle", vec![Value::Int(50), Value::Int(60), Value::Num(12.75), Value::Int(1)], "50 60 12.75 1"),
        ("BeginPath", vec![], ""),
        ("MoveTo", vec![Value::Num(0.1), Value::Num(0.2)], "0.1 0.2"),
        ("LineTo", vec![Value::Int(30), Value::Num(-4.125)], "30 -4.125"),
        ("Arc", vec![Value::Int(40), Value::Int(40), Value::Int(15), Value::Num(0.0), Value::Num(1.5707), Value::Int(0)], "40 40 15 0 1.5707 0"),
        ("Fill", vec![Value::Int(0)], "0"),
        ("SetFont", vec![Value::Text("bold 14px sans-serif".into())], "\"bold 14px sans-serif\""),
        ("DrawText", vec![Value::Text("Hello".into()), Value::Int(10), Value::Int(20), Value::Int(0), Value::Int(0)], "\"Hello\" 10 20 0 0"),
        ("Restore", vec![], ""),
        ("Present", vec![], ""),
    ];

    let mut canvas = Canvas2D::new(320, 240, 1.0, Limits::default()).unwrap();
    for (name, values, _) in &calls {
        canvas.record(name, values).unwrap_or_else(|e| panic!("the interpreter refused {name}: {e}"));
    }
    let interpreter = canvas.list().take().unwrap();

    let body: String = calls
        .iter()
        .map(|(n, _, a)| if a.is_empty() { format!("    CNV-1::{n}()\n") } else { format!("    CNV-1::{n}( {a} )\n") })
        .collect();
    let (c, m) = compile(&[("GO", "Button"), ("CNV-1", "Canvas2D")], "", &body);
    let mut host = FormHost::new(&c.wasm).unwrap();
    let (inst, _) = host.open_form(0, -1);
    let click = m.event_index("onClick").unwrap() as i32;
    assert_eq!(host.event(inst, 3, click, &[]), 0, "{:?}", host.outcome());
    let o = host.outcome();
    assert!(o.errors.is_empty() && o.calls.is_empty(), "everything was encoded in the module: {} host calls, {:?}", o.calls.len(), o.errors);
    assert_eq!(o.draws.len(), 1);
    let module = &o.draws[0].2;
    assert_eq!(module.len(), interpreter.len(), "interpreter {} bytes, module {} bytes", interpreter.len(), module.len());
    assert!(*module == interpreter, "the lists differ");
    eprintln!("AC13: {} calls -> interpreter {} bytes, module {} bytes, identical", calls.len(), interpreter.len(), module.len());
}
