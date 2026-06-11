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

use cobolt_forms::{Form, load_form, save_form};
use cobolt_codegen::generate;
use cobolt_compiler::{BuildOptions, build_project};

use crate::panels::{
    designer::DesignerPanel,
    editor::EditorPanel,
    forms_list::FormsListPanel,
    output::OutputPanel,
    project::{ProjectPanel, ProjectPanelEvent},
    toolbar::{self, ToolbarAction},
};
use crate::project_model::{
    CoboltProject, ElementStatus, FileKind,
    load_project, save_project, package_project, relative_to,
};
use crate::form_runtime::FormRuntime;
use crate::runner::{Runner, DebugRunner, RunMsg};
use crate::panels::debugger::DebuggerPanel;
use cobolt_runtime::DebugCmd;
use crate::version::VERSION;
use crate::i18n::{Language, Tr};

// ── Dialog state ──────────────────────────────────────────────────────────────

/// State for the "Report Bug" dialog available in both the IDE and designer.
struct ReportBugDialog {
    open:        bool,
    /// Short one-line title of the problem.
    title:       String,
    /// Longer description (steps to reproduce, what went wrong, etc.)
    description: String,
    /// Which surface the bug was reported from (e.g. "IDE Editor", "Form Designer").
    component:   String,
    /// Feedback shown after submission ("Saved." or an error).
    feedback:    Option<String>,
}

impl ReportBugDialog {
    fn new() -> Self {
        Self {
            open:        false,
            title:       String::new(),
            description: String::new(),
            component:   "IDE".into(),
            feedback:    None,
        }
    }

    /// Open the dialog pre-filled with the given component name.
    fn open_for(&mut self, component: impl Into<String>) {
        self.open      = true;
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
            let days  = secs / 86400;
            // Approximate calendar date (good enough for a bug-tracker timestamp).
            let y = 1970 + days / 365;
            let d = days % 365;
            let m = (d / 30) + 1;
            let dd = (d % 30) + 1;
            format!("{y:04}-{m:02}-{dd:02}")
        };

        let component = self.component.replace('|', "∣");
        let title     = self.title.trim().replace('|', "∣");
        let desc      = self.description.trim().replace('|', "∣");
        let summary   = if desc.is_empty() { title.clone() }
                        else { format!("{title} — {desc}") };
        let summary   = if summary.len() > 100 { format!("{}…", &summary[..97]) } else { summary };

        let new_row = format!(
            "| BUG-{next_id:03} | {today} | `{component}` | `MANUAL` | {summary} |\n"
        );

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
    open:      bool,
    form_name: String,
    title:     String,
    width:     String,
    height:    String,
}

impl NewFormDialog {
    fn new() -> Self {
        Self {
            open:      false,
            form_name: "MAIN-FORM".into(),
            title:     "My Form".into(),
            width:     "640".into(),
            height:    "480".into(),
        }
    }
}

struct NewProjectDialog {
    open:    bool,
    name:    String,
    version: String,
    main:    String,
}

impl NewProjectDialog {
    fn new() -> Self {
        Self {
            open:    false,
            name:    "MyApp".into(),
            version: "1.0.0".into(),
            main:    "src/main.cbl".into(),
        }
    }
}

// ── CoboltApp ─────────────────────────────────────────────────────────────────

pub struct CoboltApp {
    // Code workspace
    project:    ProjectPanel,
    editor:     EditorPanel,
    output:     OutputPanel,
    runner:     Runner,
    forms_list: FormsListPanel,

    // Open form designers (each lives in its own viewport window)
    designers: Vec<(PathBuf, DesignerPanel)>,

    // Inline form/control inspector shown in the Main Pane (from the project tree)
    inspect: Option<InspectState>,

    // Content hash of each file at its last successful/failed check (for the tree
    // "semaphore": a file edited since its last check shows yellow again).
    checked: std::collections::HashMap<PathBuf, u64>,

    // Running form instances — each has its own OS window (Phase 6)
    form_runtimes: Vec<FormRuntime>,

    // Debugger (Phase 7)
    debug_runner:  DebugRunner,
    debugger:      DebuggerPanel,
    debug_active:  bool,

    // Project model
    cobolt_project: Option<CoboltProject>,
    project_path:   Option<PathBuf>,

    // Appearance settings dialog (theme + background image, per project)
    show_settings:  bool,
    /// Cached background-image texture, keyed by the resolved absolute path.
    bg_texture:     Option<(PathBuf, egui::TextureHandle)>,

    // Dialog state
    new_form:    NewFormDialog,
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

    /// Report Bug dialog (shown from both IDE toolbar and designer toolbar).
    report_bug: ReportBugDialog,
    /// Non-empty while the "Form saved" alert should be displayed.
    save_alert_msg: Option<String>,

    /// Pending binary build result channel (Phase 11).
    pending_build_rx: Option<std::sync::mpsc::Receiver<Result<cobolt_compiler::BuildResult, String>>>,

    /// Which app-level file dialog (if any) is currently open; its result is
    /// applied by `apply_file_result` once the async picker returns.
    pending_file: Option<FileRequest>,
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

/// Inline form/control inspector shown in the Main Pane (from the project tree).
/// Holds a transient `DesignerPanel` so it reuses the designer's property-edit
/// machinery without opening a designer window.
struct InspectState {
    path:     PathBuf,
    ctrl_id:  Option<String>,
    designer: DesignerPanel,
    /// `.cfrm` modification time of the form currently held in `designer`.
    /// Used to live-refresh the Main-Pane inspector when the form is changed
    /// elsewhere (e.g. saved from the Designer window) so edits reflect back.
    mtime:    Option<std::time::SystemTime>,
}

impl InspectState {
    /// Reload the form from disk if the `.cfrm` changed since we last read it
    /// (e.g. saved from the Designer window). Returns true when reloaded.
    fn reload_if_stale(&mut self) -> bool {
        let disk = file_mtime(&self.path);
        let stale = match (disk, self.mtime) {
            (Some(d), Some(cur)) => d > cur,
            (Some(_), None)      => true,
            _                    => false,
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
        cc.egui_ctx.set_fonts(egui::FontDefinitions::default());

        Self {
            project:    ProjectPanel::new(),
            editor:     EditorPanel::new(),
            output:     OutputPanel::new(),
            runner:     Runner::new(),
            forms_list: FormsListPanel::new(),
            designers:     Vec::new(),
            inspect:       None,
            checked:       std::collections::HashMap::new(),
            form_runtimes: Vec::new(),
            debug_runner:  DebugRunner::new(),
            debugger:      DebuggerPanel::new(),
            debug_active:  false,

            cobolt_project: None,
            project_path:   None,

            show_settings:  false,
            bg_texture:     None,

            new_form:    NewFormDialog::new(),
            new_project: NewProjectDialog::new(),

            pending_open_in_editor: None,
            pending_goto_paragraph: None,
            glass_visuals_applied:  false,
            lang: Language::English,
            report_bug:      ReportBugDialog::new(),
            save_alert_msg:  None,
            pending_build_rx: None,
            pending_file:     None,
        }
    }

    // ── Code workspace actions ────────────────────────────────────────────────

    fn do_run(&mut self) {
        let Some((path, src)) = self.editor.active_source() else { return; };
        let path   = path.clone();
        let source = src.to_owned();
        self.output.clear();
        self.output.push_status(format!(
            "── Running {} ──",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("?")
        ));
        self.editor.clear_diags();
        self.runner.start(path.display().to_string(), source);
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
        let Some((path, src)) = self.editor.active_source() else { return; };
        let path   = path.clone();
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
        // Save the form and regenerate COBOL first.
        self.do_save_designer(idx);
        self.do_generate_cobol(idx);

        let form_path = self.designers[idx].0.clone();
        let form      = self.designers[idx].1.form.clone();

        self.output.clear();
        self.output.push_status(format!(
            "── Running form {} ──",
            form_path.file_name().and_then(|n| n.to_str()).unwrap_or("?")
        ));

        // Kill any existing runtime for this form first.
        self.form_runtimes.retain_mut(|rt| {
            if rt.form_path == form_path { rt.stop(); false } else { true }
        });

        match FormRuntime::launch(&form, form_path) {
            Ok(rt) => {
                self.form_runtimes.push(rt);
            }
            Err(e) => {
                self.output.push_status(format!("Error launching form: {e}"));
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
        let Some((path, src)) = self.editor.active_source() else { return; };
        let path   = path.clone();
        let source = src.to_owned();
        self.output.clear();
        self.output.push_status(format!(
            "── Checking {} ──",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("?")
        ));
        self.editor.clear_diags();

        use cobolt_lexer::{tokenize, SourceFormat};
        use cobolt_parser::parse;
        use cobolt_semantic::analyze;
        use crate::runner::{DiagMsg, DiagSeverity, RunMsg};

        let fmt = if source.lines().any(|l| {
            let b = l.as_bytes();
            b.len() > 6 && b[6] != b' '
                && b[..6].iter().all(|&c| c == b' ' || c.is_ascii_digit())
        }) { SourceFormat::Fixed } else { SourceFormat::Free };

        let tokens       = tokenize(&source, fmt);
        let parse_result = parse(tokens);

        for d in &parse_result.diagnostics {
            use cobolt_parser::Severity as PSev;
            let sev = match d.severity {
                PSev::Error   => DiagSeverity::Error,
                PSev::Warning => DiagSeverity::Warning,
            };
            let diag = DiagMsg { severity: sev, message: d.message.clone(),
                                 line: d.span.line, col: d.span.col };
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
                        Severity::Error   => DiagSeverity::Error,
                        Severity::Warning => DiagSeverity::Warning,
                        Severity::Info    => DiagSeverity::Info,
                    };
                    let diag = DiagMsg { severity: sev, message: d.message.clone(),
                                        line: d.span.line, col: d.span.col };
                    self.output.push_msg(&RunMsg::Diagnostic(diag.clone()));
                    self.editor.add_diag(&path, diag);
                }
            }
        }

        // ── Update the tree semaphore for the checked file ────────────────────
        let had_error = self.editor.diags.get(&path)
            .map(|v| v.iter().any(|d| d.severity == DiagSeverity::Error))
            .unwrap_or(false);
        self.checked.insert(path.clone(), Self::content_hash(&source));
        self.set_element_status(
            &path,
            if had_error { ElementStatus::Failed } else { ElementStatus::Tested },
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

    fn do_new_project(&mut self) { self.new_project.open = true; }

    fn create_new_project(&mut self) {
        self.begin_file_dialog(
            FileRequest::CreateProject,
            crate::file_dialog::DialogSpec::save()
                .filter("RustCOBOL Project", &["toml"])
                .file_name("cobolt.toml"),
        );
    }

    /// Finish creating a new project once the user has chosen `cobolt.toml`.
    fn create_new_project_at(&mut self, path: PathBuf) {
        let mut proj = CoboltProject::new(
            self.new_project.name.clone(),
            self.new_project.main.clone(),
        );
        proj.project.version = self.new_project.version.clone();

        match save_project(&proj, &path) {
            Ok(()) => {
                let dir = path.parent().map(|p| p.to_owned());
                self.cobolt_project = Some(proj);
                self.project_path   = Some(path);
                if let Some(dir) = dir {
                    self.project.set_root(&dir);
                    self.forms_list.set_root(&dir);
                }
                let name = self.cobolt_project.as_ref().unwrap().project.name.clone();
                self.output.push_status(format!("Created project '{name}'"));
                self.new_project.open = false;
            }
            Err(e) => {
                self.output.push_status(format!("Failed to create project: {e}"));
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
                self.output.push_status(format!("Opened project '{}'", proj.project.name));
                self.cobolt_project = Some(proj);
                self.project_path   = Some(path);
                if let Some(dir) = dir {
                    self.project.set_root(&dir);
                    self.forms_list.set_root(&dir);
                }
            }
            Err(e) => {
                self.output.push_status(format!("Failed to open project: {e}"));
            }
        }
    }

    fn do_save_project(&mut self) {
        if self.cobolt_project.is_none() { return; }

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
                self.output.push_status(format!("Project saved → {}", path.display()));
            }
            Err(e) => {
                self.output.push_status(format!("Save project failed: {e}"));
            }
        }
    }

    fn do_package_project(&mut self) {
        if self.cobolt_project.is_none() || self.project_path.is_none() {
            self.output.push_status(
                "Open or create a project first (File → New/Open Project).",
            );
            return;
        }

        let zip_name = format!(
            "{}.zip",
            self.cobolt_project.as_ref().unwrap()
                .project.name
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
        let proj_snap = proj.clone();
        match package_project(&proj_snap, &proj_dir, &out_zip) {
            Ok(count) => {
                self.output.push_status(format!(
                    "Packaged {count} files → {}",
                    out_zip.display()
                ));
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
        let Some(proj_path) = &self.project_path else {
            self.output.push_status("Open or create a project first (File → New/Open Project).");
            return;
        };

        let manifest = proj_path.clone();
        self.output.clear();
        self.output.push_status("── Building binary …  (this may take a minute) ──");

        // Run the build on a background thread; collect result via a one-shot channel.
        let (tx, rx) = std::sync::mpsc::channel::<Result<cobolt_compiler::BuildResult, String>>();
        std::thread::spawn(move || {
            let opts = BuildOptions { verbose: false, workspace_root: None };
            let result = build_project(&manifest, &opts)
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });

        // Poll the channel each frame; store the receiver so update() can drain it.
        self.pending_build_rx = Some(rx);
    }

    fn do_add_file_to_project(&mut self, kind: FileKind) {
        let proj_dir = match &self.project_path {
            Some(p) => p.parent().unwrap_or(p.as_path()).to_owned(),
            None    => {
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
            FileKind::Source =>
                spec.filter("COBOL Source", &["cbl", "cob", "cpy"]),
            FileKind::Form =>
                spec.filter("RustCOBOL Form", &["cfrm"]),
            FileKind::Documentation =>
                spec.filter("Documentation",
                    &["md", "markdown", "txt", "rst", "adoc", "pdf", "html", "htm"]),
            FileKind::Asset => spec, // no filter → every file selectable
        };

        self.begin_file_dialog(FileRequest::AddFile(kind), spec);
    }

    /// Add the chosen file to the open project under `kind`'s category. A file
    /// **outside** the project directory is **copied into** a category subfolder
    /// (`src/`, `forms/`, `assets/`, `docs/`) so it becomes part of the project
    /// (and ships with the build); a file already inside is tracked in place.
    fn add_file_to_project_path(&mut self, kind: FileKind, path: PathBuf) {
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
                    FileKind::Source        => "src",
                    FileKind::Form          => "forms",
                    FileKind::Asset         => "assets",
                    FileKind::Documentation => "docs",
                };
                let Some(fname) = path.file_name() else {
                    self.output.push_status("Invalid file name.");
                    return;
                };
                let dest_dir = proj_dir.join(subdir);
                if let Err(e) = std::fs::create_dir_all(&dest_dir) {
                    self.output.push_status(format!("Could not create {subdir}/: {e}"));
                    return;
                }
                let dest = dest_dir.join(fname);
                if let Err(e) = std::fs::copy(&path, &dest) {
                    self.output.push_status(format!("Could not import file: {e}"));
                    return;
                }
                self.output.push_status(format!(
                    "Imported {} → {}/{}",
                    fname.to_string_lossy(), subdir, fname.to_string_lossy()
                ));
                relative_to(&dest, &proj_dir)
                    .unwrap_or_else(|| format!("{subdir}/{}", fname.to_string_lossy()))
            }
        };

        let category = match kind {
            FileKind::Source        => Category::CommonCode,
            FileKind::Form          => Category::Forms,
            FileKind::Asset         => Category::Assets,
            FileKind::Documentation => Category::Documentation,
        };
        if let Some(proj) = &mut self.cobolt_project {
            proj.add_file_to(&rel, category);
        }
        self.do_save_project();
    }

    fn do_remove_file_from_project(&mut self, rel: String) {
        if let Some(proj) = &mut self.cobolt_project {
            proj.remove_file(&rel);
        }
        self.do_save_project();
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
                self.designers.push((path, DesignerPanel::new(form)));
            }
            Err(e) => {
                self.output.push_status(format!("Failed to open form: {e}"));
            }
        }
    }

