// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T41d (R10, R11, ABI A44): `COBOL::SqlDatabase`, the program-level database,
//! behaves on the WebAssembly path as it does under the interpreter. The interpreter's own programs
//! for it (`cobolt-runtime/tests/test_sql_database_object.rs`) and one more run twice: packaged into a
//! copy of the built `prc-runner`, and under `rcrun run` in a project whose SQL connection `SALES` the
//! last program reaches. The two runs must print the same lines, which must be what the interpreter's
//! own test expects. What the runner does not serve must be sent down the Cargo path, with the reason,
//! and a browser module must still refuse the object as the Backend's.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use cobolt_program_package::Kind;

/// Open, write, read back by row and by column, and close (the interpreter's test, as written there).
const ROUND_TRIP: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N     PIC 9(4).
       01 WS-ROW   PIC X(80).
       01 WS-COLS  PIC X(80).
       01 WS-NAME  PIC X(20).
       PROCEDURE DIVISION.
           MOVE COBOL::SqlDatabase::Open(":memory:") TO WS-N
           DISPLAY "open=" WS-N
           MOVE COBOL::SqlDatabase::Execute("CREATE TABLE t (id INTEGER, name TEXT)") TO WS-N
           MOVE COBOL::SqlDatabase::Execute("INSERT INTO t VALUES (1, 'Ada')") TO WS-N
           DISPLAY "inserted=" WS-N
           COBOL::SqlDatabase::Execute("INSERT INTO t VALUES (2, 'Grace')")
           MOVE COBOL::SqlDatabase::Query("SELECT id, name FROM t ORDER BY id") TO WS-N
           DISPLAY "rows=" WS-N
           MOVE COBOL::SqlDatabase::ColumnNames() TO WS-COLS
           DISPLAY "cols=[" FUNCTION TRIM(WS-COLS) "]"
           MOVE COBOL::SqlDatabase::ColumnCount() TO WS-N
           DISPLAY "ncols=" WS-N
           MOVE COBOL::SqlDatabase::ColumnName(2) TO WS-NAME
           DISPLAY "second=[" FUNCTION TRIM(WS-NAME) "]"
           PERFORM UNTIL 1 = 2
               SET WS-ROW TO COBOL::SqlDatabase::Fetch()
               IF WS-ROW = SPACES
                   EXIT PERFORM
               END-IF
               DISPLAY "row=[" FUNCTION TRIM(WS-ROW) "]"
           END-PERFORM
           DISPLAY "status=" COBOL::SqlDatabase::StatusCode
           COBOL::SqlDatabase::Close()
           DISPLAY "done"
           STOP RUN.
"#;

/// The transaction verbs, `LastError` and `StatusCode` (the interpreter's test, as written there).
const TRANSACTIONS: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N     PIC 9(4).
       01 WS-ERR   PIC X(80).
       PROCEDURE DIVISION.
           MOVE COBOL::SqlDatabase::Open(":memory:") TO WS-N
           MOVE COBOL::SqlDatabase::Execute("CREATE TABLE t (id INTEGER)") TO WS-N
           MOVE COBOL::SqlDatabase::Commit() TO WS-N
           MOVE COBOL::SqlDatabase::LastError TO WS-ERR
           DISPLAY "commit-none=" WS-N " " FUNCTION TRIM(WS-ERR)
           MOVE COBOL::SqlDatabase::BeginTransaction() TO WS-N
           MOVE COBOL::SqlDatabase::IsInTransaction() TO WS-N
           DISPLAY "open=" WS-N
           MOVE COBOL::SqlDatabase::Execute("INSERT INTO t VALUES (1)") TO WS-N
           MOVE COBOL::SqlDatabase::Rollback() TO WS-N
           MOVE COBOL::SqlDatabase::Execute("SELECT id FROM t") TO WS-N
           DISPLAY "after-rollback=" WS-N
           MOVE COBOL::SqlDatabase::BeginTransaction() TO WS-N
           MOVE COBOL::SqlDatabase::Execute("INSERT INTO t VALUES (2)") TO WS-N
           MOVE COBOL::SqlDatabase::Commit() TO WS-N
           MOVE COBOL::SqlDatabase::Execute("SELECT id FROM t") TO WS-N
           DISPLAY "after-commit=" WS-N
           MOVE COBOL::SqlDatabase::Execute("INSERT INTO nowhere VALUES (1)") TO WS-N
           MOVE COBOL::SqlDatabase::LastError TO WS-ERR
           DISPLAY "bad=" WS-N " [" FUNCTION TRIM(WS-ERR) "]"
           STOP RUN.
