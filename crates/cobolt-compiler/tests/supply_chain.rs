// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The supply-chain check (WASM-013 R9, T16; AC8 audit half). `cargo deny` runs offline over the
//! workspace lock and the web player's own lock, one section at a time, with the repository's own
//! configuration:
//!
//! * **advisories** — the RustSec database read from the local copy (`cargo deny fetch` puts it in
//!   `~/.cargo/advisory-dbs`); `deny.toml` carries the dated baseline of what has no safe upgrade yet;
//! * **licenses** — the plan's set plus the six the operator approved on 2026-11-06 (O4);
//! * **sources** — crates.io only;
//! * **bans** — what the backend and the browser build must never link, mirroring the `NEVER` lists of the
//!   dependency guards (`crates/cobolt-server/deny-bans.toml`, `crates/cobolt-web-player/deny-bans.toml`).
//!
//! Two negative fixtures prove the check can fail: a local advisory database holding one **synthetic**
//! advisory against a crate in our lock, and a configuration whose licence list lacks a licence the lock
//! uses. With no `cargo-deny` (or no local database) the test prints `SKIPPED: …` and fails when
//! `PR_SECURITY_GATE=1`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the repository root")
}

fn gate() -> bool {
    std::env::var("PR_SECURITY_GATE").is_ok_and(|v| !v.is_empty() && v != "0")
}

fn skip(why: &str) {
    println!("SKIPPED: {why}");
    assert!(!gate(), "PR_SECURITY_GATE is set but the supply-chain check cannot run: {why}");
}

fn deny(args: &[&str]) -> Output {
    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .current_dir(root())
        .arg("deny")
        .args(args)
        .output()
        .expect("cargo runs")
}

fn available() -> bool {
    deny(&["--version"]).status.success()
}

