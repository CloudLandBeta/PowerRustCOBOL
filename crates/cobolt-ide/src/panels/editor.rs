// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Code editor panel — multi-tab COBOL editor with:
//!   • Scrolling on both axes
//!   • IntelliSense: keywords, snippets, paragraphs, data items,
//!     form-control IDs, **properties and methods** (triggered on exact control ID match)
//!   • Cmd/Ctrl+F — find bar with match count and prev/next navigation
//!   • 12 pt monospace font (adjustable with A+/A- buttons)
//!   • Syntax colouring (keywords, data items, paragraphs, strings, comments)

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use egui::{
    Color32, Context, CentralPanel, FontId, Key,
    Pos2, ScrollArea, TextEdit, TopBottomPanel,
};

use crate::runner::DiagMsg;

// ── Constants ─────────────────────────────────────────────────────────────────

pub const EDITOR_FONT_SIZE: f32 = 18.0;

// ── COBOL keyword tables ──────────────────────────────────────────────────────

const VERBS: &[&str] = &[
    "MOVE", "ADD", "SUBTRACT", "MULTIPLY", "DIVIDE", "COMPUTE",
    "IF", "ELSE", "END-IF", "EVALUATE", "WHEN", "OTHER", "END-EVALUATE",
    "PERFORM", "UNTIL", "VARYING", "FROM", "BY", "AFTER", "THRU", "THROUGH",
    "TIMES", "END-PERFORM",
    "GO", "TO", "DEPENDING", "ON", "CONTINUE", "NEXT", "SENTENCE",
    "ACCEPT", "DISPLAY", "UPON", "NO", "ADVANCING",
    "CALL", "USING", "RETURNING", "EXCEPTION", "END-CALL",
    "OPEN", "CLOSE", "READ", "WRITE", "REWRITE", "DELETE", "START",
    "INTO", "AT", "END", "NOT", "INVALID", "KEY",
    "STRING", "UNSTRING", "DELIMITED", "ALL", "POINTER",
    "INSPECT", "TALLYING", "REPLACING", "CONVERTING",
    "STOP", "RUN", "GOBACK", "EXIT", "PROGRAM",
    "SORT", "MERGE", "OUTPUT", "INPUT",
    "EXEC", "END-EXEC", "INVOKE", "SET",
    "GIVING", "ROUNDED", "REMAINDER", "SIZE", "ERROR",
    "REFERENCE", "CONTENT", "VALUE",
    // CoBolt animation extensions
    "PLAY", "STOP-ANIMATION",
    // CoBolt exception handling extensions
    "TRY", "CATCH", "EXCEPTION", "FINALLY", "END-TRY", "THROW", "RAISE",
];

const DIVISION_KEYWORDS: &[&str] = &[
    "IDENTIFICATION", "ENVIRONMENT", "DATA", "PROCEDURE",
    "DIVISION", "SECTION",
    "PROGRAM-ID", "AUTHOR", "DATE-WRITTEN",
    "WORKING-STORAGE", "LOCAL-STORAGE", "LINKAGE",
    "FILE-CONTROL", "SELECT", "ASSIGN", "ORGANIZATION",
    "SEQUENTIAL", "INDEXED", "RELATIVE", "ACCESS", "MODE",
    "RECORD", "ALTERNATE", "WITH", "DUPLICATES",
    "FILE", "STATUS", "FD", "SD",
];

const DATA_KEYWORDS: &[&str] = &[
    "PIC", "PICTURE", "COMP", "COMP-1", "COMP-2", "COMP-3", "COMP-5",
    "BINARY", "PACKED-DECIMAL", "DISPLAY",
    "OCCURS", "TIMES", "INDEXED", "REDEFINES",
    "VALUES", "IS", "ARE",
    "FILLER", "GLOBAL", "EXTERNAL",
    "SPACE", "SPACES", "ZERO", "ZEROS", "ZEROES",
    "HIGH-VALUE", "HIGH-VALUES", "LOW-VALUE", "LOW-VALUES",
    "QUOTE", "QUOTES", "NULL", "NULLS",
];

// ── Control member tables ─────────────────────────────────────────────────────

/// Methods exposed by each control type (shown after `INVOKE ctrl-id '`).
fn methods_for_type(ctrl_type: &str) -> &'static [(&'static str, &'static str)] {
    match ctrl_type {
        "Button"      => &[
            ("Click",         "Trigger click event"),
            ("Show",          "Make visible"),
            ("Hide",          "Make invisible"),
            ("Enable",        "Enable interaction"),
            ("Disable",       "Disable interaction"),
            ("SetCaption",    "Change button text"),
            ("PlayAnimation", "Run a named animation"),
            ("StopAnimation", "Stop a running animation"),
        ],
        "TextBox"     => &[
            ("GetText",  "Return current text value"),
            ("SetText",  "Set text value"),
            ("Clear",    "Clear text"),
            ("Focus",    "Move focus here"),
            ("Show",     "Make visible"),
            ("Hide",     "Make invisible"),
        ],
        "Label"       => &[
            ("SetCaption",    "Change label text"),
            ("SetColor",      "Change foreground colour"),
            ("Show",          "Make visible"),
            ("Hide",          "Make invisible"),
            ("PlayAnimation", "Run a named animation"),
        ],
        "CheckBox"    => &[
            ("IsChecked",  "Returns 1 if checked"),
            ("SetChecked", "Set checked state (0/1)"),
            ("GetValue",   "Get current value"),
            ("Show",       "Make visible"),
            ("Hide",       "Make invisible"),
        ],
        "RadioButton" => &[
            ("IsChecked",  "Returns 1 if selected"),
            ("SetChecked", "Set selected state (0/1)"),
            ("Show",       "Make visible"),
            ("Hide",       "Make invisible"),
        ],
        "ComboBox"    => &[
            ("GetText",   "Get selected text"),
            ("SetText",   "Set/select text"),
            ("AddItem",   "Add item to list"),
            ("Clear",     "Clear all items"),
            ("GetIndex",  "Get selected index"),
            ("Show",      "Make visible"),
            ("Hide",      "Make invisible"),
        ],
        "ListBox"     => &[
            ("AddItem",    "Append item"),
            ("RemoveItem", "Remove item by index"),
            ("Clear",      "Remove all items"),
            ("GetSelected","Return selected text"),
            ("GetCount",   "Return item count"),
            ("Show",       "Make visible"),
            ("Hide",       "Make invisible"),
        ],
        "PictureBox"  => &[
            ("SetImage",   "Load image from file path"),
            ("Clear",      "Clear the displayed image"),
            ("Show",       "Make visible"),
            ("Hide",       "Make invisible"),
            ("Refresh",    "Reload image from ImagePath"),
        ],
        "DataGrid"    => &[
            ("Refresh",       "Reload data"),
            ("ExportCSV",     "Export rows as CSV"),
            ("GetRowCount",   "Return row count"),
            ("GetCellValue",  "Read a cell"),
            ("SetCellValue",  "Write a cell"),
            ("AddRow",        "Append empty row"),
            ("DeleteRow",     "Delete row by index"),
            ("Show",          "Make visible"),
            ("Hide",          "Make invisible"),
        ],
        "Timer"       => &[
            ("Start",       "Start / resume timer"),
            ("Stop",        "Pause timer"),
            ("SetInterval", "Change interval in ms"),
            ("IsEnabled",   "Returns 1 if running"),
        ],
        "ProgressBar" => &[
            ("SetValue",  "Set current value"),
            ("GetValue",  "Get current value"),
            ("Show",      "Make visible"),
            ("Hide",      "Make invisible"),
        ],
        "Slider"      => &[
            ("SetValue",  "Set thumb position"),
            ("GetValue",  "Get current value"),
            ("Show",      "Make visible"),
            ("Hide",      "Make invisible"),
        ],
        "AgentObject" => &[
            ("Ask",        "Send prompt to LLM, get reply"),
            ("SetPrompt",  "Set the system prompt"),
            ("SetModel",   "Switch model name"),
            ("Stop",       "Abort current request"),
        ],
        "RestClient"  => &[
            ("call",       "Generic HTTP call"),
            ("get",        "HTTP GET request"),
            ("post",       "HTTP POST request"),
            ("setHeader",  "Add/replace a request header"),
            ("setTimeout", "Set timeout in ms"),
        ],
        "ModalWindow" => &[
            ("Show",        "Open the modal window"),
            ("Close",       "Close the modal window"),
            ("GetResult",   "Return modal result value"),
            ("SetTitle",    "Change window title"),
        ],
        _ => &[
            ("Show",          "Make visible"),
            ("Hide",          "Make invisible"),
            ("Enable",        "Enable"),
            ("Disable",       "Disable"),
            ("SetProperty",   "Set any property by name"),
            ("GetProperty",   "Get any property by name"),
            ("PlayAnimation", "Run a named animation"),
            ("StopAnimation", "Stop running animation"),
        ],
    }
}

/// Properties for each control type — shown when a control ID is typed exactly.
fn properties_for_type(ctrl_type: &str) -> &'static [(&'static str, &'static str)] {
    match ctrl_type {
        "Button" => &[
            ("Caption",      "Button label text"),
            ("Visible",      "1 = visible, 0 = hidden"),
            ("Enabled",      "1 = enabled, 0 = disabled"),
            ("Width",        "Width in pixels"),
            ("Height",       "Height in pixels"),
            ("BackgroundColor",    "Background colour (RRGGBB)"),
            ("ForegroundColor",    "Text colour (RRGGBB)"),
            ("FontSize",     "Font size in points"),
            ("Bold",         "1 = bold text"),
            ("CornerRadius", "Border corner radius"),
            ("Opacity",      "Opacity 0–100"),
        ],
        "Label" => &[
            ("Caption",      "Label text"),
            ("Visible",      "1 = visible, 0 = hidden"),
            ("ForegroundColor",    "Text colour (RRGGBB)"),
            ("FontSize",     "Font size in points"),
            ("Bold",         "1 = bold"),
            ("Italic",       "1 = italic"),
            ("Underline",    "1 = underline"),
            ("Strikethrough","1 = strikethrough"),
            ("Opacity",      "Opacity 0–100"),
        ],
        "TextBox" => &[
            ("Text",         "Current text value"),
            ("Visible",      "1 = visible, 0 = hidden"),
            ("Enabled",      "1 = enabled, 0 = disabled"),
            ("MaximumLength",    "Maximum character count"),
            ("Multiline",    "1 = multiline input"),
            ("PasswordCharacter", "Masking character (e.g. *)"),
            ("BackgroundColor",    "Background colour (RRGGBB)"),
            ("ForegroundColor",    "Text colour (RRGGBB)"),
            ("FontSize",     "Font size in points"),
            ("ReadOnly",     "1 = read-only"),
        ],
        "CheckBox" => &[
            ("Caption",  "Checkbox label text"),
            ("Checked",  "1 = checked, 0 = unchecked"),
            ("Visible",  "1 = visible, 0 = hidden"),
            ("Enabled",  "1 = enabled, 0 = disabled"),
            ("ForegroundColor","Label colour (RRGGBB)"),
        ],
        "RadioButton" => &[
            ("Caption",  "Radio button label"),
            ("Checked",  "1 = selected, 0 = not selected"),
            ("Visible",  "1 = visible, 0 = hidden"),
            ("Enabled",  "1 = enabled, 0 = disabled"),
            ("ForegroundColor","Label colour (RRGGBB)"),
        ],
        "ComboBox" => &[
            ("Text",     "Selected / displayed text"),
            ("Items",    "Newline-separated item list"),
            ("Visible",  "1 = visible, 0 = hidden"),
            ("Enabled",  "1 = enabled, 0 = disabled"),
            ("BackgroundColor","Background colour (RRGGBB)"),
            ("ForegroundColor","Text colour (RRGGBB)"),
        ],
        "ListBox" => &[
            ("Items",    "Newline-separated item list"),
            ("Visible",  "1 = visible, 0 = hidden"),
            ("Enabled",  "1 = enabled, 0 = disabled"),
            ("BackgroundColor","Background colour (RRGGBB)"),
            ("ForegroundColor","Text colour (RRGGBB)"),
        ],
        "PictureBox" => &[
            ("ImagePath", "Absolute path to image file"),
            ("SizeMode",  "Normal / StretchImage / Zoom / AutoSize"),
            ("Visible",   "1 = visible, 0 = hidden"),
            ("Opacity",   "Opacity 0–100"),
            ("Width",     "Width in pixels"),
            ("Height",    "Height in pixels"),
        ],
        "GroupBox" => &[
            ("Caption",   "Group box title"),
            ("Visible",   "1 = visible, 0 = hidden"),
            ("BackgroundColor", "Background colour"),
            ("ForegroundColor", "Title text colour"),
        ],
        "Panel" => &[
            ("Visible",   "1 = visible, 0 = hidden"),
            ("BackgroundColor", "Background colour (RRGGBB)"),
            ("Opacity",   "Opacity 0–100"),
        ],
        "ProgressBar" => &[
            ("Value",     "Current value"),
            ("Minimum",   "Minimum value"),
            ("Maximum",   "Maximum value"),
            ("Visible",   "1 = visible, 0 = hidden"),
            ("BarColor",  "Fill colour (RRGGBB)"),
            ("ShowValue", "1 = display percentage text"),
        ],
        "Slider" => &[
            ("Value",    "Current thumb position"),
            ("Minimum",  "Minimum value"),
            ("Maximum",  "Maximum value"),
            ("Step",     "Step increment"),
            ("Visible",  "1 = visible, 0 = hidden"),
        ],
        "DataGrid" => &[
            ("Columns",   "Newline-separated column names"),
            ("Visible",   "1 = visible, 0 = hidden"),
            ("ExportCSV", "1 = enable CSV export button"),
            ("BackgroundColor", "Background colour"),
        ],
        "Timer" => &[
            ("Interval", "Tick interval in milliseconds"),
            ("Enabled",  "1 = running, 0 = stopped"),
        ],
        "AgentObject" => &[
            ("AgentModel", "LLM model name  (e.g. llama3.2)"),
            ("AgentURL",   "Endpoint base URL"),
            ("SystemPrompt","Optional system prompt"),
        ],
        "RestClient" => &[
            ("BaseURL",       "Base URL for all requests"),
            ("DefaultMethod", "Default HTTP method (GET/POST…)"),
            ("Timeout",       "Request timeout in ms"),
        ],
        "ModalWindow" => &[
            ("Title",          "Window title bar text"),
            ("ProgramName",    "COBOL program to CALL"),
            ("SharedDataItems","Comma-separated shared items"),
            ("Visible",        "1 = visible"),
        ],
        _ => &[
            ("Caption",  "Display text"),
            ("Visible",  "1 = visible, 0 = hidden"),
            ("Enabled",  "1 = enabled, 0 = disabled"),
            ("Width",    "Width in pixels"),
            ("Height",   "Height in pixels"),
            ("BackgroundColor","Background colour (RRGGBB)"),
            ("ForegroundColor","Foreground colour (RRGGBB)"),
            ("Opacity",  "Opacity 0–100"),
        ],
    }
}

