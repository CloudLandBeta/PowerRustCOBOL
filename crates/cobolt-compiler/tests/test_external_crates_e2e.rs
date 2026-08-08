// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 044 end-to-end: a real project with registered External Crates is
//! built through `build_project`, run, and delivered — proving the pins, the
//! vendored `[patch.crates-io]` mechanism, the manifest, determinism, and
//! the no-toolchain property, with timings reported (golden rule #7).
//!
//! The test vendors its fixture crates by downloading from crates.io exactly
//! as the IDE's add flow does (dev-dependency `ureq`); the shipped compiler
//! itself never touches the network. Covers AC2 (build+run), AC3's
//! handler-equivalent clause (a form event handler IS a nested program — 041
//! precedent), AC5, AC9, AC11, AC12 — and, implicitly, T5's wiring: the
//! build only succeeds because the pins reach `analyze_with`.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use cobolt_compiler::{build_project, external_crates, BuildOptions};

const CSV_VERSION: &str = "1.4.0";
const SERDE_VERSION: &str = "1.0.229";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

/// Download `<name>-<version>.crate` from crates.io and unpack it under the
/// project's `crates/` — the IDE add flow's vendoring, reproduced with the
/// same blocking stack.
fn vendor(project: &Path, name: &str, version: &str) {
    let target = external_crates::vendor_dir(project, name, version);
    let mut builder = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(60));
    if let Ok(connector) = native_tls::TlsConnector::new() {
        builder = builder.tls_connector(std::sync::Arc::new(connector));
    }
    let agent = builder.build();
    let url = format!("https://crates.io/api/v1/crates/{name}/{version}/download");
    let response = agent
        .get(&url)
        .set("User-Agent", "PowerRustCOBOL-spec044-e2e-test")
        .call()
        .unwrap_or_else(|e| panic!("cannot download {name} {version}: {e}"));
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(64 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .unwrap();
    let dest_root = target.parent().unwrap().to_path_buf();
    std::fs::create_dir_all(&dest_root).unwrap();
    tar::Archive::new(flate2::read::GzDecoder::new(bytes.as_slice()))
        .unpack(&dest_root)
        .unwrap();
    assert!(target.is_dir(), "expected {} after unpack", target.display());
}

