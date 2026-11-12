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
//! 2. **Warm-up.** Three stand-in programs (a form application, one that also
//!    reaches SQL, and one that draws on a Canvas) are built against the SDK,
//!    which compiles whatever the shipped libraries did not cover — the
//!    platform crates, whose cargo identity includes the folder the SDK is
//!    installed in.
//!
//! `warm/<version>` in the cache records that all three are done, so the next start
//! of the same version does nothing.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use crate::runtime_features::RuntimeFeatures;
use crate::{build_cache, generate_cargo_toml, resolve_build_root};

/// Where the cache records which versions were warmed.
const WARM_DIR: &str = "warm";
/// Names the compiler a shipped `target/` was made by.
const RUSTC_STAMP: &str = "rustc.txt";
/// One `<sha256>  <path under target/>` line per shipped library file. It travels in
/// the installer, so it is what a downloaded replacement is checked against.
const MANIFEST: &str = "manifest.txt";
/// In the cache: the size and date each shipped file had when it last matched.
const VERIFIED: &str = "verified.tsv";
/// Where a release's attachments live.
const RELEASES_URL: &str = "https://github.com/CloudLandBeta/PowerRustCOBOL/releases/download";

static CURRENT: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Name the running PowerRustCOBOL version (the IDE does, once, at start). With it
/// set, a full build REPAIRS the shared libraries ([`repair_libs`]) instead of
/// purging them; without it (`rcrun build --full`) it purges as it always did.
pub fn set_current_version(version: &str) {
    let _ = CURRENT.set(version.to_owned());
}

/// The version [`set_current_version`] named, if the host (the IDE) did.
pub(crate) fn current_version() -> Option<&'static str> {
    CURRENT.get().map(String::as_str)
}

/// A build that got this far compiled whatever the platform lacked.
pub(crate) fn mark_current_warm(cache: &Path) {
    if let Some(v) = CURRENT.get() {
        let _ = std::fs::create_dir_all(cache.join(WARM_DIR));
        let _ = std::fs::write(cache.join(WARM_DIR).join(v), v);
    }
}