    fn do_save_designer(&mut self, idx: usize) {
        if idx >= self.designers.len() { return; }
        let path      = self.designers[idx].0.clone();
        let form_name = self.designers[idx].1.form.name.clone();
        let result    = save_form(&self.designers[idx].1.form, &path);
        match result {
            Ok(()) => {
                self.designers[idx].1.dirty = false;
                self.output.push_status(format!("Saved {}", path.display()));
                self.forms_list.refresh();
                // Reflect the change in the tree + regenerate the backend COBOL.
                self.after_form_saved(&path);
                // Show the "Form <name> saved" alert (i18n template filled at render time).
                self.save_alert_msg = Some(form_name);
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
        if idx >= self.designers.len() { return; }

        // Resolve the paragraph name from the binding, or derive it the same way
        // codegen does, so the lookup matches the generated source.
        let para = {
            let form = &self.designers[idx].1.form;
            if ctrl_id.is_empty() {
                form.form_events.iter()
                    .find(|e| e.event == event)
                    .map(|e| e.paragraph.clone())
                    .unwrap_or_else(|| cobolt_forms::model::derive_paragraph_name("", event))
            } else {
                form.controls.iter()
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

        let card = crate::theme::glass_panel_frame(
            ctx.style().visuals.panel_fill, self.current_theme());
        egui::CentralPanel::default().frame(card).show(ctx, |ui| {
            let Some(st) = &mut self.inspect else { return; };
            ui.horizontal(|ui| {
                ui.heading(format!("⚙ {}", st.designer.form.name));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tr.inspect_close).clicked() { close = true; }
                    if ui.button(tr.inspect_open_designer).clicked() { open_designer = true; }
                });
            });
            match &st.ctrl_id {
                Some(id) => { ui.label(egui::RichText::new(id).strong().monospace()); }
                None     => { ui.label(egui::RichText::new(tr.inspect_form_props).italics()); }
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
            // Event editing needs the full designer.
            if action.open_event_editor.is_some() || action.open_event_in_code.is_some() {
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
            if let Some(p) = path { self.load_form_from_path(p); }
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
        let Ok(form) = load_form(cfrm_path) else { return; };
        let cbl = cfrm_path.with_extension("cbl");
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
        self.output.push_status(format!("Regenerated {}", cbl.display()));
    }

    /// The active IDE colour theme (from the open project, or the default).
    fn current_theme(&self) -> &'static crate::theme::Theme {
        let id = self.cobolt_project.as_ref().map(|p| p.ide.theme.as_str()).unwrap_or("");
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
        let Some(abs) = self.bg_image_abs_path() else { return; };

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
        let Some((_, tex)) = &self.bg_texture else { return; };

        let screen   = ctx.screen_rect();
        let tex_size = tex.size_vec2();
        if tex_size.x <= 0.0 || tex_size.y <= 0.0 {
            return;
        }
        // Cover: scale up so the image fills the window, centred.
        let s  = (screen.width() / tex_size.x).max(screen.height() / tex_size.y);
        let dw = tex_size.x * s;
        let dh = tex_size.y * s;
        let ox = (screen.width()  - dw) * 0.5;
        let oy = (screen.height() - dh) * 0.5;
        let dest = egui::Rect::from_min_size(screen.min + egui::vec2(ox, oy), egui::vec2(dw, dh));
        let uv   = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));

        // Draw the image (cover) over the opaque floor, scaled by
        // `background_opacity` so the texture shows through more or less.
        let img_a = (opacity as f32 / 100.0 * 255.0) as u8;
        ctx.layer_painter(egui::LayerId::background())
            .image(tex.id(), dest, uv, egui::Color32::from_white_alpha(img_a));
    }

    /// The IDE appearance settings dialog: colour theme + background image with
    /// an opacity (transparency) control. All values are per project.
    fn show_settings_window(&mut self, ctx: &Context, tr: &Tr) {
        if !self.show_settings {
            return;
        }
        let mut open = true;
        let has_project = self.cobolt_project.is_some();
        let mut pick_bg = false;
        let mut clear_bg = false;
        let mut new_theme: Option<String> = None;
        let mut new_opacity: Option<u8> = None;

        egui::Window::new(format!("⚙ {}", tr.settings_title))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                if !has_project {
                    ui.label(tr.settings_no_project);
                    return;
                }
                let proj = self.cobolt_project.as_ref().unwrap();
                let cur_theme = crate::theme::theme_by_id(&proj.ide.theme);

                // ── Colour theme ──────────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.label(tr.settings_theme);
                    egui::ComboBox::from_id_salt("ide_theme_combo")
                        .selected_text(cur_theme.name)
                        .width(210.0)
                        .show_ui(ui, |ui| {
                            for t in crate::theme::THEMES {
                                if ui.selectable_label(t.id == cur_theme.id, t.name).clicked() {
                                    new_theme = Some(t.id.to_string());
                                }
                            }
                        });
                });
                ui.add_space(8.0);
                ui.separator();

                // ── Background image ──────────────────────────────────────────
                ui.label(egui::RichText::new(tr.settings_background).strong());
                let cur_bg = proj.ide.background_image.clone();
                let shown = if cur_bg.is_empty() { tr.settings_bg_none.to_string() } else { cur_bg.clone() };
                ui.label(egui::RichText::new(shown).monospace().small());
                ui.horizontal(|ui| {
                    if ui.button(tr.settings_bg_browse).clicked() { pick_bg = true; }
                    if !cur_bg.is_empty() && ui.button(tr.settings_bg_clear).clicked() {
                        clear_bg = true;
                    }
                });
                ui.add_space(4.0);
                let mut op = proj.ide.background_opacity as i32;
                ui.horizontal(|ui| {
                    ui.label(tr.settings_bg_opacity);
                    if ui.add(egui::Slider::new(&mut op, 0..=100).suffix("%")).changed() {
                        new_opacity = Some(op.clamp(0, 100) as u8);
                    }
                });
            });

        // ── Apply (in-memory; persisted once the dialog closes) ───────────────
        let mut dirty = false;
        if let Some(id) = new_theme {
            if let Some(p) = &mut self.cobolt_project { p.ide.theme = id; dirty = true; }
        }
        if let Some(o) = new_opacity {
            if let Some(p) = &mut self.cobolt_project { p.ide.background_opacity = o; dirty = true; }
        }
        if clear_bg {
            if let Some(p) = &mut self.cobolt_project { p.ide.background_image.clear(); dirty = true; }
            self.bg_texture = None;
        }
        if dirty {
            self.do_save_project();
        }
        if pick_bg {
            self.begin_file_dialog(
                FileRequest::PickBackgroundImage,
                crate::file_dialog::DialogSpec::open()
                    .filter("Images", &["png", "jpg", "jpeg", "bmp", "gif"]),
            );
        }
        if !open {
            self.show_settings = false;
        }
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
        if idx >= self.designers.len() { return; }
        let cbl_path = self.designers[idx].0.with_extension("cbl");
        let cobol    = generate(&self.designers[idx].1.form);
        match std::fs::write(&cbl_path, &cobol) {
            Ok(()) => {
                self.output.push_status(format!("Generated {}", cbl_path.display()));
                // Auto-add to project if applicable.
                let proj_dir = self.project_path.as_ref()
                    .and_then(|p| p.parent()).map(|p| p.to_owned());
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
                if p.exists() { return p; }
                // Create it alongside the project if it doesn't exist yet.
                return p;
            }
        }
        // Fall back to the workspace root (look for Cargo.toml with [workspace]).
        let mut dir = std::env::current_dir().unwrap_or_default();
        loop {
            let candidate = dir.join("BUGS.md");
            if candidate.exists() { return candidate; }
            let toml = dir.join("Cargo.toml");
            if toml.exists() {
                if let Ok(t) = std::fs::read_to_string(&toml) {
                    if t.contains("[workspace]") { return candidate; }
                }
            }
            match dir.parent() {
                Some(p) => dir = p.to_owned(),
                None    => break,
            }
        }
        std::path::PathBuf::from("BUGS.md")
    }

    fn show_save_alert(&mut self, ctx: &Context) {
        let form_name = match &self.save_alert_msg {
            Some(n) => n.clone(),
            None    => return,
        };
        let tr  = self.lang.tr();
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
                }
            });

        if !open {
            self.save_alert_msg = None;
        }
    }

    fn show_report_bug_dialog(&mut self, ctx: &Context) {
        if !self.report_bug.open { return; }

        let mut open = true;
        egui::Window::new("🐛 Report a Problem")
            .collapsible(false)
            .resizable(true)
            .min_width(420.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Describe the problem so it can be tracked and fixed:");
                ui.add_space(6.0);

                egui::Grid::new("bug_form").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
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
                            .hint_text("Steps to reproduce, what went wrong, what you expected…")
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
                                self.report_bug.feedback = Some(
                                    format!("✅ Saved to {}  — next scan will pick it up.", path.display())
                                );
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

        if !open { self.report_bug.open = false; }
    }

    // ── Keyboard shortcuts (main window) ─────────────────────────────────────

    fn handle_shortcuts(&mut self, ctx: &Context) {
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S))) {
            self.do_save();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::R)))
            && !self.runner.is_running()
        {
            self.do_run();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::O))) {
            self.do_open();
        }
    }

    // ── Dialogs ───────────────────────────────────────────────────────────────

    fn show_new_project_dialog(&mut self, ctx: &Context) {
        if !self.new_project.open { return; }
        let tr = self.lang.tr();
        let mut open = true;
        egui::Window::new(tr.dlg_new_project)
            .collapsible(false).resizable(false).open(&mut open)
            .show(ctx, |ui| {
                egui::Grid::new("npg").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
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
                    if ui.button(tr.dlg_create_dots).clicked() { self.create_new_project(); }
                    if ui.button(tr.dlg_cancel).clicked()      { self.new_project.open = false; }
                });
            });
        if !open { self.new_project.open = false; }
    }

    fn show_new_form_dialog(&mut self, ctx: &Context) {
        if !self.new_form.open { return; }
        let tr = self.lang.tr();
        let mut open = true;
        egui::Window::new(tr.dlg_new_form)
            .collapsible(false).resizable(false).open(&mut open)
            .show(ctx, |ui| {
                egui::Grid::new("nfg").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
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
                    if ui.button(tr.dlg_create).clicked() { self.create_new_form(); }
                    if ui.button(tr.dlg_cancel).clicked() { self.new_form.open = false; }
                });
            });
        if !open { self.new_form.open = false; }
    }

    fn create_new_form(&mut self) {
        let w: u32 = self.new_form.width.parse().unwrap_or(640);
        let h: u32 = self.new_form.height.parse().unwrap_or(480);
        let mut form = Form::new(
            self.new_form.form_name.clone(),
            self.new_form.title.clone(),
            w, h,
        );
        form.background_color = "00000000".into(); // transparent — matches IDE glass

        let default_name = format!("{}.cfrm", self.new_form.form_name.to_lowercase());
        self.begin_file_dialog(
            FileRequest::NewForm(Box::new(form)),
            crate::file_dialog::DialogSpec::save()
                .filter("RustCOBOL Form", &["cfrm"])
                .file_name(default_name),
        );
    }

    /// Save a freshly-created form to `path`, register it, and open its designer.
    fn save_new_form_to(&mut self, form: Form, path: PathBuf) {
        if let Err(e) = save_form(&form, &path) {
            self.output.push_status(format!("Could not save new form: {e}"));
            return;
        }
        if let Some(parent) = path.parent() {
            self.forms_list.set_root(parent);
            if self.cobolt_project.is_none() { self.project.set_root(parent); }
        }
        // Auto-add to project
        let proj_dir = self.project_path.as_ref()
            .and_then(|p| p.parent()).map(|p| p.to_owned());
        if let Some(dir) = proj_dir {
            if let Some(rel) = relative_to(&path, &dir) {
                if let Some(proj) = &mut self.cobolt_project { proj.add_file(&rel); }
                self.do_save_project();
            }
        }
        self.designers.push((path, DesignerPanel::new(form)));
        self.new_form.open = false;
    }

    // ── Async file-dialog plumbing ──────────────────────────────────────────────

    /// Open an app-level file dialog without blocking the event loop and record
    /// what to do with the result (applied by `apply_file_result`).
    fn begin_file_dialog(&mut self, request: FileRequest, spec: crate::file_dialog::DialogSpec) {
        self.pending_file = Some(request);
        crate::file_dialog::begin(APP_FILE_KEY, spec);
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
            FileRequest::CreateProject  => self.create_new_project_at(path),
            FileRequest::OpenProject    => self.open_project_at(path),
            FileRequest::SaveProject    => {
                self.project_path = Some(path);
                self.do_save_project();
            }
            FileRequest::PackageProject => self.package_project_to(path),
            FileRequest::AddFile(kind)  => self.add_file_to_project_path(kind, path),
            FileRequest::OpenForm       => self.load_form_from_path(path),
            FileRequest::NewForm(form)  => self.save_new_form_to(*form, path),
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
    use egui::{Rounding, Shadow, Stroke, Visuals, style::WidgetVisuals};
    use egui::Color32;

    // Publish the editor palette for this theme so the syntax layouter picks it up.
    crate::theme::set_active(theme);

    let mut v = if theme.dark { Visuals::dark() } else { Visuals::light() };

    // Panels keep a consistent semi-opaque fill (the background painter draws a
    // matching base so the area *outside* the panes looks the same as the panes).
    // ── Theme palette ─────────────────────────────────────────────────────
    let bg_panel    = theme.bg_panel;
    let bg_widget   = theme.bg_widget;
    let bg_hover    = theme.bg_hover;
    let bg_active   = theme.bg_active;
    let bg_extreme  = theme.bg_extreme;
    let accent      = theme.accent;
    let border_dim  = theme.border_dim;
    let border_hi   = theme.border_hi;
    let text_dim    = theme.text_dim;
    let text_bright = theme.text_bright;

    // ── Window / panel fills ──────────────────────────────────────────────
    v.window_fill      = bg_panel;
    v.panel_fill       = bg_panel;
    v.faint_bg_color   = theme.faint_bg;
    v.extreme_bg_color = bg_extreme;
    v.code_bg_color    = theme.code_bg;

    // ── Window chrome ─────────────────────────────────────────────────────
    v.window_stroke   = Stroke::new(1.0, border_hi);
    v.window_shadow   = Shadow {
        offset: Vec2::new(0.0, 10.0),
        blur:   40.0,
        spread: 0.0,
        color:  Color32::from_rgba_unmultiplied(0, 0, 0, 100),
    };
    v.window_rounding = Rounding::same(12.0);
    v.window_highlight_topmost = false;

    // ── Widget states ─────────────────────────────────────────────────────
    let make_widget = |bg: Color32, stroke_c: Color32, text: Color32| WidgetVisuals {
        weak_bg_fill: bg,
        bg_fill:      bg,
        bg_stroke:    Stroke::new(1.0, stroke_c),
        fg_stroke:    Stroke::new(1.5, text),
        rounding:     Rounding::same(8.0),
        expansion:    0.0,
    };

    v.widgets.noninteractive = make_widget(bg_widget, border_dim, text_dim);
    v.widgets.inactive       = make_widget(bg_widget, border_dim, text_dim);
    v.widgets.hovered        = make_widget(bg_hover,  border_hi,  text_bright);
    v.widgets.active         = make_widget(bg_active, accent,     Color32::WHITE);
    v.widgets.open           = make_widget(bg_hover,  border_hi,  text_bright);

    // Keep separators / dividers very faint (the prominent light-grey lines were
    // too noisy). Use the theme's dim border colour.
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, theme.border_dim);

    // ── Selection ─────────────────────────────────────────────────────────
    v.selection.bg_fill = theme.selection;
    v.selection.stroke  = Stroke::new(1.0, accent);

    // ── Text / decorations ────────────────────────────────────────────────
    v.override_text_color     = None;
    v.hyperlink_color         = theme.hyperlink;
    v.warn_fg_color           = theme.warn;
    v.error_fg_color          = theme.error;

    ctx.set_visuals(v);

    // Polished spacing + fonts 50 % larger (absolute → idempotent each frame).
    // Roomier rows/padding for a less cramped, more professional feel.
    use egui::{FontFamily, FontId, TextStyle};
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing      = egui::Vec2::new(8.0, 8.0);
    style.spacing.button_padding    = egui::Vec2::new(12.0, 7.0);
    style.spacing.indent            = 20.0;
    style.spacing.window_margin     = egui::Margin::same(12.0);
    style.spacing.menu_margin       = egui::Margin::same(8.0);
    style.spacing.interact_size.y   = 30.0;
    // No vertical indent guide lines in the tree (the grey lines looked noisy).
    style.visuals.indent_has_left_vline = false;
    style.text_styles = [
        (TextStyle::Small,     FontId::new(13.5, FontFamily::Proportional)),
        (TextStyle::Body,      FontId::new(18.75, FontFamily::Proportional)),
        (TextStyle::Button,    FontId::new(18.75, FontFamily::Proportional)),
        (TextStyle::Heading,   FontId::new(27.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(18.0, FontFamily::Monospace)),
    ].into();
    ctx.set_style(style);
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
            ctx.layer_painter(egui::LayerId::background())
                .rect_filled(ctx.screen_rect(), 0.0, floor);
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
        if self.runner.is_finished() { self.runner.clear(); }

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
            if dirty { ctx.request_repaint(); }
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
                }
                Ok(Err(e)) => {
                    self.output.push_status(format!("❌ Build failed: {e}"));
                    self.pending_build_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint(); // keep polling
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.output.push_status("❌ Build thread disconnected unexpectedly.");
                    self.pending_build_rx = None;
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
        self.show_report_bug_dialog(ctx);
        self.show_save_alert(ctx);
        self.show_settings_window(ctx, &tr);

        // ── Menu bar ─────────────────────────────────────────────────────────
        let has_project = self.cobolt_project.is_some();
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
                    if ui.button(tr.menu_new_form).clicked()    { self.new_form.open = true;   ui.close_menu(); }
                    ui.separator();
                    if ui.button(tr.menu_save).clicked() { self.do_save(); ui.close_menu(); }
                    ui.separator();
                    if ui.button(tr.menu_quit).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

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

                // ── Help / Bug report ────────────────────────────────────────
                ui.menu_button("Help", |ui| {
                    if ui.button("🐛 Report a Problem…")
                        .on_hover_text("Report a bug or issue — saved to BUGS.md and picked up by the next scan")
                        .clicked()
                    {
                        self.report_bug.open_for("IDE Editor");
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
        match toolbar::show(ctx, &self.runner, &tr, &mut self.lang, compilable, debuggable) {
            ToolbarAction::Run   => self.do_run(),
            ToolbarAction::Stop  => self.do_stop(),
            ToolbarAction::Debug => self.do_debug(),
            ToolbarAction::Build => self.do_build_binary(),
            ToolbarAction::Check => self.do_check(),
            ToolbarAction::Open  => self.do_open(),
            ToolbarAction::Save  => self.do_save(),
            ToolbarAction::Settings => self.show_settings = true,
            ToolbarAction::None  => {}
        }

        // ── Debug toolbar addon (inline, in a secondary top panel) ────────────
        egui::TopBottomPanel::top("debug_toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let can_debug = self.editor.active_source().is_some()
                    && !self.runner.is_running();

                if !self.debug_active {
                    if ui.add_enabled(can_debug, egui::Button::new(tr.dbg_debug))
                        .on_hover_text("Start a debug session for the active file")
                        .clicked()
                    {
                        self.do_debug();
                    }
                } else {
                    // Already in debug session — show stop.
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
                }
            });
        });

        // ── Debugger side panel ───────────────────────────────────────────────
        if self.debug_active {
            if let Some(cmd) = self.debugger.show(ctx, &tr, self.debug_runner.is_running()) {
                self.debug_runner.send_cmd(cmd);
            }
        }

        // ── Code workspace panels ─────────────────────────────────────────────
        self.output.show(ctx, &tr);

        let proj_events = self.project.show(ctx, self.cobolt_project.as_ref(), &tr);
        for ev in proj_events {
            match ev {
                ProjectPanelEvent::Open(path) => {
                    self.inspect = None; // a file takes over the Main Pane
                    self.open_in_editor(path);
                }
                ProjectPanelEvent::OpenDesigner(path) => self.load_form_from_path(path),
                ProjectPanelEvent::InspectForm(path)  => self.open_inspect(path, None),
                ProjectPanelEvent::InspectControl { form, ctrl_id } =>
                    self.open_inspect(form, Some(ctrl_id)),
                ProjectPanelEvent::OpenEventCode { form, paragraph } => {
                    self.inspect = None;
                    // Open the form's read-only generated COBOL at the event's paragraph.
                    self.pending_open_in_editor = Some(form.with_extension("cbl"));
                    self.pending_goto_paragraph  = Some(paragraph);
                }
                ProjectPanelEvent::Select(_) => {} // applied inside the panel
                ProjectPanelEvent::Add(kind)  => self.do_add_file_to_project(kind),
                ProjectPanelEvent::Remove(rel) => self.do_remove_file_from_project(rel),
            }
        }

        // Main Pane: the inline form/control inspector, else the code editor.
        if self.inspect.is_some() {
            self.show_inspector(ctx, &tr);
        } else {
            self.editor.show(ctx);
        }

        // Tree semaphore: the active file, if edited since its last check, goes
        // back to yellow ("changed — not tested").
        let active = self.editor.active_source().map(|(p, c)| (p.clone(), Self::content_hash(c)));
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
            let title  = {
                let (path, d) = &self.designers[idx];
                let stem  = path.file_stem().and_then(|s| s.to_str()).unwrap_or("form");
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

        // ── Preview viewports (one per open form that has preview enabled) ───────
        for idx in 0..self.designers.len() {
            if !self.designers[idx].1.show_preview { continue; }

            let vp_id = ViewportId::from_hash_of(("preview", &self.designers[idx].0));
            let title  = {
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
            let title  = format!("▶ {}", self.form_runtimes[i].form_title);
            let fw = self.form_runtimes[i].form_width  as f32;
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

        if self.runner.is_running() || !self.form_runtimes.is_empty() {
            ctx.request_repaint();
        }
    }
}

// ── Preview window contents ───────────────────────────────────────────────────

impl CoboltApp {
    fn show_preview_window(&mut self, ctx: &Context, idx: usize) {
        use cobolt_forms::model::ControlType as CT;
        use egui::{Color32, Pos2, Rect, Stroke, Vec2};
        use crate::panels::designer::{draw_glass, draw_glass_circle, AnimState, anim_transform, draw_chart_preview, glass_combo_header, glass_combo_popup, GlassComboAction};

        if idx >= self.designers.len() { return; }

        // ── Animation tick ────────────────────────────────────────────────────
        {
            let d = &mut self.designers[idx].1;
            let now = std::time::Instant::now();
            let dt = d.preview_last_frame.map(|t| now.duration_since(t).as_secs_f32()).unwrap_or(0.0);
            d.preview_last_frame = Some(now);

            // Auto-start OnFormLoad animations once on first open.
            // A sentinel key "__init__" marks that initialisation has run,
            // even if no OnFormLoad animations exist (avoids re-running every frame).
            let needs_init = !d.preview_anim_states.contains_key("__init__");
            if needs_init {
                d.preview_anim_states.insert("__init__".to_owned(), AnimState::new("__init__"));
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
                let anim_meta: std::collections::HashMap<String, u64> = d.form.controls.iter()
                    .flat_map(|c| c.animations.iter()
                        .map(move |a| (format!("{}:{}", c.id, a.name), a.duration_ms)))
                    .collect();
                let mut need_repaint = false;
                for (key, state) in d.preview_anim_states.iter_mut() {
                    if !state.playing { continue; }
                    if state.delay_remaining > 0.0 {
                        state.delay_remaining -= dt;
                        if state.delay_remaining < 0.0 { state.delay_remaining = 0.0; }
                        need_repaint = true;
                        continue;
                    }
                    let dur = anim_meta.get(key).copied().unwrap_or(400) as f32 / 1000.0;
                    if dur <= 0.0 { state.stop(); continue; }
                    state.t += dt / dur;
                    if state.t >= 1.0 { state.t = 1.0; state.playing = false; }
                    need_repaint = true;
                }
                if need_repaint { ctx.request_repaint(); }
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
            // Widget backgrounds — translucent frosted glass
            let glass_fill   = Color32::from_rgba_premultiplied(50, 55, 90, 55);
            let glass_stroke = egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(180,180,230,80));
            visuals.widgets.noninteractive.bg_fill   = glass_fill;
            visuals.widgets.noninteractive.bg_stroke = glass_stroke;
            visuals.widgets.inactive.bg_fill         = glass_fill;
            visuals.widgets.inactive.bg_stroke       = glass_stroke;
            visuals.widgets.hovered.bg_fill          = Color32::from_rgba_premultiplied(70, 80, 130, 80);
            visuals.widgets.hovered.bg_stroke        = egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(200,210,255,120));
            visuals.widgets.active.bg_fill           = Color32::from_rgba_premultiplied(90,100,160,100);
            visuals.widgets.active.bg_stroke         = egui::Stroke::new(1.5, Color32::from_rgba_premultiplied(220,230,255,160));
            // Rounding
            let rnd = egui::Rounding::same(8.0);
            visuals.widgets.noninteractive.rounding = rnd;
            visuals.widgets.inactive.rounding        = rnd;
            visuals.widgets.hovered.rounding         = rnd;
            visuals.widgets.active.rounding          = rnd;
            // Text
            visuals.override_text_color = Some(Color32::from_rgb(230, 235, 255));
            // Window / panel background — transparent so the OS shows through
            visuals.panel_fill       = Color32::TRANSPARENT;
            visuals.window_fill      = Color32::TRANSPARENT;
            visuals.extreme_bg_color = Color32::from_rgba_premultiplied(20, 20, 40, 180);
            ctx.set_visuals(visuals);
        }

        // Read-only snapshot of everything we need for rendering (avoids borrow conflicts).
        let bg_color: Color32;
        let preview_anim_snap: std::collections::HashMap<String, f32>;
        let form_w: f32;
        let form_h: f32;
        {
            let d = &self.designers[idx].1;
            form_w = d.form.width as f32;
            form_h = d.form.height as f32;
            preview_anim_snap = d.preview_anim_states.iter()
                .map(|(k, s)| (k.clone(), s.t))
                .collect();

            let raw_hex = d.form.background_color.trim().to_owned();
            // Strip optional '#' prefix, take first 6 hex chars (ignore alpha byte).
            let bg_hex: &str = {
                let s = if raw_hex.starts_with('#') { &raw_hex[1..] } else { &raw_hex };
                if s.len() >= 6 { &s[..6] } else { s }
            };
            let transparency = d.form.transparency.clamp(0, 100);
            let bg_alpha = (255.0 * (1.0 - transparency as f32 / 100.0)) as u8;
            bg_color = if bg_hex.len() == 6 {
                let r = u8::from_str_radix(&bg_hex[0..2], 16).unwrap_or(20);
                let g = u8::from_str_radix(&bg_hex[2..4], 16).unwrap_or(22);
                let b = u8::from_str_radix(&bg_hex[4..6], 16).unwrap_or(45);
                // If the colour is pure black (000000) treat it as the default dark navy
                // so a transparent/unset background still looks like a proper form window.
                let (r, g, b) = if r == 0 && g == 0 && b == 0 { (20, 22, 45) } else { (r, g, b) };
                Color32::from_rgba_premultiplied(
                    (r as f32 * bg_alpha as f32 / 255.0) as u8,
                    (g as f32 * bg_alpha as f32 / 255.0) as u8,
                    (b as f32 * bg_alpha as f32 / 255.0) as u8,
                    bg_alpha,
                )
            } else {
                // No background colour set — use a solid dark navy so the form is always visible.
                Color32::from_rgba_premultiplied(
                    (20.0 * bg_alpha as f32 / 255.0) as u8,
                    (22.0 * bg_alpha as f32 / 255.0) as u8,
                    (45.0 * bg_alpha as f32 / 255.0) as u8,
                    bg_alpha.max(200), // minimum opacity so form is never invisible
                )
            };
        }

        // Eagerly load the background image into the designer's texture cache.
        {
            let bg_path = self.designers[idx].1.form.background_image.clone();
            if !bg_path.is_empty() {
                self.designers[idx].1.load_image(&bg_path, ctx);
            }
        }

        let d     = &mut self.designers[idx].1;
        let form  = &d.form;
        let state = &mut d.preview_state;

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(bg_color))
            .show(ctx, |ui| {
                let origin    = ui.min_rect().min;
                let form_rect = ui.max_rect();

                // ── Background image ──────────────────────────────────────────
                let bg_path = &form.background_image;
                if !bg_path.is_empty() {
                    if let Some(tex) = d.image_cache.get(bg_path).and_then(|o| o.as_ref()) {
                        use cobolt_forms::model::BgImageMode;
                        let tex_size = tex.size_vec2();
                        let tex_id   = tex.id();
                        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                        // Form transparency fades the background image too.
                        let img_a = ((100 - form.transparency.min(100)) as f32 / 100.0 * 255.0) as u8;
                        let tint = egui::Color32::from_white_alpha(img_a);
                        let dest = match form.bg_image_mode {
                            BgImageMode::Fill => {
                                let sx = form_rect.width()  / tex_size.x;
                                let sy = form_rect.height() / tex_size.y;
                                let s  = sx.max(sy);
                                let dw = tex_size.x * s;
                                let dh = tex_size.y * s;
                                let ox = (form_rect.width()  - dw) * 0.5;
                                let oy = (form_rect.height() - dh) * 0.5;
                                egui::Rect::from_min_size(form_rect.min + egui::vec2(ox, oy), egui::vec2(dw, dh))
                            }
                            BgImageMode::Fit => {
                                let sx = form_rect.width()  / tex_size.x;
                                let sy = form_rect.height() / tex_size.y;
                                let s  = sx.min(sy);
                                let dw = tex_size.x * s;
                                let dh = tex_size.y * s;
                                let ox = (form_rect.width()  - dw) * 0.5;
                                let oy = (form_rect.height() - dh) * 0.5;
                                egui::Rect::from_min_size(form_rect.min + egui::vec2(ox, oy), egui::vec2(dw, dh))
                            }
                            BgImageMode::Tile => form_rect, // simple stretch for now
                            BgImageMode::Stretch | BgImageMode::Center => form_rect,
                        };
                        ui.painter().image(tex_id, dest, uv, tint);
                    }
                }

                // Collect controls sorted by z_order so they render back-to-front.
                let mut sorted: Vec<&cobolt_forms::model::Control> = form.controls.iter().collect();
                sorted.sort_by_key(|c| c.z_order);

                for ctrl in &sorted {
                    if !ctrl.visible { continue; }

                    // Apply animation transform for this control (OnFormLoad etc.)
                    let (adx, ady, scale, anim_alpha) = ctrl.animations.iter()
                        .find_map(|a| {
                            let key = format!("{}:{}", ctrl.id, a.name);
                            preview_anim_snap.get(&key).map(|&t| anim_transform(a, form_w, form_h, t))
                        })
                        .unwrap_or((0.0, 0.0, 1.0, 1.0));

                    let r = &ctrl.rect;
                    let base_rect = Rect::from_min_size(
                        Pos2::new(origin.x + r.x as f32 + adx, origin.y + r.y as f32 + ady),
                        Vec2::new(r.w as f32, r.h as f32),
                    );
                    // Scale the rect about its centre (matches `draw_control`), so
                    // zoom/spin/flip animations actually resize the widget in preview.
                    let screen_rect = crate::panels::designer::scale_rect_about_center(base_rect, scale);

                    let ctrl_id = egui::Id::new(("preview_ctrl", ctrl.id.as_str()));
                    let cur_val = state.entry(ctrl.id.clone()).or_insert_with(|| {
                        use cobolt_forms::model::ControlType as CT2;
                        match ctrl.control_type {
                            // TextBox holds its value in "Text", not "Caption" or "Value".
                            CT2::TextBox => ctrl.get_prop("Text")
                                .map(|v| v.as_str().to_owned())
                                .unwrap_or_default(),
                            // ComboBox / ListBox / Slider / ProgressBar use "Value".
                            CT2::ComboBox | CT2::ListBox | CT2::Slider | CT2::ProgressBar
                            | CT2::NumericUpDown | CT2::CheckBox | CT2::RadioButton =>
                                ctrl.get_prop("Value")
                                    .map(|v| v.as_str().to_owned())
                                    .unwrap_or_default(),
                            // Everything else (Label etc.) uses Caption.
                            _ => ctrl.get_prop("Caption")
                                .map(|v| v.as_str().to_owned())
                                .unwrap_or_default(),
                        }
                    });

                    // alpha_mul: combine animation alpha, control opacity and transparency
                    let ctrl_transparency = ctrl.get_prop("Transparency")
                        .map(|v| v.as_i64()).unwrap_or(0).clamp(0, 100);
                    let ctrl_opacity = ctrl.get_prop("Opacity")
                        .map(|v| (v.as_i64() as f32 / 100.0).clamp(0.0, 1.0))
                        .unwrap_or(1.0);
                    let alpha_mul = (anim_alpha * ctrl_opacity * (1.0 - ctrl_transparency as f32 / 100.0)).clamp(0.0, 1.0);

                    let painter = ui.painter_at(screen_rect);
                    let enabled = ctrl.enabled;

                    match ctrl.control_type {
                        CT::Button => {
                            let label = ctrl.get_prop("Caption").map(|v| v.as_str().to_owned())
                                .unwrap_or_else(|| ctrl.id.clone());
                            // Interact first so we know the pressed state before drawing.
                            let resp = ui.interact(screen_rect, ctrl_id, egui::Sense::click());
                            let pressed = resp.is_pointer_button_down_on();
                            let hovered = resp.hovered();

                            // Pressed  → darker, inset feel (shrink rect 1px, richer colour)
                            // Hovered  → slightly lighter
                            // Normal   → standard glass blue
                            // iPhone Liquid Glass pressed effect (same as Run Form)
                            let (btn_color, border_a) = if pressed {
                                (Color32::from_rgb(15, 35, 95), 220u8)
                            } else if hovered {
                                (Color32::from_rgb(60, 90, 160), 160u8)
                            } else {
                                (Color32::from_rgb(40, 60, 120), 100u8)
                            };
                            let draw_rect = if pressed { screen_rect.shrink(1.5) } else { screen_rect };
                            draw_glass(&painter, draw_rect, btn_color, 10.0, false, alpha_mul);

                            // Top-edge specular (brighter when pressed)
                            let spec_alpha = if pressed { 80u8 } else { 40u8 };
                            let spec = egui::Rect::from_min_size(
                                draw_rect.min + Vec2::new(4.0, 2.0),
                                Vec2::new(draw_rect.width() - 8.0, 4.0),
                            );
                            painter.rect_filled(spec, 3.0,
                                Color32::from_rgba_premultiplied(200, 220, 255, spec_alpha));

                            // Border
                            painter.rect_stroke(draw_rect, 10.0, egui::Stroke::new(
                                1.0, Color32::from_rgba_premultiplied(130, 170, 255, border_a)));

                            // Label — drop 1px when pressed
                            let label_pos = if pressed {
                                draw_rect.center() + Vec2::new(0.0, 1.0)
                            } else {
                                screen_rect.center()
                            };
                            painter.text(label_pos, egui::Align2::CENTER_CENTER, &label,
                                egui::FontId::proportional(12.0),
                                Color32::from_rgb(230, 235, 255));
                        }
                        CT::Label => {
                            let text = ctrl.get_prop("Caption").map(|v| v.as_str().to_owned())
                                .unwrap_or_else(|| ctrl.id.clone());
                            let font_size = ctrl.get_prop("FontSize").map(|v| v.as_i64() as f32).unwrap_or(12.0);
                            let font_name = ctrl.get_prop("FontName").map(|v| v.as_str().to_owned()).unwrap_or_default();
                            // Resolve ForeColor; default to near-white so labels are always readable.
                            let fore = ctrl.get_prop("ForeColor").map(|v| v.as_str().to_owned()).unwrap_or_default();
                            let hex = if fore.starts_with('#') { &fore[1..] } else { &fore };
                            let (fr, fg, fb) = if hex.len() >= 6 {
                                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(230);
                                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(235);
                                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
                                // If pure black, treat as default text colour (designer default is #000000)
                                if r == 0 && g == 0 && b == 0 { (230u8, 235u8, 255u8) } else { (r, g, b) }
                            } else { (230, 235, 255) };
                            let a = (alpha_mul * 255.0) as u8;
                            let text_color = Color32::from_rgba_premultiplied(
                                (fr as f32 * alpha_mul) as u8,
                                (fg as f32 * alpha_mul) as u8,
                                (fb as f32 * alpha_mul) as u8,
                                a,
                            );
                            painter.text(screen_rect.min, egui::Align2::LEFT_TOP,
                                &text, crate::fonts::font_id(ui.ctx(), &font_name, font_size), text_color);
                        }
                        CT::TextBox => {
                            draw_glass(&painter, screen_rect, Color32::from_rgb(30, 40, 80), 8.0, false, alpha_mul);
                            let resp = ui.put(screen_rect,
                                egui::TextEdit::singleline(cur_val)
                                    .id(ctrl_id)
                                    .frame(false)
                                    .text_color(Color32::from_rgb(230, 235, 255)));
                            let _ = resp;
                        }
                        CT::CheckBox => {
                            let mut checked = cur_val == "true" || cur_val == "1";
                            let label = ctrl.get_prop("Caption").map(|v| v.as_str().to_owned())
                                .unwrap_or_else(|| ctrl.id.clone());
                            if ui.put(screen_rect, egui::Checkbox::new(&mut checked, &label)).changed() {
                                *cur_val = if checked { "true".into() } else { "false".into() };
                            }
                        }
                        CT::ComboBox => {
                            // Header drawn here; popup rendered in second pass below.
                            let items: Vec<String> = ctrl.get_prop("Items")
                                .map(|v| v.as_str().lines().map(|l| l.to_owned()).collect())
                                .unwrap_or_default();
                            let sel = if cur_val.is_empty() {
                                items.first().cloned().unwrap_or_default()
                            } else { cur_val.clone() };
                            let is_open = *d.preview_combo_open.get(&ctrl.id).unwrap_or(&false);
                            if glass_combo_header(&painter, ui, screen_rect, ctrl_id,
                                                  &sel, is_open, enabled, alpha_mul) {
                                let e = d.preview_combo_open.entry(ctrl.id.clone()).or_insert(false);
                                *e = !*e;
                            }
                        }
                        CT::Slider => {
                            let min_v = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0) as f32;
                            let max_v = ctrl.get_prop("Maximum").map(|v| v.as_i64()).unwrap_or(100).max(1) as f32;
                            let mut fval: f32 = cur_val.parse().unwrap_or(min_v);
                            if ui.put(screen_rect,
                                egui::Slider::new(&mut fval, min_v..=max_v).show_value(false))
                                .changed()
                            {
                                *cur_val = fval.to_string();
                            }
                        }
                        CT::ProgressBar => {
                            let min_v = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0) as f32;
                            let max_v = ctrl.get_prop("Maximum").map(|v| v.as_i64()).unwrap_or(100).max(1) as f32;
                            let fval: f32 = cur_val.parse().unwrap_or(min_v);
                            let pct = ((fval - min_v) / (max_v - min_v)).clamp(0.0, 1.0);
                            // Glass track
                            draw_glass(&painter, screen_rect, Color32::from_rgb(30, 80, 60), 6.0, false, alpha_mul * 0.6);
                            let fill_w = screen_rect.width() * pct;
                            let fill_rect = Rect::from_min_size(screen_rect.min, Vec2::new(fill_w, screen_rect.height()));
                            draw_glass(&painter, fill_rect, Color32::from_rgb(40, 180, 120), 6.0, false, alpha_mul);
                        }
                        CT::PictureBox => {
                            let image_path = ctrl.get_prop("ImagePath").map(|v| v.as_str().to_owned()).unwrap_or_default();
                            let size_mode  = ctrl.get_prop("SizeMode").map(|v| v.as_str().to_owned()).unwrap_or_default();
                            let show_frame = ctrl.get_prop("ShowFrame").map(|v| v.as_bool()).unwrap_or(true);
                            draw_picturebox(&painter, screen_rect, &image_path, &size_mode, show_frame, alpha_mul);
                        }
                        CT::Animator => {
                            let source = ctrl.get_prop("Source").map(|v| v.as_str().to_owned()).unwrap_or_default();
                            let auto    = ctrl.get_prop("AutoPlay").map(|v| v.as_bool()).unwrap_or(true);
                            let looping = ctrl.get_prop("Loop").map(|v| v.as_bool()).unwrap_or(true);
                            let size_mode = ctrl.get_prop("SizeMode").map(|v| v.as_str().to_owned())
                                .unwrap_or_else(|| "Fit".into());
                            let key = format!("{}|{}", ctrl.id, source.trim());
                            crate::panels::designer::draw_animator(
                                &painter, screen_rect, &key, source.trim(), auto, looping, &size_mode, alpha_mul, false,
                            );
                        }
                        CT::Line => {
                            let p1 = screen_rect.left_top();
                            let p2 = screen_rect.right_bottom();
                            let a = (alpha_mul * 200.0) as u8;
                            painter.line_segment([p1, p2], Stroke::new(2.0,
                                Color32::from_rgba_premultiplied(a, a, a, a)));
                        }
                        CT::Shape => {
                            let shape_type = ctrl.get_prop("ShapeType").map(|v| v.as_str().to_owned())
                                .unwrap_or_else(|| "Rectangle".into());
                            let fill_color = ctrl.get_prop("FillColor").map(|v| {
                                let hex = v.as_str();
                                if hex.len() >= 6 {
                                    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(128);
                                    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(128);
                                    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(128);
                                    Color32::from_rgb(r, g, b)
                                } else { Color32::GRAY }
                            }).unwrap_or(Color32::GRAY);
                            match shape_type.as_str() {
                                "Circle" | "Ellipse" => {
                                    let radius = screen_rect.width().min(screen_rect.height()) / 2.0;
                                    draw_glass_circle(&painter, screen_rect.center(), radius, fill_color, false, alpha_mul);
                                }
                                "Triangle" => {
                                    let pts = vec![
                                        Pos2::new(screen_rect.center().x, screen_rect.min.y),
                                        Pos2::new(screen_rect.max.x, screen_rect.max.y),
                                        Pos2::new(screen_rect.min.x, screen_rect.max.y),
                                    ];
                                    let a = (alpha_mul * 255.0) as u8;
                                    let fc = Color32::from_rgba_premultiplied(
                                        (fill_color.r() as f32 * alpha_mul) as u8,
                                        (fill_color.g() as f32 * alpha_mul) as u8,
                                        (fill_color.b() as f32 * alpha_mul) as u8,
                                        a,
                                    );
                                    painter.add(egui::Shape::convex_polygon(pts, fc, Stroke::NONE));
                                }
                                "RoundRect" => { draw_glass(&painter, screen_rect, fill_color, 10.0, false, alpha_mul); }
                                _           => { draw_glass(&painter, screen_rect, fill_color, 0.0,  false, alpha_mul); }
                            }
                        }
                        CT::GroupBox => {
                            let title = ctrl.get_prop("Caption").map(|v| v.as_str().to_owned())
                                .unwrap_or_default();
                            // Glass tinted group box border
                            draw_glass(&painter, screen_rect, Color32::from_rgb(40, 45, 80), 6.0, false, alpha_mul * 0.4);
                            painter.rect_stroke(screen_rect, 6.0,
                                Stroke::new(1.0, Color32::from_rgba_premultiplied(160,165,220,100)));
                            painter.text(
                                Pos2::new(screen_rect.min.x + 8.0, screen_rect.min.y),
                                egui::Align2::LEFT_CENTER,
                                &title, egui::FontId::proportional(11.0),
                                Color32::from_rgba_premultiplied(210, 215, 255, 220));
                        }
                        // ── Chart controls — reuse the designer's chart renderer ──
                        CT::BarChart | CT::LineChart | CT::PieChart
                        | CT::AreaChart | CT::ScatterChart | CT::DonutChart => {
                            let a = (alpha_mul * 255.0) as u8;
                            draw_chart_preview(&painter, ctrl, screen_rect, a, alpha_mul, true, false);
                        }

                        CT::Timer | CT::AgentObject | CT::SqlDatabase | CT::RestClient => {
                            // Non-visual in preview — skip
                        }
                        _ => {
                            // Generic fallback: glass box with ID label
                            let base = if enabled {
                                Color32::from_rgb(50, 55, 100)
                            } else {
                                Color32::from_rgb(35, 35, 55)
                            };
                            draw_glass(&painter, screen_rect, base, 6.0, false,
                                alpha_mul * if enabled { 1.0 } else { 0.5 });
                            painter.text(screen_rect.center(), egui::Align2::CENTER_CENTER,
                                &ctrl.id, egui::FontId::proportional(10.0),
                                Color32::from_rgba_premultiplied(200, 205, 240, 200));
                        }
                    }
                }

                // ── Second pass: open ComboBox popups (always on top) ────────
                let open_combos: Vec<(String, Vec<String>, egui::Rect, String)> = {
                    let mut v = Vec::new();
                    for ctrl in form.controls.iter() {
                        if ctrl.control_type != cobolt_forms::model::ControlType::ComboBox { continue; }
                        if !*d.preview_combo_open.get(&ctrl.id).unwrap_or(&false) { continue; }
                        let items: Vec<String> = ctrl.get_prop("Items")
                            .map(|v| v.as_str().lines().map(|l| l.to_owned()).collect())
                            .unwrap_or_default();
                        let r = &ctrl.rect;
                        let header = egui::Rect::from_min_size(
                            Pos2::new(origin.x + r.x as f32, origin.y + r.y as f32),
                            Vec2::new(r.w as f32, r.h as f32),
                        );
                        let cur = state.get(&ctrl.id).cloned().unwrap_or_default();
                        v.push((ctrl.id.clone(), items, header, cur));
                    }
                    v
                };
                for (ctrl_id_str, items, header_rect, cur_sel) in open_combos {
                    match glass_combo_popup(ui, &ctrl_id_str, header_rect, &items, &cur_sel) {
                        Some(GlassComboAction::Select(val)) => {
                            state.insert(ctrl_id_str.clone(), val);
                            d.preview_combo_open.insert(ctrl_id_str, false);
                        }
                        Some(GlassComboAction::Close) => {
                            d.preview_combo_open.insert(ctrl_id_str, false);
                        }
                        None => {}
                    }
                }
            });
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
        use egui::{Color32, Pos2, Rect, Stroke, Vec2};
        use crate::panels::designer::{draw_glass, draw_glass_circle, glass_combo_header, glass_combo_popup, GlassComboAction};

        if idx >= self.form_runtimes.len() { return; }

        // Apply glass visuals identical to the preview window.
        {
            let mut vis = ctx.style().visuals.clone();
            let gf = Color32::from_rgba_premultiplied(50, 55, 90, 55);
            let gs = egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(180,180,230,80));
            vis.widgets.noninteractive.bg_fill   = gf;
            vis.widgets.noninteractive.bg_stroke = gs;
            vis.widgets.inactive.bg_fill         = gf;
            vis.widgets.inactive.bg_stroke       = gs;
            vis.widgets.hovered.bg_fill          = Color32::from_rgba_premultiplied(70,80,130,80);
            vis.widgets.hovered.bg_stroke        = egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(200,210,255,120));
            vis.widgets.active.bg_fill           = Color32::from_rgba_premultiplied(90,100,160,100);
            vis.widgets.active.bg_stroke         = egui::Stroke::new(1.5, Color32::from_rgba_premultiplied(220,230,255,160));
            let rnd = egui::Rounding::same(8.0);
            vis.widgets.noninteractive.rounding  = rnd;
            vis.widgets.inactive.rounding        = rnd;
            vis.widgets.hovered.rounding         = rnd;
            vis.widgets.active.rounding          = rnd;
            vis.override_text_color              = Some(Color32::from_rgb(230, 235, 255));
            vis.panel_fill                       = Color32::TRANSPARENT;
            vis.window_fill                      = Color32::TRANSPARENT;
            vis.extreme_bg_color                 = Color32::from_rgba_premultiplied(20,20,40,180);
            ctx.set_visuals(vis);
        }

        // Snapshot what we need (avoids borrow-split issues with self).
        let ctrl_order = self.form_runtimes[idx].ctrl_order.clone();
        let bg_image   = self.form_runtimes[idx].background_image.clone();
        let bg_mode    = self.form_runtimes[idx].bg_image_mode;
        let bg_transp  = self.form_runtimes[idx].transparency;

        // Derive the form background colour from the stored form metadata.
        let bg_color = {
            let rt = &self.form_runtimes[idx];
            let hex = &rt.background_color;
            let bg_alpha = (255.0 * (1.0 - rt.transparency as f32 / 100.0)) as u8;
            if hex.len() >= 6 {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(20);
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(22);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(45);
                Color32::from_rgba_premultiplied(
                    (r as f32 * bg_alpha as f32 / 255.0) as u8,
                    (g as f32 * bg_alpha as f32 / 255.0) as u8,
                    (b as f32 * bg_alpha as f32 / 255.0) as u8,
                    bg_alpha,
                )
            } else {
                Color32::from_rgba_premultiplied(20, 22, 45, bg_alpha.max(180))
            }
        };

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(bg_color))
            .show(ctx, |ui| {
                let origin = ui.min_rect().min;

                // ── Form background image (cached in egui memory) ─────────────
                if !bg_image.is_empty() {
                    use cobolt_forms::model::BgImageMode;
                    let form_rect = ui.max_rect();
                    let cid = egui::Id::new(("runform_bg", bg_image.as_str()));
                    let tex = match ui.data(|d| d.get_temp::<Option<egui::TextureHandle>>(cid)) {
                        Some(t) => t,
                        None => {
                            let l = load_image_texture(ui.ctx(), &bg_image);
                            ui.data_mut(|d| d.insert_temp(cid, l.clone()));
                            l
                        }
                    };
                    if let Some(t) = tex {
                        let ts = t.size_vec2();
                        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                        let dest = match bg_mode {
                            BgImageMode::Fill | BgImageMode::Fit => {
                                let sx = form_rect.width() / ts.x;
                                let sy = form_rect.height() / ts.y;
                                let s = if matches!(bg_mode, BgImageMode::Fill) { sx.max(sy) } else { sx.min(sy) };
                                let (dw, dh) = (ts.x * s, ts.y * s);
                                egui::Rect::from_min_size(
                                    form_rect.min + egui::vec2((form_rect.width() - dw) * 0.5, (form_rect.height() - dh) * 0.5),
                                    egui::vec2(dw, dh))
                            }
                            // Stretch / Tile / Center → fill the form rect.
                            _ => form_rect,
                        };
                        // Form transparency fades the background image too.
                        let img_a = ((100 - bg_transp.min(100)) as f32 / 100.0 * 255.0) as u8;
                        ui.painter().image(t.id(), dest, uv, egui::Color32::from_white_alpha(img_a));
                    }
                }

                for meta in &ctrl_order {
                    let rt = &mut self.form_runtimes[idx];
                    let state = rt.ctrl_state.entry(meta.id.clone()).or_default().clone();

                    if !state.visible { continue; }

                    let r = &meta.rect;
                    let screen_rect = Rect::from_min_size(
                        Pos2::new(origin.x + r.x as f32, origin.y + r.y as f32),
                        Vec2::new(r.w as f32, r.h as f32),
                    );
                    let painter  = ui.painter_at(screen_rect);
                    let ctrl_id  = egui::Id::new(("run_ctrl", meta.id.as_str()));
                    let enabled  = state.enabled;
                    let alpha    = if enabled { 1.0f32 } else { 0.45f32 };

                    match meta.control_type {
                        // Widgets sharing the one runtime renderer (also used by tests).
                        CT::Button | CT::CheckBox | CT::TextBox | CT::Slider
                        | CT::DateTimePicker | CT::DataGrid | CT::RadioButton
                        | CT::NumericUpDown | CT::TabControl | CT::TreeView | CT::Splitter
                        | CT::MenuBar | CT::ToolBar | CT::StatusBar
                        | CT::BarChart | CT::LineChart | CT::PieChart
                        | CT::AreaChart | CT::ScatterChart | CT::DonutChart => {
                            let out = render_run_control(
                                ui, screen_rect, ctrl_id, &meta.id,
                                meta.control_type.clone(), &state, enabled, alpha,
                            );
                            for (k, v) in &out.prop_updates {
                                rt.ctrl_state.entry(meta.id.clone()).or_default()
                                    .props.insert(k.clone(), v.clone());
                            }
                            for e in out.events {
                                rt.send_event(e);
                            }
                        }

                        CT::Label => {
                            let text      = state.get("Caption").to_owned();
                            let font_size = state.get("FontSize").parse::<f32>().unwrap_or(12.0);
                            let font_name = state.get("FontName").to_owned();
                            // Resolve ForeColor; fall back to near-white.
                            let hex = state.get("ForeColor");
                            let (fr, fg, fb) = if hex.len() >= 7 && hex.starts_with('#') {
                                (
                                    u8::from_str_radix(&hex[1..3], 16).unwrap_or(230),
                                    u8::from_str_radix(&hex[3..5], 16).unwrap_or(235),
                                    u8::from_str_radix(&hex[5..7], 16).unwrap_or(255),
                                )
                            } else { (230, 235, 255) };
                            let a = (alpha * 255.0) as u8;
                            painter.text(
                                screen_rect.min, egui::Align2::LEFT_TOP,
                                &text, crate::fonts::font_id(ui.ctx(), &font_name, font_size),
                                Color32::from_rgba_premultiplied(
                                    (fr as f32 * alpha) as u8,
                                    (fg as f32 * alpha) as u8,
                                    (fb as f32 * alpha) as u8,
                                    a,
                                ),
                            );
                        }

                        CT::ComboBox => {
                            // Glass ComboBox header — same shared renderer as Preview.
                            // Popup is rendered in the second pass after all controls.
                            let cur = state.get("Value").to_owned();
                            let sel = if cur.is_empty() {
                                state.get("Items").lines().next().unwrap_or("").to_owned()
                            } else { cur };
                            let is_open = *rt.combo_open.get(&meta.id).unwrap_or(&false);
                            if glass_combo_header(&painter, ui, screen_rect, ctrl_id,
                                                  &sel, is_open, enabled, alpha) {
                                let e = rt.combo_open.entry(meta.id.clone()).or_insert(false);
                                *e = !*e;
                            }
                        }

                        CT::ListBox => {
                            // ScrollArea cannot be placed with ui.put() either.
                            draw_glass(&painter, screen_rect, Color32::from_rgb(30,40,80), 6.0, false, alpha);
                            let items: Vec<String> = state.get("Items")
                                .lines().map(|l| l.to_owned()).collect();
                            let cur = state.get("Value").to_owned();
                            let meta_id = meta.id.clone();
                            ui.allocate_ui_at_rect(screen_rect, |ui| {
                                ui.set_enabled(enabled);
                                egui::ScrollArea::vertical()
                                    .id_salt(ctrl_id)
                                    .max_height(screen_rect.height())
                                    .show(ui, |ui| {
                                        for item in &items {
                                            if ui.selectable_label(&cur == item, item).clicked() {
                                                if let Some(s) = rt.ctrl_state.get_mut(&meta_id) {
                                                    s.props.insert("Value".to_owned(), item.clone());
                                                }
                                                rt.send_event(FormEvent::change(&meta_id, item.as_str()));
                                            }
                                        }
                                    });
                            });
                        }

                        CT::ProgressBar => {
                            let min_v = state.get("Minimum").parse::<f32>().unwrap_or(0.0);
                            let max_v = state.get("Maximum").parse::<f32>().unwrap_or(100.0).max(min_v + 1.0);
                            let val   = state.get("Value").parse::<f32>().unwrap_or(min_v);
                            let pct   = ((val - min_v) / (max_v - min_v)).clamp(0.0, 1.0);
                            draw_glass(&painter, screen_rect, Color32::from_rgb(30,80,60), 6.0, false, alpha * 0.6);
                            let fw = screen_rect.width() * pct;
                            let fr = Rect::from_min_size(screen_rect.min, Vec2::new(fw, screen_rect.height()));
                            draw_glass(&painter, fr, Color32::from_rgb(40,180,120), 6.0, false, alpha);
                        }

                        CT::GroupBox => {
                            let title = state.get("Caption").to_owned();
                            draw_glass(&painter, screen_rect, Color32::from_rgb(40,45,80), 6.0, false, alpha * 0.4);
                            painter.rect_stroke(screen_rect, 6.0,
                                Stroke::new(1.0, Color32::from_rgba_premultiplied(160,165,220,100)));
                            painter.text(
                                Pos2::new(screen_rect.min.x + 8.0, screen_rect.min.y),
                                egui::Align2::LEFT_CENTER, &title,
                                egui::FontId::proportional(11.0),
                                Color32::from_rgba_premultiplied(210,215,255,220),
                            );
                        }

                        CT::Shape => {
                            let shape_type = state.get("ShapeType").to_owned();
                            let hex = state.get("FillColor");
                            let fill = if hex.len() >= 6 {
                                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(128);
                                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(128);
                                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(128);
                                Color32::from_rgb(r,g,b)
                            } else { Color32::GRAY };
                            match shape_type.as_str() {
                                "Circle" | "Ellipse" => {
                                    let radius = screen_rect.width().min(screen_rect.height()) / 2.0;
                                    draw_glass_circle(&painter, screen_rect.center(), radius, fill, false, alpha);
                                }
                                "Triangle" => {
                                    let pts = vec![
                                        Pos2::new(screen_rect.center().x, screen_rect.min.y),
                                        Pos2::new(screen_rect.max.x, screen_rect.max.y),
                                        Pos2::new(screen_rect.min.x, screen_rect.max.y),
                                    ];
                                    let a = (alpha * 255.0) as u8;
                                    painter.add(egui::Shape::convex_polygon(pts,
                                        Color32::from_rgba_premultiplied(
                                            (fill.r() as f32 * alpha) as u8,
                                            (fill.g() as f32 * alpha) as u8,
                                            (fill.b() as f32 * alpha) as u8, a),
                                        Stroke::NONE));
                                }
                                "RoundRect" => draw_glass(&painter, screen_rect, fill, 10.0, false, alpha),
                                _           => draw_glass(&painter, screen_rect, fill, 0.0,  false, alpha),
                            }
                        }

                        CT::Line => {
                            let a = (alpha * 200.0) as u8;
                            painter.line_segment(
                                [screen_rect.left_top(), screen_rect.right_bottom()],
                                Stroke::new(2.0, Color32::from_rgba_premultiplied(a,a,a,a)),
                            );
                        }

                        CT::PictureBox => {
                            let image_path = state.get("ImagePath").trim().to_owned();
                            let size_mode  = state.get("SizeMode").to_owned();
                            let show_frame = !matches!(state.get("ShowFrame"), "0" | "false" | "False");
                            draw_picturebox(&painter, screen_rect, &image_path, &size_mode, show_frame, alpha);
                        }

                        CT::Animator => {
                            let source = state.get("Source").trim().to_owned();
                            let auto    = !matches!(state.get("AutoPlay"), "0" | "false" | "False");
                            let looping = !matches!(state.get("Loop"),     "0" | "false" | "False");
                            let size_mode = {
                                let s = state.get("SizeMode");
                                if s.is_empty() { "Fit".to_owned() } else { s.to_owned() }
                            };
                            let key = format!("{}|{}", meta.id, source);
                            crate::panels::designer::draw_animator(
                                &painter, screen_rect, &key, &source, auto, looping, &size_mode, alpha, false,
                            );
                        }

                        CT::Timer | CT::AgentObject | CT::SqlDatabase | CT::RestClient => {
                            // Non-visual — skip rendering.
                        }

                        _ => {
                            // Generic fallback: glass box with caption.
                            let base = if enabled { Color32::from_rgb(50,55,100) }
                                       else       { Color32::from_rgb(35,35,55)  };
                            draw_glass(&painter, screen_rect, base, 6.0, false, alpha);
                            let label = state.get("Caption").to_owned();
                            let label = if label.is_empty() { format!("{:?}", meta.control_type) } else { label };
                            painter.text(screen_rect.center(), egui::Align2::CENTER_CENTER,
                                &label, egui::FontId::proportional(10.0),
                                Color32::from_rgba_premultiplied(200,205,240,200));
                        }
                    }
                }

                // ── Second pass: open ComboBox popups (always on top) ─────────
                let open_run_combos: Vec<(String, Vec<String>, egui::Rect, String)> = {
                    let rt = &self.form_runtimes[idx];
                    let mut v = Vec::new();
                    for meta in &ctrl_order {
                        if meta.control_type != cobolt_forms::ControlType::ComboBox { continue; }
                        if !*rt.combo_open.get(&meta.id).unwrap_or(&false) { continue; }
                        let items: Vec<String> = rt.ctrl_state.get(&meta.id)
                            .map(|s| s.get("Items").lines().map(|l| l.to_owned()).collect())
                            .unwrap_or_default();
                        let r = &meta.rect;
                        let header = egui::Rect::from_min_size(
                            Pos2::new(origin.x + r.x as f32, origin.y + r.y as f32),
                            Vec2::new(r.w as f32, r.h as f32),
                        );
                        let cur = rt.ctrl_state.get(&meta.id)
                            .map(|s| s.get("Value").to_owned())
                            .unwrap_or_default();
                        v.push((meta.id.clone(), items, header, cur));
                    }
                    v
                };
                for (cid, items, header_rect, cur_sel) in open_run_combos {
                    match glass_combo_popup(ui, &cid, header_rect, &items, &cur_sel) {
                        Some(GlassComboAction::Select(val)) => {
                            let rt = &mut self.form_runtimes[idx];
                            if let Some(s) = rt.ctrl_state.get_mut(&cid) {
                                s.props.insert("Value".to_owned(), val.clone());
                            }
                            rt.send_event(FormEvent::change(&cid, &val));
                            rt.combo_open.insert(cid, false);
                        }
                        Some(GlassComboAction::Close) => {
                            self.form_runtimes[idx].combo_open.insert(cid, false);
                        }
                        None => {}
                    }
                }
            });
    }
}

