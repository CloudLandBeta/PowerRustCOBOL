// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Main application state and egui update loop.
//!
//! The main window always shows the **Code workspace**:
//!   Project explorer (left) | Code editor (centre) | Output (bottom)
//!
//! Each open form designer lives in its **own OS window** opened via
//! `ctx.show_viewport_immediate`.  Windows can be open simultaneously,
//! each with its own toolbox, properties inspector and undo stack.
//!
//! A `CoboltProject` (cobolt.toml) can be open alongside either workspace,
//! tracking all source files, forms, and assets and enabling one-click zip
//! packaging.

use std::path::{Path, PathBuf};

use egui::{Context, Key, KeyboardShortcut, Modifiers, Vec2, ViewportBuilder, ViewportId};

use cobolt_forms::{load_form, save_form, Form};
// The run/preview per-control draw loops are gone — the unified render engine
// owns control rendering (spec 017) — so only the form-level background-image
// loader remains in the IDE here.
use cobolt_codegen::{generate, generate_indexed};
use cobolt_compiler::{build_project, BuildOptions};
use cobolt_forms::paint::load_image_texture;
use cobolt_indexed::{
    load_indexed, record_to_text, resolve_path, save_indexed, text_to_record, IndexedDefinition,
};
use cobolt_runtime::indexed_ide::{compare_schema, SchemaDrift};
use cobolt_runtime::indexed_import::{definition_from_inspect, inspect_any_path};

use crate::form_runtime::FormRuntime;
use crate::i18n::{Language, Tr};
use crate::panels::debugger::DebuggerPanel;
use crate::panels::{
    designer::DesignerPanel,
    editor::EditorPanel,
    forms_list::FormsListPanel,
    indexed_editor::{IndexedEditorPanel, IndexedSelection, RawDialogResult, StructureAction},
    indexed_grid::{GridAction, IndexedGridPanel},
    indexed_new_dialog::{NewIndexedAction, NewIndexedDialog},
    indexed_properties::PropertyEdit,
    output::OutputPanel,
    project::{ProjectPanel, ProjectPanelEvent},
    toolbar::{self, ToolbarAction},
};
use crate::project_model::{
    load_project, package_project, relative_to, save_project, CoboltProject, ElementStatus,
    FileKind, UserControlDef,
};
use crate::runner::{DebugRunner, RunMsg, Runner};
use crate::version::VERSION;
use cobolt_runtime::DebugCmd;
use rand::Rng;

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) struct DesignerClipboard {
    pub(crate) controls: Vec<cobolt_forms::Control>,
    pub(crate) source_form: String,
    pub(crate) origin_x: i32,
    pub(crate) origin_y: i32,
}

// ── Dialog state ──────────────────────────────────────────────────────────────

/// State for the "Report Bug" dialog available in both the IDE and designer.
struct ReportBugDialog {
    open: bool,
    /// Short one-line title of the problem.
    title: String,
    /// Longer description (steps to reproduce, what went wrong, etc.)
    description: String,
    /// Which surface the bug was reported from (e.g. "IDE Editor", "Form Designer").
    component: String,
    /// Feedback shown after submission ("Saved." or an error).
    feedback: Option<String>,
}

impl ReportBugDialog {
    fn new() -> Self {
        Self {
            open: false,
            title: String::new(),
            description: String::new(),
            component: "IDE".into(),
            feedback: None,
        }
    }

    /// Open the dialog pre-filled with the given component name.
    fn open_for(&mut self, component: impl Into<String>) {
        self.open = true;
        self.component = component.into();
        self.title.clear();
        self.description.clear();
        self.feedback = None;
    }

    /// Write the bug report to BUGS.md and return Ok or an error string.
    fn submit(&mut self, bugs_path: &std::path::Path) -> Result<(), String> {
        if self.title.trim().is_empty() {
            return Err("Please enter a title for the bug.".into());
        }

        // Read existing file.
        let existing = std::fs::read_to_string(bugs_path).unwrap_or_default();

        // Find the next BUG-NNN number.
        let last_id = existing
            .lines()
            .filter_map(|l| {
                let col = l.split('|').nth(1)?.trim();
                col.strip_prefix("BUG-").and_then(|n| n.parse::<u32>().ok())
            })
            .max()
            .unwrap_or(0);
        let next_id = last_id + 1;

        let today = {
            // Use a simple date string; chrono not in scope so derive from SystemTime.
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let days = secs / 86400;
            // Approximate calendar date (good enough for a bug-tracker timestamp).
            let y = 1970 + days / 365;
            let d = days % 365;
            let m = (d / 30) + 1;
            let dd = (d % 30) + 1;
            format!("{y:04}-{m:02}-{dd:02}")
        };

        let component = self.component.replace('|', "∣");
        let title = self.title.trim().replace('|', "∣");
        let desc = self.description.trim().replace('|', "∣");
        let summary = if desc.is_empty() {
            title.clone()
        } else {
            format!("{title} — {desc}")
        };
        let summary = if summary.len() > 100 {
            format!("{}…", &summary[..97])
        } else {
            summary
        };

        let new_row =
            format!("| BUG-{next_id:03} | {today} | `{component}` | `MANUAL` | {summary} |\n");

        // Inject into the Open Bugs table.
        let placeholder = "_No open bugs — all clear! ✅_";
        let updated = if existing.contains(placeholder) {
            existing.replace(placeholder, &new_row.trim_end())
        } else if existing.contains("| ID | Detected |") {
            // Append after the last existing open-bug row (before the next ---)
            let sep = "\n---";
            if let Some(pos) = existing.find(sep) {
                let (before, after) = existing.split_at(pos);
                format!("{before}\n{new_row}{after}")
            } else {
                format!("{existing}\n{new_row}")
            }
        } else {
            format!("{existing}\n{new_row}")
        };

        std::fs::write(bugs_path, updated).map_err(|e| e.to_string())?;
        Ok(())
    }
}

struct NewFormDialog {
    open: bool,
    form_name: String,
    title: String,
    width: String,
    height: String,
}

impl NewFormDialog {
    fn new() -> Self {
        Self {
            open: false,
            form_name: "MAIN-FORM".into(),
            title: "My Form".into(),
            width: "640".into(),
            height: "480".into(),
        }
    }
}

struct NewProjectDialog {
    open: bool,
    name: String,
    version: String,
    main: String,
}

impl NewProjectDialog {
    fn new() -> Self {
        Self {
            open: false,
            name: "MyApp".into(),
            version: "1.0.0".into(),
            main: "src/main.cbl".into(),
        }
    }
}

// ── CoboltApp ─────────────────────────────────────────────────────────────────

pub struct CoboltApp {
    // Code workspace
    project: ProjectPanel,
    editor: EditorPanel,
    output: OutputPanel,
    runner: Runner,
    forms_list: FormsListPanel,

    // Open form designers (each lives in its own viewport window)
    designers: Vec<(PathBuf, DesignerPanel)>,
    #[allow(dead_code)]
    pub(crate) clipboard: Option<DesignerClipboard>,

    // Grid browser viewports keyed by `.cidx` path
    indexed_grids: Vec<(PathBuf, IndexedGridState)>,

    // Inline form/control inspector shown in the Main Pane (from the project tree)
    inspect: Option<InspectState>,

    // Inline indexed-file inspector in the Main Pane
    indexed_inspect: Option<IndexedInspectState>,

    /// Indexed files that were created or last edited via the raw COBOL text
    /// editor. For these files we keep the editor visible / preferred and
    /// do not offer (or lock down) the properties pane for structural changes.
    raw_preferred_indexed: std::collections::HashSet<PathBuf>,

    // Content hash of each file at its last successful/failed check (for the tree
    // "semaphore": a file edited since its last check shows yellow again).
    checked: std::collections::HashMap<PathBuf, u64>,

    // Running form instances — each has its own OS window (Phase 6)
    form_runtimes: Vec<FormRuntime>,

    // Debugger (Phase 7)
    debug_runner: DebugRunner,
    debugger: DebuggerPanel,
    debug_active: bool,

    // Project model
    cobolt_project: Option<CoboltProject>,
    project_path: Option<PathBuf>,
    pending_user_control_delete: Option<String>,

    // 007 Form themes — discovered asset packs (id → pack), loaded once.
    theme_packs:
        std::collections::HashMap<String, std::sync::Arc<cobolt_forms::theme_pack::ThemePack>>,
    theme_packs_loaded: bool,

    /// The in-pane project Settings form (built when a project loads). Shown in
    /// the Main Pane on start-up and whenever the project (top tree node) is
    /// clicked.
    settings_form: Option<crate::panels::settings_form::SettingsForm>,
    /// Whether the Main Pane is currently showing the Settings form.
    show_project_settings: bool,
    /// Set while a "save unsaved settings before closing?" dialog is shown.
    settings_close_confirm: bool,
    /// Cached background-image texture, keyed by the resolved absolute path.
    bg_texture: Option<(PathBuf, egui::TextureHandle)>,

    /// Global AI-assistant configuration (cloud LLM for the code editor).
    /// Stored outside the project so the API key never lands in a repo.
    llm: crate::llm::LlmConfig,
    /// In-flight "Test connection" request from the settings dialog.
    llm_test_rx: Option<std::sync::mpsc::Receiver<crate::llm::LlmResponse>>,
    /// Last test-connection result/status line.
    llm_test_status: Option<String>,

    // Dialog state
    new_form: NewFormDialog,
    new_indexed: NewIndexedDialog,
    new_project: NewProjectDialog,

    // Cross-window pending actions
    /// A file path waiting to be opened in the code editor (set by a designer
    /// window's "Generate COBOL" action, picked up by the main window).
    pending_open_in_editor: Option<PathBuf>,

    /// A COBOL paragraph name to scroll to in the editor once the queued file has
    /// been opened (set by double-clicking an event row; see `jump_to_event_code`).
    pending_goto_paragraph: Option<String>,

    /// Track whether glass visuals have been applied (applied once on first frame).
    glass_visuals_applied: bool,

    /// Currently selected UI language.
    lang: Language,

    // State for the cycling welcome quotes on the initial screen (no project)
    welcome_quote_index: usize,
    welcome_quote_start_time: f64,

    /// Report Bug dialog (shown from both IDE toolbar and designer toolbar).
    report_bug: ReportBugDialog,
    /// Whether the Help → About window is open.
    about_open: bool,
    /// Documentation viewer window (Help → Documentation).
    doc_viewer: crate::panels::doc_viewer::DocViewer,
    /// Non-empty while the "Form saved" alert should be displayed.
    save_alert_msg: Option<String>,
    /// Which surface owns the save alert: `Some(idx)` = the designer viewport at
    /// `idx` (so the alert is not hidden behind it), `None` = the main IDE window.
    save_alert_designer: Option<usize>,

    /// Pending binary build result channel (Phase 11).
    pending_build_rx:
        Option<std::sync::mpsc::Receiver<Result<cobolt_compiler::BuildResult, String>>>,
    /// Streamed build-phase progress (fraction + message) for the Building modal.
    pending_build_progress: Option<std::sync::mpsc::Receiver<cobolt_compiler::BuildProgress>>,
    /// Latest build phase: (fraction 0..1, message).
    build_phase: (f32, String),

    /// Which app-level file dialog (if any) is currently open; its result is
    /// applied by `apply_file_result` once the async picker returns.
    pending_file: Option<FileRequest>,

    /// Clone of the root egui context, so async file dialogs can wake the UI
    /// from their worker thread when the user finishes picking.
    egui_ctx: egui::Context,
}

/// An app-level file dialog awaiting the user, identifying what to do with the
/// chosen path. File dialogs are opened asynchronously (see `crate::file_dialog`)
/// because a synchronous one nests the OS event loop and aborts winit 0.30.
#[derive(Clone)]
enum FileRequest {
    OpenCobol,
    CreateProject,
    OpenProject,
    SaveProject,
    PackageProject,
    AddFile(FileKind),
    OpenForm,
    NewForm(Box<cobolt_forms::Form>),
    /// Pick a background image for the IDE appearance settings.
    PickBackgroundImage,
}

/// The shared egui key for the single app-level file dialog.
const APP_FILE_KEY: &str = "app-file-dialog";

/// Standard project sub-folders — one per category plus working/build folders.
/// Created when a project is made, and back-filled (if missing) when one is opened.
const PROJECT_FOLDERS: &[&str] = &[
    "src",
    "forms",
    "indexed",
    "generated",
    "assets",
    "docs",
    "bin",
    "debug",
    "temp",
    "dist",
    "data",
    "copybooks",
];

/// Inline indexed-file inspector in the Main Pane.
struct IndexedInspectState {
    path: PathBuf,
    def: IndexedDefinition,
    panel: IndexedEditorPanel,
    dirty: bool,
    /// If true, this file was defined (or last significantly edited) via the
    /// raw COBOL-85 text editor. Structural edits via the properties/tree pane
    /// are discouraged/locked; the editor remains the primary/visible surface.
    prefer_raw_editor: bool,
}

/// Grid browser state for one `.cidx`.
struct IndexedGridState {
    panel: IndexedGridPanel,
    def: IndexedDefinition,
    close_requested: bool,
}

/// Inline form/control inspector shown in the Main Pane (from the project tree).
/// Holds a transient `DesignerPanel` so it reuses the designer's property-edit
/// machinery without opening a designer window.
struct InspectState {
    path: PathBuf,
    ctrl_id: Option<String>,
    designer: DesignerPanel,
    /// `.cfrm` modification time of the form currently held in `designer`.
    /// Used to live-refresh the Main-Pane inspector when the form is changed
    /// elsewhere (e.g. saved from the Designer window) so edits reflect back.
    mtime: Option<std::time::SystemTime>,
}

impl InspectState {
    /// Reload the form from disk if the `.cfrm` changed since we last read it
    /// (e.g. saved from the Designer window). Returns true when reloaded.
    fn reload_if_stale(&mut self) -> bool {
        let disk = file_mtime(&self.path);
        let stale = match (disk, self.mtime) {
            (Some(d), Some(cur)) => d > cur,
            (Some(_), None) => true,
            _ => false,
        };
        if !stale {
            return false;
        }
        if let Ok(form) = load_form(&self.path) {
            // Preserve the current selection if the control still exists.
            let keep = self.ctrl_id.clone();
            self.designer = DesignerPanel::new(form);
            self.mtime = disk;
            if let Some(id) = keep {
                if self.designer.form.find_control(&id).is_some() {
                    self.ctrl_id = Some(id);
                } else {
                    self.ctrl_id = None;
                }
            }
            true
        } else {
            false
        }
    }
}

