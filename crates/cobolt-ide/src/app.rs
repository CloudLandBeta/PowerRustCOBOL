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

use egui::{Color32, Context, Key, KeyboardShortcut, Modifiers, Vec2, ViewportBuilder, ViewportId};

use cobolt_ast::data::DataDecl;
use cobolt_ast::expr::{FigurativeConstant, Literal};
use cobolt_ast::program::DataSection;
use cobolt_forms::{
    load_form, save_form, BindingDataType, BindingField, BindingSourceDescriptor,
    BindingTargetDescriptor, BindingTargetPath, ControlType, DataBindingDef, DataGridAdvanced,
    DataGridColumn, Form, PropValue, DATAGRID_ADVANCED_PROP,
};
// The run/preview per-control draw loops are gone — the unified render engine
// owns control rendering (spec 017) — so only the form-level background-image
// loader remains in the IDE here.
use cobolt_codegen::{generate, generate_indexed};
use cobolt_compiler::{build_project, BuildOptions};
use cobolt_forms::paint::load_image_texture;
use cobolt_indexed::{
    load_indexed, record_to_text, resolve_path, save_indexed, text_to_record, IndexedDefinition,
};
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::indexed_ide::{compare_schema, create_empty_from_definition, SchemaDrift};
use cobolt_runtime::indexed_import::{definition_from_inspect, inspect_any_path};

use crate::form_runtime::FormRuntime;
use crate::i18n::{Language, Tr};
use crate::panels::debugger::{DebugAction, DebuggerPanel};
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

use crate::data_binding_guardian::{
    validate_binding_action, BindingActionGate, BindingActionGateReport,
};

/// Whitelist of *design-intent* control properties that an interactive Run-Form
/// adjustment may write back into the form definition (the control's new
/// defaults). Deliberately narrow: layout only, never runtime data (Rows,
/// Value, selection). Currently the DataGrid's column widths (carried in the
/// `AdvancedGrid` blob) and its `RowHeight`.
fn is_design_intent_prop(key: &str) -> bool {
    key == cobolt_forms::model::DATAGRID_ADVANCED_PROP || key == "RowHeight"
}

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
    /// Forms running as external `rcrun run-form` processes (the Run Form
    /// path): own window, own event loop — the IDE stays idle while they run.
    external_runs: Vec<crate::form_runtime::ExternalFormRun>,

    // Run-Form process/memory inspector (toolbar toggle; only samples while a
    // Live Interpreter is running).
    inspector: crate::inspector::ProcessInspector,
    show_inspector: bool,
    /// One-shot: the inspector window's initial size is applied only on the first
    /// frame after opening, so the user's own resizes are never reverted.
    inspector_sized: bool,

    // Debugger (Phase 7)
    debug_runner: DebugRunner,
    debugger: DebuggerPanel,
    debug_active: bool,
    /// When the debug session was started from a RAD designer (Debug Form),
    /// the owning form's `.cfrm` path — the debugger window then renders
    /// inside THAT designer viewport (in front of it), not the main IDE
    /// window. `None` = session started from the code editor.
    debug_owner_form: Option<PathBuf>,
    /// True when the active debug session controls an external
    /// `rcrun run-form --debug` process (over `@DBG` stdin/stdout lines)
    /// instead of the in-IDE `DebugRunner` thread.
    debug_external: bool,
    /// One-shot default sizing for the standalone debugger OS window (mirrors
    /// `inspector_sized`): applied on the session's first frame only, so the
    /// user's own window resizes are preserved afterwards.
    debugger_vp_sized: bool,

    // Frame-rate/perf instrumentation (why is the IDE busy while a form runs?)
    perf_window_start: Option<std::time::Instant>,
    perf_frames: u32,
    perf_busy_frames: u32,
    perf_ms_sum: f32,
    perf_ms_max: f32,
    /// Last completed 1-second window — displayed in the Run-Form Inspector.
    perf_fps: u32,
    perf_avg_ms: f32,
    perf_max_ms: f32,
    /// Last title sent to the OS window, so we only issue the viewport command
    /// on change (a per-frame AppKit set_title is wasted main-thread work).
    last_window_title: String,

    // Project model
    cobolt_project: Option<CoboltProject>,
    project_path: Option<PathBuf>,
    pending_user_control_delete: Option<String>,
    pending_indexed_delete: Option<String>,
    delete_cidx_file: bool,
    delete_data_file: bool,

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
    /// A failed connection/model test to surface in a modal dialog (`Some` ⇒ the
    /// error modal is open). Set when a "Test connection" (manual or triggered by
    /// selecting a model) returns an error.
    llm_test_error: Option<String>,
    /// In-flight "Detect API" probe from the settings dialog (spec 025).
    llm_detect_rx: Option<std::sync::mpsc::Receiver<Result<crate::llm::DetectedApi, String>>>,
    /// In-flight provider model-list fetch from the settings dialog.
    llm_models_rx: Option<std::sync::mpsc::Receiver<Result<Vec<String>, String>>>,

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
    /// A fatal form-runtime / codegen error to surface in a modal dialog. The
    /// IDE stays open; execution has already stopped on the interpreter thread.
    form_error: Option<String>,
    save_alert_msg: Option<String>,
    /// Which surface owns the save alert: `Some(idx)` = the designer viewport at
    /// `idx` (so the alert is not hidden behind it), `None` = the main IDE window.
    save_alert_designer: Option<usize>,

    /// Dev-agent change-set awaiting the developer's Approve/Reject (spec 025 T9).
    /// `Some` while a proposal is on screen; nothing is applied until approved.
    agent_preview: Option<crate::agent::AgentPreview>,
    /// Dev-agent prompt-bar state (spec 025 T10).
    agent_prompt: String,
    /// `(prompt-that-was-sent, reply channel)` — the prompt is recorded to memory
    /// only after a successful reply (spec 025 R16).
    agent_pending: Option<(String, std::sync::mpsc::Receiver<crate::llm::LlmResponse>)>,
    agent_history: Vec<crate::llm::ChatTurn>,
    /// Which form the in-memory `agent_history` belongs to (reload on change).
    agent_history_form: Option<PathBuf>,
    agent_status: Option<String>,
    /// Whether the read-only connection-log debug modal is currently open (spec 025).
    /// The content is the shared `llm` connection log, so it survives closing.
    agent_debug_open: bool,

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
    /// Pick a project icon image for Run Form / packaged app windows.
    PickProjectIcon,
    OpenGridData {
        cidx_path: PathBuf,
        def: IndexedDefinition,
    },
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
    "COPYBOOKS",
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

fn normalize_form_cobol_id(name: &str) -> String {
    name.trim().to_ascii_uppercase()
}

fn same_file_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// A one-line, human-readable label + accent colour for one agent operation
/// (spec 025 T10 preview).
fn agent_op_line(op: &crate::agent::AgentOp, tr: &crate::i18n::Tr) -> (String, Color32) {
    use crate::agent::AgentOp;
    match op {
        AgentOp::DeployControl {
            control_type, id, ..
        } => (
            format!(
                "{} {control_type} {}",
                tr.agent_op_deploy,
                id.as_deref().unwrap_or("")
            )
            .trim_end()
            .to_string(),
            Color32::from_rgb(120, 190, 120),
        ),
        AgentOp::SetProperty {
            control_id,
            key,
            value,
        } => (
            format!(
                "{} {control_id}.{key} = {}",
                tr.agent_op_set,
                agent_value_display(value)
            ),
            Color32::from_rgb(120, 170, 230),
        ),
        AgentOp::GenerateEventHandler {
            control_id, event, ..
        } => (
            format!("{} {control_id}.{event}", tr.agent_op_handler),
            Color32::from_rgb(200, 170, 110),
        ),
        AgentOp::CreateProcedure { name, .. } => (
            format!("{} {name}", tr.agent_op_procedure),
            Color32::from_rgb(190, 150, 210),
        ),
        AgentOp::Message { message } => (message.clone(), Color32::from_rgb(150, 150, 150)),
    }
}

fn agent_value_display(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn guardian_severity_label<'a>(tr: &'a Tr, severity: &cobolt_forms::GuardianSeverity) -> &'a str {
    match severity {
        cobolt_forms::GuardianSeverity::Blocker => tr.data_binding_severity_blocker,
        cobolt_forms::GuardianSeverity::Warning => tr.data_binding_severity_warning,
        cobolt_forms::GuardianSeverity::Info => tr.data_binding_severity_info,
    }
}