// ── Designer window contents ──────────────────────────────────────────────────

impl CoboltApp {
    fn show_designer_window(&mut self, ctx: &Context, idx: usize, tr: &Tr) {
        if idx >= self.designers.len() { return; }

        // Re-apply glass visuals to this designer viewport every frame.
        // The preview viewport calls ctx.set_visuals() which is globally shared
        // in egui 0.29, so we must restore them here each frame.
        apply_glass_visuals(ctx, self.current_theme());

        // The designer viewport is OPAQUE (unlike the transparent preview/run
        // windows), so any area not covered by a panel shows a white clear — the
        // "white band" below the toolbar. Composite the semi-transparent glass
        // panel colour over white into an opaque grey, use it for the panels, and
        // paint it as a full-viewport backdrop so nothing ever shows white.
        let solid_panel = {
            let pf = ctx.style().visuals.panel_fill;
            let a  = pf.a() as f32 / 255.0;
            let blend = |c: u8| (c as f32 * a + 255.0 * (1.0 - a)).round() as u8;
            egui::Color32::from_rgb(blend(pf.r()), blend(pf.g()), blend(pf.b()))
        };
        {
            let mut v = ctx.style().visuals.clone();
            v.panel_fill  = solid_panel;
            v.window_fill = solid_panel;
            ctx.set_visuals(v);
        }
        ctx.layer_painter(egui::LayerId::background())
            .rect_filled(ctx.screen_rect(), 0.0, solid_panel);

        // ── Unsaved-changes confirmation dialog ───────────────────────────────
        if self.designers[idx].1.close_confirm {
            let stem = self.designers[idx].0
                .file_stem().and_then(|s| s.to_str()).unwrap_or("form");
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
                            self.designers[idx].1.close_confirm   = false;
                            self.do_save_designer(idx);
                            self.do_generate_cobol(idx);
                            self.designers[idx].1.close_requested = true;
                        }
                        if ui.button(tr.close_discard).clicked() {
                            self.designers[idx].1.close_confirm   = false;
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
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S))) {
            self.do_save_designer(idx);
            self.do_generate_cobol(idx);
        }

        // ── Left sidebar: Forms list + Toolbox (full height — reaches top) ────
        // Rendered BEFORE the toolbar so it occupies the full window height on the
        // left; the toolbar below then fills only the area to its right.
        // Collect open paths as owned so no borrow lingers on self.designers.
        let open_paths: Vec<PathBuf> = self.designers.iter().map(|(p, _)| p.clone()).collect();
        let open_path_refs: Vec<&Path> = open_paths.iter().map(|p| p.as_path()).collect();

        let (form_to_open, toolbox_action) = egui::SidePanel::left(format!("dl_{idx}"))
            .resizable(true)
            .default_width(150.0)
            .show(ctx, |ui| {
                let to_open = self.forms_list.show(ui, &open_path_refs, tr);
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
                let tb = self.designers[idx].1.toolbox.show(ui, tr);
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
                let can_undo    = d.can_undo();
                let can_redo    = d.can_redo();
                let has_sel     = !d.selected_ids.is_empty();
                let has_multi   = d.selected_ids.len() >= 2;
                let preview_on  = d.show_preview;
                let grid_on     = d.show_grid;
                let glass_on    = d.glass_mode;
                let fp_active   = matches!(d.format_painter,
                    crate::panels::designer::FormatPainter::WaitingForTarget { .. });
                let form_path   = self.designers[idx].0.clone();
                let form_running = self.form_runtimes.iter()
                    .any(|rt| rt.form_path == form_path && rt.is_running());

                // Icons (left) + language selector (right) on a SINGLE centred row.
                // They must share one row: two stacked rows (icon row + a separate
                // selector row) make the content ~75px tall, which egui uses as the
                // panel height — overriding `exact_height(50)`.
                let mut action = DesignerToolbarAction::None;
                ui.horizontal_centered(|ui| {
                    action = draw_icon_toolbar(ui, can_undo, can_redo, has_sel, has_multi,
                        preview_on, grid_on, glass_on, form_running, fp_active);
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
                    DesignerToolbarAction::Undo => { self.designers[idx].1.undo(); }
                    DesignerToolbarAction::Redo => { self.designers[idx].1.redo(); }
                    DesignerToolbarAction::SaveAndGenerate => {
                        self.do_save_designer(idx);
                        self.do_generate_cobol(idx);
                    }
                    DesignerToolbarAction::GenerateOnly => { self.do_generate_cobol(idx); }
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
                    DesignerToolbarAction::ToggleGrid  => { self.designers[idx].1.show_grid  = !self.designers[idx].1.show_grid; }
                    DesignerToolbarAction::ToggleGlass => { self.designers[idx].1.glass_mode = !self.designers[idx].1.glass_mode; }
                    DesignerToolbarAction::RunForm  => { self.do_run_form(idx); }
                    DesignerToolbarAction::StopForm => {
                        let fp = self.designers[idx].0.clone();
                        self.form_runtimes.retain_mut(|rt| {
                            if rt.form_path == fp { rt.stop(); false } else { true }
                        });
                    }
                    DesignerToolbarAction::Delete        => { self.designers[idx].1.delete_selected(); }
                    DesignerToolbarAction::BringToFront  => { self.designers[idx].1.bring_to_front(); }
                    DesignerToolbarAction::SendToBack    => { self.designers[idx].1.send_to_back(); }
                    DesignerToolbarAction::BringForward  => { self.designers[idx].1.bring_forward(); }
                    DesignerToolbarAction::SendBackward  => { self.designers[idx].1.send_backward(); }
                    DesignerToolbarAction::AlignLeft     => { self.designers[idx].1.align_left(); }
                    DesignerToolbarAction::AlignRight    => { self.designers[idx].1.align_right(); }
                    DesignerToolbarAction::AlignTop      => { self.designers[idx].1.align_top(); }
                    DesignerToolbarAction::AlignBottom   => { self.designers[idx].1.align_bottom(); }
                    DesignerToolbarAction::CenterH       => { self.designers[idx].1.center_horizontal(); }
                    DesignerToolbarAction::CenterV       => { self.designers[idx].1.center_vertical(); }
                    DesignerToolbarAction::SpaceH        => { self.designers[idx].1.space_evenly_horizontal(); }
                    DesignerToolbarAction::SpaceV        => { self.designers[idx].1.space_evenly_vertical(); }
                    DesignerToolbarAction::FormatPainter   => { self.designers[idx].1.toggle_format_painter(); }
                    DesignerToolbarAction::ToggleAnimPreview => { self.designers[idx].1.play_all_form_load_anims(); }
                    DesignerToolbarAction::AutoArrange   => { self.designers[idx].1.auto_arrange_labels(); }
                    DesignerToolbarAction::ReportBug     => { self.report_bug.open_for("Form Designer"); }
                    DesignerToolbarAction::None          => {}
                }
            });

        // ── Properties panel (right) ──────────────────────────────────────────
        let sel_id = self.designers[idx].1.selected_ids.first().cloned();

        let inspector_action = egui::SidePanel::right(format!("props_{idx}"))
            .resizable(true)
            .default_width(260.0)
            .max_width(320.0)
            .show(ctx, |ui| {
                // Split-borrow: form (immutable) and properties (mutable) from DesignerPanel.
                let d = &mut self.designers[idx].1;
                let sel_ctrl = sel_id.as_deref().and_then(|id| d.form.find_control(id));
                // SAFETY: form and properties are different fields — field-level borrow split.
                let form  = &d.form       as *const cobolt_forms::Form;
                let props = &mut d.properties;
                // SAFETY: we only read *form; no aliased write to form or properties exists.
                props.show(ui, unsafe { &*form }, sel_ctrl, tr)
            })
            .inner;

        // ── Apply inspector actions ───────────────────────────────────────────
        let mut preview_triggered = false;
        for (ctrl_id, key, value) in inspector_action.set_props {
            if key.starts_with("_PreviewAnim") { preview_triggered = true; }
            self.designers[idx].1.set_property(&ctrl_id, &key, value);
        }
        // Kick off a repaint immediately so the animation loop starts on the next frame.
        if preview_triggered { ctx.request_repaint(); }
        if let Some((ctrl_id, ev_name)) = inspector_action.open_event_editor {
            self.designers[idx].1.open_event_modal(&ctrl_id, &ev_name);
        }
        if let Some((ctrl_id, ev_name)) = inspector_action.open_event_in_code {
            self.jump_to_event_code(idx, &ctrl_id, &ev_name);
        }
        for (key, value) in inspector_action.form_props {
            self.designers[idx].1.set_form_prop(&key, value);
        }

        // ── Apply toolbox drop (add control at canvas centre) ─────────────────
        if let Some(ct) = toolbox_action.dragged_type {
            let cx = (self.designers[idx].1.form.width  / 2) as i32;
            let cy = (self.designers[idx].1.form.height / 2) as i32;
            self.designers[idx].1.add_control(ct, cx, cy);
        }

        // ── Canvas (centre) ───────────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            self.designers[idx].1.show(ui);
        });
    }
}

