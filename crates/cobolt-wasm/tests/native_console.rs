// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T12 (R3, R10, R11 console part; ABI A27, A29): with
//! `Options::native` a desktop program's `ACCEPT` from the console, the
//! environment and the command line compiles to the desktop runner's `prn`
//! imports, and the manifest lists exactly the capabilities the program needs.
//! Each program runs on the test host's `prn` imports and gives what the
//! interpreter gives; without `native` the generator answers as it did before.

mod common;

use cobolt_wasm::host::{run_native, run_program, NativeInput};
use cobolt_wasm::{compile_program, Compiled, Options};
use std::time::Instant;

fn native(multi: bool) -> Options {
    Options { native: true, multi_memory: multi, ..Options::default() }
}

/// The capabilities and the `prn` imports the module's embedded manifest and import section name.
fn needs(c: &Compiled) -> (Vec<String>, Vec<String>) {
    let scan = cobolt_wasm_abi::scan::scan_module(&c.wasm).expect("the module scans");
    let manifest = scan.embedded_manifest().expect("the module carries its manifest");
    manifest.validate().expect("the manifest validates");
    let prn: Vec<String> =
        scan.imports.iter().filter(|i| i.module == cobolt_wasm_abi::codes::NATIVE_IMPORT_MODULE).map(|i| i.name.clone()).collect();
    (manifest.capabilities, prn)
}

/// What the interpreter DISPLAYs with these command-line arguments.
fn interpreter(src: &str, args: &[&str]) -> Vec<String> {
    let prog = common::parse(src);
    let (_events, event_rx) = std::sync::mpsc::channel();
    let (state_tx, _state_rx) = std::sync::mpsc::channel();
    let (display_tx, display_rx) = std::sync::mpsc::channel();
    let mut interp = cobolt_runtime::interpreter::Interpreter::new_with_channels(prog, event_rx, state_tx, display_tx);
    interp.set_program_args(args.iter().map(|a| a.to_string()).collect());
    let _ = interp.run();
    display_rx.try_iter().collect()
}

fn program(id: &str, storage: &str, procedure: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. {id}.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{storage}PROCEDURE DIVISION.\n{procedure}    STOP RUN.\n"
    )
}

const CONSOLE_WS: &str = "01 WS-NAME PIC X(12).\n01 WS-AGE PIC 9(3).\n01 WS-LONG PIC X(4).\n01 WS-EMPTY PIC X(3) VALUE \"ABC\".\n\
                          01 WS-REST PIC X(5) VALUE \"KEEP\".\n01 WS-NUM PIC 9(3) VALUE 7.\n";
const CONSOLE_PD: &str = "    ACCEPT WS-NAME\n    DISPLAY \"[\" WS-NAME \"]\"\n    ACCEPT WS-AGE\n    DISPLAY \"[\" WS-AGE \"]\"\n\
                          \x20   ACCEPT WS-LONG\n    DISPLAY \"[\" WS-LONG \"]\"\n    ACCEPT WS-EMPTY\n    DISPLAY \"[\" WS-EMPTY \"]\"\n\
                          \x20   ACCEPT WS-REST\n    DISPLAY \"[\" WS-REST \"]\"\n    ACCEPT WS-NUM\n    DISPLAY \"[\" WS-NUM \"]\"\n";
/// The console's input, and what `rcrun run` (the interpreter) printed for `CONSOLE_PD` with these
/// lines on its standard input (recorded 2026-11-15): the line end is removed, a long line is cut
/// to the receiver, an empty line is spaces, and at the end of the input an alphanumeric receiver
/// gets spaces while a numeric one keeps its value.
const CONSOLE_IN: &[&str] = &["Ada Lovelace\r\n", "36\n", "longer than four\n", "\n"];
const CONSOLE_WANT: &[&str] = &["[Ada Lovelace]", "[036]", "[long]", "[   ]", "[     ]", "[007]"];

const ENV_WS: &str = "01 WS-HOME PIC X(20).\n01 WS-LANG PIC X(10).\n01 WS-NONE PIC X(6) VALUE \"KEEP\".\n01 WS-NUM PIC 9(5).\n\
                      01 WS-SHORT PIC X(4).\n01 WS-NAME PIC X(30) VALUE \"PRC_T12_LANG\".\n";
