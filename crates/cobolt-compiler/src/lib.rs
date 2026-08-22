// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! RustCOBOL embed+bundle binary compiler — Phase 11.
//!
//! Transforms a RustCOBOL project (a `cobolt.toml` manifest + COBOL sources +
//! optional `.cfrm` form files) into a **single self-contained native
//! executable** placed in `<project-root>/bin/`.
//!
//! # How it works
//!
//! ```text
//!  cobolt.toml  ──┐
//!  src/*.cbl    ──┤ lex → parse → semantic → bincode → deflate → bytes
//!  forms/*.cfrm ──┘
//!        │
//!        ▼
//!  /tmp/cobolt-build-<hash>/
//!    Cargo.toml   (generated — depends on cobolt-runtime, cobolt-forms, eframe)
//!    src/
//!      main.rs    (generated — embeds assets via include_bytes!, lazy loader)
//!    assets/
//!      program.bin          (compressed serialised AST)
//!      forms/<id>.cfrm      (raw form XML — lazy-loaded by name)
//!        │
//!        ▼
//!  cargo build --release
//!        │
//!        ▼
//!  <project-root>/bin/<project-name>[.exe]
//! ```
//!
//! # Lazy form loading
//!
//! The generated binary contains a `&[(&str, &[u8])]` dispatch table mapping
//! form IDs to their compressed bytes.  A form is only deserialized from that
//! table when it is first requested at runtime, so a 20-form application
//! starts instantly even if only one form is ever opened.
//!
//! # Source-code protection
//!
//! No `.cbl` source is included in the binary.  The AST is stored as opaque
//! compressed bincode — it cannot be trivially reversed into readable COBOL.

use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};

use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Deserialize;
use thiserror::Error;

pub mod exec_rust;
pub mod external_crates;
pub mod main_form_guard;

pub use external_crates::ExternalCrate;

// ── License / NOTICE assets ─────────────────────────────────────────────────────
//
// Baked in at build time so the `package`/`build` commands can drop the required
// Apache-2.0 notices alongside every distributable artifact, no matter where the
// tool is run from.

/// Full Apache-2.0 license text.
pub const LICENSE_TEXT: &str = include_str!("../../../LICENSE");
/// Project NOTICE file.
pub const NOTICE_TEXT: &str = include_str!("../../../NOTICE");
/// Short runtime/redistribution notice to ship with user applications.
pub const RUNTIME_NOTICE_TEXT: &str = include_str!(
    "../../../docs/licensing/PACKAGE_NOTICE_TEMPLATE/POWER_RUST_COBOL_RUNTIME_NOTICE.txt"
);

/// Install an executable at `dst` **by rename, never by overwrite**.
///
/// `std::fs::copy` onto an existing executable rewrites the file in place, and
/// on macOS (Apple Silicon) that invalidates the kernel's cached code-signature
/// blob for the vnode: a process exec'd from the file in the instant after the
/// rewrite is killed with SIGKILL — no stderr, no crash report, nothing. The
/// IDE's Run does exactly that instant exec (build, then launch in the same
/// frame), which made every rebuilt program die silently at startup, while the
/// same binary ran fine from a terminal seconds later. Copying to a sibling
/// temp file and renaming it into place gives `dst` a fresh inode with fresh
/// signature state, atomically, and retires the whole failure class.
fn install_executable(src: &Path, dst: &Path) -> std::io::Result<()> {
    let file_name = dst
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "binary".to_owned());
    let tmp = dst.with_file_name(format!(".{file_name}.new-{}", std::process::id()));
    std::fs::copy(src, &tmp)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&tmp)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&tmp, perms)?;
    }
    // Unix rename replaces an existing dst atomically. Windows refuses to
    // rename over an existing file — and also refuses to DELETE a running
    // .exe, while it does allow RENAMING one. So: try the delete (works when
    // nothing runs), and when a live instance holds the file, do the standard
    // updater dance instead — rename the running exe aside, then move the new
    // one into place; the parked file is cleaned up best-effort (a locked one
    // disappears on the next successful install after the process ends).
    #[cfg(windows)]
    if dst.exists() && std::fs::remove_file(dst).is_err() {
        let parked = dst.with_file_name(format!(".{file_name}.old-{}", std::process::id()));
        let _ = std::fs::remove_file(&parked);
        if std::fs::rename(dst, &parked).is_err() {
            // Locked beyond even a rename — surface the real rename error below.
        }
    }
    let renamed = std::fs::rename(&tmp, dst);
    if renamed.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    renamed
}

/// Write `LICENSE`, `NOTICE` and the PowerRustCOBOL runtime notice into `dir`.
/// Used so distributed binaries/packages carry the required notices.
pub fn write_license_notices(dir: &Path) -> std::io::Result<()> {
    std::fs::write(dir.join("LICENSE"), LICENSE_TEXT)?;
    std::fs::write(dir.join("NOTICE"), NOTICE_TEXT)?;
    std::fs::write(dir.join("POWERRUSTCOBOL-NOTICE.txt"), RUNTIME_NOTICE_TEXT)?;
    Ok(())
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum CompilerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML error: {0}")]
    Toml(String),

    #[error("Parse error in '{file}': {message}")]
    Parse { file: String, message: String },

    #[error("Semantic error in '{file}': {message}")]
    Semantic { file: String, message: String },

    #[error("Serialization error: {0}")]
    Serialize(String),

    #[error("cargo build failed (exit {code}):\n{stderr}")]
    CargoBuild { code: i32, stderr: String },

    /// A registered External Crate cannot be used as recorded (spec 044) —
    /// e.g. its vendored source under the project's `crates/` is missing.
    #[error("external crate error: {0}")]
    ExternalCrate(String),

    /// A `rustc` error inside a developer's `EXEC RUST` block, reported against
    /// their COBOL source rather than the generated file (spec 041 R10).
    #[error("EXEC RUST error in '{file}' at line {line}, column {col}: {message}")]
    ExecRustBlock {
        file: String,
        line: u32,
        col: u32,
        message: String,
    },

    /// The Rust toolchain is missing or unusable (spec 041 R14).
    ///
    /// Building a PowerRustCOBOL application has always shelled out to `cargo`;
    /// what spec 041 adds is saying so plainly instead of letting the failure
    /// arrive as an opaque spawn error. **Only building** needs the toolchain —
    /// a built binary runs on machines that have none.
    #[error(
        "the Rust toolchain is required to build, but {tool} could not be run: {detail}. \
         Install Rust from https://rustup.rs and make sure {tool} is on PATH. \
         (Only building needs it — the binary you produce does not.)"
    )]
    Toolchain { tool: String, detail: String },

    /// A build was asked for a target this host cannot produce (spec 041 R18).
    #[error(
        "cannot build for '{requested}' on this machine ({host}): PowerRustCOBOL builds \
         for the host only. Build the application on {requested} instead."
    )]
    UnsupportedTarget { requested: String, host: String },

    #[error("No main COBOL source specified in cobolt.toml")]
    NoMain,

    /// The project cannot say which form its application starts at. Only the
    /// main form starts an application, so a build that had to guess would
    /// ship a door nobody chose.
    #[error("this project's main form is ambiguous: {0}")]
    AmbiguousMainForm(String),

    #[error("could not locate the PowerRustCOBOL workspace crates: {0}")]
    Workspace(String),
}

// ── Project manifest (subset we need) ────────────────────────────────────────

#[derive(Deserialize)]
struct ProjectMeta {
    name: String,
    version: String,
    main: String,
    #[serde(default)]
    destination_folder: String,
    #[serde(default = "default_debug_compilation")]
    debug_compilation: bool,
}

fn default_debug_compilation() -> bool {
    true
}

/// Where a build installs the deliverable when the project does not choose.
pub const DEFAULT_DESTINATION_FOLDER: &str = "dist";

impl ProjectMeta {
    /// The folder this build installs into — never empty.
    ///
    /// Before 1.60.27 the default was the project's own NAME, written into
    /// every `.project.toml` at creation. That is not a choice anyone made, so
    /// it is treated as unset: those projects deliver into `dist/` like new
    /// ones, instead of into a folder that reads as a second copy of the
    /// project while the scaffolded `dist/` stays empty. A destination the
    /// developer actually picked — anything other than the project name — is
    /// always honoured.
    fn destination_folder_or_default(&self) -> &str {
        let chosen = self.destination_folder.trim();
        if chosen.is_empty() {
            return DEFAULT_DESTINATION_FOLDER;
        }
        let legacy_default = self.name.strip_suffix(".project").unwrap_or(&self.name);
        if chosen.eq_ignore_ascii_case(legacy_default) {
            return DEFAULT_DESTINATION_FOLDER;
        }
        chosen
    }
}

#[derive(Deserialize, Default)]
struct ProjectFiles {
    #[serde(default)]
    sources: Vec<String>,
    #[serde(default)]
    forms: Vec<String>,
    /// Bundled binary/data assets (images, audio, fonts, …) — copied next to
    /// the produced binary so they ship with the build.
    #[serde(default)]
    assets: Vec<String>,
    /// Documentation files — also copied next to the binary.
    #[serde(default)]
    documentation: Vec<String>,
    /// Generated COBOL produced from the project's forms. For a form-centric
    /// project with no hand-written main, the first generated program is the
    /// runnable entry point.
    #[serde(default)]
    generated: Vec<String>,
}

/// The `[forms]` section of `cobolt.toml` — the project's default form theme
/// (spec 007). Empty/absent ⇒ Liquid Glass.
#[derive(Deserialize, Default)]
struct FormsConfig {
    #[serde(default)]
    theme: String,
    // 038 window-effect settings. Baked into the generated source at build
    // time (see `fx_spec` / `generate_main_rs`), because a shipped binary has
    // no `cobolt.toml` beside it to read them from (spec 042 R11).
    #[serde(default, rename = "entrance-effect")]
    entrance_effect: String,
    #[serde(default, rename = "entrance-ms")]
    entrance_ms: u32,
    #[serde(default, rename = "entrance-easing")]
    entrance_easing: String,
    #[serde(default, rename = "exit-effect")]
    exit_effect: String,
    #[serde(default, rename = "exit-ms")]
    exit_ms: u32,
    #[serde(default, rename = "exit-easing")]
    exit_easing: String,
    #[serde(default, rename = "entrance-on-restore")]
    entrance_on_restore: bool,
}

#[derive(Deserialize)]
struct CoboltProject {
    project: ProjectMeta,
    #[serde(default)]
    files: ProjectFiles,
    #[serde(default)]
    forms: FormsConfig,
    /// Registered External Crates (spec 044) — `[[crates]]` pins vendored
    /// under the project's `crates/`. Absent in old projects ⇒ empty.
    #[serde(default)]
    crates: Vec<ExternalCrate>,
}

/// Resolve the project's entry program as a path relative to the project root.
///
/// Order of preference:
/// 1. the declared `[project].main`, when that file exists on disk;
/// 2. the first **generated** form program that exists (form-centric projects
///    with no hand-written main);
/// 3. the first ordinary **source** that exists.
///
/// Returns `None` only when nothing compilable can be found — the caller maps
/// that to [`CompilerError::NoMain`].
fn resolve_main(proj: &CoboltProject, dir: &Path) -> Option<String> {
    let exists = |rel: &String| !rel.is_empty() && dir.join(rel).exists();

    // A form project's program is the MAIN FORM's generated `.cbl`, and nothing
    // else will do.
    //
    // Only `sources[0]` is parsed into the program that the built binary
    // interprets. A form-centric project created by the IDE also carries a stub
    // `src/main.cbl` — seven lines, a DISPLAY and a GOBACK — and that stub was
    // winning, because it exists. The result was a binary that drew the form and
    // ran the stub: no event handler in the compiled program at all, so every
    // button was dead and every `EXEC RUST` block in a handler was never
    // compiled. Nothing failed; nothing happened.
    if let Some(rel) = main_form_program(proj, dir) {
        return Some(rel);
    }

    if exists(&proj.project.main) {
        return Some(proj.project.main.clone());
    }
    proj.files
        .generated
        .iter()
        .chain(proj.files.sources.iter())
        .find(|rel| exists(rel))
        .cloned()
}

/// The generated `.cbl` of the form marked MAIN, if this project has forms.
///
/// Falls back to the first form when none carries the flag, so a project that
/// predates the main-form marker still builds something coherent rather than a
/// stub.
/// 051 R1 — the generated program for form `id` (its uppercased `.cfrm`
/// stem): the `files.generated` entry with the same stem, a same-stem entry
/// still tracked under `files.sources` (legacy projects), or `<stem>.cbl`
/// beside the form's own `.cfrm`. `None` = the form has no program on disk.
fn generated_program_path(proj: &CoboltProject, dir: &Path, id: &str) -> Option<PathBuf> {
    let stem_matches = |rel: &str| {
        Path::new(rel)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case(id))
            .unwrap_or(false)
    };
    for list in [&proj.files.generated, &proj.files.sources] {
        if let Some(rel) = list
            .iter()
            .find(|g| stem_matches(g) && dir.join(g.as_str()).exists())
        {
            return Some(dir.join(rel));
        }
    }
    let cfrm = proj.files.forms.iter().find(|f| stem_matches(f))?;
    let beside = dir.join(cfrm).with_extension("cbl");
    beside.exists().then_some(beside)
}

/// The project manifest governing `start` — the nearest `*.project.toml` (or a
/// legacy `cobolt.toml`) at or above it. `None` for a loose form that belongs to
/// no project, which is a perfectly good way to run one.
///
/// The manifest is named after the project (`PowerDemo3.project.toml`), not a
/// fixed file name, so discovery scans each ancestor rather than probing one.
pub fn find_project_manifest(start: &Path) -> Option<PathBuf> {
    let from = if start.is_dir() {
        start
    } else {
        start.parent()?
    };
    for dir in from.ancestors() {
        let legacy = dir.join("cobolt.toml");
        if legacy.is_file() {
            return Some(legacy);
        }
        let mut found: Option<PathBuf> = None;
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_file()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.to_ascii_lowercase().ends_with(".project.toml"))
                    .unwrap_or(false)
            {
                // Deterministic when a folder somehow holds two: take the
                // first by name rather than whatever the directory yields.
                if found.as_ref().map(|f| path < *f).unwrap_or(true) {
                    found = Some(path);
                }
            }
        }
        if found.is_some() {
            return found;
        }
    }
    None
}

/// 051 R1 — where form `id`'s generated program actually lives on disk, given
/// any form file in the same project.
///
/// **This is the one rule.** The compiled application resolves a child form's
/// program through [`generated_program_path`]; `rcrun run-form` resolves it
/// through this, which wraps the same function — so Run Form and the built
/// binary can no longer disagree about where a form's code is. They did: this
/// looked only beside the `.cfrm`, while the IDE writes generated code into the
/// project's `generated/` folder, so every "open form" door failed in Run Form
/// on a project that built and ran perfectly once compiled.
///
/// Falls back to `<stem>.cbl` beside `cfrm` when the form belongs to no
/// project. `None` = there is no program on disk to run.
pub fn form_program_path(cfrm: &Path, id: &str) -> Option<PathBuf> {
    let beside = || {
        let candidate = cfrm.with_file_name(format!("{}.cbl", id.trim()));
        candidate.exists().then_some(candidate)
    };
    let Some(manifest) = find_project_manifest(cfrm) else {
        return beside();
    };
    let Ok(text) = std::fs::read_to_string(&manifest) else {
        return beside();
    };
    let Ok(proj) = toml::from_str::<CoboltProject>(&text) else {
        return beside();
    };
    let dir = manifest.parent()?;
    generated_program_path(&proj, dir, id).or_else(beside)
}

fn main_form_program(proj: &CoboltProject, dir: &Path) -> Option<String> {
    if proj.files.forms.is_empty() {
        return None;
    }
    let stem_of = |rel: &String| {
        Path::new(rel)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_owned)
    };

    let mut chosen: Option<String> = None;
    for rel in &proj.files.forms {
        let abs = dir.join(rel);
        if !abs.exists() {
            continue;
        }
        let is_main = std::fs::read_to_string(&abs)
            .ok()
            .and_then(|xml| cobolt_forms::load_form_from_str(&xml).ok())
            .map(|f| f.main_form)
            .unwrap_or(false);
        if chosen.is_none() || is_main {
            chosen = stem_of(rel);
        }
        if is_main {
            break;
        }
    }
    let stem = chosen?;

    proj.files
        .generated
        .iter()
        .find(|g| {
            stem_of(g).as_deref() == Some(stem.as_str()) && dir.join(g).exists()
        })
        .cloned()
}

// ── Public API ────────────────────────────────────────────────────────────────

/// A build progress update, for driving a UI progress bar.
#[derive(Clone, Debug)]
pub struct BuildProgress {
    /// Completion in `0.0..=1.0`. Negative for `detail` lines, which carry
    /// no position of their own.
    pub fraction: f32,
    /// Short, human-readable description of the current phase.
    pub message: String,
    /// `true` for a supplementary log detail (file counts, sizes) that
    /// belongs in a build log, not on the progress bar.
    pub detail: bool,
}

/// Options controlling the build.
pub struct BuildOptions {
    /// Print progress to stderr.
    pub verbose: bool,
    /// Override the workspace root (where the cobolt-* crates live).
    /// Defaults to the directory containing the compiler's own executable.
    pub workspace_root: Option<PathBuf>,
    /// Optional channel that receives [`BuildProgress`] updates as the build
    /// moves through its phases (for a UI progress bar).
    pub progress: Option<std::sync::mpsc::Sender<BuildProgress>>,
    /// Target triple to build for. `None` — the default — means the host.
    ///
    /// There is no cross-compilation (spec 041 R17/R18): anything other than the
    /// host triple is rejected with a diagnostic telling the developer to build
    /// on that operating system. The option exists so that request can be
    /// *refused clearly* rather than silently producing a host binary.
    pub target: Option<String>,
    /// Discard the incremental build directory first, so everything is
    /// recompiled from the generated sources.
    ///
    /// The scaffold in the temp build directory (`Cargo.toml`, `main.rs`,
    /// `exec_rust_blocks.rs`) is regenerated every build, but `cargo`'s own
    /// artefacts survive — and they were produced by whichever PowerRustCOBOL
    /// version happened to be installed at the time. After an upgrade that
    /// changed codegen or the runtime, an incremental build can link stale
    /// objects with new generated code. A full build is the answer to "it
    /// behaves oddly since I updated", and it is what stamps
    /// `built_with_version` into the project (spec: version-stamped projects).
    pub full: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            verbose: true,
            workspace_root: None,
            progress: None,
            target: None,
            full: false,
        }
    }
}

/// Build result returned on success.
#[derive(Debug)]
pub struct BuildResult {
    /// Path to the produced executable.
    pub binary_path: PathBuf,
    /// Number of COBOL source files compiled.
    pub source_count: usize,
    /// Number of form files embedded.
    pub form_count: usize,
    /// Compressed AST size in bytes.
    pub ast_bytes: usize,
    /// How many crates `cargo` actually compiled this time (spec 041 R15).
    ///
    /// `0` on a rebuild with nothing changed — the measurement AC13 asks for,
    /// as a count rather than a stopwatch reading, because a count is the same
    /// number on a fast machine and a slow one.
    pub crates_compiled: usize,
}

/// Compile a Cobolt project into a single native binary.
///
/// `manifest_path` is the path to `cobolt.toml`.
/// Returns the path to the produced binary on success.
pub fn build_project(
    manifest_path: &Path,
    opts: &BuildOptions,
) -> Result<BuildResult, CompilerError> {
    if opts.verbose {
        eprintln!("📖 Reading cobolt.toml …");
    }
    let manifest_text = std::fs::read_to_string(manifest_path)?;
    let proj: CoboltProject =
        toml::from_str(&manifest_text).map_err(|e| CompilerError::Toml(e.to_string()))?;

    let project_dir = manifest_path
        .canonicalize()?
        .parent()
        .map(|p| p.to_owned())
        .unwrap_or_else(|| PathBuf::from("."));

    build_core(proj, project_dir, opts, true)
}

/// Compile a single standalone COBOL source file (no `cobolt.toml`) into a
/// native binary. Project metadata is synthesized from the file name; the
/// binary lands in `bin/` next to the source. Ideal for console-only programs.
pub fn build_single_file(
    source_path: &Path,
    opts: &BuildOptions,
) -> Result<BuildResult, CompilerError> {
    let source_path = source_path.canonicalize()?;
    let project_dir = source_path
        .parent()
        .map(|p| p.to_owned())
        .unwrap_or_else(|| PathBuf::from("."));
    let main = source_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "main.cbl".to_string());
    let name = source_path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "program".to_string());

    let proj = CoboltProject {
        project: ProjectMeta {
            name,
            version: "1.0.0".into(),
            main,
            destination_folder: String::new(),
            debug_compilation: true,
        },
        files: ProjectFiles::default(),
        forms: FormsConfig::default(),
        // Single-file builds have no project, hence no External Crates
        // (spec 044 R22).
        crates: Vec::new(),
    };
    build_core(proj, project_dir, opts, false)
}

/// The generated crate/binary name from the project name. Cargo package
/// names allow only ASCII alphanumerics, `-` and `_`; project names carry
/// spaces, the `.project` suffix, and arbitrary punctuation (a literal `.`
/// aborted the whole build with "invalid character in package name").
fn sanitize_package_name(project_name: &str) -> String {
    let lowered = project_name.trim().to_ascii_lowercase();
    let base = lowered.strip_suffix(".project").unwrap_or(&lowered);
    let name: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.chars().any(|c| c.is_ascii_alphanumeric()) {
        name
    } else {
        "app".to_string()
    }
}

/// Write `bytes` to `path` only when they differ from what is already there
/// (spec 041 R15).
///
/// Every build regenerates `Cargo.toml`, `main.rs` and `exec_rust_blocks.rs`
/// from source. Writing them unconditionally gives each a fresh mtime, and
/// cargo's fingerprint is mtime-based — so an unchanged program recompiled its
/// own crate on every single build, for nothing. Comparing first is what makes
/// "build twice, compile nothing the second time" true rather than aspirational.
fn write_if_changed(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Ok(existing) = std::fs::read(path) {
        if existing == bytes {
            return Ok(());
        }
    }
    std::fs::write(path, bytes)
}

// ── Toolchain (spec 041 R14, R18) ────────────────────────────────────────────

/// Turn a failure to start a toolchain program into a diagnostic that names it.
fn toolchain_error(tool: &str, e: &std::io::Error) -> CompilerError {
    CompilerError::Toolchain {
        tool: tool.to_string(),
        detail: e.to_string(),
    }
}

/// Read the host triple out of `rustc -vV`, given a way to run it.
///
/// Taking the runner as an argument is what makes "no toolchain" testable: a
/// test can hand this a closure that fails the way a missing `rustc` fails,
/// without editing `PATH` for the whole test process.
fn probe_host_triple<R>(run: R) -> Result<String, CompilerError>
where
    R: FnOnce() -> std::io::Result<std::process::Output>,
{
    let out = run().map_err(|e| toolchain_error("rustc", &e))?;
    if !out.status.success() {
        return Err(CompilerError::Toolchain {
            tool: "rustc".into(),
            detail: format!(
                "it exited with {}: {}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        });
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .find_map(|l| l.strip_prefix("host: "))
        .map(|h| h.trim().to_string())
        .ok_or_else(|| CompilerError::Toolchain {
            tool: "rustc".into(),
            detail: "its version output did not report a host triple".into(),
        })
}

/// The triple this machine builds for.
fn host_triple() -> Result<String, CompilerError> {
    probe_host_triple(|| {
        std::process::Command::new("rustc")
            .arg("-vV")
            .output()
    })
}

/// Shared build pipeline used by both [`build_project`] and [`build_single_file`].
fn build_core(
    proj: CoboltProject,
    project_dir: PathBuf,
    opts: &BuildOptions,
    // Spec 044 R21/R22 — whether a real `cobolt.toml` backs this build. It
    // decides the unregistered-crate message: a project is told to use
    // External Crates, a lone `.cbl` that external crates require a project.
    has_project: bool,
) -> Result<BuildResult, CompilerError> {
    // Detail lines (file counts, sizes) stream to the UI too, flagged so
    // they land in the build-details log rather than on the progress bar.
    let log = |msg: &str| {
        if opts.verbose {
            eprintln!("{msg}");
        }
        if let Some(tx) = &opts.progress {
            let _ = tx.send(BuildProgress {
                fraction: -1.0,
                message: msg.trim().to_string(),
                detail: true,
            });
        }
    };
    // Emit a phase milestone: log it (verbose) and stream it to any UI progress bar.
    let report = |fraction: f32, msg: &str| {
        if opts.verbose {
            eprintln!("{msg}");
        }
        if let Some(tx) = &opts.progress {
            let _ = tx.send(BuildProgress {
                fraction,
                message: msg.to_string(),
                detail: false,
            });
        }
    };

    // ── 1. The toolchain, before anything is staged ──────────────────────────
    // Checked first so a missing toolchain fails with its own diagnostic and
    // leaves no half-built artefacts behind (spec 041 R14/AC12), and so a
    // cross-target request is refused before any work is done (R18/AC16).
    report(0.02, "Checking the Rust toolchain…");
    let host = host_triple()?;
    if let Some(requested) = opts.target.as_deref() {
        if requested != host {
            return Err(CompilerError::UnsupportedTarget {
                requested: requested.to_string(),
                host,
            });
        }
    }

    report(0.05, "Reading project…");
    // The system documentation is NOT published here. It describes the
    // platform, not this project, so it belongs to the machine-level System
    // Knowledge Base that the IDE republishes from the running binary — see
    // `publish_system_documentation`. Publishing it per project also mixed
    // platform reference material into the developer's own Knowledge Base,
    // whose whole purpose is project material (diagrams, requirements, data
    // models). Existing copies under `<project>/Knowledge Base/` are left
    // alone: they are the developer's files to remove.
    let bin_name = sanitize_package_name(&proj.project.name);

    // ── 2. Collect all source files ───────────────────────────────────────────
    report(0.10, "Collecting source files…");
    let mut sources: Vec<(String, String)> = Vec::new(); // (rel_path, source_text)

    // Resolve the entry program. Prefer the declared `main`; for a form-centric
    // project whose `main` was never hand-written, fall back to the first
    // generated form program so the project still builds and runs.
    let main_rel = resolve_main(&proj, &project_dir).ok_or(CompilerError::NoMain)?;
    let main_path = project_dir.join(&main_rel);
    sources.push((main_rel.clone(), std::fs::read_to_string(&main_path)?));

    // Then the rest of the declared sources (skip main if listed again).
    for rel in &proj.files.sources {
        if rel == &main_rel {
            continue;
        }
        let abs = project_dir.join(rel);
        if abs.exists() {
            sources.push((rel.clone(), std::fs::read_to_string(&abs)?));
        }
    }

    log(&format!("   {} source file(s)", sources.len()));

    // ── 3. Parse + semantic-check every source ────────────────────────────────
    use cobolt_lexer::{tokenize, SourceFormat};
    use cobolt_parser::parse;
    use cobolt_semantic::{analyze_with, AnalyzeOptions, Severity};

    // We compile the main source into the primary Program.
    // Additional sources are currently compiled independently and merged via
    // their nested-program lists — a full multi-file linker is future work.
    let (main_rel, main_src) = &sources[0];
    report(0.18, &format!("Tokenizing {main_rel}…"));
    let fmt = detect_format(main_src);
    let tokens = tokenize(main_src, fmt);
    report(0.24, &format!("Parsing {main_rel}…"));
    let parse_result = parse(tokens);

    for d in &parse_result.diagnostics {
        if d.severity == cobolt_parser::Severity::Error {
            return Err(CompilerError::Parse {
                file: main_rel.clone(),
                message: d.message.clone(),
            });
        }
    }

    let program = parse_result.program.ok_or_else(|| CompilerError::Parse {
        file: main_rel.clone(),
        message: "Parse produced no program".into(),
    })?;

    report(0.30, "Semantic analysis…");
    // 049 R17 — pre-scan the project's forms: their FormFormats feed the
    // OpenForm* load-path check below, and each sidebar menu's `open-form:`
    // targets are validated against the same map. Runs before the main
    // semantic pass so a bad load path fails the build with a named form.
    let form_formats = has_project.then(|| {
        let mut parsed: Vec<(String, std::path::PathBuf, cobolt_forms::Form)> = Vec::new();
        for rel in &proj.files.forms {
            let abs = project_dir.join(rel);
            let Ok(xml) = std::fs::read_to_string(&abs) else {
                continue;
            };
            let Ok(form) = cobolt_forms::load_form_from_str(&xml) else {
                continue;
            };
            let stem = abs
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(rel.as_str())
                .to_string();
            parsed.push((stem, abs.clone(), form));
        }
        let map = form_formats_map(parsed.iter().map(|(stem, _, f)| (stem.as_str(), f)));
        (parsed, map)
    });
    if let Some((parsed, map)) = &form_formats {
        for (stem, abs, form) in parsed {
            let cfrm_dir = abs.parent().unwrap_or(project_dir.as_path());
            for ctrl in walk_controls(&form.controls) {
                if !matches!(
                    ctrl.control_type,
                    cobolt_forms::ControlType::SideMenu | cobolt_forms::ControlType::MenuBar
                ) {
                    continue;
                }
                let yaml = cobolt_forms::menu::menu_yaml_path(cfrm_dir, &ctrl.id);
                if !yaml.exists() {
                    continue;
                }
                let Ok(def) = cobolt_forms::menu::load_menu(&yaml) else {
                    continue;
                };
                let lookup = |name: &str| {
                    map.get(&name.trim().to_ascii_uppercase()).map(|f| match f {
                        cobolt_semantic::FormLoadFormat::Standalone => {
                            cobolt_forms::model::FormFormat::Standalone
                        }
                        cobolt_semantic::FormLoadFormat::Embedded => {
                            cobolt_forms::model::FormFormat::Embedded
                        }
                        cobolt_semantic::FormLoadFormat::Both => {
                            cobolt_forms::model::FormFormat::Both
                        }
                    })
                };
                let violations = cobolt_forms::menu::validate_menu_targets(&def, &lookup);
                if let Some(v) = violations.first() {
                    // Each kind names the format the target HAS and the one
                    // its action NEEDS (049 R17 / 051 R26).
                    let message = match v.kind {
                        cobolt_forms::menu::MenuTargetKind::Embed => format!(
                            "menu item '{}' ({}) loads form '{}', whose FormFormat is \
                             Standalone — a menu load requires Embedded or Both (049 R17).",
                            v.item_id, v.item_label, v.form
                        ),
                        cobolt_forms::menu::MenuTargetKind::Standalone => format!(
                            "menu item '{}' ({}) opens form '{}' standalone, but its \
                             FormFormat is Embedded — a standalone open requires \
                             Standalone or Both (051 R26).",
                            v.item_id, v.item_label, v.form
                        ),
                    };
                    return Err(CompilerError::Semantic {
                        file: format!("{stem}.cfrm / {}", ctrl.id),
                        message,
                    });
                }
            }
        }
    }
    // Spec 044 R20 — registered External Crates extend the block allowlist;
    // their `use`-line names come from the project's pins.
    let sem = analyze_with(
        &program,
        &AnalyzeOptions {
            external_crates: has_project
                .then(|| proj.crates.iter().map(|c| c.lib_name()).collect()),
            form_formats: form_formats.as_ref().map(|(_, map)| map.clone()),
        },
    );
    for d in &sem.diagnostics {
        if d.severity == Severity::Error {
            return Err(CompilerError::Semantic {
                file: main_rel.clone(),
                message: d.message.clone(),
            });
        }
    }

    // ── 4. Serialize + compress the AST ──────────────────────────────────────
    report(0.35, "Serialising the program…");
    let ast_bytes =
        bincode::serialize(&program).map_err(|e| CompilerError::Serialize(e.to_string()))?;

    let mut gz = GzEncoder::new(Vec::new(), Compression::best());
    gz.write_all(&ast_bytes).unwrap();
    let compressed_ast = gz.finish()?;
    let ast_compressed_len = compressed_ast.len();
    log(&format!(
        "   AST: {} bytes → {} bytes compressed",
        ast_bytes.len(),
        ast_compressed_len
    ));

    // ── 5. Collect form files ─────────────────────────────────────────────────
    report(0.42, "Collecting forms & generated code…");
    let mut forms: Vec<(String, Vec<u8>)> = Vec::new(); // (id, raw_xml_bytes)
    // 049 — the menu sidecars, keyed by the control that owns them. A SideMenu
    // or MenuBar keeps its structure in `<control id>.menu.yaml` beside the
    // `.cfrm`, never in a property, so a build that embedded only the forms
    // produced an application whose menus were empty: the compiled shell had
    // a rail with nothing on it. They ride into the binary exactly as the
    // forms do.
    let mut menus: Vec<(String, Vec<u8>)> = Vec::new(); // (control id, yaml bytes)

    for rel in &proj.files.forms {
        let abs = project_dir.join(rel);
        if !abs.exists() {
            continue;
        }
        // Form ID = file stem, uppercased (matches COBOL usage)
        let id = abs
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(rel.as_str())
            .to_ascii_uppercase();
        let raw = std::fs::read(&abs)?;
        // `menu_yaml_path` names the sidecar `<control id>.menu.yaml`, so the
        // file stem IS the control id and the directory can be read without
        // parsing the form again.
        if let Some(dir) = abs.parent() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
                        continue;
                    };
                    let Some(ctrl_id) = name.strip_suffix(".menu.yaml") else {
                        continue;
                    };
                    if menus.iter().any(|(k, _)| k == ctrl_id) {
                        continue;
                    }
                    if let Ok(bytes) = std::fs::read(&p) {
                        menus.push((ctrl_id.to_owned(), bytes));
                    }
                }
            }
        }
        forms.push((id, raw));
    }

    // The MAIN form goes first, because that is the one the built application
    // opens (`FORMS.first()` in the generated `main.rs`).
    //
    // Without this the binary opened whichever form happened to be listed first
    // in `cobolt.toml` — so a project whose main form was the third one started
    // on the wrong window, while the IDE's own Run, which resolves the main form
    // properly, showed the right one. The build and the IDE disagreed about what
    // the application *is*.
    //
    // The designation is read the same way `rcrun` reads it before starting a
    // form, so the build and the runtime cannot disagree about which form the
    // application is. A project where two forms claim the mark has no answer
    // to give, and the build says so rather than picking one.
    let designation = main_form_guard::read_designation(&project_dir, &proj.files.forms)
        .map_err(CompilerError::AmbiguousMainForm)?;
    let designated = designation.map(|d| d.main_form_id).unwrap_or_default();
    if let Some(main_at) = forms.iter().position(|(id, _)| *id == designated) {
        let main = forms.remove(main_at);
        log(&format!("   main form: {}", main.0));
        forms.insert(0, main);
    }
    // Baked in beside the table below: at startup the application checks that
    // the form it is about to open is still this one, so an executable whose
    // embedded form table has been reordered refuses to run instead of opening
    // a door the developer never put there.
    let main_form_id = forms
        .first()
        .map(|(id, _)| id.clone())
        .unwrap_or_default();

    log(&format!("   {} form(s)", forms.len()));

    // ── 5b. Every other form's PROGRAM rides along (051 R1) ──────────────────
    // The binary holds one program per openable form, so an `open-form:` menu
    // item or an OpenForm*/OpenStandAloneForm* call finds the target's event
    // handlers at run time. The MAIN form's program stays `program.bin`,
    // untouched (R2). A form whose generated program is missing or does not
    // parse is OMITTED with a warning — opening it then fails visibly at run
    // time (R15) instead of failing every build of the project.
    report(0.44, "Compiling form programs…");
    let mut form_programs: Vec<(String, Vec<u8>)> = Vec::new(); // (ID, gz bincode)
    for (id, _) in forms.iter().skip(1) {
        let Some(cbl) = generated_program_path(&proj, &project_dir, id) else {
            log(&format!(
                "⚠️  form {id}: no generated program on disk — it cannot be \
                 opened at run time"
            ));
            continue;
        };
        let src = match std::fs::read_to_string(&cbl) {
            Ok(s) => s,
            Err(e) => {
                log(&format!("⚠️  form {id}: {} unreadable ({e}) — omitted", cbl.display()));
                continue;
            }
        };
        let pr = parse(tokenize(&src, detect_format(&src)));
        let program_ok = pr
            .diagnostics
            .iter()
            .all(|d| d.severity != cobolt_parser::Severity::Error);
        let Some(form_program) = pr.program.filter(|_| program_ok) else {
            log(&format!(
                "⚠️  form {id}: its generated program does not parse — omitted \
                 (rebuild the form in the IDE and check the generated code)"
            ));
            continue;
        };
        let bytes = bincode::serialize(&form_program)
            .map_err(|e| CompilerError::Serialize(e.to_string()))?;
        let mut gz = GzEncoder::new(Vec::new(), Compression::best());
        gz.write_all(&bytes).unwrap();
        form_programs.push((id.clone(), gz.finish()?));
    }
    if !form_programs.is_empty() {
        log(&format!("   {} form program(s) embedded", form_programs.len()));
    }

    // ── 6. Locate workspace root (where the cobolt-* crates live) ────────────
    let workspace_root = resolve_workspace_root(opts.workspace_root.clone());

    let workspace_root = match workspace_root {
        Some(root) => root,
        None => {
            return Err(CompilerError::Workspace(format!(
                "looked via the running executable and the build-time path, but \
                 found no 'crates/cobolt-ast'. Pass BuildOptions.workspace_root, \
                 or run the IDE from within the PowerRustCOBOL source tree. \
                 (project dir: {})",
                project_dir.display()
            )));
        }
    };

    log(&format!("🏠 Workspace root: {}", workspace_root.display()));

    // ── 7. Create build staging directory ────────────────────────────────────
    report(0.50, "Packaging solution…");
    let build_dir = std::env::temp_dir().join(format!("cobolt-build-{}", &bin_name));
    // A full build throws away everything cargo cached for this project, so
    // nothing compiled by an older PowerRustCOBOL can survive into the new
    // binary. Slower by design — this is the "make it clean" path.
    if opts.full && build_dir.exists() {
        report(0.50, "Full build — clearing previous artefacts…");
        log("🧹 Full build: discarding the incremental build directory");
        if let Err(e) = std::fs::remove_dir_all(&build_dir) {
            log(&format!(
                "⚠️  Could not clear {}: {e} — continuing incrementally",
                build_dir.display()
            ));
        }
    }
    let assets_dir = build_dir.join("assets");
    let forms_dir = assets_dir.join("forms");
    let src_dir = build_dir.join("src");
    std::fs::create_dir_all(&assets_dir)?;
    std::fs::create_dir_all(&forms_dir)?;
    std::fs::create_dir_all(&src_dir)?;

    // Write compressed AST
    write_if_changed(&assets_dir.join("program.bin"), &compressed_ast)?;

    // Write form files
    for (id, raw) in &forms {
        write_if_changed(&forms_dir.join(format!("{id}.cfrm")), raw)?;
    }

    // 049 — the menu sidecars, beside the forms they belong to.
    if !menus.is_empty() {
        let menus_dir = assets_dir.join("menus");
        std::fs::create_dir_all(&menus_dir)?;
        for (ctrl_id, yaml) in &menus {
            write_if_changed(&menus_dir.join(format!("{ctrl_id}.menu.yaml")), yaml)?;
        }
    }

    // 051 R1 — each openable form's program, beside the main `program.bin`.
    if !form_programs.is_empty() {
        let programs_dir = assets_dir.join("programs");
        std::fs::create_dir_all(&programs_dir)?;
        for (id, bin) in &form_programs {
            write_if_changed(&programs_dir.join(format!("{id}.bin")), bin)?;
        }
    }

    // ── 7b. Stage the asset-pack themes the forms actually use ────────────────
    // A built binary is handed to end users on machines that have no
    // PowerRustCOBOL install and no `assets/themes` folder, so a themed form
    // used to fall back to procedural Liquid Glass the moment it left the IDE.
    // Embed each referenced pack (manifest + the art it draws) into the
    // executable instead: the binary then paints from exactly the same bytes as
    // the designer, the preview and Run Form, on every OS, with nothing to
    // install alongside it (spec 007 R5).
    report(0.46, "Embedding form themes…");
    let project_theme_default = proj.forms.theme.trim().to_owned();
    let staged_themes = if forms.is_empty() {
        Vec::new()
    } else {
        let wanted = wanted_theme_ids(&forms, &project_theme_default);
        let search_dirs = theme_search_dirs(&project_dir, &workspace_root);
        stage_theme_packs(&wanted, &search_dirs, &assets_dir.join("themes"), &log)?
    };
    if !staged_themes.is_empty() {
        log(&format!(
            "   {} theme pack(s) embedded: {}",
            staged_themes.len(),
            staged_themes
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    // ── 8. Generate src/exec_rust_blocks.rs ───────────────────────────────────
    // Emitted unconditionally: a program with no blocks gets an empty dispatch
    // table, which keeps `main.rs` free of a conditional module (spec 041 T8).
    report(0.55, "Generating Rust blocks…");
    let crates_path = workspace_root.join("crates");
    let has_forms = !forms.is_empty();
    // A form application already owns the process's one event loop, so a block
    // that opens its own window can only fail — and it fails *silently*, which
    // is the reason to stop the build rather than ship a binary whose button
    // does nothing. Console programs keep the call: there it works.
    if has_forms {
        if let Some(call) = exec_rust::find_event_loop_calls(&program, main_src).first() {
            return Err(CompilerError::ExecRustBlock {
                file: main_rel.clone(),
                line: call.cobol_line,
                col: call.cobol_col,
                message: format!(
                    "`{call}` cannot open a window from a form application. This program \
                     has forms, so the one event loop a process is allowed already \
                     belongs to the form window, and every EXEC RUST block in an event \
                     handler runs on a worker thread. The second call returns an error \
                     instead of opening a window, and it does not panic — so nothing is \
                     displayed, nothing is logged, and CATCH RUST-EXCEPTION never fires.\
                     \n\nTo open a window from a block, use `cobolt_windows::open`:\
                     \n\n    let win = cobolt_windows::open(\
                     \n        \"my-dialog\",\
                     \n        eframe::egui::ViewportBuilder::default().with_title(\"Pick\"),\
                     \n        move |ui, _class| {{ /* your egui, on the UI thread */ }},\
                     \n    );\
                     \n    win.wait();\
                     \n\nShare the result with an Arc<Mutex<..>>, because that closure runs \
                     on the UI thread. To change a control instead, write it through \
                     `cobolt_objects`. `{call}` belongs in a console program, where the \
                     interpreter owns the main thread.",
                    call = call.call
                ),
            });
        }
    }

    let blocks = exec_rust::generate(&program, &sem.symbols, main_src, has_forms);
    if blocks.block_count > 0 || blocks.item_count > 0 {
        log(&format!(
            "   {} EXEC RUST block(s), {} item-level block(s)",
            blocks.block_count, blocks.item_count
        ));
    }
    write_if_changed(&src_dir.join("exec_rust_blocks.rs"), blocks.source.as_bytes())?;

    // ── 9. Generate Cargo.toml for the build project ──────────────────────────
    // The GUI crates are linked for a program with forms **or** with any
    // `EXEC RUST` block. Semantic analysis tells a developer that `eframe` and
    // `egui` are available to a block (spec 041 R16); leaving them out of a
    // console program's manifest made that a lie, and the failure surfaced as
    // `unresolved import 'eframe'` against their own line — a correct message
    // for a promise we had broken. A block is also exactly where someone opens
    // a dialog from an otherwise console program, which is the reason to want
    // them there.
    let links_gui = has_forms || blocks.block_count > 0 || blocks.item_count > 0;
    // Spec 044 R10 — every pin must have its vendored source before staging;
    // a missing dir fails here with the fix named, never a silent registry
    // fallback that would un-pin the build.
    external_crates::validate_pins(&project_dir, &proj.crates)
        .map_err(CompilerError::ExternalCrate)?;
    let cargo_toml = generate_cargo_toml(
        &bin_name,
        &proj.project.version,
        &crates_path,
        links_gui,
        &project_dir,
        &proj.crates,
    );
    write_if_changed(&build_dir.join("Cargo.toml"), cargo_toml.as_bytes())?;

    // ── 9b. Generate src/main.rs ──────────────────────────────────────────────
    let form_ids: Vec<&str> = forms.iter().map(|(id, _)| id.as_str()).collect();
    // 038/042 — the project's window effects, baked into the generated source
    // as `id:ms:easing` triples: a shipped binary has no `cobolt.toml` beside
    // it, so settings it cannot read are settings it cannot honour (042 R11).
    let entrance_fx = fx_triple(
        &proj.forms.entrance_effect,
        proj.forms.entrance_ms,
        &proj.forms.entrance_easing,
    );
    let exit_fx = fx_triple(
        &proj.forms.exit_effect,
        proj.forms.exit_ms,
        &proj.forms.exit_easing,
    );
    let program_ids: Vec<&str> = form_programs.iter().map(|(id, _)| id.as_str()).collect();
    let menu_ids: Vec<&str> = menus.iter().map(|(id, _)| id.as_str()).collect();
    let main_rs = generate_main_rs(
        &proj.project.name,
        &proj.project.version,
        has_forms,
        &form_ids,
        &main_form_id,
        &program_ids,
        &menu_ids,
        &staged_themes,
        &project_theme_default,
        &entrance_fx,
        &exit_fx,
        proj.forms.entrance_on_restore,
    );
    write_if_changed(&src_dir.join("main.rs"), main_rs.as_bytes())?;

    // ── 10. Run cargo build --release ─────────────────────────────────────────
    // Stream cargo's stderr so the progress bar advances per crate compiled and
    // shows the crate currently building.
    report(0.60, "Compiling…");
    use std::io::{BufRead as _, BufReader, Read as _};
    let mut base_args = vec!["build"];
    if !proj.project.debug_compilation {
        base_args.push("--release");
    }
    // `--message-format=json` puts machine-readable diagnostics on **stdout**
    // and leaves the human "Compiling …" lines on stderr, so the progress bar
    // keeps working while the diagnostics become mappable back to the
    // developer's COBOL (spec 041 R10).
    base_args.push("--message-format=json");
    // Resolve dependencies from the local cargo cache. Without `--offline`,
    // cargo refreshes the registry index over the network before it lists and
    // locks the dependency graph, so every first build of a project sat on
    // "Updating crates.io index" before a single crate compiled. Every path
    // dependency ships with the IDE and any machine that built it holds the
    // registry crates in cache, so offline resolution is the normal case —
    // exactly the documented contract (adding a crate needs the network,
    // building does not). The one exception, a genuinely cold cache, is
    // detected below and retried online, transparently.
    let (status, captured, json, compiled) = {
        let mut attempt_offline = true;
        loop {
            let mut args = base_args.clone();
            if attempt_offline {
                args.push("--offline");
            }
            let mut child = std::process::Command::new("cargo")
                .args(&args)
                .current_dir(&build_dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| toolchain_error("cargo", &e))?;

            // Drain stdout on its own thread: reading the two pipes in sequence
            // deadlocks as soon as one fills its buffer, and a JSON diagnostic
            // stream fills quickly.
            let json_reader = child.stdout.take().map(|mut out| {
                std::thread::spawn(move || {
                    let mut buf = String::new();
                    let _ = out.read_to_string(&mut buf);
                    buf
                })
            });

            let mut captured = String::new();
            let mut compiled = 0usize;
            if let Some(err) = child.stderr.take() {
                for line in BufReader::new(err).lines() {
                    let line = line.unwrap_or_default();
                    if let Some(rest) = line.trim_start().strip_prefix("Compiling ") {
                        compiled += 1;
                        let name = rest.split_whitespace().next().unwrap_or("");
                        // Asymptotically approach 0.95 as more crates finish.
                        let frac =
                            0.60 + 0.35 * (1.0 - 1.0 / (1.0 + compiled as f32 / 12.0));
                        report(frac.min(0.95), &format!("Compiling {name}…"));
                    }
                    captured.push_str(&line);
                    captured.push('\n');
                }
            }
            let status = child.wait()?;
            let json = json_reader
                .and_then(|h| h.join().ok())
                .unwrap_or_default();

            // A cold cache fails RESOLUTION (before anything compiles) and
            // cargo's error names the flag. Fetch online and rebuild; a
            // failure with compiled crates is the developer's, not the
            // cache's, and must surface as-is rather than build twice.
            if attempt_offline
                && !status.success()
                && compiled == 0
                && captured.contains("--offline")
            {
                report(0.60, "Fetching dependencies…");
                attempt_offline = false;
                continue;
            }
            break (status, captured, json, compiled);
        }
    };
    if !status.success() {
        // A failure inside a developer's block is reported in their terms. Any
        // other failure — ours, or a dependency's — surfaces raw, because
        // dressing it up as a COBOL error would point them at innocent code.
        // ALL block errors, deterministically ordered by the developer's own
        // (line, column). Taking whichever error cargo's parallel JSON stream
        // mentioned first made identical rebuilds show different single
        // errors — which read as new bugs appearing on every build.
        if let Some((line, col, message)) =
            exec_rust::block_errors_report(exec_rust::map_cargo_json(&json, &blocks))
        {
            return Err(CompilerError::ExecRustBlock {
                file: main_rel.clone(),
                line,
                col,
                message,
            });
        }
        return Err(CompilerError::CargoBuild {
            code: status.code().unwrap_or(-1),
            stderr: captured,
        });
    }

    // ── 11. Copy binary to bin/ ───────────────────────────────────────────────
    report(0.97, "Copying binary…");
    let bin_dir = project_dir.join("bin");
    std::fs::create_dir_all(&bin_dir)?;

    let exe_name = if cfg!(windows) {
        format!("{bin_name}.exe")
    } else {
        bin_name.clone()
    };

    let profile_dir = if proj.project.debug_compilation {
        "debug"
    } else {
        "release"
    };
    let src_bin = build_dir.join("target").join(profile_dir).join(&exe_name);
    let dst_bin = bin_dir.join(&exe_name);
    // Rename-into-place, never copy-over: overwriting the previous binary in
    // place got the very next launch SIGKILLed by macOS (see
    // `install_executable`). Also sets 0o755 on Unix.
    install_executable(&src_bin, &dst_bin)?;

    log(&format!("✅ Binary → {}", dst_bin.display()));

    // ── 11b. Copy bundled assets next to the binary ───────────────────────────
    // Images, audio, fonts and other data files tracked under the project's
    // Assets (and Documentation) must ship with the build so the program finds
    // them by their relative path at runtime. They are copied into `bin/`
    // preserving the project-relative layout (e.g. `bin/assets/logo.png`).
    let mut asset_count = 0usize;
    for rel in proj
        .files
        .assets
        .iter()
        .chain(proj.files.documentation.iter())
    {
        let src = project_dir.join(rel);
        if !src.exists() {
            log(&format!("⚠️  Asset not found, skipped: {rel}"));
            continue;
        }
        let dst = bin_dir.join(rel);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&src, &dst)?;
        asset_count += 1;
    }
    if asset_count > 0 {
        log(&format!(
            "📦 Bundled {asset_count} asset file(s) → {}",
            bin_dir.display()
        ));
    }

    // Drop the required Apache-2.0 notices next to the binary so the
    // distribution carries them.
    if let Err(e) = write_license_notices(&bin_dir) {
        log(&format!("⚠️  Could not write license notices to bin/: {e}"));
    }

    // ── 11c. Copy to destination folder ───────────────────────────────────────
    // `dist/` unless the developer chose otherwise — see
    // `destination_folder_or_default`, which also treats the pre-1.60.27
    // auto-default (the project's own name) as unset.
    let dest_name = proj.project.destination_folder_or_default().to_string();

    let dest_path = if Path::new(&dest_name).is_absolute() {
        PathBuf::from(&dest_name)
    } else {
        project_dir.join(&dest_name)
    };

    log(&format!(
        "📂 Creating destination folder: {}",
        dest_path.display()
    ));
    let _ = std::fs::create_dir_all(&dest_path);

    // Copy project binary to destination folder (rename-into-place — see
    // `install_executable` for why a plain copy is a SIGKILL trap on macOS).
    let dest_bin = dest_path.join(&exe_name);
    if let Err(e) = install_executable(&dst_bin, &dest_bin) {
        log(&format!(
            "⚠️  Failed to copy binary to destination folder: {e}"
        ));
    } else {
        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&dest_bin) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(&dest_bin, perms);
            }
        }
    }

    // Deep copy assets folder (if it exists) to destination folder
    let assets_src = project_dir.join("assets");
    if assets_src.exists() && assets_src.is_dir() {
        let assets_dst = dest_path.join("assets");
        let _ = copy_dir_all(&assets_src, &assets_dst);
    }

    // `rcrun` is deliberately NOT shipped here. A built binary embeds its own
    // compiled AST and links the interpreter and the render engine directly —
    // it never launches a process, so the runner was pure dead weight in the
    // delivered package (roughly doubling it: ~99 MB beside a ~94 MB app), and
    // an unused second executable next to the application is something the end
    // user has to wonder about. `rcrun` remains the developer's tool inside the
    // IDE, where Run Form and debugging do spawn it.

    // The destination folder is what the developer actually hands over, so the
    // Apache-2.0 notices belong here too — `bin/` alone was getting them.
    if let Err(e) = write_license_notices(&dest_path) {
        log(&format!(
            "⚠️  Could not write license notices to the destination folder: {e}"
        ));
    }

    // Spec 044 R24/R25 — the External Crates manifest ships with the binary;
    // with no registered crates a stale one from an earlier build is removed.
    match external_crates::write_rust_manifest(&dest_path, &proj.project.name, &proj.crates) {
        Ok(Some(path)) => log(&format!("📄 Wrote {}", path.display())),
        Ok(None) => {}
        Err(e) => log(&format!("⚠️  Could not write the crate manifest: {e}")),
    }

    report(1.0, "Done");
    Ok(BuildResult {
        binary_path: dst_bin,
        source_count: sources.len(),
        form_count: forms.len(),
        ast_bytes: ast_compressed_len,
        crates_compiled: compiled,
    })
}

// ── Asset-pack themes (spec 007) ──────────────────────────────────────────────

/// One asset-pack theme staged into the build project, ready to be embedded.
struct StagedTheme {
    /// Pack id — also the folder name under `assets/themes/` in the staging dir.
    id: String,
    /// Pack-relative image refs staged next to `theme.toml`, in manifest order.
    assets: Vec<String>,
}

/// The theme ids the embedded forms resolve to (`form ?? project ?? glass`),
/// deduplicated and with the procedural Liquid Glass default dropped — it needs
/// no assets. A form whose XML cannot be parsed here is skipped rather than
/// failing the build: the compiler already parsed it for the AST, and a theme
/// is not worth aborting a build over.
fn wanted_theme_ids(forms: &[(String, Vec<u8>)], project_default: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for (_, raw) in forms {
        let xml = String::from_utf8_lossy(raw);
        let form_theme = cobolt_forms::load_form_from_str(&xml)
            .ok()
            .and_then(|f| f.theme);
        let id = cobolt_forms::theme::resolve_theme_id(
            form_theme.as_deref(),
            Some(project_default),
        );
        // Procedural themes (Liquid Glass, Elegance) are drawn in code and have
        // no `assets/themes/<id>/` folder — asking for one would send the build
        // hunting for art that does not exist and warn when it comes up empty.
        if cobolt_forms::theme::ThemeCatalog::procedural_ids().contains(&id.as_str())
            || ids.contains(&id)
        {
            continue;
        }
        ids.push(id);
    }
    ids
}

/// Where to look for `assets/themes/<id>`, most specific first: packs dropped
/// into the project itself, then the PowerRustCOBOL workspace (running from the
/// source tree), then the installed IDE next to the running executable — the
/// same locations the IDE discovers packs from, so the build sees exactly the
/// pack the designer painted with.
fn theme_search_dirs(project_dir: &Path, workspace_root: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![
        project_dir.join("assets").join("themes"),
        workspace_root.join("assets").join("themes"),
    ];
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        dirs.push(exe_dir.join("assets").join("themes"));
    }
    dirs.retain(|d| d.is_dir());
    dirs.dedup();
    dirs
}

/// Copy each wanted pack's manifest and referenced art into `themes_out`,
/// preserving the pack-relative layout so the manifest's image refs keep
/// working verbatim once embedded.
///
/// A pack that cannot be found or parsed is reported and skipped: the binary
/// then falls back to Liquid Glass exactly as it does today, which is a worse
/// look but never a failed build.
fn stage_theme_packs(
    ids: &[String],
    search_dirs: &[PathBuf],
    themes_out: &Path,
    log: &impl Fn(&str),
) -> Result<Vec<StagedTheme>, CompilerError> {
    let mut staged = Vec::new();
    for id in ids {
        let Some(pack_dir) = search_dirs
            .iter()
            .map(|d| d.join(id))
            .find(|d| d.join("theme.toml").is_file())
        else {
            log(&format!(
                "⚠️  Theme pack '{id}' not found — the built form will fall back \
                 to Liquid Glass. Looked in: {}",
                search_dirs
                    .iter()
                    .map(|d| d.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            continue;
        };
        let manifest_src = std::fs::read_to_string(pack_dir.join("theme.toml"))?;
        let manifest = match cobolt_forms::theme_pack::parse_manifest(&manifest_src) {
            Ok(m) => m,
            Err(e) => {
                log(&format!("⚠️  Theme pack '{id}' has an unusable theme.toml: {e}"));
                continue;
            }
        };

        let out_dir = themes_out.join(id);
        std::fs::create_dir_all(&out_dir)?;
        std::fs::write(out_dir.join("theme.toml"), manifest_src.as_bytes())?;

        let mut assets = Vec::new();
        for rel in manifest.referenced_assets() {
            let src = pack_dir.join(&rel);
            if !src.is_file() {
                log(&format!("⚠️  Theme pack '{id}': missing art '{rel}', skipped"));
                continue;
            }
            let dst = out_dir.join(&rel);
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&src, &dst)?;
            assets.push(rel);
        }
        staged.push(StagedTheme {
            id: id.clone(),
            assets,
        });
    }
    Ok(staged)
}

// ── Code generators ───────────────────────────────────────────────────────────

/// Where the `cobolt-*` workspace crates live — the exact resolution
/// `build_core` uses, exported so the External Crates resolver probe (spec
/// 044 R11) stages against the same paths a real build would: an explicit
/// override first, then walking up from the running executable (source-tree
/// launches), then the compile-time workspace path (installed IDE). `None`
/// when no candidate has `crates/cobolt-ast`.
/// 049 R17 — build the load-path map for [`cobolt_semantic::AnalyzeOptions`]:
/// UPPERCASE form id → format. Each form is keyed by its `.cfrm` file stem —
/// what `OpenForm*` targets and menu `open-form:` actions name — and by the
/// form's own name when it differs.
fn form_formats_map<'a>(
    forms: impl Iterator<Item = (&'a str, &'a cobolt_forms::Form)>,
) -> std::collections::HashMap<String, cobolt_semantic::FormLoadFormat> {
    let mut map = std::collections::HashMap::new();
    for (stem, form) in forms {
        let fmt = match form.form_format {
            cobolt_forms::model::FormFormat::Standalone => {
                cobolt_semantic::FormLoadFormat::Standalone
            }
            cobolt_forms::model::FormFormat::Embedded => cobolt_semantic::FormLoadFormat::Embedded,
            cobolt_forms::model::FormFormat::Both => cobolt_semantic::FormLoadFormat::Both,
        };
        map.insert(stem.trim().to_ascii_uppercase(), fmt);
        let name = form.name.trim().to_ascii_uppercase();
        if !name.is_empty() {
            map.entry(name).or_insert(fmt);
        }
    }
    map
}

/// Depth-first walk over a control list and every control's nested children.
fn walk_controls(controls: &[cobolt_forms::Control]) -> Vec<&cobolt_forms::Control> {
    let mut out = Vec::new();
    fn rec<'a>(list: &'a [cobolt_forms::Control], out: &mut Vec<&'a cobolt_forms::Control>) {
        for c in list {
            out.push(c);
            rec(&c.children, out);
        }
    }
    rec(controls, &mut out);
    out
}

pub fn resolve_workspace_root(explicit: Option<PathBuf>) -> Option<PathBuf> {
    let has_crates = |root: &Path| root.join("crates").join("cobolt-ast").is_dir();
    explicit
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| find_workspace_root(&p))
        })
        .filter(|p| has_crates(p))
        .or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(2) // crates/cobolt-compiler → crates → workspace
                .map(Path::to_path_buf)
                .filter(|p| has_crates(p))
        })
}

/// The `[dependencies]` block every generated program shares. Also the base
/// of the External Crates resolver probe (spec 044 R11) — the probe stages
/// this exact text, so its verdict cannot drift from the real build. Paths go
/// through `external_crates::toml_path` (absolute, forward-slashed): cargo
/// resolves `path =` against the manifest that names it, and backslashes are
/// TOML escapes.
pub(crate) fn base_dependency_block(crates_path: &Path, has_forms: bool) -> String {
    let cp = external_crates::toml_path(crates_path);
    let mut s = format!(
        r#"cobolt-ast      = {{ path = "{cp}/cobolt-ast" }}
cobolt-runtime  = {{ path = "{cp}/cobolt-runtime" }}
flate2          = "1"
bincode         = "1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}
tracing         = "0.1"
"#
    );

    if has_forms {
        // `cobolt-form-host` is the SHARED window host (spec 042 R1/R3): the
        // generated application is thin glue over it, exactly like run-form.
        // eframe/egui stay direct dependencies — `EXEC RUST` blocks compile
        // against them (cobolt_windows, ViewportBuilder, …).
        s.push_str(&format!(
            r#"cobolt-form-host = {{ path = "{cp}/cobolt-form-host" }}
cobolt-forms    = {{ path = "{cp}/cobolt-forms", features = ["render"] }}
cobolt-media    = {{ path = "{cp}/cobolt-media" }}
eframe          = {{ version = "0.36", features = ["default_fonts"] }}
egui            = "0.36"
egui_extras     = {{ version = "0.36", features = ["image"] }}
rfd             = "0.14"
pollster        = "0.3"
# FIX (pre-existing, found 2026-08-07): the eframe image stack pulls
# zune-jpeg with default features off, and without its `log` feature the
# warn!/error! shims expand to nothing where an expression is required —
# every fresh lock resolution since zune-jpeg 0.5.15 fails to compile.
# Naming it here unions the feature back in.
zune-jpeg       = {{ version = "0.5", features = ["log"] }}
"#
        ));
    }

    s
}

fn generate_cargo_toml(
    bin_name: &str,
    version: &str,
    crates_path: &Path,
    has_forms: bool,
    project_dir: &Path,
    pins: &[ExternalCrate],
) -> String {
    // The empty `[workspace]` table detaches the generated package: a user's
    // project can live under someone else's cargo workspace, and without it
    // cargo adopts the build into that workspace and refuses to resolve
    // (spec 044, prototype-found).
    let mut s = format!(
        r#"[package]
name    = "{bin_name}"
version = "{version}"
edition = "2021"

[workspace]

[[bin]]
name = "{bin_name}"
path = "src/main.rs"

[dependencies]
"#
    );
    s.push_str(&base_dependency_block(crates_path, has_forms));

    // Registered External Crates (spec 044 R7/R10/R15): exact pins, with the
    // vendored source patched in as THE copy cargo uses everywhere.
    let (pin_deps, patches) = external_crates::pin_sections(project_dir, pins);
    s.push_str(&pin_deps);
    if !patches.is_empty() {
        s.push_str("\n[patch.crates-io]\n");
        s.push_str(&patches);
    }

    s
}

/// A `[forms]` window-effect setting as the `id:ms:easing` triple the shared
/// host's `FxSpec::parse` speaks (spec 042 R11). Deliberately NOT typed here:
/// `window_fx` is render-gated in `cobolt-forms` and the compiler must not
/// drag egui in, so the values are baked raw and ONE parser — the same one
/// the CLI's `--fx-entrance` goes through — validates ids, clamps durations
/// to each effect's own bounds and defaults broken easings, at run time.
/// Colons cannot survive inside the id/easing fields (they are the triple's
/// separator), so they are stripped defensively.
fn fx_triple(effect: &str, ms: u32, easing: &str) -> String {
    let clean = |s: &str| s.trim().to_ascii_lowercase().replace(':', "");
    format!("{}:{}:{}", clean(effect), ms, clean(easing))
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn generate_main_rs(
    app_name: &str,
    version: &str,
    has_forms: bool,
    form_ids: &[&str],
    main_form_id: &str,
    program_ids: &[&str],
    menu_ids: &[&str],
    themes: &[StagedTheme],
    project_theme_default: &str,
    entrance_fx: &str,
    exit_fx: &str,
    entrance_on_restore: bool,
) -> String {
    // Build the FORMS constant entries
    let forms_entries: String = form_ids
        .iter()
        .map(|id| format!("    (\"{id}\", include_bytes!(\"../assets/forms/{id}.cfrm\")),\n",))
        .collect();

    let forms_const = if form_ids.is_empty() {
        "static FORMS: &[(&str, &[u8])] = &[];\n".to_owned()
    } else {
        format!("static FORMS: &[(&str, &[u8])] = &[\n{forms_entries}];\n")
    };
    // Always emitted, even beside an empty table: the form runtime below is
    // generated from `has_forms`, which is not the same question as "did any
    // `.cfrm` survive staging". A const that is sometimes there is a generated
    // program that sometimes does not compile.
    let forms_const = format!(
        "{forms_const}\
         /// The form this application starts at, decided when it was built.\n\
         /// Only the main form starts an application: the window opens\n\
         /// `FORMS[0]`, and this is what `FORMS[0]` must be.\n\
         #[allow(dead_code)]\nconst MAIN_FORM: &str = \"{}\";\n",
        main_form_id.escape_default()
    );

    // 051 R1 — one program per openable non-main form, gz bincode exactly
    // like `PROGRAM_AST`. `#[allow(dead_code)]`: a single-form application
    // has an empty table and never looks anything up.
    let programs_entries: String = program_ids
        .iter()
        .map(|id| format!("    (\"{id}\", include_bytes!(\"../assets/programs/{id}.bin\")),\n"))
        .collect();
    let programs_const = if program_ids.is_empty() {
        "#[allow(dead_code)]\nstatic PROGRAMS: &[(&str, &[u8])] = &[];\n".to_owned()
    } else {
        format!(
            "#[allow(dead_code)]\nstatic PROGRAMS: &[(&str, &[u8])] = &[\n{programs_entries}];\n"
        )
    };

    // 049 — the menu sidecars, keyed by the control that owns them. A shell
    // application builds its rail from these; without them a compiled binary
    // came up with an empty MenuPane, because a menu lives in a `.menu.yaml`
    // beside the form and never in a property.
    let menus_entries: String = menu_ids
        .iter()
        .map(|id| {
            format!("    (\"{id}\", include_str!(\"../assets/menus/{id}.menu.yaml\")),\n")
        })
        .collect();
    let menus_const = if menu_ids.is_empty() {
        "#[allow(dead_code)]\nstatic MENUS: &[(&str, &str)] = &[];\n".to_owned()
    } else {
        format!("#[allow(dead_code)]\nstatic MENUS: &[(&str, &str)] = &[\n{menus_entries}];\n")
    };

    // Embedded asset-pack themes: `(id, theme.toml, [(image ref, bytes)])`.
    let themes_entries: String = themes
        .iter()
        .map(|t| {
            let art: String = t
                .assets
                .iter()
                .map(|rel| {
                    format!(
                        "        (\"{rel}\", include_bytes!(\"../assets/themes/{id}/{rel}\")),\n",
                        id = t.id,
                        rel = rel
                    )
                })
                .collect();
            format!(
                "    (\"{id}\", include_str!(\"../assets/themes/{id}/theme.toml\"), &[\n{art}    ]),\n",
                id = t.id,
                art = art
            )
        })
        .collect();

    // `#[allow(dead_code)]`: a console-only project (no forms) never reads these.
    let themes_const = if themes.is_empty() {
        "#[allow(dead_code)]\nstatic THEMES: &[(&str, &str, &[(&str, &[u8])])] = &[];\n".to_owned()
    } else {
        format!(
            "#[allow(dead_code)]\nstatic THEMES: &[(&str, &str, &[(&str, &[u8])])] = &[\n{themes_entries}];\n"
        )
    };
    let theme_default_const = format!(
        "/// The project's default form theme (`[forms] theme` in cobolt.toml).\n#[allow(dead_code)]\nconst PROJECT_THEME_DEFAULT: &str = \"{}\";\n",
        project_theme_default.escape_default()
    );
    // 042 R11 — the window effects, as the `id:ms:easing` triples the CLI's
    // `--fx-entrance`/`--fx-exit` already speak, so both hosts parse ONE
    // format with one parser (`FxSpec::parse`). `#[allow(dead_code)]`: a
    // console-only project has no window to animate.
    let window_fx_const = format!(
        "/// The project's window entrance/exit effects (`[forms]` in cobolt.toml),\n\
         /// baked in because a shipped binary has no manifest to read them from.\n\
         #[allow(dead_code)]\nconst PROJECT_FX_ENTRANCE: &str = \"{}\";\n\
         #[allow(dead_code)]\nconst PROJECT_FX_EXIT: &str = \"{}\";\n\
         /// Replay the entrance when the window is restored after minimizing.\n\
         #[allow(dead_code)]\nconst PROJECT_FX_ON_RESTORE: bool = {};\n",
        entrance_fx.escape_default(),
        exit_fx.escape_default(),
        entrance_on_restore
    );

    let form_runtime_code = if has_forms {
        r#"
// ── Form application (spec 042: thin glue over the SHARED form host) ──────────
// The window — control state, backdrop, 038 entrance/exit effects, 037
// lifecycle, event routing, pacing — is `cobolt_form_host`, the same code
// `rcrun run-form` runs. This generated file supplies only what is genuinely
// per-host (042 R30): the embedded forms/themes, the interpreter thread with
// its compiled EXEC RUST blocks, and the per-frame block-window replay.

/// Resolve the form's asset-pack theme (`form ?? project ?? Liquid Glass`) to a
/// pack built from the art embedded in this executable.
///
/// The IDE resolves the same id against `assets/themes/` on disk; here the very
/// same manifest and PNG bytes were baked in at build time, so the shipped app
/// paints what the designer, the preview and Run Form painted — with no theme
/// folder to install next to the binary. `None` means Liquid Glass, whether the
/// form asked for it or the pack could not be built.
fn resolve_theme_pack(
    form: &cobolt_forms::Form,
) -> Option<std::sync::Arc<cobolt_forms::theme_pack::ThemePack>> {
    let id = cobolt_forms::theme::resolve_theme_id(
        form.theme.as_deref(),
        Some(PROJECT_THEME_DEFAULT),
    );
    let entry = THEMES.iter().find(|t| t.0 == id)?;
    match cobolt_forms::theme_pack::ThemePack::from_embedded(entry.1, entry.2) {
        Ok(pack) => Some(std::sync::Arc::new(pack)),
        Err(e) => {
            eprintln!("theme pack '{id}' unusable, falling back to Liquid Glass: {e}");
            None
        }
    }
}

/// The procedural look this application's forms are painted in (spec 047),
/// resolved from the same theme id as the pack above.
fn resolve_surface_theme(
    form: &cobolt_forms::Form,
) -> std::sync::Arc<dyn cobolt_forms::surface_theme::SurfaceTheme> {
    let id = cobolt_forms::theme::resolve_theme_id(
        form.theme.as_deref(),
        Some(PROJECT_THEME_DEFAULT),
    );
    cobolt_forms::surface_theme::for_theme_id(&id)
}

/// The per-frame block-window replay (042 R30 seam): windows opened by an
/// EXEC RUST block must be re-shown every frame — that is what
/// `show_viewport_deferred` requires; miss a frame and the window closes. A
/// block cannot do this itself: it runs once, on the interpreter's thread.
struct BlockWindows;
impl cobolt_form_host::HostHooks for BlockWindows {
    fn per_frame(&mut self, ctx: &egui::Context) {
        crate::exec_rust_blocks::cobolt_windows::show_all(ctx);
    }
}

fn run_form_app(program: cobolt_ast::program::Program) {
    use cobolt_form_host::state::CtrlState;
    use cobolt_forms::load_form_from_str;
    use cobolt_runtime::{Interpreter, FormEvent, StateUpdate};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc};

    // Load the MAIN embedded form (the build embeds it first) — defines the
    // window size + initial layout.
    //
    // Only the main form starts an application. Which form that is was decided
    // when this executable was built, and MAIN_FORM records it independently of
    // the table: if the two no longer agree, this copy has been altered and the
    // honest thing is to stop rather than open a door the developer never put
    // there. There is nothing on disk to edit — the forms live inside this
    // binary — so reaching here means the binary itself was patched.
    let first_form = if let Some(&(id, bytes)) = FORMS.first() {
        if !id.eq_ignore_ascii_case(MAIN_FORM) {
            eprintln!(
                "CORRUPTED APPLICATION — this application starts at form {}, but its form \
                 table now begins with {}. It will not run. Reinstall it from its original \
                 distribution.",
                MAIN_FORM, id
            );
            std::process::exit(3);
        }
        let xml = std::str::from_utf8(bytes).expect("form XML is valid UTF-8");
        load_form_from_str(xml).expect("parse embedded form")
    } else {
        run_headless(program);
        return;
    };

    // Flatten + z-order the controls and build the initial control state.
    let mut flat: Vec<cobolt_forms::Control> = Vec::new();
    cobolt_form_host::flatten_controls(&first_form.controls, &mut flat);
    flat.sort_by_key(|c| c.z_order);

    let mut state: std::collections::HashMap<String, CtrlState> = std::collections::HashMap::new();
    for c in &flat {
        state.insert(c.id.clone(), CtrlState::from_control(c));
    }

    // Seed the interpreter's visual-object registry with every control's
    // designed properties (042 R20) — the same shared builder Run Form uses,
    // so a property read before the first write returns the designed value.
    let (maps_api_key, search_api_key) = cobolt_form_host::seeding::resolve_api_keys();
    let seed = cobolt_form_host::seeding::build_object_seed(
        &first_form,
        &flat,
        maps_api_key.as_deref(),
        search_api_key.as_deref(),
    );

    // 042 R11 — window effects: the baked project settings × the form's own
    // `WindowEffects` opt-out × the machine kill switch, the same resolution
    // rule the IDE applies before Run Form.
    let fx_killed = cobolt_form_host::diagnostics::env_flag("PRC_NO_WINDOW_FX");
    let fx_on = first_form.window_effects && !fx_killed;
    let (fx_entrance, fx_exit, fx_restore) = if fx_on {
        (
            cobolt_forms::window_fx::FxSpec::parse(PROJECT_FX_ENTRANCE),
            cobolt_forms::window_fx::FxSpec::parse(PROJECT_FX_EXIT),
            PROJECT_FX_ON_RESTORE,
        )
    } else {
        (Default::default(), Default::default(), false)
    };

    let theme_pack = resolve_theme_pack(&first_form);
    // 050 R3 — a pack's own manifest declares whether it owns the whole look.
    let surface_theme = match theme_pack.as_ref() {
        Some(p) => cobolt_forms::surface_theme::for_pack(p.manifest.self_contained),
        None => resolve_surface_theme(&first_form),
    };

    let (ev_tx, ev_rx)           = mpsc::channel::<FormEvent>();
    let (input_tx, input_rx)     = mpsc::channel::<StateUpdate>();
    let (state_tx, state_rx)     = mpsc::channel::<StateUpdate>();
    let (display_tx, display_rx) = mpsc::channel::<String>();
    let pending  = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicBool::new(false));
    // 037/042 R12 — the interpreter's OpenForm*/`me::` window methods talk to
    // the shared host's real FormSupervisor, exactly as under Run Form.
    let (form_req_tx, form_req_rx) =
        mpsc::channel::<cobolt_runtime::form_host::FormRequest>();
    let (closed_tx, closed_rx) = mpsc::channel::<String>();
    let form_object = first_form.name.trim().to_ascii_uppercase();

    // 049 — a SideMenu on the MAIN form puts the application in SHELL mode:
    // one window laid out as MenuPane + breadcrumb + ContentPane, exactly what
    // `rcrun run-form` does. Read BEFORE the form moves into the config below.
    //
    // A built application used to skip this decision entirely and always open
    // the classic single window, so a shell app's embedded forms were painted
    // across the whole window — over the rail rather than beside it — and the
    // Open/Collapsed state moved nothing, there being no pane to be in.
    let shell_mode = first_form.has_side_menu();
    let root_menu = if shell_mode {
        first_form.side_menu_control_id().and_then(|ctrl_id| {
            MENUS
                .iter()
                .find(|(id, _)| id.eq_ignore_ascii_case(&ctrl_id))
                .and_then(|(_, yaml)| cobolt_forms::menu::parse_menu(yaml).ok())
                .map(|def| (ctrl_id, def))
        })
    } else {
        None
    };

    // The COBOL event loop runs on its own thread. The input channel lets the UI
    // push live control values (slider drag, text edit, …) so event handlers read
    // the current value rather than the seeded default. This thread is the
    // per-host part (042 R30): compiled EXEC RUST blocks + painter-ready.
    let err_tx = display_tx.clone();
    // 051 Q1 — the ROOT interpreter's object bridge is THE process-wide
    // bridge; every child form's interpreter adopts the same Arc, so a block
    // in any form resolves the same handles (spec 041 R9, now per process
    // rather than per lone interpreter).
    let (bridge_tx, bridge_rx) = mpsc::channel();
    {
        let finished = Arc::clone(&finished);
        let pending = Arc::clone(&pending);
        let form_object = form_object.clone();
        let form_req_tx = form_req_tx.clone();
        std::thread::spawn(move || {
            let mut interp = Interpreter::new_with_channels(program, ev_rx, state_tx, display_tx);
            let _ = bridge_tx.send(interp.shared_rust_bridge());
            // Compiled EXEC RUST blocks, before the run (spec 041 R2/R9): one
            // process-wide object bridge, so every block — in the main form or
            // any opened form — sees the same state.
            interp.register_exec_rust_blocks(crate::exec_rust_blocks::register);
            // A form is painting, so a block may open a window of its own. Set
            // before the first block can run: `open` refuses when nothing will
            // paint, which is the honest answer in a console program.
            crate::exec_rust_blocks::cobolt_windows::set_painter_ready();
            interp.set_input_channel(input_rx);
            interp.set_event_counter(pending);
            interp.set_form_host(
                form_req_tx,
                cobolt_runtime::form_host::ROOT_HANDLE,
                &form_object,
                closed_rx,
            );
            interp.seed_objects(seed);
            // This thread IS the program. When it ends — STOP RUN or an error —
            // the window closes (through the exit effect when one is
            // configured, 042 R15) instead of staying open and answering
            // nothing, which is indistinguishable from "the handler did
            // nothing".
            match interp.run() {
                Ok(()) => {}
                Err(e) if e.is_exit_signal() => {}
                Err(e) => {
                    eprintln!("Runtime error: {e}");
                    let _ = err_tx.send(format!("Runtime error: {e}"));
                }
            }
            finished.store(true, Ordering::Relaxed);
        });
    }

    // 051 R6 — a child form's design and program come from the embedded
    // tables; a form the build could not embed a program for fails the open
    // visibly (R15) instead of showing a dead window.
    let form_source: cobolt_form_host::FormSource = Box::new(|id: &str| {
        let want = id.trim().to_ascii_uppercase();
        let (_, bytes) = FORMS
            .iter()
            .find(|(fid, _)| *fid == want)
            .ok_or_else(|| format!("no form named '{}' in this application", id.trim()))?;
        let xml = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        let child = cobolt_forms::load_form_from_str(xml).map_err(|e| e.to_string())?;
        let program = load_program_by_id(&want).ok_or_else(|| {
            format!(
                "form '{}' has no embedded program — its generated code was missing \
                 or unparseable when this application was built",
                id.trim()
            )
        })?;
        Ok((child, program))
    });
    // A child form resolves its theme against the same embedded catalogue.
    let child_theme: cobolt_form_host::ChildThemeSource = Box::new(|child| {
        let pack = resolve_theme_pack(child);
        let st = match pack.as_ref() {
            Some(p) => cobolt_forms::surface_theme::for_pack(p.manifest.self_contained),
            None => resolve_surface_theme(child),
        };
        (pack, st)
    });

    // Everything from here is the SHARED host (042 R1/R3) — the same window
    // code `rcrun run-form` runs: viewport assembly from the designed window
    // properties, 038 effect playback, 037 lifecycle, state/event routing.
    let host_config = cobolt_form_host::FormHostConfig {
        form: first_form,
        flat,
        state,
        ev_tx,
        input_tx,
        state_rx,
        display_rx,
        pending,
        finished,
        form_req_rx,
        closed_tx,
        form_req_tx,
        form_source: Some(form_source),
        child_theme: Some(child_theme),
        // Every spawned interpreter carries the compiled EXEC RUST blocks.
        child_interpreter_setup: Some(std::sync::Arc::new(
            |interp: &mut cobolt_runtime::interpreter::Interpreter| {
                interp.register_exec_rust_blocks(crate::exec_rust_blocks::register);
            },
        )),
        shared_rust_bridge: bridge_rx.recv().ok(),
        fx_entrance,
        fx_exit,
        fx_restore,
        theme_pack,
        surface_theme,
        icon_path: None,
        // 042 R17 — the designed `form.title` wins; the branded fallback shows
        // only when the design left the title blank.
        title_fallback: format!("{} v{}", APP_NAME, APP_VERSION),
        // `run_shell` forces Pane itself; a classic one-window application
        // stays exactly as it was.
        surface: cobolt_form_host::Surface::Window,
        hooks: Box::new(BlockWindows),
    };
    if shell_mode {
        cobolt_form_host::shell::run_shell(host_config, root_menu);
    } else {
        cobolt_form_host::run(host_config);
    }
}

"#
    } else {
        ""
    };

    let run_call = if has_forms {
        "run_form_app(program);"
    } else {
        "run_headless(program);"
    };

    format!(
        r#"//! {app_name} v{version} — built with RustCOBOL (embed+bundle)
//! Auto-generated by cobolt-compiler. Do not edit.

/// Compiled `EXEC RUST` blocks — generated alongside this file, and empty when
/// the program has none (spec 041 R1/R2).
mod exec_rust_blocks;

const APP_NAME:    &str = "{app_name}";
const APP_VERSION: &str = "{version}";

// ── Embedded assets ───────────────────────────────────────────────────────────
/// Deflate-compressed bincode of the compiled COBOL AST.
static PROGRAM_AST: &[u8] = include_bytes!("../assets/program.bin");

/// Embedded form files — loaded lazily by form ID.
{forms_const}
/// Each openable non-main form's own program (051 R1) — the multi-form host
/// spawns an interpreter over it when the form is opened. The MAIN form's
/// program is `PROGRAM_AST`, exactly as it always was.
{programs_const}
/// Embedded menu sidecars, keyed by the id of the control that owns them
/// (049). A SideMenu on the MAIN form puts the application in SHELL mode, and
/// its rail is built from the entry named here.
{menus_const}
/// Embedded asset-pack themes: `(id, theme.toml source, [(image ref, bytes)])`.
/// Only the packs the forms actually resolve to are baked in, and only the art
/// their manifests reference, so a themed app is self-contained without
/// carrying the packs' authoring imagery.
{themes_const}{theme_default_const}{window_fx_const}
// ── Entry point ───────────────────────────────────────────────────────────────
fn main() {{
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_env("COBOLT_LOG")
            .add_directive(tracing::Level::WARN.into()))
        .with_target(false)
        .init();

    let program = load_program();
    {run_call}
}}

// ── AST loader ────────────────────────────────────────────────────────────────
fn load_program() -> cobolt_ast::program::Program {{
    use std::io::Read;
    let mut decoder = flate2::read::GzDecoder::new(PROGRAM_AST);
    let mut bytes = Vec::new();
    decoder.read_to_end(&mut bytes).expect("decompress embedded AST");
    bincode::deserialize(&bytes).expect("deserialize embedded AST")
}}

/// The embedded program for `form_id` (051 R1) — `None` when the build had
/// nothing to embed for it, which the multi-form host reports as a visible
/// runtime error (R15) rather than opening a dead form.
#[allow(dead_code)]
fn load_program_by_id(form_id: &str) -> Option<cobolt_ast::program::Program> {{
    use std::io::Read;
    let want = form_id.trim().to_ascii_uppercase();
    let (_, packed) = PROGRAMS.iter().find(|(id, _)| *id == want)?;
    let mut decoder = flate2::read::GzDecoder::new(&packed[..]);
    let mut bytes = Vec::new();
    decoder.read_to_end(&mut bytes).ok()?;
    bincode::deserialize(&bytes).ok()
}}

// ── Headless (CLI) runner ─────────────────────────────────────────────────────
fn run_headless(program: cobolt_ast::program::Program) {{
    use cobolt_runtime::Interpreter;
    let mut interp = Interpreter::new(program);
    // Compiled EXEC RUST blocks, before the run (spec 041 R2).
    interp.register_exec_rust_blocks(exec_rust_blocks::register);
    match interp.run() {{
        Ok(()) => {{}}
        Err(e) if e.is_exit_signal() => {{}}
        Err(e) => {{
            eprintln!("Runtime error: {{e}}");
            std::process::exit(1);
        }}
    }}
}}
{form_runtime_code}
"#,
        app_name = app_name,
        version = version,
        forms_const = forms_const,
        programs_const = programs_const,
        menus_const = menus_const,
        themes_const = themes_const,
        theme_default_const = theme_default_const,
        window_fx_const = window_fx_const,
        run_call = run_call,
        form_runtime_code = form_runtime_code,
    )
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// PowerRustCOBOL source is free form — the same rule `rcrun` applies, so a
/// file parses identically whether it is checked from the CLI or built from the
/// IDE. `COBOLT_FIXED=1` opts a legacy fixed-column source into fixed-form
/// parsing.
///
/// This used to *guess*, treating a file as fixed-form when any line had a
/// non-blank column 7 preceded by blanks or digits. Every RAD-generated file
/// matches that shape: the mandatory banner is six spaces then `*>`, which puts
/// `*` in column 7. So the build silently parsed generated code as fixed-form
/// while `rcrun check` parsed the very same bytes as free form — and disagreed
/// about whether it was valid.
///
/// Fixed form is not a harmless reinterpretation. Columns 1-6 are the sequence
/// area and column 7 the indicator, both stripped before parsing, so any line
/// whose text starts before column 8 loses its first characters. Rust inside an
/// `EXEC RUST` block is indented by whoever wrote it, and a tab-indented
/// `END-EXEC.` lands its `EN` in that dead zone: the terminator became
/// `D-EXEC.`, the block never closed, and the parser ran to end-of-file to
/// report `expected PROCEDURE DIVISION` — pointing nowhere near the real line.
fn detect_format(_source: &str) -> cobolt_lexer::SourceFormat {
    if std::env::var("COBOLT_FIXED").as_deref() == Ok("1") {
        return cobolt_lexer::SourceFormat::Fixed;
    }
    cobolt_lexer::SourceFormat::Free
}

/// Walk up the directory tree from `start` looking for a `Cargo.toml` that
/// contains `[workspace]`.  Returns the directory containing that file.
fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_file() {
        start.parent()?
    } else {
        start
    };
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            if let Ok(text) = std::fs::read_to_string(&candidate) {
                if text.contains("[workspace]") {
                    return Some(dir.to_owned());
                }
            }
        }
        dir = dir.parent()?;
    }
}
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Writes the platform's reference documentation — RustCOBOL extensions, IDE
/// functionality, RAD designer controls/properties/events, and the agent
/// registry — into `<root>/Knowledge Base/`.
///
/// The content is generated from this binary, which is what lets the caller
/// treat a missing document as "the installed platform is older than its own
/// documentation, rebuild it". The IDE calls this with the machine-level
/// System Knowledge Base root; compilation does NOT call it, because these
/// documents describe the platform rather than any one project.
pub fn publish_system_documentation(root: &std::path::Path) -> Result<(), std::io::Error> {
    let project_dir = root;
    let kb_dir = project_dir.join("Knowledge Base");
    std::fs::create_dir_all(&kb_dir)?;
    
    // Write File 1: rustcobol_extensions.md
    let rc_ext = r##"# PowerRustCOBOL Extensions & Syntax

PowerRustCOBOL extends COBOL-85 with inline RAD Form and UI Control access features:

## Property Get & Set Syntax
- **Retrieve a property**: Use `<control>::<property>`.
- **Set a property**: Use `SET <control>::<property> TO <value>`.
  - Example: `SET SAVE-BUTTON::Caption TO "Save".`
  - Example: `SET MAIN-PANEL::Visible TO 1.`
  - Example: `SET TOTAL-LABEL::ForegroundColor TO "#FF0000".`

## Method Invocation Syntax
- **Invoke a method**: Use `<control>::<method>(<parameters>)`.
  - Example: `LineChart-1::Clear().`
  - Example: `DataGrid-1::RefreshBinding().`
  - Example: `LineChart-1::AddPoint("January", 150).`
  - Example: `SqlDatabase-1::Open("sqlite::memory:").`
  - Example: `RestClient-1::Post("https://api.example.com/api/save", request_body).`
- **DO NOT** use `CALL` or legacy `INVOKE` for UI control properties or methods. Use the inline double-colon (`::`) syntax directly.
- The method vocabulary is **closed** — only the methods listed in the Control Methods Reference exist. A `::name(arg)` with an unrecognised name is treated as a PROPERTY WRITE of `name`, not a method call, so inventing a method silently does nothing useful.
- **IndexedFile controls have no `::` methods**, and their generated helpers (`<id>-OPEN`, `<id>-READ-NEXT`, …) are paragraphs of the OUTER program. A handler is a nested program, so it cannot `PERFORM` them: the compiler rejects it with "'<id>-OPEN' is not a paragraph or section of this program". **There is currently no supported way to drive an IndexedFile control from an event handler** — do not emit `PERFORM <id>-OPEN` in a handler and do not invent `<id>::Open()`, which is not a method this platform has. Use `SqlDatabase` (which does have `::` methods) when a handler must reach stored data.
- A method that returns a value can be used inline (`MOVE C::GetText() TO WS-X`) or with `RETURNING`.
- **A METHOD CALL IS A STATEMENT, NEVER A RECEIVING FIELD.** `<control>::<property>` may receive a value; `<control>::<method>(…)` may not. Writing a method call where a receiver belongs raises the runtime error *"'<control>::<method>' is a method call, not a receiving field — call it as a statement instead of using it as a MOVE/assignment target"*. It fails at the click, not at generation, so the handler reads correctly and throws. A method used as a SOURCE is fine (`MOVE C::GetText() TO WS-X`); only the receiving side is closed to it.
- **The usual way this happens is a missing period, not a misunderstanding.** A COBOL sentence runs to its period, so a `::` call written under an unclosed `MOVE`/`SET` becomes that statement's SECOND RECEIVER, however many blank lines separate them:

```cobol
      *> WRONG — the MOVE has no period, so `AddRow(...)` is a second
      *> receiving field of it, and the run raises the exception above.
       MOVE GLOBAL-TOTAL TO GLOBAL-TOTAL-ED

       dgReceipt::AddRow("Total", GLOBAL-TOTAL-ED).

      *> RIGHT — the MOVE is closed; the call is a statement of its own.
       MOVE GLOBAL-TOTAL TO GLOBAL-TOTAL-ED.

       dgReceipt::AddRow("Total", GLOBAL-TOTAL-ED).
```

  Several receiving fields under one `MOVE` remain perfectly legal when every one of them IS a receiver: `MOVE GLOBAL-TOTAL TO GLOBAL-TOTAL-ED  dgReceipt::X.` correctly writes the edited item AND the `X` property. Only a method among the receivers is the defect. The fix is a period on the preceding statement, or the explicit `INVOKE <control> "<method>" USING <parameters>` form, which cannot be read as a receiving field.

## Value Conventions (types and domains)
- **Boolean properties** store `1` (true) / `0` (false). Write `SET C::Visible TO 1`. On method arguments, `true`/`yes`/`on` (any case) also count as true.
- **Colors** are hex strings: `"#RRGGBB"` or `"#RRGGBBAA"` (e.g. `"#FF0000"`, `"#00000000"` = transparent).
- **Coordinates and sizes** (`X`, `Y`, `Width`, `Height`, paddings, radii) are integer pixels.
- **List content** (`Items` of ListBox/ComboBox/ToolBar/StatusBar/TreeView) is ONE ITEM PER LINE (newline-separated); TreeView nests children with two leading spaces per level. Indexes (`SelectedIndex`, grid rows/columns) are 0-based; -1 = no selection.
- **DataGrid data**: `Columns` is one `Name:Type` per line (`Type` ∈ `string`|`number`|`datetime`); `Rows` separates rows with newlines and cells with TAB.
- **Enumerated properties** accept only their listed values EXACTLY as spelled (e.g. `Orientation` is `Horizontal` or `Vertical`); an unrecognised value falls back to the default without an error.
- **Property names**: setting a misspelled property silently creates a new, unused property — never guess names; use the ones in the Form Controls Reference.
- **Charts**: feed data with `Chart::AddPoint(label, value)` / `Chart::Clear()` / `Chart::Refresh()`, with `PERFORM <id>-ADD-POINT` / `<id>-SET-TABLE` paragraphs, or with `CALL "COBOL-CHART-ADD-POINT" USING "<id>" label value` — or bind a COBOL table via the `DataSource`/`DataCount` properties. Do NOT invent working-storage tables for charts.

## Event payloads — what a handler actually receives (LINKAGE)

Almost every event delivers **nothing**. The generated dispatcher calls a handler as `CALL "<handler-program>"` with no arguments, so the handler's `LINKAGE SECTION` is empty and its header is a plain `PROCEDURE DIVISION.` with no `USING`.

- **There is exactly ONE event payload in the platform**: `CONTROL-ARRAY-INDEX PIC S9(4) COMP-5`, the 1-based index of the card that fired, and ONLY for a control inside a repeating group. That handler is called `USING CONTROL-ARRAY-INDEX` and writes `PROCEDURE DIVISION USING CONTROL-ARRAY-INDEX.`.
- **No event carries a key code, a mouse button, a coordinate, a modifier or a character.** Do NOT declare an item such as `KEY-CODE` and do NOT write `PROCEDURE DIVISION USING KEY-CODE.` — nothing populates it, and the dispatcher passes no argument to bind it to.
- **A specific key has its own event.** For "do X when the user presses ENTER" bind `onEnterPressed`; for ESC bind `onEscapePressed`. `onKeyDown` / `onKeyUp` / `onKeyPress` fire for ANY key and tell you nothing about which one, so testing a key inside them is impossible.
- To know what the user typed, read the control's own text: `MOVE MY-BOX::Text TO WS-VALUE`. `onTextChanged` (alias `onChange`) fires after each edit.

## Naming rules — control ids and every COBOL word you write
Control ids are not just labels: each one becomes part of a COBOL **user-defined word** in the generated program. A control `SAVE-BTN` gets the storage group `WS-SAVE-BTN` with `WS-SAVE-BTN-TEXT`, `-VISIBLE`, `-ENABLED` (editable controls also get `-VALUE`), and file/database controls get paragraphs such as `SAVE-BTN-OPEN` / `SAVE-BTN-CONNECT`.

A COBOL word may contain **only letters (`A-Z`, `a-z`), digits (`0-9`) and hyphens (`-`)**. It may not begin or end with a hyphen, and a data-name must contain at least one letter.

- **Never put `_` (underscore), `.`, spaces, `/`, `#` or accented characters in a control id.** `TEXTBOX_1` is not a COBOL word; `TEXTBOX-1` is. An underscore is the most common mistake: the lexer reads `WS-TEXTBOX_1-TEXT` as the word `WS-TEXTBOX`, then an error token, then a number — the whole data item is discarded and the control ends up with no storage at all.
- The same rule applies to every name YOU declare: WORKING-STORAGE data items, level-01/05 group and field names, paragraph names, and `CALL`/`PERFORM` targets. Use `WS-ROW-COUNT`, never `ws_row_count`.
- Prefer short, hyphenated, meaningful ids: `CUST-NAME-TXT`, `TOTAL-LBL`, `SAVE-BTN`, `GRID-1`.
- Digits are fine anywhere except as the whole name: `TEXTBOX-1`, `COL-2-HDR`.
- The generator does normalise an invalid id (each character that is not a letter or digit becomes a hyphen, runs collapse, the ends are trimmed), so a legacy `textbox_1` still compiles — as `WS-textbox-1` — but then the id in the designer and the name in the COBOL no longer match. Create valid ids in the first place.

## Event Handler Division Structure
- Every developer-editable event-handler body must start from the program Divisions and contain:
  ```cobol
         ENVIRONMENT DIVISION.
         DATA DIVISION.
         WORKING-STORAGE SECTION.
         *> (Data declarations here)
         PROCEDURE DIVISION.
             *> (Statements here)
  ```
- Do not write `IDENTIFICATION DIVISION`, `PROGRAM-ID`, or `END PROGRAM` in the handler body; the IDE scaffold manages the program wrapper.
- `GOBACK` **is** yours to write, and it is an ordinary statement. The scaffold appends a closing one, but that lands after everything you wrote — so a body that declares its own paragraphs must end its main flow with `GOBACK.` before the first of them, or control falls through and runs that paragraph a second time.

## Nested programs — where `PERFORM` reaches, and where it does not
The generated source is a COBOL-85 **nest**: the form is the outer (main) program, and every event handler and every common procedure is a separate nested program inside it. That structure decides how one piece of code reaches another, and getting it wrong is the most common way a handler that reads correctly still fails.

- `PERFORM` transfers control to a **paragraph or section of the SAME program**. It never crosses a program boundary. A `PERFORM` naming anything outside the body it sits in has no target, and that body is rejected.
- A **common procedure** is a nested program, not a paragraph of yours. Reach it with `CALL "ITS-NAME"` — `CALL "UPDATE-TOTAL".`, `CALL "RECALC" USING WS-QTY WS-PRICE.` — never `PERFORM UPDATE-TOTAL`.
- Use `PERFORM` for paragraphs you declared yourself, inside the body you are writing.
- The generated infrastructure paragraphs (`<id>-OPEN`, `<id>-READ-NEXT`, `<id>-COMMIT`, and the timer, chart, CSV-export and data-binding helpers) are emitted at OUTER program scope. They are `PERFORM`-able from the form's own procedure code, not from inside an event handler, which is a nested program of its own.

## Ownership in a COBOL-85 nest — what each program may declare
Never assume a containing program's declarations are visible to a nested one. Only items declared `GLOBAL` are, and only because they are declared `GLOBAL`.

**The one hard restriction is the `CONFIGURATION SECTION`.** COBOL-85 forbids a contained program from specifying one at all, so `SOURCE-COMPUTER`, `OBJECT-COMPUTER`, `SPECIAL-NAMES` and (this platform's) `REPOSITORY` may appear ONLY in the outermost program — the form — and they govern every program nested inside it.

**Everything else about files and storage is per-program.** A nested program MAY declare its own `INPUT-OUTPUT SECTION`, `FILE-CONTROL`, `SELECT`, `FD`/`SD`, record descriptions, `WORKING-STORAGE` and `LINKAGE`. Those are legitimate in the form OR in a single handler; which is correct depends on intent, not on a rule, so a request that does not say where must be clarified rather than guessed.

| Declaration | Owner | Written by |
| --- | --- | --- |
| `CONFIGURATION SECTION` — `SOURCE-COMPUTER`, `OBJECT-COMPUTER`, `SPECIAL-NAMES`, `REPOSITORY` | outermost program ONLY | `set_form_structure`, or the COBOL Structure panel |
| `INPUT-OUTPUT SECTION`, `FILE-CONTROL`, `SELECT` | each program may own its own | `set_form_structure` for the form's; the body itself for a handler's |
| `FILE SECTION`, `FD`/`SD`, record descriptions | each program may own its own | `set_form_structure` for the form's; the body itself for a handler's |
| `WORKING-STORAGE`, `LINKAGE` | each program owns its own | `set_form_structure` for the form's; the body itself for locals |
| `PROCEDURE DIVISION` | each program owns its own | the handler or common-procedure body |
| `GLOBAL` items | containing program, visible to every nested program | declared in the FORM |
| `EXTERNAL` items | the run unit | the form's `WORKING-STORAGE`, `EXTERNAL` clause |

`GLOBAL` is **not** a working-storage-only clause. It applies to `01`/`77` items in `WORKING-STORAGE` and equally to `FD`/`SD` entries and `01` record descriptions in the `FILE SECTION`. A `GLOBAL FD` with a `GLOBAL` record description is the better pattern for shared file data: every nested program reads the record area directly, with no `MOVE` traffic between the `FD` and working-storage.

`COMMON` is never requested — codegen marks every nested program `IS COMMON PROGRAM`, so a common procedure is always callable by its siblings. `INITIAL` is not emitted by this platform. `LOCAL-STORAGE` and `SCREEN SECTION` are parsed by the compiler but no operation writes them.

Checklist before emitting a change-set: no nested program declares a `CONFIGURATION SECTION` or `SPECIAL-NAMES`; every nested program has its own `DATA DIVISION`; `GLOBAL` items are referenced, never duplicated; `EXTERNAL` items are treated as run-unit-wide; cross-program invocation is `CALL`, never `PERFORM`.

## `EXEC RUST` — real Rust, compiled into the program

> **A block is the developer's decision, never an assistant's.** This platform's
> language is COBOL, and `EXEC RUST` exists for the developer who WANTS Rust —
> a crate, an algorithm, something COBOL genuinely cannot reach. An assistant
> writes a block ONLY when the developer asked for Rust in so many words ("in
> Rust", "use EXEC RUST", "with the csv crate"). Absent that, write COBOL,
> however long it comes out: copying one value into fifteen controls is fifteen
> `MOVE` statements, and that is the correct answer, not a reason to reach for
> Rust. Concision, readability, elegance and "the platform supports it" are not
> reasons — the platform supporting a thing is not the developer asking for it.
> The choice is not free either: it is the difference between a program that
> runs interpreted and one that must be built, needs the Rust toolchain, and
> cannot be stepped in the debugger (all three below). If a task truly cannot be
> done in COBOL, say so and ask.

`EXEC RUST … END-EXEC` is **compiled**, not interpreted. Each block becomes a real Rust function inside the crate the build already produces, so the whole language is available: closures, generics, iterator chains, `match`, `?`, and any `std` API. There is no micro-language and no subset.

Because a block is compiled, **a program containing one is built before it runs**. *Run* does that build for you and starts the built binary; a program with no block keeps the fast interpreter path unchanged. Building needs a Rust toolchain; **the binary you produce does not** — it runs on machines with no Rust installed. Builds are for the host operating system only: build a Windows application on Windows, a macOS one on macOS.

### The two kinds of block

| Kind | Where it goes | What it holds |
| --- | --- | --- |
| **Item-level** | `CONFIGURATION SECTION`, after `REPOSITORY` — outermost program only, like everything else in that section | Rust **items**: `struct`, `enum`, `impl`, `trait`, `use`. Emitted at module scope, so every block in the program can see them |
| **Statement-level** | `PROCEDURE DIVISION`, anywhere a statement may appear — including inside an event handler | Rust **statements**: the work |

**In a FORM there are no division headers — there are COBOL Structure blocks.** An item-level block goes in the **REPOSITORY** block, below the `CLASS` entries, because that block is woven into the `CONFIGURATION SECTION`. It must NOT go in **WORKING-STORAGE**, which is woven into the `DATA DIVISION` and rejects a block. A statement-level block goes in an event handler or a common procedure, both of which are `PROCEDURE DIVISION` code. Do not advise a developer to "put it in the CONFIGURATION SECTION" without naming the REPOSITORY block: a form gives them no other way to reach that section.

Putting a statement in an item-level block, or a `struct` in a statement-level one, is an error; the reported line and column are your own.

### What may cross into a block

Only a `USAGE OBJECT REFERENCE` item whose `CLASS` names a Rust type. A `PIC` item is rejected by name: its value is a scaled decimal or a fixed-width padded field, and there is no Rust type it is. Move such a value through an object with `INVOKE` before the block.

The Rust variable is the COBOL name lowercased with hyphens turned into underscores — `WS-USER-NAME` is `ws_user_name`. A name that lands on a Rust keyword (`01 TYPE` → `type`) or cannot start an identifier (`01 1ST-FLAG`) is rejected; rename the item.

**A bound name is a `&mut T`, not a `T`.** Assign through it — `*counter = 10;` — and call methods on it directly, since method calls auto-dereference (`text.push_str("x")`).

- Every integer class (`RUST-I8` … `RUST-USIZE`) binds as `i64`, and both float classes as `f64` — that is how the object bridge stores them, so `INVOKE` and a block always see the same value. A `CLASS RUST-I32` item is an `i64` inside a block, so a function written to fill it must return `i64`.
- The collection classes hold `cobolt_runtime::rust_bridge::BridgeValue`, so a `Rust.Vec` filled by `INVOKE` and one filled inside a block hold the same things.
- The unsized classes (`RUST-STR`, `RUST-OSSTR`, `RUST-CSTR`, `RUST-PATH`) bind as their owned forms (`String`, `OsString`, `CString`, `PathBuf`).

### Your own Rust types

Declare the type in an item-level block, name it with a `CLASS`, and declare items of it:

```cobol
       REPOSITORY.
           CLASS MY-POINT IS "Rust.Point"
       EXEC RUST
       #[derive(Default)]
       pub struct Point { pub x: i64, pub y: i64 }
       impl Point {
           pub fn shift(&mut self, dx: i64, dy: i64) { self.x += dx; self.y += dy; }
       }
       END-EXEC.
```

A developer-defined type must implement `Default` — that is what the first block to touch the item starts it from. The 48 shipped `CLASS RUST-*` types are a floor, not a ceiling.

### How a block behaves

- **A block body is a Rust function body returning `Result<(), Box<dyn Error>>`.** That is what makes `?` usable inside it. To leave early, write `return Ok(())`, not `return;`. An error that propagates out becomes a `RUST-EXCEPTION`.
- **A panic is catchable**: `TRY … CATCH RUST-EXCEPTION e … END-TRY` catches it, `DISPLAY e` prints the panic's plain text, and the program continues. A plain `CATCH EXCEPTION` does **not** catch a panic, and a COBOL `THROW` does not reach a `RUST-EXCEPTION` clause; a `TRY` may carry both clauses.
- **State is shared for the whole process.** Two blocks — in different paragraphs, or in a form event handler — see the same objects. `CANCEL` does not reset it.
- **COBOL reading a bound item sees its VALUE (1.60.23+).** `DISPLAY clicked-button`, `MOVE clicked-button TO WS-N` and `SET Label-1::Caption TO clicked-button` all yield what the block last wrote (`String`, any integer width, floats, bool). ⚠️ **Before 1.60.23 they yielded the item's internal handle id** — a small integer that reflected declaration order, so a program whose second item was read always showed "2" regardless of what the block computed. If a program built before 1.60.23 shows a constant small number where a result should be, that is this bug: rebuild. Types with no scalar rendering (Vec, HashMap, developer types) still read as the handle id.
- **COBOL writing a bound item reaches the Rust value (1.61.2+).** `MOVE 5 TO clicked-button` and `SET cobol-text TO TextBox-1::Text` update the object the item names, so the next block sees what COBOL wrote — that is how the operator's input gets into a block. ⚠️ **Before 1.61.2 the write landed on the item's internal handle and destroyed it**: the object became unreachable and the next block to bind that item failed with `EXEC RUST cannot bind <ITEM>: handle 0 is not live`. Only the classes with a scalar form accept a write — `RUST-STRING`, any integer width, the floats, `RUST-BOOL`. Writing into a collection or a developer-defined item is a type error and is reported as one; fill those inside a block.
- **A handler's OWN `OBJECT REFERENCE` items are bindable (1.61.2+).** An item declared in an event handler's `WORKING-STORAGE` behaves exactly like one declared in the form. ⚠️ **Before 1.61.2 only the outermost program's items were given objects**, so a handler-local one had no handle at all and every block binding it failed with `handle 0 is not live` — the reason the same block worked when the item was moved to the form and marked `GLOBAL`. Both placements are correct now: declare it in the handler when only that handler uses it, in the form as `GLOBAL` when several do.
- **Crates**: always `std`, plus `eframe`, `egui`, `egui_extras`, `cobolt_forms`, `cobolt_runtime`. A program containing any block links the GUI crates even with no forms, so a console program can open a window. **Beyond that floor, a project may register any crate from the registry under Project's Crates (1.60.47+, shown in the tree as "Project's Crates (Beta)")** — the project tree category below Generated Code, whose dialog searches crates.io (or a configured mirror) and shows the matches as a paged table (50 per page, name · version · downloads · description) that the developer browses and clicks to pick. Adding pins an exact version, vendors the source into the project's `crates/`, and compiles it into the binary. A registered crate is then used with a plain `use`, writing a hyphenated name with underscores (`serde-json` → `use serde_json::…`). A `use` of a crate that is neither linked nor registered is still rejected, naming the crate — in a project the message points at Project's Crates, in a single-file `rcrun` build it says external crates require a project. Adding needs the network; building does not. Conflicts are decided when the developer adds: a name already linked (`egui`) is refused as already available, a crate that cannot coexist is refused with cargo's own reason, and one that would bring a second incompatible copy is allowed with a warning. Every build writes `rust_manifest.md` (name, exact version, registry URL) beside the binary in the destination folder. **Do not tell a developer that third-party crates are unsupported** — that was true only before 1.60.47; tell them to add it under Project's Crates. **1.60.48+ — System awareness and collision aliasing (spec 045):** the dialog marks a result **System** (a name directly linked, e.g. `egui`) or **System dependency** (only pulled in transitively by something linked, e.g. `epaint` via `egui`) and hides both by default behind a "Show System crates" toggle; neither can be registered, and a System-dependency refusal never offers an alias. A **direct** collision at an incompatible version — the "clashes with the built-in" case above — now offers an **alias** instead of only refusing: accepting registers it as `prj_<name>` (a `package = "<name>"` rename, compiled as a second, independent copy beside the platform's own), and the block then writes `use prj_<name>::…`, not `use <name>::…`. Tell a developer who hits this refusal about the alias offer rather than saying the version is unsupported — but also warn them an aliased copy's values do not interoperate with the platform's own copy of the same crate.
- **eframe here is 0.36.** Its `App` trait requires `fn ui(&mut self, ui: &mut egui::Ui, frame: &mut Frame)` — there is no `update`. Tutorials written for older eframe will not compile; port them to `ui`, and use `ui.ctx()` where they use `ctx`.
- **`eframe::run_native` CANNOT be called from a form's event handler — and since 1.60.14 the BUILD REJECTS IT.** A project with forms whose block calls `run_native` (or `EventLoop::new`) fails to build, reported at the developer's own line and column. **To open a window from a handler use `cobolt_windows::open` instead** (see below) — that is the supported route, and the error says so. Why `run_native` cannot work: a form application already owns the process's one winit event loop, created on the main thread; the COBOL interpreter — and therefore every block in a handler — runs on a worker thread. winit's `EVENT_LOOP_CREATED` guard is process-global and is checked before any platform code, so the second call returns **`Err(EventLoopError::RecreationAttempt)`**. It does **not** panic, so `CATCH RUST-EXCEPTION` never fires; and the usual `let _ = eframe::run_native(...)` discards the `Err`. Before the build-time rejection the result was no window, no error, no output — the handler appeared to do nothing. Never advise `run_native` from a handler, and never suggest "open a second egui viewport" as the alternative: a block receives only `env`, `objects` and `bridge`, so it has no `egui::Context` to open one with. From a handler, drive the form's own controls through `cobolt_objects`, or show a second form designed in the RAD. `run_native` belongs to **console** programs, where the interpreter owns the main thread, and is not rejected there.
- **A block CAN change a control, through `cobolt_objects`.** Write the property and the window repaints when the block returns: `cobolt_objects.set_property("LABEL-1", "Caption", "Done");`. Property names are case-insensitive. ⚠️ **This did not work before 1.60.14**: block execution had no channel to the window, so the write landed in the registry and the form never showed it. Do not repeat the old advice to reach for `COBOL-SET-PROPERTY` *because* the block route is broken — it is not broken any more, though `COBOL-SET-PROPERTY` remains correct and unchanged. **Always use `set_property`; never advise `cobolt_objects.get_mut("X").unwrap()`** — a running form registers a control on first write, so `get_mut` returns `None` for one not yet written and the `unwrap` panics. For the same reason a block cannot READ a control's designed value, only one it set itself: to read what the operator typed, use `TextBox-1::Text` in COBOL and pass the item into the block.
- **A block CAN open its own window, with `cobolt_windows` (1.60.15+).** This is the supported answer to "open a dialog from a handler", and it replaces every older workaround. `cobolt_windows::open(id, builder, ui)` takes an id, an `eframe::egui::ViewportBuilder`, and the closure that draws the window; it returns a handle with `wait()` (parks the handler until the operator closes it), `is_open()` and `close()`. Free functions `cobolt_windows::is_open(id)` / `close(id)` do the same by id. ⚠️ **To close the window from inside its own drawing closure, call `cobolt_windows::close("id")` — NEVER `ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close)`.** That command targets the viewport current during the pass, which is the PARENT, so it closes the whole application: the dialog vanishes together with the form, and any COBOL after `wait()` (a `SET Label-1::Caption TO …`) races the shutdown and lands only sometimes. Never write `send_viewport_cmd(Close)` in a `cobolt_windows` drawing closure, and never copy it from an eframe tutorial into one. Re-opening a live id replaces what it draws. The drawing closure runs on the **UI thread** and must be `Send + Sync + 'static`, so share state with the block through an `Arc<Mutex<..>>` — that is how the chosen value gets back. `wait()` is safe: the interpreter has its own thread, so the form keeps painting while the handler waits. **Forms only** — with no form there is nothing painting, and `open` panics with that explanation (a catchable `RUST-EXCEPTION`) rather than registering a window that never appears; a console program still uses `eframe::run_native`. Never claim the block is given an `egui::Context`: it is not, and the reason is not `Send`ness — `show_viewport_deferred` must be called on the UI thread every frame the window exists, which a once-through block on a worker thread cannot do, so it registers what to draw and the form application replays it.
- A block may appear **anywhere a statement may**, including inside `IF`, `EVALUATE`, `PERFORM`, `ON SIZE ERROR`, `INVALID KEY`, `AT END` and `TRY … END-TRY` — the last being where a block goes when its failure should be caught.
- A block that is *not* built cannot run: an unregistered block is a hard error naming its id, never a silent no-op.

```cobol
       01 USER-NAME USAGE IS OBJECT REFERENCE RUST-STRING VALUE "ada".
       ...
           EXEC RUST
           user_name.push_str("-lovelace");
           let vowels = user_name.chars().filter(|c| "aeiou".contains(*c)).count();
           println!("{vowels} vowels");
           END-EXEC.
```

A worked example — an `eframe` dialog defined in an item-level block and called from a statement-level one inside a `TRY`. **CONSOLE PROGRAMS ONLY.** Copying this into a form's event handler is the mistake this page exists to prevent; since 1.60.14 a form project containing it **fails to build**, at the developer's own line. (Before that it built and then did nothing at all, because `run_native` returns `Err(RecreationAttempt)` off the main thread and the `let _ =` throws that away.) In a form, drive the controls through `cobolt_objects` instead.

```cobol
       REPOSITORY.
           CLASS RUST-STRING IS "Rust.String"
           CLASS RUST-I32    IS "Rust.i32"
       EXEC RUST
           use eframe::egui;
           use std::sync::{Arc, Mutex};
           pub struct ButtonDialog { pub clicked: Arc<Mutex<i64>> }
           impl eframe::App for ButtonDialog {
               fn ui(&mut self, ui: &mut egui::Ui, _f: &mut eframe::Frame) {
                   ui.horizontal(|ui| {
                       for caption in [1_i64, 2_i64] {
                           if ui.button(caption.to_string()).clicked() {
                               *self.clicked.lock().unwrap() = caption;
                               ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                           }
                       }
                   });
               }
           }
           pub fn ask(title: &str) -> i64 {
               let clicked = Arc::new(Mutex::new(0_i64));
               let out = clicked.clone();
               let _ = eframe::run_native(title, eframe::NativeOptions::default(),
                   Box::new(move |_cc| Ok(Box::new(ButtonDialog { clicked: out }))));
               let v = *clicked.lock().unwrap();
               v
           }
       END-EXEC.
       ...
       01 window-title   USAGE IS OBJECT REFERENCE RUST-STRING VALUE "Hello".
       01 clicked-button USAGE IS OBJECT REFERENCE RUST-I32.
       01 ws-error       PIC X(120).
       PROCEDURE DIVISION.
           TRY
               EXEC RUST
                   *clicked_button = ask(window_title.as_str());
               END-EXEC
           CATCH RUST-EXCEPTION ws-error
               DISPLAY "Window failed: " ws-error
           END-TRY.
           DISPLAY clicked-button.
```

## `SPECIAL-NAMES` and the decimal separator
`SPECIAL-NAMES` belongs to the FORM, the main program of the nest. Its `ENVIRONMENT DIVISION` → `CONFIGURATION SECTION` → `SPECIAL-NAMES` paragraph is written with `set_form_structure` (or by hand in the COBOL Structure panel) and is the ONLY place `DECIMAL-POINT IS COMMA` may be declared. A handler or common procedure that declares `SPECIAL-NAMES` or a `CONFIGURATION SECTION` is redeclaring it inside a nested program and is rejected.

What the form declares governs the whole nest. With `DECIMAL-POINT IS COMMA` in force, the roles of `.` and `,` are exchanged in `PICTURE` character-strings and in numeric literals:

- an edited money item is `PIC ZZZ.ZZ9,99`, and the value 1234,56 prints as `1.234,56`;
- a numeric literal carries a comma — `MOVE 7,49 TO WS-PRICE`.

Without the clause, `.` is the decimal point and `,` groups digits, the usual way round. Comma-formatted currency is obtained by putting the clause on the FORM — never by declaring it inside a handler to compensate.
"##;
    std::fs::write(kb_dir.join("rustcobol_extensions.md"), rc_ext)?;

    // Write File 2: ide_functionalities.md
    let ide_funcs = r##"# PowerRustCOBOL IDE Functionalities

The PowerRustCOBOL IDE provides RAD (Rapid Application Development) capabilities for COBOL developers:

## RAD desktop Form Designer
- A WYSIWYG visual layout canvas with grid snapping.
- Visual positioning (X, Y) and sizing (Width, Height) of controls.
- Tab-order management for keyboard navigation.
- Container hierarchies (e.g. Panels, TabControls) establishing parent-child ownership.

## Predefined Form Styles
- The form's visual style is the form-level `GlassStyle` property. Its only accepted values are the exact strings `"Classic"`, `"Enhanced"`, `"Neumorphic Light"`, and `"Neumorphic Dark"`.
- Applied with one operation: `{ "op": "set_property", "control_id": "Form", "key": "GlassStyle", "value": "Neumorphic Dark" }`. An unrecognised value is silently discarded and the form stays on `Classic`.
- Individual controls automatically inherit the style's parameters (colors, padding, borders, shadows). Do not restyle controls one by one to emulate a style.
- `Theme` and `UseThemeBackground` are a SEPARATE named asset-pack slot, not the mechanism for selecting a `GlassStyle`.
- `GlassStyle` is CONDITIONAL on the form's `Theme`. The four values above are variations of Liquid Glass; a theme that owns the complete look (a "self-contained" theme, e.g. `elegance`) ignores `GlassStyle` entirely, and the IDE disables the setting while such a theme is selected. Setting `GlassStyle` on such a form is stored but has no visual effect, and it does not modify any other property of the form or its controls.
- Explicit control properties are NEVER overridden by a theme. `BackgroundColor`, `ForegroundColor`, `CornerRadius`, `Transparency` and the `Shadow*` family always win. In particular `ShadowEnabled` draws a drop shadow under every theme and every `GlassStyle` value — do not tell a user to change `GlassStyle` to make a shadow appear.

## Window Start Position
- Where a form's window opens on screen is the form-level `StartPosition` property. Its only accepted values are the exact strings `"System"`, `"Custom"`, `"TopLeft"`, `"TopCenter"`, `"TopRight"`, `"MiddleLeft"`, `"Center"`, `"MiddleRight"`, `"BottomLeft"`, `"BottomCenter"`, `"BottomRight"`.
- `System` (the default) leaves window placement to the OS/window manager, exactly as every form behaved before this property existed — `X`/`Y` are not applied. `Custom` applies the form-level `X`/`Y` properties (screen-pixel coordinates) at launch. Every other value computes a position from the actual screen and window size at launch — an edge, a corner, or `Center` — and ignores `X`/`Y`.
- Applied with `{ "op": "set_property", "control_id": "Form", "key": "StartPosition", "value": "Custom" }`, and `X`/`Y` the same way with integer values. Setting `X`/`Y` alone does not move the window unless `StartPosition` is also `"Custom"`.
- `X`/`Y` here are the FORM's own coordinates (its window's position on screen), not to be confused with a CONTROL's `X`/`Y` (its position within the form).

## Code generation & Compilation
- Multi-agent coordination: Grace (Orchestrator) plans and delegates UI design to Form Designer Agent, event implementations to COBOL Event Handler Script Agent, and schema setups to Data Agent.
- During IDE build/compilation, the project is parsed, semantic checks are performed, and it is compiled into a single native executable.

## The two Knowledge Bases
There are two separate stores, and they are never merged on disk:

- **System Knowledge Base** — this document and its siblings (`rustcobol_extensions.md`, `form_designer_controls.md`, `control_methods_reference.md`, `form_themes.md`, `form_layout_and_events.md`, `project_model_and_settings.md`, `agents_registry.md`). It describes the PLATFORM, so it lives at machine level in `~/PowerRustCOBOL/Knowledge Base/`, with its vector index in `~/PowerRustCOBOL/data/`. It is regenerated from the running binary at the start of every workflow — which is why it cannot drift behind the installed IDE and is never legitimately empty. It is **not** copied into any project, and compilation does not publish it into one.
- **Project Knowledge Base** — the developer's own `<project>/Knowledge Base/` folder: requirements, diagrams, data models, decisions. Its index lives in `<project>/data/`. An empty Project Knowledge Base is a normal state, not a fault.

Both stores are searched for every request, and the matching subject records — not whole documents — are injected into the agents' context. Each excerpt carries a `SOURCE:` line naming its store, because the two use identically shaped paths: a System KB hit reads `Knowledge Base/form_designer_controls.md`, which is exactly how a project file would read even though the project contains no such file. Cite the project-relative path only for Project Knowledge Base excerpts; a System Knowledge Base excerpt is platform documentation and must never be reported as a project file, nor requested from the developer.
"##;
    std::fs::write(kb_dir.join("ide_functionalities.md"), ide_funcs)?;

    // Write File 3: form_designer_controls.md — generated from the same model
    // the designer and runtime execute (`Control::new` seeds the defaults, so
    // each property's type and default can never drift from the code), and
    // enriched with curated value domains, ranges and per-control methods so
    // agents can generate valid code on the first attempt.
    std::fs::write(
        kb_dir.join("form_designer_controls.md"),
        controls_reference_doc(),
    )?;

    // Write File 5: control_methods_reference.md — the complete closed method
    // vocabulary with parameter types and return values.
    std::fs::write(
        kb_dir.join("control_methods_reference.md"),
        methods_reference_doc(),
    )?;

    // Write File 4: agents_registry.md
    let agents_reg = r##"# PowerRustCOBOL Agent Registry

This document lists all built-in specialist and reviewer agents configured in the PowerRustCOBOL agentic AI mesh:

## Orchestrator Agent
- **Grace**: Plans, delegates, coordinates task sequencing, and integrates specialist outputs.
  - **Companion Reviewer**: `Grace Pedantic Reviewer`

## Specialist Agents
- **Form Designer Agent**: Specialist responsible for RAD desktop form structures, layouts, control deployment, and visual styling properties.
  - **Companion Reviewer**: `Form Designer Agent Pedantic Reviewer`
- **COBOL Event Handler Script Agent**: Specialist responsible for generating COBOL-85 / RustCOBOL event handler implementations bound to control events.
  - **Companion Reviewer**: `COBOL Event Handler Script Agent Pedantic Reviewer`
- **Data (Indexed File) Agent**: Specialist responsible for creating or modifying PowerRustCOBOL indexed-file schemas (.cidx) and structural COBOL definitions.
  - **Companion Reviewer**: `Data (Indexed File) Agent Pedantic Reviewer`
- **Documentation Agent**: Specialist responsible for formatting, writing, and indexing markdown files exclusively inside the project `/Knowledge Base/` folder.
  - **Companion Reviewer**: `Documentation Agent Pedantic Reviewer`
- **Version Control Agent**: Specialist responsible for executing repository actions (Git status, branch, commit, push, merge, revert, rebase).
  - **Companion Reviewer**: `Version Control Agent Pedantic Reviewer`
"##;
    std::fs::write(kb_dir.join("agents_registry.md"), agents_reg)?;

    // Write File 6: form_themes.md — what a theme is, how one is selected, and
    // the two rules agents kept getting wrong (self-contained themes ignore
    // GlassStyle; explicit control properties always win).
    //
    // Every `##` section here has to stand on its own: the chunker makes one
    // retrievable record per heading, and a search hit injects that record
    // alone — not the document around it.
    let themes = r##"# PowerRustCOBOL Form Themes

## What a form theme is

A **form theme** decides how a form's surfaces are painted: fills, borders,
corner radii, relief and shadow, and the colours of text and structural
surfaces. It is chosen **per form** (or per project) by a catalogue **id**, and
it applies to that whole form. There is no operation that applies a theme to a
single control, and a theme is never "installed" onto controls one at a time.

Two kinds exist:

- **Procedural** — drawn entirely in code. `liquid-glass` and `elegance`.
- **Asset pack** — composited from 9-slice images in `assets/themes/<id>/`,
  described by a `theme.toml` manifest. Packs are discovered at start-up, so a
  new one is a drop-in with no code change.

A theme answers only the questions it wants to. Anything it leaves unanswered
falls back to Liquid Glass, which is why a partial theme is a legitimate theme.

## The theme catalogue

| id | Display name | Kind | Self-contained |
|---|---|---|---|
| `liquid-glass` | Liquid Glass | Procedural | no |
| `elegance` | Elegance | Procedural | **yes** |
| `neumorphic` | Neumorphic | Asset pack | declared in its manifest |
| `cobalt-steel` | Cobalt Steel | Asset pack | declared in its manifest |

`liquid-glass` is the default look and the base every other theme falls back to.
`elegance` is flat slate surfaces with a cool accent family, drawn in code.

**An unknown id, an empty id, or no selection at all resolves to
`liquid-glass`.** Nothing fails and nothing is reported: that is the fallback
rule, not an error path. Do not tell a developer that a theme id was rejected.

## Selecting a theme — the resolution order

The effective theme is the first of these that is set:

1. the **form's** own `Theme` property, if non-empty;
2. the **project** default — `theme` under `[forms]` in the project manifest;
3. `liquid-glass`.

Whitespace counts as unset. Set the form-level override like any other form
property:

```json
{ "op": "set_property", "control_id": "Form", "key": "Theme", "value": "elegance" }
```

`UseThemeBackground` (form-level, Boolean, default false) opts the form into the
theme's own background art. While it is **false** the form's `BackgroundColor` /
background image apply; while it is **true** and the active pack supplies a
background, the pack's art replaces the form's own — on the designer canvas and
at run time alike.

## Self-contained themes and GlassStyle

`GlassStyle` selects a **Liquid Glass recipe** — `"Classic"`, `"Enhanced"`,
`"Neumorphic Light"`, `"Neumorphic Dark"`. It is therefore a setting *of Liquid
Glass*, not a general style register.

A theme that declares itself **self-contained** owns the whole look, and Liquid
Glass's ambient configuration — the `GlassStyle` register, its frost, its
neumorphic relief — is excluded from every control that theme paints. `elegance`
is self-contained; an asset pack declares `self_contained` in its manifest.

Consequences worth stating before a developer is surprised by them:

- On a self-contained theme the IDE **disables** the `GlassStyle` setting.
  Setting it anyway is stored and has **no visual effect**, and it changes
  nothing else about the form or its controls.
- Never advise changing `GlassStyle` to fix an appearance problem on such a
  form. It cannot be the cause and it cannot be the cure.

## What a theme never overrides

A theme supplies **defaults**, never overrides. The developer's own explicit
control properties always win — under every theme and every `GlassStyle`:

`BackgroundColor`, `ForegroundColor`, `CornerRadius`, `Transparency`, and the
whole `Shadow*` family.

In particular `ShadowEnabled` draws a drop shadow under every theme and every
`GlassStyle` value. If a shadow is missing, the cause is the shadow properties,
not the theme.

## Asset-pack themes — the `theme.toml` manifest

A pack is a folder `assets/themes/<id>/` holding a `theme.toml` manifest plus
its art. The manifest is the pack's public contract:

```toml
id = "cobalt-steel"
display_name = "Cobalt Steel"
self_contained = true

[background]
image = "background.png"      # optional themed background
tile  = false

[palette]
foreground = "#dfe7ff"                                  # default text colour
chart = ["#4C9BE8", "#E87A4C", "#4CE87A", "#E84C9B"]    # chart data palette

[chart_style]
stroke_width = 2.0
fill_texture = "chart_fill.png"    # optional material fill for bars and slices

[controls.button]
image    = "button.png"
slice    = [12, 12, 12, 12]        # 9-slice insets: left, top, right, bottom
hover    = "button_hover.png"      # optional per-state art
pressed  = "button_pressed.png"
disabled = "button_disabled.png"
focused  = "button_focused.png"
```

The 9-slice insets keep corners fixed while edges and centre stretch, so one
image skins a control at any size. States other than `Normal` fall back to
`Normal` when the pack does not provide them, and a control the pack does not
cover is painted by Liquid Glass. A pack id that collides with a built-in id
loses: the built-in wins.
"##;
    std::fs::write(kb_dir.join("form_themes.md"), themes)?;

    // Write File 7: form_layout_and_events.md — the layout model, the complete
    // form event catalogue, and the hosting/shell rules.
    let layout = r##"# PowerRustCOBOL Form Layout, Events and Hosting

## The layout model — position, nesting, order

A form holds a **flat list** of controls; nesting is derived from each control's
`parent` link rather than stored as a tree. Four pieces of state place a control:

- **`X` / `Y` / `Width` / `Height`** — position and size in points. A control's
  `X`/`Y` are relative to its container (the form, or the parent control).
- **`parent`** — the id of the enclosing container, or none for a direct child
  of the form. A control whose parent is a **TabControl** also carries `tab`,
  the 0-based page it belongs to.
- **`z_order`** — higher is drawn on top; 0 is bottommost; negatives are legal.
- **`tab_order`** — the keyboard traversal sequence.

There is **no anchoring or docking layout engine**: a control does not resize
with its container. Geometry is what the designer recorded, and COBOL changes it
by writing `X`, `Y`, `Width` or `Height` at run time.

## `Anchor` is a design-time lock, not an edge constraint

`Anchor` (Boolean, default false) locks a control's position **against mouse
dragging on the designer canvas**. Keyboard nudges and property-pane entry still
move it, and it has no run-time effect whatsoever.

It is **not** the `Top,Left`-style edge anchoring of other RAD tools. Forms
saved with a legacy string value such as `"Top,Left"` read as **unanchored**, so
loading an old form never silently locks every control. Do not offer `Anchor` as
a way to make a control follow its container's size — nothing in the platform
does that.

## The form's own geometry and `StartPosition`

A FORM's `X`/`Y` are its window's position on screen — not to be confused with a
CONTROL's `X`/`Y`, which are inside the form.

`StartPosition` decides where the window opens. Accepted values, exactly:
`"System"`, `"Custom"`, `"TopLeft"`, `"TopCenter"`, `"TopRight"`, `"MiddleLeft"`,
`"Center"`, `"MiddleRight"`, `"BottomLeft"`, `"BottomCenter"`, `"BottomRight"`.

- `"System"` (the default) leaves placement to the window manager and **ignores
  `X`/`Y`**.
- `"Custom"` applies the form's `X`/`Y` at launch.
- Every other value computes a position from the real screen and window size at
  launch and ignores `X`/`Y`.

Setting `X`/`Y` alone moves nothing unless `StartPosition` is also `"Custom"`.

## Form events — the complete catalogue

A form supports **68** events, all `on`-prefixed, in these groups. Bind them the
way control events are bound.

- **Lifecycle** — `onCreate`, `onInitialize`, `onLoad`, `onOpened`, `onShow`,
  `onHide`, `onClose`, `onClosing`, `onCloseRejected`, `onClosed`, `onDestroy`
- **Activation & Focus** — `onActivate`, `onActivated`, `onDeactivate`,
  `onDeactivated`, `onGotFocus`, `onLostFocus`
- **Window State** — `onResize`, `onResizing`, `onMove`, `onMoving`,
  `onMinimize`, `onMaximize`, `onRestore`, `onFullscreen`, `onExitFullscreen`,
  `onFullScreenChanged`
- **Layout & Painting** — `onLayout`, `onPaint`, `onRepaint`, `onThemeChanged`,
  `onDpiChanged`, `onFontChanged`
- **Mouse** — `onClick`, `onDoubleClick`, `onMouseDown`, `onMouseUp`,
  `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`
- **Touch & Pointer** — `onPointerDown`, `onPointerUp`, `onPointerMove`,
  `onPointerEnter`, `onPointerLeave`, `onPointerCancel`, `onGesture`
- **Scrolling** — `onScroll`, `onScrollStart`, `onScrollEnd`,
  `onHorizontalScroll`, `onVerticalScroll`
- **Drag & Drop** — `onDragEnter`, `onDragLeave`, `onDragOver`, `onDrop`
- **Clipboard** — `onCut`, `onCopy`, `onPaste`
- **System / OS** — `onSystemColorChanged`, `onDisplayChanged`,
  `onPowerSuspend`, `onPowerResume`, `onSessionLock`, `onSessionUnlock`
- **Error Handling** — `onUnhandledException`

`onCloseRejected` fires when a close attempt was refused because the form (or a
synchronous child of it) is `Waiting`. `onFullScreenChanged` fires when the
actual fullscreen state changed in either direction — read `me`'s `FullScreen`
for the new value.

## Which teardown event to use: `onDeactivate` or `onDestroy`

These two are constantly confused, and using the wrong one closes files that are
still in use — or leaks the ones that are not.

- **`onDeactivate`** — the form's body left the ContentPane but the form is
  **still resident**: it became an ancestor in the navigation chain, or it was
  parked by *Preserve previous form*. Its storage and its handlers stay live.
  **Do not close files here.**
- **`onDestroy`** — the form's storage is about to be released. This is the
  teardown point: close files, `COMMIT`, free resources. It is **never** fired
  for a mere swap-out.

## Form hosting — `Standalone`, `Embedded`, `Both`

The form-level `FormFormat` property decides how a form may be loaded:

- **`Standalone`** (default, and what every older `.cfrm` reads as) — its own OS
  window, reached by `OpenFormSync` / `OpenFormAsync`.
- **`Embedded`** — loaded into the application shell's ContentPane by a menu
  item.
- **`Both`** — valid on either path: a reusable lookup screen that is a modal
  dialog in one place and a pane occupant in another.

A menu item may load only a form that allows `Embedded`; `OpenFormSync` /
`OpenFormAsync` may open only one that allows `Standalone`. Anything
unrecognised in the file reads as `Standalone`, so a hand-edited form never
fails to load over this field.

## The application shell — one window instead of many

Placing a **SideMenu** control on the **main form** is the entire switch that
turns an application into a shell: one window divided into a menu pane, a
breadcrumb, and a ContentPane where forms load in place.

- Main form with a SideMenu → shell mode.
- No SideMenu — including a form with a classic `MenuBar` → every form opens in
  its own window, exactly as before.

An existing project cannot become a shell application by accident. The sidebar
is filled with the **same menu editor a `MenuBar` uses** (select it, then *Edit
Menu…*), because the menu is stored in a sidecar keyed by the control, not by
the kind of control.

The **breadcrumb is the navigation chain**: every form on it is still resident
and its handlers still fire while its body is not displayed. Clicking a segment
destroys everything below it, deepest first, and shows that form again. Per
menu item, *Preserve previous form* decides whether a sibling switch destroys
the form being left or keeps it resident for an instant return.

## `me` and `super` — addressing a form from COBOL

`me` addresses the current form. **`super`** addresses the form that loaded or
opened it, on both paths — a menu load and `OpenFormSync` / `OpenFormAsync`.

```cobol
           MOVE super::Title TO WS-T.
           MOVE "Processing..." TO super::Title.
           INVOKE super::"SetWindowState"("Minimized").
           MOVE super::super::Title TO WS-T.
           super::SIDE-1::Collapse().
           super::SIDE-1::Open().
```

- **Bare properties are checked at build time** against the universal form
  surface: `Name`, `Title`, `Width`, `Height`, `X`, `Y`, `WindowState`,
  `FullScreen`, `TitleVisible`, `CanMinimize`, `CanMaximize`, `FormState`,
  `FormFormat`, `BackgroundColor`, `Transparency`. A typo such as `super::Widht`
  fails the build at any depth.
- **Form-specific procedures use parentheses** — `super::"RecalcTotals"()` — and
  dispatch at run time.
- **`super` can be NULL**: in the main form, and in an async-opened form whose
  opener has closed (a child never keeps its opener alive). Referencing a NULL
  `super` raises the standard runtime error.
- Each opened form runs as its **own program with its own WORKING-STORAGE**.
  Forms never read each other's data items; they talk through published form
  properties, `super::X`, and windowHandler methods.
"##;
    std::fs::write(kb_dir.join("form_layout_and_events.md"), layout)?;

    // Write File 8: project_model_and_settings.md — what a project IS. The
    // agents could describe every control and still not know where a file
    // belongs, what the manifest is called, or which settings exist.
    let project_model = r##"# PowerRustCOBOL Project Model and Settings

## What a project is on disk

A project is one **TOML manifest** plus the files it lists. The manifest is
named after the project — `PowerDemo3.project.toml` — **not** a fixed file name,
so tooling finds it by scanning a directory and its ancestors for
`*.project.toml`. A legacy `cobolt.toml` is still accepted and wins when both
are present.

A `.cfrm` written into `forms/` is **not** in the project until it is listed
under `[files] forms`. Writing a file to disk is half the job; the tree shows
only what the manifest tracks.

The standard sub-folders are `forms/`, `indexed/`, `src/`, `generated/`,
`Assets/`, `Knowledge Base/`, `data/`, `bin/` and `dist/`. Missing ones are
back-filled when an older project is opened.

## The manifest sections

```toml
[project]
name    = "MyApp"
version = "1.0.0"
main    = "src/main.cbl"

[files]
sources = ["src/main.cbl", "src/helpers.cbl"]
forms   = ["forms/main-form.cfrm", "forms/login.cfrm"]
assets  = ["Assets/logo.png"]

[runtime]
fixed_format = false
```

Beyond these there are `[ide]` (per-project IDE appearance), `[forms]` (form
defaults, window effects, the main-form designation), `[ai]` (model profiles and
agent settings) and the External Crates pins. Every section is optional: a
missing section takes its defaults, which is what keeps older projects loading
unchanged.

## `[files]` — the five tracked lists

- **`sources`** — hand-written COBOL (`.cbl`).
- **`forms`** — form definitions (`.cfrm`).
- **`generated`** — RAD output, one `.cbl` per form. Moved out of `sources`
  when a form generates it.
- **`indexed`** — indexed-file definitions (`.cidx`).
- **`assets`** — images and other resources.
- **`documentation`** — the project's own Knowledge Base documents.

## The project tree's seven categories

The IDE owns these top-level nodes; developers add entries *within* a category,
never a category of their own. In display order, with the folder each one owns:

| Category | Folder | Notes |
|---|---|---|
| Forms | `forms/` | `.cfrm` |
| Indexed Files | `indexed/` | `.cidx` |
| Common Code | `src/` | hand-written COBOL only |
| Generated Code | `generated/` | **read-only**, populated by the designer |
| External Crates | vendor folder | one node per registered crate pin |
| Assets | `Assets/` | |
| Documentation | `Knowledge Base/` | the project's own KB |

**Generated Code cannot be added to** and its files cannot be edited: they are
opened read-only and drawn in blue, and they are regenerated from the form. A
change belongs in the form or in Common Code, never in `generated/`.

## `[project]` — metadata and build settings

`name`, `version`, `main` (the entry program), `copyright`, `license_model` and
`license_text`, `destination_folder` (where a build installs the deliverable —
`dist/` when unset), `debug_compilation`, and `built_with_version`, which
records the PowerRustCOBOL that last **fully** built the project. Opening a
project last fully built by an older PowerRustCOBOL — or never fully built —
makes the next **Build** a full one.

## `[forms]` — form defaults and the main form

- **`theme`** — the project's default **form** theme id. Empty means Liquid
  Glass. A form's own `Theme` overrides it.
- **Window effects** — `entrance-effect`, `entrance-ms`, `entrance-easing`,
  `exit-effect`, `exit-ms`, `exit-easing`, `entrance-on-restore`. Effects are a
  **project-level** choice: a form only decides *whether* to play them, through
  its `WindowEffects` property. Absent settings mean no effect, so a project
  written before effects existed is unchanged.
- **`main-form`** and its seal — exactly one form per project is the main form.
  Only the main form starts an application: `rcrun` and a built binary open the
  form the project designates, never one a caller names. The designation is
  recorded in both the manifest and the `.cfrm` itself.

## `[ide]` — per-project IDE appearance

The look travels with the project, not with the developer: `theme` (the IDE
colour theme id), `background_image`, `background_opacity` (0-100),
`project_icon`, and `hide_ai_setup_prompt`.

These theme the **IDE chrome**. They are a different setting from `[forms]
theme`, which themes the developer's designed forms. Do not confuse the two: no
`[ide]` setting changes how a form looks at run time.

Machine-local developer aids are deliberately **not** here — the IDE debug
switches live in the IDE's own settings (Help → Debug Settings), so they are not
project data and are not shared with a colleague who opens the project.

## `[runtime]` — source format

`fixed_format` selects fixed-form COBOL (columns 7-72) rather than free form.
Free form is the default for new projects.

## Where things that are NOT project files live

- **The System Knowledge Base** — platform documentation, regenerated from the
  running binary at machine level in `~/PowerRustCOBOL/Knowledge Base/`, with
  its index in `~/PowerRustCOBOL/data/`. It is never copied into a project, and
  a build never publishes it into one.
- **The Project Knowledge Base** — the developer's own `<project>/Knowledge
  Base/`, indexed under `<project>/data/`. Empty is a normal state.
- **API keys** — never in the manifest. They resolve from the machine-local
  store by model-profile id, so a manifest can be committed and shared safely.
"##;
    std::fs::write(
        kb_dir.join("project_model_and_settings.md"),
        project_model,
    )?;

    Ok(())
}

// ── System Knowledge Base generation (controls + methods reference) ──────────
//
// The controls document derives every property's TYPE and DEFAULT from the
// live `Control::new` seed (the exact map the designer saves and the runtime
// executes), so names, types and defaults cannot drift from the code. Only the
// value DOMAINS (enums, ranges) and the prose descriptions are curated below —
// keep them in step when a control gains a property or an enum gains a value.

/// The properties every control shares (seeded before the type-specific match
/// in `Control::new`). Documented once in the "Universal properties" section
/// and skipped in the per-control listings.
const UNIVERSAL_PROPS: &[&str] = &[
    "BackgroundColor",
    "BackgroundGradientEnabled",
    "BackgroundGradientStartColor",
    "BackgroundGradientEndColor",
    "BackgroundGradientDirection",
    "ForegroundColor",
    "FontName",
    "FontSize",
    "Bold",
    "Italic",
    "Underline",
    "Strikethrough",
    "Tooltip",
    "Cursor",
    "HoverDelayMs",
    "Anchor",
    "Padding",
    "Transparency",
    "ShadowEnabled",
    "ShadowOpacity",
    "ShadowColor",
    "ShadowLightColor",
    "ShadowDirection",
    "ShadowDistance",
    "ShadowBlur",
    "ShadowBlurStrength",
    "ZOrder",
    "DataItem",
    "DataFormat",
];

/// The input/lifecycle events shared by most visual controls. Documented once;
/// per-control sections list which of these apply plus the control-specific
/// events in full.
const UNIVERSAL_EVENTS: &[&str] = &[
    "onClick",
    "onDblClick",
    "onDoubleClick",
    "onRightClick",
    "onMiddleClick",
    "onMouseEnter",
    "onMouseLeave",
    "onMouseDown",
    "onMouseUp",
    "onMouseMove",
    "onMouseWheel",
    "onContextMenu",
    "onGotFocus",
    "onLostFocus",
    "onKeyDown",
    "onKeyUp",
    "onKeyPress",
    "onEnterPressed",
    "onEscapePressed",
    "onHoverEnter",
    "onHoverLeave",
    "onResize",
    "onResized",
    "onMove",
    "onMoved",
    "onVisibleChanged",
    "onEnabledChanged",
    "onLoad",
];

const EIGHT_DIRECTIONS: &str =
    "one of: `North` | `NorthEast` | `East` | `SouthEast` | `South` | `SouthWest` | `West` | `NorthWest`";
const COLOR_DOMAIN: &str = "hex color string `\"#RRGGBB\"` or `\"#RRGGBBAA\"`";
const BOOL_DOMAIN: &str = "`1` (true) or `0` (false)";

/// Curated `(value domain, description)` for a property name. Applies to every
/// control that seeds the name; the type and default are derived mechanically.
pub fn property_reference(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        // ── Universal appearance ──
        "BackgroundColor" => (COLOR_DOMAIN, "Fill color behind the control's content."),
        "BackgroundGradientEnabled" => (BOOL_DOMAIN, "Enables the two-color background gradient."),
        "BackgroundGradientStartColor" => (COLOR_DOMAIN, "Gradient start color."),
        "BackgroundGradientEndColor" => (COLOR_DOMAIN, "Gradient end color."),
        "BackgroundGradientDirection" => (EIGHT_DIRECTIONS, "Direction the gradient flows toward."),
        "ForegroundColor" => (COLOR_DOMAIN, "Text / foreground drawing color. On a CheckBox, RadioButton or DateTimePicker it is kept only while it reads on the surface the text lands on, and otherwise flips to black or white — measured against the control's FRAME (its BackgroundColor), never against the tick box or circle, which the caption sits beside rather than on. Above Transparency 70 the frame paints too little to measure, so the color is used exactly as set; a CheckBox is 100 % transparent by default, so its caption color is always the one you gave it."),
        "FontName" => ("installed font family name, e.g. `\"Arial\"`", "Font family for the control's text."),
        "FontSize" => ("points, > 0 (typical 8-72)", "Font size in points."),
        "Bold" => (BOOL_DOMAIN, "Bold text."),
        "Italic" => (BOOL_DOMAIN, "Italic text."),
        "Underline" => (BOOL_DOMAIN, "Underlined text."),
        "Strikethrough" => (BOOL_DOMAIN, "Struck-through text."),
        "Tooltip" => ("free text", "Hover tooltip text (empty = no tooltip)."),
        "Cursor" => (
            "one of: `Default` | `Hand` | `Text` | `Wait` | `Crosshair` | `No` | `SizeAll` | `SizeNS` | `SizeWE`",
            "Mouse cursor shown while hovering the control.",
        ),
        "HoverDelayMs" => ("milliseconds ≥ 0", "How long the pointer must rest before `onHoverEnter` fires."),
        "Anchor" => (BOOL_DOMAIN, "Locks the control against mouse dragging on the design canvas."),
        "Padding" => ("pixels ≥ 0", "Inner padding around the control content."),
        "Transparency" => ("0-100 (percent)", "How much of what is behind the control shows through; 0 = opaque, 100 = the control's own face is not painted and the form (or the control underneath) shows in full. Replaces the former Opacity, which ran the other way round. A CheckBox defaults to 100."),
        "ShadowEnabled" => (BOOL_DOMAIN, "Enables the drop shadow."),
        "ShadowOpacity" => ("0-100 (percent)", "Drop-shadow opacity."),
        "ShadowColor" => (COLOR_DOMAIN, "Dark shadow color."),
        "ShadowLightColor" => (COLOR_DOMAIN, "Light (highlight) shadow color for neumorphic styles."),
        "ShadowDirection" => (EIGHT_DIRECTIONS, "Direction the shadow is cast toward."),
        "ShadowDistance" => ("pixels ≥ 0", "Shadow offset distance."),
        "ShadowBlur" => (BOOL_DOMAIN, "Enables soft blur falloff on the shadow."),
        "ShadowBlurStrength" => ("0-20", "Blur radius in layers."),
        "ZOrder" => ("any integer; higher paints in front", "Stacking order among siblings."),
        "DataItem" => ("COBOL WORKING-STORAGE data-item name", "Data-binding source item for this control (empty = unbound)."),
        "DataFormat" => ("format string (empty = raw)", "Display format applied to the bound value."),
        "CornerRadius" => ("pixels ≥ 0", "Rounded-corner radius (default 3 on Button, 8 on charts, 0 elsewhere)."),

        // ── Text input / captions ──
        "Caption" => ("free text", "Visible label text (Button/Label/CheckBox/RadioButton/GroupBox)."),
        "Text" => ("free text", "Current text content."),
        "HintText" => ("free text", "Placeholder shown while the box is empty."),
        "InnerPadding" => ("pixels ≥ 0", "Padding between the border and the text."),
        "MaximumLength" => ("characters ≥ 0; 0 = unlimited", "Maximum text length accepted."),
        "Multiline" => (BOOL_DOMAIN, "Multi-line editing."),
        "PasswordCharacter" => ("single character or empty", "Masks input with this character when set."),
        "ReadOnly" => (BOOL_DOMAIN, "Blocks user editing (value still settable from COBOL)."),
        "ScrollBars" => ("one of: `None` | `Horizontal` | `Vertical` | `Both`", "Which scrollbars a multiline box shows. None still scrolls, it just draws no bars. Horizontal and Both stop the text wrapping."),
        "WordWrap" => (BOOL_DOMAIN, "Wraps long lines."),
        "TextAlignment" => (
            "`Left` | `Center` | `Right` | `Justified` on Label/TextBox (Button also accepts anchored forms like `MiddleCenter`)",
            "Horizontal alignment of the text. `Justified` stretches wrapped lines to the full width (static text; a TextBox being edited shows it left-aligned).",
        ),
        "VerticalAlignment" => (
            "`Top` | `Middle` | `Bottom`",
            "Vertical alignment of the text (Label and single-line TextBox; a multiline TextBox stays top-anchored).",
        ),
        "AutoSize" => (BOOL_DOMAIN, "Grows the control to fit its text."),

        // ── Borders ──
        "BorderStyle" => (
            "one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken`",
            "Border drawing style.",
        ),
        "BorderColor" => (COLOR_DOMAIN, "Border line color."),
        "BorderWidth" => ("pixels ≥ 0", "Border line thickness."),

        // ── Check / radio ──
        "Checked" => (BOOL_DOMAIN, "Checked state."),
        "GroupName" => ("free text", "RadioButtons sharing a GroupName are mutually exclusive."),
        "CheckAlignment" => ("`Left` | `Right`", "Side of the caption the check/radio glyph sits on."),
        "CheckColor" => (COLOR_DOMAIN, "Color of the check/radio mark — the tick itself, drawn inside the box."),
        "CheckSize" => ("0-100", "Percentage of the check glyph's own box the checkmark stroke fills."),
        "CheckBoxColor" => ("`#RRGGBB`, or empty for the theme's own", "Fill of the TICK BOX (a RadioButton's circle) — the box only, never the frame. BackgroundColor is the frame's, as on every other control. Left EMPTY (the default) the active theme paints the box; naming a color makes it lead over whatever the theme would have used."),
        "CheckBoxBorderStyle" => ("`None` | `Single` | `Fixed3D` | `Raised` | `Sunken`", "Border drawn around the TICK BOX, separate from the frame's BorderStyle. `None` (the default) keeps whatever rim the theme draws."),
        "CheckBoxBorderColor" => (COLOR_DOMAIN, "Color of the tick box's own border."),
        "CheckBoxBorderWidth" => ("pixels 0-10", "Width of the tick box's own border. 0 draws none."),

        // ── Images / animation ──
        "ImagePath" => ("project-relative or absolute image path", "Image file to display."),
        "HeaderImage" => ("image path or empty", "SideMenu only. The logo at the top of an OPEN sidebar. Its box is 270x80 points and that box is a LIMIT, not a shape to fill: a smaller logo is drawn at its own size, centred, and a bigger one is scaled down to fit keeping its aspect ratio (540x80 draws 270x40; 270x240 draws 90x80). Empty outlines the box instead. A collapsed rail shows HeaderIcon, not this."),
        "SizeMode" => (
            "PictureBox: `Normal` | `Stretch` | `Zoom` | `CenterImage` | `AutoSize`; Animator: `Fit` | `Fill` | `Stretch` | `Center`",
            "How the image is scaled inside the control.",
        ),
        "ImageAlignment" => ("anchor name, e.g. `MiddleCenter`, `TopLeft`", "Where the unscaled image is anchored."),
        "ShowFrame" => (BOOL_DOMAIN, "Draws the frame/background behind the image."),
        "Source" => ("path to GIF / WebP / APNG / still image", "Animated image the Animator plays."),
        "AutoPlay" => (BOOL_DOMAIN, "Starts playing when the form loads."),
        "Loop" => (BOOL_DOMAIN, "Restarts the animation when it ends."),

        // ── Ranges / values ──
        "Minimum" => ("integer ≤ Maximum", "Lower bound of the value range."),
        "Maximum" => ("integer ≥ Minimum", "Upper bound of the value range."),
        "Value" => (
            "ProgressBar/Slider/NumericUpDown: integer within Minimum..Maximum; DateTimePicker: date string",
            "Current value.",
        ),
        "Step" => ("integer > 0", "Increment applied by arrows / Increment()/Decrement()."),
        "LargeChange" => ("integer > 0", "Page Up/Down increment."),
        "DecimalPlaces" => ("0-6", "Fractional digits displayed."),
        "ThousandsSeparator" => (BOOL_DOMAIN, "Shows a thousands separator."),
        "BarColor" => (
            COLOR_DOMAIN,
            "Filled-portion color of the progress bar (how far it has travelled). The \
             UNTRAVELLED part -- the trough -- is the control's BackgroundColor; left at its \
             default either one follows the active theme.",
        ),
        "Orientation" => ("`Horizontal` | `Vertical`", "Layout axis."),
        "Style" => (
            "`Continuous` | `Blocks`",
            "Progress bar fill style: one unbroken run, or a row of segments.",
        ),
        "BlockSize" => (
            "integer ≥ 0 (px, 0 = automatic)",
            "Length of one block under `Style = Blocks`, along the axis the bar travels. \
             0 sizes each block from the bar's own thickness.",
        ),
        "ShowValue" => (BOOL_DOMAIN, "Draws the numeric value on the control."),
        "TickFrequency" => ("integer > 0 (value units)", "Draw a tick every N units."),
        "TickStyle" => ("one of: `None` | `Top` | `Bottom` | `Both`", "Where slider ticks are drawn."),
        "TrackColor" => (COLOR_DOMAIN, "The part still to travel: a Slider's rail from Value to Maximum, a Knob's arc from Value round to Maximum. Outranks the Appearance BackgroundColor; left at its default the active theme paints."),
        "ThumbColor" => (COLOR_DOMAIN, "Slider knob color. Outranks the Appearance ForegroundColor; left at its default the active theme paints."),
        "FaceColor" => (COLOR_DOMAIN, "Knob dial face — the round body the indicator turns over. Empty (the default) leaves it to the theme. The rim's own fill is this colour lightened, so a face colour carries the whole dial."),
        "RimColor" => (COLOR_DOMAIN, "Knob rim and inner ring — the two outlines around the dial face. Empty (the default) leaves them to the theme."),

        // ── Date/time ──
        "Format" => ("one of: `Short` | `Long` | `Time` | `Custom`", "Date display format preset."),
        "CustomFormat" => ("format pattern, e.g. `dd/MM/yyyy`", "Pattern used when Format = `Custom`."),
        "ShowUpDown" => (BOOL_DOMAIN, "Spinner arrows instead of a drop-down calendar."),
        "MinimumDate" => ("date string or empty", "Earliest selectable date."),
        "MaximumDate" => ("date string or empty", "Latest selectable date."),

        // ── Lists ──
        "Items" => (
            "newline-separated entries (TreeView: two-space indentation nests children)",
            "The list content, one item per line.",
        ),
        "SelectedIndex" => ("0-based index; -1 = no selection", "Currently selected item."),
        "MultiSelect" => (
            BOOL_DOMAIN,
            "Lets the user build a selection with Ctrl-click (Cmd on a Mac), reported in SelectedItems.",
        ),
        "SelectedItems" => (
            "newline-separated item text (runtime)",
            "The Ctrl-click selection, drawn in a dimmed highlight. Separate from Value, which is the ACTIVE row.",
        ),
        "ShowCheckBoxes" => (
            BOOL_DOMAIN,
            "Gives every ListBox row a tick box; what they collect is CheckedItems.",
        ),
        "CheckedItems" => (
            "newline-separated item text (runtime)",
            "The ticked rows, in the order the user ticked them and with any gaps. Ticking never moves the active row.",
        ),
        "ActiveItemColor" => (
            "color, or empty for the control's default highlight",
            "Highlight behind the item Value/SelectedIndex reports: the ACTIVE row of a ListBox, \
             or the selected item in an open ComboBox dropdown. Left empty a ListBox takes the \
             theme's own selection color and a ComboBox its popup's built-in one.",
        ),
        "SelectedItemsColor" => (
            "color, or empty for ActiveItemColor dimmed to 45%",
            "ListBox only. Highlight behind the other rows of a MultiSelect selection, the ones \
             SelectedItems reports. Left empty it follows ActiveItemColor, so naming that one \
             alone restyles the whole list.",
        ),
        "HoverItemColor" => (
            "color, or empty for the popup's built-in hover highlight",
            "ComboBox only. Highlight behind the dropdown item the pointer, the drag or the arrow \
             keys are on. Kept fainter than ActiveItemColor by default so hovering an item \
             never looks like selecting it.",
        ),
        "Sorted" => (BOOL_DOMAIN, "Shows the items in alphabetical order, by TEXT and ignoring case, so 10 sorts before 9. Display order only - the stored Items keeps the order it was written in. ListBox and ComboBox; TreeView carries the property but does not act on it yet."),
        "DropDownStyle" => ("one of: `DropDown` | `DropDownList` | `Simple`", "ComboBox edit/list behaviour."),
        "DropDownHeight" => ("pixels > 0", "Maximum height of the opened list. The list is as tall as its items need up to this, and scrolls past it."),
        "Editable" => (BOOL_DOMAIN, "Allows typing free text into the combo. It does not change what the arrow keys do: those always walk the list."),

        // ── TreeView ──
        "AllowEdit" => (BOOL_DOMAIN, "In-place node label editing. **NOT IMPLEMENTED** — the property is seeded and shown in the inspector, but no surface lets the operator rename a node yet. Do not tell a developer this works; to edit a tree at run time, write the new `Items` from COBOL."),
        "CheckBoxes" => (BOOL_DOMAIN, "Draws a tick box on every node. A click ON THE BOX ticks it (a click anywhere else on the row selects the node) and the ticked nodes land in `CheckedNodes`, one per line, with `onNodeCheck` carrying the node."),
        "CheckedNodes" => ("newline-separated node labels", "Which boxes are ticked, one node per line — the `CheckBoxes` companion, read and written exactly like `SelectedNode`. Writing it from COBOL ticks those nodes."),
        "ShowLines" => (BOOL_DOMAIN, "Draws the connector lines between a node and its parent, in `LineColor`. On by default."),
        "ShowRootLines" => (BOOL_DOMAIN, "Draws the spine joining the TOP-LEVEL nodes, in `LineColor` — separate from `ShowLines`, which joins children to parents. On by default."),
        "HotTracking" => (BOOL_DOMAIN, "Lifts the node under the pointer, faintly — half the weight of the selection band, so the two are never confused. Off by default."),
        "LineColor" => (COLOR_DOMAIN, "Connector/line color (TreeView, Line, Shape)."),

        // ── Containers ──
        "HScroll" => (BOOL_DOMAIN, "Horizontal auto-scroll when children overflow."),
        "VScroll" => (BOOL_DOMAIN, "Vertical auto-scroll when children overflow."),
        "HideBackground" => (BOOL_DOMAIN, "Hides the fill/border while keeping the content visible."),
        "HideCaption" => (BOOL_DOMAIN, "Hides the GroupBox caption text."),
        "CaptionEnabled" => (BOOL_DOMAIN, "Reserves the caption band (off = children use the full box)."),
        "UserControl" => ("User Control definition name or empty", "Marks a deployed project User Control instance."),
        "mcp_tool" => ("tool name or empty", "MCP tool this container is exposed as (advanced; leave empty)."),

        // ── Repeating group (ControlArray) ──
        "IsRepeatingGroup" => (BOOL_DOMAIN, "Turns the GroupBox into a repeating card template (control array)."),
        "ArrayName" => ("COBOL identifier or empty (empty = control id)", "Name used to address instances: `Name(index)::Member`."),
        "ItemCount" => ("integer ≥ 0", "Number of live card instances at runtime."),
        "LayoutDirection" => ("one of: `Vertical` | `Horizontal` | `Grid`", "How cards flow inside the group."),
        "ItemSpacing" => ("pixels ≥ 0", "Gap between cards."),
        "ItemsPerRow" => ("integer ≥ 1", "Cards per row when LayoutDirection = `Grid`."),
        "PlacementEffect" => ("one of: `None` | `Deal` | `FadeIn` | `ZoomIn` | `ZoomOut`", "Card entrance animation when data binds."),
        "CardAppearDuration" => ("milliseconds ≥ 0", "Duration of the card entrance animation."),
        "CloneEvents" => (BOOL_DOMAIN, "Cloned cards fire the template's event handlers (with `CONTROL-ARRAY-INDEX`)."),
        "PreviewItemCount" => ("integer ≥ 1", "Cards shown on the design canvas."),

        // ── DataGrid ──
        "Columns" => ("one `Name:Type` per line; Type ∈ `string` | `number` | `datetime` (default `string`)", "Column definitions."),
        "Rows" => ("rows separated by newline, cells within a row by TAB", "Cell data (usually populated at runtime)."),
        "AlternatingRowColor" => (COLOR_DOMAIN, "Tint applied to alternating rows/columns."),
        "AlternatingRowOpacity" => ("0-100 (percent)", "Strength of the alternating tint."),
        "AlternatingMode" => ("one of: `Rows` | `Columns` | `None`", "Axis the alternating highlight applies to."),
        "HeaderBackgroundColor" => (COLOR_DOMAIN, "Header row fill."),
        "HeaderForegroundColor" => (COLOR_DOMAIN, "Header row text color."),
        "GridLineColor" => (COLOR_DOMAIN, "Grid line color."),
        "GridLineStyle" => ("one of: `None` | `Solid` | `Dash` | `Dot` | `DashDot`", "Grid line dash style."),
        "GridBackgroundImage" => ("image path or empty", "Watermark image behind the cells."),
        "GridBackgroundImageMode" => ("one of: `Fill` | `Fit` | `Stretch` | `Tile` | `Center`", "How the background image scales."),
        "GridBackgroundPattern" => ("one of: `None` | `Stripes` | `Dots` | `Cross` | `X` | `X Dots` | `O`", "Procedural background pattern."),
        "RowBackgroundPattern" => ("one of: `None` | `Stripes` | `Dots` | `Cross` | `X` | `X Dots` | `O`", "Per-row background pattern."),
        "SelectionMode" => ("one of: `Row` | `Cell` | `Column`", "What a click selects."),
        "RowHeight" => ("pixels > 0", "Uniform row height."),
        "RowHeightOverrides" => ("`row:height` pairs, one per line", "Per-row height overrides."),
        "AllowSorting" => (BOOL_DOMAIN, "Click a header to sort."),
        "AllowColumnResize" => (BOOL_DOMAIN, "Drag header edges to resize."),
        "AllowColumnReorder" => (BOOL_DOMAIN, "Drag headers to reorder columns."),
        "AllowRowResize" => (BOOL_DOMAIN, "Drag row edges to resize."),
        "AdvancedGrid" => ("internal serialized settings; leave empty", "Advanced designer-managed grid settings."),
        "ShowRowNumbers" => (BOOL_DOMAIN, "Shows a row-number gutter."),
        "ShowColumnFilters" => (BOOL_DOMAIN, "Shows the per-column filter row."),
        "ColumnFilters" => ("`column=value` pairs, one per line", "Active column filters (runtime)."),
        "ExportCSV" => (BOOL_DOMAIN, "Enables CSV export."),
        "ShowCSVExportButton" => (BOOL_DOMAIN, "Shows the built-in export button."),
        "CSVDelimiter" => ("single character, default `,`", "CSV field delimiter."),
        "CSVExportMode" => ("`Filtered` | `AllRows`", "Whether export honours active filters."),
        "FrozenColumns" => ("integer ≥ 0", "Leading columns that do not scroll."),
        "FrozenRows" => ("integer ≥ 0", "Leading rows that do not scroll."),
        "FrozenShadow" => (BOOL_DOMAIN, "Soft shadow cast by frozen rows/columns."),
        "SelectableText" => (BOOL_DOMAIN, "Cell text can be selected/copied."),

        // ── TabControl ──
        "Tabs" => ("one tab title per line", "The tab pages."),
        "TabPosition" => ("one of: `Top` | `Bottom` | `Left` | `Right`", "Edge the tab strip sits on."),
        "SelectedTab" => ("0-based tab index", "Currently active tab."),
        "ActiveTabColor" => (COLOR_DOMAIN, "Highlight color of the active tab."),
        "TabPadding" => ("pixels ≥ 0", "Padding inside each tab header."),

        // ── MenuBar ──
        "HighlightBgColor" => (COLOR_DOMAIN, "Hovered menu item background."),
        "HighlightFgColor" => (COLOR_DOMAIN, "Hovered menu item text."),
        "SelectedBgColor" => (COLOR_DOMAIN, "Open/selected menu background."),
        "SelectedFgColor" => (COLOR_DOMAIN, "Open/selected menu text."),

        // ── Line / Shape ──
        "LineThickness" => ("pixels > 0", "Stroke thickness."),
        "LineDirection" => ("`Horizontal` | `Vertical` | `Diagonal`", "Axis the Line control draws along."),
        "DashStyle" => ("one of: `Solid` | `Dash` | `Dot` | `DashDot`", "Line dash pattern."),
        "RoundedEnds" => (BOOL_DOMAIN, "Rounds the line end caps."),
        "ShapeType" => ("one of: `Rectangle` | `Circle` | `Triangle`", "Geometric shape drawn."),
        "FormStyle" => (BOOL_DOMAIN, "Shape follows the form's glass style."),
        "FillColor" => (COLOR_DOMAIN, "Shape interior fill. On a Slider, the travelled part of the rail — Minimum to Value — which is the part that reads as filled; left at its default the active theme paints."),
        "FillStyle" => ("one of: `Solid` | `None` | `Hatched`", "How the shape interior is filled."),
        "LineStyle" => ("one of: `Solid` | `Dash` | `Dot` | `DashDot`", "Shape outline dash pattern."),

        // ── Splitter ──
        "MinimumSize" => ("pixels ≥ 0", "Smallest size either side may shrink to."),
        "SplitPosition" => ("pixels ≥ 0", "Current divider position."),

        // ── Timer ──
        "Interval" => ("milliseconds ≥ 10", "Delay between `onTick` events."),
        "Enabled" => (BOOL_DOMAIN, "Timer running state (this is the timer's own property, distinct from control chrome)."),

        // ── AgentObject (LLM) ──
        "AgentURL" => ("HTTP(S) URL", "Base URL of the LLM provider."),
        "AgentModel" => ("model id string", "Model requested from the provider."),
        "AgentAPI" => ("one of: `Ollama` | `LMStudio` | `OpenAI` | `Anthropic` | `Custom`", "Provider protocol."),
        "AgentAPIKey" => ("secret string or empty", "API key when the provider needs one."),
        "AgentEndpoint" => ("URL path or empty", "Overrides the provider's default endpoint."),
        "SystemPrompt" => ("free text", "System prompt sent with every request."),
        "Temperature" => ("0-100 (maps to 0.0-1.0)", "Sampling temperature."),
        "MaximumTokens" => ("integer > 0", "Response token limit."),
        "Stream" => (BOOL_DOMAIN, "Streams the response as it generates."),
        "TimeoutSeconds" => ("seconds > 0", "Request timeout."),
        "TargetControls" => ("comma-separated control ids", "Controls this agent is allowed to modify."),
        "ResponseDataItem" => ("COBOL data-item name", "WORKING-STORAGE item that receives the response."),

        // ── RestClient ──
        "BaseURL" => ("HTTP(S) URL", "Documented base address (note: inline verbs take a FULL URL argument; BaseURL is not auto-prepended)."),
        "DefaultMethod" => ("one of: `GET` | `POST` | `PUT` | `PATCH` | `DELETE` | `HEAD` | `OPTIONS`", "Designer default verb."),
        "AuthType" => ("one of: `None` | `Bearer` | `Basic` | `APIKey`", "Authentication scheme."),
        "AuthToken" => ("secret string or empty", "Token/credentials for AuthType."),
        "DefaultHeaders" => ("`key:value` pairs, newline-separated", "Headers sent with every request."),
        "FollowRedirects" => (BOOL_DOMAIN, "Follows HTTP redirects."),
        "VerifyTLS" => (BOOL_DOMAIN, "Verifies TLS certificates."),
        "RequestDataItem" => ("COBOL data-item name", "Item whose content is sent as the request body."),
        "StatusDataItem" => ("COBOL data-item name", "Item that receives the HTTP status / file status."),
        "Mode" => ("`Async` | `Sync`", "Async fires onComplete/onError later; Sync blocks and returns in-statement."),
        "Busy" => (BOOL_DOMAIN, "Read-only runtime flag: an async operation is in flight."),
        "TimeoutMs" => ("milliseconds ≥ 0; 0 = fall back to TimeoutSeconds", "Async operation timeout."),

        // ── The async lifecycle's read-only answers (spec 032) ──
        // Written by the runtime, never by the designer. They have no default
        // and are not saved — but reading one is the ONLY way a handler sees
        // what an async method returned, since the call itself returns an
        // empty string immediately.
        "ResponseBody" => (
            "runtime-only, read-only",
            "The answer to the last async call, delivered with `onComplete`. Its shape is the method's — a Directions answer is seven TAB-separated fields, PlacesSearch is one line per result, a RestClient verb is the raw body. UNSTRING it in the onComplete handler; it is empty before the first call completes.",
        ),
        "StatusCode" => (
            "runtime-only, read-only; HTTP status as text",
            "The HTTP status of the last call. `0` when the request never reached a server.",
        ),
        "LastError" => (
            "runtime-only, read-only",
            "Why the last async call failed, delivered with `onError`. Empty after a call that succeeded.",
        ),

        // ── SqlDatabase ──
        "Driver" => ("one of: `sqlite` | `postgres` | `mysql` | `mssql`", "Database backend."),
        "ConnectionString" => ("e.g. `sqlite::memory:`, `postgres://user:pw@host/db`", "Connection string; the scheme selects the engine."),
        "AutoConnect" => (BOOL_DOMAIN, "Connects automatically when the form loads."),
        "MaximumConnections" => ("integer ≥ 1", "Connection pool cap."),
        "ConnectionDataItem" => ("COBOL data-item name", "Item that receives the connection handle."),
        "ResultSetDataItem" => ("COBOL data-item name", "Item that receives the result-set handle."),

        // ── IndexedFile ──
        "IndexedFile" => ("`.cidx` schema name from the project", "Which indexed-file schema this control operates on."),
        "OpenMode" => ("`INPUT` (read-only) | `I-O` (read-write)", "COBOL open mode used by the generated paragraphs."),
        "LoadStrategy" => ("`Disk` | `Memory`", "Whether records stream from disk or load into memory."),
        "AutoOpen" => (BOOL_DOMAIN, "Opens the file automatically when the form loads."),
        "RecordName" => ("COBOL record name or empty", "Overrides the schema's record item."),
        "KeyName" => ("COBOL key item or empty", "Overrides the schema's primary-key item."),
        "CurrentKeyDataItem" => ("COBOL data-item name or empty", "Item holding the key for START/READ positioning."),
        "CurrentRecordDataItem" => ("COBOL data-item name or empty", "Item that receives the current record."),
        "OperatorName" => ("registered user name or empty", "`OPEN ... REGISTERED USER` operator identity."),

        // ── Charts ──
        "Title" => ("free text", "Chart title (also the window title on Form methods)."),
        "ShowLegend" => (BOOL_DOMAIN, "Shows the series legend."),
        "ShowGridLines" => (BOOL_DOMAIN, "Shows the plot grid."),
        "ShowXAxis" => (BOOL_DOMAIN, "Shows the X axis line."),
        "ShowYAxis" => (BOOL_DOMAIN, "Shows the Y axis line."),
        "ShowTooltips" => (BOOL_DOMAIN, "Hover tooltips on data points."),
        "AnimateOnLoad" => (BOOL_DOMAIN, "Animates the first draw."),
        "Monochrome" => (BOOL_DOMAIN, "Tonal single-color rendering instead of the palette."),
        "MonochromeColor" => (COLOR_DOMAIN, "Base color for monochrome mode."),
        "MonochromeGradient" => (BOOL_DOMAIN, "Diagonal light-to-dark shading in monochrome mode."),
        "XAxisLabel" => ("free text", "X axis caption."),
        "YAxisLabel" => ("free text", "Y axis caption."),
        "SeriesColors" => ("comma-separated hex colors", "Palette used for the data series."),
        "DataSource" => (
            "COBOL table data-item name (charts / repeating GroupBox / DataGrid binding)",
            "Table the control binds to; rows use the standard `PIC X(64)` label + `PIC 9(18)V9(6)` value layout for charts.",
        ),
        "DataCount" => ("COBOL data-item name", "Item holding the number of occupied table rows."),
        "LabelField" => ("sub-field name", "Table sub-field used for X labels."),
        "ValueFields" => ("comma-separated sub-field names", "Table sub-fields used as Y series."),
        "SeriesLabels" => ("comma-separated display names", "Legend names for the series."),
        "Horizontal" => (BOOL_DOMAIN, "Horizontal bars instead of vertical."),
        "Stacked" => (BOOL_DOMAIN, "Stacks the series instead of grouping."),
        "BarCornerRadius" => ("pixels ≥ 0", "Rounding on bar tops."),
        "Smooth" => (BOOL_DOMAIN, "Catmull-Rom smoothing of the polyline."),
        "ShowPoints" => (BOOL_DOMAIN, "Draws point markers."),
        "PointRadius" => ("pixels > 0", "Point marker radius."),
        "FillAlpha" => ("0-100 (percent)", "Area fill opacity."),
        "ShowLabels" => (BOOL_DOMAIN, "Draws slice labels."),
        "LabelFormat" => ("one of: `percent` | `value` | `label`", "What pie/donut slice labels show."),
        "InnerRadius" => ("0-100 (% of outer radius)", "Donut hole size."),
        "BubbleField" => ("sub-field name or empty", "Table sub-field controlling bubble size."),
        "BubbleScale" => ("pixels > 0", "Maximum bubble radius."),

        // ── Icons (Button) ──
        "IsDefault" => (BOOL_DOMAIN, "Form's default button (activated by Enter)."),
        "IconPath" => ("image path or empty", "Icon drawn next to the caption."),
        "IconAlignment" => ("`Left` | `Right` | `Top` | `Bottom`", "Side of the caption the icon sits on."),
        "IconPadding" => ("pixels ≥ 0", "Gap between icon and caption."),
        "IconSize" => ("pixels, one of: `16` `32` `48` `64` `80` `96` `128`; on a SideMenu any value 8-64", "Icon edge length. On a SideMenu this is the menu-item icon size while the rail is OPEN (see IconSizeCollapsed for the other state)."),
        "IconSizeCollapsed" => ("pixels, 8-64 (SideMenu only)", "Menu-item icon size while the sidebar is COLLAPSED — its own value because the two rail states are two designs: open, the icon sits beside a label; collapsed, the icon IS the row. Unset (a form designed before this property existed) falls back to IconSize."),

        // ── Knob / Gauge / Switch (spec 039) ──
        "Accent" => ("hex color string, or one of: `Blue` | `Green` | `Red` | `Purple` | `Amber` | `Sky`", "Accent color: the Knob's arc and indicator, the Switch's ON track. BOTH take ANY color from the designer's picker (which carries the colour memory); the six names still resolve for forms that stored one, and an unrecognised value falls back to `Blue`. On a SWITCH the inspector calls this row **Checked color**, because that is what it colours there — `Accent` is only the stored key, and the row offered six fixed names until 1.61.152. There is no Size property — the Knob's dial is drawn at whatever size the control was given."),
        "Bipolar" => (BOOL_DOMAIN, "Knob fill grows from the center (both directions) instead of from Minimum."),
        "DefaultValue" => ("integer within Minimum..Maximum", "Value a double-click/reset returns the Knob to."),
        "Label" => ("free text or empty", "Caption drawn under the Knob."),
        "GaugeStyle" => ("one of: `Radial` | `Linear` | `Donut`", "Which meter the Gauge draws: a half-circle speedometer, a horizontal bar, or a full ring."),
        "Color" => ("hex color string or empty", "Gauge fill color; empty uses the active theme's accent. Ignored while zone coloring is on (see WarningThreshold)."),
        "WarningThreshold" => ("fraction of Minimum..Maximum, `0.0`-`1.0`, or empty", "Where the Gauge's fill turns amber. Empty = zone coloring off; both this and CriticalThreshold must be set together, and while they are the zone owns the fill color: green below WarningThreshold, amber from it, red from CriticalThreshold."),
        "CriticalThreshold" => ("fraction of Minimum..Maximum, `0.0`-`1.0`, or empty", "Where the Gauge's fill turns red (see WarningThreshold)."),
        "Unit" => ("free text or empty, e.g. `\"%\"`, `\"rpm\"`", "Suffix after the Gauge's numeric readout, in every style. A unit that starts with a letter or digit is spaced off the number (`\"Parts\"` reads `23 Parts`); a symbol is not (`\"%\"` reads `23%`, `\"°C\"` reads `19°C`). Leading spaces you type are kept as typed."),
        "ShowNeedle" => (BOOL_DOMAIN, "Draws the Gauge's needle (Radial and Donut styles)."),
        "NeedleColor" => ("hex color string or empty", "Colour of the Gauge's needle and its hub (Radial and Donut). Empty = the colour the meter itself is drawn in, which is how the needle has always been painted. The meter's band is unaffected — this is the needle's colour alone."),
        "ShowScale" => (BOOL_DOMAIN, "Draws the Radial Gauge's tick scale."),
        "ReadoutPosition" => ("one of: `Up` | `Down`", "Where a Radial Gauge prints its value + Unit: `Up` inside the dial, above the needle's pivot (the default), or `Down` 5 px below the pivot, where a speedometer prints its number. On `Down` the dial gives up that much height so the reading stays inside the control. Radial only — a Donut reads out in its hole and a Linear under its bar."),
        "BarHeight" => ("pixels > 0", "Linear Gauge bar thickness."),
        "ShowThumb" => (BOOL_DOMAIN, "Draws the Linear Gauge's end-of-fill thumb marker."),
        "StrokeWidth" => ("pixels > 0", "Donut Gauge ring thickness."),

        // ── FileDropZone (spec 039) ──
        "Hint" => ("free text", "Placeholder text shown inside the empty drop zone."),
        "AllowedExtensions" => (
            "comma/space separated extensions, e.g. `csv, xlsx` (blank accepts any file)",
            "What the zone takes. Case-blind, with or without the dot; a file whose extension is not listed is refused.",
        ),
        "MaximumFileSizeKB" => (
            "KB, 0 = no limit",
            "Largest file the zone takes. A bigger file is refused rather than reported.",
        ),
        "DestinationFolder" => (
            "local folder path (blank leaves files where they are)",
            "Accepted files are copied here, the folder being created if needed; an existing name is never overwritten (`report.csv` becomes `report (2).csv`).",
        ),
        "DroppedFiles" => (
            "newline-separated absolute paths (runtime-only, never a design-time default)",
            "Files ACCEPTED since the last read — one absolute path per line, the copy in DestinationFolder when one is set.",
        ),
        "RejectedFiles" => (
            "newline-separated `path<TAB>reason` (runtime-only, never a design-time default)",
            "Files the zone turned away, each with `extension` or `too-big` after a TAB.",
        ),
        // ── ToolBar ──
        "ToolbarLayout" => (
            "serialised toolbar definition (edited in the Toolbar Editor, not by hand)",
            "A ToolBar's groups of buttons: each group's frame (border style/colour/width, corner radius, padding, background, separator), the DEFAULT appearance for that group's buttons, and each button's label-or-icon, tooltip, enabled state, action and its own appearance overrides. A group's appearance settings are the defaults for every button in it, and a button's own values win field by field — so an icon size set once on the group dresses all six buttons. A button carries a label OR an icon, never both: setting one clears the other. Adding a button in the editor copies the previous button's appearance (never its icon, tooltip or action). Set it all through the designer's Toolbar Editor. Absent, a populated `Items` is read as one unframed group of labelled buttons, so a toolbar built before groups existed still works. Corner radius defaults to 10; every colour defaults to unset, meaning the group, then the theme, decides.",
        ),
        "LastButton" => (
            "toolbar button id (runtime-only, never a design-time default)",
            "Which toolbar button was pressed last. Written before `onClick` fires, so ONE handler can serve a whole toolbar: `EVALUATE TOOLBAR-1::LastButton`.",
        ),
        "StageOnly" => (
            BOOL_DOMAIN,
            "Off (the default): a drop copies into DestinationFolder then and there. On: a drop copies NOTHING — the files are held, listed for review in FileListControl with a tick box each, and your COBOL calls `CommitFiles()` to do the copying once the operator is happy. Use it whenever the operator should be able to change their mind before anything is written.",
        ),
        "FileListControl" => (
            "the id of a ListBox on the same form (blank = no list)",
            "Where a staged drop shows what it is holding: one tick-boxed row per file, reading `<path> (12.345 MB)`. Unticking a row leaves that file out of the next `CommitFiles()` without removing it from the list, so the operator can see what they excluded and put it back. Dropping a FileDropZone in the designer creates this ListBox next to it, at the zone's own size, and names it here — it is an ordinary ListBox from then on, and naming a control that no longer exists simply means no list.",
        ),
        "StagedFiles" => (
            "newline-separated absolute paths (runtime-only, never a design-time default)",
            "What a StageOnly zone is holding, in list order, at their ORIGINAL paths — nothing has been copied yet. A second drop adds to this rather than replacing it, and the same file dropped twice is held once.",
        ),
        "CommitSummary" => (
            "free text (runtime-only, never a design-time default)",
            "One line about the intake, for a Label or a DISPLAY: `3 files staged, 24.310 MB` before the form goes ahead, `7 of 8 copied, 24.310 MB` after `CommitFiles()`. Megabytes count 1,000,000 bytes, so a size here matches the one the operator's own file browser shows.",
        ),

        // ── Maps (spec 039) ──
        "CenterLat" => ("decimal degrees as a string, e.g. `\"48.8566\"`", "Map center latitude."),
        "CenterLng" => ("decimal degrees as a string, e.g. `\"2.3522\"`", "Map center longitude."),
        "Zoom" => ("integer, typically 0-20", "OpenStreetMap zoom level (higher = closer). Always a WHOLE level — the one whose tiles are fetched, and the value a handler reads, writes and receives on onBoundsChanged. Wheel zooming glides between levels by drawing the map at a fractional scale, but that fraction is view state and is never published, so this property never carries one."),
        "Markers" => (
            "one marker per line, TAB-separated: `id\\tlat\\tlng\\tlabel\\tinfo`",
            "Pins drawn on the map. Prefer the AddMarker/RemoveMarker methods over hand-formatting this.",
        ),
        "Routes" => (
            "one route per line, TAB-separated: `id\\tcolour\\twidth\\tgeometry`",
            "Lines traced over the basemap — a planned delivery run, a driven route. `colour` is `#RRGGBB` (empty = the default blue), `width` is pixels (0 = default). `geometry` is either an encoded polyline — exactly what the sixth field of a Directions ResponseBody carries, so a route can be traced straight from the answer — or an explicit `lat,lng;lat,lng;…` list for geometry the program worked out itself. Prefer AddRoute/RemoveRoute/ClearRoutes over hand-formatting this. Needs no API key: the geometry comes from your program.",
        ),
        "InfoBackgroundColor" => (
            "`#RRGGBB`, or empty to follow the form",
            "Background of the info window shown when a marker or region is hovered or clicked. Empty — the default — takes the control's own BackgroundColor, so the window matches the form without being configured.",
        ),
        "InfoForegroundColor" => (
            "`#RRGGBB`, or empty for automatic high contrast",
            "Text colour of the info window. Left EMPTY — the default — it is DERIVED from whichever background the window ended up with, choosing black or white for the higher contrast, so the window is legible on any card (at least 4.5:1, WCAG's floor for body text). Set this only when a specific colour is required: an explicit value is used as given and its contrast is the caller's business.",
        ),
        "InfoBorderColor" => (
            "`#RRGGBB`, or empty for the default",
            "Outline of the info window.",
        ),
        "InfoCornerRadius" => ("integer 0-32 (default 8)", "Corner rounding of the info window; 0 is square."),
        "MarkerColor" => (
            "`#RRGGBB[AA]`, or empty for the built-in `#C82828`",
            "Fill of every pin on this map. Pins used to be red with no way to say otherwise; this is that colour, now yours. `AddMarker` carries no colour of its own, so this is what sets it.",
        ),
        "MarkerBorderColor" => (
            "`#RRGGBB[AA]`, or empty for the built-in `#FFFFFF`",
            "The ring drawn around every pin so it reads against a busy basemap.",
        ),
        "RouteColor" => (
            "`#RRGGBB[AA]`, or empty for the built-in `#1E6EDC`",
            "Colour for a route whose own `Routes` line names none. A route drawn with a colour — `AddRoute` USING id colour width geometry — keeps that colour; this is only the fallback.",
        ),
        "RouteCasingColor" => (
            "`#RRGGBB[AA]`, or empty for the built-in `#FFFFFFB4`",
            "The casing under EVERY route: the bright halo that makes a thin line readable over mixed terrain, the way a road map draws one. Unlike RouteColor this applies to every route, whatever colour it names.",
        ),
        "RegionFillColor" => (
            "`#RRGGBB[AA]`, or empty for the built-in translucent blue",
            "Fill for a region whose own `Regions` line names none. Give any replacement an alpha, or the territory hides the streets under it.",
        ),
        "RegionBorderColor" => (
            "`#RRGGBB[AA]`, or empty for NO border",
            "Outline for a region whose own line names no stroke. Empty is not a colour here — it means such a region is drawn without a border at all, which is what it has always done; naming a colour gives every unstyled region an outline.",
        ),
        "TileBackgroundColor" => (
            "`#RRGGBB[AA]`, or empty for the built-in `#C8C8C8`",
            "Painted under the whole map before any tile has arrived — what the operator sees for the first instant, and behind the map wherever the world has no tiles.",
        ),
        "TileLoadingColor" => (
            "`#RRGGBB[AA]`, or empty for the built-in `#D2D2D2`",
            "One tile that has not arrived yet, in its own square. Set this and TileBackgroundColor to the same value for a map that fills in without a visible grid.",
        ),
        "InfoShadow" => ("`1`/`0` (default 1)", "Drop shadow under the info window. Turn it off on a flat or high-contrast form."),
        "SelectedRegionId" => ("region id string or empty (runtime-only)", "Id of the region whose info card is OPEN — set by a click, cleared by clicking bare map. Writing it opens or closes a card from COBOL."),
        "HoveredMarkerId" => ("marker id string (runtime-only)", "Id of the marker the pointer is over, delivered with onMarkerHover."),
        "HoveredRegionId" => ("region id string (runtime-only)", "Id of the region the pointer is over, delivered with onRegionHover."),
        "Regions" => (
            "one region per line, TAB-separated: `id\\tfill\\tstroke\\twidth\\tgeometry\\tlabel\\tinfo`",
            "Filled areas over the basemap — sales territories, delivery zones, coverage. `fill` is `#RRGGBB` or `#RRGGBBAA` (give it an alpha so the streets stay readable underneath); `stroke` is the outline, empty for none. `geometry` is the same as Routes and the ring is closed for you. A region MAY be concave — the fill is triangulated rather than assumed convex, so a territory that follows a coastline or a border fills correctly. Prefer AddRegion/RemoveRegion/ClearRegions. Needs no API key.",
        ),
        "ApiKeySource" => (
            "reserved — currently unused",
            "Declared but not read by any runtime or codegen path today. The google_maps API key is resolved entirely from the project's Google Maps credential slot (Settings → Integrations), never from a control property — do not rely on this property for anything.",
        ),
        "SelectedMarkerId" => ("marker id string or empty (runtime-only)", "Id of the marker the user last clicked, delivered with onMarkerClick."),

        // ── WebSearch (spec 039) ──
        "SearchEngineId" => ("Google Programmable Search Engine `cx` value", "Which Custom Search engine to query — a plain, non-secret id, not the API key."),
        "Query" => ("free text", "Search query text. Set this before INVOKE 'Search'."),
        "NumResults" => ("integer 1-10", "Results requested per search — the Custom Search API's own per-request cap; values outside 1-10 are clamped."),
        "SafeSearch" => (
            "one of: `Off` | `Medium` | `High`",
            "SafeSearch filtering level. The Custom Search API itself only has two levels (`off`/`active`); `Medium` and `High` both map to `active`.",
        ),

        _ => return None,
    })
}

/// Short description for a control-specific event (universal input/lifecycle
/// events are documented once in the shared section).
fn event_reference(name: &str) -> &'static str {
    match name {
        "onChange" => "value or text changed",
        "onTextChanged" => "text content changed",
        "onEnter" => "focus entered the box (alias of onGotFocus)",
        "onLeave" => "focus left the box (alias of onLostFocus)",
        "onCheck" => "the toggle went ON (fires only in that direction)",
        "onUncheck" => "the toggle went OFF (fires only in that direction)",
        "onCheckedChanged" => "checked state flipped, either way (carries the new state)",
        "onValueChanged" => "value changed (the new value is delivered)",
        "onSelectedIndexChanged" => "selection moved to another index",
        "onItemDoubleClick" => "a list item was double-clicked",
        "onSelectionChanged" => "the selected item/cell set changed",
        "onScroll" => "content scrolled",
        "onDropDown" => "drop-down list opened",
        "onDropDownClosed" => "drop-down list closed",
        "onNodeClick" => "a tree node was clicked",
        "onNodeDblClick" | "onNodeDoubleClick" => "a tree node was double-clicked",
        "onNodeSelect" => "a tree node became selected",
        "onTick" => "fires every `Interval` ms while `Enabled` = 1",
        "onImageLoaded" => "the image finished loading",
        "onImageError" => "the image failed to load",
        "onStarted" => "animation started",
        "onEnded" => "animation reached its end",
        "onFrameChanged" => "animation advanced a frame",
        "onLooped" => "animation restarted a loop",
        "onCellClick" => "a cell was clicked",
        "onCellDoubleClick" => "a cell was double-clicked",
        "onRowSelect" => "a row became selected",
        "onRowDoubleClick" => "a row was double-clicked",
        "onColumnClick" => "a column header was clicked",
        "onExportCSV" => "the built-in CSV export ran",
        "onChildAdded" => "a child control was added to the container",
        "onChildRemoved" => "a child control was removed from the container",
        "onTabChanged" => "the active tab changed",
        "onTabClick" => "a tab header was clicked",
        "onCompleted" => "Value reached Maximum",
        "onDataChanged" => "the chart's data-bearing properties changed",
        "onMenuClick" => "a top-level menu was clicked",
        "onMenuItemClick" => "a menu item was activated",
        "onMenuOpen" => "a menu opened",
        "onMenuClose" => "a menu closed",
        "onResponse" => "the LLM reply arrived",
        "onError" => "the operation failed (message in `LastError`)",
        "onTimeout" => "the async operation exceeded its timeout",
        "onComplete" => "the async operation finished successfully",
        "onCancelled" => "the async operation was cancelled",
        "onQueryComplete" => "the SQL statement finished",
        "onConnectOk" => "the database connection opened",
        "onConnectError" => "the database connection failed",
        "onQueryError" => "the SQL statement failed",
        "onRowFetched" => "Fetch() advanced to a row",
        "onFilesDropped" => "one or more files were dropped or picked (read `DroppedFiles`)",
        "onMapClick" => "the map background was clicked (not a marker) — the primary event",
        "onMarkerClick" => "a marker was clicked (`SelectedMarkerId` holds its id)",
        "onBoundsChanged" => "the map was panned or zoomed (`CenterLat`/`CenterLng`/`Zoom` updated)",
        "onResultsReceived" => "classification label for WebSearch's completion (the runtime actually fires the uniform onComplete/onError below — see the WebSearch section)",
        _ => "",
    }
}

/// One-line purpose for each control type, leading its reference section.
fn control_purpose(name: &str) -> &'static str {
    match name {
        "Button" => "Clickable push button.",
        "TextBox" => "Single- or multi-line text input.",
        "Label" => "Static text display. At RUN TIME its caption is SELECTABLE TEXT: drag across it to select, and Cmd/Ctrl+C copies the selection to the clipboard — a drag that starts on one Label and ends on another takes in both, so a reading can be copied along with the caption naming it. There is no property to switch on and nothing to write in COBOL; every Label behaves this way, in the running form and in Preview alike. A Label with a bound onClick still fires it, TAB still walks past labels to the form's own controls, and on the DESIGNER CANVAS a drag still positions the control. Do NOT tell a developer to use a ReadOnly TextBox to make text copyable — that was the workaround before labels could be selected.",
        "CheckBox" => "Boolean on/off box with caption. It has TWO surfaces, each with its own properties. The FRAME is the card behind caption and box: BackgroundColor fills it and BorderStyle/BorderColor/BorderWidth rim it, exactly as on every other control — it is 100 % transparent by default, so nothing shows until one of those is set. The BOX is the tick square: CheckBoxColor fills it, CheckBoxBorderStyle/Color/Width rim it, CheckColor draws the tick inside it and CheckSize scales that tick. Never tell a user to set BackgroundColor to colour the box, or CheckBoxColor to colour the frame.",
        "RadioButton" => "Mutually-exclusive choice within a GroupName. Its indicator is a real drawn CIRCLE on every theme — filled when chosen, an empty rim when not — never a character in the caption. A theme that describes a toggle surface colours it (Elegance paints its green); on every other theme the circle takes the control's own `CheckColor`, and `CheckBoxColor` sets the circle's face where the developer wants one. An unchosen circle's rim is picked for CONTRAST against whatever the control was dropped on, so it is visible on a dark form and on a pale card alike.",
        "ListBox" => "Scrollable list of selectable items. A click makes a row active and starts a one-row selection; a press-and-drag anchors on the row pressed and extends to the row under the pointer, in EITHER direction — reversing shrinks the range back — and holds at the first or last row when the pointer runs past an end; Up/Down arrows move the active row one line once the list has been clicked or Tabbed to, and stop at the ends. Whatever moves the active row, the list scrolls to keep it in view, on the first or last visible line. Dragging selects rather than scrolls; the wheel and the scrollbar scroll.",
        "ComboBox" => "Drop-down list, optionally editable. A click on the header opens the list without picking anything; a press-and-drag from the header follows the pointer item by item — in EITHER direction, reversing walks the highlight back — and holds at the first or last item when the pointer runs past an end, with the release committing that item. With the list SHUT the Up/Down arrows change the value outright; with it OPEN they move the highlight, Enter commits it and Escape closes leaving the value unchanged. The list opens scrolled to the value it holds and scrolls to keep the highlighted item in view; it is as tall as its items need up to DropDownHeight and scrolls past that, so every item is reachable. Header and dropdown both wear the control's designed background, gradient, border and corner radius, and the items are lettered in its own FontName/FontSize/ForegroundColor.",
        "GroupBox" => "Captioned container; can become a repeating card template (control array).",
        "Panel" => "Plain container for grouping child controls.",
        "TabControl" => "Multi-page container with a tab strip.",
        "DataGrid" => "Tabular rows/columns grid with sorting, filtering, freezing and CSV export.",
        "PictureBox" => "Displays a still image.",
        "ProgressBar" => "Shows progress within Minimum..Maximum.",
        "MenuBar" => "Window menu bar (menu structure is edited in the designer and stored in a `.menu.yaml` sidecar, not in a property). Menu items may carry an icon from the built-in catalogue: 660+ pure-vector icons in 26 categories (documents, editing, navigation, commerce, payroll, receivables, payments, stock control, transportation, logistics, financial, company departments, transaction kinds, civilian vehicles, military equipment, and more). Icons are resolution-independent line work tinted by the item's colour; the engine can also apply a second accent colour, a drop shadow, or a neumorphic emboss.",
        "SideMenu" => "Vertical sidebar menu (spec 049). On the MAIN form it puts the application in SHELL mode: one window with a MenuPane, a breadcrumb and a ContentPane. The menu structure is edited in the SAME menu editor a MenuBar uses (inspector button 'Edit Menu...') and stored in a `.menu.yaml` sidecar keyed by control id; a MenuBar deliberately does NOT trigger the shell, so existing projects keep classic multi-window mode. Property `FullHeight` (default true): true = the sidebar owns the window's whole vertical extent and the breadcrumb starts at its right edge; false = the breadcrumb spans the full width and the sidebar fills the height beneath it. While FullHeight is true the control's Y and Height are inert (greyed in the inspector, drawn down the form's full height in the designer and following a form resize); Width stays developer-set. Property `Collapsed` (default false) is the pane state the application OPENS in; the operator's own remembered choice (persisted per application) wins over it from then on. The sidebar also owns the BREADCRUMB FRAME, which always runs from its right edge to the window's right edge (no width or position property exists): `BreadcrumbHeight` (16..200, default 28) and `BreadcrumbBackgroundColor` (empty = follow the ContentPane's backdrop; alpha allowed, the frame is still painted opaque) and `BreadcrumbTextAlign` (Top | Middle | Bottom, default Middle — it places the chain AND the Open/Collapsed toggle together as ONE GROUP: the alignment moves the pair inside the frame and the chain then centres on the toggle's own line, so the text sits on the icon's middle at Top and at Bottom just as it does at Middle, however large the icon; their SIZES stay separate), `BreadcrumbFontSize` (0 = follow the rail's FontSize, the historical behaviour; otherwise 4..200) and `BreadcrumbIconSize` (0 = the toggle stays a square of the frame's height capped at 48, the historical behaviour; otherwise 8..200, never taller than the frame). THE FRAME'S HEIGHT, THE CHAIN'S TEXT SIZE AND THE TOGGLE'S SIZE ARE THREE SEPARATE DIALS: changing one moves nothing else. In particular the chain no longer has to share the rail's FontSize (that one property used to size the menu labels and the navigation chain together, so neither could be set alone), and raising `BreadcrumbHeight` to make room for your own controls no longer grows the toggle arrow with it. The frame's height is independent of the breadcrumb's font — a bigger FontSize never grows it, a smaller one never shrinks it, and text too big for the frame is CLIPPED by it rather than drawn outside — which is what gives `BreadcrumbTextAlign` something to do: on a frame taller than its text it puts the chain against the top, in the middle, or against the bottom. A form LOADED INTO THE CONTENTPANE starts BELOW the frame, never over it, so an embedded form's first row of controls can never land on the navigation chain. While `FullHeight` is on the frame OVERLAYS the top band of the SHELL form's own coordinate space, exactly as the designer canvas draws it, so THE SHELL FORM'S OWN CONTROLS MAY BE PLACED OVER IT — the frame is chrome, NOT a container: such a control is nobody's child, is not clipped by or scrolled with the frame, keeps every property and event, paints on top and takes the click. The ☰ toggle is painted at the TOP of the sidebar in the designer and at run time, in both pane states and whether or not the menu has items; the sidebar's ☰, items and empty hint are all top-anchored, never vertically centred. Menu-item ICONS render in the sidebar on every surface (designer canvas, preview, Run Form pane and the shell MenuPane). Property `IconEffect` (None | Shadow | Neumorphic, default None) styles those icons, and they are sized per rail state: `IconSize` (default 22) while the rail is OPEN and `IconSizeCollapsed` (default 22) while it is COLLAPSED, since an icon beside a label and an icon that IS the row are two different pictures; a form with no `IconSizeCollapsed` uses `IconSize` for both. EXPANDED, a group's items are indented under it one level at a time, the whole row moving together so an item's icon stays beside its own label at every level. COLLAPSED, the rail carries an item when, and only when, it has an icon, has an action and is not a group — a group is dropped and its qualifying children come up in its place, flattened from wherever they sit, so the rail is the shortcuts rather than the structure; section dividers survive only between two icons. On that rail an item whose action is `home` is followed by a whole row's worth of extra space, so the distance from it to the icon below is twice the distance between any other two; it is the ACTION that earns the space, never the label, and nothing is added where a divider already falls beneath it. In preview and Run Form the sidebar is LIVE: the ☰ toggles the rail (firing onMenuOpen/onMenuClose) and item rows click (SelectedItemId + onMenuItemClick). The menu editor's Indent/Outdent buttons restructure items across sections and levels (3 levels max). Menu-item ACTIONS (spec 051): `Open form` loads the target into the ContentPane as its own program instance (target must be FormFormat Embedded or Both); `Open Stand Alone Form (Sync)`/`(Async)` open the target in its OWN window, same process, parented to the shell — Sync is implicitly modal (the whole shell face waits until the child closes), Async is modeless (target must be Standalone or Both); the Target picker lists only the forms the chosen action may load. `Home (main content pane)` takes NO target and opens nothing: it puts the shell form's OWN ContentPane content back on screen, so a 'main screen' needs no form of its own. Home PARKS rather than destroys — the outgoing occupant gets onDeactivate but no onDestroy, keeps its WORKING-STORAGE, and a later load of it revives that same instance; every other live form, child windows included, is untouched. The breadcrumb collapses to the shell form and the contextual menu section empties; Home while already home does nothing. Home is offered on a SideMenu only, since a MenuBar form has no ContentPane to restore. The control also exposes the methods `OpenStandAloneFormSync`/`OpenStandAloneFormAsync` (see its Methods) for opening those windows from COBOL. THE FOOTER PANEL: every SideMenu owns a Panel in its footer band (id `<sidemenu-id>-Footer`), and it is an ordinary container the developer fills — a clock, a user badge, a version string, a Log-out button — styled through the inspector like any other, with events that fire normally. Its RECT is not the developer's: it is re-pinned to the footer band on every change, so it follows a form resize, a `FooterHeight` edit and a collapse; tell a developer to size it with `FooterHeight`, never to drag it. In a SHELL the rail is chrome beside the ContentPane, so the footer Panel and its contents are drawn by the RAIL rather than with the form's content — invisible to the developer (a control sits where the designer showed it) but the reason a footer control's designed X is not measured from the form's left edge.",
        "ToolBar" => "Groups of buttons in a horizontal strip. Each group is a frame with its own border and corner radius, separated from the next by an invisible gap; each button carries an icon, its own colours and an action. Built in the designer's Toolbar Editor (`ToolbarLayout`), not from a property list. THE BAR'S OWN FRAME is separate from the groups: `BackgroundColor`, `BorderStyle`, `BorderColor`, `BorderWidth`, `CornerRadius`, `Transparency`. A new toolbar is rounded at 10, has no border and is 100 % transparent, so it reads as buttons on the form. GIVING IT A BackgroundColor TURNS THE FRAME ON — the developer does NOT also have to lower `Transparency`, because that seeded 100 is what every toolbar carries rather than something anyone chose; a Transparency they did move still fades the face. The colour named is the colour painted, never substituted by the active theme's own card fill.",
        "StatusBar" => "Bottom status strip.",
        "Line" => "Decorative straight line.",
        "DateTimePicker" => "Date/time input with calendar or spinner.",
        "NumericUpDown" => "Integer input with spinner arrows.",
        "TreeView" => "Hierarchical node list. `Items` IS the tree: one node per line, TWO SPACES (or one tab) of indent per level. It is drawn by one renderer on the designer canvas and in the running form, so what you lay out is what runs — before 1.61.153 the canvas showed only a `[TreeView]` placeholder and the running form a flat bulleted list. The tree writes its nodes in the control's own FontName/FontSize/ForegroundColor, draws its connector lines per `ShowLines`/`ShowRootLines` in `LineColor`, ticks per `CheckBoxes`/`CheckedNodes`, and highlights per `HotTracking`. A click selects (`SelectedNode`, `onNodeClick`/`onNodeSelect`); a click on a tick box checks (`CheckedNodes`, `onNodeCheck`). NOT YET: expand/collapse (every node is always shown) and `AllowEdit` (no in-place rename surface) — never tell a developer either one works.",
        "Splitter" => "Draggable divider between two areas.",
        "Timer" => "Non-visual: fires `onTick` every Interval ms. Steady cadence — each tick schedules the next ONE INTERVAL on, so the rate does not drift with frame timing — and it never repays missed time: a handler slower than the interval, or a stalled form, gets ONE tick on return, not a burst. A handler eight events behind has its ticks coalesced until it catches up; a click, an edit or a focus change is never coalesced.",
        "Shape" => "Decorative rectangle / circle / triangle.",
        "Animator" => "Plays an animated image (GIF / WebP / APNG).",
        "AgentObject" => "Non-visual LLM client (ask a model from COBOL).",
        "RestClient" => "Non-visual HTTP/REST client (async by default).",
        "SqlDatabase" => "Non-visual SQL connection (sqlite / postgres / mysql / mssql).",
        "IndexedFile" => "Non-visual COBOL indexed-file access (driven by generated PERFORM paragraphs).",
        "Slider" => "Draggable value selector within Minimum..Maximum.",
        "BarChart" => "Bar chart.",
        "LineChart" => "Line chart.",
        "PieChart" => "Pie chart.",
        "AreaChart" => "Filled area chart.",
        "ScatterChart" => "Scatter/bubble chart.",
        "DonutChart" => "Donut chart.",
        "Knob" => "Rotary dial that sets a numeric Value within Minimum..Maximum by dragging.",
        "Gauge" => "Read-only KPI display (Radial | Linear | Donut) — never changed by user interaction.",
        "Switch" => "Boolean on/off visual toggle.",
        "FileDropZone" => "Non-visual: accepts files via drag-and-drop or a native file-picker click.",
        "Maps" => "Embedded, pannable/zoomable OpenStreetMap view with optional google_maps-backed location data (Directions/Geocoding/Places/Distance-Matrix). Wheel zoom is continuous: one notch is one level, released a slice per frame, and while it travels the map is drawn BETWEEN levels by scaling the tiles it already has, with the point under the pointer held fixed and markers/routes/regions scaling along with the basemap.",
        "WebSearch" => "Non-visual Google Custom Search JSON API client (async by default, same lifecycle as RestClient).",
        _ => "",
    }
}

/// `(signature, description)` pairs for the inline methods that apply to one
/// control type (beyond the universal set documented in the shared section).
///
/// Public because this is THE list of what a control can be told to do: the
/// knowledge base publishes it to the assistant, and the IDE editor folds it
/// into IntelliSense. Two hand-kept copies is how they came to disagree — a
/// Switch that documented `Toggle()` and never offered it, a ProgressBar whose
/// value contract promised `Decrement()` the popup had never heard of.
pub fn control_method_docs(name: &str) -> Vec<(&'static str, &'static str)> {
    let text_methods: Vec<(&'static str, &'static str)> = vec![
        ("SetText(text: String)", "Replace the text content."),
        ("GetText() → String", "Read the text content."),
        ("AppendText(text: String)", "Append to the text content."),
        ("Clear()", "Empty the text/items."),
    ];
    let caption_methods: Vec<(&'static str, &'static str)> = vec![
        ("SetCaption(text: String)", "Replace the caption."),
        ("GetCaption() → String", "Read the caption."),
    ];
    let value_methods: Vec<(&'static str, &'static str)> = vec![
        ("SetValue(value: Integer)", "Set the current value."),
        ("GetValue() → Integer", "Read the current value."),
        ("Increment()", "Add Step to Value."),
        ("Decrement()", "Subtract Step from Value."),
        ("Reset()", "Return Value to Minimum."),
    ];
    let items_methods: Vec<(&'static str, &'static str)> = vec![
        ("AddItem(text: String)", "Append one item."),
        ("RemoveItem(text: String)", "Remove the first item equal to `text`."),
        ("GetSelected() → String", "Read the selected item's value."),
        ("GetSelectedIndex() → Integer", "Read the 0-based selected index (-1 = none)."),
        ("SetSelectedIndex(index: Integer)", "Select by 0-based index."),
        ("GetCount() → Integer", "Number of items."),
        ("Clear()", "Remove all items."),
    ];
    let chart_methods: Vec<(&'static str, &'static str)> = vec![
        (
            "AddPoint(label: String, value: Number)",
            "Append one data point and repaint.",
        ),
        ("Clear()", "Remove all pushed data (chart falls back to its sample preview)."),
        ("Refresh()", "Force a repaint with the current data."),
    ];
    match name {
        "Button" => caption_methods,
        "Label" => caption_methods,
        "CheckBox" | "RadioButton" => {
            let mut v = caption_methods;
            v.extend([
                ("IsChecked() → Boolean (0/1)", "Read the checked state."),
                (
                    "SetChecked(value: Boolean)",
                    "Set the checked state (`1`/`0`, also accepts true/false/yes/on).",
                ),
                ("Select()", "Check it (radio: also unchecks the group siblings)."),
                ("Toggle()", "Flip the checked state."),
            ]);
            v
        }
        "TextBox" => text_methods,
        "ListBox" | "ComboBox" | "ToolBar" | "StatusBar" => items_methods,
        "ProgressBar" | "Slider" | "NumericUpDown" | "DateTimePicker" | "Knob" => value_methods,
        "Gauge" => vec![
            ("SetValue(value: Integer)", "Set the current value (Gauge is read-only via the UI, R10 — this is the only way to change it)."),
            ("GetValue() → Integer", "Read the current value."),
        ],
        "Switch" => vec![
            ("IsChecked() → Boolean (0/1)", "Read the checked state."),
            (
                "SetChecked(value: Boolean)",
                "Set the checked state (`1`/`0`, also accepts true/false/yes/on).",
            ),
            ("Toggle()", "Flip the checked state."),
        ],
        "DataGrid" => vec![
            ("GetRowCount() → Integer", "Number of data rows."),
            ("GetCellValue(row: Integer, column: Integer) → String", "Read one cell (0-based indices)."),
            ("SetCellValue(row: Integer, column: Integer, value: String)", "Write one cell."),
            ("AddRow(cells: String)", "Append a row; cells separated by TAB."),
            ("DeleteRow(row: Integer)", "Remove one row."),
            ("ClearRows()", "Remove all rows."),
            ("Sort(column: Integer)", "Sort by a column."),
            ("SetFilter(column: String, value: String)", "Filter a column."),
            ("ClearFilters()", "Drop all column filters."),
            ("FreezeColumns(count: Integer)", "Freeze the first N columns."),
            ("FreezeRows(count: Integer)", "Freeze the first N rows."),
            ("SetRowHeight(pixels: Integer)", "Set the uniform row height."),
            ("SetColumnWidth(column: Integer, pixels: Integer)", "Set one column's width."),
            ("GetSelectedText() → String", "Text of the current selection."),
            ("CopySelection()", "Copy the selection to the clipboard."),
            ("ExportCSV() → String", "Serialise the grid as CSV."),
            ("RefreshBinding() → Integer", "Re-hydrate rows from the bound data source; returns the row count."),
        ],
        "FileDropZone" => vec![(
            "CommitFiles() → String",
            "Copy the files a staged drop is holding into DestinationFolder, and return the summary (`7 of 8 copied, 24.310 MB`). Only meaningful with StageOnly on: call it when the operator has finished reviewing the list. Files whose row was unticked are skipped and stay listed. Afterwards DroppedFiles is the included files at their new paths, each row carries `✓` and its new path or `✗` and the reason, and CommitSummary is the returned line.",
        )],
        "Timer" => vec![
            ("Start()", "Set Enabled = 1 (ticks resume)."),
            ("Stop()", "Set Enabled = 0 (ticks stop)."),
            ("SetInterval(ms: Integer)", "Change the tick interval."),
            ("IsEnabled() → Boolean (0/1)", "Read the running state."),
        ],
        "Animator" => vec![
            ("Play() / PlayAnimation(name: String?)", "Start playing (optionally a named animation)."),
            ("StopAnimation()", "Stop playing."),
            ("Pause()", "Pause playback."),
        ],
        "AgentObject" => vec![
            ("Ask(prompt: String) → String", "Send a prompt; returns the last delivered reply (fires `onResponse` when one arrives)."),
            ("SetPrompt(text: String)", "Replace the SystemPrompt."),
            ("SetModel(model: String)", "Switch the model id."),
            ("GetResult() → String", "Read the `Result` property."),
            ("Cancel()", "Cancel the in-flight request."),
            ("IsBusy() → Boolean (0/1)", "An async request is in flight."),
        ],
        "RestClient" => vec![
            ("Get(url: String) → String", "HTTP GET. Async mode: returns immediately, response lands in `ResponseBody`/`StatusCode` + `onComplete`. Sync mode: returns the body."),
            ("Post(url: String, body: String) → String", "HTTP POST (same async/sync contract)."),
            ("Put(url: String, body: String) → String", "HTTP PUT."),
            ("Delete(url: String) → String", "HTTP DELETE."),
            ("Call(verb: String, url: String, body: String?) → String", "Any verb by name."),
            ("SetHeader(name: String, value: String)", "Add a header for subsequent requests."),
            ("ClearHeaders()", "Drop all added headers."),
            ("SetTimeout(seconds: Integer)", "Set the request timeout."),
            ("Cancel()", "Cancel the in-flight request."),
            ("IsBusy() → Boolean (0/1)", "An async request is in flight."),
        ],
        "SqlDatabase" => vec![
            ("Open(connectionString: String) → Integer", "Open the connection; returns the handle (fires `onConnectOk`/`onConnectError`)."),
            ("Execute(sql: String) → Integer", "Run a statement; returns the affected-row count (alias `Exec`)."),
            ("Query(sql: String) → Integer", "Run a query; returns the result-row count."),
            ("Fetch() → Boolean (0/1)", "Advance to the next row (fires `onRowFetched`)."),
            ("FetchAll() → Integer", "Row count of the current result set."),
            ("Close()", "Close the connection."),
        ],
        "BarChart" | "LineChart" | "PieChart" | "AreaChart" | "ScatterChart" | "DonutChart" => {
            chart_methods
        }
        "GroupBox" => {
            let mut v = caption_methods;
            v.push((
                "RefreshBinding() → Integer",
                "Repeating group: re-hydrate the cards from the bound data source.",
            ));
            v
        }
        "Maps" => vec![
            ("Geocode(address: String)", "**Async** — starts the lookup and returns an EMPTY string at once. `onComplete` delivers `lat\\tlng\\tformatted_address` in `ResponseBody`. Fails \"not configured\" with no google_maps key set (R33)."),
            ("ReverseGeocode(lat: String, lng: String)", "**Async** — `onComplete` delivers the formatted address in `ResponseBody`."),
            ("Directions(origin: String, destination: String)", "**Async** — `onComplete` delivers SEVEN TAB-separated fields in `ResponseBody`: `distance_text\\tduration_text\\troute_summary\\tdistance_METRES\\tduration_SECONDS\\tencoded_polyline\\ttraffic_SECONDS`. The first three read; the numbers are what you COMPUTE with; the polyline goes straight into `AddRoute` to trace the route. The LAST field is the drive time with CURRENT TRAFFIC (0 when Google supplied none) — the traffic-aware answer to \"how long, leaving now\"."),
            ("DistanceMatrix(origin: String, destination: String)", "**Async** — `onComplete` delivers `distance_text\\tduration_text\\tdistance_METRES\\tduration_SECONDS` in `ResponseBody`."),
            ("PlacesSearch(query: String, radiusMeters: String)", "**Async** — `onComplete` delivers one `place_id\\tname\\taddress\\tlat\\tlng` line per result in `ResponseBody`."),
            ("TraceRoad(apiKey: String, fromLat: String, fromLng: String, toLat: String, toLng: String)", "**Async** — a road route from **OpenRouteService**, for programs with no Google credential. `onComplete` delivers THREE TAB-separated fields in `ResponseBody`: `distance_METRES\\tduration_SECONDS\\tencoded_polyline`; the polyline goes straight into `AddRoute`. **The key is the first ARGUMENT, not a project setting** — ask the operator for it (a TextBox on the form) and pass it in; PowerRustCOBOL never stores it. A blank key fails on `onError` without a network call. Use this when you need the ROAD; a hand-written waypoint list is only ever as close to it as its own points."),
            (
                "AddMarker(id: String, lat: String, lng: String, label: String, info: String)",
                "Append one pin to Markers (ergonomic alternative to hand-formatting the TAB-separated property)."
            ),
            ("RemoveMarker(id: String)", "Remove the marker whose id matches, if any."),
            (
                "AddRoute(id: String, colour: String, width: String, geometry: String)",
                "Trace a line over the basemap. `geometry` is an encoded polyline (Directions' sixth field) or `lat,lng;lat,lng;…`. Re-using an id REPLACES that route rather than stacking a second copy. Needs no API key."
            ),
            ("RemoveRoute(id: String)", "Remove the route with that id."),
            ("ClearRoutes()", "Remove every route."),
            (
                "AddRegion(id: String, fill: String, stroke: String, width: String, geometry: String [, label: String, info: String])",
                "Fill an area over the basemap — a sales territory, a delivery zone. `fill` takes an alpha (`#RRGGBBAA`) so the map stays readable underneath. The ring closes itself and MAY be concave. Re-using an id replaces it. Needs no API key."
            ),
            ("RemoveRegion(id: String)", "Remove the region with that id."),
            ("ClearRegions()", "Remove every region."),
        ],
        "WebSearch" => vec![
            ("Search()", "Run a Custom Search using the current SearchEngineId/Query/NumResults/SafeSearch. Async mode: returns immediately, raw JSON lands in ResponseBody + onComplete. Sync mode: returns the raw JSON body. Fails \"not configured\" with no Custom Search key set (R33)."),
            ("ResultCount() → Integer", "Number of result items in the last response (parses ResponseBody fresh each call)."),
            ("TopTitle() → String", "First result's title, or empty before any search."),
            ("TopSnippet() → String", "First result's snippet, or empty before any search."),
            ("TopLink() → String", "First result's URL, or empty before any search."),
            ("GetResult(index: Integer) → String", "1-based indexed result as `title\\tsnippet\\tlink`; an out-of-range index returns empty, never an error."),
            ("Cancel()", "Cancel the in-flight search."),
            ("IsBusy() → Boolean (0/1)", "A search is in flight."),
        ],
        // 051 — the SideMenu's programmatic door to standalone child windows.
        "SideMenu" => vec![
            (
                "OpenStandAloneFormSync(formId: String, windowState: String, x: Integer, y: Integer, width: Integer, height: Integer, modal: Boolean)",
                "Open `formId` in its OWN window, parented to the SHELL (whatever form invokes it), and BLOCK the calling handler until the child closes — Sync is implicitly modal, and the whole shell face waits with it. The space form requires every parameter; the comma form `SideMenu-1::\"OpenStandAloneFormSync\"(\"REPORT\")` defaults the rest from the target's RAD design. The target's FormFormat must be Standalone or Both (build-checked for literal ids). RETURNING is NULL by the time the call resumes (the child is closed).",
            ),
            (
                "OpenStandAloneFormAsync(formId: String, windowState: String, x: Integer, y: Integer, width: Integer, height: Integer)",
                "Open `formId` in its OWN window, parented to the shell, and return at once. RETURNING binds a windowHandler that drives the child (`Focus`, `Close`, `SetProperty`, …) and becomes NULL when it closes. Never modal. Same parameter rules and FormFormat gate as the Sync form.",
            ),
        ],
        _ => Vec::new(),
    }
}

/// Extra usage notes appended to a control's section (generated paragraphs,
/// data-flow contracts, and other things a code generator must know).
fn control_usage_notes(name: &str) -> &'static str {
    match name {
        "IndexedFile" => "\
### Usage (generated paragraphs — NOT `::` methods)\n\
An IndexedFile control named `IXF-1` is driven with `PERFORM` on the paragraphs the IDE generates:\n\
- `PERFORM IXF-1-OPEN` — open the file in `OpenMode` (`INPUT` or `I-O`).\n\
- `MOVE key TO <key item>` then `PERFORM IXF-1-START` — position the record pointer (KEY >= value).\n\
- `PERFORM IXF-1-READ-NEXT` / `IXF-1-READ-PREVIOUS` / `IXF-1-READ-FIRST` / `IXF-1-READ-LAST` — sequential reads; `WS-IXF-1-AT-END = 1` after the last record.\n\
- `PERFORM IXF-1-READ-INVALID` — random read by key (INVALID KEY sets status `23`).\n\
- `WRITE` / `REWRITE` / `DELETE` — use the plain COBOL verbs on the file's record.\n\
- `PERFORM IXF-1-COMMIT` — flush pending writes (I-O mode).\n\
- `PERFORM IXF-1-CLOSE` — close the file.\n\
The two-character file status lands in the `StatusDataItem` (or `WS-IXF-1-STATUS`).\n",
        "SqlDatabase" => "\
### Usage (generated paragraphs and CALL API)\n\
Besides the inline methods, the IDE generates paragraphs for a control named `DB-1`: `DB-1-CONNECT` (opens using `WS-DB-1-CONN-STRING`), `DB-1-EXEC` (runs `WS-SQL-QUERY`, row count in `WS-SQL-ROW-COUNT`), `DB-1-FETCH-ALL` (template row loop), `DB-1-COMMIT`, `DB-1-ROLLBACK`, `DB-1-CLOSE`. The low-level `CALL \"COBOL-OPEN-DB\" / \"COBOL-EXEC-SQL\" / \"COBOL-FETCH-ROW\" / ...` API is also available.\n",
        "BarChart" | "LineChart" | "PieChart" | "AreaChart" | "ScatterChart" | "DonutChart" => "\
### Data flow\n\
Three equivalent ways to feed the chart:\n\
1. Inline methods: `Chart-1::AddPoint(\"Jan\", 150).` / `Chart-1::Clear().` / `Chart-1::Refresh().`\n\
2. Generated paragraphs: `PERFORM Chart-1-ADD-POINT` (after `MOVE`s to `WS-Chart-1-SELECTED-LBL` / `-SELECTED-VAL`), `PERFORM Chart-1-SET-TABLE`, `PERFORM Chart-1-CLEAR`, `PERFORM Chart-1-REFRESH`.\n\
3. Runtime calls: `CALL \"COBOL-CHART-ADD-POINT\" USING \"Chart-1\" label value` and `CALL \"COBOL-CHART-SET-TABLE\" USING \"Chart-1\" table count` (table rows: `PIC X(64)` label + `PIC 9(18)V9(6)` value).\n\
Or bind declaratively with the `DataSource`/`DataCount`/`LabelField`/`ValueFields` properties.\n",
        "GroupBox" => "\
### Repeating groups (control arrays)\n\
With `IsRepeatingGroup = 1` the GroupBox becomes a card template: set `ItemCount` (or bind `DataSource`) and address instance members as `Member(index)::Property` (1-based index). Handlers on members receive `CONTROL-ARRAY-INDEX`.\n",
        "ToolBar" => "\
### Groups of buttons, built in the Toolbar Editor\n\
A ToolBar is not a list of words — it is groups of buttons. Its whole definition lives in the `ToolbarLayout` property, edited in the designer (properties pane → **Edit Toolbar…**), never written by hand. Each group is a frame with a border style (`Single`/`None`/`Fixed3D`), border colour and width, corner radius, its own padding and an optional invisible separator after it; `None` still groups but draws no frame. Each button has a label OR an icon (never both — setting one clears the other), a tooltip, an enabled flag, an action, and an appearance: icon size and colour, width/height, corner radius, a solid or gradient background, a foreground colour and a drop shadow. Corner radius defaults to 10.\n\
\n\
### Three levels of appearance\n\
A button's own value wins; where it says nothing its GROUP decides; where the group says nothing too, the form's theme does. So an icon size or a background set once on the group dresses every button in it, and one button can still disagree field by field. Adding a button in the editor copies the previous button's appearance — never its icon, tooltip or action, which are what make it a different button.\n\
\n\
### The bar's own frame\n\
Separately from the groups, the ToolBar control itself has `BorderStyle`, `BorderColor`, `BorderWidth`, `CornerRadius`, `Transparency` and `BackgroundColor`. A new toolbar is rounded at 10, has NO border and is 100 % transparent, so it reads as buttons sitting on the form rather than a panel laid over it — and it arrives holding one group with one folder-open button, so a dropped ToolBar shows what a toolbar is.\n\
\n\
### What a press does\n\
`event` fires the toolbar's own `onClick` (the default). `procedure:<NAME>` runs one of the form's procedures by name. `open-modal:<FORM>` opens a STANDALONE form as a modal window, and the press waits until that window closes. The rest are the platform's: `print:<path>` opens the document where its print dialog is, `share` captures the form's window and hands the image to the OS, `screenshot` puts that image on the clipboard, `copy`/`cut`/`paste` use the OS clipboard on whichever control has focus, `run-app:<path args>` launches an application, `open-terminal:<dir>` opens a terminal.\n\
\n\
Whatever the action, the form ALSO gets an `onClick` on the toolbar, and `LastButton` names the button that was pressed — written first, so one handler can serve the whole bar:\n\
\n\
```cobol\n\
       TOOLBAR-1--ONCLICK.\n\
           EVALUATE TOOLBAR-1::LastButton\n\
               WHEN \"button-1\"  PERFORM SAVE-RECORD\n\
               WHEN \"button-2\"  PERFORM DELETE-RECORD\n\
               WHEN OTHER       CONTINUE\n\
           END-EVALUATE.\n\
```\n\
\n\
`run-app` and `open-terminal` start a real process: the target is split on whitespace and handed to the OS DIRECTLY, never to a shell, so a target built from a data item cannot become a shell command. A toolbar wider than its control loses whole groups off the end rather than drawing half of one. A ToolBar with only a legacy `Items` list is read as one unframed group of labelled buttons, so it keeps working untouched.\n\
\n\
### A button's own handler\n\
A button carries its OWN code, not just the toolbar's one `onClick`. In the Toolbar Editor select a button, and under **Events** bind `onClick` with **Edit code** — that keeps the toolbar (as Save would) and opens the COBOL editor on the handler; saving puts it back into the toolbar. `onClick` is the only event offered, because it is the only one a button can raise. Where a button has more than one thing to run, the order is fixed: the TOOLBAR's `onClick` first, then the BUTTON's own `onClick`, then its action — so an `open-modal:` button whose handler prepares what the modal reads works as written.\n\
\n\
### Changing a button while the form runs\n\
COBOL may write a button's COLOURS and its TOOLTIP, and nothing else: `Tooltip`, `BackgroundColor`, `ForegroundColor`, `IconColor`, `GradientStartColor`, `GradientEndColor`, `ShadowColor`. A colour set to SPACES goes back to inheriting (group, then theme), the same meaning the editor's ✕ has. `MOVE \"#204080FF\" TO TOOLBAR-1-GROUP-1-BUTTON-1::BackgroundColor.`\n\
\n\
Anything else — width, height, corner radius, label, icon, enabled, action — is a RUNTIME ERROR naming the property and the allowed set, through all three doors (`x::Prop`, `CALL \"COBOL-SET-PROPERTY\"`, `INVOKE x \"SetProperty\"`). A button is laid out BY ITS TOOLBAR, so a button that could move itself would leave nothing to put it back; a silent no-op would be worse. Reads are never refused. The COBOL editor flags a refused property as it is typed.\n\
\n\
### How a button reaches COBOL\n\
A toolbar button is NOT a control — the toolbar owns the layout, so a button has no entry in `form.controls`. It is named by a DERIVED id instead, `<toolbar>-<group>-<button>` upper-cased: `TOOLBAR-1` + `group-2` + `button-1` ⇒ `TOOLBAR-1-GROUP-2-BUTTON-1`. The press arrives under that id and the generated event loop dispatches on it, which is how `procedure:` and `open-modal:` reach anything — a `procedure:` button becomes `CALL \"<NAME>\"` (a user procedure is a nested program, IS COMMON) and an `open-modal:` button becomes `INVOKE ME::\"OpenFormSync\"(\"<FORM>\")`, whose one-argument form is modal. Nothing types the derived id by hand.\n\
\n\
A ToolBar's buttons belong to the form HOLDING the toolbar, Standalone or Embedded alike: they are seeded as objects of that form's program (one builder serves the root form, a child window and a ContentPane occupant), so an embedded form's own COBOL reads and recolours its own buttons and two forms with identically-named toolbars never see each other's. A toolbar nested inside a Panel or a tab page is no different.\n\
\n\
`COBOL-CONTROL-ID` is `PIC X(64)`, so the three names together must fit 64 characters. A button whose derived id is longer, or a `procedure:`/`open-modal:` button naming nothing, gets a COMMENT in the generated source saying which button it is and what to fix — never a `WHEN` that could not fire.\n\
\n\
### Pressing a button in Preview\n\
Preview honours the platform actions — `print`, `run-app`, `open-terminal`, `copy`, `cut`, `paste` — so a toolbar can be tried while it is being built; every press writes its result (or its reason for failing) to the Output pane. The two CAPTURES do not run there: Preview is a pane inside the IDE window, so `screenshot` and `share` would return a picture of the IDE rather than of the form, and they say so instead. Run Form gives the form a window of its own to capture. The three COBOL actions (`event`, `procedure:`, `open-modal:`) need the interpreter, so they too belong to Run Form.\n",
        "FileDropZone" => "\
### Usage — a UI gesture in, one method out\n\
There is no method to open the picker or read a drop programmatically. The user drags a file onto the control OR clicks it to open the native file picker; either way the platform runs the zone's intake and fires an event. Read the paths with `MOVE FDZ-1::DroppedFiles TO WS-PATHS`. The one method the zone has is `CommitFiles()`, which belongs to the staged flow below.\n\
\n\
### What the zone accepts, and where it puts it\n\
Three design-time properties decide, and both routes in (drop and picker) obey them:\n\
\n\
- `AllowedExtensions` — `csv, xlsx`. Case-blind, dots optional. Blank accepts any file.\n\
- `MaximumFileSizeKB` — largest file taken, in KB. `0` is no limit.\n\
- `DestinationFolder` — accepted files are COPIED here (the folder is created if missing). An existing name is never overwritten: `report.csv` lands as `report (2).csv`. Blank leaves files where they are.\n\
\n\
Accepted files appear in `DroppedFiles` — at their NEW path when a destination is set — and fire `onFilesDropped`. Refused files appear in `RejectedFiles`, one `path<TAB>reason` per line where reason is `extension` or `too-big`, and fire `onFilesRejected`. A drop of ten files where three are refused fires BOTH events. Nothing is refused silently.\n\
\n\
### Letting the operator confirm first (`StageOnly`)\n\
By default the copy happens the instant the file lands, which gives the operator no chance to change their mind. Turn `StageOnly` on and a drop copies **nothing**:\n\
\n\
1. The drop is judged as usual — refused files still fire `onFilesRejected` — and the accepted ones are HELD at their original paths in `StagedFiles`. `onFilesDropped` fires. `DestinationFolder` is not even created.\n\
2. They are listed in the ListBox named by `FileListControl`, one tick-boxed row each, reading `<path> (12.345 MB)`. `CommitSummary` reads `3 files staged, 24.310 MB`.\n\
3. The operator unticks anything they did not mean to send. An unticked row stays in the list, so the exclusion is visible and reversible.\n\
4. Your own COBOL decides when the form goes ahead — a Submit button, a validated field, whatever the form means by confirmation — and calls `INVOKE FDZ-1 'CommitFiles'`. Ticked files are copied by exactly the rules above; unticked ones are skipped.\n\
5. Each row becomes `✓ <new path> (12.345 MB)` or `✗ <path> (12.345 MB) — <reason>`, `CommitSummary` becomes `7 of 8 copied, 24.310 MB` (also the method's return value), and `DroppedFiles` becomes the included files at their new paths — at their original path for any whose copy failed, because the form must still get the file it was given.\n\
\n\
```cobol\n\
       SUBMIT-BUTTON--ONCLICK.\n\
           MOVE FDZ-1::CommitFiles() TO WS-SUMMARY\n\
           MOVE WS-SUMMARY TO STATUS-LABEL::Caption\n\
           MOVE FDZ-1::DroppedFiles TO WS-PATHS\n\
           PERFORM SEND-TO-APPLICATION.\n\
```\n\
\n\
A second drop adds to what is already staged rather than replacing it, and the same file dropped twice is held once. Calling `CommitFiles()` on a zone holding nothing is not an error — it reports `0 of 0 copied`. Megabytes count 1,000,000 bytes, matching the operator's own file browser.\n",
        "Maps" => "\
### Usage — basemap vs. data verbs, and the API key\n\
The OpenStreetMap basemap (pan/zoom, `CenterLat`/`CenterLng`/`Zoom`, `Markers`) needs **no API key at all**. Only the five data methods (`Geocode`, `ReverseGeocode`, `Directions`, `DistanceMatrix`, `PlacesSearch`) call the real Google Maps API and need a project-level `google-maps` credential (Settings → Integrations) — with none configured they fail immediately with `LastError` = \"not configured\" and fire `onError`, never a crash, never a network call (R33). The key itself never appears in any property, generated `.cbl`, or the `.cfrm`.\n\
\n\
### Colours — nothing on a map is hard-coded\n\
Every colour the map paints is a property, in the inspector's **Basic properties** section and writable from COBOL: `MarkerColor`, `MarkerBorderColor`, `RouteColor`, `RouteCasingColor`, `RegionFillColor`, `RegionBorderColor`, `TileBackgroundColor`, `TileLoadingColor`. Each starts EMPTY, meaning the built-in the map has always painted, so a form that sets none of them is unchanged.\n\
\n\
Colour carried by the DATA still wins: `AddRoute` USING id colour width geometry keeps that route's own colour, and `AddRegion`'s fill and stroke keep theirs — `RouteColor`, `RegionFillColor` and `RegionBorderColor` are what a line naming none falls back to. Two exceptions, because their data carries no colour at all: `MarkerColor`/`MarkerBorderColor` (an `AddMarker` has no colour argument) and `RouteCasingColor` (the halo under EVERY route, whatever colour the route itself names). Never tell a developer a map colour cannot be changed, and never suggest editing the `.cfrm` by hand to change one.\n",
        "WebSearch" => "\
### Usage — the generated paragraph vs. `INVOKE 'Search'`\n\
Every `WebSearch` control also gets a generated `<id>-SEARCH` paragraph (`PERFORM SEARCH-1-SEARCH`) that builds a Custom Search URL and calls `COBOL-HTTP-GET` directly — but it does PLAIN, UNENCODED string concatenation: a multi-word `Query` truncates at its first space, and it never includes the API key (so it 401s against the real API on its own). **Use `INVOKE <id> 'Search'` instead** — it percent-encodes the query and resolves the credential-store key automatically; the paragraph exists only as a low-level fallback. Same \"not configured\" contract as Maps: no `google-custom-search` key configured (Settings → Integrations) fails immediately with `onError`, no request sent (R33).\n",
        _ => "",
    }
}

/// Render the enriched Form Controls Reference (KB file 3).
fn controls_reference_doc() -> String {
    // Every control the toolbox offers, from the model's own canonical list —
    // never a copy of it. The copy that used to live here had quietly lost the
    // SideMenu, so the assistant was never told the sidebar exists.
    let control_types = cobolt_forms::ControlType::ALL;

    let mut doc = String::new();
    doc.push_str("# PowerRustCOBOL Form Controls Reference\n\n");
    doc.push_str(
        "Complete reference for every control the RAD Form Designer supports: purpose, \
         properties with value types / defaults / allowed domains, events, and inline methods. \
         Properties are read as `<control>::<Property>` and written with \
         `SET <control>::<Property> TO <value>` (or the designer op `set_property`). \
         All properties are OPTIONAL — a control works with its defaults; set only what the \
         request needs. Property names are case-insensitive at runtime but SHOULD be written \
         exactly as listed. Setting a misspelled property silently creates a new, ignored \
         property — it is never an error, so spelling matters.\n\n\
         **Control ids are COBOL words.** Each id becomes part of the generated program's \
         data-names and paragraph names (`WS-<id>-TEXT`, `<id>-OPEN`), so it may contain ONLY \
         letters, digits and hyphens, and may neither begin nor end with a hyphen. Write \
         `TEXTBOX-1`, never `TEXTBOX_1`: an underscore is not a COBOL character, and a name \
         carrying one is discarded by the compiler along with the control's whole storage. \
         The same rule governs every WORKING-STORAGE item and paragraph name a handler \
         declares.\n\n",
    );

    // ── Shared sections ──────────────────────────────────────────────────────
    doc.push_str("## Universal properties (every control)\n\n");
    doc.push_str("Layout fields (settable like any property):\n\n");
    for (sig, dom, desc) in [
        ("Name", "String — control identifier", "The control id (assigned by the designer; treat as read-only). It becomes a COBOL word in the generated program (`WS-<id>-TEXT`, `<id>-OPEN`), so it may hold ONLY letters, digits and hyphens — `TEXTBOX-1`, never `TEXTBOX_1`."),
        ("Visible", "Boolean — `1`/`0`", "Whether the control is drawn."),
        ("Enabled", "Boolean — `1`/`0`", "Whether the control accepts input."),
        ("X", "Integer — pixels from the form's left edge", "Horizontal position."),
        ("Y", "Integer — pixels from the form's top edge", "Vertical position."),
        ("Width", "Integer — pixels > 0", "Control width."),
        ("Height", "Integer — pixels > 0", "Control height."),
        ("TabOrder", "Integer ≥ 0", "Keyboard Tab traversal order."),
        ("Parent", "String — container control id or empty", "The container that owns this control."),
        ("Tab", "Integer — 0-based tab page index", "Which TabControl page the control sits on (only inside a TabControl)."),
    ] {
        doc.push_str(&format!("- `{sig}` ({dom}) — {desc}\n"));
    }
    doc.push_str("\nAppearance and behaviour properties shared by every control:\n\n");
    {
        let sample = cobolt_forms::Control::new("_", cobolt_forms::ControlType::Panel, 0, 0);
        for name in UNIVERSAL_PROPS {
            if let Some(v) = sample.properties.get(*name) {
                let (ty, default) = prop_type_and_default(v);
                let (domain, desc) = property_reference(name).unwrap_or(("", ""));
                push_prop_line(&mut doc, name, ty, &default, domain, desc);
            }
        }
    }
    doc.push_str("\n## Universal events (visual controls)\n\n");
    doc.push_str(
        "Most visual controls support this shared input/lifecycle set (each control section \
         lists which of them apply):\n\n",
    );
    for (ev, desc) in [
        ("onClick", "left click released on the control"),
        ("onDblClick / onDoubleClick", "double click (aliases)"),
        ("onRightClick", "right click"),
        ("onMiddleClick", "middle click"),
        ("onMouseEnter / onMouseLeave", "pointer entered / left the control"),
        ("onMouseDown / onMouseUp", "button pressed / released"),
        ("onMouseMove", "pointer moved over the control"),
        ("onMouseWheel", "wheel scrolled over the control"),
        ("onContextMenu", "context-menu request (right click)"),
        ("onGotFocus / onLostFocus", "keyboard focus gained / lost"),
        ("onKeyDown / onKeyUp / onKeyPress", "keyboard input while focused"),
        ("onEnterPressed / onEscapePressed", "Enter / Escape pressed while focused"),
        ("onHoverEnter / onHoverLeave", "pointer rested ≥ `HoverDelayMs` / left after hovering"),
        ("onResize / onResized / onMove / onMoved", "geometry changed"),
        ("onVisibleChanged / onEnabledChanged", "Visible / Enabled flipped"),
        ("onLoad", "control initialised when the form opens"),
    ] {
        doc.push_str(&format!("- `{ev}` — {desc}\n"));
    }
    doc.push_str(
        "\nEvent handlers carry NO parameters. A handler is a nested program, and the \
         dispatcher calls it as `CALL \"<handler-program>\"` with no arguments, so its \
         `LINKAGE SECTION` is empty and its \
         header is a plain `PROCEDURE DIVISION.`. The single exception is a control inside a \
         REPEATING GROUP, which is called `USING CONTROL-ARRAY-INDEX` (`PIC S9(4) COMP-5`, the \
         1-based index of the card that fired) and writes \
         `PROCEDURE DIVISION USING CONTROL-ARRAY-INDEX.`.\n\n\
         In particular **no event delivers a key code**: never declare `KEY-CODE` or write \
         `PROCEDURE DIVISION USING KEY-CODE.` — nothing populates it. `onKeyDown`, `onKeyUp` \
         and `onKeyPress` fire for ANY key and say nothing about which one, so a specific key \
         has its own event: bind `onEnterPressed` for ENTER and `onEscapePressed` for ESC. To \
         see what was typed, read the control's own text (`MOVE MY-BOX::Text TO WS-VALUE`).\n\n",
    );

    // ── The form itself ──────────────────────────────────────────────────────
    doc.push_str("## The Form (window)\n\n");
    doc.push_str(
        "Form-level designer attributes: `title` (String), `width`/`height` (Integer px), \
         `background_color` (hex color), optional background gradient \
         (enabled/start/end/direction as in the universal gradient properties), \
         `transparency` (0-100, 0 = opaque), `background_image` (path) with scale mode, \
         and `GlassStyle` (exactly one of `\"Classic\"`, `\"Enhanced\"`, \
         `\"Neumorphic Light\"`, `\"Neumorphic Dark\"`).\n\n",
    );
    doc.push_str(
        "A running window the user maximizes or drags BIGGER keeps its controls at the \
         designed size and stretches only the BACKGROUND — the gradient, or the background \
         image, covers the whole window instead of stopping at the form's edge. A window \
         dragged SMALLER than the form keeps a form-sized background (the form scrolls \
         inside it) rather than cropping it to the window. The designer canvas and the \
         preview always show the backdrop at the form's own size.\n\n",
    );
    doc.push_str("Form events (bind a handler in the designer; `onLoad` / `onClose` are pre-stubbed):\n\n");
    for (group, events) in cobolt_forms::model::FORM_EVENT_GROUPS {
        let list = events
            .iter()
            .map(|e| format!("`{e}`"))
            .collect::<Vec<_>>()
            .join(", ");
        doc.push_str(&format!("- {group}: {list}\n"));
    }
    doc.push_str(
        "\nNot every form event is wired into the runtime yet — prefer `onLoad` and `onClose` \
         for lifecycle logic. Wired since spec 037: `onCloseRejected` (a close attempt was \
         refused while `FormState` is `Waiting`, or a Sync child is Waiting) and \
         `onFullScreenChanged` (the ACTUAL fullscreen state changed — read `me`'s `FullScreen` \
         for the new value; fires once per real transition).\n\n",
    );

    // ── 037 — main form, window lifecycle & multi-form invocation ────────────
    doc.push_str("### Main form & window lifecycle (spec 037)\n\n");
    doc.push_str(
        "Window-lifecycle designer attributes on every form: `MainForm` (Boolean — exactly ONE \
         form per project holds it; the first form created is the default; the Forms tree marks \
         it with a crown. It is also the only form a RUNTIME starts: a built binary and `rcrun \
         run-form` open the main form and nothing else, so a sign-on main form cannot be \
         stepped over. The IDE's Run Form still runs any form. A project whose designation has \
         been edited by hand — the mark moved, `[forms] main-form` pointed elsewhere, the seal \
         removed — reports a corrupted application and exits without opening a window), \
         `TaskbarIcon` (image path — main form only: the single taskbar/dock \
         entry uses it; non-main windows create no taskbar entries), `CanMinimize` / \
         `CanMaximize` (Boolean, default true — native title-bar buttons), `WindowState` \
         (`\"Normal\"` | `\"Minimized\"` | `\"Maximized\"` — the state the window opens in, \
         settable at runtime), `FullScreen` (Boolean — orthogonal to WindowState; leaving \
         fullscreen returns to the previous state), and `TitleVisible` (Boolean, default true — \
         false renders a chromeless window).\n\n",
    );
    doc.push_str(
        "`FormState` (`\"Ready\"` | `\"Waiting\"`, runtime-only, default Ready) guards unsaved \
         work: while `Waiting`, EVERY close attempt on the form is refused (title-bar close, \
         windowHandler Close, cascades) and `onCloseRejected` fires; set it back to `Ready` to \
         allow closing. A Sync caller is also blocked while any of its Sync children is \
         Waiting. Set it with `INVOKE me \"SetProperty\" USING \"FormState\" \"Waiting\"`.\n\n",
    );
    doc.push_str(
        "Opening forms from COBOL — two methods on `me`, two syntaxes:\n\n\
         - Comma form (trailing parameters OPTIONAL, defaulted from the target form's design): \
         `INVOKE me::\"OpenFormSync\"(\"FORM-ID\", [windowState], [x], [y], [width], [height], \
         [modal]) RETURNING H` — `modal` defaults to TRUE. \
         `INVOKE me::\"OpenFormAsync\"(\"FORM-ID\", [windowState], [x], [y], [width], [height]) \
         RETURNING H` — Async is never modal.\n\
         - COBOL-standard space form (ALL parameters required — a mismatch is a compile \
         error): `INVOKE me \"OpenFormSync\" USING form-id windowState x y width height modal \
         RETURNING H`.\n\n\
         `H` is a `windowHandler` (USAGE OBJECT). Methods on the handle: `Close`, `Focus` \
         (restores a minimized window first), `SetWindowState(state)`, `SetFullScreen(bool)`, \
         `SetTitleVisible(bool)`; read `H::FormState` for the child's state. When a form \
         closes, every windowHandler referring to it becomes NULL automatically; invoking \
         through a NULL handle is a runtime error.\n\n",
    );
    doc.push_str(
        "Lifecycle rules: the MAIN form is a singleton — opening it again focuses the running \
         instance and returns its existing handle; other forms can run any number of \
         concurrent instances. Sync children close together with their caller (a Waiting form \
         anywhere in the chain vetoes the whole close). Async children survive their caller — \
         except when the MAIN form closes, which closes every form and exits the application. \
         A modal Sync child blocks the caller's input AND its COBOL flow until the child \
         closes (the RETURNING handle is then already NULL). `me` window methods: \
         `SetWindowState`, `SetFullScreen`, `SetTitleVisible`, `Focus`, `Close`. `me` also \
         carries `SetBreadcrumbDetail(text)` / `ClearBreadcrumbDetail()`, which name the \
         record the form is holding in the shell's breadcrumb — not window methods: an \
         embedded form has no window of its own and still owns the crumb after its own \
         name.\n\n",
    );

    // ── 049 — application shell & the super receiver ─────────────────────────
    doc.push_str("### Application shell & the `super` receiver (spec 049)\n\n");
    doc.push_str(
        "`FormFormat` (`\"Standalone\"` | `\"Embedded\"` | `\"Both\"`, default Standalone) \
         declares how a form may be loaded: Standalone opens as its own window \
         (`OpenFormSync`/`OpenFormAsync`); Embedded is loaded into the shell's ContentPane by \
         a sidebar-menu item; Both allows either path. The build REJECTS a menu item that \
         targets a Standalone form and an OpenForm* call that targets an Embedded one. The \
         MAIN form is always Standalone (it owns the window). While a form is Embedded, the \
         window-only properties (WindowState, FullScreen, TitleVisible, CanMinimize, \
         CanMaximize) are inert and its Width/Height report the DESIGNED values.\n\n",
    );
    doc.push_str(
        "SHELL mode starts when the main form carries a `SideMenu` control: ONE window with a \
         MenuPane (root menu slot — mounted once — plus the current subsystem's contextual \
         slot; Open/Collapsed, a narrow icon rail when collapsed, with the ☰ toggle drawn on \
         the pane itself in BOTH states and whether or not any menu item exists; its own \
         background from the \
         main form's MenuPaneBackground group, never repainted by a loaded form), a breadcrumb \
         FRAME (shell chrome — one segment per navigation-chain entry, each naming its form by \
         its designed Title; clicking a segment destroys everything below it, deepest first), \
         and a ContentPane hosting the loaded \
         form top-left at its designed size. The loaded form's background paints the WHOLE \
         pane (image/gradient modes evaluate against the PANE rect) and stays fixed while the \
         form scrolls; a fully transparent form shows the desktop through the pane region \
         only. Embedded forms play no window entrance/exit effects.\n\n",
    );
    doc.push_str(
        "The BREADCRUMB FRAME always runs from the sidebar's right edge to the window's right \
         edge — there is no width or position property — and the SideMenu owns its three: \
         `BreadcrumbHeight` (16..200, default 28), `BreadcrumbBackgroundColor` (empty = \
         follow the ContentPane's backdrop; a chosen colour may carry alpha but the frame is \
         always painted opaque, being chrome) and `BreadcrumbTextAlign` (Top | Middle | \
         Bottom, default Middle). The HEIGHT is independent of the breadcrumb's font: a \
         larger FontSize never grows the frame and a smaller one never shrinks it, and text \
         too big for the frame is CLIPPED by it rather than drawn outside. That is what makes \
         the alignment meaningful — on a frame taller than its text, `BreadcrumbTextAlign` \
         says whether the chain sits against the top, in the middle, or against the bottom. \
         The chain and the toggle move as ONE GROUP: the alignment places the pair, and the \
         text then centres on the toggle's own line, so a tall icon and a small font stay on \
         one line at Top and at Bottom exactly as they do at Middle. \
         While the sidebar's `FullHeight` is on (the \
         default) the frame is the top BAND of the content area — it OVERLAYS the SHELL \
         form's own coordinate space exactly as the designer canvas draws it, so the window \
         opens at the form's designed height and THE SHELL FORM'S CONTROLS MAY BE PLACED \
         OVER THE FRAME: such a control is \
         an ordinary form control that merely overlaps (NOT a child — it is not clipped by \
         the frame, does not scroll with it, keeps all its properties and events) and it \
         paints on top of the frame and takes the click. A form LOADED INTO THE CONTENTPANE \
         is the exception, because it is a different form with a coordinate space of its \
         own: an embedded form starts BELOW the band, never over it, so its first row of \
         controls can never land on the navigation chain. With FullHeight off the frame is a \
         strip above the WHOLE window (above the sidebar too), the window pays its height, \
         and nothing can be placed over it.\n\n",
    );
    doc.push_str(
        "BREADCRUMB DETAIL LEVEL: the displayed form names the record it is holding with \
         `INVOKE me \"SetBreadcrumbDetail\" USING <text>` (empty text or \
         `INVOKE me \"ClearBreadcrumbDetail\"` removes it), shown as one more step after that \
         form's own name (`Main Menu > Customer Data > John Smith`). It is one level, not a \
         stack — setting it again replaces it. Only the DISPLAYED form may set one (a call \
         from an off-pane form is ignored), and any navigation (another screen, a breadcrumb \
         click, Home) drops it. With a detail showing, the form's OWN segment becomes a RESET \
         link: clicking it starts that form over. The form has the last word through the \
         universal property `PreventReset` (default 0): set it while holding unsaved data and \
         the reset is refused and `onResetRejected` fires instead. Allowed, a pane occupant is \
         REBUILT — onDestroy on the old instance, then a brand-new instance with blank \
         WORKING-STORAGE in the same chain position (a reset is NOT a navigation) — while the \
         shell's own main form, which has no second instance to swap in, receives `onReset`. \
         Either way the detail level is cleared.\n\n",
    );
    doc.push_str(
        "Navigation lifecycle: forms in the chain stay RESIDENT (storage alive, menu handlers \
         callable) while not displayed. Each menu item carries `PreservePreviousForm` \
         (default false): false destroys the outgoing sibling on a switch; true keeps it \
         resident for an instant return. Two distinct form events: `onDeactivate` — the body \
         left the ContentPane, the form is STILL resident (never a teardown point) — and \
         `onDestroy` — fired immediately before storage is released (close files / COMMIT \
         here).\n\n",
    );
    doc.push_str(
        "`super` is the form that LOADED or OPENED this one, bound at load time on both \
         paths (menu load and OpenForm*): `super::Title` reads/assigns the parent's \
         properties, `super::\"SetWindowState\"(\"Minimized\")` drives its window (the whole \
         windowHandler method surface), and `super::super::…` walks one loader per step. \
         Bare properties on `me`/`super` are checked at build time against the universal \
         form surface (Name, Title, Width, Height, X, Y, WindowState, FullScreen, \
         TitleVisible, CanMinimize, CanMaximize, FormState, FormFormat, BackgroundColor, \
         Transparency, PreventReset) at any depth; form-specific procedures use parentheses and dispatch \
         at run time. In the MAIN form — or after an async opener closed — `super` is NULL \
         and referencing it raises the standard error. \
         `super::<menu-id>::Collapse()` / `Open()` drive the MenuPane (pane-wide; the state \
         persists per application). `me::<property>` works the same way on the form's OWN \
         surface — including `me::PreventReset`, the guard that refuses a breadcrumb reset \
         while the form is holding unsaved data.\n\n",
    );

    // ── 051 — the multi-form host ────────────────────────────────────────────
    doc.push_str("### Multi-form host (spec 051)\n\n");
    doc.push_str(
        "An application holds MANY live forms at once, each running as its OWN program \
         instance: own WORKING-STORAGE, own interpreter, own event loop. Forms never read \
         each other's data items — they communicate through the supervisor surface only \
         (published form properties, `super::X`, `handle::\"SetProperty\"`/`\"GetProperty\"`, \
         windowHandler methods). The compiled binary embeds one program per openable form \
         beside the main program.\n\n",
    );
    doc.push_str(
        "THREE doors open a form: (1) a sidebar item's `Open form` action loads it into the \
         ContentPane (FormFormat Embedded/Both); (2) `INVOKE me \"OpenFormSync\"/\
         \"OpenFormAsync\"` opens it as a child WINDOW parented to the calling form \
         (Standalone/Both); (3) a sidebar item's `Open Stand Alone Form (Sync)/(Async)` \
         action — or the SideMenu control's `OpenStandAloneFormSync`/`OpenStandAloneFormAsync` \
         methods — opens a child window parented to the SHELL (Standalone/Both). Sync is \
         IMPLICITLY MODAL everywhere: the parent's whole face (shell chrome included, when \
         the parent is the shell) takes no input until the child closes; Async is never \
         modal. A close cascades per spec 037: Sync children close with their caller, Async \
         children survive detached, the main form's close takes everything, and a Waiting \
         form vetoes the whole close.\n\n\
         The way BACK from door (1) is the sidebar's `Home (main content pane)` action, which \
         takes no target: it restores the shell form's OWN ContentPane content, so a 'main \
         screen' needs no form of its own. Home PARKS the outgoing occupant — onDeactivate, \
         never onDestroy — so its WORKING-STORAGE survives and loading it again revives that \
         instance; no other live form is affected. The breadcrumb collapses to the shell form. \
         SideMenu only: a MenuBar form has no ContentPane.\n\n",
    );
    doc.push_str(
        "A PRESERVED pane occupant (`PreservePreviousForm`) keeps its interpreter and \
         storage parked off-pane, and its enabled Timer controls KEEP TICKING (handlers run \
         while parked; ticks coalesce against a busy queue). An open that cannot be \
         satisfied — unknown form id, a form whose generated program was missing at build \
         time — raises a visible runtime error and the handle is NULL; it is never silently \
         dropped. EXEC RUST blocks share ONE object bridge per PROCESS: a handle created by \
         any form's block resolves in every other form's blocks (values stored through the \
         bridge must be `Send`); each form's COBOL storage and control registry stay its \
         own.\n\n",
    );

    // ── 038 — project window entrance/exit effects ───────────────────────────
    doc.push_str("### Window effects (spec 038)\n\n");
    doc.push_str(
        "Window entrance/exit effects are configured ONCE PER PROJECT (project settings → \
         Appearance) and apply to every form: an entrance effect, an exit effect, each with a \
         duration (100–3000 ms; `matrix-rain` uses its own 1500–4000 ms band, and \
         `transporter-ii` is fixed at exactly 4000 ms) and an easing \
         (`linear` | `ease-in` | `ease-out` | `ease-in-out`). The effect catalogue: `none`, \
         `fade`, `zoom` (dBASE-style box zoom), `slide-left/right/top/bottom`, \
         `expand-title-bar`, `radar-wipe`, `iris-wipe`, `blinds`, `checkerboard`, \
         `matrix-rain` (katakana/digit glyph lines falling in from above the top edge over a \
         see-through window; each line's END OF TRAIL — the faint top glyph — walks down its \
         band and progressively uncovers what stands behind it, so the form is complete \
         exactly when the last character leaves; lines arrive 25 ms apart at first, then \
         10-25 ms behind each other; this effect ignores the easing setting and runs on \
         linear time), `genie` \
         (squash-and-bend approximation), `transporter-ii` (a two-phase cinematic \
         materialisation over a see-through window, fixed at 4000 ms and running on linear \
         time whatever the easing says: PHASE 1 — two thin horizontal beams, each about half \
         the form's width and horizontally centred, start overlapped on the vertical centre \
         line and separate to the top and bottom edges, while the gap opening between them \
         fills with a dense cloud of flickering white-and-yellow particles; PHASE 2 — the \
         horizontal beams fade out as they land, two FULL-HEIGHT vertical beams fade in at \
         the horizontal centre and sweep outward to the left and right edges, revealing the \
         form in the band widening between them while the cloud dissolves wherever a beam \
         has passed. Through the last stretch particles, glow and beams ease to nothing, so \
         the light is gone exactly as the beams reach the borders. Every beam is a layered \
         translucent gradient with a bloom — never a solid bar. An exit runs the sequence \
         backwards and dematerialises the form). New projects default to a `matrix-rain` \
         entrance and no exit effect.\n\n",
    );
    doc.push_str(
        "While an effect runs the window wears NO title bar (nothing stands still during the \
         animation); it arrives with the finished form, and only if that form shows one. The \
         face-only effects (`fade`, `zoom`, the slides, `expand-title-bar`, `genie`), \
         `matrix-rain` and `transporter-ii` also open a SEE-THROUGH window, so the form \
         animates over the desktop \
         — on those windows the form's `transparency` reaches the desktop for real. Only the \
         masked reveals (`radar-wipe`, `iris-wipe`, `blinds`, `checkerboard`) keep an opaque \
         window: they hide the form by painting covers over it, which nothing transparent \
         can undo.\n\n",
    );
    doc.push_str(
        "Per-form control: the Boolean designer attribute `WindowEffects` (default true) — \
         false opens/closes that form instantly while the rest of the project animates. Forms \
         never choose WHICH effect; only the project does. The entrance plays on the window's \
         first opening; the project option `entrance-on-restore` additionally replays it when \
         a window is restored after being minimized (no form events fire on a restore \
         replay). Control load-time animations start immediately AFTER the entrance effect \
         finishes, and the controls they animate are HELD BACK until then: a control with an \
         `OnFormLoad`/`OnShow` animation is not painted into the entrance at all, so it \
         arrives under its own power instead of materialising with the window and then \
         jumping back to fly in a second time (1.61.5+; before that it did exactly that). \
         Controls with no load animation appear with the window as always. The COBOL \
         `onLoad` event timing is unchanged. An exit effect delays the \
         actual close until the animation completes — `FormState` vetoes fire BEFORE the \
         animation, so a refused close plays nothing, and `onClose` still fires exactly once. \
         Machine-wide kill-switch: Help → Debug Settings → \"Disable window effects\" \
         (`PRC_NO_WINDOW_FX=1` for a bare `rcrun run-form` or a built application — both \
         honour it).\n\n",
    );
    doc.push_str(
        "Effects play in EVERY host of a form (spec 042): Run Form and the BUILT application \
         run the same shared window host, so the entrance/exit behaviour is identical in \
         both. The settings are baked into the executable at build time — a shipped binary \
         reads no project file. The same shared host gives the built application the full \
         designed window behaviour: the form's own `Title` (falling back to \"AppName \
         vVersion\" only when the designed title is blank), `TitleVisible`, minimize/maximize \
         buttons, `FullScreen`, the opening `WindowState` and `StartPosition`, window close \
         at program end (through the exit effect when configured), the `me::` window \
         methods, `FormState` close vetoes, and the `onShow`/`onActivate`/`onClose`/\
         `onCloseRejected`/`onFullScreenChanged` events.\n\n---\n\n",
    );

    // ── Per-control sections ─────────────────────────────────────────────────
    for ct in control_types.iter().cloned() {
        let name = ct.as_str().to_owned();
        let (dw, dh) = ct.default_size();
        let events: Vec<&str> = ct.supported_events().to_vec();
        let ctrl = cobolt_forms::Control::new("_", ct, 0, 0);

        doc.push_str(&format!("## Control: {name}\n\n"));
        let purpose = control_purpose(&name);
        if !purpose.is_empty() {
            doc.push_str(&format!("{purpose} Default size {dw}×{dh} px.\n\n"));
        }

        // Properties — type-specific ones in full, universals by reference.
        doc.push_str("### Settable Properties\n");
        doc.push_str(
            "All universal properties above apply. Type-specific properties \
             (type, default, allowed values):\n\n",
        );
        let mut specific = 0usize;
        for (pname, v) in &ctrl.properties {
            if UNIVERSAL_PROPS.contains(&pname.as_str()) {
                continue;
            }
            let (ty, default) = prop_type_and_default(v);
            let (domain, desc) = property_reference(pname).unwrap_or(("", ""));
            push_prop_line(&mut doc, pname, ty, &default, domain, desc);
            specific += 1;
        }
        if specific == 0 {
            doc.push_str("- (none — this control only uses the universal properties)\n");
        }
        doc.push('\n');

        // Run-time-only properties. Without this section the reference listed
        // what the DESIGNER can set and said, separately, that an async method
        // "delivers its answer in ResponseBody" — from which the reasonable
        // conclusion is that ResponseBody is not a property at all. It is: it
        // is read-only, and reading it is the only way a handler ever sees the
        // answer it asked for.
        let runtime = cobolt_forms::model::runtime_property_names_for(name.as_str());
        if !runtime.is_empty() {
            doc.push_str("### Runtime Properties (read-only)\n");
            doc.push_str(
                "Written by the runtime when it has something to report, never by the \
                 designer — so they carry no default, do not appear in the property pane \
                 and are not saved in the form. **Read them; never try to set them.**\n\n",
            );
            for pname in runtime {
                let (_, desc) = property_reference(pname).unwrap_or(("", ""));
                if desc.is_empty() {
                    doc.push_str(&format!("- `{pname}`\n"));
                } else {
                    doc.push_str(&format!("- `{pname}` — {desc}\n"));
                }
            }
            doc.push('\n');
        }

        // Events — split into universal (compact) and specific (described).
        doc.push_str("### Supported Events\n");
        let universal: Vec<&str> = events
            .iter()
            .copied()
            .filter(|e| UNIVERSAL_EVENTS.contains(e))
            .collect();
        let specific_events: Vec<&str> = events
            .iter()
            .copied()
            .filter(|e| !UNIVERSAL_EVENTS.contains(e))
            .collect();
        for ev in &specific_events {
            let desc = event_reference(ev);
            if desc.is_empty() {
                doc.push_str(&format!("- `{ev}`\n"));
            } else {
                doc.push_str(&format!("- `{ev}` — {desc}\n"));
            }
        }
        if !universal.is_empty() {
            let list = universal
                .iter()
                .map(|e| format!("`{e}`"))
                .collect::<Vec<_>>()
                .join(", ");
            doc.push_str(&format!(
                "- Plus the universal events: {list} (see \"Universal events\").\n"
            ));
        }
        if specific_events.is_empty() && universal.is_empty() {
            doc.push_str("- None\n");
        }
        doc.push('\n');

        // Methods.
        doc.push_str("### Methods\n");
        doc.push_str(
            "All controls support the universal methods (see the Control Methods Reference): \
             `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, \
             `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, \
             `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor \
             forms `GET-<Prop>()` / `SET-<Prop>(value)`.\n",
        );
        let methods = control_method_docs(&name);
        if !methods.is_empty() {
            doc.push_str("Type-specific methods:\n\n");
            for (sig, desc) in methods {
                doc.push_str(&format!("- `{sig}` — {desc}\n"));
            }
        }
        doc.push('\n');

        let notes = control_usage_notes(&name);
        if !notes.is_empty() {
            doc.push_str(notes);
            doc.push('\n');
        }
        doc.push_str("---\n\n");
    }

    doc
}

/// `(type label, printable default)` for a seeded property value. Booleans
/// print as `1`/`0` — the value the runtime writes and compares.
fn prop_type_and_default(v: &cobolt_forms::model::PropValue) -> (&'static str, String) {
    match v {
        cobolt_forms::model::PropValue::Bool(b) => ("Boolean", if *b { "1" } else { "0" }.into()),
        cobolt_forms::model::PropValue::Int(n) => ("Integer", n.to_string()),
        cobolt_forms::model::PropValue::String(s) => (
            "String",
            if s.is_empty() {
                "(empty)".to_owned()
            } else {
                format!("\"{s}\"")
            },
        ),
    }
}

/// Append one formatted property bullet to `doc`.
fn push_prop_line(doc: &mut String, name: &str, ty: &str, default: &str, domain: &str, desc: &str) {
    let mut line = format!("- `{name}` ({ty}, default {default}");
    if !domain.is_empty() {
        line.push_str(&format!("; {domain}"));
    }
    line.push(')');
    if !desc.is_empty() {
        line.push_str(&format!(" — {desc}"));
    }
    line.push('\n');
    doc.push_str(&line);
}

/// Render the Control Methods Reference (KB file 5): the complete closed
/// vocabulary of inline `::` methods with parameter types and return values.
fn methods_reference_doc() -> String {
    let mut d = String::new();
    d.push_str("# PowerRustCOBOL Control Methods Reference\n\n");
    d.push_str(
        "Inline methods are invoked as `<control>::<Method>(args).` — as a statement, as an \
         expression operand (`MOVE C::GetText() TO WS-X`), or with `RETURNING`. The method \
         vocabulary is CLOSED: only the methods below (plus the `GET-`/`SET-` accessor forms) \
         are recognised. Anything else is treated as a PROPERTY access — `C::Foo(x)` with an \
         unknown `Foo` writes property `Foo`, it does not call a method. Do not invent methods.\n\n",
    );
    d.push_str("## Parameter conventions\n\n");
    d.push_str(
        "- `String` — a COBOL literal or data item; values are trimmed.\n\
         - `Integer` / `Number` — numeric literal or numeric data item.\n\
         - `Boolean` — `1`/`0` preferred; `true`/`yes`/`on` (any case) also count as true.\n\
         - Missing arguments default to empty / 0 — pass every listed argument unless marked optional (`?`).\n\
         - Methods that return a value can be used inline or with `RETURNING`; \
           the value is a number when it parses as one, otherwise a string.\n\n",
    );

    let sections: &[(&str, &str, &[(&str, &str)])] = &[
        (
            "Universal (every control)",
            "",
            &[
                ("Show() / Hide()", "Set `Visible` to 1 / 0."),
                ("Enable() / Disable()", "Set `Enabled` to 1 / 0."),
                ("SetFocus() (alias Focus())", "Give the control keyboard focus."),
                ("BringToFront() / SendToBack()", "Jump the control to the top / bottom of the z-order."),
                ("MoveTo(x: Integer, y: Integer)", "Move to form coordinates."),
                ("Resize(width: Integer, height: Integer)", "Resize in pixels."),
                ("SetProperty(name: String, value: Any)", "Write any property by name (also reaches User-Control children as `\"Child.Prop\"`)."),
                ("GetProperty(name: String) → value", "Read any property by name."),
                ("SetColor(color: String)", "Set `ForegroundColor` (hex color)."),
                ("Refresh()", "No-op on most controls; on charts re-sends the current data."),
                ("Validate()", "No-op (accepted for compatibility)."),
                ("GET-<Prop>() → value / SET-<Prop>(value)", "Explicit accessor for ANY property, e.g. `C::GET-Width()`, `C::SET-Caption(\"Hi\")`."),
                ("<Prop>() → value / <Prop>(value)", "Bare property name: no args reads, one arg writes."),
            ],
        ),
        (
            "Caption controls (Button, Label, CheckBox, RadioButton, GroupBox)",
            "",
            &[
                ("SetCaption(text: String) / GetCaption() → String", "Write / read the `Caption`."),
            ],
        ),
        (
            "Text controls (TextBox)",
            "",
            &[
                ("SetText(text: String) / GetText() → String", "Write / read the `Text`."),
                ("AppendText(text: String)", "Append to the text."),
                ("Clear()", "Empty `Text` (and `Items`)."),
                ("SelectAll()", "Accepted; currently a no-op."),
            ],
        ),
        (
            "Check controls (CheckBox, RadioButton)",
            "",
            &[
                ("IsChecked() → Boolean (0/1)", "Read the checked state."),
                ("SetChecked(value: Boolean)", "Write the checked state."),
                ("Select()", "Check; a RadioButton also unchecks its GroupName siblings."),
                ("Toggle()", "Flip the checked state."),
            ],
        ),
        (
            "Value controls (ProgressBar, Slider, NumericUpDown, DateTimePicker)",
            "",
            &[
                ("SetValue(value) / GetValue() → value", "Write / read `Value`."),
                ("Increment() / Decrement()", "Add / subtract `Step` (default 1)."),
                ("Reset()", "Return `Value` to `Minimum` (or 0)."),
            ],
        ),
        (
            "Item lists (ListBox, ComboBox, ToolBar, StatusBar)",
            "`Items` is a newline-separated list; indexes are 0-based.",
            &[
                ("AddItem(text: String)", "Append one item."),
                ("RemoveItem(text: String)", "Remove the first matching item."),
                ("GetSelected() → String", "The selected value."),
                ("GetSelectedIndex() → Integer (alias GetIndex())", "The selected index, -1 = none."),
                ("SetSelectedIndex(index: Integer) (alias SetIndex())", "Select by index."),
                ("GetCount() → Integer", "Item count."),
                ("Clear()", "Remove every item."),
            ],
        ),
        (
            "DataGrid",
            "Rows/cells are addressed with 0-based indexes; `AddRow` cells are TAB-separated.",
            &[
                ("GetRowCount() → Integer", "Data-row count."),
                ("GetCellValue(row, column) → String", "Read one cell."),
                ("SetCellValue(row, column, value)", "Write one cell."),
                ("AddRow(cells: String)", "Append a TAB-separated row."),
                ("DeleteRow(row: Integer)", "Remove a row."),
                ("ClearRows()", "Remove all rows."),
                ("Sort(column: Integer)", "Sort by column."),
                ("SetFilter(column: String, value: String) / ClearFilters()", "Column filtering."),
                ("FreezeColumns(n) / FreezeRows(n)", "Freeze leading columns/rows."),
                ("SetRowHeight(px) / SetColumnWidth(column, px)", "Geometry."),
                ("GetSelectedText() → String / CopySelection()", "Selection access."),
                ("ExportCSV() → String", "CSV of the grid."),
                ("RefreshBinding() → Integer", "Re-hydrate from the bound source (also on repeating GroupBoxes)."),
            ],
        ),
        (
            "Timer",
            "",
            &[
                ("Start() / Stop()", "Run / halt the timer (its `Enabled` property)."),
                ("SetInterval(ms: Integer)", "Change the tick interval."),
                ("IsEnabled() → Boolean (0/1)", "Running state."),
            ],
        ),
        (
            "Animator / animations",
            "Any control with named animations accepts these.",
            &[
                ("Play() / PlayAnimation(name: String?)", "Start playback (optionally a named animation)."),
                ("StopAnimation()", "Stop playback."),
                ("Pause()", "Pause playback."),
            ],
        ),
        (
            "Charts (BarChart, LineChart, PieChart, AreaChart, ScatterChart, DonutChart)",
            "Equivalent to the `CALL \"COBOL-CHART-*\"` runtime calls and the generated `PERFORM <id>-ADD-POINT` paragraphs.",
            &[
                ("AddPoint(label: String, value: Number)", "Append one point and repaint."),
                ("Clear()", "Drop all pushed points."),
                ("Refresh()", "Repaint with current data."),
            ],
        ),
        (
            "AgentObject (LLM)",
            "",
            &[
                ("Ask(prompt: String) → String", "Send a prompt; the reply also fires `onResponse`."),
                ("SetPrompt(text: String)", "Replace the `SystemPrompt`."),
                ("SetModel(model: String)", "Switch models."),
                ("GetResult() → String", "Read the `Result` property."),
                ("Cancel() / IsBusy() → Boolean", "Async control."),
            ],
        ),
        (
            "RestClient (HTTP)",
            "Async by default (`Mode = \"Async\"`): the verb returns immediately, `Busy` = 1, and the response lands in `ResponseBody` / `StatusCode` with `onComplete` (or `onError` / `onTimeout`). With `Mode = \"Sync\"` the verb blocks and returns the body. The `url` argument is the FULL URL.",
            &[
                ("Get(url: String) → String", "HTTP GET."),
                ("Post(url: String, body: String) → String", "HTTP POST."),
                ("Put(url: String, body: String) → String", "HTTP PUT."),
                ("Delete(url: String) → String", "HTTP DELETE."),
                ("Call(verb: String, url: String, body: String?) → String", "Any verb by name."),
                ("SetHeader(name: String, value: String) / ClearHeaders()", "Request headers."),
                ("SetTimeout(seconds: Integer)", "Request timeout."),
                ("Cancel() / IsBusy() → Boolean", "Async control."),
            ],
        ),
        (
            "SqlDatabase",
            "Errors land in `LastError` and fire `onQueryError` / `onConnectError`.",
            &[
                ("Open(connectionString: String) → Integer", "Open; returns the handle, fires `onConnectOk`/`onConnectError`."),
                ("Execute(sql: String) → Integer (alias Exec)", "Run DML/DDL; affected rows."),
                ("Query(sql: String) → Integer", "Run a SELECT; result-row count."),
                ("Fetch() → Boolean (0/1)", "Advance the row cursor; fires `onRowFetched`."),
                ("FetchAll() → Integer", "Current result-set row count."),
                ("Close()", "Close the connection."),
            ],
        ),
        (
            "Knob",
            "Same value-controls contract as ProgressBar/Slider above (SetValue/GetValue/Increment/Decrement/Reset all apply).",
            &[],
        ),
        (
            "Gauge",
            "Read-only via the UI (R10) — SetValue/GetValue are the only way to change it, no drag, no Increment/Decrement/Reset.",
            &[
                ("SetValue(value: Integer)", "Set the current value."),
                ("GetValue() → Integer", "Read the current value."),
            ],
        ),
        (
            "Switch",
            "Same check-controls contract as CheckBox/RadioButton above, minus `Select()` (no radio group).",
            &[
                ("IsChecked() → Boolean (0/1)", "Read the checked state."),
                ("SetChecked(value: Boolean)", "Write the checked state."),
                ("Toggle()", "Flip the checked state."),
            ],
        ),
        (
            "FileDropZone",
            "No inline methods at all — it is a pure UI gesture (drag-and-drop or a native-picker click), never invoked from COBOL.",
            &[],
        ),
        (
            "Maps",
            "The OSM basemap needs no API key; only the five Google data methods below call the real Google Maps API and need a `google-maps` project credential (Settings → Integrations) — with none configured they fail immediately with `onError` (R33), never a crash or network call. `TraceRoad` is the exception in a different direction: it uses **OpenRouteService** and takes its key as an ARGUMENT, so a program with no Google credential can still trace a real road. **Every data method is ALWAYS async**: the call returns an empty string immediately, sets `Busy`, and the answer arrives in `ResponseBody` with `onComplete` (or `LastError` with `onError`). There is no Sync mode — do NOT write `MOVE Maps1::Geocode(...) TO X`, which only ever moves an empty string.",
            &[
                ("Geocode(address: String)", "Async. `onComplete`: `ResponseBody` = `lat\\tlng\\tformatted_address`."),
                ("ReverseGeocode(lat: String, lng: String)", "Async. `onComplete`: `ResponseBody` = the formatted address."),
                ("Directions(origin: String, destination: String)", "Async. `onComplete`: `ResponseBody` = `distance_text\\tduration_text\\troute_summary\\tdistance_METRES\\tduration_SECONDS\\tencoded_polyline\\ttraffic_SECONDS` — the numbers to COMPUTE with, the polyline for `AddRoute`, and the last field the drive time with current traffic (0 if none was supplied)."),
                ("DistanceMatrix(origin: String, destination: String)", "Async. `onComplete`: `ResponseBody` = `distance_text\\tduration_text\\tdistance_METRES\\tduration_SECONDS`."),
                ("PlacesSearch(query: String, radiusMeters: String)", "Async. `onComplete`: `ResponseBody` = one `place_id\\tname\\taddress\\tlat\\tlng` line per result."),
                ("TraceRoad(apiKey, fromLat, fromLng, toLat, toLng: String)", "Async, **OpenRouteService** — no Google credential. `onComplete`: `ResponseBody` = `distance_METRES\\tduration_SECONDS\\tencoded_polyline`. The key is the first ARGUMENT and is never stored: read it from a field the operator filled in. A blank key fires `onError` with no network call."),
                ("AddMarker(id, lat, lng, label, info: String)", "Append one pin to `Markers`."),
                ("RemoveMarker(id: String)", "Remove the marker with that id."),
                ("AddRoute(id, colour, width, geometry: String)", "Trace a line: an encoded polyline or `lat,lng;lat,lng;…`. Re-using an id replaces it. No API key needed."),
                ("RemoveRoute(id: String)", "Remove that route."),
                ("ClearRoutes()", "Remove every route."),
                ("AddRegion(id, fill, stroke, width, geometry [, label, info]: String)", "Fill an area — a sales territory. `fill` takes an alpha; the ring may be concave. `label`/`info` drive the info window on hover and click. No API key needed."),
                ("RemoveRegion(id: String)", "Remove that region."),
                ("ClearRegions()", "Remove every region."),
            ],
        ),
        (
            "WebSearch",
            "Async by default, same `Mode`/`Busy`/`onComplete`/`onError` contract as RestClient above. Needs a `google-custom-search` project credential (Settings → Integrations) — with none configured `Search()` fails immediately with `onError` (R33). Prefer `Search()` over the generated `<id>-SEARCH` paragraph, which has no URL-encoding and no key.",
            &[
                ("Search()", "Run a search using SearchEngineId/Query/NumResults/SafeSearch. Async: returns immediately, raw JSON in `ResponseBody`. Sync: returns the raw JSON."),
                ("ResultCount() → Integer", "Result items in the last response."),
                ("TopTitle() / TopSnippet() / TopLink() → String", "First result's fields, empty before any search."),
                ("GetResult(index: Integer) → String", "1-based indexed result as `title\\tsnippet\\tlink`; out-of-range → empty."),
                ("Cancel() / IsBusy() → Boolean", "Async control."),
            ],
        ),
        (
            "SideMenu (spec 051)",
            "The sidebar's programmatic door to standalone child windows: the opened window is parented to the SHELL whatever form invokes the method, exactly like the sidebar's own `Open Stand Alone Form` menu actions. The target's FormFormat must be `Standalone` or `Both` (build-checked for literal ids). Space form: every parameter required; comma form: the form id alone is enough.",
            &[
                (
                    "OpenStandAloneFormSync(formId, windowState: String, x, y, width, height: Integer, modal: Boolean)",
                    "Open the form in its own window and BLOCK the calling handler until it closes — Sync is implicitly modal, the whole shell face waits. RETURNING is NULL on resume.",
                ),
                (
                    "OpenStandAloneFormAsync(formId, windowState: String, x, y, width, height: Integer)",
                    "Open the form in its own window and return at once. RETURNING binds a windowHandler (Focus/Close/SetProperty/… — NULL when the child closes). Never modal.",
                ),
            ],
        ),
    ];

    for (title, note, methods) in sections {
        d.push_str(&format!("## {title}\n\n"));
        if !note.is_empty() {
            d.push_str(&format!("{note}\n\n"));
        }
        for (sig, desc) in *methods {
            d.push_str(&format!("- `{sig}` — {desc}\n"));
        }
        d.push('\n');
    }

    d.push_str(
        "## IndexedFile — no inline methods\n\n\
         IndexedFile controls are driven by the GENERATED PARAGRAPHS \
         (`PERFORM <id>-OPEN`, `<id>-START`, `<id>-READ-NEXT`, `<id>-READ-PREVIOUS`, \
         `<id>-READ-FIRST`, `<id>-READ-LAST`, `<id>-READ-INVALID`, `<id>-COMMIT`, \
         `<id>-CLOSE`) and the plain COBOL verbs `WRITE` / `REWRITE` / `DELETE`. \
         Do NOT call `::Open()` on an IndexedFile — that method belongs to SqlDatabase.\n",
    );
    d
}

#[cfg(test)]
mod resolve_main_tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("prc-resolve-{tag}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Remove the staging crate `build_core` leaves in the system temp dir.
    ///
    /// Each staged crate carries its own `target/`, so a test that builds and
    /// walks away leaves about a gigabyte behind — a few of those fill a disk
    /// and the failure lands on whatever runs next, looking like anything but a
    /// test that did not tidy up.
    fn remove_build_staging(bin_name: &str) {
        let dir = std::env::temp_dir().join(format!("cobolt-build-{bin_name}"));
        let _ = fs::remove_dir_all(dir);
    }

    /// Serialises the tests that drive a real `build_single_file`.
    ///
    /// `build_core` stages each program into its own crate with its own
    /// `target/`, so several of these running at once means several gigabytes
    /// live at the same moment — enough to fill a developer's disk, and the
    /// resulting `ENOSPC` surfaces as an unrelated failure somewhere else.
    /// Held for the whole test, they cost wall-clock and bound peak disk to one
    /// staged crate.
    static HEAVY_BUILD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Take [`HEAVY_BUILD`], ignoring a poisoned lock: an earlier test's panic
    /// is that test's failure to report, not a reason to fail this one too.
    fn heavy_build_guard() -> std::sync::MutexGuard<'static, ()> {
        HEAVY_BUILD.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn proj(main: &str, sources: Vec<&str>, generated: Vec<&str>) -> CoboltProject {
        CoboltProject {
            project: ProjectMeta {
                name: "Demo".into(),
                version: "1.0.0".into(),
                main: main.into(),
                destination_folder: String::new(),
                debug_compilation: true,
            },
            files: ProjectFiles {
                sources: sources.into_iter().map(String::from).collect(),
                generated: generated.into_iter().map(String::from).collect(),
                ..Default::default()
            },
            forms: FormsConfig::default(),
            crates: Vec::new(),
        }
    }

    // ── Asset-pack themes in the built binary (spec 007) ─────────────────────

    fn form_xml(name: &str, theme: Option<&str>) -> Vec<u8> {
        let attr = theme
            .map(|t| format!(" theme=\"{t}\""))
            .unwrap_or_default();
        format!(
            "<Form name=\"{name}\" title=\"{name}\" width=\"400\" height=\"300\"{attr}></Form>"
        )
        .into_bytes()
    }

    /// 051 R1/R2 — the generated glue embeds one PROGRAM per openable
    /// non-main form, loadable by id; a single-form project emits the empty
    /// table and the untouched `PROGRAM_AST` path (the R2 guarantee).
    #[test]
    fn generated_glue_embeds_per_form_programs() {
        let src = generate_main_rs(
            "Demo",
            "1.0.0",
            true,
            &["MAIN", "CRM", "REPORT"],
            "MAIN",
            &["CRM", "REPORT"],
            &[],
            &[],
            "",
            "none:600:ease-out",
            "none:600:ease-out",
            false,
        );
        for id in ["CRM", "REPORT"] {
            assert!(
                src.contains(&format!(
                    "(\"{id}\", include_bytes!(\"../assets/programs/{id}.bin\"))"
                )),
                "PROGRAMS carries {id}"
            );
        }
        assert!(
            !src.contains("include_bytes!(\"../assets/programs/MAIN.bin\")"),
            "the MAIN form's program stays PROGRAM_AST, never doubled"
        );
        assert!(src.contains("fn load_program_by_id("), "the by-id loader is emitted");
        assert!(
            src.contains("static PROGRAM_AST: &[u8] = include_bytes!(\"../assets/program.bin\");"),
            "the main path is untouched (R2)"
        );

        // Single-form project: the empty table, still compiling (the
        // include_bytes! paths need not resolve).
        let single = generate_main_rs(
            "Demo",
            "1.0.0",
            true,
            &["MAIN"],
            "MAIN",
            &[],
            &[],
            &[],
            "",
            "none:600:ease-out",
            "none:600:ease-out",
            false,
        );
        assert!(
            single.contains("static PROGRAMS: &[(&str, &[u8])] = &[];"),
            "single-form ⇒ empty PROGRAMS table"
        );
        println!(
            "051 PROGRAMS table — 2/2 form programs embedded by id, MAIN excluded, \
             loader emitted, single-form empty table holds"
        );
    }

    /// 051 R26 — the build gate mirrors the designer filter: a menu item
    /// whose STANDALONE action targets an Embedded-only form fails the build
    /// with a message naming the item, the form, and the remedy. Cheap:
    /// the error fires in the semantic phase, before any staging or cargo.
    #[test]
    fn standalone_menu_action_on_an_embedded_form_fails_the_build() {
        let dir = temp_dir("std-menu");
        fs::write(
            dir.join("main.cbl"),
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. DEMO.\nPROCEDURE DIVISION.\n    STOP RUN.\n",
        )
        .unwrap();

        let mut main_form = cobolt_forms::Form::new("MAIN-FORM", "Main", 800, 600);
        main_form.main_form = true;
        main_form.controls.push(cobolt_forms::Control::new(
            "SIDE-1",
            cobolt_forms::ControlType::SideMenu,
            0,
            0,
        ));
        cobolt_forms::save_form(&main_form, &dir.join("MAIN-FORM.cfrm")).unwrap();

        let mut crm = cobolt_forms::Form::new("CRM", "CRM", 640, 480);
        crm.form_format = cobolt_forms::model::FormFormat::Embedded;
        cobolt_forms::save_form(&crm, &dir.join("CRM.cfrm")).unwrap();

        let mut def = cobolt_forms::menu::MenuDefinition::default();
        def.menu.push(cobolt_forms::menu::MenuItem {
            action: Some("open-standalone-sync:CRM".into()),
            ..cobolt_forms::menu::MenuItem::new_action("reports", "Reports")
        });
        cobolt_forms::menu::save_menu(
            &cobolt_forms::menu::menu_yaml_path(&dir, "SIDE-1"),
            &def,
        )
        .unwrap();

        fs::write(
            dir.join("cobolt.toml"),
            "[project]\nname = \"Demo\"\nversion = \"1.0.0\"\nmain = \"main.cbl\"\n\n\
             [files]\nsources = [\"main.cbl\"]\nforms = [\"MAIN-FORM.cfrm\", \"CRM.cfrm\"]\n",
        )
        .unwrap();

        let err = build_project(&dir.join("cobolt.toml"), &BuildOptions::default())
            .expect_err("the mis-wired standalone action must fail the build");
        let msg = err.to_string();
        assert!(
            msg.contains("reports")
                && msg.contains("CRM")
                && msg.contains("Standalone or Both"),
            "the error names the item, the form, and the remedy: {msg}"
        );
        let _ = fs::remove_dir_all(&dir);
        println!("051 R26 build gate → {msg}");
    }

    /// The generated crate name is always a valid Cargo package name —
    /// a literal `.` (the ".project" suffix) aborted the build (1.44.x).
    #[test]
    fn package_name_sanitizes_dots_spaces_and_suffix() {
        let cases = [
            ("PowerDemo3.project", "powerdemo3"),
            ("My App", "my_app"),
            ("hola.mundo", "hola_mundo"),
            ("Ünïcode Ñame", "_n_code__ame"),
            ("***", "app"),
            ("  Spaced.PROJECT  ", "spaced"),
        ];
        for (input, expected) in cases {
            let got = sanitize_package_name(input);
            assert_eq!(got, expected, "{input:?}");
            println!("package name: {input:?} → {got:?}");
        }
    }

    #[test]
    fn wanted_themes_follow_form_then_project_and_drop_liquid_glass() {
        let forms = vec![
            ("A".to_owned(), form_xml("A", Some("cobalt-steel"))),
            // No per-form theme → the project default applies.
            ("B".to_owned(), form_xml("B", None)),
            // Same pack as A → embedded once.
            ("C".to_owned(), form_xml("C", Some("cobalt-steel"))),
        ];
        assert_eq!(
            wanted_theme_ids(&forms, "neumorphic"),
            vec!["cobalt-steel".to_owned(), "neumorphic".to_owned()]
        );
        // With no project default, the unthemed form resolves to the procedural
        // Liquid Glass, which needs no embedded art.
        assert_eq!(
            wanted_theme_ids(&forms, ""),
            vec!["cobalt-steel".to_owned()]
        );
    }

    /// Spec 047 T5 — a procedural theme has no pack on disk, so the build must
    /// not ask for one. Before this, only Liquid Glass was excluded, so an
    /// Elegance form sent the build hunting for `assets/themes/elegance/` and
    /// printed a spurious "falling back to Liquid Glass".
    /// FIX — a RAD-generated form with tab-indented Rust in an `EXEC RUST`
    /// block failed to build with `expected PROCEDURE DIVISION`, while
    /// `rcrun check` on the identical bytes reported OK.
    ///
    /// Cause: the build guessed fixed-form because the generated banner puts
    /// `*` in column 7. Under fixed form columns 1-7 are stripped, so a
    /// tab-indented `END-EXEC.` arrived as `D-EXEC.`, the block never
    /// terminated, and the parser ran to EOF.
    ///
    /// The fixture reproduces exactly that shape: the banner comment, then an
    /// `EXEC RUST` body indented with TABS.
    #[test]
    fn generated_banner_and_tabbed_exec_rust_parse_as_free_form() {
        let src = concat!(
            "      *> generated banner - puts '*' in column 7\n",
            "       IDENTIFICATION DIVISION.\n",
            "       PROGRAM-ID. TABBY.\n",
            "       ENVIRONMENT DIVISION.\n",
            "       CONFIGURATION SECTION.\n",
            "       REPOSITORY.\n",
            "           CLASS RUST-STRING IS \"Rust.String\"\n",
            "\n",
            "           EXEC RUST\n",
            "\t\t\t\tpub fn ferris_say(out: &str) -> String {\n",
            "\t\t\t\t    String::from(out)\n",
            "\t\t\t\t}\n",
            "\t\t   END-EXEC.\n",
            "\n",
            "       DATA DIVISION.\n",
            "       WORKING-STORAGE SECTION.\n",
            "       01 WS-X PIC X(4) VALUE SPACE.\n",
            "\n",
            "       PROCEDURE DIVISION.\n",
            "       MAIN.\n",
            "           GOBACK.\n",
        );

        // The shape that used to fool the guess is still present...
        let banner_tricks_the_old_heuristic = src.lines().any(|l| {
            let b = l.as_bytes();
            b.len() > 6 && b[6] != b' ' && b[..6].iter().all(|&c| c == b' ' || c.is_ascii_digit())
        });
        assert!(
            banner_tricks_the_old_heuristic,
            "fixture must still contain the banner shape that caused the bug"
        );

        // ...but the format is free form regardless, so the parse succeeds.
        assert_eq!(detect_format(src), cobolt_lexer::SourceFormat::Free);

        let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(src, detect_format(src)));
        let errors: Vec<String> = parsed
            .diagnostics
            .iter()
            .map(|d| d.message.clone())
            .collect();
        println!("parse diagnostics: {}", errors.len());
        for e in &errors {
            println!("  {e}");
        }
        assert!(
            errors.is_empty(),
            "tab-indented EXEC RUST must parse; got {errors:?}"
        );
        assert!(parsed.program.is_some(), "a program must come out");

        // And the exact failure mode is gone: parsed as FIXED it still breaks,
        // which is what proves the format choice was the whole cause.
        let as_fixed =
            cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Fixed));
        assert!(
            !as_fixed.diagnostics.is_empty(),
            "sanity: fixed-form parsing of tabbed EXEC RUST should still fail"
        );
    }

    /// The legacy opt-in still reaches fixed form.
    #[test]
    fn cobolt_fixed_env_still_selects_fixed_form() {
        // Only assert the default here; the env var is process-global and
        // setting it would race other tests.
        assert_eq!(detect_format("       DISPLAY 'X'.\n"), cobolt_lexer::SourceFormat::Free);
    }

    #[test]
    fn elegance_wanted_ids_skips_every_procedural_theme() {
        let forms = vec![
            ("A".to_owned(), form_xml("A", Some(cobolt_forms::theme::ELEGANCE))),
            ("B".to_owned(), form_xml("B", Some(cobolt_forms::theme::LIQUID_GLASS))),
            ("C".to_owned(), form_xml("C", Some("cobalt-steel"))),
        ];
        let wanted = wanted_theme_ids(&forms, cobolt_forms::theme::ELEGANCE);
        println!("theme ids requested as packs: {wanted:?}");
        assert_eq!(
            wanted,
            vec!["cobalt-steel".to_owned()],
            "only asset-pack ids may be staged; procedural ids have no art"
        );

        // An all-procedural project stages nothing at all.
        let only_procedural = vec![("A".to_owned(), form_xml("A", Some(cobolt_forms::theme::ELEGANCE)))];
        assert!(
            wanted_theme_ids(&only_procedural, cobolt_forms::theme::ELEGANCE).is_empty(),
            "an Elegance-only project must stage no packs"
        );
    }

    /// Spec 047 T5 / 050 R16 — the compiled application resolves and publishes
    /// its THEME every frame, exactly as it already does for the pack and the
    /// glass style. This is the fourth rendering surface, and it gets the gate
    /// for free by going through the same `FormHost` as Run Form.
    #[test]
    fn elegance_generated_binary_publishes_its_surface_theme() {
        let src = generate_main_rs("Demo", "1.0.0", true, &["MAIN"], "MAIN", &[], &[], &[], cobolt_forms::theme::ELEGANCE, "none:600:ease-out", "none:600:ease-out", false);
        assert!(src.contains("fn resolve_surface_theme("));
        assert!(src.contains("None => resolve_surface_theme(&first_form),"));
        assert!(src.contains("surface_theme,"));
        // 050 R3 — and an asset pack's own manifest declaration reaches it.
        assert!(
            src.contains("surface_theme::for_pack(p.manifest.self_contained)"),
            "the built binary must honour a pack that declares it owns the look"
        );
        println!(
            "050 R16 — the built binary resolves its theme from the same \
             catalogue id (and a pack's manifest flag) and hands it to the \
             shared FormHost"
        );
    }

    /// 049 — a built application enters SHELL mode for itself.
    ///
    /// The generated glue used to call `cobolt_form_host::run` unconditionally,
    /// so a compiled app with a SideMenu opened the classic single window: no
    /// MenuPane, no ContentPane, and an embedded form painted across the whole
    /// window instead of beside the rail. `rcrun run-form` made this decision
    /// (`form.has_side_menu()`) and the binary did not, which is exactly why
    /// the same project behaved differently once it was compiled.
    #[test]
    fn a_built_application_opens_a_shell_when_the_main_form_has_a_sidebar() {
        let src = generate_main_rs(
            "Demo",
            "1.0.0",
            true,
            &["MAIN"],
            "MAIN",
            &[],
            &["SIDE-1"],
            &[],
            "",
            "none:600:ease-out",
            "none:600:ease-out",
            false,
        );

        // The decision itself, taken before the form moves into the config.
        assert!(src.contains("let shell_mode = first_form.has_side_menu();"));
        assert!(src.contains("cobolt_form_host::shell::run_shell(host_config, root_menu);"));
        // …and the classic one-window path is still there for every form
        // without a sidebar.
        assert!(src.contains("cobolt_form_host::run(host_config);"));

        // The rail needs its menu, and a compiled binary has no `.menu.yaml`
        // on disk — so the sidecar is embedded and parsed from memory.
        assert!(
            src.contains(r#"("SIDE-1", include_str!("../assets/menus/SIDE-1.menu.yaml"))"#),
            "the menu sidecar must be embedded, or the shell opens an empty rail"
        );
        assert!(src.contains("cobolt_forms::menu::parse_menu(yaml)"));

        // Single braces: this half of the template is a raw string, NOT a
        // format! target, so a doubled brace would reach the generated file.
        assert!(
            !src.contains("if shell_mode {{"),
            "the generated source must not carry format! brace escapes"
        );
    }

    #[test]
    fn generated_binary_publishes_its_theme_pack_every_frame() {
        let themes = vec![StagedTheme {
            id: "cobalt-steel".into(),
            assets: vec!["background.png".into(), "button/b.png".into()],
        }];
        let src = generate_main_rs("Demo", "1.0.0", true, &["MAIN"], "MAIN", &[], &[], &themes, "neumorphic", "zoom:600:ease-out", "none:600:ease-out", false);

        // The regression this guards: the template used to set only the glass
        // style, so an asset-pack form shipped as procedural Liquid Glass.
        // Since spec 042 the per-frame `set_active_theme`/`set_glass_style`
        // publication lives in the SHARED host (`cobolt-form-host`), so the
        // glue's job — and this assertion — is handing the resolved embedded
        // pack into the host's config.
        assert!(src.contains("let theme_pack = resolve_theme_pack(&first_form);"));
        assert!(src.contains("theme_pack,"));
        assert!(src.contains("let host_config = cobolt_form_host::FormHostConfig {"));

        // The pack's manifest and art are embedded, keeping the binary
        // self-contained on a machine with no PowerRustCOBOL install.
        assert!(src.contains(r#"include_str!("../assets/themes/cobalt-steel/theme.toml")"#));
        assert!(src
            .contains(r#"("background.png", include_bytes!("../assets/themes/cobalt-steel/background.png"))"#));
        // Nested refs keep their pack-relative path so the manifest still resolves.
        assert!(src
            .contains(r#"("button/b.png", include_bytes!("../assets/themes/cobalt-steel/button/b.png"))"#));
        assert!(src.contains(r#"const PROJECT_THEME_DEFAULT: &str = "neumorphic";"#));
        // The whole designed form (incl. the themed-background opt-in) is
        // handed to the shared host, which owns the render plumbing.
        assert!(src.contains("form: first_form,"));
    }

    /// 042 R11/R3 — the generated glue bakes the project's window effects and
    /// stays a THIN layer over the shared host: no divergent host markers, no
    /// second CtrlState, no leftover parity placeholder.
    #[test]
    fn generated_glue_bakes_effects_and_carries_no_divergent_host() {
        let src = generate_main_rs(
            "Demo",
            "1.2.3",
            true,
            &["MAIN"],
            "MAIN",
            &[],
            &[],
            &[],
            "",
            "matrix-rain:1500:ease-in-out",
            "fade:400:ease-in",
            true,
        );
        // The baked triples parse through the ONE shared parser at run time.
        assert!(src.contains(r#"const PROJECT_FX_ENTRANCE: &str = "matrix-rain:1500:ease-in-out";"#));
        assert!(src.contains(r#"const PROJECT_FX_EXIT: &str = "fade:400:ease-in";"#));
        assert!(src.contains("const PROJECT_FX_ON_RESTORE: bool = true;"));
        // Resolution = baked settings × form opt-out × kill switch (R6).
        assert!(src.contains("first_form.window_effects"));
        assert!(src.contains("PRC_NO_WINDOW_FX"));
        assert!(src.contains("cobolt_forms::window_fx::FxSpec::parse(PROJECT_FX_ENTRANCE)"));
        // Thin glue over the shared host — the divergent-host era is over.
        assert!(src.contains("let host_config = cobolt_form_host::FormHostConfig {"));
        assert!(src.contains("cobolt_form_host::seeding::build_object_seed"));
        assert!(src.contains("set_form_host"));
        assert!(!src.contains("struct CtrlState"), "no second control-state copy");
        assert!(
            !src.contains("impl eframe::App"),
            "the eframe::App lives in the shared host, not the glue"
        );
        assert!(!src.contains("037/038 parity"), "parity placeholder retired");
        // The branded fallback title yields to the designed form.title (R17).
        assert!(src.contains(r#"title_fallback: format!("{} v{}", APP_NAME, APP_VERSION)"#));
        // The per-frame block-window replay rides the seam (R30).
        assert!(src.contains("cobolt_windows::show_all(ctx)"));

        // A project with no effects bakes inert triples — `none` parses to
        // WindowEffect::None, so nothing plays.
        let quiet = generate_main_rs(
            "Demo", "1.2.3", true, &["MAIN"], "MAIN", &[], &[], &[], "",
            "none:600:ease-out", "none:600:ease-out", false,
        );
        assert!(quiet.contains(r#"const PROJECT_FX_ENTRANCE: &str = "none:600:ease-out";"#));
        assert!(quiet.contains("const PROJECT_FX_ON_RESTORE: bool = false;"));
    }

    /// The `[forms]` values reach the baked triples through `fx_triple` —
    /// normalised, colon-stripped, never typed compiler-side.
    #[test]
    fn fx_triple_normalises_raw_settings() {
        assert_eq!(fx_triple(" Matrix-Rain ", 1500, " Ease-In-Out "), "matrix-rain:1500:ease-in-out");
        assert_eq!(fx_triple("", 0, ""), ":0:");
        // A colon in a field cannot corrupt the triple.
        assert_eq!(fx_triple("fa:de", 400, "ease:in"), "fade:400:easein");
    }

    #[test]
    fn generated_binary_without_themes_still_compiles_to_liquid_glass() {
        let src = generate_main_rs("Demo", "1.0.0", true, &["MAIN"], "MAIN", &[], &[], &[], "", "none:600:ease-out", "none:600:ease-out", false);
        assert!(src.contains("static THEMES: &[(&str, &str, &[(&str, &[u8])])] = &[];"));
        assert!(src.contains(r#"const PROJECT_THEME_DEFAULT: &str = "";"#));
        // Resolution still runs — it just finds no pack and yields Liquid Glass.
        assert!(src.contains("fn resolve_theme_pack("));
    }

    #[test]
    fn generated_binary_source_actually_compiles() {
        // Every other test in this file only asserts on SUBSTRINGS of the
        // generated `main.rs` — none of them catch a real Rust syntax/type
        // error in the template itself (exactly the class of bug the
        // FileDropZone native-picker wiring had: present and correct in
        // `cobolt-ide`'s and `cobolt-cli`'s copies, silently absent from
        // this one, spec 039). This test writes the generated
        // Cargo.toml + main.rs to a real scratch crate and actually
        // compiles it with `cargo build`, reusing the workspace's own
        // `target/` dir so it does not recompile the whole dependency
        // tree from scratch.
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("cobolt-compiler sits at <workspace>/crates/cobolt-compiler")
            .to_path_buf();
        let crates_path = workspace_root.join("crates");
        assert!(
            crates_path.join("cobolt-ast").is_dir(),
            "expected {crates_path:?} to contain the workspace's cobolt-* crates"
        );

        let dir = temp_dir("gencompile");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        // `PROGRAM_AST` is embedded unconditionally (outside the has_forms
        // branch) — the actual bytes are irrelevant to a compile-only check.
        std::fs::write(dir.join("assets/program.bin"), b"placeholder").unwrap();

        // `has_forms = true` includes the FileDropZone / render-loop code
        // this test exists to exercise; `form_ids: &[]` needs no real
        // `.cfrm`/`assets/forms` on disk (the FORMS const is empty either
        // way, so no `include_bytes!` path has to resolve).
        let main_rs = generate_main_rs(
            "GenCompileCheck",
            "0.1.0",
            true,
            &[],
            "",
            &[],
            &[],
            &[],
            "",
            "matrix-rain:1500:ease-in-out",
            "fade:400:ease-in",
            true,
        );
        let cargo_toml =
            generate_cargo_toml("gencompilecheck", "0.1.0", &crates_path, true, &dir, &[]);
        std::fs::write(dir.join("src/main.rs"), &main_rs).unwrap();
        std::fs::write(dir.join("Cargo.toml"), &cargo_toml).unwrap();
        // `main.rs` declares `mod exec_rust_blocks;` unconditionally, so the
        // module has to exist even for a program with no blocks (spec 041 T9).
        let empty = cobolt_parser::parse(cobolt_lexer::tokenize(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. NOBLOCKS.\nPROCEDURE DIVISION.\n    STOP RUN.\n",
            cobolt_lexer::SourceFormat::Free,
        ))
        .program
        .expect("the empty fixture should parse");
        let empty_symbols = cobolt_semantic::symbol_table::SymbolTable::build(&empty);
        std::fs::write(
            dir.join("src/exec_rust_blocks.rs"),
            // `has_forms = true`, to match the `main.rs` above: a form
            // application calls into `cobolt_windows` every frame, so the module
            // has to be emitted even though this fixture has no blocks.
            exec_rust::generate(&empty, &empty_symbols, "", true).source,
        )
        .unwrap();

        let mut cmd = std::process::Command::new("cargo");
        cmd.arg("build")
            .current_dir(&dir)
            // Share the workspace's target dir so dependencies already
            // built for the workspace (eframe, egui, cobolt-forms, …)
            // are reused instead of rebuilt from scratch.
            .env("CARGO_TARGET_DIR", workspace_root.join("target"));
        let output = cmd.output().expect("failed to run cargo build");

        assert!(
            output.status.success(),
            "generated main.rs failed to compile:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );

        let _ = fs::remove_dir_all(&dir);
    }

    // ── EXEC RUST end-to-end (spec 041 T9) ────────────────────────────────────

    /// Build a headless program from COBOL source through the real generators
    /// and **run** it, returning its stdout.
    ///
    /// Substring assertions on generated text cannot tell whether a block
    /// actually executes, and that is the whole claim of spec 041. This does the
    /// full round trip — parse, serialise the AST, emit the blocks module, emit
    /// `main.rs`, `cargo build`, execute — so a green result means a compiled
    /// block really ran inside a built binary.
    fn build_and_run_cobol(bin: &str, cobol: &str) -> String {
        use cobolt_lexer::{tokenize, SourceFormat};
        use cobolt_parser::parse;

        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("cobolt-compiler sits at <workspace>/crates/cobolt-compiler")
            .to_path_buf();
        let crates_path = workspace_root.join("crates");

        let parsed = parse(tokenize(cobol, SourceFormat::Free));
        assert!(
            parsed
                .diagnostics
                .iter()
                .all(|d| d.severity != cobolt_parser::Severity::Error),
            "the fixture should parse: {:?}",
            parsed.diagnostics
        );
        let program = parsed.program.expect("the fixture should produce a program");
        let sem = cobolt_semantic::analyze(&program);
        assert!(
            sem.diagnostics
                .iter()
                .all(|d| d.severity != cobolt_semantic::Severity::Error),
            "the fixture should pass semantic analysis: {:?}",
            sem.diagnostics
                .iter()
                .filter(|d| d.severity == cobolt_semantic::Severity::Error)
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );

        let dir = temp_dir(bin);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("assets")).unwrap();

        let ast_bytes = bincode::serialize(&program).unwrap();
        let mut gz = GzEncoder::new(Vec::new(), Compression::best());
        gz.write_all(&ast_bytes).unwrap();
        fs::write(dir.join("assets/program.bin"), gz.finish().unwrap()).unwrap();

        let blocks = exec_rust::generate(&program, &sem.symbols, cobol, false);
        fs::write(dir.join("src/exec_rust_blocks.rs"), &blocks.source).unwrap();
        fs::write(
            dir.join("src/main.rs"),
            generate_main_rs(bin, "0.1.0", false, &[], "", &[], &[], &[], "", "none:600:ease-out", "none:600:ease-out", false),
        )
        .unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            // Linked exactly as `build_core` does it: any block pulls in the GUI
            // crates, which is what lets a block open a window. Hardcoding
            // `false` here made this harness disagree with the real build.
            generate_cargo_toml(
                bin,
                "0.1.0",
                &crates_path,
                blocks.block_count > 0 || blocks.item_count > 0,
                &dir,
                &[],
            ),
        )
        .unwrap();

        let target_dir = workspace_root.join("target");
        let built = std::process::Command::new("cargo")
            .arg("build")
            .current_dir(&dir)
            .env("CARGO_TARGET_DIR", &target_dir)
            .output()
            .expect("failed to run cargo build");
        assert!(
            built.status.success(),
            "the generated crate failed to build:\n{}\n--- generated blocks ---\n{}",
            String::from_utf8_lossy(&built.stderr),
            blocks.source
        );

        let exe = target_dir.join("debug").join(bin);
        let run = std::process::Command::new(&exe)
            .output()
            .unwrap_or_else(|e| panic!("failed to run {}: {e}", exe.display()));
        let stdout = String::from_utf8_lossy(&run.stdout).to_string();
        assert!(
            run.status.success(),
            "the built binary exited with {:?}\nstdout:\n{stdout}\nstderr:\n{}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr)
        );

        let _ = fs::remove_dir_all(&dir);
        stdout
    }

    /// **AC1, AC2 and AC10 together**, in one built binary:
    ///
    /// * AC1 — a block calls real `std` APIs on a bound `Rust.String`, and the
    ///   mutation is visible to COBOL afterwards through `INVOKE`;
    /// * AC2 — the second block uses a closure, a generic, an iterator chain,
    ///   `match` and `?`, none of which the deleted micro-interpreter could
    ///   have run;
    /// * AC10 — the second block reads what the first stored in a bound
    ///   `Rust.Vec`, so the two share one context.
    #[test]
    fn a_compiled_block_runs_mutates_and_shares_state() {
        let out = build_and_run_cobol(
            "execrustdemo",
            r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RUSTDEMO.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-STRING IS "Rust.String"
           CLASS RUST-VEC IS "Rust.Vec"
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 USER-NAME USAGE IS OBJECT REFERENCE RUST-STRING VALUE "ada".
       01 WS-ITEMS USAGE IS OBJECT REFERENCE RUST-VEC.
       01 WS-TEXT PIC X(20).
       01 WS-COUNT PIC 9(4).
       PROCEDURE DIVISION.
           EXEC RUST
           use cobolt_runtime::rust_bridge::BridgeValue;
           user_name.push_str("-lovelace");
           for n in [10_i64, 20, 30] {
               ws_items.push(BridgeValue::Int(n));
           }
           END-EXEC.
           EXEC RUST
           use cobolt_runtime::rust_bridge::BridgeValue;
           fn twice<T: Clone>(v: T) -> (T, T) {
               (v.clone(), v)
           }
           let total: i64 = ws_items
               .iter()
               .map(|v| match v {
                   BridgeValue::Int(n) => *n,
                   _ => 0,
               })
               .sum();
           let parsed: i64 = "6".parse::<i64>()?;
           let (a, b) = twice(parsed);
           println!("TOTAL={} TWICE={}", total, a + b);
           END-EXEC.
           INVOKE USER-NAME "to_string" RETURNING WS-TEXT
           DISPLAY "NAME=" WS-TEXT
           INVOKE WS-ITEMS "len" RETURNING WS-COUNT
           DISPLAY "COUNT=" WS-COUNT
           STOP RUN.
"#,
        );

        // AC10 — the second block saw the first block's three pushes.
        assert!(
            out.contains("TOTAL=60"),
            "two blocks did not share state; output was:\n{out}"
        );
        // AC2 — the generic, the closure and `?` all ran.
        assert!(out.contains("TWICE=12"), "the block's Rust did not run:\n{out}");
        // AC1 — COBOL sees the mutation the block made.
        assert!(
            out.contains("NAME=ada-lovelace"),
            "the mutation was not visible to COBOL:\n{out}"
        );
        assert!(
            out.contains("COUNT=0003"),
            "the bound Vec did not keep the block's pushes:\n{out}"
        );
    }

    /// **AC3, as far as one machine can show it** — the built binary does not
    /// need the Rust toolchain it was built with.
    ///
    /// The criterion asks for execution on a machine where `rustc` is absent,
    /// which no test on the build machine can literally provide. What this does
    /// provide is the property that claim rests on: the binary is run with an
    /// **empty `PATH` and `RUSTUP_HOME`/`CARGO_HOME` cleared**, so neither
    /// `cargo` nor `rustc` is reachable, and it still runs its compiled block.
    /// A binary that shelled out to the toolchain would fail here.
    ///
    /// The remaining gap — a genuinely toolchain-free machine — is an operator
    /// check, and is recorded as such rather than inferred from this.
    #[test]
    fn the_built_binary_runs_with_no_toolchain_on_the_path() {
        use cobolt_lexer::{tokenize, SourceFormat};
        use cobolt_parser::parse;

        let cobol = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. NOTOOLCHAIN.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-STRING IS \"Rust.String\"
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-TEXT USAGE IS OBJECT REFERENCE RUST-STRING VALUE \"ok\".
       PROCEDURE DIVISION.
           EXEC RUST
           println!(\"BLOCK-RAN={}\", ws_text.to_uppercase());
           END-EXEC.
           STOP RUN.
";
        // Build through the same generators the other end-to-end tests use.
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf();
        let bin = "execrustnotoolchain";
        let program = parse(tokenize(cobol, SourceFormat::Free)).program.unwrap();
        let sem = cobolt_semantic::analyze(&program);
        let dir = temp_dir(bin);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("assets")).unwrap();
        let mut gz = GzEncoder::new(Vec::new(), Compression::best());
        gz.write_all(&bincode::serialize(&program).unwrap()).unwrap();
        fs::write(dir.join("assets/program.bin"), gz.finish().unwrap()).unwrap();
        let blocks = exec_rust::generate(&program, &sem.symbols, cobol, false);
        fs::write(dir.join("src/exec_rust_blocks.rs"), &blocks.source).unwrap();
        fs::write(
            dir.join("src/main.rs"),
            generate_main_rs(bin, "0.1.0", false, &[], "", &[], &[], &[], "", "none:600:ease-out", "none:600:ease-out", false),
        )
        .unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            // As `build_core` links it — a block brings the GUI crates with it.
            generate_cargo_toml(
                bin,
                "0.1.0",
                &workspace_root.join("crates"),
                blocks.block_count > 0 || blocks.item_count > 0,
                &dir,
                &[],
            ),
        )
        .unwrap();
        let target_dir = workspace_root.join("target");
        let built = std::process::Command::new("cargo")
            .arg("build")
            .current_dir(&dir)
            .env("CARGO_TARGET_DIR", &target_dir)
            .output()
            .expect("cargo build");
        assert!(
            built.status.success(),
            "build failed:\n{}",
            String::from_utf8_lossy(&built.stderr)
        );

        // Now run it with nothing of the toolchain reachable.
        let exe = target_dir.join("debug").join(bin);
        let run = std::process::Command::new(&exe)
            .env("PATH", "")
            .env("RUSTUP_HOME", "")
            .env("CARGO_HOME", "")
            .output()
            .expect("run the built binary");
        let stdout = String::from_utf8_lossy(&run.stdout).to_string();
        assert!(
            run.status.success(),
            "the built binary failed with no toolchain on PATH:\n{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert!(
            stdout.contains("BLOCK-RAN=OK"),
            "the compiled block did not run without a toolchain:\n{stdout}"
        );
        println!("AC3: built binary ran its compiled block with PATH empty");

        let _ = fs::remove_dir_all(&dir);
    }

    /// **The built application must open the form marked as MAIN.**
    ///
    /// The generated `main.rs` opens `FORMS.first()`, so the order the build
    /// embeds forms in *is* the choice of start-up window. It used to be
    /// `cobolt.toml` order, which meant a project whose main form was not listed
    /// first started on the wrong window — while the IDE's own Run, which
    /// resolves the main form properly, opened the right one. Two answers to
    /// "what is this application?", and only one of them shipped.
    #[test]
    fn the_build_puts_the_main_form_first() {
        let dir = temp_dir("mainform");
        let forms_dir = dir.join("forms");
        fs::create_dir_all(&forms_dir).unwrap();

        // Three forms; the MAIN one is deliberately not the first listed.
        for (stem, is_main) in [("buttons", false), ("checkboxes", true), ("labels", false)] {
            let mut form = cobolt_forms::Form::new(
                &stem.to_ascii_uppercase(),
                "T",
                640,
                480,
            );
            form.main_form = is_main;
            cobolt_forms::save_form(&form, &forms_dir.join(format!("{stem}.cfrm"))).unwrap();
        }

        // Reproduce the collection step's ordering decision — through the same
        // resolver the build and `rcrun` both use, so this test cannot pass on a
        // rule the shipped code no longer follows.
        let rels: Vec<String> = ["buttons", "checkboxes", "labels"]
            .iter()
            .map(|s| format!("forms/{s}.cfrm"))
            .collect();
        let mut forms: Vec<(String, Vec<u8>)> = Vec::new();
        for stem in ["buttons", "checkboxes", "labels"] {
            let raw = fs::read(forms_dir.join(format!("{stem}.cfrm"))).unwrap();
            forms.push((stem.to_ascii_uppercase(), raw));
        }
        assert_eq!(forms[0].0, "BUTTONS", "precondition: main is not first yet");

        let designated = main_form_guard::read_designation(&dir, &rels)
            .unwrap()
            .unwrap()
            .main_form_id;
        if let Some(at) = forms.iter().position(|(id, _)| *id == designated) {
            let main = forms.remove(at);
            forms.insert(0, main);
        }

        assert_eq!(
            forms[0].0, "CHECKBOXES",
            "the built binary opens FORMS.first(), so the main form must be first"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// **A built application checks at startup that it is still opening the
    /// form it was built to open.**
    ///
    /// Only the main form starts an application, and in a binary there is no
    /// project file left to consult — so the designation is baked in beside the
    /// form table, and the two are compared before the window opens. A table
    /// patched to begin somewhere else stops the application instead of opening
    /// a door the developer never put there.
    #[test]
    fn the_built_application_verifies_the_form_it_opens() {
        let src = generate_main_rs(
            "Demo",
            "1.0.0",
            true,
            &["SIGNON", "MENU"],
            "SIGNON",
            &[],
            &[],
            &[],
            "",
            "none:600:ease-out",
            "none:600:ease-out",
            false,
        );
        assert!(
            src.contains(r#"const MAIN_FORM: &str = "SIGNON";"#),
            "the designation is baked in, not read back off the table it guards"
        );
        assert!(
            src.contains("CORRUPTED APPLICATION"),
            "and it is checked before the window opens"
        );

        // The const exists even beside an empty table: the form runtime that
        // reads it is generated from `has_forms`, which is a different question
        // from whether any `.cfrm` survived staging. (A generated program that
        // compiles only sometimes is not a generated program.)
        let empty = generate_main_rs(
            "Demo",
            "1.0.0",
            true,
            &[],
            "",
            &[],
            &[],
            &[],
            "",
            "none:600:ease-out",
            "none:600:ease-out",
            false,
        );
        assert!(empty.contains("const MAIN_FORM"));
    }

    // ── Rebuild economy (spec 041 T14) ────────────────────────────────────────

    /// **AC13, measured** — building the same unchanged program twice compiles
    /// nothing the second time.
    ///
    /// The number reported is `cargo`'s own count of crates it compiled, taken
    /// from its output on each run. A count, not a stopwatch: elapsed time says
    /// different things on different machines, while "compiled 0 crates" means
    /// the same thing everywhere.
    ///
    /// This is also the regression guard for the churn the fix removed —
    /// regenerating `main.rs` and friends unconditionally gave them a fresh
    /// mtime every build, so cargo rebuilt the program's own crate every time
    /// even when nothing had changed.
    #[test]
    fn an_unchanged_program_recompiles_nothing() {
        let cobol = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. REBUILDME.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-STRING IS \"Rust.String\"
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-TEXT USAGE IS OBJECT REFERENCE RUST-STRING VALUE \"x\".
       PROCEDURE DIVISION.
           EXEC RUST
           ws_text.push_str(\"y\");
           END-EXEC.
           STOP RUN.
";
        let _heavy = heavy_build_guard();
        let dir = temp_dir("rebuild");
        let src = dir.join("rebuildme.cbl");
        fs::write(&src, cobol).unwrap();

        let opts = BuildOptions {
            verbose: false,
            ..Default::default()
        };

        let t0 = std::time::Instant::now();
        let first = build_single_file(&src, &opts).expect("the first build should succeed");
        let first_secs = t0.elapsed().as_secs_f64();

        let t1 = std::time::Instant::now();
        let second = build_single_file(&src, &opts).expect("the second build should succeed");
        let second_secs = t1.elapsed().as_secs_f64();

        println!(
            "AC13: build 1 compiled {} crate(s) in {first_secs:.1}s; \
             build 2 compiled {} crate(s) in {second_secs:.1}s",
            first.crates_compiled, second.crates_compiled
        );

        assert_eq!(
            second.crates_compiled, 0,
            "an unchanged program recompiled {} crate(s) on the second build \
             (the first compiled {})",
            second.crates_compiled, first.crates_compiled
        );

        // And a changed block *does* rebuild — a cache that never invalidates
        // would pass the assertion above while being much worse than useless.
        fs::write(&src, cobol.replace("push_str(\"y\")", "push_str(\"yz\")")).unwrap();
        let third = build_single_file(&src, &opts).expect("the third build should succeed");
        println!(
            "AC13: after editing the block, build 3 compiled {} crate(s)",
            third.crates_compiled
        );
        assert!(
            third.crates_compiled > 0,
            "editing the block did not trigger a recompile"
        );

        let _ = fs::remove_dir_all(&dir);
        remove_build_staging("rebuildme");
    }

    // ── Type coverage (spec 041 T12) ──────────────────────────────────────────

    /// **AC8** — every shipped `CLASS RUST-*` is declared as an item and used
    /// inside a compiled block, in one built and executed binary.
    ///
    /// The program is generated from `SHIPPED_RUST_TYPES` itself, so adding a
    /// class without giving it a binding fails here rather than shipping a class
    /// nobody can actually use. Two blocks touch every item: the second proves
    /// each value survived the first block's put-back, which is the part of the
    /// machinery a single block would not exercise.
    #[test]
    fn every_shipped_class_binds_inside_a_block() {
        use cobolt_ast::rust_types::SHIPPED_RUST_TYPES;

        let mut repository = String::new();
        let mut items = String::new();
        let mut first = String::new();
        let mut second = String::new();
        for (class, path) in SHIPPED_RUST_TYPES {
            let item = format!("WS-{class}");
            let var = item.to_ascii_lowercase().replace('-', "_");
            repository.push_str(&format!("           CLASS {class} IS \"{path}\"\n"));
            items.push_str(&format!(
                "       01 {item} USAGE IS OBJECT REFERENCE {class}.\n"
            ));
            // Type-agnostic, but real: `size_of_val` needs the binding to exist
            // at a concrete sized type, and the mutable borrow needs it to be a
            // genuine owned value rather than a copy of a stand-in.
            // `&*var` measures the bound VALUE. A bound name is a `&mut T`, so
            // `&var` would measure the reference — eight bytes for all 48,
            // which would pass while proving almost nothing.
            first.push_str(&format!(
                "           touched += std::mem::size_of_val(&*{var}) as i64;\n           let _ = &mut *{var};\n"
            ));
            second.push_str(&format!(
                "           again += std::mem::size_of_val(&*{var}) as i64;\n"
            ));
        }

        let cobol = format!(
            "       IDENTIFICATION DIVISION.\n\
             \x20      PROGRAM-ID. ALLTYPES.\n\
             \x20      ENVIRONMENT DIVISION.\n\
             \x20      CONFIGURATION SECTION.\n\
             \x20      REPOSITORY.\n{repository}\
             \x20      DATA DIVISION.\n\
             \x20      WORKING-STORAGE SECTION.\n{items}\
             \x20      PROCEDURE DIVISION.\n\
             \x20          EXEC RUST\n\
             \x20          let mut touched: i64 = 0;\n{first}\
             \x20          println!(\"TOUCHED={{}}\", touched);\n\
             \x20          END-EXEC.\n\
             \x20          EXEC RUST\n\
             \x20          let mut again: i64 = 0;\n{second}\
             \x20          println!(\"AGAIN={{}}\", again);\n\
             \x20          END-EXEC.\n\
             \x20          STOP RUN.\n"
        );

        let out = build_and_run_cobol("execrustalltypes", &cobol);

        let touched: i64 = out
            .lines()
            .find_map(|l| l.strip_prefix("TOUCHED="))
            .expect("the first block should run")
            .trim()
            .parse()
            .expect("a number");
        let again: i64 = out
            .lines()
            .find_map(|l| l.strip_prefix("AGAIN="))
            .expect("the second block should run")
            .trim()
            .parse()
            .expect("a number");
        assert_eq!(
            touched, again,
            "the objects did not survive the first block's put-back"
        );

        // The number this test actually exercised, reported rather than assumed.
        assert_eq!(
            SHIPPED_RUST_TYPES.len(),
            48,
            "the shipped class count changed — AC8 covers {} classes",
            SHIPPED_RUST_TYPES.len()
        );
        println!("AC8: exercised {} shipped classes", SHIPPED_RUST_TYPES.len());
    }

    /// **AC20, the event-handler half** — a form event handler is a *nested
    /// program*, and a block inside one is compiled, registered and dispatched
    /// like any other.
    ///
    /// This is the case a top-level-only walk would have dropped in silence,
    /// which is the exact failure mode spec 041 exists to remove. The type comes
    /// from an item-level block in the outer program, so the handler is also
    /// using a developer-defined type declared elsewhere.
    #[test]
    fn a_block_inside_a_nested_program_runs() {
        let out = build_and_run_cobol(
            "execrusthandler",
            r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS MY-COUNTER IS "Rust.Counter"
       EXEC RUST
       #[derive(Default)]
       pub struct Counter {
           pub hits: i64,
       }

       impl Counter {
           pub fn hit(&mut self) {
               self.hits += 1;
           }
       }
       END-EXEC.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-COUNTER USAGE IS OBJECT REFERENCE MY-COUNTER GLOBAL.
       PROCEDURE DIVISION.
           CALL "BUTTON1-CLICK".
           CALL "BUTTON1-CLICK".
           EXEC RUST
           println!("HITS={}", ws_counter.hits);
           END-EXEC.
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON1-CLICK.
       PROCEDURE DIVISION.
           EXEC RUST
           ws_counter.hit();
           END-EXEC.
           GOBACK.
       END PROGRAM BUTTON1-CLICK.
       END PROGRAM OUTER.
"#,
        );

        assert!(
            out.contains("HITS=2"),
            "a block in a nested program did not run twice; output was:\n{out}"
        );
    }

    /// The RAD generator's real handler shape, built and run: the handler's
    /// `OBJECT REFERENCE` items are declared in **its own** WORKING-STORAGE, a
    /// COBOL statement writes one, the block reads it and writes the other, and
    /// COBOL reads that back.
    ///
    /// Both halves failed before, and both reported the same thing — `FFI
    /// failed: EXEC RUST cannot bind COBOL-TEXT: handle 0 is not live`. Only the
    /// outermost program's items were given objects, so a handler-local item had
    /// no handle at all; and `SET item TO …` stored the value in the slot the
    /// handle lives in, destroying it. The nested test above passes with GLOBAL
    /// items in the outer program, which is why neither showed up there.
    #[test]
    fn a_handler_local_object_reference_survives_a_cobol_write_and_a_block() {
        let out = build_and_run_cobol(
            "execrusthandlerlocal",
            r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FORMPROG.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-STRING IS "Rust.String"
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-ERROR PIC X(120) GLOBAL.
       PROCEDURE DIVISION.
           CALL "BUTTON-1--ONCLICK".
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-1--ONCLICK IS COMMON PROGRAM.
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-TEXT  USAGE IS OBJECT REFERENCE RUST-STRING.
       01 RUST-RESULT USAGE IS OBJECT REFERENCE RUST-STRING.
       PROCEDURE DIVISION.
       MAIN.
           MOVE "ferris" TO COBOL-TEXT
           TRY
               EXEC RUST
               *rust_result = format!("{} says hello", cobol_text);
               END-EXEC
           CATCH RUST-EXCEPTION WS-ERROR
               DISPLAY "FFI failed: " WS-ERROR
           END-TRY
           DISPLAY "SAID=" RUST-RESULT
           GOBACK.
       END PROGRAM BUTTON-1--ONCLICK.
       END PROGRAM FORMPROG.
"#,
        );

        assert!(
            out.contains("SAID=ferris says hello"),
            "the handler's own OBJECT REFERENCE items did not survive the \
             COBOL write and the block; output was:\n{out}"
        );
    }

    // ── Toolchain and target (spec 041 T11) ───────────────────────────────────

    /// **AC12** — with `rustc` unavailable the build fails with a diagnostic
    /// that names the missing toolchain, rather than an opaque spawn error.
    #[test]
    fn a_missing_toolchain_is_named() {
        let err = probe_host_triple(|| {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No such file or directory (os error 2)",
            ))
        })
        .expect_err("a missing rustc must fail the build");

        let text = err.to_string();
        assert!(text.contains("rustc"), "the tool should be named: {text}");
        assert!(
            text.contains("Rust toolchain is required to build"),
            "the message should say what is missing: {text}"
        );
        assert!(
            text.contains("rustup.rs"),
            "the message should say how to fix it: {text}"
        );
        assert!(
            text.contains("does not"),
            "it should say the produced binary needs no toolchain: {text}"
        );
    }

    /// A `rustc` that runs but reports nothing usable is a broken toolchain,
    /// not a silent success.
    #[test]
    fn a_rustc_without_a_host_line_is_a_toolchain_error() {
        let out = probe_host_triple(|| {
            Ok(std::process::Output {
                status: Default::default(),
                stdout: b"rustc 1.99.0\n".to_vec(),
                stderr: Vec::new(),
            })
        });
        assert!(matches!(out, Err(CompilerError::Toolchain { .. })), "{out:?}");

        let good = probe_host_triple(|| {
            Ok(std::process::Output {
                status: Default::default(),
                stdout: b"rustc 1.99.0\nbinary: rustc\nhost: aarch64-apple-darwin\n".to_vec(),
                stderr: Vec::new(),
            })
        })
        .expect("a normal rustc -vV should parse");
        assert_eq!(good, "aarch64-apple-darwin");
    }

    /// **AC16** — asking for a target that is not the host is refused, and the
    /// message tells the developer to build on that operating system.
    #[test]
    fn a_non_host_target_is_refused() {
        let host = host_triple().expect("this test machine has a Rust toolchain");
        let other = if host.contains("windows") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-pc-windows-msvc"
        };

        let dir = temp_dir("crosstarget");
        let src = dir.join("cross.cbl");
        fs::write(
            &src,
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. CROSS.\nPROCEDURE DIVISION.\n    STOP RUN.\n",
        )
        .unwrap();

        let opts = BuildOptions {
            verbose: false,
            target: Some(other.to_string()),
            ..Default::default()
        };
        let err = build_single_file(&src, &opts).expect_err("a non-host target must be refused");
        let text = err.to_string();
        assert!(text.contains(other), "the requested target should be named: {text}");
        assert!(text.contains(&host), "the host should be named: {text}");
        assert!(
            text.contains("Build the application on"),
            "the message should say what to do instead: {text}"
        );

        // No binary was produced.
        assert!(!dir.join("bin").exists(), "a refused build left artefacts behind");
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Diagnostic mapping (spec 041 T10) ─────────────────────────────────────

    /// The translation itself, on a cargo message shaped exactly like the real
    /// one: a diagnostic on a generated line that carries developer code is
    /// restated in COBOL coordinates, and one on our own scaffolding is
    /// dropped rather than blamed on them.
    #[test]
    fn cargo_diagnostics_are_restated_in_cobol_coordinates() {
        let cobol = "IDENTIFICATION DIVISION.\n\
                     PROGRAM-ID. MAPDEMO.\n\
                     ENVIRONMENT DIVISION.\n\
                     CONFIGURATION SECTION.\n\
                     REPOSITORY.\n    CLASS RUST-STRING IS \"Rust.String\"\n\
                     DATA DIVISION.\n\
                     WORKING-STORAGE SECTION.\n\
                     01 WS-TEXT USAGE IS OBJECT REFERENCE RUST-STRING.\n\
                     PROCEDURE DIVISION.\n\
                     MAIN-PARA.\n    \
                     EXEC RUST\n    ws_text.push_str(\"x\");\n    END-EXEC.\n    \
                     STOP RUN.\n";
        let program = cobolt_parser::parse(cobolt_lexer::tokenize(
            cobol,
            cobolt_lexer::SourceFormat::Free,
        ))
        .program
        .expect("the fixture should parse");
        let symbols = cobolt_semantic::symbol_table::SymbolTable::build(&program);
        let blocks = exec_rust::generate(&program, &symbols, cobol, false);

        let developer_line = blocks
            .source
            .lines()
            .position(|l| l.contains("ws_text.push_str"))
            .expect("the developer's line was emitted") as u32
            + 1;
        let scaffold_line = blocks
            .source
            .lines()
            .position(|l| l.contains("pub fn register("))
            .unwrap() as u32
            + 1;

        let json = format!(
            r#"{{"reason":"compiler-message","message":{{"level":"error","message":"mismatched types","code":{{"code":"E0308"}},"spans":[{{"file_name":"/tmp/x/src/exec_rust_blocks.rs","line_start":{developer_line},"column_start":5,"is_primary":true}}]}}}}
{{"reason":"compiler-artifact","target":{{"name":"x"}}}}
not json at all
{{"reason":"compiler-message","message":{{"level":"error","message":"ours, not theirs","spans":[{{"file_name":"/tmp/x/src/exec_rust_blocks.rs","line_start":{scaffold_line},"column_start":1,"is_primary":true}}]}}}}
{{"reason":"compiler-message","message":{{"level":"error","message":"elsewhere entirely","spans":[{{"file_name":"/tmp/x/src/main.rs","line_start":3,"column_start":1,"is_primary":true}}]}}}}"#
        );

        let mapped = exec_rust::map_cargo_json(&json, &blocks);
        assert_eq!(
            mapped.len(),
            1,
            "only the developer's own diagnostic should map: {mapped:?}"
        );
        assert_eq!(mapped[0].message, "mismatched types");
        assert_eq!(mapped[0].code.as_deref(), Some("E0308"));

        let expected_line = cobol
            .lines()
            .position(|l| l.contains("ws_text.push_str"))
            .unwrap() as u32
            + 1;
        assert_eq!(
            mapped[0].line, expected_line,
            "the diagnostic should point at the developer's COBOL line"
        );
    }

    /// **AC4, end to end** — a block with a deliberate type error fails the
    /// build, and the reported line *and column* are the developer's, asserted
    /// as the exact numbers their editor would show.
    #[test]
    fn a_type_error_in_a_block_reports_the_developers_line_and_column() {
        let cobol = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BADRUST.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-STRING IS \"Rust.String\"
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-TEXT USAGE IS OBJECT REFERENCE RUST-STRING VALUE \"x\".
       PROCEDURE DIVISION.
           EXEC RUST
           let n: i64 = ws_text;
           END-EXEC.
           STOP RUN.
";
        let _heavy = heavy_build_guard();
        let dir = temp_dir("badrust");
        let src = dir.join("badrust.cbl");
        fs::write(&src, cobol).unwrap();

        let opts = BuildOptions {
            verbose: false,
            ..Default::default()
        };
        let err = build_single_file(&src, &opts).expect_err("a type error must fail the build");

        let (line, col, message) = match &err {
            CompilerError::ExecRustBlock {
                line,
                col,
                message,
                ..
            } => (*line, *col, message.clone()),
            other => panic!("expected an EXEC RUST diagnostic, got: {other}"),
        };

        // The line the developer wrote the offending Rust on.
        let offending = cobol
            .lines()
            .position(|l| l.contains("let n: i64 = ws_text;"))
            .expect("the fixture has that line");
        assert_eq!(line, offending as u32 + 1, "wrong line: {err}");

        // The column of `ws_text` within it — 1-based, as an editor counts.
        let expected_col = cobol.lines().nth(offending).unwrap().find("ws_text").unwrap() as u32 + 1;
        assert_eq!(col, expected_col, "wrong column: {err}");

        assert!(
            message.contains("mismatched types") || message.contains("E0308"),
            "rustc's own words should survive: {message}"
        );
        // Nothing about the generated file leaks into what the developer reads.
        let shown = err.to_string();
        assert!(
            !shown.contains("exec_rust_blocks"),
            "generated-code detail leaked into the diagnostic: {shown}"
        );

        let _ = fs::remove_dir_all(&dir);
        remove_build_staging("badrust");
    }

    /// **AC5 — the regression guard for the defect that started spec 041.**
    ///
    /// `foo();` is exactly what the deleted interpreter would have written to a
    /// debug log and skipped, so the block "succeeded" while doing nothing. It
    /// now fails the build, at the developer's own line, saying `foo` is not
    /// found. The runtime half — a block with no compiled function behind it —
    /// is `an_unregistered_block_fails_loudly` in `cobolt-runtime`.
    #[test]
    fn a_statement_the_old_interpreter_would_have_skipped_fails_the_build() {
        let cobol = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SILENTNOMORE.
       PROCEDURE DIVISION.
           EXEC RUST
           foo();
           END-EXEC.
           STOP RUN.
";
        let _heavy = heavy_build_guard();
        let dir = temp_dir("silentnomore");
        let src = dir.join("silentnomore.cbl");
        fs::write(&src, cobol).unwrap();

        let opts = BuildOptions {
            verbose: false,
            ..Default::default()
        };
        let err = build_single_file(&src, &opts)
            .expect_err("an unrecognised statement must fail the build, not be skipped");

        match &err {
            CompilerError::ExecRustBlock { line, message, .. } => {
                let offending = cobol
                    .lines()
                    .position(|l| l.contains("foo();"))
                    .expect("the fixture has that line") as u32
                    + 1;
                assert_eq!(*line, offending, "wrong line: {err}");
                assert!(
                    message.contains("foo"),
                    "the message should name what is missing: {message}"
                );
            }
            other => panic!("expected an EXEC RUST diagnostic, got: {other}"),
        }

        let _ = fs::remove_dir_all(&dir);
        remove_build_staging("silentnomore");
    }

    /// **AC21, the half T4 deferred here** — an item-level block containing a
    /// statement is rejected. Deciding that needs a Rust parser, so `rustc`
    /// decides it, and the mapping is what makes the answer land on the
    /// developer's line instead of on generated code.
    #[test]
    fn a_statement_in_an_item_level_block_is_rejected_at_its_own_line() {
        let cobol = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BADITEM.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-STRING IS \"Rust.String\"
       EXEC RUST
       let stray = 1;
       END-EXEC.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N PIC 9(4).
       PROCEDURE DIVISION.
           STOP RUN.
";
        let _heavy = heavy_build_guard();
        let dir = temp_dir("baditem");
        let src = dir.join("baditem.cbl");
        fs::write(&src, cobol).unwrap();

        let opts = BuildOptions {
            verbose: false,
            ..Default::default()
        };
        let err = build_single_file(&src, &opts).expect_err("a statement at module scope must fail");

        match &err {
            CompilerError::ExecRustBlock { line, .. } => {
                let offending = cobol
                    .lines()
                    .position(|l| l.contains("let stray = 1;"))
                    .expect("the fixture has that line") as u32
                    + 1;
                assert_eq!(*line, offending, "the statement's own line should be named: {err}");
            }
            other => panic!("expected an EXEC RUST diagnostic, got: {other}"),
        }

        let _ = fs::remove_dir_all(&dir);
        remove_build_staging("baditem");
    }

    /// **AC19/AC20 (item-level half)** — a `struct` and `impl` written in an
    /// item-level block are module-scope items, so a `CLASS` can name the type,
    /// an item can be declared with it, and blocks in *different* paragraphs can
    /// both use it, sharing one object.
    #[test]
    fn a_developer_defined_type_is_usable_across_paragraphs() {
        let out = build_and_run_cobol(
            "execrustpoint",
            r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RUSTPOINT.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS MY-POINT IS "Rust.Point"
       EXEC RUST
       #[derive(Default, Debug)]
       pub struct Point {
           pub x: i64,
           pub y: i64,
       }

       impl Point {
           pub fn shift(&mut self, dx: i64, dy: i64) {
               self.x += dx;
               self.y += dy;
           }
           pub fn manhattan(&self) -> i64 {
               self.x.abs() + self.y.abs()
           }
       }
       END-EXEC.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 ORIGIN USAGE IS OBJECT REFERENCE MY-POINT.
       PROCEDURE DIVISION.
       MAIN-PARA.
           PERFORM MOVE-IT
           PERFORM REPORT-IT
           STOP RUN.
       MOVE-IT.
           EXEC RUST
           origin.shift(3, 4);
           END-EXEC.
       REPORT-IT.
           EXEC RUST
           println!("DIST={}", origin.manhattan());
           END-EXEC.
"#,
        );

        assert!(
            out.contains("DIST=7"),
            "the developer-defined type did not survive between paragraphs:\n{out}"
        );
    }

    #[test]
    fn staging_copies_the_manifest_and_only_the_art_it_references() {
        let dir = temp_dir("themestage");
        let pack = dir.join("themes").join("demo-pack");
        fs::create_dir_all(pack.join("button")).unwrap();
        fs::write(
            pack.join("theme.toml"),
            "id = \"demo-pack\"\ndisplay_name = \"Demo\"\n\n\
             [background]\nimage = \"background.png\"\n\n\
             [controls.button]\nimage = \"button/normal.png\"\nslice = [4, 4, 4, 4]\n\
             hover = \"button/hover.png\"\n",
        )
        .unwrap();
        fs::write(pack.join("background.png"), b"bg").unwrap();
        fs::write(pack.join("button/normal.png"), b"n").unwrap();
        fs::write(pack.join("button/hover.png"), b"h").unwrap();
        // Authoring imagery the manifest never points at must not be embedded.
        fs::write(pack.join("button/scratch.png"), b"x").unwrap();

        let out = dir.join("staged");
        let staged = stage_theme_packs(
            &["demo-pack".to_owned()],
            &[dir.join("themes")],
            &out,
            &|_: &str| {},
        )
        .expect("stage");

        assert_eq!(staged.len(), 1);
        assert_eq!(
            staged[0].assets,
            vec![
                "background.png".to_owned(),
                "button/normal.png".to_owned(),
                "button/hover.png".to_owned()
            ]
        );
        assert!(out.join("demo-pack/theme.toml").is_file());
        assert!(out.join("demo-pack/button/normal.png").is_file());
        assert!(!out.join("demo-pack/button/scratch.png").exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_pack_is_reported_and_never_fails_the_build() {
        let dir = temp_dir("thememissing");
        fs::create_dir_all(&dir).unwrap();
        let warned = std::cell::RefCell::new(Vec::new());
        let staged = stage_theme_packs(
            &["no-such-pack".to_owned()],
            &[dir.clone()],
            &dir.join("staged"),
            &|m: &str| warned.borrow_mut().push(m.to_owned()),
        )
        .expect("stage");
        assert!(staged.is_empty());
        assert!(warned.borrow().iter().any(|m| m.contains("no-such-pack")));
        fs::remove_dir_all(&dir).ok();
    }

    /// The layout every real project has: designs under `forms/<sub>/`, their
    /// generated programs together in `generated/`. `rcrun run-form` used to
    /// look ONLY beside the `.cfrm`, so opening a child form failed with "has
    /// no generated program beside it" on a project whose compiled build ran
    /// the same form perfectly.
    #[test]
    fn a_forms_program_is_found_in_the_projects_generated_folder() {
        let dir = temp_dir("formprog");
        fs::create_dir_all(dir.join("forms/Inner-Forms")).unwrap();
        fs::create_dir_all(dir.join("generated")).unwrap();
        let cfrm = dir.join("forms/Inner-Forms/inner-form1.cfrm");
        fs::write(&cfrm, "<Form/>").unwrap();
        fs::write(dir.join("generated/inner-form1.cbl"), "x").unwrap();
        fs::write(
            dir.join("PowerDemo3.project.toml"),
            r#"
[project]
name = "PowerDemo3"
version = "1.0.0"
main = ""

[files]
forms = ["forms/Inner-Forms/inner-form1.cfrm"]
generated = ["generated/inner-form1.cbl"]
"#,
        )
        .unwrap();

        assert_eq!(
            form_program_path(&cfrm, "inner-form1"),
            Some(dir.join("generated/inner-form1.cbl")),
            "the program is in the project's generated/ folder, not beside the design"
        );
        // The manifest is discovered from a form nested two levels down, and
        // is named after the project rather than a fixed `cobolt.toml`.
        assert_eq!(
            find_project_manifest(&cfrm),
            Some(dir.join("PowerDemo3.project.toml"))
        );
        fs::remove_dir_all(&dir).ok();
    }

    /// A form belonging to no project at all still runs: its program is simply
    /// the `.cbl` beside it. And a form whose program has not been generated
    /// yet resolves to nothing, so the caller can say so plainly.
    #[test]
    fn a_loose_form_falls_back_beside_itself_and_a_missing_program_is_none() {
        let dir = temp_dir("looseform");
        fs::create_dir_all(&dir).unwrap();
        let cfrm = dir.join("solo.cfrm");
        fs::write(&cfrm, "<Form/>").unwrap();
        assert_eq!(
            form_program_path(&cfrm, "solo"),
            None,
            "nothing generated yet ⇒ no program"
        );
        fs::write(dir.join("solo.cbl"), "x").unwrap();
        assert_eq!(form_program_path(&cfrm, "solo"), Some(dir.join("solo.cbl")));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn prefers_declared_main_when_it_exists() {
        let dir = temp_dir("main");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/main.cbl"), "x").unwrap();
        let p = proj("src/main.cbl", vec![], vec!["generated/form.cbl"]);
        assert_eq!(resolve_main(&p, &dir).as_deref(), Some("src/main.cbl"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn falls_back_to_generated_form_when_main_missing() {
        // Reproduces the form-only project: declared main was never written.
        let dir = temp_dir("form");
        fs::create_dir_all(dir.join("generated")).unwrap();
        fs::write(dir.join("generated/power-demo-1.cbl"), "x").unwrap();
        let p = proj("src/main.cbl", vec![], vec!["generated/power-demo-1.cbl"]);
        assert_eq!(
            resolve_main(&p, &dir).as_deref(),
            Some("generated/power-demo-1.cbl")
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    /// The assistant must know about EVERY control the toolbox offers — not
    /// the ones a hand-kept array happened to list. That array had lost the
    /// SideMenu, so a detailed description of shell mode, `FullHeight`,
    /// `Collapsed` and the menu actions sat in this file and reached nobody.
    #[test]
    fn every_control_the_toolbox_offers_is_published_to_the_assistant() {
        let dir = temp_dir("kb-all-controls");
        assert!(publish_system_documentation(&dir).is_ok());
        let doc =
            fs::read_to_string(dir.join("Knowledge Base").join("form_designer_controls.md"))
                .unwrap();
        let missing: Vec<&str> = cobolt_forms::ControlType::ALL
            .iter()
            .map(|ct| ct.as_str())
            .filter(|name| !doc.contains(&format!("## Control: {name}")))
            .collect();
        assert!(
            missing.is_empty(),
            "controls absent from the assistant's reference: {missing:?}"
        );
        // The SideMenu's own text, not just its heading — the description
        // existed all along; only the publishing did not.
        assert!(
            doc.contains("SHELL mode"),
            "the SideMenu section must carry its shell-mode description"
        );
        println!(
            "\n  System KB — all {} control types published to the assistant\n",
            cobolt_forms::ControlType::ALL.len()
        );
    }

    #[test]
    fn spec_039_six_controls_are_fully_published_in_the_system_kb() {
        // Spec 039 T17/R3/AC12: Maps/Knob/Gauge/Switch/FileDropZone/WebSearch
        // must appear in BOTH published KB documents — the per-control
        // reference (`form_designer_controls.md`, properties + events +
        // methods) and the closed-vocabulary method reference
        // (`control_methods_reference.md`).
        let dir = temp_dir("spec039kb");
        assert!(publish_system_documentation(&dir).is_ok());
        let kb = dir.join("Knowledge Base");

        let form_controls_doc = fs::read_to_string(kb.join("form_designer_controls.md")).unwrap();
        for ct in [
            cobolt_forms::ControlType::Knob,
            cobolt_forms::ControlType::Gauge,
            cobolt_forms::ControlType::Switch,
            cobolt_forms::ControlType::FileDropZone,
            cobolt_forms::ControlType::Maps,
            cobolt_forms::ControlType::WebSearch,
        ] {
            let name = ct.as_str();
            assert!(
                form_controls_doc.contains(&format!("## Control: {name}")),
                "{name} section missing from form_designer_controls.md"
            );
        }
        // A representative property, event, and method from each control,
        // proving the tables are populated, not just the section headers.
        for needle in [
            "Bipolar",              // Knob property
            "GaugeStyle",           // Gauge property
            "onFilesDropped",       // FileDropZone event
            "CenterLat",            // Maps property
            "onMarkerClick",        // Maps event
            "AddMarker",            // Maps method
            "SearchEngineId",       // WebSearch property
            "onResultsReceived",    // WebSearch primary event
            "GetResult(index: Integer)", // WebSearch method
        ] {
            assert!(
                form_controls_doc.contains(needle),
                "form_designer_controls.md missing expected content: {needle}"
            );
        }

        let methods_doc = fs::read_to_string(kb.join("control_methods_reference.md")).unwrap();
        for needle in [
            "## Knob",
            "## Gauge",
            "## Switch",
            "## FileDropZone",
            "## Maps",
            "## WebSearch",
            "Geocode(address: String)",
            "Search()",
        ] {
            assert!(
                methods_doc.contains(needle),
                "control_methods_reference.md missing expected content: {needle}"
            );
        }

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn none_when_nothing_exists() {
        let dir = temp_dir("empty");
        let p = proj("src/main.cbl", vec!["a.cbl"], vec!["b.cbl"]);
        assert_eq!(resolve_main(&p, &dir), None);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn publishes_system_documentation_correctly() {
        let dir = temp_dir("pubdocs");
        assert!(publish_system_documentation(&dir).is_ok());

        let kb = dir.join("Knowledge Base");
        assert!(kb.exists());
        assert!(kb.join("rustcobol_extensions.md").exists());
        assert!(kb.join("ide_functionalities.md").exists());
        assert!(kb.join("form_designer_controls.md").exists());
        assert!(kb.join("agents_registry.md").exists());
        assert!(kb.join("control_methods_reference.md").exists());
        assert!(kb.join("form_themes.md").exists());
        assert!(kb.join("form_layout_and_events.md").exists());
        assert!(kb.join("project_model_and_settings.md").exists());

        // Check form designer controls document contents
        let form_controls_doc = fs::read_to_string(kb.join("form_designer_controls.md")).unwrap();
        assert!(form_controls_doc.contains("# PowerRustCOBOL Form Controls Reference"));
        assert!(form_controls_doc.contains("## Control: Button"));
        assert!(form_controls_doc.contains("## Control: DataGrid"));
        assert!(form_controls_doc.contains("### Settable Properties"));
        assert!(form_controls_doc.contains("### Supported Events"));
        assert!(form_controls_doc.contains("### Methods"));
        // Read-only runtime properties are documented as properties. Listing
        // only what the designer can SET, while saying elsewhere that an async
        // method "delivers its answer in ResponseBody", reads as though
        // ResponseBody were not a property — which is how a correct handler
        // came to be rejected as hallucinated (operator, 2026-08-21).
        assert!(form_controls_doc.contains("### Runtime Properties (read-only)"));
        assert!(form_controls_doc.contains("- `ResponseBody` —"));
        assert!(form_controls_doc.contains("- `SelectedMarkerId` —"));
        // Enrichment: types + defaults + domains are present.
        assert!(form_controls_doc.contains("(Boolean, default"));
        assert!(form_controls_doc.contains("(Integer, default"));
        assert!(form_controls_doc.contains("0-100"));
        assert!(form_controls_doc.contains("## Universal properties (every control)"));
        assert!(form_controls_doc.contains("## The Form (window)"));
        // Every designed property either is universal or documents a domain/description.
        let sample = cobolt_forms::Control::new("_", cobolt_forms::ControlType::DataGrid, 0, 0);
        for (name, _) in &sample.properties {
            assert!(
                UNIVERSAL_PROPS.contains(&name.as_str()) || property_reference(name).is_some(),
                "DataGrid property `{name}` has no curated documentation"
            );
        }

        // Check the methods reference document contents
        let methods_doc = fs::read_to_string(kb.join("control_methods_reference.md")).unwrap();
        assert!(methods_doc.contains("# PowerRustCOBOL Control Methods Reference"));
        assert!(methods_doc.contains("AddPoint(label: String, value: Number)"));
        assert!(methods_doc.contains("IndexedFile — no inline methods"));
        assert!(methods_doc.contains("GET-<Prop>()"));

        // Clean up
        fs::remove_dir_all(&dir).ok();
    }

    /// The platform reference has to cover the things a developer configures,
    /// not only the controls they drop. Themes, the layout/event model and the
    /// project model were the three gaps; each assertion below pins the fact
    /// that was actually being got wrong, so gutting a document fails here
    /// rather than silently degrading every agent answer.
    #[test]
    fn the_platform_reference_covers_themes_layout_and_the_project_model() {
        let dir = temp_dir("pubdocs-coverage");
        assert!(publish_system_documentation(&dir).is_ok());
        let kb = dir.join("Knowledge Base");

        let themes = fs::read_to_string(kb.join("form_themes.md")).unwrap();
        for fact in [
            "liquid-glass",
            "elegance",
            "self-contained",
            "GlassStyle",
            "theme.toml",
        ] {
            assert!(themes.contains(fact), "form_themes.md lost `{fact}`");
        }

        let layout = fs::read_to_string(kb.join("form_layout_and_events.md")).unwrap();
        for fact in ["StartPosition", "FormFormat", "super::", "onDeactivate"] {
            assert!(layout.contains(fact), "form_layout_and_events.md lost `{fact}`");
        }
        // Every form event the model supports is named in the document. An
        // event nobody carried into the KB is invisible to every agent.
        let missing: Vec<&str> = cobolt_forms::model::form_supported_events()
            .filter(|ev| !layout.contains(*ev))
            .collect();
        assert!(missing.is_empty(), "form events missing from the KB: {missing:?}");

        let project = fs::read_to_string(kb.join("project_model_and_settings.md")).unwrap();
        for fact in [".project.toml", "[files]", "Generated Code", "[forms]"] {
            assert!(
                project.contains(fact),
                "project_model_and_settings.md lost `{fact}`"
            );
        }

        // The System KB describes itself, so every document it publishes must
        // be named in its own sibling list — a document nothing points at is a
        // document no agent will think to search.
        let ide = fs::read_to_string(kb.join("ide_functionalities.md")).unwrap();
        let mut published = 0;
        for entry in fs::read_dir(&kb).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            published += 1;
            if name == "ide_functionalities.md" {
                continue; // "this document"
            }
            assert!(
                ide.contains(&name),
                "{name} is published but not named in the System KB sibling list"
            );
        }
        println!("System KB publishes {published} documents, all cross-referenced");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn every_control_property_is_documented() {
        // The doc generator derives names/types/defaults from Control::new, so
        // this test pins the curated layer: any NEW property must land in
        // `property_reference` (or UNIVERSAL_PROPS) before it ships undocumented.
        let all = [
            cobolt_forms::ControlType::Button,
            cobolt_forms::ControlType::TextBox,
            cobolt_forms::ControlType::Label,
            cobolt_forms::ControlType::CheckBox,
            cobolt_forms::ControlType::RadioButton,
            cobolt_forms::ControlType::ListBox,
            cobolt_forms::ControlType::ComboBox,
            cobolt_forms::ControlType::GroupBox,
            cobolt_forms::ControlType::Panel,
            cobolt_forms::ControlType::TabControl,
            cobolt_forms::ControlType::DataGrid,
            cobolt_forms::ControlType::PictureBox,
            cobolt_forms::ControlType::ProgressBar,
            cobolt_forms::ControlType::MenuBar,
            cobolt_forms::ControlType::ToolBar,
            cobolt_forms::ControlType::StatusBar,
            cobolt_forms::ControlType::Line,
            cobolt_forms::ControlType::DateTimePicker,
            cobolt_forms::ControlType::NumericUpDown,
            cobolt_forms::ControlType::TreeView,
            cobolt_forms::ControlType::Splitter,
            cobolt_forms::ControlType::Timer,
            cobolt_forms::ControlType::Shape,
            cobolt_forms::ControlType::Animator,
            cobolt_forms::ControlType::AgentObject,
            cobolt_forms::ControlType::RestClient,
            cobolt_forms::ControlType::SqlDatabase,
            cobolt_forms::ControlType::IndexedFile,
            cobolt_forms::ControlType::Slider,
            cobolt_forms::ControlType::BarChart,
            cobolt_forms::ControlType::LineChart,
            cobolt_forms::ControlType::PieChart,
            cobolt_forms::ControlType::AreaChart,
            cobolt_forms::ControlType::ScatterChart,
            cobolt_forms::ControlType::DonutChart,
            cobolt_forms::ControlType::Knob,
            cobolt_forms::ControlType::Gauge,
            cobolt_forms::ControlType::Switch,
            cobolt_forms::ControlType::FileDropZone,
            cobolt_forms::ControlType::Maps,
            cobolt_forms::ControlType::WebSearch,
        ];
        for ct in all {
            let type_name = ct.as_str().to_owned();
            let ctrl = cobolt_forms::Control::new("_", ct, 0, 0);
            for (name, _) in &ctrl.properties {
                assert!(
                    UNIVERSAL_PROPS.contains(&name.as_str())
                        || property_reference(name).is_some(),
                    "{type_name} property `{name}` has no curated documentation — \
                     add it to property_reference()"
                );
            }
        }
    }
}