/// Last-modified time of a file, if available.
fn file_mtime(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

impl CoboltApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        cc.egui_ctx.set_style(style);
        cc.egui_ctx.set_fonts(crate::fonts::base_font_definitions());
        // Image loaders (PNG/etc.) — needed by the Documentation viewer's
        // Markdown image rendering.
        egui_extras::install_image_loaders(&cc.egui_ctx);

        Self {
            project: ProjectPanel::new(),
            editor: EditorPanel::new(),
            output: OutputPanel::new(),
            runner: Runner::new(),
            forms_list: FormsListPanel::new(),
            designers: Vec::new(),
            clipboard: None,
            indexed_grids: Vec::new(),
            inspect: None,
            indexed_inspect: None,
            raw_preferred_indexed: std::collections::HashSet::new(),
            checked: std::collections::HashMap::new(),
            form_runtimes: Vec::new(),
            debug_runner: DebugRunner::new(),
            debugger: DebuggerPanel::new(),
            debug_active: false,

            cobolt_project: None,
            project_path: None,
            pending_user_control_delete: None,
            theme_packs: std::collections::HashMap::new(),
            theme_packs_loaded: false,

            settings_form: None,
            show_project_settings: false,
            settings_close_confirm: false,
            bg_texture: None,
            llm: crate::llm::LlmConfig::load(),
            llm_test_rx: None,
            llm_test_status: None,

            new_form: NewFormDialog::new(),
            new_indexed: NewIndexedDialog::new(),
            new_project: NewProjectDialog::new(),

            pending_open_in_editor: None,
            pending_goto_paragraph: None,
            glass_visuals_applied: false,
            lang: Language::English,
            welcome_quote_index: 0,
            welcome_quote_start_time: 0.0,
            report_bug: ReportBugDialog::new(),
            about_open: false,
            doc_viewer: Default::default(),
            save_alert_msg: None,
            save_alert_designer: None,
            pending_build_rx: None,
            pending_build_progress: None,
            build_phase: (0.0, String::new()),
            pending_file: None,
            egui_ctx: cc.egui_ctx.clone(),
        }
    }

    // ── 007 Form themes ───────────────────────────────────────────────────────

    /// Locate the bundled `assets/themes` directory (exe-relative first, then the
    /// current working directory for `cargo run` from the repo root).
    fn themes_dir() -> PathBuf {
        if let Some(exe_dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        {
            let c = exe_dir.join("assets/themes");
            if c.is_dir() {
                return c;
            }
        }
        PathBuf::from("assets/themes")
    }

    /// Discover asset-pack themes once (id → pack).
    fn ensure_theme_packs(&mut self) {
        if self.theme_packs_loaded {
            return;
        }
        self.theme_packs_loaded = true;
        for pack in cobolt_forms::theme_pack::discover_packs(&Self::themes_dir()) {
            self.theme_packs
                .insert(pack.id.clone(), std::sync::Arc::new(pack));
        }
    }

    /// Publish the form-theme picker choices (Liquid Glass first, then discovered
    /// asset packs in catalog order) into egui temp storage for this frame.
    fn publish_theme_choices(&mut self, ctx: &Context) {
        self.ensure_theme_packs();
        let mut choices = vec![(
            cobolt_forms::theme::LIQUID_GLASS.to_owned(),
            "Liquid Glass".to_owned(),
        )];
        let mut packs: Vec<_> = self.theme_packs.values().collect();
        packs.sort_by(|a, b| a.id.cmp(&b.id));
        for p in packs {
            choices.push((p.id.clone(), p.display_name.clone()));
        }
        crate::theme_ui::publish(ctx, choices);
    }

    /// Resolve a form's effective theme (per-form override ?? project default ??
    /// Liquid Glass) to its asset pack. Returns `None` for Liquid Glass.
    fn resolve_theme_pack(
        &mut self,
        form_theme: Option<&str>,
    ) -> Option<std::sync::Arc<cobolt_forms::theme_pack::ThemePack>> {
        self.ensure_theme_packs();
        let proj_default = self
            .cobolt_project
            .as_ref()
            .and_then(|p| p.form_theme_default());
        let id = cobolt_forms::theme::resolve_theme_id(form_theme, proj_default);
        self.theme_packs.get(&id).cloned()
    }

    // ── Code workspace actions ────────────────────────────────────────────────

    fn do_run(&mut self) {
        self.regenerate_all_forms();
        self.regenerate_all_indexed_files();
        // Prefer the file in the editor; otherwise fall back to the open
        // project's main program, so Run always does something visible.
        let target = self
            .editor
            .active_source()
            .map(|(p, s)| (p.clone(), s.to_owned()))
            .or_else(|| self.project_main_source());
        let Some((path, source)) = target else {
            self.output.clear();
            self.output
                .push_status("Open a COBOL file, or open a project, to run.");
            return;
        };
        self.output.clear();
        self.output.push_status(format!(
            "── Running {} ──",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("?")
        ));
        self.editor.clear_diags();
        self.runner.start(path.display().to_string(), source);
    }

    /// The open project's main program as `(abs_path, source)`, if any.
    ///
    /// Prefers the declared `[project].main`; for a form-centric project whose
    /// main was never hand-written, falls back to the first generated form
    /// program (then any ordinary source) that exists on disk — so Run always
    /// has something to execute.
    fn project_main_source(&self) -> Option<(PathBuf, String)> {
        let proj = self.cobolt_project.as_ref()?;
        let root = self.project_path.as_ref()?.parent()?;
        let exists = |rel: &str| !rel.is_empty() && root.join(rel).is_file();

        let main_rel = if exists(&proj.project.main) {
            proj.project.main.clone()
        } else {
            proj.files
                .generated
                .iter()
                .chain(proj.files.sources.iter())
                .find(|rel| exists(rel))
                .cloned()?
        };
        let main_abs = root.join(&main_rel);
        let src = std::fs::read_to_string(&main_abs).ok()?;
        Some((main_abs, src))
    }

    fn do_stop(&mut self) {
        self.runner.stop();
        self.output.push_status("── Stop requested ──");
    }

    // ── Debugger (Phase 7) ────────────────────────────────────────────────────

    /// Start a debug session for the active COBOL file.
    ///
    /// Syncs breakpoints from the editor gutter, resets the debugger panel,
    /// and starts `DebugRunner` with `new_with_debug_channels()`.
    fn do_debug(&mut self) {
        self.regenerate_all_forms();
        self.regenerate_all_indexed_files();
        let Some((path, src)) = self.editor.active_source() else {
            return;
        };
        let path = path.clone();
        let source = src.to_owned();

        self.output.clear();
        self.output.push_status(format!(
            "── Debug {} ──",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("?")
        ));
        self.editor.clear_diags();
        self.debugger.reset();

        // Sync breakpoints from editor gutter into the shared set.
        {
            let bp_lines = self.editor.breakpoints_for(&path);
            let mut guard = self.debug_runner.breakpoints.lock().unwrap();
            guard.clear();
            for line in bp_lines {
                guard.insert(line);
            }
        }

        self.debug_runner.start(path.display().to_string(), source);
        self.debug_active = true;
    }

    // ── Form Runtime Engine (Phase 6) ─────────────────────────────────────────

    /// Launch a `FormRuntime` for the designer at `idx`.
    /// Saves + regenerates COBOL first so the interpreter always runs the
    /// latest version of the form.
    fn do_run_form(&mut self, idx: usize) {
        // Save the form and regenerate COBOL first (silently — Run should not
        // pop a "saved" alert).
        self.do_save_designer(idx);
        self.save_alert_msg = None;
        self.save_alert_designer = None;
        self.do_generate_cobol(idx);

        let form_path = self.designers[idx].0.clone();
        let form = self.designers[idx].1.form.clone();

        self.output.clear();
        self.output.push_status(format!(
            "── Running form {} ──",
            form_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
        ));

        // Kill any existing runtime for this form first.
        self.form_runtimes.retain_mut(|rt| {
            if rt.form_path == form_path {
                rt.stop();
                false
            } else {
                true
            }
        });

        let glass = self.designers[idx].1.glass_mode;
        match FormRuntime::launch(&form, form_path) {
            Ok(mut rt) => {
                rt.glass = glass;
                self.form_runtimes.push(rt);
            }
            Err(e) => {
                self.output
                    .push_status(format!("Error launching form: {e}"));
            }
        }
    }

    /// Set a tracked element's semaphore status (converts abs path → rel).
    fn set_element_status(&mut self, abs: &std::path::Path, s: ElementStatus) {
        if let Some(dir) = self.project_path.as_ref().and_then(|p| p.parent()) {
            if let Some(rel) = relative_to(abs, dir) {
                self.project.set_status(&rel, s);
            }
        }
    }

    /// Stable content hash for the change-since-check semaphore rule.
    fn content_hash(s: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
    }

    fn do_check(&mut self) {
        self.regenerate_all_forms();
        self.regenerate_all_indexed_files();
        let Some((path, src)) = self.editor.active_source() else {
            return;
        };
        let path = path.clone();
        let source = src.to_owned();
        self.output.clear();
        self.output.push_status(format!(
            "── Checking {} ──",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("?")
        ));
        self.editor.clear_diags();

        use crate::runner::{DiagMsg, DiagSeverity, RunMsg};
        use cobolt_lexer::{tokenize, SourceFormat};
        use cobolt_parser::parse;
        use cobolt_semantic::analyze;

        let fmt = if source.lines().any(|l| {
            let b = l.as_bytes();
            b.len() > 6 && b[6] != b' ' && b[..6].iter().all(|&c| c == b' ' || c.is_ascii_digit())
        }) {
            SourceFormat::Fixed
        } else {
            SourceFormat::Free
        };

        let tokens = tokenize(&source, fmt);
        let parse_result = parse(tokens);

        for d in &parse_result.diagnostics {
            use cobolt_parser::Severity as PSev;
            let sev = match d.severity {
                PSev::Error => DiagSeverity::Error,
                PSev::Warning => DiagSeverity::Warning,
            };
            let diag = DiagMsg {
                severity: sev,
                message: d.message.clone(),
                line: d.span.line,
                col: d.span.col,
            };
            self.output.push_msg(&RunMsg::Diagnostic(diag.clone()));
            self.editor.add_diag(&path, diag);
        }

        match parse_result.program {
            None => {
                self.output.push_msg(&RunMsg::Error(
                    "Parse failed — no program recovered.".to_owned(),
                ));
            }
            Some(prog) => {
                let sem = analyze(&prog);
                if parse_result.diagnostics.is_empty() && sem.diagnostics.is_empty() {
                    self.output.push_status("Check OK — no issues found.");
                }
                for d in &sem.diagnostics {
                    use cobolt_semantic::Severity;
                    let sev = match d.severity {
                        Severity::Error => DiagSeverity::Error,
                        Severity::Warning => DiagSeverity::Warning,
                        Severity::Info => DiagSeverity::Info,
                    };
                    let diag = DiagMsg {
                        severity: sev,
                        message: d.message.clone(),
                        line: d.span.line,
                        col: d.span.col,
                    };
                    self.output.push_msg(&RunMsg::Diagnostic(diag.clone()));
                    self.editor.add_diag(&path, diag);
                }
            }
        }

        // ── Update the tree semaphore for the checked file ────────────────────
        let had_error = self
            .editor
            .diags
            .get(&path)
            .map(|v| v.iter().any(|d| d.severity == DiagSeverity::Error))
            .unwrap_or(false);
        self.checked
            .insert(path.clone(), Self::content_hash(&source));
        self.set_element_status(
            &path,
            if had_error {
                ElementStatus::Failed
            } else {
                ElementStatus::Tested
            },
        );
    }

    fn do_open(&mut self) {
        self.begin_file_dialog(
            FileRequest::OpenCobol,
            crate::file_dialog::DialogSpec::open()
                .filter("COBOL", &["cbl", "cob", "cpy"])
                .filter("All files", &["*"]),
        );
    }

    fn do_save(&mut self) {
        if let Err(e) = self.editor.save_active() {
            self.output.push_status(format!("Save failed: {e}"));
        }
    }

    // ── Project actions ───────────────────────────────────────────────────────

    fn do_new_project(&mut self) {
        self.new_project.open = true;
    }

    fn create_new_project(&mut self) {
        // The manifest is named after the project (e.g. "Inventory System.toml")
        // rather than a fixed "cobolt.toml", so the file is self-describing.
        let file_name = format!("{}.toml", sanitize_file_stem(&self.new_project.name));
        self.begin_file_dialog(
            FileRequest::CreateProject,
            crate::file_dialog::DialogSpec::save()
                .filter("RustCOBOL Project", &["toml"])
                .file_name(&file_name),
        );
    }

    /// Finish creating a new project once the user has chosen `cobolt.toml`.
    fn create_new_project_at(&mut self, path: PathBuf) {
        let mut proj =
            CoboltProject::new(self.new_project.name.clone(), self.new_project.main.clone());
        proj.project.version = self.new_project.version.clone();

        match save_project(&proj, &path) {
            Ok(()) => {
                let dir = path.parent().map(|p| p.to_owned());
                self.cobolt_project = Some(proj);
                self.project_path = Some(path);
                if let Some(dir) = dir {
                    // Create the standard project sub-folders: one per category
                    // (Common Code / Forms / Generated Code / Assets / Docs) plus
                    // build/debug/temp working folders and `dist/` (a future
                    // self-contained bundle — binary + assets + libs — for running
                    // the project on a machine without PowerRustCOBOL installed).
                    for sub in PROJECT_FOLDERS {
                        if let Err(e) = std::fs::create_dir_all(dir.join(sub)) {
                            self.output
                                .push_status(format!("Could not create {sub}/: {e}"));
                        }
                    }
                    // Scaffold a runnable starter main program so the project can
                    // be Run immediately. Track it under Common Code and open it.
                    let proj_name = self.cobolt_project.as_ref().unwrap().project.name.clone();
                    let main_rel = self.cobolt_project.as_ref().unwrap().project.main.clone();
                    let main_path = dir.join(&main_rel);
                    if !main_path.exists() {
                        if let Some(parent) = main_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let prog: String = main_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("MAIN")
                            .chars()
                            .map(|c| {
                                if c.is_ascii_alphanumeric() {
                                    c.to_ascii_uppercase()
                                } else {
                                    '-'
                                }
                            })
                            .collect();
                        let template = format!(
                            "       IDENTIFICATION DIVISION.\n\
                             \x20      PROGRAM-ID. {prog}.\n\
                             \x20     *> {proj_name} — main program.\n\
                             \n\
                             \x20      PROCEDURE DIVISION.\n\
                             \x20          DISPLAY \"Hello from {proj_name}\".\n\
                             \x20          GOBACK.\n"
                        );
                        if std::fs::write(&main_path, template).is_ok() {
                            if let Some(p) = &mut self.cobolt_project {
                                p.add_file_to(
                                    &main_rel,
                                    crate::project_model::Category::CommonCode,
                                );
                            }
                        }
                    }
                    self.project.set_root(&dir);
                    self.forms_list.set_root(&dir);
                    self.do_save_project(); // persist the tracked main

                    // Initialize the project Settings form and show it immediately
                    // (fills the main work area right of the tree; no editor controls on top).
                    if let Some(p) = &self.cobolt_project {
                        self.settings_form = Some(crate::panels::settings_form::SettingsForm::new(
                            p, &self.llm,
                        ));
                        self.show_project_settings = true;
                        self.inspect = None;
                    }
                }
                let name = self.cobolt_project.as_ref().unwrap().project.name.clone();
                self.output.push_status(format!("Created project '{name}'"));
                self.new_project.open = false;
            }
            Err(e) => {
                self.output
                    .push_status(format!("Failed to create project: {e}"));
            }
        }
    }

    fn do_open_project(&mut self) {
        self.begin_file_dialog(
            FileRequest::OpenProject,
            crate::file_dialog::DialogSpec::open().filter("RustCOBOL Project", &["toml"]),
        );
    }

    fn open_project_at(&mut self, path: PathBuf) {
        match load_project(&path) {
            Ok(proj) => {
                let dir = path.parent().map(|p| p.to_owned());
                self.output
                    .push_status(format!("Opened project '{}'", proj.project.name));
                self.cobolt_project = Some(proj);
                self.project_path = Some(path);

                // Load persisted "raw editor preferred" for indexed files from the
                // IDE-managed indexed state file in the project's data/ (dog-fooding
                // the same mechanism used for agent conversation history).
                if let Some(root) = dir.as_ref() {
                    let data_dir = root.join("data");
                    let rels = crate::llm::load_raw_preferred_indexed(&data_dir);
                    self.raw_preferred_indexed =
                        rels.into_iter().map(|rel| root.join(rel)).collect();
                }
                if let Some(dir) = dir {
                    // Back-fill any standard sub-folders missing from older projects.
                    let mut created = 0;
                    for sub in PROJECT_FOLDERS {
                        let p = dir.join(sub);
                        if !p.exists() && std::fs::create_dir_all(&p).is_ok() {
                            created += 1;
                        }
                    }
                    if created > 0 {
                        self.output
                            .push_status(format!("Added {created} standard project folder(s)"));
                    }
                    self.project.set_root(&dir);
                    self.forms_list.set_root(&dir);
                }
                // Initialize (or reset) the in-pane project Settings form and show it
                // by default (fills the main area to the right of the tree, no editor chrome).
                if let Some(p) = &self.cobolt_project {
                    self.settings_form = Some(crate::panels::settings_form::SettingsForm::new(
                        p, &self.llm,
                    ));
                    self.show_project_settings = true;
                    self.inspect = None;
                }
            }
            Err(e) => {
                self.output.push_status(format!(
                    "Failed to open project: {e}. Make sure you selected a valid project file (.toml with a [project] table)."
                ));
            }
        }
    }

    fn do_save_project(&mut self) {
        if self.cobolt_project.is_none() {
            return;
        }

        // No path yet → ask where to save (async); the result re-enters here.
        let Some(path) = self.project_path.clone() else {
            self.begin_file_dialog(
                FileRequest::SaveProject,
                crate::file_dialog::DialogSpec::save()
                    .filter("RustCOBOL Project", &["toml"])
                    .file_name("cobolt.toml"),
            );
            return;
        };

        let proj = self.cobolt_project.as_ref().unwrap().clone();
        match save_project(&proj, &path) {
            Ok(()) => {
                self.output
                    .push_status(format!("Project saved → {}", path.display()));
            }
            Err(e) => {
                self.output.push_status(format!("Save project failed: {e}"));
            }
        }
    }

    fn do_package_project(&mut self) {
        if self.cobolt_project.is_none() || self.project_path.is_none() {
            self.output
                .push_status("Open or create a project first (File → New/Open Project).");
            return;
        }

        let zip_name = format!(
            "{}.zip",
            self.cobolt_project
                .as_ref()
                .unwrap()
                .project
                .name
                .to_ascii_lowercase()
                .replace(' ', "_")
        );

        self.begin_file_dialog(
            FileRequest::PackageProject,
            crate::file_dialog::DialogSpec::save()
                .filter("Zip archive", &["zip"])
                .file_name(zip_name),
        );
    }

    /// Write the project zip once the user has chosen the destination.
    fn package_project_to(&mut self, out_zip: PathBuf) {
        let (Some(proj), Some(proj_path)) = (&self.cobolt_project, &self.project_path) else {
            return;
        };
        let proj_dir = proj_path.parent().unwrap_or(proj_path.as_path()).to_owned();
        let tr = self.lang.tr();
        for rel in &proj.files.indexed {
            let cidx_path = proj_dir.join(rel);
            if let Ok(def) = load_indexed(&cidx_path) {
                if !crate::project_model::assign_path_is_packaged(&def.assign_path, &proj_dir) {
                    self.output.push_status(format!(
                        "{} ({})",
                        tr.pkg_warn_external_path, def.assign_path
                    ));
                }
            }
        }
        let proj_snap = proj.clone();
        match package_project(&proj_snap, &proj_dir, &out_zip) {
            Ok(count) => {
                self.output
                    .push_status(format!("Packaged {count} files → {}", out_zip.display()));
            }
            Err(e) => {
                self.output.push_status(format!("Package failed: {e}"));
            }
        }
    }

    /// Compile the open project into a single native binary placed in `bin/`.
    ///
    /// Runs entirely on a background thread so the IDE stays responsive.
    /// Progress lines are forwarded to the Output panel.
    fn do_build_binary(&mut self) {
        self.regenerate_all_forms();
        self.regenerate_all_indexed_files();
        let Some(proj_path) = &self.project_path else {
            self.output
                .push_status("Open or create a project first (File → New/Open Project).");
            return;
        };

        let manifest = proj_path.clone();
        self.output.clear();
        self.output
            .push_status("── Building binary …  (this may take a minute) ──");

        // Run the build on a background thread; collect result via a one-shot
        // channel, and stream phase progress via a second channel.
        let (tx, rx) = std::sync::mpsc::channel::<Result<cobolt_compiler::BuildResult, String>>();
        let (ptx, prx) = std::sync::mpsc::channel::<cobolt_compiler::BuildProgress>();
        std::thread::spawn(move || {
            let opts = BuildOptions {
                verbose: false,
                workspace_root: None,
                progress: Some(ptx),
            };
            let result = build_project(&manifest, &opts).map_err(|e| e.to_string());
            let _ = tx.send(result);
        });

        // Poll both channels each frame; store the receivers so update() can drain them.
        self.pending_build_rx = Some(rx);
        self.pending_build_progress = Some(prx);
        self.build_phase = (0.0, "Starting…".to_string());
    }

    /// The project's root directory (where `cobolt.toml` lives), if a project is open.
    fn project_dir(&self) -> Option<PathBuf> {
        self.project_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_owned())
    }

    /// First available `<base>.<ext>` / `<base>-N.<ext>` name in `dir`.
    fn unique_file_name(dir: &Path, base: &str, ext: &str) -> String {
        let first = format!("{base}.{ext}");
        if !dir.join(&first).exists() {
            return first;
        }
        for n in 1.. {
            let cand = format!("{base}-{n}.{ext}");
            if !dir.join(&cand).exists() {
                return cand;
            }
        }
        first
    }

    /// `+` on a category — **create** a new item of that kind.
    fn do_create_in_category(&mut self, kind: FileKind) {
        match kind {
            // A form has a real "create" dialog.
            FileKind::Form => self.new_form.open = true,
            FileKind::Indexed => self.new_indexed_file_dialog(),
            FileKind::Source => self.create_new_text_file(FileKind::Source),
            FileKind::Documentation => self.create_new_text_file(FileKind::Documentation),
            // Assets can't be authored in the IDE — creating one means importing.
            FileKind::Asset => self.do_add_file_to_project(FileKind::Asset),
        }
    }

    /// Create a new editable text file (COBOL source or documentation) in the
    /// project, with a starter template, then track it and open it in the editor.
    fn create_new_text_file(&mut self, kind: FileKind) {
        use crate::project_model::Category;
        let Some(dir) = self.project_dir() else {
            self.output.push_status("Save the project first.");
            return;
        };
        let (sub, base, ext, category) = match kind {
            FileKind::Source => ("src", "new-program", "cbl", Category::CommonCode),
            _ => ("docs", "new-document", "md", Category::Documentation),
        };
        let sub_dir = dir.join(sub);
        if let Err(e) = std::fs::create_dir_all(&sub_dir) {
            self.output
                .push_status(format!("Could not create {sub}/: {e}"));
            return;
        }
        let fname = Self::unique_file_name(&sub_dir, base, ext);
        let stem = fname.trim_end_matches(&format!(".{ext}")).to_string();
        let content = if kind == FileKind::Source {
            let prog = stem.to_ascii_uppercase().replace('_', "-");
            format!(
                "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. {prog}.\n\
                 \n       PROCEDURE DIVISION.\n           DISPLAY \"Hello from {prog}\".\n           GOBACK.\n")
        } else {
            format!("# {stem}\n\n")
        };
        let path = sub_dir.join(&fname);
        if let Err(e) = std::fs::write(&path, content) {
            self.output
                .push_status(format!("Could not create file: {e}"));
            return;
        }
        let rel = format!("{sub}/{fname}");
        if let Some(proj) = &mut self.cobolt_project {
            proj.add_file_to(&rel, category);
        }
        self.do_save_project();
        self.output.push_status(format!("Created {rel}"));
        self.open_in_editor(path);
    }

    fn do_add_file_to_project(&mut self, kind: FileKind) {
        let proj_dir = match &self.project_path {
            Some(p) => p.parent().unwrap_or(p.as_path()).to_owned(),
            None => {
                self.output.push_status("Save the project first.");
                return;
            }
        };

        // Assets may be ANY binary/data file (images, audio, video, fonts, …).
        // The picker must NOT restrict to a fixed extension list: a `"*"` filter
        // greys out every file on macOS/GTK, and even named filters disable
        // anything outside their lists. So assets get **no filter at all** — any
        // file is selectable. The other kinds keep their helpful filters.
        let spec = crate::file_dialog::DialogSpec::open().directory(proj_dir);
        let spec = match kind {
            FileKind::Source => spec.filter("COBOL Source", &["cbl", "cob", "cpy"]),
            FileKind::Form => spec.filter("RustCOBOL Form", &["cfrm"]),
            FileKind::Indexed => spec.filter("Indexed data file", &["idx", "dat"]),
            FileKind::Documentation => spec.filter(
                "Documentation",
                &["md", "markdown", "txt", "rst", "adoc", "pdf", "html", "htm"],
            ),
            FileKind::Asset => spec, // no filter → every file selectable
        };

        self.begin_file_dialog(FileRequest::AddFile(kind), spec);
    }

    /// Add the chosen file to the open project under `kind`'s category. A file
    /// **outside** the project directory is **copied into** a category subfolder
    /// (`src/`, `forms/`, `assets/`, `docs/`) so it becomes part of the project
    /// (and ships with the build); a file already inside is tracked in place.
    fn add_file_to_project_path(&mut self, kind: FileKind, path: PathBuf) {
        if kind == FileKind::Indexed {
            self.import_indexed_data_file(path);
            return;
        }
        use crate::project_model::Category;
        let proj_dir = match &self.project_path {
            Some(p) => p.parent().unwrap_or(p.as_path()).to_owned(),
            None => return,
        };

        // Resolve to a project-relative path, importing (copying) when external.
        let rel = match relative_to(&path, &proj_dir) {
            Some(rel) => rel,
            None => {
                let subdir = match kind {
                    FileKind::Source => "src",
                    FileKind::Form => "forms",
                    FileKind::Indexed => "data",
                    FileKind::Asset => "assets",
                    FileKind::Documentation => "docs",
                };
                let Some(fname) = path.file_name() else {
                    self.output.push_status("Invalid file name.");
                    return;
                };
                let dest_dir = proj_dir.join(subdir);
                if let Err(e) = std::fs::create_dir_all(&dest_dir) {
                    self.output
                        .push_status(format!("Could not create {subdir}/: {e}"));
                    return;
                }
                let dest = dest_dir.join(fname);
                if let Err(e) = std::fs::copy(&path, &dest) {
                    self.output
                        .push_status(format!("Could not import file: {e}"));
                    return;
                }
                self.output.push_status(format!(
                    "Imported {} → {}/{}",
                    fname.to_string_lossy(),
                    subdir,
                    fname.to_string_lossy()
                ));
                relative_to(&dest, &proj_dir)
                    .unwrap_or_else(|| format!("{subdir}/{}", fname.to_string_lossy()))
            }
        };

        let category = match kind {
            FileKind::Source => Category::CommonCode,
            FileKind::Form => Category::Forms,
            FileKind::Indexed => Category::IndexedFiles,
            FileKind::Asset => Category::Assets,
            FileKind::Documentation => Category::Documentation,
        };
        if let Some(proj) = &mut self.cobolt_project {
            proj.add_file_to(&rel, category);
        }
        self.do_save_project();
    }

    fn new_indexed_file_dialog(&mut self) {
        self.new_indexed.open = true;
    }

    fn do_remove_file_from_project(&mut self, rel: String) {
        if let Some(proj) = &mut self.cobolt_project {
            proj.remove_file(&rel);
        }
        self.do_save_project();
    }

    fn add_user_control_def(&mut self, def: UserControlDef) {
        let name = def.name.clone();
        let Some(proj) = &mut self.cobolt_project else {
            self.output
                .push_status("Open or create a project before creating a User Control.");
            return;
        };
        if proj
            .user_controls
            .iter()
            .any(|existing| existing.name.eq_ignore_ascii_case(&name))
        {
            self.output
                .push_status(format!("User Control '{name}' already exists."));
            return;
        }
        proj.user_controls.push(def);
        self.do_save_project();
        self.output
            .push_status(format!("User Control '{name}' saved to project."));
    }

    fn remove_user_control_def(&mut self, name: &str) {
        let Some(proj) = &mut self.cobolt_project else {
            return;
        };
        let before = proj.user_controls.len();
        proj.user_controls
            .retain(|def| !def.name.eq_ignore_ascii_case(name));
        if proj.user_controls.len() != before {
            self.do_save_project();
            self.output
                .push_status(format!("User Control '{name}' removed from project."));
        }
    }

    // ── Designer actions ──────────────────────────────────────────────────────

    fn do_open_form(&mut self) {
        self.begin_file_dialog(
            FileRequest::OpenForm,
            crate::file_dialog::DialogSpec::open().filter("RustCOBOL Form", &["cfrm"]),
        );
    }

    fn load_form_from_path(&mut self, path: PathBuf) {
        if self.designers.iter().any(|(p, _)| p == &path) {
            return; // already open — the viewport is already being shown
        }
        match load_form(&path) {
            Ok(form) => {
                if let Some(parent) = path.parent() {
                    self.forms_list.set_root(parent);
                    if self.cobolt_project.is_none() {
                        self.project.set_root(parent);
                    }
                }
                let mut dp = DesignerPanel::new(form);
                dp.cfrm_dir = path.parent().map(|p| p.to_path_buf());
                self.designers.push((path, dp));
            }
            Err(e) => {
                self.output.push_status(format!("Failed to open form: {e}"));
            }
        }
    }

    fn do_save_designer(&mut self, idx: usize) {
        if idx >= self.designers.len() {
            return;
        }
        let path = self.designers[idx].0.clone();
        let form_name = self.designers[idx].1.form.name.clone();
        let result = save_form(&self.designers[idx].1.form, &path);
        match result {
            Ok(()) => {
                self.designers[idx].1.dirty = false;
                self.output.push_status(format!("Saved {}", path.display()));
                self.forms_list.refresh();
                // Reflect the change in the tree + regenerate the backend COBOL.
                self.after_form_saved(&path);
                // Show the "Form <name> saved" alert in THIS designer's viewport
                // (so it is not hidden behind the designer window).
                self.save_alert_msg = Some(form_name);
                self.save_alert_designer = Some(idx);
            }
            Err(e) => {
                self.output.push_status(format!("Save form failed: {e}"));
            }
        }
    }

    /// Double-clicking an event row jumps to that event's paragraph in the
    /// generated COBOL: (re)generate the `.cbl`, open it in the editor, and queue
    /// a scroll to the paragraph. `ctrl_id` is empty for form-level events.
    fn jump_to_event_code(&mut self, idx: usize, ctrl_id: &str, event: &str) {
        if idx >= self.designers.len() {
            return;
        }

        // Resolve the paragraph name from the binding, or derive it the same way
        // codegen does, so the lookup matches the generated source.
        let para = {
            let form = &self.designers[idx].1.form;
            if ctrl_id.is_empty() {
                form.form_events
                    .iter()
                    .find(|e| e.event == event)
                    .map(|e| e.paragraph.clone())
                    .unwrap_or_else(|| cobolt_forms::model::derive_paragraph_name("", event))
            } else {
                form.controls
                    .iter()
                    .find(|c| c.id == ctrl_id)
                    .and_then(|c| c.events.iter().find(|e| e.event == event))
                    .map(|e| e.paragraph.clone())
                    .unwrap_or_else(|| cobolt_forms::model::derive_paragraph_name(ctrl_id, event))
            }
        };

        // The first click of the double-click may have popped the modal editor —
        // close it so we cleanly hand off to the main code editor.
        self.designers[idx].1.event_modal = None;

        // Regenerate the .cbl (it is generated output) and queue it to open, then
        // scroll to the paragraph once the editor has the file loaded.
        self.do_generate_cobol(idx);
        self.pending_goto_paragraph = Some(para);
    }

    /// Open the inline inspector in the Main Pane for a form (and optionally a
    /// control), reusing a transient `DesignerPanel` (no designer window).
    fn open_inspect(&mut self, path: PathBuf, ctrl_id: Option<String>) {
        if let Some(st) = &mut self.inspect {
            if st.path == path {
                // Same form already open in the Main Pane: just retarget the
                // selected control, but pull in any on-disk change first so a
                // Designer save (or external edit) is reflected.
                st.ctrl_id = ctrl_id;
                st.reload_if_stale();
                return;
            }
        }
        match load_form(&path) {
            Ok(form) => {
                let mtime = file_mtime(&path);
                self.inspect = Some(InspectState {
                    path,
                    ctrl_id,
                    designer: DesignerPanel::new(form),
                    mtime,
                });
            }
            Err(e) => self.output.push_status(format!("Failed to read form: {e}")),
        }
    }

    fn open_indexed_inspect(&mut self, path: PathBuf, field_id: Option<String>) {
        self.indexed_inspect = None;
        match load_indexed(&path) {
            Ok(def) => {
                let mut panel = IndexedEditorPanel::new();
                if let Some(id) = field_id {
                    panel.select_field(id);
                }
                let prefer = self.raw_preferred_indexed.contains(&path);
                let mut inspect = IndexedInspectState {
                    path,
                    def,
                    panel,
                    dirty: false,
                    prefer_raw_editor: prefer,
                };
                inspect.panel.sync_from_def(&inspect.def);
                // Seed raw_text from the project's copybooks/<NAME>.fd.cpy when present
                // (the canonical source for the COBOL editor text). This ensures the
                // editor opens with any previously saved changes. Fall back to a
                // generated representation derived from the .cidx model.
                let raw_seed = if let Some(cpy) = self.indexed_fd_copybook_path(&inspect.def.name) {
                    std::fs::read_to_string(&cpy).unwrap_or_else(|_| record_to_text(&inspect.def))
                } else {
                    record_to_text(&inspect.def)
                };
                inspect.panel.raw_text = raw_seed;
                self.indexed_inspect = Some(inspect);
            }
            Err(e) => self
                .output
                .push_status(format!("Failed to read .cidx: {e}")),
        }
    }

    fn save_and_refresh_indexed(&mut self) {
        if let Some(st) = &mut self.indexed_inspect {
            st.dirty = true;
            if save_indexed(&st.path, &st.def).is_ok() {
                st.dirty = false;
                st.panel.sync_from_def(&st.def);
                let path = st.path.clone();
                let def = st.def.clone();
                // Compute cpy name + descriptor text *while* the &mut borrow of indexed_inspect
                // is active; perform all self.* calls *after* so NLL can end the borrow early
                // (avoids E0499/E0502 when calling &mut self methods).
                let name_for_cpy = def.name.clone();
                if !st.prefer_raw_editor {
                    st.panel.raw_text = record_to_text(&st.def);
                }
                let text = if !st.panel.raw_text.trim().is_empty() {
                    st.panel.raw_text.clone()
                } else {
                    let gen = record_to_text(&st.def);
                    st.panel.raw_text = gen.clone();
                    gen
                };
                self.project.refresh_indexed(&path);
                self.write_generated_indexed_for(&path, &def);
                let _ = self.write_indexed_fd_copybook(&name_for_cpy, &text);
                self.set_element_status(&path, ElementStatus::Changed);
            }
        }
    }

    /// Render the inline inspector in the Main Pane (central panel).
    fn show_inspector(&mut self, ctx: &egui::Context, tr: &Tr) {
        let mut open_designer = false;
        let mut close = false;
        let mut changed = false;

        // Live-refresh from disk before drawing so a Designer save (or any
        // external write) of this form is reflected in the Main-Pane inspector.
        if let Some(st) = &mut self.inspect {
            st.reload_if_stale();
        }

        // AI assistant bar (above the inspector). Its context is the form's
        // generated COBOL, which is read-only — so replies are shown in the
        // transcript for reference and never overwrite generated code.
        if self.llm.is_configured() {
            let form_path = self.inspect.as_ref().map(|s| s.path.clone());
            if let Some(form_path) = form_path {
                let gen_path = self.generated_cbl_path(&form_path);
                let code = std::fs::read_to_string(&gen_path).unwrap_or_default();
                let root = self
                    .project_path
                    .as_ref()
                    .and_then(|p| p.parent())
                    .map(|p| p.to_path_buf());
                self.editor.ai_bar(
                    ctx,
                    &self.llm,
                    tr,
                    "inspector_ai",
                    &form_path,
                    &code,
                    true,
                    root.as_deref(),
                );
            }
        }

        let card =
            crate::theme::glass_panel_frame(ctx.style().visuals.panel_fill, self.current_theme());
        egui::CentralPanel::default().frame(card).show(ctx, |ui| {
            let Some(st) = &mut self.inspect else {
                return;
            };
            ui.horizontal(|ui| {
                ui.heading(format!("⚙ {}", st.designer.form.name));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tr.inspect_close).clicked() {
                        close = true;
                    }
                    if ui.button(tr.inspect_open_designer).clicked() {
                        open_designer = true;
                    }
                });
            });
            match &st.ctrl_id {
                Some(id) => {
                    ui.label(egui::RichText::new(id).strong().monospace());
                }
                None => {
                    ui.label(egui::RichText::new(tr.inspect_form_props).italics());
                }
            }
            ui.separator();

            // Split-borrow form (read) + properties (mutable), like the designer.
            let ctrl_id = st.ctrl_id.clone();
            let action = {
                let d = &mut st.designer;
                let sel = ctrl_id.as_deref().and_then(|id| d.form.find_control(id));
                let form = &d.form as *const cobolt_forms::Form;
                let props = &mut d.properties;
                props.show(ui, unsafe { &*form }, sel, tr)
            };
            for (cid, key, value) in action.set_props {
                st.designer.set_property(&cid, &key, value);
                changed = true;
            }
            for (key, value) in action.form_props {
                st.designer.set_form_prop(&key, value);
                changed = true;
            }
            if let Some(i) = action.cs_del_proc {
                if i < st.designer.form.user_procedures.len() {
                    st.designer.form.user_procedures.remove(i);
                    changed = true;
                }
            }
            // Event editing and the COBOL Structure editor need the full designer.
            if action.open_event_editor.is_some()
                || action.open_event_in_code.is_some()
                || action.cs_open.is_some()
                || action.cs_add_proc
            {
                open_designer = true;
            }
        });

        if changed {
            let saved_path = if let Some(st) = &mut self.inspect {
                if save_form(&st.designer.form, &st.path).is_ok() {
                    st.designer.dirty = false;
                    // Record our own write time so the live-refresh check does
                    // not treat this save as an external change and reload.
                    st.mtime = file_mtime(&st.path);
                    self.project.refresh_form(&st.path);
                    Some(st.path.clone())
                } else {
                    None
                }
            } else {
                None
            };
            // An inline edit means the form changed and isn't re-tested → yellow.
            if let Some(p) = saved_path {
                self.after_form_saved(&p); // refresh tree + regenerate backend COBOL
                self.set_element_status(&p, ElementStatus::Changed);
            }
        }
        if open_designer {
            let path = self.inspect.take().map(|s| s.path);
            if let Some(p) = path {
                self.load_form_from_path(p);
            }
        }
        if close {
            self.inspect = None;
        }
    }

    /// After a form's `.cfrm` is saved (designer or inline inspector): refresh
    /// the tree's cached form, **regenerate the backend COBOL** (so Generated
    /// Code reflects the change), keep it tracked, and reload an open generated
    /// editor tab.
    fn after_form_saved(&mut self, cfrm_path: &std::path::Path) {
        self.project.refresh_form(cfrm_path);
        let Ok(form) = load_form(cfrm_path) else {
            return;
        };
        let cbl = self.generated_cbl_path(cfrm_path);
        if let Some(parent) = cbl.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::write(&cbl, generate(&form)).is_err() {
            return;
        }
        let rel = self
            .project_path
            .as_ref()
            .and_then(|p| p.parent())
            .and_then(|dir| relative_to(&cbl, dir));
        if let Some(rel) = rel {
            if let Some(proj) = &mut self.cobolt_project {
                proj.add_generated(&rel);
            }
            self.do_save_project();
        }
        self.editor.reload_file(&cbl);
        self.output
            .push_status(format!("Regenerated {}", cbl.display()));
    }

    /// Regenerate one form's `.cbl` from `form`, keep it tracked as generated,
    /// and refresh any open editor tab showing it. Returns whether it was written.
    fn write_generated_for(&mut self, cfrm: &std::path::Path, form: &Form) -> bool {
        let cbl = self.generated_cbl_path(cfrm);
        if let Some(parent) = cbl.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::write(&cbl, generate(form)).is_err() {
            return false;
        }
        if let Some(rel) = self
            .project_path
            .as_ref()
            .and_then(|p| p.parent())
            .and_then(|dir| relative_to(&cbl, dir))
        {
            if let Some(proj) = &mut self.cobolt_project {
                proj.add_generated(&rel);
            }
        }
        self.editor.reload_file(&cbl);
        true
    }

    /// GOLDEN RULE: regenerate every form's COBOL before Build / Run / Debug /
    /// Check, so the compiled and executed code always reflects the current
    /// forms. Open designers use their live (possibly unsaved) state; other
    /// tracked forms are reloaded from their `.cfrm` on disk.
    fn regenerate_all_forms(&mut self) {
        // Open designers first — live state wins over what's on disk.
        let open: Vec<(PathBuf, Form)> = self
            .designers
            .iter()
            .map(|(p, d)| (p.clone(), d.form.clone()))
            .collect();
        let open_paths: std::collections::HashSet<PathBuf> =
            open.iter().map(|(p, _)| p.clone()).collect();
        let mut n = 0usize;
        for (cfrm, form) in &open {
            if self.write_generated_for(cfrm, form) {
                n += 1;
            }
        }
        // Other tracked forms — load from disk and regenerate.
        if let Some(root) = self
            .project_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_owned())
        {
            let closed: Vec<PathBuf> = self
                .cobolt_project
                .as_ref()
                .map(|p| {
                    p.files
                        .forms
                        .iter()
                        .map(|rel| root.join(rel))
                        .filter(|p| !open_paths.contains(p))
                        .collect()
                })
                .unwrap_or_default();
            for cfrm in closed {
                if let Ok(form) = load_form(&cfrm) {
                    if self.write_generated_for(&cfrm, &form) {
                        n += 1;
                    }
                }
            }
        }
        if n > 0 {
            self.do_save_project();
        }
    }

    /// Path for a form's generated `.cbl`: under the project's `generated/`
    /// folder when a project is open, else next to the `.cfrm`.
    fn generated_cbl_path(&self, cfrm: &std::path::Path) -> PathBuf {
        let stem = cfrm.file_stem().and_then(|s| s.to_str()).unwrap_or("form");
        if let Some(dir) = self.project_path.as_ref().and_then(|p| p.parent()) {
            return dir.join("generated").join(format!("{stem}.cbl"));
        }
        cfrm.with_extension("cbl")
    }

    fn generated_indexed_cbl_path(&self, cidx: &std::path::Path) -> PathBuf {
        let stem = cidx
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("indexed");
        if let Some(dir) = self.project_path.as_ref().and_then(|p| p.parent()) {
            return dir.join("generated").join(format!("{stem}-indexed.cbl"));
        }
        cidx.with_extension("cbl")
    }

    /// Path under the project `copybooks/` for the canonical COBOL-85 record
    /// descriptor source for an indexed file (keyed by its logical name e.g.
    /// CUSTOMER-FILE.fd.cpy). This is the file the raw COBOL editor reads/writes.
    fn indexed_fd_copybook_path(&self, indexed_name: &str) -> Option<PathBuf> {
        let dir = self.project_dir()?;
        Some(
            dir.join("copybooks")
                .join(format!("{}.fd.cpy", indexed_name)),
        )
    }

    /// Ensure copybooks/ exists and write (or overwrite) the given text as the
    /// editable COBOL descriptor for the named indexed file.
    fn write_indexed_fd_copybook(&self, indexed_name: &str, text: &str) -> bool {
        let Some(path) = self.indexed_fd_copybook_path(indexed_name) else {
            return false;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&path, text).is_ok()
    }

    fn resolve_assign_path(&self, def: &IndexedDefinition) -> Option<PathBuf> {
        let root = self.project_dir()?;
        Some(resolve_path(&root, &def.assign_path))
    }

    fn check_schema_drift(&self, def: &IndexedDefinition) -> (bool, Option<String>) {
        let Some(data) = self.resolve_assign_path(def) else {
            return (false, None);
        };
        if !data.exists() {
            return (false, None);
        }
        let Ok(info) = inspect_any_path(&data) else {
            return (false, None);
        };
        let Some(info) = info else {
            return (true, Some("no on-disk schema".into()));
        };
        match compare_schema(def, &info) {
            SchemaDrift::Ok => (false, None),
            SchemaDrift::Mismatch { detail } => (true, Some(detail)),
            SchemaDrift::NoSchemaOnDisk => (true, Some("no schema on disk".into())),
        }
    }

    fn open_grid_for_indexed(&mut self, cidx_path: &Path, def: &IndexedDefinition) {
        let (drift, _) = self.check_schema_drift(def);
        let data_path = match self.resolve_assign_path(def) {
            Some(p) => p,
            None => return,
        };
        if let Some((_, st)) = self.indexed_grids.iter_mut().find(|(p, _)| p == cidx_path) {
            st.panel.open(def, &data_path, drift);
            st.def = def.clone();
            st.close_requested = false;
            return;
        }
        let mut panel = IndexedGridPanel::new();
        panel.open(def, &data_path, drift);
        self.indexed_grids.push((
            cidx_path.to_path_buf(),
            IndexedGridState {
                panel,
                def: def.clone(),
                close_requested: false,
            },
        ));
    }

    fn write_generated_indexed_for(
        &mut self,
        cidx: &std::path::Path,
        def: &IndexedDefinition,
    ) -> bool {
        let cbl = self.generated_indexed_cbl_path(cidx);
        if let Some(parent) = cbl.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::write(&cbl, generate_indexed(def)).is_err() {
            return false;
        }
        if let Some(rel) = self
            .project_path
            .as_ref()
            .and_then(|p| p.parent())
            .and_then(|dir| relative_to(&cbl, dir))
        {
            if let Some(proj) = &mut self.cobolt_project {
                proj.add_generated(&rel);
            }
        }
        self.editor.reload_file(&cbl);
        true
    }

    fn regenerate_all_indexed_files(&mut self) {
        // Pop-up editors removed. If the inline inspector is open, use its in-memory def.
        let open: Vec<(PathBuf, IndexedDefinition)> = if let Some(st) = &self.indexed_inspect {
            vec![(st.path.clone(), st.def.clone())]
        } else {
            vec![]
        };
        let open_paths: std::collections::HashSet<PathBuf> =
            open.iter().map(|(p, _)| p.clone()).collect();
        let mut n = 0usize;
        for (cidx, def) in &open {
            if self.write_generated_indexed_for(cidx, def) {
                n += 1;
            }
        }
        if let Some(root) = self.project_dir() {
            let closed: Vec<PathBuf> = self
                .cobolt_project
                .as_ref()
                .map(|p| {
                    p.files
                        .indexed
                        .iter()
                        .map(|rel| root.join(rel))
                        .filter(|p| !open_paths.contains(p))
                        .collect()
                })
                .unwrap_or_default();
            for cidx in closed {
                if let Ok(def) = load_indexed(&cidx) {
                    if self.write_generated_indexed_for(&cidx, &def) {
                        n += 1;
                    }
                }
            }
        }
        if n > 0 {
            self.do_save_project();
        }
    }

    fn create_new_indexed_file(&mut self) {
        // Supports both the properties form and the raw COBOL-85 text editor.
        // The dialog already enforced !exists + full validity (group + RECORD KEY).
        let Some(mut def) = self.new_indexed.get_definition() else {
            self.output.push_status(
                "Invalid indexed file parameters (incomplete or not COBOL-85 compliant)",
            );
            return;
        };
        let Some(dir) = self.project_dir() else {
            self.output.push_status("Save the project first.");
            return;
        };
        let sub_dir = dir.join("indexed");
        if let Err(e) = std::fs::create_dir_all(&sub_dir) {
            self.output
                .push_status(format!("Could not create indexed/: {e}"));
            return;
        };
        let stem = def.name.to_ascii_lowercase().replace('-', "_");
        let fname = Self::unique_file_name(&sub_dir, &stem, "cidx");
        let path = sub_dir.join(&fname);

        // The dialog checks the assign_path; we also avoid overwriting a .cidx.
        if path.exists() {
            self.output
                .push_status("Indexed definition already exists. Pick a different name.");
            return;
        }

        // When adding a new indexed file to the project, load copybooks/<name>.fd.cpy
        // if it already exists. Its content becomes the record structure source
        // (and we create the file if it did not exist, later below).
        let mut using_cpy_text: Option<String> = None;
        if let Some(cpy_path) = self.indexed_fd_copybook_path(&def.name) {
            if cpy_path.exists() {
                if let Ok(text) = std::fs::read_to_string(&cpy_path) {
                    if text_to_record(&mut def, &text).is_ok() {
                        using_cpy_text = Some(text);
                    }
                }
            }
        }

        if save_indexed(&path, &def).is_err() {
            self.output.push_status("Could not write .cidx");
            return;
        }
        let rel = format!("indexed/{fname}");
        if let Some(proj) = &mut self.cobolt_project {
            use crate::project_model::Category;
            proj.add_file_to(&rel, Category::IndexedFiles);
        }
        self.do_save_project();
        self.new_indexed.open = false;

        let via_raw = self.new_indexed.raw_mode || using_cpy_text.is_some();
        self.open_indexed_inspect(path.clone(), None);

        // Ensure the .fd.cpy is created (or overwritten with current source text).
        // Use the loaded text if we took it from an existing cpy, else the dialog's
        // raw buffer (when user created via editor) or a generated text from the def.
        let text_for_cpy = if let Some(t) = using_cpy_text {
            t
        } else if self.new_indexed.raw_mode && !self.new_indexed.raw_text.trim().is_empty() {
            self.new_indexed.raw_text.clone()
        } else {
            record_to_text(&def)
        };
        let _ = self.write_indexed_fd_copybook(&def.name, &text_for_cpy);

        // Lock this file to the COBOL text editor (no going back to properties
        // pane for structural edits). Surface the editor immediately.
        if via_raw {
            if let Some(st) = &mut self.indexed_inspect {
                st.prefer_raw_editor = true;
            }
            self.raw_preferred_indexed.insert(path.clone());
            if self.new_indexed.raw_mode {
                if let Some(st) = &mut self.indexed_inspect {
                    st.panel.request_raw_dialog = true;
                }
            }
            // Persist using IDE-managed indexed file in data/ (same as agent convos).
            if let Some(root) = self.project_path.as_ref().and_then(|p| p.parent()) {
                let data_dir = root.join("data");
                let rels: std::collections::HashSet<String> = self
                    .raw_preferred_indexed
                    .iter()
                    .filter_map(|abs| {
                        abs.strip_prefix(root)
                            .ok()
                            .map(|r| r.to_string_lossy().replace('\\', "/"))
                    })
                    .collect();
                crate::llm::save_raw_preferred_indexed(&data_dir, &rels);
            }
        }
    }

    fn import_indexed_data_file(&mut self, data_path: PathBuf) {
        let Some(proj_dir) = self.project_dir() else {
            self.output.push_status("Save the project first.");
            return;
        };
        let info = match inspect_any_path(&data_path) {
            Ok(Some(i)) => i,
            Ok(None) => {
                let tr = self.lang.tr();
                self.output
                    .push_status(tr.warn_import_no_schema.to_string());
                return;
            }
            Err(e) => {
                let tr = self.lang.tr();
                self.output
                    .push_status(format!("{}: {e}", tr.warn_import_failed));
                return;
            }
        };
        let stem = data_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("imported")
            .to_ascii_uppercase()
            .replace('_', "-");
        let mut def = definition_from_inspect(&stem, &proj_dir, &data_path, &info);
        // When adding/importing, load copybooks/<name>.fd.cpy if present for the
        // logical name and use it for the record structure (created below if absent).
        let mut using_cpy_text: Option<String> = None;
        if let Some(cpy_path) = self.indexed_fd_copybook_path(&def.name) {
            if cpy_path.exists() {
                if let Ok(text) = std::fs::read_to_string(&cpy_path) {
                    if text_to_record(&mut def, &text).is_ok() {
                        using_cpy_text = Some(text);
                    }
                }
            }
        }
        let sub_dir = proj_dir.join("indexed");
        if let Err(e) = std::fs::create_dir_all(&sub_dir) {
            self.output
                .push_status(format!("Could not create indexed/: {e}"));
            return;
        }
        let base = stem.to_ascii_lowercase().replace('-', "_");
        let fname = Self::unique_file_name(&sub_dir, &base, "cidx");
        let cidx_path = sub_dir.join(&fname);
        if save_indexed(&cidx_path, &def).is_err() {
            self.output.push_status("Could not write .cidx");
            return;
        }
        let rel = format!("indexed/{fname}");
        if let Some(proj) = &mut self.cobolt_project {
            use crate::project_model::Category;
            proj.add_file_to(&rel, Category::IndexedFiles);
        }
        self.do_save_project();
        // Create (or refresh) the .fd.cpy for the imported/added file so that the
        // COBOL editor has a place for its source and loads it on future opens.
        let text_for_cpy = if let Some(t) = using_cpy_text {
            t
        } else {
            record_to_text(&def)
        };
        let _ = self.write_indexed_fd_copybook(&def.name, &text_for_cpy);
        self.output
            .push_status(format!("Imported indexed file → {rel}"));
        self.open_indexed_inspect(cidx_path, None);
    }

    fn show_indexed_inspector(&mut self, ctx: &egui::Context, tr: &Tr) {
        let mut close = false;
        let mut open_grid = false;
        let mut property_edit = PropertyEdit::None;
        let mut structure_action = StructureAction::None;
        let mut did_add_remove = false;

        let card =
            crate::theme::glass_panel_frame(ctx.style().visuals.panel_fill, self.current_theme());

        egui::CentralPanel::default().frame(card).show(ctx, |ui| {
            let Some(st) = &mut self.indexed_inspect else { return; };

            if st.prefer_raw_editor {
                ui.colored_label(
                    egui::Color32::from_rgb(200, 160, 60),
                    "This file uses the COBOL text editor as the primary definition surface (properties pane is secondary/read-only for structure to avoid desync with COBOL-85 source).",
                );
                ui.add_space(4.0);
            }

            // Shared helper for hand-written vector icon buttons (style consistent with
            // designer toolbar and doc viewer). Any missing icon gets a simple procedural
            // vector here (no external assets, theme-aware strokes).
            let icon_btn = |ui: &mut egui::Ui, size: egui::Vec2, tip: &str, draw: &dyn Fn(&egui::Painter, egui::Rect, egui::Color32)| -> bool {
                let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());

                // Always draw a subtle rounded rect border so the + / X (and other small
                // icon buttons in the inspector) look like proper buttons with rounded rects.
                // Uses theme-adaptive color (text_color dimmed or hovered stroke) so it
                // stays visible and consistent on both light and dark themes.
                let border_color = if resp.hovered() {
                    ui.visuals().widgets.hovered.bg_stroke.color
                } else {
                    ui.visuals().text_color().linear_multiply(0.35)
                };
                ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(1.0, border_color));

                if resp.hovered() {
                    ui.painter().rect_filled(rect, 3.0, ui.visuals().widgets.hovered.bg_fill);
                }
                let c = if resp.hovered() {
                    ui.visuals().widgets.hovered.fg_stroke.color
                } else {
                    ui.visuals().text_color()
                };
                draw(ui.painter(), rect.shrink(3.5), c);
                resp.on_hover_text(tip).clicked()
            };

            ui.horizontal(|ui| {
                // Hand-written vector cabinet icon (replaces 🗂️). Same symbol used in
                // the project tree for Indexed Files + CUSTOMER-FILE etc. Ensures it
                // always renders the same regardless of system emoji/font support.
                let icon_sz = egui::vec2(16.0, 16.0);
                let (ir, _) = ui.allocate_exact_size(icon_sz, egui::Sense::hover());
                if ui.is_rect_visible(ir) {
                    crate::panels::project::draw_indexed_icon(
                        ui.painter(),
                        ir.shrink(1.0),
                        ui.visuals().text_color(),
                    );
                }
                ui.heading(&st.def.name);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Close (X) - hand-written vector icon
                    if icon_btn(ui, egui::vec2(22.0, 18.0), "Close inspector", &|p, r, c| {
                        let s = egui::Stroke::new(1.8, c);
                        let q = r.shrink(r.width() * 0.22);
                        p.line_segment([q.left_top(), q.right_bottom()], s);
                        p.line_segment([q.right_top(), q.left_bottom()], s);
                    }) {
                        close = true;
                    }

                    // Open Grid Browser - hand-written grid/table icon
                    let grid_tip = if st.def.finalized { "Open Grid Browser" } else { tr.grid_requires_finalize };
                    if icon_btn(ui, egui::vec2(22.0, 18.0), grid_tip, &|p, r, c| {
                        let col = if st.def.finalized { c } else {
                            egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 85)
                        };
                        let s = egui::Stroke::new(1.25, col);
                        let sr = r.shrink(2.5);
                        for i in 0..=2 {
                            let t = i as f32 / 2.0;
                            let x = sr.min.x + sr.width() * t;
                            let y = sr.min.y + sr.height() * t;
                            p.line_segment([egui::Pos2::new(x, sr.min.y), egui::Pos2::new(x, sr.max.y)], s);
                            p.line_segment([egui::Pos2::new(sr.min.x, y), egui::Pos2::new(sr.max.x, y)], s);
                        }
                    }) && st.def.finalized {
                        open_grid = true;
                    }

                    // Edit record as text (raw) - hand-written document-with-lines icon
                    if icon_btn(ui, egui::vec2(22.0, 18.0), "Edit record as text (raw COBOL)", &|p, r, c| {
                        let s = egui::Stroke::new(1.5, c);
                        let body = egui::Rect::from_center_size(r.center(), egui::Vec2::new(r.width() * 0.60, r.height() * 0.68));
                        p.rect_stroke(body, 1.4, s);
                        for i in 0..3 {
                            let y = body.min.y + body.height() * (0.26 + i as f32 * 0.22);
                            p.line_segment([egui::Pos2::new(body.min.x + 2.5, y), egui::Pos2::new(body.max.x - 2.5, y)], egui::Stroke::new(1.0, c));
                        }
                    }) {
                        st.panel.open_raw_dialog(&st.def);
                    }
                });
            });
            ui.separator();

            // Capture the full remaining height of the inspector panel *after the header*.
            // Using available_rect_before_wrap().height() here is more reliable in egui's
            // immediate-mode nested layouts than repeated available_height() calls (which
            // can return small/zero values during size computation passes).
            // This makes the left "record structure" (tree) column expand to the full red-rect
            // height instead of collapsing to the green (content height of the short properties).
            let remaining_rect = ui.available_rect_before_wrap();

            ui.allocate_ui_at_rect(remaining_rect, |ui| {
                if st.prefer_raw_editor {
                    // The embedded COBOL editor (raw record descriptor) is now the primary/visible
                    // form for this file's file descriptor. The tree + property pane form is
                    // replaced (per the requirement: once the user has made changes via the
                    // raw editor, they can no longer edit the descriptor information via the
                    // properties form; the editor remains the surface).
                    let applied = st.panel.show_raw_editor_inline(ui, &mut st.def, tr);
                    if applied {
                        did_add_remove = true;
                    }
                } else {
                    ui.horizontal_top(|ui| {
                        // Left: record structure tree - tall and wide (the main area for the data-items list).
                        // We explicitly set full height here so the ScrollArea inside show_structure
                        // gets the large red-rect size.
                        ui.vertical(|ui| {
                            ui.set_min_width(320.0);
                            ui.set_height(remaining_rect.height());
                            ui.spacing_mut().item_spacing.y = 0.0;

                            // When the data file does not exist yet (!finalized), allow adding/removing
                            // data-items in the FD (record structure). Once the file exists (finalized),
                            // structural changes are forbidden to avoid data loss/truncation/breaking code.
                            if !st.def.finalized {
                                ui.horizontal(|ui| {
                                    // Hand-written vector icon for Add (plus sign). Matches the project's
                                    // style for all toolbar/action icons (procedural strokes, no assets).
                                    // When the file prefers the raw COBOL editor, structural edits via the
                                    // pane are disabled (editor is the source of truth).
                                    let can_struct_edit = !st.prefer_raw_editor;
                                    if can_struct_edit && icon_btn(ui, egui::vec2(22.0, 18.0), "Add data-item", &|p, r, c| {
                                        let s = egui::Stroke::new(1.8, c);
                                        let cx = r.center().x; let cy = r.center().y;
                                        p.line_segment([egui::Pos2::new(cx - 5.0, cy), egui::Pos2::new(cx + 5.0, cy)], s);
                                        p.line_segment([egui::Pos2::new(cx, cy - 5.0), egui::Pos2::new(cx, cy + 5.0)], s);
                                    }) {
                                        st.panel.add_field(&mut st.def);
                                        did_add_remove = true;
                                    }

                                    let has_field = matches!(st.panel.selection, IndexedSelection::Field(_));
                                    // Hand-written vector icon for Remove (X). Consistent with delete icons elsewhere.
                                    if can_struct_edit && icon_btn(ui, egui::vec2(22.0, 18.0), "Remove selected data-item", &|p, r, c| {
                                        let s = egui::Stroke::new(1.8, c);
                                        let q = r.shrink(r.width() * 0.22);
                                        p.line_segment([q.left_top(), q.right_bottom()], s);
                                        p.line_segment([q.right_top(), q.left_bottom()], s);
                                    }) && has_field {
                                        if st.panel.remove_selected_field(&mut st.def) {
                                            did_add_remove = true;
                                        }
                                    }
                                });
                            }

                            st.panel.sync_from_def(&st.def);
                            structure_action = st.panel.show_structure(ui, &st.def, tr);
                        });

                        ui.separator();

                        // Right: properties area - we make this column tall too (to match the structure
                        // height), but put the actual labels+values content at the very top using
                        // horizontal_top + the row allocations already use Align::TOP.
                        // This ensures all property value controls (and labels) are top-aligned
                        // within their tall column instead of appearing vertically centered in the middle
                        // of the available area.
                        ui.vertical(|ui| {
                            ui.set_min_width(300.0);  // total width for the labels+values block
                            ui.set_height(remaining_rect.height());
                            ui.horizontal_top(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_min_width(140.0);
                                    ui.spacing_mut().item_spacing.y = 4.0;  // blank line gap so rows don't touch neighbors
                                    st.panel.show_property_labels(ui, &st.def, tr);
                                });

                                ui.separator();

                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 4.0;  // blank line gap so rows don't touch neighbors
                                    property_edit = st.panel.show_property_values(ui, &mut st.def, tr);
                                });
                            });
                        });
                    });
                }
            });
        }); // close the CentralPanel |ui| and .show(...)

        // Only open the raw modal if explicitly requested (e.g. the header icon
        // was clicked, or initial request after raw creation).
        // We no longer force the modal just because prefer_raw_editor is true,
        // because when that flag is set the *in-place* raw editor (see the
        // allocate_ui_at_rect branch above) *is* the visible form that replaced
        // the property pane. The modal is optional (can be opened via the raw
        // icon in the header). This ensures that "Apply" and the window X both
        // actually close the modal and it stays closed.
        if let Some(st) = &mut self.indexed_inspect {
            if st.panel.request_raw_dialog {
                st.panel.show_raw_dialog = true;
                st.panel.request_raw_dialog = false;
            }
        }

        // Raw text editor modal result (the "text editor" button opens the modal via open_raw_dialog above)
        // We promote prefer_raw *before* save so the .fd.cpy write receives the exact
        // user-provided COBOL text from the editor (not a canonical re-emit).
        let raw_dialog_applied = if let Some(st) = &mut self.indexed_inspect {
            st.panel.show_raw_dialog(ctx, &mut st.def, tr) == RawDialogResult::Applied
        } else {
            false
        };
        if raw_dialog_applied {
            // Lock to raw editor surface and persist the preference (like create path).
            if let Some(st) = &mut self.indexed_inspect {
                st.prefer_raw_editor = true;
                if st.panel.raw_text.trim().is_empty() {
                    st.panel.raw_text = record_to_text(&st.def);
                }
                self.raw_preferred_indexed.insert(st.path.clone());
            }
            // Persist using IDE-managed indexed file (like agent conversations).
            if let Some(root) = self.project_path.as_ref().and_then(|p| p.parent()) {
                let data_dir = root.join("data");
                let rels: std::collections::HashSet<String> = self
                    .raw_preferred_indexed
                    .iter()
                    .filter_map(|abs| {
                        abs.strip_prefix(root)
                            .ok()
                            .map(|r| r.to_string_lossy().replace('\\', "/"))
                    })
                    .collect();
                crate::llm::save_raw_preferred_indexed(&data_dir, &rels);
            }
            self.save_and_refresh_indexed();
        }

        if matches!(
            property_edit,
            PropertyEdit::Changed | PropertyEdit::Renamed { .. }
        ) {
            self.save_and_refresh_indexed();
        }

        if structure_action == StructureAction::StructureChanged {
            self.save_and_refresh_indexed();
        }

        if did_add_remove {
            self.save_and_refresh_indexed();
        }

        if open_grid {
            if let Some(st) = &self.indexed_inspect {
                let path = st.path.clone();
                let def = st.def.clone();
                self.open_grid_for_indexed(&path, &def);
            }
        }

        if close {
            self.indexed_inspect = None;
            self.show_project_settings = false;
        }
    }

    /// The active IDE colour theme. While the Settings form is open its **draft**
    /// theme wins, so picking a theme previews live (and reverts on Cancel);
    /// otherwise the saved project theme (or the default) is used.
    fn current_theme(&self) -> &'static crate::theme::Theme {
        let id = self
            .settings_form
            .as_ref()
            .map(|f| f.draft.theme_id.as_str())
            .or_else(|| self.cobolt_project.as_ref().map(|p| p.ide.theme.as_str()))
            .unwrap_or("");
        crate::theme::theme_by_id(id)
    }

    /// Absolute path of the project's IDE background image, if configured.
    fn bg_image_abs_path(&self) -> Option<PathBuf> {
        let proj = self.cobolt_project.as_ref()?;
        let raw = proj.ide.background_image.trim();
        if raw.is_empty() {
            return None;
        }
        let p = Path::new(raw);
        if p.is_absolute() {
            return Some(p.to_path_buf());
        }
        let dir = self.project_path.as_ref()?.parent()?;
        Some(dir.join(p))
    }

    /// Paint the per-project background image (if any) on the background layer of
    /// the main IDE window, scaled to cover, at the configured opacity. The
    /// translucent glass panels then blend over it.
    fn paint_ide_background(&mut self, ctx: &Context) {
        let opacity = match &self.cobolt_project {
            Some(p) => p.ide.background_opacity.min(100),
            None => return,
        };
        if opacity == 0 {
            return;
        }
        let Some(abs) = self.bg_image_abs_path() else {
            return;
        };

        let need_load = match &self.bg_texture {
            Some((p, _)) => p != &abs,
            None => true,
        };
        if need_load {
            match load_image_texture(ctx, &abs.display().to_string()) {
                Some(tex) => self.bg_texture = Some((abs.clone(), tex)),
                None => return,
            }
        }
        let Some((_, tex)) = &self.bg_texture else {
            return;
        };

        let screen = ctx.screen_rect();
        let tex_size = tex.size_vec2();
        if tex_size.x <= 0.0 || tex_size.y <= 0.0 {
            return;
        }
        // Cover: scale up so the image fills the window, centred.
        let s = (screen.width() / tex_size.x).max(screen.height() / tex_size.y);
        let dw = tex_size.x * s;
        let dh = tex_size.y * s;
        let ox = (screen.width() - dw) * 0.5;
        let oy = (screen.height() - dh) * 0.5;
        let dest = egui::Rect::from_min_size(screen.min + egui::vec2(ox, oy), egui::vec2(dw, dh));
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));

        // Draw the image (cover) over the opaque floor, scaled by
        // `background_opacity` so the texture shows through more or less.
        let img_a = (opacity as f32 / 100.0 * 255.0) as u8;
        ctx.layer_painter(egui::LayerId::background()).image(
            tex.id(),
            dest,
            uv,
            egui::Color32::from_white_alpha(img_a),
        );
    }

    /// Poll an in-flight "Test connection" request started from the Settings
    /// form, updating the status line shown next to the Test button.
    fn poll_llm_test(&mut self, tr: &Tr) {
        if let Some(rx) = &self.llm_test_rx {
            match rx.try_recv() {
                Ok(crate::llm::LlmResponse::Ok(_)) => {
                    self.llm_test_status = Some(tr.ai_test_ok.to_string());
                    self.llm_test_rx = None;
                }
                Ok(crate::llm::LlmResponse::Err(e)) => {
                    self.llm_test_status = Some(e);
                    self.llm_test_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.llm_test_status = Some("The test worker stopped unexpectedly.".into());
                    self.llm_test_rx = None;
                }
            }
        }
    }

    /// Persist the Settings form: write the draft into the project + global AI
    /// config, save both to disk, and apply the (possibly new) theme/background.
    fn save_settings_form(&mut self) {
        let Some(form) = &mut self.settings_form else {
            return;
        };
        let Some(proj) = &mut self.cobolt_project else {
            return;
        };
        form.draft.apply(proj, &mut self.llm);
        form.mark_saved();
        if let Err(e) = self.llm.save() {
            tracing::warn!("could not save AI settings: {e}");
        }
        self.do_save_project();
        self.bg_texture = None; // force the background image to reload
    }

    /// Render the project Settings form in the Main Pane. Returns the pending
    /// "Test connection" / "Browse background" actions for the caller to run.
    fn show_settings_pane(&mut self, ctx: &Context, tr: &Tr) {
        self.poll_llm_test(tr);
        if self.llm_test_rx.is_some() {
            ctx.request_repaint();
        }
        let test_busy = self.llm_test_rx.is_some();
        let test_status = self.llm_test_status.clone();

        let themes: Vec<(&'static str, &'static str)> = crate::theme::THEMES
            .iter()
            .map(|t| (t.id, t.name))
            .collect();

        let mut action = crate::panels::settings_form::SettingsFormAction::default();

        // Mirror the exact right-pane implementation used for the control
        // properties inspector: create the glass card, then CentralPanel with
        // .frame(card). This guarantees identical width/positioning of the
        // glass strokes (right border fixed) and that the pane area conforms to
        // 100% of the available central height above the output (grows/shrinks
        // naturally on window or output splitter resize).
        let mut card =
            crate::theme::glass_panel_frame(ctx.style().visuals.panel_fill, self.current_theme());
        // Moderate bottom outer margin on the frame raises the stroked glass
        // card (rounded bottom border) clearly above the output.
        // Inside the framed ui we allocate the form (scroll + buttons) in a
        // shorter rect + reserve space so the Save/Cancel sit fully visible
        // above the console (the 80px inner reservation directly lifts the
        // buttons within the glass). This fixes the clipping while keeping the
        // overall "right pane" full 100% height conforming.
        card = card.outer_margin(egui::Margin {
            left: 6.0,
            right: 6.0,
            top: 6.0,
            bottom: 50.0,
        });
        egui::CentralPanel::default().frame(card).show(ctx, |ui| {
            if let Some(form) = &mut self.settings_form {
                let avail = ui.available_rect_before_wrap();
                let bottom_res = 80.0; // dedicated inner lift for full button visibility
                let content_h = (avail.height() - bottom_res).max(180.0);
                let content_rect =
                    egui::Rect::from_min_size(avail.min, egui::vec2(avail.width(), content_h));
                ui.allocate_ui_at_rect(content_rect, |ui| {
                    action = form.show(ui, tr, &themes, test_busy, test_status.as_deref());
                });
                ui.add_space(bottom_res);
            }
        });

        if action.save {
            self.save_settings_form();
        }
        if action.test_connection {
            if let Some(form) = &self.settings_form {
                let mut cfg = self.llm.clone();
                cfg.endpoint = form.draft.llm_endpoint.clone();
                cfg.api_key = form.draft.llm_api_key.clone();
                cfg.model = form.draft.llm_model.clone();
                self.llm_test_status = Some(tr.ai_testing.to_string());
                self.llm_test_rx = Some(crate::llm::spawn_test(&cfg));
            }
        }
        if action.browse_bg {
            self.begin_file_dialog(
                FileRequest::PickBackgroundImage,
                crate::file_dialog::DialogSpec::open()
                    .filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp"]),
            );
        }
    }

    /// The PowerRustCOBOL mascot shown in the Main Pane when no project is open.
    fn show_mascot_pane(&mut self, ctx: &Context, tr: &Tr) {
        let card =
            crate::theme::glass_panel_frame(ctx.style().visuals.panel_fill, self.current_theme());
        egui::CentralPanel::default().frame(card).show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.18);
                let tex = self.mascot_texture(ctx);
                if let Some(tex) = tex {
                    let max = (ui.available_width() * 0.6).min(420.0);
                    let size = tex.size_vec2();
                    let scale = (max / size.x).min(1.0);
                    ui.image((tex.id(), size * scale));
                }
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(tr.no_project_open)
                        .size(15.0)
                        .color(self.current_theme().text_dim),
                );
            });
        });
    }

    /// Lazily decode + upload the embedded mascot PNG as a texture.
    fn mascot_texture(&mut self, ctx: &Context) -> Option<egui::TextureHandle> {
        if let Some((_, t)) = &self.bg_texture {
            // reuse a separate cache slot? keep mascot in its own ctx-memory cache.
        }
        // Mascot texture is no longer used (welcome pane with the dev guide
        // replaces the old no-project mascot view). Stub to avoid include
        // path issues in the current tree while keeping the fn for any
        // other call sites.
        let _ = ctx;
        None
    }

    /// Shown on startup (or when no project is open) as a single full-width
    /// pane below the menubar/toolbar. Centered text with cycling quotes
    /// using the exact requested format and timings.
    fn show_welcome_pane(&mut self, ctx: &Context, tr: &Tr) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ctx.request_repaint(); // ensure continuous animation for quote rotation

            // ── Daily background image (assets/images/bg<day>.jpg) ────────────
            // One image per day of the month; bg1.jpg is the fallback when the
            // day's image is absent. Stretched to fill the whole pane.
            if let Some(tex) = welcome_bg_texture(ctx) {
                let rect = ui.max_rect();
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                ui.painter().image(tex.id(), rect, uv, egui::Color32::WHITE);
                // A gentle dark scrim keeps the white/coloured text legible over
                // any photo.
                ui.painter()
                    .rect_filled(rect, 0.0, egui::Color32::from_black_alpha(90));
            }

            // ── Pick the current quote (from the active language's pool) ──────
            let pool = crate::welcome::quotes(self.lang);
            let idx = self.welcome_quote_index % pool.len();
            let (author, quote) = pool[idx];
            let now = ctx.input(|i| i.time);
            if self.welcome_quote_start_time == 0.0 {
                self.welcome_quote_start_time = now;
            }
            const CYCLE: f64 = 7.5;
            if now - self.welcome_quote_start_time > CYCLE {
                self.welcome_quote_index = rand::thread_rng().gen_range(0..pool.len());
                self.welcome_quote_start_time = now;
            }
            let elapsed = now - self.welcome_quote_start_time;
            let alpha = if elapsed < 1.0 {
                (elapsed / 1.0) as f32
            } else if elapsed < 7.0 {
                1.0
            } else if elapsed < 7.5 {
                ((7.5 - elapsed) / 0.5) as f32
            } else {
                0.0
            };

            let title = tr
                .welcome_title
                .replace("{}", &format!("PowerRustCOBOL {}", crate::version::VERSION));
            let license = tr.welcome_license;
            let author_line = format!("— {}", author);

            // ── Measure the whole block so it can be centred precisely ────────
            // (the old fixed 170 px estimate drifted, especially when the quote
            // wrapped). Item spacing is zeroed and the gaps inserted explicitly,
            // so the measured height matches what is drawn exactly.
            const TITLE_SIZE: f32 = 42.0; // 50 % larger than the previous 28
            const GAP_LICENSE: f32 = 4.0; // title → license
            const GAP_QUOTE: f32 = 40.0; // license → quote (two blank lines)
            const GAP_AUTHOR: f32 = 10.0; // quote → author
            let avail_w = ui.available_width();
            let line_h = |text: &str, size: f32| {
                ui.fonts(|f| {
                    f.layout_no_wrap(
                        text.to_owned(),
                        egui::FontId::proportional(size),
                        egui::Color32::WHITE,
                    )
                    .size()
                    .y
                })
            };
            let quote_h = ui.fonts(|f| {
                f.layout(
                    quote.to_owned(),
                    egui::FontId::proportional(16.0),
                    egui::Color32::WHITE,
                    avail_w,
                )
                .size()
                .y
            });
            let block_h = line_h(&title, TITLE_SIZE)
                + GAP_LICENSE
                + line_h(license, 16.0)
                + GAP_QUOTE
                + quote_h
                + GAP_AUTHOR
                + line_h(&author_line, 15.0);

            let green = egui::Color32::from_rgb(100, 220, 100);
            let light_blue = egui::Color32::from_rgb(130, 190, 255);

            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0; // gaps are inserted explicitly
                let top = ((ui.available_height() - block_h) * 0.5).max(0.0);
                ui.add_space(top);

                ui.label(
                    egui::RichText::new(&title)
                        .size(TITLE_SIZE)
                        .strong()
                        .color(egui::Color32::WHITE),
                );
                ui.add_space(GAP_LICENSE);
                ui.label(
                    egui::RichText::new(license)
                        .size(14.0)
                        .color(egui::Color32::WHITE),
                );
                ui.add_space(GAP_QUOTE);
                ui.label(
                    egui::RichText::new(quote)
                        .size(16.0)
                        .color(green.gamma_multiply(alpha)),
                );
                ui.add_space(GAP_AUTHOR);
                ui.label(
                    egui::RichText::new(&author_line)
                        .size(15.0)
                        .italics()
                        .color(light_blue.gamma_multiply(alpha)),
                );
            });
        });
    }

    /// True if the project Settings form has unsaved edits.
    fn settings_dirty(&self) -> bool {
        self.settings_form
            .as_ref()
            .map(|f| f.is_dirty())
            .unwrap_or(false)
    }

    /// Open a file in the editor, marking RAD-generated COBOL read-only (blue).
    fn open_in_editor(&mut self, path: PathBuf) {
        let read_only = self.path_is_generated(&path);
        self.editor.open_file_ro(path, read_only);
    }

    /// True when `path` is RAD-generated code in the open project (read-only).
    fn path_is_generated(&self, path: &std::path::Path) -> bool {
        if let (Some(proj), Some(pp)) = (&self.cobolt_project, &self.project_path) {
            if let Some(dir) = pp.parent() {
                if let Some(rel) = relative_to(path, dir) {
                    return proj.is_generated(&rel);
                }
            }
        }
        false
    }

    fn do_generate_cobol(&mut self, idx: usize) {
        if idx >= self.designers.len() {
            return;
        }
        let cbl_path = self.generated_cbl_path(&self.designers[idx].0);
        if let Some(parent) = cbl_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let cobol = generate(&self.designers[idx].1.form);
        match std::fs::write(&cbl_path, &cobol) {
            Ok(()) => {
                self.output
                    .push_status(format!("Generated {}", cbl_path.display()));
                // Auto-add to project if applicable.
                let proj_dir = self
                    .project_path
                    .as_ref()
                    .and_then(|p| p.parent())
                    .map(|p| p.to_owned());
                if let Some(dir) = proj_dir {
                    if let Some(rel) = relative_to(&cbl_path, &dir) {
                        if let Some(proj) = &mut self.cobolt_project {
                            proj.add_generated(&rel); // RAD output → read-only
                        }
                        self.do_save_project();
                    }
                }
                // Queue the file to be opened in the editor next frame.
                self.pending_open_in_editor = Some(cbl_path);
            }
            Err(e) => {
                self.output.push_status(format!("Generate failed: {e}"));
            }
        }
    }

    // ── Report Bug ────────────────────────────────────────────────────────────

    /// Path to the BUGS.md file — looks for it relative to the project root,
    /// falling back to the open project path, then the current working dir.
    fn bugs_md_path(&self) -> std::path::PathBuf {
        // If a project is open, use the project directory.
        if let Some(pp) = &self.project_path {
            if let Some(dir) = pp.parent() {
                let p = dir.join("BUGS.md");
                if p.exists() {
                    return p;
                }
                // Create it alongside the project if it doesn't exist yet.
                return p;
            }
        }
        // Fall back to the workspace root (look for Cargo.toml with [workspace]).
        let mut dir = std::env::current_dir().unwrap_or_default();
        loop {
            let candidate = dir.join("BUGS.md");
            if candidate.exists() {
                return candidate;
            }
            let toml = dir.join("Cargo.toml");
            if toml.exists() {
                if let Ok(t) = std::fs::read_to_string(&toml) {
                    if t.contains("[workspace]") {
                        return candidate;
                    }
                }
            }
            match dir.parent() {
                Some(p) => dir = p.to_owned(),
                None => break,
            }
        }
        std::path::PathBuf::from("BUGS.md")
    }

    fn show_save_alert(&mut self, ctx: &Context) {
        let form_name = match &self.save_alert_msg {
            Some(n) => n.clone(),
            None => return,
        };
        let tr = self.lang.tr();
        let msg = tr.alert_form_saved.replacen("{}", &form_name, 1);
        let mut open = true;

        egui::Window::new("✅")
            .id(egui::Id::new("save_alert"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(&msg).size(15.0).strong());
                ui.add_space(8.0);
                if ui.button("OK").clicked() {
                    self.save_alert_msg = None;
                    self.save_alert_designer = None;
                }
            });

        if !open {
            self.save_alert_msg = None;
            self.save_alert_designer = None;
        }
    }

    /// A centred modal "Building…" popup with an animated progress bar, shown
    /// while a binary build is in flight. It disappears on the frame the build
    /// finishes (its receiver is drained earlier in `update`), i.e. right before
    /// the result reaches the Output panel.
    fn show_about(&mut self, ctx: &Context) {
        if !self.about_open {
            return;
        }
        let mut open = self.about_open;
        egui::Window::new("About PowerRustCOBOL")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    ui.add(
                        egui::Image::new(egui::include_image!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../../assets/images/powerrustcobol-mascot.png"
                        )))
                        .max_height(180.0),
                    );
                    ui.add_space(8.0);
                    ui.heading("PowerRustCOBOL");
                    ui.label(format!("Version {VERSION}"));
                    ui.add_space(4.0);
                    ui.label("A modern, Rust-powered RAD environment for COBOL.");
                    ui.add_space(6.0);
                    ui.label("© 2026 Emerson Lopes and PowerRustCOBOL contributors");
                    ui.label("Distributed under the Apache 2.0 License.");
                    ui.add_space(12.0);
                    if ui.button("Close").clicked() {
                        self.about_open = false;
                    }
                    ui.add_space(4.0);
                });
            });
        if !open {
            self.about_open = false;
        }
    }

    fn show_building_modal(&mut self, ctx: &Context) {
        if self.pending_build_rx.is_none() {
            return;
        }
        // Dim the rest of the IDE so the build reads as modal.
        let screen = ctx.screen_rect();
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("building_dim"),
        ))
        .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(150));

        // Drain any phase updates that arrived since the last frame.
        let mut latest = None;
        if let Some(prx) = &self.pending_build_progress {
            while let Ok(p) = prx.try_recv() {
                latest = Some(p);
            }
        }
        if let Some(p) = latest {
            self.build_phase = (p.fraction, p.message);
        }
        let (frac, msg) = (self.build_phase.0, self.build_phase.1.clone());
        ctx.request_repaint(); // keep polling while the build runs

        egui::Window::new("Building…")
            .id(egui::Id::new("building_modal"))
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                // Determinate bar driven by the real build fraction.
                ui.add(
                    egui::ProgressBar::new(frac.clamp(0.0, 1.0))
                        .desired_width(280.0)
                        .show_percentage(),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    let label = if msg.is_empty() {
                        "Starting…"
                    } else {
                        msg.as_str()
                    };
                    ui.label(egui::RichText::new(label).size(13.0));
                });
                ui.add_space(4.0);
            });
    }

    fn show_report_bug_dialog(&mut self, ctx: &Context) {
        if !self.report_bug.open {
            return;
        }

        let mut open = true;
        egui::Window::new("🐛 Report a Problem")
            .collapsible(false)
            .resizable(true)
            .min_width(420.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Describe the problem so it can be tracked and fixed:");
                ui.add_space(6.0);

                egui::Grid::new("bug_form")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Component:");
                        ui.text_edit_singleline(&mut self.report_bug.component);
                        ui.end_row();

                        ui.label("Title:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.report_bug.title)
                                .hint_text("One-line summary of the problem")
                                .desired_width(f32::INFINITY),
                        );
                        ui.end_row();

                        ui.label("Description:");
                        ui.add(
                            egui::TextEdit::multiline(&mut self.report_bug.description)
                                .hint_text(
                                    "Steps to reproduce, what went wrong, what you expected…",
                                )
                                .desired_width(f32::INFINITY)
                                .desired_rows(4),
                        );
                        ui.end_row();
                    });

                ui.add_space(4.0);

                if let Some(fb) = &self.report_bug.feedback {
                    let color = if fb.starts_with('✅') {
                        egui::Color32::from_rgb(80, 220, 120)
                    } else {
                        egui::Color32::from_rgb(255, 120, 80)
                    };
                    ui.colored_label(color, fb.clone());
                    ui.add_space(4.0);
                }

                ui.horizontal(|ui| {
                    if ui.button("Submit to BUGS.md").clicked() {
                        let path = self.bugs_md_path();
                        match self.report_bug.submit(&path) {
                            Ok(()) => {
                                self.report_bug.feedback = Some(format!(
                                    "✅ Saved to {}  — next scan will pick it up.",
                                    path.display()
                                ));
                            }
                            Err(e) => {
                                self.report_bug.feedback = Some(format!("❌ {e}"));
                            }
                        }
                    }
                    if ui.button("Close").clicked() {
                        self.report_bug.open = false;
                    }
                });
            });

        if !open {
            self.report_bug.open = false;
        }
    }

    // ── Keyboard shortcuts (main window) ─────────────────────────────────────

    fn handle_shortcuts(&mut self, ctx: &Context) {
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S)))
        {
            self.do_save();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::R)))
            && !self.runner.is_running()
        {
            self.do_run();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::O)))
        {
            self.do_open();
        }
    }

    // ── Dialogs ───────────────────────────────────────────────────────────────

    fn show_new_project_dialog(&mut self, ctx: &Context) {
        if !self.new_project.open {
            return;
        }
        let tr = self.lang.tr();
        let mut open = true;
        egui::Window::new(tr.dlg_new_project)
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                egui::Grid::new("npg")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(tr.dlg_proj_name);
                        ui.text_edit_singleline(&mut self.new_project.name);
                        ui.end_row();
                        ui.label(tr.dlg_proj_version);
                        ui.text_edit_singleline(&mut self.new_project.version);
                        ui.end_row();
                        ui.label(tr.dlg_proj_main);
                        ui.text_edit_singleline(&mut self.new_project.main);
                        ui.end_row();
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr.dlg_create_dots).clicked() {
                        self.create_new_project();
                    }
                    if ui.button(tr.dlg_cancel).clicked() {
                        self.new_project.open = false;
                    }
                });
            });
        if !open {
            self.new_project.open = false;
        }
    }

    fn show_new_form_dialog(&mut self, ctx: &Context) {
        if !self.new_form.open {
            return;
        }
        let tr = self.lang.tr();
        let mut open = true;
        egui::Window::new(tr.dlg_new_form)
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                egui::Grid::new("nfg")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(tr.dlg_form_id);
                        ui.text_edit_singleline(&mut self.new_form.form_name);
                        ui.end_row();
                        ui.label(tr.dlg_form_title);
                        ui.text_edit_singleline(&mut self.new_form.title);
                        ui.end_row();
                        ui.label(tr.dlg_form_width);
                        ui.text_edit_singleline(&mut self.new_form.width);
                        ui.end_row();
                        ui.label(tr.dlg_form_height);
                        ui.text_edit_singleline(&mut self.new_form.height);
                        ui.end_row();
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr.dlg_create).clicked() {
                        self.create_new_form();
                    }
                    if ui.button(tr.dlg_cancel).clicked() {
                        self.new_form.open = false;
                    }
                });
            });
        if !open {
            self.new_form.open = false;
        }
    }

    fn show_new_indexed_dialog(&mut self, ctx: &Context) {
        if !self.new_indexed.open {
            return;
        }
        let tr = self.lang.tr();
        let mut open = true;
        egui::Window::new(tr.dlg_new_indexed)
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| match self.new_indexed.show(ui, &tr) {
                NewIndexedAction::Create => self.create_new_indexed_file(),
                NewIndexedAction::Cancel => self.new_indexed.open = false,
                NewIndexedAction::None => {}
            });
        if !open {
            self.new_indexed.open = false;
        }
    }

    fn create_new_form(&mut self) {
        let w: u32 = self.new_form.width.parse().unwrap_or(640);
        let h: u32 = self.new_form.height.parse().unwrap_or(480);
        let mut form = Form::new(
            self.new_form.form_name.clone(),
            self.new_form.title.clone(),
            w,
            h,
        );
        form.background_color = "00000000".into(); // transparent — matches IDE glass

        let default_name = format!("{}.cfrm", self.new_form.form_name.to_lowercase());
        let mut spec = crate::file_dialog::DialogSpec::save()
            .filter("RustCOBOL Form", &["cfrm"])
            .file_name(default_name);
        // Default into the project's forms/ folder when a project is open.
        if let Some(dir) = self.project_dir() {
            let forms = dir.join("forms");
            let _ = std::fs::create_dir_all(&forms);
            spec = spec.directory(forms);
        }
        self.begin_file_dialog(FileRequest::NewForm(Box::new(form)), spec);
    }

    /// Save a freshly-created form to `path`, register it, and open its designer.
    fn save_new_form_to(&mut self, form: Form, path: PathBuf) {
        if let Err(e) = save_form(&form, &path) {
            self.output
                .push_status(format!("Could not save new form: {e}"));
            return;
        }
        if let Some(parent) = path.parent() {
            self.forms_list.set_root(parent);
            if self.cobolt_project.is_none() {
                self.project.set_root(parent);
            }
        }
        // Auto-add to project
        let proj_dir = self
            .project_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_owned());
        if let Some(dir) = proj_dir {
            if let Some(rel) = relative_to(&path, &dir) {
                if let Some(proj) = &mut self.cobolt_project {
                    proj.add_file(&rel);
                }
                self.do_save_project();
            }
        }
        let mut dp = DesignerPanel::new(form);
        dp.cfrm_dir = path.parent().map(|p| p.to_path_buf());
        self.designers.push((path, dp));
        self.new_form.open = false;
    }

    // ── Async file-dialog plumbing ──────────────────────────────────────────────

    /// Open an app-level file dialog without blocking the event loop and record
    /// what to do with the result (applied by `apply_file_result`).
    fn begin_file_dialog(&mut self, request: FileRequest, spec: crate::file_dialog::DialogSpec) {
        self.pending_file = Some(request);
        crate::file_dialog::begin(&self.egui_ctx, APP_FILE_KEY, spec);
    }

    /// Drain a finished app-level file dialog (call once per frame). Returns
    /// whether a dialog is still open (so the caller keeps repainting).
    fn poll_file_dialog(&mut self) -> bool {
        if let Some(result) = crate::file_dialog::take(APP_FILE_KEY) {
            if let Some(request) = self.pending_file.take() {
                if let Some(path) = result {
                    self.apply_file_result(request, path);
                }
            }
        }
        self.pending_file.is_some()
    }

    /// Perform the action associated with a completed file dialog.
    fn apply_file_result(&mut self, request: FileRequest, path: PathBuf) {
        match request {
            FileRequest::OpenCobol => {
                if let Some(parent) = path.parent() {
                    if self.cobolt_project.is_none() {
                        self.project.set_root(parent);
                    }
                    self.forms_list.set_root(parent);
                }
                self.open_in_editor(path);
            }
            FileRequest::CreateProject => self.create_new_project_at(path),
            FileRequest::OpenProject => self.open_project_at(path),
            FileRequest::SaveProject => {
                self.project_path = Some(path);
                self.do_save_project();
            }
            FileRequest::PackageProject => self.package_project_to(path),
            FileRequest::AddFile(kind) => self.add_file_to_project_path(kind, path),
            FileRequest::OpenForm => self.load_form_from_path(path),
            FileRequest::NewForm(form) => self.save_new_form_to(*form, path),
            FileRequest::PickBackgroundImage => self.set_background_image(path),
        }
    }

    /// Store the chosen background image in the project's IDE settings
    /// (relative to the project root when possible), persist, and drop the
    /// texture cache so it reloads.
    fn set_background_image(&mut self, path: PathBuf) {
        let rel = self
            .project_path
            .as_ref()
            .and_then(|p| p.parent())
            .and_then(|dir| relative_to(&path, dir))
            .unwrap_or_else(|| path.display().to_string());
        if let Some(proj) = &mut self.cobolt_project {
            proj.ide.background_image = rel;
            self.bg_texture = None;
            self.do_save_project();
        }
    }
}

