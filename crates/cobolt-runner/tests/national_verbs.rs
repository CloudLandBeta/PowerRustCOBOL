// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T50a2 (R11, M6): `INSPECT`, `STRING` and `UNSTRING` over national (`PIC N`) and
//! UTF-8 (`PIC U`) data work by character in a packaged program, as they do under the interpreter.
//! Each program below runs twice, packaged into a copy of the built `prc-runner` and under
//! `rcrun run`, each in a folder of its own; the two runs must print the same lines (the timing
//! lines the test programs write are left out) and end with the same exit code.
//!
//! The programs are the repository's own: `tests/cobol/national/nat_basics.cbl`, the two interpreter
//! tests of `cobolt-runtime/tests/test_national.rs` that name these verbs (`ac7_inspect_counts_characters`
//! and `ac7_string_and_unstring_by_character`, joined into one program), and one that carries the
//! cases those leave out: a supplementary character, a receiver that runs out of room, a pointer,
//! a UTF-8 item, `JUSTIFIED RIGHT`, and `CONVERTING`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use cobolt_program_package::Kind;

const NAT_BASICS: &str = include_str!("../../../tests/cobol/national/nat_basics.cbl");

/// The two interpreter tests AC7, in one program: what they expect, in their own words, but for
/// the first item, which the tests declare one character short (`PIC N(12)` for thirteen
/// characters; the compiler refuses that VALUE), so it holds thirteen here.
const INTERPRETER_TESTS: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T50A2.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N      PIC N(13) VALUE N\"ação, coração\".
01 WS-C      PIC 9(3) VALUE 0.
01 WS-D      PIC 9(3) VALUE 0.
01 WS-E      PIC 9(3) VALUE 0.
01 WS-A      PIC N(4) VALUE N\"Ação\".
01 WS-B      PIC N(4) VALUE N\"Pão\".
01 WS-R      PIC N(10).
01 WS-P      PIC 9(3) VALUE 1.
01 WS-S      PIC N(13) VALUE N\"maçã,pêra,uva\".
01 WS-1      PIC N(5).
01 WS-2      PIC N(5).
01 WS-3      PIC N(5).
01 WS-C1     PIC 9(3).
01 WS-C2     PIC 9(3).
PROCEDURE DIVISION.
MAIN-PARA.
    INSPECT WS-N TALLYING WS-C FOR ALL N\"ç\"
    INSPECT WS-N TALLYING WS-D FOR CHARACTERS BEFORE INITIAL N\",\"
    INSPECT WS-N TALLYING WS-E FOR CHARACTERS
    DISPLAY WS-C \" \" WS-D \" \" WS-E
    INSPECT WS-N REPLACING CHARACTERS BY N\"*\" AFTER INITIAL N\", \"
    DISPLAY \"[\" WS-N \"]\"
    STRING WS-A DELIMITED BY SIZE N\"-\" DELIMITED BY SIZE WS-B DELIMITED BY SPACE INTO WS-R WITH POINTER WS-P
    DISPLAY \"[\" WS-R \"] \" WS-P
    UNSTRING WS-S DELIMITED BY N\",\" INTO WS-1 COUNT IN WS-C1 WS-2 COUNT IN WS-C2 WS-3
    DISPLAY \"[\" WS-1 \"][\" WS-2 \"][\" WS-3 \"] \" WS-C1 \" \" WS-C2
    STOP RUN.
";

/// What the two programs above leave out, each against the interpreter's own answer.
const EDGES: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T50A2E.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N      PIC N(6).
01 WS-J      PIC N(6) JUSTIFIED RIGHT.
01 WS-U      PIC U(6).
01 WS-X      PIC X(12).
01 WS-P      PIC 9(3).
01 WS-C      PIC 9(3).
01 WS-T      PIC 9(3).
01 WS-D      PIC N(2).
PROCEDURE DIVISION.
MAIN-PARA.
    MOVE SPACE TO WS-N
    MOVE 3 TO WS-P
    STRING N\"ab\" DELIMITED BY SIZE N\"cdefgh\" DELIMITED BY SIZE INTO WS-N WITH POINTER WS-P
        ON OVERFLOW DISPLAY \"overflow at \" WS-P
    END-STRING
    DISPLAY \"[\" WS-N \"] \" WS-P
    STRING U\"a😀b\" DELIMITED BY SIZE INTO WS-U
    DISPLAY \"[\" WS-U \"]\"
    STRING N\"xy\" DELIMITED BY SIZE INTO WS-J
    DISPLAY \"[\" WS-J \"]\"
    MOVE N\"a😀b😀c\" TO WS-U
    MOVE 0 TO WS-C
    INSPECT WS-U TALLYING WS-C FOR ALL U\"😀\"
    DISPLAY \"emoji \" WS-C
    INSPECT WS-U CONVERTING U\"abc\" TO U\"ABC\"
    DISPLAY \"[\" WS-U \"]\"
    MOVE N\"a,b,,c\" TO WS-N
    MOVE 1 TO WS-P
    MOVE 0 TO WS-T
    UNSTRING WS-N DELIMITED BY ALL N\",\" INTO WS-D COUNT IN WS-C WS-J WS-U
        WITH POINTER WS-P TALLYING IN WS-T
    DISPLAY \"[\" WS-D \"][\" WS-J \"][\" WS-U \"] \" WS-C \" \" WS-P \" \" WS-T
    MOVE \"abc\" TO WS-X
    INSPECT WS-X REPLACING ALL N\"b\" BY N\"é\"
    DISPLAY \"[\" WS-X \"]\"
    STOP RUN.
";

/// (tag, program, the lines the run must print)
type Program = (&'static str, &'static str, &'static [&'static str]);

const PROGRAMS: &[Program] = &[
    ("nat-basics", NAT_BASICS, &["rows: bulk iterations 0100000 tallied 0200000", "PASS 018 FAIL 000"]),
    ("interpreter", INTERPRETER_TESTS, &["002 004 013", "[ação, *******]", "[Ação-Pão  ] 009", "[maçã ][pêra ][uva  ] 004 004"]),
    (
        "edges",
        EDGES,
        &["overflow at 007", "[  abcd] 007", "[a😀b   ]", "[    xy]", "emoji 002", "[A😀B😀C ]", "[a ][     b][c     ] 001 007 003"],
    ),
];

/// A folder of its own under the system's temporary folder, removed at the end.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("prc-runner-t50a2-{tag}-{}", std::process::id()));
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
fn the_verbs_over_national_and_utf8_data_match_the_interpreter() {
    let root = Scratch::new("verbs");
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
        let packaged = cobolt_program_package::embed(&runner, &payload, "prc-t50a2-national").expect("the runner takes a payload");
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

    println!("── the_verbs_over_national_and_utf8_data_match_the_interpreter (T50a2) ──");
    println!("  nat-basics:  the repository's national-data program (18 cases), INSPECT, STRING and UNSTRING among them, 100,000 passes of MOVE and INSPECT");
    println!("  interpreter: the two interpreter tests AC7 (INSPECT counts characters; STRING and UNSTRING by character), line for line");
    println!("  edges:       a pointer and an overflow, a supplementary character, JUSTIFIED RIGHT, CONVERTING, ALL as a delimiter, a UTF-8 item,");
    println!("               and an alphanumeric item inspected for a national pattern");
    for r in &rows {
        println!("{r}");
    }
    println!("  rcrun built in {rcrun_build_ms:.0} ms");
    println!("  programs: {} of {} identical; checks: {checks} passed, 0 failed", PROGRAMS.len(), PROGRAMS.len());
}
