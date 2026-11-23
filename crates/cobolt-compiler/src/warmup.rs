// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The platform's compiled libraries, made once per PowerRustCOBOL version, when a program needs them.
//!
//! Most programs are built by the WebAssembly runner and compile nothing of the platform. A program
//! that needs Cargo (a Debug build, External Crates, EXEC RUST) links the platform's crates, and a build
//! of it is its own few seconds only when the crates under it are already compiled in the shared cache
//! ([`crate::build_cache`]). This module is what puts them there, and what judges, before the first such
//! build, how long that will take ([`platform_prep`]), so the IDE can ask first:
//!
//! 1. **Shipped libraries.** A release carries `libs/<os>/<arch>/target`, the cache a release build
//!    produced ([`produce_libs`]). The first Cargo-path build copies it into the cache ([`seed_from_shipped`])
//!    — when the compiler that made it is the compiler used here, since another `rustc` cannot read it.
//! 2. **The platform's own crates.** Their cargo identity includes the folder the SDK is installed in, so
//!    they are compiled here, once, by the program's own build.
//!
//! `warm/<version>` in the cache records that a build compiled them, so the next build of the same
//! version waits for nothing. Nothing compiles at start-up: until 2.1.19 the IDE compiled three stand-in
//! programs in the background on the first start of a version, which held the cache's lock against the
//! developer's own first build and kept every core busy for as long as a first build takes.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use crate::runtime_features::RuntimeFeatures;
use crate::{build_cache, generate_cargo_toml, resolve_build_root};

pub mod prepare;
use prepare::{CargoBuild, CargoRunner, Ended, Flow, Priority, Profile, SystemCargo, Tick};

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

/// The workspace manifest this compiler was built from. `cobolt-compiler` is not one of the
/// crates the SDK ships, so reading it here costs an installed SDK nothing.
const WORKSPACE_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml"));

/// The Rust release every PowerRustCOBOL release is built with — `release-rust` under
/// `[workspace.metadata.powerrustcobol]` in the workspace manifest, and the release workflow's
/// `RELEASE_RUST` (2.0.469). The libraries a release ships can only be used by this compiler, so
/// the IDE installs it and the Build names it. `"stable"` only if the manifest stops saying,
/// which a test forbids.
pub fn release_rust() -> &'static str {
    static PINNED: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    PINNED.get_or_init(|| pinned_rust(WORKSPACE_MANIFEST).unwrap_or_else(|| "stable".to_owned()))
}

fn pinned_rust(manifest: &str) -> Option<String> {
    let doc: toml::Value = toml::from_str(manifest).ok()?;
    let pinned = doc.get("workspace")?.get("metadata")?.get("powerrustcobol")?.get("release-rust")?;
    pinned.as_str().map(str::to_owned)
}

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

/// The stamp a build of `version` leaves in `warm/` for the profile it compiled: the debug and the
/// release platforms are compiled separately, and only the release one ever shipped.
fn stamp_name(version: &str, debug: bool) -> String {
    if debug { format!("{version}-debug") } else { version.to_owned() }
}

/// A build that got this far compiled whatever the platform lacked, for the profile it built.
pub(crate) fn mark_current_warm(cache: &Path, debug: bool) {
    if let Some(v) = CURRENT.get() {
        let _ = mark_warm(cache, v, debug);
    }
}

/// Write the stamp that says `version`'s platform is compiled in `cache` for the profile. The setup's
/// preparation (spec 112) reports a failure to write it: a profile it says it prepared must have its stamp.
pub(crate) fn mark_warm(cache: &Path, version: &str, debug: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(cache.join(WARM_DIR))?;
    std::fs::write(cache.join(WARM_DIR).join(stamp_name(version, debug)), version)
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
    let out = tool("rustc").arg("-vV").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// A tool of the Rust toolchain (`cargo`, `rustc`), asked for the release PowerRustCOBOL is built with
/// ([`release_rust`]) whenever `rustup` has it installed, whatever the machine's default is. The libraries
/// a release ships can only be used by that compiler, and a developer's default is often a newer stable:
/// installing the release beside it is enough, and nothing about their other projects changes.
/// `RUSTUP_TOOLCHAIN` outranks the default and any `rust-toolchain` file, and means nothing without rustup.
pub(crate) fn tool(program: &str) -> Command {
    let mut command = Command::new(program);
    if let Some(release) = pinned_toolchain() {
        command.env("RUSTUP_TOOLCHAIN", release);
    }
    command
}

/// [`release_rust`] when `rustup` lists it as installed. Only a yes is remembered, so installing it
/// while the IDE is open takes effect at the next build.
fn pinned_toolchain() -> Option<&'static str> {
    static FOUND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    use std::sync::atomic::Ordering::Relaxed;
    let release = release_rust();
    if !FOUND.load(Relaxed) {
        let out = Command::new("rustup").args(["toolchain", "list"]).output().ok().filter(|o| o.status.success())?;
        if !lists_toolchain(&String::from_utf8_lossy(&out.stdout), release) {
            return None;
        }
        FOUND.store(true, Relaxed);
    }
    Some(release)
}

