// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The platform's compiled libraries, made once per PowerRustCOBOL version.
//!
//! A program's build is its own few seconds only when the crates under it are
//! already compiled in the shared cache ([`crate::build_cache`]). This module
//! puts them there before the first program asks:
//!
//! 1. **Shipped libraries.** A release carries `libs/<os>/<arch>/target`, the
//!    cache a release build produced ([`produce_libs`]). The first time it is
//!    needed the folder is copied into the cache — when the compiler that made
//!    it is the compiler installed here, since another `rustc` cannot read it.
//! 2. **Warm-up.** Two stand-in programs (a form application, and one that also
//!    reaches SQL) are built against the SDK, which compiles whatever the
//!    shipped libraries did not cover — the platform crates, whose cargo
//!    identity includes the folder the SDK is installed in.
//!
//! `warm/<version>` in the cache records that both are done, so the next start
//! of the same version does nothing.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::runtime_features::RuntimeFeatures;
use crate::{build_cache, generate_cargo_toml, resolve_build_root};

/// Where the cache records which versions were warmed.
const WARM_DIR: &str = "warm";
/// Names the compiler a shipped `target/` was made by.
const RUSTC_STAMP: &str = "rustc.txt";

/// The two stand-in programs: what a form application links without a
/// database, and with one. Every other program differs from these only in
/// `audio` or the semantic model, which are compiled on demand.
fn variants() -> [(&'static str, RuntimeFeatures); 2] {
    let forms = RuntimeFeatures::default().shared_platform(true);
    [("forms", forms), ("forms-sql", RuntimeFeatures { sql: true, ..forms })]
}

/// `libs/<os>/<arch>` under the SDK: `macos`, `linux`, `windows`, `android` or
/// `ios`, then the processor (`aarch64`, `x86_64`).
pub fn libs_dir(sdk_root: &Path) -> PathBuf {
    sdk_root.join("libs").join(os_name()).join(std::env::consts::ARCH)
}

fn os_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        "android" => "android",
        "ios" => "ios",
        _ => "linux",
    }
}

fn rustc_version() -> Option<String> {
    let out = Command::new("rustc").arg("-vV").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Whether the platform has been warmed for `version` in this build folder.
pub fn is_warm(version: &str) -> bool {
    let (root, _) = resolve_build_root(None);
    build_cache::cache_dir(&root).join(WARM_DIR).join(version).is_file()
}

/// What [`warm_platform_cache`] did.
#[derive(Debug, Default, Clone)]
pub struct WarmOutcome {
    /// Already warm for this version — nothing was done.
    pub already: bool,
    /// The shipped libraries were copied into the cache.
    pub seeded_from_libs: bool,
    /// The stand-ins that were built.
    pub built: Vec<&'static str>,
}

/// Make sure the shared cache holds the platform compiled for `version`.
/// `log` receives one line per step. Safe to call from a background thread; a
/// failure leaves the cache usable (the next program's build compiles what is
/// missing, as it always did) and is returned for the log.
pub fn warm_platform_cache(
    sdk_root: &Path,
    version: &str,
    log: &mut dyn FnMut(&str),
) -> Result<WarmOutcome, String> {
    let (build_root, _) = resolve_build_root(None);
    let cache = build_cache::open(&build_root)?;
    let stamp = cache.join(WARM_DIR).join(version);
    if stamp.is_file() {
        return Ok(WarmOutcome { already: true, ..Default::default() });
    }
    let mut outcome = WarmOutcome::default();

    // A cache of an older version holds platform units this version no longer
    // matches; cargo would leave them beside the new ones for good.
    if let Ok(old) = std::fs::read_dir(cache.join(WARM_DIR)) {
        if old.flatten().next().is_some() {
            log("   a new PowerRustCOBOL version — the platform libraries are compiled again; third-party ones stay");
        }
    }

    let shipped = libs_dir(sdk_root).join("target");
    if shipped.is_dir() && !cache.join("release").join(".fingerprint").is_dir() {
        let theirs = std::fs::read_to_string(libs_dir(sdk_root).join(RUSTC_STAMP)).unwrap_or_default();
        match rustc_version() {
            Some(ours) if ours.trim() == theirs.trim() => {
                log(&format!("   using the libraries shipped in {}", libs_dir(sdk_root).display()));
                copy_tree(&shipped, &cache).map_err(|e| format!("cannot copy the shipped libraries: {e}"))?;
                outcome.seeded_from_libs = true;
            }
            _ => log("   the shipped libraries were made by another Rust compiler — compiling here instead"),
        }
    }

    let check = build_cache::check_platform(sdk_root, &cache).map_err(|e| format!("cannot judge the platform crates: {e}"))?;
    for (name, hash) in &check.record {
        let _ = build_cache::write_marker(&cache, name, hash);
    }
    for (label, features) in variants() {
        log(&format!("   compiling the platform libraries ({label})…"));
        let dir = build_root.join(format!("cobolt-build-_warmup-{label}"));
        build_variant(sdk_root, &dir, &cache, features, &check)?;
        outcome.built.push(label);
    }
    for (name, hash) in &check.hashes {
        let _ = build_cache::write_marker(&cache, name, hash);
    }

    std::fs::create_dir_all(cache.join(WARM_DIR)).map_err(|e| e.to_string())?;
    std::fs::write(&stamp, version).map_err(|e| e.to_string())?;
    Ok(outcome)
}

fn build_variant(
    sdk_root: &Path,
    dir: &Path,
    cache: &Path,
    features: RuntimeFeatures,
    check: &build_cache::PlatformCheck,
) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir.join("src")).map_err(io)?;
    let manifest = generate_cargo_toml(
        "prc-warmup",
        "0.0.0",
        &sdk_root.join("crates"),
        true,
        dir,
        &[],
        features,
    );
    std::fs::write(dir.join("Cargo.toml"), manifest).map_err(io)?;
    std::fs::write(dir.join("src").join("main.rs"), "fn main() {}\n").map_err(io)?;
    build_cache::seed_lock(sdk_root, dir);

    if !check.stale.is_empty() {
        let graph = build_cache::sdk_crates_in_graph(dir, cache)?;
        let stale: Vec<&str> = check.stale.iter().copied().filter(|c| graph.contains(c)).collect();
        build_cache::clean_platform(dir, cache, &stale)?;
    }

    for offline in [true, false] {
        let mut cmd = Command::new("cargo");
        cmd.args(["build", "--release"]);
        if offline {
            cmd.arg("--offline");
        }
        let out = cmd
            .current_dir(dir)
            .env("CARGO_TARGET_DIR", cache)
            .output()
            .map_err(|e| format!("cannot run cargo: {e}"))?;
        if out.status.success() {
            return Ok(());
        }
        if !offline {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(format!("cargo build failed: {}", err.lines().rev().take(12).collect::<Vec<_>>().join("\n")));
        }
    }
    Ok(())
}

