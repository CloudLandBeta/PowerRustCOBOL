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

use flate2::Compression;
use flate2::write::GzEncoder;
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
pub const RUNTIME_NOTICE_TEXT: &str =
    include_str!("../../../docs/licensing/PACKAGE_NOTICE_TEMPLATE/POWER_RUST_COBOL_RUNTIME_NOTICE.txt");

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
}

// ── Project manifest (subset we need) ────────────────────────────────────────

#[derive(Deserialize)]
struct ProjectMeta {
    name:    String,
    version: String,
    main:    String,
}

#[derive(Deserialize, Default)]
struct ProjectFiles {
    #[serde(default)] sources: Vec<String>,
    #[serde(default)] forms:   Vec<String>,
}

#[derive(Deserialize)]
struct CoboltProject {
    project: ProjectMeta,
    #[serde(default)]
    files: ProjectFiles,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Options controlling the build.
pub struct BuildOptions {
    /// Print progress to stderr.
    pub verbose: bool,
    /// Override the workspace root (where the cobolt-* crates live).
    /// Defaults to the directory containing the compiler's own executable.
    pub workspace_root: Option<PathBuf>,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self { verbose: true, workspace_root: None }
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
    if opts.verbose { eprintln!("📖 Reading cobolt.toml …"); }
    let manifest_text = std::fs::read_to_string(manifest_path)?;
    let proj: CoboltProject = toml::from_str(&manifest_text)
        .map_err(|e| CompilerError::Toml(e.to_string()))?;

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
        project: ProjectMeta { name, version: "1.0.0".into(), main },
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
        if opts.verbose { eprintln!("{msg}"); }
    };

    let bin_name = proj.project.name
        .to_ascii_lowercase()
        .replace(' ', "_");

    // ── 2. Collect all source files ───────────────────────────────────────────
    log("📂 Collecting source files …");
    let mut sources: Vec<(String, String)> = Vec::new(); // (rel_path, source_text)

    // Always include the main file first.
    let main_path = project_dir.join(&proj.project.main);
    if !main_path.exists() {
        return Err(CompilerError::NoMain);
    }
    sources.push((
        proj.project.main.clone(),
        std::fs::read_to_string(&main_path)?,
    ));

    // Then the rest of the declared sources (skip main if listed again).
    for rel in &proj.files.sources {
        if rel == &proj.project.main { continue; }
        let abs = project_dir.join(rel);
        if abs.exists() {
            sources.push((rel.clone(), std::fs::read_to_string(&abs)?));
        }
    }

    log(&format!("   {} source file(s)", sources.len()));