/// The three stand-in programs: what a form application links without a
/// database, with one, and with a graphics control (Canvas2D, Canvas3D,
/// ImageBuffer, Plot, Scene3D, Physics2D, Physics3D). The graphics stand-in
/// (2.0.468) carries `cobolt-graphics` and the hundred-odd libraries under it
/// (wgpu, naga, rapier, parry, nalgebra, the text shaper); without it the first
/// Build of a program that draws compiled all of them, the slowest part of the
/// platform, while every other first Build took seconds. Every other program
/// differs from these only in `audio`, the semantic model, or SQL and graphics
/// together, whose own platform crates are compiled on demand over libraries
/// these already compiled.
fn variants() -> [(&'static str, RuntimeFeatures); 3] {
    let forms = RuntimeFeatures::default().shared_platform(true);
    [
        ("forms", forms),
        ("forms-sql", RuntimeFeatures { sql: true, ..forms }),
        ("forms-graphics", RuntimeFeatures { graphics: true, ..forms }),
    ]
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
            ours => log(&other_compiler(&theirs, ours.as_deref())),
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
    // The build runs in a scratch folder under the SDK, and cargo reads a relative `CARGO_TARGET_DIR` from its own
    // working directory: the workflow's `dist/<package>/sdk` put the libraries in `_scratch/forms/dist/...`, which
    // is deleted below, left `target` empty, and on Windows made paths longer than the 260 characters its linker takes.
    let sdk_root = absolute(sdk_root)?;
    let sdk_root = sdk_root.as_path();
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
    let manifest = manifest_text(&target).map_err(|e| e.to_string())?;
    std::fs::write(out.join(MANIFEST), manifest).map_err(|e| e.to_string())?;
    Ok(dir_bytes(&target))
}

/// `path` under the current directory when it is relative; the path itself otherwise. It touches no file.
fn absolute(path: &Path) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// What the log says when the shipped libraries cannot be used: which compiler made them, which one is
/// here, what that costs, and the one command that makes them usable.
fn other_compiler(theirs: &str, ours: Option<&str>) -> String {
    let made_by = theirs.lines().next().unwrap_or("").trim();
    let here = ours.and_then(|o| o.lines().next()).map(str::trim).unwrap_or("no rustc on the PATH");
    let release = made_by.split_whitespace().nth(1).unwrap_or("");
    format!(
        "   ⚠️  The shipped libraries were made by {made_by}, and this machine has {here}. Cargo cannot \
         use libraries another compiler made, so they are compiled here instead: the first build takes \
         as long as a build from nothing (on a modest machine, an hour or more). To use them, run \
         `rustup default {release}` and start PowerRustCOBOL again."
    )
}

/// The addresses [`libs_asset_name`] is tried at, in order. A release's tag is typed when it is
/// published, and both spellings are in use: `2.0.452` (every release so far) and the workflow's
/// default `v2.0.452`. Until 2.0.467 only the second was tried, and every download answered 404.
pub fn libs_asset_urls(version: &str) -> [String; 2] {
    let name = libs_asset_name(version);
    [format!("{RELEASES_URL}/{version}/{name}"), format!("{RELEASES_URL}/v{version}/{name}")]
}

/// `PowerRustCOBOL-<version>-<os>-<arch>-libs.tar.gz`: the release attachment that holds
/// this machine's `libs/<os>/<arch>/target`.
pub fn libs_asset_name(version: &str) -> String {
    format!("PowerRustCOBOL-{version}-{}-{}-libs.tar.gz", os_name(), std::env::consts::ARCH)
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    std::io::copy(&mut std::fs::File::open(path)?, &mut hasher)?;
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn files_under(dir: &Path, rel: &str, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let relative = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        match entry.file_type() {
            Ok(k) if k.is_dir() => files_under(&entry.path(), &relative, out),
            Ok(k) if k.is_file() => out.push(relative),
            _ => {}
        }
    }
}

/// The manifest of a `target/` folder: its files, sorted, each with its SHA-256.
fn manifest_text(target: &Path) -> std::io::Result<String> {
    let mut files = Vec::new();
    files_under(target, "", &mut files);
    files.sort();
    let mut text = String::new();
    for rel in files {
        text.push_str(&format!("{}  {rel}\n", sha256_file(&target.join(&rel))?));
    }
    Ok(text)
}

fn parse_manifest(text: &str) -> Vec<(&str, &str)> {
    text.lines().filter_map(|l| l.split_once("  ")).filter(|(h, _)| h.len() == 64).collect()
}

/// The size and modification time (whole seconds) a file had when it last matched the manifest.
type Seen = std::collections::HashMap<String, (u64, u64)>;

fn stamp(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let secs = meta.modified().ok()?.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs();
    Some((meta.len(), secs))
}

fn read_seen(cache: &Path) -> Seen {
    std::fs::read_to_string(cache.join(WARM_DIR).join(VERIFIED))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            Some((f.next()?.to_owned(), (f.next()?.parse().ok()?, f.next()?.parse().ok()?)))
        })
        .collect()
}

fn write_seen(cache: &Path, seen: &Seen) {
    let mut rows: Vec<_> = seen.iter().collect();
    rows.sort();
    let text: String = rows.iter().map(|(p, (n, t))| format!("{p}\t{n}\t{t}\n")).collect();
    let _ = std::fs::create_dir_all(cache.join(WARM_DIR));
    let _ = std::fs::write(cache.join(WARM_DIR).join(VERIFIED), text);
}

/// The manifest's files that `cache` lacks or holds changed. The modified date decides
/// which files are read at all: one whose size and date are those of the last time it
/// matched is trusted, and only a file whose date or size moved is hashed against the
/// manifest (a file rewritten with the same size and the same second is not seen — the
/// price of not reading gigabytes on every full build). Matches are remembered in `seen`.
fn damaged(cache: &Path, entries: &[(&str, &str)], seen: &mut Seen) -> Vec<String> {
    let mut bad = Vec::new();
    for (hash, rel) in entries {
        let path = cache.join(rel);
        let Some(now) = stamp(&path) else {
            seen.remove(*rel);
            bad.push((*rel).to_owned());
            continue;
        };
        if seen.get(*rel) == Some(&now) {
            continue;
        }
        if sha256_file(&path).is_ok_and(|have| have == *hash) {
            seen.insert((*rel).to_owned(), now);
        } else {
            seen.remove(*rel);
            bad.push((*rel).to_owned());
        }
    }
    bad
}

