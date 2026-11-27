// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T20 and T24 (R2, R7, R12; AC2 groundwork): the executable a Build makes from `prc-backend-runner` and a Backend
//! is the executable a Cargo build makes, as far as whoever starts it can tell.
//!
//! The Backend is packed into a copy of the built runner (its interface, its table of exports, its module), and the same
//! Backend, from the same sources, is packed the way a Cargo-built file carries it and started through the stand-in of
//! `cobolt-server` (`standalone_probe`, which runs the very `standalone::main` such a file runs). The two are started with
//! the same arguments, and what they print on standard output and the code they exit with must be the same: `--version`,
//! `--describe`, `--check` accepted and refused. The runner then really serves: its port answers, and a stop signal ends
//! it with code 0. What only the runner says, which engine runs the programs and the module's identity, goes to standard
//! error and nowhere else (R12). A file with no Backend in it, a damaged one and a module that needs a capability the runner
//! does not provide are each refused in words, naming it.
#![cfg(unix)]

use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use cobolt_program_package::backend::BackendContainer;
use cobolt_program_package::{embed, encode, Kind};
use cobolt_server::backend::Backend;
use cobolt_server::bundle::Bundle;
use cobolt_server::testkit::backend_dir;
use cobolt_wasm::{compile_backend, BackendProgram, Options};

const EXPORTS: &str = "[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"ADD-TWO\"\n\n[[export]]\nprogram = \"GREET\"\n";

const ADD: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. ADD-TWO.\nDATA DIVISION.\nLINKAGE SECTION.\n01 REQ.\n    05 A PIC 9(4).\n    05 B PIC 9(4).\n01 RES.\n    05 TOTAL PIC 9(5).\nPROCEDURE DIVISION USING REQ RETURNING RES.\nMAIN-PARA.\n    COMPUTE TOTAL = A + B.\n    GOBACK.\n";

const GREET: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. GREET.\nDATA DIVISION.\nLINKAGE SECTION.\n01 WHO PIC X(10).\n01 MSG PIC X(20).\nPROCEDURE DIVISION USING WHO MSG.\nMAIN-PARA.\n    STRING \"HELLO \" DELIMITED BY SIZE WHO DELIMITED BY SPACE INTO MSG.\n    GOBACK.\n";