// ── Liquid Glass visuals ──────────────────────────────────────────────────────

fn apply_glass_visuals(ctx: &Context, theme: &crate::theme::Theme) {
    use egui::Color32;
    use egui::{style::WidgetVisuals, Rounding, Shadow, Stroke, Visuals};

    // Publish the editor palette for this theme so the syntax layouter picks it up.
    crate::theme::set_active(theme);

    let mut v = if theme.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    // Panels keep a consistent semi-opaque fill (the background painter draws a
    // matching base so the area *outside* the panes looks the same as the panes).
    // ── Theme palette ─────────────────────────────────────────────────────
    let bg_panel = theme.bg_panel;
    let bg_control = theme.bg_control;
    let bg_hover = theme.bg_hover;
    let bg_active = theme.bg_active;
    let bg_extreme = theme.bg_extreme;
    let accent = theme.accent;
    let border_dim = theme.border_dim;
    let border_hi = theme.border_hi;
    let text_dim = theme.text_dim;
    let text_bright = theme.text_bright;

    // ── Window / panel fills ──────────────────────────────────────────────
    v.window_fill = bg_panel;
    v.panel_fill = bg_panel;
    v.faint_bg_color = theme.faint_bg;
    v.extreme_bg_color = bg_extreme;
    v.code_bg_color = theme.code_bg;

    // ── Window chrome ─────────────────────────────────────────────────────
    v.window_stroke = Stroke::new(1.0, border_hi);
    v.window_shadow = Shadow {
        offset: Vec2::new(0.0, 10.0),
        blur: 40.0,
        spread: 0.0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 100),
    };
    v.window_rounding = Rounding::same(12.0);
    v.window_highlight_topmost = false;

    // ── Control states ─────────────────────────────────────────────────────
    let make_widget = |bg: Color32, stroke_c: Color32, text: Color32| WidgetVisuals {
        weak_bg_fill: bg,
        bg_fill: bg,
        bg_stroke: Stroke::new(1.0, stroke_c),
        fg_stroke: Stroke::new(1.5, text),
        rounding: Rounding::same(8.0),
        expansion: 0.0,
    };

    v.widgets.noninteractive = make_widget(bg_control, border_dim, text_dim);
    v.widgets.inactive = make_widget(bg_control, border_dim, text_dim);
    v.widgets.hovered = make_widget(bg_hover, border_hi, text_bright);
    // NOTE: egui derives `strong_text_color()` from the ACTIVE control text, so
    // this must be the theme's bright text (dark on light themes, light on
    // dark ones) — a hardcoded white washes out every `.strong()` label on
    // light themes.
    v.widgets.active = make_widget(bg_active, accent, text_bright);
    v.widgets.open = make_widget(bg_hover, border_hi, text_bright);

    // Keep separators / dividers very faint (the prominent light-grey lines were
    // too noisy). Use the theme's dim border colour.
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, theme.border_dim);

    // ── Selection ─────────────────────────────────────────────────────────
    v.selection.bg_fill = theme.selection;
    v.selection.stroke = Stroke::new(1.0, accent);

    // ── Text / decorations ────────────────────────────────────────────────
    v.override_text_color = None;
    v.hyperlink_color = theme.hyperlink;
    v.warn_fg_color = theme.warn;
    v.error_fg_color = theme.error;

    ctx.set_visuals(v);

    // Polished spacing + fonts 50 % larger (absolute → idempotent each frame).
    // Roomier rows/padding for a less cramped, more professional feel.
    use egui::{FontFamily, FontId, TextStyle};
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::Vec2::new(8.0, 8.0);
    style.spacing.button_padding = egui::Vec2::new(12.0, 7.0);
    style.spacing.indent = 20.0;
    style.spacing.window_margin = egui::Margin::same(12.0);
    style.spacing.menu_margin = egui::Margin::same(8.0);
    style.spacing.interact_size.y = 30.0;
    // No vertical indent guide lines in the tree (the grey lines looked noisy).
    style.visuals.indent_has_left_vline = false;
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(11.5, FontFamily::Proportional),
        ),
        (
            TextStyle::Body,
            FontId::new(16.75, FontFamily::Proportional),
        ),
        (
            TextStyle::Button,
            FontId::new(16.75, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(25.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(16.0, FontFamily::Monospace),
        ),
    ]
    .into();
    ctx.set_style(style);
}

