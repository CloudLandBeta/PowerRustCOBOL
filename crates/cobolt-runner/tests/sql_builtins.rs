// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T41b (R10, R11, ABI A42): the SQL built-ins (`COBOL::"OPEN-DB"`, `"EXEC-SQL"`,
//! `"FETCH-ROW"`, `"NEXT-ROW"`, `"ROW-COUNT"`, `"CLOSE-DB"`, and their `CALL "COBOL-…"` form) behave
//! on the WebAssembly path as they do under the interpreter. Each program runs twice: packaged into a
//! copy of the built `prc-runner`, and under `rcrun run`. The two runs must print the same lines.
//! The Guide's CRUD example and the interpreter's own CALL-form test must also print what the
//! interpreter's tests assert (`cobolt-runtime/tests/test_sql.rs`).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use cobolt_program_package::Kind;

/// The interpreter's own CALL-form test (`sqlite_crud_via_cobol_calls`), as written there.
const CALL_CRUD: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SQL-CRUD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-CONN     PIC X(32)  VALUE ":memory:".
       01 WS-HANDLE   PIC 9(9)   VALUE 0.
       01 WS-STATUS   PIC X(128) VALUE SPACES.
       01 WS-QUERY    PIC X(256) VALUE SPACES.
       01 WS-ROWCNT   PIC 9(9)   VALUE 0.
       01 WS-COL      PIC 9(4)   VALUE 1.
       01 WS-NAME     PIC X(16)  VALUE SPACES.
       01 WS-MORE     PIC X      VALUE "N".
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-OPEN-DB" USING WS-CONN WS-HANDLE WS-STATUS
           MOVE "CREATE TABLE c (id INTEGER, name TEXT)" TO WS-QUERY
           CALL "COBOL-EXEC-SQL"
               USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           MOVE "INSERT INTO c VALUES (1,'ANA'),(2,'BRUNO'),(3,'CARLOS')"
               TO WS-QUERY
           CALL "COBOL-EXEC-SQL"
               USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           DISPLAY "INSERTED " WS-ROWCNT
           MOVE "SELECT name FROM c ORDER BY id" TO WS-QUERY
           CALL "COBOL-EXEC-SQL"
               USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           DISPLAY "ROWS " WS-ROWCNT
           MOVE "Y" TO WS-MORE
           PERFORM UNTIL WS-MORE = "N"
               MOVE 1 TO WS-COL
               CALL "COBOL-FETCH-ROW"
                   USING WS-HANDLE WS-COL WS-NAME WS-STATUS
               DISPLAY "NAME " WS-NAME
               CALL "COBOL-NEXT-ROW" USING WS-HANDLE WS-MORE
           END-PERFORM
           CALL "COBOL-CLOSE-DB" USING WS-HANDLE
           STOP RUN.
"#;

/// What the two CRUD programs print (the interpreter's tests assert these lines).
const CRUD_LINES: [&str; 5] = ["INSERTED 000000003", "ROWS 000000003", "NAME ANA", "NAME BRUNO", "NAME CARLOS"];

