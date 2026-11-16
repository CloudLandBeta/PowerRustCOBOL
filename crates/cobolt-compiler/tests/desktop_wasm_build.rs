// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T16: the Build classifies a project and takes the WebAssembly path, with no
//! Rust toolchain on the machine (AC1, local half), or the Cargo path with a named reason.
//!
//! One test, because it takes `cargo` and `rustc` away from the whole process: `PATH` becomes an
//! empty folder, and `CARGO_HOME` and `RUSTUP_HOME` point at empty folders. The runner is built
//! first, while `cargo` is still there (`cargo build -p cobolt-runner --bin prc-runner`), and each
//! Build is handed it, so the test does not depend on where the test executable lives.
//!
//! Cases:
//! - a console project (main program and Common Code) builds on the WebAssembly path into the
//!   chosen build folder, and `bin/<name>` runs (AC1, the chosen-folder twin);
//! - `bin/` holds one executable plus the assets and licences, and `dist/` is copied (AC13);
//! - no PROCEDURE DIVISION statement line of the project is in the built file (AC15);
//! - an `EXEC RUST` block takes the Cargo path before anything is compiled, naming its file and
//!   line (AC12);
//! - a construct the generator refuses takes the Cargo path, naming its file and line: one in
//!   Common Code, one built-in in the main program (AC11);
//! - with no toolchain, the Cargo path stops and `bin/` is left as it was (AC17);
//! - a Debug Build gives `DebugBuild`, the project's choice `Chosen`, no runner
//!   `RunnerUnavailable`;
//! - the forms' current code is compiled, not a stale generated file: the front end runs before
//!   the toolchain is looked for (the twin of `regenerate_before_build_tests`).

use cobolt_compiler::{build_project, BuildOptions, BuildPath, BuildPhase, CargoReason, CompilerError};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

const MAIN: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T16MAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A       PIC 9(4) VALUE 12.
       01 WS-B       PIC 9(4) VALUE 34.
       01 WS-TOTAL   PIC 9(6) VALUE 0.
       01 WS-COUNT   PIC 9(4) VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           COMPUTE WS-TOTAL = WS-A * WS-B + 7
           PERFORM ADD-ONE 3 TIMES
           CALL \"T16COMMON\" USING WS-TOTAL
           DISPLAY \"TOTAL \" WS-TOTAL \" COUNT \" WS-COUNT
           STOP RUN.
       ADD-ONE.
           ADD 1 TO WS-COUNT.
";

const COMMON: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T16COMMON.
       DATA DIVISION.
       LINKAGE SECTION.
       01 LK-TOTAL   PIC 9(6).
       PROCEDURE DIVISION USING LK-TOTAL.
           ADD 1000 TO LK-TOTAL
           GOBACK.
";

/// What `MAIN` prints: 12 × 34 + 7 = 415, plus 1000 in Common Code; ADD-ONE three times.
const EXPECTED: &str = "TOTAL 001415 COUNT 0003";