// ── Runtime control renderer (single source of truth) ──────────────────────────
//
// Renders one running-form control as a LIVE interactive egui widget and reports
// the resulting events + control-state updates. Used by `show_running_form_window`
// and by behavioral interaction tests, so the tests exercise the real code path.
pub(crate) struct RunOutcome {
    pub events: Vec<cobolt_runtime::FormEvent>,
    pub prop_updates: Vec<(String, String)>,
}

pub(crate) fn render_run_control(
    ui: &mut egui::Ui,
    screen_rect: egui::Rect,
    ctrl_id: egui::Id,
    id: &str,
    ct: cobolt_forms::ControlType,
    state: &crate::form_runtime::CtrlState,
    enabled: bool,
    alpha: f32,
) -> RunOutcome {
    use crate::panels::designer::draw_glass;
    use cobolt_forms::ControlType as CT;
    use cobolt_runtime::FormEvent;
    use egui::{Color32, Vec2};

    let mut out = RunOutcome { events: Vec::new(), prop_updates: Vec::new() };
    let painter = ui.painter_at(screen_rect);

    match &ct {
        CT::Button => {
            let label = {
                let l = state.get("Caption").to_owned();
                if l.is_empty() { id.to_owned() } else { l }
            };
            let resp = ui.interact(screen_rect, ctrl_id, egui::Sense::click());
            let pressed = resp.is_pointer_button_down_on() && enabled;
            let hovered = resp.hovered() && enabled;
            let (btn_color, border_a) = if pressed {
                (Color32::from_rgb(15, 35, 95), 220u8)
            } else if hovered {
                (Color32::from_rgb(60, 90, 160), 160u8)
            } else {
                (Color32::from_rgb(40, 60, 120), 100u8)
            };
            let draw_rect = if pressed { screen_rect.shrink(1.5) } else { screen_rect };
            draw_glass(&painter, draw_rect, btn_color, 10.0, false, alpha);
            let spec_alpha = if pressed { 80u8 } else { 40u8 };
            let spec = egui::Rect::from_min_size(
                draw_rect.min + Vec2::new(4.0, 2.0),
                Vec2::new(draw_rect.width() - 8.0, 4.0),
            );
            painter.rect_filled(spec, 3.0, Color32::from_rgba_premultiplied(200, 220, 255, spec_alpha));
            painter.rect_stroke(draw_rect, 10.0,
                egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(130, 170, 255, border_a)));
            let label_pos = if pressed {
                draw_rect.center() + Vec2::new(0.0, 1.0)
            } else {
                screen_rect.center()
            };
            painter.text(label_pos, egui::Align2::CENTER_CENTER, &label,
                egui::FontId::proportional(12.0), Color32::from_rgb(230, 235, 255));
            if resp.clicked() && enabled {
                out.events.push(FormEvent::click(id));
            }
        }
        CT::CheckBox => {
            let label = state.get("Caption").to_owned();
            let cur = state.get("Value");
            let mut checked = cur == "true" || cur == "1";
            let resp = ui.put(screen_rect, egui::Checkbox::new(&mut checked, &label));
            if resp.changed() && enabled {
                let v = if checked { "1" } else { "0" };
                out.prop_updates.push(("Value".to_owned(), v.to_owned()));
                out.events.push(FormEvent::change(id, v));
            }
        }
        CT::TextBox => {
            draw_glass(&painter, screen_rect, Color32::from_rgb(30, 40, 80), 8.0, false, alpha);
            let mut buf = state.get("Text").to_owned();
            let resp = ui.put(screen_rect,
                egui::TextEdit::singleline(&mut buf)
                    .id(ctrl_id)
                    .frame(false)
                    .interactive(enabled)
                    .text_color(Color32::from_rgb(230, 235, 255)),
            );
            if resp.changed() {
                out.prop_updates.push(("Text".to_owned(), buf.clone()));
                out.events.push(FormEvent::change(id, &buf));
            }
            if resp.gained_focus() {
                out.events.push(FormEvent::new(id, "GotFocus"));
            }
            if resp.lost_focus() {
                out.events.push(FormEvent::new(id, "LostFocus"));
            }
        }
        CT::Slider => {
            let min_v = state.get("Minimum").parse::<f32>().unwrap_or(0.0);
            let max_v = state.get("Maximum").parse::<f32>().unwrap_or(100.0).max(min_v + 1.0);
            let cur = state.get("Value").parse::<f32>().unwrap_or(min_v);
            let mut val = cur;
            let resp = ui.put(screen_rect, egui::Slider::new(&mut val, min_v..=max_v).show_value(true));
            if resp.changed() && enabled {
                out.prop_updates.push(("Value".to_owned(), val.to_string()));
                out.events.push(FormEvent::change(id, val.to_string()));
            }
        }
        CT::DateTimePicker => {
            use egui::{vec2, Align2, FontId, Sense, Stroke};
            let white = Color32::from_rgb(230, 235, 255);
            let dim = Color32::from_rgb(150, 160, 200);

            // ── Field ────────────────────────────────────────────────────────
            draw_glass(&painter, screen_rect, Color32::from_rgb(30, 40, 80), 6.0, false, alpha);
            let val = state.get("Value").to_owned();
            let shown = if val.is_empty() { "YYYY-MM-DD".to_owned() } else { val.clone() };
            painter.text(screen_rect.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER,
                &shown, FontId::proportional(12.0), if val.is_empty() { dim } else { white });
            painter.text(screen_rect.right_center() - vec2(12.0, 0.0), Align2::CENTER_CENTER,
                "▾", FontId::proportional(13.0), Color32::from_rgb(200, 210, 255));
            let resp = ui.interact(screen_rect, ctrl_id, Sense::click());

            // ── Popup open/viewed-month state (kept in egui temp memory) ─────
            let mut cal: CalState = ui.data(|d| d.get_temp::<CalState>(ctrl_id))
                .unwrap_or_else(|| match parse_ymd(&val) {
                    Some((y, m, _)) => CalState { open: false, year: y, month: m },
                    None => CalState::default(),
                });
            if resp.clicked() && enabled {
                cal.open = !cal.open;
            }

            // ── Calendar popup (floats above other controls) ─────────────────
            if cal.open {
                let area_pos = screen_rect.left_bottom() + vec2(0.0, 2.0);
                let inner = egui::Area::new(ctrl_id.with("cal"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(area_pos)
                    .show(ui.ctx(), |ui| {
                        let area_rect = egui::Rect::from_min_size(
                            area_pos, vec2(CAL_W, CAL_GRID_Y + CAL_CELL * 6.0));
                        let p = ui.painter();
                        p.rect_filled(area_rect, 6.0, Color32::from_rgb(28, 34, 60));
                        p.rect_stroke(area_rect, 6.0,
                            Stroke::new(1.0, Color32::from_rgba_premultiplied(160, 170, 230, 150)));

                        // Month navigation row: ◀  Month YYYY  ▶
                        let prev = ui.put(egui::Rect::from_min_size(area_pos, vec2(CAL_CELL, CAL_NAV_H)),
                            egui::Button::new("◀").frame(false));
                        let next = ui.put(egui::Rect::from_min_size(area_pos + vec2(CAL_W - CAL_CELL, 0.0), vec2(CAL_CELL, CAL_NAV_H)),
                            egui::Button::new("▶").frame(false));
                        ui.painter().text(area_pos + vec2(CAL_W / 2.0, CAL_NAV_H / 2.0), Align2::CENTER_CENTER,
                            format!("{} {}", MONTHS[(cal.month.clamp(1, 12) - 1) as usize], cal.year),
                            FontId::proportional(13.0), white);
                        if prev.clicked() { if cal.month == 1 { cal.month = 12; cal.year -= 1; } else { cal.month -= 1; } }
                        if next.clicked() { if cal.month == 12 { cal.month = 1; cal.year += 1; } else { cal.month += 1; } }

                        // Weekday labels
                        for (i, wd) in ["S", "M", "T", "W", "T", "F", "S"].iter().enumerate() {
                            ui.painter().text(
                                area_pos + vec2(i as f32 * CAL_CELL + CAL_CELL / 2.0, CAL_NAV_H + CAL_WK_H / 2.0),
                                Align2::CENTER_CENTER, *wd, FontId::proportional(10.0), dim);
                        }

                        // Day grid
                        let first_wd = day_of_week(cal.year, cal.month, 1);
                        let ndays = days_in_month(cal.year, cal.month);
                        let mut picked: Option<u32> = None;
                        for day in 1..=ndays {
                            let idx = first_wd + (day - 1);
                            let (col, row) = (idx % 7, idx / 7);
                            let cell = egui::Rect::from_min_size(
                                area_pos + vec2(col as f32 * CAL_CELL, CAL_GRID_Y + row as f32 * CAL_CELL),
                                vec2(CAL_CELL, CAL_CELL));
                            if ui.put(cell, egui::Button::new(format!("{day}")).frame(false)).clicked() {
                                picked = Some(day);
                            }
                        }
                        picked
                    });

                if let Some(day) = inner.inner {
                    let date = format!("{:04}-{:02}-{:02}", cal.year, cal.month, day);
                    out.prop_updates.push(("Value".to_owned(), date.clone()));
                    out.events.push(FormEvent::change(id, date));
                    cal.open = false;
                } else if !resp.clicked() && inner.response.clicked_elsewhere() {
                    cal.open = false; // click outside closes
                }
            }
            ui.data_mut(|d| d.insert_temp(ctrl_id, cal));
        }
        CT::DataGrid => {
            use egui::{pos2, vec2, Align2, FontId, Stroke};
            let cell_fg = Color32::from_rgb(225, 230, 250);

            // Parse columns ("Name:Type" per line) and row data (TAB-separated cells).
            let cols: Vec<(String, String)> = state.get("Columns").lines()
                .filter_map(|l| {
                    let mut it = l.splitn(2, ':');
                    let name = it.next().unwrap_or("").trim().to_owned();
                    if name.is_empty() { return None; }
                    let ty = it.next().unwrap_or("string").trim().to_lowercase();
                    Some((name, ty))
                })
                .collect();
            let ncols = cols.len().max(1);
            let rows: Vec<Vec<String>> = state.get("Rows").lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.split('\t').map(|c| c.to_owned()).collect())
                .collect();
            let row_h = state.get("RowHeight").parse::<f32>().unwrap_or(22.0).clamp(14.0, 60.0);
            let col_w = screen_rect.width() / ncols as f32;

            let header_bg = parse_hex(state.get("HeaderBackColor")).unwrap_or(Color32::from_rgb(60, 66, 96));
            let header_fg = parse_hex(state.get("HeaderForeColor")).unwrap_or(Color32::from_rgb(235, 238, 250));
            let alt_bg = parse_hex(state.get("AlternatingRowColor")).unwrap_or(Color32::from_rgb(38, 44, 72));
            let grid_c = parse_hex(state.get("GridLineColor"))
                .unwrap_or(Color32::from_rgba_premultiplied(150, 160, 200, 90));

            draw_glass(&painter, screen_rect, Color32::from_rgb(26, 32, 58), 4.0, false, alpha * 0.7);

            // Header row.
            let header_rect = egui::Rect::from_min_size(screen_rect.min, vec2(screen_rect.width(), row_h));
            painter.rect_filled(header_rect, 0.0, header_bg);
            for (i, (name, _)) in cols.iter().enumerate() {
                let x = screen_rect.min.x + i as f32 * col_w;
                painter.text(pos2(x + 6.0, header_rect.center().y), Align2::LEFT_CENTER,
                    name, FontId::proportional(12.0), header_fg);
            }

            // Data rows, each cell formatted per its column type.
            for (r, row) in rows.iter().enumerate() {
                let y = screen_rect.min.y + row_h * (r as f32 + 1.0);
                if y >= screen_rect.max.y { break; } // simple clip (no scroll yet)
                let rrect = egui::Rect::from_min_size(pos2(screen_rect.min.x, y),
                    vec2(screen_rect.width(), row_h));
                if r % 2 == 1 {
                    painter.rect_filled(rrect, 0.0, alt_bg);
                }
                for (i, (_, ty)) in cols.iter().enumerate() {
                    let raw = row.get(i).map(|s| s.as_str()).unwrap_or("");
                    let x0 = screen_rect.min.x + i as f32 * col_w;

                    // Image cells: load (once, cached in egui memory) and paint — no text.
                    if matches!(ty.as_str(), "image" | "img" | "picture") {
                        let path = raw.trim();
                        let cell = egui::Rect::from_min_size(pos2(x0, rrect.min.y), vec2(col_w, row_h)).shrink(2.0);
                        if !path.is_empty() {
                            let cid = egui::Id::new(("dg_img", path));
                            let tex = match ui.data(|d| d.get_temp::<Option<egui::TextureHandle>>(cid)) {
                                Some(t) => t,
                                None => {
                                    let loaded = load_image_texture(ui.ctx(), path);
                                    ui.data_mut(|d| d.insert_temp(cid, loaded.clone()));
                                    loaded
                                }
                            };
                            if let Some(t) = tex {
                                let sz = t.size_vec2();
                                let scale = (cell.width() / sz.x).min(cell.height() / sz.y).min(1.0).max(0.01);
                                let irect = egui::Rect::from_center_size(cell.center(), sz * scale);
                                painter.image(t.id(), irect,
                                    egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                            } else {
                                // unreadable / missing → placeholder frame (still no text)
                                painter.rect_stroke(cell, 2.0, Stroke::new(1.0, Color32::from_rgb(110, 120, 160)));
                            }
                        }
                        continue;
                    }

                    let (text, right) = format_cell(raw, ty);
                    if right {
                        painter.text(pos2(x0 + col_w - 6.0, rrect.center().y), Align2::RIGHT_CENTER,
                            &text, FontId::proportional(12.0), cell_fg);
                    } else {
                        painter.text(pos2(x0 + 6.0, rrect.center().y), Align2::LEFT_CENTER,
                            &text, FontId::proportional(12.0), cell_fg);
                    }
                }
            }

            // Grid lines.
            for i in 1..ncols {
                let x = screen_rect.min.x + i as f32 * col_w;
                painter.line_segment([pos2(x, screen_rect.min.y), pos2(x, screen_rect.max.y)],
                    Stroke::new(1.0, grid_c));
            }
            painter.line_segment(
                [pos2(screen_rect.min.x, screen_rect.min.y + row_h),
                 pos2(screen_rect.max.x, screen_rect.min.y + row_h)],
                Stroke::new(1.0, grid_c));
        }
        CT::RadioButton => {
            let label = state.get("Caption").to_owned();
            let selected = matches!(state.get("Value"), "1" | "true");
            let resp = ui.put(screen_rect, egui::RadioButton::new(selected, label));
            if resp.clicked() && enabled {
                out.prop_updates.push(("Value".to_owned(), "1".to_owned()));
                out.events.push(FormEvent::change(id, "1"));
            }
        }
        CT::NumericUpDown => {
            draw_glass(&painter, screen_rect, Color32::from_rgb(30, 40, 80), 6.0, false, alpha);
            let min = state.get("Minimum").parse::<f64>().unwrap_or(0.0);
            let max = state.get("Maximum").parse::<f64>().unwrap_or(100.0);
            let step = state.get("Step").parse::<f64>().unwrap_or(1.0).max(0.0001);
            let mut val = state.get("Value").parse::<f64>().unwrap_or(min);
            let resp = ui.put(screen_rect, egui::DragValue::new(&mut val).range(min..=max).speed(step));
            if resp.changed() && enabled {
                let s = format!("{val}");
                out.prop_updates.push(("Value".to_owned(), s.clone()));
                out.events.push(FormEvent::change(id, s));
            }
        }
        CT::TabControl => {
            use egui::{pos2, vec2, Align2, FontId, Sense, Stroke};
            let tabs: Vec<String> = state.get("Tabs").lines().map(|s| s.to_owned()).collect();
            let sel = state.get("SelectedTab").parse::<usize>().unwrap_or(0);
            let tab_h = 26.0_f32;
            let content = egui::Rect::from_min_max(
                pos2(screen_rect.min.x, screen_rect.min.y + tab_h), screen_rect.max);
            draw_glass(&painter, content, Color32::from_rgb(34, 40, 70), 6.0, false, alpha * 0.6);
            let mut x = screen_rect.min.x;
            for (i, tab) in tabs.iter().enumerate() {
                let w = 84.0_f32;
                let tr = egui::Rect::from_min_size(pos2(x, screen_rect.min.y), vec2(w, tab_h));
                let active = i == sel;
                painter.rect_filled(tr, 4.0,
                    if active { Color32::from_rgb(60, 80, 140) } else { Color32::from_rgb(40, 46, 78) });
                painter.text(tr.center(), Align2::CENTER_CENTER, tab, FontId::proportional(12.0),
                    if active { Color32::from_rgb(235, 240, 255) } else { Color32::from_rgb(180, 188, 220) });
                if ui.interact(tr, ctrl_id.with(("tab", i)), Sense::click()).clicked() && enabled {
                    out.prop_updates.push(("SelectedTab".to_owned(), i.to_string()));
                    out.events.push(FormEvent::new(id, "Change"));
                }
                x += w + 2.0;
            }
            painter.rect_stroke(content, 6.0,
                Stroke::new(1.0, Color32::from_rgba_premultiplied(160, 170, 230, 110)));
        }
        CT::TreeView => {
            use egui::{pos2, Align2, FontId};
            draw_glass(&painter, screen_rect, Color32::from_rgb(28, 36, 64), 6.0, false, alpha * 0.7);
            let fg = Color32::from_rgb(220, 226, 250);
            let mut y = screen_rect.min.y + 12.0;
            for line in state.get("Items").lines() {
                if y > screen_rect.max.y { break; }
                let depth = (line.len() - line.trim_start().len()) / 2;
                let text = line.trim();
                if text.is_empty() { continue; }
                painter.text(pos2(screen_rect.min.x + 8.0 + depth as f32 * 16.0, y),
                    Align2::LEFT_CENTER, format!("• {text}"), FontId::proportional(12.0), fg);
                y += 18.0;
            }
        }
        CT::Splitter => {
            let horiz = !state.get("Orientation").starts_with('V');
            draw_glass(&painter, screen_rect, Color32::from_rgb(60, 66, 96), 3.0, false, alpha);
            let c = screen_rect.center();
            let dot = Color32::from_rgba_premultiplied(200, 210, 240, 160);
            for k in -1..=1 {
                let p = if horiz { egui::pos2(c.x + k as f32 * 5.0, c.y) }
                        else { egui::pos2(c.x, c.y + k as f32 * 5.0) };
                painter.circle_filled(p, 1.5, dot);
            }
        }
        CT::MenuBar | CT::ToolBar | CT::StatusBar => {
            use egui::{pos2, FontId};
            draw_glass(&painter, screen_rect, Color32::from_rgb(40, 46, 76), 4.0, false, alpha * 0.85);
            let fg = Color32::from_rgb(225, 230, 250);
            let mut x = screen_rect.min.x + 8.0;
            for item in state.get("Items").lines().filter(|l| !l.trim().is_empty()) {
                let galley = painter.layout_no_wrap(item.trim().to_owned(), FontId::proportional(12.0), fg);
                let w = galley.size().x;
                painter.galley(pos2(x, screen_rect.center().y - galley.size().y / 2.0), galley, fg);
                x += w + 18.0;
            }
        }
        CT::BarChart | CT::LineChart | CT::PieChart
        | CT::AreaChart | CT::ScatterChart | CT::DonutChart => {
            use cobolt_forms::model::PropValue;
            // Reconstruct a Control from the live state so we can reuse the chart painter.
            let mut ctrl = cobolt_forms::Control::new(id, ct.clone(), 0, 0);
            for (k, v) in &state.props {
                ctrl.set_prop(k.clone(), PropValue::String(v.clone()));
            }
            crate::panels::designer::draw_chart_preview(
                &painter, &ctrl, screen_rect, (alpha * 255.0) as u8, alpha, /*glass*/ true, false);
        }
        _ => {}
    }
    out
}

const MONTH_ABBR: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun",
    "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// Format a DataGrid cell value by its declared column type.
/// Returns `(display_text, right_aligned)`.
fn format_cell(raw: &str, ty: &str) -> (String, bool) {
    match ty {
        "number" | "num" | "int" | "integer" | "float" | "decimal" => {
            match raw.trim().parse::<f64>() {
                Ok(n) if n.fract() == 0.0 => (format!("{}", n as i64), true),
                Ok(n) => (format!("{n}"), true),
                Err(_) => (raw.to_owned(), true),
            }
        }
        "datetime" | "date" => match parse_ymd(raw.trim()) {
            Some((y, m, d)) => (format!("{:02} {} {}", d, MONTH_ABBR[(m.clamp(1, 12) - 1) as usize], y), false),
            None => (raw.to_owned(), false),
        },
        _ => (raw.to_owned(), false),
    }
}

/// Load an image file into an egui texture (for DataGrid image cells).
/// Caching of the returned handle is the caller's responsibility.
fn load_image_texture(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.into_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let pixels: Vec<egui::Color32> = img
        .pixels()
        .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    let ci = egui::ColorImage { size: [w, h], pixels };
    Some(ctx.load_texture(path, ci, egui::TextureOptions::LINEAR))
}

/// Load (and cache in egui memory) a PictureBox image texture, so it isn't
/// re-read from disk and re-uploaded every frame.
fn picturebox_texture(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    if path.trim().is_empty() {
        return None;
    }
    let id = egui::Id::new(("pb_img", path));
    if let Some(h) = ctx.memory(|m| m.data.get_temp::<egui::TextureHandle>(id)) {
        return Some(h);
    }
    let h = load_image_texture(ctx, path)?;
    ctx.memory_mut(|m| m.data.insert_temp(id, h.clone()));
    Some(h)
}

/// Map a PictureBox `SizeMode` to the scaling modes understood by `media_dest_rect`.
fn pic_size_mode(m: &str) -> &'static str {
    match m {
        "Stretch" | "StretchImage" => "Stretch",
        "Zoom" | "Fit"             => "Fit",
        "Fill"                     => "Fill",
        _                          => "Center", // Normal / CenterImage / AutoSize
    }
}

