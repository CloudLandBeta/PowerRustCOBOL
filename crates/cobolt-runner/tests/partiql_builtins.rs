// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T48 (R10, R11, ABI A56): `COBOL::"PARTIQL"`, `"PARTIQL-FETCH"` and
//! `"PARTIQL-ERROR"` behave on the WebAssembly path as they do under the interpreter. The program
//! runs twice, packaged into a copy of the built `prc-runner` and under `rcrun run`, each in a folder
//! of its own. The two runs must print the same lines and leave the same files, byte for byte; the
//! interpreter's own test's answers (`cobolt-runtime/tests/test_partiql_builtin.rs`) must hold too.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use cobolt_program_package::Kind;

/// Every statement and every kind of argument and receiver, on three INDEXED files: one with an
/// alternate key WITH DUPLICATES, one whose RECORD KEY is split over two fields, and one the program
/// declares and never opens.
///
/// - INSERT with `?` taken from numeric items (`9(6)`, `S9(5)V99` negative, `COMP-3`, `ZZ9.99`), an
///   alphanumeric item, numeric and decimal literals, a text literal, `ZERO`; a duplicate key (22); a
///   text longer than its column;
/// - the file argument as a literal, in lower case with spaces around it, and in a data item;
/// - the statement in a data item and as a literal;
/// - SELECT of every item, FETCH to past the last (10) and once more; SELECT on the alternate key;
///   UPDATE; DELETE; SELECT on the split key;
/// - a numeric item holding characters that spell no number, a statement that does not parse, a
///   column the file does not have, `COUNT(*)`, a file the program does not declare and one it has not
///   opened, a FETCH and an ERROR of a file no statement ran on;
/// - the count into `9(4)`, `X(5)` and `ZZZ9`; the status into `XX`, `99` and a group; the row into
///   `X(80)`, a `JUSTIFIED RIGHT` item, a group and `X(8)`; the error into `X(120)` and a
///   `JUSTIFIED RIGHT` `X(30)`; `CALL "COBOL-PARTIQL-ERROR"`.
const PROGRAM: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T48PQ.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT ORDERS ASSIGN TO \"orders.idx\"
        ORGANIZATION IS INDEXED
        ACCESS MODE IS DYNAMIC
        RECORD KEY IS ORD-ID
        ALTERNATE RECORD KEY IS ORD-CITY WITH DUPLICATES
        FILE STATUS IS WS-FS.
    SELECT ORDER-LINES ASSIGN TO \"lines.idx\"
        ORGANIZATION IS INDEXED
        ACCESS MODE IS DYNAMIC
        RECORD KEY IS LN-KEY = LN-ORD LN-NO
        FILE STATUS IS WS-FS.
    SELECT NOTES ASSIGN TO \"notes.idx\"
        ORGANIZATION IS INDEXED
        ACCESS MODE IS DYNAMIC
        RECORD KEY IS NT-ID
        FILE STATUS IS WS-FS.
DATA DIVISION.
FILE SECTION.
FD ORDERS.
01 ORD-REC.
   05 ORD-ID     PIC 9(6).
   05 ORD-CITY   PIC X(12).
   05 ORD-AMOUNT PIC S9(5)V99.
   05 ORD-QTY    PIC 9(3).
FD ORDER-LINES.
01 LN-REC.
   05 LN-ORD  PIC 9(6).
   05 LN-NO   PIC 9(2).
   05 LN-ITEM PIC X(10).
FD NOTES.
01 NT-REC.
   05 NT-ID   PIC 9(4).
   05 NT-TEXT PIC X(20).
WORKING-STORAGE SECTION.
01 WS-FS     PIC XX.
01 WS-STMT   PIC X(160).
01 WS-FILE   PIC X(20).
01 WS-N      PIC 9(4).
01 WS-NX     PIC X(5).
01 WS-NE     PIC ZZZ9.
01 WS-ST     PIC XX.
01 WS-ST9    PIC 99.
01 WS-STG.
   05 WS-ST1 PIC X.
   05 WS-ST2 PIC X.