fn data_binding_action_label<'a>(tr: &'a Tr, action: BindingActionGate) -> &'a str {
    match action {
        BindingActionGate::SaveForm => tr.menu_save,
        BindingActionGate::RunForm => tr.tb_run,
        BindingActionGate::RunProject => tr.menu_run,
        BindingActionGate::DebugProject => tr.tb_debug,
        BindingActionGate::CheckProject => tr.tb_check,
        BindingActionGate::BuildProject => tr.tb_build,
        BindingActionGate::PackageProject => tr.menu_package_project,
    }
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
            external_runs: Vec::new(),
            inspector: crate::inspector::ProcessInspector::new(Default::default()),
            show_inspector: false,
            inspector_sized: false,
            debug_runner: DebugRunner::new(),
            debugger: DebuggerPanel::new(),
            debug_active: false,
            debug_owner_form: None,
            debug_external: false,
            debugger_vp_sized: false,

            perf_window_start: None,
            perf_frames: 0,
            perf_busy_frames: 0,
            perf_ms_sum: 0.0,
            perf_ms_max: 0.0,
            perf_fps: 0,
            perf_avg_ms: 0.0,
            perf_max_ms: 0.0,
            last_window_title: String::new(),

            cobolt_project: None,
            project_path: None,
            pending_user_control_delete: None,
            pending_indexed_delete: None,
            delete_cidx_file: false,
            delete_data_file: false,
            theme_packs: std::collections::HashMap::new(),
            theme_packs_loaded: false,

            settings_form: None,
            show_project_settings: false,
            settings_close_confirm: false,
            bg_texture: None,
            llm: crate::llm::LlmConfig::load(),
            llm_test_rx: None,
            llm_test_status: None,
            llm_test_error: None,
            llm_detect_rx: None,
            llm_models_rx: None,

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
            form_error: None,
            save_alert_msg: None,
            save_alert_designer: None,
            agent_preview: None,
            agent_prompt: String::new(),
            agent_pending: None,
            agent_history: Vec::new(),
            agent_history_form: None,
            agent_status: None,
            agent_debug_open: false,
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

    fn allow_data_binding_form_action(
        &mut self,
        action: BindingActionGate,
        form: &Form,
        label: &str,
    ) -> bool {
        let report = validate_binding_action(form, action);
        self.emit_data_binding_gate_report(label, &report);
        !report.blocked()
    }

    fn allow_data_binding_project_action(&mut self, action: BindingActionGate) -> bool {
        let reports = self.data_binding_project_reports(action);
        let blocked = reports.iter().any(|(_, report)| report.blocked());
        for (label, report) in &reports {
            self.emit_data_binding_gate_report(label, report);
        }
        !blocked
    }

    fn data_binding_project_reports(
        &self,
        action: BindingActionGate,
    ) -> Vec<(String, BindingActionGateReport)> {
        let mut reports = Vec::new();
        let mut seen = std::collections::HashSet::<PathBuf>::new();

        for (path, designer) in &self.designers {
            seen.insert(path.clone());
            reports.push((
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("form")
                    .to_owned(),
                validate_binding_action(&designer.form, action),
            ));
        }

        if let Some(inspect) = &self.inspect {
            if seen.insert(inspect.path.clone()) {
                reports.push((
                    inspect
                        .path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("form")
                        .to_owned(),
                    validate_binding_action(&inspect.designer.form, action),
                ));
            }
        }

        if let (Some(project), Some(root)) = (
            self.cobolt_project.as_ref(),
            self.project_path.as_ref().and_then(|path| path.parent()),
        ) {
            for rel in &project.files.forms {
                let path = root.join(rel);
                if !seen.insert(path.clone()) {
                    continue;
                }
                if let Ok(form) = load_form(&path) {
                    reports.push((
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("form")
                            .to_owned(),
                        validate_binding_action(&form, action),
                    ));
                }
            }
        }

        reports
    }

    fn emit_data_binding_gate_report(&mut self, label: &str, report: &BindingActionGateReport) {
        let blockers = report.blocker_count();
        let warnings = report.warning_count();
        let tr = self.lang.tr();
        let action = data_binding_action_label(&tr, report.action);
        if blockers > 0 {
            self.output.push_status(
                tr.data_binding_guardian_blocked
                    .replace("{action}", action)
                    .replace("{label}", label)
                    .replace("{count}", &blockers.to_string()),
            );
        } else if warnings > 0 {
            self.output.push_status(
                tr.data_binding_guardian_warning
                    .replace("{action}", action)
                    .replace("{label}", label)
                    .replace("{count}", &warnings.to_string()),
            );
        }
        for finding in &report.findings {
            if finding.severity == cobolt_forms::GuardianSeverity::Info {
                continue;
            }
            self.output.push_status(format!(
                "[{}] {}: {}",
                guardian_severity_label(&tr, &finding.severity),
                finding.code,
                finding.message
            ));
        }
    }

    fn do_run(&mut self) {
        if !self.allow_data_binding_project_action(BindingActionGate::RunProject) {
            return;
        }
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
        if !self.allow_data_binding_project_action(BindingActionGate::DebugProject) {
            return;
        }
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

        // Sync breakpoints into the interpreter's shared set and into the debug window.
        let bp_lines = self.editor.breakpoints_for(&path);
        let bp_set: std::collections::HashSet<u32> = bp_lines.iter().cloned().collect();
        self.debugger
            .set_source(path.display().to_string(), &source, &bp_set);
        {
            let mut guard = self.debug_runner.breakpoints.lock().unwrap();
            guard.clear();
            for line in &bp_lines {
                guard.insert(*line);
            }
        }

        self.debug_runner.start(path.display().to_string(), source);
        self.debug_active = true;
        self.debug_owner_form = None; // editor-owned session
        self.debug_external = false;
        self.debugger_vp_sized = false; // fresh window → default size
    }

    // ── Form Runtime Engine (Phase 6) ─────────────────────────────────────────

    /// Remove data bindings whose target/source control no longer exists from the
    /// designer form at `idx`. Self-heals a form whose orphan predates delete-time
    /// pruning, so the guardian can't block Run/Save with `missing-target-control`.
    /// Marks the designer dirty and reports the cleanup when anything is removed.
    fn autoclean_orphaned_bindings(&mut self, idx: usize) -> usize {
        if idx >= self.designers.len() {
            return 0;
        }
        let removed = self.designers[idx].1.form.prune_orphaned_data_bindings();
        if removed > 0 {
            self.designers[idx].1.dirty = true;
            self.output.push_status(format!(
                "Removed {removed} orphaned data-binding item(s) whose target/source control no longer exists."
            ));
        }
        removed
    }

    /// Run Form: launch the form as a standalone `rcrun run-form` process.
    fn do_run_form(&mut self, idx: usize) {
        self.launch_form_process(idx, false);
    }

    /// Debug Form: same standalone process, but with `--debug` — the live,
    /// interactive form window runs while the IDE debugger controls the
    /// interpreter over the process's stdin/stdout (`@DBG` JSON lines). The
    /// same wire protocol can later drive Android/iOS debuggees remotely.
    fn do_debug_form(&mut self, idx: usize) {
        self.launch_form_process(idx, true);
    }

    /// Launch the designer-at-`idx`'s form as an external rcrun process.
    /// Saves + regenerates COBOL first so the process always runs the latest
    /// version of the form. With `debug`, wires the IDE debugger to it.
    fn launch_form_process(&mut self, idx: usize, debug: bool) {
        if idx >= self.designers.len() {
            return;
        }
        // Self-heal orphaned data bindings (target/source control since deleted)
        // before the guardian runs, so a stale config can't block Run with
        // `missing-target-control`.
        self.autoclean_orphaned_bindings(idx);
        let form = self.designers[idx].1.form.clone();
        let label = self.designers[idx]
            .0
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("form")
            .to_owned();
        if !self.allow_data_binding_form_action(BindingActionGate::RunForm, &form, &label) {
            return;
        }
        refresh_data_binding_target_properties(&mut self.designers[idx].1.form);
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
            "── {} form {} ──",
            if debug { "Debugging" } else { "Running" },
            form_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
        ));

        // Kill any existing run for this form first (external + legacy).
        self.external_runs.retain_mut(|run| {
            if run.form_path == form_path {
                run.stop();
                false
            } else {
                true
            }
        });
        self.form_runtimes.retain_mut(|rt| {
            if rt.form_path == form_path {
                rt.stop();
                false
            } else {
                true
            }
        });

        // Pre-validate in-process so syntax/semantic errors surface instantly
        // in a modal with the red tree semaphore — execution is refused until
        // the code is fixed (same contract as the old in-IDE runtime).
        {
            use crate::runner::DiagSeverity;
            let diags = Self::validate_form_source(&form);
            if let Some(err) = diags
                .iter()
                .find(|d| d.severity == DiagSeverity::Error)
                .map(|d| format!("{} (line {})", d.message, d.line))
            {
                self.output
                    .push_status(format!("Error launching form: {err}"));
                self.set_element_status(&form_path, ElementStatus::Failed);
                self.form_error = Some(format!(
                    "This form's code has a syntax or semantic error — it cannot run until you fix it:\n\n{err}"
                ));
                return;
            }
        }

        // Run the form as a standalone `rcrun run-form` process — its own
        // window, event loop, and interpreter, exactly like a binary built by
        // `rcrun build`. The IDE stays idle while the form runs.
        let cbl_path = self.generated_cbl_path(&form_path);
        let theme_default = self
            .cobolt_project
            .as_ref()
            .and_then(|p| p.form_theme_default())
            .map(|s| s.to_owned());
        let project_icon = self.project_icon_abs_path();
        match crate::form_runtime::ExternalFormRun::spawn(
            form_path.clone(),
            form.name.clone(),
            &cbl_path,
            theme_default.as_deref(),
            project_icon.as_deref(),
            debug,
        ) {
            Ok(run) => {
                if debug {
                    // Wire the IDE debugger to the child: show the generated
                    // source in the debugger window, push the initial
                    // breakpoint set, and take ownership of the session. The
                    // child starts paused at line 1.
                    let source = std::fs::read_to_string(&cbl_path).unwrap_or_default();
                    let bp_lines = self.editor.breakpoints_for(&cbl_path);
                    let bp_set: std::collections::HashSet<u32> = bp_lines.iter().cloned().collect();
                    self.debugger.reset();
                    self.debugger
                        .set_source(cbl_path.display().to_string(), &source, &bp_set);
                    run.send_debug(&cobolt_runtime::RemoteDebugCmd::SetBreakpoints(bp_lines));
                    self.debug_active = true;
                    self.debug_external = true;
                    self.debug_owner_form = Some(form_path.clone());
                    self.debugger_vp_sized = false; // fresh window → default size
                    self.output.push_status(
                        "Debugging form in a separate rcrun process — paused at line 1.",
                    );
                } else {
                    self.output
                        .push_status("Form running as a separate rcrun process.");
                }
                self.external_runs.push(run);
                // The form's code compiled clean → green semaphore.
                self.set_element_status(&form_path, ElementStatus::Tested);
            }
            Err(e) => {
                self.output
                    .push_status(format!("Error launching form: {e}"));
                self.set_element_status(&form_path, ElementStatus::Failed);
                self.form_error = Some(e);
            }
        }
    }

    /// Render the debugger window on `ctx` and apply the returned action.
    /// Called with the main IDE ctx (editor-owned session) or from inside a
    /// designer viewport (RAD-owned session) so the window appears in front
    /// of whichever surface started it.
    /// Render the debugger as its own always-on-top OS window (a viewport,
    /// like the Run-Form Inspector) so the user can watch the running form and
    /// step through code side by side, without the designer window in the way.
    /// Closing the window stops the debug session.
    fn show_debugger_viewport(&mut self, ctx: &Context, tr: &crate::i18n::Tr) {
        let vp_id = ViewportId::from_hash_of("debugger_viewport");
        let mut builder = ViewportBuilder::default()
            .with_title(self.debugger.window_title())
            .with_resizable(true)
            .with_always_on_top();
        // Apply the default size ONLY on the first frame after the session
        // starts; afterwards the OS window size is the user's alone.
        if !self.debugger_vp_sized {
            builder = builder.with_inner_size([900.0, 520.0]);
            self.debugger_vp_sized = true;
        }
        ctx.show_viewport_immediate(vp_id, builder, |vp_ctx, _class| {
            let close = vp_ctx.input(|i| i.viewport().close_requested());
            let action = self.debugger.show_viewport_body(vp_ctx, tr);
            if close {
                self.handle_debug_action(DebugAction::Stop);
            } else if let Some(a) = action {
                self.handle_debug_action(a);
            }
        });
    }

    /// Apply a debugger toolbar/shortcut action to whichever session is live:
    /// the external `rcrun run-form --debug` process or the in-IDE DebugRunner.
    fn handle_debug_action(&mut self, action: DebugAction) {
        if self.debug_external {
            // Remote session: commands travel to the rcrun child as `@DBG`
            // stdin lines; Stop kills the process (form window closes too).
            use cobolt_runtime::RemoteDebugCmd;
            let owner = self.debug_owner_form.clone();
            let run = self
                .external_runs
                .iter_mut()
                .find(|r| r.debug && owner.as_ref() == Some(&r.form_path));
            match action {
                DebugAction::Stop => {
                    self.debugger.center_current_line_next_frame();
                    if let Some(run) = run {
                        run.stop();
                    }
                    self.external_runs
                        .retain_mut(|r| !(r.debug && owner.as_ref() == Some(&r.form_path)));
                    self.debug_active = false;
                    self.debug_external = false;
                    self.debug_owner_form = None;
                    self.debugger.reset();
                }
                DebugAction::Continue => {
                    if let Some(run) = run {
                        run.send_debug(&RemoteDebugCmd::Cmd(DebugCmd::Continue));
                    }
                }
                DebugAction::StepOver => {
                    if let Some(run) = run {
                        run.send_debug(&RemoteDebugCmd::Cmd(DebugCmd::StepOver));
                    }
                }
                DebugAction::StepIn => {
                    if let Some(run) = run {
                        run.send_debug(&RemoteDebugCmd::Cmd(DebugCmd::StepIn));
                    }
                }
                DebugAction::Pause => {
                    self.debugger.center_current_line_next_frame();
                    if let Some(run) = run {
                        run.send_debug(&RemoteDebugCmd::Cmd(DebugCmd::Pause));
                    }
                }
            }
            return;
        }
        match action {
            DebugAction::Stop => {
                self.debugger.center_current_line_next_frame();
                self.debug_runner.stop();
                self.debug_active = false;
                self.debug_owner_form = None;
                self.debugger.reset();
                self.editor.debug_line = None;
            }
            DebugAction::Continue => self.debug_runner.send_cmd(DebugCmd::Continue),
            DebugAction::StepOver => self.debug_runner.send_cmd(DebugCmd::StepOver),
            DebugAction::StepIn => self.debug_runner.send_cmd(DebugCmd::StepIn),
            DebugAction::Pause => {
                self.debugger.center_current_line_next_frame();
                self.debug_runner.send_cmd(DebugCmd::Pause);
            }
        }
    }

    /// Toggle the Run-Form inspector window (from the designer's RAD toolbar).
    /// On open it reloads the per-project dump config, clears the timeline, and
    /// arms the one-shot default sizing.
    fn toggle_inspector(&mut self) {
        self.show_inspector = !self.show_inspector;
        if self.show_inspector {
            self.inspector.config = self.inspector_config();
            self.inspector.reset();
            self.inspector_sized = false;
        }
    }

    /// Build the Run-Form inspector config from the current project's IDE
    /// settings (or defaults when no project is open).
    fn inspector_config(&self) -> crate::inspector::InspectorConfig {
        let mut cfg = crate::inspector::InspectorConfig::default();
        if let Some(p) = self.cobolt_project.as_ref() {
            cfg.dump_enabled = p.ide.inspector_dump_enabled;
            if !p.ide.inspector_dump_path.trim().is_empty() {
                cfg.dump_path = p.ide.inspector_dump_path.clone();
            }
        }
        cfg
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

    /// Validate a form's *generated* COBOL (syntax **and** semantic) and return
    /// the diagnostics. Validating the whole generated program — the same source
    /// the interpreter runs (spec 017) — keeps every event handler and the shared
    /// WORKING-STORAGE in one scope, so a data item defined by one handler never
    /// false-flags another.
    fn validate_form_source(form: &Form) -> Vec<crate::runner::DiagMsg> {
        use crate::runner::{DiagMsg, DiagSeverity};
        use cobolt_semantic::analyze;
        // Generated form source is always free-form.
        let src = cobolt_codegen::generate(form);
        let parse_result = parse(tokenize(&src, SourceFormat::Free));
        let mut diags = Vec::new();
        for d in &parse_result.diagnostics {
            use cobolt_parser::Severity as PSev;
            diags.push(DiagMsg {
                severity: match d.severity {
                    PSev::Error => DiagSeverity::Error,
                    PSev::Warning => DiagSeverity::Warning,
                },
                message: d.message.clone(),
                line: d.span.line,
                col: d.span.col,
            });
        }
        if let Some(prog) = parse_result.program {
            let sem = analyze(&prog);
            for d in &sem.diagnostics {
                use cobolt_semantic::Severity;
                diags.push(DiagMsg {
                    severity: match d.severity {
                        Severity::Error => DiagSeverity::Error,
                        Severity::Warning => DiagSeverity::Warning,
                        Severity::Info => DiagSeverity::Info,
                    },
                    message: d.message.clone(),
                    line: d.span.line,
                    col: d.span.col,
                });
            }
        }
        diags
    }

    /// Validate one form and update its tree semaphore (green = clean, red = has
    /// an error-severity issue). When `report`, the diagnostics are echoed to the
    /// Output panel. Returns the first error message, if any (for Run/Build gating
    /// dialogs).
    fn revalidate_form(
        &mut self,
        cfrm_path: &std::path::Path,
        form: &Form,
        report: bool,
    ) -> Option<String> {
        use crate::runner::{DiagSeverity, RunMsg};
        let diags = Self::validate_form_source(form);
        let first_error = diags
            .iter()
            .find(|d| d.severity == DiagSeverity::Error)
            .map(|d| format!("{} (line {})", d.message, d.line));
        if report {
            for d in &diags {
                self.output.push_msg(&RunMsg::Diagnostic(d.clone()));
            }
        }
        self.set_element_status(
            cfrm_path,
            if first_error.is_some() {
                ElementStatus::Failed
            } else {
                ElementStatus::Tested
            },
        );
        first_error
    }

    /// Validate every tracked form (open designers + on-disk) and refresh each
    /// form's tree semaphore. Returns the forms that have an error, as
    /// `(cfrm_path, first_error_message)` — used to block Build/Run up front.
    fn revalidate_all_forms(&mut self) -> Vec<(PathBuf, String)> {
        let mut forms: Vec<(PathBuf, Form)> = self
            .designers
            .iter()
            .map(|(p, d)| (p.clone(), d.form.clone()))
            .collect();
        let open_paths: std::collections::HashSet<PathBuf> =
            forms.iter().map(|(p, _)| p.clone()).collect();
        if let Some(root) = self
            .project_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_owned())
        {
            if let Some(proj) = &self.cobolt_project {
                for rel in &proj.files.forms {
                    let abs = root.join(rel);
                    if !open_paths.contains(&abs) {
                        if let Ok(form) = load_form(&abs) {
                            forms.push((abs, form));
                        }
                    }
                }
            }
        }
        let mut bad = Vec::new();
        for (path, form) in forms {
            if let Some(msg) = self.revalidate_form(&path, &form, false) {
                bad.push((path, msg));
            }
        }
        bad
    }

    fn do_check(&mut self) {
        if !self.allow_data_binding_project_action(BindingActionGate::CheckProject) {
            return;
        }
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
        if !self.new_project.name.ends_with(".project") {
            self.new_project.name.push_str(".project");
        }
        // The manifest is named after the project (e.g. "Inventory System.project.toml")
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
                // Light each form's tree semaphore on open (green = clean, red =
                // has a code error) so pre-existing problems are visible without
                // opening or running anything.
                let bad = self.revalidate_all_forms();
                if !bad.is_empty() {
                    self.output.push_status(format!(
                        "⚠ {} form(s) have code errors (marked red in the tree).",
                        bad.len()
                    ));
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
        if !self.allow_data_binding_project_action(BindingActionGate::PackageProject) {
            return;
        }
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
        if !self.allow_data_binding_project_action(BindingActionGate::BuildProject) {
            return;
        }
        self.regenerate_all_forms();
        self.regenerate_all_indexed_files();
        let Some(manifest) = self.project_path.clone() else {
            self.output
                .push_status("Open or create a project first (File → New/Open Project).");
            return;
        };

        // Refuse to compile while any form has a syntax/semantic error: mark the
        // offending forms red in the tree, list the problems in the Output panel,
        // and pop a modal. The build is not attempted until they are fixed.
        let bad_forms = self.revalidate_all_forms();
        if !bad_forms.is_empty() {
            self.output.clear();
            self.output
                .push_status("── Build blocked: fix these code errors first ──");
            for (path, msg) in &bad_forms {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("form");
                self.output.push_status(format!("  ✗ {name}: {msg}"));
            }
            self.form_error = Some(format!(
                "Build blocked — {} form(s) have code errors that must be fixed first \
                 (see the Output panel).",
                bad_forms.len()
            ));
            return;
        }

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

    /// Memory key for a form's dev-agent conversation.
    fn agent_history_key(form_path: &std::path::Path) -> String {
        let stem = form_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("form");
        format!("agent-{stem}")
    }

    /// The dev-agent prompt bar + preview (spec 025 T10). Shown above the inspector,
    /// which holds the live form. Submit sends the request (prompt + skills + memory
    /// + fresh form context); the reply is parsed into a previewed change-set that
    /// the developer Approves (applied as one undoable action, then saved) or Rejects.
    fn agent_bar(&mut self, ctx: &Context, tr: &crate::i18n::Tr) {
        let Some(form_path) = self.inspect.as_ref().map(|s| s.path.clone()) else {
            return;
        };
        let dir = self.project_dir();
        let key = Self::agent_history_key(&form_path);

        // (Re)load conversation memory when the inspected form changes.
        if self.agent_history_form.as_ref() != Some(&form_path) {
            self.agent_history = dir
                .as_ref()
                .map(|d| crate::llm::load_history(&d.join("data"), &key))
                .unwrap_or_default();
            self.agent_history_form = Some(form_path.clone());
            self.agent_preview = None;
            self.agent_status = None;
        }

        // Poll an in-flight request.
        let mut completed: Option<(String, crate::llm::LlmResponse)> = None;
        if let Some((prompt, rx)) = self.agent_pending.take() {
            let mut keep_pending = true;
            loop {
                match rx.try_recv() {
                    Ok(crate::llm::LlmResponse::Chunk(_text)) => {
                        // In the future, we can stream this text to the UI.
                        ctx.request_repaint();
                    }
                    Ok(resp) => {
                        completed = Some((prompt.clone(), resp));
                        keep_pending = false;
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        ctx.request_repaint();
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        completed = Some((
                            prompt.clone(),
                            crate::llm::LlmResponse::Err("The agent worker stopped.".into()),
                        ));
                        keep_pending = false;
                        break;
                    }
                }
            }
            if keep_pending {
                self.agent_pending = Some((prompt, rx));
            }
        }
        if let Some((prompt, resp)) = completed {
            match resp {
                crate::llm::LlmResponse::Ok(reply) => {
                    let form = self.inspect.as_ref().unwrap().designer.form.clone();
                    match crate::agent::parse_change_set(&reply) {
                        Ok(cs) => {
                            self.agent_status = cs.note.clone();
                            if !cs.operations.is_empty() {
                                self.agent_preview =
                                    Some(crate::agent::AgentPreview::build(cs, &form));
                            } else {
                                self.agent_preview = None;
                            }
                        }
                        Err(e) => {
                            if e.contains("did not contain a JSON change-set") {
                                // The model answered in plain text without JSON. Treat it as a conversation turn.
                                self.agent_status = Some(reply.trim().to_string());
                                self.agent_preview = None;
                            } else {
                                // The full request/response is in the connection log; open
                                // the debug modal so the developer can inspect it.
                                self.agent_debug_open = true;
                                self.agent_status = Some(e);
                                self.agent_preview = None;
                            }
                        }
                    }
                    // Record only the turns to memory (R16).
                    self.agent_history.push(crate::llm::ChatTurn::user(prompt));
                    self.agent_history
                        .push(crate::llm::ChatTurn::assistant(reply));
                    if let Some(d) = &dir {
                        crate::llm::save_history(&d.join("data"), &key, &self.agent_history);
                    }
                }
                crate::llm::LlmResponse::Err(e) => {
                    // Full request/response captured in the connection log.
                    self.agent_debug_open = true;
                    self.agent_status = Some(e);
                    self.agent_preview = None;
                }
                crate::llm::LlmResponse::Chunk(_) => {}
            }
        }

        let busy = self.agent_pending.is_some();
        let mut prompt = std::mem::take(&mut self.agent_prompt);
        let status = self.agent_status.clone();
        let preview = self.agent_preview.clone();
        let has_debug = crate::llm::has_connection_log();
        let mut do_send = false;
        let mut do_approve = false;
        let mut do_reject = false;
        let mut do_details = false;

        let frame = crate::theme::glass_panel_frame(
            ctx.style().visuals.panel_fill,
            &crate::theme::active(),
        );
        egui::TopBottomPanel::top("inspector_agent")
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("🤖").size(15.0));
                    ui.label(egui::RichText::new(tr.agent_mode).small().strong());
                    let can_send = !busy && !prompt.trim().is_empty();
                    if ui
                        .add_enabled(can_send, egui::Button::new(tr.ai_send))
                        .clicked()
                    {
                        do_send = true;
                    }
                    if busy {
                        ui.add(egui::Spinner::new());
                        ui.label(
                            egui::RichText::new(tr.agent_hint)
                                .small()
                                .color(Color32::from_gray(170)),
                        );
                    }
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut prompt)
                            .hint_text(tr.agent_hint)
                            .desired_width(ui.available_width())
                            .interactive(!busy),
                    );
                    if resp.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && !prompt.trim().is_empty()
                        && !busy
                    {
                        do_send = true;
                    }
                });

                if status.is_some() || has_debug {
                    ui.horizontal_wrapped(|ui| {
                        if let Some(s) = &status {
                            ui.label(
                                egui::RichText::new(s)
                                    .small()
                                    .color(Color32::from_rgb(210, 150, 90)),
                            );
                        }
                        // Reopen the retained raw response / error for debugging.
                        if has_debug && ui.small_button(tr.agent_details).clicked() {
                            do_details = true;
                        }
                    });
                }

                // Preview of the proposed change-set (nothing applied yet).
                if let Some(pv) = &preview {
                    ui.separator();
                    ui.label(egui::RichText::new(tr.agent_preview_title).strong());
                    if pv.change_set.operations.is_empty() {
                        ui.label(egui::RichText::new(tr.agent_no_ops).small());
                    }
                    egui::ScrollArea::vertical()
                        .max_height(180.0)
                        .auto_shrink([false, true])
                        .id_salt("agent_preview_ops")
                        .show(ui, |ui| {
                            for (op, st) in pv.change_set.operations.iter().zip(pv.statuses.iter())
                            {
                                let (label, colour) = agent_op_line(op, tr);
                                ui.horizontal_wrapped(|ui| {
                                    if let Some(err) = st {
                                        ui.label(
                                            egui::RichText::new("✗")
                                                .small()
                                                .color(Color32::from_rgb(220, 90, 90)),
                                        );
                                        ui.label(
                                            egui::RichText::new(format!("{label} — {err}"))
                                                .small()
                                                .color(Color32::from_rgb(220, 120, 120)),
                                        );
                                    } else {
                                        ui.label(egui::RichText::new("•").small().color(colour));
                                        ui.label(egui::RichText::new(label).small());
                                    }
                                });
                            }
                        });
                    ui.horizontal(|ui| {
                        let can_apply = pv.is_applicable();
                        if ui
                            .add_enabled(can_apply, egui::Button::new(tr.agent_approve))
                            .clicked()
                        {
                            do_approve = true;
                        }
                        if ui.button(tr.agent_reject).clicked() {
                            do_reject = true;
                        }
                    });
                }
            });

        self.agent_prompt = prompt;

        if do_send && !busy {
            let form = self.inspect.as_ref().unwrap().designer.form.clone();
            let context = crate::agent::build_context(&form);
            let (sys, skills) = match &dir {
                Some(d) => (
                    crate::agent::effective_prompt(d),
                    crate::agent::load_skills(d),
                ),
                None => (crate::agent::effective_prompt(Path::new("")), String::new()),
            };
            let sent = std::mem::take(&mut self.agent_prompt);
            let rx = crate::llm::spawn_agent_request(
                &self.llm,
                &sys,
                &skills,
                &self.agent_history,
                &sent,
                &context,
                None, // Let Orchestrator route to FormsDesigner or EventBinder
            );
            self.agent_status = None;
            self.agent_preview = None;
            self.agent_pending = Some((sent, rx));
        }

        if do_reject {
            self.reject_agent_preview();
        }

        if do_approve {
            if let Some(cs) = self.agent_preview.take().map(|p| p.change_set) {
                let saved = if let Some(st) = &mut self.inspect {
                    let n = st.designer.apply_agent_change_set(&cs);
                    if n > 0 {
                        let _ = save_form(&st.designer.form, &st.path);
                        st.designer.dirty = false;
                        st.mtime = file_mtime(&st.path);
                        Some(st.path.clone())
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(p) = saved {
                    self.project.refresh_form(&p);
                }
            }
        }

        if do_details {
            self.agent_debug_open = true;
        }
        // The modal itself is drawn once at the top level (render_agent_debug_modal)
        // so it also works from Settings → Test Connection, not only the agent bar.
    }

    /// The read-only debug modal showing the full error / raw model response (spec
    /// 025). Rendered once per frame at the top level so it can be opened from the
    /// agent bar **or** the settings Test Connection. Closing keeps the text so the
    /// "Details" button can reopen it.
    fn render_agent_debug_modal(&mut self, ctx: &Context, tr: &Tr) {
        if !self.agent_debug_open {
            return;
        }
        let text = crate::llm::connection_log_text();
        let mut open = true;
        let mut clear = false;
        egui::Window::new(tr.agent_debug_title)
            .collapsible(false)
            .resizable(true)
            .default_size([660.0, 440.0])
            .open(&mut open)
            .show(ctx, |ui| {
                if ui.button(tr.agent_clear_log).clicked() {
                    clear = true;
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut t = text;
                        ui.add(
                            egui::TextEdit::multiline(&mut t)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .desired_rows(22),
                        );
                    });
            });
        if clear {
            crate::llm::clear_connection_log();
            self.agent_debug_open = false;
        }
        if !open {
            self.agent_debug_open = false; // the log persists for reopening
        }
    }

    /// Approve the pending dev-agent change-set (spec 025 R6): apply its valid
    /// operations to the designer at `designer_idx` as **one** undoable action, then
    /// clear the preview. Returns how many operations were applied.
    // Wired to the editor prompt bar's Approve button in T10.
    #[allow(dead_code)]
    fn approve_agent_preview(&mut self, designer_idx: usize) -> usize {
        let Some(preview) = self.agent_preview.take() else {
            return 0;
        };
        if designer_idx >= self.designers.len() {
            return 0;
        }
        self.designers[designer_idx]
            .1
            .apply_agent_change_set(&preview.change_set)
    }

    /// Reject the pending change-set (spec 025 R7): discard it, mutating nothing.
    #[allow(dead_code)]
    fn reject_agent_preview(&mut self) {
        self.agent_preview = None;
    }

    /// The project's root directory (where `cobolt.toml` lives), if a project is open.
    fn project_dir(&self) -> Option<PathBuf> {
        self.project_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_owned())
    }

    fn form_cobol_id_conflict(
        &self,
        form_name: &str,
        exclude_path: Option<&Path>,
    ) -> Option<PathBuf> {
        let wanted = normalize_form_cobol_id(form_name);
        if wanted.is_empty() {
            return None;
        }

        for (path, designer) in &self.designers {
            if exclude_path.is_some_and(|exclude| same_file_path(path, exclude)) {
                continue;
            }
            if normalize_form_cobol_id(&designer.form.name) == wanted {
                return Some(path.clone());
            }
        }

        let Some(project) = &self.cobolt_project else {
            return None;
        };
        let Some(project_dir) = self.project_dir() else {
            return None;
        };
        for rel in &project.files.forms {
            let path = project_dir.join(rel);
            if exclude_path.is_some_and(|exclude| same_file_path(&path, exclude)) {
                continue;
            }
            let Ok(form) = load_form(&path) else {
                continue;
            };
            if normalize_form_cobol_id(&form.name) == wanted {
                return Some(path);
            }
        }
        None
    }

    fn reject_duplicate_form_cobol_id(
        &mut self,
        form_name: &str,
        exclude_path: Option<&Path>,
        action: &str,
    ) -> bool {
        if let Some(conflict) = self.form_cobol_id_conflict(form_name, exclude_path) {
            self.output.push_status(format!(
                "Cannot {action}: form COBOL ID '{form_name}' is already used by {}.",
                conflict.display()
            ));
            true
        } else {
            false
        }
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

        if kind == FileKind::Form {
            let form = match load_form(&path) {
                Ok(form) => form,
                Err(e) => {
                    self.output
                        .push_status(format!("Could not import form: {e}"));
                    return;
                }
            };
            if self.reject_duplicate_form_cobol_id(&form.name, Some(&path), "import form") {
                return;
            }
        }

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
        self.new_indexed = NewIndexedDialog::new();
        self.new_indexed.open = true;
    }

    fn do_remove_file_from_project(&mut self, rel: String) {
        let mut abs_path = None;
        if let Some(dir) = self.project_dir() {
            abs_path = Some(dir.join(&rel));
        }
        if let Some(proj) = &mut self.cobolt_project {
            proj.remove_file(&rel);
        }
        if let Some(abs) = abs_path {
            // Close inspector if it is showing this file
            if let Some(st) = &self.indexed_inspect {
                if st.path == abs {
                    self.indexed_inspect = None;
                }
            }
            // Close grid browser window if it is showing this file
            self.indexed_grids.retain(|(p, _)| p != &abs);
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
        // Drop data bindings orphaned by a since-deleted target/source control so
        // the saved .cfrm stays clean and Save is never blocked on a stale config.
        self.autoclean_orphaned_bindings(idx);
        let path = self.designers[idx].0.clone();
        let form = self.designers[idx].1.form.clone();
        let label = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("form")
            .to_owned();
        if !self.allow_data_binding_form_action(BindingActionGate::SaveForm, &form, &label) {
            return;
        }
        refresh_data_binding_target_properties(&mut self.designers[idx].1.form);
        let form_name = self.designers[idx].1.form.name.clone();
        if self.reject_duplicate_form_cobol_id(&form_name, Some(&path), "save form") {
            return;
        }
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

        // Dev-agent prompt bar (spec 025 T10) — the inspector has the live form, so
        // the agent can propose control/property/handler/procedure changes that the
        // developer previews and approves.
        if self.llm.is_configured() && self.inspect.is_some() {
            self.agent_bar(ctx, tr);
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
            let indexed_files: Vec<String> = self
                .cobolt_project
                .as_ref()
                .map(|project| project.files.indexed.clone())
                .unwrap_or_default();
            let action = {
                let d = &mut st.designer;
                let sel = ctrl_id.as_deref().and_then(|id| d.form.find_control(id));
                let form = &d.form as *const cobolt_forms::Form;
                let props = &mut d.properties;
                props.show(ui, unsafe { &*form }, sel, &indexed_files, tr)
            };
            for (cid, key, value) in action.set_props {
                st.designer.set_property(&cid, &key, value);
                changed = true;
            }
            if let Some(binding) = action.create_data_binding {
                let b = binding.clone();
                apply_data_binding_to_form(&mut st.designer.form, binding);
                seed_control_array_binding_preview_values(&mut st.designer, &b);
                st.designer.dirty = true;
                changed = true;
            }
            if let Some((old, new)) = action.rename_control {
                if st.designer.rename_control(&old, &new) {
                    changed = true;
                }
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
            let gate_input = self.inspect.as_ref().map(|st| {
                (
                    st.path.clone(),
                    st.designer.form.clone(),
                    st.path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("form")
                        .to_owned(),
                )
            });
            let gate_ok = gate_input
                .as_ref()
                .map(|(_, form, label)| {
                    self.allow_data_binding_form_action(BindingActionGate::SaveForm, form, label)
                })
                .unwrap_or(true);
            let saved_path = if gate_ok {
                if let Some(st) = &mut self.inspect {
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
                }
            } else {
                if let Some(st) = &mut self.inspect {
                    st.designer.dirty = true;
                }
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

        // Validate the saved form (syntax + semantic) so the tree semaphore turns
        // green (clean) or red (has an error) right after a save — the developer
        // learns about a bad event handler immediately, not only at Run time.
        if let Some(err) = self.revalidate_form(cfrm_path, &form, true) {
            self.output.push_status(format!(
                "⚠ {} has a code error — fix it before running: {err}",
                form.name
            ));
        }
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
            dir.join("COPYBOOKS")
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
        let (_, detail) = self.check_schema_drift(def);
        let data_path = match self.resolve_assign_path(def) {
            Some(p) => p,
            None => return,
        };
        if let Some((_, st)) = self.indexed_grids.iter_mut().find(|(p, _)| p == cidx_path) {
            st.panel.open(def, &data_path, detail.clone());
            st.def = def.clone();
            st.close_requested = false;
            return;
        }
        let mut panel = IndexedGridPanel::new();
        panel.open(def, &data_path, detail);
        self.indexed_grids.push((
            cidx_path.to_path_buf(),
            IndexedGridState {
                panel,
                def: def.clone(),
                close_requested: false,
            },
        ));
    }

    fn open_grid_for_indexed_with_data_path(
        &mut self,
        cidx_path: &Path,
        def: &IndexedDefinition,
        data_path: &Path,
    ) {
        let detail = if data_path.exists() {
            if let Ok(Some(info)) = inspect_any_path(data_path) {
                match compare_schema(def, &info) {
                    SchemaDrift::Ok => None,
                    SchemaDrift::Mismatch { detail } => Some(detail),
                    SchemaDrift::NoSchemaOnDisk => Some("no schema on disk".into()),
                }
            } else {
                None
            }
        } else {
            None
        };
        if let Some((_, st)) = self.indexed_grids.iter_mut().find(|(p, _)| p == cidx_path) {
            st.panel.open(def, data_path, detail.clone());
            st.def = def.clone();
            st.close_requested = false;
            return;
        }
        let mut panel = IndexedGridPanel::new();
        panel.open(def, data_path, detail);
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
        if let Some(dir) = self.project_dir() {
            let copybooks_dir = dir.join("COPYBOOKS");
            let _ = std::fs::create_dir_all(&copybooks_dir);
            let stem = cidx
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&def.name);
            let sel_path = copybooks_dir.join(format!("{}.SEL", stem));
            let fd_path = copybooks_dir.join(format!("{}.FD", stem));
            let sel_content = cobolt_codegen::generate_indexed_select(def);
            let fd_content = cobolt_codegen::generate_indexed_fd(def);
            let _ = std::fs::write(sel_path, sel_content);
            let _ = std::fs::write(fd_path, fd_content);
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

        // Initialize the empty indexed data file at the resolved assign_path
        if let Some(data_path) = self.resolve_assign_path(&def) {
            if let Some(parent) = data_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = create_empty_from_definition(&def, &data_path) {
                self.output
                    .push_status(format!("Could not create empty indexed data file: {e}"));
            }
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

                    // Open Indexed File Browser - hand-written grid/table icon
                    let grid_tip = if st.def.finalized { tr.btn_open_grid_browser } else { tr.grid_requires_finalize };
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
                ui.horizontal_top(|ui| {
                    // Left column: either raw text editor or the tree structure
                    ui.vertical(|ui| {
                        let left_w = (remaining_rect.width() - 330.0).max(350.0);
                        ui.set_min_width(left_w);
                        ui.set_max_width(left_w);
                        ui.set_height(remaining_rect.height());

                        if st.prefer_raw_editor {
                            let applied = st.panel.show_raw_editor_inline(ui, &mut st.def, tr);
                            if applied {
                                did_add_remove = true;
                            }
                        } else {
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
                        }
                    });

                    ui.separator();

                    // Right column: properties area - always shown!
                    ui.vertical(|ui| {
                        ui.set_min_width(300.0);  // total width for the labels+values block
                        ui.set_height(remaining_rect.height());
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_min_width(140.0);
                                ui.spacing_mut().item_spacing.y = 4.0;  // blank line gap so rows don't touch neighbors
                                if st.prefer_raw_editor {
                                    crate::panels::indexed_properties::IndexedPropertiesPanel::show_file_labels(ui, tr);
                                } else {
                                    st.panel.show_property_labels(ui, &st.def, tr);
                                }
                            });

                            ui.separator();

                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 4.0;  // blank line gap so rows don't touch neighbors
                                if st.prefer_raw_editor {
                                    property_edit = crate::panels::indexed_properties::IndexedPropertiesPanel::show_file_values(ui, &mut st.def, tr);
                                } else {
                                    property_edit = st.panel.show_property_values(ui, &mut st.def, tr);
                                }
                            });
                        });
                    });
                });
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
                let default_dir = self.project_dir().map(|d| d.join("data"));
                let mut spec = crate::file_dialog::DialogSpec::open()
                    .filter("Indexed Data File", &["idx"])
                    .filter("All files", &["*"]);
                if let Some(ref d) = default_dir {
                    spec = spec.directory(d);
                }
                self.begin_file_dialog(
                    FileRequest::OpenGridData {
                        cidx_path: path,
                        def,
                    },
                    spec,
                );
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

    /// Absolute path of the project's icon image, if configured.
    fn project_icon_abs_path(&self) -> Option<PathBuf> {
        let proj = self.cobolt_project.as_ref()?;
        let raw = proj.ide.project_icon.trim();
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

    fn project_icon_data(&self) -> Option<egui::IconData> {
        let path = self.project_icon_abs_path()?;
        let bytes = std::fs::read(path).ok()?;
        decode_icon_data(&bytes)
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
                    // Surface the failure in a modal (the full request/response is
                    // in the connection log, reachable via the modal's Details).
                    self.llm_test_status = Some(e.clone());
                    self.llm_test_error = Some(e);
                    self.llm_test_rx = None;
                }
                Ok(crate::llm::LlmResponse::Chunk(_)) => {}
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.llm_test_status = Some("The test worker stopped unexpectedly.".into());
                    self.llm_test_rx = None;
                }
            }
        }
        self.poll_llm_detect();
    }

    /// Poll an in-flight "Detect API" probe (spec 025). On success it fills the
    /// draft's endpoint (and model, if empty) from what the server advertises and
    /// reports the provider + model count next to the button.
    fn poll_llm_detect(&mut self) {
        let result = match &self.llm_detect_rx {
            Some(rx) => match rx.try_recv() {
                Ok(r) => Some(r),
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err("The detect worker stopped unexpectedly.".into()))
                }
            },
            None => return,
        };
        self.llm_detect_rx = None;
        match result {
            Some(Ok(found)) => {
                let n = found.models.len();
                let sample: Vec<&str> = found.models.iter().take(3).map(|s| s.as_str()).collect();
                if let Some(form) = &mut self.settings_form {
                    form.draft.llm_endpoint = found.endpoint.clone();
                    if !found.models.is_empty() {
                        form.set_available_models(found.models.clone());
                    }
                    if form.draft.llm_model.trim().is_empty() {
                        if let Some(first) = found.models.first() {
                            form.draft.llm_model = first.clone();
                        }
                    }
                }
                self.llm_test_status = Some(format!(
                    "Detected {}: {n} model(s){}",
                    found.provider,
                    if sample.is_empty() {
                        String::new()
                    } else {
                        format!(" — {}", sample.join(", "))
                    }
                ));
            }
            Some(Err(e)) => self.llm_test_status = Some(e),
            None => {}
        }
    }

    /// Poll the in-flight provider model-list fetch; on completion populate the
    /// settings form's picker (and auto-select the first model if none is set).
    fn poll_llm_models(&mut self) {
        let result = match &self.llm_models_rx {
            Some(rx) => match rx.try_recv() {
                Ok(r) => Some(r),
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err("The model-list worker stopped unexpectedly.".into()))
                }
            },
            None => return,
        };
        self.llm_models_rx = None;
        match result {
            Some(Ok(models)) => {
                let n = models.len();
                if let Some(form) = &mut self.settings_form {
                    if form.draft.llm_model.trim().is_empty() {
                        if let Some(first) = models.first() {
                            form.draft.llm_model = first.clone();
                        }
                    }
                    form.set_available_models(models);
                }
                self.llm_test_status = Some(format!("{n} model(s) available"));
            }
            Some(Err(e)) => self.llm_test_status = Some(e),
            None => {}
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

    /// Control/member metadata for the Settings → AI prompt editor. Prefer the
    /// form currently shown in the Main Pane, then any open designer viewport.
    fn settings_prompt_known_controls(&self) -> Vec<crate::panels::editor::KnownControl> {
        if let Some(st) = &self.inspect {
            return crate::panels::editor::build_known_controls(&st.designer.form);
        }
        self.designers
            .first()
            .map(|(_, d)| crate::panels::editor::build_known_controls(&d.form))
            .unwrap_or_default()
    }

    /// Render the project Settings form in the Main Pane. Returns the pending
    /// "Test connection" / "Browse background" actions for the caller to run.
    fn show_settings_pane(&mut self, ctx: &Context, tr: &Tr) {
        self.poll_llm_test(tr);
        self.poll_llm_models();
        if self.llm_test_rx.is_some() || self.llm_models_rx.is_some() {
            ctx.request_repaint();
        }
        let test_busy = self.llm_test_rx.is_some() || self.llm_models_rx.is_some();
        let test_status = self.llm_test_status.clone();
        let has_debug = crate::llm::has_connection_log();

        let themes: Vec<(&'static str, &'static str)> = crate::theme::THEMES
            .iter()
            .map(|t| (t.id, t.name))
            .collect();
        let prompt_known_controls = self.settings_prompt_known_controls();

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
                    action = form.show(
                        ui,
                        tr,
                        &themes,
                        test_busy,
                        test_status.as_deref(),
                        has_debug,
                        &prompt_known_controls,
                    );
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
                self.llm_test_error = None;
                self.llm_test_rx = Some(crate::llm::spawn_test(&cfg));
            }
        }
        if action.detect_api {
            if let Some(form) = &self.settings_form {
                self.llm_test_status = Some(tr.ai_detecting.to_string());
                self.llm_detect_rx = Some(crate::llm::spawn_detect(&form.draft.llm_endpoint));
            }
        }
        if action.fetch_models {
            // When the system prompt is empty, (re)load it from the project's
            // agentic_ai/assistant-prompt.md template — but never overwrite a prompt
            // the developer has written. (This is the general code/event assistant
            // prompt, distinct from the dev agent's system-prompt.md.)
            let proj_dir = self
                .project_path
                .as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf());
            if let (Some(form), Some(dir)) = (&mut self.settings_form, &proj_dir) {
                if form.draft.llm_system_prompt.trim().is_empty() {
                    form.draft.llm_system_prompt = crate::agent::effective_assistant_prompt(dir);
                    form.sync_prompt_editor_from_draft();
                }
            }
            if let Some(form) = &self.settings_form {
                if let Some(provider) = crate::llm::Provider::from_id(&form.draft.llm_provider) {
                    self.llm_test_status = Some(tr.ai_detecting.to_string());
                    self.llm_models_rx = Some(crate::llm::spawn_list_models(
                        provider,
                        &form.draft.llm_endpoint,
                        &form.draft.llm_api_key,
                    ));
                }
            }
        }
        if action.show_debug {
            self.agent_debug_open = true;
        }
        if action.browse_bg {
            self.begin_file_dialog(
                FileRequest::PickBackgroundImage,
                crate::file_dialog::DialogSpec::open()
                    .filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp"]),
            );
        }
        if action.browse_project_icon {
            self.begin_file_dialog(
                FileRequest::PickProjectIcon,
                crate::file_dialog::DialogSpec::open()
                    .filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp"]),
            );
        }

        // Connection-test error modal — shown when a manual "Test connection" or a
        // model-selection test returns an error.
        if let Some(err) = self.llm_test_error.clone() {
            let mut close = false;
            let mut details = false;
            egui::Window::new(tr.ai_test_failed_title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_max_width(460.0);
                    ui.label(
                        egui::RichText::new(&err).color(egui::Color32::from_rgb(220, 120, 120)),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if crate::llm::has_connection_log() && ui.button(tr.agent_details).clicked()
                        {
                            details = true;
                        }
                        if ui.button(tr.inspect_close).clicked() {
                            close = true;
                        }
                    });
                });
            if details {
                self.agent_debug_open = true;
                self.llm_test_error = None;
            }
            if close {
                self.llm_test_error = None;
            }
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
            const CYCLE: f64 = 7.5;
            // Schedule the next repaint only when the quote cycle or fade requires it.
            // Avoids continuous max-FPS repaints when the welcome pane is visible.
            let now = ctx.input(|i| i.time);
            let elapsed = if self.welcome_quote_start_time == 0.0 {
                0.0
            } else {
                now - self.welcome_quote_start_time
            };
            let remaining = (CYCLE - elapsed).max(0.05);
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(remaining));

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

    /// Modal shown when a form's generated COBOL fails to launch (parse /
    /// semantic error) or the interpreter reports a fatal runtime error. The
    /// message is also in the Output console; this dialog just makes it
    /// unmissable. Closing it leaves the IDE fully usable.
    fn show_form_error(&mut self, ctx: &Context) {
        let msg = match &self.form_error {
            Some(m) => m.clone(),
            None => return,
        };
        let mut open = true;
        egui::Window::new("⛔ COBOL error")
            .id(egui::Id::new("form_runtime_error"))
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .default_width(520.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Execution stopped. See the Output panel for details.")
                        .strong(),
                );
                ui.add_space(6.0);
                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&msg).monospace());
                    });
                ui.add_space(8.0);
                if ui.button("OK").clicked() {
                    self.form_error = None;
                }
            });
        if !open {
            self.form_error = None;
        }
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
        let form_name = self.new_form.form_name.trim().to_owned();
        if form_name.is_empty() {
            self.output.push_status("Form COBOL ID cannot be empty.");
            return;
        }
        if self.reject_duplicate_form_cobol_id(&form_name, None, "create form") {
            return;
        }
        let mut form = Form::new(form_name.clone(), self.new_form.title.clone(), w, h);
        form.background_color = "00000000".into(); // transparent — matches IDE glass

        let default_name = format!("{}.cfrm", form_name.to_lowercase());
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
        if self.reject_duplicate_form_cobol_id(&form.name, Some(&path), "create form") {
            return;
        }
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
        let mut rep = false;
        if let Some(result) = crate::file_dialog::take(APP_FILE_KEY) {
            if let Some(request) = self.pending_file.take() {
                if let Some(path) = result {
                    self.apply_file_result(request, path);
                }
            }
            rep = true;
        }
        rep || self.pending_file.is_some()
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
            FileRequest::PickProjectIcon => self.set_project_icon(path),
            FileRequest::OpenGridData { cidx_path, def } => {
                self.open_grid_for_indexed_with_data_path(&cidx_path, &def, &path);
            }
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
            if let Some(form) = &mut self.settings_form {
                form.set_bg_image(proj.ide.background_image.clone());
            }
            self.bg_texture = None;
            self.do_save_project();
        }
    }

    /// Store the chosen project icon in IDE settings, relative to the project
    /// root when possible, and persist it for Run Form / packaged windows.
    fn set_project_icon(&mut self, path: PathBuf) {
        let rel = self
            .project_path
            .as_ref()
            .and_then(|p| p.parent())
            .and_then(|dir| relative_to(&path, dir))
            .unwrap_or_else(|| path.display().to_string());
        if let Some(proj) = &mut self.cobolt_project {
            proj.ide.project_icon = rel;
            if let Some(form) = &mut self.settings_form {
                form.set_project_icon(proj.ide.project_icon.clone());
            }
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
        let frame_start = std::time::Instant::now();

        // ── Compute the translation table for this frame ───────────────────────
        let tr = self.lang.tr();
        crate::i18n::set_language(ctx, self.lang);

        // Update window title to reflect the current project's build mode.
        {
            let mode_suffix = self
                .cobolt_project
                .as_ref()
                .map(|p| {
                    if p.project.debug_compilation {
                        tr.title_debug_mode
                    } else {
                        tr.title_release_mode
                    }
                })
                .unwrap_or("");
            let title = if mode_suffix.is_empty() {
                format!("PowerRustCOBOL v{VERSION}")
            } else {
                format!("PowerRustCOBOL v{VERSION} — {mode_suffix}")
            };
            // Only touch the OS window when the title actually changes.
            if title != self.last_window_title {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
                self.last_window_title = title;
            }
        }

        // Surface AI request activity (sending → streaming → done / errors, plus
        // live reasoning) into the output/log pane, drained on the UI thread so it
        // appears line-by-line as the request unfolds.
        for entry in crate::llm::drain_ai_log() {
            self.output.push_ai_line(entry.kind, entry.text);
        }

        // Read-only LLM debug modal (spec 025) — rendered once at the top level so it
        // works from the agent bar and from Settings → Test Connection alike.
        self.render_agent_debug_modal(ctx, &tr);

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

        // ── Drain debugger events (in-IDE DebugRunner sessions only — remote
        // `rcrun --debug` sessions are fed from the external-run drain) ───────
        if self.debug_active && !self.debug_external {
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
                self.debug_owner_form = None;
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
        // Fatal COBOL error (launch or runtime) — modal, IDE stays open.
        self.show_form_error(ctx);

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

        // ── Debugger floating window ──────────────────────────────────────────
        // The debugger renders as its own standalone always-on-top OS window,
        // so the user can watch the running form while stepping through code.
        if self.debug_active {
            self.show_debugger_viewport(ctx, &tr);
        }

        // ── Run-Form process/memory inspector (bottom dock) ───────────────────
        self.show_inspector_panel(ctx);

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
                    ProjectPanelEvent::ConfirmRemoveIndexed(rel) => {
                        self.pending_indexed_delete = Some(rel);
                    }
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

        self.show_indexed_delete_confirm(ctx, &tr);

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

        // ── External form runs (`rcrun run-form` processes) ──────────────────────
        // Pipe their stdout into the Output pane — except `@DBG ` lines, which
        // are debugger events from a `--debug` child and feed the debugger
        // panel. Reap exited processes, surfacing a failure exit in a modal.
        {
            let mut ext_error: Option<String> = None;
            let mut dbg_events: Vec<cobolt_runtime::DebugEvent> = Vec::new();
            let mut route =
                |run: &crate::form_runtime::ExternalFormRun,
                 output: &mut crate::panels::output::OutputPanel,
                 dbg_events: &mut Vec<cobolt_runtime::DebugEvent>| {
                    for line in run.drain_output() {
                        match line.strip_prefix("@DBG ") {
                            Some(json) if run.debug => {
                                match serde_json::from_str::<cobolt_runtime::DebugEvent>(json) {
                                    Ok(ev) => dbg_events.push(ev),
                                    Err(e) => {
                                        output.push_status(format!("debug: bad @DBG event: {e}"))
                                    }
                                }
                            }
                            _ => output.push_line(line),
                        }
                    }
                };
            for run in &self.external_runs {
                route(run, &mut self.output, &mut dbg_events);
            }
            let mut i = 0;
            while i < self.external_runs.len() {
                if self.external_runs[i].is_running() {
                    i += 1;
                    continue;
                }
                let mut run = self.external_runs.remove(i);
                // Drain any output that raced the exit.
                route(&run, &mut self.output, &mut dbg_events);
                if let Some(err) = run.take_exit_error() {
                    self.output
                        .push_status(format!("Form {} failed: {err}", run.form_name));
                    if ext_error.is_none() {
                        ext_error = Some(err);
                    }
                } else {
                    self.output
                        .push_status(format!("── Form {} finished ──", run.form_name));
                }
                // The debug child ended → close the debug session with it.
                if run.debug
                    && self.debug_external
                    && self.debug_owner_form.as_ref() == Some(&run.form_path)
                {
                    self.debug_active = false;
                    self.debug_external = false;
                    self.debug_owner_form = None;
                    self.debugger.reset();
                }
            }
            if !dbg_events.is_empty() {
                for ev in dbg_events {
                    self.debugger.apply_event(ev);
                }
                ctx.request_repaint();
            }
            if let Some(err) = ext_error {
                self.form_error = Some(err);
            }
        }

        // ── Running form viewports (Phase 6) ─────────────────────────────────────
        // Drain display output and state updates from every running runtime each frame.
        let mut display_lines: Vec<String> = Vec::new();
        let mut fatal_error: Option<String> = None;
        for rt in self.form_runtimes.iter_mut() {
            display_lines.extend(rt.drain_display());
            rt.drain_state();
            // Surface a fatal runtime error (already logged to the console by the
            // interpreter thread) in a modal dialog — the IDE stays open.
            if fatal_error.is_none() {
                fatal_error = rt.take_error();
            }
        }
        for line in display_lines {
            self.output.push_line(line);
        }
        if let Some(err) = fatal_error {
            self.form_error = Some(err);
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
            let icon = self.project_icon_data();
            let mut builder = ViewportBuilder::default()
                .with_title(&title)
                .with_inner_size([fw + 4.0, fh + 4.0])
                .with_resizable(true)
                .with_transparent(true);
            if let Some(icon) = icon {
                builder = builder.with_icon(icon);
            }

            ctx.show_viewport_immediate(vp_id, builder, |vp_ctx, _class| {
                if vp_ctx.input(|inp| inp.viewport().close_requested()) {
                    // User closed the window → cooperatively cancel + quit so
                    // even a looping handler aborts and the window can close.
                    self.form_runtimes[i].request_stop();
                    vp_ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                self.show_running_form_window(vp_ctx, i);
            });
        }

        // Reap finished runtimes.
        self.form_runtimes.retain(|rt| rt.is_running());

        // Remove any designer windows the user has closed.
        self.designers.retain(|(_, d)| !d.close_requested);
        self.indexed_grids.retain(|(_, g)| !g.close_requested);

        // Reactive event loop — do NOT repaint continuously (an unconditional
        // top-level repaint pegged a whole core even when a form sat idle between
        // timer ticks; the inspector correctly flagged it). Only schedule a
        // repaint when there is real work to drain: queued interpreter events for
        // a running form, or the console runner polling its output. Otherwise the
        // app sleeps — timer ticks (render Timer arm), animations, channel output,
        // and user input each schedule their own targeted repaints.
        if self.runner.is_running() {
            // The console runner streams output over a channel; poll at ~20 Hz.
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        } else if !self.form_runtimes.is_empty() {
            // A form is running: repaint fast (60 Hz) only while interpreter
            // events are queued to drain; otherwise a slow 5 Hz safety heartbeat
            // keeps timer ticks firing and channel output draining without
            // pegging a core. When NO form runs, nothing is scheduled here and
            // the app sleeps until the next user input (fully reactive).
            let busy = self.form_runtimes.iter().any(|rt| rt.pending_events() > 0);
            let ms = if busy { 16 } else { 200 };
            ctx.request_repaint_after(std::time::Duration::from_millis(ms));
        } else if !self.external_runs.is_empty() {
            // External `rcrun run-form` processes do their own rendering — the
            // IDE only needs a slow heartbeat to drain their stdout into the
            // Output pane and to notice when they exit.
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }

        // ── Perf instrumentation ──────────────────────────────────────────────
        // Accumulate this frame; once per second publish fps / frame cost to the
        // Run-Form Inspector and, while a form runs, to /tmp/cobolt-debug.log —
        // so "why is the IDE busy while the form idles?" is answerable with data.
        {
            let frame_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
            self.perf_frames += 1;
            self.perf_ms_sum += frame_ms;
            if frame_ms > self.perf_ms_max {
                self.perf_ms_max = frame_ms;
            }
            if self.form_runtimes.iter().any(|rt| rt.pending_events() > 0) {
                self.perf_busy_frames += 1;
            }
            let elapsed = self
                .perf_window_start
                .get_or_insert_with(std::time::Instant::now)
                .elapsed();
            if elapsed.as_secs_f32() >= 1.0 {
                self.perf_fps = self.perf_frames;
                self.perf_avg_ms = self.perf_ms_sum / self.perf_frames.max(1) as f32;
                self.perf_max_ms = self.perf_ms_max;
                if !self.form_runtimes.is_empty() || !self.external_runs.is_empty() {
                    crate::runner::dbg_log(&format!(
                        "[PERF] fps={} avg={:.1}ms max={:.1}ms busy_frames={} forms={} external={} designers={} inspector={}",
                        self.perf_fps,
                        self.perf_avg_ms,
                        self.perf_max_ms,
                        self.perf_busy_frames,
                        self.form_runtimes.len(),
                        self.external_runs.len(),
                        self.designers.len(),
                        self.show_inspector,
                    ));
                }
                self.perf_frames = 0;
                self.perf_busy_frames = 0;
                self.perf_ms_sum = 0.0;
                self.perf_ms_max = 0.0;
                self.perf_window_start = Some(std::time::Instant::now());
            }
        }
    }
}

// ── Preview window contents ───────────────────────────────────────────────────

/// The property key that holds a control's live preview value, by type. The
/// preview keeps one value per control; this maps it to the key the engine reads.
pub(crate) fn preview_value_key(ct: &cobolt_forms::ControlType) -> &'static str {
    use cobolt_forms::ControlType as CT;
    match ct {
        CT::TextBox => "Text",
        CT::PictureBox => "ImagePath",
        CT::CheckBox | CT::RadioButton => "Checked",
        CT::TabControl => "SelectedTab",
        CT::ComboBox | CT::ListBox | CT::Slider | CT::ProgressBar | CT::NumericUpDown => "Value",
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
            let key = preview_value_key(&base.control_type).to_owned();
            c.set_prop(key, cobolt_forms::PropValue::String(v.clone()));
            // Ensure ControlArray-mapped properties that are not the type's primary
            // preview key (ImagePath for thumbs, Checked for bools) still receive data.
            c.set_prop(
                "ImagePath".to_string(),
                cobolt_forms::PropValue::String(v.clone()),
            );
            c.set_prop(
                "Checked".to_string(),
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
    run_id: u64,
}
impl cobolt_forms::render::FormState for RunState<'_> {
    fn run_id(&self) -> u64 {
        self.run_id
    }
    fn live(&self, base: &cobolt_forms::Control) -> cobolt_forms::Control {
        if base
            .id
            .to_ascii_lowercase()
            .starts_with("groupbox-2.groupbox-2-")
        {
            let has_state = self.state.keys().any(|k| k.eq_ignore_ascii_case(&base.id));
            tracing::debug!(target: "databinding", "LIVE for instance {} has_ctrl_state={}", base.id, has_state);
        }
        let key = self
            .state
            .keys()
            .find(|k| k.eq_ignore_ascii_case(&base.id))
            .cloned();
        match key.and_then(|k| self.state.get(&k)) {
            Some(s) => cobolt_forms::render::merge_props(base, s.props.iter()),
            None => base.clone(),
        }
    }
    fn visible(&self, base: &cobolt_forms::Control) -> bool {
        let key = self.state.keys().find(|k| k.eq_ignore_ascii_case(&base.id));
        key.and_then(|k| self.state.get(k))
            .map(|s| s.visible)
            .unwrap_or(true)
    }
    fn enabled(&self, base: &cobolt_forms::Control) -> bool {
        let key = self.state.keys().find(|k| k.eq_ignore_ascii_case(&base.id));
        key.and_then(|k| self.state.get(k))
            .map(|s| s.enabled)
            .unwrap_or(true)
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
        let dform = &self.designers[idx].1.form;
        cobolt_forms::paint::set_active_theme(ctx, self.designers[idx].1.active_theme_pack.clone());
        cobolt_forms::paint::set_glass_style(ctx, dform.glass_style);

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
        // Refresh ALL databindings first (DataGrids get fresh Rows/DataSource etc.; arrays get counts).
        // Then do array-specific seeding of preview_state values for #N instances.
        refresh_data_binding_target_properties(&mut self.designers[idx].1.form);
        {
            let array_bindings: Vec<_> = self.designers[idx]
                .1
                .form
                .data_bindings
                .iter()
                .filter(|b| matches!(&b.target, BindingTargetDescriptor::ControlArray { .. }))
                .cloned()
                .collect();
            for b in &array_bindings {
                seed_control_array_binding_preview_values(&mut self.designers[idx].1, b);
            }
        }
        // Keep the main editor's intellisense in sync with current form (for RefreshBinding etc on array groupboxes)
        self.editor.known_controls =
            crate::panels::editor::build_known_controls(&self.designers[idx].1.form);
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
        let active_tabs: cobolt_forms::containers::ActiveTabs = controls
            .iter()
            .filter(|c| matches!(c.control_type, cobolt_forms::ControlType::TabControl))
            .filter_map(|c| {
                values_snap
                    .get(&c.id)
                    .and_then(|v| v.trim().parse::<u32>().ok())
                    .map(|tab| (c.id.clone(), tab))
            })
            .collect();
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
        for (id, key, val) in updates {
            let expected = controls
                .iter()
                .find(|c| c.id == id)
                .map(|c| preview_value_key(&c.control_type))
                .unwrap_or("Caption");
            if key == expected {
                self.designers[idx].1.preview_state.insert(id, val);
            }
        }

        // Use a conservative heartbeat for the preview viewport. The animation
        // block inside already advances on dt and only needs repaints during
        // active entrance animations. This prevents max-FPS CPU spin.
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

// ── Run-Form process/memory inspector panel ──────────────────────────────────

impl CoboltApp {
    /// Bottom dock (~1/6 of the screen) with real-time process/memory charts for
    /// the running form. Only samples while a Live Interpreter is active; drawn
    /// only when toggled on from the toolbar. Purely observational.
    fn show_inspector_panel(&mut self, ctx: &Context) {
        if !self.show_inspector {
            return;
        }
        let form_running = !self.form_runtimes.is_empty() || !self.external_runs.is_empty();
        // Sample only while a form runs; `processing` = the interpreter has queued
        // work, so growth-while-idle can be told apart from real processing.
        if form_running {
            let processing = self.form_runtimes.iter().any(|rt| rt.pending_events() > 0);
            // One CPU timeline per open external rcrun run-form process, keyed
            // by pid so the inspector can label + retire a series per form.
            let tracked: Vec<(u32, String)> = self
                .external_runs
                .iter()
                .map(|run| (run.pid(), run.form_name.clone()))
                .collect();
            self.inspector.maybe_sample(processing, &tracked);
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        // The inspector lives in its own always-on-top OS window (a viewport, like
        // the running form) so the charts stay visible while you interact with the
        // app and can correlate a spike with what you just did.
        let vp_id = ViewportId::from_hash_of("run_form_inspector");
        // Apply the default size ONLY on the first frame after opening; on later
        // frames we omit `inner_size` so egui never re-commands a size and the
        // user's own window resizes are preserved.
        let mut builder = ViewportBuilder::default()
            .with_title("📊 Run-Form Inspector")
            .with_resizable(true)
            .with_always_on_top();
        if !self.inspector_sized {
            let sh = ctx.screen_rect();
            builder = builder.with_inner_size([
                (sh.width() / 3.0).clamp(560.0, 900.0),
                (sh.height() / 6.0).clamp(200.0, 320.0),
            ]);
            self.inspector_sized = true;
        }
        ctx.show_viewport_immediate(vp_id, builder, |vp_ctx, _class| {
            if vp_ctx.input(|i| i.viewport().close_requested()) {
                self.show_inspector = false;
            }
            // Animate the charts only while a form is being sampled; when no
            // form runs the window is static and requests no repaints (idle).
            if form_running {
                vp_ctx.request_repaint_after(std::time::Duration::from_millis(250));
            }
            egui::CentralPanel::default().show(vp_ctx, |ui| {
                self.inspector_body(ui, form_running);
            });
        });
    }

    /// The inspector window contents: health header + the four sparklines.
    fn inspector_body(&mut self, ui: &mut egui::Ui, form_running: bool) {
        {
            ui.horizontal(|ui| {
                ui.strong("📊 Run-Form Inspector");
                ui.separator();
                if !form_running {
                    ui.colored_label(
                        egui::Color32::from_rgb(200, 170, 90),
                        "no form running — start Run Form to sample",
                    );
                } else if let Some(a) = &self.inspector.last_anomaly {
                    ui.colored_label(egui::Color32::from_rgb(240, 100, 100), format!("⚠ {a}"));
                } else {
                    ui.colored_label(egui::Color32::from_rgb(120, 200, 120), "healthy");
                }
                // IDE render cost: repaints/sec and time spent inside update().
                // High fps with a running form = something requests repaints
                // continuously; high avg ms = each frame itself is expensive.
                if self.perf_fps > 0 {
                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!(
                            "IDE: {} fps · {:.1} ms/frame (max {:.1})",
                            self.perf_fps, self.perf_avg_ms, self.perf_max_ms
                        ))
                        .monospace()
                        .size(11.0)
                        .color(egui::Color32::from_gray(170)),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✕ close").clicked() {
                        self.show_inspector = false;
                    }
                    let dumping = self.inspector.config.dump_enabled;
                    ui.label(if dumping {
                        format!("dumps → {}", self.inspector.config.dump_path)
                    } else {
                        "dumps off (Settings)".to_string()
                    });
                });
            });
            ui.separator();

            let hist = self.inspector.history();
            let latest = self.inspector.latest().unwrap_or_default();
            let n = hist.len();
            let cpu: Vec<f32> = hist.iter().map(|s| s.cpu_pct).collect();
            let rss: Vec<f32> = hist
                .iter()
                .map(|s| s.rss_bytes as f32 / (1024.0 * 1024.0))
                .collect();
            let kids: Vec<f32> = hist.iter().map(|s| s.children as f32).collect();
            // System memory used (MB) as a chart — replaces the redundant
            // second CPU% metric (System CPU) with a distinct signal.
            let sysmem: Vec<f32> = hist
                .iter()
                .map(|s| s.sys_mem_used as f32 / (1024.0 * 1024.0))
                .collect();
            let sysmem_total = (latest.sys_mem_total as f32 / (1024.0 * 1024.0)).max(1.0);

            let avail = ui.available_size();
            let chart_w = (avail.x - 24.0) / 4.0;
            // Reserve the top ~55% for the four charts; the process tree
            // (5th panel) fills the rest of the window below.
            let chart_h = (avail.y * 0.55).clamp(64.0, 190.0);
            ui.horizontal(|ui| {
                ui.strong("IDE stats");
            });
            ui.horizontal(|ui| {
                Self::sparkline(
                    ui,
                    chart_w,
                    chart_h,
                    "Process CPU",
                    "%",
                    &cpu,
                    latest.cpu_pct,
                    Some(100.0),
                    egui::Color32::from_rgb(120, 200, 255),
                );
                Self::sparkline(
                    ui,
                    chart_w,
                    chart_h,
                    "Memory (RSS)",
                    "MB",
                    &rss,
                    latest.rss_bytes as f32 / (1024.0 * 1024.0),
                    None,
                    egui::Color32::from_rgb(150, 230, 150),
                );
                Self::sparkline(
                    ui,
                    chart_w,
                    chart_h,
                    "Child procs",
                    "",
                    &kids,
                    latest.children as f32,
                    None,
                    egui::Color32::from_rgb(240, 200, 120),
                );
                Self::sparkline(
                    ui,
                    chart_w,
                    chart_h,
                    "System Mem",
                    "MB",
                    &sysmem,
                    latest.sys_mem_used as f32 / (1024.0 * 1024.0),
                    Some(sysmem_total),
                    egui::Color32::from_rgb(200, 160, 240),
                );
            });
            if n == 0 && form_running {
                ui.weak("sampling…");
            }

            // ── Per-form CPU charts ─────────────────────────────────────────
            // One chart per open `rcrun run-form` process, so a form that's
            // eating CPU is identifiable by name at a glance instead of only
            // showing up as a lump in "Child procs" above.
            let child_count = self.inspector.child_series().len();
            if child_count > 0 {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.strong("Per-form CPU (rcrun)");
                    ui.weak(format!("({child_count} running)"));
                });
                let palette = [
                    egui::Color32::from_rgb(120, 200, 255),
                    egui::Color32::from_rgb(255, 170, 120),
                    egui::Color32::from_rgb(180, 220, 120),
                    egui::Color32::from_rgb(230, 140, 220),
                    egui::Color32::from_rgb(240, 210, 100),
                ];
                let per_row = (avail.x / (chart_w + 8.0)).floor().max(1.0) as usize;
                egui::ScrollArea::vertical()
                    .id_salt("per_form_cpu_scroll")
                    .max_height(chart_h + 12.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for chunk_idx in 0..child_count.div_ceil(per_row) {
                            ui.horizontal(|ui| {
                                let start = chunk_idx * per_row;
                                let end = (start + per_row).min(child_count);
                                for (i, series) in
                                    self.inspector.child_series()[start..end].iter().enumerate()
                                {
                                    let cpu_vals: Vec<f32> = series.cpu.iter().cloned().collect();
                                    let current = cpu_vals.last().cloned().unwrap_or(0.0);
                                    let color = palette[(start + i) % palette.len()];
                                    Self::sparkline(
                                        ui,
                                        chart_w,
                                        chart_h,
                                        &format!("▶ {}", series.label),
                                        "%",
                                        &cpu_vals,
                                        current,
                                        Some(100.0),
                                        color,
                                    );
                                }
                            });
                        }
                    });
            }

            // ── 5th panel: application process tree ───────────────────────
            ui.separator();
            ui.horizontal(|ui| {
                ui.strong("Process tree");
                ui.weak("(this app + any child processes)");
            });
            let tree = self.inspector.process_tree();
            egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if tree.is_empty() {
                            ui.weak(if form_running {
                                "sampling…"
                            } else {
                                "run a form to sample the process tree"
                            });
                        }
                        for (depth, pid, name, cpu, rss) in &tree {
                            let indent = "    ".repeat(*depth);
                            let branch = if *depth == 0 { "● " } else { "└ " };
                            let mb = *rss as f64 / (1024.0 * 1024.0);
                            ui.monospace(format!(
                                "{indent}{branch}{name}  ·  pid {pid}  ·  CPU {cpu:.0}%  ·  RSS {mb:.0} MB"
                            ));
                        }
                    });
        }
    }

    /// Draw one Grafana-style sparkline in a fixed box: filled area under the
    /// line, current value + unit, and the window's peak.
    #[allow(clippy::too_many_arguments)]
    fn sparkline(
        ui: &mut egui::Ui,
        w: f32,
        h: f32,
        title: &str,
        unit: &str,
        values: &[f32],
        current: f32,
        max_hint: Option<f32>,
        color: egui::Color32,
    ) {
        let (rect, _resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 4.0, egui::Color32::from_rgb(18, 20, 30));
        // Header line.
        let peak = values.iter().cloned().fold(0.0_f32, f32::max);
        p.text(
            rect.left_top() + egui::vec2(6.0, 4.0),
            egui::Align2::LEFT_TOP,
            title,
            egui::FontId::proportional(11.0),
            egui::Color32::from_rgb(170, 180, 200),
        );
        p.text(
            rect.right_top() + egui::vec2(-6.0, 4.0),
            egui::Align2::RIGHT_TOP,
            format!("{current:.1}{unit}"),
            egui::FontId::monospace(12.0),
            color,
        );
        // Plot area.
        let plot = egui::Rect::from_min_max(
            rect.left_top() + egui::vec2(6.0, 22.0),
            rect.right_bottom() - egui::vec2(6.0, 14.0),
        );
        let vmax = max_hint.unwrap_or(peak).max(peak).max(1.0);
        if values.len() >= 2 {
            let dx = plot.width() / (values.len() - 1) as f32;
            let y_of = |v: f32| plot.bottom() - (v / vmax).clamp(0.0, 1.0) * plot.height();
            let pts: Vec<egui::Pos2> = values
                .iter()
                .enumerate()
                .map(|(i, &v)| egui::pos2(plot.left() + i as f32 * dx, y_of(v)))
                .collect();
            // Line only (no filled area under the curve).
            p.add(egui::Shape::line(pts, egui::Stroke::new(1.5, color)));
        }
        // Peak label bottom-left.
        p.text(
            plot.left_bottom() + egui::vec2(0.0, 2.0),
            egui::Align2::LEFT_TOP,
            format!("peak {peak:.0}{unit}"),
            egui::FontId::proportional(9.0),
            egui::Color32::from_rgb(110, 120, 140),
        );
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

        // Apply glass visuals identical to the preview window (or the light
        // soft-UI visuals when the form's style is Neumorphic).
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
        // Run-form only: diagnose why groupbox cards not generated. Log effective ItemCount for any repeating GroupBox.
        for c in &controls {
            if matches!(c.control_type, cobolt_forms::ControlType::GroupBox) {
                let is_rep = c
                    .get_prop("IsRepeatingGroup")
                    .map(|v| v.as_bool())
                    .unwrap_or(false);
                let mut item_cnt = c.get_prop("ItemCount").map(|v| v.as_i64()).unwrap_or(0);
                // Check if live state overrode it
                if let Some(live) = states_snap.get(&c.id) {
                    let lv = live.get("ItemCount");
                    if !lv.is_empty() {
                        if let Ok(n) = lv.parse::<i64>() {
                            item_cnt = n;
                        }
                    }
                }
                if is_rep || item_cnt > 0 || c.id.to_ascii_lowercase().contains("groupbox") {
                    // debug during databind troubleshooting removed
                }
            }
        }
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
            run_id: self.form_runtimes[idx].run_id,
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
                // Capture *design-intent* layout adjustments (DataGrid column
                // widths via AdvancedGrid, row height) so the "Apply layout to
                // design" button can persist them as the control's new defaults.
                // Runtime data (populated Rows, Value, selection) is never
                // captured — only this whitelist.
                if is_design_intent_prop(key) {
                    rt.pending_design_props
                        .entry(id.clone())
                        .or_default()
                        .insert(key.clone(), val.clone());
                }
            }
            // A Timer keeps emitting `onTick` every frame the interval elapses,
            // regardless of whether the previous tick's handler has finished.
            // If the handler is slower than the interval those ticks would pile
            // up in the unbounded event queue and starve close/quit — freezing
            // the form (and the IDE, once a relaunch tried to join the thread).
            // Coalesce: drop a new tick while any event is still queued, exactly
            // like a WinForms timer skips ticks when the app is busy. User
            // events (clicks, edits, focus, quit) are never dropped.
            let busy = rt.pending_events() > 0;
            for ev in output.events {
                let is_tick = ev.event.eq_ignore_ascii_case("onTick");
                if is_tick && busy {
                    continue;
                }
                // For instanced members of repeating groups (ControlArray), the
                // drawn event id is "Group.Group-N.Member". Route the *event* to the
                // designed (base) member id so the generated handler is found, and
                // forward the correct instance_index so the handler receives it via
                // CONTROL-ARRAY-INDEX (property updates already used instance_index).
                let (dispatch_id, inst) = if ev.ctrl_id.contains('.') {
                    let base = ev
                        .ctrl_id
                        .rsplit('.')
                        .next()
                        .unwrap_or(&ev.ctrl_id)
                        .to_string();
                    // Format is "group.group-N.member" (the instanced id generated by
                    // expand_repeating_groups). Extract the number after the last '-'
                    // in the middle segment. This is more robust than simple nth.
                    let inst = {
                        let parts: Vec<&str> = ev.ctrl_id.split('.').collect();
                        if parts.len() >= 2 {
                            let mid = parts[1];
                            mid.rsplit('-')
                                .next()
                                .and_then(|s| s.parse::<usize>().ok())
                                .unwrap_or(0)
                        } else {
                            0
                        }
                    };
                    (base, inst)
                } else {
                    (ev.ctrl_id.clone(), 0)
                };
                rt.send_event(FormEvent::new(dispatch_id, ev.event).with_index(inst));
            }
        }

        // ── Floating "Apply layout to design" affordance ──────────────────────
        // When the user has interactively adjusted a DataGrid (column widths /
        // row height) while the form runs with real data, offer to persist those
        // as the control's new design defaults. Purely additive: if it is never
        // clicked, the running form behaves exactly as before.
        let pending_count: usize = self.form_runtimes[idx]
            .pending_design_props
            .values()
            .map(|m| m.len())
            .sum();
        if pending_count > 0 {
            let mut apply = false;
            egui::Area::new(egui::Id::new(("apply-layout-to-design", idx)))
                .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-14.0, 14.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        if ui
                            .button(format!("✓ Apply layout to design ({pending_count})"))
                            .on_hover_text(
                                "Write the adjusted DataGrid column widths / row height back \
                                 into the form as the control's new defaults, then Save (Ctrl+S).",
                            )
                            .clicked()
                        {
                            apply = true;
                        }
                    });
                });
            if apply {
                self.apply_runtime_layout_to_design(idx);
            }
        }

        // NOTE: do NOT self-request an unconditional repaint here. This runs
        // once per frame inside the running-form viewport, and an unconditional
        // `request_repaint()` asks for the next frame *immediately* — so the
        // window spins at the machine's max frame rate and pegs a whole core
        // even while the form sits idle (98% CPU). That defeats the reactive
        // scheduling in `update()` and the per-tick `request_repaint_after` in
        // the Timer/animation render arms.
        //
        // Frames are driven reactively instead:
        //   • the root `update()` schedules 16 ms while interpreter events are
        //     queued to drain, and a 200 ms heartbeat otherwise;
        //   • the Timer arm wakes exactly when the next tick is due;
        //   • animations and channel output schedule their own targeted repaints.
        // Between those, the form sleeps.
    }

    /// Persist the interactively-adjusted, whitelisted layout properties captured
    /// while the form at `idx` runs (DataGrid column widths / row height) back
    /// into the owning designer's form model, as the control's new defaults. The
    /// designer is marked dirty; the user Saves to write the `.cfrm`.
    fn apply_runtime_layout_to_design(&mut self, idx: usize) {
        if idx >= self.form_runtimes.len() {
            return;
        }
        let form_path = self.form_runtimes[idx].form_path.clone();
        let fname = self.form_runtimes[idx].form_name.clone();
        let pending = std::mem::take(&mut self.form_runtimes[idx].pending_design_props);
        if pending.is_empty() {
            return;
        }
        // Resolve the owning designer by path, then by form name.
        let pos = self
            .designers
            .iter()
            .position(|(p, _)| *p == form_path)
            .or_else(|| {
                self.designers
                    .iter()
                    .position(|(_, d)| d.form.name == fname)
            });
        let Some(pos) = pos else {
            self.output.push_status(
                "Apply layout: the form's designer is not open — reopen the form, run, and try again.",
            );
            return;
        };
        let designer = &mut self.designers[pos].1;
        let mut applied = 0usize;
        let mut controls = 0usize;
        for (ctrl_id, props) in &pending {
            if let Some(ctrl) = designer.form.find_control_mut(ctrl_id) {
                controls += 1;
                for (key, val) in props {
                    ctrl.set_prop(key.clone(), cobolt_forms::PropValue::String(val.clone()));
                    applied += 1;
                }
            }
        }
        if applied > 0 {
            designer.dirty = true;
            self.output.push_status(format!(
                "Applied {applied} layout adjustment(s) to {controls} control(s) in {fname} — Save (Ctrl+S) to keep."
            ));
        } else {
            self.output
                .push_status("Apply layout: no matching controls found in the form.");
        }
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
            // Slow heartbeat when these auxiliary viewports are open.
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
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
        // Indexed grids are mostly static; use a slow heartbeat instead of
        // unconditional repaint every frame.
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
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

    fn show_indexed_delete_confirm(&mut self, ctx: &Context, tr: &Tr) {
        let Some(rel) = self.pending_indexed_delete.clone() else {
            return;
        };
        let stem = Path::new(&rel)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&rel);

        let mut cancel = false;
        let mut confirm = false;

        // Resolve paths on disk
        let cidx_path = self.project_dir().map(|d| d.join(&rel));
        let def = cidx_path.as_ref().and_then(|p| load_indexed(p).ok());
        let data_path = def.as_ref().and_then(|d| self.resolve_assign_path(d));

        egui::Window::new("Confirm removal")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("Remove indexed file '{}' from the project?", stem));
                ui.add_space(8.0);

                // Option to delete configuration file (.cidx)
                if let Some(p) = &cidx_path {
                    let text = format!("Delete configuration file from disk\n  ({})", p.display());
                    ui.checkbox(&mut self.delete_cidx_file, text);
                    ui.add_space(4.0);
                }

                // Option to delete data file (.idx)
                if let Some(p) = &data_path {
                    if p.exists() {
                        let text = format!("Delete data file from disk\n  ({})", p.display());
                        ui.checkbox(&mut self.delete_data_file, text);
                    } else {
                        ui.add_enabled_ui(false, |ui| {
                            let text = format!(
                                "Delete data file from disk (file not found)\n  ({})",
                                p.display()
                            );
                            let mut dummy = false;
                            ui.checkbox(&mut dummy, text);
                        });
                    }
                    ui.add_space(8.0);
                }

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
            self.pending_indexed_delete = None;
            self.delete_cidx_file = false;
            self.delete_data_file = false;
        }
        if confirm {
            self.pending_indexed_delete = None;
            let del_cidx = self.delete_cidx_file;
            let del_data = self.delete_data_file;
            self.delete_cidx_file = false;
            self.delete_data_file = false;

            // Delete configuration file on disk
            if let Some(p) = &cidx_path {
                if del_cidx {
                    let _ = std::fs::remove_file(p);
                    // Also delete the copybook file in COPYBOOKS/<name>.fd.cpy
                    if let Some(d) = &def {
                        if let Some(cpy_path) = self.indexed_fd_copybook_path(&d.name) {
                            if cpy_path.exists() {
                                let _ = std::fs::remove_file(&cpy_path);
                            }
                        }
                    }
                }
            }

            // Delete data file on disk
            if let Some(p) = &data_path {
                if del_data && p.exists() {
                    let _ = std::fs::remove_file(p);
                }
            }

            self.do_remove_file_from_project(rel);
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
                // Exited external runs are reaped every frame in update(), so
                // presence in the list means the process is alive.
                let form_running = self
                    .form_runtimes
                    .iter()
                    .any(|rt| rt.form_path == form_path && rt.is_running())
                    || self
                        .external_runs
                        .iter()
                        .any(|run| run.form_path == form_path);

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
                        self.show_inspector,
                        self.debug_active,
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
                    DesignerToolbarAction::ToggleInspector => {
                        self.toggle_inspector();
                    }
                    DesignerToolbarAction::StopForm => {
                        let fp = self.designers[idx].0.clone();
                        self.external_runs.retain_mut(|run| {
                            if run.form_path == fp {
                                run.stop();
                                false
                            } else {
                                true
                            }
                        });
                        self.form_runtimes.retain_mut(|rt| {
                            if rt.form_path == fp {
                                rt.stop();
                                false
                            } else {
                                true
                            }
                        });
                        // If this form owned the remote debug session, close it.
                        if self.debug_external && self.debug_owner_form.as_ref() == Some(&fp) {
                            self.debug_active = false;
                            self.debug_external = false;
                            self.debug_owner_form = None;
                            self.debugger.reset();
                        }
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
                    DesignerToolbarAction::DebugForm => {
                        self.do_debug_form(idx);
                    }
                    DesignerToolbarAction::ReportBug => {
                        self.report_bug.open_for("Form Designer");
                    }
                    DesignerToolbarAction::None => {}
                }
            });

        // ── Properties panel (right) ──────────────────────────────────────────
        let sel_id = self.designers[idx].1.selected_ids.first().cloned();
        let indexed_files: Vec<String> = self
            .cobolt_project
            .as_ref()
            .map(|project| project.files.indexed.clone())
            .unwrap_or_default();

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
                props.show(ui, unsafe { &*form }, sel_ctrl, &indexed_files, tr)
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
        if let Some(binding) = inspector_action.create_data_binding {
            let d = &mut self.designers[idx].1;
            let b = binding.clone();
            apply_data_binding_to_form(&mut d.form, binding);
            seed_control_array_binding_preview_values(d, &b);
            d.dirty = true;
        }
        if let Some((old, new)) = inspector_action.rename_control {
            self.designers[idx].1.rename_control(&old, &new);
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
        let llm_cfg = self.llm.clone();
        // Project directory (holds the `agentic_ai/` prompt + skills) for the
        // event-editor assistant. Cloned so the closure doesn't borrow `self`.
        let proj_root = self
            .project_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf());
        let designer_result = egui::CentralPanel::default()
            .show(ctx, |ui| {
                self.designers[idx].1.show(
                    ui,
                    &mut self.clipboard,
                    &user_controls,
                    &llm_cfg,
                    proj_root.as_deref(),
                )
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
        self.designers[idx]
            .1
            .show_cobol_structure_window(ctx, tr, &llm_cfg, proj_root.as_deref());

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

fn apply_data_binding_to_form(form: &mut Form, binding: DataBindingDef) {
    apply_data_binding_target_properties(form, &binding);
    form.data_bindings
        .retain(|existing| !same_binding_target(&existing.target, &binding.target));
    form.data_bindings.push(binding);
}

pub(crate) fn refresh_data_binding_target_properties(form: &mut Form) {
    let bindings = form.data_bindings.clone();
    tracing::debug!(target: "databinding", "refresh_data_binding_target_properties count={}", bindings.len());
    for binding in &bindings {
        apply_data_binding_target_properties(form, binding);
    }
}

fn same_binding_target(left: &BindingTargetDescriptor, right: &BindingTargetDescriptor) -> bool {
    left.primary_control_id()
        .eq_ignore_ascii_case(right.primary_control_id())
}

fn apply_data_binding_target_properties(form: &mut Form, binding: &DataBindingDef) {
    // Designer / RAD path. Noisy runtime diagnostics removed (see run-form path in interpreter + form_runtime).
    tracing::debug!(target: "databinding", "APPLY binding id={} source={:?} target={:?} mappings={:?}",
        binding.id, binding.source, binding.target, binding.mappings);

    if let BindingTargetDescriptor::DataGrid { control_id } = &binding.target {
        let fields = binding.source.fields();
        let columns = fields
            .iter()
            .map(|field| {
                let display_name = field.display_name.trim();
                let name = if display_name.is_empty() {
                    field.name.as_str()
                } else {
                    display_name
                };
                format!("{name}:{}", datagrid_column_type(&field.data_type))
            })
            .collect::<Vec<_>>()
            .join("\n");
        let data_source = binding_source_basic_label(&binding.source);
        let preview_rows = data_binding_preview_rows(form, binding);
        tracing::debug!(target: "databinding", "[DataGrid] control={} fields={:?}",
            control_id, fields.iter().map(|f| &f.name).collect::<Vec<_>>());
        if let Some(control) = form.find_control_mut(control_id) {
            let advanced_grid = datagrid_advanced_for_binding(control, binding);
            control.set_prop("Columns", PropValue::String(columns));
            control.set_prop("DataSource", PropValue::String(data_source));
            control.set_prop("Rows", PropValue::String(preview_rows));
            if let Some(adv) = advanced_grid {
                control.set_prop(DATAGRID_ADVANCED_PROP, PropValue::String(adv));
            }
        }
    }
    if let BindingTargetDescriptor::ControlArray { array_id, .. } = &binding.target {
        let data_source = binding_source_basic_label(&binding.source);
        let fields = binding.source.fields();
        let rows_str = data_binding_preview_rows(form, binding);
        let occurs_n = get_cobol_table_occurs_count(form, &binding.source);
        let n = if let Some(sz) = occurs_n {
            sz as i64
        } else if rows_str.trim().is_empty() {
            3i64
        } else {
            rows_str.lines().count() as i64
        };
        tracing::debug!(target: "databinding", "[ControlArray] array_id={} n={} source_fields={:?}",
            array_id, n, fields.iter().map(|f| &f.name).collect::<Vec<_>>());
        fn find_group_mut<'a>(
            ctrl: &'a mut cobolt_forms::Control,
            array_id: &str,
        ) -> Option<&'a mut cobolt_forms::Control> {
            if matches!(ctrl.control_type, ControlType::GroupBox)
                && ctrl.explicit_control_array_id().as_deref() == Some(array_id)
            {
                return Some(ctrl);
            }
            for child in &mut ctrl.children {
                if let Some(res) = find_group_mut(child, array_id) {
                    return Some(res);
                }
            }
            None
        }
        let mut group = None;
        for c in &mut form.controls {
            if let Some(res) = find_group_mut(c, array_id.as_str()) {
                group = Some(res);
                break;
            }
        }
        if let Some(group) = group {
            group.set_prop("DataSource", PropValue::String(data_source.clone()));
            group.set_prop("ItemCount", PropValue::Int(n));
            group.set_prop("PreviewItemCount", PropValue::Int(n));
        }
    }
}

