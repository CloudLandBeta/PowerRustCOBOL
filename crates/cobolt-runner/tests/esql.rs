// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T41 (R10, R11, ABI A41): `EXEC SQL` behaves on the WebAssembly path as it does
//! under the interpreter. Each program of the interpreter's own embedded-SQL suite
//! (`tests/cobol/esql`, spec 087) runs twice: packaged into a copy of the built `prc-runner`, and
//! under `rcrun run` in a project whose SQL connection `SALES` the named-connection program reaches.
//! The two runs must print the same lines (what was measured left out), and every program must
//! report no failure. The program that needs an SQL descriptor area must be sent down the Cargo path
//! by the generator, which names why.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use cobolt_program_package::Kind;

/// The suite's programs the WebAssembly path runs: host variables and structures, NULL and
/// truncation, round trips of every kind of item, the status items with and without an SQLCA,
/// WHENEVER, cursors (positioned updates, batches), units of work, a named connection, national
/// and UTF-8 host variables, dynamic SQL through an SQL descriptor area (T50a5), and host variables
/// of 50,000 and 65,535 bytes.
const PROGRAMS: [&str; 13] = [
    "ac2_host_structure.cbl",
    "ac3_null_truncation.cbl",
    "ac4_round_trips.cbl",
    "ac5_status_sqlca.cbl",
    "ac5_status_standalone.cbl",
    "ac6_whenever.cbl",
    "ac7_cursors.cbl",
    "ac7b_cursor_batches.cbl",
    "ac8_units_of_work.cbl",
    "ac9_named_connection.cbl",
    "ac12_national.cbl",
    "ac11_dynamic.cbl",
    "ac19_large_host_variable.cbl",
];

/// The project `rcrun run` runs the programs in: the connection `SALES`, made on first use.
const MANIFEST: &str = "[project]
name = \"Parity\"
version = \"1.0.0\"
main = \"src/ac2_host_structure.cbl\"

[[sql-connections]]
name = \"SALES\"
path = \"data/sales.db\"
create-if-missing = true
";

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("prc-runner-t41-{tag}-{}", std::process::id()));
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

/// A program of the suite, read as the Build reads it: `COPY` and `EXEC SQL INCLUDE` (SQLCA, SQLDA)
/// expanded first. `@DIR@`, where a program keeps its own files, becomes `dir`, as the suite's own
/// harness does.
fn parse(path: &Path, dir: &Path) -> cobolt_ast::program::Program {
    parse_text(std::fs::read_to_string(path).unwrap().replace("@DIR@", &dir.to_string_lossy()), path)
}

/// `raw`, the text of the program at `path` (whose folder its copybooks are read from), parsed as
/// [`parse`] parses it.
fn parse_text(raw: String, path: &Path) -> cobolt_ast::program::Program {
    let src = match cobolt_lexer::preprocess_program(&raw, path, cobolt_lexer::SourceFormat::Free) {
        Some(x) => {
            assert!(x.errors.is_empty(), "{}: COPY: {:?}", path.display(), x.errors);
            x.text
        }
        None => raw,
    };
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
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

/// A line without what was measured (`in 000000310 ms`, `(000025641 rows/s)`), which differs
/// from run to run by nature.
fn without_timings(line: &str) -> String {
    let words: Vec<&str> = line.split_whitespace().collect();
    let digits = |w: &str| !w.is_empty() && w.trim_start_matches('(').bytes().all(|b| b.is_ascii_digit());
    let mut out = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let next = words.get(i + 1).copied().unwrap_or("");
        if digits(words[i]) && (next.starts_with("ms") || next.starts_with("rows/s")) {
            i += 2;
            continue;
        }
        // `elapsed ms: 000000010`: the label stays, the number goes.
        if words[i] == "ms:" && digits(next) {
            out.push(words[i]);
            i += 2;
            continue;
        }
        out.push(words[i]);
        i += 1;
    }
    out.join(" ")
}