/// Every kind of receiver and every refusal: a connection that cannot open, a handle into an item of
/// characters, a statement the database refuses, a column named by text and by a text that is no
/// number (column 1, as the interpreter reads it), values into a short item, a `JUSTIFIED` item, a
/// numeric item (a number, a text that is none, white space around one, a floating form), a
/// numeric-edited item, a signed item with decimals and a floating-point item (one without a `VALUE`:
/// the interpreter holds a COMP-2 with one as a fixed-point number, a fault of its own); the end of the rows,
/// handle 0, a program it calls sharing the connection, and a closed handle.
const EDGES: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SQL-EDGES.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-H        PIC 9(9)   VALUE 0.
       01 WS-HX       PIC X(5)   VALUE SPACES.
       01 WS-ST       PIC X(80)  VALUE SPACES.
       01 WS-Q        PIC X(200) VALUE SPACES.
       01 WS-N        PIC 9(5)   VALUE 0.
       01 WS-NX       PIC X(8)   VALUE SPACES.
       01 WS-COLT     PIC X(3)   VALUE "2".
       01 WS-COLBAD   PIC X(3)   VALUE "ZZ".
       01 WS-QTY      PIC 9(5)   VALUE 99.
       01 WS-PRICE    PIC ZZ9.99 VALUE ZERO.
       01 WS-AMT      PIC S9(3)V99 VALUE 0.
       01 WS-F        COMP-2.
       01 WS-RJ       PIC X(10)  JUSTIFIED RIGHT VALUE SPACES.
       01 WS-NAME     PIC X(4)   VALUE SPACES.
       01 WS-MORE     PIC X      VALUE SPACE.
       PROCEDURE DIVISION.
       MAIN.
           COMPUTE WS-F = 1 / 4
           COBOL::"OPEN-DB" ( "/no/such/folder/edges.db" WS-H WS-ST )
           DISPLAY "BAD OPEN H=" WS-H " ST=" FUNCTION TRIM(WS-ST)
           COBOL::"OPEN-DB" ( ":memory:" WS-H WS-ST )
           DISPLAY "OPEN H=" WS-H " ST=[" FUNCTION TRIM(WS-ST) "]"
           COBOL::"OPEN-DB" ( "  :memory:  " WS-HX WS-ST )
           DISPLAY "OPEN2 HX=[" WS-HX "]"
           MOVE "CREATE TABLE t (id INTEGER, name TEXT, qty TEXT, price REAL)"
               TO WS-Q
           COBOL::"EXEC-SQL" ( WS-H WS-Q WS-N WS-ST )
           MOVE "INSERT INTO t VALUES (1,'ANA','42',3.5),(2,'  BRUNO  ','n/a',12.25),(3,'CARLOTA',' 7 ',-1.5),(4,'DORA','1e3',0.004)"
               TO WS-Q
           COBOL::"EXEC-SQL" ( WS-H WS-Q WS-NX WS-ST )
           DISPLAY "INSERTED NX=[" WS-NX "]"
           COBOL::"EXEC-SQL" ( WS-H "SELECT nope FROM nowhere" WS-N WS-ST )
           DISPLAY "BAD SQL N=" WS-N " ST=" FUNCTION TRIM(WS-ST)
           COBOL::"EXEC-SQL" ( WS-H "  SELECT id, name, qty, price FROM t ORDER BY id  " WS-N WS-ST )
           COBOL::"ROW-COUNT" ( WS-H WS-N )
           DISPLAY "ROWS " WS-N
           MOVE "Y" TO WS-MORE
           PERFORM UNTIL WS-MORE = "N"
               COBOL::"FETCH-ROW" ( WS-H WS-COLT WS-NAME WS-ST )
               COBOL::"FETCH-ROW" ( WS-H WS-COLBAD WS-NX WS-ST )
               COBOL::"FETCH-ROW" ( WS-H 2 WS-RJ WS-ST )
               COBOL::"FETCH-ROW" ( WS-H 3 WS-QTY WS-ST )
               COBOL::"FETCH-ROW" ( WS-H 4 WS-PRICE WS-ST )
               COBOL::"FETCH-ROW" ( WS-H 4 WS-AMT WS-ST )
               COBOL::"FETCH-ROW" ( WS-H 4 WS-F WS-ST )
               DISPLAY "ROW NAME=[" WS-NAME "] ID=[" WS-NX "] RJ=[" WS-RJ "] QTY=" WS-QTY " PRICE=[" WS-PRICE "] AMT=" WS-AMT " F=" WS-F
               COBOL::"FETCH-ROW" ( WS-H 9 WS-NAME WS-ST )
               DISPLAY "COLUMN 9 NAME=[" WS-NAME "] ST=[" FUNCTION TRIM(WS-ST) "]"
               COBOL::"NEXT-ROW" ( WS-H WS-MORE )
           END-PERFORM
           COBOL::"FETCH-ROW" ( WS-H 1 WS-NAME WS-ST )
           DISPLAY "PAST END NAME=[" WS-NAME "] ST=" FUNCTION TRIM(WS-ST)
           COBOL::"NEXT-ROW" ( WS-H WS-MORE )
           DISPLAY "NEXT AGAIN " WS-MORE
           COBOL::"FETCH-ROW" ( 0 1 WS-NAME WS-ST )
           DISPLAY "HANDLE 0 ST=" FUNCTION TRIM(WS-ST)
           CALL "COUNTER" USING WS-H WS-N
           DISPLAY "COUNTED " WS-N
           COBOL::"CLOSE-DB" ( WS-H )
           MOVE 7 TO WS-N
           COBOL::"EXEC-SQL" ( WS-H "SELECT 1" WS-N WS-ST )
           DISPLAY "CLOSED N=" WS-N " ST=" FUNCTION TRIM(WS-ST)
           COBOL::"ROW-COUNT" ( WS-H WS-N )
           COBOL::"NEXT-ROW" ( WS-H WS-MORE )
           DISPLAY "CLOSED ROWS " WS-N " NEXT " WS-MORE
           COBOL::"CLOSE-DB" ( WS-HX )
           STOP RUN.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. COUNTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 C-ST        PIC X(80).
       LINKAGE SECTION.
       01 L-H         PIC 9(9).
       01 L-N         PIC 9(5).
       PROCEDURE DIVISION USING L-H L-N.
           CALL "COBOL-EXEC-SQL" USING L-H "SELECT COUNT(*) FROM t" L-N C-ST
           CALL "COBOL-FETCH-ROW" USING L-H 1 L-N C-ST
           GOBACK.
       END PROGRAM COUNTER.
       END PROGRAM SQL-EDGES.