/// Apply the IDE theme to an **opaque** child viewport (designer, indexed editor,
/// grid browser). Semi-transparent glass colours are composited over white so
/// gaps between panels never show the OS clear colour.
fn apply_opaque_viewport_theme(ctx: &Context, theme: &crate::theme::Theme) {
    apply_glass_visuals(ctx, theme);

    let solid_panel = {
        let pf = ctx.style().visuals.panel_fill;
        let a = pf.a() as f32 / 255.0;
        let blend = |c: u8| (c as f32 * a + 255.0 * (1.0 - a)).round() as u8;
        egui::Color32::from_rgb(blend(pf.r()), blend(pf.g()), blend(pf.b()))
    };
    {
        let mut v = ctx.style().visuals.clone();
        v.panel_fill = solid_panel;
        v.window_fill = solid_panel;
        ctx.set_visuals(v);
    }
    ctx.layer_painter(egui::LayerId::background())
        .rect_filled(ctx.screen_rect(), 0.0, solid_panel);
}

// ── eframe::App ───────────────────────────────────────────────────────────────

impl eframe::App for CoboltApp {
    /// Clear to fully transparent so the OS compositor blends our semi-transparent
    /// panels directly against the desktop wallpaper.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // ── Compute the translation table for this frame ───────────────────────
        let tr = self.lang.tr();
        crate::i18n::set_language(ctx, self.lang);