"#;

/// The connection named by property (the interpreter's test, as written there).
const BY_PROPERTY: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N     PIC 9(4).
       PROCEDURE DIVISION.
           MOVE ":memory:" TO COBOL::SqlDatabase::ConnectionString
           MOVE COBOL::SqlDatabase::Open() TO WS-N
           IF WS-N = 0
               DISPLAY "no connection"
           ELSE
               DISPLAY "connected"
           END-IF
           MOVE COBOL::SqlDatabase::Execute("CREATE TABLE t (id INTEGER)") TO WS-N
           DISPLAY "created=" WS-N
           MOVE COBOL::SqlDatabase::Open("/no/such/dir/x.db") TO WS-N
           DISPLAY "bad-open=" WS-N " status=" COBOL::SqlDatabase::StatusCode
           STOP RUN.
"#;

/// The project's connection `SALES` by `SqlConnection` and by `sql-connection:`, a statement built with
/// `STRING`, a verb in a condition and as a statement, value methods after a verb and a property, a
/// column named by a data item, the properties written and read back, `IsBusy`, `Cancel`, and a verb
/// after `Close`.
const NAMED: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. NAMED.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N       PIC 9(4).
       01 WS-I       PIC 9.
       01 WS-SQL     PIC X(120).
       01 WS-ROW     PIC X(80).
       01 WS-CITY    PIC X(12) VALUE "Lisbon".
       PROCEDURE DIVISION.
           MOVE "SALES" TO COBOL::SqlDatabase::SqlConnection
           MOVE COBOL::SqlDatabase::Open() TO WS-N
           IF WS-N = 0
               DISPLAY "named=no [" COBOL::SqlDatabase::LastError "]"
           ELSE
               DISPLAY "named=yes"
           END-IF
           DISPLAY "sql-connection=[" COBOL::SqlDatabase::SqlConnection "]"
           COBOL::SqlDatabase::Execute("DROP TABLE IF EXISTS city")
           COBOL::SqlDatabase::Execute("CREATE TABLE city (id INTEGER, name TEXT)")
           STRING "INSERT INTO city VALUES (1, '" DELIMITED BY SIZE
                  WS-CITY DELIMITED BY SPACE
                  "')" DELIMITED BY SIZE
                  INTO WS-SQL
           END-STRING
           MOVE COBOL::SqlDatabase::Execute(WS-SQL) TO WS-N
           DISPLAY "inserted=" WS-N
           COBOL::SqlDatabase::Execute("INSERT INTO city VALUES (2, 'Porto')")
           IF COBOL::SqlDatabase::Query("SELECT id, name FROM city ORDER BY id") = 2
               DISPLAY "query-in-if=2"
           ELSE
               DISPLAY "query-in-if=other"
           END-IF
           MOVE COBOL::SqlDatabase::FetchAll() TO WS-N
           DISPLAY "fetchall=" WS-N
           MOVE 2 TO WS-I
           MOVE COBOL::SqlDatabase::ColumnName(WS-I) TO WS-ROW
           DISPLAY "column=[" FUNCTION TRIM(WS-ROW) "]"
           SET WS-ROW TO COBOL::SqlDatabase::Fetch()::Upper()
           DISPLAY "row=[" FUNCTION TRIM(WS-ROW) "]"
           DISPLAY "next=[" COBOL::SqlDatabase::Fetch() "]"
           DISPLAY "after-last=[" COBOL::SqlDatabase::Fetch() "]"
           MOVE COBOL::SqlDatabase::Execute("SELECT name FROM nowhere") TO WS-N
           DISPLAY "failed=" WS-N " status=" COBOL::SqlDatabase::StatusCode
           DISPLAY "error=[" COBOL::SqlDatabase::LastError::Trim()::Upper() "]"
           MOVE "none" TO COBOL::SqlDatabase::LastError
           DISPLAY "written=[" COBOL::SqlDatabase::LastError "]"
           DISPLAY "resultcount=[" COBOL::SqlDatabase::ResultCount "]"
           DISPLAY "mode=[" COBOL::SqlDatabase::Mode "]"
           DISPLAY "busy=" COBOL::SqlDatabase::IsBusy()
           COBOL::SqlDatabase::Cancel()
           DISPLAY "in-transaction=" COBOL::SqlDatabase::IsInTransaction()
           COBOL::SqlDatabase::Close()
           MOVE COBOL::SqlDatabase::Execute("SELECT 1") TO WS-N
           DISPLAY "after-close=" WS-N
           MOVE COBOL::SqlDatabase::Open("sql-connection:SALES") TO WS-N
           MOVE COBOL::SqlDatabase::Query("SELECT id FROM city") TO WS-N
           DISPLAY "reopened-rows=" WS-N
           COBOL::SqlDatabase::Close()
           DISPLAY "done"
           STOP RUN.
