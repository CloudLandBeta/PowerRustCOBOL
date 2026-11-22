// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `INITIALIZE` of a table nested more than four levels deep, in a WebAssembly module, browser and
//! desktop. `rt_initialize` walks four levels of OCCURS under a leaf; a module used to refuse the
//! statement below more of them, though COBOL-2002 allows seven. The generator now resets the outer
//! levels one occurrence at a time, each with its own entry. Every case runs on the interpreter, on a
//! browser module (both memory layouts) and on a desktop module, and the lines `DISPLAY` writes must
//! be identical.

mod common;

use cobolt_wasm::host::{run_native, NativeInput};
use cobolt_wasm::{compile_program, Options};
use std::time::Instant;

fn program(ws: &str, body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. INITD.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{ws}PROCEDURE DIVISION.\nMAIN-PARA.\n{body}    STOP RUN.\n"
    )
}

/// `levels` nested tables of `per` occurrences under `01 T`, the leaf `PIC 9`.
fn nest(levels: usize, per: usize) -> String {
    let names = ["A", "B", "C", "D", "E", "F", "G"];
    let mut s = String::from("01 T.\n");
    for (k, n) in names.iter().enumerate().take(levels) {
        s.push_str(&format!("{}{:02} {n} OCCURS {per}", " ".repeat(k + 1), 5 + 5 * k));
        if k + 1 == levels {
            s.push_str(" PIC 9");
        }
        s.push_str(".\n");
    }
    s
}

/// (what the case measures, the data, the statements).
fn cases() -> Vec<(String, String, String)> {
    let fill = "    MOVE ALL \"7\" TO T\n";
    let mixed = "01 T.\n 05 A OCCURS 3.\n  10 B OCCURS 2.\n   15 C OCCURS 2.\n    20 D OCCURS 2.\n     25 E OCCURS 2.\n      30 F OCCURS 2.\n       35 N PIC 9.\n       35 S PIC X.\n".to_string();
    vec![
        ("four levels, the most one entry walks".into(), nest(4, 2), format!("{fill}    INITIALIZE T\n    DISPLAY T\n")),
        ("five levels".into(), nest(5, 2), format!("{fill}    INITIALIZE T\n    DISPLAY T\n")),
        ("six levels".into(), nest(6, 2), format!("{fill}    INITIALIZE T\n    DISPLAY T\n")),
        ("seven levels, the most COBOL-2002 allows".into(), nest(7, 2), format!("{fill}    INITIALIZE T\n    DISPLAY T\n")),
        ("five levels of three".into(), nest(5, 3), format!("{fill}    INITIALIZE T\n    DISPLAY T\n")),
        (
            "six levels with a number and a character at each leaf".into(),
            mixed.clone(),
            "    MOVE ALL \"7\" TO T\n    INITIALIZE T\n    DISPLAY \"[\" T \"]\"\n".into(),
        ),
        ("one occurrence of the outer table, five levels below it".into(), nest(6, 2), format!("{fill}    INITIALIZE A(2)\n    DISPLAY T\n")),
        ("one occurrence two levels down, four levels below it".into(), nest(6, 2), format!("{fill}    INITIALIZE B(1 2)\n    DISPLAY T\n")),
        ("a leaf of a table five levels deep".into(), nest(5, 2), format!("{fill}    INITIALIZE E(1 2 1 2 1)\n    DISPLAY T\n")),
        ("REPLACING NUMERIC DATA, five levels".into(), nest(5, 2), format!("{fill}    INITIALIZE T REPLACING NUMERIC DATA BY 5\n    DISPLAY T\n")),
        (
            "REPLACING each category, six levels with a number and a character".into(),
            mixed,
            "    MOVE ALL \"7\" TO T\n    INITIALIZE T REPLACING NUMERIC DATA BY 3 ALPHANUMERIC DATA BY \"z\"\n    DISPLAY \"[\" T \"]\"\n".into(),
        ),
    ]
}

fn browser(src: &str, multi: bool) -> Vec<String> {
    let o = common::run_wasm(src, &Options { multi_memory: multi, ..Options::default() });
    assert!(o.trapped.is_none() && o.errors.is_empty(), "browser multi={multi}: trapped {:?} errors {:?}\n{src}", o.trapped, o.errors);
    common::wasm_lines(&o)
}

fn desktop(src: &str) -> Vec<String> {
    let c = compile_program(&common::parse(src), &Options { native: true, ..Options::default() }).unwrap_or_else(|e| panic!("desktop: {e}\n{src}"));
    let o = run_native(&c.wasm, NativeInput::default()).expect("the host runs the module");
    assert!(o.trapped.is_none() && o.errors.is_empty(), "desktop: trapped {:?} errors {:?}\n{src}", o.trapped, o.errors);
    common::wasm_lines(&o)
}

#[test]
fn initialize_below_more_than_four_levels_of_occurs_runs_as_the_interpreter_runs_it() {
    let cases = cases();
    let (mut t_interp, mut t_browser, mut t_desktop) = (0.0, 0.0, 0.0);
    let mut passed = 0;
    println!("== initialize_deep_tables: INITIALIZE below 4 to 7 levels of OCCURS, interpreter = browser module = desktop module ==");
    for (what, ws, body) in &cases {
        let src = program(ws, body);
        let t = Instant::now();
        let i = common::run_interpreter(&src);
        t_interp += t.elapsed().as_secs_f64() * 1000.0;
        assert!(!i.is_empty(), "the interpreter showed nothing: {what}");
        let t = Instant::now();
        let (b1, b2) = (browser(&src, true), browser(&src, false));
        t_browser += t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let d = desktop(&src);
        t_desktop += t.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(b1, i, "browser module (multi-memory): {what}\n{src}");
        assert_eq!(b2, i, "browser module (single memory): {what}\n{src}");
        assert_eq!(d, i, "desktop module: {what}\n{src}");
        let shown = i.join(" / ");
        let shown: String = if shown.len() > 70 { format!("{}…", shown.chars().take(70).collect::<String>()) } else { shown };
        println!("  pass {what}: {shown}");
        passed += 1;
    }
    println!("timings: interpreter {t_interp:.1} ms; browser module (2 layouts) {t_browser:.1} ms; desktop module {t_desktop:.1} ms");
    println!("result: {passed} of {} cases identical on the three engines", cases.len());
    assert_eq!(passed, cases.len());
}

/// What the generator will not unroll: a table so wide at its outer levels that resetting it one
/// occurrence at a time would write thousands of entries. The refusal says so, and names the line.
#[test]
fn an_outer_table_too_wide_to_reset_one_by_one_is_refused_with_its_size() {
    let src = program("01 T.\n 05 A OCCURS 5000.\n  10 B OCCURS 2.\n   15 C OCCURS 2.\n    20 D OCCURS 2.\n     25 E OCCURS 2 PIC 9.\n", "    INITIALIZE T\n");
    let t = Instant::now();
    let browser = compile_program(&common::parse(&src), &Options::default()).err().map(|e| e.message).unwrap_or_default();
    let desktop = compile_program(&common::parse(&src), &Options { native: true, ..Options::default() }).err().map(|e| e.message).unwrap_or_default();
    println!("== initialize_deep_tables: the limit ==");
    println!("browser: {browser}");
    println!("desktop: {desktop}");
    println!("compiled twice in {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    assert!(browser.contains("5000 occurrences") && browser.contains("4096"), "{browser}");
    assert_eq!(browser, desktop);
    println!("result: 2 of 2 checks (both modules refuse alike, with the count and the limit)");
}