/// What a full build does for the shared libraries: it checks every file the release
/// shipped against its recorded hash and, for those that are missing or changed,
/// downloads the release's libraries from GitHub and puts back exactly those files,
/// checking each again. Nothing that is fine is touched or recompiled. Returns how
/// many files were repaired; a release without shipped libraries has nothing to check.
pub fn repair_libs(sdk_root: &Path, version: &str, log: &mut dyn FnMut(&str)) -> Result<usize, String> {
    let Ok(text) = std::fs::read_to_string(libs_dir(sdk_root).join(MANIFEST)) else {
        return Ok(0);
    };
    let entries = parse_manifest(&text);
    let (build_root, _) = resolve_build_root(None);
    let cache = build_cache::open(&build_root)?;
    let mut seen = read_seen(&cache);
    let bad = damaged(&cache, &entries, &mut seen);
    write_seen(&cache, &seen);
    if bad.is_empty() {
        log(&format!("   all {} shared libraries are intact", entries.len()));
        return Ok(0);
    }
    // Libraries another compiler made are of no use here: cargo would compile every one of them again.
    // Downloading them would only add the wait for the download.
    let theirs = std::fs::read_to_string(libs_dir(sdk_root).join(RUSTC_STAMP)).unwrap_or_default();
    let ours = rustc_version();
    if ours.as_deref().map(str::trim) != Some(theirs.trim()) {
        log(&other_compiler(&theirs, ours.as_deref()));
        return Ok(0);
    }
    let urls = libs_asset_urls(version);
    log(&format!("   {} of {} shared libraries are missing or changed — downloading them from {}", bad.len(), entries.len(), urls[0]));
    let archive = build_root.join(format!("_repair-{}.tar.gz", std::process::id()));
    let list = build_root.join(format!("_repair-{}.txt", std::process::id()));
    let result = (|| {
        let path = archive.to_string_lossy();
        let fetch = |url: &str| run("curl", &["--fail", "--location", "--silent", "--show-error", "--proto", "=https", "--tlsv1.2", "-o", &path, url]);
        if let Err(first) = fetch(&urls[0]) {
            log(&format!("   not there ({first}) — trying {}", urls[1]));
            fetch(&urls[1])?;
        }
        std::fs::write(&list, bad.join("\n")).map_err(|e| e.to_string())?;
        run("tar", &["-xzf", &archive.to_string_lossy(), "-C", &cache.to_string_lossy(), "-T", &list.to_string_lossy()])?;
        let still = damaged(&cache, &entries, &mut seen);
        write_seen(&cache, &seen);
        if still.is_empty() { Ok(bad.len()) } else { Err(format!("{} files still differ from the release after the download", still.len())) }
    })();
    let _ = std::fs::remove_file(&archive);
    let _ = std::fs::remove_file(&list);
    result
}

