// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T45 (R11, M5): `TRY … CATCH … FINALLY … END-TRY` and `THROW` (T45a), and
//! `USAGE POINTER` with `SET … TO ADDRESS OF` and `SET ADDRESS OF` (T45b), behave in a packaged
//! program as they do under the interpreter. Each program below runs twice, packaged into a copy of
//! the built `prc-runner` and under `rcrun run`, each in a folder of its own; the two runs must
//! print the same bytes and end with the same exit code.
//!
//! A `THROW` nobody catches is left out: both engines end the run at it, but the runner words a
//! run-time error as "Runtime error 1112 at line …" and `rcrun run` as "Runtime error: …", a
//! difference older than T45 (`cobolt_runner::messages`).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use cobolt_program_package::Kind;

/// Exceptions in one program: a `THROW` caught with its text and `FINALLY` after `CATCH`; a `THROW`
/// two `PERFORM`s deep, after which a `PERFORM` still returns; a `THROW` from an inner `CATCH` to
/// the outer `TRY`, skipping the inner `FINALLY`; the text cut to the `CATCH` item, `JUSTIFIED
/// RIGHT`, a group; a `TRY` with no `CATCH` clause; `EXIT PERFORM CYCLE` and `EXIT PERFORM` out of a
/// `TRY` in an inline loop; `EXIT PARAGRAPH` out of one; 500 `THROW`s caught in 1,000 turns of a loop
/// (the stack stays level); a `THROW` from `FINALLY`; `GO TO` out of a `TRY`; `STOP RUN` in a
/// paragraph `PERFORM`ed from a `TRY`, which runs its `FINALLY` before the run ends.
const TRY: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T45TRY.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-ERR    PIC X(30).
01 WS-MSG    PIC X(20) VALUE \"TOO LONG FOR TEN\".
01 WS-SHORT  PIC X(10).
01 WS-RIGHT  PIC X(20) JUSTIFIED RIGHT.
01 WS-GRP.
   05 WS-G1  PIC X(4).
   05 WS-G2  PIC X(6).
01 WS-I      PIC 9(4) VALUE 0.
01 WS-K      PIC 9 VALUE 0.
01 WS-ODD    PIC 9(4) VALUE 0.
01 WS-EVEN   PIC 9(4) VALUE 0.
PROCEDURE DIVISION.
MAIN-PARA.
    TRY
        DISPLAY \"body\"
        THROW \"BUSINESS RULE\"
        DISPLAY \"not reached\"
    CATCH EXCEPTION WS-ERR
        DISPLAY \"caught [\" WS-ERR \"]\"
    FINALLY
        DISPLAY \"finally after catch\"
    END-TRY
    TRY
        PERFORM P1
        DISPLAY \"not reached\"
    CATCH WS-ERR
        DISPLAY \"two deep [\" WS-ERR \"]\"
    END-TRY
    PERFORM P3
    TRY
        TRY
            THROW \"FIRST\"
        CATCH WS-ERR
            DISPLAY \"inner \" WS-ERR
            THROW \"SECOND\"
        FINALLY
            DISPLAY \"inner finally, not reached\"
        END-TRY
    CATCH WS-ERR
        DISPLAY \"outer \" WS-ERR
    FINALLY
        DISPLAY \"outer finally\"
    END-TRY
    TRY
        THROW WS-MSG
    CATCH WS-SHORT
        DISPLAY \"cut [\" WS-SHORT \"]\"
    END-TRY
    TRY
        THROW \"ABC\"
    CATCH WS-RIGHT
        DISPLAY \"right [\" WS-RIGHT \"]\"
    END-TRY
    MOVE ALL \"*\" TO WS-GRP
    TRY
        THROW \"AB\"
    CATCH WS-GRP
        DISPLAY \"group [\" WS-G1 \"][\" WS-G2 \"]\"
    END-TRY
    TRY
        THROW \"SWALLOWED\"
    END-TRY
    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 6
        TRY
            IF WS-I = 2
                EXIT PERFORM CYCLE
            END-IF
            IF WS-I = 4
                EXIT PERFORM
            END-IF
            DISPLAY \"loop body \" WS-I
        FINALLY
            DISPLAY \"loop finally \" WS-I
        END-TRY
    END-PERFORM
    PERFORM P4
    PERFORM P4
    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 1000
        TRY
            ADD 1 TO WS-K
            EVALUATE WS-K
                WHEN 2
                    MOVE 0 TO WS-K
                    PERFORM P2
                WHEN OTHER
                    ADD 1 TO WS-ODD
            END-EVALUATE
        CATCH WS-ERR
            ADD 1 TO WS-EVEN
        END-TRY
    END-PERFORM
    DISPLAY \"thrown \" WS-EVEN \" not thrown \" WS-ODD
    TRY
        TRY
            DISPLAY \"inner body\"
        FINALLY
            THROW \"FROM FINALLY\"
        END-TRY
        DISPLAY \"not reached\"
    CATCH WS-ERR
        DISPLAY \"from finally [\" WS-ERR \"]\"
    END-TRY
    TRY
        DISPLAY \"go to\"
        GO TO P-OUT
    FINALLY
        DISPLAY \"finally before go to\"
    END-TRY
    DISPLAY \"not reached\".