/// What a run printed, line by line, without its measurements and the interpreter's log lines.
fn said(o: &Output) -> Vec<String> {
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| !(l.starts_with("\u{1b}[2m") && (l.contains(" WARN") || l.contains(" INFO"))))
        .map(without_timings)
        .collect()
}

#[test]
fn embedded_sql_matches_the_interpreter() {
    let root = Scratch::new("esql");
    let cache = root.0.join("cache");
    let t = Instant::now();
    let rcrun = rcrun();
    let rcrun_build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let runner = std::fs::read(env!("CARGO_BIN_EXE_prc-runner")).expect("the built prc-runner");
    let esql = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/esql");
    let opts = cobolt_wasm::Options { native: true, ..cobolt_wasm::Options::default() };
    let mut checks = 0;

    let project = root.0.join("project");
    for d in ["src", "data"] {
        std::fs::create_dir_all(project.join(d)).unwrap();
    }
    std::fs::write(project.join("parity.project.toml"), MANIFEST).unwrap();

    let mut rows = Vec::new();
    let (mut total_compile, mut total_runner, mut total_rcrun, mut statements) = (0.0, 0.0, 0.0, 0usize);
    for name in PROGRAMS {
        // Each engine keeps the program's own files in a folder of its own.
        let ours = root.0.join(name.trim_end_matches(".cbl"));
        std::fs::create_dir_all(&ours).unwrap();
        let theirs = project.join("data");
        let src = std::fs::read_to_string(esql.join(name)).unwrap().replace("@DIR@", &theirs.to_string_lossy());
        let t = Instant::now();
        let module = cobolt_wasm::compile_program(&parse(&esql.join(name), &ours), &opts).unwrap_or_else(|e| panic!("{name}: the generator refused it: {e}")).wasm;
        total_compile += t.elapsed().as_secs_f64() * 1000.0;
        let scan = cobolt_wasm_abi::scan::scan_module(&module).unwrap();
        let caps = scan.embedded_manifest().unwrap().capabilities;
        assert!(caps.iter().any(|c| c == "sql"), "{name}: the manifest asks for `sql`: {caps:?}");
        let section: cobolt_wasm_abi::sql::SqlSection =
            serde_json::from_slice(scan.custom(cobolt_wasm_abi::sql::SQL_SECTION).expect("a prsql section")).expect("the section reads");
        statements += section.statements.len();
        checks += 2;

        // The runner's folder: the program, and the connection SALES beside it, as a Build writes it.
        let sales = ours.join("sales.db");
        std::fs::write(ours.join("sql-connections.toml"), format!("[[connection]]\nname = \"SALES\"\npath = {:?}\ncreate-if-missing = true\n", sales.display().to_string()))
            .unwrap();
        let exe = ours.join(format!("prog{}", std::env::consts::EXE_SUFFIX));
        let payload = cobolt_program_package::encode(&[(Kind::Module, &module)]);
        let packaged = cobolt_program_package::embed(&runner, &payload, "prc-t41-esql").expect("the runner takes a payload");
        cobolt_program_package::write_executable(&exe, &packaged).expect("write the program");
        let mut a = Command::new(&exe);
        a.current_dir(&ours);
        let (runner_out, runner_ms) = run(a, &cache);

        let _ = std::fs::remove_file(project.join("data/sales.db"));
        std::fs::write(project.join("src").join(name), &src).unwrap();
        let mut b = Command::new(&rcrun);
        b.args(["run", &format!("src/{name}"), "--source-format", "free"]).current_dir(&project);
        let (rcrun_out, rcrun_ms) = run(b, &cache);

        assert!(rcrun_out.status.success(), "{name}: rcrun run failed: {}", String::from_utf8_lossy(&rcrun_out.stderr));
        assert!(runner_out.status.success(), "{name}: the runner failed: {}", String::from_utf8_lossy(&runner_out.stderr));
        let (ours_said, theirs_said) = (said(&runner_out), said(&rcrun_out));
        assert_eq!(ours_said, theirs_said, "{name}: the runner prints what rcrun run prints\nrunner stderr: {}", String::from_utf8_lossy(&runner_out.stderr));
        assert!(!ours_said.iter().any(|l| l.starts_with("FAIL")), "{name} reports a failure:\n{}", ours_said.join("\n"));
        let verdict = ours_said.iter().rev().find(|l| l.starts_with("PASS ")).cloned().unwrap_or_else(|| format!("{} lines", ours_said.len()));
        checks += 4;
        total_runner += runner_ms;
        total_rcrun += rcrun_ms;
        rows.push(format!("  {name:<28} {:>3} statements  runner {runner_ms:>7.1} ms  rcrun {rcrun_ms:>7.1} ms  {verdict}", section.statements.len()));
    }

    println!("── embedded_sql_matches_the_interpreter ──");
    println!("  programs: {} of tests/cobol/esql on the WebAssembly path", PROGRAMS.len());
    println!("  forms:    host variables and structures, indicators and NULL, truncation (01004), every item kind round-tripped,");
    println!("            SQLSTATE/SQLCODE/SQLMSG and the SQLCA fields, WHENEVER, cursors with positioned UPDATE/DELETE and");
    println!("            batches, COMMIT/ROLLBACK and the run unit's end, CONNECT TO a named connection (sql-connections.toml)");
    println!("            national and UTF-8 host variables (INSERT, SELECT INTO, NULL, a cut text) crossing as their own images");
    println!("            an SQL descriptor area (T50a5): DESCRIBE into one too small (01005) and one that holds, a pointer in a table of");
    println!("            entries, FETCH USING DESCRIPTOR through the entries' pointers and as text inside them, OPEN USING :host,");
    println!("            EXECUTE IMMEDIATE, EXECUTE USING, EXECUTE of a statement not prepared on this path (07003)");
    println!("            host variables as large as an item can be: 50000 and 65535 bytes out and back, and a 50000-byte item read as a BLOB");
    for r in &rows {
        println!("{r}");
    }
    println!(
        "  timings:  rcrun built {rcrun_build_ms:.0} ms; {statements} statements compiled in {total_compile:.1} ms; runner {total_runner:.1} ms, rcrun run {total_rcrun:.1} ms"
    );
    println!("  result:   {checks} of {checks} checks passed; {} programs print the same lines on both paths", PROGRAMS.len());
}

