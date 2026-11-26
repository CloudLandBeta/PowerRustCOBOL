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
use cobolt_project_tools::web::package::{package_web, PackageError, Step};
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
    assert!(text.contains("backend_url = \"ws://127.0.0.1:8443\""), "the example's address changed: update the test");
    std::fs::write(&manifest, text.replace("backend_url = \"ws://127.0.0.1:8443\"", &format!("backend_url = \"{backend_url}\""))).unwrap();
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
        let e = package_web(&manifest, &out, "en", &mut |s| steps.push(s)).expect_err(what);
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
    let _ = package_web(&manifest, &out, "en", &mut |_| {}).expect_err("refused");
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
    let packaged = package_web(&manifest, &out, "en", &mut |s| {
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
    println!("  total      : {:.1} s", packaged.seconds);
    for n in &packaged.notes {
        println!("  note       : {n}");
    }
    let _ = std::fs::remove_dir_all(&root);
}