fn probe() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let p = exe.parent().unwrap().parent().unwrap().join("examples").join("standalone_probe");
    if !p.exists() {
        // `cargo test --test …` does not build examples; the stand-in is built here, once.
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let st = Command::new(cargo)
            .args(["build", "-p", "cobolt-server", "--example", "standalone_probe"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .unwrap();
        assert!(st.success(), "building the standalone_probe example failed");
    }
    assert!(p.exists(), "the example {} is not built", p.display());
    p
}

fn parse(src: &str) -> cobolt_ast::program::Program {
    use cobolt_lexer::{tokenize, SourceFormat};
    let parsed = cobolt_parser::parse(tokenize(src, SourceFormat::Free));
    assert!(parsed.diagnostics.iter().all(|d| !d.is_error()), "the source does not parse");
    parsed.program.expect("a program")
}

fn module() -> Vec<u8> {
    let programs: Vec<_> = [ADD, GREET].iter().map(|s| parse(s)).collect();
    let tops: Vec<_> = programs
        .iter()
        .map(|p| {
            let mut top = p.clone();
            for other in &programs {
                if other.identification.program_id != p.identification.program_id {
                    top.nested_programs.push(other.clone());
                }
            }
            top
        })
        .collect();
    let list: Vec<BackendProgram<'_>> = tops.iter().map(|t| BackendProgram { name: t.identification.program_id.to_ascii_uppercase(), program: t }).collect();
    compile_backend(&list, &Options { native: true, ..Options::default() }).unwrap_or_else(|e| panic!("compile failed: {e}")).wasm
}

/// What a Build puts inside a copy of the runner: the Backend as a container, in the payload.
fn container(dir: &Path, module: Vec<u8>) -> BackendContainer {
    let backend = Backend::load(dir).expect("the test Backend loads");
    BackendContainer {
        state: "per-call".into(),
        interface: backend.interface.to_json().into_bytes(),
        exports: serde_json::to_vec(&backend.export_entries()).unwrap(),
        history: b"[]".to_vec(),
        slots: b"[]".to_vec(),
        module,
        name: "PowerTest".into(),
    }
}

/// A copy of the built runner with `container` inside, named `name` in `dir`.
fn packed(dir: &Path, name: &str, container: &BackendContainer) -> PathBuf {
    let runner = std::fs::read(env!("CARGO_BIN_EXE_prc-backend-runner")).unwrap();
    let payload = encode(&[(Kind::Backend, &container.encode())]);
    let exe = embed(&runner, &payload, name).unwrap_or_else(|e| panic!("embedding the Backend: {e}"));
    let path = dir.join(name);
    cobolt_program_package::write_executable(&path, &exe).unwrap();
    path
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

struct Ran {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run(exe: &Path, dir: &Path, args: &[&str], secret_var: &str) -> Ran {
    let out = Command::new(exe).args(args).env_remove(secret_var).env_remove("NOTIFY_SOCKET").current_dir(dir).output().unwrap();
    Ran { code: out.status.code(), stdout: String::from_utf8_lossy(&out.stdout).into_owned(), stderr: String::from_utf8_lossy(&out.stderr).into_owned() }
}

fn run_probe(bundle: &Path, dir: &Path, args: &[&str], secret_var: &str) -> Ran {
    let out =
        Command::new(probe()).args(args).env("PRC_PROBE_BUNDLE", bundle).env_remove(secret_var).env_remove("NOTIFY_SOCKET").current_dir(dir).output().unwrap();
    Ran { code: out.status.code(), stdout: String::from_utf8_lossy(&out.stdout).into_owned(), stderr: String::from_utf8_lossy(&out.stderr).into_owned() }
}

#[test]
fn the_packed_runner_starts_as_a_cargo_built_backend_does() {
    let started = Instant::now();
    let dir = backend_dir("exe", EXPORTS, &[("add.cbl", ADD), ("greet.cbl", GREET)]);
    let wasm = module();
    let wasm_len = wasm.len();
    let runner = packed(&dir, "my-backend", &container(&dir, wasm));
    let bundle = dir.join("backend.bin");
    std::fs::write(&bundle, Bundle::from_dir(&dir).expect("loads").to_bytes().expect("packs")).unwrap();

    let var = format!("PRC_RUNNER_SECRET_{}", std::process::id());
    let port = free_port();
    let cfg = dir.join("backend.toml");
    std::fs::write(
        &cfg,
        format!("[server]\nlisten = \"127.0.0.1:{port}\"\nallowed_origins = [\"https://app.example.test\"]\n[auth]\nissuer = [\"https://idp.example.test/pool\"]\naudiences = [\"prc-api\"]\n[[configuration]]\nname = \"CRM\"\nAuthToken = \"env:{var}\"\n"),
    )
    .unwrap();
    let cfg_s = cfg.to_str().unwrap().to_string();
    let env_ok = dir.join("secrets.env");
    std::fs::write(&env_ok, format!("{var}=s3cr3t-value\n")).unwrap();
    std::fs::set_permissions(&env_ok, std::fs::Permissions::from_mode(0o600)).unwrap();
    let env_s = env_ok.to_str().unwrap().to_string();

    // The same arguments, the same standard output, the same exit code.
    let cases: Vec<(&str, Vec<&str>)> = vec![
        ("--version", vec!["--version"]),
        ("--describe", vec!["--describe"]),
        ("--check, accepted", vec!["--check", "--config", &cfg_s, "--env-file", &env_s]),
        ("--check, a secret missing", vec!["--check", "--config", &cfg_s]),
        ("--check, no such configuration", vec!["--check", "--config", "nope.toml"]),
        ("--service, where there is no Service Control Manager", vec!["--service", "--config", &cfg_s]),
    ];
    let mut same = 0u32;
    for (what, args) in &cases {
        let mine = run(&runner, &dir, args, &var);
        let theirs = run_probe(&bundle, &dir, args, &var);
        assert_eq!(mine.code, theirs.code, "{what}: the exit codes differ");
        assert_eq!(mine.stdout, theirs.stdout, "{what}: standard output differs");
        // Standard error is the same too, but for the one line only the runner says (R12): the engine and the module.
        let mine_err: Vec<&str> = mine.stderr.lines().filter(|l| !l.starts_with("engine: ")).collect();
        let theirs_err: Vec<&str> = theirs.stderr.lines().collect();
        assert_eq!(mine_err, theirs_err, "{what}: standard error differs beyond the engine line");
        assert!(!mine.stdout.contains("engine:"), "{what}: the engine line reached standard output");
        same += 1;
    }
    let describe = run(&runner, &dir, &["--describe"], &var).stdout;
    assert!(describe.contains("\"ADD-TWO\"") && describe.contains("\"GREET\""), "the interface is the Backend's: {describe}");
    let check = run(&runner, &dir, &["--check", "--config", &cfg_s, "--env-file", &env_s], &var);
    assert!(check.stdout.starts_with("configuration ok: contract 1.0.0 on 127.0.0.1:") && check.stdout.contains("(2 exported program(s))"), "{}", check.stdout);
    let engine_line = check.stderr.lines().find(|l| l.starts_with("engine: ")).unwrap_or_default().to_string();
    assert!(engine_line.contains("WebAssembly module") && engine_line.contains("sha256 ") && engine_line.contains("2 exported program(s)"), "{engine_line:?}");

    // It really serves: the port answers, and a stop signal ends it cleanly.
    let log = dir.join("logs").join("backend.log");
    let mut child = Command::new(&runner)
        .args(["--config", &cfg_s, "--env-file", &env_s, "--log-file", log.to_str().unwrap()])
        .env_remove("NOTIFY_SOCKET")
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let t0 = Instant::now();
    let mut up = false;
    while t0.elapsed() < Duration::from_secs(60) {
        if std::net::TcpStream::connect_timeout(&format!("127.0.0.1:{port}").parse().unwrap(), Duration::from_millis(200)).is_ok() {
            up = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let up_ms = t0.elapsed().as_millis();
    assert!(up, "the runner's port never answered");
    assert!(Command::new("kill").args(["-TERM", &child.id().to_string()]).status().unwrap().success());
    let end = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break Some(s);
        }
        if Instant::now() > end {
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let status = status.expect("the runner did not stop within 30 s of SIGTERM");
    assert_eq!(status.code(), Some(0));
    let (mut out, mut err) = (String::new(), String::new());
    child.stdout.take().unwrap().read_to_string(&mut out).unwrap();
    child.stderr.take().unwrap().read_to_string(&mut err).unwrap();
    // With a log file and no terminal the run's lines go to the file (the Cargo-built file's rule); the engine line is one of them.
    assert!(!out.contains("engine:") && !out.contains("listening"), "nothing the operator reads is on standard output: {out}");
    let logged = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(logged.contains("backend listening on 127.0.0.1:") && logged.contains("stopping"), "{logged}");
    assert!(logged.contains("engine: WebAssembly module") && logged.contains("sha256 "), "the engine line is in the log: {logged}");
    assert!(err.is_empty() || !err.contains("engine:"), "with a log file the lines are not repeated on standard error: {err}");
    println!(
        "packed runner (module {wasm_len} bytes): {same} invocations with the same standard output and exit code as the Cargo-built stand-in; served on port {port} {up_ms} ms after start and stopped on SIGTERM with 0; engine line in the log (and on standard error without one), never on standard output; whole test {} ms",
        started.elapsed().as_millis()
    );
    // Four checks per invocation (exit code, standard output, standard error, no engine line on standard output) and nine more.
    println!("PASS {:03} FAIL 000", same * 4 + 9);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_with_no_backend_a_damaged_one_and_a_module_the_runner_cannot_serve_are_refused_in_words() {
    let started = Instant::now();
    let dir = backend_dir("refused", EXPORTS, &[("add.cbl", ADD), ("greet.cbl", GREET)]);
    let var = "PRC_RUNNER_NO_SECRET";
    let mut refused = 0u32;

    // The bare runner: no payload.
    let bare = dir.join("bare");
    std::fs::copy(env!("CARGO_BIN_EXE_prc-backend-runner"), &bare).unwrap();
    let r = run(&bare, &dir, &["--describe"], var);
    assert_eq!(r.code, Some(1), "{}", r.stderr);
    assert!(r.stderr.contains("bare Backend runner"), "{}", r.stderr);
    refused += 1;

    // A Desktop program's payload: no Backend part.
    let runner = std::fs::read(env!("CARGO_BIN_EXE_prc-backend-runner")).unwrap();
    let desktop = embed(&runner, &encode(&[(Kind::Module, b"\0asm\x01\0\0\0".as_slice())]), "desktop").unwrap();
    let path = dir.join("desktop");
    cobolt_program_package::write_executable(&path, &desktop).unwrap();
    let r = run(&path, &dir, &["--describe"], var);
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("Desktop program, not a Backend"), "{}", r.stderr);
    refused += 1;

    // A container whose module is not one.
    let mut c = container(&dir, b"this is not a module".to_vec());
    let path = packed(&dir, "damaged", &c);
    let r = run(&path, &dir, &["--describe"], var);
    assert_eq!(r.code, Some(1));
    assert!(!r.stderr.trim().is_empty() && r.stdout.is_empty(), "the refusal is said on standard error: {}", r.stderr);
    refused += 1;

    // A configuration that asks for the state to be kept over a module built to start every call afresh: refused, in words.
    c = container(&dir, module());
    c.state = "per-connection".into();
    let path = packed(&dir, "perconn", &c);
    let r = run(&path, &dir, &["--describe"], var);
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("per-connection") && r.stderr.contains("per-call") && r.stderr.contains("build it again"), "{}", r.stderr);
    refused += 1;

    println!("{refused} files refused with exit 1 and a sentence on standard error: no payload, a Desktop payload, a module that is not one, a configuration whose state the module was not built for ({} ms)", started.elapsed().as_millis());
    println!("PASS {refused:03} FAIL 000");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_module_that_needs_a_capability_the_runner_does_not_provide_is_refused_naming_it() {
    use cobolt_backend_runner::payload::{load, Package};
    let started = Instant::now();
    let dir = backend_dir("caps", EXPORTS, &[("add.cbl", ADD), ("greet.cbl", GREET)]);
    // A Backend whose programs reach a database declares `sql`: a runner without it names it and starts nothing.
    let db = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. ADD-TWO.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N PIC 9(4).
LINKAGE SECTION.
01 REQ.
    05 A PIC 9(4).
01 RES.
    05 TOTAL PIC 9(5).
PROCEDURE DIVISION USING REQ RETURNING RES.
MAIN-PARA.
    MOVE COBOL::SqlDatabase::Open(":memory:") TO WS-N
    COBOL::SqlDatabase::Close()
    GOBACK.
"#;
    let wasm = {
        let p = parse(db);
        compile_backend(&[BackendProgram { name: "ADD-TWO".into(), program: &p }], &Options { native: true, ..Options::default() }).unwrap().wasm
    };
    let all = cobolt_backend_runner::provided();
    let without: Vec<&str> = all.iter().copied().filter(|c| *c != "sql").collect();
    // The two lists are explicit, so the test holds for every variant the runner is built as.
    let with: Vec<&str> = without.iter().copied().chain(["sql"]).collect();
    let source = Backend::load(&dir).unwrap();
    let make = |wasm: &Vec<u8>| {
        // The table of the one program the module holds.
        let mut c = container(&dir, wasm.clone());
        let entries: Vec<_> = source.export_entries().into_iter().filter(|e| e.name == "ADD-TWO").collect();
        c.exports = serde_json::to_vec(&entries).unwrap();
        let mut iface = source.interface.clone();
        iface.programs.retain(|p| p.name == "ADD-TWO");
        c.interface = iface.to_json().into_bytes();
        Package { container: c, code: None }
    };
    let refused = load(make(&wasm), &without).err().expect("a runner without `sql` must refuse a module that declares it");
    let said = refused.join("\n");
    assert!(said.contains("sql"), "the capability is named: {said}");
    let started_ok = load(make(&wasm), &with).unwrap_or_else(|e| panic!("a runner with `sql` serves it: {e:?}"));
    assert!(started_ok.notice.as_deref().is_some_and(|n| n.starts_with("engine: WebAssembly module")));
    println!(
        "a Backend declaring `sql` refused by a runner without it, naming it ({said:?}), and started by one with it ({} ms)",
        started.elapsed().as_millis()
    );
    println!("PASS 002 FAIL 000");
    let _ = std::fs::remove_dir_all(&dir);
}