pub(crate) fn seed_control_array_binding_preview_values(
    d: &mut DesignerPanel,
    binding: &DataBindingDef,
) {
    let BindingTargetDescriptor::ControlArray {
        array_id,
        member_control_ids: _,
    } = &binding.target
    else {
        return;
    };
    tracing::debug!(target: "databinding", "[SEED] ControlArray array_id={} mappings={}", array_id, binding.mappings.len());
    let fields = binding.source.fields();
    if fields.is_empty() {
        return;
    }
    let move_rows = cobol_table_move_rows(&d.form, &binding.source, fields);
    let value_rows_opt = if move_rows.is_none() {
        cobol_table_value_rows(&d.form, &binding.source, fields)
    } else {
        None
    };
    let rows = move_rows
        .clone()
        .or_else(|| value_rows_opt.clone())
        .unwrap_or_else(|| Vec::new());
    let n = if let Some(sz) = get_cobol_table_occurs_count(&d.form, &binding.source) {
        sz
    } else {
        rows.len().clamp(1, 20)
    };
    fn find_group_ref<'a>(
        ctrl: &'a cobolt_forms::Control,
        array_id: &str,
    ) -> Option<&'a cobolt_forms::Control> {
        if matches!(ctrl.control_type, ControlType::GroupBox)
            && ctrl.explicit_control_array_id().as_deref() == Some(array_id)
        {
            return Some(ctrl);
        }
        for child in &ctrl.children {
            if let Some(res) = find_group_ref(child, array_id) {
                return Some(res);
            }
        }
        None
    }
    let mut group_ctrl_id = None;
    for c in &d.form.controls {
        if let Some(res) = find_group_ref(c, array_id.as_str()) {
            group_ctrl_id = Some(res.id.clone());
            break;
        }
    }
    let Some(group_ctrl_id) = group_ctrl_id else {
        return;
    };
    if let Some(g) = d.form.find_control_mut(&group_ctrl_id) {
        g.set_prop("ItemCount", PropValue::Int(n as i64));
        g.set_prop("PreviewItemCount", PropValue::Int(n as i64));
    }
    let num_data = rows.len();
    for i in 0..n {
        let inst = i + 1;
        // Cycle available data rows so that *all* cards (including ones revealed
        // by scrolling) get databound values in preview. Previously only the
        // first num_data got real data; scrolled/"other" got fakes or defaults.
        let row = if num_data > 0 {
            &rows[i % num_data]
        } else {
            &fields
                .iter()
                .map(|f| format!("{}#{}", f.name, inst))
                .collect::<Vec<String>>()
        };
        for mapping in &binding.mappings {
            if let BindingTargetPath::ControlProperty {
                control_id: member_id,
                property_name: _,
                ..
            } = &mapping.target
            {
                let src_field = &mapping.source_field;
                if let Some(fidx) = fields.iter().position(|f| &f.name == src_field) {
                    let val = if fidx < row.len() {
                        row[fidx].clone()
                    } else {
                        format!("{}#{}", src_field, inst)
                    };
                    let inst_id =
                        cobolt_forms::render::member_instance_id(&group_ctrl_id, member_id, inst);
                    d.preview_state.insert(inst_id, val);
                }
            }
        }
    }
}

