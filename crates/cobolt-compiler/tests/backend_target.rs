// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 115 T6, T7 (R3, R5, R9, R10; AC5): a Backend built for another system.
//!
//! The Backend of a Web project is the application's payload joined to a runner. For another system the Build joins the same
//! payload to **that system's** runner, installs the file in a folder of its own, and makes no machine code (it is for the
//! CPU that made it). Here the runner is a stand-in: any bytes that begin with the ELF magic are a format the join accepts,
//! so the mechanics (names, folders, payload, bill of materials, refusals) are proven on any machine; that a real Linux
//! runner starts is proven by the container runs of spec 115 T18 and T19, not here.

use std::path::{Path, PathBuf};
use std::time::Instant;

use cobolt_compiler::backend_build::{build_backend, BackendBuildOptions, BackendTarget};
use cobolt_compiler::backend_module::{BackendEngine, BackendPath, RunnerFiles, BACKEND_RUNNER_SBOM};
use cobolt_compiler::backend_runners::{Arch, Os, System};
use cobolt_compiler::CompilerError;
use cobolt_program_package::Kind;

const LINUX: System = System { os: Os::Linux, arch: Arch::X86_64 };
const WINDOWS: System = System { os: Os::Windows, arch: Arch::X86_64 };

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let (path, target) = (entry.path(), to.join(entry.file_name()));
        if path.is_dir() {
            copy_tree(&path, &target);
        } else if !entry.file_name().to_string_lossy().ends_with(".lck") {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

/// A scratch copy of the smallest shipped Backend: its manifest, `backend/` and `COPYBOOKS/`.
fn project(base: &Path) -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerWebHello");
    let to = base.join("PowerWebHello");
    for sub in ["backend", "COPYBOOKS"] {
        if from.join(sub).is_dir() {
            copy_tree(&from.join(sub), &to.join(sub));
        }
    }
    std::fs::copy(from.join("PowerWebHello.project.toml"), to.join("PowerWebHello.project.toml")).unwrap();
    to
}

/// A stand-in runner and the bill of materials a release writes beside it.
fn stand_in(dir: &Path, name: &str) -> RunnerFiles {
    std::fs::create_dir_all(dir).unwrap();
    let runner = dir.join(name);
    let mut bytes = vec![0x7f, b'E', b'L', b'F'];
    bytes.extend((0..4096u32).map(|i| (i % 251) as u8));
    std::fs::write(&runner, bytes).unwrap();
    let sbom = serde_json::json!({
        "bomFormat": "CycloneDX", "specVersion": "1.5", "version": 1,
        "components": [
            cobolt_wire::sbom::file_component("prc-backend-runner", "00"),
            cobolt_wire::sbom::library_component("wasmtime", "44.0.3", "Apache-2.0"),
        ]
    });
    let sbom_path = dir.join(BACKEND_RUNNER_SBOM);
    std::fs::write(&sbom_path, serde_json::to_string_pretty(&sbom).unwrap()).unwrap();
    RunnerFiles { runner, sbom: sbom_path }
}

