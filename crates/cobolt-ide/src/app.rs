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

use std::io::Write;
use std::path::{Path, PathBuf};

use egui::{Color32, Context, Key, KeyboardShortcut, Modifiers, Vec2, ViewportBuilder, ViewportId};

use cobolt_ast::data::DataDecl;
use cobolt_ast::expr::{FigurativeConstant, Literal};
use cobolt_ast::program::DataSection;
use cobolt_forms::{
    form_to_string, load_form, load_form_from_str, save_form, BindingDataType, BindingField,
    BindingSourceDescriptor, BindingTargetDescriptor, BindingTargetPath, ControlType,
    DataBindingDef, DataGridAdvanced, DataGridColumn, Form, PropValue, DATAGRID_ADVANCED_PROP,
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

use crate::i18n::{Language, Tr};
use crate::panels::debugger::{DebugAction, DebuggerPanel};
use crate::panels::{
    designer::DesignerPanel,
    editor::EditorPanel,
    forms_list::{FormsListAction, FormsListPanel},
    grace_chat::GraceChatPanel,
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

struct NewFormDialog {
    open: bool,
    form_name: String,
    title: String,
    width: String,
    height: String,
    /// The GLASS STYLE ("Classic", "Enhanced", "Neumorphic Light",
    /// "Neumorphic Dark"). Named `theme` historically, which is exactly the
    /// confusion 050 removes — the dialog labelled this row "Theme" and offered
    /// the four glass styles, so the real theme catalogue (Liquid Glass,
    /// Elegance, any installed pack) could not be chosen at creation at all.
    theme: String,
    /// 050 — the FORM THEME, a catalogue id. Empty means "inherit the project
    /// default", which is the same thing an empty `Form::theme` means, so the
    /// dialog writes nothing when the developer leaves it alone.
    form_theme: String,
    /// Project-relative folder the save dialog should open in — set when the
    /// dialog was raised by a folder row's `[+]`. `None` means `forms/`.
    target_dir: Option<String>,
}

impl NewFormDialog {
    fn new() -> Self {
        Self {
            open: false,
            form_name: "MAIN-FORM".into(),
            title: "My Form".into(),
            width: "640".into(),
            height: "480".into(),
            theme: "Classic".into(),
            form_theme: String::new(), // inherit the project default
            target_dir: None,
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

#[derive(Default)]
struct DesignerActivationRequests {
    paths: std::collections::HashSet<PathBuf>,
}

impl DesignerActivationRequests {
    fn request(&mut self, path: PathBuf) {
        self.paths.insert(path);
    }

    fn take(&mut self, path: &Path) -> bool {
        self.paths.remove(path)
    }
}

/// Spec 046 R7/R8 — a Paste Form whose form name collides with one already
/// in the project, awaiting the developer's rename-or-replace choice.
struct PendingPasteConflict {
    form: Form,
    dest_dir: PathBuf,
    new_name: String,
    /// R8 — Replace needs its own confirmation, separate from the initial
    /// rename/replace choice: true once the developer has clicked Replace
    /// once, showing a second, plain confirmation before anything is
    /// deleted.
    confirming_replace: bool,
}

/// Pending "New folder" dialog state (spec 033).
struct PendingFolderCreate {
    parent_rel: PathBuf,
    category_root: String,
    name: String,
}

/// Pending "Rename folder" dialog state (spec 033).
struct PendingFolderRename {
    folder_rel: PathBuf,
    category_root: String,
    name: String,
}

/// Pending "Delete folder" confirmation state (spec 033).
struct PendingFolderDelete {
    folder_rel: PathBuf,
    category_root: String,
}

/// What the developer was trying to start when the version stamp was found
/// stale — so the same action can be resumed after a full build.
#[derive(Debug, Clone, PartialEq, Eq)]
enum StaleBuildIntent {
    /// The IDE toolbar's Run.
    Run,
    /// A designer's Run Form, for the form at this index.
    RunForm(usize),
}

/// A pending "this project was last built by an older PowerRustCOBOL" prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StaleBuildPrompt {
    intent: StaleBuildIntent,
}

/// Whether a build of `project` must discard every cached artefact: true when
/// the running `current` version is newer than the one that last fully built
/// it — which includes a project that has never been fully built at all.
///
/// ONE predicate, read by both the Build button (to choose full over
/// incremental) and Run's stale gate (to decide whether to prompt). They have
/// to agree: 1.60.30 wired Build to the plain incremental path, which leaves
/// the version stamp untouched, so Run then asked for the very full build the
/// developer had just waited through — the project was built twice.
fn build_needs_full(
    project: Option<&crate::project_model::CoboltProject>,
    current: &str,
) -> bool {
    project.is_some_and(|p| p.project.build_is_stale_for(current))
}

pub struct CoboltApp {
    // Code workspace
    project: ProjectPanel,
    editor: EditorPanel,
    output: OutputPanel,
    runner: Runner,
    forms_list: FormsListPanel,

    /// Work rescued from a session that did not exit cleanly, waiting for the
    /// developer to accept or discard it. Empty on a normal start.
    pending_recovery: Vec<crate::crash::Recovered>,

    /// When unsaved work was last copied to the recovery directory.
    ///
    /// A timer rather than a save-on-every-edit, because the point is to bound
    /// what a hard kill can cost, not to mirror every keystroke.
    last_autosave: std::time::Instant,

    // Open form designers (each lives in its own viewport window)
    designers: Vec<(PathBuf, DesignerPanel)>,
    designer_activation_requests: DesignerActivationRequests,
    /// Carries out a toolbar button's PLATFORM action pressed in **Preview**,
    /// so a toolbar can be tried at design time instead of only under Run Form.
    /// Shared by every open preview: an action is begun and finished inside one
    /// frame, so there is no per-preview state to keep apart. (Window captures
    /// are deliberately NOT routed here — see `show_preview_window`.)
    preview_toolbar_runner: cobolt_forms::toolbar_actions::Runner,
    #[allow(dead_code)]
    pub(crate) clipboard: Option<DesignerClipboard>,
    /// Spec 046 R3/R4 — the project-relative destination directory for a
    /// Paste Form request awaiting the OS clipboard's `Event::Paste`, which
    /// `RequestPaste` triggers but doesn't deliver until a later frame.
    /// `None` = no paste request in flight.
    pending_form_paste: Option<PathBuf>,
    /// Spec 046 R7/R8 — a parsed paste awaiting the rename-or-replace
    /// choice for a form-name collision.
    pending_paste_conflict: Option<PendingPasteConflict>,

    // Grid browser viewports keyed by `.cidx` path
    indexed_grids: Vec<(PathBuf, IndexedGridState)>,

    // Inline form/control inspector shown in the Main Pane (from the project tree)
    inspect: Option<InspectState>,

    // Inline indexed-file inspector in the Main Pane
    indexed_inspect: Option<IndexedInspectState>,

    // Inline asset preview in the Main Pane
    asset_preview: Option<AssetPreviewState>,

    /// Indexed files that were created or last edited via the raw COBOL text
    /// editor. For these files we keep the editor visible / preferred and
    /// do not offer (or lock down) the properties pane for structural changes.
    raw_preferred_indexed: std::collections::HashSet<PathBuf>,

    // Content hash of each file at its last successful/failed check (for the tree
    // "semaphore": a file edited since its last check shows yellow again).
    checked: std::collections::HashMap<PathBuf, u64>,

    /// Forms running as external `rcrun run-form` processes (the Run Form
    /// path): own window, own event loop — the IDE stays idle while they run.
    external_runs: Vec<crate::form_runtime::ExternalFormRun>,
    /// Compiled applications started by Run (spec 041 T13) — tracked so their
    /// output streams into the Output panel and their death is reported, never
    /// silent. A re-Run replaces the previous instance.
    built_runs: Vec<crate::form_runtime::BuiltAppRun>,
    /// The form whose DESIGNER window hosts the build modal, when the build was
    /// started by that designer's Run Form button. `None` = the IDE main window
    /// hosts it (toolbar Build). Exactly one surface shows the modal.
    build_modal_host: Option<PathBuf>,
    /// Whether the build now running is a FULL one — only a full build stamps
    /// the project with the PowerRustCOBOL version that produced it.
    pending_build_full: bool,
    /// A Run/Run-Form the developer asked for while the project's last full
    /// build was older than this PowerRustCOBOL. Holds what to do once they
    /// answer the prompt; `None` when no prompt is up.
    stale_build_prompt: Option<StaleBuildPrompt>,

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
    /// External Crates dialog visibility (spec 044) — opened from the
    /// project tree's category; state lives in `external_crates_panel`.
    show_external_crates: bool,
    external_crates_panel: crate::panels::external_crates::ExternalCratesPanel,
    pending_form_delete: Option<PathBuf>,
    pending_generated_delete: Option<PathBuf>,
    pending_asset_delete: Option<PathBuf>,
    knowledge_folder_parent: Option<PathBuf>,
    knowledge_folder_name: String,
    pending_knowledge_folder_delete: Option<PathBuf>,
    // Generic project-tree folder dialogs (spec 033).
    folder_create: Option<PendingFolderCreate>,
    folder_rename: Option<PendingFolderRename>,
    folder_delete: Option<PendingFolderDelete>,
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
    /// Whether the Main Pane is showing the project-wide Grace chatbot.
    show_grace_chat: bool,
    /// Project-scoped Grace conversation state.
    grace_chat: GraceChatPanel,
    /// Set while the "save unsaved changes before closing?" dialog is shown
    /// (covers dirty forms, code editor tabs, and project settings).
    close_confirm: bool,
    /// Once the user has chosen Save-before-close or Close-without-saving, this
    /// lets the next close request through without re-prompting.
    allow_close: bool,
    /// Cached background-image texture, keyed by the resolved absolute path.
    bg_texture: Option<(PathBuf, egui::TextureHandle)>,

    /// Global AI-assistant configuration (cloud LLM for the code editor).
    /// Stored outside the project so the API key never lands in a repo.
    llm: crate::llm::LlmConfig,
    /// In-flight "Test connection" request from the settings dialog.
    llm_test_rx: Option<std::sync::mpsc::Receiver<crate::llm::LlmResponse>>,
    llm_test_from_model_selection: bool,
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
    llm_reviewer_models_rx: Option<std::sync::mpsc::Receiver<Result<Vec<String>, String>>>,
    /// Agents Manager modal (spec 028), present while open.
    agents_modal: Option<crate::panels::agents_modal::AgentsModal>,
    /// Models Manager modal (spec 031), present while open.
    models_modal: Option<crate::panels::models_modal::ModelsModal>,
    /// A KB document add found the semantic model absent — the confirmation
    /// dialog is showing.
    semantic_offer_open: bool,
    /// The developer answered "Later" this session — do not nag on every add.
    semantic_offer_declined: bool,
    /// In-flight semantic-model download, rendered as an IDE-blocking modal.
    semantic_download: Option<SemanticModelDownload>,
    llm_benchmark_offer: Option<crate::llm::LlmConfig>,
    llm_benchmark_config: Option<crate::llm::LlmConfig>,
    llm_benchmark_rx: Option<std::sync::mpsc::Receiver<crate::llm::LlmResponse>>,
    llm_benchmark_status: Option<String>,
    llm_benchmark_report: Option<String>,
    /// Machine-wide ranked record of every proficiency test (spec 040).
    leaderboard: crate::leaderboard::Leaderboard,
    /// The Model Leaderboard panel, present while open.
    leaderboard_modal: Option<crate::panels::leaderboard_modal::LeaderboardModal>,
    /// In-flight capability probe and the `(provider, model, endpoint)` it
    /// answers for.
    llm_caps_rx: Option<std::sync::mpsc::Receiver<crate::leaderboard::ModelCapabilities>>,
    llm_caps_target: Option<(String, String, String)>,

    // Dialog state
    new_form: NewFormDialog,
    new_indexed: NewIndexedDialog,
    /// Project-relative folder the pending new indexed file goes in — set when
    /// the dialog was raised by a folder row's `[+]`. `None` means `indexed/`.
    new_indexed_dir: Option<String>,
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
    /// The language last written to the machine-local preferences. The selector
    /// hands out a `&mut Language`, so the change is noticed by comparison on
    /// the next frame rather than at the point of the click.
    lang_persisted: Language,

    // State for the cycling welcome quotes on the initial screen (no project)
    welcome_quote_index: usize,
    welcome_quote_start_time: f64,

    /// 037 R2 — previous MainForm holder per claim, so an undo (un-claim)
    /// restores exactly the form that held the role before, not merely the
    /// first in the list.
    main_form_prev: Vec<Option<String>>,

    /// Whether the Help → About window is open.
    about_open: bool,
    /// Shown once after opening a project that has no usable AI model or no
    /// configured agent, inviting the user to set them up.
    ai_setup_modal: bool,
    /// The first-run Rust question, present only while it is unanswered. A
    /// machine that can already build never raises it — see [`crate::toolchain`].
    toolchain_prompt: Option<crate::toolchain::FirstRunPrompt>,
    /// Project-structure upgrades due on the open project, offered once per
    /// open. Empty = the project is current (or the developer said Not now).
    project_upgrades: Vec<&'static dyn crate::project_upgrade::ProjectUpgrade>,
    /// Documentation viewer window (Help → Documentation).
    doc_viewer: crate::panels::doc_viewer::DocViewer,
    /// IDE-wide debug switches (Help → Debug Settings) and their modal. Machine-
    /// local, not project data, so they are loaded once at startup.
    debug: crate::debug_settings::DebugSettings,
    debug_modal: crate::debug_settings::DebugSettingsModal,
    /// F12 documentation capture — an authoring tool for this checkout, behind
    /// the `doc_screenshots` debug switch.
    doc_shots: crate::doc_shots::DocShots,
    /// Non-empty while the "Form saved" alert should be displayed.
    /// A fatal form-runtime / codegen error to surface in a modal dialog. The
    /// IDE stays open; execution has already stopped on the interpreter thread.
    form_error: Option<String>,
    /// A pending "delete this common procedure?" confirmation (operator,
    /// 2026-08-05: user code is never removed without being asked).
    ///
    /// `designer` is the index into `designers`, or `None` when the request came
    /// from the Run-Form inspector, which carries its own designer state.
    pending_proc_delete: Option<PendingProcDelete>,
    alert_error: Option<String>,
    /// Font size for the message text in the error modals (adjusted with the
    /// A− / A+ buttons in the dialog; session-only, like the output-log size).
    error_font_size: f32,
    /// Transient "form saved" cue: while the deadline is in the future, the
    /// designer Save button paints a checkmark instead of its normal icon
    /// (replaces the old modal alert). Keyed by the saved form's path so only
    /// that designer flashes.
    save_flash: Option<(std::path::PathBuf, std::time::Instant)>,

    /// Dev-agent change-set awaiting the developer's Approve/Reject (spec 025 T9).
    /// `Some` while a proposal is on screen; nothing is applied until approved.
    agent_preview: Option<crate::agent::AgentPreview>,
    /// Dev-agent prompt-bar state (spec 025 T10).
    agent_prompt: String,
    /// `(prompt-that-was-sent, reply channel)` — the prompt is recorded to memory
    /// only after a successful reply (spec 025 R16).
    agent_pending: Option<(String, std::sync::mpsc::Receiver<crate::llm::LlmResponse>)>,
    /// Route the next request through Grace's multi-agent workflow (spec 029
    /// Phase C) instead of the single-agent path.
    use_grace: bool,
    /// The running (or just-finished) Grace workflow, when routed.
    grace_session: Option<crate::grace_session::GraceSession>,
    /// The target-disambiguation modal for the agent surface (spec 034).
    target_picker: crate::panels::target_picker::TargetPicker,
    /// One-shot guard: whether the current finished session's approved
    /// form-design output has been applied to the form yet (spec 030 R7).
    grace_applied: bool,
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
    /// Set when *Run* started a build because the program contains `EXEC RUST`
    /// (spec 041 T13): the form whose run is waiting for that build.
    ///
    /// A block is compiled, so `rcrun run-form` — which walks the AST — cannot
    /// execute one. Run therefore builds first and starts the built binary. The
    /// build is asynchronous, so the intent to run has to survive until its
    /// result arrives; `None` means the build was a plain Build.
    pending_build_then_run: Option<PathBuf>,
    /// Manual KB reindex (File menu): progress/done messages from the worker
    /// thread, drained each frame. `Some` = running, which disables the menu
    /// item and shows the progress modal.
    kb_reindex_rx: Option<std::sync::mpsc::Receiver<KbReindexMsg>>,
    /// Latest reindex phase for the modal bar: (fraction 0..1, "n/m — subject").
    kb_reindex_phase: (f32, String),
    /// Hide flag for the KB-reindex background modal: the work continues,
    /// only the dialog is dismissed (reset when a new run starts).
    kb_reindex_modal_hidden: bool,
    /// The Building dialog is modal and outlives the build: it stays up,
    /// showing the outcome, until the user presses Close — which is the only
    /// thing that sets this. Reset when a new build starts.
    build_modal_closed: bool,
    /// Outcome of the finished build, so the Building dialog still has
    /// something to show once the worker is gone: `Ok` = the success summary
    /// line, `Err` = the failure message. Set by the drain in `update`,
    /// cleared when a new build starts.
    build_outcome: Option<Result<String, String>>,
    /// Full log of the current/last build (phases, details, result), colored
    /// per line in the details window. Cleared when a build starts.
    build_log: Vec<(BuildLogKind, String)>,
    /// The "Build details" window (resizable, movable, centered by default).
    /// Auto-opens when a build fails.
    build_details_open: bool,
    /// Ticker state: how many `build_log` lines are revealed so far, and when
    /// the last one appeared — one line every 250 ms so the log reads as a
    /// feed instead of dumping all at once.
    build_log_shown: usize,
    build_log_last_reveal: Option<std::time::Instant>,
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
    NewForm(Box<cobolt_forms::Form>),
    /// Save the build-details log (payload: the plain-text log).
    SaveBuildLog(String),
    /// Pick a background image for the IDE appearance settings.
    PickBackgroundImage,
    /// Pick a project icon image for Run Form / packaged app windows.
    PickProjectIcon,
    OpenGridData {
        cidx_path: PathBuf,
        def: IndexedDefinition,
    },
    /// Save the given error-modal message text to the chosen file.
    SaveErrorText(String),
    /// Export the COBOL proficiency benchmark report to PDF.
    SaveBenchmarkPdf(String),
}

/// The shared egui key for the single app-level file dialog.
const APP_FILE_KEY: &str = "app-file-dialog";

/// Initial size of the (user-resizable) error modals. A seed only: after the
/// first frame the size lives in egui's window state and changes exclusively
/// through the user's resize drag.
const ERROR_MODAL_SIZE: [f32; 2] = [800.0, 450.0];

/// Actions requested by [`error_modal_body_ui`], applied by the caller.
#[derive(Default)]
struct ErrorBodyAction {
    close: bool,
    save: bool,
}

/// Put an error/confirmation modal above every other window in ITS viewport.
///
/// A plain `egui::Window` is `Order::Middle`, so any other window the user
/// clicks rises over it. `Window::order(Order::Foreground)` beats all of
/// `Middle` outright; inside `Foreground` (shared with menus and popups) egui
/// still stacks by last interaction, so this must also be called every frame
/// the modal is on screen. Call it only AFTER the "is it open?" guard: it
/// marks the layer visible for the frame.
///
/// This is per-viewport ONLY — layer order lives in `Memory::areas`, which is
/// keyed by `ViewportId`. It cannot lift a window over a separate OS window;
/// that is what `WindowLevel`/`with_always_on_top` is for.
pub(crate) fn raise_modal_layer(ctx: &egui::Context, id: egui::Id) {
    ctx.move_to_top(egui::LayerId::new(egui::Order::Foreground, id));
}

/// A `Resize` salt that is unique per OS window.
///
/// `Areas` are per-viewport but `ctx.data()` — where `egui::Resize` keeps the
/// user's box size — is shared by all of them. The same modal painted in the
/// main window and in a designer viewport would otherwise fight over one size
/// state, which in this codebase is how self-inflation starts.
fn per_viewport_salt(ctx: &egui::Context, base: &str) -> String {
    format!("{base}_{}", ctx.viewport_id().0.value())
}

/// The user-resizable box every error modal lives in. The inner `egui::Resize`
/// is the single size authority: seeded at [`ERROR_MODAL_SIZE`], changed only
/// by the user's grip drag. The body must keep its measured content within the
/// box — egui (0.35) ratchets `Resize` up to the content min-size every frame,
/// so any overflow becomes runaway growth. Pair with [`error_modal_body_ui`],
/// whose embedded panels partition the box exactly.
fn error_modal_scaffold(ui: &mut egui::Ui, id_salt: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::Resize::default()
        .id_salt(id_salt)
        .resizable([true, true])
        .min_size(egui::vec2(380.0, 220.0))
        .max_size(egui::vec2(4000.0, 4000.0))
        .default_size(egui::Vec2::from(ERROR_MODAL_SIZE)) // seed only
        .show(ui, |ui| {
            // `sz` is the Resize box: user/default state, bounded — NOT
            // "remaining space" of an auto-sizing container.
            let sz = ui.available_size();
            ui.allocate_ui(sz, |ui| {
                ui.set_min_size(sz);
                body(ui);
            });
        });
}

/// Error-modal interior: intro, scrollable message, button row. Laid out with
/// embedded panels (footer `Panel::bottom`, message `CentralPanel`) so the
/// content partitions the fixed box EXACTLY — no estimated heights. Estimated
/// reserves regressed under egui 0.35: skrifa font metrics made the real row
/// taller than the estimate, and Resize's per-frame `max(content)` ratchet
/// turned the few overflow pixels into unbounded growth.
fn error_modal_body_ui(
    ui: &mut egui::Ui,
    intro: Option<&str>,
    msg: &str,
    font_size: &mut f32,
) -> ErrorBodyAction {
    let mut act = ErrorBodyAction::default();
    egui::Panel::bottom(ui.id().with("error_modal_footer"))
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    act.close = true;
                }
                ui.separator();
                if ui
                    .button("Copy")
                    .on_hover_text("Copy the error message to the clipboard")
                    .clicked()
                {
                    ui.ctx().copy_text(msg.to_owned());
                }
                if ui
                    .button("Save…")
                    .on_hover_text("Save the error message to a text file")
                    .clicked()
                {
                    act.save = true;
                }
                ui.separator();
                if ui
                    .small_button("A−")
                    .on_hover_text("Decrease font size")
                    .clicked()
                {
                    *font_size = (*font_size - 1.0).max(MIN_ERROR_FONT_SIZE);
                }
                ui.label(egui::RichText::new(format!("{} px", font_size.round() as i32)).small());
                if ui
                    .small_button("A+")
                    .on_hover_text("Increase font size")
                    .clicked()
                {
                    *font_size = (*font_size + 1.0).min(MAX_ERROR_FONT_SIZE);
                }
            });
        });
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            ui.add_space(4.0);
            if let Some(intro) = intro {
                ui.label(egui::RichText::new(intro).strong());
                ui.add_space(6.0);
            }
            // The reason, first and set apart. The log below is every request
            // line, header and retry — which is what you want when something is
            // wrong, and which buried the one sentence that says WHAT is wrong,
            // usually inside a JSON body on a single very long line (operator,
            // 2026-08-20). The headline is the provider's own words, quoted;
            // when there is no payload to quote there is no headline, and this
            // modal looks exactly as it always did.
            let summary = crate::error_summary::summarize(msg);
            if let Some(summary) = &summary {
                ui.add_space(2.0);
                // Wrapped, unlike the log: a reason is prose and belongs on
                // screen in full, not off the right-hand edge.
                ui.label(
                    egui::RichText::new(&summary.headline)
                        .strong()
                        .size(*font_size + 2.0)
                        .color(egui::Color32::from_rgb(240, 170, 90)),
                );
                let mut detail: Vec<String> = Vec::new();
                if let Some(param) = &summary.param {
                    if !summary.headline.contains(param.as_str()) {
                        detail.push(format!("parameter: {param}"));
                    }
                }
                if let Some(code) = &summary.code {
                    if !summary.headline.contains(code.as_str()) {
                        detail.push(code.clone());
                    }
                }
                if !detail.is_empty() {
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(detail.join("   ·   "))
                            .monospace()
                            .size(*font_size)
                            .color(crate::theme::active().text_dim),
                    );
                }
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
            }
            egui::ScrollArea::both()
                .id_salt("error_modal_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // `both()` disables wrapping, so long single-line errors
                    // scroll horizontally instead of inflating the window.
                    ui.label(egui::RichText::new(msg).monospace().size(*font_size));
                });
        });
    act
}
/// Clamp range for the error-modal message font size.
const MIN_ERROR_FONT_SIZE: f32 = 8.0;
const MAX_ERROR_FONT_SIZE: f32 = 28.0;

/// Standard project sub-folders — one per category plus working/build folders.
/// Created when a project is made, and back-filled (if missing) when one is opened.
const PROJECT_FOLDERS: &[&str] = &[
    "src",
    "forms",
    "indexed",
    "generated",
    "Assets",
    "assets",
    "Knowledge Base",
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

#[derive(Clone)]
struct AssetPreviewState {
    path: PathBuf,
    rel: String,
    content: AssetPreviewContent,
    zoom_percent: f32,
    search_open: bool,
    search_query: String,
    animation_playing: bool,
    animation_frame: usize,
    animation_last_tick: Option<std::time::Instant>,
}

#[derive(Clone)]
enum AssetPreviewContent {
    Image {
        texture: egui::TextureHandle,
        size: egui::Vec2,
        svg_path: Option<PathBuf>,
    },
    Animation {
        frames: Vec<AssetAnimationFrame>,
        size: egui::Vec2,
    },
    Text(String),
    Binary(Vec<u8>),
}

#[derive(Clone)]
struct AssetAnimationFrame {
    texture: egui::TextureHandle,
    delay: std::time::Duration,
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
/// One line of the build-details log, colored by what it reports.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BuildLogKind {
    /// A phase milestone ("Parsing main.cbl…") — theme foreground.
    Phase,
    /// A supplementary detail (counts, sizes) — dimmed.
    Detail,
    /// Successful completion — green.
    Success,
    /// A build error — red.
    Error,
}

/// A common procedure the developer asked to delete, held until they confirm
/// (operator, 2026-08-05: user code is never removed without being asked).
#[derive(Clone)]
struct PendingProcDelete {
    /// Index into `designers`, or `None` for the Run-Form inspector's own
    /// designer state.
    designer: Option<usize>,
    index: usize,
    name: String,
    /// Non-blank lines of body, so the dialog can say what is at stake.
    lines: usize,
}

/// Messages from the manual KB-reindex worker (File menu) to the UI thread.
enum KbReindexMsg {
    /// Embedding progress for the modal's determinate bar: fraction 0..1 and
    /// a "n/m — subject" label. Sent per record; the drain keeps the latest.
    Phase(f32, String),
    /// The worker finished: the summary line, or the error.
    Done(Result<String, String>),
}

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

fn ensure_pdf_extension(path: PathBuf) -> PathBuf {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("pdf") => path,
        _ => path.with_extension("pdf"),
    }
}

fn sanitize_filename_component(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_sep = false;
    for ch in value.trim().chars() {
        let valid =
            !matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') && !ch.is_control();
        let mapped = if valid { ch } else { '-' };
        let mapped = if mapped.is_whitespace() { '-' } else { mapped };
        if mapped == '-' || mapped == '_' || mapped == '.' {
            if last_was_sep {
                continue;
            }
            last_was_sep = true;
        } else {
            last_was_sep = false;
        }
        out.push(mapped);
    }
    let trimmed = out
        .trim_matches(|c| c == '-' || c == '_' || c == '.' || c == ' ')
        .to_string();
    let candidate = if trimmed.is_empty() {
        "model".to_string()
    } else {
        trimmed
    };
    let upper = candidate.to_ascii_uppercase();
    let reserved = matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if reserved {
        format!("model-{candidate}")
    } else {
        candidate
    }
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

/// Spec 046 R5 — the `.cfrm` file name a pasted form registers under,
/// matching `create_new_form`'s own convention (`form_name.to_lowercase()`)
/// so a pasted and a hand-created form are indistinguishable on disk.
fn pasted_form_file_name(form_name: &str) -> String {
    format!("{}.cfrm", form_name.to_lowercase())
}

/// Spec 046 R3/R4 — the text of the first `Event::Paste` in this frame's
/// raw input, if any. Pure and free of `egui::Context` so it's directly
/// testable: `RequestPaste`'s delivered event (or an ordinary Cmd/Ctrl+V)
/// sits alongside every other event a frame carries, and this is the exact
/// scan `poll_form_paste` runs over `ctx.input(|i| &i.events)`.
fn extract_pasted_text(events: &[egui::Event]) -> Option<String> {
    events.iter().find_map(|e| match e {
        egui::Event::Paste(text) => Some(text.clone()),
        _ => None,
    })
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
        AgentOp::SetFormStructure { block, .. } => (
            format!("{} {block}", tr.agent_op_form_structure),
            Color32::from_rgb(150, 200, 190),
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
        let mut style = (*cc.egui_ctx.global_style()).clone();
        style.visuals = egui::Visuals::dark();
        cc.egui_ctx.set_global_style(style);
        cc.egui_ctx.set_fonts(crate::fonts::base_font_definitions());
        // Image loaders (PNG/etc.) — needed by the Documentation viewer's
        // Markdown image rendering.
        egui_extras::install_image_loaders(&cc.egui_ctx);

        // ── Agent access (spec 027 R3): egui inspection / MCP endpoint ─────────
        // Always on, loopback only. External agents connect through the official
        // `egui-mcp` bridge; the in-IDE agents drive the same plugin in-process
        // via `ctx.with_plugin`. Never compiled into rcrun / packaged apps (R4).
        let inspection_port = crate::llm::LlmConfig::load().inspection_port;
        let inspection_addr = format!("127.0.0.1:{inspection_port}");
        cc.egui_ctx
            .add_plugin(egui_inspection::InspectionPlugin::new(Some(format!(
                "PowerRustCOBOL {}",
                crate::version::VERSION
            ))));
        let inspection_status = match egui_inspection::serve(&cc.egui_ctx, &inspection_addr) {
            Ok(()) => Ok(inspection_addr),
            Err(e) => Err(format!("{inspection_addr}: {e}")),
        };

        let mut app = Self {
            project: ProjectPanel::new(),
            editor: EditorPanel::new(),
            // Only a session that never reached its clean exit leaves both a
            // marker and copies behind; a normal start finds neither.
            pending_recovery: if crate::crash::ended_badly() {
                crate::crash::recovered()
            } else {
                Vec::new()
            },
            last_autosave: std::time::Instant::now(),
            output: OutputPanel::new(),
            runner: Runner::new(),
            forms_list: FormsListPanel::new(),
            designers: Vec::new(),
            designer_activation_requests: DesignerActivationRequests::default(),
            preview_toolbar_runner: cobolt_forms::toolbar_actions::Runner::default(),
            clipboard: None,
            pending_form_paste: None,
            pending_paste_conflict: None,
            indexed_grids: Vec::new(),
            inspect: None,
            indexed_inspect: None,
            asset_preview: None,
            raw_preferred_indexed: std::collections::HashSet::new(),
            checked: std::collections::HashMap::new(),
            external_runs: Vec::new(),
            built_runs: Vec::new(),
            build_modal_host: None,
            pending_build_full: false,
            stale_build_prompt: None,
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
            perf_ms_sum: 0.0,
            perf_ms_max: 0.0,
            perf_fps: 0,
            perf_avg_ms: 0.0,
            perf_max_ms: 0.0,
            last_window_title: String::new(),

            cobolt_project: None,
            project_path: None,
            show_external_crates: false,
            external_crates_panel: crate::panels::external_crates::ExternalCratesPanel::new(),
            pending_form_delete: None,
            pending_generated_delete: None,
            pending_asset_delete: None,
            knowledge_folder_parent: None,
            knowledge_folder_name: String::new(),
            pending_knowledge_folder_delete: None,
            folder_create: None,
            folder_rename: None,
            folder_delete: None,
            pending_user_control_delete: None,
            pending_indexed_delete: None,
            delete_cidx_file: false,
            delete_data_file: false,
            theme_packs: std::collections::HashMap::new(),
            theme_packs_loaded: false,

            settings_form: None,
            show_project_settings: false,
            show_grace_chat: false,
            grace_chat: GraceChatPanel::new(),
            close_confirm: false,
            allow_close: false,
            bg_texture: None,
            llm: crate::llm::LlmConfig::load(),
            llm_test_rx: None,
            llm_test_from_model_selection: false,
            llm_test_status: None,
            llm_test_error: None,
            llm_detect_rx: None,
            llm_models_rx: None,
            llm_reviewer_models_rx: None,
            agents_modal: None,
            models_modal: None,
            semantic_offer_open: false,
            semantic_offer_declined: false,
            semantic_download: None,
            llm_benchmark_offer: None,
            llm_benchmark_config: None,
            llm_benchmark_rx: None,
            llm_benchmark_status: None,
            llm_benchmark_report: None,
            leaderboard: crate::leaderboard::Leaderboard::load(),
            leaderboard_modal: None,
            llm_caps_rx: None,
            llm_caps_target: None,

            new_form: NewFormDialog::new(),
            new_indexed: NewIndexedDialog::new(),
            new_indexed_dir: None,
            new_project: NewProjectDialog::new(),

            pending_open_in_editor: None,
            pending_goto_paragraph: None,
            glass_visuals_applied: false,
            lang: crate::ui_prefs::load_language(),
            lang_persisted: crate::ui_prefs::load_language(),
            welcome_quote_index: 0,
            welcome_quote_start_time: 0.0,
            main_form_prev: Vec::new(),
            about_open: false,
            ai_setup_modal: false,
            toolchain_prompt: None,
            project_upgrades: Vec::new(),
            doc_viewer: Default::default(),
            debug: crate::debug_settings::DebugSettings::load(),
            debug_modal: Default::default(),
            doc_shots: Default::default(),
            form_error: None,
            pending_proc_delete: None,
            alert_error: None,
            error_font_size: 13.0,
            save_flash: None,
            agent_preview: None,
            agent_prompt: String::new(),
            agent_pending: None,
            use_grace: true,
            grace_session: None,
            target_picker: crate::panels::target_picker::TargetPicker::default(),
            grace_applied: false,
            agent_history: Vec::new(),
            agent_history_form: None,
            agent_status: None,
            agent_debug_open: false,
            pending_build_rx: None,
            pending_build_then_run: None,
            kb_reindex_rx: None,
            kb_reindex_phase: (0.0, String::new()),
            kb_reindex_modal_hidden: false,
            build_modal_closed: false,
            build_outcome: None,
            build_log: Vec::new(),
            build_details_open: false,
            build_log_shown: 0,
            build_log_last_reveal: None,
            pending_build_progress: None,
            build_phase: (0.0, String::new()),
            pending_file: None,
            egui_ctx: cc.egui_ctx.clone(),
        };
        // Surface the agent endpoint in the Output console (translated when the
        // language loads; English at first frame matches the console's startup
        // lines).
        match inspection_status {
            Ok(addr) => {
                let tr = app.lang.tr();
                app.output
                    .push_status(tr.ai_inspection_listening.replacen("{}", &addr, 1));
            }
            Err(e) => {
                app.output
                    .push_status(format!("Agent access endpoint failed to start: {e}"));
            }
        }

        // Can this machine Build? Probed on every start — it is one cheap
        // process — because a `rustc` that lives outside the desktop session's
        // PATH has to be put back on it for the `cargo` that Build spawns.
        // Only the *first* run turns a "no" into a question.
        let toolchain = crate::toolchain::detect();
        if let crate::toolchain::Status::Ok { path, .. } = &toolchain {
            crate::toolchain::ensure_on_path(path);
        }
        if !crate::ui_prefs::rust_check_done() {
            match crate::toolchain::FirstRunPrompt::for_status(toolchain) {
                Some(prompt) => app.toolchain_prompt = Some(prompt),
                None => crate::ui_prefs::mark_rust_check_done(),
            }
        }
        app
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
        // The built-in procedural themes come from the catalog itself (Liquid
        // Glass, then Elegance — spec 047 R1/AC1), so a new built-in surfaces in
        // both pickers with no change here.
        let mut choices: Vec<crate::theme_ui::ThemeChoice> =
            cobolt_forms::theme::ThemeCatalog::builtin()
                .themes()
                .iter()
                .map(|t| crate::theme_ui::ThemeChoice {
                    id: t.id.clone(),
                    display_name: t.display_name.clone(),
                    self_contained: t.self_contained,
                })
                .collect();
        let mut packs: Vec<_> = self.theme_packs.values().collect();
        packs.sort_by(|a, b| a.id.cmp(&b.id));
        for p in packs {
            choices.push(crate::theme_ui::ThemeChoice {
                id: p.id.clone(),
                display_name: p.display_name.clone(),
                // 050 R3 — the pack's own declaration.
                self_contained: p.manifest.self_contained,
            });
        }
        // 050 R19 — the per-form picker needs this to show what an unset
        // override actually resolves to. It used to pass `None` and therefore
        // reported Liquid Glass for a form inheriting a themed project.
        let project_default = self
            .cobolt_project
            .as_ref()
            .and_then(|p| p.form_theme_default())
            .map(|s| s.to_owned());
        crate::theme_ui::publish(ctx, choices, project_default);
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

    /// Resolve a form's effective theme to the implementation its controls are
    /// painted by (spec 047/050). Pairs with [`Self::resolve_theme_pack`]: that
    /// one answers "which asset pack", this one "which procedural look
    /// underneath".
    fn resolve_surface_theme(
        &self,
        form_theme: Option<&str>,
    ) -> std::sync::Arc<dyn cobolt_forms::surface_theme::SurfaceTheme> {
        let proj_default = self
            .cobolt_project
            .as_ref()
            .and_then(|p| p.form_theme_default());
        let id = cobolt_forms::theme::resolve_theme_id(form_theme, proj_default);
        // 050 R3 — an asset pack's own manifest says whether it owns the whole
        // look; a procedural theme is resolved from the registry.
        match self.theme_packs.get(&id) {
            Some(p) => cobolt_forms::surface_theme::for_pack(p.manifest.self_contained),
            None => cobolt_forms::surface_theme::for_theme_id(&id),
        }
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
        // Was this project last fully built by an older PowerRustCOBOL? Ask
        // before running anything — the prompt offers the full build itself.
        if self.stale_build_blocks(StaleBuildIntent::Run) {
            return;
        }
        self.regenerate_all_forms();
        self.regenerate_all_indexed_files();
        // A project is a DESKTOP project (the only kind today): Run starts the
        // MAIN form — never a bare source file (spec 037 R5).
        if self.cobolt_project.is_some() {
            self.run_project_main_form();
            return;
        }
        // Single-file mode: run the editor's COBOL source in the console runner.
        let Some((path, source)) = self
            .editor
            .active_source()
            .map(|(p, s)| (p.clone(), s.to_owned()))
        else {
            self.output.clear_run_output();
            self.output
                .push_status("Open a COBOL file, or open a project, to run.");
            return;
        };
        self.output.clear_run_output();
        self.output.push_status(format!(
            "── Running {} ──",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("?")
        ));
        self.editor.clear_diags();
        self.runner.start(path.display().to_string(), source);
    }

    /// Run the open (desktop) project: resolve the MAIN form — repairing the
    /// exactly-one invariant if needed — and launch it as a standalone
    /// `rcrun run-form` process. An open designer's live state wins; a closed
    /// main form is loaded from disk (its `.cbl` was just regenerated by
    /// `do_run`'s `regenerate_all_forms`).
    fn run_project_main_form(&mut self) {
        let Some(dir) = self.project_dir() else {
            return;
        };
        let forms: Vec<String> = self
            .cobolt_project
            .as_ref()
            .map(|p| p.files.forms.clone())
            .unwrap_or_default();
        if forms.is_empty() {
            self.output.clear_run_output();
            self.output.push_status(
                "A desktop project needs at least one form — create one to Run.",
            );
            return;
        }
        let holder = match crate::main_form::normalize_main_form(&dir, &forms) {
            Ok(outcome) => outcome.holder().map(|s| s.to_owned()),
            Err(e) => {
                self.output.push_status(format!("Run: {e}"));
                None
            }
        };
        let Some(rel) = holder else {
            self.output
                .push_status("Run: the project has no main form.");
            return;
        };
        let abs = dir.join(&rel);
        // Open in a designer → its Run Form path (saves dirty state first).
        if let Some(idx) = self.designers.iter().position(|(p, _)| *p == abs) {
            self.do_run_form(idx);
            return;
        }
        match load_form(&abs) {
            Ok(form) => self.spawn_form_run(abs, form, false),
            Err(e) => {
                self.output
                    .push_status(format!("Run: could not load the main form {rel}: {e}"));
            }
        }
    }

    /// Start the binary a build just produced, because *Run* needed a build
    /// (the program contains `EXEC RUST` — spec 041 T13).
    ///
    /// The binary is its own application: it embeds the program, its forms, and
    /// the compiled blocks, and needs no toolchain. It is started detached, the
    /// way a developer would start it from a file manager — the IDE does not
    /// own its window, and closing the IDE does not close it.
    fn launch_built_binary(&mut self, binary: &Path, form_path: &Path) {
        let tr = self.lang.tr();
        // A re-Run replaces the previous instance — before this, every Run
        // left the last binary running, and a stale window could mask a new
        // launch that failed.
        for run in &mut self.built_runs {
            run.stop();
        }
        self.built_runs.clear();
        match crate::form_runtime::BuiltAppRun::spawn(
            binary,
            form_path.to_path_buf(),
            self.debug.child_env(),
        ) {
            Ok(run) => {
                // The pid is the operator's proof that something started; the
                // old message alone was also the last thing they ever heard.
                self.output.push_status(
                    tr.status_built_started
                        .replace("{name}", &run.name)
                        .replace("{pid}", &run.pid().to_string()),
                );
                self.set_element_status(form_path, ElementStatus::Tested);
                self.built_runs.push(run);
            }
            Err(e) => {
                let msg = format!("Error starting {}: {e}", binary.display());
                self.output.push_status(msg.clone());
                self.set_element_status(form_path, ElementStatus::Failed);
                self.set_form_error(msg);
                // The error dialog is an Order::Foreground window; the Build
                // modal's backdrop would block it. Get the modal out of the way
                // so the failure is the thing on screen, not "Build succeeded".
                self.build_modal_closed = true;
            }
        }
    }

    /// Whether the open project was last fully built by an OLDER
    /// PowerRustCOBOL than the one running.
    fn build_is_stale(&self) -> bool {
        build_needs_full(self.cobolt_project.as_ref(), crate::version::VERSION)
    }

    /// Gate every Run path on the version stamp.
    ///
    /// Returns `true` when the caller should stop and let the developer answer
    /// the prompt. The prompt reappears on every Run until a full build
    /// actually happens — which is the point: an incremental build cannot
    /// promise that nothing compiled by the older version survived, so nothing
    /// short of the full build clears it.
    ///
    /// It asks a NARROWER question than [`build_needs_full`], and must. That
    /// one answers "does a build have to discard its cache", and a project that
    /// has never been built qualifies. This one answers "was this output made
    /// by a version I am not" — and a project created moments ago by the
    /// running IDE has no output at all. Sharing the predicate meant every
    /// brand-new project was told on every single Run that it "was built by an
    /// older PowerRustCOBOL", which was both untrue and the first thing a new
    /// user saw (user report, 2026-08-21).
    fn stale_build_blocks(&mut self, intent: StaleBuildIntent) -> bool {
        let built_elsewhere = self
            .cobolt_project
            .as_ref()
            .is_some_and(|p| p.project.built_by_a_different_version(crate::version::VERSION));
        if !built_elsewhere {
            return false;
        }
        self.stale_build_prompt = Some(StaleBuildPrompt { intent });
        true
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

        self.output.clear_run_output();
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

    // NOTE: there is deliberately **no** `autoclean_orphaned_procedures` here
    // any more (operator, 2026-08-05).
    //
    // Save and Run used to sweep orphaned procedures as a self-heal for forms
    // predating the delete-time sweep. The cost showed up the first time a
    // developer created one: a procedure they had just written had no caller
    // yet — nobody calls something that did not exist a minute ago — so half
    // the orphan test was satisfied by novelty alone, and the other half by any
    // control name that did not resolve. Pressing Save made it disappear.
    //
    // A procedure is now only removed where the removal is actually justified:
    // in the designer, at the moment controls are deleted, by the same undoable
    // command with a notice on the record (`Designer::delete_controls`). Save
    // and Run write what the developer wrote.

    /// Run Form: launch the form as a standalone `rcrun run-form` process.
    fn do_run_form(&mut self, idx: usize) {
        // Same version gate as the IDE's Run — the developer is starting their
        // solution either way, and a stale full build affects both equally.
        if self.stale_build_blocks(StaleBuildIntent::RunForm(idx)) {
            return;
        }
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
        // Procedures are NOT swept here: a procedure the developer wrote a
        // minute ago has no caller yet, and Run would delete it for that alone.
        // Control deletion is where that cleanup belongs.
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
        self.save_flash = None;
        self.do_generate_cobol(idx);

        let form_path = self.designers[idx].0.clone();
        let form = self.designers[idx].1.form.clone();
        self.spawn_form_run(form_path, form, debug);
    }

    /// Launch `form` (already saved, with its `.cbl` regenerated) as a
    /// standalone `rcrun run-form` process. Shared tail of the designer Run
    /// Form button and the project Run button (which launches the MAIN form,
    /// open in a designer or not — spec 037 R5).
    fn spawn_form_run(&mut self, form_path: PathBuf, form: Form, debug: bool) {
        self.output.clear_run_output();
        self.output.push_status(format!(
            "── {} form {} ──",
            if debug { "Debugging" } else { "Running" },
            form_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
        ));

        // Kill any existing run for this form first.
        self.external_runs.retain_mut(|run| {
            if run.form_path == form_path {
                run.stop();
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
                self.set_form_error(format!(
                    "This form's code has a syntax or semantic error — it cannot run until you fix it:\n\n{err}"
                ));
                return;
            }
        }

        // Run the form as a standalone `rcrun run-form` process — its own
        // window, event loop, and interpreter, exactly like a binary built by
        // `rcrun build`. The IDE stays idle while the form runs.
        let cbl_path = self.generated_cbl_path(&form_path);

        // …unless the program contains `EXEC RUST` (spec 041 T13). A block is
        // compiled into the built binary, so the interpreter `rcrun run-form`
        // uses has nothing to call; running it anyway would fail loudly at the
        // first block, which is correct but useless. Build first and run what
        // the build produced. Programs without a block never reach this and
        // keep the fast path exactly as it was.
        if crate::exec_rust_run::file_has_blocks(&cbl_path) {
            let tr = self.lang.tr();
            if debug {
                // The debugger drives the interpreter over `@DBG` lines; a
                // compiled block is native code with no such protocol. Say so
                // rather than starting a session that cannot step into it.
                let msg = tr.status_exec_rust_debug_unsupported.to_owned();
                self.output.push_status(msg.clone());
                self.set_form_error(msg);
                return;
            }
            if self.cobolt_project.is_none() {
                // Building needs a project manifest; a single form has none.
                self.output.push_status(
                    "EXEC RUST needs a project to build — open or create one to run this form."
                        .to_owned(),
                );
                return;
            }
            let building = tr.status_exec_rust_building.to_owned();
            // `do_build_binary` clears the run output and may refuse (no
            // project, forms with errors), so the notice goes in afterwards and
            // the pending intent is only recorded once a build really started.
            self.do_build_binary();
            if self.pending_build_rx.is_some() {
                self.output.push_status(building);
                self.pending_build_then_run = Some(form_path.clone());
            } else {
                // The build refused to start (guardian gate, missing manifest,
                // form errors — each already reported its own reason). Without
                // this line the Run intent just evaporated: no build, no
                // launch, and nothing saying so.
                let tr = self.lang.tr();
                self.output
                    .push_status(tr.status_run_not_started.to_owned());
            }
            return;
        }
        let theme_default = self
            .cobolt_project
            .as_ref()
            .and_then(|p| p.form_theme_default())
            .map(|s| s.to_owned());
        let project_icon = self.project_icon_abs_path();
        // The dump fires when ANY debug switch is on — including the IDE-only
        // ones (rounded clip, AI-pane debug) — so a single toggle is enough to
        // get a full per-control record on the run.
        let diagnostics = crate::form_runtime::RunDiagnostics {
            env: self.debug.child_env(),
            dump_project: self
                .cobolt_project
                .as_ref()
                .filter(|_| self.debug.any_enabled())
                .map(|p| p.project.name.clone()),
        };
        // 038 — window effects: project settings × the form's opt-out × the
        // kill-switch, resolved IDE-side so the child needs no project file.
        let fx = crate::form_runtime::resolve_fx_args(
            self.cobolt_project.as_ref(),
            form.window_effects,
            self.debug.no_window_fx,
        );
        // Credentials resolved IDE-side (spec 039 T12/T15) — the Maps and
        // Custom Search API keys reach the child only via its environment,
        // never the .cfrm/.cbl.
        let secrets: Vec<(&'static str, String)> =
            crate::form_runtime::resolve_maps_api_key_secret(&form, &self.llm)
                .into_iter()
                .chain(crate::form_runtime::resolve_search_api_key_secret(
                    &form, &self.llm,
                ))
                .collect();
        match crate::form_runtime::ExternalFormRun::spawn(
            form_path.clone(),
            form.name.clone(),
            &cbl_path,
            theme_default.as_deref(),
            project_icon.as_deref(),
            debug,
            &diagnostics,
            fx.as_ref(),
            &secrets,
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
                self.set_form_error(e);
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
        // Always-on-top, EXCEPT while a dialog is waiting for an answer: an
        // always-on-top OS window covers every egui window in every other
        // viewport, and no layer `Order` can beat it. Note the level must be
        // set explicitly both ways — egui only emits `ViewportCommand::
        // WindowLevel` when the rebuilt builder carries a DIFFERENT `Some`
        // level (egui 0.35 viewport.rs:848); merely dropping the call leaves
        // the window pinned on top.
        let level = if self.blocking_modal_open() {
            egui::WindowLevel::Normal
        } else {
            egui::WindowLevel::AlwaysOnTop
        };
        let mut builder = ViewportBuilder::default()
            .with_title(self.debugger.window_title())
            .with_resizable(true)
            .with_window_level(level);
        // Apply the default size ONLY on the first frame after the session
        // starts; afterwards the OS window size is the user's alone.
        if !self.debugger_vp_sized {
            builder = builder.with_inner_size([900.0, 520.0]);
            self.debugger_vp_sized = true;
        }
        ctx.show_viewport_immediate(vp_id, builder, |vp_ctx, _class| {
            self.doc_shots.poll(vp_ctx, self.debug.doc_screenshots);
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
        // Spec 044 R20 — the service wrapper allows registered External Crates.
        use crate::external_crates_service::analyze_project as analyze;
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
        self.output.clear_run_output();
        self.output.push_status(format!(
            "── Checking {} ──",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("?")
        ));
        self.editor.clear_diags();

        use crate::runner::{DiagMsg, DiagSeverity, RunMsg};
        use cobolt_lexer::{tokenize, SourceFormat};
        use cobolt_parser::parse;
        // Spec 044 R20 — the service wrapper allows registered External Crates.
        use crate::external_crates_service::analyze_project as analyze;

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

    /// Toolbar Save / Ctrl+S. Persists everything `has_unsaved_changes` counts —
    /// every dirty editor tab, every dirty form designer (each regenerating its
    /// COBOL), and the project Settings form. Action and enablement predicate
    /// must stay in step, otherwise the Save button can never switch itself off.
    fn do_save(&mut self) {
        self.save_all_unsaved();
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
        self.llm = crate::llm::LlmConfig::load();
        proj.ai.apply_to_llm(&mut self.llm);

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
                    self.ensure_project_agent_system(&dir);
                    self.sync_project_documentation_membership(&dir);
                    self.do_save_project(); // persist the tracked main

                    // Initialize the project Settings form and show it immediately
                    // (fills the main work area right of the tree; no editor controls on top).
                    if let Some(p) = &self.cobolt_project {
                        self.settings_form = Some(crate::panels::settings_form::SettingsForm::new(
                            p, &self.llm,
                        ));
                        self.show_project_settings = true;
                        self.show_grace_chat = false;
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
            Ok(mut proj) => {
                let dir = path.parent().map(|p| p.to_owned());
                let migrated_ai = self.activate_project_ai(&mut proj);
                self.output
                    .push_status(format!("Opened project '{}'", proj.project.name));
                self.cobolt_project = Some(proj);
                self.project_path = Some(path);
                self.agents_modal = None;
                self.models_modal = None;
                if migrated_ai {
                    self.do_save_project();
                    self.output.push_status(
                        "Imported legacy AI models into this project's settings.".to_string(),
                    );
                }

                // Load persisted "raw editor preferred" for indexed files from the
                // IDE-managed indexed state file in the project's data/ (dog-fooding
                // the same mechanism used for agent conversation history).
                if let Some(root) = dir.as_ref() {
                    self.ensure_project_agent_system(root);
                    self.sync_project_documentation_membership(root);
                    let data_dir = root.join("data");
                    let rels = crate::llm::load_raw_preferred_indexed(&data_dir);
                    self.raw_preferred_indexed =
                        rels.into_iter().map(|rel| root.join(rel)).collect();
                    // Invite the user to configure AI when this project has no
                    // usable model / agent — unless they asked not to be asked.
                    let suppressed = self
                        .cobolt_project
                        .as_ref()
                        .map(|p| p.ide.hide_ai_setup_prompt)
                        .unwrap_or(false);
                    self.ai_setup_modal = !suppressed && self.ai_setup_needed(root);
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
                    self.show_grace_chat = false;
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
                // Spec 037 R3 — exactly one main form per project.
                self.apply_main_form_invariant();
                // A project of an older shape is offered its upgrades — after
                // the R3 repair, so an ambiguous designation is settled before
                // the seal upgrade is asked whether it applies.
                self.detect_project_upgrades();
            }
            Err(e) => {
                self.output.push_status(format!(
                    "Failed to open project: {e}. Make sure you selected a valid project file (.toml with a [project] table)."
                ));
            }
        }
    }

    /// Activate one project's AI settings over the machine-local credential
    /// store. Projects predating the `[ai]` table receive a one-time copy of
    /// legacy global model metadata; the legacy source remains untouched.
    fn activate_project_ai(&mut self, project: &mut CoboltProject) -> bool {
        let mut llm = crate::llm::LlmConfig::load();
        let migrated = project.ai.schema_version == 0;
        if migrated {
            project.ai = crate::project_model::ProjectAiSettings::from_llm(&llm);
        }
        project.ai.apply_to_llm(&mut llm);
        llm.repair_project_profiles();
        self.llm = llm;
        // The board follows the project's models (spec 040): opening a project
        // lists its models straight away, and replays any proficiency reports
        // this project archived before the board existed.
        self.sync_leaderboard_models();
        migrated
    }

    /// Persist the active project's non-secret AI configuration alongside the
    /// project and merge its credentials into the machine-local secret store.
    fn persist_active_project_ai(&mut self) {
        if let Some(project) = &mut self.cobolt_project {
            project.ai = crate::project_model::ProjectAiSettings::from_llm(&self.llm);
        } else {
            return;
        }
        if let Err(e) = self.llm.save() {
            tracing::warn!("could not save machine-local AI credentials: {e}");
            self.output
                .push_status(format!("Could not save AI credentials: {e}"));
        }
        self.do_save_project();
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
    /// An ordinary, incremental build.
    fn do_build_binary(&mut self) {
        self.do_build_binary_with(false);
    }

    /// The Build button (toolbar and the File menu item).
    ///
    /// A project the running PowerRustCOBOL has never fully built gets the
    /// FULL build. An incremental one there produces a binary the IDE itself
    /// refuses to trust: it leaves the version stamp untouched, so the next
    /// Run still asks for a full build and the incremental minutes are spent
    /// twice over — which is exactly what an operator hit (2026-08-06).
    ///
    /// The decision lives here rather than in [`Self::do_build_binary`] so the
    /// EXEC RUST auto-build inside Run stays incremental: reaching that one
    /// means the developer already answered the stale prompt with "Run
    /// anyway", and silently full-building would overrule the answer they
    /// just gave.
    fn do_build_binary_button(&mut self) {
        let full = self.build_is_stale();
        self.do_build_binary_with(full);
    }

    /// Build, optionally discarding every cached artefact first.
    ///
    /// A full build is what answers "it behaves oddly since I updated": the
    /// generated sources are regenerated every time, but cargo's own artefacts
    /// survive across PowerRustCOBOL upgrades, so an incremental build can link
    /// objects produced by an older version against newly generated code. It is
    /// also the only build that stamps [`ProjectMeta::built_with_version`],
    /// because it is the only one that can promise nothing older survived.
    fn do_build_binary_with(&mut self, full: bool) {
        self.pending_build_full = full;
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
            self.output.clear_run_output();
            self.output
                .push_status("── Build blocked: fix these code errors first ──");
            for (path, msg) in &bad_forms {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("form");
                self.output.push_status(format!("  ✗ {name}: {msg}"));
            }
            // Name them HERE, in the modal. The Output panel has carried the
            // list all along, but the modal is what the developer is looking
            // at, and "a form has errors" with no name is a search rather than
            // a fix — on a project with a dozen forms there is nothing to go
            // on. Long lists stay bounded; the panel still has all of them.
            const SHOWN: usize = 6;
            let mut lines: Vec<String> = bad_forms
                .iter()
                .take(SHOWN)
                .map(|(path, msg)| {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("form");
                    format!("  • {name}: {msg}")
                })
                .collect();
            if bad_forms.len() > SHOWN {
                lines.push(format!("  … and {} more", bad_forms.len() - SHOWN));
            }
            self.set_form_error(format!(
                "Build blocked — {} form(s) have code errors that must be fixed first:\n\n{}",
                bad_forms.len(),
                lines.join("\n")
            ));
            return;
        }

        self.output.clear_run_output();
        self.output
            .push_status("── Building binary …  (this may take a minute) ──");
        // A full build discards every cached artefact, so it takes minutes
        // longer than an incremental one. Say why it is happening — otherwise
        // a Build that used to be quick just looks stuck.
        if full {
            let last = self
                .cobolt_project
                .as_ref()
                .map(|p| p.project.built_with_display().to_owned())
                .unwrap_or_else(|| "never fully built".to_owned());
            let msg = self
                .lang
                .tr()
                .status_build_full_stale
                .replace("{last}", &last);
            self.output.push_status(msg);
        }

        // Run the build on a background thread; collect result via a one-shot
        // channel, and stream phase progress via a second channel.
        let (tx, rx) = std::sync::mpsc::channel::<Result<cobolt_compiler::BuildResult, String>>();
        let (ptx, prx) = std::sync::mpsc::channel::<cobolt_compiler::BuildProgress>();
        std::thread::spawn(move || {
            let opts = BuildOptions {
                verbose: false,
                workspace_root: None,
                progress: Some(ptx),
                // Host only — there is no cross-compilation (spec 041 R17).
                target: None,
                full,
            };
            let result = build_project(&manifest, &opts).map_err(|e| e.to_string());
            let _ = tx.send(result);
        });

        // Poll both channels each frame; store the receivers so update() can drain them.
        self.pending_build_rx = Some(rx);
        self.pending_build_progress = Some(prx);
        self.build_phase = (0.0, "Starting…".to_string());
        self.build_modal_closed = false;
        // Hosted by the IDE main window unless the caller (a designer's Run
        // Form) claims it right after this returns.
        self.build_modal_host = None;
        self.build_outcome = None;
        self.build_log.clear();
        self.build_details_open = false;
        self.build_log_shown = 0;
        self.build_log_last_reveal = None;
    }

    /// File → Reindex Knowledge Bases: run the same incremental sync a Grace
    /// workflow performs at start (System KB always, Project KB when a project
    /// is open) on a worker thread, streaming coarse progress to the Output
    /// panel. The menu item stays disabled while the worker runs.
    fn do_reindex_kb(&mut self) {
        if self.kb_reindex_rx.is_some() {
            return;
        }
        let project_dir = self.project_dir();
        let tr = self.lang.tr();
        self.output.push_status(tr.status_kb_reindex_started.to_owned());
        let (tx, rx) = std::sync::mpsc::channel::<KbReindexMsg>();
        std::thread::spawn(move || {
            // Per-record phase updates drive the modal's determinate bar; the
            // UI drain keeps only the latest, so no throttling is needed.
            let ptx = tx.clone();
            let mut on_progress = move |done: usize, total: usize, subject: &str| {
                if total == 0 {
                    return;
                }
                let _ = ptx.send(KbReindexMsg::Phase(
                    done as f32 / total as f32,
                    format!("{done}/{total} — {subject}"),
                ));
            };
            let result = crate::grace_host::reindex_knowledge_bases(
                project_dir.as_deref(),
                &mut on_progress,
            );
            let _ = tx.send(KbReindexMsg::Done(result));
        });
        self.kb_reindex_rx = Some(rx);
        self.kb_reindex_phase = (0.0, String::new());
        self.kb_reindex_modal_hidden = false;
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
    fn agent_bar(&mut self, panel_ui: &mut egui::Ui, tr: &crate::i18n::Tr) {
        // Panels are Ui-hosted since egui 0.35; everything else in this
        // method still wants a Context.
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

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

        let grace_running = self.grace_session.as_ref().is_some_and(|s| s.is_running());
        let busy = self.agent_pending.is_some() || grace_running;
        let mut prompt = std::mem::take(&mut self.agent_prompt);
        let mut use_grace = self.use_grace;
        // Spec 036 R4/R10: this surface renders the typed live-action stream,
        // never the raw progress log (which may carry payloads under verbose).
        let grace_actions: Vec<crate::agent_actions::AgentAction> = self
            .grace_session
            .as_ref()
            .map(|s| s.actions.clone())
            .unwrap_or_default();
        let grace_current_action = self
            .grace_session
            .as_mut()
            .and_then(|s| s.current_action().cloned());
        let grace_indexing = self
            .grace_session
            .as_ref()
            .and_then(|s| s.indexing_progress());
        let grace_done = self.grace_session.as_ref().and_then(|s| {
            s.finished().map(|r| match r {
                Ok((rec, path)) => {
                    format!(
                        "Workflow {}: {} · saved to {}",
                        rec.workflow_id,
                        rec.status,
                        path.display()
                    )
                }
                Err(e) => format!("Grace workflow failed: {e}"),
            })
        });
        // A gated git op (push, rebase…) awaiting the operator's decision (R12).
        let grace_confirm: Option<String> = self
            .grace_session
            .as_ref()
            .and_then(|s| s.pending_confirm())
            .map(|r| r.command.clone());
        let mut do_grace_confirm: Option<bool> = None;
        let mut do_grace_stop = false;
        let grace_stop_requested = self
            .grace_session
            .as_ref()
            .is_some_and(|s| s.stop_requested());
        // Live (input, output) token totals, updated as each model returns.
        let grace_tokens: Option<(u64, u64)> = self
            .grace_session
            .as_ref()
            .map(|s| s.token_totals())
            .filter(|(input, output)| *input > 0 || *output > 0);
        let status = self.agent_status.clone();
        let preview = self.agent_preview.clone();
        let has_debug = crate::llm::has_connection_log();
        let mut do_send = false;
        let mut do_approve = false;
        let mut do_reject = false;
        let mut do_details = false;
        let mut do_grace_dismiss = false;

        let frame = crate::theme::glass_panel_frame(
            ctx.global_style().visuals.panel_fill,
            &crate::theme::active(),
        );
        egui::Panel::top("inspector_agent")
            .frame(frame)
            .show(panel_ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("🤖").size(15.0));
                    ui.label(egui::RichText::new(tr.agent_mode).small().strong());
                    // 👑 Grace: route this request through the multi-agent
                    // workflow (plan → delegate → pedantic review → integrate).
                    use_grace = true;
                    ui.add_enabled(false, egui::Checkbox::new(&mut use_grace, "👑"))
                        .on_hover_text(tr.agent_use_grace_hint);
                    if busy {
                        ui.add(egui::Spinner::new());
                        // Stop sign, shown only while the spinner is: halts
                        // Grace/agents when the in-flight call returns.
                        if grace_running {
                            if grace_stop_requested {
                                ui.label(
                                    egui::RichText::new("Stopping…")
                                        .small()
                                        .color(Color32::from_gray(170)),
                                );
                            } else if ui
                                .button(egui::RichText::new("🛑").size(14.0))
                                .on_hover_text("Stop Grace and the agents")
                                .clicked()
                            {
                                do_grace_stop = true;
                            }
                        }
                        ui.label(
                            egui::RichText::new(tr.agent_hint)
                                .small()
                                .color(Color32::from_gray(170)),
                        );
                    }
                });
                ui.horizontal(|ui| {
                    let prompt_width = crate::panels::chat_prompt_width(
                        ui.available_width(),
                        ui.spacing().item_spacing.x,
                    );
                    let resp = ui.add_sized(
                        [prompt_width, ui.spacing().interact_size.y],
                        egui::TextEdit::singleline(&mut prompt)
                            .hint_text(tr.agent_hint)
                            .interactive(!busy),
                    );
                    if resp.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && !prompt.trim().is_empty()
                        && !busy
                    {
                        do_send = true;
                    }
                    let can_send = !busy && !prompt.trim().is_empty();
                    if ui
                        .add_enabled(
                            can_send,
                            egui::Button::new(tr.ai_send).min_size(egui::vec2(
                                crate::panels::CHAT_SEND_BUTTON_WIDTH,
                                ui.spacing().interact_size.y,
                            )),
                        )
                        .clicked()
                    {
                        do_send = true;
                    }
                });
                // Up-to-date token usage, refreshed as each model returns.
                if let Some((tokens_in, tokens_out)) = grace_tokens {
                    ui.label(
                        egui::RichText::new(format!(
                            "Tokens: {tokens_in} in / {tokens_out} out"
                        ))
                        .small()
                        .color(Color32::from_gray(170)),
                    );
                }

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

                // 👑 Grace workflow progress (spec 029 Phase C, spec 036):
                // the collapsed action history plus the throttled
                // current-action line — same helpers as the project Grace
                // chat, so both surfaces behave identically (R10).
                if !grace_actions.is_empty() {
                    ui.separator();
                    ui.label(egui::RichText::new(tr.agent_grace_progress).strong());
                    egui::ScrollArea::vertical()
                        .max_height(160.0)
                        .auto_shrink([false, true])
                        .id_salt("grace_progress_log")
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            crate::panels::editor::chat_action_history(
                                ui,
                                egui::Id::new("form_inspector_grace_actions"),
                                &grace_actions,
                                tr,
                                13.0,
                            );
                            if grace_running {
                                match &grace_current_action {
                                    Some(action) => crate::panels::editor::chat_current_action(
                                        ui,
                                        action,
                                        tr,
                                        13.0,
                                        grace_tokens,
                                    ),
                                    None => crate::panels::editor::chat_thinking_indicator(
                                        ui,
                                        tr.ai_thinking,
                                        13.0,
                                        grace_tokens,
                                    ),
                                }
                                if let Some((done, total, _)) = &grace_indexing {
                                    crate::panels::editor::chat_indexing_bar(
                                        ui, *done, *total, tr, 13.0,
                                    );
                                }
                            }
                        });
                    if let Some(summary) = &grace_done {
                        ui.label(
                            egui::RichText::new(summary)
                                .small()
                                .color(Color32::from_rgb(125, 214, 160)),
                        );
                        if ui.small_button(tr.agent_grace_dismiss).clicked() {
                            do_grace_dismiss = true;
                        }
                    }

                    // Gated git op awaiting Approve/Deny (spec 030 R12).
                    if let Some(cmd) = &grace_confirm {
                        ui.separator();
                        ui.label(egui::RichText::new(tr.agent_git_confirm).strong());
                        ui.label(egui::RichText::new(cmd).monospace().small());
                        ui.horizontal(|ui| {
                            if ui.button(tr.agent_approve).clicked() {
                                do_grace_confirm = Some(true);
                            }
                            if ui.button(tr.agent_reject).clicked() {
                                do_grace_confirm = Some(false);
                            }
                        });
                    }
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
        self.use_grace = use_grace;

        if do_grace_dismiss {
            self.grace_session = None;
        }

        // Answer a pending gated git op (spec 030 R12).
        if let Some(approved) = do_grace_confirm {
            if let Some(sess) = self.grace_session.as_mut() {
                sess.respond_confirm(approved);
            }
            ctx.request_repaint();
        }

        // Stop the running Grace workflow at the developer's request.
        if do_grace_stop {
            if let Some(sess) = self.grace_session.as_ref() {
                sess.stop();
            }
            ctx.request_repaint();
        }

        // 👑 Grace routing: hand the request to the multi-agent workflow.
        if do_send && !busy && self.use_grace {
            let sent = std::mem::take(&mut self.agent_prompt);
            match self.project_dir() {
                Some(dir) => {
                    self.agent_status = None;
                    self.agent_preview = None;
                    self.grace_applied = false;
                    let form = self
                        .inspect
                        .as_ref()
                        .map(|state| state.designer.form.clone());
                    let context = form
                        .as_ref()
                        .map(|form| {
                            crate::agent::build_context_with_project(
                                form,
                                self.cobolt_project.as_ref(),
                                Some(dir.as_path()),
                            )
                        })
                        .unwrap_or_default();
                    self.grace_session =
                        Some(crate::grace_session::GraceSession::spawn_with_context(
                            &dir,
                            &self.llm,
                            &sent,
                            crate::grace_host::GraceRoutingContext::new(
                                "Form inspector chatbot",
                                Some(crate::agents_db::FORM_DESIGNER),
                                context,
                            ),
                        ));
                    ctx.request_repaint();
                }
                None => {
                    self.agent_status = Some(tr.agent_grace_no_project.to_string());
                    self.agent_prompt = sent;
                }
            }
            do_send = false;
        }

        if do_send && !busy {
            let form = self.inspect.as_ref().unwrap().designer.form.clone();
            let project_dir = self.project_dir();
            let context = crate::agent::build_context_with_project(
                &form,
                self.cobolt_project.as_ref(),
                project_dir.as_deref(),
            );
            let (sys, skills) = match &dir {
                Some(d) => (
                    crate::agent::effective_prompt(d),
                    crate::agent::load_skills(d),
                ),
                None => (crate::agent::effective_prompt(Path::new("")), String::new()),
            };
            let sent = std::mem::take(&mut self.agent_prompt);
            // Spec 028 R8: the Form Designer Agent DB entry (when present)
            // overrides the legacy connection for the designer flow.
            let eff_llm = self.designer_effective_llm();
            let rx = crate::llm::spawn_agent_request(
                &eff_llm,
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

    /// Apply a finished Grace workflow's approved Form-Designer output to the
    /// originating form (spec 030 R6/R7). Runs once per finished session; each
    /// approved change-set goes through the existing validated, undoable
    /// `apply_agent_change_set` path — an all-invalid change-set applies nothing
    /// and leaves the form unchanged (R8). Appends a status line to the session
    /// log so the developer sees the outcome.
    fn apply_grace_form_output(&mut self) {
        if self.grace_applied {
            return;
        }
        let record = match self.grace_session.as_ref().and_then(|s| s.finished()) {
            Some(Ok((rec, _))) => rec.clone(),
            Some(Err(_)) => {
                self.grace_applied = true; // failed run: nothing to apply
                return;
            }
            None => return, // still running
        };
        self.grace_applied = true;

        let sets =
            crate::grace_host::approved_form_change_sets(&record, crate::agents_db::FORM_DESIGNER);
        if sets.is_empty() {
            return;
        }
        let mut notes: Vec<String> = Vec::new();
        let mut saved_path: Option<PathBuf> = None;
        if let Some(st) = self.inspect.as_mut() {
            for set in sets {
                match set {
                    Ok(cs) => {
                        // Name what will be skipped BEFORE applying — a silently
                        // discarded handler is exactly how a workflow reports
                        // success while the form gains no events.
                        let discarded = crate::agent::discarded_ops(&cs, &st.designer.form);
                        let n = st.designer.apply_agent_change_set(&cs);
                        if !discarded.is_empty() {
                            notes.push(format!(
                                "⚠ {} operation(s) from Grace could not be applied and were discarded:\n  • {}",
                                discarded.len(),
                                discarded.join("\n  • ")
                            ));
                        }
                        if n > 0 {
                            let _ = save_form(&st.designer.form, &st.path);
                            st.designer.dirty = false;
                            st.mtime = file_mtime(&st.path);
                            saved_path = Some(st.path.clone());
                            notes.push(format!("✎ applied {n} form change(s) from Grace."));
                        } else {
                            notes.push(
                                "⚠ Grace's approved form change-set had no applicable operations; the form is unchanged.".into(),
                            );
                        }
                    }
                    Err(e) => notes.push(format!("⚠ Grace's form output was not applicable: {e}")),
                }
            }
        } else {
            notes
                .push("⚠ Grace produced form changes but no form is open to apply them to.".into());
        }
        if let Some(p) = saved_path {
            self.project.refresh_form(&p);
        }
        if let Some(sess) = self.grace_session.as_mut() {
            sess.log.extend(notes);
        }
    }

    /// The project's root directory (where `cobolt.toml` lives), if a project is open.
    fn project_dir(&self) -> Option<PathBuf> {
        self.project_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_owned())
    }

    /// True when this project has no usable AI model, or Grace has none — the cue
    /// to invite the user to set them up on open. Call AFTER
    /// [`Self::ensure_project_agent_system`] so the fixed agents already exist.
    /// The request CONTEXT the project-wide Grace chatbot hands to Grace.
    ///
    /// With a form open this is the same block the designer's own AI panel
    /// sends — control inventory, per-type property keys, control API, project
    /// tree — so a request typed in the project chat ("add a data bound datagrid
    /// to form X") reaches the delegated specialist with real ids, geometry and
    /// property names. With no form open it is the project tree alone, which is
    /// still what lets Grace name real forms, indexed files and sources rather
    /// than inventing them.
    fn grace_chat_surface_context(&self) -> String {
        let dir = self.project_dir();
        match self.inspect.as_ref() {
            Some(state) => crate::agent::build_context_with_project(
                &state.designer.form,
                self.cobolt_project.as_ref(),
                dir.as_deref(),
            ),
            None => crate::agent::build_project_tree_context(
                self.cobolt_project.as_ref(),
                dir.as_deref(),
            ),
        }
    }

    // ── Error surfaces (operator, 2026-08-09) ────────────────────────────
    //
    // Every error shown to the developer is also written to the Output panel.
    // A dialog is dismissed and the text goes with it; the console is what can
    // still be scrolled back to and pasted into a bug report. These three
    // write straight to the console because the app owns it — the panels that
    // cannot reach it record through `crate::error_log` instead, and the frame
    // loop drains that into the same place.

    /// Show the build/form error dialog, and record it.
    fn set_form_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.output.push_status(format!("✗ {message}"));
        self.form_error = Some(message);
    }

    /// Show the general alert dialog, and record it.
    fn set_alert_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.output.push_status(format!("✗ {message}"));
        self.alert_error = Some(message);
    }

    /// Show a model connection/proficiency failure, and record it.
    fn set_llm_test_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.output.push_status(format!("✗ {message}"));
        self.llm_test_error = Some(message);
    }

    fn ai_setup_needed(&self, project_root: &Path) -> bool {
        let db = crate::agents_db::AgentsDb::load(project_root);
        ai_setup_needed_for(&self.llm, &db.agents)
    }

    fn ensure_project_agent_system(&mut self, project_root: &Path) {
        let mut db = crate::agents_db::AgentsDb::load(project_root);
        let changed = db.ensure_fixed_agents(&self.llm);
        if changed > 0 {
            self.output.push_status(format!(
                "Prepared {changed} fixed-agent or project-knowledge capability update(s)."
            ));
        }
        // Retire the model-profile layer before anything resolves a connection
        // (spec 048 R24). Asks nothing, blocks nothing — but every credential it
        // has to drop is named, because a key that quietly disappeared is a
        // support call nobody can answer.
        self.migrate_model_profiles(&mut db);
    }

    /// Run the spec 048 profiles→providers migration and report it (R24–R27).
    ///
    /// Silent and promptless by design: on an already-migrated project this is
    /// a no-op that prints nothing, so the Output panel only ever mentions it
    /// on the one open where it actually did something.
    fn migrate_model_profiles(&mut self, db: &mut crate::agents_db::AgentsDb) {
        let report = db.migrate_profiles_to_providers(&mut self.llm);
        if report.is_empty() {
            return;
        }
        if report.agents_migrated > 0 {
            self.output.push_status(format!(
                "Model providers: moved {} agent(s) off model profiles onto their own settings.",
                report.agents_migrated
            ));
        }
        if !report.providers_created.is_empty() {
            self.output.push_status(format!(
                "Model providers: configured {}.",
                report.providers_created.join(", ")
            ));
        }
        for (provider, label) in &report.discarded {
            self.output.push_status(format!(
                "Model providers: {provider} keeps one key — the credential from \"{label}\" was \
                 dropped. Re-enter it in the Model Providers Manager if it was the one you wanted."
            ));
        }
        for (provider, endpoint) in &report.endpoint_conflicts {
            self.output.push_status(format!(
                "Model providers: {provider} had more than one endpoint on file; kept {endpoint}."
            ));
        }
        for agent in &report.dangling {
            self.output.push_status(format!(
                "Model providers: {agent} referenced a model profile that no longer exists and \
                 now has no model."
            ));
        }
        // The direct AI surfaces read the top-level model, which profiles used
        // to seed (spec 048 T4).
        self.llm.ensure_default_model_from_agents(db);
        if let Err(error) = self.llm.save() {
            self.output
                .push_status(format!("Could not save model providers: {error}"));
        }
    }

    fn sync_project_documentation_membership(&mut self, project_root: &Path) {
        match cobolt_agents::project_knowledge::ensure_knowledge_base(project_root) {
            Ok(moved) if moved > 0 => self.output.push_status(format!(
                "Migrated {moved} legacy documentation file(s) into the project Knowledge Base."
            )),
            Ok(_) => {}
            Err(error) => {
                self.output
                    .push_status(format!("Could not prepare project Knowledge Base: {error}"));
                return;
            }
        }
        let paths = match cobolt_agents::project_knowledge::documentation_paths(project_root) {
            Ok(paths) => paths,
            Err(error) => {
                self.output
                    .push_status(format!("Could not scan project Knowledge Base: {error}"));
                return;
            }
        };
        let Some(project) = self.cobolt_project.as_mut() else {
            return;
        };
        let before = project.files.documentation.clone();
        project.files.documentation.retain(|relative| {
            let first =
                Path::new(relative)
                    .components()
                    .next()
                    .and_then(|component| match component {
                        std::path::Component::Normal(part) => part.to_str(),
                        _ => None,
                    });
            let managed = first.is_some_and(|part| {
                part.eq_ignore_ascii_case("Knowledge Base")
                    || part.eq_ignore_ascii_case("Documentation")
                    || part.eq_ignore_ascii_case("docs")
            });
            !managed || project_root.join(relative).exists()
        });
        for relative in paths {
            project.add_file_to(&relative, crate::project_model::Category::Documentation);
        }
        if project.files.documentation != before {
            let count = project.files.documentation.len();
            self.do_save_project();
            self.output.push_status(format!(
                "Project Knowledge Base refreshed: {count} tracked file(s)."
            ));
        }
    }

    fn sync_project_indexed_membership(&mut self, project_root: &Path) {
        let indexed_dir = project_root.join("indexed");
        let mut definitions = Vec::new();
        if indexed_dir.exists() {
            match std::fs::read_dir(&indexed_dir) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && path
                                .extension()
                                .and_then(|extension| extension.to_str())
                                .is_some_and(|extension| extension.eq_ignore_ascii_case("cidx"))
                        {
                            if let Some(relative) = relative_to(&path, project_root) {
                                definitions.push(relative);
                            }
                        }
                    }
                }
                Err(error) => {
                    self.output
                        .push_status(format!("Could not scan project indexed files: {error}"));
                    return;
                }
            }
        }
        definitions.sort();
        let Some(project) = self.cobolt_project.as_mut() else {
            return;
        };
        let before_indexed = project.files.indexed.clone();
        let before_generated = project.files.generated.clone();
        project.files.indexed.retain(|relative| {
            !relative.starts_with("indexed/") || project_root.join(relative).exists()
        });
        for relative in &definitions {
            project.add_file_to(relative, crate::project_model::Category::IndexedFiles);
            let stem = Path::new(relative)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("indexed");
            let generated = format!("generated/{stem}-indexed.cbl");
            if project_root.join(&generated).exists() {
                project.add_generated(&generated);
            }
        }
        if project.files.indexed != before_indexed || project.files.generated != before_generated {
            let count = project.files.indexed.len();
            self.do_save_project();
            self.output.push_status(format!(
                "Project indexed files refreshed: {count} tracked file(s)."
            ));
        }
        if let Some(inspector) = self.indexed_inspect.as_mut() {
            if let Ok(definition) = load_indexed(&inspector.path) {
                inspector.def = definition;
            }
        }
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
            let msg = format!(
                "Cannot {action}: form COBOL ID '{form_name}' is already used by {}.",
                conflict.display()
            );
            self.output.push_status(msg.clone());
            self.set_alert_error(msg);
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

    /// `+` on a category — **create** a new item of that kind, in the category root.
    fn do_create_in_category(&mut self, kind: FileKind) {
        self.create_of_kind(kind, None)
    }

    /// `+` on a folder row — create a new item of that kind directly inside
    /// `dir_rel` (project-relative). Same dialogs and same templates as the
    /// category `[+]`; only the destination directory differs.
    fn do_create_in_folder(&mut self, kind: FileKind, dir_rel: &Path) {
        let dir = crate::project_fs::rel_string(dir_rel);
        self.create_of_kind(kind, Some(dir))
    }

    /// Create a new item of `kind`. `dir_rel` is the project-relative destination
    /// folder; `None` means the category's root subdir. Carrying the destination
    /// as state (rather than a parameter on each creator) is what lets the form
    /// and indexed-file dialogs — which return asynchronously, after the user
    /// fills them in — still land in the folder whose `[+]` was clicked.
    fn create_of_kind(&mut self, kind: FileKind, dir_rel: Option<String>) {
        match kind {
            // A form has a real "create" dialog.
            FileKind::Form => {
                self.new_form.target_dir = dir_rel;
                self.new_form.open = true;
            }
            FileKind::Indexed => {
                self.new_indexed_file_dialog();
                self.new_indexed_dir = dir_rel;
            }
            FileKind::Source => self.create_new_text_file(FileKind::Source, dir_rel),
            FileKind::Documentation => {
                self.create_new_text_file(FileKind::Documentation, dir_rel)
            }
            // Assets can't be authored in the IDE — creating one means importing.
            FileKind::Asset => self.do_add_file_to_project(FileKind::Asset),
        }
    }

    /// Create a new editable text file (COBOL source or documentation) in the
    /// project, with a starter template, then track it and open it in the editor.
    ///
    /// `dir_rel` is the project-relative destination folder; `None` puts the file
    /// in the category's root subdir.
    fn create_new_text_file(&mut self, kind: FileKind, dir_rel: Option<String>) {
        use crate::project_model::Category;
        let Some(dir) = self.project_dir() else {
            self.output.push_status("Save the project first.");
            return;
        };
        let (root_sub, base, ext, category) = match kind {
            FileKind::Source => ("src", "new-program", "cbl", Category::CommonCode),
            _ => (
                cobolt_agents::project_knowledge::KNOWLEDGE_BASE_ROOT,
                "new-document",
                "md",
                Category::Documentation,
            ),
        };
        // New Knowledge Base document: offer the semantic model download
        // (confirmation dialog) when it is not installed.
        if category == Category::Documentation {
            self.offer_semantic_model_for_kb();
        }
        let sub = dir_rel.as_deref().unwrap_or(root_sub);
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
                "Knowledge Base documents",
                &["md", "markdown", "txt", "rst", "adoc", "pdf", "html", "htm"],
            ),
            FileKind::Asset => spec, // no filter → every file selectable
        };

        self.begin_file_dialog(FileRequest::AddFile(kind), spec);
    }

    /// Add the chosen file to the open project under `kind`'s category. A file
    /// **outside** the project directory is **copied into** a category subfolder
    /// (`src/`, `forms/`, `assets/`, `Knowledge Base/`) so it becomes part of the project
    /// (and ships with the build); a file already inside is tracked in place.
    fn add_file_to_project_path(&mut self, kind: FileKind, path: PathBuf) {
        if kind == FileKind::Indexed {
            self.import_indexed_data_file(path);
            return;
        }
        // A document entering the Knowledge Base is the moment semantic search
        // starts mattering: offer the model download (confirmation dialog) when
        // it is not installed. The add itself proceeds either way.
        if kind == FileKind::Documentation {
            self.offer_semantic_model_for_kb();
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
                    FileKind::Asset => "Assets",
                    FileKind::Documentation => {
                        cobolt_agents::project_knowledge::KNOWLEDGE_BASE_ROOT
                    }
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
        if kind == FileKind::Asset {
            self.show_project_settings = false;
            self.inspect = None;
            self.indexed_inspect = None;
            self.open_asset_preview(proj_dir.join(&rel));
        }
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

    fn load_form_from_path(&mut self, path: PathBuf) {
        if self.designers.iter().any(|(p, _)| p == &path) {
            self.designer_activation_requests.request(path);
            self.egui_ctx.request_repaint();
            return;
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
                self.designer_activation_requests.request(path.clone());
                self.designers.push((path, dp));
                self.egui_ctx.request_repaint();
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
        //
        // Procedures are deliberately NOT swept here. Save must write what the
        // developer wrote; a procedure created minutes ago has no caller yet,
        // and sweeping on Save deleted it for exactly that.
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
                // Flash a checkmark on the Save button for ~1s instead of a
                // modal. Keyed by path so this designer's button flashes.
                self.save_flash = Some((
                    path.clone(),
                    std::time::Instant::now() + std::time::Duration::from_millis(1000),
                ));
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
    fn show_inspector(&mut self, panel_ui: &mut egui::Ui, tr: &Tr) {
        // Panels are Ui-hosted since egui 0.35; everything else in this
        // method still wants a Context.
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        let mut open_designer = false;
        let mut close = false;
        let mut changed = false;
        // Collected inside the closure that borrows `self.inspect`; applied after.
        let mut pending_proc_delete: Option<PendingProcDelete> = None;

        // Live-refresh from disk before drawing so a Designer save (or any
        // external write) of this form is reflected in the Main-Pane inspector.
        if let Some(st) = &mut self.inspect {
            st.reload_if_stale();
        }

        // Dev-agent prompt bar (spec 025 T10) — the inspector has the live form, so
        // the agent can propose control/property/handler/procedure changes that the
        // developer previews and approves.
        if self.llm.is_configured() && self.inspect.is_some() {
            self.agent_bar(panel_ui, tr);
        }

        let card = crate::theme::glass_panel_frame(
            ctx.global_style().visuals.panel_fill,
            self.current_theme(),
        );
        egui::CentralPanel::default()
            .frame(card)
            .show(panel_ui, |ui| {
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
                    // Publish the form's surface theme before the inspector
                    // draws. The colour picker offers the ACTIVE theme's
                    // palette, read from the context — and this surface never
                    // published one, so it fell back to Liquid Glass, which
                    // supplies no swatches at all. The picker therefore opened
                    // with an empty grid here while the same control's picker
                    // on the designer canvas showed Elegance's 24 colours.
                    cobolt_forms::paint::set_surface_theme(
                        ui.ctx(),
                        d.active_surface_theme.clone(),
                    );
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
                    // Ask first — and route through the undoable command, which
                    // the direct `Vec::remove` here used to bypass entirely.
                    if let Some(p) = st.designer.form.user_procedures.get(i) {
                        pending_proc_delete = Some(PendingProcDelete {
                            designer: None,
                            index: i,
                            name: p.name.clone(),
                            lines: p.code.lines().filter(|l| !l.trim().is_empty()).count(),
                        });
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

        if pending_proc_delete.is_some() {
            self.pending_proc_delete = pending_proc_delete;
        }

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
        // A form save can be the moment a MainForm claim reaches disk, so the
        // sealed copy in the project file is restated here too.
        self.reseal_project_designation();

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

    /// The tracked `generated/` entry whose file name is `file_name`, if the user
    /// relocated it into a subfolder (spec 033, R7). Lets regenerate rewrite the
    /// moved file in place instead of resurrecting it at the default path.
    fn tracked_generated_rel(&self, file_name: &str) -> Option<String> {
        tracked_generated_rel(self.cobolt_project.as_ref(), file_name)
    }

    /// Path for a form's generated `.cbl`: the tracked (possibly relocated) entry
    /// when one exists, else the project's `generated/` folder, else next to the
    /// `.cfrm`.
    fn generated_cbl_path(&self, cfrm: &std::path::Path) -> PathBuf {
        let stem = cfrm.file_stem().and_then(|s| s.to_str()).unwrap_or("form");
        let file_name = format!("{stem}.cbl");
        if let Some(dir) = self.project_path.as_ref().and_then(|p| p.parent()) {
            if let Some(rel) = self.tracked_generated_rel(&file_name) {
                return dir.join(rel);
            }
            return dir.join("generated").join(&file_name);
        }
        cfrm.with_extension("cbl")
    }

    fn generated_indexed_cbl_path(&self, cidx: &std::path::Path) -> PathBuf {
        let stem = cidx
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("indexed");
        let file_name = format!("{stem}-indexed.cbl");
        if let Some(dir) = self.project_path.as_ref().and_then(|p| p.parent()) {
            if let Some(rel) = self.tracked_generated_rel(&file_name) {
                return dir.join(rel);
            }
            return dir.join("generated").join(&file_name);
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
        // The folder whose [+] raised the dialog, or indexed/ from the category header.
        let sub = self
            .new_indexed_dir
            .clone()
            .unwrap_or_else(|| "indexed".to_string());
        let sub_dir = dir.join(&sub);
        if let Err(e) = std::fs::create_dir_all(&sub_dir) {
            self.output
                .push_status(format!("Could not create {sub}/: {e}"));
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

        let rel = format!("{sub}/{fname}");
        if let Some(proj) = &mut self.cobolt_project {
            use crate::project_model::Category;
            proj.add_file_to(&rel, Category::IndexedFiles);
        }
        self.do_save_project();
        self.new_indexed.open = false;
        self.new_indexed_dir = None;

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

    fn show_indexed_inspector(&mut self, panel_ui: &mut egui::Ui, tr: &Tr) {
        // Panels are Ui-hosted since egui 0.35; everything else in this
        // method still wants a Context.
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        let mut close = false;
        let mut open_grid = false;
        let mut property_edit = PropertyEdit::None;
        let mut structure_action = StructureAction::None;
        let mut did_add_remove = false;

        let card = crate::theme::glass_panel_frame(
            ctx.global_style().visuals.panel_fill,
            self.current_theme(),
        );

        egui::CentralPanel::default().frame(card).show(panel_ui, |ui| {
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
                ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(1.0, border_color), egui::StrokeKind::Middle);

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
                    // Header action icons, enlarged 2× (22×18 → 44×36) with a two-word
                    // caption beneath each so their purpose reads at a glance. `labeled`
                    // stacks the (bigger) icon button over a small caption in a fixed cell
                    // so the three line up evenly in the right-to-left header row.
                    let icon_size = egui::vec2(44.0, 36.0);
                    let cell_w = 68.0_f32;
                    let mut labeled = |ui: &mut egui::Ui,
                                       tip: &str,
                                       caption: &str,
                                       draw: &dyn Fn(&egui::Painter, egui::Rect, egui::Color32)|
                     -> bool {
                        let mut clicked = false;
                        ui.allocate_ui_with_layout(
                            egui::vec2(cell_w, icon_size.y + 16.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                clicked = icon_btn(ui, icon_size, tip, draw);
                                ui.label(
                                    egui::RichText::new(caption)
                                        .size(10.0)
                                        .color(ui.visuals().weak_text_color()),
                                );
                            },
                        );
                        clicked
                    };

                    // Close (X) - hand-written vector icon
                    if labeled(ui, tr.idx_prop_close_tip, tr.idx_prop_close_cap, &|p, r, c| {
                        let s = egui::Stroke::new(1.8, c);
                        let q = r.shrink(r.width() * 0.22);
                        p.line_segment([q.left_top(), q.right_bottom()], s);
                        p.line_segment([q.right_top(), q.left_bottom()], s);
                    }) {
                        close = true;
                    }

                    // Open Indexed File Browser - hand-written grid/table icon
                    let grid_tip = if st.def.finalized { tr.btn_open_grid_browser } else { tr.grid_requires_finalize };
                    if labeled(ui, grid_tip, tr.idx_prop_grid_cap, &|p, r, c| {
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
                    if labeled(ui, tr.idx_prop_raw_tip, tr.idx_prop_raw_cap, &|p, r, c| {
                        let s = egui::Stroke::new(1.5, c);
                        let body = egui::Rect::from_center_size(r.center(), egui::Vec2::new(r.width() * 0.60, r.height() * 0.68));
                        p.rect_stroke(body, 1.4, s, egui::StrokeKind::Middle);
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

            ui.scope_builder(egui::UiBuilder::new().max_rect(remaining_rect), |ui| {
                ui.horizontal_top(|ui| {
                    // Left column: either raw text editor or the tree structure
                    ui.vertical(|ui| {
                        // The structure tree only needs room for the data-item names, so
                        // sizing it to the whole pane (minus the property block) pushed the
                        // property details far to the right of the item they describe. Give
                        // the tree a moderate width so the details sit right beside the list.
                        // The raw COBOL editor, by contrast, wants all the room it can get,
                        // so it keeps the wide column.
                        let left_w = if st.prefer_raw_editor {
                            (remaining_rect.width() - 330.0).max(350.0)
                        } else {
                            380.0_f32.min((remaining_rect.width() - 330.0).max(300.0))
                        };
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
        // allocate_new_ui branch above) *is* the visible form that replaced
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

        let screen = ctx.content_rect();
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
                    if let Some(form) = &self.settings_form {
                        let mut cfg = self.llm.clone();
                        cfg.provider = form.draft.llm_provider.clone();
                        cfg.endpoint = form.draft.llm_endpoint.clone();
                        cfg.api_key = form.draft.llm_api_key.clone();
                        cfg.model = form.draft.llm_model.clone();
                        if !cfg.model.trim().is_empty() {
                            self.llm_benchmark_offer = Some(cfg);
                        }
                    }
                    self.llm_test_from_model_selection = false;
                    self.llm_test_rx = None;
                }
                Ok(crate::llm::LlmResponse::Err(e)) => {
                    // Surface the failure in a modal (the full request/response is
                    // in the connection log, reachable via the modal's Details).
                    self.llm_test_status = Some(e.clone());
                    self.set_llm_test_error(e);
                    self.llm_test_from_model_selection = false;
                    self.llm_test_rx = None;
                }
                Ok(crate::llm::LlmResponse::Chunk(_)) => {}
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.llm_test_status = Some("The test worker stopped unexpectedly.".into());
                    self.llm_test_from_model_selection = false;
                    self.llm_test_rx = None;
                }
            }
        }
        self.poll_llm_detect();
    }

    fn poll_llm_benchmark(&mut self) {
        let Some(rx) = &self.llm_benchmark_rx else {
            return;
        };
        let final_result = loop {
            match rx.try_recv() {
                Ok(crate::llm::LlmResponse::Chunk(_)) => {
                    self.llm_benchmark_status = Some("Running COBOL proficiency check...".into());
                }
                Ok(crate::llm::LlmResponse::Ok(report)) => {
                    break Ok(report);
                }
                Ok(crate::llm::LlmResponse::Err(e)) => {
                    break Err(e);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    break Err("The benchmark worker stopped unexpectedly.".into());
                }
            }
        };
        self.llm_benchmark_rx = None;
        match final_result {
            Ok(report) => {
                self.llm_benchmark_status = Some("COBOL proficiency check complete.".into());
                self.save_llm_benchmark_stats(&report);
                self.record_benchmark_success(&report);
                self.llm_benchmark_report = Some(report);
            }
            Err(e) => {
                self.llm_benchmark_status = Some(e.clone());
                self.record_benchmark_failure(&e);
                self.set_llm_test_error(format!("COBOL proficiency check failed: {e}"));
            }
        }
    }

    /// Start a proficiency run, and alongside it the capability probe whose
    /// token limits land on the same leaderboard row (spec 040).
    fn start_proficiency_benchmark(&mut self, cfg: crate::llm::LlmConfig) {
        self.llm_benchmark_status = Some("Running COBOL proficiency check...".into());
        self.llm_benchmark_config = Some(cfg.clone());
        self.llm_benchmark_rx = Some(crate::llm::spawn_cobol_proficiency_benchmark(&cfg));
        self.start_capability_probe(&cfg);
    }

    /// Ask the provider what the model supports. Never blocks and never fails
    /// the run: an unknown limit stays unknown.
    fn start_capability_probe(&mut self, cfg: &crate::llm::LlmConfig) {
        if cfg.model.trim().is_empty() || self.llm_caps_rx.is_some() {
            return;
        }
        let Some(provider) = crate::llm::Provider::from_id(&cfg.provider) else {
            return;
        };
        self.llm_caps_target = Some((
            cfg.provider.clone(),
            cfg.model.clone(),
            cfg.endpoint.clone(),
        ));
        self.llm_caps_rx = Some(crate::llm::spawn_probe_capabilities(
            provider,
            &cfg.endpoint,
            &cfg.api_key,
            &cfg.model,
        ));
    }

    fn poll_capability_probe(&mut self) {
        let Some(rx) = &self.llm_caps_rx else {
            return;
        };
        match rx.try_recv() {
            Ok(caps) => {
                if let Some((provider, model, endpoint)) = self.llm_caps_target.take() {
                    self.leaderboard
                        .apply_capabilities(&provider, &model, &endpoint, caps);
                    self.save_leaderboard();
                }
                self.llm_caps_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.llm_caps_rx = None;
                self.llm_caps_target = None;
            }
        }
    }

    /// Whether the COBOL Proficiency Judge can actually judge — it exists, is
    /// enabled, and has a model of its own (spec 040).
    fn judge_has_model(&self) -> bool {
        self.project_dir()
            .map(|root| {
                crate::agents_db::AgentsDb::load(&root)
                    .proficiency_judge_config(&self.llm)
                    .is_some()
            })
            .unwrap_or(false)
    }

    /// Carry out what the leaderboard row was clicked for (spec 040).
    fn handle_leaderboard_action(
        &mut self,
        act: crate::panels::leaderboard_modal::LeaderboardAction,
    ) {
        if act.open_judge_setup {
            if let Some(dir) = self
                .project_path
                .as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf())
            {
                self.agents_modal = Some(crate::panels::agents_modal::AgentsModal::open_at(
                    &dir,
                    &mut self.llm,
                    crate::agents_db::PROFICIENCY_JUDGE,
                ));
                self.persist_active_project_ai();
            }
        }
        if let Some((provider, model)) = act.run_tests {
            let cfg = self.leaderboard_run_config(&provider, &model);
            // Say which way the run went before it starts: a score that was
            // never contested must not look like one that was.
            let tr = self.lang.tr();
            let status = if cfg.reviewer_configured() {
                tr.leaderboard_judged_by
                    .replacen("{}", &cfg.reviewer_model, 1)
            } else {
                tr.leaderboard_unjudged.to_string()
            };
            self.output.push_status(status.clone());
            if let Some(m) = self.leaderboard_modal.as_mut() {
                m.set_status(status);
            }
            self.start_proficiency_benchmark(cfg);
        }
        // The board's "Use for Grace / Judge / All Specialists" actions are
        // gone (operator, 2026-08-09) — assigning a model is the Agent × Model
        // table's job, where every agent is visible and the separation rule is
        // checked as you pick.
        if let Some((provider, model)) = act.add_model {
            // Spec 048 R20 — a model can be benchmarked without any agent
            // running it, so the board takes it on the developer's say-so.
            let endpoint = self.llm.provider_endpoint(&provider);
            if self
                .leaderboard
                .ensure_models(&[(provider.clone(), model.clone(), endpoint)])
            {
                self.save_leaderboard();
                self.output
                    .push_status(format!("Leaderboard: added {provider}/{model}."));
            }
        }
        if let Some((provider, model)) = act.retire {
            // The developer's own say-so, for a model the provider shut down
            // before its catalogue caught up. Same ending as the automatic
            // path: the row goes, a tombstone keeps the archive replay from
            // bringing it back, and an agent left holding it gets said so.
            let stranded = self.agents_running_model(&provider, &model);
            if let Some(label) = self.leaderboard.retire(
                &provider,
                &model,
                crate::leaderboard::RetiredBecause::Removed,
            ) {
                let tr = self.lang.tr();
                self.save_leaderboard();
                self.output
                    .push_status(tr.leaderboard_retired.replacen("{}", &label, 1));
                for agent in &stranded {
                    self.output.push_status(
                        tr.leaderboard_agent_stranded
                            .replacen("{}", agent, 1)
                            .replacen("{}", &model, 1),
                    );
                }
                if let Some(agent) = stranded.first() {
                    if let Some(dir) = self
                        .project_path
                        .as_ref()
                        .and_then(|p| p.parent())
                        .map(|p| p.to_path_buf())
                    {
                        self.agents_modal =
                            Some(crate::panels::agents_modal::AgentsModal::open_at(
                                &dir, &mut self.llm, agent,
                            ));
                    }
                }
            }
        }
        if let Some((provider, model)) = act.open_report {
            // The full report text lives in the project archive, not in the
            // ranked store; re-running is the only way to see it if this
            // project never ran the test.
            match self.stored_benchmark_report(&provider, &model) {
                Some(report) => {
                    self.llm_benchmark_config =
                        Some(self.leaderboard_primary_config(&provider, &model));
                    self.llm_benchmark_report = Some(report);
                }
                None => {
                    if let Some(m) = self.leaderboard_modal.as_mut() {
                        m.set_status(format!(
                            "No stored report for {model} in this project — run the tests to produce one."
                        ));
                    }
                }
            }
        }
    }

    /// The runnable config for a leaderboard row: the matching project profile
    /// when there is one (so its key and sampling are used), otherwise the
    /// row's own provider/endpoint over the active config.
    fn leaderboard_run_config(&self, provider: &str, model: &str) -> crate::llm::LlmConfig {
        let mut cfg = self.leaderboard_primary_config(provider, model);
        self.attach_proficiency_judge(&mut cfg);
        cfg
    }

    /// Point the run's reviewer fields at the COBOL Proficiency Judge, so the
    /// score the board ranks was contested by a second model rather than
    /// awarded by the model to itself (spec 040).
    ///
    /// A judge resolving to the same model as the one under test is left
    /// unattached: `reviewer_configured` would reject it anyway, and an
    /// unattached reviewer is at least an honest "unjudged" the caller can
    /// report.
    fn attach_proficiency_judge(&self, cfg: &mut crate::llm::LlmConfig) {
        let Some(root) = self.project_dir() else {
            return;
        };
        let db = crate::agents_db::AgentsDb::load(&root);
        let Some(judge) = db.proficiency_judge_config(&self.llm) else {
            return;
        };
        if judge.model.trim().eq_ignore_ascii_case(cfg.model.trim())
            && judge.provider.trim().eq_ignore_ascii_case(cfg.provider.trim())
        {
            return;
        }
        cfg.reviewer_provider = judge.provider.clone();
        cfg.reviewer_endpoint = judge.endpoint.clone();
        cfg.reviewer_model = judge.model.clone();
        let prompt = db.load_agent_core_instructions(crate::agents_db::PROFICIENCY_JUDGE);
        if !prompt.trim().is_empty() {
            cfg.pedantic_prompt = prompt;
        }
    }

    /// The model-under-test half of a leaderboard run.
    ///
    /// Since spec 048 the connection comes from the model's PROVIDER: one key
    /// and one endpoint serve every model that provider offers, so a model can
    /// be benchmarked without any per-model configuration existing for it.
    fn leaderboard_primary_config(&self, provider: &str, model: &str) -> crate::llm::LlmConfig {
        let mut cfg = self.llm.clone();
        cfg.provider = provider.to_string();
        cfg.model = model.to_string();
        let provider_endpoint = self.llm.provider_endpoint(provider);
        if !provider_endpoint.trim().is_empty() {
            cfg.endpoint = provider_endpoint;
        } else if let Some(e) = self.leaderboard.get(provider, model) {
            // A row tested before its provider was configured still knows the
            // host it reached.
            if !e.endpoint.trim().is_empty() {
                cfg.endpoint = e.endpoint.clone();
            }
        }
        let provider_slot = crate::llm::provider_key_slot(provider);
        let legacy_slot = crate::llm::api_key_slot(provider, model);
        cfg.api_key_slot = if self.llm.api_keys.contains_key(&provider_slot) {
            provider_slot.clone()
        } else {
            legacy_slot.clone()
        };
        cfg.api_key = self
            .llm
            .api_keys
            .get(&provider_slot)
            .or_else(|| self.llm.api_keys.get(&legacy_slot))
            .cloned()
            .unwrap_or_default();
        cfg
    }

    // `assign_model_to_judge` and `assign_model_to_agents` lived here. They
    // existed only for the Leaderboard's "Use for Grace / Judge / All
    // Specialists" buttons, removed by the operator on 2026-08-09 — assigning
    // a model is the Agent × Model table's job, where every agent is on screen
    // and the separation rule is checked as you choose, rather than three
    // buttons silently rewriting a pool of agents none of which is visible.

    /// The most recent stored report for this model in the open project.
    fn stored_benchmark_report(&self, provider: &str, model: &str) -> Option<String> {
        let path = self
            .project_dir()?
            .join("agentic_ai")
            .join("model-benchmarks.jsonl");
        let text = std::fs::read_to_string(path).ok()?;
        text.lines()
            .rev()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|v| {
                v.get("provider").and_then(|p| p.as_str()) == Some(provider)
                    && v.get("model").and_then(|m| m.as_str()) == Some(model)
            })
            .and_then(|v| {
                v.get("report")
                    .and_then(|r| r.as_str())
                    .map(str::to_string)
            })
    }

    /// The `(provider, model, endpoint)` list the open project's agents run on
    /// (spec 048 R17) — what the board is populated and pruned against now that
    /// model profiles are gone.
    fn assigned_models(&self) -> Vec<(String, String, String)> {
        let Some(root) = self.project_dir() else {
            return Vec::new();
        };
        crate::agents_db::AgentsDb::load(&root).assigned_models(&self.llm)
    }

    /// Bring the board up to date with what this project actually runs on
    /// (spec 040, re-sourced by spec 048): every model an agent is assigned
    /// gets a row, and any proficiency report this project archived before the
    /// board existed is folded back in.
    ///
    /// Cheap and idempotent — it only writes when something changed — so it can
    /// run on startup, on project open, and whenever the panel is opened.
    fn sync_leaderboard_models(&mut self) {
        let assigned = self.assigned_models();
        let mut changed = self.leaderboard.ensure_models(&assigned);
        changed |= self.backfill_leaderboard_from_archive();
        if changed {
            self.save_leaderboard();
        }
    }

    /// Which agents run `(provider, model)` right now, by name.
    ///
    /// Needed when a model is retired: an agent left pointing at a model that
    /// no longer exists is the real damage — a board row is only a list entry.
    fn agents_running_model(&self, provider: &str, model: &str) -> Vec<String> {
        let Some(root) = self.project_dir() else {
            return Vec::new();
        };
        let db = crate::agents_db::AgentsDb::load(&root);
        db.agents
            .iter()
            .filter(|a| {
                crate::agents_db::resolve_agent_connection(a, &self.llm)
                    .map(|cfg| {
                        cfg.provider.trim().eq_ignore_ascii_case(provider.trim())
                            && cfg.model.trim().eq_ignore_ascii_case(model.trim())
                    })
                    .unwrap_or(false)
            })
            .map(|a| a.name.clone())
            .collect()
    }

    /// A provider's catalogue came back and no longer lists models this board
    /// carries: they were decommissioned, so they go (operator, 2026-08-20).
    ///
    /// Nothing here fires on a failed or empty listing — see
    /// [`crate::leaderboard::Leaderboard::retire_missing`], which refuses it —
    /// and nothing fires for a provider that was not the one refreshed.
    ///
    /// A retirement that strands an agent does not end silently: the Agents
    /// Manager opens on the first affected agent so a replacement model can be
    /// chosen there and then. Leaving that to be discovered on the next run,
    /// as a connection error, is how a tidy-up becomes an outage.
    fn retire_decommissioned_models(&mut self, provider: &str, catalogue: &[String]) {
        // Who runs what, BEFORE the rows go: afterwards the board no longer
        // knows these models existed.
        let doomed: Vec<(String, String)> = self
            .leaderboard
            .entries
            .iter()
            .filter(|e| e.provider.eq_ignore_ascii_case(provider.trim()))
            .filter(|e| {
                !catalogue
                    .iter()
                    .any(|m| m.trim().eq_ignore_ascii_case(&e.model))
            })
            .map(|e| (e.provider.clone(), e.model.clone()))
            .collect();
        let stranded: Vec<(String, String)> = doomed
            .iter()
            .flat_map(|(p, m)| {
                self.agents_running_model(p, m)
                    .into_iter()
                    .map(move |agent| (agent, m.clone()))
            })
            .collect();

        let removed = self.leaderboard.retire_missing(provider, catalogue);
        if removed.is_empty() {
            return;
        }
        let tr = self.lang.tr();
        self.output.push_status(
            tr.leaderboard_decommissioned
                .replacen("{}", &removed.len().to_string(), 1)
                .replacen("{}", &removed.join(", "), 1),
        );
        self.save_leaderboard();

        for (agent, model) in &stranded {
            self.output.push_status(
                tr.leaderboard_agent_stranded
                    .replacen("{}", agent, 1)
                    .replacen("{}", model, 1),
            );
        }
        // One window, on the first stranded agent — opening several would bury
        // the developer under modals for what is one decision per agent, and
        // the manager lists them all anyway.
        if let Some((agent, _)) = stranded.first() {
            if let Some(dir) = self
                .project_path
                .as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf())
            {
                self.agents_modal = Some(crate::panels::agents_modal::AgentsModal::open_at(
                    &dir, &mut self.llm, agent,
                ));
            }
        }
    }

    /// Housekeeping when the board is OPENED: a row that no agent uses **and**
    /// that was never tested is noise, and goes (spec 048 R18).
    ///
    /// Deliberately not folded into [`Self::sync_leaderboard_models`], which
    /// also runs at startup and on project open: this one removes rows, and it
    /// belongs to the moment the developer asked to look at the board.
    ///
    /// A row with runs on it is **never** removed (R19). That is the change
    /// from 1.61.6, which pruned on registry membership alone and could delete
    /// a model's whole score history because an agent had moved on. The board
    /// is machine-wide while the assignments belong to the open project, so
    /// that rule made another project's results collateral damage; with scores
    /// protected, the worst this can now do is drop an empty row.
    fn prune_leaderboard_orphans(&mut self) {
        let assigned = self.assigned_models();
        let removed = self.leaderboard.prune_untested_orphans(&assigned);
        if removed.is_empty() {
            return;
        }
        self.output.push_status(format!(
            "Leaderboard housekeeping: dropped {} untested model(s) no agent uses — {}",
            removed.len(),
            removed.join(", ")
        ));
        self.save_leaderboard();
    }

    /// Replay `agentic_ai/model-benchmarks.jsonl` into the board.
    ///
    /// Every proficiency test ever run in this project was archived there with
    /// its full report; the board arrived afterwards. Rather than making the
    /// developer re-run tests that already happened (and re-pay for them), the
    /// archive is parsed oldest-first and the scores recovered. A model that
    /// already has a result on the board is left alone — a live run is always
    /// better evidence than a replayed one.
    fn backfill_leaderboard_from_archive(&mut self) -> bool {
        let Some(root) = self.project_dir() else {
            return false;
        };
        let path = root.join("agentic_ai").join("model-benchmarks.jsonl");
        let Ok(text) = std::fs::read_to_string(&path) else {
            return false;
        };
        // Models already carrying a result before this pass are left alone; a
        // live run always beats a replayed one. Captured up front so that
        // within the replay itself the later archive lines still overwrite the
        // earlier ones — the archive is append-ordered, so the newest run of a
        // model must be the one that stands.
        let already_rated: std::collections::HashSet<(String, String)> = self
            .leaderboard
            .entries
            .iter()
            .filter(|e| e.rated())
            .map(|e| (e.provider.to_lowercase(), e.model.to_lowercase()))
            .collect();
        let mut changed = false;
        for line in text.lines() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let (Some(provider), Some(model)) = (
                v.get("provider").and_then(|p| p.as_str()),
                v.get("model").and_then(|m| m.as_str()),
            ) else {
                continue;
            };
            if provider.trim().is_empty() || model.trim().is_empty() {
                continue;
            }
            if already_rated.contains(&(provider.to_lowercase(), model.to_lowercase())) {
                continue;
            }
            // A retired model stays off the board. This replay is the reason a
            // removal needed a tombstone at all: the archive keeps every scored
            // report forever, so without this the model deleted a moment ago is
            // back the next time the board opens. A LIVE run still revives it
            // (`record_success`) — replaying old evidence is not the same as
            // the model answering today.
            if self.leaderboard.is_retired(provider, model) {
                continue;
            }
            let Some(report) = v.get("report").and_then(|r| r.as_str()) else {
                continue;
            };
            // Only a report that actually carries scores is worth replaying;
            // the inferred fallback would invent a rank out of prose.
            let Some(metrics) = Self::llm_benchmark_metrics(report) else {
                continue;
            };
            if metrics.get("overall_score").is_none() {
                continue;
            }
            let endpoint = v
                .get("endpoint")
                .and_then(|e| e.as_str())
                .unwrap_or_default();
            self.leaderboard.record_success(
                provider,
                model,
                endpoint,
                crate::leaderboard::RunOutcome { metrics },
            );
            changed = true;
        }
        changed
    }

    fn save_leaderboard(&mut self) {
        if let Err(e) = self.leaderboard.save() {
            self.output
                .push_status(format!("Could not save the leaderboard: {e}"));
        }
    }

    /// Fold a completed proficiency run into the leaderboard (spec 040).
    fn record_benchmark_success(&mut self, report: &str) {
        let Some(cfg) = self.llm_benchmark_config.clone() else {
            return;
        };
        let metrics = Self::llm_benchmark_metrics(report)
            .unwrap_or_else(|| Self::fallback_benchmark_metrics(report));
        self.leaderboard.record_success(
            &cfg.provider,
            &cfg.model,
            &cfg.endpoint,
            crate::leaderboard::RunOutcome { metrics },
        );
        self.save_leaderboard();
    }

    /// Record a run that could not be carried out: the model keeps whatever it
    /// scored before, and shows the reason instead of a rank.
    fn record_benchmark_failure(&mut self, error: &str) {
        let Some(cfg) = self.llm_benchmark_config.clone() else {
            return;
        };
        self.leaderboard
            .record_failure(&cfg.provider, &cfg.model, &cfg.endpoint, error);
        self.save_leaderboard();
        // A test started from the board reports back to the board, rather than
        // to the generic connection alert behind it.
        if let Some(m) = self.leaderboard_modal.as_mut() {
            m.show_error(cfg.model.clone(), error.to_string());
        }
    }

    fn save_llm_benchmark_stats(&mut self, report: &str) {
        let Some(root) = self.project_dir() else {
            return;
        };
        let dir = root.join("agentic_ai");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.output
                .push_status(format!("Could not create benchmark folder: {e}"));
            return;
        }
        let path = dir.join("model-benchmarks.jsonl");
        let cfg = self.llm_benchmark_config.as_ref().unwrap_or(&self.llm);
        let entry = serde_json::json!({
            "timestamp_unix": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            "provider": cfg.provider,
            "model": cfg.model,
            "endpoint": cfg.endpoint,
            "report": report,
        });
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            Ok(mut f) => {
                if let Err(e) = writeln!(f, "{entry}") {
                    self.output
                        .push_status(format!("Could not save benchmark result: {e}"));
                }
            }
            Err(e) => self
                .output
                .push_status(format!("Could not open benchmark history: {e}")),
        }
    }

    fn llm_benchmark_metrics(report: &str) -> Option<serde_json::Value> {
        let mut candidates = Vec::new();
        let mut rest = report;
        while let Some(start) = rest.find("```") {
            rest = &rest[start + 3..];
            let Some(end) = rest.find("```") else {
                break;
            };
            let block = &rest[..end];
            rest = &rest[end + 3..];
            candidates.push(Self::strip_fence_preamble(block));
        }
        if let (Some(start), Some(end)) = (report.rfind('{'), report.rfind('}')) {
            if start < end {
                candidates.push(report[start..=end].to_string());
            }
        }
        let mut first_valid: Option<serde_json::Value> = None;
        for text in candidates {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                let v = value
                    .get("metrics")
                    .cloned()
                    .or_else(|| value.get("overall_score").is_some().then(|| value.clone()));
                if let Some(v) = v {
                    // The Pedantic Agent's FINAL assessment overrides the
                    // primary's self-scores whenever the tandem loop ran.
                    if value.get("pedantic_final").is_some() || v.get("pedantic_final").is_some() {
                        return Some(v);
                    }
                    if first_valid.is_none() {
                        first_valid = Some(v);
                    }
                }
            }
        }
        first_valid
    }

    /// Reduce a fenced block to the JSON inside it.
    ///
    /// Models label the metrics block every way the prompt could be read:
    /// ```` ```json ````, ```` ```metrics ````, and — the one that used to
    /// defeat this — a fence tagged `json` whose body opens `metrics = {`, an
    /// assignment rather than a value. Each marker is peeled in turn instead of
    /// once, so `json` + `metrics` + `=` on the same block all come off and the
    /// scores are read rather than silently discarded.
    fn strip_fence_preamble(block: &str) -> String {
        let mut text = block.trim();
        for _ in 0..4 {
            let before = text;
            for tag in ["json", "metrics"] {
                if let Some(rest) = text.strip_prefix(tag) {
                    // Only a real tag, never the start of a longer word.
                    if rest
                        .chars()
                        .next()
                        .map(|c| c.is_whitespace() || c == '=' || c == '{')
                        .unwrap_or(false)
                    {
                        text = rest.trim_start();
                    }
                }
            }
            if let Some(rest) = text.strip_prefix('=') {
                text = rest.trim_start();
            }
            if std::ptr::eq(before, text) {
                break;
            }
        }
        text.trim().to_string()
    }

    fn metric_score(metrics: &serde_json::Value, key: &str) -> Option<f32> {
        metrics
            .get(key)
            .and_then(|v| v.as_f64())
            .map(|v| v.clamp(0.0, 100.0) as f32)
    }

    fn fallback_benchmark_metrics(report: &str) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        for key in [
            "overall_score",
            "compilation_score",
            "functional_score",
            "instruction_following",
            "semantic_correctness",
            "code_preservation",
            "runtime_correctness",
            "hallucination_resistance",
            "formatting_preservation",
            "cobol85_score",
            "powerrustcobol_score",
            "program_structure_score",
            "data_description_score",
            "control_flow_score",
            "file_handling_score",
            "forms_extensions_score",
            "unsupported_feature_avoidance",
        ] {
            if let Some(v) = Self::scan_metric_number(report, key) {
                obj.insert(key.to_string(), serde_json::json!(v));
            }
        }
        obj.insert("_metrics_inferred".to_string(), serde_json::json!(true));
        serde_json::Value::Object(obj)
    }

    fn scan_metric_number(text: &str, key: &str) -> Option<f32> {
        let key_pos = text.find(key)?;
        let after_key = &text[key_pos + key.len()..];
        let colon_pos = after_key.find(':')?;
        let mut chars = after_key[colon_pos + 1..]
            .chars()
            .skip_while(|c| c.is_whitespace() || *c == '"' || *c == '`');
        let mut n = String::new();
        while let Some(c) = chars.next() {
            if c.is_ascii_digit() || c == '.' {
                n.push(c);
            } else if !n.is_empty() {
                break;
            } else if !c.is_whitespace() {
                return None;
            }
        }
        n.parse::<f32>().ok().map(|v| v.clamp(0.0, 100.0))
    }

    fn benchmark_scope_scores() -> [(&'static str, &'static str, &'static str); 8] {
        [
            (
                "Program structure",
                "program_structure_score",
                "compilation_score",
            ),
            (
                "Data descriptions",
                "data_description_score",
                "semantic_correctness",
            ),
            ("Control flow", "control_flow_score", "functional_score"),
            (
                "File handling",
                "file_handling_score",
                "runtime_correctness",
            ),
            (
                "Forms/extensions",
                "forms_extensions_score",
                "powerrustcobol_score",
            ),
            (
                "Avoid unsupported",
                "unsupported_feature_avoidance",
                "hallucination_resistance",
            ),
            (
                "Code preservation",
                "code_preservation",
                "code_preservation",
            ),
            (
                "Formatting",
                "formatting_preservation",
                "formatting_preservation",
            ),
        ]
    }

    fn benchmark_metric_score(
        metrics: &serde_json::Value,
        key: &str,
        fallback_key: &str,
    ) -> Option<f32> {
        Self::metric_score(metrics, key).or_else(|| Self::metric_score(metrics, fallback_key))
    }

    fn benchmark_scores_are_all_perfect(metrics: &serde_json::Value) -> bool {
        let keys = [
            "overall_score",
            "compilation_score",
            "functional_score",
            "instruction_following",
            "semantic_correctness",
            "code_preservation",
            "runtime_correctness",
            "hallucination_resistance",
            "formatting_preservation",
            "cobol85_score",
            "powerrustcobol_score",
            "program_structure_score",
            "data_description_score",
            "control_flow_score",
            "file_handling_score",
            "forms_extensions_score",
            "unsupported_feature_avoidance",
        ];
        keys.iter()
            .filter_map(|key| Self::metric_score(metrics, key))
            .all(|score| score >= 100.0)
    }

    fn benchmark_metric_description(label: &str) -> &'static str {
        match label {
            "Program structure" => "Keeps COBOL divisions and program shape valid.",
            "Data descriptions" => "Uses PIC, WS, FD, and storage items correctly.",
            "Control flow" => "Builds valid decisions, loops, and handler flow.",
            "File handling" => "Handles indexed-file and record operations safely.",
            "Forms/extensions" => "Uses controls, events, properties, and methods.",
            "Avoid unsupported" => "Stays inside implemented PowerRustCOBOL features.",
            "Code preservation" => "Edits without deleting required existing code.",
            "Formatting" => "Preserves readable COBOL layout and spacing.",
            "COBOL-85 coverage" => "Understands supported COBOL-85 syntax and patterns.",
            "PowerRustCOBOL coverage" => "Understands PowerRustCOBOL GUI extensions.",
            "Unsupported avoided" => "Avoids invented APIs and unsupported syntax.",
            _ => "Benchmark score for this capability.",
        }
    }

    fn benchmark_text_list(metrics: &serde_json::Value, key: &str) -> Vec<String> {
        metrics
            .get(key)
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn benchmark_summary_markdown(metrics: &serde_json::Value) -> String {
        let overall = Self::metric_score(metrics, "overall_score").unwrap_or(0.0);
        let cobol85 = Self::metric_score(metrics, "cobol85_score").unwrap_or(overall);
        let prc = Self::metric_score(metrics, "powerrustcobol_score").unwrap_or(overall);
        let unsupported =
            Self::metric_score(metrics, "unsupported_feature_avoidance").unwrap_or(overall);
        let hallucination =
            Self::metric_score(metrics, "hallucination_resistance").unwrap_or(overall);
        let recommendation = if overall >= 90.0 && unsupported >= 90.0 && hallucination >= 90.0 {
            "Recommended"
        } else if overall >= 75.0 {
            "Use with review"
        } else {
            "Not recommended"
        };
        let usage = metrics
            .get("recommended_usage")
            .and_then(|v| v.as_str())
            .unwrap_or("Review the detailed benchmark before using this model.");

        let mut out = String::new();
        out.push_str("## Summary\n\n");
        if Self::benchmark_scores_are_all_perfect(metrics) {
            out.push_str("**Warning:** every metric was returned as 100%. This is a model-estimated, chat-only benchmark and was not independently compiled or runtime-verified by PowerRustCOBOL. Treat this as suspicious until the generated COBOL is checked by the compiler/runtime.\n\n");
        }
        out.push_str(&format!(
            "**Decision:** {recommendation}. **Overall competency:** {overall:.0}%.\n\n"
        ));
        out.push_str(&format!(
            "The model scored {cobol85:.0}% on COBOL-85 coverage and {prc:.0}% on PowerRustCOBOL-specific behavior. It scored {unsupported:.0}% for avoiding unsupported features and {hallucination:.0}% for hallucination resistance.\n\n"
        ));
        out.push_str(&format!("**Recommended usage:** {usage}\n\n"));

        let strengths = Self::benchmark_text_list(metrics, "strengths");
        if !strengths.is_empty() {
            out.push_str("**Strengths:**\n\n");
            for item in strengths.iter().take(5) {
                out.push_str(&format!("- {item}\n"));
            }
            out.push('\n');
        }

        let weaknesses = Self::benchmark_text_list(metrics, "weaknesses");
        if !weaknesses.is_empty() {
            out.push_str("**Watch points:**\n\n");
            for item in weaknesses.iter().take(5) {
                out.push_str(&format!("- {item}\n"));
            }
            out.push('\n');
        }
        out
    }

    fn benchmark_tested_points_markdown(metrics: &serde_json::Value) -> String {
        let mut out = String::new();
        out.push_str("## Tested points\n\n");
        out.push_str("This benchmark evaluates whether the model can produce complete, valid COBOL-85 and PowerRustCOBOL code inside the features currently supported by the project.\n\n");

        let points = [
            (
                "Overall competency",
                "overall_score",
                "End-to-end suitability for using the model in PowerRustCOBOL-assisted development.",
            ),
            (
                "Compilation",
                "compilation_score",
                "Ability to emit code that is syntactically valid and suitable for the current compiler/parser pipeline.",
            ),
            (
                "Functional behavior",
                "functional_score",
                "Ability to generate code that performs the requested business or form behavior instead of producing inert stubs.",
            ),
            (
                "Instruction following",
                "instruction_following",
                "Respect for user constraints, requested scope, and PowerRustCOBOL-specific directions.",
            ),
            (
                "Semantic correctness",
                "semantic_correctness",
                "Correct variable usage, paragraph/program structure, data item references, and control/property semantics.",
            ),
            (
                "Code preservation",
                "code_preservation",
                "Ability to modify existing code without deleting required divisions, declarations, handlers, or unrelated logic.",
            ),
            (
                "Runtime correctness",
                "runtime_correctness",
                "Likelihood that generated code behaves correctly when interpreted or run through the form/runtime path.",
            ),
            (
                "Hallucination resistance",
                "hallucination_resistance",
                "Avoidance of invented APIs, unsupported syntax, fake properties, or unavailable runtime calls.",
            ),
            (
                "Formatting preservation",
                "formatting_preservation",
                "Ability to preserve readable COBOL formatting and avoid damaging source layout during edits.",
            ),
            (
                "COBOL-85 coverage",
                "cobol85_score",
                "Knowledge of supported COBOL-85 structure, data descriptions, control flow, and file-oriented patterns.",
            ),
            (
                "PowerRustCOBOL coverage",
                "powerrustcobol_score",
                "Knowledge of PowerRustCOBOL inline object syntax, form controls, properties, events, and extensions.",
            ),
            (
                "Program structure",
                "program_structure_score",
                "Correct use and preservation of IDENTIFICATION, ENVIRONMENT, DATA, and PROCEDURE divisions.",
            ),
            (
                "Data descriptions",
                "data_description_score",
                "Correct use of WORKING-STORAGE, FD records, local/global data items, PIC clauses, and supported numeric formats.",
            ),
            (
                "Control flow",
                "control_flow_score",
                "Correct use of PERFORM, IF/EVALUATE-style decisions, loops, and handler-friendly flow.",
            ),
            (
                "File handling",
                "file_handling_score",
                "Correct use of supported indexed-file and record-oriented patterns without assuming unimplemented features.",
            ),
            (
                "Forms/extensions",
                "forms_extensions_score",
                "Correct use of controls, properties, events, non-visual controls, and inline get/set/invoke syntax.",
            ),
            (
                "Unsupported feature avoidance",
                "unsupported_feature_avoidance",
                "Ability to stay inside implemented PowerRustCOBOL behavior and ask for direction instead of inventing missing APIs.",
            ),
        ];

        for (label, key, description) in points {
            if let Some(score) = Self::metric_score(metrics, key) {
                out.push_str(&format!("### {label}: {score:.0}%\n\n{description}\n\n"));
            }
        }

        let failures = Self::benchmark_text_list(metrics, "typical_failure_patterns");
        if !failures.is_empty() {
            out.push_str("### Typical failure patterns\n\n");
            for item in failures {
                out.push_str(&format!("- {item}\n"));
            }
            out.push('\n');
        }

        out
    }

    fn benchmark_generated_cobol_markdown(report: &str) -> String {
        let blocks = Self::extract_benchmark_cobol_blocks(report);
        let mut out = String::new();
        out.push_str("## Generated COBOL code and accuracy analysis\n\n");
        if blocks.is_empty() {
            out.push_str("No fenced COBOL code block was returned by the model. Future benchmark runs request generated COBOL samples explicitly; if this section is empty, treat the report as incomplete for code-level review.\n\n");
            return out;
        }

        out.push_str("The following COBOL/PowerRustCOBOL code blocks were returned by the model during the benchmark. Review them together with the accuracy notes below and the metric explanations.\n\n");
        for (idx, code) in blocks.iter().enumerate() {
            out.push_str(&format!("### Generated code sample {}\n\n", idx + 1));
            out.push_str("```cobol\n");
            out.push_str(code.trim());
            out.push_str("\n```\n\n");
        }
        out.push_str("### Accuracy checklist\n\n");
        out.push_str("- Division completeness: verify IDENTIFICATION, ENVIRONMENT, DATA, and PROCEDURE divisions are present when the sample is a full program.\n");
        out.push_str("- Data correctness: verify PIC, USAGE, WORKING-STORAGE, FD, LOCAL-STORAGE, GLOBAL, and file status items match the generated behavior.\n");
        out.push_str("- Procedure behavior: verify statements implement the requested behavior, not inert stubs.\n");
        out.push_str("- PowerRustCOBOL syntax: verify controls use inline `Control::Property` and `Control::Method(...)` syntax instead of legacy helper CALLs.\n");
        out.push_str("- Unsupported features: verify the sample does not invent controls, methods, properties, runtime calls, or compiler internals.\n");
        out.push_str("- Runtime plausibility: verify file handling, invalid-key paths, EOF paths, commits/rollbacks, and event handlers can execute safely.\n\n");
        out
    }

    fn extract_benchmark_cobol_blocks(report: &str) -> Vec<String> {
        let mut blocks = Vec::new();
        let mut rest = report;
        while let Some(start) = rest.find("```") {
            rest = &rest[start + 3..];
            let Some(end) = rest.find("```") else {
                break;
            };
            let block = &rest[..end];
            rest = &rest[end + 3..];
            let mut lines = block.lines();
            let first = lines.next().unwrap_or("").trim().to_ascii_lowercase();
            let body = if first == "cobol"
                || first == "cbl"
                || first == "rustcobol"
                || first == "powerrustcobol"
            {
                lines.collect::<Vec<_>>().join("\n")
            } else {
                block.to_string()
            };
            let upper = body.to_ascii_uppercase();
            let looks_like_cobol = matches!(
                first.as_str(),
                "cobol" | "cbl" | "rustcobol" | "powerrustcobol"
            ) || upper.contains("IDENTIFICATION DIVISION")
                || upper.contains("PROCEDURE DIVISION")
                || upper.contains("WORKING-STORAGE SECTION")
                || upper.contains("ENVIRONMENT DIVISION")
                || upper.contains("DATA DIVISION");
            if looks_like_cobol {
                blocks.push(body);
            }
        }
        blocks
    }

    fn benchmark_metadata_markdown(
        cfg: &crate::llm::LlmConfig,
        metrics: &serde_json::Value,
        report: &str,
    ) -> String {
        let mut out = String::new();
        out.push_str("## Model tested\n\n");
        out.push_str(&format!("- **Provider:** {}\n", Self::provider_label(cfg)));
        out.push_str(&format!(
            "- **Model:** {}\n",
            Self::display_or_unknown(&cfg.model)
        ));
        out.push_str(&format!(
            "- **Endpoint:** {}\n",
            Self::display_or_unknown(&cfg.endpoint)
        ));
        out.push_str("- **Access/subscription:** connection verified; no subscription error was returned during the test.\n");
        out.push_str("- **Scoring basis:** model-estimated chat benchmark; no independent compiler/runtime verification was performed.\n");
        if Self::benchmark_scores_are_all_perfect(metrics) {
            out.push_str("- **Score warning:** all returned metrics are 100%; treat this report as suspicious and verify the generated COBOL manually.\n");
        }
        out.push_str(&format!(
            "- **Configured max output tokens:** {}\n",
            cfg.max_tokens
        ));
        out.push_str(&format!(
            "- **Input tokens:** {}\n",
            Self::benchmark_usage_value(metrics, report, &["input_tokens", "prompt_tokens"])
        ));
        out.push_str(&format!(
            "- **Output tokens:** {}\n",
            Self::benchmark_usage_value(metrics, report, &["output_tokens", "completion_tokens"])
        ));
        out.push_str(&format!(
            "- **Total tokens:** {}\n",
            Self::benchmark_usage_value(metrics, report, &["total_tokens"])
        ));
        out.push_str(&format!(
            "- **Tokenizer:** {}\n\n",
            Self::benchmark_string_value(metrics, report, &["tokenizer", "tokenizer_name"])
        ));
        out
    }

    fn provider_label(cfg: &crate::llm::LlmConfig) -> String {
        crate::llm::Provider::from_id(&cfg.provider)
            .map(|p| p.label.to_string())
            .unwrap_or_else(|| Self::display_or_unknown(&cfg.provider))
    }

    fn display_or_unknown(value: &str) -> String {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            "not set".to_string()
        } else {
            trimmed.to_string()
        }
    }

    fn benchmark_usage_value(metrics: &serde_json::Value, report: &str, keys: &[&str]) -> String {
        for key in keys {
            if let Some(v) = metrics.get(*key).and_then(|v| v.as_u64()) {
                return v.to_string();
            }
            if let Some(v) = Self::scan_metric_number(report, key) {
                return format!("{v:.0}");
            }
        }
        "not reported by provider".to_string()
    }

    fn benchmark_string_value(metrics: &serde_json::Value, report: &str, keys: &[&str]) -> String {
        for key in keys {
            if let Some(v) = metrics.get(*key).and_then(|v| v.as_str()) {
                return v.to_string();
            }
            if let Some(v) = Self::scan_metric_string(report, key) {
                return v;
            }
        }
        "not reported by provider".to_string()
    }

    fn scan_metric_string(text: &str, key: &str) -> Option<String> {
        let key_pos = text.find(key)?;
        let after_key = &text[key_pos + key.len()..];
        let colon_pos = after_key.find(':')?;
        let mut value = after_key[colon_pos + 1..].trim_start();
        value = value.trim_start_matches(|c| c == '"' || c == '`');
        let mut out = String::new();
        for c in value.chars() {
            if c == '"' || c == '`' || c == ',' || c == '\n' || c == '\r' {
                break;
            }
            out.push(c);
        }
        let out = out.trim();
        if out.is_empty() {
            None
        } else {
            Some(out.to_string())
        }
    }

    fn render_benchmark_metadata(
        ui: &mut egui::Ui,
        cfg: &crate::llm::LlmConfig,
        metrics: &serde_json::Value,
        report: &str,
    ) {
        egui::Frame::NONE
            .fill(Color32::from_rgb(10, 18, 28))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(62, 139, 205)))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Model tested").strong().size(16.0));
                    ui.separator();
                    Self::metadata_chip(ui, "Provider", &Self::provider_label(cfg));
                    Self::metadata_chip(ui, "Model", &Self::display_or_unknown(&cfg.model));
                    Self::metadata_chip(
                        ui,
                        "Subscription",
                        "verified; no subscription error returned",
                    );
                    Self::metadata_chip(
                        ui,
                        "Input tokens",
                        &Self::benchmark_usage_value(
                            metrics,
                            report,
                            &["input_tokens", "prompt_tokens"],
                        ),
                    );
                    Self::metadata_chip(
                        ui,
                        "Output tokens",
                        &Self::benchmark_usage_value(
                            metrics,
                            report,
                            &["output_tokens", "completion_tokens"],
                        ),
                    );
                    Self::metadata_chip(
                        ui,
                        "Tokenizer",
                        &Self::benchmark_string_value(
                            metrics,
                            report,
                            &["tokenizer", "tokenizer_name"],
                        ),
                    );
                    Self::metadata_chip(ui, "Max out", &cfg.max_tokens.to_string());
                });
                ui.add_space(4.0);
                ui.weak(format!(
                    "Endpoint: {}",
                    Self::display_or_unknown(&cfg.endpoint)
                ));
            });
    }

    fn metadata_chip(ui: &mut egui::Ui, label: &str, value: &str) {
        ui.label(
            egui::RichText::new(format!("{label}: "))
                .strong()
                .color(Color32::from_rgb(169, 206, 236)),
        );
        ui.label(value);
        ui.add_space(8.0);
    }

    fn benchmark_pdf_markdown(
        report: &str,
        metrics: Option<&serde_json::Value>,
        cfg: &crate::llm::LlmConfig,
    ) -> String {
        let mut out = String::new();
        out.push_str("# COBOL proficiency report\n\n");
        let Some(metrics) = metrics else {
            out.push_str(report.trim());
            out.push_str("\n\n");
            return out;
        };

        out.push_str(&Self::benchmark_metadata_markdown(cfg, metrics, report));
        out.push_str("## Benchmark dashboard\n\n");
        out.push_str(&Self::benchmark_pdf_dashboard_mermaid(metrics));
        if let Some(overall) = Self::metric_score(metrics, "overall_score") {
            out.push_str(&format!("**Overall competency:** {:.0}%\n\n", overall));
        }
        if let Some(usage) = metrics.get("recommended_usage").and_then(|v| v.as_str()) {
            out.push_str(&format!("**Recommended usage:** {}\n\n", usage));
        }

        out.push_str("### Score distribution\n\n");
        for (label, key, fallback_key) in Self::benchmark_scope_scores() {
            if let Some(v) = Self::benchmark_metric_score(metrics, key, fallback_key) {
                let filled = (v / 5.0).round() as usize;
                let bar = format!(
                    "{}{}",
                    "#".repeat(filled),
                    "-".repeat(20usize.saturating_sub(filled))
                );
                out.push_str(&format!("- **{}:** {:>3.0}% `{}`\n", label, v, bar));
            }
        }

        out.push('\n');
        out.push_str(&Self::benchmark_summary_markdown(metrics));
        out.push_str(&Self::benchmark_tested_points_markdown(metrics));
        out.push_str(&Self::benchmark_generated_cobol_markdown(report));
        out.push_str("## Model report\n\n");
        out.push_str(report.trim());
        out.push_str("\n\n");
        out
    }

    fn benchmark_pdf_dashboard_mermaid(metrics: &serde_json::Value) -> String {
        let overall = Self::metric_score(metrics, "overall_score").unwrap_or(0.0);
        let decision = if overall >= 90.0
            && Self::metric_score(metrics, "unsupported_feature_avoidance").unwrap_or(0.0) >= 90.0
            && Self::metric_score(metrics, "hallucination_resistance").unwrap_or(0.0) >= 90.0
        {
            "Recommended"
        } else if overall >= 75.0 {
            "Use with review"
        } else {
            "Not recommended"
        };

        let mut out = String::new();
        out.push_str("```mermaid\n");
        out.push_str("flowchart LR\n");
        out.push_str(&format!(
            "  Overall[{}]\n",
            Self::mermaid_label(&format!("Overall {:.0} percent {}", overall, decision))
        ));
        out.push_str("  Overall-->Scores[Supported scope scores]\n");
        for (idx, (label, key, fallback_key)) in Self::benchmark_scope_scores().iter().enumerate() {
            if let Some(score) = Self::benchmark_metric_score(metrics, key, fallback_key) {
                let node = format!("M{idx}");
                out.push_str(&format!(
                    "  Scores-->{node}[{}]\n",
                    Self::mermaid_label(&format!("{label} {:.0}", score)),
                ));
            }
        }
        out.push_str("```\n\n");
        out
    }

    fn mermaid_label(text: &str) -> String {
        let mut label = String::new();
        let mut last_was_space = false;
        for c in text.chars() {
            let next = if c.is_ascii_alphanumeric() { c } else { ' ' };
            if next == ' ' {
                if !last_was_space && !label.is_empty() {
                    label.push(next);
                }
                last_was_space = true;
            } else {
                label.push(next);
                last_was_space = false;
            }
        }
        let label = label.trim();
        if label.is_empty() {
            "Score".to_string()
        } else {
            label.to_string()
        }
    }

    fn render_llm_benchmark_dashboard(ui: &mut egui::Ui, metrics: &serde_json::Value) {
        let overall = Self::metric_score(metrics, "overall_score").unwrap_or(0.0);
        let inferred = metrics
            .get("_metrics_inferred")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(10.0);
        ui.heading("Benchmark dashboard");
        if inferred {
            ui.weak(
                "Structured metrics JSON was not found or was incomplete; this dashboard uses any score fields that could be recovered from the report text.",
            );
        }
        if Self::benchmark_scores_are_all_perfect(metrics) {
            ui.colored_label(
                Color32::from_rgb(230, 187, 79),
                "Warning: all metrics are 100%. This is model-estimated and was not compiler/runtime verified.",
            );
        }
        ui.add_space(8.0);

        let scores = Self::benchmark_scope_scores();

        ui.horizontal_top(|ui| {
            let kpi_size = egui::vec2(210.0, 150.0);
            let (rect, _) = ui.allocate_exact_size(kpi_size, egui::Sense::hover());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 8.0, Color32::from_rgb(13, 24, 36));
            painter.rect_stroke(
                rect,
                8.0,
                egui::Stroke::new(1.0, Color32::from_rgb(62, 139, 205)),
                egui::StrokeKind::Middle,
            );
            let accent = if overall >= 85.0 {
                Color32::from_rgb(61, 205, 139)
            } else if overall >= 70.0 {
                Color32::from_rgb(230, 187, 79)
            } else {
                Color32::from_rgb(238, 101, 101)
            };
            painter.text(
                rect.center_top() + egui::vec2(0.0, 18.0),
                egui::Align2::CENTER_TOP,
                "Overall competency",
                egui::FontId::proportional(17.0),
                ui.visuals().strong_text_color(),
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{overall:.0}%"),
                egui::FontId::proportional(46.0),
                accent,
            );
            painter.text(
                rect.center_bottom() - egui::vec2(0.0, 22.0),
                egui::Align2::CENTER_BOTTOM,
                metrics
                    .get("recommended_usage")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Review report before production use"),
                egui::FontId::proportional(12.0),
                ui.visuals().weak_text_color(),
            );

            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Supported-scope scores").strong());
                ui.add_space(4.0);
                for (label, key, fallback_key) in scores {
                    if let Some(v) = Self::benchmark_metric_score(metrics, key, fallback_key) {
                        Self::draw_metric_bar(
                            ui,
                            label,
                            Self::benchmark_metric_description(label),
                            v,
                        );
                    }
                }
            });

            ui.add_space(16.0);
            Self::draw_metric_radar(ui, metrics, &scores);
        });

        ui.add_space(12.0);
        ui.horizontal_top(|ui| {
            let decision = if overall >= 90.0
                && Self::metric_score(metrics, "unsupported_feature_avoidance").unwrap_or(0.0)
                    >= 90.0
                && Self::metric_score(metrics, "hallucination_resistance").unwrap_or(0.0) >= 90.0
            {
                ("Recommended", Color32::from_rgb(61, 205, 139))
            } else if overall >= 75.0 {
                ("Use with review", Color32::from_rgb(230, 187, 79))
            } else {
                ("Not recommended", Color32::from_rgb(238, 101, 101))
            };
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Decision").strong());
                ui.colored_label(
                    decision.1,
                    egui::RichText::new(decision.0).strong().size(22.0),
                );
                ui.add_space(4.0);
                ui.label(
                    metrics
                        .get("recommended_usage")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Use this model only after reviewing the full report."),
                );
            });
            ui.add_space(30.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Scope fit").strong());
                for (label, key) in [
                    ("COBOL-85 coverage", "cobol85_score"),
                    ("PowerRustCOBOL coverage", "powerrustcobol_score"),
                    ("Unsupported avoided", "unsupported_feature_avoidance"),
                ] {
                    if let Some(v) = Self::metric_score(metrics, key) {
                        Self::draw_metric_bar(
                            ui,
                            label,
                            Self::benchmark_metric_description(label),
                            v,
                        );
                    }
                }
            });
        });

        ui.add_space(12.0);
        ui.columns(3, |cols| {
            Self::draw_benchmark_text_card(
                &mut cols[0],
                "Best at",
                Color32::from_rgb(61, 205, 139),
                &Self::benchmark_text_list(metrics, "strengths"),
                "No structured strengths were returned.",
            );
            Self::draw_benchmark_text_card(
                &mut cols[1],
                "Watch",
                Color32::from_rgb(230, 187, 79),
                &Self::benchmark_text_list(metrics, "weaknesses"),
                "No structured weaknesses were returned.",
            );
            Self::draw_benchmark_text_card(
                &mut cols[2],
                "Failure pattern",
                Color32::from_rgb(238, 101, 101),
                &Self::benchmark_text_list(metrics, "typical_failure_patterns"),
                "No structured failure patterns were returned.",
            );
        });
    }

    fn draw_benchmark_text_card(
        ui: &mut egui::Ui,
        title: &str,
        accent: Color32,
        items: &[String],
        empty: &str,
    ) {
        egui::Frame::NONE
            .fill(Color32::from_rgb(10, 18, 28))
            .stroke(egui::Stroke::new(1.0, accent.linear_multiply(0.8)))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.colored_label(accent, egui::RichText::new(title).strong());
                ui.add_space(4.0);
                if items.is_empty() {
                    ui.weak(empty);
                } else {
                    for item in items.iter().take(4) {
                        ui.label(format!("- {item}"));
                    }
                }
            });
    }

    fn draw_metric_bar(ui: &mut egui::Ui, label: &str, description: &str, value: f32) {
        let width = ui.available_width().clamp(320.0, 760.0);
        let size = egui::vec2(width, 62.0);
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter_at(rect);
        let text_clip =
            egui::Rect::from_min_max(rect.left_top(), rect.right_top() + egui::vec2(-48.0, 36.0));
        let text_painter = painter.with_clip_rect(text_clip);
        let bar_rect = egui::Rect::from_min_max(
            rect.left_bottom() + egui::vec2(0.0, -20.0),
            rect.right_bottom() - egui::vec2(44.0, 6.0),
        );
        text_painter.text(
            rect.left_top() + egui::vec2(0.0, 9.0),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(12.5),
            ui.visuals().text_color(),
        );
        text_painter.text(
            rect.left_top() + egui::vec2(0.0, 30.0),
            egui::Align2::LEFT_CENTER,
            description,
            egui::FontId::proportional(10.5),
            ui.visuals().weak_text_color(),
        );
        painter.rect_filled(bar_rect, 5.0, Color32::from_rgb(20, 31, 42));
        let fill_w = bar_rect.width() * (value / 100.0);
        let fill = egui::Rect::from_min_size(bar_rect.min, egui::vec2(fill_w, bar_rect.height()));
        let color = if value >= 85.0 {
            Color32::from_rgb(61, 205, 139)
        } else if value >= 70.0 {
            Color32::from_rgb(230, 187, 79)
        } else {
            Color32::from_rgb(238, 101, 101)
        };
        painter.rect_filled(fill, 5.0, color);
        painter.text(
            bar_rect.right_center() + egui::vec2(44.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            format!("{value:.0}"),
            egui::FontId::monospace(12.0),
            ui.visuals().strong_text_color(),
        );
    }

    fn draw_metric_radar(
        ui: &mut egui::Ui,
        metrics: &serde_json::Value,
        scores: &[(&str, &str, &str)],
    ) {
        let size = egui::vec2(390.0, 310.0);
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 8.0, Color32::from_rgb(10, 18, 28));
        painter.rect_stroke(
            rect,
            8.0,
            egui::Stroke::new(1.0, Color32::from_rgb(62, 139, 205)),
            egui::StrokeKind::Middle,
        );
        painter.text(
            rect.center_top() + egui::vec2(0.0, 10.0),
            egui::Align2::CENTER_TOP,
            "Ability radar",
            egui::FontId::proportional(16.0),
            ui.visuals().strong_text_color(),
        );
        let center = rect.center() + egui::vec2(0.0, 18.0);
        let radius = 92.0;
        let count = scores.len().max(3);
        for ring in 1..=4 {
            let r = radius * ring as f32 / 4.0;
            let mut pts = Vec::new();
            for i in 0..count {
                let angle =
                    -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / count as f32;
                pts.push(center + egui::vec2(angle.cos() * r, angle.sin() * r));
            }
            painter.add(egui::Shape::closed_line(
                pts,
                egui::Stroke::new(1.0, Color32::from_gray(55)),
            ));
        }
        let mut data = Vec::new();
        for (i, (label, key, fallback_key)) in scores.iter().enumerate() {
            let angle =
                -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / count as f32;
            let outer = center + egui::vec2(angle.cos() * radius, angle.sin() * radius);
            painter.line_segment(
                [center, outer],
                egui::Stroke::new(1.0, Color32::from_gray(50)),
            );
            let v = Self::benchmark_metric_score(metrics, key, fallback_key).unwrap_or(0.0) / 100.0;
            data.push(center + egui::vec2(angle.cos() * radius * v, angle.sin() * radius * v));
            let label_pos =
                center + egui::vec2(angle.cos() * (radius + 42.0), angle.sin() * (radius + 42.0));
            let align = if angle.cos() > 0.35 {
                egui::Align2::LEFT_CENTER
            } else if angle.cos() < -0.35 {
                egui::Align2::RIGHT_CENTER
            } else if angle.sin() < 0.0 {
                egui::Align2::CENTER_BOTTOM
            } else {
                egui::Align2::CENTER_TOP
            };
            let score = (v * 100.0).round() as i32;
            painter.text(
                label_pos,
                align,
                format!("{} {score}", Self::radar_label(label)),
                egui::FontId::proportional(11.0),
                ui.visuals().text_color(),
            );
        }
        painter.add(egui::Shape::convex_polygon(
            data.clone(),
            Color32::from_rgba_unmultiplied(61, 205, 139, 72),
            egui::Stroke::new(2.0, Color32::from_rgb(61, 205, 139)),
        ));
        for p in data {
            painter.circle_filled(p, 3.0, Color32::from_rgb(230, 246, 255));
        }
    }

    fn radar_label(label: &str) -> &'static str {
        match label {
            "Program structure" => "Structure",
            "Data descriptions" => "Data",
            "Control flow" => "Flow",
            "File handling" => "Files",
            "Forms/extensions" => "Forms",
            "Avoid unsupported" => "Supported",
            "Code preservation" => "Preserve",
            "Formatting" => "Format",
            _ => "Score",
        }
    }

    fn render_llm_benchmark_modals(&mut self, ctx: &Context) {
        if let Some(cfg) = self.llm_benchmark_offer.clone() {
            let mut run = false;
            let mut skip = false;
            egui::Window::new("COBOL proficiency check")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_max_width(520.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "Model `{}` is reachable. Do you want to check how proficient it is at producing valid COBOL-85 and PowerRustCOBOL code?",
                            cfg.model
                        ))
                        .strong(),
                    );
                    ui.add_space(6.0);
                    ui.label("This runs a lightweight benchmark prompt, shows a report, and saves the result for later model comparisons. It will not modify the project.");
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Run check").clicked() {
                            run = true;
                        }
                        if ui.button("Not now").clicked() {
                            skip = true;
                        }
                    });
                });
            if run {
                // Spec 028: the pedantic COMPANION from the agent database
                // handles the review side of the check. Resolve it (falls
                // back to the offered legacy config when no DB entry).
                let mut eff = self.designer_effective_llm();
                if !cfg.model.trim().is_empty() && eff.model.trim().is_empty() {
                    eff = cfg.clone();
                }
                if !eff.reviewer_configured() {
                    let tr = self.lang.tr();
                    self.output
                        .push_status(tr.agents_unreviewed_warning.replacen("{}", &eff.model, 1));
                }
                self.start_proficiency_benchmark(eff.clone());
                self.llm_benchmark_offer = None;
            }
            if skip {
                self.llm_benchmark_offer = None;
            }
        }

        if self.llm_benchmark_rx.is_some() {
            egui::Window::new("COBOL proficiency check")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_max_width(420.0);
                    ui.label(
                        self.llm_benchmark_status
                            .as_deref()
                            .unwrap_or("Running COBOL proficiency check..."),
                    );
                    ui.add(egui::Spinner::new());
                });
        }

        if let Some(report) = self.llm_benchmark_report.clone() {
            let mut close = false;
            let mut copy = false;
            let mut save_pdf = false;
            let metrics = Self::llm_benchmark_metrics(&report)
                .unwrap_or_else(|| Self::fallback_benchmark_metrics(&report));
            let benchmark_cfg = self
                .llm_benchmark_config
                .as_ref()
                .unwrap_or(&self.llm)
                .clone();
            egui::Window::new("COBOL proficiency report")
                .id(egui::Id::new("llm_cobol_proficiency_report"))
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(980.0, 700.0))
                .show(ctx, |ui| {
                    ui.label("Use this report to decide whether this model is suitable for COBOL-85 and PowerRustCOBOL work. No project action is applied.");
                    ui.add_space(8.0);
                    let scroll_h = (ctx.content_rect().height() * 0.72).clamp(320.0, 680.0);
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .max_height(scroll_h)
                        .show(ui, |ui| {
                            let opts = crate::panels::md_render::RenderOpts {
                                search: "",
                                base: 15.0,
                                scroll_to_heading: None,
                                active_match: None,
                                scroll_to_active: false,
                                anchors: &[],
                                table_layout: crate::panels::md_render::TableLayout::Equal,
                            };
                            Self::render_benchmark_metadata(ui, &benchmark_cfg, &metrics, &report);
                            ui.add_space(12.0);
                            Self::render_llm_benchmark_dashboard(ui, &metrics);
                            ui.add_space(14.0);
                            crate::panels::md_render::render(
                                ui,
                                &Self::benchmark_summary_markdown(&metrics),
                                &opts,
                                &mut |ui, code| {
                                    ui.label(egui::RichText::new(code).monospace());
                                },
                            );
                            crate::panels::md_render::render(
                                ui,
                                &Self::benchmark_tested_points_markdown(&metrics),
                                &opts,
                                &mut |ui, code| {
                                    ui.label(egui::RichText::new(code).monospace());
                                },
                            );
                            crate::panels::md_render::render(
                                ui,
                                &Self::benchmark_generated_cobol_markdown(&report),
                                &opts,
                                &mut |ui, code| {
                                    ui.label(egui::RichText::new(code).monospace());
                                },
                            );
                            ui.heading("Model report");
                            ui.add_space(4.0);
                            crate::panels::md_render::render(ui, &report, &opts, &mut |ui, code| {
                                ui.label(egui::RichText::new(code).monospace());
                            });
                        });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Copy").clicked() {
                            copy = true;
                        }
                        if ui.button("Save as PDF").clicked() {
                            save_pdf = true;
                        }
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                });
            if copy {
                ctx.copy_text(report.clone());
            }
            if save_pdf {
                let model_id = sanitize_filename_component(&benchmark_cfg.model);
                let pdf_name = format!("cobol-proficiency-report-{model_id}.pdf");
                self.begin_file_dialog(
                    FileRequest::SaveBenchmarkPdf(report.clone()),
                    crate::file_dialog::DialogSpec::save()
                        .filter("PDF", &["pdf"])
                        .file_name(&pdf_name),
                );
            }
            if close {
                self.llm_benchmark_report = None;
            }
        }
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

    /// Spec 028 R8: resolve the designer agent's effective connection from
    /// the project agent database ("Form Designer Agent" entry), falling
    /// back to the legacy config. Loaded fresh per send — sends are rare and
    /// this can never go stale.
    fn designer_effective_llm(&self) -> crate::llm::LlmConfig {
        let Some(dir) = self.project_path.as_ref().and_then(|p| p.parent()) else {
            return self.llm.clone();
        };
        let db = crate::agents_db::AgentsDb::load(dir);
        crate::agents_db::designer_agent_config(&db, &self.llm)
    }

    /// Poll the reviewer-model list fetch (Pedantic Agent model picker).
    fn poll_llm_reviewer_models(&mut self) {
        let result = match &self.llm_reviewer_models_rx {
            Some(rx) => match rx.try_recv() {
                Ok(r) => Some(r),
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err("The model-list worker stopped unexpectedly.".into()))
                }
            },
            None => return,
        };
        self.llm_reviewer_models_rx = None;
        match result {
            Some(Ok(models)) => {
                if let Some(form) = &mut self.settings_form {
                    form.set_available_reviewer_models(models);
                }
            }
            Some(Err(e)) => {
                self.llm_test_status = Some(e);
            }
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

    /// Persist the Settings form: write the draft and project-owned AI metadata
    /// into the project, while credentials stay in the machine-local store.
    fn save_settings_form(&mut self) {
        let Some(form) = &mut self.settings_form else {
            return;
        };
        let Some(proj) = &mut self.cobolt_project else {
            return;
        };
        form.draft.apply(proj, &mut self.llm);
        form.mark_saved();
        self.persist_active_project_ai();
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
    fn show_settings_pane(&mut self, panel_ui: &mut egui::Ui, tr: &Tr) {
        // Panels are Ui-hosted since egui 0.35; everything else in this
        // method still wants a Context.
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        self.poll_llm_test(tr);
        self.poll_llm_models();
        self.poll_llm_reviewer_models();
        // 👑 Grace workflow (spec 029 Phase C): drain progress every frame so
        // it advances even when the agent bar is not the visible pane.
        if let Some(sess) = self.grace_session.as_mut() {
            if sess.poll() {
                ctx.request_repaint();
            }
            // Keep the live-UI snapshot fresh so specialists' egui observe tools
            // read the current frame (spec 030 R4).
            if sess.is_running() {
                crate::agent_inspection::request_snapshot(ctx);
            }
        }
        // Once a workflow finishes, apply its approved Form-Designer output to the
        // originating form as one undoable, reviewable change (spec 030 R6/R7).
        self.apply_grace_form_output();
        // Agents Manager modal (spec 028) — taken out of self to split borrows.
        if let Some(mut m) = self.agents_modal.take() {
            let act = m.show(ctx, &mut self.llm, &self.leaderboard, &self.lang.tr());
            if m.open {
                self.agents_modal = Some(m);
            } else if self.leaderboard_modal.is_some() {
                // The judge may have just been given a model in there; the
                // board asks again rather than holding a stale answer.
                let ready = self.judge_has_model();
                if let Some(lb) = self.leaderboard_modal.as_mut() {
                    lb.set_judge_ready(ready);
                }
            }
            // "Check proficiency" on a specialist: run the tandem benchmark for
            // its resolved config (its model, reviewed by its pedantic companion
            // when set) — the report window opens on top (spec 029).
            if let Some(cfg) = act.run_proficiency {
                if !cfg.reviewer_configured() {
                    let tr = self.lang.tr();
                    self.output
                        .push_status(tr.agents_unreviewed_warning.replacen("{}", &cfg.model, 1));
                }
                self.start_proficiency_benchmark(cfg.clone());
            }
            if act.applied {
                self.persist_active_project_ai();
            }
        }
        // Model Leaderboard (spec 040) — taken out of self to split borrows.
        if let Some(mut m) = self.leaderboard_modal.take() {
            let theme = self.current_theme();
            let act = m.show(ctx, &self.leaderboard, &self.llm, theme, &self.lang.tr());
            if m.open {
                self.leaderboard_modal = Some(m);
            }
            self.handle_leaderboard_action(act);
        }
        // Models Manager modal (spec 031) — taken out of self to split borrows.
        if let Some(mut m) = self.models_modal.take() {
            let act = m.show(ctx, &mut self.llm, &self.lang.tr());
            if m.open {
                self.models_modal = Some(m);
            }
            for line in act.log_lines {
                self.output.push_status(line);
            }
            // A catalogue that actually listed models is the only thing that
            // can tell a decommissioned model from an unreachable provider.
            if let Some((provider, catalogue)) = act.catalogue {
                self.retire_decommissioned_models(&provider, &catalogue);
            }
            if let Some(err) = act.alert_error {
                crate::llm::push_connection_log(&format!("=== MODELS MANAGER ERROR ===\n{err}\n"));
                // The dialog shows the whole connection log, which is the right
                // thing to READ while diagnosing and the wrong thing to copy
                // into the console — it is already kept, in full, by
                // `push_connection_log` above. What the console records is the
                // error itself (operator, 2026-08-09).
                self.output.push_status(format!("✗ {err}"));
                self.alert_error = Some(crate::llm::connection_log_text());
            }
            if act.applied {
                self.persist_active_project_ai();
                if let Some(root) = self.project_dir() {
                    self.ensure_project_agent_system(&root);
                }
                if act.save_requested {
                    self.output
                        .push_status("Models Manager: project settings saved to disk.");
                }
                if !self.llm.agentic_ai_enabled {
                    self.use_grace = true;
                    self.agent_pending = None;
                    self.grace_session = None;
                    self.agent_prompt.clear();
                    self.agent_status = None;
                    self.agent_preview = None;
                }
            }
            // Proficiency testing lives in the Leaderboard and nowhere else
            // (spec 048 R15/R16). It scores a MODEL, and the Leaderboard is
            // where models are compared and their history kept — running it
            // from the provider manager benchmarked the same model repeatedly
            // and recorded it in a window that could not show the result.
            if act.semantic_download_requested && self.semantic_download.is_none() {
                self.start_semantic_download();
            }
            ctx.request_repaint();
        }
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
        let mut card = crate::theme::glass_panel_frame(
            ctx.global_style().visuals.panel_fill,
            self.current_theme(),
        );
        // Moderate bottom outer margin on the frame raises the stroked glass
        // card (rounded bottom border) clearly above the output.
        // Inside the framed ui we allocate the form (scroll + buttons) in a
        // shorter rect + reserve space so the Save/Cancel sit fully visible
        // above the console (the 80px inner reservation directly lifts the
        // buttons within the glass). This fixes the clipping while keeping the
        // overall "right pane" full 100% height conforming.
        card = card.outer_margin(egui::Margin {
            left: 6,
            right: 6,
            top: 6,
            bottom: 50,
        });
        egui::CentralPanel::default()
            .frame(card)
            .show(panel_ui, |ui| {
                if let Some(form) = &mut self.settings_form {
                    let avail = ui.available_rect_before_wrap();
                    let bottom_res = 80.0; // dedicated inner lift for full button visibility
                    let content_h = (avail.height() - bottom_res).max(180.0);
                    let content_rect =
                        egui::Rect::from_min_size(avail.min, egui::vec2(avail.width(), content_h));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                        action = form.show(
                            ui,
                            tr,
                            &themes,
                            test_busy,
                            test_status.as_deref(),
                            has_debug,
                            &prompt_known_controls,
                            self.debug.no_window_fx,
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
                cfg.provider = form.draft.llm_provider.clone();
                cfg.endpoint = form.draft.llm_endpoint.clone();
                cfg.api_key = form.draft.llm_api_key.clone();
                cfg.model = form.draft.llm_model.clone();
                self.llm_test_status = Some(tr.ai_testing.to_string());
                self.llm_test_error = None;
                self.llm_test_from_model_selection = action.test_connection_from_model_selection;
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
        if action.manage_agents {
            if let Some(dir) = self
                .project_path
                .as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf())
            {
                self.agents_modal = Some(crate::panels::agents_modal::AgentsModal::open_for(
                    &dir,
                    &mut self.llm,
                ));
                // Opening can migrate legacy embedded agent model settings onto
                // project model profiles. Persist that migration immediately.
                self.persist_active_project_ai();
            }
        }
        if action.open_leaderboard {
            // Pick up models configured since the last sync, so the board is
            // never a shorter list than the Models Manager.
            self.sync_leaderboard_models();
            // …and drop the rows nothing registers any more, so it is never a
            // LONGER list either.
            self.prune_leaderboard_orphans();
            let judge_ready = self.judge_has_model();
            self.leaderboard_modal = Some(
                crate::panels::leaderboard_modal::LeaderboardModal::new(judge_ready),
            );
        }
        if action.manage_models {
            self.models_modal = Some(crate::panels::models_modal::ModelsModal::new());
        }
        if action.fetch_reviewer_models {
            if let Some(form) = &self.settings_form {
                if let Some(provider) =
                    crate::llm::Provider::from_id(&form.draft.llm_reviewer_provider)
                {
                    self.llm_reviewer_models_rx = Some(crate::llm::spawn_list_models(
                        provider,
                        &form.draft.llm_reviewer_endpoint,
                        &form.draft.llm_reviewer_api_key,
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
    fn show_mascot_pane(&mut self, panel_ui: &mut egui::Ui, tr: &Tr) {
        // Panels are Ui-hosted since egui 0.35; everything else in this
        // method still wants a Context.
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        let card = crate::theme::glass_panel_frame(
            ctx.global_style().visuals.panel_fill,
            self.current_theme(),
        );
        egui::CentralPanel::default()
            .frame(card)
            .show(panel_ui, |ui| {
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
    fn show_welcome_pane(&mut self, panel_ui: &mut egui::Ui, tr: &Tr) {
        // Panels are Ui-hosted since egui 0.35; everything else in this
        // method still wants a Context.
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        egui::CentralPanel::default().show(panel_ui, |ui| {
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

            // Plain form for MEASUREMENT (same glyphs as the rich layout);
            // the rendered label is the two-color brand job below.
            let brand_with_version =
                format!("{} {}", crate::theme::brand_name(), crate::version::VERSION);
            let title = tr.welcome_title.replace("{}", &brand_with_version);
            // Split the localized template around its {} so the brand keeps
            // its colored "AI" in every language.
            let (welcome_prefix, welcome_suffix) =
                tr.welcome_title.split_once("{}").unwrap_or(("", ""));
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
                ui.fonts_mut(|f| {
                    f.layout_no_wrap(
                        text.to_owned(),
                        egui::FontId::proportional(size),
                        egui::Color32::WHITE,
                    )
                    .size()
                    .y
                })
            };
            let quote_h = ui.fonts_mut(|f| {
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

            // egui has no text-outline style, so the 1 px black outline is
            // painted by hand: the same galley eight times at the 1 px
            // neighbour offsets with every glyph forced black, then the
            // coloured galley on top — readable even where the photo behind
            // the text is bright despite the scrim. `outline_alpha` follows
            // the quote fade so the outline never lingers after its text.
            fn outlined_label(
                ui: &mut egui::Ui,
                mut job: egui::text::LayoutJob,
                outline_alpha: f32,
            ) {
                job.halign = egui::Align::Center;
                let galley = ui.fonts_mut(|f| f.layout_job(job));
                let (rect, _) =
                    ui.allocate_exact_size(galley.size(), egui::Sense::hover());
                let pos = rect.center_top();
                let outline = egui::Color32::BLACK.gamma_multiply(outline_alpha);
                for dx in [-1.0_f32, 0.0, 1.0] {
                    for dy in [-1.0_f32, 0.0, 1.0] {
                        if dx == 0.0 && dy == 0.0 {
                            continue;
                        }
                        let mut shadow = egui::epaint::TextShape::new(
                            pos + egui::vec2(dx, dy),
                            galley.clone(),
                            outline,
                        );
                        shadow.override_text_color = Some(outline);
                        ui.painter().add(shadow);
                    }
                }
                ui.painter()
                    .add(egui::epaint::TextShape::new(pos, galley, egui::Color32::WHITE));
            }

            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0; // gaps are inserted explicitly
                let top = ((ui.available_height() - block_h) * 0.5).max(0.0);
                ui.add_space(top);

                let mut title_job = crate::theme::brand_layout_job(
                    welcome_prefix,
                    &format!(" {}{welcome_suffix}", crate::version::VERSION),
                    TITLE_SIZE,
                    egui::Color32::WHITE,
                );
                title_job.wrap.max_width = avail_w;
                outlined_label(ui, title_job, 1.0);
                ui.add_space(GAP_LICENSE);
                outlined_label(
                    ui,
                    egui::text::LayoutJob::simple(
                        license.to_owned(),
                        egui::FontId::proportional(14.0),
                        egui::Color32::WHITE,
                        avail_w,
                    ),
                    1.0,
                );
                ui.add_space(GAP_QUOTE);
                outlined_label(
                    ui,
                    egui::text::LayoutJob::simple(
                        quote.to_owned(),
                        egui::FontId::proportional(16.0),
                        green.gamma_multiply(alpha),
                        avail_w,
                    ),
                    alpha,
                );
                ui.add_space(GAP_AUTHOR);
                let mut author_job = egui::text::LayoutJob::default();
                author_job.append(
                    &author_line,
                    0.0,
                    egui::TextFormat {
                        font_id: egui::FontId::proportional(15.0),
                        color: light_blue.gamma_multiply(alpha),
                        italics: true,
                        ..Default::default()
                    },
                );
                outlined_label(ui, author_job, alpha);
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

    /// True when anything is changed and not saved: an open form designer with
    /// pending edits, a dirty code-editor tab, or unsaved project settings.
    /// Drives the on-close confirmation dialog.
    fn has_unsaved_changes(&self) -> bool {
        self.designers.iter().any(|(_, d)| d.dirty)
            || self.inspect.as_ref().is_some_and(|st| st.designer.dirty)
            || self.editor.any_dirty()
            || self.settings_dirty()
    }

    /// Persist everything unsaved, used by the close dialog's "Save before close".
    /// Saves each dirty form (regenerating its COBOL), all dirty editor tabs, and
    /// the project settings form when dirty.
    fn save_all_unsaved(&mut self) {
        let dirty_designers: Vec<usize> = self
            .designers
            .iter()
            .enumerate()
            .filter(|(_, (_, d))| d.dirty)
            .map(|(i, _)| i)
            .collect();
        for idx in dirty_designers {
            self.do_save_designer(idx);
        }
        // The Main-Pane inspector holds its own transient designer; an inline edit
        // whose auto-save the data-binding gate blocked is still unsaved.
        if let Some(st) = &mut self.inspect {
            if st.designer.dirty && save_form(&st.designer.form, &st.path).is_ok() {
                st.designer.dirty = false;
                st.mtime = file_mtime(&st.path);
                let p = st.path.clone();
                self.project.refresh_form(&p);
                self.after_form_saved(&p);
            }
        }
        if let Err(e) = self.editor.save_all_dirty() {
            self.output.push_status(format!("Save failed: {e}"));
        }
        if self.settings_dirty() {
            self.save_settings_form();
        }
    }

    /// Open a file in the editor, marking RAD-generated COBOL read-only (blue).
    fn open_in_editor(&mut self, path: PathBuf) {
        if self.path_is_asset(&path) {
            self.open_asset_preview(path);
            return;
        }
        self.asset_preview = None;
        let read_only = self.path_is_generated(&path);
        self.editor.open_file_ro(path, read_only);
    }

    fn path_is_asset(&self, path: &std::path::Path) -> bool {
        self.project_dir()
            .and_then(|dir| relative_to(path, &dir))
            .map(|rel| {
                let rel = rel.replace('\\', "/").to_ascii_lowercase();
                rel.starts_with("assets/")
            })
            .unwrap_or(false)
    }

    fn open_asset_preview(&mut self, path: PathBuf) {
        let rel = self
            .project_dir()
            .and_then(|dir| relative_to(&path, &dir))
            .unwrap_or_else(|| path.display().to_string());
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let content = if Self::asset_text_ext(&ext) {
            std::fs::read_to_string(&path)
                .map(AssetPreviewContent::Text)
                .unwrap_or_else(|_| {
                    AssetPreviewContent::Binary(std::fs::read(&path).unwrap_or_default())
                })
        } else if Self::asset_animation_ext(&ext) {
            self.load_asset_animation(&path)
                .or_else(|| self.load_asset_image(&path, &ext))
                .unwrap_or_else(|| {
                    AssetPreviewContent::Binary(std::fs::read(&path).unwrap_or_default())
                })
        } else if Self::asset_image_ext(&ext) {
            self.load_asset_image(&path, &ext).unwrap_or_else(|| {
                AssetPreviewContent::Binary(std::fs::read(&path).unwrap_or_default())
            })
        } else {
            AssetPreviewContent::Binary(std::fs::read(&path).unwrap_or_default())
        };
        self.asset_preview = Some(AssetPreviewState {
            path,
            rel,
            content,
            zoom_percent: 100.0,
            search_open: false,
            search_query: String::new(),
            animation_playing: false,
            animation_frame: 0,
            animation_last_tick: None,
        });
    }

    fn load_asset_image(&self, path: &Path, ext: &str) -> Option<AssetPreviewContent> {
        if ext.eq_ignore_ascii_case("svg") {
            let tex =
                cobolt_forms::paint::load_image_texture(&self.egui_ctx, &path.to_string_lossy())?;
            return Some(AssetPreviewContent::Image {
                size: tex.size_vec2(),
                svg_path: Some(path.to_path_buf()),
                texture: tex,
            });
        }
        let bytes = std::fs::read(path).ok()?;
        let img = image::load_from_memory(&bytes).ok()?;
        let rgba = img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        let tex = self.egui_ctx.load_texture(
            format!("asset-preview:{}", path.display()),
            color,
            egui::TextureOptions::LINEAR,
        );
        Some(AssetPreviewContent::Image {
            size: tex.size_vec2(),
            svg_path: None,
            texture: tex,
        })
    }

    fn load_asset_animation(&self, path: &Path) -> Option<AssetPreviewContent> {
        use image::AnimationDecoder;

        let file = std::fs::File::open(path).ok()?;
        let reader = std::io::BufReader::new(file);
        let decoder = image::codecs::gif::GifDecoder::new(reader).ok()?;
        let frames = decoder.into_frames().collect_frames().ok()?;
        let mut preview_frames = Vec::new();
        let mut logical_size = egui::Vec2::ZERO;
        for (idx, frame) in frames.into_iter().enumerate() {
            let (num, den) = frame.delay().numer_denom_ms();
            let delay_ms = if den == 0 {
                100
            } else {
                ((num as f32 / den as f32).round() as u64).max(20)
            };
            let rgba = frame.into_buffer();
            let size = [rgba.width() as usize, rgba.height() as usize];
            if logical_size == egui::Vec2::ZERO {
                logical_size = egui::vec2(size[0] as f32, size[1] as f32);
            }
            let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            let texture = self.egui_ctx.load_texture(
                format!("asset-animation:{}:{idx}", path.display()),
                color,
                egui::TextureOptions::LINEAR,
            );
            preview_frames.push(AssetAnimationFrame {
                texture,
                delay: std::time::Duration::from_millis(delay_ms),
            });
        }
        (!preview_frames.is_empty()).then_some(AssetPreviewContent::Animation {
            frames: preview_frames,
            size: logical_size,
        })
    }

    fn asset_text_ext(ext: &str) -> bool {
        matches!(
            ext,
            "txt"
                | "md"
                | "markdown"
                | "ini"
                | "json"
                | "toml"
                | "yaml"
                | "yml"
                | "csv"
                | "tsv"
                | "xml"
                | "html"
                | "htm"
                | "css"
                | "js"
                | "rs"
                | "cbl"
                | "cob"
                | "cpy"
                | "log"
        )
    }

    fn asset_image_ext(ext: &str) -> bool {
        matches!(
            ext,
            "png"
                | "jpg"
                | "jpeg"
                | "bmp"
                | "gif"
                | "webp"
                | "apng"
                | "tif"
                | "tiff"
                | "avif"
                | "svg"
        )
    }

    fn asset_animation_ext(ext: &str) -> bool {
        matches!(ext, "gif")
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

    fn show_asset_preview(&mut self, panel_ui: &mut egui::Ui, _tr: &Tr) {
        // Panels are Ui-hosted since egui 0.35; everything else in this
        // method still wants a Context.
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        let Some(preview) = self.asset_preview.clone() else {
            return;
        };
        let search_shortcut =
            ctx.input(|i| i.key_pressed(egui::Key::F) && (i.modifiers.command || i.modifiers.ctrl));
        if search_shortcut {
            if let Some(p) = self.asset_preview.as_mut() {
                p.search_open = true;
            }
        }
        self.advance_asset_animation(ctx);

        let theme = crate::theme::active();
        let frame = crate::theme::glass_panel_frame(ctx.global_style().visuals.panel_fill, &theme);
        let mut close = false;
        let mut zoom_delta = 0.0;
        let mut zoom_exact: Option<f32> = None;
        let mut toggle_play = false;
        egui::CentralPanel::default()
            .frame(frame)
            .show(panel_ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Asset preview");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                });
                ui.label(
                    egui::RichText::new(preview.rel.as_str())
                        .small()
                        .color(theme.text_dim),
                );
                ui.separator();

                let is_image = matches!(
                    &preview.content,
                    AssetPreviewContent::Image { .. } | AssetPreviewContent::Animation { .. }
                );
                let is_animation =
                    matches!(&preview.content, AssetPreviewContent::Animation { .. });
                if is_image {
                    ui.horizontal(|ui| {
                        if ui.button("-").on_hover_text("Zoom out").clicked() {
                            zoom_delta = -10.0;
                        }
                        let mut z = format!("{:.0}%", preview.zoom_percent);
                        let resp = ui.add_sized(
                            egui::vec2(70.0, 28.0),
                            egui::TextEdit::singleline(&mut z)
                                .horizontal_align(egui::Align::Center),
                        );
                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            let cleaned = z.trim().trim_end_matches('%');
                            if let Ok(value) = cleaned.parse::<f32>() {
                                zoom_exact = Some(value);
                            }
                        }
                        if ui.button("+").on_hover_text("Zoom in").clicked() {
                            zoom_delta = 10.0;
                        }
                        let mut slider_zoom = if preview.zoom_percent <= 0.0 {
                            100.0
                        } else {
                            preview.zoom_percent
                        };
                        let slider = egui::Slider::new(&mut slider_zoom, 10.0..=999.0)
                            .show_value(false)
                            .text("Zoom");
                        if ui.add_sized(egui::vec2(220.0, 24.0), slider).changed() {
                            zoom_exact = Some(slider_zoom);
                        }
                        if ui.button("Fit").clicked() {
                            zoom_exact = Some(0.0);
                        }
                        if is_animation {
                            let label = if preview.animation_playing {
                                "Pause"
                            } else {
                                "Play"
                            };
                            if ui.button(label).clicked() {
                                toggle_play = true;
                            }
                            if let AssetPreviewContent::Animation { frames, .. } = &preview.content
                            {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Frame {}/{}",
                                        preview.animation_frame.min(frames.len().saturating_sub(1))
                                            + 1,
                                        frames.len()
                                    ))
                                    .small()
                                    .color(theme.text_dim),
                                );
                            }
                        }
                    });
                    ui.separator();
                } else if let AssetPreviewContent::Text(text) = &preview.content {
                    if preview.search_open {
                        ui.horizontal(|ui| {
                            ui.label("Search");
                            let mut query = preview.search_query.clone();
                            let resp = ui.add_sized(
                                egui::vec2(260.0, 28.0),
                                egui::TextEdit::singleline(&mut query).hint_text("Command+F"),
                            );
                            if resp.changed() {
                                if let Some(p) = self.asset_preview.as_mut() {
                                    p.search_query = query.clone();
                                }
                            }
                            let count = if query.is_empty() {
                                0
                            } else {
                                text.to_ascii_lowercase()
                                    .matches(&query.to_ascii_lowercase())
                                    .count()
                            };
                            ui.label(
                                egui::RichText::new(format!("{count} match(es)"))
                                    .small()
                                    .color(theme.text_dim),
                            );
                        });
                        ui.separator();
                    }
                }

                let metadata_h = 154.0;
                let content_h = (ui.available_height() - metadata_h).max(120.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), content_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| match &preview.content {
                        AssetPreviewContent::Image {
                            texture,
                            size,
                            svg_path,
                        } => {
                            let avail = ui.available_size();
                            let fit = (avail.x / size.x).min(avail.y / size.y).min(1.0).max(0.05);
                            let scale = if preview.zoom_percent <= 0.0 {
                                fit
                            } else {
                                preview.zoom_percent / 100.0
                            };
                            let display = *size * scale;
                            egui::ScrollArea::both()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.vertical_centered(|ui| {
                                        let svg_texture = svg_path.as_ref().and_then(|path| {
                                            cobolt_forms::paint::load_svg_texture_at_size(
                                                ui.ctx(),
                                                &path.to_string_lossy(),
                                                display,
                                            )
                                        });
                                        let texture = svg_texture.as_ref().unwrap_or(texture);
                                        Self::draw_asset_image_preview(ui, texture, display);
                                        ui.add_space(6.0);
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{:.0} × {:.0}px",
                                                size.x, size.y
                                            ))
                                            .small()
                                            .color(theme.text_dim),
                                        );
                                    });
                                });
                        }
                        AssetPreviewContent::Animation { frames, size } => {
                            if let Some(frame) =
                                frames.get(preview.animation_frame.min(frames.len() - 1))
                            {
                                let avail = ui.available_size();
                                let fit =
                                    (avail.x / size.x).min(avail.y / size.y).min(1.0).max(0.05);
                                let scale = if preview.zoom_percent <= 0.0 {
                                    fit
                                } else {
                                    preview.zoom_percent / 100.0
                                };
                                let display = *size * scale;
                                egui::ScrollArea::both().auto_shrink([false, false]).show(
                                    ui,
                                    |ui| {
                                        ui.vertical_centered(|ui| {
                                            Self::draw_asset_image_preview(
                                                ui,
                                                &frame.texture,
                                                display,
                                            );
                                        });
                                    },
                                );
                            }
                        }
                        AssetPreviewContent::Text(text) => {
                            egui::ScrollArea::both()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let mut display = text.as_str();
                                    ui.add(
                                        egui::TextEdit::multiline(&mut display)
                                            .font(egui::TextStyle::Monospace)
                                            .desired_width(f32::INFINITY)
                                            .lock_focus(true)
                                            .interactive(false),
                                    );
                                });
                        }
                        AssetPreviewContent::Binary(bytes) => {
                            let mut dump = Self::binary_hex_ascii(bytes);
                            egui::ScrollArea::both()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::multiline(&mut dump)
                                            .font(egui::TextStyle::Monospace)
                                            .desired_width(f32::INFINITY)
                                            .interactive(false),
                                    );
                                });
                        }
                    },
                );
                ui.separator();
                self.asset_metadata_table(ui, &preview);
                ui.add_space(18.0);
            });
        if close {
            self.asset_preview = None;
        }
        if let Some(p) = self.asset_preview.as_mut() {
            if let Some(z) = zoom_exact {
                p.zoom_percent = z.clamp(0.0, 999.0);
            }
            if zoom_delta != 0.0 {
                let base = if p.zoom_percent <= 0.0 {
                    100.0
                } else {
                    p.zoom_percent
                };
                p.zoom_percent = (base + zoom_delta).clamp(10.0, 999.0);
            }
            if toggle_play {
                p.animation_playing = !p.animation_playing;
                p.animation_last_tick = Some(std::time::Instant::now());
            }
        }
    }

    fn advance_asset_animation(&mut self, ctx: &Context) {
        let Some(preview) = self.asset_preview.as_mut() else {
            return;
        };
        if !preview.animation_playing {
            return;
        }
        let AssetPreviewContent::Animation { frames, .. } = &preview.content else {
            return;
        };
        if frames.is_empty() {
            return;
        }
        let now = std::time::Instant::now();
        let idx = preview.animation_frame.min(frames.len() - 1);
        let due = preview.animation_last_tick.unwrap_or(now) + frames[idx].delay;
        if now >= due {
            preview.animation_frame = (idx + 1) % frames.len();
            preview.animation_last_tick = Some(now);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }

    fn binary_hex_ascii(bytes: &[u8]) -> String {
        let mut out = String::new();
        for (offset, chunk) in bytes.chunks(16).enumerate() {
            use std::fmt::Write as _;
            let _ = write!(out, "{:08X}  ", offset * 16);
            for i in 0..16 {
                if let Some(b) = chunk.get(i) {
                    let _ = write!(out, "{b:02X} ");
                } else {
                    out.push_str("   ");
                }
                if i == 7 {
                    out.push(' ');
                }
            }
            out.push(' ');
            for b in chunk {
                let ch = if b.is_ascii_graphic() || *b == b' ' {
                    *b as char
                } else {
                    '.'
                };
                out.push(ch);
            }
            out.push('\n');
        }
        out
    }

    fn asset_metadata_table(&self, ui: &mut egui::Ui, preview: &AssetPreviewState) {
        let meta = std::fs::metadata(&preview.path).ok();
        let size = meta
            .as_ref()
            .map(|m| format!("{} bytes", m.len()))
            .unwrap_or_else(|| "unknown".to_string());
        let modified = meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| format!("{} seconds since Unix epoch", d.as_secs()))
            .unwrap_or_else(|| "unknown".to_string());
        let ext = preview
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let kind = match &preview.content {
            AssetPreviewContent::Image { .. } => "Image",
            AssetPreviewContent::Animation { .. } => "Animation",
            AssetPreviewContent::Text(_) => "Text",
            AssetPreviewContent::Binary(_) => "Binary",
        };
        let dimensions = match &preview.content {
            AssetPreviewContent::Image { size, .. }
            | AssetPreviewContent::Animation { size, .. } => {
                format!("{:.0} x {:.0}px", size.x, size.y)
            }
            _ => "-".to_string(),
        };
        let frame_count = match &preview.content {
            AssetPreviewContent::Animation { frames, .. } => frames.len().to_string(),
            _ => "-".to_string(),
        };

        ui.label(egui::RichText::new("Metadata").strong());
        egui::Grid::new("asset_metadata_table")
            .num_columns(4)
            .striped(true)
            .min_col_width(120.0)
            .show(ui, |ui| {
                ui.label("File");
                ui.label(
                    preview
                        .path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(""),
                );
                ui.label("Type");
                ui.label(kind);
                ui.end_row();

                ui.label("Path");
                ui.label(preview.path.display().to_string());
                ui.label("Extension");
                ui.label(if ext.is_empty() { "-" } else { ext.as_str() });
                ui.end_row();

                ui.label("Size");
                ui.label(size);
                ui.label("Dimensions");
                ui.label(dimensions);
                ui.end_row();

                ui.label("Modified");
                ui.label(modified);
                ui.label("Frames");
                ui.label(frame_count);
                ui.end_row();
            });
    }

    fn draw_asset_image_preview(
        ui: &mut egui::Ui,
        texture: &egui::TextureHandle,
        display: egui::Vec2,
    ) {
        let padding = egui::vec2(24.0, 24.0);
        let outer_size = display + padding * 2.0;
        let (outer, _) = ui.allocate_exact_size(outer_size, egui::Sense::hover());
        let painter = ui.painter_at(outer);
        let bg = egui::Color32::from_rgb(226, 232, 240);
        let alt = egui::Color32::from_rgb(176, 186, 200);
        painter.rect_filled(outer, 6.0, bg);

        let tile = 12.0;
        let cols = (outer.width() / tile).ceil() as i32;
        let rows = (outer.height() / tile).ceil() as i32;
        for y in 0..rows {
            for x in 0..cols {
                if (x + y) % 2 == 0 {
                    let min = outer.min + egui::vec2(x as f32 * tile, y as f32 * tile);
                    let max = egui::pos2(
                        (min.x + tile).min(outer.max.x),
                        (min.y + tile).min(outer.max.y),
                    );
                    painter.rect_filled(egui::Rect::from_min_max(min, max), 0.0, alt);
                }
            }
        }

        painter.rect_stroke(
            outer,
            6.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(92, 111, 128)),
            egui::StrokeKind::Middle,
        );
        let image_rect = egui::Rect::from_center_size(outer.center(), display);
        painter.image(
            texture.id(),
            image_rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
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

    /// True while any error or confirmation dialog is waiting for the user.
    /// The always-on-top auxiliary OS windows (debugger, Run-Form Inspector)
    /// drop to a normal level while this holds, so they cannot sit over a
    /// question the user has to answer.
    fn blocking_modal_open(&self) -> bool {
        self.form_error.is_some()
            || self.alert_error.is_some()
            || self.pending_proc_delete.is_some()
            || self.pending_user_control_delete.is_some()
            || self.pending_indexed_delete.is_some()
            || self.designers.iter().any(|(_, d)| d.has_blocking_modal())
            || self
                .inspect
                .as_ref()
                .is_some_and(|st| st.designer.has_blocking_modal())
    }

    /// Modal shown when a form's generated COBOL fails to launch (parse /
    /// semantic error) or the interpreter reports a fatal runtime error. The
    /// message is also in the Output console; this dialog just makes it
    /// unmissable. Closing it leaves the IDE fully usable.
    /// Ask before deleting a common procedure, and only then remove it.
    ///
    /// A common procedure is code the developer wrote by hand. Pressing its
    /// delete button is an explicit request, but an explicit request is still
    /// not a licence to destroy work without a word — so the body's size is
    /// shown, the default is Cancel, and the removal is undoable.
    fn show_proc_delete_confirmation(&mut self, ctx: &Context) {
        let Some(pending) = self.pending_proc_delete.clone() else {
            return;
        };
        let tr = self.lang.tr();
        let message = tr
            .proc_delete_confirm_message
            .replace("{name}", &pending.name)
            .replace("{lines}", &pending.lines.to_string());
        let mut cancel = false;
        let mut confirm = false;

        let win_id = egui::Id::new("proc_delete_confirm");
        raise_modal_layer(ctx, win_id);
        egui::Window::new(tr.proc_delete_confirm_title)
            .id(win_id)
            .order(egui::Order::Foreground) // above every ordinary window
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(message);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr.delete_confirm_cancel).clicked() {
                        cancel = true;
                    }
                    if ui.button(tr.delete_confirm_ok).clicked() {
                        confirm = true;
                    }
                });
            });

        if cancel {
            self.pending_proc_delete = None;
            return;
        }
        if !confirm {
            return;
        }
        self.pending_proc_delete = None;
        match pending.designer {
            Some(idx) => {
                if idx < self.designers.len() {
                    let d = &mut self.designers[idx].1;
                    if pending.index < d.form.user_procedures.len() {
                        // Undoable, with the full body on the stack.
                        d.remove_user_procedure(pending.index);
                        if matches!(
                            d.cobol_structure_edit,
                            Some(crate::panels::cobol_structure::CsTarget::Procedure(_))
                        ) {
                            d.cobol_structure_edit = None;
                        }
                    }
                }
            }
            None => {
                if let Some(st) = &mut self.inspect {
                    if pending.index < st.designer.form.user_procedures.len() {
                        st.designer.remove_user_procedure(pending.index);
                    }
                }
            }
        }
        self.output.push_status(format!(
            "Deleted procedure {} — undo restores it.",
            pending.name
        ));
    }

    /// "This project was last fully built by an older PowerRustCOBOL" — shown
    /// on every Run until a full build actually happens.
    /// True while the pending stale-build prompt was raised by a still-open
    /// designer's Run Form — that designer's viewport hosts the prompt, and
    /// the main window must not render it a second time.
    fn stale_prompt_hosted_by_open_designer(&self) -> bool {
        matches!(
            self.stale_build_prompt.as_ref().map(|p| &p.intent),
            Some(StaleBuildIntent::RunForm(idx)) if *idx < self.designers.len()
        )
    }

    /// True while a still-open designer's Run Form owns the current build —
    /// that designer's viewport hosts the build modal **and** the build-details
    /// window, and the main window must not render either a second time.
    ///
    /// Details follows the modal because it is opened from it: shown in the main
    /// window it appears *behind* the designer the operator is looking at, so
    /// pressing Details looks like it did nothing. If the designer closed
    /// mid-build this goes false and both fall back to the main window rather
    /// than being orphaned.
    fn build_hosted_by_open_designer(&self) -> bool {
        self.build_modal_host
            .as_ref()
            .is_some_and(|host| self.designers.iter().any(|(p, _)| p == host))
    }

    fn show_stale_build_prompt(&mut self, ctx: &Context) {
        let Some(prompt) = self.stale_build_prompt.clone() else {
            return;
        };
        let tr = self.lang.tr();
        let last = self
            .cobolt_project
            .as_ref()
            .map(|p| p.project.built_with_display().to_owned())
            .unwrap_or_else(|| "never fully built".to_owned());

        let mut build_now = false;
        let mut run_anyway = false;
        let mut dismissed = false;

        egui::Modal::new(egui::Id::new("stale_build_prompt")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.heading(tr.stale_build_title);
            ui.add_space(8.0);
            ui.label(
                tr.stale_build_body
                    .replace("{last}", &last)
                    .replace("{current}", crate::version::VERSION),
            );
            ui.add_space(6.0);
            ui.label(egui::RichText::new(tr.stale_build_hint).weak().italics());
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button(tr.stale_build_full_now).clicked() {
                    build_now = true;
                }
                if ui.button(tr.stale_build_run_anyway).clicked() {
                    run_anyway = true;
                }
                if ui.button(tr.stale_build_cancel).clicked() {
                    dismissed = true;
                }
            });
        });

        if build_now {
            self.stale_build_prompt = None;
            // The full build stamps the project on success; the developer
            // presses Run again once it finishes. Starting the run
            // automatically would race the build they just asked for.
            self.do_build_binary_with(true);
            // Claim the modal host only AFTER the call, and only when a build
            // really started: do_build_binary_with resets the host to "main
            // window" as part of its fresh-build state, so a claim made
            // before it is silently wiped. A prompt raised by a designer's
            // Run Form hands the progress modal to that designer window, so
            // the whole flow stays under the operator's eyes.
            if self.pending_build_rx.is_some() {
                if let StaleBuildIntent::RunForm(idx) = &prompt.intent {
                    if *idx < self.designers.len() {
                        self.build_modal_host = Some(self.designers[*idx].0.clone());
                    }
                }
            }
            return;
        }
        if run_anyway {
            // Deliberately does NOT clear the stamp check: the prompt returns
            // on the next Run, because nothing was rebuilt. That is the
            // requirement — it nags until the full build happens.
            self.stale_build_prompt = None;
            match prompt.intent {
                StaleBuildIntent::Run => self.run_project_main_form(),
                StaleBuildIntent::RunForm(idx) => {
                    if idx < self.designers.len() {
                        self.launch_form_process(idx, false);
                    }
                }
            }
            return;
        }
        if dismissed {
            self.stale_build_prompt = None;
        }
    }

    fn show_form_error(&mut self, ctx: &Context) {
        let msg = match &self.form_error {
            Some(m) => m.clone(),
            None => return,
        };
        let mut open = true;
        let mut close = false;
        let win_id = egui::Id::new("form_runtime_error");
        let salt = per_viewport_salt(ctx, "form_runtime_error_resize");
        raise_modal_layer(ctx, win_id);
        egui::Window::new("⛔ COBOL error")
            .id(win_id)
            .order(egui::Order::Foreground) // above every ordinary window
            .collapsible(false)
            .resizable(false) // the inner `Resize` grip is the sole size control
            .default_pos(Self::error_modal_default_pos(ctx))
            .open(&mut open)
            .show(ctx, |ui| {
                close = self.error_modal_resize_box(ui, &salt, |app, ui| {
                    app.error_modal_body(
                        ui,
                        Some("Execution stopped. See the Output panel for details."),
                        &msg,
                    )
                });
            });
        if !open || close {
            self.form_error = None;
        }
    }

    fn show_alert_error(&mut self, ctx: &Context) {
        let msg = match &self.alert_error {
            Some(m) => m.clone(),
            None => return,
        };
        let mut open = true;
        let mut close = false;
        let win_id = egui::Id::new("alert_error_dialog");
        let salt = per_viewport_salt(ctx, "alert_error_resize");
        raise_modal_layer(ctx, win_id);
        egui::Window::new("⛔ Error")
            .id(win_id)
            .order(egui::Order::Foreground) // above every ordinary window
            .collapsible(false)
            .resizable(false) // the inner `Resize` grip is the sole size control
            .default_pos(Self::error_modal_default_pos(ctx))
            .open(&mut open)
            .show(ctx, |ui| {
                close = self.error_modal_resize_box(ui, &salt, |app, ui| {
                    app.error_modal_body(ui, None, &msg)
                });
            });
        if !open || close {
            self.alert_error = None;
        }
    }

    /// Top-left position that centers a freshly opened error modal. A seed for
    /// `default_pos` only — NOT an anchor: in egui 0.29 an anchored `Area`
    /// re-pins its position from the current size every frame, which fights
    /// the edge-drag rect during a user resize (the grip drifts away from the
    /// pointer). A one-time centered default keeps resizing well-behaved.
    fn error_modal_default_pos(ctx: &Context) -> egui::Pos2 {
        ctx.content_rect().center() - 0.5 * egui::Vec2::from(ERROR_MODAL_SIZE)
    }

    /// Wrap an error-modal body in the sizing pattern shared with the debugger
    /// window (anti self-inflation): the inner `egui::Resize` is the single
    /// size authority — seeded at `ERROR_MODAL_SIZE`, changed only by the
    /// user's grip drag, never by measured content — and the content fills the
    /// box exactly so its reported min-size equals the box.
    fn error_modal_resize_box(
        &mut self,
        ui: &mut egui::Ui,
        id_salt: &str,
        body: impl FnOnce(&mut Self, &mut egui::Ui) -> bool,
    ) -> bool {
        let mut close = false;
        error_modal_scaffold(ui, id_salt, |ui| {
            close = body(self, ui);
        });
        close
    }

    /// Shared body of the two error modals: optional intro line, the message
    /// in a two-axis scroll area, and the Copy / Save / font-size / OK row.
    /// Returns `true` when OK was clicked (the caller clears its message).
    fn error_modal_body(&mut self, ui: &mut egui::Ui, intro: Option<&str>, msg: &str) -> bool {
        let act = error_modal_body_ui(ui, intro, msg, &mut self.error_font_size);
        if act.save {
            self.begin_file_dialog(
                FileRequest::SaveErrorText(msg.to_owned()),
                crate::file_dialog::DialogSpec::save()
                    .filter("Text file", &["txt"])
                    .file_name("error.txt"),
            );
        }
        act.close
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
        egui::Window::new(format!("About {}", crate::theme::brand_name()))
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
                    // The brand rule: "AI" is always #70f3fc.
                    ui.label(crate::theme::brand_layout_job(
                        "",
                        "",
                        22.0,
                        ui.visuals().strong_text_color(),
                    ));
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

    /// "Set up the AI" invitation, shown once after opening a project that has no
    /// usable model or no configured agent. The two buttons open the very same
    /// managers as the Project Settings rows (`manage_models` / `manage_agents`),
    /// so there is a single way to configure each.
    /// Note which project-structure upgrades the open project is due, so the
    /// dialog can offer them. Called when a project opens; costs one pass over
    /// the registry and, for the upgrades that need it, a read of the forms.
    fn detect_project_upgrades(&mut self) {
        let (Some(proj), Some(dir)) = (self.cobolt_project.as_ref(), self.project_dir()) else {
            self.project_upgrades.clear();
            return;
        };
        self.project_upgrades = crate::project_upgrade::pending(proj, &dir);
    }

    /// Offer the pending project-structure upgrades. The developer's project,
    /// the developer's call: **Not now** costs nothing and is offered again on
    /// the next open, so declining is never a dead end.
    fn show_project_upgrade_modal(&mut self, ctx: &Context, tr: &Tr) {
        if self.project_upgrades.is_empty() {
            return;
        }
        let mut open = true;
        let mut accept = false;
        let mut later = false;
        let upgrades = self.project_upgrades.clone();
        let dim = self.current_theme().text_dim;

        egui::Window::new(tr.upgrade_title)
            .id(egui::Id::new("project_upgrade"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_max_width(560.0);
                ui.add_space(4.0);
                ui.label(egui::RichText::new(tr.upgrade_intro).size(13.0));
                ui.add_space(12.0);
                for u in &upgrades {
                    ui.label(egui::RichText::new(u.title(tr)).size(13.0).strong());
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new(u.detail(tr)).size(12.0).color(dim));
                    ui.add_space(10.0);
                }
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr.upgrade_apply).clicked() {
                        accept = true;
                    }
                    if ui.button(tr.upgrade_later).clicked() {
                        later = true;
                    }
                });
                ui.add_space(4.0);
            });

        if accept {
            self.apply_project_upgrades(&upgrades, tr);
        }
        // ✕ and Not now are the same answer: nothing changes, and the offer
        // returns next time this project is opened.
        if accept || later || !open {
            self.project_upgrades.clear();
        }
    }

    /// Run the accepted upgrades and save the project once. A failure keeps
    /// whatever succeeded before it — a partial run is still a consistent
    /// project, and the next open offers the rest.
    fn apply_project_upgrades(
        &mut self,
        upgrades: &[&'static dyn crate::project_upgrade::ProjectUpgrade],
        tr: &Tr,
    ) {
        let (Some(proj), Some(dir)) = (self.cobolt_project.as_mut(), self.project_path.clone())
        else {
            return;
        };
        let Some(project_dir) = dir.parent().map(|p| p.to_path_buf()) else {
            return;
        };
        let (done, failure) = crate::project_upgrade::apply_all(proj, &project_dir, upgrades);
        if !done.is_empty() {
            self.do_save_project();
            self.output
                .push_status(tr.upgrade_done.replacen("{}", &done.join(", "), 1));
        }
        if let Some(e) = failure {
            self.output
                .push_status(tr.upgrade_failed.replacen("{}", &e, 1));
        }
    }

    fn show_ai_setup_modal(&mut self, ctx: &Context, tr: &Tr) {
        if !self.ai_setup_modal {
            return;
        }
        // A manager opened from here takes over the screen: keep the invite alive
        // but unpainted while it is up (the managers are drawn earlier in the frame,
        // so painting the invite too would put it on top). Closing the manager
        // brings the invite back, so the model and the agent can both be set from
        // one place — only ✕ / "Later" dismisses it.
        if self.models_modal.is_some() || self.agents_modal.is_some() {
            return;
        }
        let mut open = true;
        let mut hide_again = self
            .cobolt_project
            .as_ref()
            .map(|p| p.ide.hide_ai_setup_prompt)
            .unwrap_or(false);
        let mut open_models = false;
        let mut open_agents = false;
        let mut open_judge = false;
        let mut close = false;

        egui::Window::new(tr.ai_setup_title)
            .id(egui::Id::new("ai_setup_invite"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_max_width(520.0);
                ui.horizontal_top(|ui| {
                    // Grace, sitting — decoration only.
                    ui.add(
                        egui::Image::new(egui::include_image!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../../assets/images/gracesitting.png"
                        )))
                        .max_height(190.0),
                    );
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(tr.ai_setup_msg).size(13.0));
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            if ui.button(tr.ai_setup_models).clicked() {
                                open_models = true;
                            }
                            if ui.button(tr.ai_setup_agents).clicked() {
                                open_agents = true;
                            }
                            // Spec 040 R11: the judge is the third thing that
                            // has to be set up, and the only one a developer
                            // would not think to look for.
                            if ui
                                .button(tr.ai_setup_judge)
                                .on_hover_text(tr.ai_setup_judge_hint)
                                .clicked()
                            {
                                open_judge = true;
                            }
                        });
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(tr.ai_setup_separation)
                                .size(12.0)
                                .color(self.current_theme().text_dim),
                        );
                        ui.add_space(10.0);
                        ui.checkbox(&mut hide_again, tr.ai_setup_hide);
                        ui.add_space(8.0);
                        if ui.button(tr.ai_setup_later).clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(4.0);
            });

        // Persist the "don't ask again" choice as soon as it changes.
        let stored = self
            .cobolt_project
            .as_ref()
            .map(|p| p.ide.hide_ai_setup_prompt)
            .unwrap_or(false);
        if hide_again != stored {
            if let Some(p) = self.cobolt_project.as_mut() {
                p.ide.hide_ai_setup_prompt = hide_again;
            }
            self.do_save_project();
        }

        if open_models {
            self.models_modal = Some(crate::panels::models_modal::ModelsModal::new());
        }
        if open_agents || open_judge {
            if let Some(dir) = self
                .project_path
                .as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf())
            {
                self.agents_modal = Some(if open_judge {
                    crate::panels::agents_modal::AgentsModal::open_at(
                        &dir,
                        &mut self.llm,
                        crate::agents_db::PROFICIENCY_JUDGE,
                    )
                } else {
                    crate::panels::agents_modal::AgentsModal::open_for(&dir, &mut self.llm)
                });
                // Opening can migrate legacy agent model settings onto project
                // profiles — persist that immediately (same as Project Settings).
                self.persist_active_project_ai();
            }
        }
        // Only the user dismisses the invite: Later / ✕. Opening a manager just
        // hides it for as long as that manager is up (see the guard above).
        if close || !open {
            self.ai_setup_modal = false;
        }
    }

    /// The first-run Rust question (see [`crate::toolchain`]).
    ///
    /// There is no ✕: the only ways out are Install and a refusal, because a
    /// window closed by its corner would skip the second ask the operator
    /// asked for — and skipping it is exactly how somebody ends up discovering
    /// at Build time what declining cost them.
    fn show_toolchain_prompt(&mut self, ctx: &Context, tr: &Tr) {
        use crate::toolchain::{Decision, Install, Stage, Status};

        if self.toolchain_prompt.is_none() {
            return;
        }
        // Read the palette before borrowing the prompt: both come from `self`.
        let dim = self.current_theme().text_dim;
        let minimum = crate::toolchain::minimum().to_string();
        let command = crate::toolchain::install_command();
        let prompt = self
            .toolchain_prompt
            .as_mut()
            .expect("presence checked above");
        prompt.poll_install();

        let mut install = false;
        let mut decline = false;
        let mut settle = false;

        let title = match prompt.stage {
            Stage::Offer => tr.rust_check_title,
            Stage::LastChance => tr.rust_check_last_title,
        };
        egui::Window::new(title)
            .id(egui::Id::new("rust_first_run"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_max_width(520.0);
                match &prompt.install {
                    // Under way: the installer owns the dialog until it answers.
                    Some(Install::Running(_)) => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(tr.rust_check_installing);
                        });
                        ctx.request_repaint_after(std::time::Duration::from_millis(250));
                    }
                    Some(Install::Finished(outcome)) => {
                        let text = match (outcome.ok, outcome.version) {
                            (true, Some(v)) => {
                                tr.rust_check_installed.replacen("{}", &v.to_string(), 1)
                            }
                            _ => tr.rust_check_failed.to_owned(),
                        };
                        ui.label(text);
                        if !outcome.ok && !outcome.detail.is_empty() {
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(&outcome.detail)
                                    .monospace()
                                    .size(11.0)
                                    .color(dim),
                            );
                        }
                        ui.add_space(12.0);
                        if ui.button(tr.rust_check_close).clicked() {
                            settle = true;
                        }
                    }
                    None => {
                        match prompt.stage {
                            Stage::Offer => {
                                ui.label(match &prompt.status {
                                    Status::TooOld { version, .. } => tr
                                        .rust_check_too_old
                                        .replacen("{}", &version.to_string(), 1)
                                        .replacen("{}", &minimum, 1),
                                    _ => tr.rust_check_missing.to_owned(),
                                });
                                ui.add_space(6.0);
                                ui.label(tr.rust_check_why);
                            }
                            Stage::LastChance => {
                                ui.label(egui::RichText::new(tr.rust_check_lost).size(13.0));
                            }
                        }
                        ui.add_space(12.0);
                        // The command is shown before it is approved, and it is
                        // the string `install_argv` runs — never a paraphrase.
                        ui.label(egui::RichText::new(tr.rust_check_command).size(11.0).color(dim));
                        ui.add_space(2.0);
                        ui.add(
                            egui::Label::new(egui::RichText::new(command).monospace().size(11.0))
                                .selectable(true)
                                .wrap(),
                        );
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            if ui.button(tr.rust_check_install).clicked() {
                                install = true;
                            }
                            let refuse = match prompt.stage {
                                Stage::Offer => tr.rust_check_later,
                                Stage::LastChance => tr.rust_check_continue,
                            };
                            if ui.button(refuse).clicked() {
                                decline = true;
                            }
                        });
                    }
                }
            });

        if install {
            prompt.start_install();
        } else if decline && prompt.decline() == Decision::Accepted {
            settle = true;
        }
        if settle {
            self.toolchain_prompt = None;
            crate::ui_prefs::mark_rust_check_done();
        }
    }

    /// Is the Building dialog on screen? It is modal, so the rest of the IDE
    /// has to keep out of the way while it is up — the keyboard included,
    /// because egui's modal layer blocks the pointer, not key events.
    fn build_modal_visible(&self) -> bool {
        !self.build_modal_closed
            && (self.pending_build_rx.is_some() || self.build_outcome.is_some())
    }

    /// The Building dialog. A real [`egui::Modal`]: while it is up nothing
    /// else in the IDE accepts input, and it does NOT vanish when the build
    /// ends — the outcome replaces the progress bar and the dialog waits for
    /// Close. Close is the only way out: `ModalResponse::should_close` is
    /// deliberately never consulted, so Esc and clicks on the backdrop do
    /// nothing.
    ///
    /// The width is pinned with `ui.set_width`, so neither a long binary path
    /// nor a long error message can make the dialog size itself, and nothing
    /// here is measured from the space around it.
    fn show_building_modal(&mut self, ctx: &Context) {
        let building = self.pending_build_rx.is_some();
        if !building && self.build_outcome.is_none() {
            return;
        }
        // Drain any phase updates that arrived since the last frame. Every
        // line is also captured for the Build-details window: phases as
        // milestones, `detail` lines as dimmed supplements.
        if building {
            let mut latest = None;
            if let Some(prx) = &self.pending_build_progress {
                while let Ok(p) = prx.try_recv() {
                    if p.detail {
                        self.build_log.push((BuildLogKind::Detail, p.message));
                    } else {
                        self.build_log
                            .push((BuildLogKind::Phase, p.message.clone()));
                        latest = Some(p);
                    }
                }
            }
            if let Some(p) = latest {
                self.build_phase = (p.fraction, p.message);
            }
            ctx.request_repaint(); // keep polling while the build runs
        }
        if self.build_modal_closed {
            return; // the user closed it; the next build brings it back
        }
        let tr = self.lang.tr();
        let (frac, msg) = (self.build_phase.0, self.build_phase.1.clone());
        let outcome = self.build_outcome.clone();
        let mut details = false;
        let mut close = false;
        egui::Modal::new(egui::Id::new("building_modal"))
            // The same dim the dialog painted by hand before it became a
            // real modal.
            .backdrop_color(egui::Color32::from_black_alpha(150))
            .show(ctx, |ui| {
                // One fixed width for both states.
                ui.set_width(360.0);
                match &outcome {
                    None => {
                        ui.heading(tr.build_modal_title);
                        ui.add_space(8.0);
                        // Determinate bar driven by the real build fraction.
                        ui.add(
                            egui::ProgressBar::new(frac.clamp(0.0, 1.0))
                                .desired_width(344.0)
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
                            // Truncated, not extended: a long phase line must
                            // not push the dialog wider.
                            ui.add(
                                egui::Label::new(egui::RichText::new(label).size(13.0)).truncate(),
                            );
                        });
                    }
                    Some(Ok(summary)) => {
                        ui.heading(
                            egui::RichText::new(tr.build_modal_succeeded)
                                .color(Color32::from_rgb(60, 190, 100)),
                        );
                        ui.add_space(8.0);
                        ui.add(egui::Label::new(summary.as_str()).wrap());
                    }
                    Some(Err(error)) => {
                        ui.heading(
                            egui::RichText::new(tr.build_modal_failed)
                                .color(Color32::from_rgb(235, 80, 80)),
                        );
                        ui.add_space(8.0);
                        // A failed cargo build can be hundreds of lines; without
                        // a scroll bound the text pushed the Close button off
                        // the screen and the modal could not be dismissed.
                        egui::ScrollArea::vertical()
                            .max_height(240.0)
                            .show(ui, |ui| {
                                ui.add(egui::Label::new(error.as_str()).wrap());
                            });
                    }
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(tr.build_details_btn).clicked() {
                        details = true;
                    }
                    // Close is the only exit — and it must ALWAYS be there. It
                    // was disabled until the build finished, so a hung build
                    // worker left a modal that blocked the whole IDE with no
                    // way out but force-quit. Closing mid-build only hides the
                    // dialog; the build itself keeps running to its outcome.
                    if ui.button(tr.build_modal_close).clicked() {
                        close = true;
                    }
                });
                ui.add_space(4.0);
            });
        if details {
            self.build_details_open = true;
        }
        if close {
            self.build_modal_closed = true;
        }
    }

    #[allow(dead_code)]
    fn show_building_modal_legacy_window(&mut self, ctx: &Context) {
        let (frac, msg) = (self.build_phase.0, self.build_phase.1.clone());

        egui::Window::new("Building…")
            .id(egui::Id::new("building_modal_legacy"))
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
                ui.add_space(6.0);
                // Dismiss the dialog (the build keeps running; the result
                // still lands in the Output panel), or open the live log.
                ui.horizontal(|ui| {
                    let tr = self.lang.tr();
                    if ui.button(tr.modal_hide).clicked() {
                        self.build_modal_closed = true;
                    }
                    if ui.button(tr.build_details_btn).clicked() {
                        self.build_details_open = true;
                    }
                });
                ui.add_space(4.0);
            });
    }

    /// The Build-details window: the full build log, one colored line per
    /// entry — phases in the theme foreground, supplements dimmed, success
    /// green, errors red. Resizable and freely movable; opens centered.
    /// Auto-opens when a build fails; Copy/Save export the plain text.
    fn show_build_details_window(&mut self, ctx: &Context) {
        if !self.build_details_open {
            return;
        }
        let tr = self.lang.tr();
        // Ticker: reveal one more line every 75 ms while lines are pending,
        // so the log reads as a feed. `stick_to_bottom` keeps the view
        // following the newest line whenever the user is at the bottom.
        // (250 ms originally — a forty-line build took ten seconds to read
        // out, which read as the build being slow when it was the ticker.)
        if self.build_log_shown < self.build_log.len() {
            let now = std::time::Instant::now();
            let due = self
                .build_log_last_reveal
                .map(|t| now.duration_since(t).as_millis() >= 75)
                .unwrap_or(true);
            if due {
                self.build_log_shown += 1;
                self.build_log_last_reveal = Some(now);
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        let mut open = true;
        let mut copy = false;
        let mut save = false;
        // The Building dialog is a modal, and a modal blocks every layer at or
        // below its own. While it is up the log therefore rides one order
        // ABOVE it: `Order::Tooltip` > `Order::Foreground`, so it is never
        // blocked and never fights the modal for the top slot inside a single
        // order. With the dialog gone it is an ordinary window again.
        let order = if self.build_modal_visible() {
            egui::Order::Tooltip
        } else {
            egui::Order::Middle
        };
        egui::Window::new(tr.build_details_title)
            .id(egui::Id::new("build_details_window"))
            .order(order)
            .open(&mut open)
            .resizable(true)
            .default_size([640.0, 400.0])
            // Centered on first open; the user drags it anywhere after.
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(tr.clipboard_copy).clicked() {
                        copy = true;
                    }
                    if ui.button(tr.build_details_save).clicked() {
                        save = true;
                    }
                });
                ui.separator();
                // High-contrast per theme: neutral lines take the theme's
                // strong/weak text colors; success/error use fixed colors
                // readable on light and dark faces alike.
                let strong = ui.visuals().strong_text_color();
                let weak = ui.visuals().weak_text_color();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for (kind, line) in self.build_log.iter().take(self.build_log_shown) {
                            let color = match kind {
                                BuildLogKind::Phase => strong,
                                BuildLogKind::Detail => weak,
                                BuildLogKind::Success => Color32::from_rgb(60, 190, 100),
                                BuildLogKind::Error => Color32::from_rgb(235, 80, 80),
                            };
                            ui.label(
                                egui::RichText::new(line).monospace().color(color),
                            );
                        }
                    });
            });
        self.build_details_open = open;
        if copy || save {
            let text: String = self
                .build_log
                .iter()
                .map(|(_, l)| l.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if copy {
                ctx.copy_text(text.clone());
            }
            if save {
                self.begin_file_dialog(
                    FileRequest::SaveBuildLog(text),
                    crate::file_dialog::DialogSpec::save().filter("Text", &["txt"]),
                );
            }
        }
    }

    /// Progress modal for File → Reindex Knowledge Bases: same shape as the
    /// Building modal — dimmed background, determinate bar, spinner + the
    /// current "n/m — subject" label. Shown while the worker runs.
    fn show_kb_reindex_modal(&mut self, ctx: &Context) {
        if self.kb_reindex_rx.is_none() || self.kb_reindex_modal_hidden {
            return; // hidden: the reindex continues, summary goes to Output
        }
        let tr = self.lang.tr();
        // Dim the rest of the IDE so the reindex reads as modal (same idiom
        // as the Building modal).
        let screen = ctx.content_rect();
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("kb_reindex_dim"),
        ))
        .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(150));
        let (frac, msg) = (self.kb_reindex_phase.0, self.kb_reindex_phase.1.clone());
        ctx.request_repaint(); // keep the bar live while the worker runs

        egui::Window::new(tr.menu_reindex_kb_busy)
            .id(egui::Id::new("kb_reindex_modal"))
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.add(
                    egui::ProgressBar::new(frac.clamp(0.0, 1.0))
                        .desired_width(280.0)
                        .show_percentage(),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    // Until the first record reports (model load, unchanged
                    // scan) the label falls back to the started status line.
                    let label = if msg.is_empty() {
                        tr.status_kb_reindex_started
                    } else {
                        msg.as_str()
                    };
                    ui.label(egui::RichText::new(label).size(13.0));
                });
                ui.add_space(6.0);
                ui.vertical_centered(|ui| {
                    if ui.button(tr.modal_hide).clicked() {
                        self.kb_reindex_modal_hidden = true;
                    }
                });
                ui.add_space(4.0);
            });
    }

    // ── Keyboard shortcuts (main window) ─────────────────────────────────────

    fn handle_shortcuts(&mut self, ctx: &Context) {
        // The Building dialog is modal: while it is up the IDE takes no
        // commands. egui's modal layer blocks the pointer, not key events, so
        // the shortcuts have to stand down themselves.
        if self.build_modal_visible() {
            return;
        }
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
                        // 050 — the FORM THEME: the whole catalogue (Liquid
                        // Glass, Elegance, and every installed asset pack), the
                        // same list the inspector offers. This row used to be
                        // labelled "Theme" while offering the four GLASS STYLES,
                        // so a real theme could not be chosen at creation and
                        // the two settings looked like one.
                        ui.label(tr.lbl_theme);
                        let choices = crate::theme_ui::choices(ui.ctx());
                        let project_default = crate::theme_ui::project_default(ui.ctx());
                        let resolved = cobolt_forms::theme::resolve_theme_id(
                            Some(self.new_form.form_theme.as_str()),
                            project_default.as_deref(),
                        );
                        let inherited = self.new_form.form_theme.trim().is_empty()
                            && project_default.is_some();
                        let shown = choices
                            .iter()
                            .find(|c| c.id == resolved)
                            .map(|c| c.display_name.clone())
                            .unwrap_or_else(|| resolved.clone());
                        let shown = if inherited {
                            format!("{shown} {}", tr.theme_inherited)
                        } else {
                            shown
                        };
                        egui::ComboBox::from_id_salt("new-form-theme")
                            .selected_text(shown)
                            .width(200.0)
                            .show_ui(ui, |ui| {
                                for c in &choices {
                                    // Liquid Glass IS the default, so selecting
                                    // it stores nothing rather than writing a
                                    // redundant override.
                                    let write =
                                        if c.id == cobolt_forms::theme::LIQUID_GLASS {
                                            String::new()
                                        } else {
                                            c.id.clone()
                                        };
                                    ui.selectable_value(
                                        &mut self.new_form.form_theme,
                                        write,
                                        &c.display_name,
                                    );
                                }
                            });
                        ui.end_row();

                        // 050 R17 — the glass style is its OWN row, and it is
                        // disabled under a theme that owns the whole look.
                        let self_contained =
                            crate::theme_ui::is_self_contained(ui.ctx(), &resolved);
                        ui.label(tr.lbl_glass_style);
                        let resp = ui
                            .add_enabled_ui(!self_contained, |ui| {
                                egui::ComboBox::from_id_salt("new-form-glass-style")
                                    .selected_text(self.new_form.theme.as_str())
                                    .width(200.0)
                                    .show_ui(ui, |ui| {
                                        for opt in [
                                            "Classic",
                                            "Enhanced",
                                            "Neumorphic Light",
                                            "Neumorphic Dark",
                                        ] {
                                            ui.selectable_value(
                                                &mut self.new_form.theme,
                                                opt.to_owned(),
                                                opt,
                                            );
                                        }
                                    });
                            })
                            .response;
                        if self_contained {
                            resp.on_hover_text(tr.hint_theme_owns_look);
                        }
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

    /// Spec 046 R1/R2/R11 — Copy Form: serialize the complete form (every
    /// control's properties, every bound event's full COBOL body,
    /// animations, data bindings) to the `.cfrm` XML text and put it on the
    /// OS clipboard. If `cfrm_path` is open in a Designer, its **live**
    /// (possibly unsaved) state is copied — "copy" means "copy what I'm
    /// looking at," not a stale on-disk snapshot.
    fn copy_form(&mut self, ctx: &egui::Context, cfrm_path: &Path) {
        let form = match self.designers.iter().find(|(p, _)| same_file_path(p, cfrm_path)) {
            Some((_, dp)) => dp.form.clone(),
            None => match load_form(cfrm_path) {
                Ok(form) => form,
                Err(e) => {
                    self.output
                        .push_status(format!("Could not copy form {}: {e}", cfrm_path.display()));
                    return;
                }
            },
        };
        match form_to_string(&form) {
            Ok(xml) => {
                ctx.copy_text(xml);
                self.output.push_status(format!("Copied form \"{}\" to the clipboard", form.name));
            }
            Err(e) => {
                self.output.push_status(format!("Could not copy form \"{}\": {e}", form.name));
            }
        }
    }

    /// Spec 046 R3 — the developer clicked Paste Form. There is no
    /// synchronous "read the clipboard now": `RequestPaste` asks the
    /// platform layer for it, and the text arrives as `Event::Paste` on a
    /// later frame (`poll_form_paste` below), the same path egui itself
    /// uses for an ordinary Cmd/Ctrl+V.
    fn paste_form_requested(&mut self, ctx: &egui::Context, dir: &Path) {
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        self.pending_form_paste = Some(dir.to_path_buf());
    }

    /// Spec 046 R3/R4/R9 — consumes the `Event::Paste` a pending Paste Form
    /// request produced. Gated on `pending_form_paste` being set so an
    /// unrelated Cmd/Ctrl+V into some other focused field is left alone —
    /// `ctx.input`'s event list is the same one every widget reads, this
    /// does not "steal" the event from anything.
    fn poll_form_paste(&mut self, ctx: &egui::Context, tr: &Tr) {
        let Some(dir) = self.pending_form_paste.clone() else {
            return;
        };
        let text = ctx.input(|i| extract_pasted_text(&i.events));
        let Some(text) = text else {
            return;
        };
        self.pending_form_paste = None;
        match load_form_from_str(&text) {
            Ok(form) => self.finish_form_paste(ctx, form, &dir),
            Err(e) => {
                self.output
                    .push_status(format!("{}: {e}", tr.paste_form_invalid_clipboard));
            }
        }
    }

    /// Spec 046 R5/R7 — a form successfully parsed off the clipboard.
    /// `form_cobol_id_conflict` (the same check `create_new_form` already
    /// uses) decides whether this registers immediately or waits for T5's
    /// rename/replace modal.
    fn finish_form_paste(&mut self, _ctx: &egui::Context, form: Form, dest_dir: &Path) {
        if self.form_cobol_id_conflict(&form.name, None).is_some() {
            let new_name = format!("{} (2)", form.name);
            self.pending_paste_conflict = Some(PendingPasteConflict {
                new_name,
                form,
                dest_dir: dest_dir.to_path_buf(),
                confirming_replace: false,
            });
            return;
        }
        self.register_pasted_form(form, dest_dir);
    }

    /// Spec 046 R7/R8 — the rename-or-replace prompt, same shape as
    /// `show_form_delete_confirm`. Renaming re-checks the live-edited name
    /// against the same `form_cobol_id_conflict` the initial detection used,
    /// so both agree on what counts as a conflict; Replace requires its own
    /// second click (`confirming_replace`) before `delete_form_path` — the
    /// exact helper the tree's own form-delete confirmation already uses —
    /// runs.
    fn show_paste_form_conflict(&mut self, ctx: &Context, tr: &Tr) {
        let Some(conflict) = &self.pending_paste_conflict else {
            return;
        };
        let original_name = conflict.form.name.clone();
        let mut new_name = conflict.new_name.clone();
        let confirming_replace = conflict.confirming_replace;

        let mut cancel = false;
        let mut do_rename = false;
        let mut do_replace_confirm = false;
        let mut do_replace_now = false;

        egui::Window::new(tr.paste_form_name_conflict_title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                if confirming_replace {
                    ui.label(format!("{}: \"{original_name}\"?", tr.paste_form_replace));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(tr.btn_cancel).clicked() {
                            cancel = true;
                        }
                        if ui.button(tr.delete_confirm_ok).clicked() {
                            do_replace_now = true;
                        }
                    });
                } else {
                    ui.label(tr.paste_form_name_conflict_body);
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(format!("{original_name} →"));
                        ui.text_edit_singleline(&mut new_name);
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(tr.btn_cancel).clicked() {
                            cancel = true;
                        }
                        if ui.button(tr.paste_form_replace).clicked() {
                            do_replace_confirm = true;
                        }
                        let trimmed = new_name.trim();
                        let can_rename = !trimmed.is_empty()
                            && self.form_cobol_id_conflict(trimmed, None).is_none();
                        if ui
                            .add_enabled(can_rename, egui::Button::new(tr.paste_form_rename))
                            .clicked()
                        {
                            do_rename = true;
                        }
                    });
                }
            });

        if cancel {
            self.pending_paste_conflict = None;
        } else if do_replace_confirm {
            if let Some(c) = &mut self.pending_paste_conflict {
                c.confirming_replace = true;
            }
        } else if do_replace_now {
            if let Some(c) = self.pending_paste_conflict.take() {
                if let Some(existing) = self.form_cobol_id_conflict(&c.form.name, None) {
                    self.delete_form_path(existing);
                }
                self.register_pasted_form(c.form, &c.dest_dir);
            }
        } else if do_rename {
            if let Some(mut c) = self.pending_paste_conflict.take() {
                c.form.name = new_name.trim().to_string();
                self.register_pasted_form(c.form, &c.dest_dir);
            }
        } else if let Some(c) = &mut self.pending_paste_conflict {
            c.new_name = new_name;
        }
    }

    /// Spec 046 R5/R6/R10 — write the parsed form's `.cfrm`, register it in
    /// the project, regenerate its COBOL immediately, and open it in a
    /// Designer — the same sequence `save_new_form_to` uses for a
    /// hand-created form. Control IDs and paragraph names are written
    /// exactly as parsed (R6 — no remap: each form is its own `PROGRAM-ID`
    /// and its own runtime process, so nothing here can collide with an
    /// unrelated form already in the project).
    fn register_pasted_form(&mut self, form: Form, dest_dir: &Path) {
        let path = dest_dir.join(pasted_form_file_name(&form.name));
        if let Err(e) = save_form(&form, &path) {
            self.output.push_status(format!("Could not paste form: {e}"));
            return;
        }
        if let Some(parent) = path.parent() {
            self.forms_list.set_root(parent);
        }
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
        self.write_generated_for(&path, &form);
        self.output
            .push_status(format!("Pasted form \"{}\"", form.name));
        let mut dp = DesignerPanel::new(form);
        dp.cfrm_dir = path.parent().map(|p| p.to_path_buf());
        self.designers.push((path, dp));
        self.apply_main_form_invariant();
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
        // 050 — the form theme the developer picked in the dialog. Empty means
        // "inherit the project default", which is what an empty `Form::theme`
        // already means, so nothing is written in that case.
        let chosen_theme = self.new_form.form_theme.trim().to_owned();
        if !chosen_theme.is_empty() {
            form.theme = Some(chosen_theme.clone());
        }
        let style = cobolt_forms::model::GlassStyle::from_str(&self.new_form.theme);
        // 050 R7 — a new form under a self-contained theme is not seeded with
        // glass defaults: they would be written into the `.cfrm` and then
        // ignored by the theme that is actually painting it. Gated on the theme
        // this form will ACTUALLY use — the dialog's pick, falling back to the
        // project default — not on the project default alone.
        let glass_applies = !self
            .resolve_surface_theme(Some(chosen_theme.as_str()))
            .is_self_contained();
        if style.is_neumorphic() && glass_applies {
            form.apply_glass_style_defaults(style);
        } else {
            form.glass_style = style;
            form.background_color = "00000000".into(); // transparent — matches IDE glass
        }

        let default_name = format!("{}.cfrm", form_name.to_lowercase());
        let mut spec = crate::file_dialog::DialogSpec::save()
            .filter("RustCOBOL Form", &["cfrm"])
            .file_name(default_name);
        // Default into the folder whose [+] was clicked, or the project's forms/
        // folder when the create came from the category header.
        if let Some(dir) = self.project_dir() {
            let dest = dir.join(self.new_form.target_dir.as_deref().unwrap_or("forms"));
            let _ = std::fs::create_dir_all(&dest);
            spec = spec.directory(dest);
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
        // Spec 037 R3 — the first form created in a project becomes the main
        // form (and any later violation heals to the first in the list).
        self.apply_main_form_invariant();
    }

    /// Spec 037 R3 — run [`crate::main_form::normalize_main_form`] over the
    /// open project, mirror the on-disk designation into every open designer
    /// (so the properties panel and tree crown agree with the files), and
    /// report a repair in the status line.
    fn apply_main_form_invariant(&mut self) {
        let Some(dir) = self.project_dir() else {
            return;
        };
        let Some(proj) = self.cobolt_project.as_ref() else {
            return;
        };
        let forms = proj.files.forms.clone();
        let tr = self.lang.tr();
        match crate::main_form::normalize_main_form(&dir, &forms) {
            Ok(outcome) => {
                let holder_abs = outcome.holder().map(|rel| dir.join(rel));
                for (path, dp) in &mut self.designers {
                    dp.form.main_form = holder_abs.as_deref() == Some(path.as_path());
                }
                match outcome {
                    crate::main_form::MainFormOutcome::Assigned { holder } => {
                        self.output.push_status(
                            tr.status_main_form_assigned.replacen("{}", &holder, 1),
                        );
                    }
                    crate::main_form::MainFormOutcome::Trimmed { holder, cleared } => {
                        self.output.push_status(
                            tr.status_main_form_trimmed
                                .replacen("{}", &holder, 1)
                                .replacen("{}", &cleared.to_string(), 1),
                        );
                    }
                    crate::main_form::MainFormOutcome::Unchanged { .. } => {}
                }
            }
            Err(e) => self.output.push_status(format!("Main form check: {e}")),
        }
        self.reseal_project_designation();
    }

    /// Restate the main-form designation in the project file.
    ///
    /// The designation itself lives in the `.cfrm` files — exactly one carries
    /// `main-form="true"` (037 R3). The project file keeps a **sealed copy**,
    /// because only the main form starts an application and a runtime has to
    /// be able to tell a designation the IDE made from one somebody edited by
    /// hand. [`save_project`] recomputes the seal, so every path that can
    /// change which form is main ends up here.
    ///
    /// Quiet on purpose: this is bookkeeping the developer did not ask for, and
    /// it happens several times in an ordinary editing session. Only a failure
    /// is worth a status line.
    fn reseal_project_designation(&mut self) {
        let (Some(proj), Some(path)) = (self.cobolt_project.as_ref(), self.project_path.clone())
        else {
            return;
        };
        // A project of an older shape carries no seal, and the IDE does not
        // give it one uninvited — so there is nothing to restate, and no
        // reason to rewrite the developer's project file just by opening it.
        if proj.project.structure < crate::project_upgrade::STRUCTURE_MAIN_FORM_SEAL {
            return;
        }
        let proj = proj.clone();
        if let Err(e) = save_project(&proj, &path) {
            self.output
                .push_status(format!("Main form seal not written: {e}"));
        }
    }

    /// 037 R2 — settle MainForm flag transitions emitted by the designers'
    /// undo stacks. A claim demotes the previous holder (open designers in
    /// memory + dirty, closed forms directly on disk — an open designer's
    /// unsaved edits are never committed as a side effect); an un-claim
    /// (undo) restores the recorded previous holder. Both directions are one
    /// user action; the status line names the forms involved.
    fn drain_main_form_changes(&mut self) {
        let mut events: Vec<(std::path::PathBuf, bool)> = Vec::new();
        for (path, d) in &mut self.designers {
            for claim in std::mem::take(&mut d.main_form_changes) {
                events.push((path.clone(), claim));
            }
        }
        // The project-panel "Form properties" view edits a form through its
        // own embedded DesignerState — its MainForm claim must settle through
        // the SAME path, or checking the box there leaves the previous holder
        // crowned (two mains on disk).
        if let Some(st) = &mut self.inspect {
            for claim in std::mem::take(&mut st.designer.main_form_changes) {
                events.push((st.path.clone(), claim));
            }
        }
        if events.is_empty() {
            return;
        }
        let Some(dir) = self.project_dir() else {
            return; // standalone form outside a project — nothing to settle
        };
        let forms: Vec<String> = self
            .cobolt_project
            .as_ref()
            .map(|p| p.files.forms.clone())
            .unwrap_or_default();
        let tr = self.lang.tr();
        let open_paths: Vec<std::path::PathBuf> =
            self.designers.iter().map(|(p, _)| p.clone()).collect();
        for (path, claim) in events {
            if claim {
                let Some(rel) = relative_to(&path, &dir) else {
                    continue;
                };
                // Demote the previous holder: open designers in memory …
                let mut prev: Option<String> = None;
                for (p, d) in &mut self.designers {
                    if *p != path && d.form.main_form {
                        d.form.main_form = false;
                        d.dirty = true;
                        if prev.is_none() {
                            prev = relative_to(p, &dir);
                        }
                    }
                }
                // … the inspected form's in-memory copy (its file is handled
                // by the on-disk clear below; without this the inspect pane
                // keeps showing a checked box until its mtime refresh) …
                if let Some(st) = &mut self.inspect {
                    if st.path != path && st.designer.form.main_form {
                        st.designer.form.main_form = false;
                        if prev.is_none() {
                            prev = relative_to(&st.path, &dir);
                        }
                    }
                }
                // … and closed forms directly on disk.
                match crate::main_form::clear_other_holders_on_disk(
                    &dir,
                    &forms,
                    &rel,
                    &open_paths,
                ) {
                    Ok(cleared) => {
                        // Refresh the tree's cached copy of every demoted
                        // form, so its crown falls off even after the
                        // override clears (stale cache = second crown).
                        for c in &cleared {
                            self.project.refresh_form(&dir.join(c));
                        }
                        if prev.is_none() {
                            prev = cleared.into_iter().next();
                        }
                    }
                    Err(e) => self.output.push_status(format!("Main form change: {e}")),
                }
                self.output.push_status(
                    tr.status_main_form_now
                        .replacen("{}", &rel, 1)
                        .replacen("{}", prev.as_deref().unwrap_or("—"), 1),
                );
                self.main_form_prev.push(prev);
            } else {
                match self.main_form_prev.pop().flatten() {
                    Some(prev_rel) => {
                        let prev_abs = dir.join(&prev_rel);
                        let mut in_memory = false;
                        for (p, d) in &mut self.designers {
                            if *p == prev_abs {
                                d.form.main_form = true;
                                d.dirty = true;
                                in_memory = true;
                            }
                        }
                        if !in_memory {
                            if let Err(e) = crate::main_form::restore_holder_on_disk(
                                &dir,
                                &prev_rel,
                                &open_paths,
                            ) {
                                self.output
                                    .push_status(format!("Main form restore: {e}"));
                            } else {
                                self.project.refresh_form(&prev_abs);
                            }
                        }
                        self.output.push_status(
                            tr.status_main_form_restored.replacen("{}", &prev_rel, 1),
                        );
                    }
                    // No recorded holder (e.g. history cleared) — heal to the
                    // R3 default rather than leaving the project holderless.
                    None => self.apply_main_form_invariant(),
                }
            }
            // Which form is main just changed on disk: restate the sealed copy
            // in the project file, or the next `rcrun` reports as corruption a
            // change the developer made here deliberately.
            self.reseal_project_designation();
        }
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
            FileRequest::NewForm(form) => self.save_new_form_to(*form, path),
            FileRequest::PickBackgroundImage => self.set_background_image(path),
            FileRequest::PickProjectIcon => self.set_project_icon(path),
            FileRequest::OpenGridData { cidx_path, def } => {
                self.open_grid_for_indexed_with_data_path(&cidx_path, &def, &path);
            }
            // Status lines only here — never set `alert_error` from the
            // error-save path, or a failed save would reopen the modal.
            FileRequest::SaveErrorText(text) => match std::fs::write(&path, text) {
                Ok(()) => self
                    .output
                    .push_status(format!("Error message saved to {}", path.display())),
                Err(e) => self.output.push_status(format!(
                    "Could not save error message to {}: {e}",
                    path.display()
                )),
            },
            FileRequest::SaveBuildLog(text) => {
                match std::fs::write(&path, &text) {
                    Ok(()) => self
                        .output
                        .push_status(format!("Build log saved → {}", path.display())),
                    Err(e) => self
                        .output
                        .push_status(format!("Build log save failed: {e}")),
                }
            }
            FileRequest::SaveBenchmarkPdf(report) => {
                let path = ensure_pdf_extension(path);
                let metrics = Self::llm_benchmark_metrics(&report)
                    .unwrap_or_else(|| Self::fallback_benchmark_metrics(&report));
                let benchmark_cfg = self.llm_benchmark_config.as_ref().unwrap_or(&self.llm);
                let markdown = Self::benchmark_pdf_markdown(&report, Some(&metrics), benchmark_cfg);
                match crate::pdf_export::export("COBOL proficiency report", &markdown, &path) {
                    Ok(()) => self
                        .output
                        .push_status(format!("COBOL proficiency PDF saved to {}", path.display())),
                    Err(e) => self.output.push_status(format!(
                        "Could not save COBOL proficiency PDF to {}: {e}",
                        path.display()
                    )),
                }
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

pub(crate) fn apply_glass_visuals(ctx: &Context, theme: &crate::theme::Theme) {
    use egui::Color32;
    use egui::{style::WidgetVisuals, CornerRadius, Shadow, Stroke, Visuals};

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
        offset: [0, 10],
        blur: 40,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 100),
    };
    v.window_corner_radius = CornerRadius::same(12);
    v.window_highlight_topmost = false;

    // ── Control states ─────────────────────────────────────────────────────
    let make_widget = |bg: Color32, stroke_c: Color32, text: Color32| WidgetVisuals {
        weak_bg_fill: bg,
        bg_fill: bg,
        bg_stroke: Stroke::new(1.0, stroke_c),
        fg_stroke: Stroke::new(1.5, text),
        corner_radius: CornerRadius::same(8),
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

    if theme.is_neumorphic() {
        // Discrete 3D shadow for window/panel chrome (intensity dialled up
        // 50% over the original discrete relief — see `theme::paint_neumorphic_relief`).
        // Neumorphic Dark uses a near-black shadow (its surface is already
        // dark, so the blue-grey tint used on Neumorphic Light would barely
        // register); Neumorphic Light keeps the blue-grey tint.
        v.window_shadow = Shadow {
            offset: [3, 3],
            blur: 12,
            spread: 0,
            color: if theme.dark {
                Color32::from_rgba_unmultiplied(0, 0, 0, 170)
            } else {
                Color32::from_rgba_unmultiplied(165, 175, 205, 135)
            },
        };
        // Labels have no borders and no drop shadows
        v.widgets.noninteractive.bg_stroke = Stroke::NONE;
        v.widgets.noninteractive.weak_bg_fill = Color32::TRANSPARENT;
        v.widgets.noninteractive.bg_fill = Color32::TRANSPARENT;
    }

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
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::Vec2::new(8.0, 8.0);
    style.spacing.button_padding = egui::Vec2::new(12.0, 7.0);
    style.spacing.indent = 20.0;
    style.spacing.window_margin = egui::Margin::same(12);
    style.spacing.menu_margin = egui::Margin::same(8);
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
    ctx.set_global_style(style);
}

/// Apply the IDE theme to an **opaque** child viewport (designer, indexed editor,
/// grid browser). Semi-transparent glass colours are composited over white so
/// gaps between panels never show the OS clear colour.
fn apply_opaque_viewport_theme(ctx: &Context, theme: &crate::theme::Theme) {
    apply_glass_visuals(ctx, theme);

    let solid_panel = {
        let pf = ctx.global_style().visuals.panel_fill;
        let a = pf.a() as f32 / 255.0;
        let blend = |c: u8| (c as f32 * a + 255.0 * (1.0 - a)).round() as u8;
        egui::Color32::from_rgb(blend(pf.r()), blend(pf.g()), blend(pf.b()))
    };
    {
        let mut v = ctx.global_style().visuals.clone();
        v.panel_fill = solid_panel;
        v.window_fill = solid_panel;
        ctx.set_visuals(v);
    }
    ctx.layer_painter(egui::LayerId::background()).rect_filled(
        ctx.content_rect(),
        0.0,
        solid_panel,
    );
}

// ── eframe::App ───────────────────────────────────────────────────────────────

/// An in-flight download of the semantic Knowledge Base model, surfaced as an
/// IDE-blocking modal (the IDE must not run half-installed: a workflow syncing
/// the index mid-download would stamp it with the lexical fallback).
struct SemanticModelDownload {
    progress: std::sync::Arc<cobolt_agents::bert_embedder::DownloadProgress>,
    rx: std::sync::mpsc::Receiver<Result<std::path::PathBuf, String>>,
    /// The worker's error once it failed; the modal switches from the progress
    /// bar to a retry / continue-without choice.
    failed: Option<String>,
}

impl CoboltApp {
    /// Gate for the semantic Knowledge Base model. A
    /// missing model is OFFERED (confirmation dialog) when a document is added
    /// to the project Knowledge Base, and downloadable from the Models Manager
    /// at any time; unreadable files are discarded by the accepted download's
    /// worker. While a download runs, an [`egui::Modal`] (IDE-themed by
    /// construction — it renders with the live style) blocks the whole IDE and
    /// shows total size, progress and the translated explanation; it
    /// disappears when the download completes and only ever returns when the
    /// model must be fetched again.
    fn semantic_model_gate(&mut self, ctx: &egui::Context) {
        use std::sync::atomic::Ordering;
        use std::sync::mpsc::TryRecvError;

        // The download is OFFERED, never imposed: adding a document to the
        // project Knowledge Base while the model is absent opens a
        // confirmation dialog (see `offer_semantic_model_for_kb`); only an
        // accepted offer — or the Models Manager button — starts the blocking
        // download below. Declining leaves the search lexical and quiet.
        if self.semantic_offer_open && self.semantic_download.is_none() {
            let tr = self.lang.tr();
            let mut accept = false;
            let mut later = false;
            egui::Modal::new(egui::Id::new("semantic_model_offer")).show(ctx, |ui| {
                ui.set_width(440.0);
                ui.heading(tr.models_semantic_label);
                ui.add_space(6.0);
                ui.label(tr.models_semantic_offer);
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(tr.models_semantic_download).clicked() {
                        accept = true;
                    }
                    if ui.button(tr.ai_setup_later).clicked() {
                        later = true;
                    }
                });
            });
            if accept {
                self.semantic_offer_open = false;
                self.start_semantic_download();
            }
            if later {
                self.semantic_offer_open = false;
                // Once per session: the next document add must not nag again.
                self.semantic_offer_declined = true;
                self.output.push_status(
                    "Semantic model not downloaded — Knowledge Base search stays lexical. \
                     Download it any time from the Models Manager.",
                );
            }
        }

        // Drain the worker before rendering, outside the borrow of the modal.
        let mut finished: Option<Result<std::path::PathBuf, String>> = None;
        if let Some(download) = &mut self.semantic_download {
            if download.failed.is_none() {
                match download.rx.try_recv() {
                    Ok(result) => finished = Some(result),
                    Err(TryRecvError::Empty) => {
                        ctx.request_repaint_after(std::time::Duration::from_millis(150));
                    }
                    Err(TryRecvError::Disconnected) => {
                        download.failed =
                            Some("the download thread ended unexpectedly".to_string());
                    }
                }
            }
        }
        match finished {
            Some(Ok(dir)) => {
                self.output.push_status(format!(
                    "Semantic search model installed in {} — Knowledge Base search is now multilingual.",
                    dir.display()
                ));
                self.semantic_download = None;
                if let Some(manager) = &mut self.models_modal {
                    manager.invalidate_semantic_probe();
                }
            }
            Some(Err(error)) => {
                if let Some(download) = &mut self.semantic_download {
                    download.failed = Some(error);
                }
            }
            None => {}
        }

        let Some(download) = &self.semantic_download else {
            return;
        };
        let tr = self.lang.tr();
        let mut retry = false;
        let mut continue_without = false;
        egui::Modal::new(egui::Id::new("semantic_model_gate")).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.heading(tr.models_semantic_downloading);
            ui.add_space(6.0);
            ui.label(tr.models_semantic_progress_why);
            ui.add_space(10.0);
            if let Some(error) = &download.failed {
                ui.label(egui::RichText::new(error).color(ui.visuals().error_fg_color));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr.models_semantic_retry).clicked() {
                        retry = true;
                    }
                    if ui.button(tr.models_semantic_continue_without).clicked() {
                        continue_without = true;
                    }
                });
            } else {
                let downloaded = download.progress.downloaded.load(Ordering::Relaxed);
                let total = download.progress.total.load(Ordering::Relaxed);
                let fraction = if total > 0 {
                    (downloaded as f32 / total as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                ui.add(egui::ProgressBar::new(fraction).show_percentage().animate(true));
                ui.add_space(4.0);
                let mb = |bytes: u64| bytes as f64 / (1024.0 * 1024.0);
                if total > 0 {
                    ui.label(format!("{:.1} MB / {:.1} MB", mb(downloaded), mb(total)));
                } else {
                    // The preflight is still sizing the transfer.
                    ui.label(format!("{:.1} MB", mb(downloaded)));
                }
            }
        });
        if retry {
            self.start_semantic_download();
        }
        if continue_without {
            self.output.push_status(
                "Semantic model not installed — Knowledge Base search stays lexical. \
                 Download it any time from the Models Manager.",
            );
            self.semantic_download = None;
            self.semantic_offer_declined = true;
        }
    }

    /// Offer the semantic model when a document is being added to the project
    /// Knowledge Base and the model is not available. Confirmation first —
    /// the 470 MB download never starts itself. The document add itself
    /// proceeds regardless of the answer: the per-workflow sync restamps the
    /// index with whichever embedder is active, so a document added before
    /// the download becomes semantic on the next sync.
    fn offer_semantic_model_for_kb(&mut self) {
        if !self.semantic_offer_declined
            && self.semantic_download.is_none()
            && !cobolt_agents::project_knowledge::semantic_model_is_ready()
        {
            self.semantic_offer_open = true;
        }
    }

    /// Spawn the download worker and open the blocking modal (replaces a
    /// failed attempt when called from Retry).
    fn start_semantic_download(&mut self) {
        let progress =
            std::sync::Arc::new(cobolt_agents::bert_embedder::DownloadProgress::default());
        let worker = progress.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use cobolt_agents::project_knowledge as pk;
            // Present-but-unreadable files (truncation, corruption) would make
            // the download a no-op that "succeeds" into the same broken state;
            // probe and discard them first, on this worker — the probe loads
            // the model and is too heavy for the UI thread.
            if pk::semantic_model_is_ready()
                && pk::semantic_model_probe() == pk::SemanticModelState::Corrupt
            {
                let _ = pk::discard_semantic_model();
            }
            let _ = tx.send(pk::download_semantic_model(&worker));
        });
        self.output.push_status(format!(
            "Downloading {} …",
            cobolt_agents::bert_embedder::MODEL_ID
        ));
        self.semantic_download = Some(SemanticModelDownload {
            progress,
            rx,
            failed: None,
        });
    }
}

impl CoboltApp {
    /// Keep the crash log's context current, and copy unsaved work aside on a
    /// timer.
    ///
    /// The open-file list is rebuilt every frame rather than on the timer: the
    /// reported crash happened seconds after opening a window, and a report
    /// that failed to name what was open would have been no use. It is a
    /// handful of short paths — cheap beside anything else in a frame.
    /// Offer work rescued from a session that ended badly.
    ///
    /// Shown before anything else can take focus, because the developer's first
    /// question after the IDE disappears is whether their work went with it.
    fn show_recovery_prompt(&mut self, ctx: &egui::Context) {
        if self.pending_recovery.is_empty() {
            return;
        }
        let tr = self.lang.tr();
        let mut restore = false;
        let mut discard = false;

        egui::Window::new(tr.recover_title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_max_width(560.0);
                ui.label(tr.recover_body);
                ui.add_space(8.0);
                for item in &self.pending_recovery {
                    ui.label(format!("• {}", item.origin.display()));
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    restore = ui.button(tr.recover_restore).clicked();
                    discard = ui.button(tr.recover_discard).clicked();
                });
            });

        if restore {
            let written = crate::crash::restore_beside_originals(&self.pending_recovery);
            for path in &written {
                self.output.push_status(format!("↺ {}", path.display()));
            }
            crate::crash::discard();
            self.pending_recovery.clear();
        } else if discard {
            crate::crash::discard();
            self.pending_recovery.clear();
        }
    }

    fn recovery_tick(&mut self) {
        let mut open: Vec<PathBuf> = self.editor.tabs.iter().map(|t| t.path.clone()).collect();
        open.extend(self.designers.iter().map(|(p, _)| p.clone()));
        crate::crash::note_open(open);

        if self.last_autosave.elapsed().as_secs_f64() < crate::crash::AUTOSAVE_SECS {
            return;
        }
        self.last_autosave = std::time::Instant::now();

        let mut items: Vec<crate::crash::Recoverable> = Vec::new();
        for tab in &self.editor.tabs {
            // Generated code is read-only and regenerated on demand; recovering
            // it would restore a copy of something the project rebuilds anyway.
            if tab.dirty && !tab.read_only {
                items.push(crate::crash::Recoverable {
                    origin: tab.path.clone(),
                    body: tab.content.clone(),
                });
            }
        }
        for (path, designer) in &self.designers {
            if !designer.dirty {
                continue;
            }
            // A form that will not serialize is skipped rather than reported:
            // autosave must never interrupt the developer mid-edit.
            if let Ok(xml) = cobolt_forms::form_to_string(&designer.form) {
                items.push(crate::crash::Recoverable {
                    origin: path.clone(),
                    body: xml,
                });
            }
        }
        crate::crash::autosave(&items);
    }
}

impl eframe::App for CoboltApp {
    /// Clear to fully transparent so the OS compositor blends our semi-transparent
    /// panels directly against the desktop wallpaper.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // The whole IDE is laid out with Context-level panels (top bar, side
        // panels, central canvas), so the per-frame entry point only needs the
        // Context; the root `Ui` itself hosts nothing directly.
        let ctx = root_ui.ctx().clone();
        let ctx = &ctx;
        let frame_start = std::time::Instant::now();

        // Every error shown to the developer also lands in the console
        // (operator, 2026-08-09). The panels that raise them cannot reach the
        // Output panel, so they record instead and this drains the record —
        // once a frame, before anything else can add to it.
        // Unsaved work is copied aside on a timer so a hard kill costs at most
        // one interval. Runs before the UI: if this frame is the one that
        // panics, the copy is already on disk.
        self.recovery_tick();
        self.show_recovery_prompt(ctx);

        for message in crate::error_log::drain() {
            self.output.push_status(format!("✗ {message}"));
        }

        // ── Compute the translation table for this frame ───────────────────────
        let tr = self.lang.tr();
        crate::i18n::set_language(ctx, self.lang);
        // Spec 044 R20 — publish the project's registered crates for every
        // semantic-analysis site (Check/Run/Debug/Build workers included),
        // the same per-frame in-process sync the theme and debug switches use.
        crate::external_crates_service::set_active_project_crates(
            self.cobolt_project
                .as_ref()
                .map(|p| p.crates.iter().map(|c| c.lib_name()).collect()),
        );
        // Remember the language across restarts. Written only on a real change,
        // so this costs nothing on a normal frame.
        if self.lang != self.lang_persisted {
            crate::ui_prefs::save_language(self.lang);
            self.lang_persisted = self.lang;
        }
        self.poll_llm_benchmark();
        self.poll_capability_probe();
        self.poll_form_paste(ctx, &tr);
        // Cleanups the designers performed on their own reach the Output panel
        // here — an automatic removal that leaves no trace is how a developer
        // loses work without knowing it.
        let notices: Vec<String> = self
            .designers
            .iter_mut()
            .flat_map(|(_, d)| d.orphan_notices.drain(..))
            .collect();
        for notice in notices {
            self.output.push_status(notice);
        }
        if self.llm_benchmark_rx.is_some() {
            ctx.request_repaint();
        }
        // Semantic KB model gate: probes on startup and, when a download is
        // needed (first run, cleaned cache, corrupt files), blocks the IDE
        // behind a themed progress modal until the model is installed.
        self.semantic_model_gate(ctx);

        // Keep the in-process diagnostics (design canvas / IDE) in sync with the
        // debug settings, so toggling one in Help → Debug Settings applies
        // immediately without a rebuild or an env var. Run Form is a separate
        // process and picks its flags up via env on its next launch.
        self.debug.apply_in_process();

        // F12 documentation capture. Polled per viewport (here for the main
        // window, and at each `show_viewport_immediate` site) because the key
        // and the capture reply both belong to whichever window has focus.
        self.doc_shots.poll(ctx, self.debug.doc_screenshots);

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
                format!("{} {VERSION}", crate::theme::brand_name())
            } else {
                format!("{} {VERSION} — {mode_suffix}", crate::theme::brand_name())
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

        // Intercept main window close if anything is changed and not saved
        // (dirty forms, code editor tabs, or project settings). Once the user has
        // decided (Save / Close without saving), `allow_close` lets it through.
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && self.has_unsaved_changes()
            && !self.close_confirm
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_confirm = true;
        }

        // ── Indexed editor / grid viewports (before main-shell theme) ─────────
        // egui 0.29 shares `set_visuals` globally; paint opaque child windows first
        // so `apply_glass_visuals` below can restore the translucent main shell.
        self.show_indexed_grid_viewports(ctx, &tr);

        // ── Apply Liquid Glass visuals every frame on the root context ─────────
        // (the indexed grid viewports above write the shared context style, and
        //  a theme switch must land immediately — re-applying here keeps the
        //  IDE shell correct in both cases. The preview window styles only its
        //  own Ui subtree and no longer touches the context.)
        apply_glass_visuals(ctx, self.current_theme());
        self.glass_visuals_applied = true;

        // ── Opaque background that matches the panes ───────────────────────────
        // 1) an opaque dark floor (no desktop bleed), 2) the optional background
        // image as a subtle texture, 3) the SAME semi-opaque pane fill over the
        // whole window, so the area around/between the panes looks exactly like a
        // pane (not a brighter "transparent" wallpaper showing through the gaps).
        {
            let p = ctx.global_style().visuals.panel_fill;
            let floor = egui::Color32::from_rgb(p.r(), p.g(), p.b());
            ctx.layer_painter(egui::LayerId::background()).rect_filled(
                ctx.content_rect(),
                0.0,
                floor,
            );
        }
        self.paint_ide_background(ctx);
        {
            let p = ctx.global_style().visuals.panel_fill;
            ctx.layer_painter(egui::LayerId::background())
                .rect_filled(ctx.content_rect(), 0.0, p);
        }

        // ── Drain a finished async file dialog (Open/Save/Browse) ──────────────
        // Repaint while one is open so its result is collected promptly.
        if self.poll_file_dialog() {
            ctx.request_repaint();
        }
        if let Some(project_root) = self.project_dir() {
            if crate::panels::editor::take_chat_documentation_changed(&project_root) {
                self.sync_project_documentation_membership(&project_root);
            }
            if crate::tool_exec::take_indexed_files_changed(&project_root) {
                self.sync_project_indexed_membership(&project_root);
            }
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
                    let line = format!(
                        "✅ Build complete!  Binary → {}   ({} source(s), {} form(s), {} bytes AST)",
                        result.binary_path.display(),
                        result.source_count,
                        result.form_count,
                        result.ast_bytes,
                    );
                    self.output.push_status(line.clone());
                    self.build_log.push((BuildLogKind::Success, line.clone()));
                    // The Building dialog does not self-dismiss: it stays up
                    // with the outcome until the user closes it.
                    self.build_outcome = Some(Ok(line));
                    self.pending_build_rx = None;
                    self.pending_build_progress = None;
                    // Only a FULL build stamps the project: it is the only one
                    // that can promise no artefact from an older
                    // PowerRustCOBOL survived into this binary. Persisted at
                    // once, so the prompt does not return on the next Run.
                    if self.pending_build_full {
                        self.pending_build_full = false;
                        if let Some(p) = &mut self.cobolt_project {
                            if p.project.built_with_version != crate::version::VERSION {
                                p.project.built_with_version =
                                    crate::version::VERSION.to_owned();
                                let tr = self.lang.tr();
                                self.output.push_status(
                                    tr.status_build_stamped
                                        .replace("{version}", crate::version::VERSION),
                                );
                                if let (Some(path), Some(proj)) =
                                    (self.project_path.clone(), self.cobolt_project.as_ref())
                                {
                                    if let Err(e) =
                                        crate::project_model::save_project(proj, &path)
                                    {
                                        self.output.push_status(format!(
                                            "⚠️  Could not record the build version: {e}"
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    // Run started this build because the program contains
                    // EXEC RUST (spec 041 T13) — start what it produced.
                    if let Some(form_path) = self.pending_build_then_run.take() {
                        self.launch_built_binary(&result.binary_path, &form_path);
                        // The operator pressed Run, not Build: the launched
                        // application is the outcome they asked for, and it
                        // opens *behind* this modal — which never self-dismisses
                        // and blocks the whole IDE. Leaving it up buried the
                        // app; a plain Build keeps the modal, as before.
                        if self.build_outcome.as_ref().is_some_and(|o| o.is_ok()) {
                            self.build_modal_closed = true;
                        }
                    }
                }
                Ok(Err(e)) => {
                    self.output.push_status(format!("❌ Build failed: {e}"));
                    self.build_log
                        .push((BuildLogKind::Error, format!("Build failed: {e}")));
                    // Errors are what the details window exists for — open it.
                    self.build_details_open = true;
                    // …and the Building dialog stays up showing the failure
                    // until the user closes it. `e` is still needed below.
                    self.build_outcome = Some(Err(e.clone()));
                    self.pending_build_rx = None;
                    self.pending_build_progress = None;
                    if let Some(form_path) = self.pending_build_then_run.take() {
                        let tr = self.lang.tr();
                        // A build the developer did not ask for failed, so say
                        // plainly that nothing was started, and — when the
                        // toolchain is what is missing — how to fix it.
                        self.output
                            .push_status(tr.status_exec_rust_build_failed.to_owned());
                        if e.contains("Rust toolchain is required") {
                            self.output
                                .push_status(tr.status_exec_rust_toolchain_missing.to_owned());
                        }
                        self.set_element_status(&form_path, ElementStatus::Failed);
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint(); // keep polling
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    let msg = "Build thread disconnected unexpectedly.";
                    self.output.push_status(format!("❌ {msg}"));
                    self.build_log.push((BuildLogKind::Error, msg.to_string()));
                    // Without an outcome the dialog would spin forever with a
                    // disabled Close — report the loss and let the user out.
                    self.build_outcome = Some(Err(msg.to_string()));
                    self.pending_build_rx = None;
                    self.pending_build_progress = None;
                    // A Run intent dies with the build thread — say so, and
                    // mark the form, instead of dropping it in silence.
                    if let Some(form_path) = self.pending_build_then_run.take() {
                        let tr = self.lang.tr();
                        self.output
                            .push_status(tr.status_run_not_started.to_owned());
                        self.set_element_status(&form_path, ElementStatus::Failed);
                    }
                }
            }
        }

        // ── Drain manual KB-reindex progress (File menu) ─────────────────────
        if let Some(rx) = &self.kb_reindex_rx {
            let mut finished = false;
            loop {
                match rx.try_recv() {
                    Ok(KbReindexMsg::Phase(fraction, label)) => {
                        self.kb_reindex_phase = (fraction, label);
                    }
                    Ok(KbReindexMsg::Done(Ok(summary))) => {
                        self.output.push_status(format!("✅ {summary}"));
                        finished = true;
                    }
                    Ok(KbReindexMsg::Done(Err(e))) => {
                        self.output
                            .push_status(format!("❌ Knowledge Base reindex: {e}"));
                        finished = true;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        // Worker gone without a Done (panic): report, stop.
                        if !finished {
                            self.output
                                .push_status("❌ Knowledge Base reindex stopped unexpectedly.");
                        }
                        finished = true;
                        break;
                    }
                }
            }
            if finished {
                self.kb_reindex_rx = None;
            } else {
                ctx.request_repaint(); // keep polling while the worker runs
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
        self.show_about(ctx);
        self.show_project_upgrade_modal(ctx, &tr);
        self.show_toolchain_prompt(ctx, &tr);
        self.show_ai_setup_modal(ctx, &tr);
        // External Crates (spec 044): the service mutates `cobolt.toml` on
        // disk; when an action finished, reload so the tree shows the pins
        // (the project was saved before the dialog opened — see the
        // OpenExternalCrates event arm).
        if self.show_external_crates {
            let crates = self
                .cobolt_project
                .as_ref()
                .map(|p| p.crates.clone())
                .unwrap_or_default();
            let mut open = self.show_external_crates;
            let changed = self.external_crates_panel.show(
                ctx,
                &mut open,
                self.project_path.as_deref(),
                &crates,
                &tr,
            );
            self.show_external_crates = open;
            if changed {
                if let Some(path) = self.project_path.clone() {
                    match crate::project_model::load_project(&path) {
                        Ok(project) => self.cobolt_project = Some(project),
                        Err(e) => tracing::warn!("cannot reload project after crate change: {e}"),
                    }
                }
            }
        }
        // Save alert: render it in the MAIN window only when it doesn't belong
        // to a designer viewport (those render it themselves, on top).
        // "Building…" modal — blocks the IDE and stays up, showing the
        // outcome, until the user closes it.
        // Exactly one surface hosts the build modal: the designer window whose
        // Run Form started the build, else this main window. If that designer
        // closed mid-build, fall back here so the modal is never orphaned.
        let hosted_by_open_designer = self.build_hosted_by_open_designer();
        if !hosted_by_open_designer {
            self.show_building_modal(ctx);
        }
        self.show_kb_reindex_modal(ctx);
        // Build details goes wherever the modal it opens from goes.
        if !hosted_by_open_designer {
            self.show_build_details_window(ctx);
        }
        // Fatal COBOL error (launch or runtime) — modal, IDE stays open.
        self.show_proc_delete_confirmation(ctx);
        self.show_form_error(ctx);
        // "Built by an older PowerRustCOBOL" — offered before every Run until
        // a full build clears it. A prompt raised by a designer's Run Form is
        // hosted by THAT designer window (the operator is looking there);
        // this main window shows it for the toolbar's Run, or as the fallback
        // when that designer has closed.
        if !self.stale_prompt_hosted_by_open_designer() {
            self.show_stale_build_prompt(ctx);
        }
        // Duplicate COBOL ID / Validation alert — modal.
        self.show_alert_error(ctx);
        // Model benchmark offer/progress/report is global: the worker can finish
        // after the user leaves Project Settings.
        self.render_llm_benchmark_modals(ctx);
        // Help → Debug Settings. A change re-applies the in-process flags at
        // once, so the canvas reacts on this very frame.
        if self.debug_modal.show(ctx, &mut self.debug, &tr) {
            self.debug.apply_in_process();
        }
        // Placement popup for a captured shot. Main window only: it must never
        // be part of the frame the operator is photographing. The theme's panel
        // colour is what the transparent capture gets flattened onto.
        self.doc_shots
            .ui(ctx, self.current_theme().bg_panel.to_opaque());

        // ── Menu bar ─────────────────────────────────────────────────────────
        let has_project = self.cobolt_project.is_some();
        // "Active" = a project is open or a file is being edited; gates the
        // Run / View menus (and the Save/Check toolbar buttons below).
        let menu_has_active = has_project || self.editor.active_source().is_some();
        egui::Panel::top("menu_bar").show(root_ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button(tr.menu_file, |ui| {
                    if ui.button(tr.menu_new_project).clicked()     { self.do_new_project();  ui.close(); }
                    if ui.button(tr.menu_open_project).clicked()    { self.do_open_project(); ui.close(); }
                    if ui.add_enabled(has_project, egui::Button::new(tr.menu_save_project)).clicked() {
                        self.do_save_project(); ui.close();
                    }
                    if ui.add_enabled(has_project, egui::Button::new(tr.menu_package_project)).clicked() {
                        self.do_package_project(); ui.close();
                    }
                    let building = self.pending_build_rx.is_some();
                    let build_label = if building { "⏳ Building…" } else { "🔨 Build Binary  (bin/)" };
                    if ui.add_enabled(has_project && !building, egui::Button::new(build_label))
                        .on_hover_text("Compile project → single native executable in bin/")
                        .clicked()
                    {
                        self.do_build_binary_button(); ui.close();
                    }
                    ui.separator();
                    // Manual KB reindex — the same incremental sync a Grace
                    // workflow runs at start, without sending a message.
                    let reindexing = self.kb_reindex_rx.is_some();
                    let reindex_label = if reindexing {
                        tr.menu_reindex_kb_busy
                    } else {
                        tr.menu_reindex_kb
                    };
                    if ui
                        .add_enabled(!reindexing, egui::Button::new(reindex_label))
                        .on_hover_text(tr.menu_reindex_kb_hint)
                        .clicked()
                    {
                        self.do_reindex_kb();
                        ui.close();
                    }
                    ui.separator();
                    if ui.add_enabled(self.has_unsaved_changes(), egui::Button::new(tr.menu_save)).clicked() { self.do_save(); ui.close(); }
                    ui.separator();
                    if ui.button(tr.menu_quit).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.add_enabled_ui(menu_has_active, |ui| {
                    ui.menu_button(tr.menu_run, |ui| {
                        if ui.add_enabled(!self.runner.is_running(),
                                         egui::Button::new(tr.menu_run_btn)).clicked() {
                            self.do_run(); ui.close();
                        }
                        if ui.add_enabled(self.runner.is_running(),
                                         egui::Button::new(tr.menu_stop)).clicked() {
                            self.do_stop(); ui.close();
                        }
                        ui.separator();
                        if ui.button(tr.menu_check_only).clicked() { self.do_check(); ui.close(); }
                    });

                    ui.menu_button(tr.menu_view, |ui| {
                        ui.checkbox(&mut self.editor.show_line_numbers, tr.menu_line_numbers);
                    });
                });

                // ── Help / Bug report ────────────────────────────────────────
                ui.menu_button("Help", |ui| {
                    if ui.button(tr.doc_menu_label).clicked() {
                        self.doc_viewer.open(self.lang);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(tr.debug_menu_label).clicked() {
                        self.debug_modal.toggle();
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .button(format!("ℹ About {}", crate::theme::brand_name()))
                        .clicked()
                    {
                        self.about_open = true;
                        ui.close();
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
        // Save is additionally gated on there being something to save. Computed
        // before the call so it does not overlap the `&mut self.lang` borrow.
        let has_unsaved = self.has_unsaved_changes();
        match toolbar::show(
            root_ui,
            ctx,
            &self.runner,
            &tr,
            &mut self.lang,
            compilable,
            debuggable,
            has_active,
            has_unsaved,
        ) {
            ToolbarAction::Run => self.do_run(),
            ToolbarAction::Stop => self.do_stop(),
            ToolbarAction::Debug => self.do_debug(),
            ToolbarAction::Build => self.do_build_binary_button(),
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
            self.output.show(root_ui, &tr);

            // 037 R4 — an open designer's (possibly unsaved) MainForm claim
            // outranks the on-disk flags, so the tree crown moves the moment
            // the checkbox is ticked, not on save. The project-panel inspect
            // view carries the same weight: its claim saves eagerly, but the
            // demotion of the previous holder settles a frame later — the
            // override keeps the crown unique on screen meanwhile.
            self.project.main_form_override = self
                .designers
                .iter()
                .find(|(_, d)| d.form.main_form)
                .map(|(p, _)| p.clone())
                .or_else(|| {
                    self.inspect
                        .as_ref()
                        .filter(|st| st.designer.form.main_form)
                        .map(|st| st.path.clone())
                });

            let proj_events = self
                .project
                .show(root_ui, self.cobolt_project.as_ref(), &tr);

            // OS file-manager drop into the tree (spec 033, R10). eframe surfaces
            // dropped paths on `raw.dropped_files`; the panel tells us which folder
            // was under the pointer.
            // egui 0.36: DroppedFile is a trait, `path()` always present.
            let dropped: Vec<PathBuf> = ctx.input(|i| {
                i.raw
                    .dropped_files
                    .iter()
                    .map(|f| f.path().to_path_buf())
                    .collect()
            });
            if !dropped.is_empty() {
                if let Some(dest) = self.project.hovered_dir().map(str::to_string) {
                    self.do_import_os_files(dropped, dest);
                }
            }

            for ev in proj_events {
                if !matches!(&ev, ProjectPanelEvent::OpenGraceChat) {
                    self.show_grace_chat = false;
                }
                match ev {
                    ProjectPanelEvent::OpenGraceChat => {
                        self.show_grace_chat = true;
                        self.show_project_settings = false;
                        self.inspect = None;
                        self.indexed_inspect = None;
                        self.asset_preview = None;
                        self.pending_open_in_editor = None;
                    }
                    ProjectPanelEvent::Open(path) => {
                        self.show_project_settings = false;
                        self.inspect = None;
                        self.indexed_inspect = None;
                        self.open_in_editor(path);
                    }
                    ProjectPanelEvent::OpenDesigner(path) => {
                        self.show_project_settings = false;
                        self.asset_preview = None;
                        self.load_form_from_path(path);
                    }
                    ProjectPanelEvent::OpenIndexedEditor(path) => {
                        self.show_project_settings = false;
                        self.asset_preview = None;
                        self.open_indexed_inspect(path, None);
                    }
                    ProjectPanelEvent::InspectIndexedFile(path) => {
                        self.show_project_settings = false;
                        self.inspect = None;
                        self.asset_preview = None;
                        self.open_indexed_inspect(path, None);
                    }
                    ProjectPanelEvent::InspectIndexedField { cidx, field_id } => {
                        self.show_project_settings = false;
                        self.inspect = None;
                        self.asset_preview = None;
                        self.open_indexed_inspect(cidx, Some(field_id));
                    }
                    ProjectPanelEvent::InspectForm(path) => {
                        self.show_project_settings = false;
                        self.indexed_inspect = None;
                        self.asset_preview = None;
                        self.open_inspect(path, None);
                    }
                    ProjectPanelEvent::InspectControl { form, ctrl_id } => {
                        self.show_project_settings = false;
                        self.asset_preview = None;
                        self.open_inspect(form, Some(ctrl_id));
                    }
                    ProjectPanelEvent::OpenEventCode { form, paragraph } => {
                        self.show_project_settings = false;
                        self.inspect = None;
                        self.asset_preview = None;
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
                    ProjectPanelEvent::CreateIn { kind, dir_rel } => {
                        self.do_create_in_folder(kind, &dir_rel)
                    }
                    // Spec 044 R3 — the External Crates category routes every
                    // affordance to its dialog. The service works on
                    // `cobolt.toml` directly, so persist any in-memory project
                    // state first (the panel's state contract).
                    ProjectPanelEvent::OpenExternalCrates => {
                        self.do_save_project();
                        self.show_external_crates = true;
                    }
                    ProjectPanelEvent::Add(kind) => self.do_add_file_to_project(kind),
                    ProjectPanelEvent::Remove(rel) => self.do_remove_file_from_project(rel),
                    ProjectPanelEvent::CreateKnowledgeFolder(parent) => {
                        self.knowledge_folder_parent = Some(parent);
                        self.knowledge_folder_name.clear();
                    }
                    ProjectPanelEvent::ConfirmDeleteKnowledgeFolder(folder) => {
                        self.pending_knowledge_folder_delete = Some(folder);
                    }
                    ProjectPanelEvent::ConfirmRemoveForm(path) => {
                        self.pending_form_delete = Some(path);
                    }
                    ProjectPanelEvent::CopyForm(path) => {
                        self.copy_form(ctx, &path);
                    }
                    ProjectPanelEvent::PasteForm { dir_rel } => {
                        if let Some(root) = self.project_dir() {
                            let dest = root.join(&dir_rel);
                            let _ = std::fs::create_dir_all(&dest);
                            self.paste_form_requested(ctx, &dest);
                        }
                    }
                    ProjectPanelEvent::ConfirmRemoveGenerated(path) => {
                        self.pending_generated_delete = Some(path);
                    }
                    ProjectPanelEvent::ConfirmRemoveAsset(path) => {
                        self.pending_asset_delete = Some(path);
                    }
                    ProjectPanelEvent::ConfirmRemoveIndexed(rel) => {
                        self.pending_indexed_delete = Some(rel);
                    }
                    ProjectPanelEvent::ShowProjectSettings => {
                        self.show_project_settings = true;
                        self.inspect = None;
                        self.indexed_inspect = None;
                        self.asset_preview = None;
                        // Any pending editor open should yield to the settings form.
                        self.pending_open_in_editor = None;
                    }
                    ProjectPanelEvent::CreateFolder {
                        parent_rel,
                        category_root,
                    } => {
                        self.folder_create = Some(PendingFolderCreate {
                            parent_rel,
                            category_root,
                            name: String::new(),
                        });
                    }
                    ProjectPanelEvent::RenameFolder {
                        folder_rel,
                        category_root,
                    } => {
                        let name = folder_rel
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string();
                        self.folder_rename = Some(PendingFolderRename {
                            folder_rel,
                            category_root,
                            name,
                        });
                    }
                    ProjectPanelEvent::DeleteFolder {
                        folder_rel,
                        category_root,
                    } => {
                        self.folder_delete = Some(PendingFolderDelete {
                            folder_rel,
                            category_root,
                        });
                    }
                    ProjectPanelEvent::MoveInternal {
                        src_rel,
                        dest_dir_rel,
                    } => self.do_move_tracked_file(src_rel, dest_dir_rel),
                    ProjectPanelEvent::ImportOs {
                        paths,
                        dest_dir_rel,
                    } => self.do_import_os_files(paths, dest_dir_rel),
                }
            }
        }

        // Main Pane priority: when no project show the localized welcome
        // (developer's guide); otherwise the previous logic (settings / inspector / editor).
        if !has_project {
            self.show_welcome_pane(root_ui, &tr);
        } else if self.show_grace_chat {
            if let Some(root) = self.project_dir() {
                // Computed before the panel borrows `self.grace_chat` mutably.
                let surface = self.grace_chat_surface_context();
                let action = self
                    .grace_chat
                    .show(root_ui, &root, &self.llm, &tr, &surface);
                if action.rescan_documentation {
                    self.sync_project_documentation_membership(&root);
                }
                if action.close {
                    self.show_grace_chat = false;
                }
            }
        } else if self.show_project_settings && self.settings_form.is_some() {
            self.show_settings_pane(root_ui, &tr);
        } else if self.indexed_inspect.is_some() {
            self.show_indexed_inspector(root_ui, &tr);
        } else if self.asset_preview.is_some() {
            self.show_asset_preview(root_ui, &tr);
        } else if self.inspect.is_some() {
            self.show_inspector(root_ui, &tr);
        } else {
            let root = self
                .project_path
                .as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf());
            self.editor
                .show(root_ui, ctx, Some(&self.llm), &tr, root.as_deref());
        }

        // ── Unsaved project settings close-confirmation dialog (main window) ────
        if self.close_confirm {
            let mut open = true;
            egui::Window::new(tr.app_close_title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(tr.app_close_msg);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        // Save before close.
                        if ui.button(tr.close_save).clicked() {
                            self.save_all_unsaved();
                            self.close_confirm = false;
                            self.allow_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        // Close without saving.
                        if ui.button(tr.close_discard).clicked() {
                            self.close_confirm = false;
                            self.allow_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        // Cancel — the close was already vetoed above, so just
                        // dismiss the prompt and keep working.
                        if ui.button(tr.close_cancel).clicked() {
                            self.close_confirm = false;
                        }
                    });
                });
            // Window "X" / Esc ⇒ same as Cancel.
            if !open {
                self.close_confirm = false;
            }
        }

        self.show_form_delete_confirm(ctx, &tr);
        self.show_paste_form_conflict(ctx, &tr);
        self.show_generated_delete_confirm(ctx, &tr);
        self.show_asset_delete_confirm(ctx, &tr);
        self.show_knowledge_folder_dialog(ctx, &tr);
        self.show_knowledge_folder_delete_confirm(ctx, &tr);
        self.show_folder_create_dialog(ctx, &tr);
        self.show_folder_rename_dialog(ctx, &tr);
        self.show_folder_delete_confirm(ctx, &tr);
        self.show_target_picker(ctx, &tr);
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
            let activate = self
                .designer_activation_requests
                .take(&self.designers[idx].0);
            let title = {
                let (path, d) = &self.designers[idx];
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("form");
                let dirty = if d.dirty { " ●" } else { "" };
                format!(
                    "{} Form Designer  v{VERSION} — {stem}{dirty}",
                    crate::theme::brand_name()
                )
            };

            ctx.show_viewport_immediate(
                vp_id,
                ViewportBuilder::default()
                    .with_title(&title)
                    .with_inner_size([1200.0, 800.0]),
                |vp_ctx, _class| {
                    self.doc_shots.poll(vp_ctx, self.debug.doc_screenshots);
                    if activate {
                        vp_ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        vp_ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        vp_ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
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
                    // The build modal renders HERE when this designer's Run
                    // Form started the build — the operator is looking at this
                    // window, and a modal in the main window behind it goes
                    // unseen (the exact complaint that motivated this).
                    if self.build_modal_host.as_deref()
                        == Some(self.designers[idx].0.as_path())
                    {
                        self.show_building_modal(vp_ctx);
                        // …and so does the Build-details window it opens: in
                        // the main window it lands BEHIND this one, so the
                        // Details button read as doing nothing at all.
                        self.show_build_details_window(vp_ctx);
                    }
                    // Same rule for the stale-build prompt this designer's
                    // Run Form raised: it must appear under the operator's
                    // eyes, not in the main window behind this one.
                    if matches!(
                        self.stale_build_prompt.as_ref().map(|p| &p.intent),
                        Some(StaleBuildIntent::RunForm(i)) if *i == idx
                    ) {
                        self.show_stale_build_prompt(vp_ctx);
                    }
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

        // 037 R2 — settle MainForm claims/un-claims emitted by designer undo
        // stacks this frame (checkbox tick, Cmd+Z, Cmd+Y all end up here).
        self.drain_main_form_changes();

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
                    self.doc_shots.poll(vp_ctx, self.debug.doc_screenshots);
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
                self.set_form_error(err);
            }
        }

        // ── Launched BUILT binaries (spec 041 T13) ───────────────────────────
        // Stream everything the compiled application prints into the Output
        // panel, and report its exit — success or failure. Before this the
        // spawned Child was discarded on the spot, so a program that died at
        // startup left the IDE showing "starting the built program" and a green
        // semaphore forever: the exact silence the operator kept reporting.
        if !self.built_runs.is_empty() {
            let mut built_error: Option<String> = None;
            let mut i = 0;
            while i < self.built_runs.len() {
                for line in self.built_runs[i].drain_output() {
                    self.output.push_line(line);
                }
                if self.built_runs[i].is_running() {
                    i += 1;
                    continue;
                }
                let run = self.built_runs.remove(i);
                // Drain what raced the exit — often the only evidence.
                for line in run.drain_output() {
                    self.output.push_line(line);
                }
                let tr = self.lang.tr();
                match run.exit_error() {
                    None => {
                        self.output.push_status(
                            tr.status_built_exited_ok.replace("{name}", &run.name),
                        );
                    }
                    Some(err) => {
                        let msg = tr
                            .status_built_exited_err
                            .replace("{name}", &run.name)
                            .replace("{err}", &err);
                        self.output.push_status(msg.clone());
                        self.set_element_status(&run.form_path, ElementStatus::Failed);
                        if built_error.is_none() {
                            built_error = Some(msg);
                        }
                    }
                }
            }
            if let Some(err) = built_error {
                self.set_form_error(err);
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        // ── Running forms ────────────────────────────────────────────────────────
        // Run Form executes in an EXTERNAL `rcrun run-form` process hosting the
        // shared `cobolt-form-host` window (spec 042); the in-IDE `FormRuntime`
        // viewport host was retired with it (042 R4) — it had been unreachable
        // since the external run landed.

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
            let elapsed = self
                .perf_window_start
                .get_or_insert_with(std::time::Instant::now)
                .elapsed();
            if elapsed.as_secs_f32() >= 1.0 {
                self.perf_fps = self.perf_frames;
                self.perf_avg_ms = self.perf_ms_sum / self.perf_frames.max(1) as f32;
                self.perf_max_ms = self.perf_ms_max;
                if !self.external_runs.is_empty() {
                    crate::runner::dbg_log(&format!(
                        "[PERF] fps={} avg={:.1}ms max={:.1}ms external={} designers={} inspector={}",
                        self.perf_fps,
                        self.perf_avg_ms,
                        self.perf_max_ms,
                        self.external_runs.len(),
                        self.designers.len(),
                        self.show_inspector,
                    ));
                }
                self.perf_frames = 0;
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
        // A Switch reports its state as `Checked`, like the other two toggles.
        // It fell through to "Caption", so every toggle the operator made in the
        // preview was discarded on the way back and the switch never moved.
        CT::CheckBox | CT::RadioButton | CT::Switch => "Checked",
        CT::TabControl => "SelectedTab",
        CT::ComboBox | CT::ListBox | CT::Slider | CT::ProgressBar | CT::NumericUpDown => "Value",
        _ => "Caption",
    }
}

/// Whether an update the engine reported belongs in the preview's value map,
/// which holds exactly one value per control (keyed by [`preview_value_key`]).
///
/// A toggle's state has two names: a CheckBox and a RadioButton report a click
/// as `Value`, a Switch as `Checked`, while the preview keys all three by
/// `Checked`. Accepting only the exact key meant the click was collected from
/// the engine and then thrown away here, so a CheckBox in the preview never
/// toggled however well it was drawn. `PreviewState::live` writes the stored
/// value into `Checked` either way, so taking the engine's spelling is enough.
pub(crate) fn preview_accepts_update(expected: &str, key: &str) -> bool {
    key == expected || (expected == "Checked" && key == "Value")
}

/// A control may report more than one value — a ListBox writes its active row,
/// its Ctrl-click selection AND its ticked set. The preview map holds one value
/// per control, so these are kept beside it under `id::Prop` (see
/// [`PreviewState::live`]). Listed explicitly rather than "everything else", so
/// a transient the engine emits cannot quietly become preview state.
pub(crate) fn preview_keeps_extra_update(key: &str) -> bool {
    matches!(
        key,
        "SelectedItems" | "CheckedItems" | "SelectedIndex" | "RejectedFiles" | "DroppedFiles"
    )
}

/// What Preview does with one toolbar-button press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreviewPress {
    /// The form's own COBOL owns this action, and Preview has no interpreter to
    /// run it. Nothing to say — Run Form is where it happens.
    LeaveToTheForm,
    /// A window capture, refused with a reason. Preview is a PANE inside the IDE
    /// window, so a capture taken here photographs the IDE and not the form — the
    /// wrong image, silently. Run Form gives the form a window of its own.
    NeedsRunForm,
    /// Preview carries it out itself.
    Perform,
}

/// The whole Preview rule for a toolbar action, in one place so a test can pin
/// every verb without standing up a designer window (operator, 2026-08-17).
pub(crate) fn preview_press(action: &cobolt_forms::toolbar::ToolbarAction) -> PreviewPress {
    use cobolt_forms::toolbar::ToolbarAction as TA;
    match action {
        _ if !action.is_platform_action() => PreviewPress::LeaveToTheForm,
        TA::Screenshot | TA::Share => PreviewPress::NeedsRunForm,
        _ => PreviewPress::Perform,
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
        // Values a control writes BESIDES its primary one — a ListBox's
        // SelectedItems and CheckedItems, say — are keyed `id::Prop`. The map
        // holds one value per control by design, and anything that did not fit
        // that shape used to be dropped on the way back, so a second selection
        // could never survive a frame in the preview.
        let prefix = format!("{}::", base.id);
        for (key, value) in self.values {
            if let Some(prop) = key.strip_prefix(&prefix) {
                c.set_prop(
                    prop.to_owned(),
                    cobolt_forms::PropValue::String(value.clone()),
                );
            }
        }
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
        // ONLY the animation's alpha, exactly like the designer canvas
        // (`DesignerState`) and the running form (`LiveState`).
        //
        // The control's own `Transparency` must NOT be folded in here. It is
        // about the control's FACE — how much of what is behind shows through —
        // and `draw_control` already applies it to the face alone, keeping the
        // caption, border, tick box and glyph fully legible. Multiplying it in a
        // second time as a whole-control alpha erased the control instead of its
        // card: a CheckBox defaults to `Transparency = 100`, so the preview drew
        // it at alpha 0 and its tick box disappeared (the caption survived only
        // because a premultiplied colour with alpha 0 renders additively).
        // Container opacity is folded in by the engine separately.
        cobolt_forms::render::RenderTransform {
            dx,
            dy,
            scale,
            alpha: anim_alpha,
        }
    }
}

impl CoboltApp {
    fn show_preview_window(&mut self, panel_ui: &mut egui::Ui, idx: usize) {
        // Panels are Ui-hosted since egui 0.35; everything else in this
        // method still wants a Context.
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

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
        cobolt_forms::paint::set_surface_theme(
            ctx,
            self.designers[idx].1.active_surface_theme.clone(),
        );

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

        // ── Apply glass visuals to this preview's OWN Ui subtree ──────────────
        // The frosted-glass look must never be written context-wide
        // (`ctx.set_visuals`): the Context style is shared by every viewport,
        // so a context write left the IDE shell painting with preview glass —
        // visibly stripping a neumorphic theme's chrome — for as long as a
        // preview stayed open, and the shell could only fight back by
        // re-applying its theme every frame. Scoped to this Ui, the preview
        // keeps its glass and the rest of the IDE is structurally untouchable.
        // Start from the current IDE glass visuals so we inherit the base
        // colour scheme, then layer in the preview-specific transparency.
        let mut visuals = ctx.global_style().visuals.clone();
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
        // CornerRadius
        let rnd = egui::CornerRadius::same(8);
        visuals.widgets.noninteractive.corner_radius = rnd;
        visuals.widgets.inactive.corner_radius = rnd;
        visuals.widgets.hovered.corner_radius = rnd;
        visuals.widgets.active.corner_radius = rnd;
        // Text
        visuals.override_text_color = Some(Color32::from_rgb(230, 235, 255));
        // Window / panel background — transparent so the OS shows through
        visuals.panel_fill = Color32::TRANSPARENT;
        visuals.window_fill = Color32::TRANSPARENT;
        visuals.extreme_bg_color = Color32::from_rgba_premultiplied(20, 20, 40, 180);
        let mut preview_style = (*ctx.global_style()).clone();
        preview_style.visuals = visuals;
        panel_ui.set_style(preview_style);

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
        // A rail shown collapsed is DRAWN at the collapsed width — the same
        // rule the designer canvas and the running shell follow. Without it the
        // preview painted the rail at its DESIGNED width with collapsed,
        // icon-only rows: a bar that looked open and behaved closed, with the
        // breadcrumb (which positions from the rail width) landing inside it.
        let controls = match cobolt_forms::breadcrumb::shell_side_menu_in(&controls) {
            Some(side) => {
                let collapsed = matches!(
                    values_snap.get(&side.id).map(String::as_str),
                    Some("1") | Some("true")
                ) || (!values_snap.contains_key(&side.id) && side.side_menu_collapsed());
                cobolt_forms::sidebar::rail_view(&controls, side, collapsed)
            }
            None => controls,
        };
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
                paint: true,
                color_hex: d.form.background_color.clone(),
                transparency: d.form.transparency.min(100) as u8,
                gradient_enabled: d.form.background_gradient_enabled,
                gradient_start_hex: d.form.background_gradient_start_color.clone(),
                gradient_end_hex: d.form.background_gradient_end_color.clone(),
                gradient_direction: d.form.background_gradient_direction.clone(),
                image,
                image_mode: d.form.bg_image_mode,
                use_theme_background: d.form.use_theme_background,
                // Preview panel: the backdrop stays pinned to the FORM, so the
                // designed extent is still visible while editing. Only a real
                // window (run form, compiled binary) stretches it.
                window_size: None,
                // The IDE owns its own ambient visuals and renders the preview
                // into them, so the ambient panel fill really is what the form
                // sits on here — the one surface where the fallback is right.
                behind_fill: None,
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

        // 049 — the label the shell's breadcrumb opens on, and the content
        // pane's backdrop, which is the strip's background.
        let crumb_label = cobolt_forms::breadcrumb::design_label(&self.designers[idx].1.form);
        // The preview runs the rail LIVE, so the strip follows the state the
        // preview is actually in — the live `Collapsed`, not the designed one —
        // or the strip's arrow and its left edge both lie.
        let crumb_side = cobolt_forms::breadcrumb::shell_side_menu_in(&controls).cloned();
        let crumb_collapsed = crumb_side.as_ref().is_some_and(|side| {
            matches!(
                self.designers[idx].1.preview_state.get(&side.id).map(String::as_str),
                Some("1") | Some("true")
            ) || (!self.designers[idx].1.preview_state.contains_key(&side.id)
                && side.side_menu_collapsed())
        });
        let crumb_bg = {
            let f = &self.designers[idx].1.form;
            match &crumb_side {
                Some(side) => cobolt_forms::breadcrumb::strip_background_for(
                    side,
                    &f.background_color,
                    f.transparency.min(100) as u8,
                ),
                None => cobolt_forms::breadcrumb::strip_background(
                    &f.background_color,
                    f.transparency.min(100) as u8,
                ),
            }
        };
        // The strip is chrome painted ON the form's background and UNDER its
        // controls: a control the developer put over the band paints on top of
        // it, exactly as it does on the designer canvas. The frame is not a
        // container — the control is nobody's child, it merely overlaps.
        let crumb_ctx = panel_ui.ctx().clone();
        let paint_crumb = |painter: &egui::Painter, form_rect: egui::Rect| {
            let Some(side) = &crumb_side else { return };
            let rail_w = cobolt_forms::sidebar::shown_width(side, crumb_collapsed);
            let Some(rect) = cobolt_forms::breadcrumb::strip_rect(
                side,
                rail_w,
                form_rect.width(),
                form_rect.min,
            ) else {
                return;
            };
            cobolt_forms::breadcrumb::draw_static_strip(
                painter,
                &crumb_ctx,
                side,
                &crumb_label,
                rect,
                crumb_bg,
                cobolt_forms::breadcrumb::DesignView {
                    collapsed: crumb_collapsed,
                    toggle_hovered: false,
                },
            );
        };

        let mut updates: Vec<(String, String, String)> = Vec::new();
        let mut toolbar_presses: Vec<(String, String, String)> = Vec::new();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(panel_ui, |ui| {
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
                        // 049 — the shell's breadcrumb, static (one segment, the
                        // form itself), painted in `render_form`'s own chrome
                        // slot: on the backdrop, under the controls.
                        let out = cobolt_forms::render::render_form_with_chrome(
                            ui,
                            &input,
                            Some(&paint_crumb),
                        );
                        updates = out.prop_updates;
                        // Preview has no COBOL event loop; UI events are
                        // discarded — but a toolbar button's PLATFORM action
                        // needs no interpreter, so those are carried out below.
                        toolbar_presses = out.toolbar_actions;
                    });
            });

        // ── Toolbar buttons whose action is the platform's work ───────────────
        //
        // Preview is where a toolbar gets BUILT, so its buttons have to be
        // pressable here and not only under Run Form: print a document, launch
        // an application, open a terminal, use the clipboard. None of those need
        // an interpreter, which is why Preview can honour them at all — the three
        // COBOL actions (`event`, `procedure:`, `open-modal:`) do, and are left
        // to the running form.
        for (ctrl_id, button_id, action) in toolbar_presses {
            let parsed = cobolt_forms::toolbar::ToolbarAction::parse(&action);
            match preview_press(&parsed) {
                // Say so. A press that produces nothing AND says nothing is the
                // thing this whole area exists to stop: the developer cannot tell
                // "Preview does not run COBOL" from "my action is broken".
                PreviewPress::LeaveToTheForm => {
                    self.output.push_status(format!(
                        "Preview: {ctrl_id}/{button_id} — `{}` is the form's own COBOL, \
                         which Preview does not run. Use Run Form.",
                        parsed.to_action_string()
                    ));
                    continue;
                }
                PreviewPress::NeedsRunForm => {
                    self.output.push_status(format!(
                        "Preview: {ctrl_id}/{button_id} — `{}` captures the form's own \
                         window, which only exists under Run Form",
                        parsed.verb()
                    ));
                    continue;
                }
                PreviewPress::Perform => {}
            }
            // Copy/Cut/Paste act on whichever control has keyboard focus. egui
            // reports that as a widget id, and a control's TextEdit is built with
            // `Id::new(("rt_ctrl", <control id>))` — so the focused control is
            // found by matching that back. Same rule as the running host.
            let focused = ctx.memory(|m| m.focused()).and_then(|focus| {
                controls.iter().find_map(|c| {
                    (egui::Id::new(("rt_ctrl", c.id.as_str())) == focus).then(|| {
                        let text = self.designers[idx]
                            .1
                            .preview_state
                            .get(&c.id)
                            .cloned()
                            .unwrap_or_default();
                        (c.id.clone(), text)
                    })
                })
            });
            let focused_ref = focused
                .as_ref()
                .map(|(id, text)| cobolt_forms::toolbar_actions::Focused {
                    control_id: id.as_str(),
                    text: text.clone(),
                });
            let (outcome, new_text) = self
                .preview_toolbar_runner
                .perform(ctx, &parsed, focused_ref);
            // Nothing fails in silence: the Output pane carries the reason, the
            // same way the running host logs it.
            self.output
                .push_status(format!("Preview: {}", outcome.message()));
            // A Cut or a Paste changed the focused field — feed it back through
            // the update path a keystroke would have taken.
            if let (Some(text), Some((target, _))) = (new_text, focused) {
                updates.push((target, "Text".to_owned(), text));
            }
        }

        // Apply the engine's value updates back to the preview value map so the
        // next frame renders the edited state (text typed, slider moved, combo
        // selected, checkbox toggled).
        for (id, key, val) in updates {
            let expected = controls
                .iter()
                .find(|c| c.id == id)
                .map(|c| preview_value_key(&c.control_type))
                .unwrap_or("Caption");
            if preview_accepts_update(expected, &key) {
                self.designers[idx].1.preview_state.insert(id, val);
            } else if preview_keeps_extra_update(&key) {
                // A second value from the same control — kept under `id::Prop`,
                // which `PreviewState::live` merges back onto the control.
                self.designers[idx]
                    .1
                    .preview_state
                    .insert(format!("{id}::{key}"), val);
            }
        }

        // Reactive frame scheduling, the same rule as the shared form host:
        // full frame rate (16 ms) while any preview animation is playing, a
        // slow liveness heartbeat once everything is still, so an idle
        // preview never spins a core. The busy check must run down here, not
        // rely on the tick above alone: the first frame after OnFormLoad
        // seeding has dt == 0, so the tick requests nothing and a flat slow
        // heartbeat would hold the opening animation frame back visibly.
        let busy = self.designers[idx]
            .1
            .preview_anim_states
            .values()
            .any(|s| s.playing);
        let ms = if busy { 16 } else { 100 };
        ctx.request_repaint_after(std::time::Duration::from_millis(ms));
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
        let form_running = !self.external_runs.is_empty();
        // Sample only while a form runs. The run is an external process, so the
        // IDE has no view of its interpreter queue — `processing` is false and
        // the inspector reads growth against wall-clock instead.
        if form_running {
            // One CPU timeline per open external rcrun run-form process, keyed
            // by pid so the inspector can label + retire a series per form.
            let tracked: Vec<(u32, String)> = self
                .external_runs
                .iter()
                .map(|run| (run.pid(), run.form_name.clone()))
                .collect();
            self.inspector.maybe_sample(false, &tracked);
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        // The inspector lives in its own always-on-top OS window (a viewport, like
        // the running form) so the charts stay visible while you interact with the
        // app and can correlate a spike with what you just did.
        let vp_id = ViewportId::from_hash_of("run_form_inspector");
        // Apply the default size ONLY on the first frame after opening; on later
        // frames we omit `inner_size` so egui never re-commands a size and the
        // user's own window resizes are preserved.
        // Always-on-top, except while a dialog needs an answer — see
        // `show_debugger_viewport` for why the level is set explicitly.
        let level = if self.blocking_modal_open() {
            egui::WindowLevel::Normal
        } else {
            egui::WindowLevel::AlwaysOnTop
        };
        let mut builder = ViewportBuilder::default()
            .with_title("📊 Run-Form Inspector")
            .with_resizable(true)
            .with_window_level(level);
        if !self.inspector_sized {
            let sh = ctx.content_rect();
            builder = builder.with_inner_size([
                (sh.width() / 3.0).clamp(560.0, 900.0),
                (sh.height() / 6.0).clamp(200.0, 320.0),
            ]);
            self.inspector_sized = true;
        }
        ctx.show_viewport_immediate(vp_id, builder, |vp_ctx, _class| {
            self.doc_shots.poll(vp_ctx, self.debug.doc_screenshots);
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

// Host C — the in-IDE running-form viewport (`FormRuntime` +
// `show_running_form_window`) — was retired by spec 042 R4: Run Form executes
// in the external `rcrun run-form` process hosting the shared
// `cobolt-form-host` window, and the in-process copy had been unreachable
// since that landed (nothing ever constructed a `FormRuntime`).

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
                    self.doc_shots.poll(vp_ctx, self.debug.doc_screenshots);
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

    fn show_indexed_grid_window(&mut self, panel_ui: &mut egui::Ui, gi: usize, tr: &Tr) {
        // Panels are Ui-hosted since egui 0.35; everything else in this
        // method still wants a Context.
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        if gi >= self.indexed_grids.len() {
            return;
        }
        let theme = self.current_theme();
        apply_opaque_viewport_theme(ctx, theme);
        let panel_frame =
            crate::theme::glass_panel_frame(ctx.global_style().visuals.panel_fill, theme);
        let mut toolbar_action = GridAction::None;
        let mut status_msg: Option<String> = None;
        egui::CentralPanel::default()
            .frame(panel_frame)
            .show(panel_ui, |ui| {
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
    fn show_knowledge_folder_dialog(&mut self, ctx: &Context, tr: &Tr) {
        let Some(parent) = self.knowledge_folder_parent.clone() else {
            return;
        };
        let mut cancel = false;
        let mut create = false;
        egui::Window::new("New Knowledge Base subfolder")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("Create inside {}", parent.display()));
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.knowledge_folder_name)
                        .hint_text("Folder name")
                        .desired_width(320.0),
                );
                if response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                    create = true;
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr.btn_cancel).clicked() {
                        cancel = true;
                    }
                    if ui
                        .add_enabled(
                            !self.knowledge_folder_name.trim().is_empty(),
                            egui::Button::new("Create"),
                        )
                        .clicked()
                    {
                        create = true;
                    }
                });
            });

        if cancel {
            self.knowledge_folder_parent = None;
            self.knowledge_folder_name.clear();
        } else if create {
            let Some(root) = self.project_dir() else {
                self.knowledge_folder_parent = None;
                return;
            };
            match cobolt_agents::project_knowledge::create_knowledge_subfolder(
                &root,
                &parent,
                &self.knowledge_folder_name,
            ) {
                Ok(path) => {
                    self.output
                        .push_status(format!("Created Knowledge Base folder {}", path.display()));
                    self.knowledge_folder_parent = None;
                    self.knowledge_folder_name.clear();
                }
                Err(error) => {
                    self.set_alert_error(format!(
                        "Could not create Knowledge Base folder.\n\n{error}"
                    ))
                }
            }
        }
    }

    fn show_knowledge_folder_delete_confirm(&mut self, ctx: &Context, tr: &Tr) {
        let Some(folder) = self.pending_knowledge_folder_delete.clone() else {
            return;
        };
        let mut cancel = false;
        let mut confirm = false;
        egui::Window::new("Delete Knowledge Base folder")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Delete '{}' and every document and subfolder inside it?",
                    folder.display()
                ));
                ui.label(
                    egui::RichText::new(
                        "This action removes the files from disk and cannot be undone.",
                    )
                    .color(Color32::from_rgb(230, 150, 120)),
                );
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
            self.pending_knowledge_folder_delete = None;
        } else if confirm {
            self.pending_knowledge_folder_delete = None;
            let Some(root) = self.project_dir() else {
                return;
            };
            let absolute = root.join(&folder);
            match cobolt_agents::project_knowledge::delete_knowledge_subfolder(&root, &folder) {
                Ok(deleted) => {
                    self.editor
                        .tabs
                        .retain(|tab| !tab.path.starts_with(&absolute));
                    if self.editor.active >= self.editor.tabs.len() && !self.editor.tabs.is_empty()
                    {
                        self.editor.active = self.editor.tabs.len() - 1;
                    }
                    self.sync_project_documentation_membership(&root);
                    self.output.push_status(format!(
                        "Deleted Knowledge Base folder {}",
                        deleted.display()
                    ));
                }
                Err(error) => {
                    self.set_alert_error(format!(
                        "Could not delete Knowledge Base folder.\n\n{error}"
                    ))
                }
            }
        }
    }

    /// Render the Grace target-disambiguation modal when a workflow is paused
    /// awaiting a target pick, and feed the choice back to the worker (spec 034).
    fn show_target_picker(&mut self, ctx: &Context, tr: &Tr) {
        let Some(req) = self
            .grace_session
            .as_ref()
            .and_then(|s| s.pending_select())
            .cloned()
        else {
            return;
        };
        let Some(root) = self.project_dir() else {
            return;
        };
        if let Some(outcome) = self.target_picker.show(ctx, &req, &root, tr) {
            if let Some(sess) = self.grace_session.as_mut() {
                sess.respond_select(outcome);
            }
        }
    }

    /// Localised message for a `project_fs` error (spec 033, R19).
    fn folder_err_message(&self, tr: &Tr, err: &crate::project_fs::FolderOpError) -> String {
        use crate::project_fs::FolderOpError as E;
        match err {
            E::EmptyName | E::DottedName | E::NotSingleComponent | E::IllegalChar => {
                tr.folder_err_invalid_name
            }
            E::IsCategoryRoot => tr.folder_err_is_category_root,
            E::Collision(_) => tr.folder_err_exists,
            E::SelfDescendant => tr.folder_err_self_descendant,
            _ => tr.folder_err_generic,
        }
        .to_string()
    }

    /// New-folder dialog for any category (spec 033, R1–R3).
    fn show_folder_create_dialog(&mut self, ctx: &Context, tr: &Tr) {
        let Some(state) = self.folder_create.as_ref() else {
            return;
        };
        let parent_rel = state.parent_rel.clone();
        let category_root = state.category_root.clone();
        let mut cancel = false;
        let mut create = false;
        egui::Window::new(tr.dlg_new_folder_title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("{} {}", tr.dlg_folder_create_in, parent_rel.display()));
                let name = &mut self.folder_create.as_mut().unwrap().name;
                let response = ui.add(
                    egui::TextEdit::singleline(name)
                        .hint_text(tr.dlg_folder_name_hint)
                        .desired_width(320.0),
                );
                if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    create = true;
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr.btn_cancel).clicked() {
                        cancel = true;
                    }
                    if ui
                        .add_enabled(
                            !name.trim().is_empty(),
                            egui::Button::new(tr.btn_create),
                        )
                        .clicked()
                    {
                        create = true;
                    }
                });
            });

        if cancel {
            self.folder_create = None;
        } else if create {
            let name = self.folder_create.as_ref().unwrap().name.clone();
            let Some(root) = self.project_dir() else {
                self.folder_create = None;
                return;
            };
            let _ = category_root;
            match crate::project_fs::create_folder(&root, &parent_rel, &name) {
                Ok(rel) => {
                    self.output
                        .push_status(format!("Created folder {}", rel.display()));
                    self.folder_create = None;
                }
                Err(err) => {
                    let message = self.folder_err_message(tr, &err);
                    self.set_alert_error(message);
                }
            }
        }
    }

    /// Rename-folder dialog for any category (spec 033, R4).
    fn show_folder_rename_dialog(&mut self, ctx: &Context, tr: &Tr) {
        let Some(state) = self.folder_rename.as_ref() else {
            return;
        };
        let folder_rel = state.folder_rel.clone();
        let category_root = PathBuf::from(state.category_root.clone());
        let mut cancel = false;
        let mut rename = false;
        egui::Window::new(tr.dlg_rename_folder_title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(folder_rel.display().to_string()).small());
                let name = &mut self.folder_rename.as_mut().unwrap().name;
                let response = ui.add(
                    egui::TextEdit::singleline(name)
                        .hint_text(tr.dlg_folder_name_hint)
                        .desired_width(320.0),
                );
                if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    rename = true;
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr.btn_cancel).clicked() {
                        cancel = true;
                    }
                    if ui
                        .add_enabled(
                            !name.trim().is_empty(),
                            egui::Button::new(tr.btn_rename),
                        )
                        .clicked()
                    {
                        rename = true;
                    }
                });
            });

        if cancel {
            self.folder_rename = None;
        } else if rename {
            let new_name = self.folder_rename.as_ref().unwrap().name.clone();
            let Some(root) = self.project_dir() else {
                self.folder_rename = None;
                return;
            };
            match crate::project_fs::rename_folder(&root, &folder_rel, &new_name, &category_root) {
                Ok(new_rel) => {
                    let old = crate::project_fs::rel_string(&folder_rel);
                    let new = crate::project_fs::rel_string(&new_rel);
                    if let Some(proj) = &mut self.cobolt_project {
                        proj.rename_prefix(&old, &new);
                    }
                    self.rewrite_open_paths(&root, &old, &new);
                    self.do_save_project();
                    self.output
                        .push_status(format!("Renamed folder to {new}"));
                    self.folder_rename = None;
                }
                Err(err) => {
                    let message = self.folder_err_message(tr, &err);
                    self.set_alert_error(message);
                }
            }
        }
    }

    /// Recursive folder-delete confirmation for any category (spec 033, R5, R6).
    fn show_folder_delete_confirm(&mut self, ctx: &Context, tr: &Tr) {
        let Some(state) = self.folder_delete.as_ref() else {
            return;
        };
        let folder_rel = state.folder_rel.clone();
        let category_root = PathBuf::from(state.category_root.clone());
        let mut cancel = false;
        let mut confirm = false;
        egui::Window::new(tr.dlg_delete_folder_title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(tr.dlg_delete_folder_body);
                ui.add_space(4.0);
                ui.label(egui::RichText::new(folder_rel.display().to_string()).small());
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(tr.dlg_delete_folder_warning)
                        .color(Color32::from_rgb(230, 150, 120)),
                );
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
            self.folder_delete = None;
        } else if confirm {
            self.folder_delete = None;
            let Some(root) = self.project_dir() else {
                return;
            };
            match crate::project_fs::delete_folder(&root, &folder_rel, &category_root) {
                Ok(removed) => {
                    let dir = crate::project_fs::rel_string(&removed);
                    // Drop tracked members under the folder and close their views.
                    let dropped = self
                        .cobolt_project
                        .as_mut()
                        .map(|proj| proj.drain_under(&dir))
                        .unwrap_or_default();
                    let absolute = root.join(&removed);
                    self.close_views_under(&absolute);
                    let _ = dropped;
                    // Knowledge-Base deletes also refresh documentation membership.
                    if category_root == Path::new(crate::project_model::Category::Documentation.root_subdir())
                    {
                        self.sync_project_documentation_membership(&root);
                    }
                    self.do_save_project();
                    self.output
                        .push_status(format!("Deleted folder {dir}"));
                }
                Err(err) => {
                    let message = self.folder_err_message(tr, &err);
                    self.set_alert_error(message);
                }
            }
        }
    }

    /// Move a tracked file into another folder via drag-and-drop (spec 033, R9).
    fn do_move_tracked_file(&mut self, src_rel: String, dest_dir_rel: String) {
        let Some(root) = self.project_dir() else {
            return;
        };
        match crate::project_fs::move_path(
            &root,
            Path::new(&src_rel),
            Path::new(&dest_dir_rel),
        ) {
            Ok(new_rel) => {
                let new = crate::project_fs::rel_string(&new_rel);
                if let Some(proj) = &mut self.cobolt_project {
                    proj.move_entry(&src_rel, &new);
                }
                self.rewrite_open_paths(&root, &src_rel, &new);
                self.do_save_project();
                self.output.push_status(format!("Moved {src_rel} → {new}"));
            }
            Err(err) => {
                let tr = self.lang.tr();
                let message = self.folder_err_message(&tr, &err);
                self.set_alert_error(message);
            }
        }
    }

    /// Import files dropped from the OS file manager into a folder (spec 033,
    /// R10, R14, R21). Copies each file in and tracks a project-relative path.
    fn do_import_os_files(&mut self, paths: Vec<PathBuf>, dest_dir_rel: String) {
        use crate::project_model::{Category, FileKind};
        let Some(root) = self.project_dir() else {
            return;
        };
        let dest_cat = Category::from_root_component(&dest_dir_rel);
        for path in paths {
            if !path.is_file() {
                continue;
            }
            let kind = FileKind::from_path(&path.to_string_lossy());
            // Reject a file whose kind does not belong in the destination
            // category (R14).
            if let Some(dest_cat) = dest_cat {
                if Category::of_kind(kind) != dest_cat {
                    let tr = self.lang.tr();
                    self.set_alert_error(tr.folder_err_incompatible_kind);
                    continue;
                }
            }
            self.import_file_into_folder(path, &root, &dest_dir_rel);
        }
    }

    /// Copy one OS file into the project-relative folder `dest_dir_rel` and track
    /// it under the matching category with a **relative** path (spec 033, R21).
    fn import_file_into_folder(&mut self, src: PathBuf, root: &Path, dest_dir_rel: &str) {
        let Some(fname) = src.file_name().and_then(|n| n.to_str()) else {
            return;
        };
        let dest_dir = root.join(dest_dir_rel);
        let _ = std::fs::create_dir_all(&dest_dir);
        let dest = dest_dir.join(fname);
        if dest.exists() {
            let tr = self.lang.tr();
            self.set_alert_error(tr.folder_err_exists);
            return;
        }
        if let Err(e) = std::fs::copy(&src, &dest) {
            self.output.push_status(format!("Could not import {fname}: {e}"));
            return;
        }
        let rel = format!("{}/{fname}", dest_dir_rel.trim_end_matches('/'));
        if let Some(proj) = &mut self.cobolt_project {
            proj.add_file_to(&rel, crate::project_model::Category::of_path(&rel));
        }
        self.do_save_project();
        self.output.push_status(format!("Imported {rel}"));
    }

    /// Rewrite open editor tabs, designer and indexed views whose path lies under
    /// the moved/renamed folder prefix `old` → `new` (spec 033, R4, Q3).
    fn rewrite_open_paths(&mut self, root: &Path, old: &str, new: &str) {
        let old_abs = root.join(old);
        let new_abs = root.join(new);
        let remap = |p: &Path| -> Option<PathBuf> {
            if p == old_abs {
                Some(new_abs.clone())
            } else if let Ok(rest) = p.strip_prefix(&old_abs) {
                Some(new_abs.join(rest))
            } else {
                None
            }
        };
        for tab in self.editor.tabs.iter_mut() {
            if let Some(np) = remap(&tab.path) {
                tab.path = np;
            }
        }
        for (p, _) in self.designers.iter_mut() {
            if let Some(np) = remap(p) {
                *p = np;
            }
        }
        for (p, _) in self.indexed_grids.iter_mut() {
            if let Some(np) = remap(p) {
                *p = np;
            }
        }
        if let Some(st) = &mut self.indexed_inspect {
            if let Some(np) = remap(&st.path) {
                st.path = np;
            }
        }
    }

    /// Close editor tabs, designer and indexed views bound to a file under the
    /// deleted directory `dir_abs` (spec 033, R6).
    fn close_views_under(&mut self, dir_abs: &Path) {
        self.editor.tabs.retain(|tab| !tab.path.starts_with(dir_abs));
        if self.editor.active >= self.editor.tabs.len() && !self.editor.tabs.is_empty() {
            self.editor.active = self.editor.tabs.len() - 1;
        }
        self.designers.retain(|(p, _)| !p.starts_with(dir_abs));
        self.indexed_grids.retain(|(p, _)| !p.starts_with(dir_abs));
        if let Some(st) = &self.indexed_inspect {
            if st.path.starts_with(dir_abs) {
                self.indexed_inspect = None;
            }
        }
    }

    fn show_form_delete_confirm(&mut self, ctx: &Context, tr: &Tr) {
        let Some(path) = self.pending_form_delete.clone() else {
            return;
        };
        let mut cancel = false;
        let mut confirm = false;

        egui::Window::new(tr.dlg_delete_form_title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(tr.dlg_delete_form_body);
                ui.add_space(4.0);
                ui.label(egui::RichText::new(path.display().to_string()).small());
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
            self.pending_form_delete = None;
        }
        if confirm {
            self.pending_form_delete = None;
            self.delete_form_path(path);
        }
    }

    fn delete_form_path(&mut self, path: PathBuf) {
        let rel = self.project_dir().and_then(|dir| relative_to(&path, &dir));
        self.designers.retain(|(open_path, _)| open_path != &path);
        if let Some(rel) = rel {
            self.do_remove_file_from_project(rel);
        }
        match std::fs::remove_file(&path) {
            Ok(()) => {
                self.forms_list.refresh();
                self.output
                    .push_status(format!("Deleted form {}", path.display()));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.forms_list.refresh();
                self.output
                    .push_status(format!("Form file was already missing: {}", path.display()));
            }
            Err(e) => {
                self.output
                    .push_status(format!("Could not delete form {}: {e}", path.display()));
            }
        }
        // Spec 037 R3 — deleting the main form auto-assigns the first
        // remaining form (with a status notice).
        self.apply_main_form_invariant();
    }

    fn show_generated_delete_confirm(&mut self, ctx: &Context, tr: &Tr) {
        let Some(path) = self.pending_generated_delete.clone() else {
            return;
        };
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("generated COBOL");
        let mut cancel = false;
        let mut confirm = false;

        egui::Window::new("Delete generated COBOL")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Delete generated COBOL '{}' from the project and remove its .cbl file from disk?",
                    name
                ));
                ui.add_space(4.0);
                ui.label(egui::RichText::new(path.display().to_string()).small());
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
            self.pending_generated_delete = None;
        }
        if confirm {
            self.pending_generated_delete = None;
            self.delete_generated_path(path);
        }
    }

    fn delete_generated_path(&mut self, path: PathBuf) {
        let rel = self.project_dir().and_then(|dir| relative_to(&path, &dir));
        if let Some(rel) = rel {
            self.do_remove_file_from_project(rel);
        }
        if self.pending_open_in_editor.as_ref() == Some(&path) {
            self.pending_open_in_editor = None;
        }
        self.editor.tabs.retain(|tab| tab.path != path);
        if self.editor.active >= self.editor.tabs.len() && !self.editor.tabs.is_empty() {
            self.editor.active = self.editor.tabs.len() - 1;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => {
                self.output
                    .push_status(format!("Deleted generated COBOL {}", path.display()));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.output.push_status(format!(
                    "Generated COBOL file was already missing: {}",
                    path.display()
                ));
            }
            Err(e) => {
                self.output.push_status(format!(
                    "Could not delete generated COBOL {}: {e}",
                    path.display()
                ));
            }
        }
    }

    fn show_asset_delete_confirm(&mut self, ctx: &Context, tr: &Tr) {
        let Some(path) = self.pending_asset_delete.clone() else {
            return;
        };
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("asset");
        let mut cancel = false;
        let mut confirm = false;

        egui::Window::new("Delete asset")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Delete asset '{}' from the project and remove it from disk?",
                    name
                ));
                ui.add_space(4.0);
                ui.label(egui::RichText::new(path.display().to_string()).small());
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
            self.pending_asset_delete = None;
        }
        if confirm {
            self.pending_asset_delete = None;
            self.delete_asset_path(path);
        }
    }

    fn delete_asset_path(&mut self, path: PathBuf) {
        let rel = self.project_dir().and_then(|dir| relative_to(&path, &dir));
        if let Some(rel) = rel {
            self.do_remove_file_from_project(rel);
        }
        if self.asset_preview.as_ref().map(|p| p.path.as_path()) == Some(path.as_path()) {
            self.asset_preview = None;
        }
        self.editor.tabs.retain(|tab| tab.path != path);
        if self.editor.active >= self.editor.tabs.len() && !self.editor.tabs.is_empty() {
            self.editor.active = self.editor.tabs.len() - 1;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => self
                .output
                .push_status(format!("Deleted asset {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => self.output.push_status(format!(
                "Asset file was already missing: {}",
                path.display()
            )),
            Err(e) => self
                .output
                .push_status(format!("Could not delete asset {}: {e}", path.display())),
        }
    }

    fn show_user_control_delete_confirm(&mut self, ctx: &Context, tr: &Tr) {
        let Some(name) = self.pending_user_control_delete.clone() else {
            return;
        };
        let mut cancel = false;
        let mut confirm = false;
        let message = tr.uc_delete_confirm.replace("{name}", &name);

        let win_id = egui::Id::new("user_control_delete_confirm");
        raise_modal_layer(ctx, win_id);
        egui::Window::new(tr.uc_delete)
            .id(win_id)
            .order(egui::Order::Foreground) // above every ordinary window
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

        let win_id = egui::Id::new("indexed_delete_confirm");
        raise_modal_layer(ctx, win_id);
        egui::Window::new("Confirm removal")
            .id(win_id)
            .order(egui::Order::Foreground) // above every ordinary window
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

    fn show_designer_window(&mut self, panel_ui: &mut egui::Ui, idx: usize, tr: &Tr) {
        // Panels are Ui-hosted since egui 0.35; everything else in this
        // method still wants a Context.
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        if idx >= self.designers.len() {
            return;
        }

        // Re-apply the theme to this designer viewport every frame: the
        // designer window needs the opaque panel fills (no OS bleed-through),
        // and a theme switch must land immediately.
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
            let win_id = egui::Id::new(("designer_close_confirm", idx));
            raise_modal_layer(ctx, win_id);
            egui::Window::new(&title)
                .id(win_id)
                .order(egui::Order::Foreground) // above every ordinary window
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

        // Collapsible left sidebar (spec 033): forms list + toolbox share one
        // panel. We use egui 0.35's NATIVE drawer, `Panel::show_switched`, which
        // animates between a thin collapsed panel (`dl_rail_*`, a FIXED-width icon
        // rail) and the resizable expanded panel (`dl_*`). egui owns the collapse
        // state via `&mut is_expanded` and persists each panel's size per id, so
        // there is no hand-rolled id-swap fighting egui's own `PanelState`.
        // Neither width comes from available/max space, so the sidebar cannot
        // self-inflate; the expanded width is remembered by egui and seeded from
        // `toolbox_width`.
        use crate::panels::designer::{clamp_toolbox_width, TOOLBOX_MIN_W, TOOLBOX_RAIL_W};
        let mut tb_expanded = !self.designers[idx].1.toolbox_collapsed;
        let tb_width = self.designers[idx].1.toolbox_width;
        let tb_max_w = (ctx.content_rect().width() * 0.5).max(320.0);

        let tb_collapsed_panel = egui::Panel::left(format!("dl_rail_{idx}"))
            .resizable(false)
            .exact_size(TOOLBOX_RAIL_W);
        let tb_expanded_panel = egui::Panel::left(format!("dl_{idx}"))
            .resizable(true)
            .default_size(tb_width)
            .min_size(TOOLBOX_MIN_W)
            .max_size(tb_max_w);

        let left_resp = egui::Panel::show_switched(
            panel_ui,
            &mut tb_expanded,
            tb_collapsed_panel,
            tb_expanded_panel,
            |ui, expanded| {
                // The forms list needs real width; at the icon-rail size it's
                // hidden and returns on expand.
                let forms_action = if expanded {
                    let a = self.forms_list.show(ui, &open_path_refs, tr);
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(2.0);
                    a
                } else {
                    None
                };
                let tb = self.designers[idx]
                    .1
                    .toolbox
                    .show(ui, tr, &user_controls, !expanded);
                (forms_action, tb)
            },
        );
        let (forms_list_action, toolbox_action) = left_resp.inner;

        // Seed `toolbox_width` from the panel's OWN width when expanded (never
        // from available space). egui's per-id `PanelState` is the authoritative
        // store and overrides this seed once present, so a transient slide-frame
        // value here can't affect the displayed width.
        if tb_expanded {
            self.designers[idx].1.toolbox_width =
                clamp_toolbox_width(left_resp.response.rect.width(), TOOLBOX_MIN_W, tb_max_w);
        }
        // The chevron buttons (in the expanded header / the rail) flip the state;
        // egui's drag-to-collapse already updated `tb_expanded` in place.
        if toolbox_action.toggle_collapse {
            tb_expanded = !tb_expanded;
        }
        self.designers[idx].1.toolbox_collapsed = !tb_expanded;

        if let Some(action) = forms_list_action {
            match action {
                FormsListAction::Open(path) => {
                    self.load_form_from_path(path);
                    return; // re-render next frame with the new designer added
                }
                FormsListAction::Delete(path) => {
                    self.pending_form_delete = Some(path);
                }
            }
        }

        // ── Unified 50-px icon toolbar (replaces both old toolbars) ──────────
        use crate::panels::designer::{draw_icon_toolbar, DesignerToolbarAction};
        // Transparent frame + no separator line; `draw_icon_toolbar` fills the
        // whole reserved height itself with the toolbox colour (see designer.rs).
        egui::Panel::top(format!("dtb_{idx}"))
            .exact_size(50.0)
            .frame(egui::Frame::NONE)
            .show_separator_line(false)
            .show(panel_ui, |ui| {
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
                    .external_runs
                    .iter()
                    .any(|run| run.form_path == form_path);

                // Icons (left) + language selector (right) on a SINGLE centred row.
                // They must share one row: two stacked rows (icon row + a separate
                // selector row) make the content ~75px tall, which egui uses as the
                // panel height — overriding `exact_height(50)`.
                let mut action = DesignerToolbarAction::None;
                // Transient checkmark on the Save button after a save of THIS form.
                let saved_flash = matches!(
                    &self.save_flash,
                    Some((p, until)) if *p == form_path && std::time::Instant::now() < *until
                );
                // Building the form's binary, or that binary still running —
                // the Run button reads as engaged for the whole stretch.
                let run_busy = self.pending_build_then_run.as_deref() == Some(form_path.as_path())
                    || self.built_runs.iter().any(|r| r.form_path == form_path);
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
                        run_busy,
                        fp_active,
                        self.show_inspector,
                        self.debug_active,
                        saved_flash,
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("designer_lang_selector")
                            .selected_text(self.lang.native_name())
                            .width(130.0)
                            .show_ui(ui, |ui| {
                                for &l in Language::ALL {
                                    if crate::flags::language_row(ui, l, self.lang == l).clicked() {
                                        self.lang = l;
                                    }
                                }
                            });
                        crate::flags::flag_widget(ui, self.lang);
                        ui.add_space(4.0);
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
                            egui::Popup::close_all(ctx);
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
                        // If Run started a build, the build modal belongs to
                        // THIS designer window, not the IDE main window — the
                        // operator is looking here. Exactly one surface shows
                        // it (the main-window call is gated on this).
                        if self.pending_build_then_run.is_some() {
                            self.build_modal_host = Some(self.designers[idx].0.clone());
                        }
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
        let half_win = (ctx.content_rect().width() * 0.5).max(320.0);
        // 10px right inner margin so the pane's content keeps a small gap from the
        // window border instead of butting against it.
        let props_frame =
            egui::Frame::side_top_panel(&ctx.global_style()).inner_margin(egui::Margin {
                left: 6,
                right: 10,
                top: 6,
                bottom: 6,
            });
        // Properties DRAWER (spec 033), rebuilt to the operator's target layout:
        //   form | [ ◀/▶ strip ] | [ properties content ]
        // The collapse control lives on its OWN fixed-width strip that pushes the
        // properties content to its right. The content is an ordinary resizable
        // egui right panel: egui persists the user's dragged width per its id, so
        // it opens at a CONSTANT default and only the user's drag changes it. We
        // never read the rendered width back into `default_size`, so there is no
        // self-inflation feedback loop (the previous bug: the pane grew every
        // frame and could not be dragged smaller). When collapsed, only the strip
        // remains and the form reclaims the width.
        use crate::panels::designer::{PROPS_DEFAULT_W, PROPS_MIN_W, PROPS_TAB_W};
        let props_hidden = self.designers[idx].1.props_hidden;

        // The colour picker's fixed grid is the ACTIVE theme's palette, read
        // from the context — so this pane must publish its OWN form's theme
        // before drawing. It never did: it relied on the canvas below having
        // published one earlier, which makes the grid depend on paint order.
        // With two designers open on differently-themed forms the picker showed
        // whichever painted last, and before a canvas had ever run it showed
        // Liquid Glass, which offers no swatches at all — an empty grid where
        // Elegance's colours belong. (The canvas resolves the same theme a few
        // hundred lines below; resolving it again here is cheap and makes the
        // pane independent of that ordering.)
        {
            let form_theme = self.designers[idx].1.form.theme.clone();
            let surface = self.resolve_surface_theme(form_theme.as_deref());
            cobolt_forms::paint::set_surface_theme(panel_ui.ctx(), surface);
        }

        // Rightmost region: the resizable properties content (only when open).
        let inspector_action = if !props_hidden {
            egui::Panel::right(format!("props_{idx}"))
                .resizable(true)
                .default_size(PROPS_DEFAULT_W)
                .min_size(PROPS_MIN_W)
                .max_size(half_win)
                .frame(props_frame)
                .show(panel_ui, |ui| {
                    // Sole vertical child of the pane (full width for its ScrollArea).
                    let d = &mut self.designers[idx].1;
                    let sel_ctrl = sel_id.as_deref().and_then(|id| d.form.find_control(id));
                    // With several controls selected the pane speaks for all of
                    // them: the primary supplies the values, the caller fans the
                    // edits out (operator, 2026-08-21).
                    let selection = crate::panels::properties::MultiSelection {
                        count: d.selected_ids.len(),
                        uniform: d.selection_is_uniform(),
                        common_keys: d.common_property_keys(),
                    };
                    // SAFETY: form and properties are different fields — field-level split.
                    let form = &d.form as *const cobolt_forms::Form;
                    let props = &mut d.properties;
                    // SAFETY: we only read *form; no aliased write exists.
                    props.show_multi(
                        ui,
                        unsafe { &*form },
                        sel_ctrl,
                        &indexed_files,
                        tr,
                        selection,
                    )
                })
                .inner
        } else {
            crate::panels::properties::InspectorAction::default()
        };

        // The collapse strip — added AFTER the content so it sits to its LEFT — a
        // fixed-width, non-resizable panel with a vertically-centered tab: ◀ hides
        // the pane, ▶ reopens it. Fixed width ⇒ it can never drive a resize.
        let strip_frame = egui::Frame::side_top_panel(&ctx.global_style()).inner_margin(2);
        let mut props_toggle = false;
        egui::Panel::right(format!("props_strip_{idx}"))
            .resizable(false)
            .exact_size(PROPS_TAB_W)
            .frame(strip_frame)
            .show(panel_ui, |ui| {
                // Cross-axis (height) read only — positions a fixed button, never
                // sizes the strip's width.
                let h = ui.available_height();
                ui.add_space((h * 0.5 - 14.0).max(0.0));
                ui.vertical_centered(|ui| {
                    // ▶ when the pane is open (points toward hiding it right),
                    // ◀ when hidden (points toward sliding it back in).
                    let (glyph, tip) = if props_hidden {
                        ("◀", tr.props_show)
                    } else {
                        ("▶", tr.props_hide)
                    };
                    if ui
                        .button(
                            egui::RichText::new(glyph)
                                .size(crate::panels::designer::COLLAPSE_CHEVRON_SIZE),
                        )
                        .on_hover_text(tip)
                        .clicked()
                    {
                        props_toggle = true;
                    }
                });
            });
        if props_toggle {
            self.designers[idx].1.props_hidden = !props_hidden;
        }

        // ── Apply inspector actions ───────────────────────────────────────────
        let mut preview_triggered = false;
        for (ctrl_id, key, value) in inspector_action.set_props {
            if key.starts_with("_PreviewAnim") {
                preview_triggered = true;
            }
            let d = &mut self.designers[idx].1;
            // A control background on a styled form (any GlassStyle other than
            // Classic) breaks the style unit — hold the edit and ask the
            // developer once per form before applying (operator, 2026-07-28).
            // The colour picker streams a change per tick, so only the newest
            // value per control is kept while the confirmation is up.
            if key == "BackgroundColor"
                && !d.style_break_ack
                && d.form.glass_style != cobolt_forms::GlassStyle::Classic
                && !d.is_form_id(&ctrl_id)
            {
                d.style_break_pending
                    .retain(|(c, k, _)| !(c == &ctrl_id && k == &key));
                d.style_break_pending.push((ctrl_id, key, value));
                continue;
            }
            // With several controls selected, the pane is showing what they have
            // in COMMON and an edit belongs to all of them — one undoable step,
            // not one per control (operator, 2026-08-21). The id the pane
            // reports is the primary's; `set_property_multi` fans the change out
            // to every selected control that carries the property and leaves the
            // rest alone.
            if d.selected_ids.len() > 1 && d.is_selected(&ctrl_id) {
                d.set_property_multi(&key, value);
            } else {
                d.set_property(&ctrl_id, &key, value);
            }
        }
        // The style-unit confirmation for the held background edits.
        if !self.designers[idx].1.style_break_pending.is_empty() {
            let mut confirm = false;
            let mut cancel = false;
            egui::Window::new(tr.style_break_title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_max_width(460.0);
                    ui.label(tr.style_break_body);
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button(tr.style_break_continue).clicked() {
                            confirm = true;
                        }
                        if ui.button(tr.btn_cancel).clicked() {
                            cancel = true;
                        }
                    });
                });
            let d = &mut self.designers[idx].1;
            if confirm {
                d.style_break_ack = true;
                let pending = std::mem::take(&mut d.style_break_pending);
                for (ctrl_id, key, value) in pending {
                    d.set_property(&ctrl_id, &key, value);
                }
            } else if cancel {
                d.style_break_pending.clear();
            }
        }
        // An undo/redo that changes COBOL procedure code waits here for the
        // developer's explicit confirmation (operator, 2026-07-29).
        if let Some(dir) = self.designers[idx].1.pending_history_confirm {
            let mut confirm = false;
            let mut cancel = false;
            egui::Window::new(tr.proc_history_title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_max_width(460.0);
                    ui.label(match dir {
                        crate::panels::designer::HistoryDir::Undo => tr.proc_undo_body,
                        crate::panels::designer::HistoryDir::Redo => tr.proc_redo_body,
                    });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button(tr.proc_history_confirm).clicked() {
                            confirm = true;
                        }
                        if ui.button(tr.btn_cancel).clicked() {
                            cancel = true;
                        }
                    });
                });
            if confirm || cancel {
                self.designers[idx].1.confirm_pending_history(confirm);
            }
        }
        if let Some(binding) = inspector_action.create_data_binding {
            // Undoable: the command snapshots the pre-apply bindings and
            // controls (binding application rewrites target properties).
            self.designers[idx].1.apply_data_binding(binding);
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
            // 051 R16 — the editor is shared with MenuBar; only a SideMenu's
            // menu offers the standalone actions.
            let is_side_menu = self.designers[idx]
                .1
                .form
                .find_control(&ctrl_id)
                .map(|c| c.control_type == cobolt_forms::ControlType::SideMenu)
                .unwrap_or(false);
            self.designers[idx].1.menu_modal = Some(
                super::panels::designer::MenuEditorModal::new(ctrl_id, existing)
                    .for_side_menu(is_side_menu),
            );
        }
        // The toolbar's own editor. Its definition lives on the control, not in a
        // side-car file, so opening it is just reading the property back.
        if let Some(ctrl_id) = inspector_action.open_toolbar_editor {
            let def = self.designers[idx]
                .1
                .form
                .find_control(&ctrl_id)
                .map(cobolt_forms::toolbar::ToolbarDef::from_control)
                .unwrap_or_default();
            self.designers[idx].1.toolbar_modal = Some(
                crate::panels::toolbar_editor::ToolbarEditorModal::new(ctrl_id, def),
            );
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
            let new_idx = d.add_user_procedure();
            d.cobol_structure_edit =
                Some(crate::panels::cobol_structure::CsTarget::Procedure(new_idx));
        }
        if let Some(i) = inspector_action.cs_del_proc {
            // Hold the request for confirmation. This is code the developer
            // wrote by hand; pressing the button says what they want, and the
            // dialog says what it costs before it happens.
            let d = &self.designers[idx].1;
            if let Some(p) = d.form.user_procedures.get(i) {
                self.pending_proc_delete = Some(PendingProcDelete {
                    designer: Some(idx),
                    index: i,
                    name: p.name.clone(),
                    lines: p.code.lines().filter(|l| !l.trim().is_empty()).count(),
                });
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
        self.designers[idx].1.active_surface_theme =
            self.resolve_surface_theme(form_theme.as_deref());
        let llm_cfg = self.llm.clone();
        // Project directory (holds the `agentic_ai/` prompt + skills) for the
        // event-editor assistant. Cloned so the closure doesn't borrow `self`.
        let proj_root = self
            .project_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf());
        let project_snapshot = self.cobolt_project.clone();
        let designer_result = egui::CentralPanel::default()
            .show(panel_ui, |ui| {
                self.designers[idx].1.show(
                    ui,
                    &mut self.clipboard,
                    &user_controls,
                    &llm_cfg,
                    project_snapshot.as_ref(),
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

        // Model benchmark dialogs must also be painted in the designer viewport.
        // Otherwise a report produced from the designer assistant can be visible
        // only behind the active OS window, which looks like "it only went to log".
        self.render_llm_benchmark_modals(ctx);

        // Errors and confirmations raised FROM this designer belong in THIS OS
        // window. `Order::Foreground` only orders layers inside one viewport;
        // it cannot lift a window in the main IDE over the designer's own OS
        // window. So the same dialog is painted here as well — one state, so
        // answering either copy answers it everywhere.
        //   • Run Form / Save from the designer toolbar → `form_error`
        //     (do_run_form) and `alert_error` (reject_duplicate_form_cobol_id).
        //   • Delete-procedure in the COBOL Structure inspector →
        //     `pending_proc_delete` with `designer: Some(idx)`.
        self.show_proc_delete_confirmation(ctx);
        self.show_form_error(ctx);
        self.show_alert_error(ctx);
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

/// Locate the bundled `assets/images` directory — beside the executable first,
/// then the working directory for `cargo run` from the repository root.
///
/// It used to be the build machine's own `CARGO_MANIFEST_DIR`, baked into the
/// binary at compile time. That path exists on the machine that COMPILED the
/// IDE and nowhere else, so every installed copy looked for its pictures in a
/// directory belonging to a different computer, found nothing, and silently
/// dropped the welcome background. A shipped binary can only ever find its
/// files relative to itself.
fn images_dir() -> PathBuf {
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    {
        let c = exe_dir.join("assets/images");
        if c.is_dir() {
            return c;
        }
    }
    PathBuf::from("assets/images")
}

/// The welcome-pane background for today, cached in egui memory. Loads
/// `assets/images/bg<day>.jpg`, falling back to `bg1.jpg`.
fn welcome_bg_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let dir = images_dir();
    let dir = dir.display();
    let day = day_of_month();
    let primary = format!("{dir}/bg{day}.jpg");
    let path = if std::path::Path::new(&primary).exists() {
        primary
    } else {
        format!("{dir}/bg1.jpg")
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
/// Does an agent actually resolve a model? It must either name one directly, or
/// point at a model profile that still exists AND carries a provider + model — a
/// dangling `model_profile` id (or a profile with no model, as several pedantic
/// reviewers have) resolves to nothing.
fn agent_resolves_model(llm: &crate::llm::LlmConfig, agent: &crate::agents_db::AgentDef) -> bool {
    if !agent.model.trim().is_empty() {
        return true;
    }
    match agent.model_profile.as_deref() {
        Some(id) => llm.model_profiles.iter().any(|p| {
            p.id == id && !p.provider.trim().is_empty() && !p.model.trim().is_empty()
        }),
        None => false,
    }
}

/// Should the "set up the AI" invitation be shown for this project?
///
/// Two conditions, either of which makes the AI unusable:
///   1. there is no usable model anywhere (no default provider+model and no
///      usable model profile), or
///   2. **Grace has no model**. Grace is the single coordination authority — with
///      no model on her there is no AI, no matter how many specialists are
///      configured. (Checking "any agent has a model" was the original bug: the
///      specialists kept their profile after the user unset Grace's, so the
///      invitation never appeared.)
///
/// Pure so it can be unit-tested against real project shapes.
fn ai_setup_needed_for(
    llm: &crate::llm::LlmConfig,
    agents: &[crate::agents_db::AgentDef],
) -> bool {
    let has_model = (!llm.provider.trim().is_empty() && !llm.model.trim().is_empty())
        || llm.has_usable_model_profile();
    if !has_model {
        return true;
    }
    let grace_ready = agents.iter().any(|a| {
        matches!(a.kind, crate::agents_db::AgentKind::Orchestrator)
            && a.enabled
            && agent_resolves_model(llm, a)
    });
    !grace_ready
}

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

pub(crate) fn apply_data_binding_to_form(form: &mut Form, binding: DataBindingDef) {
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

/// The tracked `generated/` entry whose file name is `file_name`, if any. Free
/// function so the resolve-relocated-generated behaviour (spec 033, R7) is
/// unit-testable without constructing the full `App`.
fn tracked_generated_rel(
    project: Option<&CoboltProject>,
    file_name: &str,
) -> Option<String> {
    project?
        .files_in(crate::project_model::Category::Generated)
        .iter()
        .find(|rel| {
            std::path::Path::new(rel)
                .file_name()
                .and_then(|n| n.to_str())
                == Some(file_name)
        })
        .cloned()
}

#[cfg(test)]
mod build_button_full_tests {
    use super::*;
    use crate::project_model::CoboltProject;

    fn proj(built_with: &str) -> CoboltProject {
        let mut p = CoboltProject::new("Demo.project", "main.cbl");
        p.project.built_with_version = built_with.to_owned();
        p
    }

    /// The Build button discards cached artefacts exactly when Run would stop
    /// and ask for a full build. Before this, Build was unconditionally
    /// incremental: it left the stamp alone, so the developer waited out a
    /// build and Run immediately asked for another one.
    #[test]
    fn build_is_full_exactly_when_run_would_prompt() {
        // Never fully built (every project created before the stamp existed,
        // and every brand-new one) → full.
        assert!(build_needs_full(Some(&proj("")), "1.60.36"));
        // Built by an older PowerRustCOBOL → full.
        assert!(build_needs_full(Some(&proj("1.60.35")), "1.60.36"));
        // Already built by this one → incremental, so the common case stays
        // as quick as it ever was.
        assert!(!build_needs_full(Some(&proj("1.60.36")), "1.60.36"));
        // Downgraded IDE → full as well (operator, 2026-08-11): the output was
        // produced by a compiler this IDE is not, whichever way the version
        // moved. Build and Run stay in lockstep, which is the point of this
        // test — Build discards exactly when Run would ask.
        assert!(build_needs_full(Some(&proj("1.61.0")), "1.60.36"));
    }

    /// No project open — there is nothing to full-build, and `do_build_binary`
    /// refuses for want of a manifest anyway.
    #[test]
    fn no_project_never_forces_a_full_build() {
        assert!(!build_needs_full(None, "1.60.36"));
    }

    /// A full build is what writes the stamp, so the SECOND build of an
    /// upgraded project is incremental again: the prompt and the long build
    /// happen once per upgrade, not once per Build click.
    #[test]
    fn the_full_build_settles_it_for_this_version() {
        let mut p = proj("1.60.35");
        assert!(build_needs_full(Some(&p), "1.60.36"));
        // What the build-result handler stamps on success.
        p.project.built_with_version = "1.60.36".to_owned();
        assert!(!build_needs_full(Some(&p), "1.60.36"));
    }
}

#[cfg(test)]
mod bundled_asset_path_tests {
    use super::*;

    /// Everything the IDE opens at run time must be reachable from the
    /// executable, never from the machine that compiled it.
    ///
    /// `images_dir` used to be the build machine's `CARGO_MANIFEST_DIR`, so a
    /// packaged IDE looked for its welcome background in a directory that only
    /// existed on a GitHub runner. This asserts the two directories the binary
    /// reads from disk resolve the same way — beside the executable, or under
    /// the working directory — and that neither ever names a build path.
    #[test]
    fn bundled_directories_resolve_beside_the_executable() {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .expect("a test binary has a directory");

        for dir in [images_dir(), CoboltApp::themes_dir()] {
            let s = dir.to_string_lossy().into_owned();
            assert!(
                !s.contains(env!("CARGO_MANIFEST_DIR")),
                "a bundled directory must not carry the build machine's path: {s}"
            );
            assert!(
                dir.starts_with(&exe_dir) || dir.is_relative(),
                "a bundled directory must sit beside the executable or under the \
                 working directory, got {s}"
            );
        }
    }
}

#[cfg(test)]
mod generated_path_tests {
    use super::*;
    use crate::project_model::{Category, CoboltProject};

    #[test]
    fn relocated_generated_file_resolves_to_tracked_path() {
        let mut proj = CoboltProject::new("T", "src/main.cbl");
        proj.add_file_to("generated/customers/order.cbl", Category::Generated);
        // A form whose stem matches a relocated generated entry resolves there.
        assert_eq!(
            tracked_generated_rel(Some(&proj), "order.cbl").as_deref(),
            Some("generated/customers/order.cbl")
        );
        // An unknown stem falls through (caller uses the default path).
        assert_eq!(tracked_generated_rel(Some(&proj), "unknown.cbl"), None);
        assert_eq!(tracked_generated_rel(None, "order.cbl"), None);
    }
}

#[cfg(test)]
mod preview_alpha_tests {
    use super::*;
    use cobolt_forms::render::FormState;
    use cobolt_forms::{Control, ControlType};

    fn preview_alpha(c: &Control) -> f32 {
        let values = std::collections::HashMap::new();
        let anim = std::collections::HashMap::new();
        PreviewState {
            values: &values,
            anim: &anim,
            form_w: 400.0,
            form_h: 300.0,
        }
        .transform(c)
        .alpha
    }

    /// A control's `Transparency` fades its FACE — `draw_control` applies it
    /// there and nowhere else. The preview must not multiply it in a second
    /// time as a whole-control alpha: a CheckBox is seeded at `Transparency =
    /// 100` (no card to lift), which made the preview draw the entire control
    /// at alpha 0 — its tick box vanished and clicking it changed nothing
    /// visible, while the designer canvas beside it drew the box normally.
    #[test]
    fn the_preview_never_erases_a_control_with_a_transparent_face() {
        let checkbox = Control::new("CheckBox-1", ControlType::CheckBox, 10, 10);
        assert_eq!(
            cobolt_forms::model::transparency_of(&checkbox),
            100,
            "a CheckBox has no face of its own"
        );
        assert_eq!(
            preview_alpha(&checkbox),
            1.0,
            "the preview must draw the tick box, not erase the control"
        );

        // And the same for every degree of transparency, on any control type:
        // the preview's alpha is the animation's alone.
        let mut panel = Control::new("Panel-1", ControlType::Panel, 0, 0);
        for t in [0, 30, 50, 100] {
            panel.set_prop("Transparency", cobolt_forms::PropValue::Int(t));
            assert_eq!(
                preview_alpha(&panel),
                1.0,
                "Transparency {t} is the face's business, not the control's"
            );
        }

        println!(
            "\n  preview alpha — CheckBox (Transparency 100) and Panel at 0/30/50/100 \
             all render at alpha 1.0; the face alone is faded, by draw_control\n"
        );
    }

    /// Clicking a toggle in the preview has to survive the trip back into the
    /// preview's value map. The engine reports a CheckBox/RadioButton click as
    /// `Value` and a Switch's as `Checked`, while the map keys all three by
    /// `Checked` — so the exact-key rule dropped two of the three on the floor.
    #[test]
    fn a_toggle_click_reaches_the_preview_value_map() {
        for ct in [
            ControlType::CheckBox,
            ControlType::RadioButton,
            ControlType::Switch,
        ] {
            let expected = preview_value_key(&ct);
            assert_eq!(expected, "Checked");
            assert!(
                preview_accepts_update(expected, "Value"),
                "{ct:?}: the engine's own spelling must be accepted"
            );
            assert!(preview_accepts_update(expected, "Checked"), "{ct:?}");
            assert!(
                !preview_accepts_update(expected, "Caption"),
                "{ct:?}: unrelated keys still do not belong in a one-value map"
            );
        }
        // A control whose value really is `Value` is unaffected.
        assert!(preview_accepts_update(
            preview_value_key(&ControlType::Slider),
            "Value"
        ));
        assert!(!preview_accepts_update(
            preview_value_key(&ControlType::TextBox),
            "Value"
        ));

        // …and the stored value is what the renderer reads back as "checked".
        let checkbox = Control::new("CheckBox-1", ControlType::CheckBox, 10, 10);
        let mut values = std::collections::HashMap::new();
        values.insert("CheckBox-1".to_owned(), "1".to_owned());
        let anim = std::collections::HashMap::new();
        let live = PreviewState {
            values: &values,
            anim: &anim,
            form_w: 400.0,
            form_h: 300.0,
        }
        .live(&checkbox);
        assert_eq!(
            live.get_prop("Checked").map(|v| v.as_str().to_owned()),
            Some("1".to_owned()),
            "the click must come back as the checked state"
        );

        println!(
            "\n  preview toggles — CheckBox/RadioButton report `Value`, Switch reports \
             `Checked`; all three now land in the map and read back as checked\n"
        );
    }

    /// A ListBox reports three values — the active row, the Ctrl-click
    /// selection and the ticked set — but the preview map holds one per
    /// control. The extra two are kept beside it under `id::Prop` and merged
    /// back on, or a second selection could never survive a frame.
    #[test]
    fn a_preview_keeps_a_controls_second_and_third_value() {
        let expected = preview_value_key(&ControlType::ListBox);
        assert_eq!(expected, "Value", "the list's primary value is the active row");
        assert!(preview_keeps_extra_update("SelectedItems"));
        assert!(preview_keeps_extra_update("CheckedItems"));
        assert!(
            !preview_keeps_extra_update("Caption"),
            "…and only the values a control really reports are kept"
        );

        let list = Control::new("ListBox-1", ControlType::ListBox, 0, 0);
        let mut values = std::collections::HashMap::new();
        values.insert("ListBox-1".to_owned(), "Beta".to_owned());
        values.insert(
            "ListBox-1::SelectedItems".to_owned(),
            "Alpha\nBeta".to_owned(),
        );
        values.insert("ListBox-1::CheckedItems".to_owned(), "Gamma".to_owned());
        let anim = std::collections::HashMap::new();
        let live = PreviewState {
            values: &values,
            anim: &anim,
            form_w: 400.0,
            form_h: 300.0,
        }
        .live(&list);

        assert_eq!(
            live.get_prop("Value").map(|v| v.as_str().to_owned()),
            Some("Beta".to_owned())
        );
        assert_eq!(
            live.get_prop("SelectedItems").map(|v| v.as_str().to_owned()),
            Some("Alpha\nBeta".to_owned())
        );
        assert_eq!(
            live.get_prop("CheckedItems").map(|v| v.as_str().to_owned()),
            Some("Gamma".to_owned())
        );

        println!(
            "\n  preview values — a ListBox's active row, its Ctrl-click selection and its \
             ticked set all survive the trip back\n"
        );
    }
}

#[cfg(test)]
mod preview_toolbar_tests {
    use super::*;
    use cobolt_forms::toolbar::ToolbarAction as TA;

    /// Every one of the eleven toolbar verbs, and what Preview does with it.
    ///
    /// A toolbar is BUILT in Preview, so a button that only works under Run Form
    /// is a button you cannot design against. Six of the verbs need nothing but
    /// the platform and are carried out here; three are the form's own COBOL and
    /// need an interpreter Preview does not have; and the two captures are
    /// refused ON PURPOSE — Preview is a pane inside the IDE window, so a capture
    /// taken here would return a picture of the IDE.
    #[test]
    fn preview_performs_the_platform_verbs_and_refuses_the_captures() {
        let carried_out = [
            TA::Print("/tmp/report.pdf".into()),
            TA::Copy,
            TA::Cut,
            TA::Paste,
            TA::RunApp("/usr/bin/vim".into()),
            TA::OpenTerminal("/tmp".into()),
        ];
        for action in &carried_out {
            assert_eq!(
                preview_press(action),
                PreviewPress::Perform,
                "`{}` needs nothing but the platform — Preview must honour it",
                action.verb()
            );
        }

        for action in [TA::Screenshot, TA::Share] {
            assert_eq!(
                preview_press(&action),
                PreviewPress::NeedsRunForm,
                "`{}` would photograph the IDE, not the form",
                action.verb()
            );
        }

        for action in [
            TA::Event,
            TA::Procedure("UPDATE-TOTAL".into()),
            TA::OpenModal("CUST-LOOKUP".into()),
        ] {
            assert_eq!(
                preview_press(&action),
                PreviewPress::LeaveToTheForm,
                "`{}` is the form's own COBOL",
                action.verb()
            );
        }

        // Every advertised verb is accounted for — a new one cannot slip in
        // without a decision being made about it here.
        assert_eq!(
            carried_out.len() + 2 + 3,
            TA::VERBS.len(),
            "the editor offers {} verbs; this test covers {}",
            TA::VERBS.len(),
            carried_out.len() + 5
        );

        println!(
            "\n  Preview toolbar — all {} verbs decided: 6 carried out (print, copy, cut, \
             paste, run-app, open-terminal), 2 refused with a reason (screenshot, share — \
             they would capture the IDE window), 3 left to the running form (event, \
             procedure, open-modal)\n",
            TA::VERBS.len()
        );
    }
}

#[cfg(test)]
mod form_paste_tests {
    use super::*;

    /// Spec 046 R5 — a pasted form's destination file name matches
    /// `create_new_form`'s own convention, so it's registered exactly as a
    /// hand-created form would be.
    #[test]
    fn pasted_form_file_name_matches_the_create_convention() {
        assert_eq!(pasted_form_file_name("MAIN-FORM"), "main-form.cfrm");
        assert_eq!(pasted_form_file_name("Login"), "login.cfrm");
    }

    /// Spec 046 R3/R4 — `extract_pasted_text` finds the `Paste` event among
    /// whatever else a frame's input carries, and correctly reports absence
    /// when there isn't one — the exact scan `poll_form_paste` runs, tested
    /// without needing an `egui::Context`.
    #[test]
    fn extract_pasted_text_finds_paste_among_other_events() {
        let events = vec![
            egui::Event::PointerMoved(egui::pos2(1.0, 2.0)),
            egui::Event::Paste("<Form name=\"X\"></Form>".to_string()),
            egui::Event::Text("ignored".to_string()),
        ];
        assert_eq!(
            extract_pasted_text(&events),
            Some("<Form name=\"X\"></Form>".to_string())
        );
        let none: Vec<egui::Event> = vec![egui::Event::PointerMoved(egui::pos2(0.0, 0.0))];
        assert_eq!(extract_pasted_text(&none), None);
    }

    /// Spec 046 R9/AC5 — the same parser `poll_form_paste` calls on
    /// whatever text a paste delivers refuses both arbitrary non-XML text
    /// and well-formed XML that isn't a `<Form>` — the two ways a
    /// developer's clipboard can hold something that isn't a copied form.
    /// `poll_form_paste` only reaches this call on `Err`, does nothing else
    /// (an early return, no file/project mutation reachable), so this
    /// proves R9's "changes nothing" by construction.
    #[test]
    fn invalid_clipboard_text_is_refused_and_changes_nothing() {
        assert!(load_form_from_str("this is not xml at all").is_err());
        assert!(load_form_from_str("<NotAForm><Something/></NotAForm>").is_err());
        assert!(load_form_from_str("").is_err());
    }
}

#[cfg(test)]
mod manifest_name_tests {
    use super::*;
    use cobolt_forms::{
        BindingDataType, BindingField, BindingSourceDescriptor, BindingTargetDescriptor, Control,
        ControlType, EventBinding, FieldMapping,
    };

    #[test]
    fn designer_activation_request_waits_for_its_target_and_is_one_shot() {
        let form_a = PathBuf::from("forms/a.cfrm");
        let form_b = PathBuf::from("forms/b.cfrm");
        let mut requests = DesignerActivationRequests::default();

        requests.request(form_a.clone());
        requests.request(form_a.clone());

        assert!(!requests.take(&form_b));
        assert!(requests.take(&form_a));
        assert!(!requests.take(&form_a));
    }

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

#[cfg(test)]
mod ai_setup_invite_tests {
    use super::*;
    use crate::agents_db::AgentDef;
    use crate::llm::{LlmConfig, ModelProfile};

    // Fixtures are built through serde (every field has a serde default), which
    // also exercises the real manifest-deserialization path.
    fn profile(id: &str, provider: &str, model: &str) -> ModelProfile {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": id, "provider": provider, "model": model
        }))
        .expect("model profile fixture")
    }

    fn agent(
        name: &str,
        kind: &str,
        enabled: bool,
        prof: Option<&str>,
        model: &str,
    ) -> AgentDef {
        serde_json::from_value(serde_json::json!({
            "id": name, "name": name, "kind": kind, "enabled": enabled,
            "model_profile": prof, "model": model
        }))
        .expect("agent fixture")
    }

    /// The operator's real shape (PowerDemo2): every specialist keeps a working
    /// profile, but Grace's model association was removed. "No Grace, no AI" —
    /// so the invitation MUST appear. Checking "any agent has a model" (the
    /// original bug) reported everything fine and never showed it.
    #[test]
    fn invite_shows_when_only_grace_lost_its_model() {
        let mut llm = LlmConfig::load_defaults_for_test();
        llm.provider.clear();
        llm.model.clear();
        llm.model_profiles = vec![profile("d9566a66", "ollama_cloud", "gemma4:31b")];
        let agents = vec![
            agent("Grace", "orchestrator", true, None, ""),
            agent("Form Designer Agent", "specialist", true, Some("d9566a66"), "gemma4:31b"),
            agent("Documentation Agent", "specialist", true, Some("d9566a66"), "gemma4:31b"),
            agent("Version Control Agent", "specialist", true, Some("d9566a66"), "gemma4:31b"),
        ];
        assert!(
            ai_setup_needed_for(&llm, &agents),
            "Grace has no model — the AI setup invitation must be shown"
        );
    }

    /// Fully configured project: Grace resolves a model through a profile.
    #[test]
    fn invite_hidden_when_grace_resolves_a_model() {
        let mut llm = LlmConfig::load_defaults_for_test();
        llm.provider.clear();
        llm.model.clear();
        llm.model_profiles = vec![profile("d9566a66", "ollama_cloud", "gemma4:31b")];
        let agents = vec![
            agent("Grace", "orchestrator", true, Some("d9566a66"), ""),
            agent("Form Designer Agent", "specialist", true, Some("d9566a66"), "gemma4:31b"),
        ];
        assert!(!ai_setup_needed_for(&llm, &agents));
    }

    /// A dangling profile id (or one carrying no model — several pedantic
    /// reviewers look like that) resolves to nothing, so Grace is NOT configured.
    #[test]
    fn dangling_or_empty_profile_does_not_configure_grace() {
        let mut llm = LlmConfig::load_defaults_for_test();
        llm.provider.clear();
        llm.model.clear();
        llm.model_profiles = vec![
            profile("d9566a66", "ollama_cloud", "gemma4:31b"),
            profile("a17ed3fb", "", ""), // profile with no provider/model
        ];
        let dangling = vec![agent("Grace", "orchestrator", true, Some("nope"), "")];
        assert!(ai_setup_needed_for(&llm, &dangling), "dangling profile id");
        let empty = vec![agent("Grace", "orchestrator", true, Some("a17ed3fb"), "")];
        assert!(ai_setup_needed_for(&llm, &empty), "profile without a model");
    }

    /// No usable model anywhere → invite regardless of the agent roster.
    #[test]
    fn invite_shows_when_no_model_exists_at_all() {
        let mut llm = LlmConfig::load_defaults_for_test();
        llm.provider.clear();
        llm.model.clear();
        llm.model_profiles.clear();
        let agents = vec![agent("Grace", "orchestrator", true, None, "")];
        assert!(ai_setup_needed_for(&llm, &agents));
    }

    /// A disabled Grace cannot run, so the project still needs setup.
    #[test]
    fn disabled_grace_still_needs_setup() {
        let mut llm = LlmConfig::load_defaults_for_test();
        llm.model_profiles = vec![profile("d9566a66", "ollama_cloud", "gemma4:31b")];
        let agents = vec![agent("Grace", "orchestrator", false, Some("d9566a66"), "")];
        assert!(ai_setup_needed_for(&llm, &agents));
    }
}

#[cfg(test)]
mod benchmark_metrics_tests {
    use super::*;

    /// Every metrics-block shape found in the real archived reports under
    /// `agentic_ai/model-benchmarks.jsonl` (spec 040). The middle one is what
    /// used to be dropped: a `json` fence whose body is an assignment.
    #[test]
    fn every_archived_fence_shape_yields_scores() {
        for body in [
            "```json\n{\n  \"overall_score\": 82\n}\n```",
            "```json\nmetrics = {\n  \"overall_score\": 82\n}\n```",
            "```metrics\n{\n  \"overall_score\": 82\n}\n```",
            "```metrics\n= {\n  \"overall_score\": 82\n}\n```",
        ] {
            let report = format!("Executive summary.\n\n{body}\n");
            let metrics = CoboltApp::llm_benchmark_metrics(&report)
                .unwrap_or_else(|| panic!("no metrics parsed from:\n{body}"));
            assert_eq!(
                CoboltApp::metric_score(&metrics, "overall_score"),
                Some(82.0),
                "wrong score from:\n{body}"
            );
        }
    }

    /// A fence tag must not be peeled off a word that merely starts with it.
    #[test]
    fn a_key_beginning_with_a_tag_name_is_not_mistaken_for_one() {
        let report = "```json\n{\"metrics_version\": 2, \"overall_score\": 70}\n```";
        let metrics = CoboltApp::llm_benchmark_metrics(report).expect("parsed");
        assert_eq!(
            CoboltApp::metric_score(&metrics, "overall_score"),
            Some(70.0)
        );
    }

    /// The pedantic reviewer's verdict overrides the primary's self-scoring.
    #[test]
    fn the_pedantic_final_block_wins() {
        let report = "```json\n{\"overall_score\": 95}\n```\n\
                      ```json\n{\"pedantic_final\": true, \"overall_score\": 61}\n```";
        let metrics = CoboltApp::llm_benchmark_metrics(report).expect("parsed");
        assert_eq!(
            CoboltApp::metric_score(&metrics, "overall_score"),
            Some(61.0)
        );
    }
}

#[cfg(test)]
mod error_modal_tests {
    use super::*;

    /// Render one frame of the production error-modal stack (Window +
    /// resize scaffold + panel body) and report the window's area rect.
    fn run_frame(ctx: &egui::Context, font: &mut f32, msg: &str) -> Option<egui::Rect> {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(1600.0, 1000.0),
        ));
        ctx.run_ui(input, |root_ui| {
            let ctx2 = root_ui.ctx().clone();
            egui::Window::new("⛔ Error")
                .id(egui::Id::new("test_error_modal"))
                .collapsible(false)
                .resizable(false)
                .show(&ctx2, |ui| {
                    error_modal_scaffold(ui, "test_error_modal_resize", |ui| {
                        let _ = error_modal_body_ui(
                            ui,
                            Some("Execution stopped. See the Output panel for details."),
                            msg,
                            font,
                        );
                    });
                });
        })
        .textures_delta
        .clear();
        ctx.memory(|m| m.area_rect(egui::Id::new("test_error_modal")))
    }

    /// R9 / AC7 (spec 027): the error modal opens at its seeded size and holds
    /// it — egui 0.35's `Resize` ratchets up to the measured content min every
    /// frame, so ANY body overflow becomes runaway self-inflation. 120 frames
    /// with a long multi-line message must produce an identical rect.
    #[test]
    fn error_modal_holds_seeded_size_across_frames() {
        let ctx = egui::Context::default();
        let mut font = 13.0;
        let long_line = "The model returned 33292 reasoning characters but no assistant \
                         message content. PowerRustCOBOL cannot apply hidden reasoning as \
                         form operations. "
            .repeat(4);
        let msg = format!("{long_line}\n").repeat(40);

        let mut sizes: Vec<egui::Vec2> = Vec::new();
        for _ in 0..120 {
            if let Some(r) = run_frame(&ctx, &mut font, &msg) {
                sizes.push(r.size());
            }
        }
        assert!(sizes.len() >= 100, "window rect missing most frames");
        let settled = sizes[4];
        for (i, s) in sizes.iter().enumerate().skip(4) {
            assert!(
                (s.x - settled.x).abs() < 0.5 && (s.y - settled.y).abs() < 0.5,
                "error modal size drifted at frame {i}: {settled:?} -> {s:?} \
                 (self-inflation regression)"
            );
        }
        // Seeded 800x450 box + window chrome; anything near screen size means
        // the ratchet is back.
        assert!(
            settled.x < 900.0 && settled.y < 600.0,
            "error modal settled larger than seed+chrome: {settled:?}"
        );
        println!(
            "error modal stable at {:.0}x{:.0} px across {} frames",
            settled.x,
            settled.y,
            sizes.len()
        );
    }
}