fn datagrid_advanced_for_binding(
    control: &cobolt_forms::Control,
    binding: &DataBindingDef,
) -> Option<String> {
    let fields = binding.source.fields();
    if fields.is_empty() {
        return None;
    }
    let mut advanced = DataGridAdvanced::from_control(control);
    let existing_columns = advanced.columns.clone();
    let mut used = vec![false; fields.len()];
    let mut next_columns = Vec::new();

    for existing in &existing_columns {
        if let Some((index, field)) = fields
            .iter()
            .enumerate()
            .find(|(index, field)| !used[*index] && column_matches_field(existing, field))
        {
            used[index] = true;
            next_columns.push(merge_datagrid_binding_column(
                existing.clone(),
                binding,
                field,
            ));
        }
    }

    for (index, field) in fields.iter().enumerate() {
        if !used[index] {
            next_columns.push(merge_datagrid_binding_column(
                DataGridColumn::default(),
                binding,
                field,
            ));
        }
    }

    advanced.columns = next_columns;
    advanced.to_json().ok()
}

fn merge_datagrid_binding_column(
    mut column: DataGridColumn,
    binding: &DataBindingDef,
    field: &BindingField,
) -> DataGridColumn {
    column.id = datagrid_binding_column_id(binding, field);
    column.title = if field.display_name.trim().is_empty() {
        field.name.clone()
    } else {
        field.display_name.clone()
    };
    column.source_name = field.name.clone();
    column.value_type = datagrid_column_type(&field.data_type).to_owned();
    // Seed the COBOL mask from the bound field's PICTURE only when the column has
    // none yet. A mask typed in the DataGrid column editor is a deliberate
    // override that must survive save/run binding refreshes — otherwise the user
    // "cannot change" the mask because every refresh resets it (and the cell value
    // never passes through their mask). Clearing the field re-seeds from the bind.
    if column.cobol_mask.trim().is_empty() {
        column.cobol_mask = field.cobol_mask.clone();
    }
    column.edit_control = if field.edit_control.trim().is_empty() {
        "Textbox".to_owned()
    } else {
        field.edit_control.clone()
    };
    if column.width <= 0.0 {
        column.width = DataGridColumn::default().width;
    }
    column
}