"#;

/// The project `rcrun run` runs the programs in: the connection `SALES`, made on first use.
const MANIFEST: &str = "[project]
name = \"ParitySqlObject\"
version = \"1.0.0\"
main = \"src/t.cbl\"

[[sql-connections]]
name = \"SALES\"
path = \"data/sales.db\"
create-if-missing = true
";

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("prc-runner-t41d-{tag}-{}", std::process::id()));
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
        .map(|l| l.trim_end().to_owned())
        .collect()
}

fn line<'a>(out: &'a [String], prefix: &str) -> &'a str {
    out.iter().find(|l| l.starts_with(prefix)).map(String::as_str).unwrap_or_else(|| panic!("no {prefix} in {out:?}"))
}

/// What the interpreter's own test expects of each program (`test_sql_database_object.rs`), and of
/// [`NAMED`]: the lines both engines print must be these, not merely the same. Answers the checks made.
fn expected(name: &str, out: &[String]) -> usize {
    let checks: Vec<(bool, String)> = match name {
        "round_trip" => vec![
            (line(out, "open=").starts_with("open=000") && line(out, "open=") != "open=0000", "Open answers the handle".into()),
            (line(out, "inserted=") == "inserted=0001", "Execute answers the rows it changed".into()),
            (line(out, "rows=") == "rows=0002", "Query answers the row count".into()),
            (line(out, "cols=") == "cols=[id\tname]", "ColumnNames".into()),
            (line(out, "ncols=") == "ncols=0002", "ColumnCount".into()),
            (line(out, "second=") == "second=[name]", "ColumnName(2)".into()),
            (line(out, "row=[1") == "row=[1\tAda]", "Fetch answers a row".into()),
            (out.iter().filter(|l| l.starts_with("row=[")).count() == 2, "two rows, then SPACES".into()),
            (line(out, "status=") == "status=0", "StatusCode after a good Open".into()),
            (out.last().map(String::as_str) == Some("done"), "the program ends".into()),
        ],
        "transactions" => vec![
            (line(out, "commit-none=").starts_with("commit-none=0000 ") && line(out, "commit-none=").contains("no transaction is open"), "Commit with none open".into()),
            (line(out, "open=") == "open=0001", "IsInTransaction".into()),
            (line(out, "after-rollback=") == "after-rollback=0000", "Rollback".into()),
            (line(out, "after-commit=") == "after-commit=0001", "Commit".into()),
            (line(out, "bad=").starts_with("bad=0000 [") && !line(out, "bad=").ends_with("[]"), "a failed statement and LastError".into()),
        ],
        "by_property" => vec![
            (out.iter().any(|l| l == "connected"), "Open() through ConnectionString".into()),
            (line(out, "created=").starts_with("created="), "a statement on it".into()),
            (line(out, "bad-open=").starts_with("bad-open=0000") && line(out, "bad-open=").ends_with("status=1"), "a bad Open: 0 and StatusCode 1".into()),
        ],
        "named" => vec![
            (line(out, "named=") == "named=yes", "Open() through SqlConnection".into()),
            (line(out, "sql-connection=") == "sql-connection=[SALES]", "a property reads back as written".into()),
            (line(out, "inserted=") == "inserted=0001", "a statement built with STRING".into()),
            (line(out, "query-in-if=") == "query-in-if=2", "a verb in a condition".into()),
            (line(out, "fetchall=") == "fetchall=0002", "FetchAll".into()),
            (line(out, "column=") == "column=[name]", "ColumnName of a data item".into()),
            (line(out, "row=") == "row=[1\tLISBON]", "a value method after a verb".into()),
            (line(out, "next=") == "next=[2\tPorto]" && line(out, "after-last=") == "after-last=[]", "Fetch runs out".into()),
            (line(out, "failed=") == "failed=0000 status=0", "a failed statement leaves StatusCode".into()),
            (line(out, "error=[").contains("NOWHERE"), "value methods after a property".into()),
            (line(out, "written=") == "written=[none]", "LastError written and read back".into()),
            (line(out, "resultcount=") == "resultcount=[]" && line(out, "mode=") == "mode=[]", "a property never written is empty".into()),
            (line(out, "busy=") == "busy=0" && line(out, "in-transaction=") == "in-transaction=0", "IsBusy and IsInTransaction".into()),
            (line(out, "after-close=") == "after-close=0000", "a verb after Close".into()),
            (line(out, "reopened-rows=") == "reopened-rows=0002", "Open(\"sql-connection:SALES\") reaches the same file".into()),
        ],
        _ => unreachable!(),
    };
    for (ok, what) in &checks {
        assert!(ok, "{name}: {what}:\n{}", out.join("\n"));
    }
    checks.len()
}

