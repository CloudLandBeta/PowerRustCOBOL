// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 115 T10 (R2, R8, AC8): `rcrun package --target <system>`, through the real binary.
//!
//! The refusals run in every sweep: they stop at the first step, before anything is built. The real package builds the
//! page and the Backend and starts this machine's own runner, so it is `#[ignore]` like the other real builds:
//! `PRC_BACKEND_RUNNER=<this machine's prc-backend-runner> cargo test -p cobolt-cli --test package_target -- --ignored --nocapture`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

use cobolt_compiler::backend_runners::{Arch, Os, System};
use sha2::{Digest, Sha256};

const ADDRESS: &str = "wss://oficina.example.com/ws";
const AUTH: &str = "\n[auth]\nissuer = [\"https://login.example.com\"]\naudiences = [\"api\"]\n";

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// A system that is not this machine's.
fn other_system() -> System {
    let linux_x86 = System { os: Os::Linux, arch: Arch::X86_64 };
    if System::host() == Some(linux_x86) {
        System { os: Os::Linux, arch: Arch::Aarch64 }
    } else {
        linux_x86
    }
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

/// A copy of PowerOficina without its built output, with the address and the sign-in the test gives it.
fn project(tag: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("prc-spec115-cli-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerOficina");
    let dir = root.join("PowerOficina");
    for entry in std::fs::read_dir(&source).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if matches!(name.as_str(), "dist" | "debug" | "temp" | "spec" | "tests") {
            continue;
        }
        let (src, dst) = (entry.path(), dir.join(&name));
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::copy(&src, &dst).unwrap();
        }
    }
    let manifest = dir.join("PowerOficina.project.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    let lines: Vec<String> = text.lines().map(|l| if l.starts_with("backend_url = ") { format!("backend_url = \"{ADDRESS}\"") } else { l.to_string() }).collect();
    std::fs::write(&manifest, lines.join("\n") + "\n").unwrap();
    let toml = dir.join("backend").join("backend.toml");
    let mut text = std::fs::read_to_string(&toml).unwrap();
    text.push_str(AUTH);
    std::fs::write(&toml, text).unwrap();
    (root, manifest)
}

/// The runners' list an installer brings, naming `system` with the hashes of the two files given.
fn list_file(root: &Path, system: System, runner: &[u8], sbom: &[u8]) -> PathBuf {
    let path = root.join("manifest.txt");
    std::fs::write(&path, format!("{}  {system}/{}\n{}  {system}/prc-backend-runner.sbom.json\n", hex(runner), system.runner_name(), hex(sbom))).unwrap();
    path
}