fn datagrid_binding_column_id(binding: &DataBindingDef, field: &BindingField) -> String {
    binding
        .mappings
        .iter()
        .find_map(|mapping| {
            if !mapping.source_field.eq_ignore_ascii_case(&field.name) {
                return None;
            }
            match &mapping.target {
                BindingTargetPath::GridColumn { column_id, .. } if !column_id.trim().is_empty() => {
                    Some(column_id.clone())
                }
                _ => None,
            }
        })
        .unwrap_or_else(|| normalize_datagrid_column_id(&field.name))
}

fn column_matches_field(column: &DataGridColumn, field: &BindingField) -> bool {
    column.source_name.eq_ignore_ascii_case(&field.name)
        || column.id.eq_ignore_ascii_case(&field.name)
        || column.title.eq_ignore_ascii_case(&field.name)
        || (!field.display_name.trim().is_empty()
            && column.title.eq_ignore_ascii_case(&field.display_name))
}

fn normalize_datagrid_column_id(name: &str) -> String {
    let normalized: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    normalized.trim_matches('_').to_owned()
}

fn data_binding_preview_rows(form: &Form, binding: &DataBindingDef) -> String {
    let fields = binding.source.fields();
    if fields.is_empty() {
        return String::new();
    }
    let move_rows = cobol_table_move_rows(form, &binding.source, fields);
    let value_rows = if move_rows.is_none() {
        cobol_table_value_rows(form, &binding.source, fields)
    } else {
        None
    };
    let rows = move_rows
        .clone()
        .or(value_rows.clone())
        .unwrap_or_else(|| fallback_binding_preview_rows(&binding.source, fields));
    rows.into_iter()
        .filter(|row| !row.is_empty())
        .map(|row| row.join("\t"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn cobol_table_move_rows(
    form: &Form,
    source: &BindingSourceDescriptor,
    fields: &[BindingField],
) -> Option<Vec<Vec<String>>> {
    if !matches!(source, BindingSourceDescriptor::CobolTable { .. }) {
        return None;
    }
    let field_names = fields
        .iter()
        .map(|field| field.name.to_ascii_uppercase())
        .collect::<std::collections::HashSet<_>>();
    let mut row_values =
        std::collections::BTreeMap::<usize, std::collections::HashMap<String, String>>::new();
    for code in form_binding_code_blocks(form) {
        collect_move_rows_from_code(&code, &field_names, &mut row_values);
    }
    if row_values.is_empty() {
        return None;
    }
    let rows = row_values
        .into_iter()
        .map(|(_, values)| {
            fields
                .iter()
                .map(|field| {
                    values
                        .get(&field.name.to_ascii_uppercase())
                        .cloned()
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
        })
        .filter(|row| row.iter().any(|value| !value.trim().is_empty()))
        .collect::<Vec<_>>();
    if rows.is_empty() {
        None
    } else {
        Some(rows)
    }
}

fn get_cobol_table_occurs_count(form: &Form, source: &BindingSourceDescriptor) -> Option<usize> {
    let BindingSourceDescriptor::CobolTable {
        table_name,
        occurs_item,
        ..
    } = source
    else {
        return None;
    };
    let ws = form.user_ws_source.trim();
    if ws.is_empty() {
        return None;
    }
    let source_code = format!(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. GET-OCCURS.\n\
         DATA DIVISION.\n\
         WORKING-STORAGE SECTION.\n\
         {ws}\n\
         PROCEDURE DIVISION.\n\
         MAIN.\n\
             STOP RUN.\n"
    );
    let result = parse(tokenize(&source_code, SourceFormat::Free));
    let program = result.program?;
    let data = program.data?;
    for section in &data.sections {
        if let DataSection::WorkingStorage(items) = section {
            for root in items {
                if root.level == 1
                    && root
                        .name
                        .as_deref()
                        .map_or(false, |name| name.eq_ignore_ascii_case(table_name))
                {
                    let item = if root
                        .name
                        .as_deref()
                        .map_or(false, |name| name.eq_ignore_ascii_case(occurs_item))
                    {
                        Some(root)
                    } else {
                        find_data_decl_by_name(root, occurs_item)
                    };
                    if let Some(item) = item {
                        if let Some(occ) = &item.occurs {
                            return Some(occ.max as usize);
                        }
                    }
                }
            }
        }
    }
    None
}

fn form_binding_code_blocks(form: &Form) -> Vec<String> {
    let mut blocks = form
        .form_events
        .iter()
        .map(|event| event.code.clone())
        .collect::<Vec<_>>();
    for control in &form.controls {
        collect_control_event_code(control, &mut blocks);
    }
    blocks.extend(
        form.user_procedures
            .iter()
            .map(|procedure| procedure.code.clone()),
    );
    blocks
}

fn collect_control_event_code(control: &cobolt_forms::Control, blocks: &mut Vec<String>) {
    blocks.extend(control.events.iter().map(|event| event.code.clone()));
    for child in &control.children {
        collect_control_event_code(child, blocks);
    }
}

fn collect_move_rows_from_code(
    code: &str,
    field_names: &std::collections::HashSet<String>,
    row_values: &mut std::collections::BTreeMap<usize, std::collections::HashMap<String, String>>,
) {
    for statement in split_cobol_statements(code) {
        if let Some((value, field, row_index)) = parse_indexed_move_statement(statement) {
            let field = field.to_ascii_uppercase();
            if field_names.contains(&field) {
                row_values
                    .entry(row_index)
                    .or_default()
                    .insert(field, value);
            }
        }
    }
}

fn split_cobol_statements(code: &str) -> Vec<&str> {
    let mut statements = Vec::new();
    let mut in_quote: Option<char> = None;
    let mut start = 0;
    let mut index = 0;
    while index < code.len() {
        let ch = code[index..].chars().next().unwrap_or_default();
        if matches!(ch, '"' | '\'') {
            if in_quote == Some(ch) {
                in_quote = None;
            } else if in_quote.is_none() {
                in_quote = Some(ch);
            }
        }
        if ch == '.' && in_quote.is_none() && !is_decimal_point(code, index) {
            let statement = code[start..index].trim();
            if !statement.is_empty() {
                statements.push(statement);
            }
            start = index + ch.len_utf8();
        }
        index += ch.len_utf8();
    }
    let statement = code[start..].trim();
    if !statement.is_empty() {
        statements.push(statement);
    }
    statements
}

fn is_decimal_point(text: &str, index: usize) -> bool {
    let before = text[..index].chars().next_back();
    let after = text[index + 1..].chars().next();
    before.map(|ch| ch.is_ascii_digit()).unwrap_or(false)
        && after.map(|ch| ch.is_ascii_digit()).unwrap_or(false)
}

fn parse_indexed_move_statement(statement: &str) -> Option<(String, String, usize)> {
    let statement = statement.trim();
    if !starts_with_keyword(statement, "MOVE") {
        return None;
    }
    let after_move = statement.get(4..)?.trim_start();
    let to_pos = find_keyword_outside_quotes(after_move, "TO")?;
    let raw_value = after_move[..to_pos].trim();
    let target = after_move[to_pos + 2..].trim();
    let open = target.find('(')?;
    let close = target[open + 1..].find(')')? + open + 1;
    let field = target[..open].trim();
    let index = target[open + 1..close].trim().parse::<usize>().ok()?;
    if field.is_empty() || index == 0 {
        return None;
    }
    Some((clean_move_value(raw_value), field.to_owned(), index))
}

fn starts_with_keyword(text: &str, keyword: &str) -> bool {
    let Some(prefix) = text.get(..keyword.len()) else {
        return false;
    };
    prefix.eq_ignore_ascii_case(keyword)
        && text
            .get(keyword.len()..)
            .and_then(|rest| rest.chars().next())
            .map(|ch| ch.is_whitespace())
            .unwrap_or(false)
}

fn find_keyword_outside_quotes(text: &str, keyword: &str) -> Option<usize> {
    let mut in_quote: Option<char> = None;
    let mut index = 0;
    while index < text.len() {
        let ch = text[index..].chars().next()?;
        if matches!(ch, '"' | '\'') {
            if in_quote == Some(ch) {
                in_quote = None;
            } else if in_quote.is_none() {
                in_quote = Some(ch);
            }
        }
        if in_quote.is_none()
            && text[index..].len() >= keyword.len()
            && text[index..index + keyword.len()].eq_ignore_ascii_case(keyword)
        {
            let before_ok = index == 0
                || text[..index]
                    .chars()
                    .next_back()
                    .map(|ch| ch.is_whitespace())
                    .unwrap_or(true);
            let after_ok = text[index + keyword.len()..]
                .chars()
                .next()
                .map(|ch| ch.is_whitespace())
                .unwrap_or(true);
            if before_ok && after_ok {
                return Some(index);
            }
        }
        index += ch.len_utf8();
    }
    None
}

fn clean_move_value(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2 {
        let first = value.chars().next().unwrap_or_default();
        let last = value.chars().next_back().unwrap_or_default();
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            return value[1..value.len() - 1].to_owned();
        }
    }
    value.to_owned()
}

fn cobol_table_value_rows(
    form: &Form,
    source: &BindingSourceDescriptor,
    fields: &[BindingField],
) -> Option<Vec<Vec<String>>> {
    let BindingSourceDescriptor::CobolTable {
        table_name,
        occurs_item,
        ..
    } = source
    else {
        return None;
    };
    let ws = form.user_ws_source.trim();
    if ws.is_empty() {
        return None;
    }
    let source = format!(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. BINDING-ROWS.\n\
         DATA DIVISION.\n\
         WORKING-STORAGE SECTION.\n\
         {ws}\n\
         PROCEDURE DIVISION.\n\
         MAIN.\n\
             STOP RUN.\n"
    );
    let result = parse(tokenize(&source, SourceFormat::Free));
    let program = result.program?;
    let data = program.data?;
    let mut values = Vec::<(String, String)>::new();
    for section in &data.sections {
        if let DataSection::WorkingStorage(items) = section {
            for root in items {
                if root.level == 1
                    && root
                        .name
                        .as_deref()
                        .map(|name| name.eq_ignore_ascii_case(table_name))
                        .unwrap_or(false)
                {
                    let item = if root
                        .name
                        .as_deref()
                        .map(|name| name.eq_ignore_ascii_case(occurs_item))
                        .unwrap_or(false)
                    {
                        Some(root)
                    } else {
                        find_data_decl_by_name(root, occurs_item)
                    };
                    if let Some(item) = item {
                        collect_decl_values(item, &mut values);
                    }
                }
            }
        }
    }
    if values.is_empty() {
        return None;
    }
    let row = fields
        .iter()
        .map(|field| {
            values
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&field.name))
                .map(|(_, value)| value.clone())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let has_data = row.iter().any(|value| !value.trim().is_empty());
    if has_data {
        Some(vec![row])
    } else {
        None
    }
}

fn find_data_decl_by_name<'a>(decl: &'a DataDecl, name: &str) -> Option<&'a DataDecl> {
    if decl
        .name
        .as_deref()
        .map(|candidate| candidate.eq_ignore_ascii_case(name))
        .unwrap_or(false)
    {
        return Some(decl);
    }
    decl.children
        .iter()
        .find_map(|child| find_data_decl_by_name(child, name))
}

fn collect_decl_values(decl: &DataDecl, values: &mut Vec<(String, String)>) {
    if let (Some(name), Some(value)) = (&decl.name, &decl.value) {
        values.push((name.clone(), literal_preview_value(value)));
    }
    for child in &decl.children {
        collect_decl_values(child, values);
    }
}

fn literal_preview_value(value: &Literal) -> String {
    match value {
        Literal::String(value) => value.clone(),
        Literal::Integer(value) => value.to_string(),
        Literal::Float(value) => trim_float_preview(*value),
        Literal::Decimal(mantissa, scale) => decimal_preview_value(*mantissa, *scale),
        Literal::Figurative(figurative) => figurative_preview_value(figurative),
    }
}

fn trim_float_preview(value: f64) -> String {
    let mut text = value.to_string();
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}

fn decimal_preview_value(mantissa: i128, scale: u8) -> String {
    if scale == 0 {
        return mantissa.to_string();
    }
    let negative = mantissa < 0;
    let digits = mantissa.abs().to_string();
    let scale = scale as usize;
    let padded = if digits.len() <= scale {
        format!("{:0>width$}", digits, width = scale + 1)
    } else {
        digits
    };
    let split = padded.len() - scale;
    let sign = if negative { "-" } else { "" };
    format!("{sign}{}.{}", &padded[..split], &padded[split..])
}

fn figurative_preview_value(value: &FigurativeConstant) -> String {
    match value {
        FigurativeConstant::Zero => "0".to_owned(),
        FigurativeConstant::Space => String::new(),
        FigurativeConstant::HighValue => "HIGH-VALUE".to_owned(),
        FigurativeConstant::LowValue => "LOW-VALUE".to_owned(),
        FigurativeConstant::Quote => "\"".to_owned(),
        FigurativeConstant::Null => String::new(),
        FigurativeConstant::All(inner) => literal_preview_value(inner),
    }
}

fn fallback_binding_preview_rows(
    source: &BindingSourceDescriptor,
    fields: &[BindingField],
) -> Vec<Vec<String>> {
    let row_count = match source {
        BindingSourceDescriptor::RestApi { .. } => 2,
        BindingSourceDescriptor::CobolTable { .. } => 3,
        BindingSourceDescriptor::IndexedFile { .. } | BindingSourceDescriptor::Sql { .. } => 2,
        BindingSourceDescriptor::AgentAi { .. } => 1,
    };
    (0..row_count)
        .map(|row_index| {
            fields
                .iter()
                .map(|field| fallback_field_value(field, row_index))
                .collect()
        })
        .collect()
}

fn fallback_field_value(field: &BindingField, row_index: usize) -> String {
    let ordinal = row_index + 1;
    let name = field.name.to_ascii_uppercase();
    match field.data_type {
        BindingDataType::Integer => {
            if name.contains("ID") {
                format!("{ordinal}")
            } else {
                format!("{}", ordinal * 10)
            }
        }
        BindingDataType::Decimal => format!("{}.00", ordinal * 100),
        BindingDataType::Boolean => {
            if row_index % 2 == 0 {
                "Y".to_owned()
            } else {
                "N".to_owned()
            }
        }
        BindingDataType::Date => format!("2026-06-{:02}", ordinal),
        BindingDataType::DateTime => format!("2026-06-{:02} 09:00:00", ordinal),
        BindingDataType::Text | BindingDataType::Json | BindingDataType::Unknown => {
            fallback_text_field_value(field, ordinal)
        }
    }
}

fn fallback_text_field_value(field: &BindingField, ordinal: usize) -> String {
    let label = if field.display_name.trim().is_empty() {
        field.name.as_str()
    } else {
        field.display_name.as_str()
    };
    format!("{label} {ordinal}")
}

fn datagrid_column_type(data_type: &BindingDataType) -> &'static str {
    match data_type {
        BindingDataType::Integer | BindingDataType::Decimal => "number",
        BindingDataType::Date | BindingDataType::DateTime => "datetime",
        _ => "string",
    }
}

fn binding_source_basic_label(source: &BindingSourceDescriptor) -> String {
    match source {
        BindingSourceDescriptor::IndexedFile {
            definition_path,
            record_name,
            ..
        } => {
            if record_name.trim().is_empty() {
                definition_path.clone()
            } else {
                format!("{definition_path} / {record_name}")
            }
        }
        BindingSourceDescriptor::Sql {
            source_control_id,
            result_set_name,
            ..
        } => {
            if result_set_name.trim().is_empty() {
                source_control_id.clone()
            } else {
                format!("{source_control_id} / {result_set_name}")
            }
        }
        BindingSourceDescriptor::CobolTable {
            table_name,
            occurs_item,
            ..
        } => {
            if occurs_item.trim().is_empty() {
                table_name.clone()
            } else {
                format!("{table_name} / {occurs_item}")
            }
        }
        BindingSourceDescriptor::RestApi { endpoint_name, .. } => endpoint_name.clone(),
        BindingSourceDescriptor::AgentAi { output_name, .. } => output_name.clone(),
    }
}

fn decode_icon_data(bytes: &[u8]) -> Option<egui::IconData> {
    let img = image::load_from_memory(bytes)
        .ok()?
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    })
}

