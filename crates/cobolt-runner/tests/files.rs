// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T38 (R11 for files, AC9 groundwork): a desktop program's files on disk, of
//! every organisation, behave on the WebAssembly path as they do under the interpreter. Each
//! program below runs twice, packaged into a copy of the built `prc-runner` and under `rcrun run`,
//! each in a folder of its own. The two runs must print the same lines (every FILE STATUS is
//! printed) and leave the same files, byte for byte.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use cobolt_program_package::Kind;

/// SEQUENTIAL and LINE SEQUENTIAL: OUTPUT, EXTEND, I-O with REWRITE, INPUT to the end and past it,
/// an OPEN of an open file and a CLOSE of a closed one, CLOSE WITH LOCK, an absent file with and
/// without OPTIONAL, a path held in a data item, and a variable-length file with DEPENDING ON.
const SEQUENTIAL: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T38SEQ.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT SEQ-FILE ASSIGN TO \"seq.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS.
    SELECT LINE-FILE ASSIGN TO WS-LINE-PATH
        ORGANIZATION IS LINE SEQUENTIAL
        FILE STATUS IS WS-FS.
    SELECT OPTIONAL OPT-FILE ASSIGN TO \"optional.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS.
    SELECT MISSING-FILE ASSIGN TO \"missing.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS.
    SELECT VAR-FILE ASSIGN TO \"var.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS.
DATA DIVISION.
FILE SECTION.
FD SEQ-FILE.
01 SEQ-REC.
   05 SEQ-ID   PIC 9(4).
   05 SEQ-NAME PIC X(16).
FD LINE-FILE.
01 LINE-REC PIC X(30).
FD OPT-FILE.
01 OPT-REC PIC X(10).
FD MISSING-FILE.
01 MISSING-REC PIC X(10).
FD VAR-FILE
   RECORD VARYING FROM 5 TO 40 CHARACTERS DEPENDING ON WS-LEN.
01 VAR-REC PIC X(40).
WORKING-STORAGE SECTION.
01 WS-FS        PIC XX.
01 WS-LINE-PATH PIC X(40) VALUE \"lines.txt\".
01 WS-LEN       PIC 99.
01 WS-I         PIC 99.
01 WS-EOF       PIC X.
PROCEDURE DIVISION.
    OPEN OUTPUT SEQ-FILE
    DISPLAY \"open output \" WS-FS
    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 5
        MOVE WS-I TO SEQ-ID
        MOVE \"RECORD\" TO SEQ-NAME
        WRITE SEQ-REC
        DISPLAY \"write \" WS-I \" \" WS-FS
    END-PERFORM
    OPEN OUTPUT SEQ-FILE
    DISPLAY \"open of an open file \" WS-FS
    CLOSE SEQ-FILE
    DISPLAY \"close \" WS-FS
    CLOSE SEQ-FILE
    DISPLAY \"close of a closed file \" WS-FS
    OPEN EXTEND SEQ-FILE
    MOVE 6 TO SEQ-ID
    MOVE \"EXTENDED\" TO SEQ-NAME
    WRITE SEQ-REC
    DISPLAY \"extend write \" WS-FS
    CLOSE SEQ-FILE
    OPEN I-O SEQ-FILE
    READ SEQ-FILE
    DISPLAY \"read \" SEQ-ID \" \" SEQ-NAME \" \" WS-FS
    MOVE \"REWRITTEN\" TO SEQ-NAME
    REWRITE SEQ-REC
    DISPLAY \"rewrite \" WS-FS
    REWRITE SEQ-REC
    DISPLAY \"rewrite with no read \" WS-FS
    CLOSE SEQ-FILE
    OPEN INPUT SEQ-FILE
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ SEQ-FILE
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"seq \" SEQ-ID \" \" SEQ-NAME
        END-READ
    END-PERFORM
    DISPLAY \"at end \" WS-FS
    READ SEQ-FILE
        AT END DISPLAY \"at end again\"
    END-READ
    DISPLAY \"past the end \" WS-FS
    CLOSE SEQ-FILE WITH LOCK
    DISPLAY \"close with lock \" WS-FS
    OPEN INPUT SEQ-FILE
    DISPLAY \"open after lock \" WS-FS
    OPEN OUTPUT LINE-FILE
    MOVE \"first line\" TO LINE-REC
    WRITE LINE-REC
    MOVE \"  second, indented\" TO LINE-REC
    WRITE LINE-REC
    CLOSE LINE-FILE
    OPEN INPUT LINE-FILE
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ LINE-FILE
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"line [\" LINE-REC \"]\"
        END-READ
    END-PERFORM
    CLOSE LINE-FILE
    OPEN INPUT MISSING-FILE
    DISPLAY \"absent \" WS-FS
    OPEN INPUT OPT-FILE
    DISPLAY \"absent optional \" WS-FS
    READ OPT-FILE
        AT END DISPLAY \"optional at end \" WS-FS
    END-READ
    CLOSE OPT-FILE
    OPEN OUTPUT VAR-FILE
    MOVE 10 TO WS-LEN
    MOVE \"ten bytes!\" TO VAR-REC
    WRITE VAR-REC
    DISPLAY \"var write 10 \" WS-FS
    MOVE 3 TO WS-LEN
    WRITE VAR-REC
    DISPLAY \"var write 3 \" WS-FS
    MOVE 25 TO WS-LEN
    MOVE \"twenty-five bytes long...\" TO VAR-REC
    WRITE VAR-REC
    DISPLAY \"var write 25 \" WS-FS
    CLOSE VAR-FILE
    OPEN INPUT VAR-FILE
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ VAR-FILE
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"var \" WS-LEN \" [\" VAR-REC \"]\"
        END-READ
    END-PERFORM
    CLOSE VAR-FILE
    STOP RUN.