01 WS-ROW    PIC X(80).
01 WS-RJ     PIC X(20) JUSTIFIED RIGHT.
01 WS-ROWG.
   05 WS-RG1 PIC X(10).
   05 WS-RG2 PIC X(10).
01 WS-SHORT  PIC X(8).
01 WS-ERR    PIC X(120).
01 WS-ERRJ   PIC X(30) JUSTIFIED RIGHT.
01 WS-ID     PIC 9(6).
01 WS-CITY   PIC X(12).
01 WS-AMT    PIC S9(5)V99.
01 WS-QTY    PIC 9(3).
01 WS-PACKED PIC S9(5)V99 COMP-3.
01 WS-EDIT   PIC ZZ9.99.
01 WS-KEYG.
   05 WS-KEYN PIC 9(6).
PROCEDURE DIVISION.
MAIN.
    COBOL::\"PARTIQL-FETCH\" ( \"ORDER-LINES\" WS-ROW WS-ST )
    DISPLAY \"NONE YET \" WS-ST \" [\" WS-ROW \"]\"
    COBOL::\"PARTIQL-ERROR\" ( \"ORDER-LINES\" WS-ERR )
    DISPLAY \"NO ERROR [\" WS-ERR \"]\"
    OPEN OUTPUT ORDERS
    CLOSE ORDERS
    OPEN I-O ORDERS
    OPEN OUTPUT ORDER-LINES
    CLOSE ORDER-LINES
    OPEN I-O ORDER-LINES
    MOVE \"INSERT INTO ORDERS VALUE {'ORD-ID': ?, 'ORD-CITY': ?, 'ORD-AMOUNT': ?, 'ORD-QTY': ?}\" TO WS-STMT
    MOVE 100 TO WS-ID
    MOVE \"Lisbon\" TO WS-CITY
    MOVE -12.5 TO WS-AMT
    MOVE 3 TO WS-QTY
    COBOL::\"PARTIQL\" ( \"ORDERS\" WS-STMT WS-N WS-ST WS-ID WS-CITY WS-AMT WS-QTY )
    DISPLAY \"INS1 \" WS-N \" \" WS-ST
    COBOL::\"PARTIQL\" ( \"  orders \" WS-STMT WS-NX WS-STG 200 \"Porto\" 1234.56 7 )
    DISPLAY \"INS2 [\" WS-NX \"] [\" WS-STG \"]\"
    MOVE 999.99 TO WS-PACKED
    MOVE \" Orders\" TO WS-FILE
    COBOL::\"PARTIQL\" ( WS-FILE WS-STMT WS-NE WS-ST9 300 \"Lisbon\" WS-PACKED ZERO )
    DISPLAY \"INS3 [\" WS-NE \"] \" WS-ST9
    MOVE 12.5 TO WS-EDIT
    COBOL::\"PARTIQL\" ( \"ORDERS\" WS-STMT WS-N WS-ST 400 \"Faro\" WS-EDIT 1 )
    DISPLAY \"INS4 \" WS-N \" \" WS-ST
    COBOL::\"PARTIQL\" ( \"ORDERS\" WS-STMT WS-N WS-ST 500 \"A city name far too long\" 1 1 )
    DISPLAY \"INS5 \" WS-N \" \" WS-ST
    COBOL::\"PARTIQL-ERROR\" ( \"ORDERS\" WS-ERR )
    DISPLAY \"INS5 [\" WS-ERR \"]\"
    COBOL::\"PARTIQL\" ( \"ORDERS\" WS-STMT WS-N WS-ST WS-ID WS-CITY WS-AMT WS-QTY )
    DISPLAY \"DUP \" WS-N \" \" WS-ST
    COBOL::\"PARTIQL-ERROR\" ( \"ORDERS\" WS-ERR )
    DISPLAY \"DUP [\" WS-ERR \"]\"
    COBOL::\"PARTIQL-ERROR\" ( \"ORDERS\" WS-ERRJ )
    DISPLAY \"DUP [\" WS-ERRJ \"]\"
    MOVE \"SELECT * FROM ORDERS\" TO WS-STMT
    COBOL::\"PARTIQL\" ( \"ORDERS\" WS-STMT WS-N WS-ST )
    DISPLAY \"ALL \" WS-N \" \" WS-ST
    MOVE \"00\" TO WS-ST
    PERFORM UNTIL WS-ST NOT = \"00\"
        COBOL::\"PARTIQL-FETCH\" ( \"ORDERS\" WS-ROW WS-ST )
        DISPLAY \"ROW \" WS-ST \" [\" WS-ROW \"]\"
    END-PERFORM
    COBOL::\"PARTIQL-FETCH\" ( \"ORDERS\" WS-ROW WS-ST )
    DISPLAY \"AGAIN \" WS-ST \" [\" WS-ROW \"]\"
    MOVE \"SELECT ORD-ID, ORD-CITY FROM ORDERS WHERE ORD-CITY = ?\" TO WS-STMT
    COBOL::\"PARTIQL\" ( \"ORDERS\" WS-STMT WS-N WS-ST WS-CITY )
    DISPLAY \"CITY \" WS-N \" \" WS-ST
    COBOL::\"PARTIQL-FETCH\" ( \"ORDERS\" WS-RJ WS-ST9 )
    DISPLAY \"RJ [\" WS-RJ \"] \" WS-ST9
    COBOL::\"PARTIQL-FETCH\" ( \"ORDERS\" WS-ROWG WS-STG )
    DISPLAY \"GROUP [\" WS-RG1 \"|\" WS-RG2 \"] \" WS-STG
    COBOL::\"PARTIQL-FETCH\" ( \"ORDERS\" WS-SHORT WS-ST )
    DISPLAY \"SHORT [\" WS-SHORT \"] \" WS-ST
    MOVE \"UPDATE ORDERS SET ORD-QTY = ? WHERE ORD-ID = ?\" TO WS-STMT
    MOVE 42 TO WS-QTY
    COBOL::\"PARTIQL\" ( \"ORDERS\" WS-STMT WS-N WS-ST WS-QTY 200 )
    DISPLAY \"UPDATE \" WS-N \" \" WS-ST
    COBOL::\"PARTIQL\" ( \"ORDERS\" \"DELETE FROM ORDERS WHERE ORD-ID = ?\" WS-N WS-ST 300 )
    DISPLAY \"DELETE \" WS-N \" \" WS-ST
    MOVE \"ABCDEF\" TO WS-KEYG
    COBOL::\"PARTIQL\" ( \"ORDERS\" \"SELECT * FROM ORDERS WHERE ORD-ID = ?\" WS-N WS-ST WS-KEYN )
    DISPLAY \"NO NUMBER \" WS-N \" \" WS-ST
    COBOL::\"PARTIQL-ERROR\" ( \"ORDERS\" WS-ERR )
    DISPLAY \"NO NUMBER [\" WS-ERR \"]\"
    COBOL::\"PARTIQL\" ( \"ORDERS\" \"SELEC * FROM ORDERS\" WS-N WS-ST )
    DISPLAY \"PARSE \" WS-N \" \" WS-ST
    CALL \"COBOL-PARTIQL-ERROR\" USING \"ORDERS\" WS-ERR
    DISPLAY \"PARSE [\" WS-ERR \"]\"
    COBOL::\"PARTIQL\" ( \"ORDERS\" \"SELECT NOPE FROM ORDERS\" WS-N WS-ST )
    DISPLAY \"COLUMN \" WS-N \" \" WS-ST
    COBOL::\"PARTIQL-ERROR\" ( \"ORDERS\" WS-ERRJ )
    DISPLAY \"COLUMN [\" WS-ERRJ \"]\"
    COBOL::\"PARTIQL\" ( \"ORDERS\" \"SELECT COUNT(*) FROM ORDERS\" WS-N WS-ST )
    DISPLAY \"COUNT \" WS-N \" \" WS-ST
    COBOL::\"PARTIQL-ERROR\" ( \"ORDERS\" WS-ERR )
    DISPLAY \"COUNT [\" WS-ERR \"]\"
    COBOL::\"PARTIQL\" ( \"GHOST\" \"SELECT * FROM GHOST\" WS-N WS-ST )
    DISPLAY \"GHOST \" WS-N \" \" WS-ST
    COBOL::\"PARTIQL-ERROR\" ( \"GHOST\" WS-ERR )
    DISPLAY \"GHOST [\" WS-ERR \"]\"
    COBOL::\"PARTIQL\" ( \"NOTES\" \"SELECT * FROM NOTES\" WS-N WS-ST )
    DISPLAY \"NOT OPEN \" WS-N \" \" WS-ST
    COBOL::\"PARTIQL-ERROR\" ( \"NOTES\" WS-ERR )
    DISPLAY \"NOT OPEN [\" WS-ERR \"]\"
    MOVE \"INSERT INTO ORDER-LINES VALUE {'LN-ORD': ?, 'LN-NO': ?, 'LN-ITEM': ?}\" TO WS-STMT
    COBOL::\"PARTIQL\" ( \"ORDER-LINES\" WS-STMT WS-N WS-ST 200 1 \"Bolts\" )
    DISPLAY \"LINE1 \" WS-N \" \" WS-ST
    COBOL::\"PARTIQL\" ( \"ORDER-LINES\" WS-STMT WS-N WS-ST 200 2 \"Nuts\" )
    DISPLAY \"LINE2 \" WS-N \" \" WS-ST
    COBOL::\"PARTIQL\" ( \"ORDER-LINES\" WS-STMT WS-N WS-ST 200 2 \"Again\" )
    DISPLAY \"LINE DUP \" WS-N \" \" WS-ST
    MOVE \"SELECT LN-ITEM FROM ORDER-LINES WHERE LN-ORD = ? AND LN-NO = ?\" TO WS-STMT
    COBOL::\"PARTIQL\" ( \"ORDER-LINES\" WS-STMT WS-N WS-ST 200 2 )
    DISPLAY \"LINE KEY \" WS-N \" \" WS-ST
    COBOL::\"PARTIQL-FETCH\" ( \"ORDER-LINES\" WS-ROW WS-ST )
    DISPLAY \"LINE ROW \" WS-ST \" [\" WS-ROW \"]\"
    COBOL::\"PARTIQL-FETCH\" ( \"ORDERS\" WS-ROW WS-ST )
    DISPLAY \"ORDERS KEPT \" WS-ST
    MOVE \"SELECT * FROM ORDERS\" TO WS-STMT
    COBOL::\"PARTIQL\" ( \"ORDERS\" WS-STMT WS-N WS-ST )
    DISPLAY \"LEFT \" WS-N \" \" WS-ST
    MOVE \"00\" TO WS-ST
    PERFORM UNTIL WS-ST NOT = \"00\"
        COBOL::\"PARTIQL-FETCH\" ( \"ORDERS\" WS-ROW WS-ST )
        DISPLAY \"LEFT \" WS-ST \" [\" WS-ROW \"]\"
    END-PERFORM
    CLOSE ORDERS
    CLOSE ORDER-LINES
    COBOL::\"PARTIQL\" ( \"ORDERS\" \"SELECT * FROM ORDERS\" WS-N WS-ST )
    DISPLAY \"CLOSED \" WS-N \" \" WS-ST
    STOP RUN.