#[cfg(test)]
mod manifest_name_tests {
    use super::*;
    use cobolt_forms::{
        BindingDataType, BindingField, BindingSourceDescriptor, BindingTargetDescriptor, Control,
        ControlType, EventBinding, FieldMapping,
    };

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

    #[test]
    fn form_cobol_id_normalization_is_case_insensitive_and_trimmed() {
        assert_eq!(normalize_form_cobol_id(" main-form "), "MAIN-FORM");
        assert_eq!(
            normalize_form_cobol_id("CustomerEntry"),
            normalize_form_cobol_id("customerentry")
        );
    }

    #[test]
    fn applying_grid_data_binding_updates_datagrid_basic_properties() {
        let mut form = Form::new("CustomerForm", "CustomerForm", 800, 600);
        form.add_control(Control::new("GRID-1", ControlType::DataGrid, 0, 0));
        let fields = vec![
            {
                let mut field = BindingField::new("CUSTOMER-ID", BindingDataType::Integer);
                field.display_name = "Customer ID".to_owned();
                field.key = true;
                field
            },
            {
                let mut field = BindingField::new("CUSTOMER-NAME", BindingDataType::Text);
                field.display_name = "Customer Name".to_owned();
                field
            },
        ];
        let binding = DataBindingDef::new(
            "BIND-GRID",
            "Customers",
            BindingSourceDescriptor::CobolTable {
                table_name: "WS-CUSTOMER-TABLE".to_owned(),
                occurs_item: "WS-CUSTOMER-ROW".to_owned(),
                fields,
                key_fields: vec!["CUSTOMER-ID".to_owned()],
                writable: true,
            },
            BindingTargetDescriptor::DataGrid {
                control_id: "GRID-1".to_owned(),
            },
        );

        apply_data_binding_to_form(&mut form, binding);

        let grid = form.find_control("GRID-1").expect("grid should exist");
        assert_eq!(
            grid.get_prop("Columns").map(PropValue::as_str),
            Some("Customer ID:number\nCustomer Name:string")
        );
        assert_eq!(
            grid.get_prop("DataSource").map(PropValue::as_str),
            Some("WS-CUSTOMER-TABLE / WS-CUSTOMER-ROW")
        );
        assert_eq!(
            grid.get_prop("Rows").map(PropValue::as_str),
            Some("1\tCustomer Name 1\n2\tCustomer Name 2\n3\tCustomer Name 3")
        );
        assert_eq!(form.data_bindings.len(), 1);
    }