const ENV_PD: &str = "    ACCEPT WS-HOME FROM ENVIRONMENT \"PRC_T12_HOME\"\n    DISPLAY \"[\" WS-HOME \"]\"\n\
                      \x20   DISPLAY WS-NAME UPON ENVIRONMENT-NAME\n    ACCEPT WS-LANG FROM ENVIRONMENT-VALUE\n    DISPLAY \"[\" WS-LANG \"]\"\n\
                      \x20   ACCEPT WS-NONE FROM ENVIRONMENT \"PRC_T12_NOT_SET\"\n    DISPLAY \"[\" WS-NONE \"]\"\n\
                      \x20   ACCEPT WS-NUM FROM ENVIRONMENT \"PRC_T12_NUM\"\n    DISPLAY \"[\" WS-NUM \"]\"\n\
                      \x20   ACCEPT WS-SHORT FROM ENVIRONMENT \"PRC_T12_HOME\"\n    DISPLAY \"[\" WS-SHORT \"]\"\n\
                      \x20   DISPLAY \"   PRC_T12_NUM  \" UPON ENVIRONMENT-NAME\n    ACCEPT WS-LANG FROM ENVIRONMENT-VALUE\n    DISPLAY \"[\" WS-LANG \"]\"\n";
const ENV_VARS: &[(&str, &str)] = &[("PRC_T12_HOME", "/home/ada"), ("PRC_T12_LANG", "pt_BR.UTF-8"), ("PRC_T12_NUM", "123")];

const ARGS_WS: &str = "01 WS-COUNT PIC 99.\n01 WS-LINE PIC X(40).\n01 WS-ARG PIC X(10).\n01 WS-N PIC 9 VALUE 3.\n";
const ARGS_PD: &str = "    ACCEPT WS-COUNT FROM ARGUMENT-NUMBER\n    DISPLAY \"[\" WS-COUNT \"]\"\n\
                       \x20   ACCEPT WS-LINE FROM COMMAND-LINE\n    DISPLAY \"[\" WS-LINE \"]\"\n\
                       \x20   ACCEPT WS-ARG FROM ARGUMENT-VALUE\n    DISPLAY \"[\" WS-ARG \"]\"\n\
                       \x20   DISPLAY WS-N UPON ARGUMENT-NUMBER\n    ACCEPT WS-ARG FROM ARGUMENT-VALUE\n    DISPLAY \"[\" WS-ARG \"]\"\n\
                       \x20   DISPLAY 9 UPON ARGUMENT-NUMBER\n    ACCEPT WS-ARG FROM ARGUMENT-VALUE\n    DISPLAY \"[\" WS-ARG \"]\"\n\
                       \x20   DISPLAY 0 UPON ARGUMENT-NUMBER\n    ACCEPT WS-ARG FROM ARGUMENT-VALUE\n    DISPLAY \"[\" WS-ARG \"]\"\n";
const ARGS: &[&str] = &["alpha", "beta two", "gamma"];

const SCREEN_WS: &str = "01 WS-KEY PIC XX VALUE \"ZZ\".\n01 WS-CRT PIC X(4) VALUE \"ZZZZ\".\n01 WS-KEYN PIC 99 VALUE 55.\n";
const SCREEN_PD: &str = "    ACCEPT WS-KEY FROM ESCAPE KEY\n    ACCEPT WS-CRT FROM CRT STATUS\n    ACCEPT WS-KEYN FROM ESCAPE KEY\n\
                         \x20   DISPLAY \"[\" WS-KEY \"][\" WS-CRT \"][\" WS-KEYN \"]\"\n";