";

/// What the interpreter's own test asserts, in this program's words; and a numeric item holding
/// letters, which the interpreter sends as text (`Interpreter::partiql_arg`), so the statement fails
/// with 94 (the module's routine once sent what decoding the item made of them, and found a row count of 0).
const EXPECTED: [&str; 7] = ["INS1 0001 00", "DUP 0000 22", "DUP [DuplicateItemException", "NO NUMBER 0000 94", "PARSE 0000 94", "AGAIN 10", "CLOSED 0000 47"];

/// A folder of its own under the system's temporary folder, removed at the end.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("prc-runner-t48-{tag}-{}", std::process::id()));
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
    cmd.env("PRC_RUNNER_CACHE", cache).env("LC_ALL", "en_US.UTF-8").env_remove("COBOL_INDEXED_ENGINE");
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let t = Instant::now();
    let mut child = cmd.spawn().expect("start the program");
    let _ = child.stdin.take().unwrap().write_all(b"");
    let out = child.wait_with_output().unwrap();
    (out, t.elapsed().as_secs_f64() * 1000.0)
}

/// Every file a run left in `dir`, by name, but the program's own source and executable.
fn files_in(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap()))
        .filter(|(n, _)| !n.ends_with(".cbl") && !n.starts_with("prog"))
        .collect()
}

