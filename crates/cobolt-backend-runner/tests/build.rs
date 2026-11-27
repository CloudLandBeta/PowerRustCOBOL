// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T28, T29, T31 (R7, R13, R17, R22; AC1, AC2, AC7): the Backend Build on the module path.
//!
//! The four Backends the repository ships are built by `build_backend` with this crate's runner, and the file each Build
//! installs is started and asked what a Cargo-built file is asked: `--version`, `--describe`, `--check`. The Build must
//! choose the module path for each (no construct to refuse), write `bin/sbom.cdx.json` with the module's hash, and leave
//! the developer's sources as they were (R22). A Backend that holds a construct the module does not serve (`EXEC RUST`,
//! `[identity]`) goes the Cargo path with each reason named, and `--wasm` on it refuses. A Backend whose state is
//! `per-connection` is served by the module (ABI A60) and builds like any other.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use cobolt_compiler::backend_build::build_backend;
use cobolt_compiler::backend_module::{BackendCargoReason, BackendEngine, BackendPath, BACKEND_RUNNER_SBOM};

mod common;
use common::{options, project, run, runner_with_sbom, sources};

const SHIPPED: [&str; 4] = ["PowerOficina", "PowerAnalyticsWeb", "PowerWebHello", "PowerMobileHello"];

#[test]
fn the_four_shipped_backends_build_as_modules_and_answer_what_a_backend_answers() {
    let scratch = std::env::temp_dir().join(format!("prc-backend-build-runner-{}", std::process::id()));
    let runner = runner_with_sbom(&scratch);
    let mut rows = Vec::new();
    let (mut passed, mut failed) = (0u32, 0u32);
    let mut check = |ok: bool, what: String| {
        if ok {
            passed += 1;
        } else {
            failed += 1;
            eprintln!("FAIL {what}");
        }
    };
    for name in SHIPPED {
        let dir = project(name, "four");
        let before = sources(&dir);
        let began = Instant::now();
        let built = build_backend(&dir, &options(&runner, Some(BackendPath::Auto))).unwrap_or_else(|e| panic!("{name}: {e}"));
        let total = began.elapsed().as_millis();
        check(built.engine == BackendEngine::Module, format!("{name}: the engine is the module, not {:?} ({:?})", built.engine, built.reasons));
        check(built.reasons.is_empty(), format!("{name}: no reason to name: {:?}", built.reasons));
        check(built.binary.is_file() && built.binary.starts_with(dir.join("bin")), format!("{name}: bin/<name>-backend is installed"));
        check(sources(&dir) == before, format!("{name}: no source changed (R22)"));

        // The bill of materials lists the module's hash and the runner's libraries.
        let bom: serde_json::Value = serde_json::from_slice(&std::fs::read(&built.sbom).unwrap()).unwrap();
        let names: Vec<String> = bom["components"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap().to_string()).collect();
        check(built.sbom == dir.join("bin").join("sbom.cdx.json"), format!("{name}: the bill is bin/sbom.cdx.json"));
        check(names.iter().any(|n| n.ends_with(".module.wasm")) && names.iter().any(|n| n == "wasmtime"), format!("{name}: the bill lists the module and the runner's libraries: {names:?}"));
        check(bom["components"][1]["hashes"][0]["content"].as_str() == built.module_sha256.as_deref(), format!("{name}: the bill carries the module's hash"));

        // What a Cargo-built file answers.
        let version = run(&built.binary, &dir, &["--version"]);
        check(version.code == Some(0) && version.stdout.starts_with("backend (cobolt-server"), format!("{name}: --version: {:?}", version.stdout));
        let describe = run(&built.binary, &dir, &["--describe"]);
        let interface = std::fs::read_to_string(&built.interface).unwrap();
        check(describe.code == Some(0) && describe.stdout.trim() == interface.trim(), format!("{name}: --describe prints the interface the Build wrote"));
        // `--check` with the configuration the example ships, or none (a Backend with no backend.toml refuses, as a Cargo-built one does).
        let checked = run(&built.binary, &dir, &["--check"]);
        check(checked.code.is_some(), format!("{name}: --check ended"));
        rows.push(format!(
            "{name:<18} {} program(s), module {} bytes, machine code {} bytes, {total} ms ({}), exit codes version {:?} describe {:?} check {:?}",
            built.programs,
            built.module_bytes.unwrap_or(0),
            built.machine_code_bytes.unwrap_or(0),
            built.phases.iter().map(|p| format!("{} {} ms", p.name, p.millis)).collect::<Vec<_>>().join(", "),
            version.code,
            describe.code,
            checked.code,
        ));
        let _ = checked.stderr;
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
    println!("Backend Build, module path: {} Backends built, none by Cargo", SHIPPED.len());
    for r in &rows {
        println!("  {r}");
    }
    println!("PASS {passed:03} FAIL {failed:03}");
    let _ = std::fs::remove_dir_all(&scratch);
    assert_eq!(failed, 0, "{failed} checks failed");
}

/// A Backend with a verb or a setting the module does not serve takes the Cargo path, and `--wasm` refuses it naming every
/// reason. The Cargo build itself is not run here (it needs a toolchain and minutes): the path is asked with a runner that
/// is missing, so the Build reports what it would do through `--wasm`'s refusal.
#[test]
fn a_backend_the_module_does_not_serve_is_refused_by_wasm_with_each_reason() {
    let runner = runner_with_sbom(&std::env::temp_dir().join(format!("prc-backend-build-refuse-{}", std::process::id())));
    let dir = project("PowerWebHello", "refuse");
    // An identity section: a reason the module path does not serve yet (the sign-in programs).
    let toml = dir.join("backend").join("backend.toml");
    let mut config = std::fs::read_to_string(&toml).unwrap_or_default();
    config.push_str("\n[identity]\nname = \"hello\"\n");
    std::fs::write(&toml, config).unwrap();

    let err = build_backend(&dir, &options(&runner, Some(BackendPath::Wasm))).expect_err("--wasm must refuse a Backend the module does not serve");
    let message = err.to_string();
    let named = [BackendCargoReason::Identity.to_string()];
    let mut passed = 0u32;
    for n in &named {
        assert!(message.contains(n.as_str()), "the refusal names `{n}`: {message}");
        passed += 1;
    }
    assert!(!dir.join("bin").join("poweriwebhello-backend").exists());
    println!("--wasm on a Backend with an [identity] section: refused, {passed} reason named; PASS {passed:03} FAIL 000");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// `[backend] state = "per-connection"` is served by the module (ABI A60, spec 114 T37): the Build chooses the module path
/// with no reason, the module it makes says `per-connection` and has the second entry, and the file it installs starts
/// (the runner checks the module's state against the configuration's before it serves anything).
#[test]
fn a_backend_with_per_connection_state_builds_as_a_module_and_starts() {
    let runner = runner_with_sbom(&std::env::temp_dir().join(format!("prc-backend-build-perconn-{}", std::process::id())));
    let dir = project("PowerWebHello", "perconn");
    let manifest = dir.join("PowerWebHello.project.toml");
    let mut text = std::fs::read_to_string(&manifest).unwrap();
    if text.contains("[backend]") {
        text = text.replacen("[backend]", "[backend]\nstate = \"per-connection\"", 1);
    } else {
        text.push_str("\n[backend]\nstate = \"per-connection\"\n");
    }
    std::fs::write(&manifest, text).unwrap();

    let began = Instant::now();
    let built = build_backend(&dir, &options(&runner, Some(BackendPath::Auto))).unwrap_or_else(|e| panic!("the Build refuses per-connection state: {e}"));
    let ms = began.elapsed().as_millis();
    let (mut passed, mut failed) = (0u32, 0u32);
    let mut check = |ok: bool, what: String| {
        if ok {
            passed += 1;
        } else {
            failed += 1;
            eprintln!("FAIL {what}");
        }
    };
    check(built.engine == BackendEngine::Module, format!("the engine is the module, not {:?} ({:?})", built.engine, built.reasons));
    check(built.reasons.is_empty(), format!("no reason to name: {:?}", built.reasons));
    let describe = run(&built.binary, &dir, &["--describe"]);
    check(describe.code == Some(0), format!("the file starts and describes itself: {:?} {}", describe.code, describe.stderr));
    println!("a Backend with per-connection state, built on the module path in {ms} ms; PASS {passed:03} FAIL {failed:03}");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    assert_eq!(failed, 0, "{failed} checks failed");
}

/// The Build decides from `cap::BACKEND_RUNNER` whether a module fits the runner; the runner refuses at start by what it
/// provides. The two lists must be the same set, or a Backend the Build put in a runner would be refused when it starts
/// (or a Backend the runner could serve would be sent to Cargo for nothing).
#[test]
fn the_capabilities_the_build_checks_are_the_capabilities_the_runner_provides() {
    let mut provided: Vec<&str> = cobolt_backend_runner::provided();
    let mut checked: Vec<&str> = cobolt_wasm_abi::codes::cap::BACKEND_RUNNER.to_vec();
    provided.sort_unstable();
    checked.sort_unstable();
    println!("runner provides {} capabilities, the Build checks {}: {:?}", provided.len(), checked.len(), provided);
    assert_eq!(provided, checked, "cap::BACKEND_RUNNER must equal the runner's provided() list");
    println!("PASS {:03} FAIL 000", provided.len());
}

const PING_EXPORTS: &str = "[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"PING\"\npublic = true\n";

const PING: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. PING.\nDATA DIVISION.\nLINKAGE SECTION.\n01 LK-PONG.\n    05 REPLY PIC X(4).\nPROCEDURE DIVISION RETURNING LK-PONG.\nMAIN.\n    MOVE \"pong\" TO REPLY\n    GOBACK.\n";

struct Killed(std::process::Child);

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn get(port: u16, path: &str) -> Option<(u16, String)> {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).ok()?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok()?;
    write!(s, "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").ok()?;
    let mut raw = String::new();
    s.read_to_string(&mut raw).ok()?;
    let status = raw.split(' ').nth(1)?.parse().ok()?;
    Some((status, raw.split("\r\n\r\n").nth(1).unwrap_or("").to_string()))
}

/// Spec 114 T31 (R7, AC2): the file a Build makes on the module path is asked what the Cargo-built file is asked in
/// `cobolt-compiler`'s `backend_build_wasm003` (the same fixture, the same arguments, the same words) and answers the same:
/// `--version`, `--describe` equal to `interface.json`, no configuration (exit 2, "needs its configuration"), an unsafe
/// setting refused naming it, a good configuration accepted, and the health probe answered by the running file.
#[test]
fn the_file_a_build_makes_answers_what_a_cargo_built_one_answers() {
    let dir = std::env::temp_dir().join(format!("prc-backend-build-iface-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("backend").join("src")).unwrap();
    std::fs::write(dir.join("backend").join("exports.toml"), PING_EXPORTS).unwrap();
    std::fs::write(dir.join("backend").join("src").join("ping.cbl"), PING).unwrap();
    let runner = runner_with_sbom(&std::env::temp_dir().join(format!("prc-backend-build-iface-runner-{}", std::process::id())));

    let began = Instant::now();
    let built = build_backend(&dir, &options(&runner, None)).expect("the Backend builds");
    let build_ms = began.elapsed().as_millis();
    let (mut passed, mut failed) = (0u32, 0u32);
    let mut check = |ok: bool, what: String| {
        if ok {
            passed += 1;
        } else {
            failed += 1;
            eprintln!("FAIL {what}");
        }
    };
    check(built.engine == BackendEngine::Module && built.reasons.is_empty(), format!("module path, no reason: {:?} {:?}", built.engine, built.reasons));
    check(built.contract == "1.0.0" && built.programs == 1, format!("contract {} programs {}", built.contract, built.programs));

    let version = run(&built.binary, &dir, &["--version"]);
    check(version.code == Some(0) && version.stdout.contains("backend (cobolt-server"), format!("--version {:?}", version.stdout));
    let describe = run(&built.binary, &dir, &["--describe"]);
    let printed: serde_json::Value = serde_json::from_str(&describe.stdout).unwrap_or(serde_json::Value::Null);
    let written: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&built.interface).unwrap()).unwrap();
    check(describe.code == Some(0) && printed == written && printed.to_string().contains("PING"), "--describe prints what interface.json holds".into());

    let none = run(&built.binary, &dir, &["--check", "--config", dir.join("missing.toml").to_str().unwrap()]);
    check(none.code == Some(2) && none.stderr.contains("needs its configuration"), format!("no configuration: {:?} {:?}", none.code, none.stderr));

    let (health, listen) = (free_port(), free_port());
    let base = format!(
        "[server]\nlisten = \"127.0.0.1:{listen}\"\nhealth_listen = \"127.0.0.1:{health}\"\nallow_missing_origin = true\n[auth]\nissuer = [\"https://idp.example.test/pool\"]\naudiences = [\"prc-api\"]\n"
    );
    let unsafe_cfg = dir.join("unsafe.toml");
    std::fs::write(&unsafe_cfg, format!("unsafe_allow_any_origin = true\n{base}")).unwrap();
    let refused = run(&built.binary, &dir, &["--check", "--config", unsafe_cfg.to_str().unwrap()]);
    check(refused.code == Some(2) && refused.stderr.contains("unsafe_allow_any_origin"), format!("an unsafe setting is refused: {:?} {:?}", refused.code, refused.stderr));

    let cfg = dir.join("backend.toml");
    std::fs::write(&cfg, &base).unwrap();
    let ok = run(&built.binary, &dir, &["--check", "--config", cfg.to_str().unwrap()]);
    check(ok.code == Some(0) && ok.stdout.contains("configuration ok: contract 1.0.0"), format!("a good configuration is accepted: {:?} {:?}", ok.code, ok.stdout));

    let child = Command::new(&built.binary).args(["--config"]).arg(&cfg).env_remove("NOTIFY_SOCKET").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().expect("starts");
    let guard = Killed(child);
    let started = Instant::now();
    let mut answer = None;
    while started.elapsed() < std::time::Duration::from_secs(30) {
        if let Some(a) = get(health, "/health") {
            answer = Some(a);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let up_ms = started.elapsed().as_millis();
    let (status, body) = answer.unwrap_or((0, String::new()));
    let h: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    check(status == 200 && h["status"] == "ok" && h["contract"] == "1.0.0", format!("health: {status} {body}"));
    check(get(health, "/other").map(|a| a.0) == Some(404), "/other is 404".into());
    drop(guard);

    let size = std::fs::metadata(&built.binary).unwrap().len();
    println!(
        "Backend the Build made (module path), compared with the Cargo-built file's checks: built in {build_ms} ms, file {:.1} MB, answered the health probe {up_ms} ms after start",
        size as f64 / 1e6
    );
    println!("PASS {passed:03} FAIL {failed:03}");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(failed, 0, "{failed} checks failed");
}

/// Spec 114 T29 (R17, C8): a runner with no bill of materials beside it, or no runner at all, cannot take the module: `--wasm`
/// refuses and says so (the Cargo path, which `auto` would take, is not run here: it needs a toolchain and minutes).
#[test]
fn a_runner_without_its_bill_of_materials_or_not_found_is_named_by_the_refusal() {
    let dir = project("PowerWebHello", "nosbom");
    let scratch = std::env::temp_dir().join(format!("prc-backend-build-nosbom-runner-{}", std::process::id()));
    let runner = runner_with_sbom(&scratch);
    std::fs::remove_file(scratch.join(BACKEND_RUNNER_SBOM)).unwrap();
    let err = build_backend(&dir, &options(&runner, Some(BackendPath::Wasm))).expect_err("a runner with no bill of materials is refused").to_string();
    let named = BackendCargoReason::RunnerSbomMissing { sbom: scratch.join(BACKEND_RUNNER_SBOM).display().to_string() }.to_string();
    assert!(err.contains(&named), "the refusal names the missing bill of materials: {err}");
    assert!(!dir.join("bin").exists(), "nothing was installed");

    let missing = scratch.join("no-such-runner");
    let err = build_backend(&dir, &options(&missing, Some(BackendPath::Wasm))).expect_err("a missing runner is refused").to_string();
    assert!(err.contains("Backend runner") && err.contains("was not found"), "{err}");
    println!("--wasm with a runner that has no bill of materials, and with no runner: refused naming each; PASS 004 FAIL 000");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    let _ = std::fs::remove_dir_all(&scratch);
}
