// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T16, T20 (R5, R6): what the Backend runner must never link, read from the dependency graph Cargo itself
//! resolves.
//!
//! The runner is a server and the data capabilities of the Desktop runner (`cobolt-runner`, built without its `forms`
//! feature, which is the window side): files, SQL, HTTP, PartiQL, pictures. It links the interpreter's runtime for those
//! providers, so the guard is not "no `cobolt-runtime`" (the server's own guard has that one); it is that **none of the
//! window system, none of the IDE and no sound or graphics library comes with it**, and that the Desktop runner, built the
//! default way, still has them.

use std::collections::BTreeSet;
use std::process::Command;

/// The package names Cargo resolves for `args` (normal edges only).
fn closure(args: &[&str]) -> BTreeSet<String> {
    let cargo = option_env!("CARGO").unwrap_or("cargo");
    let out = Command::new(cargo)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["tree", "--offline", "-e", "normal", "--prefix", "none"])
        .args(args)
        .output()
        .expect("cargo tree runs");
    assert!(out.status.success(), "cargo tree {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.split(' ').next()).filter(|n| !n.is_empty()).map(str::to_owned).collect()
}

fn present<'a>(set: &BTreeSet<String>, names: &'a [&'a str]) -> Vec<&'a str> {
    names.iter().copied().filter(|n| set.contains(*n)).collect()
}

/// A window system, the IDE, the form host, the graphics library and the sound library.
const WINDOWS: &[&str] =
    &["egui", "eframe", "winit", "wgpu", "epaint", "cobolt-ide", "cobolt-form-host", "cobolt-graphics", "cobolt-agents", "cobolt-dap", "rodio", "cpal"];
/// The controls' data services of a window application: the Maps client and the AWS SDK.
const CONTROL_SERVICES: &[&str] = &["google_maps", "aws-config", "aws-sigv4"];

#[test]
fn the_backend_runner_links_no_window_system_and_the_desktop_runner_still_does() {
    let backend = closure(&["-p", "cobolt-backend-runner"]);
    assert!(
        backend.contains("tokio")
            && backend.contains("native-tls")
            && backend.contains("cobolt-server")
            && backend.contains("cobolt-runtime")
            && backend.contains("wasmtime"),
        "the guard looks at the real tree of the Backend runner, not an empty one"
    );
    assert_eq!(present(&backend, WINDOWS), Vec::<&str>::new(), "the Backend runner must not link a window system, the IDE, or a sound or graphics library");
    assert_eq!(present(&backend, CONTROL_SERVICES), Vec::<&str>::new(), "the controls' data services belong to the windows");

    // The Desktop runner, built the default way, has the window side: the guard cannot pass for want of one.
    let desktop = closure(&["-p", "cobolt-runner"]);
    assert!(desktop.contains("cobolt-form-host") && desktop.contains("egui"), "the Desktop runner keeps its windows");
    let headless = closure(&["-p", "cobolt-runner", "--no-default-features"]);
    assert_eq!(present(&headless, WINDOWS), Vec::<&str>::new(), "`cobolt-runner` without `forms` links no window system");

    println!(
        "Backend runner: {} crates, no window system; Desktop runner: {} crates; the window side is {} of them; runner without `forms`: {} crates",
        backend.len(),
        desktop.len(),
        desktop.difference(&headless).count(),
        headless.len()
    );
    println!("PASS 005 FAIL 000");
}