/// An INDEXED file held in the module's memory (WASM-010 Q8), written out of order and read back.
const INDEXED: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T12IDX.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
    \x20   SELECT CUST ASSIGN TO \"data.idx\"\n        ORGANIZATION IS INDEXED\n        ACCESS MODE IS DYNAMIC\n        RECORD KEY IS C-ID\n\
    \x20       STORAGE IS MEMORY\n        FILE STATUS IS FS.\nDATA DIVISION.\nFILE SECTION.\nFD CUST.\n01 C-REC.\n    05 C-ID PIC 9(5).\n    05 C-NAME PIC X(5).\n\
    WORKING-STORAGE SECTION.\n01 FS PIC XX.\n01 EOF PIC 9 VALUE 0.\nPROCEDURE DIVISION.\n    OPEN OUTPUT CUST\n\
    \x20   MOVE 30 TO C-ID MOVE \"ANA\" TO C-NAME WRITE C-REC\n    MOVE 10 TO C-ID MOVE \"BRUNO\" TO C-NAME WRITE C-REC\n\
    \x20   MOVE 20 TO C-ID MOVE \"CARLA\" TO C-NAME WRITE C-REC\n    CLOSE CUST\n    OPEN INPUT CUST\n\
    \x20   PERFORM UNTIL EOF = 1\n        READ CUST NEXT RECORD AT END MOVE 1 TO EOF NOT AT END DISPLAY C-ID \" \" C-NAME END-READ\n    END-PERFORM\n\
    \x20   CLOSE CUST\n    STOP RUN.\n";
const INDEXED_WANT: &[&str] = &["00010 BRUNO", "00020 CARLA", "00030 ANA  "];

/// SORT with an INPUT and an OUTPUT PROCEDURE: the work file is the module's.
const SORTED: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T12SRT.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
    \x20   SELECT WORK-FILE ASSIGN TO \"work.tmp\".\nDATA DIVISION.\nFILE SECTION.\nSD WORK-FILE.\n01 W-REC.\n    05 W-KEY PIC 99.\n\
    WORKING-STORAGE SECTION.\n01 K PIC 99.\n01 EOF PIC 9.\nPROCEDURE DIVISION.\nMAIN-PARA.\n\
    \x20   SORT WORK-FILE ON DESCENDING KEY W-KEY INPUT PROCEDURE IS GIVE-RECORDS OUTPUT PROCEDURE IS TAKE-RECORDS\n    STOP RUN.\n\
    GIVE-RECORDS.\n    PERFORM VARYING K FROM 1 BY 1 UNTIL K > 4\n        MOVE K TO W-KEY\n        RELEASE W-REC\n    END-PERFORM.\n\
    TAKE-RECORDS.\n    MOVE 0 TO EOF\n    PERFORM UNTIL EOF = 1\n        RETURN WORK-FILE AT END MOVE 1 TO EOF NOT AT END DISPLAY W-KEY END-RETURN\n    END-PERFORM.\n";

/// What the program is compared with.
enum Want {
    /// The interpreter, run in this process with these arguments.
    Interpreter(&'static [&'static str]),
    /// Recorded lines (the interpreter's console reads this process's standard input).
    Lines(&'static [&'static str]),
}

struct Case {
    name: &'static str,
    src: String,
    input: NativeInput,
    caps: &'static [&'static str],
    want: Want,
}

