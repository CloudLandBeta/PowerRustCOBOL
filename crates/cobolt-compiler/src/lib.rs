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

    #[error("No main COBOL source specified in cobolt.toml")]
    NoMain,

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
}

#[derive(Deserialize)]
struct CoboltProject {
    project: ProjectMeta,
    #[serde(default)]
    files: ProjectFiles,
    #[serde(default)]
    forms: FormsConfig,
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

// ── Public API ────────────────────────────────────────────────────────────────

/// A build progress update, for driving a UI progress bar.
#[derive(Clone, Debug)]
pub struct BuildProgress {
    /// Completion in `0.0..=1.0`.
    pub fraction: f32,
    /// Short, human-readable description of the current phase.
    pub message: String,
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
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            verbose: true,
            workspace_root: None,
            progress: None,
        }
    }
}

/// Build result returned on success.
pub struct BuildResult {
    /// Path to the produced executable.
    pub binary_path: PathBuf,
    /// Number of COBOL source files compiled.
    pub source_count: usize,
    /// Number of form files embedded.
    pub form_count: usize,
    /// Compressed AST size in bytes.
    pub ast_bytes: usize,
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

    build_core(proj, project_dir, opts)
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
    };
    build_core(proj, project_dir, opts)
}

/// Shared build pipeline used by both [`build_project`] and [`build_single_file`].
fn build_core(
    proj: CoboltProject,
    project_dir: PathBuf,
    opts: &BuildOptions,
) -> Result<BuildResult, CompilerError> {
    let log = |msg: &str| {
        if opts.verbose {
            eprintln!("{msg}");
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
            });
        }
    };

    report(0.05, "Reading project…");
    // The system documentation is NOT published here. It describes the
    // platform, not this project, so it belongs to the machine-level System
    // Knowledge Base that the IDE republishes from the running binary — see
    // `publish_system_documentation`. Publishing it per project also mixed
    // platform reference material into the developer's own Knowledge Base,
    // whose whole purpose is project material (diagrams, requirements, data
    // models). Existing copies under `<project>/Knowledge Base/` are left
    // alone: they are the developer's files to remove.
    let bin_name = proj.project.name.to_ascii_lowercase().replace(' ', "_");

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
    report(0.25, "Parsing & analysing…");
    use cobolt_lexer::{tokenize, SourceFormat};
    use cobolt_parser::parse;
    use cobolt_semantic::{analyze, Severity};

    // We compile the main source into the primary Program.
    // Additional sources are currently compiled independently and merged via
    // their nested-program lists — a full multi-file linker is future work.
    let (main_rel, main_src) = &sources[0];
    let fmt = detect_format(main_src);
    let tokens = tokenize(main_src, fmt);
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

    let sem = analyze(&program);
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
    report(0.42, "Collecting forms…");
    let mut forms: Vec<(String, Vec<u8>)> = Vec::new(); // (id, raw_xml_bytes)

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
        forms.push((id, raw));
    }

    log(&format!("   {} form(s)", forms.len()));

    // ── 6. Locate workspace root (where the cobolt-* crates live) ────────────
    let has_crates = |root: &Path| root.join("crates").join("cobolt-ast").is_dir();
    let workspace_root = opts
        .workspace_root
        .clone()
        // Walk up from the running exe — works when launched from the source tree.
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| find_workspace_root(&p))
        })
        .filter(|p| has_crates(p))
        // Compile-time fallback: the PowerRustCOBOL workspace this compiler was
        // built in. Works when the IDE runs from an installed location (e.g. a
        // macOS .app) whose path is outside the source tree.
        .or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(2) // crates/cobolt-compiler → crates → workspace
                .map(Path::to_path_buf)
                .filter(|p| has_crates(p))
        });

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
    report(0.50, "Preparing build project…");
    let build_dir = std::env::temp_dir().join(format!("cobolt-build-{}", &bin_name));
    let assets_dir = build_dir.join("assets");
    let forms_dir = assets_dir.join("forms");
    let src_dir = build_dir.join("src");
    std::fs::create_dir_all(&assets_dir)?;
    std::fs::create_dir_all(&forms_dir)?;
    std::fs::create_dir_all(&src_dir)?;

    // Write compressed AST
    std::fs::write(assets_dir.join("program.bin"), &compressed_ast)?;

    // Write form files
    for (id, raw) in &forms {
        std::fs::write(forms_dir.join(format!("{id}.cfrm")), raw)?;
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

    // ── 8. Generate Cargo.toml for the build project ──────────────────────────
    let crates_path = workspace_root.join("crates");
    let has_forms = !forms.is_empty();

    let cargo_toml = generate_cargo_toml(&bin_name, &proj.project.version, &crates_path, has_forms);
    std::fs::write(build_dir.join("Cargo.toml"), cargo_toml)?;

    // ── 9. Generate src/main.rs ───────────────────────────────────────────────
    let form_ids: Vec<&str> = forms.iter().map(|(id, _)| id.as_str()).collect();
    let main_rs = generate_main_rs(
        &proj.project.name,
        &proj.project.version,
        has_forms,
        &form_ids,
        &staged_themes,
        &project_theme_default,
    );
    std::fs::write(src_dir.join("main.rs"), main_rs)?;

    // ── 10. Run cargo build --release ─────────────────────────────────────────
    // Stream cargo's stderr so the progress bar advances per crate compiled and
    // shows the crate currently building.
    report(0.60, "Compiling…");
    use std::io::{BufRead as _, BufReader};
    let mut args = vec!["build"];
    if !proj.project.debug_compilation {
        args.push("--release");
    }
    let mut child = std::process::Command::new("cargo")
        .args(&args)
        .current_dir(&build_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut captured = String::new();
    let mut compiled = 0usize;
    if let Some(err) = child.stderr.take() {
        for line in BufReader::new(err).lines() {
            let line = line.unwrap_or_default();
            if let Some(rest) = line.trim_start().strip_prefix("Compiling ") {
                compiled += 1;
                let name = rest.split_whitespace().next().unwrap_or("");
                // Asymptotically approach 0.95 as more crates finish.
                let frac = 0.60 + 0.35 * (1.0 - 1.0 / (1.0 + compiled as f32 / 12.0));
                report(frac.min(0.95), &format!("Compiling {name}…"));
            }
            captured.push_str(&line);
            captured.push('\n');
        }
    }
    let status = child.wait()?;
    if !status.success() {
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
    std::fs::copy(&src_bin, &dst_bin)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dst_bin)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dst_bin, perms)?;
    }

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
    let dest_name = if proj.project.destination_folder.trim().is_empty() {
        if let Some(stripped) = proj.project.name.strip_suffix(".project") {
            stripped.to_string()
        } else {
            proj.project.name.clone()
        }
    } else {
        proj.project.destination_folder.trim().to_string()
    };

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

    // Copy project binary to destination folder
    let dest_bin = dest_path.join(&exe_name);
    if let Err(e) = std::fs::copy(&dst_bin, &dest_bin) {
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

    report(1.0, "Done");
    Ok(BuildResult {
        binary_path: dst_bin,
        source_count: sources.len(),
        form_count: forms.len(),
        ast_bytes: ast_compressed_len,
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
        if id == cobolt_forms::theme::LIQUID_GLASS || ids.contains(&id) {
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

fn generate_cargo_toml(
    bin_name: &str,
    version: &str,
    crates_path: &Path,
    has_forms: bool,
) -> String {
    let cp = crates_path.display();
    let mut s = format!(
        r#"[package]
name    = "{bin_name}"
version = "{version}"
edition = "2021"

[[bin]]
name = "{bin_name}"
path = "src/main.rs"

[dependencies]
cobolt-ast      = {{ path = "{cp}/cobolt-ast" }}
cobolt-runtime  = {{ path = "{cp}/cobolt-runtime" }}
flate2          = "1"
bincode         = "1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}
tracing         = "0.1"
"#
    );

    if has_forms {
        s.push_str(&format!(
            r#"cobolt-forms    = {{ path = "{cp}/cobolt-forms", features = ["render"] }}
cobolt-media    = {{ path = "{cp}/cobolt-media" }}
eframe          = {{ version = "0.35", features = ["default_fonts"] }}
egui            = "0.35"
egui_extras     = {{ version = "0.35", features = ["image"] }}
"#
        ));
    }

    s
}

fn generate_main_rs(
    app_name: &str,
    version: &str,
    has_forms: bool,
    form_ids: &[&str],
    themes: &[StagedTheme],
    project_theme_default: &str,
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

    let form_runtime_code = if has_forms {
        r#"
// ── Form application (spec 017: one renderer for every surface) ───────────────

/// Mutable UI-side state of a single control (mirrors the IDE's CtrlState).
/// An entry created on the fly starts VISIBLE and ENABLED: a derived Default
/// would make it `false`, so a control the interpreter writes to before it
/// exists in the map (a repeating-group card instance) would never be drawn.
#[derive(Clone)]
struct CtrlState {
    props:   std::collections::HashMap<String, String>,
    visible: bool,
    enabled: bool,
}
impl Default for CtrlState {
    fn default() -> Self {
        CtrlState { props: std::collections::HashMap::new(), visible: true, enabled: true }
    }
}
impl CtrlState {
    fn from_control(ctrl: &cobolt_forms::Control) -> Self {
        let mut props = std::collections::HashMap::new();
        for (k, v) in &ctrl.properties {
            props.insert(k.clone(), v.to_xml_string());
        }
        CtrlState { props, visible: ctrl.visible, enabled: ctrl.enabled }
    }
    fn set(&mut self, key: &str, value: String) {
        match key {
            "Visible" => self.visible = value != "0" && value != "false",
            "Enabled" => self.enabled = value != "0" && value != "false",
            _ => {}
        }
        self.props.insert(key.to_owned(), value);
    }
}

fn flatten_controls(controls: &[cobolt_forms::Control], out: &mut Vec<cobolt_forms::Control>) {
    for c in controls {
        out.push(c.clone());
        flatten_controls(&c.children, out);
    }
}

/// `FormState` over the compiled control-state map: merges live property values
/// onto each designed control so the unified render engine paints the binary
/// exactly like the IDE preview / running form (background, glass, charts,
/// styled widgets — spec 017 T7).
struct CompiledState<'a> {
    state: &'a std::collections::HashMap<String, CtrlState>,
    anim:  &'a cobolt_forms::anim::AnimRuntime,
}
impl<'a> cobolt_forms::render::FormState for CompiledState<'a> {
    fn live(&self, base: &cobolt_forms::Control) -> cobolt_forms::Control {
        match self.state.get(&base.id) {
            Some(s) => cobolt_forms::render::merge_props(base, s.props.iter()),
            None => base.clone(),
        }
    }
    fn visible(&self, base: &cobolt_forms::Control) -> bool {
        self.state.get(&base.id).map(|s| s.visible).unwrap_or(true)
    }
    fn enabled(&self, base: &cobolt_forms::Control) -> bool {
        self.state.get(&base.id).map(|s| s.enabled).unwrap_or(true)
    }
    fn transform(&self, base: &cobolt_forms::Control) -> cobolt_forms::render::RenderTransform {
        self.anim.transform(base)
    }
}

/// What a COBOL animation verb asked for. `PLAY ANIMATION`, `STOP-ANIMATION`
/// and `PAUSE` reach the UI as writes to these pseudo-properties.
#[derive(Clone, Copy, PartialEq)]
enum AnimCommand { Play, Stop, Pause }

fn anim_command(prop: &str) -> Option<AnimCommand> {
    match prop.trim() {
        p if p.eq_ignore_ascii_case("_PlayAnimation")  => Some(AnimCommand::Play),
        p if p.eq_ignore_ascii_case("_StopAnimation")  => Some(AnimCommand::Stop),
        p if p.eq_ignore_ascii_case("_PauseAnimation") => Some(AnimCommand::Pause),
        _ => None,
    }
}

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

fn run_form_app(program: cobolt_ast::program::Program) {
    use cobolt_forms::load_form_from_str;
    use cobolt_runtime::{Interpreter, FormEvent, StateUpdate};
    use std::sync::mpsc;

    // Load the first embedded form — defines the window size + initial layout.
    let first_form = if let Some(&(_, bytes)) = FORMS.first() {
        let xml = std::str::from_utf8(bytes).expect("form XML is valid UTF-8");
        load_form_from_str(xml).expect("parse embedded form")
    } else {
        run_headless(program);
        return;
    };

    // Flatten + z-order the controls and build the initial control state.
    let mut flat: Vec<cobolt_forms::Control> = Vec::new();
    flatten_controls(&first_form.controls, &mut flat);
    flat.sort_by_key(|c| c.z_order);

    let mut state: std::collections::HashMap<String, CtrlState> = std::collections::HashMap::new();
    for c in &flat {
        state.insert(c.id.clone(), CtrlState::from_control(c));
    }

    let (fw, fh) = (first_form.width as f32, first_form.height as f32);
    let title = format!("{} v{}", APP_NAME, APP_VERSION);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&title)
            .with_inner_size([fw + 4.0, fh + 4.0]),
        ..Default::default()
    };

    let (ev_tx, ev_rx)           = mpsc::channel::<FormEvent>();
    let (input_tx, input_rx)     = mpsc::channel::<StateUpdate>();
    let (state_tx, state_rx)     = mpsc::channel::<StateUpdate>();
    let (display_tx, display_rx) = mpsc::channel::<String>();

    // The COBOL event loop runs on its own thread. The input channel lets the UI
    // push live control values (slider drag, text edit, …) so event handlers read
    // the current value rather than the seeded default.
    std::thread::spawn(move || {
        let mut interp = Interpreter::new_with_channels(program, ev_rx, state_tx, display_tx);
        interp.set_input_channel(input_rx);
        let _ = interp.run();
    });

    let app = FormApp {
        controls: flat,
        state,
        bg_hex: first_form.background_color.clone(),
        bg_gradient_enabled: first_form.background_gradient_enabled,
        bg_gradient_start: first_form.background_gradient_start_color.clone(),
        bg_gradient_end: first_form.background_gradient_end_color.clone(),
        bg_gradient_direction: first_form.background_gradient_direction.clone(),
        transparency: first_form.transparency.clamp(0, 100) as u8,
        bg_image: first_form.background_image.clone(),
        bg_mode: first_form.bg_image_mode,
        use_theme_background: first_form.use_theme_background,
        glass_style: first_form.glass_style,
        theme_pack: resolve_theme_pack(&first_form),
        visuals_set: false,
        form_size: egui::vec2(fw, fh),
        ev_tx,
        input_tx,
        state_rx,
        display_rx,
        start: std::time::Instant::now(),
        anim: cobolt_forms::anim::AnimRuntime::new(fw, fh),
        anim_started: false,
        last_frame: None,
        hovered: std::collections::HashSet::new(),
    };
    let _ = eframe::run_native(
        &title,
        native_options,
        Box::new(move |_cc| Ok(Box::new(app) as Box<dyn eframe::App>)),
    );
}

struct FormApp {
    controls:     Vec<cobolt_forms::Control>,
    state:        std::collections::HashMap<String, CtrlState>,
    bg_hex:       String,
    bg_gradient_enabled: bool,
    bg_gradient_start: String,
    bg_gradient_end: String,
    bg_gradient_direction: String,
    transparency: u8,
    bg_image:     String,
    bg_mode:      cobolt_forms::model::BgImageMode,
    /// The form's `UseThemeBackground` opt-in — the pack's background art
    /// replaces the form's own image when the active theme provides one.
    use_theme_background: bool,
    glass_style:  cobolt_forms::model::GlassStyle,
    /// The form's asset-pack theme, built from art embedded in this binary.
    /// `None` = the built-in procedural Liquid Glass.
    theme_pack:   Option<std::sync::Arc<cobolt_forms::theme_pack::ThemePack>>,
    visuals_set:  bool,
    form_size:    egui::Vec2,
    ev_tx:        std::sync::mpsc::Sender<cobolt_runtime::FormEvent>,
    input_tx:     std::sync::mpsc::Sender<cobolt_runtime::StateUpdate>,
    state_rx:     std::sync::mpsc::Receiver<cobolt_runtime::StateUpdate>,
    display_rx:   std::sync::mpsc::Receiver<String>,
    /// When the window opened. Input events are ignored for a short warm-up so
    /// that a click already in progress as the window appears cannot be mistaken
    /// for an intentional interaction.
    start:        std::time::Instant,
    /// Control animations (fly-in, fade, pulse, …), driven by the shared
    /// `cobolt_forms::anim` runtime so a built binary animates exactly like the
    /// designer preview and the run form.
    anim:         cobolt_forms::anim::AnimRuntime,
    /// One-shot guard for the load-time (`OnFormLoad` / `OnShow`) animations.
    anim_started: bool,
    /// Previous frame's timestamp — the animation clock's delta.
    last_frame:   Option<std::time::Instant>,
    /// Controls the pointer was inside last frame, so `OnHover` fires on entry
    /// only. Pointer triggers come from the rendered rects because the engine
    /// emits onClick/onHoverEnter only for events with a bound COBOL handler.
    hovered:      std::collections::HashSet<String>,
}

impl eframe::App for FormApp {
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root_ui.ctx().clone();
        let ctx = &ctx;
        // Light visuals baseline — egui defaults to dark mode, which leaks dark
        // widget fills into the form and breaks parity with the designer.
        if !self.visuals_set {
            self.visuals_set = true;
            ctx.set_visuals(egui::Visuals::light());
        }
        // Theme pack + glass style for the unified painter (per frame — the same
        // contract the IDE's canvas, preview and run form follow). Without the
        // theme pack a form skinned with an asset pack rendered as procedural
        // Liquid Glass here, so the shipped app looked nothing like the design.
        cobolt_forms::paint::set_active_theme(ctx, self.theme_pack.clone());
        cobolt_forms::paint::set_glass_style(ctx, self.glass_style);

        // Apply property changes coming from the COBOL interpreter. COBOL
        // upper-cases control ids, so resolve each to the designer-case state
        // key — otherwise handler writes land in an orphan entry the renderer
        // never reads and events appear not to fire.
        //  Animation clock: load-time animations start with the window, then
        //  `tick` advances everything and reports whether a frame is still owed.
        if !self.anim_started {
            self.anim_started = true;
            self.anim.start_form_load(&self.controls);
        }
        let now = std::time::Instant::now();
        let dt = self.last_frame
            .map(|t| now.duration_since(t).as_secs_f32())
            .unwrap_or(0.0);
        self.last_frame = Some(now);
        let animating = self.anim.tick(dt);

        let mut drained = 0usize;
        while let Ok(u) = self.state_rx.try_recv() {
            // COBOL's PLAY ANIMATION / STOP-ANIMATION / PAUSE: act on the write,
            // don't store it as a property.
            if let Some(cmd) = anim_command(&u.prop) {
                match cmd {
                    AnimCommand::Play  => self.anim.play_programmatic(&self.controls, &u.ctrl_id, &u.value),
                    AnimCommand::Stop  => self.anim.stop_all(&u.ctrl_id),
                    AnimCommand::Pause => self.anim.pause_all(&u.ctrl_id),
                }
                drained += 1;
                continue;
            }
            let key = self.state.keys()
                .find(|k| k.eq_ignore_ascii_case(&u.ctrl_id))
                .cloned()
                .unwrap_or_else(|| u.ctrl_id.clone());
            self.state.entry(key).or_default().set(&u.prop, u.value);
            drained += 1;
        }
        // DISPLAY output → stdout.
        while let Ok(line) = self.display_rx.try_recv() {
            println!("{}", line);
        }

        // Ignore input for a brief warm-up after the window appears.
        let armed = self.start.elapsed().as_millis() > 450;

        // Background image texture (cached in egui memory by path).
        let backdrop_image = if self.bg_image.trim().is_empty() {
            None
        } else {
            let path = self.bg_image.clone();
            let id = egui::Id::new(("compiled_bg", path.as_str()));
            let cached = ctx.memory(|m| m.data.get_temp::<Option<egui::TextureHandle>>(id));
            let tex = match cached {
                Some(t) => t,
                None => {
                    let loaded = cobolt_forms::paint::load_image_texture(ctx, &path);
                    ctx.memory_mut(|m| m.data.insert_temp(id, loaded.clone()));
                    loaded
                }
            };
            tex.map(|t| (t.id(), t.size_vec2()))
        };

        let bg_fill = cobolt_forms::render::backdrop_color(&self.bg_hex, self.transparency);
        let form_size = self.form_size;

        // Render the whole form through the unified engine (one renderer for the
        // designer, preview, running form, and this compiled binary).
        let output = {
            let controls = self.controls.clone();
            let st = CompiledState { state: &self.state, anim: &self.anim };
            let active_tabs = cobolt_forms::containers::ActiveTabs::default();
            let backdrop = cobolt_forms::render::Backdrop {
                color_hex: self.bg_hex.clone(),
                transparency: self.transparency,
                gradient_enabled: self.bg_gradient_enabled,
                gradient_start_hex: self.bg_gradient_start.clone(),
                gradient_end_hex: self.bg_gradient_end.clone(),
                gradient_direction: self.bg_gradient_direction.clone(),
                image: backdrop_image,
                image_mode: self.bg_mode,
                use_theme_background: self.use_theme_background,
            };
            let mut out = cobolt_forms::render::RenderOutput::default();
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(bg_fill))
                .show(root_ui, |ui| {
                    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                        ui.set_min_size(form_size);
                        let input = cobolt_forms::render::RenderInput {
                            controls: &controls,
                            state: &st,
                            form_size,
                            glass: true,
                            mode: cobolt_forms::render::RenderMode::Interactive,
                            active_tabs: &active_tabs,
                            backdrop,
                        };
                        out = cobolt_forms::render::render_form(ui, &input);
                    });
                });
            out
        };

        // Animation triggers: pointer ones from the rendered rects (an animation
        // is reason enough to react, with or without a COBOL handler), focus and
        // timer ones from the event stream.
        if armed {
            let (clicked, pointer) = ctx.input(|i| (i.pointer.primary_clicked(), i.pointer.interact_pos()));
            let mut still_hovered = std::collections::HashSet::new();
            for (id, rect) in &output.control_rects {
                // Repeating-group card instances carry their own placement effect.
                if id.contains('.') { continue; }
                if pointer.map(|p| rect.contains(p)).unwrap_or(false) {
                    still_hovered.insert(id.clone());
                    if !self.hovered.contains(id) {
                        self.anim.fire_event(&self.controls, id, "onHoverEnter");
                    }
                    if clicked {
                        self.anim.fire_event(&self.controls, id, "onClick");
                    }
                }
            }
            self.hovered = still_hovered;
            for ev in &output.events {
                // Already covered by the rect pass — firing again would restart
                // the same animation twice in one frame.
                if ev.event.eq_ignore_ascii_case("onClick")
                    || ev.event.eq_ignore_ascii_case("onHoverEnter") {
                    continue;
                }
                self.anim.fire_event(&self.controls, &ev.ctrl_id, &ev.event);
            }
        }

        // Apply value updates locally, sync them to the interpreter (so handlers
        // read the live value), and forward UI events — but only once warmed up,
        // so phantom pointer input as the window opens can't mutate state or fire
        // events (a click/drag already in progress when the window appears).
        let mut interacted = false;
        if armed {
            for (id, key, val) in &output.prop_updates {
                self.state.entry(id.clone()).or_default().set(key, val.clone());
                let _ = self.input_tx.send(
                    cobolt_runtime::StateUpdate::new(id.clone(), key.clone(), val.clone()));
                interacted = true;
            }
            for ev in output.events {
                let _ = self.ev_tx.send(cobolt_runtime::FormEvent::new(ev.ctrl_id, ev.event));
                interacted = true;
            }
        }

        // Reactive frame scheduling — never spin at max FPS (an unconditional
        // request_repaint() pegged a whole core even while the form sat idle).
        // While interpreter traffic flows, poll fast; otherwise a slow
        // heartbeat keeps DISPLAY output timely. Timer controls schedule their
        // own precise wake-ups inside the render engine, and user input wakes
        // egui automatically — between all of those, the process sleeps.
        // A running animation needs frames of its own; without this the binary
        // sleeps between interpreter traffic and a fly-in advances in 200 ms jumps.
        let busy = drained > 0 || interacted || animating || self.anim.is_animating();
        let ms = if busy { 16 } else { 200 };
        ctx.request_repaint_after(std::time::Duration::from_millis(ms));
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

const APP_NAME:    &str = "{app_name}";
const APP_VERSION: &str = "{version}";

// ── Embedded assets ───────────────────────────────────────────────────────────
/// Deflate-compressed bincode of the compiled COBOL AST.
static PROGRAM_AST: &[u8] = include_bytes!("../assets/program.bin");

/// Embedded form files — loaded lazily by form ID.
{forms_const}
/// Embedded asset-pack themes: `(id, theme.toml source, [(image ref, bytes)])`.
/// Only the packs the forms actually resolve to are baked in, and only the art
/// their manifests reference, so a themed app is self-contained without
/// carrying the packs' authoring imagery.
{themes_const}{theme_default_const}
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

// ── Headless (CLI) runner ─────────────────────────────────────────────────────
fn run_headless(program: cobolt_ast::program::Program) {{
    use cobolt_runtime::Interpreter;
    let mut interp = Interpreter::new(program);
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
        themes_const = themes_const,
        theme_default_const = theme_default_const,
        run_call = run_call,
        form_runtime_code = form_runtime_code,
    )
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn detect_format(source: &str) -> cobolt_lexer::SourceFormat {
    let looks_fixed = source.lines().any(|line| {
        let b = line.as_bytes();
        b.len() > 6 && b[6] != b' ' && b[..6].iter().all(|&c| c == b' ' || c.is_ascii_digit())
    });
    if looks_fixed {
        cobolt_lexer::SourceFormat::Fixed
    } else {
        cobolt_lexer::SourceFormat::Free
    }
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
- **IndexedFile controls have no `::` methods** — drive them with the generated paragraphs (`PERFORM <id>-OPEN`, `<id>-READ-NEXT`, …) and the COBOL verbs `WRITE`/`REWRITE`/`DELETE`.
- A method that returns a value can be used inline (`MOVE C::GetText() TO WS-X`) or with `RETURNING`.

## Value Conventions (types and domains)
- **Boolean properties** store `1` (true) / `0` (false). Write `SET C::Visible TO 1`. On method arguments, `true`/`yes`/`on` (any case) also count as true.
- **Colors** are hex strings: `"#RRGGBB"` or `"#RRGGBBAA"` (e.g. `"#FF0000"`, `"#00000000"` = transparent).
- **Coordinates and sizes** (`X`, `Y`, `Width`, `Height`, paddings, radii) are integer pixels.
- **List content** (`Items` of ListBox/ComboBox/ToolBar/StatusBar/TreeView) is ONE ITEM PER LINE (newline-separated); TreeView nests children with two leading spaces per level. Indexes (`SelectedIndex`, grid rows/columns) are 0-based; -1 = no selection.
- **DataGrid data**: `Columns` is one `Name:Type` per line (`Type` ∈ `string`|`number`|`datetime`); `Rows` separates rows with newlines and cells with TAB.
- **Enumerated properties** accept only their listed values EXACTLY as spelled (e.g. `Orientation` is `Horizontal` or `Vertical`); an unrecognised value falls back to the default without an error.
- **Property names**: setting a misspelled property silently creates a new, unused property — never guess names; use the ones in the Form Controls Reference.
- **Charts**: feed data with `Chart::AddPoint(label, value)` / `Chart::Clear()` / `Chart::Refresh()`, with `PERFORM <id>-ADD-POINT` / `<id>-SET-TABLE` paragraphs, or with `CALL "COBOL-CHART-ADD-POINT" USING "<id>" label value` — or bind a COBOL table via the `DataSource`/`DataCount` properties. Do NOT invent working-storage tables for charts.

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
- Do not write `IDENTIFICATION DIVISION`, `PROGRAM-ID`, `GOBACK`, or `END PROGRAM` in the handler body; these are automatically managed by the IDE scaffold.
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

## Code generation & Compilation
- Multi-agent coordination: Grace (Orchestrator) plans and delegates UI design to Form Designer Agent, event implementations to COBOL Event Handler Script Agent, and schema setups to Data Agent.
- During IDE build/compilation, the project is parsed, semantic checks are performed, and it is compiled into a single native executable.
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
    "Opacity",
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
fn property_reference(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        // ── Universal appearance ──
        "BackgroundColor" => (COLOR_DOMAIN, "Fill color behind the control's content."),
        "BackgroundGradientEnabled" => (BOOL_DOMAIN, "Enables the two-color background gradient."),
        "BackgroundGradientStartColor" => (COLOR_DOMAIN, "Gradient start color."),
        "BackgroundGradientEndColor" => (COLOR_DOMAIN, "Gradient end color."),
        "BackgroundGradientDirection" => (EIGHT_DIRECTIONS, "Direction the gradient flows toward."),
        "ForegroundColor" => (COLOR_DOMAIN, "Text / foreground drawing color."),
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
        "Opacity" => ("0-100 (percent)", "Overall control opacity; 100 = fully opaque."),
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
        "ScrollBars" => ("one of: `None` | `Horizontal` | `Vertical` | `Both`", "Which scrollbars a multiline box shows."),
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
        "CheckColor" => (COLOR_DOMAIN, "Color of the check/radio mark."),

        // ── Images / animation ──
        "ImagePath" => ("project-relative or absolute image path", "Image file to display."),
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
        "BarColor" => (COLOR_DOMAIN, "Filled-portion color of the progress bar."),
        "Orientation" => ("`Horizontal` | `Vertical`", "Layout axis."),
        "Style" => ("`Continuous` | `Blocks`", "Progress bar fill style."),
        "ShowValue" => (BOOL_DOMAIN, "Draws the numeric value on the control."),
        "TickFrequency" => ("integer > 0 (value units)", "Draw a tick every N units."),
        "TickStyle" => ("one of: `None` | `Top` | `Bottom` | `Both`", "Where slider ticks are drawn."),
        "TrackColor" => (COLOR_DOMAIN, "Slider track color."),
        "ThumbColor" => (COLOR_DOMAIN, "Slider knob color."),

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
        "MultiSelect" => (BOOL_DOMAIN, "Allows selecting several items."),
        "Sorted" => (BOOL_DOMAIN, "Keeps items alphabetically sorted."),
        "DropDownStyle" => ("one of: `DropDown` | `DropDownList` | `Simple`", "ComboBox edit/list behaviour."),
        "DropDownHeight" => ("pixels > 0", "Maximum height of the opened list."),
        "Editable" => (BOOL_DOMAIN, "Allows typing free text into the combo."),

        // ── TreeView ──
        "AllowEdit" => (BOOL_DOMAIN, "In-place node label editing."),
        "CheckBoxes" => (BOOL_DOMAIN, "Shows a checkbox on every node."),
        "ShowLines" => (BOOL_DOMAIN, "Draws connector lines between nodes."),
        "ShowRootLines" => (BOOL_DOMAIN, "Draws connector lines at the root level."),
        "HotTracking" => (BOOL_DOMAIN, "Highlights the node under the pointer."),
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
        "FillColor" => (COLOR_DOMAIN, "Shape interior fill (also Slider filled-track color)."),
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
        "IconSize" => ("pixels, one of: `16` `32` `48` `64` `80` `96` `128`", "Icon edge length."),

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
        "onCheckedChanged" => "checked state flipped",
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
        _ => "",
    }
}

/// One-line purpose for each control type, leading its reference section.
fn control_purpose(name: &str) -> &'static str {
    match name {
        "Button" => "Clickable push button.",
        "TextBox" => "Single- or multi-line text input.",
        "Label" => "Static text display.",
        "CheckBox" => "Boolean on/off box with caption.",
        "RadioButton" => "Mutually-exclusive choice within a GroupName.",
        "ListBox" => "Scrollable list of selectable items.",
        "ComboBox" => "Drop-down list, optionally editable.",
        "GroupBox" => "Captioned container; can become a repeating card template (control array).",
        "Panel" => "Plain container for grouping child controls.",
        "TabControl" => "Multi-page container with a tab strip.",
        "DataGrid" => "Tabular rows/columns grid with sorting, filtering, freezing and CSV export.",
        "PictureBox" => "Displays a still image.",
        "ProgressBar" => "Shows progress within Minimum..Maximum.",
        "MenuBar" => "Window menu bar (menu structure is edited in the designer and stored in a `.menu.yaml` sidecar, not in a property).",
        "ToolBar" => "Horizontal strip of action items.",
        "StatusBar" => "Bottom status strip.",
        "Line" => "Decorative straight line.",
        "DateTimePicker" => "Date/time input with calendar or spinner.",
        "NumericUpDown" => "Integer input with spinner arrows.",
        "TreeView" => "Hierarchical node list.",
        "Splitter" => "Draggable divider between two areas.",
        "Timer" => "Non-visual: fires `onTick` every Interval ms.",
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
        _ => "",
    }
}

/// `(signature, description)` pairs for the inline methods that apply to one
/// control type (beyond the universal set documented in the shared section).
fn control_method_docs(name: &str) -> Vec<(&'static str, &'static str)> {
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
        "ProgressBar" | "Slider" | "NumericUpDown" | "DateTimePicker" => value_methods,
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
        _ => "",
    }
}