    #[test]
    fn applying_grid_data_binding_uses_cobol_table_initial_values_when_available() {
        let mut form = Form::new("ActorForm", "ActorForm", 800, 600);
        form.user_ws_source = "\
01 WS-ACTOR-TABLE GLOBAL.
   05 WS-ACTOR-ROW OCCURS 10 TIMES.
      10 ACTOR-ID      PIC 9(06) VALUE 42.
      10 ACTOR-CAPTION PIC X(20) VALUE \"Lead Actor\".
      10 ACTOR-SALARY  PIC S9(9)V99 VALUE 1250.75.
"
        .to_owned();
        form.add_control(Control::new("GRID-1", ControlType::DataGrid, 0, 0));
        let fields = vec![
            BindingField::new("ACTOR-ID", BindingDataType::Integer),
            BindingField::new("ACTOR-CAPTION", BindingDataType::Text),
            BindingField::new("ACTOR-SALARY", BindingDataType::Decimal),
        ];
        let binding = DataBindingDef::new(
            "BIND-ACTOR",
            "Actors",
            BindingSourceDescriptor::CobolTable {
                table_name: "WS-ACTOR-TABLE".to_owned(),
                occurs_item: "WS-ACTOR-ROW".to_owned(),
                fields,
                key_fields: vec!["ACTOR-ID".to_owned()],
                writable: true,
            },
            BindingTargetDescriptor::DataGrid {
                control_id: "GRID-1".to_owned(),
            },
        );

        apply_data_binding_to_form(&mut form, binding);

        let grid = form.find_control("GRID-1").expect("grid should exist");
        assert_eq!(
            grid.get_prop("Rows").map(PropValue::as_str),
            Some("42\tLead Actor\t1250.75")
        );
    }