/// Render a PictureBox into `rect`: an optional frame (card + border) plus the
/// image, honouring `SizeMode`, opacity (`alpha_mul`), and `ShowFrame`. When the
/// frame is hidden, transparent PNG areas reveal whatever is behind the control.
fn draw_picturebox(
    painter: &egui::Painter,
    rect: egui::Rect,
    image_path: &str,
    size_mode: &str,
    show_frame: bool,
    alpha_mul: f32,
) {
    use crate::panels::designer::{draw_glass, media_dest_rect};
    if show_frame {
        draw_glass(painter, rect, egui::Color32::from_rgb(20, 30, 60), 4.0, false, alpha_mul * 0.7);
    }
    let a = (alpha_mul.clamp(0.0, 1.0) * 255.0) as u8;
    if let Some(tex) = picturebox_texture(painter.ctx(), image_path) {
        let dest = media_dest_rect(rect, tex.size_vec2(), pic_size_mode(size_mode));
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.with_clip_rect(rect).image(tex.id(), dest, uv, egui::Color32::from_white_alpha(a));
    } else if show_frame {
        painter.text(
            rect.center(), egui::Align2::CENTER_CENTER, "🖼",
            egui::FontId::proportional(32.0),
            egui::Color32::from_rgba_premultiplied(160, 160, 200, (160.0 * alpha_mul) as u8),
        );
    }
}

