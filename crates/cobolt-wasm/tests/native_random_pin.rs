// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T15 (R28 runner half): `FUNCTION RANDOM` keeps its generator inside the
//! module, so a desktop program that uses it reads `PRC_TEST_SEED` first, through the runner's
//! `env_get`, and starts as the interpreter starts under the same pin. Each value of the variable
//! (none, valid, padded, signed, too large, not a number) gives the interpreter's numbers; a
//! program without RANDOM, a browser module and a form's module are unchanged.
//!
//! The interpreter's pin holds for the whole process (`test_pins::set_for_test`), so this file is
//! one test.

mod common;

use cobolt_wasm::host::{run_native, NativeInput};
use cobolt_wasm::Options;
use std::time::Instant;

const RANDOMS: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T15RND.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
    01 WS-R PIC 9(9).\n01 WS-F PIC 9V9(8).\n01 WS-I PIC 9.\nPROCEDURE DIVISION.\n\
    \x20   PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 4\n        COMPUTE WS-R = FUNCTION RANDOM * 1000000000\n\
    \x20       MOVE FUNCTION RANDOM TO WS-F\n        DISPLAY WS-I \" \" WS-R \" \" WS-F\n    END-PERFORM\n\
    \x20   COMPUTE WS-R = FUNCTION RANDOM(5) * 1000000000\n    DISPLAY \"reseeded \" WS-R\n    STOP RUN.\n";

const PLAIN: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T15PLN.\nPROCEDURE DIVISION.\n    DISPLAY \"no random\"\n    STOP RUN.\n";

/// The values of `PRC_TEST_SEED` tried, and the seed the interpreter's `trim().parse::<u64>()` reads.
const SEEDS: &[Option<&str>] =
    &[None, Some("0"), Some("7"), Some("20261114"), Some("18446744073709551615"), Some("  42\t"), Some("+42"), Some("abc"), Some("-1"), Some("18446744073709551616"), Some("")];

fn interpreter(src: &str) -> Vec<String> {
    let prog = common::parse(src);
    let (_events, event_rx) = std::sync::mpsc::channel();
    let (state_tx, _state_rx) = std::sync::mpsc::channel();
    let (display_tx, display_rx) = std::sync::mpsc::channel();
    let mut interp = cobolt_runtime::interpreter::Interpreter::new_with_channels(prog, event_rx, state_tx, display_tx);
    let _ = interp.run();
    display_rx.try_iter().collect()
}

#[test]
fn prc_test_seed_pins_random_in_a_desktop_module_as_in_the_interpreter() {
    let t = Instant::now();
    let native = common::compile(RANDOMS, &Options { native: true, ..Options::default() });
    let browser = common::compile(RANDOMS, &Options::default());
    let compile_ms = t.elapsed().as_secs_f64() * 1000.0;
    let mut checks = 0;

    // Only the desktop module reads the variable; it needs `env` for it.
    let imports = |w: &[u8]| -> Vec<String> {
        cobolt_wasm_abi::scan::scan_module(w).unwrap().imports.iter().map(|i| format!("{}.{}", i.module, i.name)).collect()
    };
    let caps = |w: &[u8]| cobolt_wasm_abi::scan::scan_module(w).unwrap().embedded_manifest().unwrap().capabilities;
    assert!(imports(&native.wasm).contains(&"prn.env_get".to_string()), "{:?}", imports(&native.wasm));
    assert_eq!(caps(&native.wasm), vec!["env".to_string()]);
    assert!(!imports(&browser.wasm).iter().any(|i| i.starts_with("prn.")), "a browser module imports nothing of the runner's");
    let plain_native = common::compile(PLAIN, &Options { native: true, ..Options::default() });
    let plain_browser = common::compile(PLAIN, &Options::default());
    assert_eq!(plain_native.wasm, plain_browser.wasm, "a desktop program without RANDOM is the browser module, byte for byte");
    checks += 4;

    let mut rows = Vec::new();
    let mut lines = 0;
    let t = Instant::now();
    for &value in SEEDS {
        let seed = value.and_then(|v| v.trim().parse::<u64>().ok());
        cobolt_runtime::test_pins::set_for_test(None, seed);
        let want = interpreter(RANDOMS);
        let env = value.map(|v| vec![("PRC_TEST_SEED".to_string(), v.to_string())]).unwrap_or_default();
        let out = run_native(&native.wasm, NativeInput { env, ..NativeInput::default() }).expect("the module runs");
        assert!(out.trapped.is_none(), "{value:?}: {:?}", out.trapped);
        let got: Vec<String> = out.output.lines().map(str::to_owned).collect();
        assert_eq!(got, want, "PRC_TEST_SEED={value:?} (the interpreter reads {seed:?})");
        assert_eq!(want.len(), 5);
        checks += 3;
        lines += got.len();
        rows.push(format!("{:<24} → {:<22} first {}", format!("{value:?}"), format!("{seed:?}"), want[0]));
    }
    cobolt_runtime::test_pins::clear_for_test();
    let run_ms = t.elapsed().as_secs_f64() * 1000.0;

    // The pin moves the numbers: the unpinned run and seed 7 differ, "+42" and "  42\t" agree.
    let firsts: Vec<&str> = rows.iter().map(|r| r.rsplit("first ").next().unwrap()).collect();
    assert_ne!(firsts[0], firsts[2]);
    assert_eq!(firsts[5], firsts[6]);
    assert_eq!(firsts[0], firsts[7], "a value that is not a number pins nothing");
    checks += 3;

    println!("── prc_test_seed_pins_random_in_a_desktop_module_as_in_the_interpreter ──");
    println!("  program: RANDOM ×8 (COMPUTE × 10^9 and MOVE to 9V9(8)), then RANDOM(5); {} bytes desktop, {} browser", native.wasm.len(), browser.wasm.len());
    for r in &rows {
        println!("  PRC_TEST_SEED {r}");
    }
    println!("  {} values of PRC_TEST_SEED, {lines} lines equal to the interpreter's; compiled in {compile_ms:.1} ms, ran in {run_ms:.1} ms", SEEDS.len());
    println!("  checks: {checks} of {checks} passed");
}
