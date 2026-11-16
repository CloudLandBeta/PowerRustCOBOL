// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! An `rcrun run` look-alike that runs a program the way a Desktop Build on the WebAssembly path
//! ships it (DESKTOP-WASM-001 T23; R11, R21, R22; AC9): the generator compiles it in the native
//! mode, the module is packaged into a copy of `prc-runner`, and that executable runs it. Pointed
//! at by the NIST harness:
//!
//! ```text
//! cargo build -p cobolt-runner --bin prc-runner --example nist_runner_run
//! RCRUN=target/debug/examples/nist_runner_run target/debug/examples/nist_conformance run NC
//! ```
//!
//! It takes what the harness passes `rcrun`: `run <file.cbl> [--source-format fixed|fixed-relaxed]
//! [--switch NAME=ON|OFF]…`, with the working directory as the program's file system and standard
//! input and output passed through. A program the native mode refuses exits 1, and its reason goes
//! to the file `NIST_RUNNER_LOG` names, one line each (`<file>: not compiled: <reason>`), which is
//! how the compile axis is counted. The runner is `PRC_RUNNER`, or `prc-runner` beside this
//! example's folder (`target/<profile>/`). Machine code is cached under `PRC_RUNNER_CACHE`, by
//! default a folder of the system's temporary folder shared by every run.
//!
//! Switches: the native mode has no external-switch capability at M1, so `--switch` is accepted
//! and ignored; a program that tests a switch sees it OFF (recorded in `WASM-PARITY.md`).
//!
//! The second mode compares the runner with the interpreter over a folder of programs (AC9's
//! `tests/cobol` half):
//!
//! ```text
//! target/debug/examples/nist_runner_run compare --rcrun target/debug/rcrun [tests/cobol]
//! ```
//!
//! Every `.cbl` under the folder is compiled in the native mode. Each one the generator accepts
//! runs twice, under `rcrun run` (the interpreter) and packaged into `prc-runner`, each in its own
//! copy of the program's folder, with no standard input, the clock and `RANDOM` pinned
//! (`PRC_TEST_CLOCK`, `PRC_TEST_SEED`) and 20 s each. Standard output (line ends as LF) and the exit
//! code must be the same. One result block: the programs, those refused with their reasons, and the
//! identical-output count, with the first differing line of each that differs.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn log(line: &str) {
    if let Some(p) = std::env::var_os("NIST_RUNNER_LOG") {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

fn runner() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("PRC_RUNNER") {
        let p = PathBuf::from(p);
        return p.is_file().then_some(p);
    }
    // target/<profile>/examples/nist_runner_run → target/<profile>/prc-runner
    let exe = std::env::current_exe().ok()?;
    let p = exe
        .parent()?
        .parent()?
        .join(format!("prc-runner{}", std::env::consts::EXE_SUFFIX));
    p.is_file().then_some(p)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("compare") {
        if let Err(e) = compare(&args[1..]) {
            eprintln!("nist_runner_run compare: {e}");
            std::process::exit(1);
        }
        return;
    }
    if args.first().map(String::as_str) != Some("run") || args.len() < 2 {
        eprintln!("usage: nist_runner_run run <file.cbl> [--source-format fixed|fixed-relaxed] [--switch NAME=ON]");
        std::process::exit(2);
    }
    let path = PathBuf::from(&args[1]);
    let format = match args
        .iter()
        .position(|a| a == "--source-format")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
    {
        Some("fixed") => cobolt_lexer::SourceFormat::FixedStrict,
        Some("fixed-relaxed") => cobolt_lexer::SourceFormat::Fixed,
        // `rcrun run` and the Desktop Build read free form unless told otherwise.
        _ => cobolt_lexer::SourceFormat::Free,
    };
    let module = match compile(&path, format) {
        Ok(m) => m,
        Err(reason) => {
            eprintln!("{}: not compiled: {reason}", path.display());
            log(&format!("{}: not compiled: {reason}", path.display()));
            std::process::exit(1);
        }
    };
    let Some(runner) = runner() else {
        eprintln!("prc-runner not found: build it (cargo build -p cobolt-runner --bin prc-runner) or set PRC_RUNNER");
        std::process::exit(2);
    };
    let code = match run(&runner, &module, &path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            log(&format!("{}: run failed: {e}", path.display()));
            1
        }
    };
    std::process::exit(code);
}