/// Parse `#RRGGBB` / `#RRGGBBAA` (or without `#`) into a Color32.
fn parse_hex(s: &str) -> Option<egui::Color32> {
    let h = s.trim().trim_start_matches('#');
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

// ── DateTimePicker calendar support ────────────────────────────────────────────
const CAL_CELL: f32 = 28.0;
const CAL_W: f32 = CAL_CELL * 7.0;
const CAL_NAV_H: f32 = 24.0;
const CAL_WK_H: f32 = 20.0;
const CAL_GRID_Y: f32 = CAL_NAV_H + CAL_WK_H; // area-top → first day row
const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December"];

#[derive(Clone)]
pub(crate) struct CalState {
    pub open: bool,
    pub year: i32,
    pub month: u32, // 1-12
}
impl Default for CalState {
    fn default() -> Self { Self { open: false, year: 2026, month: 6 } }
}

fn is_leap(y: i32) -> bool { (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 }

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap(y) { 29 } else { 28 },
        _ => 30,
    }
}

/// Day of week for a date, 0 = Sunday (Sakamoto's algorithm).
fn day_of_week(y: i32, m: u32, d: u32) -> u32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let yy = if m < 3 { y - 1 } else { y };
    let v = (yy + yy / 4 - yy / 100 + yy / 400 + t[(m.clamp(1, 12) - 1) as usize] + d as i32) % 7;
    ((v + 7) % 7) as u32
}