fn rcrun(args: &[&str], envs: &[(&str, &Path)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rcrun"));
    cmd.arg("package").args(args).env_remove("PRC_BACKEND_RUNNER_DIR").env_remove("PRC_RUNNERS_MANIFEST");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output().expect("rcrun starts")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn the_flags_of_package_are_refused_or_listed_before_anything_is_built() {
    let began = Instant::now();
    let mut rows: Vec<(&str, f64)> = Vec::new();
    let mut case = |what: &'static str, t: Instant| rows.push((what, t.elapsed().as_secs_f64() * 1000.0));
    let (root, manifest) = project("flags");
    let manifest_arg = manifest.to_str().unwrap();
    let none = root.join("no-list.txt");
    std::fs::write(&none, "").unwrap();
    let host = System::host().map(|s| s.to_string()).unwrap_or_default();

    // A system outside the list is refused, naming the list; with no list of runners the list is this machine's own system.
    let t = Instant::now();
    let out = rcrun(&["--target", "bogus", manifest_arg], &[("PRC_RUNNERS_MANIFEST", &none)]);
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("\"bogus\"") && err.contains(&format!("the systems are: {host}")), "{err}");
    case("--target bogus is refused naming the systems; with no list of runners, the machine's own", t);

    // The list the installer brought is the list the message names (the one list every entry point reads, AC8).
    let t = Instant::now();
    let other = other_system();
    let list = list_file(&root, other, b"runner", b"sbom");
    let out = rcrun(&["--target", "bogus", manifest_arg], &[("PRC_RUNNERS_MANIFEST", &list)]);
    let err = text(&out.stderr);
    let offered = cobolt_compiler::backend_runners::System::offered(&cobolt_compiler::backend_runners::RunnerManifest::parse(&std::fs::read_to_string(&list).unwrap()));
    assert_eq!(offered.len(), 2, "this machine's own system and {other}: {offered:?}");
    assert!(err.contains(&format!("the systems are: {}", offered.iter().map(System::to_string).collect::<Vec<_>>().join(", "))), "{err}");
    case("the systems named are System::offered of the installer's list", t);

    // A word that is a system but that no runner is listed for is refused the same way.
    let t = Instant::now();
    let unlisted = if other == (System { os: Os::Linux, arch: Arch::Aarch64 }) { "linux-x86_64" } else { "linux-aarch64" };
    let out = rcrun(&["--target", unlisted, manifest_arg], &[("PRC_RUNNERS_MANIFEST", &none)]);
    assert!(text(&out.stderr).contains(&format!("\"{unlisted}\"")), "{}", text(&out.stderr));
    assert_eq!(out.status.code(), Some(1));
    case("a system no runner is listed for is refused", t);

    // The two flags that say where a runner comes from mean nothing without --target.
    let t = Instant::now();
    for flag in [vec!["--runner-dir", "/nowhere"], vec!["--fetch-runner"]] {
        let mut args = flag.clone();
        args.push(manifest_arg);
        let out = rcrun(&args, &[]);
        assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
        assert!(text(&out.stderr).contains("--target"), "{}", text(&out.stderr));
    }
    case("--runner-dir and --fetch-runner without --target are a usage error", t);

    // A Desktop project is not packaged for another system.
    let t = Instant::now();
    let desktop = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerDemo3/PowerDemo3.project.toml");
    let out = rcrun(&["--target", &other.to_string(), desktop.to_str().unwrap()], &[("PRC_RUNNERS_MANIFEST", &list)]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("only a Web project is packaged for another system"), "{}", text(&out.stderr));
    case("a Desktop project is refused: only a Web project is packaged for another system", t);

    // The download is never made unless asked, and a failure names the address and the other way (R8): the proxy that
    // refuses the connection keeps this test off the network.
    let t = Instant::now();
    let build = root.join("build");
    std::fs::create_dir_all(&build).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rcrun"));
    cmd.args(["package", "--target", &other.to_string(), "--fetch-runner", manifest_arg, "--output"])
        .arg(root.join("out"))
        .env("PRC_RUNNERS_MANIFEST", &list)
        .env("POWERRUSTCOBOL_BUILD_DIR", &build)
        .env_remove("PRC_BACKEND_RUNNER_DIR")
        .env("https_proxy", "http://127.0.0.1:1")
        .env("HTTPS_PROXY", "http://127.0.0.1:1")
        .env("ALL_PROXY", "http://127.0.0.1:1");
    let out = cmd.output().unwrap();
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("could not be downloaded from https://github.com/") && err.contains("--runner-dir"), "{err}");
    assert!(!root.join("out").exists(), "nothing was written");
    case("--fetch-runner with no network exits 1 naming the address and --runner-dir", t);

    // Without the flag the same project says how to get the runner and downloads nothing.
    let t = Instant::now();
    let out = rcrun(&["--target", &other.to_string(), manifest_arg, "--output", root.join("out").to_str().unwrap()], &[("PRC_RUNNERS_MANIFEST", &list), ("POWERRUSTCOBOL_BUILD_DIR", &build)]);
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("is not on this machine") && err.contains("--fetch-runner"), "{err}");
    case("without --fetch-runner the runner is not downloaded: the refusal names the flag", t);

    // The help lists the three flags.
    let t = Instant::now();
    let help = text(&Command::new(env!("CARGO_BIN_EXE_rcrun")).arg("help").output().unwrap().stdout);
    for flag in ["--target <system>", "--runner-dir <folder>", "--fetch-runner"] {
        assert!(help.contains(flag), "the help lists {flag}");
    }
    case("the help lists --target, --runner-dir and --fetch-runner", t);

    let _ = std::fs::remove_dir_all(&root);
    println!("\n== Spec 115 T10: rcrun package --target, refusals and help (host {host}, other {other}) ==");
    for (what, ms) in &rows {
        println!("  ok   {what:<96} {ms:>8.1} ms");
    }
    println!("  {} cases, {} passed, 0 failed, {:.1} ms", rows.len(), rows.len(), began.elapsed().as_secs_f64() * 1000.0);
}