        // 007 Form themes — publish the picker choices (Liquid Glass + discovered
        // packs) so the Settings form and the per-form Appearance pane list them.
        self.publish_theme_choices(ctx);

        // Intercept main window close if the project Settings form has unsaved changes.
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.settings_dirty() && !self.settings_close_confirm {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.settings_close_confirm = true;
            }
        }

        // ── Indexed editor / grid viewports (before main-shell theme) ─────────
        // egui 0.29 shares `set_visuals` globally; paint opaque child windows first
        // so `apply_glass_visuals` below can restore the translucent main shell.
        self.show_indexed_grid_viewports(ctx, &tr);

        // ── Apply Liquid Glass visuals every frame on the root context ─────────
        // (preview window calls ctx.set_visuals() on its viewport which in egui
        //  0.29 is global — re-applying here ensures the IDE shell always looks
        //  correct even when a preview window is open.)
        apply_glass_visuals(ctx, self.current_theme());
        self.glass_visuals_applied = true;

        // ── Opaque background that matches the panes ───────────────────────────
        // 1) an opaque dark floor (no desktop bleed), 2) the optional background
        // image as a subtle texture, 3) the SAME semi-opaque pane fill over the
        // whole window, so the area around/between the panes looks exactly like a
        // pane (not a brighter "transparent" wallpaper showing through the gaps).
        {
            let p = ctx.style().visuals.panel_fill;
            let floor = egui::Color32::from_rgb(p.r(), p.g(), p.b());
            ctx.layer_painter(egui::LayerId::background()).rect_filled(
                ctx.screen_rect(),
                0.0,
                floor,
            );
        }
        self.paint_ide_background(ctx);
        {
            let p = ctx.style().visuals.panel_fill;
            ctx.layer_painter(egui::LayerId::background())
                .rect_filled(ctx.screen_rect(), 0.0, p);
        }

        // ── Drain a finished async file dialog (Open/Save/Browse) ──────────────
        // Repaint while one is open so its result is collected promptly.
        if self.poll_file_dialog() {
            ctx.request_repaint();
        }

        // ── Drain runner output ───────────────────────────────────────────────
        let msgs = self.runner.drain_output();
        for msg in &msgs {
            self.output.push_msg(msg);
            if let RunMsg::Diagnostic(d) = msg {
                if let Some((path, _)) = self.editor.active_source() {
                    let path = path.clone();
                    self.editor.add_diag(&path, d.clone());
                }
            }
        }
        if self.runner.is_finished() {
            self.runner.clear();
        }

        // ── Drain debugger events ─────────────────────────────────────────────
        if self.debug_active {
            let dirty = self.debugger.process(&mut self.debug_runner);
            // Forward output/diagnostic messages to the output panel.
            for msg in self.debugger.pending_output.drain(..) {
                self.output.push_msg(&msg);
            }
            // Sync current paused line to editor gutter highlight.
            let dbg_line = self.debugger.current_line();
            if let Some((path, _)) = self.editor.active_source() {
                let path = path.clone();
                if dbg_line > 0 {
                    self.editor.debug_line = Some((path, dbg_line));
                } else {
                    self.editor.debug_line = None;
                }
            }
            if !self.debug_runner.is_running() {
                self.debug_active = false;
                self.debugger.reset();
                self.editor.debug_line = None;
            }
            if dirty {
                ctx.request_repaint();
            }
        }

        // ── Drain binary build result (Phase 11) ─────────────────────────────
        if let Some(rx) = &self.pending_build_rx {
            match rx.try_recv() {
                Ok(Ok(result)) => {
                    self.output.push_status(format!(
                        "✅ Build complete!  Binary → {}   ({} source(s), {} form(s), {} bytes AST)",
                        result.binary_path.display(),
                        result.source_count,
                        result.form_count,
                        result.ast_bytes,
                    ));
                    self.pending_build_rx = None;
                    self.pending_build_progress = None;
                }
                Ok(Err(e)) => {
                    self.output.push_status(format!("❌ Build failed: {e}"));
                    self.pending_build_rx = None;
                    self.pending_build_progress = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint(); // keep polling
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.output
                        .push_status("❌ Build thread disconnected unexpectedly.");
                    self.pending_build_rx = None;
                    self.pending_build_progress = None;
                }
            }
        }

        // ── Pending editor open from a designer window ────────────────────────
        if let Some(path) = self.pending_open_in_editor.take() {
            self.open_in_editor(path);
            // If a paragraph jump was queued (event row double-click), perform it
            // now that the freshly-generated file is the active editor tab.
            if let Some(para) = self.pending_goto_paragraph.take() {
                self.editor.goto_paragraph(&para);
            }
        } else if let Some(para) = self.pending_goto_paragraph.take() {
            self.editor.goto_paragraph(&para);
        }

        // ── Keyboard shortcuts ────────────────────────────────────────────────
        self.handle_shortcuts(ctx);

        // ── Dialogs ───────────────────────────────────────────────────────────
        self.show_new_project_dialog(ctx);
        self.show_new_form_dialog(ctx);
        self.show_new_indexed_dialog(ctx);
        self.show_report_bug_dialog(ctx);
        self.show_about(ctx);
        // Save alert: render it in the MAIN window only when it doesn't belong
        // to a designer viewport (those render it themselves, on top).
        if self.save_alert_designer.is_none() {
            self.show_save_alert(ctx);
        }
        // "Building…" progress modal (closes right before the result is shown).
        self.show_building_modal(ctx);

        // ── Menu bar ─────────────────────────────────────────────────────────
        let has_project = self.cobolt_project.is_some();
        // "Active" = a project is open or a file is being edited; gates the
        // Run / View menus (and the Save/Check toolbar buttons below).
        let menu_has_active = has_project || self.editor.active_source().is_some();
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button(tr.menu_file, |ui| {
                    if ui.button(tr.menu_new_project).clicked()     { self.do_new_project();  ui.close_menu(); }
                    if ui.button(tr.menu_open_project).clicked()    { self.do_open_project(); ui.close_menu(); }
                    if ui.add_enabled(has_project, egui::Button::new(tr.menu_save_project)).clicked() {
                        self.do_save_project(); ui.close_menu();
                    }
                    if ui.add_enabled(has_project, egui::Button::new(tr.menu_package_project)).clicked() {
                        self.do_package_project(); ui.close_menu();
                    }
                    let building = self.pending_build_rx.is_some();
                    let build_label = if building { "⏳ Building…" } else { "🔨 Build Binary  (bin/)" };
                    if ui.add_enabled(has_project && !building, egui::Button::new(build_label))
                        .on_hover_text("Compile project → single native executable in bin/")
                        .clicked()
                    {
                        self.do_build_binary(); ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(tr.menu_open_cobol).clicked()  { self.do_open();             ui.close_menu(); }
                    if ui.button(tr.menu_open_form).clicked()   { self.do_open_form();         ui.close_menu(); }
                    if ui.button(tr.menu_import_form).clicked() { self.do_add_file_to_project(FileKind::Form); ui.close_menu(); }
                    ui.separator();
                    if ui.button(tr.menu_save).clicked() { self.do_save(); ui.close_menu(); }
                    ui.separator();
                    if ui.button(tr.menu_quit).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.add_enabled_ui(menu_has_active, |ui| {
                    ui.menu_button(tr.menu_run, |ui| {
                        if ui.add_enabled(!self.runner.is_running(),
                                         egui::Button::new(tr.menu_run_btn)).clicked() {
                            self.do_run(); ui.close_menu();
                        }
                        if ui.add_enabled(self.runner.is_running(),
                                         egui::Button::new(tr.menu_stop)).clicked() {
                            self.do_stop(); ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(tr.menu_check_only).clicked() { self.do_check(); ui.close_menu(); }
                    });

                    ui.menu_button(tr.menu_view, |ui| {
                        ui.checkbox(&mut self.editor.show_line_numbers, tr.menu_line_numbers);
                    });
                });

                // ── Help / Bug report ────────────────────────────────────────
                ui.menu_button("Help", |ui| {
                    if ui.button(tr.doc_menu_label).clicked() {
                        self.doc_viewer.open(self.lang);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("🐛 Report a Problem…")
                        .on_hover_text("Report a bug or issue — saved to BUGS.md and picked up by the next scan")
                        .clicked()
                    {
                        self.report_bug.open_for("IDE Editor");
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("ℹ About PowerRustCOBOL").clicked() {
                        self.about_open = true;
                        ui.close_menu();
                    }
                });
            });
        });

        // ── Toolbar ───────────────────────────────────────────────────────────
        // A project compiles only if it has a COBOL program or a form; with no
        // project (single-file mode) gate on an open source / designer.
        let compilable = match &self.cobolt_project {
            Some(p) => p.is_compilable(),
            None => self.editor.active_source().is_some() || !self.designers.is_empty(),
        };
        // Debug is enabled only when a Generated Code element is selected in the tree.
        let debuggable = match (&self.cobolt_project, self.project.selected_file()) {
            (Some(p), Some(rel)) => p.is_generated(rel),
            _ => false,
        };
        // "Active" = a project is open or a file is being edited. Gates Save /
        // Check (toolbar) and the Run / View menus.
        let has_active = self.cobolt_project.is_some() || self.editor.active_source().is_some();
        match toolbar::show(
            ctx,
            &self.runner,
            &tr,
            &mut self.lang,
            compilable,
            debuggable,
            has_active,
        ) {
            ToolbarAction::Run => self.do_run(),
            ToolbarAction::Stop => self.do_stop(),
            ToolbarAction::Debug => self.do_debug(),
            ToolbarAction::Build => self.do_build_binary(),
            ToolbarAction::Check => self.do_check(),
            // The toolbar Open button always opens (or switches to) a project, so
            // you can change projects at any time. Opening an individual COBOL
            // file lives in File → Open COBOL (and the project tree).
            ToolbarAction::Open => self.do_open_project(),
            ToolbarAction::Save => self.do_save(),
            ToolbarAction::None => {}
        }

        // ── Active debug-session controls (shown only while debugging) ────────
        // Debugging is started from the main toolbar's Debug button (right of
        // Run); this secondary row only appears during a session to Stop it.
        if self.debug_active {
            egui::TopBottomPanel::top("debug_toolbar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("■ Stop Debug").clicked() {
                        self.debug_runner.stop();
                        self.debug_active = false;
                        self.debugger.reset();
                        self.editor.debug_line = None;
                    }
                    // F5 / F10 keyboard shortcuts.
                    if ctx.input(|i| i.key_pressed(egui::Key::F5)) {
                        self.debug_runner.send_cmd(DebugCmd::Continue);
                    }
                    if ctx.input(|i| i.key_pressed(egui::Key::F10)) {
                        self.debug_runner.send_cmd(DebugCmd::StepOver);
                    }
                });
            });
        }

        // ── Debugger side panel ───────────────────────────────────────────────
        if self.debug_active {
            if let Some(cmd) = self.debugger.show(ctx, &tr, self.debug_runner.is_running()) {
                self.debug_runner.send_cmd(cmd);
            }
        }

        // ── Code workspace panels ─────────────────────────────────────────────
        // Until a project is open we show a single full welcome pane (the
        // localized developer's guide) with no left tree, no editor, no output,
        // no editor controls. The top menu/toolbar remain so the user can
        // New/Open a project.
        let has_project = self.cobolt_project.is_some();
        if has_project {
            self.output.show(ctx, &tr);

            let proj_events = self.project.show(ctx, self.cobolt_project.as_ref(), &tr);
            for ev in proj_events {
                match ev {
                    ProjectPanelEvent::Open(path) => {
                        self.show_project_settings = false;
                        self.inspect = None;
                        self.indexed_inspect = None;
                        self.open_in_editor(path);
                    }
                    ProjectPanelEvent::OpenDesigner(path) => {
                        self.show_project_settings = false;
                        self.load_form_from_path(path);
                    }
                    ProjectPanelEvent::OpenIndexedEditor(path) => {
                        self.show_project_settings = false;
                        self.open_indexed_inspect(path, None);
                    }
                    ProjectPanelEvent::InspectIndexedFile(path) => {
                        self.show_project_settings = false;
                        self.inspect = None;
                        self.open_indexed_inspect(path, None);
                    }
                    ProjectPanelEvent::InspectIndexedField { cidx, field_id } => {
                        self.show_project_settings = false;
                        self.inspect = None;
                        self.open_indexed_inspect(cidx, Some(field_id));
                    }
                    ProjectPanelEvent::InspectForm(path) => {
                        self.show_project_settings = false;
                        self.indexed_inspect = None;
                        self.open_inspect(path, None);
                    }
                    ProjectPanelEvent::InspectControl { form, ctrl_id } => {
                        self.show_project_settings = false;
                        self.open_inspect(form, Some(ctrl_id));
                    }
                    ProjectPanelEvent::OpenEventCode { form, paragraph } => {
                        self.show_project_settings = false;
                        self.inspect = None;
                        // Open the form's read-only generated COBOL at the event's
                        // paragraph. The generated file lives in `generated/`, not
                        // next to the `.cfrm` in `forms/` — using the form path with
                        // a swapped extension opened a non-existent file (empty
                        // editor). Generate it first if it isn't on disk yet.
                        let cbl = self.generated_cbl_path(&form);
                        if !cbl.exists() {
                            if let Some(i) = self.designers.iter().position(|(p, _)| *p == form) {
                                self.do_generate_cobol(i);
                            } else if let Ok(f) = load_form(&form) {
                                if let Some(parent) = cbl.parent() {
                                    let _ = std::fs::create_dir_all(parent);
                                }
                                let _ = std::fs::write(&cbl, generate(&f));
                            }
                        }
                        self.pending_open_in_editor = Some(cbl);
                        self.pending_goto_paragraph = Some(paragraph);
                    }
                    ProjectPanelEvent::Select(_) => {} // applied inside the panel
                    ProjectPanelEvent::Create(kind) => self.do_create_in_category(kind),
                    ProjectPanelEvent::Add(kind) => self.do_add_file_to_project(kind),
                    ProjectPanelEvent::Remove(rel) => self.do_remove_file_from_project(rel),
                    ProjectPanelEvent::ShowProjectSettings => {
                        self.show_project_settings = true;
                        self.inspect = None;
                        self.indexed_inspect = None;
                        // Any pending editor open should yield to the settings form.
                        self.pending_open_in_editor = None;
                    }
                }
            }
        }

        // Main Pane priority: when no project show the localized welcome
        // (developer's guide); otherwise the previous logic (settings / inspector / editor).
        if !has_project {
            self.show_welcome_pane(ctx, &tr);
        } else if self.show_project_settings && self.settings_form.is_some() {
            self.show_settings_pane(ctx, &tr);
        } else if self.indexed_inspect.is_some() {
            self.show_indexed_inspector(ctx, &tr);
        } else if self.inspect.is_some() {
            self.show_inspector(ctx, &tr);
        } else {
            let root = self
                .project_path
                .as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf());
            self.editor.show(ctx, Some(&self.llm), &tr, root.as_deref());
        }

        // ── Unsaved project settings close-confirmation dialog (main window) ────
        if self.settings_close_confirm {
            let mut open = true;
            egui::Window::new(tr.settings_close_title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(tr.settings_close_msg);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(tr.close_save).clicked() {
                            self.save_settings_form();
                            self.settings_close_confirm = false;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button(tr.close_discard).clicked() {
                            if let Some(f) = &mut self.settings_form {
                                f.cancel();
                            }
                            self.settings_close_confirm = false;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button(tr.close_cancel).clicked() {
                            self.settings_close_confirm = false;
                        }
                    });
                });
            if !open {
                self.settings_close_confirm = false;
            }
        }

        // Tree semaphore: the active file, if edited since its last check, goes
        // back to yellow ("changed — not tested").
        let active = self
            .editor
            .active_source()
            .map(|(p, c)| (p.clone(), Self::content_hash(c)));
        if let Some((path, h)) = active {
            let changed = self.checked.get(&path).map(|c| *c != h).unwrap_or(true);
            if changed {
                self.set_element_status(&path, ElementStatus::Changed);
            }
        }

        // ── Designer viewports (one OS window per open form) ──────────────────
        let n = self.designers.len();
        for idx in 0..n {
            // Compute stable viewport ID and title before entering the closure.
            let vp_id = ViewportId::from_hash_of(&self.designers[idx].0);
            let title = {
                let (path, d) = &self.designers[idx];
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("form");
                let dirty = if d.dirty { " ●" } else { "" };
                format!("PowerRustCOBOL Form Designer  v{VERSION} — {stem}{dirty}")
            };

            ctx.show_viewport_immediate(
                vp_id,
                ViewportBuilder::default()
                    .with_title(&title)
                    .with_inner_size([1200.0, 800.0]),
                |vp_ctx, _class| {
                    if vp_ctx.input(|i| i.viewport().close_requested()) {
                        let d = &mut self.designers[idx].1;
                        if d.dirty {
                            // Cancel the OS close and show our Save/Discard/Cancel dialog.
                            vp_ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                            d.close_confirm = true;
                        } else {
                            d.close_requested = true;
                        }
                    }
                    self.show_designer_window(vp_ctx, idx, &tr);
                },
            );
        }

        // If the user closed a designer window this frame, force its live
        // preview flag off. The preview state (and show_preview) lives in the
        // DesignerState entry; leaving the flag on would cause the subsequent
        // preview viewport loop to access a soon-to-be-reaped idx or leave a
        // dangling "Preview — xxx" window with no backing designer.
        for (_, d) in &mut self.designers {
            if d.close_requested {
                d.show_preview = false;
            }
        }

        // ── Documentation viewer window (Help → Documentation) ───────────────────
        self.doc_viewer.show(ctx, self.lang, &tr);

        // ── Preview viewports (one per open form that has preview enabled) ───────
        for idx in 0..self.designers.len() {
            if !self.designers[idx].1.show_preview {
                continue;
            }

            let vp_id = ViewportId::from_hash_of(("preview", &self.designers[idx].0));
            let title = {
                let (path, _) = &self.designers[idx];
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("form");
                format!("Preview — {stem}")
            };
            let (form_w, form_h) = {
                let d = &self.designers[idx].1;
                (d.form.width as f32, d.form.height as f32)
            };

            ctx.show_viewport_immediate(
                vp_id,
                ViewportBuilder::default()
                    .with_title(&title)
                    .with_inner_size([form_w + 4.0, form_h + 4.0])
                    .with_resizable(true)
                    .with_transparent(true),
                |vp_ctx, _class| {
                    if vp_ctx.input(|i| i.viewport().close_requested()) {
                        self.designers[idx].1.show_preview = false;
                    }
                    self.show_preview_window(vp_ctx, idx);
                },
            );
        }

        // ── Running form viewports (Phase 6) ─────────────────────────────────────
        // Drain display output and state updates from every running runtime each frame.
        let mut display_lines: Vec<String> = Vec::new();
        for rt in &mut self.form_runtimes {
            display_lines.extend(rt.drain_display());
            rt.drain_state();
        }
        for line in display_lines {
            self.output.push_line(line);
        }

        // Collect indices of runtimes that are still alive.
        let running_indices: Vec<usize> = (0..self.form_runtimes.len())
            .filter(|&i| self.form_runtimes[i].is_running())
            .collect();

        for i in running_indices {
            let vp_id = ViewportId::from_hash_of(("run_form", &self.form_runtimes[i].form_path));
            let title = format!("▶ {}", self.form_runtimes[i].form_title);
            let fw = self.form_runtimes[i].form_width as f32;
            let fh = self.form_runtimes[i].form_height as f32;

            ctx.show_viewport_immediate(
                vp_id,
                ViewportBuilder::default()
                    .with_title(&title)
                    .with_inner_size([fw + 4.0, fh + 4.0])
                    .with_resizable(true)
                    .with_transparent(true),
                |vp_ctx, _class| {
                    if vp_ctx.input(|inp| inp.viewport().close_requested()) {
                        // User closed the window → send quit sentinel to interpreter.
                        self.form_runtimes[i].send_event(cobolt_runtime::FormEvent::quit());
                    }
                    self.show_running_form_window(vp_ctx, i);
                },
            );
        }

        // Reap finished runtimes.
        self.form_runtimes.retain(|rt| rt.is_running());

        // Remove any designer windows the user has closed.
        self.designers.retain(|(_, d)| !d.close_requested);
        self.indexed_grids.retain(|(_, g)| !g.close_requested);

        if self.runner.is_running() || !self.form_runtimes.is_empty() {
            ctx.request_repaint();
        }
    }
}