P-OUT.
    TRY
        PERFORM STOPPER
        DISPLAY \"not reached\"
    FINALLY
        DISPLAY \"finally before stop run\"
    END-TRY
    DISPLAY \"not reached\".
P1.
    DISPLAY \"in p1\"
    PERFORM P2
    DISPLAY \"p1 not reached\".
P2.
    THROW WS-MSG.
P3.
    DISPLAY \"p3 returns\".
P4.
    TRY
        DISPLAY \"in p4\"
        EXIT PARAGRAPH
    FINALLY
        DISPLAY \"p4 finally\"
    END-TRY
    DISPLAY \"not reached\".
STOPPER.
    DISPLAY \"stopping\"
    STOP RUN.
";

/// Exceptions across programs: a `THROW` in a `CALL`ed program caught by the caller, twice;
/// `GOBACK` in a called program's `TRY`, which runs its `FINALLY`; `STOP RUN` in a called program,
/// which runs the caller's `FINALLY` before the run ends.
const TRY_CALL: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T45CALL.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-ERR PIC X(20).
PROCEDURE DIVISION.
MAIN-PARA.
    PERFORM 2 TIMES
        TRY
            CALL \"THROWER\"
            DISPLAY \"not reached\"
        CATCH WS-ERR
            DISPLAY \"caught \" WS-ERR
        END-TRY
    END-PERFORM
    CALL \"LEAVER\"
    DISPLAY \"back from leaver\"
    TRY
        CALL \"STOPPER\"
        DISPLAY \"not reached\"
    FINALLY
        DISPLAY \"caller finally before stop run\"
    END-TRY
    DISPLAY \"not reached\"
    STOP RUN.
IDENTIFICATION DIVISION.
PROGRAM-ID. THROWER.
PROCEDURE DIVISION.
THROWER-PARA.
    DISPLAY \"in thrower\"
    PERFORM THROW-IT
    EXIT PROGRAM.
THROW-IT.
    THROW \"FROM THROWER\".
END PROGRAM THROWER.
IDENTIFICATION DIVISION.
PROGRAM-ID. LEAVER.
PROCEDURE DIVISION.
LEAVER-PARA.
    TRY
        DISPLAY \"leaver body\"
        GOBACK
    FINALLY
        DISPLAY \"leaver finally\"
    END-TRY
    DISPLAY \"not reached\".
END PROGRAM LEAVER.
IDENTIFICATION DIVISION.
PROGRAM-ID. STOPPER.
PROCEDURE DIVISION.
STOPPER-PARA.
    DISPLAY \"stopper stops\"
    STOP RUN.
END PROGRAM STOPPER.
END PROGRAM T45CALL.
";

