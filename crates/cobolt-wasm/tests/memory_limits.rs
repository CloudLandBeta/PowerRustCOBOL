// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Fixed memory has a ceiling (WASM-017 R7, AC5): memory 0 never grows, so
//! data that cannot fit a 32-bit WebAssembly memory is a compile error that
//! names the form, the item and the arithmetic — never an invalid module. Data
//! that cannot be sized at all names its item too, and no module this file
//! compiles contains `memory.grow`.

mod common;

use cobolt_wasm::module::{compile_project, FormSpec, MAX_DATA_PAGES};
use cobolt_wasm::{compile_program, Compiled, GenError, Options};
use cobolt_wasm_abi::scan::scan_module;

/// A one-form program whose WORKING-STORAGE is `data`.
fn program(data: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. BIGFORM.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{data}\
         PROCEDURE DIVISION.\n    DISPLAY \"OK\"\n    STOP RUN.\nEND PROGRAM BIGFORM.\n"
    )
}

/// `data` as the one form of a project, with `instances` instances.
fn project(data: &str, instances: u32, multi: bool) -> Result<Compiled, GenError> {
    let prog = common::parse(&program(data));
    let spec = FormSpec {
        name: "BIGFORM".into(),
        main: false,
        instances,
        program: &prog,
        controls: vec![],
        bindings: vec![],
        layers: vec![],
        lines: vec![],
    };
    compile_project(
        std::slice::from_ref(&spec),
        &Options {
            multi_memory: multi,
            ..Options::default()
        },
    )
}

fn opts(multi: bool) -> Options {
    Options {
        multi_memory: multi,
        ..Options::default()
    }
}

/// Every module compiled here: none has `memory.grow`.
fn no_grow(c: &Compiled, scanned: &mut (u32, u32)) {
    let s = scan_module(&c.wasm).expect("the module scans");
    scanned.0 += 1;
    if s.has_memory_grow {
        scanned.1 += 1;
    }
    assert!(!s.has_memory_grow, "generated code contains no memory.grow");
}

/// 1 100 000 000 bytes: one instance fits a 4 GiB memory, four do not.
const BIG_TABLE: &str = "01 WS-BIG.\n   05 WS-ROW OCCURS 1100 TIMES PIC X(1000000).\n";

#[test]
fn memory_ceiling_names_the_form_the_item_and_the_arithmetic_017() {
    assert_eq!(
        MAX_DATA_PAGES, 65_536,
        "no product cap below the 32-bit limit (Q-17b open)"
    );
    let mut scanned = (0u32, 0u32);
    let mut refused = 0;
    for multi in [true, false] {
        let e = project(BIG_TABLE, 4, multi)
            .err()
            .unwrap_or_else(|| panic!("multi={multi}: four instances refused"));
        let text = e.to_string();
        for want in [
            "BIGFORM",
            "WS-BIG",
            "× 4 instances",
            "WS-BIG (1100000000 bytes)",
            "4294967296",
            "lower Instances",
        ] {
            assert!(text.contains(want), "multi={multi}: {want:?} in {text}");
        }
        assert_eq!(e.form, Some(0), "the refusal points at the form");
        refused += 1;
        println!("multi={multi}, Instances 4: {text}");
    }
    // The same table with one instance compiles, with the data memory sized
    // to fit it and no more.
    let one = project(BIG_TABLE, 1, true).unwrap_or_else(|e| panic!("one instance fits: {e}"));
    assert!(
        one.sizes.data_pages > 1_100_000_000 / 65_536 && one.sizes.data_pages <= MAX_DATA_PAGES,
        "{}",
        one.sizes.data_pages
    );
    no_grow(&one, &mut scanned);
    println!("Instances 1: accepted, {} data pages", one.sizes.data_pages);
    drop(one);

    // An item larger than a WebAssembly memory is refused by name in layout.
    let huge = "01 WS-HUGE.\n   05 WS-CELL OCCURS 5000 TIMES PIC X(1000000).\n";
    for multi in [true, false] {
        let e = compile_program(&common::parse(&program(huge)), &opts(multi))
            .err()
            .expect("refused");
        assert!(
            e.to_string().contains("WS-CELL") && e.to_string().contains("4 GiB"),
            "{e}"
        );
        refused += 1;
        println!("multi={multi}: {e}");
    }

    // A small program in both variants: fixed memory, no growth.
    for multi in [true, false] {
        let c = compile_program(
            &common::parse(&program("01 WS-A PIC X(10) VALUE \"A\".\n")),
            &opts(multi),
        )
        .unwrap();
        no_grow(&c, &mut scanned);
    }
    println!("memory ceiling: {refused} refusals naming form/item; {} modules scanned, {} with memory.grow", scanned.0, scanned.1);
}

#[test]
fn data_that_cannot_be_sized_names_its_item_017() {
    let mut named = 0;
    for (data, item, what) in [
        ("01 WS-NOPIC.\n", "WS-NOPIC", "needs a PICTURE"),
        ("01 WS-WIDE PIC 9(39).\n", "WS-WIDE", "more than 38 digits"),
    ] {
        for multi in [true, false] {
            let e = compile_program(&common::parse(&program(data)), &opts(multi))
                .err()
                .unwrap_or_else(|| panic!("{data} refused"));
            let text = e.to_string();
            assert!(text.contains(item) && text.contains(what), "{text}");
            named += 1;
            println!("multi={multi}: {text}");
        }
    }
    println!("unsized data: {named} refusals, each naming its item");
}
