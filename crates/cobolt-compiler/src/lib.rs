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

#[derive(Deserialize)]
struct CoboltProject {
    project: ProjectMeta,
    #[serde(default)]
    files: ProjectFiles,
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
    // Publish system documentation to project's Knowledge Base
    if let Err(e) = publish_system_documentation(&project_dir) {
        eprintln!("Warning: could not publish system documentation to Knowledge Base: {e}");
    }
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

    // Copy rcrun binary to destination folder
    if let Some(rcrun_src) = find_rcrun() {
        let rcrun_name = if cfg!(windows) { "rcrun.exe" } else { "rcrun" };
        let dest_rcrun = dest_path.join(rcrun_name);
        if let Err(e) = std::fs::copy(&rcrun_src, &dest_rcrun) {
            log(&format!("⚠️  Failed to copy rcrun to destination: {e}"));
        } else {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = std::fs::metadata(&dest_rcrun) {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o755);
                    let _ = std::fs::set_permissions(&dest_rcrun, perms);
                }
            }
            log(&format!("rcrun binary copied to {}", dest_rcrun.display()));
        }
    } else {
        log("⚠️  rcrun binary not found, skipped copying rcrun.");
    }

    report(1.0, "Done");
    Ok(BuildResult {
        binary_path: dst_bin,
        source_count: sources.len(),
        form_count: forms.len(),
        ast_bytes: ast_compressed_len,
    })
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

fn generate_main_rs(app_name: &str, version: &str, has_forms: bool, form_ids: &[&str]) -> String {
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

    let form_runtime_code = if has_forms {
        r#"
// ── Form application (spec 017: one renderer for every surface) ───────────────

/// Mutable UI-side state of a single control (mirrors the IDE's CtrlState).
#[derive(Clone, Default)]
struct CtrlState {
    props:   std::collections::HashMap<String, String>,
    visible: bool,
    enabled: bool,
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
        glass_style: first_form.glass_style,
        visuals_set: false,
        form_size: egui::vec2(fw, fh),
        ev_tx,
        input_tx,
        state_rx,
        display_rx,
        start: std::time::Instant::now(),
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
    glass_style:  cobolt_forms::model::GlassStyle,
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
        // Glass style for the unified painter (same contract as the IDE).
        cobolt_forms::paint::set_glass_style(ctx, self.glass_style);

        // Apply property changes coming from the COBOL interpreter. COBOL
        // upper-cases control ids, so resolve each to the designer-case state
        // key — otherwise handler writes land in an orphan entry the renderer
        // never reads and events appear not to fire.
        let mut drained = 0usize;
        while let Ok(u) = self.state_rx.try_recv() {
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
            let st = CompiledState { state: &self.state };
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
        let busy = drained > 0 || interacted;
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

fn find_rcrun() -> Option<PathBuf> {
    let rcrun_name = if cfg!(windows) { "rcrun.exe" } else { "rcrun" };
    if let Ok(current_exe) = std::env::current_exe() {
        let p = current_exe.with_file_name(rcrun_name);
        if p.exists() {
            return Some(p);
        }
        if let Some(parent) = current_exe.parent() {
            let p2 = parent.parent().map(|p| p.join(rcrun_name));
            if let Some(p2) = p2 {
                if p2.exists() {
                    return Some(p2);
                }
            }
        }
    }
    if let Ok(path_val) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_val) {
            let p = dir.join(rcrun_name);
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

/// Publishes RustCOBOL extensions, IDE functionalities, RAD form designer controls/properties/events,
/// and the Agent Registry into the project's Knowledge Base during compilation.
pub fn publish_system_documentation(project_dir: &std::path::Path) -> Result<(), std::io::Error> {
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
  - Example: `IndexedFile-1::Open().`
  - Example: `RestClient-1::Post("/api/save", request_body).`
- **DO NOT** use `CALL` or legacy `INVOKE` for UI control properties or methods. Use the inline double-colon (`::`) syntax directly.

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

    // Write File 3: form_designer_controls.md
    let control_types = vec![
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
    doc.push_str("This document inventories all visual and non-visual controls supported by the PowerRustCOBOL RAD Form Designer, listing their properties and events.\n\n");

    for ct in control_types {
        let name = ct.as_str();
        doc.push_str(&format!("## Control: {name}\n\n"));
        
        doc.push_str("### Settable Properties\n");
        let mut props = cobolt_forms::model::property_names_for(name);
        props.sort();
        if props.is_empty() {
            doc.push_str("- None\n");
        } else {
            for prop in props {
                doc.push_str(&format!("- `{prop}`\n"));
            }
        }
        doc.push_str("\n");

        doc.push_str("### Supported Events\n");
        let mut events: Vec<String> = ct.supported_events().iter().map(|e| (*e).to_string()).collect();
        events.sort();
        if events.is_empty() {
            doc.push_str("- None\n");
        } else {
            for event in events {
                doc.push_str(&format!("- `{event}`\n"));
            }
        }
        doc.push_str("\n---\n\n");
    }
    std::fs::write(kb_dir.join("form_designer_controls.md"), doc)?;

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
        }
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

        // Check form designer controls document contents
        let form_controls_doc = fs::read_to_string(kb.join("form_designer_controls.md")).unwrap();
        assert!(form_controls_doc.contains("# PowerRustCOBOL Form Controls Reference"));
        assert!(form_controls_doc.contains("## Control: Button"));
        assert!(form_controls_doc.contains("## Control: DataGrid"));
        assert!(form_controls_doc.contains("### Settable Properties"));
        assert!(form_controls_doc.contains("### Supported Events"));

        // Clean up
        fs::remove_dir_all(&dir).ok();
    }
}
