// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T50a3a (R11, M6): the console, the environment and the arguments reach national
//! (`PIC N`) and UTF-8 (`PIC U`) items, and a number meets national text in `MOVE`, in a
//! comparison, in a class test and in a condition-name, in a packaged program as under the
//! interpreter. Each program below runs twice, packaged into a copy of the built `prc-runner`
//! and under `rcrun run`, each in a folder of its own, with the same standard input, arguments
//! and variables; the two runs must print the same lines and end with the same exit code.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use cobolt_program_package::Kind;

/// `ACCEPT` into national and UTF-8 items: a line of the console, a variable, the arguments.
const CONSOLE: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T50A3C.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N      PIC N(10).
01 WS-U      PIC U(10).
01 WS-J      PIC N(8) JUSTIFIED RIGHT.
01 WS-X      PIC X(8).
01 WS-HOME   PIC N(12).
01 WS-NONE   PIC N(6) VALUE N\"keep\".
01 WS-COUNT  PIC 99.
01 WS-LINE   PIC N(24).
01 WS-ARG    PIC U(10).
PROCEDURE DIVISION.
MAIN-PARA.
    ACCEPT WS-N
    ACCEPT WS-U
    ACCEPT WS-J
    ACCEPT WS-X
    DISPLAY \"console [\" WS-N \"][\" WS-U \"][\" WS-J \"][\" WS-X \"]\"
    ACCEPT WS-HOME FROM ENVIRONMENT \"PRC_T50A3A_HOME\"
    DISPLAY \"home [\" WS-HOME \"]\"
    ACCEPT WS-NONE FROM ENVIRONMENT \"PRC_T50A3A_NOT_SET\"
    DISPLAY \"none [\" WS-NONE \"]\"
    ACCEPT WS-COUNT FROM ARGUMENT-NUMBER
    ACCEPT WS-LINE FROM COMMAND-LINE
    ACCEPT WS-ARG FROM ARGUMENT-VALUE
    DISPLAY \"args \" WS-COUNT \" [\" WS-LINE \"] first [\" WS-ARG \"]\"
    STOP RUN.
";

/// Numbers and national text: moves each way, comparisons, class tests, condition-names, a value
/// computed for several receivers.
const NUMBERS: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T50A3N.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-Q      PIC 9(4) VALUE 1234.
01 WS-S      PIC S9(3)V99 VALUE -12.5.
01 WS-N      PIC N(8).
01 WS-U      PIC U(8).
01 WS-J      PIC N(6) JUSTIFIED RIGHT.
01 WS-T      PIC N(5) OCCURS 3.
01 WS-R      PIC 9(4) VALUE 7.
01 WS-D      PIC 9(3)V9 VALUE 0.
01 WS-A      PIC N(4) VALUE N\"42\".
01 WS-B      PIC N(4) VALUE N\"0.5\".
01 WS-W      PIC N(4) VALUE N\"abc\".
01 WS-K      PIC N(2).
   88 K-OK   VALUE N\"ok\" N\"OK\".
   88 K-LOW  VALUE N\"a\" THRU N\"m\".
