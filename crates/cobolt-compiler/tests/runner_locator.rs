// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T20 (R6, AC13): a Build finds the PowerRustCOBOL runner where the packages put
//! it. Each layout is laid out under a temporary folder with stand-in files:
//! - a Linux or Windows package: the IDE, `rcrun` and `prc-runner` in one folder;
//! - the macOS application: all three in `Contents/MacOS`, and the archive's loose `rcrun` a link
//!   into it (Unix only);
//! - a Linux package's `/usr/bin/rcrun`, a link into `/opt/powerrustcobol` (Unix only);
//! - a checkout: `target/<profile>/` once `cargo build -p cobolt-runner` has run;
//! - a checkout's test in `target/<profile>/deps`, which finds none and names its runner;
//! - an IDE with no runner beside it, which finds none.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use cobolt_compiler::{runner_near, RUNNER_NAME};

const EXE: &str = std::env::consts::EXE_SUFFIX;

fn file(p: &Path) -> PathBuf {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, b"stand-in").unwrap();
    p.to_path_buf()
}

#[test]
fn the_runner_is_found_beside_the_ide_and_in_a_checkout() {
    let t = Instant::now();
    let root = std::env::temp_dir().join(format!("prc-t20-locator-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let mut checks: Vec<(String, bool)> = Vec::new();
    let mut check = |what: &str, ok: bool| checks.push((what.to_owned(), ok));
    let same = |a: Option<PathBuf>, b: &Path| a.and_then(|a| a.canonicalize().ok()) == b.canonicalize().ok();

    // A Linux or Windows package: one flat folder.
    let flat = root.join("PowerRustCOBOL-2.0.519-linux-x86_64");
    let ide = file(&flat.join(format!("PowerRustCOBOL{EXE}")));
    let rcrun = file(&flat.join(format!("rcrun{EXE}")));
    let runner = file(&flat.join(RUNNER_NAME));
    check("package: the IDE finds the runner beside it", same(runner_near(&ide), &runner));
    check("package: rcrun finds the runner beside it", same(runner_near(&rcrun), &runner));

    // The macOS application.
    let macos = root.join("mac/PowerRustCOBOL.app/Contents/MacOS");
    let app_ide = file(&macos.join("PowerRustCOBOL"));
    let app_rcrun = file(&macos.join("rcrun"));
    let app_runner = file(&macos.join(RUNNER_NAME));
    check("macOS: the IDE in Contents/MacOS finds the runner", same(runner_near(&app_ide), &app_runner));
    let mut links = 0;
    #[cfg(unix)]
    {
        // The archive's loose `rcrun`, a link into the application.
        let loose = root.join("mac/rcrun");
        std::os::unix::fs::symlink(&app_rcrun, &loose).unwrap();
        check("macOS: the archive's linked rcrun finds the runner in the application", same(runner_near(&loose), &app_runner));
        // A Linux package's /usr/bin/rcrun, a link into /opt/powerrustcobol.
        let usr_bin = root.join("usr/bin");
        fs::create_dir_all(&usr_bin).unwrap();
        let linked = usr_bin.join("rcrun");
        std::os::unix::fs::symlink(&rcrun, &linked).unwrap();
        check("Linux: /usr/bin/rcrun finds the runner in the package folder", same(runner_near(&linked), &runner));
        links = 2;
    }
    let _ = app_rcrun;

    // A checkout: target/<profile>/.
    let profile = root.join("checkout/target/debug");
    let dev_ide = file(&profile.join(format!("cobolt-ide{EXE}")));
    let dev_runner = file(&profile.join(RUNNER_NAME));
    check("checkout: the IDE in target/debug finds target/debug/prc-runner", same(runner_near(&dev_ide), &dev_runner));
    let test_exe = file(&profile.join("deps").join(format!("a_test-0123456789abcdef{EXE}")));
    check("checkout: a test in target/debug/deps finds none (it names its runner)", runner_near(&test_exe).is_none());

    // An IDE with no runner beside it.
    let bare = file(&root.join("bare").join(format!("PowerRustCOBOL{EXE}")));
    check("no runner beside the IDE: none", runner_near(&bare).is_none());

    let failures: Vec<&(String, bool)> = checks.iter().filter(|(_, ok)| !ok).collect();
    println!("== the runner locator (DESKTOP-WASM-001 T20: R6, AC13) ==");
    println!("layouts: package, macOS application, checkout, checkout test, bare IDE; links followed {links}");
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "ok  " } else { "FAIL" });
    }
    println!("{} of {} checks passed in {:.1} ms", checks.len() - failures.len(), checks.len(), t.elapsed().as_secs_f64() * 1000.0);
    let _ = fs::remove_dir_all(&root);
    assert!(failures.is_empty(), "failed: {failures:?}");
}