#[test]
#[ignore = "builds the page and the Backend and starts this machine's runner (PRC_BACKEND_RUNNER); takes a minute or more"]
fn a_package_for_another_system_through_the_command_line() {
    let own = std::env::var_os("PRC_BACKEND_RUNNER").expect("PRC_BACKEND_RUNNER names this machine's Backend runner: the twin that Prove starts");
    assert!(Path::new(&own).is_file());
    let system = other_system();
    let (root, manifest) = project("real");
    let runners = root.join("runners");
    let folder = runners.join(system.to_string());
    std::fs::create_dir_all(&folder).unwrap();
    let mut runner = vec![0x7f, b'E', b'L', b'F'];
    runner.extend(format!("stand-in runner of {system} ").bytes().cycle().take(6000));
    let sbom = serde_json::to_vec_pretty(&serde_json::json!({
        "bomFormat": "CycloneDX", "specVersion": "1.5", "version": 1,
        "components": [{"type": "library", "name": "wasmtime", "version": "44.0.3", "licenses": [{"license": {"name": "Apache-2.0"}}]}]
    }))
    .unwrap();
    std::fs::write(folder.join(system.runner_name()), &runner).unwrap();
    std::fs::write(folder.join("prc-backend-runner.sbom.json"), &sbom).unwrap();
    let list = list_file(&root, system, &runner, &sbom);
    let (out_dir, build) = (root.join("out"), root.join("build"));
    let started = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_rcrun"))
        .args(["package", manifest.to_str().unwrap(), "--target", &system.to_string(), "--runner-dir", runners.to_str().unwrap(), "--output", out_dir.to_str().unwrap()])
        .env("PRC_RUNNERS_MANIFEST", &list)
        .env("POWERRUSTCOBOL_BUILD_DIR", &build)
        .output()
        .unwrap();
    let (stdout, stderr) = (text(&out.stdout), text(&out.stderr));
    assert!(out.status.success(), "{stdout}\n{stderr}");
    assert!(stdout.contains(&format!("packaging for a server running {system}")) && stdout.contains(&format!("Package ready for {system}")), "{stdout}");
    assert!(stdout.contains("Not run on this machine"), "the report says what was not proven: {stdout}");
    let mut names: Vec<String> = std::fs::read_dir(&out_dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names, ["PowerOficina.project-install.sh", "PowerOficina.project.zip"]);
    let mut z = zip::ZipArchive::new(std::fs::File::open(out_dir.join("PowerOficina.project.zip")).unwrap()).unwrap();
    let mut exe = Vec::new();
    std::io::Read::read_to_end(&mut z.by_name("bin/poweroficina-backend").unwrap(), &mut exe).unwrap();
    assert!(exe.starts_with(&runner), "the Backend begins with the runner of {system}, whole");
    let script = std::fs::read_to_string(out_dir.join("PowerOficina.project-install.sh")).unwrap();
    assert!(script.contains("OS=linux") && script.contains("systemd"));
    assert!(build.join("runners").is_dir(), "the verified runner was kept in the cache folder the build uses");
    println!("\n== Spec 115 T10: rcrun package --target {system} (PowerOficina) ==");
    println!("  exit 0 in {:.1} s; the two files; bin/poweroficina-backend {} bytes begins with the {system} runner; script OS=linux", started.elapsed().as_secs_f64(), exe.len());
    for line in stdout.lines().filter(|l| l.contains("Proven on this machine") || l.contains("Package ready")) {
        println!("  {}", line.trim());
    }
    let _ = std::fs::remove_dir_all(&root);
}