/// Pointers to text: a pointer never set is `NULL`; addresses numbered in the order items are first
/// named, the same item keeping its number; pointers compared; a LINKAGE item read and written
/// through a pointer and through `ADDRESS OF`; a pointer copied; `SET ADDRESS OF … TO NULL` and
/// bound again; two pointers set to `NULL` at once.
const POINTERS: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T45PTR.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-A PIC X(5) VALUE \"HELLO\".
01 WS-B PIC X(5) VALUE \"BBBBB\".
01 WS-C PIC X(5) VALUE \"CCCCC\".
01 P1 USAGE POINTER.
01 P2 USAGE POINTER.
01 P3 USAGE POINTER.
LINKAGE SECTION.
01 LK-X PIC X(5).
PROCEDURE DIVISION.
MAIN-PARA.
    IF P1 = NULL DISPLAY \"never set is null\" END-IF
    SET P1 TO ADDRESS OF WS-B
    SET P2 TO ADDRESS OF WS-A
    SET P3 TO ADDRESS OF WS-B
    DISPLAY \"numbers \" P1 \" \" P2 \" \" P3
    IF P1 = P3 DISPLAY \"p1 = p3\" ELSE DISPLAY \"p1 <> p3\" END-IF
    IF P1 = P2 DISPLAY \"p1 = p2\" ELSE DISPLAY \"p1 <> p2\" END-IF
    SET ADDRESS OF LK-X TO P2
    DISPLAY \"through p2 [\" LK-X \"]\"
    MOVE \"WORLD\" TO LK-X
    DISPLAY \"ws-a now [\" WS-A \"]\"
    SET P3 TO P2
    DISPLAY \"p3 copies p2 \" P3
    SET ADDRESS OF LK-X TO ADDRESS OF WS-C
    DISPLAY \"straight [\" LK-X \"]\"
    MOVE \"NEW C\" TO LK-X
    DISPLAY \"ws-c now [\" WS-C \"]\"
    SET ADDRESS OF LK-X TO NULL
    SET ADDRESS OF LK-X TO P1
    DISPLAY \"through p1 [\" LK-X \"]\"
    SET P1 P2 TO NULL
    DISPLAY \"nulls \" P1 \" \" P2
    IF P1 = NULL DISPLAY \"p1 is null again\" END-IF
    STOP RUN.
";

/// Pointers to packed numbers: a LINKAGE item added to, multiplied and compared through a pointer,
/// bound to one item and then another.
const POINTERS_NUMBER: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T45PTRN.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N PIC S9(5) COMP-3 VALUE -123.
01 WS-M PIC S9(5) COMP-3 VALUE 500.
01 WS-P USAGE POINTER.
LINKAGE SECTION.
01 LK-N PIC S9(5) COMP-3.
PROCEDURE DIVISION.
MAIN-PARA.
    SET WS-P TO ADDRESS OF WS-N
    SET ADDRESS OF LK-N TO WS-P
    ADD 1000 TO LK-N
    DISPLAY \"ws-n \" WS-N \" lk-n \" LK-N
    SET WS-P TO ADDRESS OF WS-M
    SET ADDRESS OF LK-N TO WS-P
    COMPUTE LK-N = LK-N * 3
    DISPLAY \"ws-m \" WS-M \" lk-n \" LK-N
    IF LK-N > WS-N DISPLAY \"compared\" END-IF
    DISPLAY \"pointer \" WS-P
    STOP RUN.
";

/// (tag, source, lines the run must print).
const PROGRAMS: &[(&str, &str, &[&str])] = &[
    (
        "try",
        TRY,
        &[
            "caught [BUSINESS RULE                 ]",
            "finally after catch",
            "two deep [TOO LONG FOR TEN              ]",
            "p3 returns",
            "inner FIRST",
            "outer SECOND",
            "outer finally",
            "cut [TOO LONG F]",
            "right [                 ABC]",
            "group [AB  ][      ]",
            "loop finally 0002",
            "loop finally 0004",
            "p4 finally",
            "thrown 0500 not thrown 0500",
            "from finally [FROM FINALLY",
            "finally before go to",
            "stopping",
            "finally before stop run",
        ],
    ),
    (
        "try-call",
        TRY_CALL,
        &["caught FROM THROWER", "leaver finally", "back from leaver", "stopper stops", "caller finally before stop run"],
    ),
    (
        "pointers",
        POINTERS,
        &[
            "never set is null",
            "numbers 1 2 1",
            "p1 = p3",
            "p1 <> p2",
            "through p2 [HELLO]",
            "ws-a now [WORLD]",
            "p3 copies p2 2",
            "straight [CCCCC]",
            "ws-c now [NEW C]",
            "through p1 [BBBBB]",
            "nulls 0 0",
            "p1 is null again",
        ],
    ),
    ("pointers-n", POINTERS_NUMBER, &["compared", "pointer 2"]),
];