// ── Preview window contents ───────────────────────────────────────────────────

/// The property key that holds a control's live preview value, by type. The
/// preview keeps one value per control; this maps it to the key the engine reads.
fn preview_value_key(ct: &cobolt_forms::ControlType) -> &'static str {
    use cobolt_forms::ControlType as CT;
    match ct {
        CT::TextBox => "Text",
        CT::ComboBox
        | CT::ListBox
        | CT::Slider
        | CT::ProgressBar
        | CT::NumericUpDown
        | CT::CheckBox
        | CT::RadioButton => "Value",
        _ => "Caption",
    }
}

/// `FormState` for the live preview: injects the single per-control preview value
/// into the right property key and supplies the OnFormLoad animation transform.
/// The engine does everything else (spec 017 T4).
struct PreviewState<'a> {
    values: &'a std::collections::HashMap<String, String>,
    anim: &'a std::collections::HashMap<String, f32>,
    form_w: f32,
    form_h: f32,
}
impl cobolt_forms::render::FormState for PreviewState<'_> {
    fn live(&self, base: &cobolt_forms::Control) -> cobolt_forms::Control {
        let mut c = base.clone();
        if let Some(v) = self.values.get(&base.id) {
            c.set_prop(
                preview_value_key(&base.control_type).to_owned(),
                cobolt_forms::PropValue::String(v.clone()),
            );
        }
        c
    }
    fn visible(&self, base: &cobolt_forms::Control) -> bool {
        base.visible
    }
    fn enabled(&self, base: &cobolt_forms::Control) -> bool {
        base.enabled
    }
    fn transform(&self, base: &cobolt_forms::Control) -> cobolt_forms::render::RenderTransform {
        let (dx, dy, scale, anim_alpha) = base
            .animations
            .iter()
            .find_map(|a| {
                let key = format!("{}:{}", base.id, a.name);
                self.anim.get(&key).map(|&t| {
                    crate::panels::designer::anim_transform(a, self.form_w, self.form_h, t)
                })
            })
            .unwrap_or((0.0, 0.0, 1.0, 1.0));
        // The control's own Transparency fades it (its Opacity is applied inside
        // draw_control); container opacity is folded in by the engine separately.
        let transparency = base
            .get_prop("Transparency")
            .map(|v| v.as_i64())
            .unwrap_or(0)
            .clamp(0, 100);
        cobolt_forms::render::RenderTransform {
            dx,
            dy,
            scale,
            alpha: anim_alpha * (1.0 - transparency as f32 / 100.0),
        }
    }
}