/// Whether the output of `rustup toolchain list` (`1.95.0-aarch64-apple-darwin (default)`) names `release`.
pub fn lists_toolchain(list: &str, release: &str) -> bool {
    list.lines().any(|line| line.trim().strip_prefix(release).is_some_and(|rest| rest.starts_with('-')))
}

/// What a Build that takes the Cargo path has to do to the platform before it can compile the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformPrep {
    /// The platform is compiled in the cache for this version and profile: nothing to wait for.
    Ready,
    /// The third-party libraries are in the cache or shipped for this compiler: PowerRustCOBOL's own crates
    /// are compiled here, once. Minutes.
    OwnCrates,
    /// Every crate is compiled here, once. On a modest machine, an hour or more.
    Everything(Why),
}

/// Why the third-party libraries cannot be reused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Why {
    /// A Debug build: only release libraries are shipped.
    DebugProfile,
    /// The shipped libraries were made by another compiler than the one this machine would use.
    OtherCompiler {
        /// The Rust release PowerRustCOBOL is built with, which makes them usable.
        release: String,
        /// What `rustc -V` says here, or that there is none.
        here: String,
    },
    /// This installation has no libraries for this machine.
    NoLibraries,
}

/// What a Cargo-path build of `version` has to compile before it reaches the program, judged from the
/// shared `cache` and the SDK's shipped libraries, without compiling anything.
pub fn platform_prep(sdk_root: &Path, cache: &Path, version: &str, debug: bool) -> PlatformPrep {
    let ready = cache.join(WARM_DIR).join(stamp_name(version, debug)).is_file();
    let populated = cache.join(if debug { "debug" } else { "release" }).join(".fingerprint").is_dir();
    let libs = libs_dir(sdk_root);
    let shipped = libs.join("target").is_dir().then(|| std::fs::read_to_string(libs.join(RUSTC_STAMP)).unwrap_or_default());
    judge(ready, populated, debug, shipped.as_deref(), || rustc_version())
}

/// [`platform_prep`] from the facts: whether this version's stamp is there, whether the cache already holds
/// compiled crates of the profile, `shipped` (what compiler made the shipped libraries, when there are any)
/// and, asked only when it matters, this machine's `rustc -vV`.
fn judge(ready: bool, populated: bool, debug: bool, shipped: Option<&str>, ours: impl FnOnce() -> Option<String>) -> PlatformPrep {
    if ready {
        return PlatformPrep::Ready;
    }
    if populated {
        return PlatformPrep::OwnCrates;
    }
    if debug {
        return PlatformPrep::Everything(Why::DebugProfile);
    }
    let Some(theirs) = shipped else {
        return PlatformPrep::Everything(Why::NoLibraries);
    };
    match ours() {
        Some(here) if here.trim() == theirs.trim() => PlatformPrep::OwnCrates,
        here => PlatformPrep::Everything(Why::OtherCompiler {
            release: release_rust().to_owned(),
            here: here.as_deref().and_then(|h| h.lines().next()).map(str::trim).unwrap_or("").to_owned(),
        }),
    }
}

/// Put the libraries the release shipped into the cache, when the cache holds no compiled crates yet and
/// this machine's compiler is the one that made them. `Ok(true)` when it copied. The platform's own crates
/// are not among them (see [`produce_libs`]): the program's build compiles those.
pub(crate) fn seed_from_shipped(sdk_root: &Path, cache: &Path, log: &mut dyn FnMut(&str)) -> Result<bool, String> {
    let shipped = libs_dir(sdk_root).join("target");
    if !shipped.is_dir() || cache.join("release").join(".fingerprint").is_dir() {
        return Ok(false);
    }
    let theirs = std::fs::read_to_string(libs_dir(sdk_root).join(RUSTC_STAMP)).unwrap_or_default();
    match rustc_version() {
        Some(ours) if ours.trim() == theirs.trim() => {
            log(&format!("   using the libraries shipped in {}", libs_dir(sdk_root).display()));
            copy_tree(&shipped, cache).map_err(|e| format!("cannot copy the shipped libraries: {e}"))?;
            Ok(true)
        }
        ours => {
            log(&other_compiler(&theirs, ours.as_deref()));
            Ok(false)
        }
    }
}

