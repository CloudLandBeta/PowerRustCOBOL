// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The compiled-crate cache every build shares (spec 101).
//!
//! Cargo compiles a crate once and compiles it again only when its source,
//! features, profile or toolchain change. A program used to keep that work in
//! a `target/` of its own, so every program compiled its ~500 crates — the
//! third-party ones, which no PowerRustCOBOL version can change, included.
//! Now every program builds into one `shared-target` folder in the build
//! folder, and a crate compiled for one program is reused by the next.
//!
//! What cargo cannot be trusted with is the *platform* crates (the fourteen
//! of [`crate::SDK_CRATES`]): it judges a path crate by file times, and an
//! SDK put back from an older copy carries files older than the last compile,
//! so cargo would link code compiled from sources that are no longer there.
//! Each platform crate therefore has a **marker** — a hash of its files and the
//! time it was written — and [`check_platform`] says which crates are *stale*:
//! different from what the cache compiled, yet not looking newer. Any such
//! crate makes the build a full one (the operator's rule, spec 101 R15).
//!
//! Everything here is a plain function over folders, so it is tested without
//! running cargo. The pieces that do run cargo ([`sdk_crates_in_graph`],
//! [`clean_platform`]) are the only ones that delete anything from the cache,
//! and they do it through cargo, which holds the cache's lock while it does.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use sha2::{Digest, Sha256};

/// The cache's folder, directly in the build folder (spec 100), beside the
/// programs' `cobolt-build-<name>` folders.
const CACHE_DIR: &str = "shared-target";
/// Where the platform markers live, inside the cache.
const MARKER_DIR: &str = "platform";

/// `<build folder>/shared-target`.
pub(crate) fn cache_dir(build_root: &Path) -> PathBuf {
    build_root.join(CACHE_DIR)
}

/// Create the cache and its marker folder. The reason it could not be used is
/// the answer, in words for the build's log (spec 101 R14).
pub(crate) fn open(build_root: &Path) -> Result<PathBuf, String> {
    let dir = cache_dir(build_root);
    std::fs::create_dir_all(dir.join(MARKER_DIR))
        .map_err(|e| format!("cannot create '{}': {e}", dir.display()))?;
    Ok(dir)
}

// ── The platform's state ─────────────────────────────────────────────────────

/// One platform crate as it is on disk now.
#[derive(Debug, Clone)]
pub(crate) struct CrateState {
    /// SHA-256 over its files, as lower-case hex.
    pub hash: String,
    /// The newest modification time among those files.
    pub newest: SystemTime,
}

/// Every file cargo could read for a crate: all of its folder but `target/`
/// and anything whose name starts with a dot (`.DS_Store`, editor droppings),
/// which come and go without the crate changing.
fn collect_files(dir: &Path, rel: &str, out: &mut Vec<(String, PathBuf)>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "target" || name.starts_with('.') {
            continue;
        }
        let relative = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_files(&entry.path(), &relative, out)?;
        } else if kind.is_file() {
            out.push((relative, entry.path()));
        }
    }
    Ok(())
}

