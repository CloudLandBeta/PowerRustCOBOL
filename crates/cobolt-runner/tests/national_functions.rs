// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T50a1 (R11, M6): the functions over national (`PIC N`) and UTF-8 (`PIC U`) data
//! (`NATIONAL-OF`, `DISPLAY-OF`, `UPPER-CASE` and `LOWER-CASE` by Unicode's rules, the U-functions,
//! and every function that reads its argument's text) behave in a packaged program as they do under
//! the interpreter. Each program below runs twice, packaged into a copy of the built `prc-runner` and
//! under `rcrun run`, each in a folder of its own; the two runs must print the same lines (the
//! timing lines the test programs write are left out) and end with the same exit code.
//!
//! The programs are the repository's own: `tests/cobol/national/nat_functions.cbl`,
//! `tests/cobol/utf8/utf8_basics.cbl`, and one made of the four interpreter tests of
//! `cobolt-runtime/tests/test_national.rs` that name these functions (AC4, AC12, AC14 and
//! `upper_case_of_national_data_is_unicode`).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use cobolt_program_package::Kind;

const NAT_FUNCTIONS: &str = include_str!("../../../tests/cobol/national/nat_functions.cbl");
const UTF8_BASICS: &str = include_str!("../../../tests/cobol/utf8/utf8_basics.cbl");

/// AC4, AC12, AC14 and the Unicode case rule, in one program: what the interpreter's four tests
/// expect, in their own words (`test_national.rs`).
const INTERPRETER_TESTS: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T50A1.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-X      PIC X(10).
01 WS-N      PIC N(10).
01 WS-CP     PIC X(12) VALUE \"WINDOWS-1252\".
01 WS-U      PIC U(5) VALUE U\"Ação!\".
01 WS-T      PIC X(10) VALUE \"Aç€😀\".
01 WS-S      PIC X(10).
01 WS-L      PIC 9(3).
01 G.
   05 G-N    PIC N(1) VALUE N\"é\".
01 G-R REDEFINES G PIC X(2).
01 WS-N5     PIC N(5) VALUE N\"ação\".
01 WS-M      PIC N(5).
PROCEDURE DIVISION.
MAIN-PARA.
    MOVE FUNCTION DISPLAY-OF(FUNCTION NATIONAL-OF(\"Ação\")) TO WS-X
    DISPLAY \"[\" WS-X \"]\"
    MOVE FUNCTION DISPLAY-OF(N\"Ação\", 1252) TO WS-X
    MOVE FUNCTION NATIONAL-OF(WS-X, WS-CP) TO WS-N
    DISPLAY \"[\" WS-N \"]\"
    MOVE FUNCTION ULENGTH(WS-U) TO WS-L
    DISPLAY WS-L
    MOVE FUNCTION ULENGTH(FUNCTION TRIM(WS-T)) TO WS-L
    DISPLAY WS-L
    MOVE FUNCTION UPOS(WS-T, 3) TO WS-L
    DISPLAY WS-L
    MOVE FUNCTION USUBSTR(WS-T, 2, 2) TO WS-S
    DISPLAY \"[\" WS-S \"]\"
    MOVE FUNCTION UWIDTH(WS-T, 4) TO WS-L
    DISPLAY WS-L
    MOVE FUNCTION USUPPLEMENTARY(WS-T) TO WS-L
    DISPLAY WS-L
    MOVE FUNCTION UVALID(WS-T) TO WS-L
    DISPLAY WS-L
    MOVE FUNCTION UVALID(G-R) TO WS-L
    DISPLAY WS-L
    MOVE FUNCTION UPPER-CASE(WS-N5) TO WS-M
    DISPLAY \"[\" WS-M \"]\"
    MOVE FUNCTION LOWER-CASE(N\"ÉLAN\") TO WS-M
    DISPLAY \"[\" WS-M \"]\"
    STOP RUN.
";

/// (tag, program, the lines the run must print, what each of them is)
type Program = (&'static str, &'static str, &'static [&'static str]);

const PROGRAMS: &[Program] = &[
    ("nat-functions", NAT_FUNCTIONS, &["rows: round trips 0050000", "PASS 007 FAIL 000"]),
    ("utf8-basics", UTF8_BASICS, &["rows: bulk iterations 0100000 sum 001100000", "PASS 014 FAIL 000"]),
    (
        "interpreter",
        INTERPRETER_TESTS,
        &["[Ação    ]", "[Ação      ]", "005", "004", "004", "[ç€     ]", "004", "007", "000", "002", "[AÇÃO ]", "[élan ]"],
    ),
];

/// A folder of its own under the system's temporary folder, removed at the end.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("prc-runner-t50a1-{tag}-{}", std::process::id()));
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
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let t = Instant::now();
    let mut child = cmd.spawn().expect("start the program");
    let _ = child.stdin.take().unwrap().write_all(b"");
    let out = child.wait_with_output().unwrap();
    (out, t.elapsed().as_secs_f64() * 1000.0)
}

/// What a run printed, without the timing lines (`bulk: … ms`), which differ from run to run.
fn lines_of(o: &Output) -> Vec<String> {
    String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.starts_with("bulk: ")).map(str::to_owned).collect()
}

#[test]
fn the_functions_over_national_and_utf8_data_match_the_interpreter() {
    let root = Scratch::new("fn");
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
        let packaged = cobolt_program_package::embed(&runner, &payload, "prc-t50a1-national").expect("the runner takes a payload");
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

    println!("── the_functions_over_national_and_utf8_data_match_the_interpreter (T50a1) ──");
    println!("  nat-functions: NATIONAL-OF and DISPLAY-OF by UTF-8, Windows-1252 and ISO-8859-1 (by name, by CCSID, from a data item),");
    println!("                 the substitution character, UPPER-CASE and LOWER-CASE of national data, 50,000 round trips");
    println!("  utf8-basics:   ULENGTH, UPOS, USUBSTR, UWIDTH, USUPPLEMENTARY and UVALID over alphanumeric text and over bytes that are");
    println!("                 not UTF-8, 100,000 passes of ULENGTH and UPOS");
    println!("  interpreter:   the four interpreter tests AC4, AC12, AC14 and the Unicode case rule, line for line");
    for r in &rows {
        println!("{r}");
    }
    println!("  rcrun built in {rcrun_build_ms:.0} ms");
    println!("  programs: {} of {} identical; checks: {checks} passed, 0 failed", PROGRAMS.len(), PROGRAMS.len());
}
