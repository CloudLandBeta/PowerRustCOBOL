// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The bill of materials of the Backend runner (spec 114 T29, R17, C8): written by the release step, beside the runner.
//!
//! ```text
//! cargo run -p cobolt-compiler --example runner_sbom -- <the runner file> [<workspace root>]
//! ```
//!
//! A Backend built on the module path has no `cargo` to ask what it links, so the runner carries its own list: every
//! package `cargo tree` resolves for `cobolt-backend-runner` on this platform, with its version and licence, and the runner's
//! own SHA-256. The file is `prc-backend-runner.sbom.json` in the runner's folder. The Build reads it, replaces the runner's
//! own entry by the Backend's file and the module, and writes `bin/sbom.cdx.json`; a runner without this file sends the Build
//! to the Cargo path, named.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

fn fail(message: String) -> ! {
    eprintln!("runner_sbom: {message}");
    std::process::exit(1);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let runner = PathBuf::from(args.next().unwrap_or_else(|| fail("usage: runner_sbom <the runner file> [<workspace root>]".into())));
    let root = args.next().map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));
    let bytes = std::fs::read(&runner).unwrap_or_else(|e| fail(format!("{}: {e}", runner.display())));

    let out = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["tree", "-p", "cobolt-backend-runner", "-e", "normal", "--prefix", "none", "--format", "{p}|{l}", "--offline"])
        .current_dir(&root)
        .output()
        .unwrap_or_else(|e| fail(format!("cargo tree cannot run: {e}")));
    if !out.status.success() {
        fail(format!("cargo tree failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let mut libs: BTreeSet<(String, String, String)> = BTreeSet::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let (package, license) = line.split_once('|').unwrap_or((line, ""));
        let mut words = package.split_whitespace();
        let (Some(name), Some(version)) = (words.next(), words.next()) else { continue };
        if name == "cobolt-backend-runner" {
            continue;
        }
        let license = license.trim();
        libs.insert((name.to_string(), version.trim_start_matches('v').to_string(), if license.is_empty() { "NOASSERTION".into() } else { license.to_string() }));
    }
    if libs.len() < 10 {
        fail(format!("cargo tree listed only {} packages: it did not resolve the runner's tree", libs.len()));
    }

    let digest: String = {
        use sha2::{Digest, Sha256};
        Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect()
    };
    let name = runner.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut components = vec![cobolt_wire::sbom::file_component(&name, &digest)];
    components.extend(libs.iter().map(|(n, v, l)| cobolt_wire::sbom::library_component(n, v, l)));
    let bom = cobolt_wire::sbom::cyclonedx("prc-backend-runner", components, None);
    let path = runner.with_file_name(cobolt_compiler::backend_module::BACKEND_RUNNER_SBOM);
    std::fs::write(&path, serde_json::to_string_pretty(&bom).expect("sbom")).unwrap_or_else(|e| fail(format!("{}: {e}", path.display())));
    println!("runner bill of materials: {} libraries and the runner's SHA-256 ({} bytes) → {}", libs.len(), bytes.len(), path.display());
}