fn parse_ymd(s: &str) -> Option<(i32, u32, u32)> {
    let p: Vec<&str> = s.split('-').collect();
    if p.len() == 3 {
        Some((p[0].parse().ok()?, p[1].parse().ok()?, p[2].parse().ok()?))
    } else {
        None
    }
}

// ── Runtime interaction tests — Phase 2b ───────────────────────────────────────
// Drive the REAL `render_run_control` with simulated pointer/text input and
// assert the produced events + state updates (Button click, CheckBox toggle,
// TextBox typing/focus, Slider change).
#[cfg(test)]
mod run_interaction_tests {
    use super::*;
    use crate::form_runtime::CtrlState;
    use cobolt_forms::ControlType as CT;
    use cobolt_runtime::FormEvent;
    use egui::{pos2, vec2, Event, Modifiers, PointerButton, Pos2, RawInput, Rect};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn cs(pairs: &[(&str, &str)]) -> CtrlState {
        let mut s = CtrlState::default();
        s.visible = true;
        s.enabled = true;
        for (k, v) in pairs {
            s.props.insert((*k).to_owned(), (*v).to_owned());
        }
        s
    }

    fn press(p: Pos2) -> Event {
        Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::default() }
    }
    fn release(p: Pos2) -> Event {
        Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::default() }
    }

    /// Run a sequence of input frames against `render_run_control`, applying state
    /// updates between frames; returns (events produced, final control state).
    fn drive(ct: CT, mut state: CtrlState, rect: Rect, frames: Vec<Vec<Event>>) -> (Vec<FormEvent>, CtrlState) {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let mut all: Vec<FormEvent> = Vec::new();
        for evs in frames {
            let mut input = RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 800.0)));
            input.focused = true;
            input.events = evs;
            let out = Rc::new(RefCell::new((Vec::<FormEvent>::new(), Vec::<(String, String)>::new())));
            {
                let o = out.clone();
                let st = &state;
                ctx.run(input, |ctx| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::none())
                        .show(ctx, |ui| {
                            let id = egui::Id::new(("test_run", "W1"));
                            let r = render_run_control(ui, rect, id, "W1", ct.clone(), st, true, 1.0);
                            let mut b = o.borrow_mut();
                            b.0.extend(r.events);
                            b.1.extend(r.prop_updates);
                        });
                });
            }
            let b = out.borrow();
            all.extend(b.0.clone());
            for (k, v) in &b.1 {
                state.props.insert(k.clone(), v.clone());
            }
        }
        (all, state)
    }

    #[test]
    fn button_click_fires_click_event() {
        let rect = Rect::from_min_size(pos2(100.0, 100.0), vec2(80.0, 30.0));
        let c = rect.center();
        let (evs, _st) = drive(CT::Button, cs(&[("Caption", "OK")]), rect,
            vec![vec![], vec![Event::PointerMoved(c), press(c)], vec![Event::PointerMoved(c), release(c)]]);
        assert!(
            evs.iter().any(|e| e.event_id == "Click" && e.ctrl_id == "W1"),
            "Button click produced no Click event; got {:?}",
            evs.iter().map(|e| (e.ctrl_id.clone(), e.event_id.clone())).collect::<Vec<_>>()
        );
    }

    #[test]
    fn checkbox_toggle_fires_change() {
        let rect = Rect::from_min_size(pos2(50.0, 50.0), vec2(140.0, 24.0));
        let c = rect.center();
        let (evs, _st) = drive(CT::CheckBox, cs(&[("Caption", "On"), ("Value", "0")]), rect,
            vec![vec![], vec![Event::PointerMoved(c), press(c)], vec![Event::PointerMoved(c), release(c)]]);
        assert!(
            evs.iter().any(|e| e.event_id == "Change" && e.ctrl_id == "W1"),
            "CheckBox toggle produced no Change event"
        );
    }

    #[test]
    fn textbox_typing_fires_focus_and_change() {
        let rect = Rect::from_min_size(pos2(20.0, 20.0), vec2(200.0, 24.0));
        let c = rect.center();
        let (evs, _st) = drive(CT::TextBox, cs(&[("Text", "")]), rect, vec![
            vec![],
            vec![Event::PointerMoved(c), press(c)],
            vec![Event::PointerMoved(c), release(c)],
            vec![Event::Text("Z".to_owned())],
        ]);
        assert!(evs.iter().any(|e| e.event_id == "GotFocus"), "TextBox click produced no GotFocus");
        assert!(evs.iter().any(|e| e.event_id == "Change"), "TextBox typing produced no Change");
    }

    #[test]
    fn datetimepicker_calendar_opens_and_picks_a_day() {
        // Field at (20,20) 200×24 → calendar Area at field.left_bottom()+(0,2)=(20,46).
        let rect = Rect::from_min_size(pos2(20.0, 20.0), vec2(200.0, 24.0));
        let field_c = rect.center();

        // June 2026 starts on Monday (weekday col 1), so day 10 is at col 3, row 1.
        // Cell min = area_pos + (col*CELL, GRID_Y + row*CELL); center = +CELL/2.
        let area = pos2(20.0, 46.0);
        let (col, row) = (3.0_f32, 1.0_f32);
        let day10 = area
            + vec2(col * super::CAL_CELL + super::CAL_CELL / 2.0,
                   super::CAL_GRID_Y + row * super::CAL_CELL + super::CAL_CELL / 2.0);

        let (evs, st) = drive(
            CT::DateTimePicker,
            cs(&[("Value", "2026-06-15")]),
            rect,
            vec![
                vec![],                                              // lay out field
                vec![Event::PointerMoved(field_c), press(field_c)],  // open calendar
                vec![Event::PointerMoved(field_c), release(field_c)],
                vec![],                                              // lay out calendar grid
                vec![Event::PointerMoved(day10), press(day10)],      // pick day 10
                vec![Event::PointerMoved(day10), release(day10)],
            ],
        );

        assert!(
            evs.iter().any(|e| e.event_id == "Change" && e.ctrl_id == "W1"),
            "picking a calendar day produced no Change event"
        );
        assert_eq!(
            st.get("Value"),
            "2026-06-10",
            "calendar day pick did not set Value to the chosen date"
        );
    }

    /// Render a control once and capture the painted shapes (for display widgets).
    fn render_run_shapes(ct: CT, state: &CtrlState, rect: Rect) -> Vec<egui::Shape> {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let mut input = RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 800.0)));
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::none())
                .show(ctx, |ui| {
                    render_run_control(ui, rect, egui::Id::new(("t", "DG")), "DG", ct.clone(), state, true, 1.0);
                });
        });
        out.shapes.into_iter().map(|cs| cs.shape).collect()
    }

    fn run_texts(shapes: &[egui::Shape]) -> Vec<egui::epaint::TextShape> {
        shapes.iter().filter_map(|s| match s {
            egui::Shape::Text(t) => Some(t.clone()),
            _ => None,
        }).collect()
    }

    #[test]
    fn datagrid_renders_typed_cells() {
        let rect = Rect::from_min_size(pos2(10.0, 10.0), vec2(400.0, 120.0));
        let state = cs(&[
            ("Columns", "Name:string\nAge:number\nJoined:datetime"),
            ("Rows", "Alice\t30\t2026-06-01\nBob\t7\t2025-12-15"),
        ]);
        let ts = run_texts(&render_run_shapes(CT::DataGrid, &state, rect));
        let all: Vec<String> = ts.iter().map(|t| t.galley.text().trim().to_owned()).collect();

        // Column headers rendered.
        for h in ["Name", "Age", "Joined"] {
            assert!(all.iter().any(|s| s == h), "missing header '{h}'; got {all:?}");
        }

        let col_w = rect.width() / 3.0;
        let find = |needle: &str| ts.iter().find(|t| t.galley.text().trim() == needle).cloned();

        // string cell → left-aligned in column 0.
        let alice = find("Alice").expect("string cell 'Alice' not rendered");
        assert!(
            alice.pos.x < rect.min.x + col_w * 0.4,
            "string cell not left-aligned (x={})", alice.pos.x
        );

        // number cell → right-aligned in column 1.
        let age = find("30").expect("number cell '30' not rendered");
        assert!(
            age.pos.x > rect.min.x + col_w + col_w * 0.4,
            "number cell not right-aligned (x={})", age.pos.x
        );

        // datetime cell → reformatted "01 Jun 2026".
        assert!(
            all.iter().any(|s| s == "01 Jun 2026"),
            "datetime cell not reformatted to 'DD Mon YYYY'; got {all:?}"
        );
    }

    #[test]
    fn datagrid_renders_image_cells() {
        // Write a tiny real PNG to disk to use as the image cell value.
        let png = std::env::temp_dir().join("rcobol_dg_img_test.png");
        let mut buf = image::RgbaImage::new(4, 4);
        for p in buf.pixels_mut() {
            *p = image::Rgba([200, 40, 40, 255]);
        }
        buf.save(&png).expect("write test png");
        let path = png.to_string_lossy().to_string();

        let rect = Rect::from_min_size(pos2(10.0, 10.0), vec2(300.0, 100.0));
        let rows = format!("{path}\tAlice");
        let state = cs(&[("Columns", "Photo:image\nName:string"), ("Rows", &rows)]);

        let shapes = render_run_shapes(CT::DataGrid, &state, rect);
        let all: Vec<String> = run_texts(&shapes).iter().map(|t| t.galley.text().trim().to_owned()).collect();

        // Header + the string cell still render as text.
        assert!(all.iter().any(|s| s == "Photo"), "missing image-column header");
        assert!(all.iter().any(|s| s == "Alice"), "string cell not rendered");
        // Image cell must NOT leak its path as text (it's drawn as an image).
        assert!(
            !all.iter().any(|s| s.contains(&path)),
            "image path was rendered as text instead of an image: {all:?}"
        );
        // A textured mesh (the actual image) was emitted for the image cell.
        assert!(
            shapes.iter().any(|s| matches!(s, egui::Shape::Mesh(_))),
            "no image mesh emitted for the image cell"
        );

        let _ = std::fs::remove_file(&png);
    }

    // ── #142: previously-unrendered runtime widgets ───────────────────────────
    fn run_all_texts(ct: CT, state: &CtrlState, rect: Rect) -> Vec<String> {
        run_texts(&render_run_shapes(ct, state, rect))
            .iter().map(|t| t.galley.text().trim().to_owned()).collect()
    }

    #[test]
    fn radiobutton_click_selects() {
        let rect = Rect::from_min_size(pos2(50.0, 50.0), vec2(140.0, 24.0));
        let c = rect.center();
        let (evs, st) = drive(CT::RadioButton, cs(&[("Caption", "Opt"), ("Value", "0")]), rect,
            vec![vec![], vec![Event::PointerMoved(c), press(c)], vec![Event::PointerMoved(c), release(c)]]);
        assert!(evs.iter().any(|e| e.event_id == "Change"), "radio click fired no Change");
        assert_eq!(st.get("Value"), "1", "radio not selected");
    }

    #[test]
    fn tabcontrol_click_switches_tab() {
        let rect = Rect::from_min_size(pos2(10.0, 10.0), vec2(300.0, 120.0));
        // Tab i origin x = rect.min.x + i*(84+2); width 84, height 26 → center.
        let tab1 = pos2(10.0 + 86.0 + 42.0, 10.0 + 13.0);
        let (evs, st) = drive(CT::TabControl, cs(&[("Tabs", "Alpha\nBeta\nGamma"), ("SelectedTab", "0")]), rect,
            vec![vec![], vec![Event::PointerMoved(tab1), press(tab1)], vec![Event::PointerMoved(tab1), release(tab1)]]);
        assert!(evs.iter().any(|e| e.event_id == "Change"), "tab click fired no Change");
        assert_eq!(st.get("SelectedTab"), "1", "tab not switched to index 1");
    }

    #[test]
    fn numericupdown_shows_value() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(120.0, 24.0));
        let all = run_all_texts(CT::NumericUpDown, &cs(&[("Value", "42"), ("Minimum", "0"), ("Maximum", "100")]), rect);
        assert!(all.iter().any(|s| s.contains("42")), "NumericUpDown didn't show value: {all:?}");
    }

    #[test]
    fn menubar_renders_item_text() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(300.0, 24.0));
        let all = run_all_texts(CT::MenuBar, &cs(&[("Items", "File\nEdit\nView")]), rect);
        for it in ["File", "Edit", "View"] {
            assert!(all.iter().any(|s| s == it), "MenuBar missing '{it}': {all:?}");
        }
    }

    #[test]
    fn treeview_renders_items() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 120.0));
        let all: Vec<String> = run_texts(&render_run_shapes(CT::TreeView,
            &cs(&[("Items", "Root\n  Child A\n  Child B")]), rect))
            .iter().map(|t| t.galley.text().to_owned()).collect();
        assert!(all.iter().any(|s| s.contains("Root")), "TreeView missing Root: {all:?}");
        assert!(all.iter().any(|s| s.contains("Child A")), "TreeView missing Child A");
    }

    #[test]
    fn chart_renders_shapes() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(220.0, 150.0));
        let shapes = render_run_shapes(CT::BarChart, &cs(&[("Title", "Sales")]), rect);
        assert!(shapes.len() > 3, "BarChart drew almost nothing ({} shapes)", shapes.len());
    }
}