/// `FormState` for the running (interpreted) form: merges each control's live
/// `CtrlState` (values + SET-PROPERTY geometry) onto the designed control so the
/// unified engine renders the run window exactly like the designer (spec 017 T5).
struct RunState<'a> {
    state: &'a std::collections::HashMap<String, crate::form_runtime::CtrlState>,
}
impl cobolt_forms::render::FormState for RunState<'_> {
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

impl CoboltApp {
    fn show_preview_window(&mut self, ctx: &Context, idx: usize) {
        use crate::panels::designer::AnimState;
        use egui::Color32;

        if idx >= self.designers.len() {
            return;
        }

        // Match the designer's glass toggle so the preview looks identical to the
        // canvas (WYSIWYG, spec 003).
        let glass = self.designers[idx].1.glass_mode;

        // Apply the designer's active theme pack to this preview viewport's context
        // (a separate egui Context) so themed controls and charts match the canvas.
        cobolt_forms::paint::set_active_theme(ctx, self.designers[idx].1.active_theme_pack.clone());
        cobolt_forms::paint::set_glass_style(ctx, self.designers[idx].1.form.glass_style);

        // ── Animation tick ────────────────────────────────────────────────────
        {
            let d = &mut self.designers[idx].1;
            let now = std::time::Instant::now();
            let dt = d
                .preview_last_frame
                .map(|t| now.duration_since(t).as_secs_f32())
                .unwrap_or(0.0);
            d.preview_last_frame = Some(now);

            // Auto-start OnFormLoad animations once on first open.
            // A sentinel key "__init__" marks that initialisation has run,
            // even if no OnFormLoad animations exist (avoids re-running every frame).
            let needs_init = !d.preview_anim_states.contains_key("__init__");
            if needs_init {
                d.preview_anim_states
                    .insert("__init__".to_owned(), AnimState::new("__init__"));
                for ctrl in &d.form.controls {
                    for anim in &ctrl.animations {
                        if matches!(anim.trigger, cobolt_forms::model::AnimTrigger::OnFormLoad) {
                            let key = format!("{}:{}", ctrl.id, anim.name);
                            let delay_secs = anim.delay_ms as f32 / 1000.0;
                            let mut state = AnimState::new(&anim.name);
                            state.play(delay_secs);
                            d.preview_anim_states.insert(key, state);
                        }
                    }
                }
            }

            // Advance all playing animations
            if dt > 0.0 {
                let anim_meta: std::collections::HashMap<String, u64> = d
                    .form
                    .controls
                    .iter()
                    .flat_map(|c| {
                        c.animations
                            .iter()
                            .map(move |a| (format!("{}:{}", c.id, a.name), a.duration_ms))
                    })
                    .collect();
                let mut need_repaint = false;
                for (key, state) in d.preview_anim_states.iter_mut() {
                    if !state.playing {
                        continue;
                    }
                    if state.delay_remaining > 0.0 {
                        state.delay_remaining -= dt;
                        if state.delay_remaining < 0.0 {
                            state.delay_remaining = 0.0;
                        }
                        need_repaint = true;
                        continue;
                    }
                    let dur = anim_meta.get(key).copied().unwrap_or(400) as f32 / 1000.0;
                    if dur <= 0.0 {
                        state.stop();
                        continue;
                    }
                    state.t += dt / dur;
                    if state.t >= 1.0 {
                        state.t = 1.0;
                        state.playing = false;
                    }
                    need_repaint = true;
                }
                if need_repaint {
                    ctx.request_repaint();
                }
            }
        }

        // ── Apply glass visuals to this preview viewport ──────────────────────
        // NOTE: egui 0.29 shares visuals globally across all viewports.
        // We override here for the preview, and show_designer_window re-applies
        // the IDE glass visuals on every frame to counteract this.
        {
            // Start from the current IDE glass visuals so we inherit the base
            // colour scheme, then layer in the preview-specific transparency.
            let mut visuals = ctx.style().visuals.clone();
            // Control backgrounds — translucent frosted glass
            let glass_fill = Color32::from_rgba_premultiplied(50, 55, 90, 55);
            let glass_stroke =
                egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(180, 180, 230, 80));
            visuals.widgets.noninteractive.bg_fill = glass_fill;
            visuals.widgets.noninteractive.bg_stroke = glass_stroke;
            visuals.widgets.inactive.bg_fill = glass_fill;
            visuals.widgets.inactive.bg_stroke = glass_stroke;
            visuals.widgets.hovered.bg_fill = Color32::from_rgba_premultiplied(70, 80, 130, 80);
            visuals.widgets.hovered.bg_stroke =
                egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(200, 210, 255, 120));
            visuals.widgets.active.bg_fill = Color32::from_rgba_premultiplied(90, 100, 160, 100);
            visuals.widgets.active.bg_stroke =
                egui::Stroke::new(1.5, Color32::from_rgba_premultiplied(220, 230, 255, 160));
            // Rounding
            let rnd = egui::Rounding::same(8.0);
            visuals.widgets.noninteractive.rounding = rnd;
            visuals.widgets.inactive.rounding = rnd;
            visuals.widgets.hovered.rounding = rnd;
            visuals.widgets.active.rounding = rnd;
            // Text
            visuals.override_text_color = Some(Color32::from_rgb(230, 235, 255));
            // Window / panel background — transparent so the OS shows through
            visuals.panel_fill = Color32::TRANSPARENT;
            visuals.window_fill = Color32::TRANSPARENT;
            visuals.extreme_bg_color = Color32::from_rgba_premultiplied(20, 20, 40, 180);
            ctx.set_visuals(visuals);
        }

        // Read-only snapshot of what the engine's transform hook needs (the form
        // background is now owned by the engine's Backdrop, below).
        let preview_anim_snap: std::collections::HashMap<String, f32>;
        let form_w: f32;
        let form_h: f32;
        {
            let d = &self.designers[idx].1;
            form_w = d.form.width as f32;
            form_h = d.form.height as f32;
            preview_anim_snap = d
                .preview_anim_states
                .iter()
                .map(|(k, s)| (k.clone(), s.t))
                .collect();
        }

        // Eagerly load the background image into the designer's texture cache.
        {
            let bg_path = self.designers[idx].1.form.background_image.clone();
            if !bg_path.is_empty() {
                self.designers[idx].1.load_image(&bg_path, ctx);
            }
        }

        // ── Render the whole form through the unified engine (spec 017 T4). ────
        // `PreviewState` supplies live values + the animation transform; the
        // engine owns the backdrop, render order, clipping, faces, and the
        // interactive widgets. The old preview control loop + per-type branches
        // are gone.
        let glass_v = glass;
        let controls = self.designers[idx].1.form.controls.clone();
        let values_snap = self.designers[idx].1.preview_state.clone();
        let backdrop = {
            let d = &self.designers[idx].1;
            let bg_path = d.form.background_image.clone();
            let image = if bg_path.is_empty() {
                None
            } else {
                d.image_cache
                    .get(&bg_path)
                    .and_then(|o| o.as_ref())
                    .map(|t| (t.id(), t.size_vec2()))
            };
            cobolt_forms::render::Backdrop {
                color_hex: d.form.background_color.clone(),
                transparency: d.form.transparency.min(100) as u8,
                image,
                image_mode: d.form.bg_image_mode,
            }
        };
        let active_tabs = cobolt_forms::containers::ActiveTabs::default();
        let st = PreviewState {
            values: &values_snap,
            anim: &preview_anim_snap,
            form_w,
            form_h,
        };

        let mut updates: Vec<(String, String, String)> = Vec::new();
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_size(egui::vec2(form_w, form_h));
                        let input = cobolt_forms::render::RenderInput {
                            controls: &controls,
                            state: &st,
                            form_size: egui::vec2(form_w, form_h),
                            glass: glass_v,
                            mode: cobolt_forms::render::RenderMode::Interactive,
                            active_tabs: &active_tabs,
                            backdrop,
                        };
                        let out = cobolt_forms::render::render_form(ui, &input);
                        updates = out.prop_updates;
                        // Preview has no COBOL event loop; UI events are discarded.
                    });
            });

        // Apply the engine's value updates back to the preview value map so the
        // next frame renders the edited state (text typed, slider moved, combo
        // selected, checkbox toggled).
        for (id, _key, val) in updates {
            self.designers[idx].1.preview_state.insert(id, val);
        }

        // Ensure the separate live preview viewport keeps receiving frames for
        // its animation ticker and interactive simulation even if the main
        // IDE window is idle (e.g. viewing the indexed inspector).
        ctx.request_repaint();
    }
}

// ── Running form window (Phase 6) ────────────────────────────────────────────

impl CoboltApp {
    /// Render the live interactive form window for `form_runtimes[idx]`.
    ///
    /// Each egui frame:
    ///  1. Control states were already updated by `drain_state()` in the main loop.
    ///  2. We render each control from `FormRuntime::ctrl_state`.
    ///  3. User interactions (clicks, text changes) fire `send_event()`.
    fn show_running_form_window(&mut self, ctx: &Context, idx: usize) {
        use cobolt_forms::ControlType as CT;
        use cobolt_runtime::FormEvent;
        use egui::Color32;

        if idx >= self.form_runtimes.len() {
            return;
        }

        // Match the designer's glass toggle live so the running form tracks the
        // canvas (WYSIWYG, spec 003). Resolve the owning designer by path first,
        // then by form name (robust to path-normalisation differences), and keep
        // the runtime's snapshot in sync so a closed designer still renders right.
        let glass = {
            let fp = self.form_runtimes[idx].form_path.clone();
            let fname = self.form_runtimes[idx].form_name.clone();
            let found = self
                .designers
                .iter()
                .find(|(p, _)| *p == fp)
                .or_else(|| self.designers.iter().find(|(_, d)| d.form.name == fname))
                .map(|(_, d)| d.glass_mode);
            if let Some(g) = found {
                self.form_runtimes[idx].glass = g;
            }
            found.unwrap_or(self.form_runtimes[idx].glass)
        };

        // Apply the owning designer's active theme pack to THIS viewport's context
        // so charts and themed controls render identically to the canvas. Each
        // run/preview window is a separate egui Context, so the theme set on the
        // designer's context does not carry over (spec 017 parity).
        {
            let fp = self.form_runtimes[idx].form_path.clone();
            let fname = self.form_runtimes[idx].form_name.clone();
            let pack = self
                .designers
                .iter()
                .find(|(p, _)| *p == fp)
                .or_else(|| self.designers.iter().find(|(_, d)| d.form.name == fname))
                .and_then(|(_, d)| d.active_theme_pack.clone());
            let glass_style = self
                .designers
                .iter()
                .find(|(p, _)| *p == fp)
                .or_else(|| self.designers.iter().find(|(_, d)| d.form.name == fname))
                .map(|(_, d)| d.form.glass_style)
                .unwrap_or_default();
            cobolt_forms::paint::set_active_theme(ctx, pack);
            cobolt_forms::paint::set_glass_style(ctx, glass_style);
        }

        // ── Form-level lifecycle events ───────────────────────────────────────
        // onShow / onActivate fire once when the running form first appears;
        // onResize fires whenever its canvas size changes. All addressed to the
        // form's own id so the generated loop dispatches them.
        {
            let rt = &self.form_runtimes[idx];
            let fname = rt.form_name.clone();
            let cur_size = (rt.form_width, rt.form_height);
            let shown_id = egui::Id::new(("form-shown", idx));
            let already = ctx.memory(|m| m.data.get_temp::<bool>(shown_id).unwrap_or(false));
            if !already {
                self.form_runtimes[idx].send_event(FormEvent::new(&fname, "onShow"));
                self.form_runtimes[idx].send_event(FormEvent::new(&fname, "onActivate"));
                ctx.memory_mut(|m| m.data.insert_temp(shown_id, true));
                ctx.memory_mut(|m| {
                    m.data
                        .insert_temp(egui::Id::new(("form-size", idx)), cur_size)
                });
            } else {
                let size_id = egui::Id::new(("form-size", idx));
                let prev = ctx.memory(|m| m.data.get_temp::<(u32, u32)>(size_id));
                if prev.is_some() && prev != Some(cur_size) {
                    self.form_runtimes[idx].send_event(FormEvent::new(&fname, "onResize"));
                }
                ctx.memory_mut(|m| m.data.insert_temp(size_id, cur_size));
            }
        }

        // Apply glass visuals identical to the preview window.
        {
            let mut vis = ctx.style().visuals.clone();
            let gf = Color32::from_rgba_premultiplied(50, 55, 90, 55);
            let gs = egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(180, 180, 230, 80));
            vis.widgets.noninteractive.bg_fill = gf;
            vis.widgets.noninteractive.bg_stroke = gs;
            vis.widgets.inactive.bg_fill = gf;
            vis.widgets.inactive.bg_stroke = gs;
            vis.widgets.hovered.bg_fill = Color32::from_rgba_premultiplied(70, 80, 130, 80);
            vis.widgets.hovered.bg_stroke =
                egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(200, 210, 255, 120));
            vis.widgets.active.bg_fill = Color32::from_rgba_premultiplied(90, 100, 160, 100);
            vis.widgets.active.bg_stroke =
                egui::Stroke::new(1.5, Color32::from_rgba_premultiplied(220, 230, 255, 160));
            let rnd = egui::Rounding::same(8.0);
            vis.widgets.noninteractive.rounding = rnd;
            vis.widgets.inactive.rounding = rnd;
            vis.widgets.hovered.rounding = rnd;
            vis.widgets.active.rounding = rnd;
            vis.override_text_color = Some(Color32::from_rgb(230, 235, 255));
            vis.panel_fill = Color32::TRANSPARENT;
            vis.window_fill = Color32::TRANSPARENT;
            vis.extreme_bg_color = Color32::from_rgba_premultiplied(20, 20, 40, 180);
            ctx.set_visuals(vis);
        }

        // Snapshot what we need (avoids borrow-split issues with self).
        let bg_image = self.form_runtimes[idx].background_image.clone();
        let bg_mode = self.form_runtimes[idx].bg_image_mode;
        let bg_transp = self.form_runtimes[idx].transparency;
        let form_w = self.form_runtimes[idx].form_width as f32;
        let form_h = self.form_runtimes[idx].form_height as f32;

        // Derive the form background colour from the stored form metadata. This
        // mirrors the preview window exactly (strip '#', take the first 6 hex
        // digits, and treat pure black / unset as the default dark navy) so the
        // running form sits on the same backdrop as the designer and preview —
        // otherwise translucent (glass) content like charts looks washed out over
        // a pure-black window.
        let bg_color = {
            let rt = &self.form_runtimes[idx];
            let raw = rt.background_color.trim();
            let s = if let Some(stripped) = raw.strip_prefix('#') {
                stripped
            } else {
                raw
            };
            let hex = if s.len() >= 6 { &s[..6] } else { s };
            let bg_alpha = (255.0 * (1.0 - rt.transparency as f32 / 100.0)) as u8;
            if hex.len() == 6 {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(20);
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(22);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(45);
                // Pure black (000000) ⇒ default dark navy, matching the preview.
                let (r, g, b) = if r == 0 && g == 0 && b == 0 {
                    (20, 22, 45)
                } else {
                    (r, g, b)
                };
                Color32::from_rgba_premultiplied(
                    (r as f32 * bg_alpha as f32 / 255.0) as u8,
                    (g as f32 * bg_alpha as f32 / 255.0) as u8,
                    (b as f32 * bg_alpha as f32 / 255.0) as u8,
                    bg_alpha,
                )
            } else {
                Color32::from_rgba_premultiplied(20, 22, 45, bg_alpha.max(200))
            }
        };

        // ── Render the whole form through the unified engine (spec 017 T5). ────
        // The old per-control run loop + `render_run_control` are gone; the engine
        // owns the backdrop, render order, container clipping, faces, and the
        // interactive widgets. `RunState` supplies live values from `CtrlState`.
        let controls = self.form_runtimes[idx].controls.clone();
        let states_snap = self.form_runtimes[idx].ctrl_state.clone();
        let bg_hex = self.form_runtimes[idx].background_color.clone();
        // Live tab selection per TabControl (so a runtime SET-PROPERTY SelectedTab
        // or a tab click hides/shows the right page).
        let active_tabs: cobolt_forms::containers::ActiveTabs = controls
            .iter()
            .filter(|c| matches!(c.control_type, CT::TabControl))
            .filter_map(|c| {
                states_snap
                    .get(&c.id)
                    .and_then(|s| s.props.get("SelectedTab"))
                    .and_then(|v| v.trim().parse::<u32>().ok())
                    .map(|t| (c.id.clone(), t))
            })
            .collect();
        let st = RunState {
            state: &states_snap,
        };

        let mut output = cobolt_forms::render::RenderOutput::default();
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(bg_color))
            .show(ctx, |ui| {
                // Scrollbars appear automatically when the form is larger than the
                // window viewport; the content area is at least the form's size.
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_size(egui::vec2(form_w, form_h));
                        // Background image texture (cached in egui memory).
                        let image = if bg_image.trim().is_empty() {
                            None
                        } else {
                            let cid = egui::Id::new(("runform_bg", bg_image.as_str()));
                            let tex =
                                match ui.data(|d| d.get_temp::<Option<egui::TextureHandle>>(cid)) {
                                    Some(t) => t,
                                    None => {
                                        let l = load_image_texture(ui.ctx(), &bg_image);
                                        ui.data_mut(|d| d.insert_temp(cid, l.clone()));
                                        l
                                    }
                                };
                            tex.map(|t| (t.id(), t.size_vec2()))
                        };
                        let input = cobolt_forms::render::RenderInput {
                            controls: &controls,
                            state: &st,
                            form_size: egui::vec2(form_w, form_h),
                            glass,
                            mode: cobolt_forms::render::RenderMode::Interactive,
                            active_tabs: &active_tabs,
                            backdrop: cobolt_forms::render::Backdrop {
                                color_hex: bg_hex.clone(),
                                transparency: bg_transp,
                                image,
                                image_mode: bg_mode,
                            },
                        };
                        output = cobolt_forms::render::render_form(ui, &input);
                    });
            });

        // Apply value updates back to CtrlState, sync them to the interpreter (so
        // an event handler reads the live value), then map UI events -> FormEvent
        // and dispatch. Order matters: inputs before events.
        {
            let rt = &mut self.form_runtimes[idx];
            for (id, key, val) in &output.prop_updates {
                rt.ctrl_state
                    .entry(id.clone())
                    .or_default()
                    .props
                    .insert(key.clone(), val.clone());
                rt.send_input(id, key, val);
            }
            for ev in output.events {
                rt.send_event(FormEvent::new(ev.ctrl_id, ev.event));
            }
        }

        // Keep the live interpreter window (and the root-side drain of its
        // channels) ticking at a good rate even when the primary window is
        // showing a "static" inspector (e.g. the new indexed file properties)
        // or has no other animation. The root only requests when it sees
        // runtimes; a self-request here makes the RAD "Run Form (live
        // interpreter)" reliably smooth and responsive.
        ctx.request_repaint();
    }
}