/// Lines `[program] … PROCEDURE DIVISION.` with `body` as the procedure: the generator's verdict on it
/// in a desktop module (`native`) or a browser module.
fn refusal(body: &str, native: bool) -> Vec<String> {
    let src = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. R.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-N PIC 9(4).\n01 WS-ROW PIC X(80).\nPROCEDURE DIVISION.\n{body}\nSTOP RUN.\n"
    );
    let opts = cobolt_wasm::Options { native, ..cobolt_wasm::Options::default() };
    match cobolt_wasm::compile_program(&parse(&src), &opts) {
        Ok(_) => Vec::new(),
        Err(e) => e.all().into_iter().map(|(line, m)| format!("line {line}: {m}")).collect(),
    }
}

#[test]
fn the_program_level_database_matches_the_interpreter() {
    let root = Scratch::new("obj");
    let cache = root.0.join("cache");
    let t = Instant::now();
    let rcrun = rcrun();
    let rcrun_build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let runner = std::fs::read(env!("CARGO_BIN_EXE_prc-runner")).expect("the built prc-runner");
    let opts = cobolt_wasm::Options { native: true, ..cobolt_wasm::Options::default() };
    let mut checks = 0;

    // What the runner does not serve takes the Cargo path, each with its reason; a browser module's
    // database is still the Backend's.
    let refusals: [(&str, bool, &str); 6] = [
        ("MOVE \"Async\" TO COBOL::SqlDatabase::Mode", true, "asynchronous statement of the program-level database"),
        ("MOVE \"x\" TO COBOL::SqlDatabase::LastError::Trim()", true, "a desktop module runs the database's verbs"),
        ("MOVE \"WS-N\" TO COBOL::SqlDatabase::ConnectionDataItem", true, "names the program's data items"),
        ("MOVE COBOL::SqlDatabase::Visible TO WS-ROW", true, "a desktop module runs the database's verbs"),
        ("MOVE COBOL::SqlDatabase::Rows(1) TO WS-ROW", true, "a desktop module runs the database's verbs"),
        ("MOVE COBOL::SqlDatabase::Open(\":memory:\") TO WS-N", false, "runs on the backend"),
    ];
    for (body, native, why) in refusals {
        let said = refusal(body, native);
        assert!(said.iter().any(|m| m.contains(why)), "`{body}` ({}) is refused saying {why:?}: {said:?}", if native { "desktop" } else { "browser" });
        checks += 1;
    }

    let project = root.0.join("project");
    for d in ["src", "data"] {
        std::fs::create_dir_all(project.join(d)).unwrap();
    }
    std::fs::write(project.join("paritysqlobject.project.toml"), MANIFEST).unwrap();

    let programs = [("round_trip", ROUND_TRIP), ("transactions", TRANSACTIONS), ("by_property", BY_PROPERTY), ("named", NAMED)];
    let mut rows = Vec::new();
    let (mut total_compile, mut total_runner, mut total_rcrun, mut bytes) = (0.0, 0.0, 0.0, 0usize);
    for (name, src) in programs {
        let ours = root.0.join(name);
        std::fs::create_dir_all(&ours).unwrap();
        let t = Instant::now();
        let module = cobolt_wasm::compile_program(&parse(src), &opts).unwrap_or_else(|e| panic!("{name}: the generator refused it: {e}")).wasm;
        total_compile += t.elapsed().as_secs_f64() * 1000.0;
        bytes += module.len();
        let scan = cobolt_wasm_abi::scan::scan_module(&module).unwrap();
        let caps = scan.embedded_manifest().unwrap().capabilities;
        assert!(caps.iter().any(|c| c == "sql"), "{name}: the manifest asks for `sql`: {caps:?}");
        let imports: Vec<String> = scan.imports.iter().map(|i| format!("{}.{}", i.module, i.name)).collect();
        assert!(imports.iter().any(|i| i == "prn.sql_obj"), "{name}: the module imports prn.sql_obj: {imports:?}");
        checks += 2;

        // The runner's folder: the program, and the connection SALES beside it, as a Build writes it.
        let sales = ours.join("sales.db");
        std::fs::write(ours.join("sql-connections.toml"), format!("[[connection]]\nname = \"SALES\"\npath = {:?}\ncreate-if-missing = true\n", sales.display().to_string()))
            .unwrap();
        let exe = ours.join(format!("prog{}", std::env::consts::EXE_SUFFIX));
        let payload = cobolt_program_package::encode(&[(Kind::Module, &module)]);
        let packaged = cobolt_program_package::embed(&runner, &payload, "prc-t41d-obj").expect("the runner takes a payload");
        cobolt_program_package::write_executable(&exe, &packaged).expect("write the program");
        let mut a = Command::new(&exe);
        a.current_dir(&ours);
        let (runner_out, runner_ms) = run(a, &cache);

        let _ = std::fs::remove_file(project.join("data/sales.db"));
        std::fs::write(project.join("src/t.cbl"), src).unwrap();
        let mut b = Command::new(&rcrun);
        b.args(["run", "src/t.cbl", "--source-format", "free"]).current_dir(&project);
        let (rcrun_out, rcrun_ms) = run(b, &cache);

        assert!(rcrun_out.status.success(), "{name}: rcrun run failed: {}", String::from_utf8_lossy(&rcrun_out.stderr));
        assert!(runner_out.status.success(), "{name}: the runner failed: {}", String::from_utf8_lossy(&runner_out.stderr));
        let (ours_said, theirs_said) = (said(&runner_out), said(&rcrun_out));
        assert_eq!(ours_said, theirs_said, "{name}: the runner prints what rcrun run prints\nrunner stderr: {}", String::from_utf8_lossy(&runner_out.stderr));
        let n = expected(name, &ours_said);
        checks += 3 + n;
        total_runner += runner_ms;
        total_rcrun += rcrun_ms;
        rows.push(format!("  {name:<13} {:>3} lines alike, {n:>2} expectations met  runner {runner_ms:>7.1} ms  rcrun {rcrun_ms:>7.1} ms", ours_said.len()));
    }

    println!("── the_program_level_database_matches_the_interpreter ──");
    println!("  programs: the interpreter's three (round trip, transactions, connection by property) and one more:");
    println!("            SqlConnection and sql-connection:SALES, STRING-built statements, a verb in IF and as a statement,");
    println!("            value methods after a verb and a property, ColumnName of a data item, written properties read");
    println!("            back, IsBusy, Cancel, IsInTransaction, a verb after Close");
    println!("  refused:  {} constructs (Mode, a value method as a receiving field, ConnectionDataItem, Visible, Rows(1) on the desktop;", refusals.len());
    println!("            Open in a browser module)");
    for r in &rows {
        println!("{r}");
    }
    println!("  timings:  rcrun built {rcrun_build_ms:.0} ms; 4 modules ({bytes} bytes) compiled in {total_compile:.1} ms; runner {total_runner:.1} ms, rcrun run {total_rcrun:.1} ms");
    println!("  result:   {checks} of {checks} checks passed; 4 programs print the same lines on both paths");
}