/// The state of one platform crate under `workspace_root`, or `None` when the
/// SDK does not carry it. The icon `cobolt-form-host` bakes in lives outside
/// the crates (see [`crate::SDK_EXTRA_PATHS`]) and counts as one of its files.
pub(crate) fn crate_state(
    workspace_root: &Path,
    name: &'static str,
) -> std::io::Result<Option<CrateState>> {
    let dir = workspace_root.join("crates").join(name);
    if !dir.is_dir() {
        return Ok(None);
    }
    let mut files = Vec::new();
    collect_files(&dir, "", &mut files)?;
    if name == "cobolt-form-host" {
        let icon = workspace_root.join(crate::SDK_EXTRA_PATHS[0]);
        if icon.is_file() {
            files.push((format!("@{}", crate::SDK_EXTRA_PATHS[0]), icon));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    let mut newest = SystemTime::UNIX_EPOCH;
    for (relative, path) in &files {
        let bytes = std::fs::read(path)?;
        hasher.update(relative.as_bytes());
        hasher.update([0u8]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
        if let Ok(modified) = std::fs::metadata(path).and_then(|m| m.modified()) {
            newest = newest.max(modified);
        }
    }
    let hash = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    Ok(Some(CrateState { hash, newest }))
}

// ── Markers ──────────────────────────────────────────────────────────────────

fn marker_path(cache: &Path, name: &str) -> PathBuf {
    cache.join(MARKER_DIR).join(format!("{name}.sha256"))
}

/// What the cache last recorded for `name`: the hash, and when it was written.
pub(crate) fn read_marker(cache: &Path, name: &str) -> Option<(String, SystemTime)> {
    let path = marker_path(cache, name);
    let text = std::fs::read_to_string(&path).ok()?;
    let written = std::fs::metadata(&path).ok()?.modified().ok()?;
    Some((text.trim().to_owned(), written))
}

/// Record `hash` for `name`, replacing the old marker whole (a temporary file
/// renamed into place, so a reader never sees half of one).
pub(crate) fn write_marker(cache: &Path, name: &str, hash: &str) -> std::io::Result<()> {
    // Unique per call, not per process: two builds in one process (the IDE's Run
    // and Build) write the same markers at the same moment, and a shared
    // temporary name has one rename the other then cannot find.
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = marker_path(cache, name);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temporary = path.with_extension(format!("tmp-{}-{unique}", std::process::id()));
    std::fs::write(&temporary, format!("{hash}\n"))?;
    std::fs::rename(&temporary, &path)
}

/// Whether the cache holds a compiled unit of `name` in either profile. Cargo
/// names a unit's fingerprint folder `<crate>-<16 hex digits>`; the digits are
/// checked so `cobolt-forms` is never mistaken for `cobolt-forms-extra`.
pub(crate) fn has_artefacts(cache: &Path, name: &str) -> bool {
    ["debug", "release"].iter().any(|profile| {
        std::fs::read_dir(cache.join(profile).join(".fingerprint"))
            .into_iter()
            .flatten()
            .flatten()
            .any(|entry| is_unit_of(&entry.file_name().to_string_lossy(), name))
    })
}

fn is_unit_of(entry: &str, name: &str) -> bool {
    entry
        .strip_prefix(name)
        .and_then(|rest| rest.strip_prefix('-'))
        .is_some_and(|hash| !hash.is_empty() && hash.chars().all(|c| c.is_ascii_hexdigit()))
}

// ── The verdict ──────────────────────────────────────────────────────────────

/// What one platform crate needs before a build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// Its source is what the cache compiled.
    Fresh,
    /// Nothing to clean: record the hash and let cargo do its usual work.
    Record,
    /// Its source differs from what the cache compiled, but its files do not
    /// look newer than the record — cargo would believe the old units.
    Stale,
}

/// The decision, as a table (spec 101 plan §1):
///
/// | marker | hash | files newer than the marker | artefacts | verdict |
/// |---|---|---|---|---|
/// | same | same | — | — | Fresh |
/// | none | — | — | none | Record |
/// | none | — | — | some | Stale |
/// | differs | differs | — | none | Record |
/// | differs | differs | yes | some | Record |
/// | differs | differs | no | some | Stale |
pub(crate) fn verdict(
    marker: Option<(&str, SystemTime)>,
    hash: &str,
    newest: SystemTime,
    has_artefacts: bool,
) -> Verdict {
    match marker {
        Some((recorded, _)) if recorded == hash => Verdict::Fresh,
        Some((_, written)) => {
            if !has_artefacts || newest > written {
                Verdict::Record
            } else {
                Verdict::Stale
            }
        }
        None => {
            if has_artefacts {
                Verdict::Stale
            } else {
                Verdict::Record
            }
        }
    }
}

/// The platform as the cache sees it before a build.
#[derive(Debug, Default)]
pub(crate) struct PlatformCheck {
    /// Crates that look fresh to cargo and are not.
    pub stale: Vec<&'static str>,
    /// Crates whose marker is to be written now, with the hash.
    pub record: Vec<(&'static str, String)>,
    /// Every crate's current hash, for the markers written after a clean.
    pub hashes: Vec<(&'static str, String)>,
}

/// Judge every platform crate under `workspace_root` against `cache`.
pub(crate) fn check_platform(workspace_root: &Path, cache: &Path) -> std::io::Result<PlatformCheck> {
    let mut check = PlatformCheck::default();
    for name in crate::SDK_CRATES {
        let Some(state) = crate_state(workspace_root, name)? else {
            continue;
        };
        let marker = read_marker(cache, name);
        // Only a crate that differs from its record needs the cache looked at.
        let differs = marker.as_ref().is_none_or(|(recorded, _)| *recorded != state.hash);
        let artefacts = differs && has_artefacts(cache, name);
        let decided = verdict(
            marker.as_ref().map(|(recorded, written)| (recorded.as_str(), *written)),
            &state.hash,
            state.newest,
            artefacts,
        );
        match decided {
            Verdict::Fresh => {}
            Verdict::Record => check.record.push((name, state.hash.clone())),
            Verdict::Stale => check.stale.push(name),
        }
        check.hashes.push((name, state.hash));
    }
    Ok(check)
}

// ── A program's lock ─────────────────────────────────────────────────────────

/// Start a program's `Cargo.lock` from the SDK's, so two programs resolve the
/// same version of every third-party crate they share (spec 101 R4). Only a
/// program with no lock of its own is seeded; the answer is whether it was.
pub(crate) fn seed_lock(workspace_root: &Path, build_dir: &Path) -> bool {
    let target = build_dir.join("Cargo.lock");
    if target.exists() {
        return false;
    }
    let source = workspace_root.join("Cargo.lock");
    if !source.is_file() {
        return false;
    }
    if std::fs::copy(&source, &target).is_ok() {
        return true;
    }
    // A copy that stopped halfway must not be left for cargo to choke on.
    let _ = std::fs::remove_file(&target);
    false
}

// ── Cargo's own words ────────────────────────────────────────────────────────

/// Whether `line` of cargo's stderr says it is waiting for another cargo to
/// release a lock. Cargo 1.95 names the folder `artifact directory`; older
/// ones said `build directory`, so only the start of the sentence is matched.
pub(crate) fn is_lock_wait_line(line: &str) -> bool {
    line.trim_start().starts_with("Blocking waiting for file lock")
}

/// The platform crates named in `cargo tree --prefix none -f "{p}"` output
/// (`cobolt-ast v0.2.0 (/path)` per line), in [`crate::SDK_CRATES`] order.
pub(crate) fn sdk_crates_in_tree_output(output: &str) -> Vec<&'static str> {
    crate::SDK_CRATES
        .iter()
        .copied()
        .filter(|name| output.lines().any(|line| line.split_whitespace().next() == Some(name)))
        .collect()
}

/// A byte count in words for the build's log: `310 MB`, `1.4 GB`.
pub(crate) fn human_size(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    let b = bytes as f64;
    if b >= 1024.0 * MB {
        format!("{:.1} GB", b / (1024.0 * MB))
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else {
        format!("{:.0} KB", b / 1024.0)
    }
}

// ── The two things that run cargo ────────────────────────────────────────────

/// The last lines of cargo's complaint, for a log line.
fn tail(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(2)..].join(" / ")
}

/// The platform crates in the dependency graph of the program staged in
/// `build_dir`. Cargo is asked, offline, because only it knows which features
/// switch which crate on; as a side effect it prunes the program's lock to what
/// the program uses. Nothing is compiled and nothing is deleted.
pub(crate) fn sdk_crates_in_graph(build_dir: &Path, cache: &Path) -> Result<Vec<&'static str>, String> {
    let out = Command::new("cargo")
        .args(["tree", "--offline", "--prefix", "none", "-e", "normal,build", "-f", "{p}"])
        .current_dir(build_dir)
        .env("CARGO_TARGET_DIR", cache)
        .output()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if !out.status.success() {
        return Err(format!("cargo tree failed: {}", tail(&out.stderr)));
    }
    Ok(sdk_crates_in_tree_output(&String::from_utf8_lossy(&out.stdout)))
}

/// Drop every compiled unit of `crates` from the cache, in both profiles,
/// through `cargo clean -p` run in the program's own folder. `crates` must all
/// be in that program's graph ([`sdk_crates_in_graph`]) or cargo refuses.
/// Cargo removes every variant of each crate and nothing else, and holds the
/// cache's lock while it does, so a build running at the same time is safe.
pub(crate) fn clean_platform(build_dir: &Path, cache: &Path, crates: &[&str]) -> Result<(), String> {
    if crates.is_empty() {
        return Ok(());
    }
    for release in [false, true] {
        let mut command = Command::new("cargo");
        command.arg("clean");
        for name in crates {
            command.args(["-p", name]);
        }
        if release {
            command.arg("--release");
        }
        let out = command
            .arg("--offline")
            .current_dir(build_dir)
            .env("CARGO_TARGET_DIR", cache)
            .output()
            .map_err(|e| format!("cannot run cargo: {e}"))?;
        if !out.status.success() {
            return Err(format!("cargo clean failed: {}", tail(&out.stderr)));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("prc-cache-{tag}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A workspace with `cobolt-ast` and `cobolt-lexer`, each of two files.
    fn fake_workspace(root: &Path) {
        for name in ["cobolt-ast", "cobolt-lexer"] {
            let src = root.join("crates").join(name).join("src");
            std::fs::create_dir_all(&src).unwrap();
            std::fs::write(src.join("lib.rs"), format!("pub fn {}() {{}}", name.replace('-', "_"))).unwrap();
            std::fs::write(root.join("crates").join(name).join("Cargo.toml"), "[package]\n").unwrap();
        }
    }

    fn set_modified(path: &Path, when: SystemTime) {
        std::fs::File::options().write(true).open(path).unwrap().set_modified(when).unwrap();
    }

    #[test]
    fn the_hash_ignores_order_target_and_dot_files_and_sees_one_changed_byte() {
        let root = temp_dir("hash");
        fake_workspace(&root);
        let before = crate_state(&root, "cobolt-ast").unwrap().unwrap();

        // What cargo or the system leaves in a crate's folder is not the crate.
        let ast = root.join("crates/cobolt-ast");
        std::fs::create_dir_all(ast.join("target/debug")).unwrap();
        std::fs::write(ast.join("target/debug/junk.rlib"), b"x").unwrap();
        std::fs::write(ast.join(".DS_Store"), b"x").unwrap();
        std::fs::write(ast.join("src/.hidden"), b"x").unwrap();
        let ignored = crate_state(&root, "cobolt-ast").unwrap().unwrap();
        assert_eq!(before.hash, ignored.hash, "target/ and dot-files must not count");

        // The same files written in the other order hash the same.
        let other = temp_dir("hash-order");
        fake_workspace(&other);
        assert_eq!(before.hash, crate_state(&other, "cobolt-ast").unwrap().unwrap().hash);

        // One changed byte does.
        std::fs::write(ast.join("src/lib.rs"), "pub fn cobolt_asT() {}").unwrap();
        let changed = crate_state(&root, "cobolt-ast").unwrap().unwrap();
        assert_ne!(before.hash, changed.hash, "one byte must change the hash");
        assert_eq!(before.hash.len(), 64);

        // A crate the SDK does not carry is not an error.
        assert!(crate_state(&root, "cobolt-media").unwrap().is_none());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&other);
        println!("hash: 64 hex digits; target/, dot-files and file order ignored; 1 changed byte seen");
    }

    #[test]
    fn the_verdict_follows_the_table() {
        let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let written = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000);
        let new = SystemTime::UNIX_EPOCH + Duration::from_secs(3_000);
        let m = |h: &'static str| Some((h, written));
        // (row, marker, hash, newest, artefacts, expected)
        let rows: [(&str, Option<(&str, SystemTime)>, &str, SystemTime, bool, Verdict); 7] = [
            ("same marker and hash", m("a"), "a", old, true, Verdict::Fresh),
            ("no marker, nothing compiled", None, "a", old, false, Verdict::Record),
            ("no marker, something compiled", None, "a", old, true, Verdict::Stale),
            ("changed, nothing compiled", m("b"), "a", old, false, Verdict::Record),
            ("changed, files newer", m("b"), "a", new, true, Verdict::Record),
            ("changed, files older", m("b"), "a", old, true, Verdict::Stale),
            ("changed, files as old as the record", m("b"), "a", written, true, Verdict::Stale),
        ];
        println!("{:<38} verdict", "case");
        for (label, marker, hash, newest, artefacts, expected) in rows {
            let got = verdict(marker, hash, newest, artefacts);
            println!("{label:<38} {got:?}");
            assert_eq!(got, expected, "{label}");
        }
        println!("verdict table: 7 of 7 rows as specified");
    }

    #[test]
    fn artefacts_are_matched_by_whole_crate_names() {
        let cache = temp_dir("artefacts");
        let fp = cache.join("debug/.fingerprint");
        std::fs::create_dir_all(fp.join("cobolt-form-host-0123456789abcdef")).unwrap();
        std::fs::create_dir_all(fp.join("cobolt-forms-extra-0123456789abcdef")).unwrap();
        assert!(has_artefacts(&cache, "cobolt-form-host"));
        assert!(!has_artefacts(&cache, "cobolt-forms"), "cobolt-forms is not cobolt-forms-extra");
        assert!(!has_artefacts(&cache, "cobolt-ast"));
        std::fs::create_dir_all(cache.join("release/.fingerprint/cobolt-ast-fedcba9876543210")).unwrap();
        assert!(has_artefacts(&cache, "cobolt-ast"), "either profile counts");
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn a_marker_is_replaced_whole() {
        let build_root = temp_dir("marker");
        let cache = open(&build_root).unwrap();
        assert!(read_marker(&cache, "cobolt-ast").is_none());
        write_marker(&cache, "cobolt-ast", "aaaa").unwrap();
        assert_eq!(read_marker(&cache, "cobolt-ast").unwrap().0, "aaaa");
        write_marker(&cache, "cobolt-ast", "bbbb").unwrap();
        assert_eq!(read_marker(&cache, "cobolt-ast").unwrap().0, "bbbb");
        let leftovers = std::fs::read_dir(cache.join("platform")).unwrap().count();
        assert_eq!(leftovers, 1, "no temporary file may stay beside the marker");
        let _ = std::fs::remove_dir_all(&build_root);
    }

    #[test]
    fn the_cache_cannot_be_opened_where_a_file_stands() {
        let build_root = temp_dir("blocked");
        std::fs::write(build_root.join(CACHE_DIR), b"not a folder").unwrap();
        let why = open(&build_root).unwrap_err();
        assert!(why.contains("shared-target"), "the reason names the folder: {why}");
        let _ = std::fs::remove_dir_all(&build_root);
    }

    #[test]
    fn a_lock_is_seeded_only_when_the_program_has_none() {
        let sdk = temp_dir("seed-sdk");
        let program = temp_dir("seed-prog");
        std::fs::write(sdk.join("Cargo.lock"), "sdk lock").unwrap();
        assert!(seed_lock(&sdk, &program), "a program without a lock is seeded");
        assert_eq!(std::fs::read_to_string(program.join("Cargo.lock")).unwrap(), "sdk lock");
        std::fs::write(program.join("Cargo.lock"), "its own").unwrap();
        assert!(!seed_lock(&sdk, &program), "a lock the program has is kept");
        assert_eq!(std::fs::read_to_string(program.join("Cargo.lock")).unwrap(), "its own");
        let bare = temp_dir("seed-bare");
        let other = temp_dir("seed-other");
        assert!(!seed_lock(&bare, &other), "an SDK with no lock seeds nothing");
        assert!(!other.join("Cargo.lock").exists());
        for d in [sdk, program, bare, other] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn sizes_are_said_in_words() {
        assert_eq!(human_size(1_503_238_554), "1.4 GB");
        assert_eq!(human_size(325_058_560), "310 MB");
        assert_eq!(human_size(2_048), "2 KB");
    }

    #[test]
    fn the_lock_wait_line_of_cargo_1_95_is_recognised() {
        // Captured from cargo 1.95.0 on 2026-10-15, two builds on one cache.
        assert!(is_lock_wait_line("    Blocking waiting for file lock on artifact directory"));
        assert!(is_lock_wait_line("    Blocking waiting for file lock on build directory"));
        assert!(!is_lock_wait_line("   Compiling serde v1.0.228"));
    }

    #[test]
    fn the_platform_crates_are_read_out_of_cargo_trees_output() {
        let out = "\
cobolt-ast v0.2.0 (/sdk/crates/cobolt-ast)
cobolt-lexer v0.2.0 (/sdk/crates/cobolt-lexer)
serde v1.0.228
cobolt-forms v0.2.0 (/sdk/crates/cobolt-forms)
cobolt-ast v0.2.0 (/sdk/crates/cobolt-ast) (*)
cobolt-forms-extra v1.0.0
";
        assert_eq!(
            sdk_crates_in_tree_output(out),
            vec!["cobolt-ast", "cobolt-forms", "cobolt-lexer"],
            "SDK order, no duplicates, whole names only"
        );
    }

    #[test]
    fn the_check_follows_a_crate_from_new_to_stale() {
        let root = temp_dir("check-ws");
        let build_root = temp_dir("check-cache");
        fake_workspace(&root);
        let cache = open(&build_root).unwrap();

        // A cache with nothing in it: both crates are recorded, none stale.
        let first = check_platform(&root, &cache).unwrap();
        assert!(first.stale.is_empty());
        assert_eq!(first.record.len(), 2);
        for (name, hash) in &first.record {
            write_marker(&cache, name, hash).unwrap();
        }

        // Unchanged: nothing to do.
        let second = check_platform(&root, &cache).unwrap();
        assert!(second.stale.is_empty() && second.record.is_empty());

        // The cache has compiled both. Then lexer's source changes and its file
        // looks OLD — an SDK put back from an older copy.
        for name in ["cobolt-ast", "cobolt-lexer"] {
            std::fs::create_dir_all(cache.join(format!("debug/.fingerprint/{name}-0123456789abcdef"))).unwrap();
        }
        let lib = root.join("crates/cobolt-lexer/src/lib.rs");
        std::fs::write(&lib, "pub fn from_an_older_copy() {}").unwrap();
        set_modified(&lib, SystemTime::now() - Duration::from_secs(86_400));
        let third = check_platform(&root, &cache).unwrap();
        assert_eq!(third.stale, vec!["cobolt-lexer"], "old-looking changed source is stale");

        // The same change with a NEW file time is cargo's to notice.
        set_modified(&lib, SystemTime::now() + Duration::from_secs(60));
        let fourth = check_platform(&root, &cache).unwrap();
        assert!(fourth.stale.is_empty());
        assert_eq!(fourth.record.len(), 1);
        assert_eq!(fourth.record[0].0, "cobolt-lexer");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&build_root);
        println!("check: new -> recorded (2), unchanged -> nothing, old-looking change -> stale (1), newer change -> recorded (1)");
    }
}