/// The summary `cargo deny` prints last (`advisories ok`, `bans FAILED`, …), from either stream.
fn verdict(o: &Output) -> String {
    let text = format!("{}\n{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    text.lines().rev().map(str::trim).find(|l| l.ends_with(" ok") || l.ends_with("FAILED") || l.contains(" ok, ") || l.contains("FAILED")).unwrap_or("(no verdict line)").to_owned()
}

fn db_present() -> bool {
    std::env::var_os("HOME").is_some_and(|h| Path::new(&h).join(".cargo/advisory-dbs").is_dir())
}

#[test]
fn the_four_sections_pass_on_both_locks_and_the_check_can_fail() {
    if !available() {
        return skip("cargo-deny not installed (cargo install cargo-deny --locked)");
    }
    if !db_present() {
        return skip("no local RustSec database (run `cargo deny fetch` once)");
    }
    let player = "crates/cobolt-web-player/Cargo.toml";
    let server_cfg = "crates/cobolt-server/deny-bans.toml";
    let player_cfg = "crates/cobolt-web-player/deny-bans.toml";
    let runs: Vec<(&str, Vec<&str>)> = vec![
        ("advisories, workspace lock", vec!["--offline", "--config", "deny.toml", "check", "advisories"]),
        ("advisories, player lock", vec!["--offline", "--manifest-path", player, "--config", "deny.toml", "check", "advisories"]),
        ("licenses, workspace lock", vec!["--offline", "--config", "deny.toml", "check", "licenses"]),
        ("licenses, player lock", vec!["--offline", "--manifest-path", player, "--config", "deny.toml", "check", "licenses"]),
        ("sources, workspace lock", vec!["--offline", "--config", "deny.toml", "check", "sources"]),
        ("sources, player lock", vec!["--offline", "--manifest-path", player, "--config", "deny.toml", "check", "sources"]),
        ("bans, backend closure", vec!["--offline", "--manifest-path", "crates/cobolt-server/Cargo.toml", "--exclude-dev", "--config", server_cfg, "check", "bans"]),
        ("bans, browser closure", vec!["--offline", "--manifest-path", player, "--exclude-dev", "--config", player_cfg, "check", "bans"]),
    ];
    let mut failed = Vec::new();
    for (name, args) in &runs {
        let t = Instant::now();
        let o = deny(args);
        let v = verdict(&o);
        println!("{name:<28} {v:<22} {:>6.1} s", t.elapsed().as_secs_f64());
        if !o.status.success() {
            failed.push(format!("{name}: {v}\n{}", String::from_utf8_lossy(&o.stderr).lines().filter(|l| l.starts_with("error")).take(8).collect::<Vec<_>>().join("\n")));
        }
    }
    assert!(failed.is_empty(), "the supply-chain check failed:\n{}", failed.join("\n"));

    // ── negative 1: a local database with one synthetic advisory against `anyhow`, which is in our lock ──
    let scratch = std::env::temp_dir().join(format!("prc-supply-chain-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let repo = scratch.join("db");
    std::fs::create_dir_all(repo.join("crates/anyhow")).unwrap();
    std::fs::write(
        repo.join("crates/anyhow/RUSTSEC-2099-0001.md"),
        "```toml\n[advisory]\nid = \"RUSTSEC-2099-0001\"\npackage = \"anyhow\"\ndate = \"2099-01-01\"\nurl = \"https://example.invalid/synthetic\"\ncategories = [\"denial-of-service\"]\nkeywords = [\"synthetic\"]\n\n[versions]\npatched = [\">=99.0.0\"]\n```\n\n# Synthetic advisory used by the test of the supply-chain check\n\nThis advisory does not exist upstream.\n",
    )
    .unwrap();
    let git = |dir: &Path, args: &[&str]| {
        let o = Command::new("git").current_dir(dir).args(["-c", "user.email=test@example.invalid", "-c", "user.name=test"]).args(args).output().expect("git runs");
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    };
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "synthetic"]);
    // cargo-deny names the database folder after the URL; ask it once (it fails offline and says where it looked).
    let url = "https://example.invalid/fakedb";
    let dbs = scratch.join("dbs");
    std::fs::create_dir_all(&dbs).unwrap();
    let cfg = scratch.join("deny.toml");
    std::fs::write(&cfg, format!("[advisories]\ndb-path = {:?}\ndb-urls = [{:?}]\n", dbs.display().to_string(), url)).unwrap();
    let probe = deny(&["--offline", "--config", &cfg.display().to_string(), "check", "advisories"]);
    let text = String::from_utf8_lossy(&probe.stderr).into_owned();
    let folder = text.split('"').chain(text.split_whitespace()).find_map(|w| w.strip_prefix(&format!("{}/", dbs.display())).map(|r| r.trim_end_matches(&['\'', '"', ':'][..]).to_owned())).filter(|f| f.starts_with("fakedb-"));
    let Some(folder) = folder else {
        panic!("cargo-deny did not say which folder it expects for the database: {text}");
    };
    let clone = Command::new("git").args(["clone", "-q", &repo.display().to_string(), &dbs.join(&folder).display().to_string()]).output().unwrap();
    assert!(clone.status.success(), "{}", String::from_utf8_lossy(&clone.stderr));
    git(&dbs.join(&folder), &["fetch", "-q", "origin"]); // writes FETCH_HEAD, which cargo-deny reads for the database's age
    let o = deny(&["--offline", "--config", &cfg.display().to_string(), "check", "advisories"]);
    let err = String::from_utf8_lossy(&o.stderr).into_owned();
    println!("negative 1, synthetic advisory against anyhow: {}", verdict(&o));
    assert!(!o.status.success() && err.contains("Synthetic advisory"), "a database with an advisory against a crate in the lock must fail the check: {err}");

    // ── negative 2: the licence list without BSL-1.0, which xxhash-rust, clipboard-win and error-code use ──
    let real = std::fs::read_to_string(root().join("deny.toml")).unwrap();
    let without = real.replace("\"BSL-1.0\", ", "");
    assert_ne!(real, without, "the fixture removes a licence");
    let bad = scratch.join("deny-no-bsl.toml");
    std::fs::write(&bad, without).unwrap();
    let o = deny(&["--offline", "--config", &bad.display().to_string(), "check", "licenses"]);
    let err = String::from_utf8_lossy(&o.stderr).into_owned();
    println!("negative 2, BSL-1.0 left out of the licence list: {}", verdict(&o));
    assert!(!o.status.success() && err.contains("BSL-1.0"), "a licence missing from the list must fail the check: {err}");

    let _ = std::fs::remove_dir_all(&scratch);
    println!("supply chain: {} checks passed on 2 locks, 2 negative fixtures failed as they must", runs.len());
}