/// Build the platform for `libs_dir(sdk_root)` — what the release workflow runs
/// after staging the SDK. The result is `target/` stripped of what a user's
/// machine cannot use (incremental state, build-script scratch space) and
/// the `rustc.txt` that [`warm_platform_cache`] checks before copying it.
pub fn produce_libs(sdk_root: &Path, log: &mut dyn FnMut(&str)) -> Result<u64, String> {
    let out = libs_dir(sdk_root);
    let target = out.join("target");
    std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    let rustc = rustc_version().ok_or("rustc is not available")?;
    let check = build_cache::PlatformCheck::default();
    let scratch = out.join("_scratch");
    for (label, features) in variants() {
        log(&format!("compiling {label}…"));
        build_variant(sdk_root, &scratch.join(label), &target, features, &check)?;
    }
    let _ = std::fs::remove_dir_all(&scratch);
    for profile in ["release", "debug"] {
        let _ = std::fs::remove_dir_all(target.join(profile).join("incremental"));
        let _ = std::fs::remove_dir_all(target.join(profile).join("examples"));
        // The platform's own units carry the folder the SDK was built in; on a
        // user's machine the SDK sits elsewhere, so cargo would never match
        // them. Only the third-party crates travel.
        for (sub, prefixes) in [
            (".fingerprint", &["cobolt-", "prc-warmup"][..]),
            ("build", &["cobolt-", "prc-warmup"][..]),
            ("deps", &["cobolt_", "libcobolt_", "prc_warmup", "prc-warmup"][..]),
        ] {
            for entry in std::fs::read_dir(target.join(profile).join(sub)).into_iter().flatten().flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if prefixes.iter().any(|p| name.starts_with(p)) {
                    let path = entry.path();
                    let _ = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
                }
            }
        }
    }
    std::fs::write(out.join(RUSTC_STAMP), format!("{rustc}\n")).map_err(|e| e.to_string())?;
    Ok(dir_bytes(&target))
}

fn dir_bytes(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_bytes(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

/// Copy a folder tree, keeping modification times (cargo judges freshness by
/// them) and not overwriting what is already there.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_tree(&entry.path(), &dest)?;
        } else if kind.is_file() && !dest.exists() {
            std::fs::copy(entry.path(), &dest)?;
            if let Ok(t) = entry.metadata().and_then(|m| m.modified()) {
                let _ = std::fs::File::options().write(true).open(&dest).and_then(|f| f.set_modified(t));
            }
        } else if kind.is_symlink() && !dest.exists() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(std::fs::read_link(entry.path())?, &dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_stand_ins_share_everything_but_sql() {
        let [(a, plain), (b, sql)] = variants();
        assert_eq!((a, b), ("forms", "forms-sql"));
        assert!(!plain.sql && sql.sql);
        assert!(plain.aws && plain.maps && plain.pdf && plain.office && plain.http && plain.images);
        assert!(!plain.audio && !plain.kb_semantic);
        assert_eq!(RuntimeFeatures { sql: false, ..sql }, plain);
    }

    #[test]
    fn libs_are_found_by_os_and_processor() {
        let dir = libs_dir(Path::new("/sdk"));
        let text = dir.to_string_lossy().replace('\\', "/");
        assert!(text.starts_with("/sdk/libs/"), "{text}");
        assert!(text.ends_with(std::env::consts::ARCH), "{text}");
        assert!(["macos", "linux", "windows", "android", "ios"].contains(&dir.parent().unwrap().file_name().unwrap().to_str().unwrap()));
    }

    #[test]
    fn copying_keeps_times_and_never_overwrites() {
        let base = std::env::temp_dir().join(format!("prc-warm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (from, to) = (base.join("from"), base.join("to"));
        std::fs::create_dir_all(from.join("a")).unwrap();
        std::fs::create_dir_all(&to).unwrap();
        std::fs::write(from.join("a/x"), "new").unwrap();
        std::fs::write(from.join("y"), "new").unwrap();
        std::fs::write(to.join("y"), "mine").unwrap();
        let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        std::fs::File::options().write(true).open(from.join("a/x")).unwrap().set_modified(old).unwrap();
        copy_tree(&from, &to).unwrap();
        assert_eq!(std::fs::read_to_string(to.join("a/x")).unwrap(), "new");
        assert_eq!(std::fs::metadata(to.join("a/x")).unwrap().modified().unwrap(), old);
        assert_eq!(std::fs::read_to_string(to.join("y")).unwrap(), "mine");
        let _ = std::fs::remove_dir_all(&base);
    }
}