// ── Auto-completion types ─────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum AcKind { Keyword, Snippet, Paragraph, DataItem, Property, Method, Control }

#[derive(Clone)]
struct AcItem {
    label:  String,
    insert: String,
    detail: String,
    kind:   AcKind,
}

impl AcItem {
    fn kw(word: &str) -> Self {
        Self { label: word.into(), insert: word.into(), detail: "keyword".into(), kind: AcKind::Keyword }
    }
    fn snip(label: &str, insert: &str, detail: &str) -> Self {
        Self { label: label.into(), insert: insert.into(), detail: detail.into(), kind: AcKind::Snippet }
    }
    fn para(name: &str) -> Self {
        Self { label: name.into(), insert: name.into(), detail: "paragraph".into(), kind: AcKind::Paragraph }
    }
    fn data(name: &str) -> Self {
        Self { label: name.into(), insert: name.into(), detail: "data item".into(), kind: AcKind::DataItem }
    }
    fn prop(name: &str, detail: &str) -> Self {
        Self { label: name.into(), insert: name.into(), detail: detail.into(), kind: AcKind::Property }
    }
    fn method(name: &str, detail: &str) -> Self {
        Self { label: name.into(), insert: format!("'{name}'"), detail: detail.into(), kind: AcKind::Method }
    }
    fn ctrl(id: &str, ctrl_type: &str) -> Self {
        Self { label: id.into(), insert: id.into(), detail: format!("{ctrl_type} control"), kind: AcKind::Control }
    }

    fn badge(&self) -> (&str, Color32) {
        match self.kind {
            AcKind::Keyword   => ("K", Color32::from_rgb( 86, 156, 214)),
            AcKind::Snippet   => ("S", Color32::from_rgb(220, 180,  60)),
            AcKind::Paragraph => ("¶", Color32::from_rgb(197, 134, 192)),
            AcKind::DataItem  => ("D", Color32::from_rgb( 78, 201, 176)),
            AcKind::Property  => ("●", Color32::from_rgb(120, 220, 110)),
            AcKind::Method    => ("M", Color32::from_rgb(255, 160,  80)),
            AcKind::Control   => ("C", Color32::from_rgb(140, 200, 255)),
        }
    }
}

// ── AutoComplete state ────────────────────────────────────────────────────────

#[derive(Default)]
struct AutoComplete {
    visible:      bool,
    items:        Vec<AcItem>,
    selected:     usize,
    prefix:       String,
    trigger_pos:  usize,
    popup_pos:    Pos2,
    /// When true the popup is showing members of a specific control (property/method list).
    member_mode:  bool,
}

// ── Search / Find state ───────────────────────────────────────────────────────

#[derive(Default)]
struct SearchState {
    visible:      bool,
    query:        String,
    /// Replacement text for the find/replace bar.
    replace:      String,
    /// Byte offsets of match starts in the active tab.
    matches:      Vec<usize>,
    /// Index into `matches` currently highlighted.
    current:      usize,
    /// Set to `true` when the next render should scroll to `current`.
    needs_scroll: bool,
}

// ── EditorTab ─────────────────────────────────────────────────────────────────

pub struct EditorTab {
    pub path:    PathBuf,
    pub content: String,
    pub dirty:   bool,
    /// RAD-generated code: shown in blue, never editable.
    pub read_only: bool,
}

impl EditorTab {
    pub fn new(path: PathBuf, content: String) -> Self {
        Self { path, content, dirty: false, read_only: false }
    }
    pub fn title(&self) -> String {
        let name = self.path.file_name().and_then(|n| n.to_str()).unwrap_or("untitled");
        if self.read_only {
            format!("🔒 {name}")
        } else if self.dirty {
            format!("● {name}")
        } else {
            name.into()
        }
    }
}

// ── Known control (for IntelliSense) ─────────────────────────────────────────

#[derive(Clone)]
pub struct KnownControl {
    pub id:        String,
    pub ctrl_type: String,
}

// ── EditorPanel ───────────────────────────────────────────────────────────────

pub struct EditorPanel {
    pub tabs:              Vec<EditorTab>,
    pub active:            usize,
    pub diags:             HashMap<PathBuf, Vec<DiagMsg>>,
    pub show_line_numbers: bool,
    pub known_controls:    Vec<KnownControl>,
    /// Active breakpoint line numbers per file (1-based).
    pub breakpoints:       HashMap<PathBuf, HashSet<u32>>,
    /// Line being highlighted by the debugger (current pause location).
    pub debug_line:        Option<(PathBuf, u32)>,
    ac:        AutoComplete,
    search:    SearchState,
    font_size: f32,

    // ── AI assistant (only used when an LLM is configured) ───────────────────
    /// The current prompt text in the editor's AI bar.
    ai_prompt:  String,
    /// Per-file conversation history (loaded lazily from disk).
    ai_history: HashMap<PathBuf, Vec<crate::llm::ChatTurn>>,
    /// Paths whose history has already been loaded from disk this session.
    ai_loaded:  HashSet<PathBuf>,
    /// In-flight request: the channel the worker thread will answer on, plus
    /// the path it targets (so a tab switch mid-flight applies to the right file).
    ai_pending: Option<(PathBuf, std::sync::mpsc::Receiver<crate::llm::LlmResponse>)>,
    /// Last status / error line shown under the AI bar.
    ai_status:  Option<String>,
    /// Whether the conversation panel is expanded.
    ai_show_history: bool,

    // ── Status bar ───────────────────────────────────────────────────────────
    /// 1-based caret line / column in the active tab (last known).
    cur_line: usize,
    cur_col:  usize,
    /// Overwrite (vs. insert) typing mode — toggled with the Insert key.
    overwrite: bool,
    /// Trim trailing whitespace from every line when saving.
    pub trim_on_save: bool,
}

impl Default for EditorPanel {
    fn default() -> Self {
        Self {
            tabs:              Vec::new(),
            active:            0,
            diags:             HashMap::new(),
            show_line_numbers: true,
            known_controls:    Vec::new(),
            breakpoints:       HashMap::new(),
            debug_line:        None,
            ac:                AutoComplete::default(),
            search:            SearchState::default(),
            font_size:         EDITOR_FONT_SIZE,
            ai_prompt:         String::new(),
            ai_history:        HashMap::new(),
            ai_loaded:         HashSet::new(),
            ai_pending:        None,
            ai_status:         None,
            ai_show_history:   false,
            cur_line:          1,
            cur_col:           1,
            overwrite:         false,
            trim_on_save:      true,
        }
    }
}

impl EditorPanel {
    pub fn new() -> Self { Self::default() }

    // ── Breakpoint helpers ────────────────────────────────────────────────────

    /// Toggle a breakpoint on `line` (1-based) in the active file.
    pub fn toggle_breakpoint(&mut self, line: u32) {
        if let Some(tab) = self.tabs.get(self.active) {
            let set = self.breakpoints.entry(tab.path.clone()).or_default();
            if !set.remove(&line) {
                set.insert(line);
            }
        }
    }

    /// Return the set of active breakpoint lines for the active file.
    pub fn active_breakpoints(&self) -> Option<&HashSet<u32>> {
        self.tabs.get(self.active).and_then(|t| self.breakpoints.get(&t.path))
    }