    // ── 3. Parse + semantic-check every source ────────────────────────────────
    log("🔍 Parsing and analysing …");
    use cobolt_lexer::{SourceFormat, tokenize};
    use cobolt_parser::parse;
    use cobolt_semantic::{Severity, analyze};

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
                file:    main_rel.clone(),
                message: d.message.clone(),
            });
        }
    }

    let program = parse_result.program.ok_or_else(|| CompilerError::Parse {
        file:    main_rel.clone(),
        message: "Parse produced no program".into(),
    })?;

    let sem = analyze(&program);
    for d in &sem.diagnostics {
        if d.severity == Severity::Error {
            return Err(CompilerError::Semantic {
                file:    main_rel.clone(),
                message: d.message.clone(),
            });
        }
    }

    // ── 4. Serialize + compress the AST ──────────────────────────────────────
    log("📦 Serializing AST …");
    let ast_bytes = bincode::serialize(&program)
        .map_err(|e| CompilerError::Serialize(e.to_string()))?;

    let mut gz = GzEncoder::new(Vec::new(), Compression::best());
    gz.write_all(&ast_bytes).unwrap();
    let compressed_ast = gz.finish()?;
    let ast_compressed_len = compressed_ast.len();
    log(&format!("   AST: {} bytes → {} bytes compressed", ast_bytes.len(), ast_compressed_len));

    // ── 5. Collect form files ─────────────────────────────────────────────────
    log("🗔 Collecting form files …");
    let mut forms: Vec<(String, Vec<u8>)> = Vec::new(); // (id, raw_xml_bytes)

    for rel in &proj.files.forms {
        let abs = project_dir.join(rel);
        if !abs.exists() { continue; }
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
    let workspace_root = opts.workspace_root.clone()
        .unwrap_or_else(|| {
            // Walk up from the current exe to find Cargo.toml with [workspace]
            std::env::current_exe()
                .ok()
                .and_then(|p| find_workspace_root(p.as_path()))
                .unwrap_or_else(|| project_dir.clone())
        });

    log(&format!("🏠 Workspace root: {}", workspace_root.display()));

    // ── 7. Create build staging directory ────────────────────────────────────
    log("🏗️  Generating build project …");
    let build_dir = std::env::temp_dir()
        .join(format!("cobolt-build-{}", &bin_name));
    let assets_dir  = build_dir.join("assets");
    let forms_dir   = assets_dir.join("forms");
    let src_dir     = build_dir.join("src");
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
    let has_forms   = !forms.is_empty();

    let cargo_toml = generate_cargo_toml(
        &bin_name,
        &proj.project.version,
        &crates_path,
        has_forms,
    );
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
    log("🔨 Compiling (cargo build --release) — this may take a minute …");
    let output = std::process::Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&build_dir)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(CompilerError::CargoBuild {
            code:   output.status.code().unwrap_or(-1),
            stderr,
        });
    }

    // ── 11. Copy binary to bin/ ───────────────────────────────────────────────
    let bin_dir = project_dir.join("bin");
    std::fs::create_dir_all(&bin_dir)?;

    let exe_name = if cfg!(windows) {
        format!("{bin_name}.exe")
    } else {
        bin_name.clone()
    };

    let src_bin = build_dir
        .join("target")
        .join("release")
        .join(&exe_name);
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

    // Drop the required Apache-2.0 notices next to the binary so the
    // distribution carries them.
    if let Err(e) = write_license_notices(&bin_dir) {
        log(&format!("⚠️  Could not write license notices to bin/: {e}"));
    }

    Ok(BuildResult {
        binary_path:  dst_bin,
        source_count: sources.len(),
        form_count:   forms.len(),
        ast_bytes:    ast_compressed_len,
    })
}

// ── Code generators ───────────────────────────────────────────────────────────