PROCEDURE DIVISION.
MAIN-PARA.
    MOVE WS-Q TO WS-N
    DISPLAY \"[\" WS-N \"]\"
    MOVE WS-S TO WS-U
    DISPLAY \"[\" WS-U \"]\"
    MOVE 7 TO WS-J
    DISPLAY \"[\" WS-J \"]\"
    MOVE 2.50 TO WS-J
    DISPLAY \"[\" WS-J \"]\"
    MOVE 0012 TO WS-T(2)
    DISPLAY \"[\" WS-T(1) \"][\" WS-T(2) \"]\"
    MOVE WS-A TO WS-R
    DISPLAY WS-R
    MOVE WS-B TO WS-D
    DISPLAY WS-D
    MOVE WS-W TO WS-R
    DISPLAY WS-R
    IF WS-A = 42 DISPLAY \"A=42\" ELSE DISPLAY \"A#42\" END-IF
    IF WS-A > WS-R DISPLAY \"A>R\" ELSE DISPLAY \"A<=R\" END-IF
    IF WS-W = ZERO DISPLAY \"W=0\" ELSE DISPLAY \"W#0\" END-IF
    IF WS-A IS NUMERIC DISPLAY \"A num\" ELSE DISPLAY \"A not num\" END-IF
    IF WS-W IS ALPHABETIC-LOWER DISPLAY \"W lower\" ELSE DISPLAY \"W not lower\" END-IF
    MOVE N\"ok\" TO WS-K
    IF K-OK DISPLAY \"K ok\" ELSE DISPLAY \"K not ok\" END-IF
    IF K-LOW DISPLAY \"K low\" ELSE DISPLAY \"K not low\" END-IF
    SET K-LOW TO TRUE
    DISPLAY \"[\" WS-K \"]\"
    STOP RUN.
";

/// (tag, program, the lines the run must print)
type Program = (&'static str, &'static str, &'static [&'static str]);

const STDIN: &str = "Zoë Åström\r\n日本語😀\nab\nxyz\n";
const ARGS: &[&str] = &["alpha", "çé beta"];
const VARS: &[(&str, &str)] = &[("PRC_T50A3A_HOME", "/home/açaí")];

const PROGRAMS: &[Program] = &[
    (
        "console",
        CONSOLE,
        &[
            "console [Zoë Åström][日本語😀      ][      ab][xyz     ]",
            "home [/home/açaí  ]",
            "none [      ]",
            "args 02 [alpha çé beta           ] first [alpha     ]",
        ],
    ),
    (
        "numbers",
        NUMBERS,
        &[
            "[1234    ]",
            "[01250   ]",
            "[7     ]",
            "[  2.50]",
            "[     ][0012 ]",
            "0042",
            "0005",
            "A=42",
            "A>R",
            "W#0",
            "A not num",
            "W lower",
            "K ok",
            "K not low",
            "[a ]",
        ],
    ),
];

/// A folder of its own under the system's temporary folder, removed at the end.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("prc-runner-t50a3a-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Scratch(p)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if std::env::var_os("PRC_RUNNER_KEEP").is_none() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn parse(src: &str) -> cobolt_ast::program::Program {
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<String> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
    assert!(errors.is_empty(), "the program does not parse: {errors:?}");
    parsed.program.expect("a program")
}

/// `rcrun`, built now from the current source (a stale binary would compare against old code).
fn rcrun() -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new(cargo).current_dir(&root).args(["build", "-q", "-p", "cobolt-cli", "--bin", "rcrun"]).status().expect("run cargo");
    assert!(status.success(), "cargo build -p cobolt-cli --bin rcrun failed");
    let exe = std::env::current_exe().unwrap();
    let dir = exe.parent().and_then(Path::parent).unwrap();
    dir.join(format!("rcrun{}", std::env::consts::EXE_SUFFIX))
}