fn env_input() -> Vec<(String, String)> {
    ENV_VARS.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn cases() -> Vec<Case> {
    let console = |lines: &[&str]| lines.iter().map(|l| l.to_string()).collect::<Vec<_>>();
    vec![
        Case {
            name: "console",
            src: program("T12CON", CONSOLE_WS, CONSOLE_PD),
            input: NativeInput { console: console(CONSOLE_IN), ..NativeInput::default() },
            caps: &["console"],
            want: Want::Lines(CONSOLE_WANT),
        },
        Case {
            name: "environment",
            src: program("T12ENV", ENV_WS, ENV_PD),
            input: NativeInput { env: env_input(), ..NativeInput::default() },
            caps: &["env"],
            want: Want::Interpreter(&[]),
        },
        Case {
            name: "arguments",
            src: program("T12ARG", ARGS_WS, ARGS_PD),
            input: NativeInput { args: ARGS.iter().map(|a| a.to_string()).collect(), ..NativeInput::default() },
            caps: &["env"],
            want: Want::Interpreter(ARGS),
        },
        Case {
            name: "no-arguments",
            src: program("T12NOA", ARGS_WS, ARGS_PD),
            input: NativeInput::default(),
            caps: &["env"],
            want: Want::Interpreter(&[]),
        },
        Case {
            name: "escape-key-crt-status",
            src: program("T12SCR", SCREEN_WS, SCREEN_PD),
            input: NativeInput::default(),
            caps: &[],
            want: Want::Interpreter(&[]),
        },
        Case {
            name: "console-and-environment",
            src: program(
                "T12BTH",
                "01 WS-A PIC X(5).\n01 WS-B PIC X(9).\n",
                "    ACCEPT WS-B FROM ENVIRONMENT \"PRC_T12_HOME\"\n    ACCEPT WS-A\n    DISPLAY \"[\" WS-A \"][\" WS-B \"]\"\n",
            ),
            input: NativeInput { console: console(&["hello"]), env: env_input(), ..NativeInput::default() },
            // In the order of A27's list, whatever the order of the statements.
            caps: &["console", "env"],
            want: Want::Lines(&["[hello][/home/ada]"]),
        },
        Case { name: "in-memory-indexed", src: INDEXED.to_string(), input: NativeInput::default(), caps: &[], want: Want::Lines(INDEXED_WANT) },
        Case { name: "sort-procedures", src: SORTED.to_string(), input: NativeInput::default(), caps: &[], want: Want::Interpreter(&[]) },
    ]
}

#[test]
fn the_console_and_the_environment_reach_the_runner_and_the_manifest_names_them() {
    // The interpreter reads this process's environment; the module reads only the host's list.
    for (k, v) in ENV_VARS {
        std::env::set_var(k, v);
    }
    std::env::remove_var("PRC_T12_NOT_SET");

    let all = cases();
    let mut checks = 0usize;
    let mut passed = 0usize;
    let mut lines = 0usize;
    let mut rows: Vec<String> = Vec::new();
    let (mut compile_ms, mut run_ms) = (0f64, 0f64);
    let mut check = |ok: bool, what: String, failures: &mut Vec<String>| {
        checks += 1;
        if ok {
            passed += 1;
        } else {
            failures.push(what);
        }
    };
    let mut failures: Vec<String> = Vec::new();

    for case in &all {
        let want: Vec<String> = match &case.want {
            Want::Interpreter(args) => interpreter(&case.src, args),
            Want::Lines(l) => l.iter().map(|s| s.to_string()).collect(),
        };
        let mut case_compile = 0f64;
        let mut case_run = 0f64;
        for multi in [true, false] {
            let prog = common::parse(&case.src);
            let t = Instant::now();
            let c = compile_program(&prog, &native(multi)).unwrap_or_else(|e| panic!("{}: compile failed: {e}", case.name));
            case_compile += t.elapsed().as_secs_f64() * 1000.0;
            let (caps, prn) = needs(&c);
            check(caps == case.caps, format!("{} multi={multi}: capabilities {caps:?}, want {:?}", case.name, case.caps), &mut failures);
            // Each capability's imports are what the module imports from `prn`, and nothing else
            // (`exit` only in a program that names RETURN-CODE, which none of these does: T15).
            let want_prn: Vec<String> = cobolt_wasm_abi::codes::NATIVE_IMPORTS
                .iter()
                .filter(|(cap, _)| case.caps.contains(cap))
                .flat_map(|(_, names)| names.iter().filter(|n| **n != "exit").map(|n| n.to_string()))
                .collect();
            check(prn == want_prn, format!("{} multi={multi}: prn imports {prn:?}, want {want_prn:?}", case.name), &mut failures);
            // A module with no capability is the module the browser path makes, byte for byte
            // (ESCAPE KEY and CRT STATUS are the desktop's alone: the browser build refuses them,
            // as the refusals below check).
            if case.caps.is_empty() {
                if let Ok(web) = compile_program(&prog, &Options { multi_memory: multi, ..Options::default() }) {
                    check(web.wasm == c.wasm, format!("{} multi={multi}: the module differs from the browser build", case.name), &mut failures);
                }
            } else {
                let problems = cobolt_wasm_abi::scan::bundle_problems(&c.wasm, !multi);
                let named = case.caps.iter().all(|cap| {
                    let import = cobolt_wasm_abi::codes::NATIVE_IMPORTS.iter().find(|(c, _)| c == cap).unwrap().1[0];
                    problems.iter().any(|p| p.contains(&format!("prn.{import}")))
                });
                check(named, format!("{} multi={multi}: the bundler does not name the runner's imports: {problems:?}", case.name), &mut failures);
            }
            let t = Instant::now();
            let o = run_native(&c.wasm, case.input.clone()).unwrap_or_else(|e| panic!("{}: host failed: {e}", case.name));
            case_run += t.elapsed().as_secs_f64() * 1000.0;
            check(o.trapped.is_none() && o.errors.is_empty(), format!("{} multi={multi}: trapped {:?} errors {:?}", case.name, o.trapped, o.errors), &mut failures);
            let got = common::wasm_lines(&o);
            check(got == want, format!("{} multi={multi}: module {got:?}, want {want:?}", case.name), &mut failures);
            lines += want.len();
        }
        compile_ms += case_compile;
        run_ms += case_run;
        rows.push(format!(
            "  {:<24} caps {:<16} compile {:>6.1} ms  run {:>6.1} ms  {} lines: {}",
            case.name,
            format!("{:?}", case.caps),
            case_compile,
            case_run,
            want.len(),
            want.join(" | ")
        ));
    }

    // Without `native`, the browser build's answers, as before T12.
    let refusals: [(&str, String, &str); 5] = [
        ("environment", program("T12ENV", ENV_WS, ENV_PD), "ACCEPT FROM Environment"),
        ("arguments", program("T12ARG", ARGS_WS, ARGS_PD), "ACCEPT FROM ArgumentNumber"),
        ("environment-name", program("T12DEN", "", "    DISPLAY \"HOME\" UPON ENVIRONMENT-NAME\n"), "DISPLAY UPON ENVIRONMENT-NAME"),
        ("argument-number", program("T12DAN", "", "    DISPLAY 2 UPON ARGUMENT-NUMBER\n"), "DISPLAY UPON ARGUMENT-NUMBER"),
        ("escape-key-crt-status", program("T12SCR", SCREEN_WS, SCREEN_PD), "ACCEPT FROM EscapeKey"),
    ];
    for (name, src, says) in &refusals {
        let prog = common::parse(src);
        let refused = compile_program(&prog, &Options::default()).err().map(|e| e.to_string()).unwrap_or_default();
        check(
            refused.contains(says) && refused.contains("is not supported in a WebAssembly module"),
            format!("without native, {name}: {refused:?} does not say {says:?}"),
            &mut failures,
        );
    }
    // `RETURN-CODE` compiles in a desktop module only (T15, `tests/native_return_code.rs`); the
    // browser build refuses it, as before.
    let rc = common::parse(&program("T12RC", "", "    MOVE 4 TO RETURN-CODE\n"));
    let rc_refused = compile_program(&rc, &Options::default()).err().map(|e| e.to_string()).unwrap_or_default();
    check(rc_refused.contains("RETURN-CODE"), format!("without native, RETURN-CODE is refused: {rc_refused:?}"), &mut failures);
    let prog = common::parse(&program("T12CON", CONSOLE_WS, CONSOLE_PD));
    let ignored = compile_program(&prog, &Options::default()).expect("a browser module ignores ACCEPT");
    let kinds: Vec<_> = ignored.warnings.iter().map(|w| w.kind).collect();
    check(
        kinds.len() == 6 && kinds.iter().all(|k| *k == cobolt_wasm::WarningKind::AcceptIgnored) && needs(&ignored).0.is_empty(),
        format!("without native, the console's ACCEPT is ignored with a warning: {kinds:?}"),
        &mut failures,
    );
    let o = run_program(&ignored.wasm).expect("the browser module runs");
    check(
        common::wasm_lines(&o) == ["[            ]", "[000]", "[    ]", "[ABC]", "[KEEP ]", "[007]"],
        format!("without native, the receivers keep their values: {:?}", common::wasm_lines(&o)),
        &mut failures,
    );

    println!("── the_console_and_the_environment_reach_the_runner_and_the_manifest_names_them (DESKTOP-WASM-001 T12) ──");
    for r in &rows {
        println!("{r}");
    }
    println!(
        "native builds: {} programs × 2 memory variants; {lines} lines compared (interpreter or its recorded console run); \
         compile {compile_ms:.1} ms, run {run_ms:.1} ms ({:.0} runs/s)",
        all.len(),
        (all.len() * 2) as f64 / (run_ms / 1000.0).max(1e-9)
    );
    println!("without native: {} refusals, the console ignored with 6 warnings, as before", refusals.len());
    println!("without native: a program naming RETURN-CODE is refused: {rc_refused}");
    println!("checks: {passed} of {checks} passed");
    for f in &failures {
        println!("  FAIL {f}");
    }
    assert!(failures.is_empty(), "{} of {checks} checks failed:\n{}", failures.len(), failures.join("\n"));
}