// ── Indexed grid window contents (structure editing is now only in the inline inspector) ───────────────────────────────────

impl CoboltApp {
    /// Show any open indexed *grid browser* viewports (for the live data of finalized .cidx files).
    /// The dedicated pop-up "Indexed File Editor" (structure tree + properties in separate OS window)
    /// has been removed; all structure editing now happens in the inline inspector ("regular treeview properties").
    fn show_indexed_grid_viewports(&mut self, ctx: &Context, tr: &Tr) {
        for gi in 0..self.indexed_grids.len() {
            let vp_id = ViewportId::from_hash_of(("indexed_grid", &self.indexed_grids[gi].0));
            let title = {
                let (path, _) = &self.indexed_grids[gi];
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("grid");
                format!("{} — {stem}", tr.grid_browser_title)
            };
            ctx.show_viewport_immediate(
                vp_id,
                ViewportBuilder::default()
                    .with_title(&title)
                    .with_inner_size([1000.0, 600.0]),
                |vp_ctx, _class| {
                    if vp_ctx.input(|i| i.viewport().close_requested()) {
                        self.indexed_grids[gi].1.close_requested = true;
                    }
                    self.show_indexed_grid_window(vp_ctx, gi, tr);
                },
            );
        }

        if !self.indexed_grids.is_empty() || self.settings_form.is_some() {
            ctx.request_repaint();
        }
    }

    fn show_indexed_grid_window(&mut self, ctx: &Context, gi: usize, tr: &Tr) {
        if gi >= self.indexed_grids.len() {
            return;
        }
        let theme = self.current_theme();
        apply_opaque_viewport_theme(ctx, theme);
        let panel_frame = crate::theme::glass_panel_frame(ctx.style().visuals.panel_fill, theme);
        let mut toolbar_action = GridAction::None;
        let mut status_msg: Option<String> = None;
        egui::CentralPanel::default()
            .frame(panel_frame)
            .show(ctx, |ui| {
                let st = &mut self.indexed_grids[gi].1;
                let (act, msg) = st.panel.show(ui, &st.def, tr);
                toolbar_action = act;
                status_msg = msg;
            });
        if let Some(msg) = status_msg {
            self.output.push_status(msg);
        }
        if toolbar_action != GridAction::None {
            if let Some(msg) = self.indexed_grids[gi]
                .1
                .panel
                .apply_action(toolbar_action, tr)
            {
                self.output.push_status(msg);
            }
        }
        ctx.request_repaint();
    }
}

// ── Designer window contents ──────────────────────────────────────────────────

impl CoboltApp {
    fn show_user_control_delete_confirm(&mut self, ctx: &Context, tr: &Tr) {
        let Some(name) = self.pending_user_control_delete.clone() else {
            return;
        };
        let mut cancel = false;
        let mut confirm = false;
        let message = tr.uc_delete_confirm.replace("{name}", &name);

        egui::Window::new(tr.uc_delete)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(message);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr.btn_cancel).clicked() {
                        cancel = true;
                    }
                    if ui.button(tr.delete_confirm_ok).clicked() {
                        confirm = true;
                    }
                });
            });

        if cancel {
            self.pending_user_control_delete = None;
        }
        if confirm {
            self.pending_user_control_delete = None;
            self.remove_user_control_def(&name);
        }
    }

    fn show_designer_window(&mut self, ctx: &Context, idx: usize, tr: &Tr) {
        if idx >= self.designers.len() {
            return;
        }

        // Re-apply glass visuals to this designer viewport every frame.
        // The preview viewport calls ctx.set_visuals() which is globally shared
        // in egui 0.29, so we must restore them here each frame.
        apply_opaque_viewport_theme(ctx, self.current_theme());

        // ── Unsaved-changes confirmation dialog ───────────────────────────────
        if self.designers[idx].1.close_confirm {
            let stem = self.designers[idx]
                .0
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("form");
            let title = format!("Save changes to '{stem}'?");
            let mut close_confirm = true; // controls the egui::Window open state
            egui::Window::new(&title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .open(&mut close_confirm)
                .show(ctx, |ui| {
                    ui.label(tr.close_msg);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(tr.close_save).clicked() {
                            self.designers[idx].1.close_confirm = false;
                            self.do_save_designer(idx);
                            self.do_generate_cobol(idx);
                            self.designers[idx].1.close_requested = true;
                        }
                        if ui.button(tr.close_discard).clicked() {
                            self.designers[idx].1.close_confirm = false;
                            self.designers[idx].1.close_requested = true;
                        }
                        if ui.button(tr.close_cancel).clicked() {
                            self.designers[idx].1.close_confirm = false;
                        }
                    });
                });
            // If the user dismisses the window via the X button of the dialog itself, treat as Cancel.
            if !close_confirm {
                self.designers[idx].1.close_confirm = false;
            }
            // While the confirm dialog is showing, don't render the rest of the designer.
            return;
        }

        // ── Designer keyboard shortcuts ───────────────────────────────────────
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S)))
        {
            self.do_save_designer(idx);
            self.do_generate_cobol(idx);
        }

        // ── Left sidebar: Forms list + Toolbox (full height — reaches top) ────
        // Rendered BEFORE the toolbar so it occupies the full window height on the
        // left; the toolbar below then fills only the area to its right.
        // Collect open paths as owned so no borrow lingers on self.designers.
        let open_paths: Vec<PathBuf> = self.designers.iter().map(|(p, _)| p.clone()).collect();
        let open_path_refs: Vec<&Path> = open_paths.iter().map(|p| p.as_path()).collect();
        let user_controls: Vec<UserControlDef> = self
            .cobolt_project
            .as_ref()
            .map(|project| project.user_controls.clone())
            .unwrap_or_default();

        let (form_to_open, toolbox_action) = egui::SidePanel::left(format!("dl_{idx}"))
            .resizable(true)
            .default_width(150.0)
            .show(ctx, |ui| {
                let to_open = self.forms_list.show(ui, &open_path_refs, tr);
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
                let tb = self.designers[idx].1.toolbox.show(ui, tr, &user_controls);
                (to_open, tb)
            })
            .inner;

        if let Some(path) = form_to_open {
            self.load_form_from_path(path);
            return; // re-render next frame with the new designer added
        }

        // ── Unified 50-px icon toolbar (replaces both old toolbars) ──────────
        use crate::panels::designer::{draw_icon_toolbar, DesignerToolbarAction};
        // Transparent frame + no separator line; `draw_icon_toolbar` fills the
        // whole reserved height itself with the toolbox colour (see designer.rs).
        egui::TopBottomPanel::top(format!("dtb_{idx}"))
            .exact_height(50.0)
            .frame(egui::Frame::none())
            .show_separator_line(false)
            .show(ctx, |ui| {
                let d = &self.designers[idx].1;
                let can_undo = d.can_undo();
                let can_redo = d.can_redo();
                let has_sel = !d.selected_ids.is_empty();
                let has_multi = d.selected_ids.len() >= 2;
                let has_clipboard = self
                    .clipboard
                    .as_ref()
                    .map(|clip| !clip.controls.is_empty())
                    .unwrap_or(false);
                let preview_on = d.show_preview;
                let grid_on = d.show_grid;
                let glass_on = d.glass_mode;
                let fp_active = matches!(
                    d.format_painter,
                    crate::panels::designer::FormatPainter::WaitingForTarget { .. }
                );
                let form_path = self.designers[idx].0.clone();
                let form_running = self
                    .form_runtimes
                    .iter()
                    .any(|rt| rt.form_path == form_path && rt.is_running());

                // Icons (left) + language selector (right) on a SINGLE centred row.
                // They must share one row: two stacked rows (icon row + a separate
                // selector row) make the content ~75px tall, which egui uses as the
                // panel height — overriding `exact_height(50)`.
                let mut action = DesignerToolbarAction::None;
                ui.horizontal_centered(|ui| {
                    action = draw_icon_toolbar(
                        ui,
                        can_undo,
                        can_redo,
                        has_sel,
                        has_multi,
                        has_clipboard,
                        tr.clipboard_cut,
                        tr.clipboard_copy,
                        tr.clipboard_paste,
                        tr.clipboard_duplicate,
                        preview_on,
                        grid_on,
                        glass_on,
                        form_running,
                        fp_active,
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("designer_lang_selector")
                            .selected_text(self.lang.native_name())
                            .width(130.0)
                            .show_ui(ui, |ui| {
                                for &l in Language::ALL {
                                    ui.selectable_value(&mut self.lang, l, l.native_name());
                                }
                            });
                    });
                });

                // Dispatch actions
                match action {
                    DesignerToolbarAction::Undo => {
                        self.designers[idx].1.undo();
                    }
                    DesignerToolbarAction::Redo => {
                        self.designers[idx].1.redo();
                    }
                    DesignerToolbarAction::SaveAndGenerate => {
                        self.do_save_designer(idx);
                        self.do_generate_cobol(idx);
                    }
                    DesignerToolbarAction::GenerateOnly => {
                        self.do_generate_cobol(idx);
                    }
                    DesignerToolbarAction::TogglePreview => {
                        let d = &mut self.designers[idx].1;
                        d.show_preview = !d.show_preview;
                        if d.show_preview {
                            d.preview_anim_states.clear();
                            d.preview_last_frame = None;
                            d.preview_state.clear();
                            d.preview_combo_open.clear();
                            ctx.memory_mut(|mem| mem.close_popup());
                        }
                    }
                    DesignerToolbarAction::ToggleGrid => {
                        self.designers[idx].1.show_grid = !self.designers[idx].1.show_grid;
                    }
                    DesignerToolbarAction::ToggleGlass => {
                        self.designers[idx].1.glass_mode = !self.designers[idx].1.glass_mode;
                    }
                    DesignerToolbarAction::RunForm => {
                        self.do_run_form(idx);
                    }
                    DesignerToolbarAction::StopForm => {
                        let fp = self.designers[idx].0.clone();
                        self.form_runtimes.retain_mut(|rt| {
                            if rt.form_path == fp {
                                rt.stop();
                                false
                            } else {
                                true
                            }
                        });
                    }
                    DesignerToolbarAction::Cut => {
                        self.designers[idx].1.cut_selected(&mut self.clipboard);
                    }
                    DesignerToolbarAction::Copy => {
                        self.designers[idx].1.copy_selected(&mut self.clipboard);
                    }
                    DesignerToolbarAction::Paste => {
                        self.designers[idx].1.paste_from_clipboard(&self.clipboard);
                    }
                    DesignerToolbarAction::Duplicate => {
                        self.designers[idx]
                            .1
                            .duplicate_selected(&mut self.clipboard);
                    }
                    DesignerToolbarAction::Delete => {
                        self.designers[idx].1.delete_selected();
                    }
                    DesignerToolbarAction::BringToFront => {
                        self.designers[idx].1.bring_to_front();
                    }
                    DesignerToolbarAction::SendToBack => {
                        self.designers[idx].1.send_to_back();
                    }
                    DesignerToolbarAction::BringForward => {
                        self.designers[idx].1.bring_forward();
                    }
                    DesignerToolbarAction::SendBackward => {
                        self.designers[idx].1.send_backward();
                    }
                    DesignerToolbarAction::AlignLeft => {
                        self.designers[idx].1.align_left();
                    }
                    DesignerToolbarAction::AlignRight => {
                        self.designers[idx].1.align_right();
                    }
                    DesignerToolbarAction::AlignTop => {
                        self.designers[idx].1.align_top();
                    }
                    DesignerToolbarAction::AlignBottom => {
                        self.designers[idx].1.align_bottom();
                    }
                    DesignerToolbarAction::CenterH => {
                        self.designers[idx].1.center_horizontal();
                    }
                    DesignerToolbarAction::CenterV => {
                        self.designers[idx].1.center_vertical();
                    }
                    DesignerToolbarAction::SpaceH => {
                        self.designers[idx].1.space_evenly_horizontal();
                    }
                    DesignerToolbarAction::SpaceV => {
                        self.designers[idx].1.space_evenly_vertical();
                    }
                    DesignerToolbarAction::FormatPainter => {
                        self.designers[idx].1.toggle_format_painter();
                    }
                    DesignerToolbarAction::ToggleAnimPreview => {
                        self.designers[idx].1.play_all_form_load_anims();
                    }
                    DesignerToolbarAction::AutoArrange => {
                        self.designers[idx].1.auto_arrange_labels();
                    }
                    DesignerToolbarAction::ReportBug => {
                        self.report_bug.open_for("Form Designer");
                    }
                    DesignerToolbarAction::None => {}
                }
            });

        // ── Properties panel (right) ──────────────────────────────────────────
        let sel_id = self.designers[idx].1.selected_ids.first().cloned();

        // Allow the properties panel to be resized up to half the window width so
        // long values (paths, titles) aren't clipped by the window border.
        let half_win = (ctx.screen_rect().width() * 0.5).max(320.0);
        // 10px right inner margin so the pane's content keeps a small gap from the
        // window border instead of butting against it.
        let props_frame = egui::Frame::side_top_panel(&ctx.style()).inner_margin(egui::Margin {
            left: 6.0,
            right: 10.0,
            top: 6.0,
            bottom: 6.0,
        });
        let inspector_action = egui::SidePanel::right(format!("props_{idx}"))
            .resizable(true)
            .default_width(300.0)
            .min_width(220.0)
            .max_width(half_win)
            .frame(props_frame)
            .show(ctx, |ui| {
                // Split-borrow: form (immutable) and properties (mutable) from DesignerPanel.
                let d = &mut self.designers[idx].1;
                let sel_ctrl = sel_id.as_deref().and_then(|id| d.form.find_control(id));
                // SAFETY: form and properties are different fields — field-level borrow split.
                let form = &d.form as *const cobolt_forms::Form;
                let props = &mut d.properties;
                // SAFETY: we only read *form; no aliased write to form or properties exists.
                props.show(ui, unsafe { &*form }, sel_ctrl, tr)
            })
            .inner;

        // ── Apply inspector actions ───────────────────────────────────────────
        let mut preview_triggered = false;
        for (ctrl_id, key, value) in inspector_action.set_props {
            if key.starts_with("_PreviewAnim") {
                preview_triggered = true;
            }
            self.designers[idx].1.set_property(&ctrl_id, &key, value);
        }
        // Kick off a repaint immediately so the animation loop starts on the next frame.
        if preview_triggered {
            ctx.request_repaint();
        }
        if let Some((ctrl_id, ev_name)) = inspector_action.open_event_editor {
            self.designers[idx].1.open_event_modal(&ctrl_id, &ev_name);
        }
        if let Some(ctrl_id) = inspector_action.open_menu_editor {
            let dir = self.designers[idx].1.cfrm_dir.clone();
            let existing = dir
                .as_ref()
                .map(|d| cobolt_forms::menu::menu_yaml_path(d, &ctrl_id))
                .and_then(|p| cobolt_forms::menu::load_menu(&p).ok())
                .unwrap_or_default();
            self.designers[idx].1.menu_modal = Some(super::panels::designer::MenuEditorModal::new(
                ctrl_id, existing,
            ));
        }
        if let Some((ctrl_id, ev_name)) = inspector_action.open_event_in_code {
            self.jump_to_event_code(idx, &ctrl_id, &ev_name);
        }
        for (key, value) in inspector_action.form_props {
            self.designers[idx].1.set_form_prop(&key, value);
        }
        // COBOL Structure (spec 005): open a block, add or delete a procedure.
        if let Some(t) = inspector_action.cs_open {
            self.designers[idx].1.cobol_structure_edit = Some(t);
        }
        if inspector_action.cs_add_proc {
            let d = &mut self.designers[idx].1;
            let n = d.form.user_procedures.len() + 1;
            d.form
                .user_procedures
                .push(cobolt_forms::model::UserProcedure {
                    name: format!("USER-PROC-{n}"),
                    code: String::new(),
                });
            d.dirty = true;
            let new_idx = d.form.user_procedures.len() - 1;
            d.cobol_structure_edit =
                Some(crate::panels::cobol_structure::CsTarget::Procedure(new_idx));
        }
        if let Some(i) = inspector_action.cs_del_proc {
            let d = &mut self.designers[idx].1;
            if i < d.form.user_procedures.len() {
                d.form.user_procedures.remove(i);
                d.dirty = true;
                // The popup may have been editing a procedure whose index shifted.
                if matches!(
                    d.cobol_structure_edit,
                    Some(crate::panels::cobol_structure::CsTarget::Procedure(_))
                ) {
                    d.cobol_structure_edit = None;
                }
            }
        }

        // ── Apply toolbox drop (add control at canvas centre) ─────────────────
        if let Some(ct) = toolbox_action.dragged_type {
            let cx = (self.designers[idx].1.form.width / 2) as i32;
            let cy = (self.designers[idx].1.form.height / 2) as i32;
            self.designers[idx].1.add_control(ct, cx, cy);
        }
        if let Some(name) = toolbox_action.dragged_user_control {
            if let Some(def) = user_controls
                .iter()
                .find(|def| def.name.eq_ignore_ascii_case(&name))
            {
                let cx = (self.designers[idx].1.form.width / 2) as i32;
                let cy = (self.designers[idx].1.form.height / 2) as i32;
                self.designers[idx]
                    .1
                    .deploy_user_control(def, cx, cy, &user_controls);
            }
        }

        // ── Canvas (centre) ───────────────────────────────────────────────────
        // 007 — resolve the form's theme (per-form override ?? project default ??
        // Liquid Glass) and hand the designer its asset pack for this frame.
        let form_theme = self.designers[idx].1.form.theme.clone();
        let pack = self.resolve_theme_pack(form_theme.as_deref());
        self.designers[idx].1.active_theme_pack = pack;
        let designer_result = egui::CentralPanel::default()
            .show(ctx, |ui| {
                self.designers[idx]
                    .1
                    .show(ui, &mut self.clipboard, &user_controls)
            })
            .inner;
        if let Some(def) = designer_result.user_control_created {
            self.add_user_control_def(def);
        }
        if let Some(name) = designer_result.user_control_delete_requested {
            self.pending_user_control_delete = Some(name);
        }
        self.show_user_control_delete_confirm(ctx, tr);

        // ── COBOL Structure editor window (spec 005) ──────────────────────────
        // Hosts the shared `EditorPanel` (IntelliSense) — same editor everywhere.
        self.designers[idx].1.show_cobol_structure_window(ctx, tr);

        // The "Form saved" alert belongs to THIS viewport (so it appears on top
        // of the designer, not hidden behind it in the main window).
        if self.save_alert_designer == Some(idx) {
            self.show_save_alert(ctx);
        }
    }
}

/// Current day of the month (1–31) in UTC — used to pick the welcome-pane
/// background. UTC is fine for a decorative daily rotation. Civil-from-days
/// (Howard Hinnant's algorithm).
fn day_of_month() -> u32 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    (doy - (153 * mp + 2) / 5 + 1) as u32
}

/// The welcome-pane background for today, cached in egui memory. Loads
/// `assets/images/bg<day>.jpg`, falling back to `bg1.jpg`.
fn welcome_bg_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/images");
    let day = day_of_month();
    let primary = format!("{DIR}/bg{day}.jpg");
    let path = if std::path::Path::new(&primary).exists() {
        primary
    } else {
        format!("{DIR}/bg1.jpg")
    };
    let id = egui::Id::new(("welcome-bg", &path));
    if let Some(t) = ctx.memory(|m| m.data.get_temp::<egui::TextureHandle>(id)) {
        return Some(t);
    }
    let tex = load_image_texture(ctx, &path);
    if let Some(t) = &tex {
        ctx.memory_mut(|m| m.data.insert_temp(id, t.clone()));
    }
    tex
}

/// Turn a project name into a safe file stem for its `<name>.toml` manifest.
/// Spaces are kept (so "Financial Asset Management System.toml" is valid);
/// filesystem-illegal characters become `-`; leading/trailing dots and spaces
/// are trimmed. An empty/blank name falls back to `project`.
fn sanitize_file_stem(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim().to_string();
    if cleaned.is_empty() {
        "project".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod manifest_name_tests {
    use super::*;

    #[test]
    fn project_name_becomes_manifest_stem() {
        assert_eq!(
            sanitize_file_stem("Financial Asset Management System"),
            "Financial Asset Management System"
        );
        // Illegal path chars → '-'; surrounding dots/spaces trimmed.
        assert_eq!(sanitize_file_stem("  My/Project:v2  "), "My-Project-v2");
        assert_eq!(sanitize_file_stem("...."), "project");
        assert_eq!(sanitize_file_stem(""), "project");
    }
}