";

/// RELATIVE: a sequential creator told each record's number (and status 14 once the number no
/// longer fits its one-digit RELATIVE KEY), then dynamic access: WRITE by number, a duplicate
/// (22), READ by number and a missing one (23), REWRITE, DELETE, START and READ NEXT.
const RELATIVE: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T38REL.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT REL-SEQ ASSIGN TO \"relseq.dat\"
        ORGANIZATION IS RELATIVE
        ACCESS MODE IS SEQUENTIAL
        RELATIVE KEY IS WS-SK
        FILE STATUS IS WS-FS.
    SELECT REL-FILE ASSIGN TO \"rel.dat\"
        ORGANIZATION IS RELATIVE
        ACCESS MODE IS DYNAMIC
        RELATIVE KEY IS WS-RK
        FILE STATUS IS WS-FS.
DATA DIVISION.
FILE SECTION.
FD REL-SEQ.
01 SEQ-REC.
   05 SEQ-TAG PIC X(4).
   05 SEQ-NUM PIC 99.
   05 FILLER  PIC XX.
FD REL-FILE.
01 REL-REC.
   05 REL-ID   PIC 9(3).
   05 REL-TEXT PIC X(12).
WORKING-STORAGE SECTION.
01 WS-FS  PIC XX.
01 WS-SK  PIC 9.
01 WS-RK  PIC 9(4).
01 WS-I   PIC 99.
01 WS-EOF PIC X.
PROCEDURE DIVISION.
    OPEN OUTPUT REL-SEQ
    DISPLAY \"open output \" WS-FS
    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 11
        MOVE SPACES TO SEQ-REC
        MOVE \"ROW\" TO SEQ-TAG
        MOVE WS-I TO SEQ-NUM
        WRITE SEQ-REC
        DISPLAY \"write \" WS-I \" key \" WS-SK \" \" WS-FS
    END-PERFORM
    CLOSE REL-SEQ
    OPEN INPUT REL-SEQ
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ REL-SEQ
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"read key \" WS-SK \" [\" SEQ-REC \"]\"
        END-READ
    END-PERFORM
    DISPLAY \"end \" WS-FS
    CLOSE REL-SEQ
    OPEN OUTPUT REL-FILE
    MOVE 5 TO WS-RK
    MOVE 105 TO REL-ID
    MOVE \"FIVE\" TO REL-TEXT
    WRITE REL-REC
        INVALID KEY DISPLAY \"invalid\"
    END-WRITE
    DISPLAY \"write 5 \" WS-FS
    MOVE 2 TO WS-RK
    MOVE 102 TO REL-ID
    MOVE \"TWO\" TO REL-TEXT
    WRITE REL-REC
    DISPLAY \"write 2 \" WS-FS
    MOVE 9 TO WS-RK
    MOVE 109 TO REL-ID
    MOVE \"NINE\" TO REL-TEXT
    WRITE REL-REC
    DISPLAY \"write 9 \" WS-FS
    MOVE 5 TO WS-RK
    WRITE REL-REC
        INVALID KEY DISPLAY \"write 5 again invalid \" WS-FS
    END-WRITE
    CLOSE REL-FILE
    OPEN I-O REL-FILE
    MOVE 2 TO WS-RK
    READ REL-FILE
        INVALID KEY DISPLAY \"read 2 invalid\"
    END-READ
    DISPLAY \"read 2 \" REL-ID \" \" REL-TEXT \" \" WS-FS
    MOVE 3 TO WS-RK
    READ REL-FILE
        INVALID KEY DISPLAY \"read 3 invalid \" WS-FS
    END-READ
    MOVE 2 TO WS-RK
    MOVE \"TWO, AGAIN\" TO REL-TEXT
    REWRITE REL-REC
    DISPLAY \"rewrite 2 \" WS-FS
    MOVE 9 TO WS-RK
    DELETE REL-FILE
    DISPLAY \"delete 9 \" WS-FS
    READ REL-FILE
        INVALID KEY DISPLAY \"read 9 invalid \" WS-FS
    END-READ
    MOVE 1 TO WS-RK
    START REL-FILE KEY IS NOT LESS THAN WS-RK
    DISPLAY \"start \" WS-FS
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ REL-FILE NEXT RECORD
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"next \" WS-RK \" \" REL-ID \" \" REL-TEXT
        END-READ
    END-PERFORM
    CLOSE REL-FILE
    STOP RUN.