/// Render the enriched Form Controls Reference (KB file 3).
fn controls_reference_doc() -> String {
    let control_types = [
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
    ];

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
         property — it is never an error, so spelling matters.\n\n",
    );

    // ── Shared sections ──────────────────────────────────────────────────────
    doc.push_str("## Universal properties (every control)\n\n");
    doc.push_str("Layout fields (settable like any property):\n\n");
    for (sig, dom, desc) in [
        ("Name", "String — control identifier", "The control id (assigned by the designer; treat as read-only)."),
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
        "\nEvent handlers carry no parameters (repeating-group members receive \
         `CONTROL-ARRAY-INDEX`, the 1-based index of the card that fired).\n\n",
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
         it with a crown), `TaskbarIcon` (image path — main form only: the single taskbar/dock \
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
         `SetWindowState`, `SetFullScreen`, `SetTitleVisible`, `Focus`, `Close`.\n\n---\n\n",
    );

    // ── Per-control sections ─────────────────────────────────────────────────
    for ct in control_types {
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

    #[test]
    fn generated_binary_publishes_its_theme_pack_every_frame() {
        let themes = vec![StagedTheme {
            id: "cobalt-steel".into(),
            assets: vec!["background.png".into(), "button/b.png".into()],
        }];
        let src = generate_main_rs("Demo", "1.0.0", true, &["MAIN"], &themes, "neumorphic");

        // The regression this guards: the template used to set only the glass
        // style, so an asset-pack form shipped as procedural Liquid Glass.
        assert!(src.contains("set_active_theme(ctx, self.theme_pack.clone())"));
        assert!(src.contains("set_glass_style(ctx, self.glass_style)"));
        assert!(src.contains("theme_pack: resolve_theme_pack(&first_form)"));

        // The pack's manifest and art are embedded, keeping the binary
        // self-contained on a machine with no PowerRustCOBOL install.
        assert!(src.contains(r#"include_str!("../assets/themes/cobalt-steel/theme.toml")"#));
        assert!(src
            .contains(r#"("background.png", include_bytes!("../assets/themes/cobalt-steel/background.png"))"#));
        // Nested refs keep their pack-relative path so the manifest still resolves.
        assert!(src
            .contains(r#"("button/b.png", include_bytes!("../assets/themes/cobalt-steel/button/b.png"))"#));
        assert!(src.contains(r#"const PROJECT_THEME_DEFAULT: &str = "neumorphic";"#));
        // The form's themed-background opt-in reaches the render engine.
        assert!(src.contains("use_theme_background: self.use_theme_background"));
    }

    #[test]
    fn generated_binary_without_themes_still_compiles_to_liquid_glass() {
        let src = generate_main_rs("Demo", "1.0.0", true, &["MAIN"], &[], "");
        assert!(src.contains("static THEMES: &[(&str, &str, &[(&str, &[u8])])] = &[];"));
        assert!(src.contains(r#"const PROJECT_THEME_DEFAULT: &str = "";"#));
        // Resolution still runs — it just finds no pack and yields Liquid Glass.
        assert!(src.contains("fn resolve_theme_pack("));
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

        // Check form designer controls document contents
        let form_controls_doc = fs::read_to_string(kb.join("form_designer_controls.md")).unwrap();
        assert!(form_controls_doc.contains("# PowerRustCOBOL Form Controls Reference"));
        assert!(form_controls_doc.contains("## Control: Button"));
        assert!(form_controls_doc.contains("## Control: DataGrid"));
        assert!(form_controls_doc.contains("### Settable Properties"));
        assert!(form_controls_doc.contains("### Supported Events"));
        assert!(form_controls_doc.contains("### Methods"));
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