/// The tables a program creates (`CREATE TABLE name`), which a server keeps between runs.
fn created_tables(src: &str) -> Vec<String> {
    let upper = src.to_ascii_uppercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(k) = upper[from..].find("CREATE TABLE ") {
        let s = from + k + "CREATE TABLE ".len();
        let name: String = upper[s..].trim_start().chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        if !name.is_empty() && name != "IF" && !out.contains(&name) {
            out.push(name);
        }
        from = s;
    }
    out
}

/// The same programs on a live MySQL server: each `CONNECT TO ':memory:'` is pointed at the server
/// (`PRC_TEST_MYSQL_URL`), as the interpreter's live suite does, and the tables a program creates are
/// dropped before each engine runs it. The named-connection program is left out: a packaged module
/// carries no application name to read a connection's override under, and the SqlDatabase twin on
/// MySQL (`cobolt-cli`) already reaches the server through named connections on both paths.
/// `SKIPPED` without the variable.
#[test]
#[ignore = "needs a MySQL server: PRC_TEST_MYSQL_URL"]
fn embedded_sql_matches_the_interpreter_on_mysql() {
    let Some(url) = std::env::var("PRC_TEST_MYSQL_URL").ok().filter(|u| !u.trim().is_empty()) else {
        println!("SKIPPED: PRC_TEST_MYSQL_URL is not set");
        return;
    };
    let root = Scratch::new("esql-mysql");
    let cache = root.0.join("cache");
    let rcrun = rcrun();
    let runner = std::fs::read(env!("CARGO_BIN_EXE_prc-runner")).expect("the built prc-runner");
    let esql = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/esql");
    let opts = cobolt_wasm::Options { native: true, ..cobolt_wasm::Options::default() };
    let project = root.0.join("project");
    for d in ["src", "data"] {
        std::fs::create_dir_all(project.join(d)).unwrap();
    }
    std::fs::write(project.join("parity.project.toml"), MANIFEST).unwrap();
    let drop_tables = |src: &str| {
        let mut backend = cobolt_runtime::esql::backend::open(&url).unwrap_or_else(|e| panic!("the MySQL server: {}", e.message));
        for table in created_tables(src) {
            let _ = backend.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
        }
    };
    let to_server = |text: &str, dir: &Path| text.replace("@DIR@", &dir.to_string_lossy()).replace("CONNECT TO ':memory:'", &format!("CONNECT TO '{url}'"));

    let (mut checks, mut rows, mut total_runner, mut total_rcrun) = (0, Vec::new(), 0.0, 0.0);
    let programs: Vec<&str> = PROGRAMS.iter().copied().filter(|n| *n != "ac9_named_connection.cbl").collect();
    for name in &programs {
        let original = std::fs::read_to_string(esql.join(name)).unwrap();
        assert!(original.contains("CONNECT TO ':memory:'"), "{name} connects to ':memory:', which the test points at the server");
        let ours = root.0.join(name.trim_end_matches(".cbl"));
        std::fs::create_dir_all(&ours).unwrap();
        let module = cobolt_wasm::compile_program(&parse_text(to_server(&original, &ours), &esql.join(name)), &opts)
            .unwrap_or_else(|e| panic!("{name}: the generator refused it: {e}"))
            .wasm;
        let exe = ours.join(format!("prog{}", std::env::consts::EXE_SUFFIX));
        let payload = cobolt_program_package::encode(&[(Kind::Module, &module)]);
        let packaged = cobolt_program_package::embed(&runner, &payload, "prc-t41-esql-mysql").expect("the runner takes a payload");
        cobolt_program_package::write_executable(&exe, &packaged).expect("write the program");
        drop_tables(&original);
        let mut a = Command::new(&exe);
        a.current_dir(&ours);
        let (runner_out, runner_ms) = run(a, &cache);

        drop_tables(&original);
        std::fs::write(project.join("src").join(name), to_server(&original, &project.join("data"))).unwrap();
        let mut b = Command::new(&rcrun);
        b.args(["run", &format!("src/{name}"), "--source-format", "free"]).current_dir(&project);
        let (rcrun_out, rcrun_ms) = run(b, &cache);
        drop_tables(&original);

        assert!(rcrun_out.status.success(), "{name}: rcrun run failed: {}", String::from_utf8_lossy(&rcrun_out.stderr));
        assert!(runner_out.status.success(), "{name}: the runner failed: {}", String::from_utf8_lossy(&runner_out.stderr));
        let (ours_said, theirs_said) = (said(&runner_out), said(&rcrun_out));
        assert_eq!(ours_said, theirs_said, "{name}: on MySQL the runner prints what rcrun run prints");
        assert!(!ours_said.iter().any(|l| l.starts_with("FAIL")), "{name} reports a failure on MySQL:\n{}", ours_said.join("\n"));
        let verdict = ours_said.iter().rev().find(|l| l.starts_with("PASS ")).cloned().unwrap_or_else(|| format!("{} lines", ours_said.len()));
        checks += 3;
        total_runner += runner_ms;
        total_rcrun += rcrun_ms;
        rows.push(format!("  {name:<28} runner {runner_ms:>7.1} ms  rcrun {rcrun_ms:>7.1} ms  {verdict}"));
    }
    println!("── embedded_sql_matches_the_interpreter_on_mysql ──");
    println!("  programs: {} of tests/cobol/esql, CONNECT TO ':memory:' pointed at a live MySQL server, on both paths", programs.len());
    for r in &rows {
        println!("{r}");
    }
    println!("  timings:  runner {total_runner:.1} ms, rcrun run {total_rcrun:.1} ms");
    println!("  result:   {checks} of {checks} checks passed; {} programs print the same lines on both paths", programs.len());
}