/// The same three programs of the interpreter's test on a live MySQL server: each `:memory:` is
/// pointed at the server (`PRC_TEST_MYSQL_URL`) and table `t` dropped before each engine runs.
/// [`NAMED`] is left out: a packaged module carries no application name to read a connection's
/// override under. `SKIPPED` without the variable.
#[test]
#[ignore = "needs a MySQL server: PRC_TEST_MYSQL_URL"]
fn the_program_level_database_matches_the_interpreter_on_mysql() {
    let Some(url) = std::env::var("PRC_TEST_MYSQL_URL").ok().filter(|u| !u.trim().is_empty()) else {
        println!("SKIPPED: PRC_TEST_MYSQL_URL is not set");
        return;
    };
    let root = Scratch::new("obj-mysql");
    let cache = root.0.join("cache");
    let rcrun = rcrun();
    let runner = std::fs::read(env!("CARGO_BIN_EXE_prc-runner")).expect("the built prc-runner");
    let opts = cobolt_wasm::Options { native: true, ..cobolt_wasm::Options::default() };
    let project = root.0.join("project");
    for d in ["src", "data"] {
        std::fs::create_dir_all(project.join(d)).unwrap();
    }
    std::fs::write(project.join("paritysqlobject.project.toml"), MANIFEST).unwrap();
    let drop_t = || {
        let mut backend = cobolt_runtime::esql::backend::open(&url).unwrap_or_else(|e| panic!("the MySQL server: {}", e.message));
        let _ = backend.execute("DROP TABLE IF EXISTS t", &[]);
    };
    let (mut checks, mut rows, mut total_runner, mut total_rcrun) = (0, Vec::new(), 0.0, 0.0);
    for (name, original) in [("round_trip", ROUND_TRIP), ("transactions", TRANSACTIONS), ("by_property", BY_PROPERTY)] {
        assert!(original.contains("\":memory:\""), "{name} opens \":memory:\", which the test points at the server");
        let src = original.replace("\":memory:\"", &format!("\"{url}\""));
        let ours = root.0.join(name);
        std::fs::create_dir_all(&ours).unwrap();
        let module = cobolt_wasm::compile_program(&parse(&src), &opts).unwrap_or_else(|e| panic!("{name}: the generator refused it: {e}")).wasm;
        let exe = ours.join(format!("prog{}", std::env::consts::EXE_SUFFIX));
        let payload = cobolt_program_package::encode(&[(Kind::Module, &module)]);
        let packaged = cobolt_program_package::embed(&runner, &payload, "prc-t41d-obj-mysql").expect("the runner takes a payload");
        cobolt_program_package::write_executable(&exe, &packaged).expect("write the program");
        drop_t();
        let mut a = Command::new(&exe);
        a.current_dir(&ours);
        let (runner_out, runner_ms) = run(a, &cache);

        drop_t();
        std::fs::write(project.join("src/t.cbl"), &src).unwrap();
        let mut b = Command::new(&rcrun);
        b.args(["run", "src/t.cbl", "--source-format", "free"]).current_dir(&project);
        let (rcrun_out, rcrun_ms) = run(b, &cache);
        drop_t();

        assert!(rcrun_out.status.success(), "{name}: rcrun run failed: {}", String::from_utf8_lossy(&rcrun_out.stderr));
        assert!(runner_out.status.success(), "{name}: the runner failed: {}", String::from_utf8_lossy(&runner_out.stderr));
        let (ours_said, theirs_said) = (said(&runner_out), said(&rcrun_out));
        assert_eq!(ours_said, theirs_said, "{name}: on MySQL the runner prints what rcrun run prints");
        checks += 3;
        total_runner += runner_ms;
        total_rcrun += rcrun_ms;
        rows.push(format!("  {name:<13} {:>3} lines alike  runner {runner_ms:>7.1} ms  rcrun {rcrun_ms:>7.1} ms", ours_said.len()));
    }
    println!("── the_program_level_database_matches_the_interpreter_on_mysql ──");
    println!("  programs: the interpreter's three, \":memory:\" pointed at a live MySQL server, on both paths");
    for r in &rows {
        println!("{r}");
    }
    println!("  timings:  runner {total_runner:.1} ms, rcrun run {total_rcrun:.1} ms");
    println!("  result:   {checks} of {checks} checks passed; 3 programs print the same lines on both paths");
}