"#;

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("prc-runner-t41b-{tag}-{}", std::process::id()));
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

/// What a run printed, line by line, without the interpreter's log lines.
fn said(o: &Output) -> Vec<String> {
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| !(l.starts_with("\u{1b}[2m") && (l.contains(" WARN") || l.contains(" INFO"))))
        .map(|l| l.trim_end().to_string())
        .collect()
}

/// The Guide's CRUD example (`docs/database-runtime-en.md` §3), read from the document as the
/// interpreter's test reads it.
fn guide_example() -> String {
    let doc = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/database-runtime-en.md")).unwrap();
    let section = &doc[doc.find("## 3. Example").expect("the example section")..];
    let start = section.find("```cobol\n").expect("its COBOL block") + "```cobol\n".len();
    section[start..start + section[start..].find("```").unwrap()].to_string()
}

#[test]
fn the_sql_builtins_match_the_interpreter() {
    let root = Scratch::new("db");
    let cache = root.0.join("cache");
    let t = Instant::now();
    let rcrun = rcrun();
    let rcrun_build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let runner = std::fs::read(env!("CARGO_BIN_EXE_prc-runner")).expect("the built prc-runner");
    let opts = cobolt_wasm::Options { native: true, ..cobolt_wasm::Options::default() };
    let mut checks = 0;

    // Fewer arguments than a built-in takes: the generator refuses, so the Build takes the Cargo path.
    let short = "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. SHORT.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 WS-H PIC 9(9).\n       PROCEDURE DIVISION.\n           COBOL::\"OPEN-DB\" ( \":memory:\" WS-H )\n           STOP RUN.\n";
    let refused: Vec<String> = cobolt_wasm::compile_program(&parse(short), &opts)
        .err()
        .map(|e| e.all().into_iter().map(|(line, m)| format!("line {line}: {m}")).collect())
        .unwrap_or_default();
    assert!(refused.iter().any(|m| m.contains("takes the connection string")), "OPEN-DB with two arguments takes the Cargo path: {refused:?}");
    checks += 1;

    let project = root.0.join("project");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(project.join("parity.project.toml"), "[project]\nname = \"Parity\"\nversion = \"1.0.0\"\nmain = \"src/guide.cbl\"\n").unwrap();

    let guide = guide_example();
    let programs: [(&str, &str, Option<&[&str]>); 3] =
        [("guide", guide.as_str(), Some(&CRUD_LINES)), ("call_crud", CALL_CRUD, Some(&CRUD_LINES)), ("edges", EDGES, None)];
    let mut rows = Vec::new();
    let (mut total_compile, mut total_runner, mut total_rcrun, mut lines) = (0.0, 0.0, 0.0, 0usize);
    for (name, src, expected) in programs {
        let t = Instant::now();
        let module = cobolt_wasm::compile_program(&parse(src), &opts).unwrap_or_else(|e| panic!("{name}: the generator refused it: {e}")).wasm;
        total_compile += t.elapsed().as_secs_f64() * 1000.0;
        let scan = cobolt_wasm_abi::scan::scan_module(&module).unwrap();
        let caps = scan.embedded_manifest().unwrap().capabilities;
        assert!(caps.iter().any(|c| c == "sql"), "{name}: the manifest asks for `sql`: {caps:?}");
        assert!(scan.imports.iter().any(|i| i.module == "prn" && i.name == "sql_db"), "{name}: the module imports prn.sql_db");
        assert!(scan.custom(cobolt_wasm_abi::sql::SQL_SECTION).is_none(), "{name}: no EXEC SQL, so no prsql section");
        checks += 3;

        let ours = root.0.join(name);
        std::fs::create_dir_all(&ours).unwrap();
        let exe = ours.join(format!("prog{}", std::env::consts::EXE_SUFFIX));
        let payload = cobolt_program_package::encode(&[(Kind::Module, &module)]);
        let packaged = cobolt_program_package::embed(&runner, &payload, "prc-t41b-db").expect("the runner takes a payload");
        cobolt_program_package::write_executable(&exe, &packaged).expect("write the program");
        let mut a = Command::new(&exe);
        a.current_dir(&ours);
        let (runner_out, runner_ms) = run(a, &cache);

        std::fs::write(project.join("src").join(format!("{name}.cbl")), src).unwrap();
        let mut b = Command::new(&rcrun);
        b.args(["run", &format!("src/{name}.cbl"), "--source-format", "free"]).current_dir(&project);
        let (rcrun_out, rcrun_ms) = run(b, &cache);

        assert!(rcrun_out.status.success(), "{name}: rcrun run failed: {}", String::from_utf8_lossy(&rcrun_out.stderr));
        assert!(runner_out.status.success(), "{name}: the runner failed: {}", String::from_utf8_lossy(&runner_out.stderr));
        let (ours_said, theirs_said) = (said(&runner_out), said(&rcrun_out));
        assert_eq!(ours_said, theirs_said, "{name}: the runner prints what rcrun run prints\nrunner stderr: {}", String::from_utf8_lossy(&runner_out.stderr));
        checks += 3;
        if let Some(expected) = expected {
            assert_eq!(ours_said, expected, "{name}: the lines the interpreter's test asserts");
            checks += 1;
        }
        if std::env::var_os("PRC_RUNNER_SHOW").is_some() {
            println!("{name}:\n  {}", ours_said.join("\n  "));
        }
        lines += ours_said.len();
        total_runner += runner_ms;
        total_rcrun += rcrun_ms;
        rows.push(format!("  {name:<10} {:>3} lines  module {:>6} bytes  runner {runner_ms:>6.1} ms  rcrun {rcrun_ms:>6.1} ms", ours_said.len(), module.len()));
    }

    println!("── the_sql_builtins_match_the_interpreter (DESKTOP-WASM-001 T41b) ──");
    println!("  programs: the Guide's CRUD example (inline COBOL::), the interpreter's CALL \"COBOL-…\" test, and a program of edges");
    println!("  forms:    OPEN-DB (good, refused, padded), EXEC-SQL (count into 9(5) and X(8), refused statement, padded query),");
    println!("            FETCH-ROW into X(4), X(8), JUSTIFIED X(10), 9(5) (number, no number, padded, 1e3), ZZ9.99, S9(3)V99, COMP-2,");
    println!("            column by text, by a text that is no number, past the last; NEXT-ROW past the end; ROW-COUNT; handle 0;");
    println!("            a called program sharing the connection; CLOSE-DB and a closed handle; OPEN-DB with two arguments refused");
    for r in &rows {
        println!("{r}");
    }
    println!("  timings:  rcrun built {rcrun_build_ms:.0} ms; compiled in {total_compile:.1} ms; runner {total_runner:.1} ms, rcrun run {total_rcrun:.1} ms");
    println!("  result:   {checks} of {checks} checks passed; {lines} lines printed the same on both paths");
}
