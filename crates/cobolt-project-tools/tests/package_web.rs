// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 113 on a real Web project (a copy of `examples/PowerOficina`): `package_web` writes **exactly two files**
//! (`<name>.zip` and the install script of this system) and nothing else; a setting the package cannot install is
//! refused at the first step, before the page is built, and leaves the chosen folder as it was.
//!
//! The refusals run in every sweep. The real package builds the page and the Backend, so it is `#[ignore]` like the
//! other real builds: `cargo test -p cobolt-project-tools --test package_web -- --ignored --nocapture`.

use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::package::{package_web, PackageError, PackageOptions, Step};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

const ADDRESS: &str = "wss://oficina.example.com/ws";
const AUTH: &str = "\n[auth]\nissuer = [\"https://login.example.com\"]\naudiences = [\"api\"]\n";

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

/// A copy of PowerOficina without its built output, with the address and the sign-in the test gives it.
fn project(tag: &str, backend_url: &str, auth: bool) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("prc-spec113-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerOficina");
    let dir = root.join("PowerOficina");
    for entry in std::fs::read_dir(&source).unwrap() {
        let entry = entry.unwrap();
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
    // The example's own address is whatever the committed manifest holds (empty at the time of writing; the IDE writes
    // `ws://127.0.0.1:8443` when it saves the project), so the test sets the line instead of matching a value: a test that
    // matched the value passed on a developer's re-saved copy and failed on a clean checkout.
    let mut set = 0;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            if line.starts_with("backend_url = ") {
                set += 1;
                format!("backend_url = \"{backend_url}\"")
            } else {
                line.to_string()
            }
        })
        .collect();
    assert_eq!(set, 1, "the example's manifest must have exactly one `backend_url` line");
    std::fs::write(&manifest, lines.join("\n") + "\n").unwrap();
    if auth {
        let toml = dir.join("backend").join("backend.toml");
        let mut text = std::fs::read_to_string(&toml).unwrap();
        text.push_str(AUTH);
        std::fs::write(&toml, text).unwrap();
    }
    (root, manifest)
}

fn link() {
    cobolt_project_tools::web::link(Arc::new(GeneratorBackend));
}

/// The package of this machine's own system, in English: what `package_web` has always made.
fn en() -> PackageOptions {
    PackageOptions { lang: "en".into(), ..Default::default() }
}

#[test]
fn a_setting_the_package_cannot_install_stops_at_the_first_step_and_writes_nothing() {
    link();
    let started = Instant::now();
    let cases = [
        ("the address of the example (this computer)", "ws://127.0.0.1:8443", true, "computer of whoever opens the page"),
        ("a Backend that anyone may call", ADDRESS, false, "[auth]"),
        ("no address", "", true, "backend_url is empty"),
    ];
    for (what, url, auth, expect) in cases {
        let (root, manifest) = project("refuse", url, auth);
        let out = root.join("out");
        let mut steps = Vec::new();
        let e = package_web(&manifest, &out, &en(), &mut |s| steps.push(s)).expect_err(what);
        assert!(matches!(&e, PackageError::Refused(why) if why.contains(expect)), "{what}: {e}");
        assert_eq!(steps, [Step::Check], "{what}: a refusal stops before the page is built");
        assert!(!out.exists(), "{what}: the chosen folder is not even made");
        assert!(!root.join("PowerOficina").join("dist").exists(), "{what}: nothing was built");
        let _ = std::fs::remove_dir_all(&root);
    }
    println!("package_web: {} refusals, each at Check, nothing written; {} ms", cases.len(), started.elapsed().as_millis());
}