fn run(mut cmd: Command, cache: &Path) -> (Output, f64) {
    cmd.env("PRC_RUNNER_CACHE", cache).env("LC_ALL", "en_US.UTF-8");
    for (k, v) in VARS {
        cmd.env(k, v);
    }
    cmd.env_remove("PRC_T50A3A_NOT_SET");
    cmd.args(ARGS);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let t = Instant::now();
    let mut child = cmd.spawn().expect("start the program");
    child.stdin.take().unwrap().write_all(STDIN.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    (out, t.elapsed().as_secs_f64() * 1000.0)
}

/// What a run printed.
fn lines_of(o: &Output) -> Vec<String> {
    String::from_utf8_lossy(&o.stdout).lines().map(str::to_owned).collect()
}

#[test]
fn the_console_and_numbers_over_national_and_utf8_data_match_the_interpreter() {
    let root = Scratch::new("numbers");
    let cache = root.0.join("cache");
    let t = Instant::now();
    let rcrun = rcrun();
    let rcrun_build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let runner = std::fs::read(env!("CARGO_BIN_EXE_prc-runner")).expect("the built prc-runner");

    let mut checks = 0;
    let mut rows = Vec::new();
    for (tag, src, expected) in PROGRAMS {
        let t = Instant::now();
        let opts = cobolt_wasm::Options { native: true, ..cobolt_wasm::Options::default() };
        let module = cobolt_wasm::compile_program(&parse(src), &opts).unwrap_or_else(|e| panic!("{tag}: the generator refused the program: {e}")).wasm;
        let compile_ms = t.elapsed().as_secs_f64() * 1000.0;

        let ours = root.0.join(format!("{tag}-runner"));
        let theirs = root.0.join(format!("{tag}-rcrun"));
        std::fs::create_dir_all(&ours).unwrap();
        std::fs::create_dir_all(&theirs).unwrap();
        let exe = ours.join(format!("prog{}", std::env::consts::EXE_SUFFIX));
        let payload = cobolt_program_package::encode(&[(Kind::Module, &module)]);
        let packaged = cobolt_program_package::embed(&runner, &payload, "prc-t50a3a-national").expect("the runner takes a payload");
        cobolt_program_package::write_executable(&exe, &packaged).expect("write the program");
        let source = theirs.join(format!("{tag}.cbl"));
        std::fs::write(&source, src).unwrap();

        let mut a = Command::new(&exe);
        a.current_dir(&ours);
        let (runner_out, runner_ms) = run(a, &cache);
        let mut b = Command::new(&rcrun);
        b.arg("run").arg(&source).current_dir(&theirs);
        let (rcrun_out, rcrun_ms) = run(b, &cache);

        let err = |o: &Output| String::from_utf8_lossy(&o.stderr).into_owned();
        assert!(runner_out.status.success(), "{tag}: the runner failed: {}", err(&runner_out));
        assert!(rcrun_out.status.success(), "{tag}: rcrun run failed: {}", err(&rcrun_out));
        let (ours_lines, theirs_lines) = (lines_of(&runner_out), lines_of(&rcrun_out));
        assert_eq!(ours_lines, theirs_lines, "{tag}: the runner prints what rcrun run prints\nrunner stderr: {}", err(&runner_out));
        assert_eq!(runner_out.status.code(), rcrun_out.status.code(), "{tag}: the same exit code");
        checks += 2;
        for e in *expected {
            assert!(ours_lines.iter().any(|l| l == e), "{tag}: the run prints `{e}`:\n{}", ours_lines.join("\n"));
            checks += 1;
        }
        rows.push(format!(
            "  {tag:<14}: {:>2} lines, exit {}; generate {compile_ms:.1} ms ({} bytes), runner {runner_ms:.1} ms, rcrun run {rcrun_ms:.1} ms",
            ours_lines.len(),
            runner_out.status.code().unwrap_or(-1),
            module.len()
        ));
    }

    println!("── the_console_and_numbers_over_national_and_utf8_data_match_the_interpreter (T50a3a) ──");
    println!("  console: ACCEPT of a line into N, U, JUSTIFIED RIGHT N and X items; ENVIRONMENT (set, not set); ARGUMENT-NUMBER, COMMAND-LINE, ARGUMENT-VALUE");
    println!("  numbers: a number moved into N and U items (item, literal, decimal, table element), national text moved into a number, a number against");
    println!("           national text and ZERO, class tests, a condition-name with a range and SET TO TRUE");
    for r in &rows {
        println!("{r}");
    }
    println!("  rcrun built in {rcrun_build_ms:.0} ms");
    println!("  programs: {} of {} identical; checks: {checks} passed, 0 failed", PROGRAMS.len(), PROGRAMS.len());
}