    /// Return all breakpoint lines for a given path (for the runner to sync).
    pub fn breakpoints_for(&self, path: &PathBuf) -> Vec<u32> {
        self.breakpoints.get(path)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    // ── File operations ────────────────────────────────────────────────────────

    pub fn open_file(&mut self, path: PathBuf) {
        self.open_file_ro(path, false);
    }

    /// Replace all tabs with a single in-memory editable buffer. Used by the
    /// embedded RAD event editor (the modal hosts its own `EditorPanel`); the
    /// synthetic `path` is an identity only and is never written to disk.
    pub fn open_buffer(&mut self, path: PathBuf, content: String) {
        self.tabs   = vec![EditorTab::new(path, content)];
        self.active = 0;
        self.search.visible = false;
        self.ac.visible     = false;
    }

    /// The active buffer's text (for reading an embedded editor back).
    pub fn buffer_content(&self) -> Option<&str> {
        self.tabs.get(self.active).map(|t| t.content.as_str())
    }

    /// The active buffer's text, trimmed of trailing whitespace when the
    /// `Trim on save` toggle is on (used when an embedded editor is committed).
    pub fn buffer_for_save(&self) -> Option<String> {
        self.tabs.get(self.active).map(|t| {
            if self.trim_on_save { trim_trailing_ws(&t.content) } else { t.content.clone() }
        })
    }

    /// Open `path`, marking the tab read-only (blue, non-editable) when
    /// `read_only` is set (RAD-generated COBOL).
    pub fn open_file_ro(&mut self, path: PathBuf, read_only: bool) {
        if let Some(idx) = self.tabs.iter().position(|t| t.path == path) {
            self.active = idx;
            self.tabs[idx].read_only = read_only;
            return;
        }
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let mut tab = EditorTab::new(path, content);
        tab.read_only = read_only;
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
    }

    /// Reload an already-open tab's content from disk (e.g. after the form
    /// designer regenerated its COBOL). No-op if the file isn't open.
    pub fn reload_file(&mut self, path: &std::path::Path) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.path == path) {
            if let Ok(content) = std::fs::read_to_string(path) {
                tab.content = content;
                tab.dirty = false;
            }
        }
    }

    pub fn save_active(&mut self) -> std::io::Result<()> {
        let trim = self.trim_on_save;
        let Some(tab) = self.tabs.get_mut(self.active) else { return Ok(()); };
        if tab.read_only { return Ok(()); } // never write generated source
        if trim {
            let trimmed = trim_trailing_ws(&tab.content);
            if trimmed != tab.content {
                tab.content = trimmed;
            }
        }
        std::fs::write(&tab.path, &tab.content)?;
        tab.dirty = false;
        Ok(())
    }

    /// "Beautify" the active tab: a conservative whitespace tidy that never
    /// touches COBOL area-A/B alignment — trim trailing spaces, collapse runs of
    /// blank lines, and end with a single newline.
    pub fn beautify_active(&mut self) {
        let Some(tab) = self.tabs.get_mut(self.active) else { return; };
        if tab.read_only { return; }
        let tidy = beautify_cobol(&tab.content);
        if tidy != tab.content {
            tab.content = tidy;
            tab.dirty = true;
        }
    }

    pub fn active_source(&self) -> Option<(&PathBuf, &str)> {
        self.tabs.get(self.active).map(|t| (&t.path, t.content.as_str()))
    }

    pub fn clear_diags(&mut self) { self.diags.clear(); }

    pub fn add_diag(&mut self, path: &PathBuf, diag: DiagMsg) {
        self.diags.entry(path.clone()).or_default().push(diag);
    }

    // ── Search helpers ────────────────────────────────────────────────────────

    fn update_search_matches(&mut self) {
        self.search.matches.clear();
        self.search.current = 0;
        if self.search.query.is_empty() { return; }
        let Some(tab) = self.tabs.get(self.active) else { return; };
        let lower_text  = tab.content.to_lowercase();
        let lower_query = self.search.query.to_lowercase();
        let qlen = lower_query.len();
        if qlen == 0 { return; }
        let mut start = 0;
        while start + qlen <= lower_text.len() {
            if let Some(rel) = lower_text[start..].find(&lower_query) {
                let pos = start + rel;
                self.search.matches.push(pos);
                start = pos + 1;
            } else {
                break;
            }
        }
    }

    /// Scroll the active tab to the definition of `paragraph` (a COBOL paragraph
    /// header or `PROGRAM-ID. NAME`) and place the cursor there. Reuses the
    /// search-scroll machinery. Returns `false` if the name isn't found.
    pub fn goto_paragraph(&mut self, paragraph: &str) -> bool {
        let Some(tab) = self.tabs.get(self.active) else { return false; };
        let needle = paragraph.trim().to_ascii_uppercase();
        if needle.is_empty() { return false; }
        let upper = tab.content.to_ascii_uppercase();

        // Prefer a real definition: the name at the start of an indented line,
        // either as a paragraph header (`NAME.`) or `PROGRAM-ID. NAME.`.
        let mut found: Option<usize> = None;
        let mut off = 0usize;
        for line in upper.split_inclusive('\n') {
            let trimmed = line.trim_start();
            let indent  = line.len() - trimmed.len();
            if let Some(rest) = trimmed.strip_prefix("PROGRAM-ID.") {
                if rest.trim().trim_end_matches('.').trim() == needle {
                    found = Some(off + indent);
                    break;
                }
            }
            if trimmed.starts_with(&needle) {
                let after = trimmed[needle.len()..].trim_start();
                if after.starts_with('.') {
                    found = Some(off + indent);
                    break;
                }
            }
            off += line.len();
        }

        // Fallback: first textual occurrence anywhere (e.g. a CALL site).
        let target = found.or_else(|| upper.find(&needle));
        if let Some(byte_off) = target {
            self.search.matches = vec![byte_off];
            self.search.current = 0;
            self.search.needs_scroll = true;
            true
        } else {
            false
        }
    }

    fn search_next(&mut self) {
        if self.search.matches.is_empty() { return; }
        self.search.current = (self.search.current + 1) % self.search.matches.len();
        self.search.needs_scroll = true;
    }

    fn search_prev(&mut self) {
        if self.search.matches.is_empty() { return; }
        self.search.current = if self.search.current == 0 {
            self.search.matches.len() - 1
        } else {
            self.search.current - 1
        };
        self.search.needs_scroll = true;
    }

    /// Replace the currently-highlighted match with the replacement text, then
    /// re-scan and keep the find cursor valid.
    fn replace_current(&mut self) {
        let q = self.search.query.clone();
        if q.is_empty() { return; }
        let repl = self.search.replace.clone();
        let cur  = self.search.current;
        let Some(&byte_off) = self.search.matches.get(cur) else { return; };
        {
            let Some(tab) = self.tabs.get_mut(self.active) else { return; };
            if tab.read_only { return; }
            let end = (byte_off + q.len()).min(tab.content.len());
            if byte_off <= tab.content.len()
                && tab.content.is_char_boundary(byte_off)
                && tab.content.is_char_boundary(end)
                && tab.content[byte_off..end].eq_ignore_ascii_case(&q)
            {
                tab.content.replace_range(byte_off..end, &repl);
                tab.dirty = true;
            }
        }
        self.update_search_matches();
        if self.search.current >= self.search.matches.len() {
            self.search.current = 0;
        }
        self.search.needs_scroll = !self.search.matches.is_empty();
    }

    /// Replace every match in the active tab (case-insensitive).
    fn replace_all(&mut self) {
        let q = self.search.query.clone();
        if q.is_empty() { return; }
        let repl = self.search.replace.clone();
        {
            let Some(tab) = self.tabs.get_mut(self.active) else { return; };
            if tab.read_only { return; }
            let new = replace_all_ci(&tab.content, &q, &repl);
            if new != tab.content {
                tab.content = new;
                tab.dirty = true;
            }
        }
        self.update_search_matches();
        self.search.current = 0;
    }

    // ── Main render ────────────────────────────────────────────────────────────

    // ── AI assistant bar ─────────────────────────────────────────────────────

    /// Render the AI prompt bar for an arbitrary target and return `Some(code)`
    /// when the model's reply should replace the target's source.
    ///
    /// The bar is reusable: the code editor passes the active tab (editable), and
    /// the inline form inspector passes the form's generated COBOL (read-only).
    /// `target` is both the buffer identity and the conversation key. The model
    /// receives the standard system prompt, the per-target conversation history,
    /// the current `code`, and the developer's request.
    pub fn ai_bar(
        &mut self,
        ctx: &Context,
        cfg: &crate::llm::LlmConfig,
        tr: &crate::i18n::Tr,
        panel_id: &str,
        target: &std::path::Path,
        code: &str,
        read_only: bool,
        project_root: Option<&std::path::Path>,
    ) -> Option<String> {
        let path = target.to_path_buf();
        let panel = egui::Id::new(panel_id);
        let transcript_salt = egui::Id::new((panel_id, "transcript"));

        // Lazily load this target's saved conversation the first time we see it.
        if self.ai_loaded.insert(path.clone()) {
            if let Some((data_dir, key)) = Self::ai_store_key(project_root, &path) {
                let turns = crate::llm::load_history(&data_dir, &key);
                if !turns.is_empty() {
                    self.ai_history.insert(path.clone(), turns);
                }
            }
        }

        // Poll an in-flight request for this target.
        let mut completed: Option<crate::llm::LlmResponse> = None;
        if let Some((pending_path, rx)) = &self.ai_pending {
            if pending_path == &path {
                match rx.try_recv() {
                    Ok(resp) => completed = Some(resp),
                    Err(std::sync::mpsc::TryRecvError::Empty) => { ctx.request_repaint(); }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        completed = Some(crate::llm::LlmResponse::Err(
                            "The assistant worker stopped unexpectedly.".into(),
                        ));
                    }
                }
            } else {
                ctx.request_repaint();
            }
        }
        let mut applied: Option<String> = None;
        if let Some(resp) = completed {
            self.ai_pending = None;
            applied = self.apply_ai_response(&path, resp, tr, read_only, project_root);
        }

        let busy = self.ai_pending.as_ref().map(|(p, _)| *p == path).unwrap_or(false);
        let history_len = self.ai_history.get(&path).map(|v| v.len()).unwrap_or(0);

        // Snapshot UI-owned state so the panel closure borrows locals, not `self`.
        let mut prompt = std::mem::take(&mut self.ai_prompt);
        let mut show_history = self.ai_show_history;
        let status = self.ai_status.clone();
        let history_snapshot: Vec<crate::llm::ChatTurn> =
            self.ai_history.get(&path).cloned().unwrap_or_default();

        let mut do_send  = false;
        let mut do_clear = false;

        let frame = crate::theme::glass_panel_frame(
            ctx.style().visuals.panel_fill, &crate::theme::active());
        TopBottomPanel::top(panel).frame(frame).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("✨").size(15.0));

                let can_send = !busy && !prompt.trim().is_empty();
                if ui.add_enabled(can_send, egui::Button::new(tr.ai_send)).clicked() {
                    do_send = true;
                }
                if busy {
                    ui.add(egui::Spinner::new());
                    ui.label(egui::RichText::new(tr.ai_thinking)
                        .small().color(Color32::from_gray(170)));
                }
                if history_len > 0 {
                    ui.toggle_value(&mut show_history,
                        egui::RichText::new(format!("💬 {history_len}")).small());
                    if ui.small_button("🗑").on_hover_text(tr.ai_clear_history).clicked() {
                        do_clear = true;
                    }
                }

                // The prompt fills the rest of the row.
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut prompt)
                        .hint_text(tr.ai_prompt_placeholder)
                        .desired_width(ui.available_width())
                        .interactive(!busy),
                );
                let entered = resp.lost_focus()
                    && ui.input(|i| i.key_pressed(Key::Enter))
                    && !prompt.trim().is_empty();
                if entered && !busy {
                    do_send = true;
                }
            });

            if read_only {
                ui.label(egui::RichText::new(tr.ai_read_only)
                    .small().color(Color32::from_gray(150)));
            } else if let Some(s) = &status {
                ui.label(egui::RichText::new(s).small().color(Color32::from_gray(165)));
            }

            // Conversation transcript.
            if show_history && !history_snapshot.is_empty() {
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(170.0)
                    .auto_shrink([false, true])
                    .id_salt(transcript_salt)
                    .show(ui, |ui| {
                        for turn in &history_snapshot {
                            let (tag, colour) = if turn.role == "assistant" {
                                ("AI", Color32::from_rgb(120, 180, 250))
                            } else {
                                ("You", Color32::from_rgb(180, 200, 160))
                            };
                            ui.horizontal_wrapped(|ui| {
                                ui.label(egui::RichText::new(tag).small().strong().color(colour));
                                ui.label(egui::RichText::new(turn.content.trim()).small());
                            });
                            ui.add_space(2.0);
                        }
                    });
            }
        });

        // Restore UI-owned state.
        self.ai_prompt = prompt;
        self.ai_show_history = show_history;

        if do_clear {
            self.ai_history.remove(&path);
            Self::persist_history(project_root, &path, &[]);
            self.ai_status = None;
            self.ai_show_history = false;
        }

        if do_send && !busy {
            self.send_ai_prompt(&path, cfg, code, project_root);
        }

        applied
    }

    /// Conversation storage location: `(project data dir, relative-path key)`.
    /// `None` when there is no open project (conversation stays in memory only).
    fn ai_store_key(
        project_root: Option<&std::path::Path>,
        path: &std::path::Path,
    ) -> Option<(PathBuf, String)> {
        let root = project_root?;
        let data_dir = root.join("data");
        let key = path.strip_prefix(root).ok()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| {
                path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
            });
        Some((data_dir, key))
    }

    /// Persist a target's conversation to the project's indexed file (if any).
    fn persist_history(
        project_root: Option<&std::path::Path>,
        path: &std::path::Path,
        turns: &[crate::llm::ChatTurn],
    ) {
        if let Some((data_dir, key)) = Self::ai_store_key(project_root, path) {
            crate::llm::save_history(&data_dir, &key, turns);
        }
    }

    /// Fire a request for `path` using the current prompt + supplied `code`.
    fn send_ai_prompt(
        &mut self,
        path: &PathBuf,
        cfg: &crate::llm::LlmConfig,
        code: &str,
        project_root: Option<&std::path::Path>,
    ) {
        let prompt = self.ai_prompt.trim().to_string();
        if prompt.is_empty() { return; }
        let filename = path.file_name()
            .and_then(|n| n.to_str()).unwrap_or("source.cbl").to_string();

        let prior = self.ai_history.get(path).cloned().unwrap_or_default();
        let rx = crate::llm::spawn_request(cfg, &prior, &prompt, code, &filename);

        // Record the developer's turn (prompt only, to keep the log readable).
        let log = self.ai_history.entry(path.clone()).or_default();
        log.push(crate::llm::ChatTurn::user(&prompt));
        Self::persist_history(project_root, path, log);

        self.ai_pending = Some((path.clone(), rx));
        self.ai_status = None;
        self.ai_prompt.clear();
        self.ai_show_history = true;
    }

    /// Handle a finished request: log the reply, and return the COBOL to apply
    /// (when the target is editable and the reply carried a code block).
    fn apply_ai_response(
        &mut self,
        path: &PathBuf,
        resp: crate::llm::LlmResponse,
        tr: &crate::i18n::Tr,
        read_only: bool,
        project_root: Option<&std::path::Path>,
    ) -> Option<String> {
        match resp {
            crate::llm::LlmResponse::Ok(reply) => {
                let log = self.ai_history.entry(path.clone()).or_default();
                log.push(crate::llm::ChatTurn::assistant(&reply));
                Self::persist_history(project_root, path, log);

                match crate::llm::extract_code(&reply) {
                    Some(code) if !read_only => {
                        self.ai_status = Some(tr.ai_updated.to_string());
                        Some(code)
                    }
                    Some(_) => {
                        self.ai_status = Some(tr.ai_read_only.to_string());
                        None
                    }
                    None => {
                        self.ai_status = Some(tr.ai_no_code.to_string());
                        None
                    }
                }
            }
            crate::llm::LlmResponse::Err(e) => {
                self.ai_status = Some(e);
                None
            }
        }
    }

    /// The bottom status bar: caret position, Insert/Overwrite mode, a
    /// trim-on-save toggle, and a Beautify command. Dimmed-green text.
    fn show_status_bar(&mut self, ctx: &Context) {
        let frame = egui::Frame::default()
            .fill(ctx.style().visuals.panel_fill)
            .inner_margin(egui::Margin::symmetric(8.0, 3.0));
        TopBottomPanel::bottom("editor_status").frame(frame).show(ctx, |ui| {
            self.status_row(ui);
        });
    }

    /// Draw the status row (caret position · Insert/Overwrite · Trim-on-save ·
    /// Beautify) into an arbitrary `ui`, in dimmed green. Shared by the main
    /// editor's bottom bar and the embedded RAD editor.
    pub(crate) fn status_row(&mut self, ui: &mut egui::Ui) {
        let read_only = self.tabs.get(self.active).map(|t| t.read_only).unwrap_or(false);
        let green = Color32::from_rgb(118, 158, 110); // dimmed green
        let txt = |s: String| egui::RichText::new(s).monospace().size(12.0).color(green);
        let mut do_beautify = false;

        ui.horizontal(|ui| {
            ui.label(txt(format!("Ln {}, Col {}", self.cur_line, self.cur_col)));
            ui.label(txt("│".into()));
            let mode = if self.overwrite { "OVR" } else { "INS" };
            if ui.add(egui::Label::new(txt(mode.into())).sense(egui::Sense::click()))
                .on_hover_text("Toggle Insert/Overwrite (Insert key)")
                .clicked()
            {
                self.overwrite = !self.overwrite;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(!read_only,
                        egui::Button::new(txt("✨ Beautify".into())))
                    .on_hover_text("Tidy whitespace (safe for COBOL columns)")
                    .clicked()
                {
                    do_beautify = true;
                }
                ui.add_enabled(!read_only,
                    egui::Checkbox::new(&mut self.trim_on_save, txt("Trim on save".into())));
            });
        });

        if do_beautify {
            self.beautify_active();
        }
    }

    pub fn show(
        &mut self,
        ctx: &Context,
        llm: Option<&crate::llm::LlmConfig>,
        tr: &crate::i18n::Tr,
        project_root: Option<&std::path::Path>,
    ) {
        // ─── Tab bar ─────────────────────────────────────────────────────────
        TopBottomPanel::top("editor_tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let mut close_idx: Option<usize> = None;
                for (i, tab) in self.tabs.iter().enumerate() {
                    let sel = i == self.active;
                    let resp = ui.selectable_label(sel, tab.title());
                    if resp.clicked() { self.active = i; }
                    if resp.middle_clicked() { close_idx = Some(i); }
                    if sel && ui.small_button("×").clicked() { close_idx = Some(i); }
                    ui.separator();
                }
                if let Some(idx) = close_idx {
                    self.tabs.remove(idx);
                    if self.active >= self.tabs.len() && !self.tabs.is_empty() {
                        self.active = self.tabs.len() - 1;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("A+").on_hover_text("Increase font size").clicked() {
                        self.font_size = (self.font_size + 1.0).min(24.0);
                    }
                    if ui.small_button("A−").on_hover_text("Decrease font size").clicked() {
                        self.font_size = (self.font_size - 1.0).max(8.0);
                    }
                    ui.label(
                        egui::RichText::new(format!("{}pt", self.font_size as u32))
                            .small()
                            .color(Color32::from_gray(160))
                    );
                });
            });
        });

        // ─── AI assistant bar (only when a model is configured) ───────────────
        if let Some(cfg) = llm {
            if cfg.is_configured() && !self.tabs.is_empty() {
                let (tpath, tcode, tro) = {
                    let t = &self.tabs[self.active];
                    (t.path.clone(), t.content.clone(), t.read_only)
                };
                if let Some(new_code) =
                    self.ai_bar(ctx, cfg, tr, "editor_ai", &tpath, &tcode, tro, project_root)
                {
                    if let Some(t) = self.tabs.iter_mut().find(|t| t.path == tpath) {
                        if !t.read_only {
                            t.content = new_code;
                            t.dirty = true;
                        }
                    }
                }
            }
        }

        // ─── Status bar (bottom) ──────────────────────────────────────────────
        if !self.tabs.is_empty() {
            self.show_status_bar(ctx);
        }

        // ─── Editor body ──────────────────────────────────────────────────────
        let body_frame = crate::theme::glass_panel_frame(
            ctx.style().visuals.panel_fill, &crate::theme::active());
        CentralPanel::default().frame(body_frame).show(ctx, |ui| {
            self.render_code_area(ctx, ui);
        });
    }

    /// Render the code area (line numbers + editor + IntelliSense + find/replace
    /// bar) into an arbitrary `ui`. The main editor calls this inside its central
    /// panel; the embedded RAD event editor calls it inside its modal — so both
    /// share identical behaviour.
    pub(crate) fn render_code_area(&mut self, ctx: &Context, ui: &mut egui::Ui) {
            if self.tabs.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new(
                            "Open a COBOL file to get started.\n\n\
                             File → Open  or  toolbar 📂\n\n\
                             Ctrl+Space — trigger completion\n\
                             Cmd+F / Ctrl+F — find in file"
                        )
                        .color(Color32::from_gray(150))
                        .size(15.0),
                    );
                });
                return;
            }

            // ── Global key handling ───────────────────────────────────────────

            // Cmd/Ctrl+F → toggle find bar
            let open_search = ctx.input(|i| i.key_pressed(Key::F) && i.modifiers.command);
            if open_search {
                self.search.visible = !self.search.visible;
                if self.search.visible {
                    self.update_search_matches();
                    ctx.memory_mut(|m| m.request_focus(egui::Id::new("cobolt_search_input")));
                }
            }

            // Key intercept for auto-complete navigation
            let mut key_down    = false;
            let mut key_up      = false;
            let mut key_apply   = false;
            let mut key_dismiss = false;
            let trigger_manual  = ctx.input(|i| i.key_pressed(Key::Space) && i.modifiers.ctrl);

            // Insert key toggles Insert/Overwrite typing mode.
            if ctx.input(|i| i.key_pressed(Key::Insert) && i.modifiers.is_none()) {
                self.overwrite = !self.overwrite;
            }

            // Search key handling (only when search focused)
            let search_has_focus = ctx.memory(|m| m.has_focus(egui::Id::new("cobolt_search_input")));
            if self.search.visible && search_has_focus {
                if ctx.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift) {
                    self.search_next();
                }
                if ctx.input(|i| i.key_pressed(Key::Enter) && i.modifiers.shift) {
                    self.search_prev();
                }
                if ctx.input(|i| i.key_pressed(Key::Escape)) {
                    self.search.visible = false;
                }
            }

            if self.ac.visible && !self.ac.items.is_empty() {
                ctx.input_mut(|inp| {
                    inp.events.retain(|ev| match ev {
                        egui::Event::Key { key: Key::ArrowDown, pressed: true, modifiers, .. }
                            if modifiers.is_none() => { key_down = true; false }
                        egui::Event::Key { key: Key::ArrowUp, pressed: true, modifiers, .. }
                            if modifiers.is_none() => { key_up = true; false }
                        egui::Event::Key { key: Key::Tab, pressed: true, .. } =>
                            { key_apply = true; false }
                        egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. }
                            if modifiers.is_none() => { key_apply = true; false }
                        egui::Event::Key { key: Key::Escape, pressed: true, .. } =>
                            { key_dismiss = true; false }
                        _ => true,
                    });
                });
                if key_down    { self.ac.selected = (self.ac.selected + 1).min(self.ac.items.len().saturating_sub(1)); }
                if key_up      { self.ac.selected = self.ac.selected.saturating_sub(1); }
                if key_dismiss { self.ac.visible = false; }
            }

            // ── Apply selected completion ──────────────────────────────────────
            let mut set_cursor_to: Option<usize> = None;
            if key_apply {
                if let Some(item) = self.ac.items.get(self.ac.selected).cloned() {
                    let tab = &mut self.tabs[self.active];
                    // trigger_pos is a *char* index; convert to byte offset for replace_range.
                    let trigger_byte = tab.content
                        .char_indices()
                        .nth(self.ac.trigger_pos)
                        .map(|(b, _)| b)
                        .unwrap_or(tab.content.len());
                    let end_byte = (trigger_byte + self.ac.prefix.len()).min(tab.content.len());
                    tab.content.replace_range(trigger_byte..end_byte, &item.insert);
                    // set_cursor_to is a *char* count for CCursor
                    let insert_chars = item.insert.chars().count();
                    set_cursor_to = Some(self.ac.trigger_pos + insert_chars);
                    tab.dirty = true;
                }
                self.ac.visible = false;
            }

            // ── Layout ────────────────────────────────────────────────────────
            let font = FontId::monospace(self.font_size);
            let editor_id = egui::Id::new("cobolt_editor");

            let kw_set: std::collections::HashSet<&'static str> = VERBS.iter()
                .chain(DIVISION_KEYWORDS.iter())
                .chain(DATA_KEYWORDS.iter())
                .copied()
                .collect();
            let font_hl = font.clone();
            // Read-only RAD-generated source renders in flat blue (no syntax
            // colours) so it's visually distinct from editable Common Code.
            let read_only = self.tabs.get(self.active).map(|t| t.read_only).unwrap_or(false);
            let mut layouter = move |ui: &egui::Ui, text: &str, _wrap: f32| -> Arc<egui::Galley> {
                let lj = if read_only {
                    mono_layout_job(text, font_hl.clone(), crate::theme::active().ed_generated)
                } else {
                    cobol_layout_job(text, font_hl.clone(), &kw_set)
                };
                ui.fonts(|f| f.layout_job(lj))
            };

            let avail = ui.available_size();
            let editor_rect_cell: std::cell::Cell<egui::Rect> =
                std::cell::Cell::new(egui::Rect::NOTHING);

            ScrollArea::both()
                .id_salt("cobolt_editor_scroll")
                .auto_shrink([false, false])
                .min_scrolled_height(avail.y)
                .show(ui, |ui| {
                    ui.set_min_height(avail.y);
                    ui.horizontal_top(|ui| {
                        // ── Breakpoint + line-number gutter ──────────────────
                        if self.show_line_numbers {
                            let n_lines = self.tabs[self.active].content.lines().count().max(1);
                            let line_h  = self.font_size * 1.45;
                            // Gutter width: 8px bp zone + 4px gap + ~36px line numbers
                            let gutter_w = 54.0_f32;
                            let (gutter_rect, gutter_resp) = ui.allocate_exact_size(
                                egui::vec2(gutter_w, line_h * n_lines as f32),
                                egui::Sense::click(),
                            );

                            // Handle click → toggle breakpoint.
                            if gutter_resp.clicked() {
                                if let Some(pos) = gutter_resp.interact_pointer_pos() {
                                    let rel_y = pos.y - gutter_rect.min.y;
                                    let clicked_line = (rel_y / line_h).floor() as u32 + 1;
                                    self.toggle_breakpoint(clicked_line);
                                }
                            }

                            // Paint gutter.
                            let painter = ui.painter_at(gutter_rect);
                            let active_bp  = self.active_breakpoints();
                            let debug_line = self.debug_line.as_ref().and_then(|(p, l)| {
                                self.tabs.get(self.active).filter(|t| t.path == *p).map(|_| *l)
                            });

                            for line_idx in 0..n_lines {
                                let line_num = (line_idx + 1) as u32;
                                let y = gutter_rect.min.y + line_idx as f32 * line_h;

                                // Debug arrow (current paused line).
                                if debug_line == Some(line_num) {
                                    painter.rect_filled(
                                        egui::Rect::from_min_size(
                                            egui::pos2(gutter_rect.min.x, y),
                                            egui::vec2(gutter_w, line_h),
                                        ),
                                        0.0,
                                        Color32::from_rgba_unmultiplied(255, 220, 0, 40),
                                    );
                                    painter.text(
                                        egui::pos2(gutter_rect.min.x + 4.0, y + line_h * 0.5),
                                        egui::Align2::LEFT_CENTER,
                                        "→",
                                        FontId::monospace(self.font_size - 1.0),
                                        Color32::from_rgb(255, 200, 0),
                                    );
                                }

                                // Breakpoint dot.
                                let has_bp = active_bp.map(|s| s.contains(&line_num)).unwrap_or(false);
                                if has_bp {
                                    let dot_cx = gutter_rect.min.x + 6.0;
                                    let dot_cy = y + line_h * 0.5;
                                    painter.circle_filled(
                                        egui::pos2(dot_cx, dot_cy),
                                        4.5,
                                        Color32::from_rgb(220, 60, 60),
                                    );
                                }

                                // Line number.
                                painter.text(
                                    egui::pos2(gutter_rect.max.x - 4.0, y + line_h * 0.5),
                                    egui::Align2::RIGHT_CENTER,
                                    format!("{line_num}"),
                                    FontId::monospace(self.font_size - 1.0),
                                    Color32::from_gray(if debug_line == Some(line_num) { 220 } else { 100 }),
                                );
                            }

                            ui.add(egui::Separator::default().vertical().spacing(2.0));
                        }

                        // ── Overwrite mode ────────────────────────────────────
                        // egui's TextEdit is insert-only, so we emulate overwrite:
                        // when a printable character is about to be typed and the
                        // caret is collapsed, pre-select the next character so the
                        // insert replaces it (unless at end-of-line / EOF).
                        if self.overwrite && !self.tabs[self.active].read_only {
                            let typing = ctx.input(|i| i.events.iter().any(|e|
                                matches!(e, egui::Event::Text(t)
                                    if t.chars().any(|c| !c.is_control()))));
                            if typing {
                                let content = self.tabs[self.active].content.clone();
                                if let Some(mut st) = egui::TextEdit::load_state(ctx, editor_id) {
                                    if let Some(range) = st.cursor.char_range() {
                                        if range.primary == range.secondary {
                                            let idx = range.primary.index;
                                            let next = content.chars().nth(idx);
                                            if matches!(next, Some(c) if c != '\n') {
                                                let mut r = range;
                                                r.secondary.index = idx + 1;
                                                st.cursor.set_char_range(Some(r));
                                                st.store(ctx, editor_id);
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // ── Source (read-only for RAD-generated code) ─────────
                        let tab = &mut self.tabs[self.active];
                        let te_out = TextEdit::multiline(&mut tab.content)
                            .id(editor_id)
                            .font(font.clone())
                            .desired_width(f32::INFINITY)
                            .lock_focus(true)
                            .interactive(!tab.read_only)
                            .layouter(&mut layouter)
                            .show(ui);

                        editor_rect_cell.set(te_out.response.rect);

                        if te_out.response.changed() && !tab.read_only { tab.dirty = true; }

                        // Reposition cursor (completion apply or search navigate)
                        // Search navigation: set cursor to match position
                        if self.search.needs_scroll {
                            self.search.needs_scroll = false;
                            if let Some(&byte_off) = self.search.matches.get(self.search.current) {
                                let char_idx = tab.content[..byte_off.min(tab.content.len())]
                                    .chars().count();
                                set_cursor_to = Some(char_idx);
                                // Scroll the viewport so the match is visible
                                let content_before = &tab.content[..byte_off.min(tab.content.len())];
                                let line_num = content_before.matches('\n').count();
                                let line_h   = self.font_size * 1.45;
                                let match_y  = te_out.galley_pos.y + line_num as f32 * line_h;
                                ui.scroll_to_rect(
                                    egui::Rect::from_min_size(
                                        Pos2::new(te_out.response.rect.min.x, match_y),
                                        egui::Vec2::new(1.0, line_h),
                                    ),
                                    Some(egui::Align::Center),
                                );
                            }
                        }

                        if let Some(pos) = set_cursor_to {
                            if let Some(mut state) = egui::TextEdit::load_state(ctx, te_out.response.id) {
                                let cc = egui::text::CCursor::new(pos);
                                state.cursor.set_char_range(Some(
                                    egui::text::CCursorRange::one(cc)
                                ));
                                state.store(ctx, te_out.response.id);
                            }
                            // Move keyboard focus into the editor so the cursor
                            // is visible and the user can type immediately.
                            ctx.memory_mut(|m| m.request_focus(editor_id));
                        }

                        // ── IntelliSense update ───────────────────────────────
                        if let Some(cr) = te_out.cursor_range {
                            let char_idx = cr.primary.ccursor.index;
                            let (l, c) = char_index_to_line_col(&tab.content, char_idx);
                            self.cur_line = l;
                            self.cur_col  = c;
                            let (word_start, prefix) =
                                word_before_cursor(&tab.content, char_idx);

                            // Detect INVOKE … ' context → method completions
                            let invoke = detect_invoke_context(
                                &tab.content, char_idx, &self.known_controls,
                            );

                            // Detect exact control ID → member (property+method) popup
                            let member_ctrl = if invoke.is_none() {
                                detect_control_exact(&prefix, &self.known_controls)
                            } else {
                                None
                            };

                            let refresh = trigger_manual
                                || (te_out.response.changed() && prefix.len() >= 2)
                                || (te_out.response.changed() && invoke.is_some())
                                || (te_out.response.changed() && member_ctrl.is_some());

                            if refresh || (self.ac.visible && prefix.len() >= 1) {
                                let (items, member_mode) = if let Some((ctrl_id, ctrl_type, method_pfx)) = &invoke {
                                    // Inside INVOKE ctrl-id '…' → filter methods
                                    let _ = ctrl_id;
                                    let v = methods_for_type(ctrl_type)
                                        .iter()
                                        .filter(|(m, _)| m.to_lowercase().starts_with(&method_pfx.to_lowercase()))
                                        .map(|(m, d)| AcItem::method(m, d))
                                        .collect::<Vec<_>>();
                                    (v, true)
                                } else if let Some((ctrl_type, member_pfx)) = &member_ctrl {
                                    // Exact control ID typed → show properties + methods
                                    let up = member_pfx.to_ascii_uppercase();
                                    let mut v: Vec<AcItem> = properties_for_type(ctrl_type)
                                        .iter()
                                        .filter(|(p, _)| p.to_ascii_uppercase().starts_with(&up))
                                        .map(|(p, d)| AcItem::prop(p, d))
                                        .collect();
                                    for (m, d) in methods_for_type(ctrl_type)
                                        .iter()
                                        .filter(|(m, _)| m.to_ascii_uppercase().starts_with(&up))
                                    {
                                        v.push(AcItem::method(m, d));
                                    }
                                    (v, true)
                                } else {
                                    (build_completions(&prefix, &tab.content, &self.known_controls), false)
                                };

                                if !items.is_empty() {
                                    let ppos = {
                                        // Use galley-based exact cursor position when available
                                        let cr_rect      = te_out.galley.pos_from_cursor(&cr.primary);
                                        let raw_x        = te_out.galley_pos.x + cr_rect.min.x;
                                        let raw_y        = te_out.galley_pos.y + cr_rect.max.y + 4.0;
                                        let cursor_top_y = te_out.galley_pos.y + cr_rect.min.y;
                                        let scr          = ctx.screen_rect();
                                        let popup_h      = 280.0_f32;
                                        let popup_w      = 480.0_f32;
                                        // Clamp horizontally so popup stays on screen
                                        let x = raw_x.min(scr.max.x - popup_w - 8.0).max(scr.min.x + 4.0);
                                        // If popup would clip the bottom, show it above the cursor instead
                                        let y = if raw_y + popup_h > scr.max.y {
                                            (cursor_top_y - popup_h - 4.0).max(scr.min.y)
                                        } else {
                                            raw_y
                                        };
                                        Pos2::new(x, y)
                                    };
                                    if !self.ac.visible || self.ac.prefix != prefix || self.ac.member_mode != member_mode {
                                        self.ac.selected = 0;
                                    }
                                    self.ac.visible     = true;
                                    self.ac.member_mode = member_mode;
                                    self.ac.items       = items;
                                    self.ac.prefix      = prefix.clone();
                                    self.ac.trigger_pos = word_start;
                                    self.ac.popup_pos   = ppos;
                                } else if !self.ac.member_mode {
                                    // Only auto-dismiss if NOT in member mode (member mode
                                    // dismisses only on non-matching keystrokes or Esc).
                                    if prefix.is_empty() || prefix.len() < 2 {
                                        self.ac.visible = false;
                                    }
                                } else {
                                    // In member mode: dismiss when prefix no longer matches any member
                                    self.ac.visible = false;
                                    self.ac.member_mode = false;
                                }
                            }
                        }
                        // NOTE: do NOT dismiss the popup when cursor_range is None.
                        // That happens on the same frame the user clicks a popup row
                        // (the click briefly steals focus from the TextEdit). Dismissal
                        // is handled explicitly via Escape, a successful insertion, or
                        // the prefix-no-longer-matches path above.
                    });
                });

            // ─── Auto-completion popup ────────────────────────────────────────
            if self.ac.visible && !self.ac.items.is_empty() {
                let popup_pos  = self.ac.popup_pos;
                let items      = self.ac.items.clone();
                let selected   = self.ac.selected;
                let member_mode = self.ac.member_mode;
                let mut clicked: Option<usize> = None;

                egui::Area::new(egui::Id::new("cobolt_ac_popup"))
                    .fixed_pos(popup_pos)
                    .order(egui::Order::Tooltip)
                    .interactable(true)
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style())
                            .rounding(egui::Rounding::same(7.0))
                            .show(ui, |ui| {
                                ui.set_min_width(320.0);
                                ui.set_max_width(480.0);

                                if member_mode {
                                    ui.label(
                                        egui::RichText::new("  Properties & Methods")
                                            .small()
                                            .color(Color32::from_gray(160))
                                    );
                                    ui.separator();
                                }

                                ScrollArea::vertical()
                                    .id_salt("ac_list")
                                    .max_height(220.0)
                                    .show(ui, |ui| {
                                        for (i, item) in items.iter().enumerate() {
                                            let is_sel = i == selected;
                                            let (badge, badge_col) = item.badge();

                                            let row_frame = if is_sel {
                                                egui::Frame::default()
                                                    .fill(Color32::from_rgba_unmultiplied(65, 115, 225, 170))
                                                    .rounding(egui::Rounding::same(4.0))
                                            } else {
                                                egui::Frame::default()
                                            };

                                            let row_resp = row_frame.show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(badge)
                                                            .monospace()
                                                            .size(10.0)
                                                            .color(badge_col)
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(&item.label)
                                                            .monospace()
                                                            .size(12.0)
                                                    );
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(egui::Align::Center),
                                                        |ui| {
                                                            ui.label(
                                                                egui::RichText::new(&item.detail)
                                                                    .small()
                                                                    .color(Color32::from_gray(145))
                                                            );
                                                        },
                                                    );
                                                });
                                            });

                                            // Frame responses only sense hover by default.
                                            // Use ui.interact() over the same rect with a
                                            // unique per-row ID to properly detect clicks.
                                            let click_resp = ui.interact(
                                                row_resp.response.rect,
                                                egui::Id::new("ac_row").with(i),
                                                egui::Sense::click(),
                                            ).on_hover_cursor(egui::CursorIcon::PointingHand);
                                            if click_resp.clicked() {
                                                clicked = Some(i);
                                            }
                                        }
                                    });

                                ui.separator();
                                ui.label(
                                    egui::RichText::new(
                                        "↑↓ navigate   Tab/↵ insert   Esc dismiss   Ctrl+Space force"
                                    )
                                    .small()
                                    .color(Color32::from_gray(130)),
                                );
                            });
                    });

                if let Some(idx) = clicked {
                    if let Some(item) = self.ac.items.get(idx).cloned() {
                        if let Some(tab) = self.tabs.get_mut(self.active) {
                            // trigger_pos is a char index; convert to byte offset.
                            let trigger_byte = tab.content
                                .char_indices()
                                .nth(self.ac.trigger_pos)
                                .map(|(b, _)| b)
                                .unwrap_or(tab.content.len());
                            let end_byte = (trigger_byte + self.ac.prefix.len())
                                .min(tab.content.len());
                            tab.content.replace_range(trigger_byte..end_byte, &item.insert);
                            tab.dirty = true;
                            // Move cursor (char index) to end of inserted text, restore focus.
                            let new_pos = self.ac.trigger_pos + item.insert.chars().count();
                            if let Some(mut state) = egui::TextEdit::load_state(ctx, editor_id) {
                                let cc = egui::text::CCursor::new(new_pos);
                                state.cursor.set_char_range(Some(egui::text::CCursorRange::one(cc)));
                                state.store(ctx, editor_id);
                            }
                            ctx.memory_mut(|m| m.request_focus(editor_id));
                        }
                    }
                    self.ac.visible = false;
                    self.ac.member_mode = false;
                }
            }

            // ─── Find / Search bar (Cmd+F) ─────────────────────────────────────
            if self.search.visible && !self.tabs.is_empty() {
                let editor_rect = editor_rect_cell.get();
                // Anchor: top-right corner of the editor area
                let bar_w   = 320.0_f32;
                let bar_x   = (editor_rect.max.x - bar_w - 8.0).max(editor_rect.min.x);
                let bar_y   = editor_rect.min.y + 6.0;

                let prev_query = self.search.query.clone();
                let active_ro  = self.tabs.get(self.active).map(|t| t.read_only).unwrap_or(false);
                let mut do_replace_one = false;
                let mut do_replace_all = false;

                egui::Area::new(egui::Id::new("cobolt_search_bar"))
                    .fixed_pos(Pos2::new(bar_x, bar_y))
                    .order(egui::Order::Foreground)
                    .interactable(true)
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style())
                            .rounding(egui::Rounding::same(7.0))
                            .inner_margin(egui::Margin::same(6.0))
                            .show(ui, |ui| {
                                ui.set_min_width(bar_w - 12.0);
                                ui.horizontal(|ui| {
                                    // Search icon label
                                    ui.label(
                                        egui::RichText::new("🔍")
                                            .size(13.0)
                                            .color(Color32::from_gray(160))
                                    );

                                    // Query text input
                                    let te_resp = ui.add(
                                        TextEdit::singleline(&mut self.search.query)
                                            .id(egui::Id::new("cobolt_search_input"))
                                            .desired_width(165.0)
                                            .hint_text("Find…")
                                    );

                                    if te_resp.changed() || self.search.query != prev_query {
                                        self.update_search_matches();
                                        if !self.search.matches.is_empty() {
                                            self.search.needs_scroll = true;
                                        }
                                    }

                                    // Match counter
                                    let total = self.search.matches.len();
                                    let (count_txt, count_col) = if self.search.query.is_empty() {
                                        ("".to_owned(), Color32::from_gray(140))
                                    } else if total == 0 {
                                        ("No matches".to_owned(), Color32::from_rgb(255, 100, 100))
                                    } else {
                                        let cur = self.search.current + 1;
                                        (format!("{cur}/{total}"), Color32::from_gray(200))
                                    };
                                    ui.label(
                                        egui::RichText::new(count_txt)
                                            .small()
                                            .color(count_col)
                                    );

                                    ui.separator();

                                    // < previous match
                                    if ui.small_button("<")
                                        .on_hover_text("Previous match (Shift+Enter)")
                                        .clicked()
                                    {
                                        self.search_prev();
                                    }
                                    // > next match
                                    if ui.small_button(">")
                                        .on_hover_text("Next match (Enter)")
                                        .clicked()
                                    {
                                        self.search_next();
                                    }
                                    // ✕ close
                                    if ui.small_button("✕")
                                        .on_hover_text("Close (Esc)")
                                        .clicked()
                                    {
                                        self.search.visible = false;
                                    }
                                });

                                // ── Replace row ───────────────────────────────
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("⇄")
                                            .size(13.0)
                                            .color(Color32::from_gray(160))
                                    );
                                    ui.add(
                                        TextEdit::singleline(&mut self.search.replace)
                                            .id(egui::Id::new("cobolt_replace_input"))
                                            .desired_width(165.0)
                                            .hint_text("Replace…")
                                    );
                                    ui.separator();
                                    let can = !active_ro && !self.search.query.is_empty()
                                        && !self.search.matches.is_empty();
                                    if ui.add_enabled(can, egui::Button::new("Replace").small())
                                        .on_hover_text("Replace this match")
                                        .clicked()
                                    {
                                        do_replace_one = true;
                                    }
                                    if ui.add_enabled(can, egui::Button::new("All").small())
                                        .on_hover_text("Replace all matches")
                                        .clicked()
                                    {
                                        do_replace_all = true;
                                    }
                                });

                                ui.label(
                                    egui::RichText::new("Enter = next   Shift+Enter = prev   Esc = close")
                                        .small()
                                        .color(Color32::from_gray(120))
                                );
                            });
                    });

                if do_replace_one { self.replace_current(); }
                if do_replace_all { self.replace_all(); }
            }
    }
}