/// One stand-in program to compile: what it links, where it is staged, which cache it compiles into and in
/// which profile.
pub(crate) struct Stand<'a> {
    pub sdk_root: &'a Path,
    /// Where the stand-in is staged (a `Cargo.toml` and a `main.rs`).
    pub dir: &'a Path,
    pub cache: &'a Path,
    pub features: RuntimeFeatures,
    pub check: &'a build_cache::PlatformCheck,
    pub profile: Profile,
    pub priority: Priority,
}

/// How a stand-in's compile ended, when it did not fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Built {
    Done,
    /// The caller stopped it ([`Flow::Stop`]).
    Stopped,
}

/// The stale platform crates this stand-in has still to clean: the ones in its graph that no earlier stand-in
/// of the run has cleaned. Cleaning drops a crate's compiled units in BOTH profiles, so a crate cleaned for one
/// stand-in and compiled again must not be cleaned again for the next, or the second stand-in would throw away
/// what the first compiled.
pub(crate) fn to_clean(
    stale: &[&'static str],
    graph: &[&'static str],
    cleaned: &std::collections::HashSet<&'static str>,
) -> Vec<&'static str> {
    stale.iter().copied().filter(|c| graph.contains(c) && !cleaned.contains(c)).collect()
}

/// Compile one stand-in into the cache, through `runner`, telling `on` what cargo says and stopping when it
/// answers [`Flow::Stop`]. Offline first, and online when that fails (a cold crate cache), as every Build does.
pub(crate) fn build_variant(
    stand: &Stand<'_>,
    cleaned: &mut std::collections::HashSet<&'static str>,
    runner: &mut dyn CargoRunner,
    on: &mut dyn FnMut(Tick) -> Flow,
) -> Result<Built, String> {
    let Stand { sdk_root, dir, cache, features, check, profile, priority } = *stand;
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

    if check.stale.iter().any(|c| !cleaned.contains(c)) {
        let graph = build_cache::sdk_crates_in_graph(dir, cache)?;
        let stale = to_clean(&check.stale, &graph, cleaned);
        build_cache::clean_platform(dir, cache, &stale)?;
        // A cleaned crate is recorded as it is now, as a Build does, so the next Build finds it fresh.
        for name in &stale {
            cleaned.insert(name);
            if let Some((_, hash)) = check.hashes.iter().find(|(n, _)| n == name) {
                build_cache::write_marker(cache, name, hash).map_err(|e| format!("cannot record {name}: {e}"))?;
            }
        }
    }

    let mut last_tail = Vec::new();
    for offline in [true, false] {
        match runner.build(&CargoBuild { dir, cache, profile, offline, priority }, on) {
            Ended::Success => return Ok(Built::Done),
            Ended::Stopped => return Ok(Built::Stopped),
            Ended::NotRun(why) => return Err(format!("cannot run cargo: {why}")),
            Ended::Failed { tail, .. } => last_tail = tail,
        }
    }
    Err(format!("cargo build failed: {}", last_tail.join("\n")))
}

/// Build the platform for `libs_dir(sdk_root)` — what the release workflow runs
/// after staging the SDK. The result is `target/` stripped of what a user's
/// machine cannot use (incremental state, build-script scratch space) and
/// the `rustc.txt` that [`seed_from_shipped`] checks before copying it.
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
    let mut cleaned = std::collections::HashSet::new();
    for (label, features) in variants() {
        log(&format!("compiling {label}…"));
        let dir = scratch.join(label);
        let stand = Stand {
            sdk_root,
            dir: &dir,
            cache: &target,
            features,
            check: &check,
            profile: Profile::Release,
            priority: Priority::BelowNormal,
        };
        build_variant(&stand, &mut cleaned, &mut SystemCargo, &mut |_| Flow::Go)?;
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
         as long as a build from nothing (on a modest machine, an hour or more). PowerRustCOBOL is \
         built with Rust {release}: to use them, run `rustup toolchain install {release}` (a few minutes; \
         your default Rust is not changed) and build again."
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

    /// **2.0.469 — one Rust release, pinned in one place and obeyed everywhere.** The manifest names it,
    /// the release workflow builds every package with it (never a floating `stable`), its installer
    /// texts name it, and the code's own floor (`rust-version`) is no higher than it.
    #[test]
    fn the_release_rust_is_pinned_everywhere() {
        let pinned = release_rust();
        let parts: Vec<u32> = pinned.split('.').map(|p| p.parse().expect("a numbered release")).collect();
        assert_eq!(parts.len(), 3, "a full release number, not {pinned}");
        let workflow = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.github/workflows/build-all-platforms.yml"));
        assert!(workflow.contains(&format!("RELEASE_RUST: \"{pinned}\"")), "the workflow does not pin {pinned}");
        assert!(!workflow.contains("rust-toolchain@stable"), "a job still builds with a floating stable");
        let uses = workflow.matches("dtolnay/rust-toolchain@").count();
        let pinned_uses = workflow.matches("toolchain: ${{ env.RELEASE_RUST }}").count();
        assert_eq!(uses, pinned_uses, "every toolchain step names RELEASE_RUST");
        assert!(!workflow.contains("sh.rustup.rs | sh\n"), "an installer text installs an unpinned Rust");
        let floor = WORKSPACE_MANIFEST
            .lines()
            .find_map(|l| l.trim().strip_prefix("rust-version"))
            .and_then(|l| l.split('"').nth(1))
            .expect("the workspace declares rust-version");
        let floor: Vec<u32> = floor.split('.').map(|p| p.parse().unwrap()).collect();
        assert!(floor[..] <= parts[..floor.len()], "rust-version {floor:?} is above the pinned {pinned}");
        assert_eq!(pinned_rust("[workspace]\nmembers = []\n"), None);
    }

    /// **2.0.539 — the Linux packages name what the IDE needs to start.** The real ALSA library
    /// comes first and the old name carries a version (Ubuntu 24.04's OSS stand-in provides
    /// `libasound2` with no version and lacks the ALSA symbols), and the libraries the IDE opens
    /// at start, which no dependency scan sees, are in both the `.deb` and the `.rpm`.
    #[test]
    fn the_linux_packages_declare_what_the_ide_opens_at_start() {
        let workflow = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.github/workflows/build-all-platforms.yml"));
        let depends: Vec<&str> = workflow
            .lines()
            .find_map(|l| l.trim().strip_prefix("Depends: "))
            .expect("the .deb has a Depends line")
            .split(',')
            .map(str::trim)
            .collect();
        assert_eq!(depends[0], "libasound2t64 | libasound2 (>= 1.0.16)", "the real ALSA library first, the old name versioned");
        let opened = ["libegl1", "libgl1-mesa-dri", "libx11-6", "libx11-xcb1", "libxcursor1", "libxi6", "libxkbcommon0", "libxkbcommon-x11-0"];
        for package in opened {
            assert!(depends.contains(&package), "the .deb does not depend on {package}");
        }
        let requires: Vec<&str> =
            workflow.lines().filter_map(|l| l.trim().strip_prefix("Requires:")).map(str::trim).collect();
        let sonames = ["libasound.so.2", "libEGL.so.1", "libX11.so.6", "libX11-xcb.so.1", "libXcursor.so.1", "libXi.so.6", "libxkbcommon.so.0", "libxkbcommon-x11.so.0"];
        for soname in sonames {
            assert!(requires.contains(&format!("{soname}()(64bit)").as_str()), "the .rpm does not require {soname}");
        }
        println!(".deb: ALSA first and versioned + {} opened libraries; .rpm: {} sonames", opened.len(), sonames.len());
    }

    /// 2.0.467 — the log names both compilers, the cost, and the command that makes the libraries usable.
    #[test]
    fn libraries_from_another_compiler_are_explained_with_the_fix() {
        let theirs = "rustc 1.95.0 (59807616e 2026-04-14)\nbinary: rustc\nhost: x86_64-pc-windows-msvc\n";
        let said = other_compiler(theirs, Some("rustc 1.93.1 (01f6ddf75 2026-02-11)\nbinary: rustc\n"));
        assert!(said.contains("made by rustc 1.95.0 (59807616e 2026-04-14)"), "{said}");
        assert!(said.contains("this machine has rustc 1.93.1"), "{said}");
        assert!(said.contains("`rustup toolchain install 1.95.0`"), "{said}");
        assert!(other_compiler(theirs, None).contains("no rustc on the PATH"));
    }

    /// **2.1.19 — the Build asks before it compiles the platform, and says how much.** What the cache and the
    /// shipped libraries hold decides it, with nothing run but one `rustc -vV`, asked only when it matters.
    #[test]
    fn the_prep_is_judged_from_the_cache_and_the_shipped_libraries() {
        let made = "rustc 1.95.0 (59807616e 2026-04-14)\nbinary: rustc\n";
        let never = || -> Option<String> { panic!("the compiler is only asked when shipped libraries could be used") };
        // The version's stamp is there: nothing to wait for, whatever else is true.
        assert_eq!(judge(true, false, true, None, never), PlatformPrep::Ready);
        // A cache that holds compiled crates leaves only PowerRustCOBOL's own; the shipped ones are not looked at.
        assert_eq!(judge(false, true, false, Some(made), never), PlatformPrep::OwnCrates);
        assert_eq!(judge(false, true, true, None, never), PlatformPrep::OwnCrates);
        // A Debug build with an empty cache: nothing was ever shipped for it.
        assert_eq!(judge(false, false, true, Some(made), never), PlatformPrep::Everything(Why::DebugProfile));
        // No libraries shipped for this machine.
        assert_eq!(judge(false, false, false, None, never), PlatformPrep::Everything(Why::NoLibraries));
        // Shipped libraries and the compiler that made them: only the platform's own crates.
        assert_eq!(judge(false, false, false, Some(made), || Some(format!("{made}\n"))), PlatformPrep::OwnCrates);
        // Another compiler: everything, and the words say which one is here and which one is needed.
        let other = judge(false, false, false, Some(made), || Some("rustc 1.93.1 (01f6ddf75 2026-02-11)\nbinary: rustc".into()));
        assert_eq!(
            other,
            PlatformPrep::Everything(Why::OtherCompiler { release: release_rust().to_owned(), here: "rustc 1.93.1 (01f6ddf75 2026-02-11)".into() })
        );
        let none = judge(false, false, false, Some(made), || None);
        assert!(matches!(none, PlatformPrep::Everything(Why::OtherCompiler { ref here, .. }) if here.is_empty()), "{none:?}");
    }

    /// 2.1.19 — the release and the debug platforms are stamped apart: a debug Build compiling its platform
    /// does not make the release one look compiled, and the other way round.
    #[test]
    fn a_debug_build_leaves_its_own_stamp() {
        assert_eq!(stamp_name("2.1.19", false), "2.1.19");
        assert_eq!(stamp_name("2.1.19", true), "2.1.19-debug");
        let cache = std::env::temp_dir().join(format!("prc-prep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cache);
        let sdk = cache.join("no-sdk");
        let stamp = |name: &str| {
            std::fs::create_dir_all(cache.join(WARM_DIR)).unwrap();
            std::fs::write(cache.join(WARM_DIR).join(name), "x").unwrap();
        };
        assert_eq!(platform_prep(&sdk, &cache, "2.1.19", false), PlatformPrep::Everything(Why::NoLibraries));
        assert_eq!(platform_prep(&sdk, &cache, "2.1.19", true), PlatformPrep::Everything(Why::DebugProfile));
        stamp("2.1.19-debug");
        assert_eq!(platform_prep(&sdk, &cache, "2.1.19", true), PlatformPrep::Ready);
        assert_eq!(platform_prep(&sdk, &cache, "2.1.19", false), PlatformPrep::Everything(Why::NoLibraries), "release is not stamped");
        stamp("2.1.19");
        assert_eq!(platform_prep(&sdk, &cache, "2.1.19", false), PlatformPrep::Ready);
        assert_eq!(platform_prep(&sdk, &cache, "2.1.20", false), PlatformPrep::Everything(Why::NoLibraries), "a new version asks again");
        let _ = std::fs::remove_dir_all(&cache);
    }

    /// 2.1.19 — the toolchain `rustup` lists is read by its release number and a dash, so `1.95.0` is not
    /// found in `1.95.01` or `nightly-1.95.0`, and a default marker or spaces around it change nothing.
    #[test]
    fn the_pinned_toolchain_is_found_in_the_rustup_list() {
        let list = "stable-aarch64-apple-darwin (default)\n1.95.0-aarch64-apple-darwin\nnightly-aarch64-apple-darwin\n";
        assert!(lists_toolchain(list, "1.95.0"));
        assert!(lists_toolchain("  1.95.0-x86_64-pc-windows-msvc (default)  \n", "1.95.0"));
        assert!(!lists_toolchain(list, "1.94.0"));
        assert!(!lists_toolchain("1.95.01-x86_64-pc-windows-msvc\n", "1.95.0"));
        assert!(!lists_toolchain("nightly-1.95.0-x86_64-pc-windows-msvc\n", "1.95.0"));
        assert!(!lists_toolchain("", "1.95.0"));
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