/// Every file under `dir` with its SHA-256, for the check that a Build left a folder as it was.
fn snapshot(dir: &Path) -> Vec<(String, String)> {
    fn walk(root: &Path, at: &Path, out: &mut Vec<(String, String)>) {
        let Ok(read) = std::fs::read_dir(at) else { return };
        for e in read.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                out.push((rel, cobolt_program_package::sha256(&std::fs::read(&p).unwrap()).iter().map(|b| format!("{b:02x}")).collect()));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn payload_part(exe: &[u8], kind: Kind) -> Option<Vec<u8>> {
    let payload = cobolt_program_package::locate(exe).expect("the file holds a payload");
    cobolt_program_package::decode(payload).expect("the payload decodes").part(kind).map(|p| p.verified().expect("the part's hash holds").to_vec())
}

#[test]
fn a_backend_is_built_for_another_system_from_the_same_payload() {
    let mut cases: Vec<(&str, f64)> = Vec::new();
    let began = Instant::now();
    let mut case = |what: &'static str, ok: bool, t: Instant| {
        assert!(ok, "case failed: {what}");
        cases.push((what, t.elapsed().as_secs_f64() * 1000.0));
    };

    let base = std::env::temp_dir().join(format!("prc-target-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let proj = project(&base);
    let host_runner = stand_in(&base.join("host-runner"), cobolt_compiler::backend_module::BACKEND_RUNNER_NAME);
    let linux_runner = stand_in(&base.join("linux-runner"), "prc-backend-runner");
    let windows_runner = stand_in(&base.join("windows-runner"), "prc-backend-runner.exe");

    // The Build of this machine's own system, as it has always been: into the project's bin/.
    let t = Instant::now();
    let host = build_backend(&proj, &BackendBuildOptions { runner: Some(host_runner.runner.clone()), ..BackendBuildOptions::default() }).expect("the host Build");
    case("the Build of this machine's own system takes the module path and installs into the project's bin/", host.engine == BackendEngine::Module && host.binary.starts_with(proj.join("bin")), t);
    let before = snapshot(&proj.join("bin"));

    // For Linux: the same payload, the other runner, a folder of its own.
    let t = Instant::now();
    let linux_bin = base.join("linux-bin");
    let opts = BackendBuildOptions { target: Some(BackendTarget { system: LINUX, runner: linux_runner.clone(), bin_dir: linux_bin.clone() }), ..BackendBuildOptions::default() };
    let linux = build_backend(&proj, &opts).expect("the Build for linux-x86_64");
    case("a Build for linux-x86_64 takes the module path and installs into the folder it was given", linux.engine == BackendEngine::Module && linux.binary.starts_with(&linux_bin), t);

    let t = Instant::now();
    case("the project's own bin/ is byte for byte as before (Run and Preview start that Backend)", snapshot(&proj.join("bin")) == before, t);

    let t = Instant::now();
    let (host_name, linux_name) = (host.binary.file_name().unwrap().to_string_lossy().into_owned(), linux.binary.file_name().unwrap().to_string_lossy().into_owned());
    case("the file is named <name>-backend, with no .exe for Linux", linux_name == host_name.trim_end_matches(".exe") && !linux_name.ends_with(".exe"), t);

    let t = Instant::now();
    case("the module is the same module: equal SHA-256", host.module_sha256 == linux.module_sha256 && host.module_sha256.is_some(), t);

    let t = Instant::now();
    let (host_exe, linux_exe) = (std::fs::read(&host.binary).unwrap(), std::fs::read(&linux.binary).unwrap());
    let same_backend = payload_part(&host_exe, Kind::Backend).is_some() && payload_part(&host_exe, Kind::Backend) == payload_part(&linux_exe, Kind::Backend);
    case("the Backend part of the payload (interface, exports, module) is equal byte for byte", same_backend, t);

    let t = Instant::now();
    case("this machine's own Build still holds its machine code (the host path is unchanged)", host.machine_code_bytes.is_some() && payload_part(&host_exe, Kind::MachineCode).is_some(), t);

    let t = Instant::now();
    case("no machine code for another system, and the Output of the Build says so", linux.machine_code_bytes.is_none() && payload_part(&linux_exe, Kind::MachineCode).is_none() && linux.notes.iter().any(|n| n.contains("no machine code is made for linux-x86_64")), t);

    let t = Instant::now();
    let runner_bytes = std::fs::read(&linux_runner.runner).unwrap();
    case("the file begins with that system's runner, whole", linux_exe.starts_with(&runner_bytes), t);

    let t = Instant::now();
    let sbom = std::fs::read_to_string(linux_bin.join("sbom.cdx.json")).expect("the bill of materials is beside the file");
    case("the bill of materials lists the module's hash and the libraries of that runner's own list", sbom.contains(linux.module_sha256.as_deref().unwrap()) && sbom.contains("wasmtime"), t);

    // The file name of a Windows system.
    let t = Instant::now();
    let windows_bin = base.join("windows-bin");
    let opts = BackendBuildOptions { target: Some(BackendTarget { system: WINDOWS, runner: windows_runner, bin_dir: windows_bin.clone() }), ..BackendBuildOptions::default() };
    let windows = build_backend(&proj, &opts).expect("the Build for windows-x86_64");
    case("a Build for a Windows system names the file with .exe", windows.binary.file_name().unwrap().to_string_lossy().ends_with("-backend.exe") && windows.binary.starts_with(&windows_bin), t);

    // A Backend that needs Cargo is refused, and nothing is installed (R5, AC5 in its unit form).
    let t = Instant::now();
    let refused_bin = base.join("refused-bin");
    let cargo_target = |bin: &Path| BackendTarget { system: LINUX, runner: linux_runner.clone(), bin_dir: bin.to_path_buf() };
    let cargo = build_backend(&proj, &BackendBuildOptions { path: Some(BackendPath::Cargo), target: Some(cargo_target(&refused_bin)), ..BackendBuildOptions::default() }).expect_err("a Backend sent to Cargo is not built for another system");
    case("a Backend the developer sent to Cargo is refused for another system, naming the system and the reason", matches!(&cargo, CompilerError::BackendForeignRefused { system, reasons } if system == "linux-x86_64" && reasons.contains("Cargo")) && cargo.to_string().contains("packaged on the system it will run on"), t);

    let t = Instant::now();
    let config_path = proj.join("backend").join("backend.toml");
    let config = std::fs::read_to_string(&config_path).unwrap();
    std::fs::write(&config_path, format!("{config}\n[identity]\nname = \"hello\"\n")).unwrap();
    let identity = build_backend(&proj, &BackendBuildOptions { target: Some(cargo_target(&refused_bin)), ..BackendBuildOptions::default() }).expect_err("an [identity] section sends the Backend to Cargo");
    let named = matches!(&identity, CompilerError::BackendForeignRefused { reasons, .. } if reasons.contains("identity"));
    std::fs::write(&config_path, config).unwrap();
    case("a Backend with an [identity] section is refused for another system, naming the Build's own reason", named, t);

    let t = Instant::now();
    case("a refused Build installs nothing: the target folder was not made", !refused_bin.exists(), t);

    let t = Instant::now();
    let none = build_backend(&proj, &BackendBuildOptions { target: Some(BackendTarget { system: LINUX, runner: RunnerFiles { runner: base.join("nowhere").join("prc-backend-runner"), sbom: base.join("nowhere").join(BACKEND_RUNNER_SBOM) }, bin_dir: refused_bin.clone() }), ..BackendBuildOptions::default() }).expect_err("no runner");
    case("a runner that is not there is refused (the Build never falls back to this machine's runner)", matches!(none, CompilerError::BackendForeignRefused { .. }) && !refused_bin.exists(), t);

    println!("\n== Spec 115 T6, T7: a Backend built for another system (R3, R5, R9, R10; AC5) ==");
    for (what, ms) in &cases {
        println!("  ok   {what:<110} {ms:>8.1} ms");
    }
    println!("  {} cases, {} passed, 0 failed, {:.1} ms", cases.len(), cases.len(), began.elapsed().as_secs_f64() * 1000.0);
    let _ = std::fs::remove_dir_all(&base);
}