/// Run a built binary and return stdout; `bare_env` clears `PATH` and the
/// toolchain homes (the 041 AC3 recipe) to prove no toolchain is consulted.
fn run_binary(bin: &Path, bare_env: bool) -> String {
    let mut cmd = std::process::Command::new(bin);
    if bare_env {
        cmd.env("PATH", "").env("RUSTUP_HOME", "").env("CARGO_HOME", "");
    }
    let out = cmd.output().unwrap_or_else(|e| panic!("cannot run {}: {e}", bin.display()));
    assert!(
        out.status.success(),
        "binary failed ({}):\nstdout:\n{}\nstderr:\n{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

const COBOLT_TOML_WITH_CRATES: &str = r#"[project]
name = "cratedemo"
version = "1.0.0"
main = "src/main.cbl"

[[crates]]
name = "csv"
requirement = ""
version = "1.4.0"
features = []
url = "https://crates.io/crates/csv"

[[crates]]
name = "serde"
requirement = ""
version = "1.0.229"
features = ["derive"]
url = "https://crates.io/crates/serde"
"#;

/// The fixture: the top-level block parses a CSV and derives Serialize on an
/// item-level type (serde/derive); the nested program — the shape of every
/// form event handler — uses csv independently.
const MAIN_CBL: &str = r#"       IDENTIFICATION DIVISION.
       PROGRAM-ID. CRATEDEMO.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-STRING IS "Rust.String"
       EXEC RUST
       use serde::Serialize;

       #[derive(Serialize)]
       pub struct Receipt {
           pub qty: i64,
       }
       END-EXEC.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 CSV-MAIN USAGE IS OBJECT REFERENCE RUST-STRING GLOBAL.
       01 WS-TEXT PIC X(40).
       PROCEDURE DIVISION.
           EXEC RUST
           use csv::ReaderBuilder;
           let data = "name,qty\nada,2\ngrace,3\n";
           let mut rows = 0_i64;
           let mut qty = 0_i64;
           let mut rdr = ReaderBuilder::new()
               .from_reader(data.as_bytes());
           for rec in rdr.records() {
               let rec = rec?;
               rows += 1;
               qty += rec.get(1).unwrap_or("0").parse::<i64>()?;
           }
           let receipt = Receipt { qty };
           csv_main.push_str(&format!(
               "rows={} qty={}", rows, receipt.qty));
           END-EXEC.
           INVOKE CSV-MAIN "to_string" RETURNING WS-TEXT
           DISPLAY "MAIN=" WS-TEXT
           CALL "GRID1-CLICK".
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. GRID1-CLICK.
       PROCEDURE DIVISION.
           EXEC RUST
           use csv::ReaderBuilder;
           let mut n = 0_i64;
           let mut rdr = ReaderBuilder::new()
               .from_reader("h\nx\ny\nz\n".as_bytes());
           for rec in rdr.records() {
               let _ = rec?;
               n += 1;
           }
           println!("HANDLER-ROWS={}", n);
           END-EXEC.
           GOBACK.
       END PROGRAM GRID1-CLICK.
       END PROGRAM CRATEDEMO.
"#;

#[test]
fn external_crates_build_run_manifest_and_determinism() {
    let t_all = Instant::now();
    let project = std::env::temp_dir().join(format!("prc044-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(project.join("cobolt.toml"), COBOLT_TOML_WITH_CRATES).unwrap();
    std::fs::write(project.join("src/main.cbl"), MAIN_CBL).unwrap();

    // ── Vendor (the IDE add flow's download, reproduced) ─────────────────────
    let t = Instant::now();
    vendor(&project, "csv", CSV_VERSION);
    vendor(&project, "serde", SERDE_VERSION);
    let vendor_ms = t.elapsed().as_millis();

    let opts = BuildOptions {
        verbose: false,
        workspace_root: Some(workspace_root()),
        ..Default::default()
    };

    // ── Build #1 (proves R7/R10/R20 and T5's wiring) ─────────────────────────
    let t = Instant::now();
    let built = build_project(&project.join("cobolt.toml"), &opts)
        .unwrap_or_else(|e| panic!("build failed: {e}"));
    let build1_s = t.elapsed().as_secs_f32();

    // ── Run — the blocks used the crates, main and handler alike (AC2/AC3) ──
    let out = run_binary(&built.binary_path, false);
    assert!(out.contains("MAIN=rows=2 qty=5"), "csv block wrong:\n{out}");
    assert!(out.contains("HANDLER-ROWS=3"), "nested-program block wrong:\n{out}");

    // ── AC12 — same binary, no toolchain reachable ───────────────────────────
    let out_bare = run_binary(&built.binary_path, true);
    assert!(out_bare.contains("MAIN=rows=2 qty=5"), "bare-env run wrong:\n{out_bare}");

    // ── AC11 — the delivered manifest, exact columns ─────────────────────────
    let manifest_path = project.join("dist").join(external_crates::RUST_MANIFEST_FILE);
    let manifest1 = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("missing {}: {e}", manifest_path.display()));
    assert!(manifest1.contains("| csv | 1.4.0 | https://crates.io/crates/csv |"));
    assert!(manifest1.contains("| serde | 1.0.229 | https://crates.io/crates/serde |"));

    // ── AC5 + R10/R15 — the staged lockfile: exactly ONE serde, and the pins
    //    resolve as path (vendored) packages, i.e. no `source =` line ─────────
    let build_dir = std::env::temp_dir().join("cobolt-build-cratedemo");
    let lock = std::fs::read_to_string(build_dir.join("Cargo.lock"))
        .expect("staged build dir must carry a lockfile");
    let serde_entries = lock.matches("\nname = \"serde\"\n").count();
    assert_eq!(serde_entries, 1, "expected exactly one serde in the lock");
    for pinned in ["csv", "serde"] {
        let block = lock
            .split("[[package]]")
            .find(|b| b.contains(&format!("name = \"{pinned}\"\n")))
            .unwrap_or_else(|| panic!("{pinned} missing from lock"));
        assert!(
            !block.contains("source = "),
            "`{pinned}` resolved from a registry, not the vendored source:\n{block}"
        );
    }

    // ── AC9 — determinism: an untouched rebuild changes nothing ──────────────
    let t = Instant::now();
    let _ = build_project(&project.join("cobolt.toml"), &opts).unwrap();
    let build2_s = t.elapsed().as_secs_f32();
    let manifest2 = std::fs::read_to_string(&manifest_path).unwrap();
    assert_eq!(manifest1, manifest2, "manifest must be byte-identical");
    let lock2 = std::fs::read_to_string(build_dir.join("Cargo.lock")).unwrap();
    assert_eq!(lock, lock2, "lockfile must not move between untouched builds");

    // ── AC11's second half — no crates ⇒ the stale manifest is removed ───────
    std::fs::write(
        project.join("cobolt.toml"),
        "[project]\nname = \"cratedemo\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. CRATEDEMO.\n\
                PROCEDURE DIVISION.\n           DISPLAY \"PLAIN-OK\".\n           STOP RUN.\n",
    )
    .unwrap();
    let t = Instant::now();
    let plain = build_project(&project.join("cobolt.toml"), &opts).unwrap();
    let build3_s = t.elapsed().as_secs_f32();
    assert!(!manifest_path.exists(), "stale rust_manifest.md must be removed");
    assert!(run_binary(&plain.binary_path, false).contains("PLAIN-OK"));

    // ── Result summary (golden rule #7) ──────────────────────────────────────
    println!("──────────────────────────────────────────────────────────");
    println!("spec 044 e2e — External Crates");
    println!("cases: csv block in main program; csv block in nested");
    println!("       (handler-equivalent) program; serde derive in an");
    println!("       item-level block; bare-env run; manifest columns;");
    println!("       one-serde lock; vendored (path) resolution;");
    println!("       byte-stable manifest+lock; stale-manifest removal");
    println!("vendor csv+serde:        {vendor_ms} ms");
    println!("build #1 (cold, pins):   {build1_s:.1} s");
    println!("build #2 (warm, no-op):  {build2_s:.1} s");
    println!("build #3 (crates gone):  {build3_s:.1} s");
    println!("total:                   {:.1} s", t_all.elapsed().as_secs_f32());
    println!("──────────────────────────────────────────────────────────");

    let _ = std::fs::remove_dir_all(&project);
}

const ALIAS_EGUI_VERSION: &str = "0.29.0";

/// Spec 045 R1/AC1 — an `egui` version that collides with the platform's own
/// linked `0.36` at an INCOMPATIBLE version is registered as an alias
/// (`prj_egui`) instead of refused; the generated build links both copies
/// side by side (`package = "egui"` path dependency, no `[patch.crates-io]`
/// entry — see `external_crates::pin_sections`), and a block's `use
/// prj_egui::…` compiles and runs.
const COBOLT_TOML_WITH_ALIAS: &str = r#"[project]
name = "aliasdemo"
version = "1.0.0"
main = "src/main.cbl"

[[crates]]
name = "egui"
requirement = "=0.29.0"
version = "0.29.0"
alias = "prj_egui"
url = "https://crates.io/crates/egui"
"#;

const ALIAS_MAIN_CBL: &str = r#"       IDENTIFICATION DIVISION.
       PROGRAM-ID. ALIASDEMO.
       PROCEDURE DIVISION.
           EXEC RUST
           let c = prj_egui::Color32::WHITE;
           println!("ALIAS-OK={}", c.r());
           END-EXEC.
           STOP RUN.
"#;

#[test]
fn external_crates_alias_build_and_run() {
    let t_all = Instant::now();
    let project = std::env::temp_dir().join(format!("prc045-alias-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(project.join("cobolt.toml"), COBOLT_TOML_WITH_ALIAS).unwrap();
    std::fs::write(project.join("src/main.cbl"), ALIAS_MAIN_CBL).unwrap();

    let t = Instant::now();
    vendor(&project, "egui", ALIAS_EGUI_VERSION);
    let vendor_ms = t.elapsed().as_millis();

    let opts = BuildOptions {
        verbose: false,
        workspace_root: Some(workspace_root()),
        ..Default::default()
    };

    let t = Instant::now();
    let built = build_project(&project.join("cobolt.toml"), &opts)
        .unwrap_or_else(|e| panic!("aliased build failed: {e}"));
    let build_s = t.elapsed().as_secs_f32();

    let out = run_binary(&built.binary_path, false);
    assert!(out.contains("ALIAS-OK=255"), "aliased egui block wrong:\n{out}");

    // The manifest notes the alias (spec 045 R4).
    let manifest_path = project.join("dist").join(external_crates::RUST_MANIFEST_FILE);
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("missing {}: {e}", manifest_path.display()));
    assert!(
        manifest.contains("| egui (as `prj_egui`) | 0.29.0 | https://crates.io/crates/egui |"),
        "manifest did not note the alias:\n{manifest}"
    );

    // The staged manifest carries a `package =` path dependency and no patch
    // for it (spec 045 R1) — the platform's own egui 0.36 stays unpatched.
    let build_dir = std::env::temp_dir().join("cobolt-build-aliasdemo");
    let cargo_toml = std::fs::read_to_string(build_dir.join("Cargo.toml"))
        .expect("staged build dir must carry the generated Cargo.toml");
    assert!(
        cargo_toml.contains("prj_egui = { package = \"egui\""),
        "got:\n{cargo_toml}"
    );
    assert!(
        !cargo_toml.contains("[patch.crates-io]") || !cargo_toml.contains("egui = { path"),
        "the aliased egui must not be patched:\n{cargo_toml}"
    );

    println!("────────────────────────────────────────────────");
    println!("spec 045 e2e — collision-alias build");
    println!("cases: egui 0.29.0 registered as prj_egui (collides");
    println!("       with the linked 0.36); block uses prj_egui::…");
    println!("vendor egui:   {vendor_ms} ms");
    println!("build (alias): {build_s:.1} s");
    println!("total:         {:.1} s", t_all.elapsed().as_secs_f32());
    println!("────────────────────────────────────────────────");

    let _ = std::fs::remove_dir_all(&project);
}