/// The module the Desktop Build would put in the executable, or the generator's refusal.
fn compile(path: &Path, format: cobolt_lexer::SourceFormat) -> Result<Vec<u8>, String> {
    let text = cobolt_lexer::read_source_file(path).map_err(|e| e.to_string())?;
    let expanded = cobolt_lexer::expand_copybooks_for(&text, path, format);
    if let Some(e) = expanded.errors.first() {
        return Err(format!("copybook: {e}"));
    }
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize_expansion(&expanded));
    if let Some(d) = parsed.diagnostics.iter().find(|d| d.is_error()) {
        return Err(format!(
            "it does not parse: line {}: {}",
            d.span.line, d.message
        ));
    }
    let program = parsed.program.ok_or("it does not parse")?;
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("program")
        .to_owned();
    let tolerate = std::env::var_os("COBOL_TOLERATE_UNDECLARED").is_some();
    let opts = cobolt_wasm::Options {
        native: true,
        tolerate_undeclared: tolerate,
        name,
        ..cobolt_wasm::Options::default()
    };
    let compiled =
        cobolt_wasm::compile_program(&program, &opts).map_err(|e| match e.all().first() {
            Some((line, m)) => format!("line {line}: {m}"),
            None => e.to_string(),
        })?;
    Ok(compiled.wasm)
}

/// The module packaged alone into a copy of the runner, written into `dir`.
fn package(runner: &Path, module: &[u8], source: &Path, dir: &Path) -> Result<PathBuf, String> {
    use cobolt_program_package::Kind;
    let bytes = std::fs::read(runner).map_err(|e| format!("{}: {e}", runner.display()))?;
    let payload = cobolt_program_package::encode(&[(Kind::Module, module)]);
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("program");
    let exe = cobolt_program_package::embed(&bytes, &payload, stem).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let program = dir.join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
    cobolt_program_package::write_executable(&program, &exe).map_err(|e| e.to_string())?;
    Ok(program)
}