/// Lines that must not appear: what a `THROW`, a jump or `STOP RUN` skips.
const NEVER: &[&str] = &["not reached"];

/// A folder of its own under the system's temporary folder, removed at the end.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("prc-runner-t45-{tag}-{}", std::process::id()));
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

#[test]
fn exceptions_and_pointers_match_the_interpreter() {
    let root = Scratch::new("exc-ptr");
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
        let packaged = cobolt_program_package::embed(&runner, &payload, "prc-t45-exc-ptr").expect("the runner takes a payload");
        cobolt_program_package::write_executable(&exe, &packaged).expect("write the program");
        let source = theirs.join(format!("{tag}.cbl"));
        std::fs::write(&source, src).unwrap();

        let mut a = Command::new(&exe);
        a.current_dir(&ours);
        let (runner_out, runner_ms) = run(a, &cache);
        let mut b = Command::new(&rcrun);
        b.arg("run").arg(&source).current_dir(&theirs);
        let (rcrun_out, rcrun_ms) = run(b, &cache);

        let said = |o: &Output| String::from_utf8_lossy(&o.stdout).into_owned();
        let err = |o: &Output| String::from_utf8_lossy(&o.stderr).into_owned();
        assert!(runner_out.status.success(), "{tag}: the runner failed: {}", err(&runner_out));
        assert!(rcrun_out.status.success(), "{tag}: rcrun run failed: {}", err(&rcrun_out));
        assert_eq!(said(&runner_out), said(&rcrun_out), "{tag}: the runner prints what rcrun run prints\nrunner stderr: {}", err(&runner_out));
        assert_eq!(runner_out.stdout, rcrun_out.stdout, "{tag}: the same bytes");
        assert_eq!(runner_out.status.code(), rcrun_out.status.code(), "{tag}: the same exit code");
        checks += 5;
        let out = said(&runner_out);
        for e in *expected {
            assert!(out.contains(e), "{tag}: the run prints `{e}`:\n{out}");
            checks += 1;
        }
        for n in NEVER {
            assert!(!out.contains(n), "{tag}: the run never prints `{n}`:\n{out}");
            checks += 1;
        }
        rows.push(format!(
            "  {tag:<10}: {:>2} lines, {:>4} bytes, exit {}; generate {compile_ms:.1} ms ({} bytes), runner {runner_ms:.1} ms, rcrun run {rcrun_ms:.1} ms",
            out.lines().count(),
            runner_out.stdout.len(),
            runner_out.status.code().unwrap_or(-1),
            module.len()
        ));
    }

    println!("── exceptions_and_pointers_match_the_interpreter ──");
    println!("  try:        a THROW caught with its text, FINALLY after CATCH; a THROW two PERFORMs deep; a THROW from an inner");
    println!("              CATCH to the outer TRY (inner FINALLY skipped); the text cut, JUSTIFIED RIGHT, a group; no CATCH clause;");
    println!("              EXIT PERFORM CYCLE and EXIT PERFORM out of a TRY; EXIT PARAGRAPH twice; 500 THROWs caught in 1,000 turns;");
    println!("              a THROW from FINALLY; GO TO out of a TRY; STOP RUN PERFORMed from a TRY runs its FINALLY");
    println!("  try-call:   a THROW in a CALLed program caught twice; GOBACK in a called TRY runs its FINALLY; STOP RUN in a");
    println!("              called program runs the caller's FINALLY");
    println!("  pointers:   NULL never set; numbers in first-named order (1 2 1); compared; read and written through a pointer");
    println!("              and through ADDRESS OF; copied; SET ADDRESS OF … TO NULL, bound again; SET P1 P2 TO NULL");
    println!("  pointers-n: a COMP-3 LINKAGE item added to, multiplied and compared through a pointer, bound to two items");
    for r in &rows {
        println!("{r}");
    }
    println!("  rcrun built in {rcrun_build_ms:.0} ms");
    println!("  programs: {} of {} identical; checks: {checks} passed, 0 failed", PROGRAMS.len(), PROGRAMS.len());
}