";

/// INDEXED: dynamic access with an alternate key WITH DUPLICATES (records written out of key
/// order, a duplicate primary key 22, READ by key and a missing one 23, REWRITE, DELETE, START on
/// the primary and on the alternate key, READ NEXT), then sequential access, where a key out of
/// order is 21.
const INDEXED: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T38IDX.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT CUST-FILE ASSIGN TO \"cust.idx\"
        ORGANIZATION IS INDEXED
        ACCESS MODE IS DYNAMIC
        RECORD KEY IS CUST-ID
        ALTERNATE RECORD KEY IS CUST-CITY WITH DUPLICATES
        FILE STATUS IS WS-FS.
    SELECT SEQ-IDX ASSIGN TO \"seq.idx\"
        ORGANIZATION IS INDEXED
        ACCESS MODE IS SEQUENTIAL
        RECORD KEY IS SQ-KEY
        FILE STATUS IS WS-FS.
DATA DIVISION.
FILE SECTION.
FD CUST-FILE.
01 CUST-REC.
   05 CUST-ID   PIC 9(5).
   05 CUST-NAME PIC X(15).
   05 CUST-CITY PIC X(10).
FD SEQ-IDX.
01 SQ-REC.
   05 SQ-KEY  PIC 9(3).
   05 SQ-DATA PIC X(7).