const EXEC_RUST: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T16RUST.
       PROCEDURE DIVISION.
           DISPLAY \"before\"
           EXEC RUST
               println!(\"from rust\");
           END-EXEC
           STOP RUN.
";

const CALLS_FILES: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T16CALLER.
       PROCEDURE DIVISION.
           CALL \"T16FILES\"
           STOP RUN.
";

const FILES: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T16FILES.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT LOG-FILE ASSIGN TO \"t16.log\"
               ORGANIZATION IS LINE SEQUENTIAL.
       DATA DIVISION.
       FILE SECTION.
       FD LOG-FILE.
       01 LOG-REC    PIC X(40).
       PROCEDURE DIVISION.
           OPEN OUTPUT LOG-FILE
           CLOSE LOG-FILE
           GOBACK.
";

const HTTP: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T16HTTP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-URL     PIC X(60) VALUE \"https://example.invalid/\".
       01 WS-BODY    PIC X(200).
       01 WS-STATUS  PIC 9(3).
       PROCEDURE DIVISION.
           COBOL::\"HTTP-GET\" ( WS-URL WS-BODY WS-STATUS )
           STOP RUN.
";

/// The 1-based line of `src` that contains `needle`.
fn line_of(src: &str, needle: &str) -> u32 {
    src.lines().position(|l| l.contains(needle)).map(|i| i as u32 + 1).expect("the line is in the source")
}

/// A project folder: `files` are (relative path, text); `manifest` is the `.project.toml` body.
fn project(root: &Path, name: &str, manifest: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = root.join(name);
    let _ = fs::remove_dir_all(&dir);
    for (rel, text) in files {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    let m = dir.join(format!("{name}.project.toml"));
    fs::write(&m, manifest).unwrap();
    m
}

fn console_manifest(name: &str, main: &str, sources: &[&str], extra: &str) -> String {
    let list = sources.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", ");
    format!("[project]\nname = \"{name}\"\nversion = \"0.1.0\"\nmain = \"{main}\"\n\n[files]\nsources = [{list}]\n{extra}")
}

/// Every file under `dir`, relative, sorted.
fn tree(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, d: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(d) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push(p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// The sha256 of every path and content under `dir`, in order.
fn tree_hash(dir: &Path) -> String {
    let mut h = Sha256::new();
    for rel in tree(dir) {
        h.update(rel.as_bytes());
        h.update([0]);
        h.update(fs::read(dir.join(&rel)).unwrap());
        h.update([0]);
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(p).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe"))
    }
}

/// The PROCEDURE DIVISION statement lines of `src`: trimmed, with the paragraph names and the
/// division header left out.
fn statement_lines(src: &str) -> Vec<String> {
    let mut inside = false;
    let mut out = Vec::new();
    for l in src.lines() {
        let t = l.trim();
        if t.starts_with("PROCEDURE DIVISION") {
            inside = true;
            continue;
        }
        let paragraph = t.ends_with('.') && !t.contains(' ');
        if inside && !t.is_empty() && !t.starts_with('*') && !paragraph {
            out.push(t.to_string());
        }
    }
    out
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// The reason inside a `CargoNeedsToolchain`, or a description of what came instead.
fn reason_of(r: &Result<cobolt_compiler::BuildResult, CompilerError>) -> Result<CargoReason, String> {
    match r {
        Err(CompilerError::CargoNeedsToolchain { reason, .. }) => Ok(reason.clone()),
        Err(e) => Err(format!("another error: {e}")),
        Ok(b) => Err(format!("it built, on the {} path", b.path.name())),
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

#[test]
fn the_build_classifies_and_takes_the_webassembly_path() {
    let root = std::env::temp_dir().join(format!("prc-t16-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    // ── The runner, built while cargo is still on PATH ──
    let t = Instant::now();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new(&cargo)
        .current_dir(&workspace)
        .args(["build", "-q", "-p", "cobolt-runner", "--bin", "prc-runner"])
        .status()
        .expect("run cargo");
    assert!(status.success(), "cargo build -p cobolt-runner --bin prc-runner failed");
    let exe = std::env::current_exe().unwrap();
    let runner = exe.parent().and_then(Path::parent).unwrap().join(cobolt_compiler::RUNNER_NAME);
    assert!(runner.is_file(), "the runner is at {}", runner.display());
    let runner_ms = ms(t);
    let runner_bytes = fs::metadata(&runner).unwrap().len();

    // ── No Rust toolchain from here on (AC1) ──
    let empty_path = root.join("empty-path");
    let cargo_home = root.join("cargo-home");
    let rustup_home = root.join("rustup-home");
    for d in [&empty_path, &cargo_home, &rustup_home] {
        fs::create_dir_all(d).unwrap();
    }
    std::env::set_var("PATH", &empty_path);
    std::env::set_var("CARGO_HOME", &cargo_home);
    std::env::set_var("RUSTUP_HOME", &rustup_home);
    std::env::remove_var(cobolt_compiler::RUNNER_ENV);
    let no_rustc = Command::new("rustc").arg("-V").output().is_err() && Command::new("cargo").arg("-V").output().is_err();
    let cache = root.join("runner-cache");
    let chosen = root.join("chosen-builds");
    let opts = |runner: Option<&Path>| BuildOptions {
        verbose: false,
        build_dir: Some(chosen.clone()),
        runner: runner.map(Path::to_path_buf),
        ..Default::default()
    };

    let mut checks: Vec<(String, bool)> = Vec::new();
    let mut check = |what: &str, ok: bool| checks.push((what.to_string(), ok));
    check("cargo and rustc cannot be started", no_rustc);

    // ── 1. A console project on the WebAssembly path (AC1, AC13, AC15, the chosen-folder twin) ──
    let name = "prct16console";
    let manifest = project(
        &root,
        name,
        &console_manifest(name, "src/main.cbl", &["src/main.cbl", "src/common.cbl"], "assets = [\"assets/logo.txt\"]\n"),
        &[("src/main.cbl", MAIN), ("src/common.cbl", COMMON), ("assets/logo.txt", "logo"), ("data/seed.dat", "seed")],
    );
    let dir = manifest.parent().unwrap().to_path_buf();
    let (default_root, _) = cobolt_compiler::resolve_build_root(None);
    let default_staging = default_root.join(format!("cobolt-build-{name}"));
    let _ = fs::remove_dir_all(&default_staging);
    let t = Instant::now();
    let built = build_project(&manifest, &opts(Some(&runner)));
    let wasm_ms = ms(t);
    let built = match built {
        Ok(b) => b,
        Err(e) => panic!("the console project should build on the WebAssembly path: {e}"),
    };
    let bin = dir.join("bin").join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    check("path is WebAssembly, no Cargo build", built.path == BuildPath::Wasm && built.cargo.is_none());
    let phases: Vec<BuildPhase> = built.phases.iter().map(|(p, _)| *p).collect();
    check(
        "phases: regenerate, front end, classify, generate, machine code, assemble, install",
        phases
            == [
                BuildPhase::Regenerate,
                BuildPhase::FrontEnd,
                BuildPhase::Classify,
                BuildPhase::Generate,
                BuildPhase::MachineCode,
                BuildPhase::Assemble,
                BuildPhase::Install,
            ],
    );
    // The Build names the project folder canonically (`/private/var/…` for `/var/…` on macOS).
    check("the program is bin/<name>", bin.is_file() && built.binary_path.canonicalize().ok() == bin.canonicalize().ok());

    let run = |p: &Path| Command::new(p).env("PRC_RUNNER_CACHE", &cache).env("LC_ALL", "en_US.UTF-8").output().expect("start the built program");
    let t = Instant::now();
    let out = run(&bin);
    let run_ms = ms(t);
    let stdout = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
    check("bin/<name> runs and exits 0", out.status.success());
    check(&format!("it prints {EXPECTED:?}"), stdout.lines().any(|l| l.trim_end() == EXPECTED));

    let exe_bytes = fs::read(&bin).unwrap();
    let payload = cobolt_program_package::locate(&exe_bytes).ok().and_then(|p| cobolt_program_package::decode(p).ok().map(|d| (d.part(cobolt_program_package::Kind::Module).is_some(), d.part(cobolt_program_package::Kind::MachineCode).is_some())));
    check("the file carries the module and its machine code", payload == Some((true, true)));

    // The chosen build folder (twin of `a_build_lands_in_the_chosen_folder_and_leaves_the_default_alone`).
    let staged = chosen.join(format!("cobolt-build-{name}")).join("wasm").join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    check("staged in the chosen build folder", built.build_dir == chosen.join(format!("cobolt-build-{name}")) && staged.is_file());
    check("nothing under the default build folder", !default_staging.exists());

    // AC13: bin/ holds one executable plus the assets and licences; dist/ is copied.
    let bin_tree = tree(&dir.join("bin"));
    let exe_name = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    let mut want_bin = vec![exe_name.clone(), "LICENSE".into(), "NOTICE".into(), "POWERRUSTCOBOL-NOTICE.txt".into(), "assets/logo.txt".into()];
    want_bin.sort();
    let executables = bin_tree.iter().filter(|rel| is_executable(&dir.join("bin").join(rel))).count();
    check("bin/ holds the program, assets/logo.txt and the three notices", bin_tree == want_bin);
    check("bin/ holds exactly one executable", executables == 1);
    let dist_tree = tree(&dir.join("dist"));
    let mut want_dist = vec![exe_name.clone(), "LICENSE".into(), "NOTICE".into(), "POWERRUSTCOBOL-NOTICE.txt".into(), "assets/logo.txt".into(), "data/seed.dat".into()];
    want_dist.sort();
    check("dist/ holds the program, assets/, data/ and the notices", dist_tree == want_dist);
    check("dist/<name> is the same file as bin/<name>", fs::read(dir.join("dist").join(&exe_name)).ok().as_deref() == Some(exe_bytes.as_slice()));
    let dist_out = run(&dir.join("dist").join(&exe_name));
    check("dist/<name> runs too", dist_out.status.success() && String::from_utf8_lossy(&dist_out.stdout).contains(EXPECTED));

    // AC15: no PROCEDURE DIVISION statement line of the project is in the built file. A line the
    // bare runner already holds (`STOP RUN.` is text in the runtime and form crates it links since
    // T30) is the runner's own, not the program's: it is named, and left out of the count.
    let lines: Vec<String> = statement_lines(MAIN).into_iter().chain(statement_lines(COMMON)).collect();
    let bare = fs::read(&runner).unwrap_or_default();
    let in_runner: Vec<&String> = lines.iter().filter(|l| contains(&bare, l.as_bytes())).collect();
    let found: Vec<&String> = lines.iter().filter(|l| !in_runner.contains(l) && contains(&exe_bytes, l.as_bytes())).collect();
    check(
        &format!("none of the {} statement lines the bare runner lacks is in the built file", lines.len() - in_runner.len()),
        lines.len() > in_runner.len() && found.is_empty(),
    );

    // ── 2. EXEC RUST takes the Cargo path before anything is compiled (AC12) ──
    let rust_line = line_of(EXEC_RUST, "EXEC RUST");
    let m = project(&root, "prct16rust", &console_manifest("prct16rust", "src/main.cbl", &["src/main.cbl"], ""), &[("src/main.cbl", EXEC_RUST)]);
    let t = Instant::now();
    let r = build_project(&m, &opts(Some(&runner)));
    let rust_ms = ms(t);
    let text = r.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
    check(
        &format!("EXEC RUST → ExecRust {{ src/main.cbl, line {rust_line} }}"),
        reason_of(&r) == Ok(CargoReason::ExecRust { file: "src/main.cbl".into(), line: rust_line }),
    );
    check("the message names the file and line, and Rust", text.contains(&format!("src/main.cbl line {rust_line}")) && text.contains("Rust toolchain is required"));
    check("nothing compiled: no bin/, no staged Cargo project", !m.parent().unwrap().join("bin").exists() && !chosen.join("cobolt-build-prct16rust").exists());

    // ── 3. A refused construct takes the Cargo path, naming its file and line (AC11) ──
    let open_line = line_of(FILES, "OPEN OUTPUT");
    let m = project(
        &root,
        "prct16files",
        &console_manifest("prct16files", "src/main.cbl", &["src/main.cbl", "src/files.cbl"], ""),
        &[("src/main.cbl", CALLS_FILES), ("src/files.cbl", FILES)],
    );
    let t = Instant::now();
    let r = build_project(&m, &opts(Some(&runner)));
    let files_ms = ms(t);
    let files_reason = reason_of(&r);
    let text = r.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
    check(
        &format!("a file on disk in Common Code → Refused at src/files.cbl line {open_line}"),
        matches!(&files_reason, Ok(CargoReason::Refused { construct, file, line, reason }) if construct == "file LOG-FILE" && file == "src/files.cbl" && *line == open_line && reason.starts_with("files are not on the WebAssembly path")),
    );
    check("the message names the file, the line and why", text.contains(&format!("src/files.cbl line {open_line}")) && text.contains("WebAssembly path"));
    let http_line = line_of(HTTP, "HTTP-GET");
    let m = project(&root, "prct16http", &console_manifest("prct16http", "src/main.cbl", &["src/main.cbl"], ""), &[("src/main.cbl", HTTP)]);
    let t = Instant::now();
    let r = build_project(&m, &opts(Some(&runner)));
    let http_ms = ms(t);
    let http_reason = reason_of(&r);
    check(
        &format!("COBOL::\"HTTP-GET\" in the main program → Refused at src/main.cbl line {http_line}"),
        matches!(&http_reason, Ok(CargoReason::Refused { construct, file, line, reason }) if construct == "COBOL::\"HTTP-GET\"" && file == "src/main.cbl" && *line == http_line && reason.contains("HTTP")),
    );

    // ── 4. No toolchain: the Cargo path stops and bin/ is untouched (AC17); the chosen setting ──
    let m = project(
        &root,
        "prct16chosen",
        &console_manifest("prct16chosen", "src/main.cbl", &["src/main.cbl", "src/common.cbl"], "\n[desktop]\nbuild_path = \"cargo\"\n"),
        &[("src/main.cbl", MAIN), ("src/common.cbl", COMMON), ("bin/prct16chosen", "an earlier build"), ("bin/assets/old.txt", "old")],
    );
    let bin_dir = m.parent().unwrap().join("bin");
    let before = tree_hash(&bin_dir);
    let t = Instant::now();
    let r = build_project(&m, &opts(Some(&runner)));
    let chosen_ms = ms(t);
    check("[desktop] build_path = \"cargo\" → Chosen", reason_of(&r) == Ok(CargoReason::Chosen));
    check("bin/'s tree hash is the same before and after", tree_hash(&bin_dir) == before);

    // ── 5. Debug → DebugBuild; no runner → RunnerUnavailable ──
    let debug = build_project(&manifest, &BuildOptions { debug: true, ..opts(Some(&runner)) });
    check("a Debug Build → DebugBuild", reason_of(&debug) == Ok(CargoReason::DebugBuild));
    let none = build_project(&manifest, &opts(None));
    check("no runner beside this program, none in PRC_RUNNER → RunnerUnavailable", reason_of(&none) == Ok(CargoReason::RunnerUnavailable));
    let missing = build_project(&manifest, &opts(Some(&root.join("no-such-runner"))));
    check("a runner path that is not a file → RunnerUnavailable", reason_of(&missing) == Ok(CargoReason::RunnerUnavailable));

    // ── 6. The forms' current code, not a stale generated file, before the toolchain is looked for ──
    let fdir = root.join("prct16f9");
    fs::create_dir_all(fdir.join("forms")).unwrap();
    fs::create_dir_all(fdir.join("generated")).unwrap();
    let mut form = cobolt_forms::Form::new("MAIN", "Main", 400, 300);
    form.main_form = true;
    let mut btn = cobolt_forms::Control::new("BTN-GO", cobolt_forms::ControlType::Button, 10, 10);
    let mut on_click = cobolt_forms::EventBinding::for_control("BTN-GO", "onClick");
    on_click.code = "       PROCEDURE DIVISION.\n           COPY \"FRESH-T16-BOOK\".\n".into();
    btn.events.push(on_click);
    form.controls.push(btn);
    cobolt_forms::save_form(&form, &fdir.join("forms/main.cfrm")).unwrap();
    fs::write(fdir.join("generated/main.cbl"), "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           COPY \"STALE-T16-BOOK\".\n").unwrap();
    let fm = fdir.join("prct16f9.project.toml");
    fs::write(&fm, "[project]\nname = \"prct16f9\"\nversion = \"0.1.0\"\nmain = \"\"\n\n[files]\nforms = [\"forms/main.cfrm\"]\ngenerated = [\"generated/main.cbl\"]\n").unwrap();
    let f9 = |regenerate_forms: bool| build_project(&fm, &BuildOptions { regenerate_forms, ..opts(Some(&runner)) }).err().map(|e| e.to_string()).unwrap_or_default();
    let kept = f9(false);
    let fresh = f9(true);
    check("regeneration off compiles the file as it is (STALE-T16-BOOK)", kept.contains("STALE-T16-BOOK"));
    check("the regenerated program is compiled (FRESH-T16-BOOK), before any toolchain error", fresh.contains("FRESH-T16-BOOK") && !fresh.contains("toolchain"));

    // ── The result ──
    println!("== desktop_wasm_build (DESKTOP-WASM-001 T16: AC1 local half, AC11, AC12, AC13, AC15, AC17) ==");
    println!("environment: PATH = an empty folder; CARGO_HOME and RUSTUP_HOME = empty folders");
    println!("runner: {} ({:.1} MB), built first in {runner_ms:.0} ms", runner.display(), runner_bytes as f64 / 1_048_576.0);
    println!("1. console project, main + Common Code, WebAssembly path: {wasm_ms:.1} ms, {} bytes", exe_bytes.len());
    for (phase, took) in &built.phases {
        println!("     {:<13} {:>9.1} ms", phase.name(), took.as_secs_f64() * 1000.0);
    }
    println!("   bin/<name> ran in {run_ms:.1} ms and printed {:?}", stdout.trim_end());
    println!(
        "   statement lines searched in the built file: {}, in the bare runner itself: {} {:?}, found: {}",
        lines.len(),
        in_runner.len(),
        in_runner,
        found.len()
    );
    println!("2. EXEC RUST at src/main.cbl line {rust_line}: Cargo path named in {rust_ms:.1} ms");
    println!("3. refused: {files_reason:?} in {files_ms:.1} ms");
    println!("   refused: {http_reason:?} in {http_ms:.1} ms");
    println!("4. build_path = \"cargo\": stopped in {chosen_ms:.1} ms, bin/ unchanged");
    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("result: {passed} of {} checks passed", checks.len());

    if std::env::var_os("PRC_T16_KEEP").is_none() {
        let _ = fs::remove_dir_all(&root);
    }
    let failed: Vec<&String> = checks.iter().filter(|(_, ok)| !*ok).map(|(w, _)| w).collect();
    assert!(failed.is_empty(), "failed: {failed:?}; stdout {stdout:?}; stderr {:?}", String::from_utf8_lossy(&out.stderr));
}