#[test]
fn a_folder_that_already_holds_files_is_left_alone_when_the_package_is_refused() {
    link();
    let (root, manifest) = project("keep", "ws://localhost:8443", true);
    let out = root.join("out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("mine.txt"), "the developer's").unwrap();
    let before: Vec<_> = std::fs::read_dir(&out).unwrap().map(|e| e.unwrap().file_name()).collect();
    let _ = package_web(&manifest, &out, &en(), &mut |_| {}).expect_err("refused");
    let after: Vec<_> = std::fs::read_dir(&out).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(before, after);
    assert_eq!(std::fs::read_to_string(out.join("mine.txt")).unwrap(), "the developer's");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
#[ignore = "builds the page and the Backend (cargo and the SDK); takes minutes"]
fn a_real_package_is_exactly_two_files() {
    link();
    let (root, manifest) = project("real", ADDRESS, true);
    let out = root.join("out");
    let started = Instant::now();
    let mut steps = Vec::new();
    let mut at: Vec<(Step, f64)> = Vec::new();
    let packaged = package_web(&manifest, &out, &en(), &mut |s| {
        at.push((s, started.elapsed().as_secs_f64()));
        steps.push(s);
    })
    .unwrap_or_else(|e| panic!("the package was not made: {e}"));

    assert_eq!(steps, [Step::Check, Step::Page, Step::Backend, Step::Prove, Step::Zip, Step::Script, Step::Done]);
    let mut names: Vec<String> = std::fs::read_dir(&out).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    // The example's manifest names the project `PowerOficina.project`: the files carry the project's own name.
    let stem = "PowerOficina.project";
    let mut want = [format!("{stem}.zip"), format!("{stem}-install.{}", if cfg!(windows) { "ps1" } else { "sh" })];
    want.sort();
    assert_eq!(names, want, "exactly two files, and no leftovers");
    assert!(packaged.has_backend && packaged.wss);

    let mut z = zip::ZipArchive::new(std::fs::File::open(&packaged.zip).unwrap()).unwrap();
    let listing: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().unwrap().into_owned()).collect();
    assert!(listing.iter().any(|n| n.starts_with("bin/") && n.contains("backend") && !n.ends_with(".json")), "{listing:?} has the Backend");
    // Spec 114 T32 (R19, AC8): the bill of materials is in the zip whichever way the Backend was made, and when the runner is
    // named (`PRC_BACKEND_RUNNER`) PowerOficina, whose programs the module serves in full, is made as a module.
    let made = packaged.backend.as_ref().expect("a package with a Backend says how the Backend was made");
    if std::env::var_os(cobolt_compiler::backend_module::BACKEND_RUNNER_ENV).is_some() {
        assert_eq!(made.engine, cobolt_compiler::backend_module::BackendEngine::Module, "{:?}", made.reasons);
        assert!(made.reasons.is_empty(), "{:?}", made.reasons);
    }
    assert!(listing.iter().any(|n| n == "bin/sbom.cdx.json"), "the bill of materials is in the zip: {listing:?}");
    for want in ["web/index.html", "backend.toml", "LICENSE", "NOTICE"] {
        assert!(listing.iter().any(|n| n == want), "{want} in {listing:?}");
    }
    // What the zip holds is the page, the Backend, its settings, the seed data and the notices: nothing of the project's source
    // sits at its root (`web/forms/*.json` is the built page's own data, not the project's forms).
    let top: std::collections::BTreeSet<&str> = listing.iter().map(|n| n.split('/').next().unwrap_or("")).collect();
    let allowed = ["web", "bin", "data", "assets", "backend.toml", "LICENSE", "NOTICE", "NOTICE-RUNTIME.txt"];
    let strangers: Vec<_> = top.iter().filter(|t| !allowed.contains(*t)).collect();
    assert!(strangers.is_empty(), "the zip's root holds only what runs: {strangers:?}");
    for bad in ["hosting", ".wcfrm", ".cfrm", ".cbl", ".map", ".pdb", "agentic_ai", "Knowledge Base"] {
        let found: Vec<_> = listing.iter().filter(|n| n.contains(bad)).take(5).collect();
        assert!(found.is_empty(), "{bad} must not be in the zip: {found:?}");
    }
    let mut toml = String::new();
    std::io::Read::read_to_string(&mut z.by_name("backend.toml").unwrap(), &mut toml).unwrap();
    assert!(toml.contains("https://oficina.example.com") && toml.contains("127.0.0.1:8443"), "{toml}");

    let text = std::fs::read_to_string(&packaged.script).unwrap();
    assert!(text.contains(&packaged.sha256), "the script checks the zip it was made with");
    assert!(!text.to_lowercase().contains("cobolt"), "no internal name in a script a customer reads");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&packaged.script).unwrap().permissions().mode() & 0o111, 0o111, "the script is executable");
        let ok = std::process::Command::new("bash").arg("-n").arg(&packaged.script).status().unwrap();
        assert!(ok.success(), "bash parses the script");
    }

    println!("package_web (PowerOficina, {ADDRESS}):");
    println!("  files      : {names:?}");
    println!("  zip        : {} bytes, {} entries, sha256 {}...", packaged.zip_bytes, packaged.files_in_zip, &packaged.sha256[..12]);
    println!("  script     : {} bytes; the command: {}", text.len(), packaged.command());
    println!("  secrets    : {:?}", packaged.required_secrets);
    for (step, secs) in &packaged.timings {
        println!("  step {:<8}: {:>7.1} s", step.key(), secs);
    }
    println!("  backend    : {} ({} ms in the Build: {})", made.engine.name(), made.phases.iter().map(|(_, ms)| ms).sum::<u128>(), made.phases.iter().map(|(n, ms)| format!("{n} {ms} ms")).collect::<Vec<_>>().join(", "));
    println!("  total      : {:.1} s", packaged.seconds);
    for n in &packaged.notes {
        println!("  note       : {n}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

// ───────────────────────── spec 115: a package for another system ─────────────────────────

use cobolt_compiler::backend_runners::{Arch, Fetch, Os, RunnerManifest, System};
use cobolt_project_tools::web::package::package_web_with;

const VERSION: &str = "0.0.0-spec115";

/// A system that is not this machine's, so the package for it is made the way a package for another system is made.
fn other_system() -> System {
    let linux_x86 = System { os: Os::Linux, arch: Arch::X86_64 };
    if System::host() == Some(linux_x86) {
        System { os: Os::Linux, arch: Arch::Aarch64 }
    } else {
        linux_x86
    }
}

fn runner_bytes(system: System) -> Vec<u8> {
    let mut bytes = vec![0x7f, b'E', b'L', b'F'];
    bytes.extend(format!("stand-in runner of {system} ").bytes().cycle().take(6000));
    bytes
}

fn sha(bytes: &[u8]) -> String {
    cobolt_web_bundle::hash::sha256_hex(bytes)
}

/// A folder `dir/<system>/` holding a stand-in runner (any bytes that begin with the ELF magic are a format the join accepts)
/// and its bill of materials, and the list that names their SHA-256, as the installer's `runners/manifest.txt` does.
fn stand_in(dir: &Path, system: System) -> RunnerManifest {
    let folder = dir.join(system.to_string());
    std::fs::create_dir_all(&folder).unwrap();
    let runner = runner_bytes(system);
    let sbom = serde_json::to_vec_pretty(&serde_json::json!({
        "bomFormat": "CycloneDX", "specVersion": "1.5", "version": 1,
        "components": [
            {"type": "file", "name": "prc-backend-runner", "hashes": [{"alg": "SHA-256", "content": sha(&runner)}]},
            {"type": "library", "name": "wasmtime", "version": "44.0.3", "licenses": [{"license": {"name": "Apache-2.0"}}]},
        ]
    }))
    .unwrap();
    std::fs::write(folder.join(system.runner_name()), &runner).unwrap();
    std::fs::write(folder.join("prc-backend-runner.sbom.json"), &sbom).unwrap();
    RunnerManifest::parse(&format!("{}  {system}/{}\n{}  {system}/prc-backend-runner.sbom.json\n", sha(&runner), system.runner_name(), sha(&sbom)))
}

/// A download that is not allowed to happen, or that fails the way an offline machine does.
struct Offline;

impl Fetch for Offline {
    fn get(&self, url: &str, _to: &Path) -> Result<(), String> {
        Err(format!("curl: could not reach {url} (the test is offline)"))
    }
}

fn options_for(system: System, root: &Path, runner_dir: Option<PathBuf>) -> PackageOptions {
    PackageOptions { lang: "en".into(), system: Some(system), runner_dir, fetch_runner: false, version: VERSION.into(), cache_root: Some(root.join("cache")) }
}

/// What the cache of the runner holds, for the check that a refused runner left nothing in it.
fn cache_files(root: &Path) -> Vec<String> {
    fn walk(at: &Path, out: &mut Vec<String>) {
        if let Ok(read) = std::fs::read_dir(at) {
            for e in read.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else {
                    out.push(p.file_name().unwrap().to_string_lossy().into_owned());
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join("cache"), &mut out);
    out.sort();
    out
}

#[test]
fn a_runner_nobody_verified_is_refused_at_the_first_step_and_nothing_is_written() {
    link();
    let began = Instant::now();
    let system = other_system();
    let mut rows: Vec<(String, f64)> = Vec::new();
    let (root, manifest) = project("target-refuse", ADDRESS, true);
    let out = root.join("out");

    let good_dir = root.join("runners");
    let list = stand_in(&good_dir, system);
    let original = runner_bytes(system);
    let (runner_file, expected) = (good_dir.join(system.to_string()).join(system.runner_name()), sha(&original));

    // A byte of the runner changed after the list was made (AC6): refused, naming the hash expected and the one found.
    let tampered = {
        let mut bytes = original.clone();
        bytes[100] ^= 1;
        bytes
    };
    let tampered_dir = root.join("tampered");
    let _ = stand_in(&tampered_dir, system);
    std::fs::write(tampered_dir.join(system.to_string()).join(system.runner_name()), &tampered).unwrap();
    let empty_dir = root.join("empty");
    std::fs::create_dir_all(&empty_dir).unwrap();

    type Case<'a> = (&'a str, PackageOptions, RunnerManifest, Vec<String>, bool);
    let cases: Vec<Case> = vec![
        ("an installation that carries no list of runners", options_for(system, &root, Some(good_dir.clone())), RunnerManifest::default(), vec!["no runner is offered for".into(), "no list of runners".into()], false),
        ("a list that names no runner for the system", options_for(system, &root, Some(good_dir.clone())), RunnerManifest::parse(&format!("{expected}  macos-x86_64/prc-backend-runner\n")), vec!["does not name one".into()], false),
        ("a runner of which one byte changed", options_for(system, &root, Some(tampered_dir.clone())), list.clone(), vec![format!("expected SHA-256 {expected}"), format!("found {}", sha(&tampered)), "nothing was written".into()], false),
        ("a folder that holds no runner of the system", options_for(system, &root, Some(empty_dir.clone())), list.clone(), vec!["is not on this machine".into(), "--runner-dir".into(), "--fetch-runner".into(), expected.clone()], false),
        ("no folder, no cache, and no permission to download", options_for(system, &root, None), list.clone(), vec!["is not on this machine".into(), "--fetch-runner".into()], false),
        ("a download that fails", PackageOptions { fetch_runner: true, ..options_for(system, &root, None) }, list.clone(), vec!["could not be downloaded from https://github.com/".into(), "--runner-dir".into()], true),
        ("a program that does not know its release", PackageOptions { version: String::new(), ..options_for(system, &root, Some(good_dir.clone())) }, list.clone(), vec!["release this program belongs to is not known".into()], true),
    ];
    for (what, options, runners, expect, is_failure) in &cases {
        let t = Instant::now();
        let mut steps = Vec::new();
        let e = package_web_with(&manifest, &out, options, runners, &Offline, &mut |s| steps.push(s)).expect_err(what);
        let text = e.to_string();
        for want in expect {
            assert!(text.contains(want.as_str()), "{what}: the message names {want:?}: {text}");
        }
        assert_eq!(matches!(e, PackageError::Failed(_)), *is_failure, "{what}: {e:?}");
        assert_eq!(steps, [Step::Check], "{what}: a runner that cannot be trusted stops before the page is built");
        assert!(!out.exists(), "{what}: the chosen folder is not even made");
        assert!(!root.join("PowerOficina").join("dist").exists(), "{what}: nothing was built");
        assert!(!cache_files(&root).iter().any(|f| f.ends_with(".part") || f == &system.runner_name()), "{what}: the cache holds no runner nobody verified: {:?}", cache_files(&root));
        rows.push((what.to_string(), t.elapsed().as_secs_f64() * 1000.0));
    }
    assert_eq!(std::fs::read(&runner_file).unwrap(), original, "the developer's own folder is read, never changed");

    // The settings are refused for another system exactly as for this one (AC7): a Backend that anyone may call.
    let t = Instant::now();
    let (open_root, open_manifest) = project("target-open", ADDRESS, false);
    let e = package_web_with(&open_manifest, &open_root.join("out"), &options_for(system, &open_root, Some(good_dir.clone())), &list, &Offline, &mut |_| {}).expect_err("no [auth]");
    assert!(matches!(&e, PackageError::Refused(why) if why.contains("[auth]")), "{e}");
    rows.push(("a Backend that anyone may call is refused as before".into(), t.elapsed().as_secs_f64() * 1000.0));
    let _ = std::fs::remove_dir_all(&open_root);
    let _ = std::fs::remove_dir_all(&root);

    println!("\n== Spec 115 T9: a package for {system} is refused before anything is built, when it cannot be trusted ==");
    for (what, ms) in &rows {
        println!("  ok   {what:<70} {ms:>8.1} ms");
    }
    println!("  {} cases, {} passed, 0 failed, {:.1} ms", rows.len(), rows.len(), began.elapsed().as_secs_f64() * 1000.0);
}

#[test]
#[ignore = "builds the page and the Backend (cargo and the SDK) and starts this machine's runner (PRC_BACKEND_RUNNER); takes minutes"]
fn a_package_for_another_system_carries_that_systems_backend() {
    link();
    let own = std::env::var_os(cobolt_compiler::backend_module::BACKEND_RUNNER_ENV).expect("PRC_BACKEND_RUNNER names this machine's Backend runner: the twin that Prove starts");
    assert!(Path::new(&own).is_file(), "{own:?} is a file");
    let system = other_system();
    let (root, manifest) = project("target-real", ADDRESS, true);
    let project_dir = root.join("PowerOficina");
    let runners = root.join("runners");
    let list = stand_in(&runners, system);
    let options = options_for(system, &root, Some(runners.clone()));
    let out = root.join("out");
    let bin_before = std::fs::read_dir(project_dir.join("bin")).map(|r| r.count()).unwrap_or(0);
    let started = Instant::now();
    let mut steps = Vec::new();
    let packaged = package_web_with(&manifest, &out, &options, &list, &Offline, &mut |s| steps.push(s)).unwrap_or_else(|e| panic!("the package was not made: {e}"));
    assert_eq!(steps, [Step::Check, Step::Page, Step::Backend, Step::Prove, Step::Zip, Step::Script, Step::Done]);
    assert_eq!(std::fs::read_dir(project_dir.join("bin")).map(|r| r.count()).unwrap_or(0), bin_before, "the Backend of another system is not installed in the project's own bin/ (Run and Preview start that one)");

    // AC1: two files, the Backend of that system, the script of that system.
    let mut names: Vec<String> = std::fs::read_dir(&out).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names, ["PowerOficina.project-install.sh".to_string(), "PowerOficina.project.zip".to_string()], "exactly two files, the script of Linux whatever this machine is");
    assert_eq!(packaged.system, Some(system));
    assert!(!packaged.executable_run, "the executable of another system was not run here");
    let mut z = zip::ZipArchive::new(std::fs::File::open(&packaged.zip).unwrap()).unwrap();
    let listing: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().unwrap().into_owned()).collect();
    let exe_name = listing.iter().find(|n| n.starts_with("bin/") && n.ends_with("-backend")).unwrap_or_else(|| panic!("{listing:?} holds the Backend")).clone();
    assert!(!exe_name.ends_with(".exe"), "{exe_name}: no .exe for Linux");
    let mut exe = Vec::new();
    std::io::Read::read_to_end(&mut z.by_name(&exe_name).unwrap(), &mut exe).unwrap();
    assert!(exe.starts_with(&runner_bytes(system)), "the file begins with the runner of {system}, whole");
    assert_eq!(z.by_name(&exe_name).unwrap().unix_mode(), Some(0o100755));
    let payload = cobolt_program_package::locate(&exe).expect("the file holds the application's payload");
    assert!(cobolt_program_package::decode(payload).is_ok(), "and the payload decodes");
    let mut sbom = String::new();
    std::io::Read::read_to_string(&mut z.by_name("bin/sbom.cdx.json").unwrap(), &mut sbom).unwrap();
    assert!(sbom.contains("wasmtime"), "the bill of materials lists the libraries of that runner's own list");
    assert!(sbom.contains(&sha(&exe)), "and the hash of the executable shipped");
    let script = std::fs::read_to_string(&packaged.script).unwrap();
    assert!(script.contains("OS=linux") && script.contains("systemd"), "the script is the one of Linux");
    assert!(script.contains(&packaged.sha256), "and checks the zip it was made with");

    // AC2: what runs and nothing of the source.
    for bad in [".cbl", ".cob", ".cfrm", ".wcfrm", ".mcfrm", ".cidx", ".rs"] {
        assert!(!listing.iter().any(|n| n.ends_with(bad)), "{bad} is not in the zip: {listing:?}");
    }
    assert!(!listing.iter().any(|n| n.starts_with("src/") || n.starts_with("COPYBOOKS/")), "{listing:?}");

    // The module is the one this machine's own Build makes.
    let host = cobolt_compiler::backend_build::build_backend(&project_dir, &cobolt_compiler::backend_build::BackendBuildOptions::default()).expect("this machine's own Build");
    let module = host.module_sha256.expect("the module path");
    assert!(sbom.contains(&module), "the module in the package is the module this machine's Build makes");

    // AC7: what was proven and what was not.
    let note = packaged.notes.iter().find(|n| n.contains("Not run on this machine")).unwrap_or_else(|| panic!("{:?}", packaged.notes));
    assert!(note.contains("Proven on this machine") && note.contains(&system.to_string()), "{note}");
    assert!(packaged.notes.iter().any(|n| n.contains("no machine code is made")), "{:?}", packaged.notes);

    println!("\n== Spec 115 T9: PowerOficina packaged for {system} on this machine ==");
    println!("  files      : {names:?}");
    println!("  zip        : {} bytes, {} entries, sha256 {}...", packaged.zip_bytes, packaged.files_in_zip, &packaged.sha256[..12]);
    println!("  backend    : {exe_name}, {} bytes, runner {} bytes + payload {} bytes, mode 0755, module sha256 {}...", exe.len(), runner_bytes(system).len(), payload.len(), &module[..12]);
    for (step, secs) in &packaged.timings {
        println!("  step {:<8}: {:>7.1} s", step.key(), secs);
    }
    println!("  total      : {:.1} s ({:.1} s measured by the test)", packaged.seconds, started.elapsed().as_secs_f64());
    for n in &packaged.notes {
        println!("  note       : {n}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
#[ignore = "builds the page and compiles the Backend's module; takes minutes"]
fn a_backend_that_needs_cargo_is_not_packaged_for_another_system() {
    link();
    let system = other_system();
    let (root, manifest) = project("target-cargo", ADDRESS, false);
    let project_dir = root.join("PowerOficina");
    let toml = project_dir.join("backend").join("backend.toml");
    let mut text = std::fs::read_to_string(&toml).unwrap();
    text.push_str("\n[identity]\nname = \"oficina\"\n");
    std::fs::write(&toml, text).unwrap();
    let runners = root.join("runners");
    let list = stand_in(&runners, system);
    let before = std::fs::read_dir(project_dir.join("bin")).map(|r| r.count()).unwrap_or(0);
    let out = root.join("out");
    let mut steps = Vec::new();
    let e = package_web_with(&manifest, &out, &options_for(system, &root, Some(runners)), &list, &Offline, &mut |s| steps.push(s)).expect_err("a Backend that needs Cargo");
    let text = e.to_string();
    assert!(matches!(e, PackageError::Refused(_)), "{text}");
    assert!(text.contains(&system.to_string()) && text.contains("packaged on the system it will run on") && text.contains("identity"), "{text}");
    assert_eq!(steps, [Step::Check, Step::Page, Step::Backend], "refused where the Build says so");
    assert!(!out.exists(), "the chosen folder was not made");
    assert_eq!(std::fs::read_dir(project_dir.join("bin")).map(|r| r.count()).unwrap_or(0), before, "the project's own bin/ is as it was");
    println!("\n== Spec 115 T9 (AC5): a Backend with an [identity] section is refused for {system}, at the Backend step, nothing written ==\n  {text}");
    let _ = std::fs::remove_dir_all(&root);
}

/// Spec 115 T14 (D5, R8): the window of Package Project asks before a download, and only when one would happen: the
/// package is for another system, the project has a Backend, and no runner is in the folder named or in the cache.
/// A runner that is here but cannot be trusted, no list to check one against, and a project with no Backend to join
/// it to are not "to download": the package itself answers them, in words, so the question is never asked for them.
#[test]
fn the_download_is_asked_for_only_when_one_would_happen() {
    use cobolt_project_tools::web::package::needs_runner_download_with;
    let began = Instant::now();
    let system = other_system();
    let (root, manifest) = project("ask-fetch", ADDRESS, true);
    let list = stand_in(&root.join("runners"), system);
    let empty = root.join("empty");
    std::fs::create_dir_all(&empty).unwrap();

    // The runner as a release attachment lands in the cache the first time: the same folder the download writes.
    let cached_root = root.join("cached");
    let cache_list = stand_in(&cached_root.join("cache").join("runners").join(VERSION), system);
    assert_eq!(cache_list.expected(system, cobolt_compiler::backend_runners::RunnerFile::Runner), list.expected(system, cobolt_compiler::backend_runners::RunnerFile::Runner));

    let tampered = root.join("tampered");
    let _ = stand_in(&tampered, system);
    let mut bytes = runner_bytes(system);
    bytes[100] ^= 1;
    std::fs::write(tampered.join(system.to_string()).join(system.runner_name()), &bytes).unwrap();

    let no_backend = project("ask-fetch-none", ADDRESS, true);
    std::fs::remove_file(no_backend.1.parent().unwrap().join("backend").join("exports.toml")).unwrap();

    let host = System::host().expect("a system the product is built for");
    let here = |dir: Option<PathBuf>| options_for(system, &root, dir);
    let cases: Vec<(&str, bool, bool)> = vec![
        ("another system, a Backend, a folder with no runner in it", needs_runner_download_with(&manifest, &here(Some(empty.clone())), &list), true),
        ("another system, a Backend, no folder and an empty cache", needs_runner_download_with(&manifest, &here(None), &list), true),
        ("another system, a Backend, the runner in the folder named", needs_runner_download_with(&manifest, &here(Some(root.join("runners"))), &list), false),
        ("another system, a Backend, the runner already in the cache", needs_runner_download_with(&manifest, &options_for(system, &cached_root, None), &list), false),
        ("a runner that is here but one byte changed", needs_runner_download_with(&manifest, &here(Some(tampered.clone())), &list), false),
        ("an installation with no list of runners", needs_runner_download_with(&manifest, &here(Some(empty.clone())), &RunnerManifest::default()), false),
        ("a project with no Backend to join the runner to", needs_runner_download_with(&no_backend.1, &here(Some(empty.clone())), &list), false),
        ("this computer's own system", needs_runner_download_with(&manifest, &options_for(host, &root, Some(empty.clone())), &list), false),
        ("no system chosen", needs_runner_download_with(&manifest, &PackageOptions { system: None, ..here(Some(empty.clone())) }, &list), false),
        ("a program that does not know its release", needs_runner_download_with(&manifest, &PackageOptions { version: String::new(), ..here(Some(empty.clone())) }, &list), false),
    ];
    println!("\n== Spec 115 T14 (D5): the question before a download ==");
    for (what, got, want) in &cases {
        println!("  {}   {what:<62} asks: {got}", if got == want { "ok  " } else { "FAIL" });
    }
    println!("  {} cases, {} passed, {:.1} ms", cases.len(), cases.iter().filter(|(_, g, w)| g == w).count(), began.elapsed().as_secs_f64() * 1000.0);
    for (what, got, want) in &cases {
        assert_eq!(got, want, "{what}");
    }
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(no_backend.0);
}