WORKING-STORAGE SECTION.
01 WS-FS  PIC XX.
01 WS-EOF PIC X.
PROCEDURE DIVISION.
    OPEN OUTPUT CUST-FILE
    DISPLAY \"open output \" WS-FS
    MOVE 30 TO CUST-ID
    MOVE \"CARLA\" TO CUST-NAME
    MOVE \"RIO\" TO CUST-CITY
    WRITE CUST-REC
    DISPLAY \"write 30 \" WS-FS
    MOVE 10 TO CUST-ID
    MOVE \"ANA\" TO CUST-NAME
    MOVE \"SAO PAULO\" TO CUST-CITY
    WRITE CUST-REC
    DISPLAY \"write 10 \" WS-FS
    MOVE 50 TO CUST-ID
    MOVE \"EVA\" TO CUST-NAME
    MOVE \"RIO\" TO CUST-CITY
    WRITE CUST-REC
    DISPLAY \"write 50 \" WS-FS
    MOVE 20 TO CUST-ID
    MOVE \"BRUNO\" TO CUST-NAME
    MOVE \"RECIFE\" TO CUST-CITY
    WRITE CUST-REC
    DISPLAY \"write 20 \" WS-FS
    MOVE 40 TO CUST-ID
    MOVE \"DAVI\" TO CUST-NAME
    MOVE \"RIO\" TO CUST-CITY
    WRITE CUST-REC
    DISPLAY \"write 40 \" WS-FS
    MOVE 10 TO CUST-ID
    WRITE CUST-REC
        INVALID KEY DISPLAY \"write 10 again invalid \" WS-FS
    END-WRITE
    CLOSE CUST-FILE
    OPEN I-O CUST-FILE
    MOVE 20 TO CUST-ID
    READ CUST-FILE
        INVALID KEY DISPLAY \"read 20 invalid\"
    END-READ
    DISPLAY \"read 20 \" CUST-NAME \" \" CUST-CITY \" \" WS-FS
    MOVE 99 TO CUST-ID
    READ CUST-FILE
        INVALID KEY DISPLAY \"read 99 invalid \" WS-FS
    END-READ
    MOVE 20 TO CUST-ID
    MOVE \"BRUNO SILVA\" TO CUST-NAME
    MOVE \"RECIFE\" TO CUST-CITY
    REWRITE CUST-REC
    DISPLAY \"rewrite 20 \" WS-FS
    MOVE 50 TO CUST-ID
    DELETE CUST-FILE
    DISPLAY \"delete 50 \" WS-FS
    MOVE 15 TO CUST-ID
    START CUST-FILE KEY IS NOT LESS THAN CUST-ID
    DISPLAY \"start id 15 \" WS-FS
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ CUST-FILE NEXT RECORD
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"by id \" CUST-ID \" \" CUST-NAME \" \" CUST-CITY
        END-READ
    END-PERFORM
    MOVE \"RIO\" TO CUST-CITY
    START CUST-FILE KEY IS EQUAL TO CUST-CITY
    DISPLAY \"start city RIO \" WS-FS
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ CUST-FILE NEXT RECORD
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"by city \" CUST-CITY \" \" CUST-ID \" \" CUST-NAME
        END-READ
    END-PERFORM
    CLOSE CUST-FILE
    OPEN OUTPUT SEQ-IDX
    MOVE 1 TO SQ-KEY
    MOVE \"ONE\" TO SQ-DATA
    WRITE SQ-REC
    DISPLAY \"seq write 1 \" WS-FS
    MOVE 2 TO SQ-KEY
    MOVE \"TWO\" TO SQ-DATA
    WRITE SQ-REC
    DISPLAY \"seq write 2 \" WS-FS
    MOVE 5 TO SQ-KEY
    MOVE \"FIVE\" TO SQ-DATA
    WRITE SQ-REC
    DISPLAY \"seq write 5 \" WS-FS
    MOVE 4 TO SQ-KEY
    MOVE \"FOUR\" TO SQ-DATA
    WRITE SQ-REC
        INVALID KEY DISPLAY \"seq write 4 out of order \" WS-FS
    END-WRITE
    CLOSE SEQ-IDX
    OPEN INPUT SEQ-IDX
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ SEQ-IDX
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"seq \" SQ-KEY \" \" SQ-DATA
        END-READ
    END-PERFORM
    CLOSE SEQ-IDX
    STOP RUN.
";

/// The programs, by organisation, with lines each must print: the statuses the cases are there for.
const PROGRAMS: &[(&str, &str, &[&str])] = &[
    (
        "sequential",
        SEQUENTIAL,
        &[
            "open of an open file 41",
            "close of a closed file 42",
            "rewrite 00",
            "rewrite with no read 43",
            "past the end 46",
            "open after lock 38",
            "absent 35",
            "absent optional 05",
            "var write 3 44",
            "var 25 [twenty-five bytes long...",
        ],
    ),
    ("relative", RELATIVE, &["write 10 key 0 00", "end 14", "write 5 again invalid 22", "read 3 invalid 23", "read 9 invalid 23", "next 0005 105 FIVE"]),
    (
        "indexed",
        INDEXED,
        &["write 10 again invalid 22", "read 99 invalid 23", "by id 00020 BRUNO SILVA", "by city RIO        00040 DAVI", "seq write 4 out of order 21", "seq 005 FIVE"],
    ),
];