fn cache_dir() -> PathBuf {
    std::env::var_os("PRC_RUNNER_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("prc-nist-runner-cache"))
}

/// The module packaged alone into a copy of the runner, run in the current directory.
fn run(runner: &Path, module: &[u8], source: &Path) -> Result<i32, String> {
    let scratch = std::env::temp_dir().join(format!("prc-nist-runner-{}", std::process::id()));
    let program = package(runner, module, source, &scratch)?;
    let status = Command::new(&program)
        .env("PRC_RUNNER_CACHE", cache_dir())
        .status();
    let _ = std::fs::remove_dir_all(&scratch);
    let status = status.map_err(|e| format!("{}: {e}", program.display()))?;
    Ok(status.code().unwrap_or(1))
}

/// The pins both engines run under in `compare`.
const CLOCK: &str = "2026-01-02T03:04:05.67-03:00";
const SEED: &str = "20260102";
const LIMIT: Duration = Duration::from_secs(20);

/// What one engine did with a program.
struct Ran {
    stdout: String,
    code: Option<i32>,
    ms: f64,
    timed_out: bool,
}

fn compare(args: &[String]) -> Result<(), String> {
    let t = Instant::now();
    let mut rcrun: Option<PathBuf> = None;
    let mut folder: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--rcrun" => rcrun = it.next().map(PathBuf::from),
            _ => folder = Some(PathBuf::from(a)),
        }
    }
    let rcrun = rcrun
        .ok_or("--rcrun <the interpreter's rcrun> is required")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let folder = folder
        .unwrap_or_else(|| workspace.join("tests/cobol"))
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let runner = runner().ok_or("prc-runner not found: build it or set PRC_RUNNER")?;
    let scratch = std::env::temp_dir().join(format!("prc-nist-compare-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let mut programs = Vec::new();
    collect_cbl(&folder, &mut programs);
    programs.sort();

    let (mut refused, mut identical, mut ran) = (Vec::new(), 0usize, 0usize);
    let mut rows = Vec::new();
    let mut differ = Vec::new();
    let (mut ms_interp, mut ms_runner) = (0.0, 0.0);
    for (i, p) in programs.iter().enumerate() {
        let rel = p
            .strip_prefix(&folder)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/");
        // Free form, as `rcrun run` and the Desktop Build read a source.
        let module = match compile(p, cobolt_lexer::SourceFormat::Free) {
            Ok(m) => m,
            Err(reason) => {
                refused.push((rel, reason));
                continue;
            }
        };
        ran += 1;
        let dir = p.parent().unwrap_or(Path::new("."));
        let name = p.file_name().unwrap_or_default();
        let (a, b) = (
            scratch.join(format!("{i}-interpreter")),
            scratch.join(format!("{i}-runner")),
        );
        copy_tree(dir, &a)?;
        copy_tree(dir, &b)?;
        let interp = spawn(
            Command::new(&rcrun)
                .arg("run")
                .arg(a.join(name))
                .current_dir(&a),
        )?;
        let exe = package(&runner, &module, p, &scratch.join(format!("{i}-exe")))?;
        let native = spawn(
            Command::new(&exe)
                .env("PRC_RUNNER_CACHE", cache_dir())
                .current_dir(&b),
        )?;
        ms_interp += interp.ms;
        ms_runner += native.ms;
        let same = interp.stdout == native.stdout
            && interp.code == native.code
            && !interp.timed_out
            && !native.timed_out;
        if same {
            identical += 1;
        } else {
            let (il, nl): (Vec<&str>, Vec<&str>) = (
                interp.stdout.lines().collect(),
                native.stdout.lines().collect(),
            );
            let at = (0..il.len().max(nl.len())).find(|&k| il.get(k) != nl.get(k));
            differ.push(match at {
                Some(k) => format!(
                    "{rel}: line {}: interpreter {:?}, runner {:?}; exit {:?} / {:?}{}",
                    k + 1,
                    il.get(k).copied().unwrap_or("<none>"),
                    nl.get(k).copied().unwrap_or("<none>"),
                    interp.code,
                    native.code,
                    if interp.timed_out || native.timed_out {
                        " (timed out)"
                    } else {
                        ""
                    }
                ),
                None => format!(
                    "{rel}: same output; exit {:?} / {:?}",
                    interp.code, native.code
                ),
            });
        }
        rows.push(format!(
            "  {} {rel:<58} {:>3} lines, exit {:?}/{:?}, interpreter {:>6.0} ms, runner {:>6.0} ms",
            if same { "ok  " } else { "DIFF" },
            interp.stdout.lines().count(),
            interp.code,
            native.code,
            interp.ms,
            native.ms
        ));
    }
    let _ = std::fs::remove_dir_all(&scratch);

    println!(
        "== tests/cobol: the runner against the interpreter (DESKTOP-WASM-001 T23: AC9, R22) =="
    );
    println!(
        "folder {}; pins PRC_TEST_CLOCK={CLOCK} PRC_TEST_SEED={SEED}; no standard input; {} s each",
        folder.display(),
        LIMIT.as_secs()
    );
    println!(
        "programs {}, compiled in the native mode {ran}, refused {}",
        programs.len(),
        refused.len()
    );
    for r in &rows {
        println!("{r}");
    }
    let mut reasons: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for (rel, why) in &refused {
        reasons.entry(bucket(why)).or_default().push(rel);
    }
    println!("refused, by reason:");
    for (why, names) in &reasons {
        println!("  {:>3}  {why}", names.len());
        println!("         {}", names.join(", "));
    }
    for d in &differ {
        println!("  differs: {d}");
    }
    println!(
        "identical output and exit code: {identical} of {ran} (interpreter {ms_interp:.0} ms, runner {ms_runner:.0} ms), in {:.0} ms",
        t.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}

/// A refusal without its line number and its own names, so alike refusals count together.
fn bucket(why: &str) -> String {
    let s = why
        .split_once(": ")
        .filter(|(l, _)| l.starts_with("line "))
        .map(|(_, r)| r)
        .unwrap_or(why);
    let s = s.split(" (DESKTOP-WASM-001").next().unwrap_or(s);
    let mut words: Vec<String> = s.split(' ').map(str::to_owned).collect();
    if words.first().map(String::as_str) == Some("file") && words.len() > 1 {
        words[1] = "<name>:".into();
    }
    words.join(" ").chars().take(110).collect()
}

fn spawn(cmd: &mut Command) -> Result<Ran, String> {
    cmd.env("PRC_TEST_CLOCK", CLOCK)
        .env("PRC_TEST_SEED", SEED)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let t = Instant::now();
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let mut out = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out.read_to_end(&mut buf);
        buf
    });
    let mut timed_out = false;
    let status = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break Some(s);
        }
        if t.elapsed() > LIMIT {
            let _ = child.kill();
            let _ = child.wait();
            timed_out = true;
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let bytes = reader.join().unwrap_or_default();
    Ok(Ran {
        stdout: String::from_utf8_lossy(&bytes).replace("\r\n", "\n"),
        code: status.and_then(|s| s.code()),
        ms: t.elapsed().as_secs_f64() * 1000.0,
        timed_out,
    })
}

fn collect_cbl(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_cbl(&p, out);
        } else if p
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|x| x.eq_ignore_ascii_case("cbl"))
        {
            out.push(p);
        }
    }
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for e in std::fs::read_dir(from)
        .map_err(|e| format!("{}: {e}", from.display()))?
        .flatten()
    {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", src.display()))?;
        }
    }
    Ok(())
}