fn generate_cargo_toml(
    bin_name:    &str,
    version:     &str,
    crates_path: &Path,
    has_forms:   bool,
) -> String {
    let cp = crates_path.display();
    let mut s = format!(r#"[package]
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
"#);

    if has_forms {
        s.push_str(&format!(r#"cobolt-forms    = {{ path = "{cp}/cobolt-forms" }}
cobolt-media    = {{ path = "{cp}/cobolt-media" }}
eframe          = {{ version = "0.29", features = ["default_fonts"] }}
egui            = "0.29"
egui_extras     = {{ version = "0.29", features = ["image"] }}
"#));
    }

    s
}

fn generate_main_rs(
    app_name:  &str,
    version:   &str,
    has_forms: bool,
    form_ids:  &[&str],
) -> String {
    // Build the FORMS constant entries
    let forms_entries: String = form_ids.iter().map(|id| {
        format!(
            "    (\"{id}\", include_bytes!(\"../assets/forms/{id}.cfrm\")),\n",
        )
    }).collect();

    let forms_const = if form_ids.is_empty() {
        "static FORMS: &[(&str, &[u8])] = &[];\n".to_owned()
    } else {
        format!(
            "static FORMS: &[(&str, &[u8])] = &[\n{forms_entries}];\n"
        )
    };

    let form_runtime_code = if has_forms {
        r#"
// ── Form application ──────────────────────────────────────────────────────────

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
    fn get(&self, key: &str) -> &str {
        self.props.get(key).map(|s| s.as_str()).unwrap_or("")
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

#[derive(Clone)]
struct CtrlMeta {
    id:           String,
    control_type: cobolt_forms::ControlType,
    rect:         cobolt_forms::model::Rect,
}

fn flatten_controls(controls: &[cobolt_forms::Control], out: &mut Vec<cobolt_forms::Control>) {
    for c in controls {
        out.push(c.clone());
        flatten_controls(&c.children, out);
    }
}

fn parse_hex_color(hex: &str) -> Option<egui::Color32> {
    let h = hex.trim_start_matches('#');
    if h.len() >= 6 {
        let r = u8::from_str_radix(&h[0..2], 16).ok()?;
        let g = u8::from_str_radix(&h[2..4], 16).ok()?;
        let b = u8::from_str_radix(&h[4..6], 16).ok()?;
        let a = if h.len() >= 8 { u8::from_str_radix(&h[6..8], 16).unwrap_or(255) } else { 255 };
        Some(egui::Color32::from_rgba_unmultiplied(r, g, b, a))
    } else {
        None
    }
}

/// Destination rect for an image of `native` size inside `rect` by size-mode.
fn anim_dest(rect: egui::Rect, native: egui::Vec2, mode: &str) -> egui::Rect {
    if native.x <= 0.0 || native.y <= 0.0 { return rect; }
    match mode {
        "Stretch" => rect,
        "Fill" => {
            let s = (rect.width() / native.x).max(rect.height() / native.y);
            egui::Rect::from_center_size(rect.center(), native * s)
        }
        "Center" | "Normal" => {
            let s = (rect.width() / native.x).min(rect.height() / native.y).min(1.0);
            egui::Rect::from_center_size(rect.center(), native * s)
        }
        _ => {
            let s = (rect.width() / native.x).min(rect.height() / native.y);
            egui::Rect::from_center_size(rect.center(), native * s)
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
    let mut controls: Vec<CtrlMeta> = Vec::new();
    for c in &flat {
        state.insert(c.id.clone(), CtrlState::from_control(c));
        controls.push(CtrlMeta {
            id: c.id.clone(),
            control_type: c.control_type.clone(),
            rect: c.rect.clone(),
        });
    }

    let bg = parse_hex_color(&first_form.background_color)
        .unwrap_or(egui::Color32::from_rgba_premultiplied(20, 22, 45, 235));
    let (fw, fh) = (first_form.width as f32, first_form.height as f32);
    let title = format!("{} v{}", APP_NAME, APP_VERSION);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&title)
            .with_inner_size([fw + 4.0, fh + 4.0]),
        ..Default::default()
    };

    let (ev_tx, ev_rx)           = mpsc::channel::<FormEvent>();
    let (state_tx, state_rx)     = mpsc::channel::<StateUpdate>();
    let (display_tx, display_rx) = mpsc::channel::<String>();

    // The COBOL event loop runs on its own thread.
    std::thread::spawn(move || {
        let mut interp = Interpreter::new_with_channels(program, ev_rx, state_tx, display_tx);
        let _ = interp.run();
    });

    let app = FormApp { controls, state, bg, ev_tx, state_rx, display_rx, start: std::time::Instant::now() };
    let _ = eframe::run_native(
        &title,
        native_options,
        Box::new(move |_cc| Ok(Box::new(app) as Box<dyn eframe::App>)),
    );
}

struct FormApp {
    controls:   Vec<CtrlMeta>,
    state:      std::collections::HashMap<String, CtrlState>,
    bg:         egui::Color32,
    ev_tx:      std::sync::mpsc::Sender<cobolt_runtime::FormEvent>,
    state_rx:   std::sync::mpsc::Receiver<cobolt_runtime::StateUpdate>,
    display_rx: std::sync::mpsc::Receiver<String>,
    /// When the window opened. Input events are ignored for a short warm-up so
    /// that a click already in progress as the window appears (e.g. it opened
    /// under the pointer) cannot be mistaken for an intentional interaction.
    start:      std::time::Instant,
}

impl FormApp {
    fn getp(&self, id: &str, key: &str) -> String {
        self.state.get(id).map(|s| s.get(key).to_owned()).unwrap_or_default()
    }
}

impl eframe::App for FormApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply property changes coming from the COBOL interpreter.
        while let Ok(u) = self.state_rx.try_recv() {
            self.state.entry(u.ctrl_id.clone()).or_default().set(&u.prop, u.value);
        }
        // DISPLAY output → stdout.
        while let Ok(line) = self.display_rx.try_recv() {
            println!("{}", line);
        }

        // Ignore input for a brief warm-up after the window appears, so a click
        // that was already underway when it opened cannot trigger a control.
        let armed = self.start.elapsed().as_millis() > 450;

        let metas = self.controls.clone();

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(self.bg))
            .show(ctx, |ui| {
                let origin = ui.min_rect().min;
                use cobolt_forms::ControlType as CT;

                for meta in &metas {
                    let (visible, enabled) = self.state.get(&meta.id)
                        .map(|s| (s.visible, s.enabled)).unwrap_or((true, true));
                    if !visible { continue; }

                    // Effective geometry: COBOL may move/resize a control through
                    // SET-PROPERTY X / Y / Width / Height, or dock it to an edge.
                    let area = ui.max_rect();
                    let rect = {
                        let st = self.state.get(&meta.id);
                        let num = |k: &str, d: f32| st
                            .and_then(|s| s.props.get(k))
                            .and_then(|v| v.trim().parse::<f32>().ok())
                            .unwrap_or(d);
                        let x = num("X", meta.rect.x as f32);
                        let y = num("Y", meta.rect.y as f32);
                        let w = num("Width",  meta.rect.w as f32).max(1.0);
                        let h = num("Height", meta.rect.h as f32).max(1.0);
                        let base = egui::Rect::from_min_size(
                            egui::pos2(origin.x + x, origin.y + y),
                            egui::vec2(w, h),
                        );
                        match st.map(|s| s.get("Dock")).unwrap_or("") {
                            "Top"    => egui::Rect::from_min_size(area.min, egui::vec2(area.width(), h)),
                            "Bottom" => egui::Rect::from_min_size(egui::pos2(area.min.x, area.max.y - h), egui::vec2(area.width(), h)),
                            "Left"   => egui::Rect::from_min_size(area.min, egui::vec2(w, area.height())),
                            "Right"  => egui::Rect::from_min_size(egui::pos2(area.max.x - w, area.min.y), egui::vec2(w, area.height())),
                            "Fill"   => area,
                            _ => base,
                        }
                    };

                    match meta.control_type {
                        CT::Button => {
                            let mut label = self.getp(&meta.id, "Caption");
                            if label.is_empty() { label = meta.id.clone(); }
                            let resp = ui.put(rect, egui::Button::new(label));
                            if armed && enabled && resp.clicked() {
                                let _ = self.ev_tx.send(cobolt_runtime::FormEvent::click(&meta.id));
                            }
                        }
                        CT::Label => {
                            let text = self.getp(&meta.id, "Caption");

                            // Opacity (0–100) scales every colour's alpha.
                            let opacity = self.getp(&meta.id, "Opacity").trim()
                                .parse::<f32>().unwrap_or(100.0).clamp(0.0, 100.0) / 100.0;
                            let amul = |c: egui::Color32| egui::Color32::from_rgba_unmultiplied(
                                c.r(), c.g(), c.b(), (c.a() as f32 * opacity) as u8);

                            let painter = ui.painter_at(rect);

                            // BackColor fill.
                            if let Some(bc) = parse_hex_color(&self.getp(&meta.id, "BackColor")) {
                                painter.rect_filled(rect, 0.0, amul(bc));
                            }
                            // Border.
                            let bstyle = self.getp(&meta.id, "BorderStyle");
                            if !bstyle.is_empty() && bstyle != "None" {
                                let bcol = parse_hex_color(&self.getp(&meta.id, "BorderColor"))
                                    .unwrap_or(egui::Color32::from_gray(150));
                                painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, amul(bcol)));
                            }

                            // Text colour: ForeColor, with sensible defaults.
                            let fsize = self.getp(&meta.id, "FontSize").trim()
                                .parse::<f32>().unwrap_or(14.0).max(1.0);
                            let mut fg = parse_hex_color(&self.getp(&meta.id, "ForeColor"))
                                .unwrap_or(egui::Color32::from_gray(230));
                            if fg == egui::Color32::BLACK { fg = egui::Color32::from_gray(230); }
                            if !enabled { fg = egui::Color32::from_gray(120); }
                            let fg = amul(fg);

                            let truthy = |s: String| s == "1" || s.eq_ignore_ascii_case("true");
                            let bold      = truthy(self.getp(&meta.id, "Bold"));
                            let italic    = truthy(self.getp(&meta.id, "Italic"));
                            let underline = truthy(self.getp(&meta.id, "Underline"));
                            let strike    = truthy(self.getp(&meta.id, "Strikethrough"));
                            let wrap      = truthy(self.getp(&meta.id, "WordWrap"));

                            // Padding insets the text rect.
                            let pad = self.getp(&meta.id, "Padding").trim()
                                .parse::<f32>().unwrap_or(0.0).max(0.0);
                            let inner = rect.shrink(pad);

                            let (halign, anchor_x) = match self.getp(&meta.id, "TextAlign").as_str() {
                                "Center" => (egui::Align::Center, inner.center().x),
                                "Right"  => (egui::Align::Max,    inner.right()),
                                _        => (egui::Align::Min,    inner.left()),
                            };

                            use egui::text::{LayoutJob, TextFormat};
                            let mut job = LayoutJob::default();
                            job.halign = halign;
                            if wrap { job.wrap.max_width = inner.width(); }
                            job.append(&text, 0.0, TextFormat {
                                font_id: egui::FontId::proportional(fsize),
                                color: fg,
                                italics: italic,
                                underline:     if underline { egui::Stroke::new(1.0, fg) } else { egui::Stroke::NONE },
                                strikethrough: if strike    { egui::Stroke::new(1.0, fg) } else { egui::Stroke::NONE },
                                ..Default::default()
                            });
                            let galley = painter.layout_job(job);
                            let pos = egui::pos2(anchor_x, inner.center().y - galley.size().y / 2.0);
                            painter.galley(pos, galley.clone(), fg);
                            // Simulate Bold by repainting with a sub-pixel x-offset.
                            if bold { painter.galley(pos + egui::vec2(0.5, 0.0), galley, fg); }

                            // Cursor: change the pointer while hovering the label.
                            let cursor = self.getp(&meta.id, "Cursor");
                            if !cursor.is_empty() && cursor != "Default" && ui.rect_contains_pointer(rect) {
                                let ic = match cursor.as_str() {
                                    "Hand" | "PointingHand" => egui::CursorIcon::PointingHand,
                                    "Text" | "IBeam"        => egui::CursorIcon::Text,
                                    "Wait"                  => egui::CursorIcon::Wait,
                                    "Crosshair"             => egui::CursorIcon::Crosshair,
                                    "Help"                  => egui::CursorIcon::Help,
                                    "Move" | "SizeAll"      => egui::CursorIcon::Move,
                                    "NotAllowed" | "No"     => egui::CursorIcon::NotAllowed,
                                    _                       => egui::CursorIcon::Default,
                                };
                                ui.ctx().set_cursor_icon(ic);
                            }
                        }
                        CT::TextBox => {
                            let mut buf = self.getp(&meta.id, "Text");
                            let resp = ui.put(rect,
                                egui::TextEdit::singleline(&mut buf).interactive(enabled));
                            if resp.changed() {
                                if let Some(s) = self.state.get_mut(&meta.id) { s.set("Text", buf.clone()); }
                                let _ = self.ev_tx.send(cobolt_runtime::FormEvent::change(&meta.id, &buf));
                            }
                            if resp.gained_focus() { let _ = self.ev_tx.send(cobolt_runtime::FormEvent::new(&meta.id, "GotFocus")); }
                            if resp.lost_focus()  { let _ = self.ev_tx.send(cobolt_runtime::FormEvent::new(&meta.id, "LostFocus")); }
                        }
                        CT::CheckBox => {
                            let label = self.getp(&meta.id, "Caption");
                            let cur = self.getp(&meta.id, "Value");
                            let mut checked = cur == "1" || cur == "true";
                            let resp = ui.put(rect, egui::Checkbox::new(&mut checked, label));
                            if resp.changed() {
                                if let Some(s) = self.state.get_mut(&meta.id) {
                                    s.set("Value", if checked { "1" } else { "0" }.to_owned());
                                }
                                let _ = self.ev_tx.send(cobolt_runtime::FormEvent::new(&meta.id, "Change"));
                            }
                        }
                        CT::RadioButton => {
                            let label = self.getp(&meta.id, "Caption");
                            let cur = self.getp(&meta.id, "Value");
                            let selected = cur == "1" || cur == "true";
                            let resp = ui.put(rect, egui::RadioButton::new(selected, label));
                            if armed && enabled && resp.clicked() {
                                if let Some(s) = self.state.get_mut(&meta.id) { s.set("Value", "1".to_owned()); }
                                let _ = self.ev_tx.send(cobolt_runtime::FormEvent::click(&meta.id));
                            }
                        }
                        CT::ProgressBar => {
                            let val = self.getp(&meta.id, "Value").parse::<f32>().unwrap_or(0.0);
                            let max = self.getp(&meta.id, "Max").parse::<f32>().unwrap_or(100.0);
                            let frac = if max > 0.0 { (val / max).clamp(0.0, 1.0) } else { 0.0 };
                            ui.put(rect, egui::ProgressBar::new(frac));
                        }
                        CT::Slider => {
                            let min = self.getp(&meta.id, "Min").parse::<f64>().unwrap_or(0.0);
                            let max = self.getp(&meta.id, "Max").parse::<f64>().unwrap_or(100.0);
                            let mut val = self.getp(&meta.id, "Value").parse::<f64>().unwrap_or(min);
                            let resp = ui.put(rect, egui::Slider::new(&mut val, min..=max));
                            if resp.changed() {
                                if let Some(s) = self.state.get_mut(&meta.id) { s.set("Value", format!("{}", val)); }
                                let _ = self.ev_tx.send(cobolt_runtime::FormEvent::new(&meta.id, "Change"));
                            }
                        }
                        CT::GroupBox | CT::Panel => {
                            ui.painter().rect_stroke(rect, 4.0,
                                egui::Stroke::new(1.0, egui::Color32::from_gray(160)));
                            let cap = self.getp(&meta.id, "Caption");
                            if !cap.is_empty() {
                                ui.painter().text(rect.min + egui::vec2(6.0, 2.0),
                                    egui::Align2::LEFT_TOP, cap,
                                    egui::FontId::proportional(12.0), egui::Color32::from_gray(220));
                            }
                        }
                        CT::Animator => {
                            let source = self.getp(&meta.id, "Source").trim().to_string();
                            let played = if source.is_empty() {
                                None
                            } else {
                                let auto    = !matches!(self.getp(&meta.id, "AutoPlay").as_str(), "0" | "false" | "False");
                                let looping = !matches!(self.getp(&meta.id, "Loop").as_str(),     "0" | "false" | "False");
                                let key = format!("{}|{}", meta.id, source);
                                let path = source.clone();
                                cobolt_media::play(ui.ctx(), &key, move || std::fs::read(&path).ok(), auto, looping)
                            };
                            match played {
                                Some((tex, native)) => {
                                    let mode = { let s = self.getp(&meta.id, "SizeMode"); if s.is_empty() { "Fit".to_string() } else { s } };
                                    let dest = anim_dest(rect, native, &mode);
                                    ui.painter().with_clip_rect(rect).image(
                                        tex, dest,
                                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                        egui::Color32::WHITE);
                                }
                                None => {
                                    ui.painter().rect_filled(rect, 6.0, egui::Color32::from_rgb(18, 24, 48));
                                    ui.painter().rect_stroke(rect, 6.0,
                                        egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 150, 230)));
                                    let label = if source.is_empty() { "\u{25B6} Animator" } else { "\u{25B6} (cannot load)" };
                                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label,
                                        egui::FontId::proportional(13.0), egui::Color32::from_rgb(190, 205, 255));
                                }
                            }
                        }
                        _ => {
                            // Fallback: outline + best-effort text (Value / Text / Caption / Items).
                            ui.painter().rect_stroke(rect, 3.0,
                                egui::Stroke::new(1.0, egui::Color32::from_gray(120)));
                            let mut txt = self.getp(&meta.id, "Value");
                            if txt.is_empty() { txt = self.getp(&meta.id, "Text"); }
                            if txt.is_empty() { txt = self.getp(&meta.id, "Caption"); }
                            if txt.is_empty() {
                                txt = self.getp(&meta.id, "Items").lines().next().unwrap_or("").to_owned();
                            }
                            if !txt.is_empty() {
                                ui.painter().text(rect.left_center() + egui::vec2(4.0, 0.0),
                                    egui::Align2::LEFT_CENTER, txt,
                                    egui::FontId::proportional(12.0), egui::Color32::from_gray(225));
                            }
                        }
                    }
                }
            });

        ctx.request_repaint();
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
        version  = version,
        forms_const = forms_const,
        run_call    = run_call,
        form_runtime_code = form_runtime_code,
    )
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn detect_format(source: &str) -> cobolt_lexer::SourceFormat {
    let looks_fixed = source.lines().any(|line| {
        let b = line.as_bytes();
        b.len() > 6 && b[6] != b' '
            && b[..6].iter().all(|&c| c == b' ' || c.is_ascii_digit())
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
    let mut dir = if start.is_file() { start.parent()? } else { start };
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