// ── Status-bar / save helpers ──────────────────────────────────────────────────

/// Case-insensitive (ASCII) replace-all. COBOL source is ASCII, so we match on
/// ASCII-folded bytes and copy any non-matching UTF-8 char through verbatim.
fn replace_all_ci(haystack: &str, needle: &str, repl: &str) -> String {
    if needle.is_empty() { return haystack.to_string(); }
    let hb = haystack.as_bytes();
    let nb = needle.as_bytes();
    let mut out = String::with_capacity(haystack.len());
    let mut i = 0;
    while i < hb.len() {
        if i + nb.len() <= hb.len() && hb[i..i + nb.len()].eq_ignore_ascii_case(nb) {
            out.push_str(repl);
            i += nb.len();
        } else {
            let ch = haystack[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// 1-based (line, column) for a char index into `text`.
fn char_index_to_line_col(text: &str, char_idx: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col  = 1usize;
    for (i, ch) in text.chars().enumerate() {
        if i >= char_idx { break; }
        if ch == '\n' { line += 1; col = 1; } else { col += 1; }
    }
    (line, col)
}

/// Trim trailing spaces/tabs from every line, preserving line endings.
fn trim_trailing_ws(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let ends_with_nl = text.ends_with('\n');
    let mut lines = text.split('\n').peekable();
    while let Some(line) = lines.next() {
        out.push_str(line.trim_end_matches([' ', '\t']));
        if lines.peek().is_some() {
            out.push('\n');
        }
    }
    if ends_with_nl && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum CobolDiv { Ident, Env, Data, Proc }

#[derive(Clone, Copy, PartialEq)]
enum CobolScope { If, Evaluate, When, Perform }

/// **Beautify**: re-format free-format COBOL to the standard column layout.
///
///   * comment lines → indicator (`*` / `*>`) in **column 7**,
///   * divisions, sections, paragraphs and `01`/`77`/`78` items → **Area A**
///     (column 8),
///   * PROCEDURE statements and lower-level data items → **Area B** (column 12),
///   * nested blocks (`IF` / `EVALUATE` / inline `PERFORM`) indented **4 spaces**
///     per level, honouring scope terminators (`END-…`, `ELSE`, `WHEN`) and the
///     period that ends a sentence,
///   * runs of spaces collapsed to one — **except** the gap that separates a
///     `PIC` clause from its data name (alignment is preserved),
///   * no fixed-format column-72 limit (free format), trailing blank lines and
///     consecutive blank lines trimmed.
fn beautify_cobol(text: &str) -> String {
    let reserved: std::collections::HashSet<&'static str> = VERBS.iter()
        .chain(DIVISION_KEYWORDS.iter())
        .chain(DATA_KEYWORDS.iter())
        .copied()
        .collect();

    let mut out  = String::with_capacity(text.len());
    let mut div  = CobolDiv::Ident;
    let mut scopes: Vec<CobolScope> = Vec::new();
    let mut prev_blank = false;

    // Append `content` at 1-based `col` (col-1 leading spaces).
    fn put(out: &mut String, col: usize, content: &str) {
        for _ in 1..col { out.push(' '); }
        out.push_str(content);
        out.push('\n');
    }
    let word_at = |words: &[&str], i: usize| -> String {
        words.get(i).map(|w| w.trim_end_matches('.').to_string()).unwrap_or_default()
    };

    for raw in text.lines() {
        let t = raw.trim();
        if t.is_empty() {
            if !prev_blank { out.push('\n'); prev_blank = true; }
            continue;
        }
        prev_blank = false;

        // Full-line comment → indicator in column 7.
        if t.starts_with("*>") || t.starts_with('*') || t.starts_with('/') {
            put(&mut out, 7, t);
            continue;
        }

        let content = collapse_spaces_keep_pic(t);
        let upper = content.to_ascii_uppercase();
        let words: Vec<&str> = upper.split_whitespace().collect();
        let first = words.first().copied().unwrap_or("");
        let ends_period = content.trim_end().ends_with('.');

        // Division header → Area A, and switch context.
        if word_at(&words, 1) == "DIVISION"
            && matches!(first, "IDENTIFICATION" | "ID" | "ENVIRONMENT" | "DATA" | "PROCEDURE")
        {
            put(&mut out, 8, &content);
            div = match first {
                "PROCEDURE"   => CobolDiv::Proc,
                "DATA"        => CobolDiv::Data,
                "ENVIRONMENT" => CobolDiv::Env,
                _             => CobolDiv::Ident,
            };
            scopes.clear();
            continue;
        }

        // Section header → Area A.
        if word_at(&words, 1) == "SECTION" {
            put(&mut out, 8, &content);
            scopes.clear();
            continue;
        }

        match div {
            CobolDiv::Proc => {
                // Paragraph header: a lone `name.` that is not a verb.
                if words.len() == 1 && first.ends_with('.')
                    && !reserved.contains(first.trim_end_matches('.'))
                {
                    put(&mut out, 8, &content);
                    scopes.clear();
                    continue;
                }

                // Dedent-before for terminators / case labels.
                if first.starts_with("END-") {
                    if first == "END-EVALUATE"
                        && matches!(scopes.last(), Some(CobolScope::When))
                    {
                        scopes.pop(); // close a trailing WHEN body
                    }
                    scopes.pop();
                } else if first == "WHEN"
                    && matches!(scopes.last(), Some(CobolScope::When))
                {
                    scopes.pop(); // close the previous WHEN body
                }

                let mut level = scopes.len();
                if first == "ELSE" { level = level.saturating_sub(1); }
                put(&mut out, 12 + level * 4, &content);

                // Indent-after for openers (only when the scope continues onto
                // following lines — no inline terminator and no closing period).
                if !ends_period {
                    match first {
                        "IF"       if !upper.contains("END-IF")       => scopes.push(CobolScope::If),
                        "EVALUATE" if !upper.contains("END-EVALUATE") => scopes.push(CobolScope::Evaluate),
                        "WHEN"                                        => scopes.push(CobolScope::When),
                        "PERFORM"  if !upper.contains("END-PERFORM")
                                      && is_inline_perform(&words)    => scopes.push(CobolScope::Perform),
                        _ => {}
                    }
                }
                // A period ends the sentence → all in-line scopes close.
                if ends_period { scopes.clear(); }
            }
            CobolDiv::Data => {
                if matches!(first, "FD" | "SD" | "RD" | "CD") {
                    put(&mut out, 8, &content);
                } else if let Some(level) = cobol_leading_level(&content) {
                    let col = if matches!(level, 1 | 77 | 78) { 8 } else { 12 };
                    put(&mut out, col, &content);
                } else {
                    put(&mut out, 12, &content); // a continued clause
                }
            }
            CobolDiv::Env | CobolDiv::Ident => {
                // Paragraph entries (`PROGRAM-ID.`, `SOURCE-COMPUTER.`, …) sit in
                // Area A; anything else (a clause) goes to Area B.
                if first.ends_with('.') {
                    put(&mut out, 8, &content);
                } else {
                    put(&mut out, 12, &content);
                }
            }
        }
    }

    while out.ends_with("\n\n") { out.pop(); }
    out
}

/// Collapse runs of 2+ spaces to one, but preserve the gap immediately before a
/// `PIC` / `PICTURE` clause (data-item alignment) and never touch the contents
/// of `"…"` / `'…'` string literals.
fn collapse_spaces_keep_pic(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut quote: Option<char> = None;
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            out.push(c);
            if c == q { quote = None; }
            i += 1;
            continue;
        }
        if c == '"' || c == '\'' { quote = Some(c); out.push(c); i += 1; continue; }
        if c == ' ' {
            let start = i;
            while i < chars.len() && chars[i] == ' ' { i += 1; }
            let run = i - start;
            let rest: String = chars[i..].iter().collect();
            let next = rest.trim_start().to_ascii_uppercase();
            let before_pic = next.starts_with("PIC ") || next.starts_with("PICTURE");
            if before_pic && run > 1 {
                for _ in 0..run { out.push(' '); } // keep alignment
            } else {
                out.push(' ');
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// The leading level number of a data-description entry (e.g. `01`, `05`, `77`).
fn cobol_leading_level(s: &str) -> Option<u32> {
    s.split_whitespace().next()?.parse::<u32>().ok()
}

/// True when a `PERFORM` opens an *in-line* body (closed by `END-PERFORM`)
/// rather than calling an out-of-line paragraph.
fn is_inline_perform(words: &[&str]) -> bool {
    match words.get(1).copied() {
        None => true,                                              // bare PERFORM
        Some("UNTIL") | Some("VARYING") | Some("WITH") | Some("FOREVER") => true,
        _ => words.last().map_or(false, |w| w.trim_end_matches('.') == "TIMES"),
    }
}

// ── Completion helpers ────────────────────────────────────────────────────────

/// Returns `(word_start_char_idx, prefix)` for the identifier immediately
/// before the cursor (COBOL identifiers include hyphens).
fn word_before_cursor(text: &str, cursor_char: usize) -> (usize, String) {
    let char_indices: Vec<(usize, char)> = text.char_indices().collect();
    let cursor_byte = char_indices
        .get(cursor_char)
        .map(|(b, _)| *b)
        .unwrap_or(text.len());

    let prefix_text = &text[..cursor_byte];
    let word_start_byte = prefix_text
        .rfind(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .map(|p| p + 1)
        .unwrap_or(0);

    let prefix = prefix_text[word_start_byte..].to_owned();
    let word_start_char = text[..word_start_byte].chars().count();
    (word_start_char, prefix)
}

/// When the current word **exactly matches** a known control ID, return
/// `(ctrl_type, member_prefix)` where `member_prefix` is the text of the
/// **next** word being typed (the property/method being filtered).
///
/// We look one word back: if the word just behind the cursor is a known
/// control ID AND the cursor is now at the start of a new word (or in an
/// empty gap), we enter member mode with that empty prefix.
fn detect_control_exact<'a>(
    prefix: &str,
    controls: &'a [KnownControl],
) -> Option<(String, String)> {
    // Case 1: the currently typed word IS a known control ID exactly.
    if let Some(ctrl) = controls.iter().find(|c| c.id.eq_ignore_ascii_case(prefix)) {
        return Some((ctrl.ctrl_type.clone(), String::new()));
    }
    None
}

/// Detect `INVOKE ctrl-id '` or `ctrl-id::` patterns.
fn detect_invoke_context(
    text: &str,
    cursor_char: usize,
    controls: &[KnownControl],
) -> Option<(String, String, String)> {
    let char_indices: Vec<(usize, char)> = text.char_indices().collect();
    let cursor_byte = char_indices
        .get(cursor_char)
        .map(|(b, _)| *b)
        .unwrap_or(text.len());
    let line_start = text[..cursor_byte].rfind('\n').map(|p| p + 1).unwrap_or(0);
    let line = &text[line_start..cursor_byte];
    let upper = line.to_ascii_uppercase();

    // ── INVOKE ctrl-id 'prefix ────────────────────────────────────────────
    if let Some(inv_pos) = upper.find("INVOKE ") {
        let after = line[inv_pos + 7..].trim_start();
        if let Some(sp) = after.find(|c: char| c.is_whitespace()) {
            let ctrl_tok = after[..sp].to_ascii_uppercase();
            let rest = after[sp..].trim_start();
            if rest.starts_with('\'') || rest.starts_with('"') {
                let mprefix = &rest[1..];
                let ctrl_type = controls
                    .iter()
                    .find(|c| c.id.eq_ignore_ascii_case(&ctrl_tok))
                    .map(|c| c.ctrl_type.clone())
                    .unwrap_or_else(|| "Generic".into());
                return Some((ctrl_tok, ctrl_type, mprefix.into()));
            }
        }
    }

    // ── ctrl-id:: ─────────────────────────────────────────────────────────
    if let Some(pos) = line.rfind("::") {
        let before = line[..pos].trim_end();
        let ctrl_tok = before
            .rsplit(|c: char| c.is_whitespace())
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        let mprefix = &line[pos + 2..];
        let ctrl_type = controls
            .iter()
            .find(|c| c.id.eq_ignore_ascii_case(&ctrl_tok))
            .map(|c| c.ctrl_type.clone())
            .unwrap_or_else(|| "Generic".into());
        return Some((ctrl_tok, ctrl_type, mprefix.into()));
    }

    None
}

/// Build the completion list for a given prefix string.
fn build_completions(
    prefix: &str,
    source: &str,
    controls: &[KnownControl],
) -> Vec<AcItem> {
    let up = prefix.to_ascii_uppercase();
    let mut seen: std::collections::HashSet<String> = Default::default();
    let mut items: Vec<AcItem> = Vec::new();

    // ── 1. Snippets ───────────────────────────────────────────────────────
    const SNIPPETS: &[(&str, &str, &str)] = &[
        ("IF",            "IF \nEND-IF",                                                           "IF … END-IF"),
        ("EVALUATE",      "EVALUATE \n    WHEN \n        CONTINUE\n    WHEN OTHER\n        CONTINUE\nEND-EVALUATE", "EVALUATE block"),
        ("PERFORM",       "PERFORM \nEND-PERFORM",                                                 "PERFORM … END-PERFORM"),
        ("PERFORM UNTIL", "PERFORM UNTIL  = 1\n    \nEND-PERFORM",                                "Loop with condition"),
        ("MOVE",          "MOVE  TO ",                                                             "Move value"),
        ("INVOKE",        "INVOKE \"\" ''\n    USING BY VALUE \n    RETURNING ",                   "OO method call"),
        ("SET",           "SET  TO ",                                                              "Set variable / OO call"),
        ("CALL",          "CALL \"\" USING \nEND-CALL",                                           "Static sub-program call"),
        ("DISPLAY",       "DISPLAY \"\"",                                                         "Display text"),
        ("ACCEPT",        "ACCEPT  FROM ",                                                         "Accept input"),
        ("COMPUTE",       "COMPUTE  = ",                                                           "Arithmetic"),
        ("ADD",           "ADD  TO ",                                                              "Add"),
        ("SUBTRACT",      "SUBTRACT  FROM ",                                                       "Subtract"),
        ("MULTIPLY",      "MULTIPLY  BY  GIVING ",                                                 "Multiply"),
        ("DIVIDE",        "DIVIDE  INTO  GIVING ",                                                 "Divide"),
        ("STOP RUN",      "STOP RUN",                                                             "End program"),
        ("GOBACK",        "GOBACK",                                                               "Return to caller"),
        ("IDENTIFICATION DIVISION", "IDENTIFICATION DIVISION.\nPROGRAM-ID. .\n",                  "Program header"),
        ("DATA DIVISION", "DATA DIVISION.\nWORKING-STORAGE SECTION.\n",                           "Data division"),
        ("PROCEDURE DIVISION", "PROCEDURE DIVISION.\n",                                           "Procedure division"),
        ("COBOLT-SET-PROPERTY",
         "CALL 'COBOLT-SET-PROPERTY'\n    USING BY VALUE \n          BY VALUE \n          BY VALUE .",
         "Set control property at runtime"),
        ("COBOLT-GET-PROPERTY",
         "CALL 'COBOLT-GET-PROPERTY'\n    USING BY VALUE \n          BY VALUE \n          BY REFERENCE .",
         "Get control property at runtime"),
    ];
    for (label, insert, detail) in SNIPPETS {
        if label.to_ascii_uppercase().starts_with(&up) {
            let key = label.to_ascii_uppercase();
            if seen.insert(key) { items.push(AcItem::snip(label, insert, detail)); }
        }
    }

    // ── 2. COBOL keywords ─────────────────────────────────────────────────
    for &kw in VERBS.iter().chain(DIVISION_KEYWORDS).chain(DATA_KEYWORDS) {
        if kw.starts_with(&up) && seen.insert(kw.into()) {
            items.push(AcItem::kw(kw));
        }
    }

    // ── 3. Paragraph names ────────────────────────────────────────────────
    for p in extract_paragraphs(source) {
        if p.to_ascii_uppercase().starts_with(&up) && seen.insert(p.to_ascii_uppercase()) {
            items.push(AcItem::para(&p));
        }
    }

    // ── 4. Data items ─────────────────────────────────────────────────────
    for d in extract_data_items(source) {
        if d.to_ascii_uppercase().starts_with(&up) && seen.insert(d.to_ascii_uppercase()) {
            items.push(AcItem::data(&d));
        }
    }

    // ── 5. Known form controls ────────────────────────────────────────────
    for ctrl in controls {
        if ctrl.id.to_ascii_uppercase().starts_with(&up)
            && seen.insert(ctrl.id.to_ascii_uppercase())
        {
            items.push(AcItem::ctrl(&ctrl.id, &ctrl.ctrl_type));
        }
    }

    items.truncate(25);
    items
}

fn extract_paragraphs(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in source.lines() {
        let t = line.trim();
        if t.starts_with("*>") || t.starts_with('*') { continue; }
        if t.ends_with("DIVISION.") || t.ends_with("SECTION.") { continue; }
        if t.ends_with('.') {
            let candidate = &t[..t.len() - 1];
            let words: Vec<&str> = candidate.split_whitespace().collect();
            if words.len() == 1 {
                let w = words[0];
                if w.len() > 2
                    && w.chars().all(|c| c.is_alphanumeric() || c == '-')
                    && w.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false)
                    && !DIVISION_KEYWORDS.contains(&w.to_ascii_uppercase().as_str())
                {
                    out.push(w.into());
                }
            }
        }
    }
    out
}

fn extract_data_items(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_data = false;
    for line in source.lines() {
        let upper = line.to_ascii_uppercase();
        if upper.contains("WORKING-STORAGE") || upper.contains("LOCAL-STORAGE") || upper.contains("LINKAGE")
        { in_data = true; }
        if upper.contains("PROCEDURE DIVISION") { in_data = false; }
        if !in_data { continue; }
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.len() >= 2 && parts[0].chars().all(|c| c.is_ascii_digit()) {
            let name = parts[1];
            if name != "FILLER"
                && name.chars().all(|c| c.is_alphanumeric() || c == '-')
                && name.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false)
            {
                out.push(name.into());
            }
        }
    }
    out
}

fn cursor_screen_pos(
    text: &str,
    cursor_char: usize,
    editor_rect: egui::Rect,
    font_size: f32,
) -> Pos2 {
    let cursor_byte = text.char_indices().nth(cursor_char).map(|(b, _)| b).unwrap_or(text.len());
    let before = &text[..cursor_byte];
    let lines: Vec<&str> = before.split('\n').collect();
    let line_num = lines.len().saturating_sub(1);
    let col_num  = lines.last().map(|l| l.chars().count()).unwrap_or(0);
    let char_w   = font_size * 0.601;
    let line_h   = font_size * 1.45;
    let x = (editor_rect.min.x + col_num as f32 * char_w).max(editor_rect.min.x);
    let y = editor_rect.min.y + (line_num + 1) as f32 * line_h + 4.0;
    Pos2::new(x, y)
}

// ── Syntax highlighting ───────────────────────────────────────────────────────

pub fn cobol_layout_job(
    text:    &str,
    font_id: FontId,
    kw_set:  &std::collections::HashSet<&'static str>,
) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};

    // Syntax colours come from the active IDE theme (published once per frame).
    let th        = crate::theme::active();
    let c_plain   = th.ed_plain;
    let c_kw      = th.ed_keyword;
    let c_data    = th.ed_data;
    let c_para    = th.ed_paragraph;
    let c_str     = th.ed_string;
    let c_comment = th.ed_comment;

    let fmt = |c: Color32| TextFormat { font_id: font_id.clone(), color: c, ..Default::default() };

    let mut job = LayoutJob::default();
    for (li, line) in text.split('\n').enumerate() {
        if li > 0 { job.append("\n", 0.0, fmt(c_plain)); }
        cobol_highlight_line(&mut job, line, kw_set, &fmt,
            c_plain, c_kw, c_data, c_para, c_str, c_comment);
    }
    job
}

/// Lay out `text` in a single flat colour (used for read-only generated code).
pub fn mono_layout_job(text: &str, font_id: FontId, color: Color32) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};
    let mut job = LayoutJob::default();
    job.append(text, 0.0, TextFormat { font_id, color, ..Default::default() });
    job
}

pub fn highlight_cobol(text: &str) -> egui::text::LayoutJob {
    let kw: std::collections::HashSet<&'static str> = VERBS.iter()
        .chain(DIVISION_KEYWORDS.iter())
        .chain(DATA_KEYWORDS.iter())
        .copied().collect();
    cobol_layout_job(text, FontId::monospace(EDITOR_FONT_SIZE), &kw)
}

#[allow(clippy::too_many_arguments)]
fn cobol_highlight_line(
    job:       &mut egui::text::LayoutJob,
    line:      &str,
    kw_set:    &std::collections::HashSet<&'static str>,
    fmt:       &impl Fn(Color32) -> egui::text::TextFormat,
    c_plain:   Color32,
    c_kw:      Color32,
    c_data:    Color32,
    c_para:    Color32,
    c_str:     Color32,
    c_comment: Color32,
) {
    if line.chars().nth(6).map(|c| c == '*' || c == '/').unwrap_or(false) {
        job.append(line, 0.0, fmt(c_comment));
        return;
    }
    let trimmed = line.trim_start();
    if trimmed.starts_with("*>") {
        job.append(line, 0.0, fmt(c_comment));
        return;
    }

    let mut next_is_data = false;
    {
        let fe = trimmed.find(|c: char| c.is_whitespace()).unwrap_or(trimmed.len());
        let fw = &trimmed[..fe];
        if !fw.is_empty() && fw.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(lvl) = fw.parse::<u32>() {
                if (1..=49).contains(&lvl) || matches!(lvl, 66 | 77 | 78 | 88) {
                    next_is_data = true;
                }
            }
        }
    }

    let mut first_is_para = false;
    if !next_is_data {
        let fe  = trimmed.find(|c: char| c.is_whitespace()).unwrap_or(trimmed.len());
        let fw  = &trimmed[..fe];
        if !fw.is_empty() && !fw.starts_with(|c: char| c.is_ascii_digit()) {
            let rest     = trimmed[fe..].trim_start().trim_end_matches('.');
            let fw_upper = fw.trim_end_matches('.').to_ascii_uppercase();
            if !kw_set.contains(fw_upper.as_str())
                && rest.is_empty()
                && fw.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
            {
                first_is_para = true;
            }
        }
    }

    let bytes = line.as_bytes();
    let n     = line.len();
    let mut i   = 0usize;
    let mut seg = 0usize;
    let mut in_str: Option<u8> = None;
    let mut tok_num = 0usize;

    while i < n {
        if let Some(q) = in_str {
            if bytes[i] == q {
                if bytes.get(i + 1) == Some(&q) {
                    i += 2;
                } else {
                    i += 1;
                    job.append(&line[seg..i], 0.0, fmt(c_str));
                    seg    = i;
                    in_str = None;
                }
            } else {
                i += line[i..].chars().next().map_or(1, |c| c.len_utf8());
            }
            continue;
        }

        if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'>') {
            if i > seg { emit_word(job, &line[seg..i], tok_num, next_is_data, first_is_para, kw_set, fmt, c_plain, c_kw, c_data, c_para); }
            job.append(&line[i..], 0.0, fmt(c_comment));
            return;
        }

        if bytes[i] == b'"' || bytes[i] == b'\'' {
            if i > seg { emit_word(job, &line[seg..i], tok_num, next_is_data, first_is_para, kw_set, fmt, c_plain, c_kw, c_data, c_para); }
            seg    = i;
            in_str = Some(bytes[i]);
            i += 1;
            continue;
        }

        let ch = line[i..].chars().next().unwrap();
        if ch.is_alphanumeric() || ch == '-' || ch == '_' {
            i += ch.len_utf8();
        } else {
            if i > seg {
                let word = &line[seg..i];
                if word.chars().any(|c| c.is_alphanumeric()) {
                    emit_word(job, word, tok_num, next_is_data, first_is_para,
                              kw_set, fmt, c_plain, c_kw, c_data, c_para);
                    tok_num += 1;
                } else {
                    job.append(word, 0.0, fmt(c_plain));
                }
            }
            let end = i + ch.len_utf8();
            job.append(&line[i..end], 0.0, fmt(c_plain));
            seg = end;
            i   = end;
        }
    }

    if seg < n {
        if in_str.is_some() {
            job.append(&line[seg..], 0.0, fmt(c_str));
        } else {
            let word = &line[seg..];
            if word.chars().any(|c| c.is_alphanumeric()) {
                emit_word(job, word, tok_num, next_is_data, first_is_para,
                          kw_set, fmt, c_plain, c_kw, c_data, c_para);
            } else {
                job.append(word, 0.0, fmt(c_plain));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn emit_word(
    job:           &mut egui::text::LayoutJob,
    word:          &str,
    tok_num:       usize,
    next_is_data:  bool,
    first_is_para: bool,
    kw_set:        &std::collections::HashSet<&'static str>,
    fmt:           &impl Fn(Color32) -> egui::text::TextFormat,
    c_plain:       Color32,
    c_kw:          Color32,
    c_data:        Color32,
    c_para:        Color32,
) {
    let up = word.trim_end_matches('.').to_ascii_uppercase();
    let color = if kw_set.contains(up.as_str()) {
        c_kw
    } else if tok_num == 0 && first_is_para {
        c_para
    } else if tok_num == 1 && next_is_data && up != "FILLER" {
        c_data
    } else {
        c_plain
    };
    job.append(word, 0.0, fmt(color));
}

#[cfg(test)]
mod goto_tests {
    use super::*;
    use std::path::PathBuf;

    fn editor_with(content: &str) -> EditorPanel {
        let mut ed = EditorPanel::new();
        ed.tabs.push(EditorTab::new(PathBuf::from("main.cbl"), content.to_owned()));
        ed.active = 0;
        ed
    }

    const SRC: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN.
       PROCEDURE DIVISION.
           CALL \"BTN-OK--CLICK\"
           GOBACK.
       BTN-OK--CLICK.
           DISPLAY \"hi\".
";

    #[test]
    fn jumps_to_paragraph_definition_not_call_site() {
        let mut ed = editor_with(SRC);
        assert!(ed.goto_paragraph("BTN-OK--CLICK"));
        assert!(ed.search.needs_scroll);
        let off = ed.search.matches[0];
        // The match must land on the paragraph header, not the earlier CALL line.
        assert!(SRC[off..].to_ascii_uppercase().starts_with("BTN-OK--CLICK."),
            "expected to land on the header, got: {:?}", &SRC[off..off + 20]);
    }

    #[test]
    fn jumps_to_program_id() {
        let mut ed = editor_with(SRC);
        assert!(ed.goto_paragraph("main"));
        let off = ed.search.matches[0];
        // Lands on the PROGRAM-ID line (its definition), case-insensitively.
        assert!(SRC[off..].to_ascii_uppercase().starts_with("PROGRAM-ID. MAIN."));
    }

    #[test]
    fn missing_paragraph_returns_false() {
        let mut ed = editor_with(SRC);
        assert!(!ed.goto_paragraph("DOES-NOT-EXIST"));
    }

    #[test]
    fn line_col_from_char_index() {
        let t = "AB\nCDE\nF";
        assert_eq!(char_index_to_line_col(t, 0), (1, 1));
        assert_eq!(char_index_to_line_col(t, 2), (1, 3));  // before the \n
        assert_eq!(char_index_to_line_col(t, 3), (2, 1));  // start of line 2
        assert_eq!(char_index_to_line_col(t, 7), (3, 1));  // 'F'
    }

    #[test]
    fn trim_trailing_ws_preserves_lines() {
        let s = "AB  \n  CD\t\nEF\n";
        assert_eq!(trim_trailing_ws(s), "AB\n  CD\nEF\n");
        assert_eq!(trim_trailing_ws("no newline   "), "no newline");
    }

    #[test]
    fn beautify_indents_to_cobol_columns() {
        let input = "\
ENVIRONMENT DIVISION.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-X PIC 9(4).
PROCEDURE DIVISION.
MOVE 1 TO WS-X
IF WS-X > 0
DISPLAY \"POS\"
END-IF
*> trailing note
";
        let out = beautify_cobol(input);
        // Area A (col 8 = 7 spaces): divisions, sections, 01 items.
        assert!(out.starts_with("       ENVIRONMENT DIVISION.\n"), "got: {out:?}");
        assert!(out.contains("\n       WORKING-STORAGE SECTION.\n"));
        assert!(out.contains("\n       01 WS-X PIC 9(4).\n"));
        assert!(out.contains("\n       PROCEDURE DIVISION.\n"));
        // Area B (col 12 = 11 spaces): statements.
        assert!(out.contains("\n           MOVE 1 TO WS-X\n"));
        assert!(out.contains("\n           IF WS-X > 0\n"));
        // Nested under IF → col 16 (15 spaces); END-IF back at col 12.
        assert!(out.contains("\n               DISPLAY \"POS\"\n"));
        assert!(out.contains("\n           END-IF\n"));
        // Comment indicator in column 7 (6 spaces).
        assert!(out.contains("\n      *> trailing note\n"));
    }

    #[test]
    fn beautify_collapses_spaces_but_keeps_pic_gap() {
        // Double spaces collapse, except the alignment gap before PIC.
        assert_eq!(
            collapse_spaces_keep_pic("01  WS-NAME      PIC X(20)."),
            "01 WS-NAME      PIC X(20)."
        );
        assert_eq!(collapse_spaces_keep_pic("MOVE    1   TO   WS-X"), "MOVE 1 TO WS-X");
        // Spaces inside a string literal are untouched.
        assert_eq!(collapse_spaces_keep_pic("DISPLAY \"a    b\""), "DISPLAY \"a    b\"");
    }

    #[test]
    fn beautify_evaluate_when_nesting() {
        let input = "\
PROCEDURE DIVISION.
EVALUATE WS-X
WHEN 1
MOVE A TO B
WHEN OTHER
MOVE C TO D
END-EVALUATE
";
        let out = beautify_cobol(input);
        assert!(out.contains("\n           EVALUATE WS-X\n"));      // col 12
        assert!(out.contains("\n               WHEN 1\n"));         // col 16
        assert!(out.contains("\n                   MOVE A TO B\n")); // col 20
        assert!(out.contains("\n               WHEN OTHER\n"));     // col 16
        assert!(out.contains("\n           END-EVALUATE\n"));       // col 12
    }

    #[test]
    fn replace_all_ci_is_case_insensitive() {
        assert_eq!(replace_all_ci("Move move MOVE", "move", "ADD"), "ADD ADD ADD");
        assert_eq!(replace_all_ci("abc", "x", "y"), "abc");
        // Non-matching UTF-8 passes through untouched.
        assert_eq!(replace_all_ci("café move", "MOVE", "ADD"), "café ADD");
    }
}