/// A folder of its own under the system's temporary folder, removed at the end.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("prc-runner-t38-{tag}-{}", std::process::id()));
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

/// Every file a run left in `dir`, by name, but the program's own source.
fn files_in(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap()))
        .filter(|(n, _)| !n.ends_with(".cbl") && !n.starts_with("prog"))
        .collect()
}

#[test]
fn files_of_every_organisation_match_the_interpreter() {
    let root = Scratch::new("files");
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
        let scan = cobolt_wasm_abi::scan::scan_module(&module).unwrap();
        let caps = scan.embedded_manifest().unwrap().capabilities;
        assert!(caps.iter().any(|c| c.starts_with("files.")), "{tag}: the manifest asks for a file capability: {caps:?}");
        assert!(scan.custom(cobolt_wasm_abi::files::FILES_SECTION).is_some(), "{tag}: the module carries its files");
        checks += 2;

        let ours = root.0.join(format!("{tag}-runner"));
        let theirs = root.0.join(format!("{tag}-rcrun"));
        std::fs::create_dir_all(&ours).unwrap();
        std::fs::create_dir_all(&theirs).unwrap();
        let exe = ours.join(format!("prog{}", std::env::consts::EXE_SUFFIX));
        let payload = cobolt_program_package::encode(&[(Kind::Module, &module)]);
        let packaged = cobolt_program_package::embed(&runner, &payload, "prc-t38-files").expect("the runner takes a payload");
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
        assert!(runner_out.status.success(), "{tag}: the runner failed: {}", String::from_utf8_lossy(&runner_out.stderr));
        assert!(rcrun_out.status.success(), "{tag}: rcrun run failed: {}", String::from_utf8_lossy(&rcrun_out.stderr));
        assert_eq!(said(&runner_out), said(&rcrun_out), "{tag}: the runner prints what rcrun run prints\nrunner stderr: {}", String::from_utf8_lossy(&runner_out.stderr));
        checks += 3;
        let lines = said(&runner_out).lines().count();
        for e in *expected {
            assert!(said(&runner_out).contains(e), "{tag}: the run prints `{e}`:\n{}", said(&runner_out));
            checks += 1;
        }

        let left = files_in(&ours);
        let right = files_in(&theirs);
        assert_eq!(left.keys().collect::<Vec<_>>(), right.keys().collect::<Vec<_>>(), "{tag}: the same files are made");
        checks += 1;
        let mut bytes = 0;
        for (name, data) in &left {
            assert!(data == &right[name], "{tag}: {name} differs ({} bytes from the runner, {} from rcrun run)", data.len(), right[name].len());
            bytes += data.len();
            checks += 1;
        }
        rows.push(format!(
            "  {tag:<10}: {lines:>2} lines printed, {} files ({bytes} bytes) equal; generate {compile_ms:.1} ms, runner {runner_ms:.1} ms, rcrun run {rcrun_ms:.1} ms",
            left.len()
        ));
    }

    println!("── files_of_every_organisation_match_the_interpreter ──");
    println!("  sequential: OUTPUT/EXTEND/I-O/INPUT, REWRITE (00, 43), past the end (46), open file (41), closed file (42),");
    println!("              WITH LOCK (38), absent (35), OPTIONAL absent (05), path in a data item, LINE SEQUENTIAL,");
    println!("              VARYING 5 TO 40 DEPENDING ON (10, 25 written; 3 out of range 44)");
    println!("  relative:   sequential creator (keys 1-9, then 10 and 11 on a 1-digit key), READ to 14,");
    println!("              dynamic WRITE/duplicate (22)/READ/missing (23)/REWRITE/DELETE/START/READ NEXT");
    println!("  indexed:    dynamic with alternate key WITH DUPLICATES, duplicate (22), missing (23), REWRITE, DELETE,");
    println!("              START on both keys, READ NEXT; sequential access out of order (21)");
    for r in &rows {
        println!("{r}");
    }
    println!("  rcrun built in {rcrun_build_ms:.0} ms");
    println!("  checks: {checks} passed, 0 failed");
}