#[cfg(test)]
mod inspect_refresh_tests {
    use super::*;
    use cobolt_forms::model::{Control, ControlType, Form, PropValue};

    fn form_with_title(title: &str) -> Form {
        let mut f = Form::new("FRM_MAIN", title, 400, 300);
        let mut c = Control::new("BTN_OK", ControlType::Button, 10, 10);
        c.properties.insert("Caption".to_string(), PropValue::String("Old".to_string()));
        f.controls.push(c);
        f
    }

    /// Editing a form on disk (e.g. a Designer save) must live-refresh the
    /// Main-Pane inspector: `reload_if_stale` pulls the new values in.
    #[test]
    fn inspector_reloads_when_cfrm_changes_on_disk() {
        let dir = std::env::temp_dir().join(format!("prc_inspect_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("main.cfrm");

        save_form(&form_with_title("First"), &path).unwrap();
        let mut st = InspectState {
            path: path.clone(),
            ctrl_id: Some("BTN_OK".to_string()),
            designer: DesignerPanel::new(load_form(&path).unwrap()),
            mtime: file_mtime(&path),
        };
        assert_eq!(st.designer.form.title, "First");

        // Simulate a Designer save: rewrite the .cfrm with new values after a
        // short delay so the modification time advances.
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut updated = form_with_title("Second");
        updated.controls[0]
            .properties
            .insert("Caption".to_string(), PropValue::String("New".to_string()));
        save_form(&updated, &path).unwrap();

        assert!(st.reload_if_stale(), "should detect the on-disk change");
        assert_eq!(st.designer.form.title, "Second", "form prop not refreshed");
        let c = st.designer.form.find_control("BTN_OK").unwrap();
        assert_eq!(c.get_prop("Caption").map(|v| v.as_str().to_owned()).as_deref(), Some("New"));
        // Selection preserved because the control still exists.
        assert_eq!(st.ctrl_id.as_deref(), Some("BTN_OK"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn filetime_set(path: &Path, t: std::time::SystemTime) {
        // Best-effort: touch mtime via a second write so it advances even if the
        // platform clamps to second granularity.
        let _ = (path, t);
        // Re-save with a tiny delay already covers the > comparison; nothing else
        // to do here without an extra dependency.
    }
}