    #[test]
    fn datagrid_binding_metadata_preserves_advanced_column_identity() {
        let mut form = Form::new("ActorForm", "ActorForm", 800, 600);
        let mut grid = Control::new("ActorGrid", ControlType::DataGrid, 0, 0);
        let mut advanced = DataGridAdvanced::default();
        advanced.columns.push(DataGridColumn {
            id: "ACTOR_CAPTION".into(),
            title: "Actor Caption".into(),
            source_name: "ACTOR-CAPTION".into(),
            width: 240.0,
            background_color: "#112233".into(),
            ..DataGridColumn::default()
        });
        advanced.columns.push(DataGridColumn {
            id: "ACTOR_ID".into(),
            title: "Actor Id".into(),
            source_name: "ACTOR-ID".into(),
            width: 90.0,
            ..DataGridColumn::default()
        });
        grid.set_prop(
            DATAGRID_ADVANCED_PROP,
            PropValue::String(
                advanced
                    .to_json()
                    .expect("advanced metadata should serialize"),
            ),
        );
        form.add_control(grid);

        let mut actor_id = BindingField::new("ACTOR-ID", BindingDataType::Integer);
        actor_id.display_name = "Actor Id".to_owned();
        let mut caption = BindingField::new("ACTOR-CAPTION", BindingDataType::Text);
        caption.display_name = "Actor Caption".to_owned();
        let mut salary = BindingField::new("ACTOR-SALARY", BindingDataType::Decimal);
        salary.display_name = "Actor Salary".to_owned();
        let binding = DataBindingDef::new(
            "BIND-ACTORS",
            "Actors",
            BindingSourceDescriptor::CobolTable {
                table_name: "WS-ACTOR-TABLE".to_owned(),
                occurs_item: "WS-ACTOR-ROW".to_owned(),
                fields: vec![actor_id, caption, salary],
                key_fields: vec!["ACTOR-ID".to_owned()],
                writable: true,
            },
            BindingTargetDescriptor::DataGrid {
                control_id: "ActorGrid".to_owned(),
            },
        )
        .with_mappings(vec![
            FieldMapping::new(
                "ACTOR-ID",
                BindingTargetPath::GridColumn {
                    control_id: "ActorGrid".into(),
                    column_id: "ACTOR_ID".into(),
                },
            ),
            FieldMapping::new(
                "ACTOR-CAPTION",
                BindingTargetPath::GridColumn {
                    control_id: "ActorGrid".into(),
                    column_id: "ACTOR_CAPTION".into(),
                },
            ),
            FieldMapping::new(
                "ACTOR-SALARY",
                BindingTargetPath::GridColumn {
                    control_id: "ActorGrid".into(),
                    column_id: "ACTOR_SALARY".into(),
                },
            ),
        ]);

        apply_data_binding_to_form(&mut form, binding);

        let grid = form.find_control("ActorGrid").expect("grid should exist");
        let parsed = DataGridAdvanced::from_control(grid);
        assert_eq!(parsed.columns.len(), 3);
        assert_eq!(parsed.columns[0].source_name, "ACTOR-CAPTION");
        assert_eq!(parsed.columns[0].width, 240.0);
        assert_eq!(parsed.columns[0].background_color, "#112233");
        assert_eq!(parsed.columns[1].source_name, "ACTOR-ID");
        assert_eq!(parsed.columns[1].width, 90.0);
        assert_eq!(parsed.columns[2].id, "ACTOR_SALARY");
        assert_eq!(parsed.columns[2].source_name, "ACTOR-SALARY");
        assert_eq!(
            grid.get_prop("Columns").map(PropValue::as_str),
            Some("Actor Id:number\nActor Caption:string\nActor Salary:number")
        );
    }

    #[test]
    fn datagrid_binding_refresh_preserves_user_cobol_mask_override() {
        let mut form = Form::new("ActorForm", "ActorForm", 800, 600);
        let mut grid = Control::new("ActorGrid", ControlType::DataGrid, 0, 0);
        // The user has already typed a custom mask on the salary column.
        let mut advanced = DataGridAdvanced::default();
        advanced.columns.push(DataGridColumn {
            id: "ACTOR_SALARY".into(),
            title: "Actor Salary".into(),
            source_name: "ACTOR-SALARY".into(),
            cobol_mask: "ZZ9.99-".into(),
            ..DataGridColumn::default()
        });
        grid.set_prop(
            DATAGRID_ADVANCED_PROP,
            PropValue::String(advanced.to_json().expect("advanced serializes")),
        );
        form.add_control(grid);

        // Salary field carries a different PIC; the id field has no column yet.
        let mut actor_id = BindingField::new("ACTOR-ID", BindingDataType::Integer);
        actor_id.cobol_mask = "9(6)".to_owned();
        let mut salary = BindingField::new("ACTOR-SALARY", BindingDataType::Decimal);
        salary.cobol_mask = "S9(9)V99".to_owned();
        let binding = DataBindingDef::new(
            "BIND-ACTORS",
            "Actors",
            BindingSourceDescriptor::CobolTable {
                table_name: "WS-ACTOR-TABLE".to_owned(),
                occurs_item: "WS-ACTOR-ROW".to_owned(),
                fields: vec![actor_id, salary],
                key_fields: vec!["ACTOR-ID".to_owned()],
                writable: true,
            },
            BindingTargetDescriptor::DataGrid {
                control_id: "ActorGrid".to_owned(),
            },
        );
        apply_data_binding_to_form(&mut form, binding);

        // Simulate the save/run refresh that previously wiped the override.
        refresh_data_binding_target_properties(&mut form);

        let grid = form.find_control("ActorGrid").expect("grid should exist");
        let parsed = DataGridAdvanced::from_control(grid);
        let salary_col = parsed
            .columns
            .iter()
            .find(|c| c.source_name.eq_ignore_ascii_case("ACTOR-SALARY"))
            .expect("salary column");
        assert_eq!(
            salary_col.cobol_mask, "ZZ9.99-",
            "user's typed mask must survive binding refresh"
        );
        let id_col = parsed
            .columns
            .iter()
            .find(|c| c.source_name.eq_ignore_ascii_case("ACTOR-ID"))
            .expect("id column");
        assert_eq!(
            id_col.cobol_mask, "9(6)",
            "a column with no mask is seeded from the bound field's PICTURE"
        );
    }

    #[test]
    fn applying_grid_data_binding_uses_indexed_move_rows_from_form_code() {
        let mut form = Form::new("ActorForm", "ActorForm", 800, 600);
        form.add_control(Control::new("ActorGrid", ControlType::DataGrid, 0, 0));
        form.form_events.push(EventBinding {
            event: "onShow".to_owned(),
            paragraph: "ActorForm--onShow".to_owned(),
            code: "\
       PROCEDURE DIVISION.
           MOVE 000000001 TO ACTOR-ID(1).
           MOVE \"assets/images/photo000000001.jpg\" TO ACTOR-THUMB(1).
           MOVE \"Leonardo DiCaprio\" TO ACTOR-CAPTION(1).
           MOVE 30000000.00 TO ACTOR-SALARY(1).

           MOVE 000000002 TO ACTOR-ID(2).
           MOVE \"assets/images/photo000000002.jpg\" TO ACTOR-THUMB(2).
           MOVE \"Joe Pesci\" TO ACTOR-CAPTION(2).
           MOVE 12000000.00 TO ACTOR-SALARY(2).
"
            .to_owned(),
        });

        let mut actor_id = BindingField::new("ACTOR-ID", BindingDataType::Integer);
        actor_id.display_name = "Actor Id".to_owned();
        let mut thumb = BindingField::new("ACTOR-THUMB", BindingDataType::Text);
        thumb.display_name = "Actor Thumb".to_owned();
        let mut caption = BindingField::new("ACTOR-CAPTION", BindingDataType::Text);
        caption.display_name = "Actor Caption".to_owned();
        let mut salary = BindingField::new("ACTOR-SALARY", BindingDataType::Decimal);
        salary.display_name = "Actor Salary".to_owned();
        let binding = DataBindingDef::new(
            "BIND-ACTORS",
            "Actors",
            BindingSourceDescriptor::CobolTable {
                table_name: "WS-ACTOR-TABLE".to_owned(),
                occurs_item: "WS-ACTOR-ROW".to_owned(),
                fields: vec![actor_id, thumb, caption, salary],
                key_fields: vec!["ACTOR-ID".to_owned()],
                writable: true,
            },
            BindingTargetDescriptor::DataGrid {
                control_id: "ActorGrid".to_owned(),
            },
        );

        apply_data_binding_to_form(&mut form, binding);

        let grid = form.find_control("ActorGrid").expect("grid should exist");
        assert_eq!(
            grid.get_prop("Rows").map(PropValue::as_str),
            Some(
                "000000001\tassets/images/photo000000001.jpg\tLeonardo DiCaprio\t30000000.00\n\
000000002\tassets/images/photo000000002.jpg\tJoe Pesci\t12000000.00"
            )
        );
    }

    #[test]
    fn applying_grid_data_binding_replaces_existing_target_binding() {
        let mut form = Form::new("CustomerForm", "CustomerForm", 800, 600);
        form.add_control(Control::new("GRID-1", ControlType::DataGrid, 0, 0));

        let first = DataBindingDef::new(
            "BIND-OLD",
            "Old",
            BindingSourceDescriptor::CobolTable {
                table_name: "WS-OLD".to_owned(),
                occurs_item: "WS-OLD-ROW".to_owned(),
                fields: vec![BindingField::new("OLD-FIELD", BindingDataType::Text)],
                key_fields: Vec::new(),
                writable: false,
            },
            BindingTargetDescriptor::DataGrid {
                control_id: "GRID-1".to_owned(),
            },
        );
        let second = DataBindingDef::new(
            "BIND-NEW",
            "New",
            BindingSourceDescriptor::CobolTable {
                table_name: "WS-NEW".to_owned(),
                occurs_item: "WS-NEW-ROW".to_owned(),
                fields: vec![BindingField::new("NEW-FIELD", BindingDataType::Text)],
                key_fields: Vec::new(),
                writable: false,
            },
            BindingTargetDescriptor::DataGrid {
                control_id: "GRID-1".to_owned(),
            },
        );

        apply_data_binding_to_form(&mut form, first);
        apply_data_binding_to_form(&mut form, second);

        assert_eq!(form.data_bindings.len(), 1);
        assert_eq!(form.data_bindings[0].id, "BIND-NEW");
        let grid = form.find_control("GRID-1").expect("grid should exist");
        assert_eq!(
            grid.get_prop("Columns").map(PropValue::as_str),
            Some("NEW-FIELD:string")
        );
        assert_eq!(
            grid.get_prop("DataSource").map(PropValue::as_str),
            Some("WS-NEW / WS-NEW-ROW")
        );
        assert_eq!(
            grid.get_prop("Rows").map(PropValue::as_str),
            Some("NEW-FIELD 1\nNEW-FIELD 2\nNEW-FIELD 3")
        );
    }

    // ── Event-handler validation (syntax + semantic) ──────────────────────────

    fn form_with_onload(code: &str) -> cobolt_forms::Form {
        let mut f = cobolt_forms::Form::new("T", "T", 320, 200);
        let mut ev = EventBinding::new("onLoad", "T--ONLOAD");
        ev.code = code.to_string();
        f.form_events.push(ev);
        f
    }

    #[test]
    fn design_intent_whitelist_is_layout_only_never_data() {
        // Layout defaults that Run-Form adjustments may persist.
        assert!(is_design_intent_prop(
            cobolt_forms::model::DATAGRID_ADVANCED_PROP
        ));
        assert!(is_design_intent_prop("RowHeight"));
        // Runtime DATA must NEVER be captured back into the form definition.
        for data_key in ["Rows", "Value", "Text", "SelectedIndex", "Items", "Checked"] {
            assert!(
                !is_design_intent_prop(data_key),
                "{data_key} is runtime data and must not be persisted as a default"
            );
        }
    }

    #[test]
    fn validate_form_source_passes_clean_handler() {
        use crate::runner::DiagSeverity;
        let f = form_with_onload(
            "       ENVIRONMENT DIVISION.\n       PROCEDURE DIVISION.\n           CONTINUE.",
        );
        let diags = CoboltApp::validate_form_source(&f);
        assert!(
            !diags.iter().any(|d| d.severity == DiagSeverity::Error),
            "a clean handler must not report an error: {diags:?}"
        );
    }

    #[test]
    fn validate_form_source_flags_syntax_error_in_handler() {
        use crate::runner::DiagSeverity;
        // A stray ')' — the exact class of typo that previously slipped through
        // unvalidated to Run time. Validation must surface it as an error so the
        // tree semaphore turns red and Run/Build are blocked.
        let f = form_with_onload(
            "       ENVIRONMENT DIVISION.\n       PROCEDURE DIVISION.\n           DISPLAY \"x\" ).",
        );
        let diags = CoboltApp::validate_form_source(&f);
        assert!(
            diags.iter().any(|d| d.severity == DiagSeverity::Error),
            "a syntactically broken handler must report an error: {diags:?}"
        );
    }
}