fn run(program: &str, args: &[&str]) -> Result<(), String> {
    let out = Command::new(program).args(args).output().map_err(|e| format!("cannot run {program}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
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
    fn a_relative_sdk_folder_is_made_absolute_before_cargo_is_given_a_target_in_it() {
        // The release workflow passes `dist/<package>/sdk`; cargo reads a relative target folder from the
        // scratch folder it runs in, so the libraries must be asked for at a path that does not depend on it.
        let relative = Path::new("dist").join("PowerRustCOBOL-0.0.0-test").join("sdk");
        let made = absolute(&relative).unwrap();
        assert!(made.is_absolute(), "{}", made.display());
        assert!(made.starts_with(std::env::current_dir().unwrap()), "{}", made.display());
        assert!(made.ends_with(&relative), "{}", made.display());
        let there = std::env::temp_dir();
        assert_eq!(absolute(&there).unwrap(), there, "an absolute folder stays as it is");
        // What the compile is given: the target is under the SDK folder, not under the scratch folder.
        let target = libs_dir(&made).join("target");
        assert!(target.is_absolute() && !target.starts_with(libs_dir(&made).join("_scratch")));
    }

    #[test]
    fn the_stand_ins_share_everything_but_sql_or_graphics() {
        let [(a, plain), (b, sql), (c, graphics)] = variants();
        assert_eq!((a, b, c), ("forms", "forms-sql", "forms-graphics"));
        assert!(!plain.sql && sql.sql);
        assert!(plain.aws && plain.maps && plain.pdf && plain.office && plain.http && plain.images);
        assert!(!plain.audio && !plain.kb_semantic && !plain.graphics);
        assert_eq!(RuntimeFeatures { sql: false, ..sql }, plain);
        // 2.0.468 — a Canvas program's first Build finds the graphics libraries compiled.
        assert!(graphics.graphics && !graphics.sql);
        assert_eq!(RuntimeFeatures { graphics: false, ..graphics }, plain);
    }

    /// 2.0.468 — the stand-in is what a form with a Canvas actually links: the build's own
    /// reading of such a form turns `graphics` on, and the shared feature set keeps it.
    #[test]
    fn a_canvas_form_links_what_the_graphics_stand_in_compiled() {
        let mut form = cobolt_forms::Form::new("F", "F", 800, 600);
        form.controls = vec![cobolt_forms::Control::new("CNV-1", cobolt_forms::ControlType::from_str("Canvas2D"), 0, 0)];
        let needs = crate::runtime_features::scan_forms([&form]).shared_platform(true);
        let [_, _, (_, graphics)] = variants();
        assert_eq!(needs, graphics);
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
    fn a_manifest_finds_the_missing_and_the_changed() {
        let base = std::env::temp_dir().join(format!("prc-manifest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("release/deps")).unwrap();
        std::fs::write(base.join("release/deps/a.rlib"), "one").unwrap();
        std::fs::write(base.join("release/deps/b.rlib"), "two").unwrap();
        std::fs::write(base.join("c.txt"), "three").unwrap();
        let text = manifest_text(&base).unwrap();
        let entries = parse_manifest(&text);
        assert_eq!(entries.len(), 3);
        let mut seen = Seen::new();
        assert!(damaged(&base, &entries, &mut seen).is_empty());
        assert_eq!(seen.len(), 3, "every match is remembered with its size and date");

        // A file whose date is the remembered one is trusted without being read: its
        // content is changed here to the same size, then the date put back.
        let b = base.join("release/deps/b.rlib");
        let when = std::fs::metadata(&b).unwrap().modified().unwrap();
        std::fs::write(&b, "TWO").unwrap();
        std::fs::File::options().write(true).open(&b).unwrap().set_modified(when).unwrap();
        assert!(damaged(&base, &entries, &mut seen).is_empty(), "same size and date: not read");
        // The same change with a new date is read, and found.
        std::fs::File::options().write(true).open(&b).unwrap().set_modified(when + std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(damaged(&base, &entries, &mut seen), ["release/deps/b.rlib"]);
        std::fs::write(&b, "two").unwrap();
        assert!(damaged(&base, &entries, &mut seen).is_empty(), "restored content is accepted again");

        std::fs::write(base.join("release/deps/a.rlib"), "ONE").unwrap();
        std::fs::remove_file(base.join("c.txt")).unwrap();
        let mut bad = damaged(&base, &entries, &mut Seen::new());
        bad.sort();
        assert_eq!(bad, ["c.txt", "release/deps/a.rlib"]);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_release_attachment_is_named_for_the_version_system_and_processor() {
        let name = libs_asset_name("2.0.102");
        assert!(name.starts_with("PowerRustCOBOL-2.0.102-") && name.ends_with("-libs.tar.gz"), "{name}");
        // 2.0.467 — the tag every release has used comes first; the workflow's default `v` spelling second.
        let [bare, with_v] = libs_asset_urls("2.0.102");
        assert_eq!(bare, format!("{RELEASES_URL}/2.0.102/{name}"));
        assert_eq!(with_v, format!("{RELEASES_URL}/v2.0.102/{name}"));
        assert!(name.contains(std::env::consts::ARCH));
    }

    /// 2.0.467 — the log names both compilers, the cost, and the command that makes the libraries usable.
    #[test]
    fn libraries_from_another_compiler_are_explained_with_the_fix() {
        let theirs = "rustc 1.95.0 (59807616e 2026-04-14)\nbinary: rustc\nhost: x86_64-pc-windows-msvc\n";
        let said = other_compiler(theirs, Some("rustc 1.93.1 (01f6ddf75 2026-02-11)\nbinary: rustc\n"));
        assert!(said.contains("made by rustc 1.95.0 (59807616e 2026-04-14)"), "{said}");
        assert!(said.contains("this machine has rustc 1.93.1"), "{said}");
        assert!(said.contains("`rustup default 1.95.0`"), "{said}");
        assert!(other_compiler(theirs, None).contains("no rustc on the PATH"));
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