fn refusal(src: &str) -> Vec<String> {
    let native = cobolt_wasm::Options { native: true, ..cobolt_wasm::Options::default() };
    cobolt_wasm::compile_program(&parse(src), &native)
        .err()
        .map(|e| e.all().into_iter().map(|(line, m)| format!("line {line}: {m}")).collect())
        .unwrap_or_default()
}

#[test]
fn the_partiql_builtins_match_the_interpreter() {
    let root = Scratch::new("partiql");
    let cache = root.0.join("cache");
    let t = Instant::now();
    let rcrun = rcrun();
    let rcrun_build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let runner = std::fs::read(env!("CARGO_BIN_EXE_prc-runner")).expect("the built prc-runner");
    let mut checks = 0;

    // Fewer arguments than a built-in takes: the generator refuses, so the Build takes the Cargo path.
    let head = "IDENTIFICATION DIVISION.\nPROGRAM-ID. SHORT.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-R PIC X(9).\nPROCEDURE DIVISION.\n";
    for (call, words) in [
        ("COBOL::\"PARTIQL\" ( \"F\" \"SELECT * FROM F\" WS-R )", "takes the file, the statement"),
        ("COBOL::\"PARTIQL-FETCH\" ( \"F\" WS-R )", "takes the file and the data items that receive the row and the status"),
        ("COBOL::\"PARTIQL-ERROR\" ( \"F\" )", "takes the file and the data item"),
    ] {
        let refused = refusal(&format!("{head}    {call}\n    STOP RUN.\n"));
        assert!(refused.iter().any(|m| m.contains(words)), "`{call}` is refused, naming what it takes: {refused:?}");
        checks += 1;
    }

    let t = Instant::now();
    let opts = cobolt_wasm::Options { native: true, ..cobolt_wasm::Options::default() };
    let module = cobolt_wasm::compile_program(&parse(PROGRAM), &opts).unwrap_or_else(|e| panic!("the generator refused the program: {e}")).wasm;
    let compile_ms = t.elapsed().as_secs_f64() * 1000.0;
    let scan = cobolt_wasm_abi::scan::scan_module(&module).unwrap();
    let caps = scan.embedded_manifest().unwrap().capabilities;
    assert!(caps.iter().any(|c| c == "partiql.files") && caps.iter().any(|c| c == "files.indexed"), "the manifest asks for `partiql.files` and `files.indexed`: {caps:?}");
    assert!(scan.imports.iter().any(|i| i.module == "prn" && i.name == "pq_file"), "the module imports prn.pq_file");
    checks += 2;

    let ours = root.0.join("runner");
    let theirs = root.0.join("rcrun");
    std::fs::create_dir_all(&ours).unwrap();
    std::fs::create_dir_all(&theirs).unwrap();
    let exe = ours.join(format!("prog{}", std::env::consts::EXE_SUFFIX));
    let payload = cobolt_program_package::encode(&[(Kind::Module, &module)]);
    let packaged = cobolt_program_package::embed(&runner, &payload, "prc-t48-partiql").expect("the runner takes a payload");
    cobolt_program_package::write_executable(&exe, &packaged).expect("write the program");
    let source = theirs.join("partiql.cbl");
    std::fs::write(&source, PROGRAM).unwrap();

    let mut a = Command::new(&exe);
    a.current_dir(&ours);
    let (runner_out, runner_ms) = run(a, &cache);
    let mut b = Command::new(&rcrun);
    b.arg("run").arg(&source).current_dir(&theirs);
    let (rcrun_out, rcrun_ms) = run(b, &cache);

    let said = |o: &Output| String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(runner_out.status.success(), "the runner failed: {}", String::from_utf8_lossy(&runner_out.stderr));
    assert!(rcrun_out.status.success(), "rcrun run failed: {}", String::from_utf8_lossy(&rcrun_out.stderr));
    if std::env::var_os("PRC_RUNNER_SHOW").is_some() {
        println!("runner:\n{}\nrcrun run:\n{}", said(&runner_out), said(&rcrun_out));
    }
    let (runner_said, rcrun_said) = (said(&runner_out), said(&rcrun_out));
    let left_lines: Vec<&str> = runner_said.lines().collect();
    let right_lines: Vec<&str> = rcrun_said.lines().collect();
    for (i, (l, r)) in left_lines.iter().zip(&right_lines).enumerate() {
        assert_eq!(l, r, "line {}: the runner prints what rcrun run prints", i + 1);
    }
    assert_eq!(left_lines.len(), right_lines.len(), "both paths print as many lines\nrunner stderr: {}", String::from_utf8_lossy(&runner_out.stderr));
    checks += 3 + left_lines.len();
    for e in EXPECTED {
        assert!(left_lines.iter().any(|l| l.starts_with(e)), "the run prints `{e}`:\n{}", said(&runner_out));
        checks += 1;
    }

    let left = files_in(&ours);
    let right = files_in(&theirs);
    assert_eq!(left.keys().collect::<Vec<_>>(), right.keys().collect::<Vec<_>>(), "the same files are made");
    checks += 1;
    let mut bytes = 0;
    for (name, data) in &left {
        assert!(data == &right[name], "{name} differs ({} bytes from the runner, {} from rcrun run)", data.len(), right[name].len());
        bytes += data.len();
        checks += 1;
    }

    println!("── the_partiql_builtins_match_the_interpreter (DESKTOP-WASM-001 T48) ──");
    println!("  statements: INSERT (6, a duplicate 22, a text longer than its column), SELECT (every item, the alternate key,");
    println!("              the split key), UPDATE, DELETE; a numeric item holding no number, a statement that does not parse (94),");
    println!("              a column the file lacks, COUNT(*), an undeclared file, a file not open, a closed file");
    println!("  arguments:  9(6), S9(5)V99 negative, COMP-3, ZZ9.99, X(12), 200, 1234.56, \"Porto\", ZERO; the file as a literal,");
    println!("              in lower case with spaces, in a data item; the statement in a data item and as a literal");
    println!("  receivers:  count 9(4), X(5), ZZZ9; status XX, 99, a group; row X(80), JUSTIFIED RIGHT X(20), a group, X(8);");
    println!("              error X(120), JUSTIFIED RIGHT X(30); CALL \"COBOL-PARTIQL-ERROR\"; FETCH and ERROR before any statement");
    println!("  refused:    PARTIQL with 3 arguments, PARTIQL-FETCH with 2, PARTIQL-ERROR with 1");
    println!(
        "  run:        {} lines printed the same, {} files ({bytes} bytes) equal; module {} bytes",
        left_lines.len(),
        left.len(),
        module.len()
    );
    println!("  timings:    rcrun built {rcrun_build_ms:.0} ms; generated in {compile_ms:.1} ms; runner {runner_ms:.1} ms, rcrun run {rcrun_ms:.1} ms");
    println!("  checks:     {checks} passed, 0 failed");
}
