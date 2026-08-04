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
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use egui::{CentralPanel, Color32, Context, FontId, Key, Panel, Pos2, ScrollArea, TextEdit};

use crate::runner::DiagMsg;

// ── Constants ─────────────────────────────────────────────────────────────────

pub const EDITOR_FONT_SIZE: f32 = 16.0;

// ── COBOL keyword tables ──────────────────────────────────────────────────────

const VERBS: &[&str] = &[
    "MOVE",
    "ADD",
    "SUBTRACT",
    "MULTIPLY",
    "DIVIDE",
    "COMPUTE",
    "IF",
    "ELSE",
    "END-IF",
    "EVALUATE",
    "WHEN",
    "OTHER",
    "END-EVALUATE",
    "PERFORM",
    "UNTIL",
    "VARYING",
    "FROM",
    "BY",
    "AFTER",
    "THRU",
    "THROUGH",
    "TIMES",
    "END-PERFORM",
    "GO",
    "TO",
    "DEPENDING",
    "ON",
    "CONTINUE",
    "NEXT",
    "SENTENCE",
    "ACCEPT",
    "DISPLAY",
    "UPON",
    "NO",
    "ADVANCING",
    "CALL",
    "USING",
    "RETURNING",
    "EXCEPTION",
    "END-CALL",
    "OPEN",
    "CLOSE",
    "READ",
    "WRITE",
    "REWRITE",
    "DELETE",
    "START",
    "INTO",
    "AT",
    "END",
    "NOT",
    "INVALID",
    "KEY",
    "STRING",
    "UNSTRING",
    "DELIMITED",
    "ALL",
    "POINTER",
    "INSPECT",
    "TALLYING",
    "REPLACING",
    "CONVERTING",
    "STOP",
    "RUN",
    "GOBACK",
    "EXIT",
    "PROGRAM",
    "SORT",
    "MERGE",
    "OUTPUT",
    "INPUT",
    "EXEC",
    "END-EXEC",
    "INVOKE",
    "SET",
    "GIVING",
    "ROUNDED",
    "REMAINDER",
    "SIZE",
    "ERROR",
    "REFERENCE",
    "CONTENT",
    "VALUE",
    // CoBolt animation extensions
    "PLAY",
    "STOP-ANIMATION",
    // CoBolt exception handling extensions
    "TRY",
    "CATCH",
    "EXCEPTION",
    "FINALLY",
    "END-TRY",
    "THROW",
    "RAISE",
];

const DIVISION_KEYWORDS: &[&str] = &[
    "IDENTIFICATION",
    "ENVIRONMENT",
    "DATA",
    "PROCEDURE",
    "DIVISION",
    "SECTION",
    "PROGRAM-ID",
    "AUTHOR",
    "DATE-WRITTEN",
    "WORKING-STORAGE",
    "LOCAL-STORAGE",
    "LINKAGE",
    "FILE-CONTROL",
    "SELECT",
    "ASSIGN",
    "ORGANIZATION",
    "SEQUENTIAL",
    "INDEXED",
    "RELATIVE",
    "ACCESS",
    "MODE",
    "RECORD",
    "ALTERNATE",
    "WITH",
    "DUPLICATES",
    "FILE",
    "STATUS",
    "FD",
    "SD",
];

const DATA_KEYWORDS: &[&str] = &[
    "PIC",
    "PICTURE",
    "COMP",
    "COMP-1",
    "COMP-2",
    "COMP-3",
    "COMP-5",
    "BINARY",
    "PACKED-DECIMAL",
    "DISPLAY",
    "OCCURS",
    "TIMES",
    "INDEXED",
    "REDEFINES",
    "VALUES",
    "IS",
    "ARE",
    "FILLER",
    "GLOBAL",
    "EXTERNAL",
    "SPACE",
    "SPACES",
    "ZERO",
    "ZEROS",
    "ZEROES",
    "HIGH-VALUE",
    "HIGH-VALUES",
    "LOW-VALUE",
    "LOW-VALUES",
    "QUOTE",
    "QUOTES",
    "NULL",
    "NULLS",
];

/// COBOL-2002 reserved words (object orientation, the new data types, dynamic
/// storage, conditional-expression and program attributes). PowerRustCOBOL
/// supports the COBOL-2002 subset used by the Rust-FFI bridge and form modules;
/// these are offered for completion and treated as reserved by the beautifier.
const COBOL2002_KEYWORDS: &[&str] = &[
    // Object orientation
    "CLASS",
    "CLASS-ID",
    "METHOD",
    "METHOD-ID",
    "FACTORY",
    "OBJECT",
    "INHERITS",
    "IMPLEMENTS",
    "INTERFACE",
    "INTERFACE-ID",
    "OVERRIDE",
    "SELF",
    "SUPER",
    "UNIVERSAL",
    "ACTIVE-CLASS",
    "PROPERTY",
    "REPOSITORY",
    "FUNCTION",
    "FUNCTION-ID",
    "END-INVOKE",
    // Types and data description
    "TYPEDEF",
    "STRONG",
    "BASED",
    "CONSTANT",
    "BIT",
    "BOOLEAN",
    "BINARY-CHAR",
    "BINARY-SHORT",
    "BINARY-LONG",
    "BINARY-DOUBLE",
    "FLOAT-SHORT",
    "FLOAT-LONG",
    "FLOAT-EXTENDED",
    "NATIONAL",
    "NATIONAL-EDITED",
    "GROUP-USAGE",
    "ALIGNED",
    "ANY",
    "ANYCASE",
    // Dynamic storage
    "ALLOCATE",
    "FREE",
    "INITIALIZED",
    // Conditional expressions / program attributes
    "RAISING",
    "EC",
    "PRESENT",
    "OMITTED",
    "VALIDATE",
    "VALIDATING",
    "DEFAULT",
    "FORMAT",
    "RECURSIVE",
    "COMMON",
    "INITIAL",
    // Scope terminators new in COBOL-2002 (or commonly omitted earlier)
    "END-ACCEPT",
    "END-DISPLAY",
    "END-ADD",
    "END-SUBTRACT",
    "END-MULTIPLY",
    "END-DIVIDE",
    "END-COMPUTE",
    "END-STRING",
    "END-UNSTRING",
    "END-SEARCH",
    "END-READ",
    "END-WRITE",
    "END-REWRITE",
    "END-DELETE",
    "END-START",
];

// ── Control member tables ─────────────────────────────────────────────────────

/// Methods exposed by each control type (shown after `CTRL::` or after
/// `INVOKE ctrl-id ` in the classic form). Completion inserts the bare
/// method name (the inline `::Method(arg)` and INVOKE forms both accept
/// bare identifiers for the method).
type Method = (&'static str, &'static str);

/// Methods every *visual* widget supports (lifecycle, geometry, animation,
/// validation, generic property access).
const UNIVERSAL_VISUAL: &[Method] = &[
    ("Show", "Make the control visible"),
    ("Hide", "Make the control invisible"),
    ("Enable", "Enable interaction"),
    ("Disable", "Disable interaction"),
    ("SetFocus", "Move keyboard focus to this control"),
    ("MoveTo", "Move to (X, Y) in pixels"),
    ("Resize", "Resize to (Width, Height) in pixels"),
    ("BringToFront", "Raise above sibling controls"),
    ("SendToBack", "Lower beneath sibling controls"),
    ("Refresh", "Force a redraw"),
    ("Validate", "Run the control's validation rule"),
    ("PlayAnimation", "Run a named animation"),
    ("StopAnimation", "Stop a running animation"),
    ("SetProperty", "Set any property by name"),
    ("GetProperty", "Get any property by name"),
];

/// Methods for non-visual widgets (Timer, AI agent, REST/SQL clients).
const UNIVERSAL_NONVISUAL: &[Method] = &[
    ("SetProperty", "Set any property by name"),
    ("GetProperty", "Get any property by name"),
];

/// Per-type methods plus the relevant universal set, used by `ctrl::`/`INVOKE`
/// completion. Returns an owned vec so universal + specific can be merged.
fn methods_for_type(ctrl_type: &str) -> Vec<Method> {
    let (base, specific): (&[Method], &[Method]) = match ctrl_type {
        "Button" => (
            UNIVERSAL_VISUAL,
            &[
                ("Click", "Raise the onClick event"),
                ("SetCaption", "Change the button text"),
                ("PerformClick", "Programmatically click"),
            ],
        ),
        "TextBox" => (
            UNIVERSAL_VISUAL,
            &[
                ("GetText", "Return the current text"),
                ("SetText", "Replace the text"),
                ("AppendText", "Append to the text"),
                ("Clear", "Clear the text"),
                ("SelectAll", "Select all text"),
            ],
        ),
        "Label" => (
            UNIVERSAL_VISUAL,
            &[
                ("SetCaption", "Change the label text"),
                ("SetColor", "Change the foreground colour"),
            ],
        ),
        "CheckBox" => (
            UNIVERSAL_VISUAL,
            &[
                ("IsChecked", "Returns 1 if checked"),
                ("SetChecked", "Set the checked state (0/1)"),
                ("Toggle", "Flip the checked state"),
            ],
        ),
        "RadioButton" => (
            UNIVERSAL_VISUAL,
            &[
                ("IsChecked", "Returns 1 if selected"),
                ("SetChecked", "Set the selected state (0/1)"),
                ("Select", "Select this option"),
            ],
        ),
        "ComboBox" => (
            UNIVERSAL_VISUAL,
            &[
                ("GetText", "Get the selected text"),
                ("SetText", "Set / select text"),
                ("AddItem", "Append an item"),
                ("RemoveItem", "Remove an item by index"),
                ("Clear", "Remove all items"),
                ("GetIndex", "Get the selected index"),
                ("SetIndex", "Select an item by index"),
                ("GetCount", "Return the item count"),
            ],
        ),
        "ListBox" => (
            UNIVERSAL_VISUAL,
            &[
                ("AddItem", "Append an item"),
                ("RemoveItem", "Remove an item by index"),
                ("Clear", "Remove all items"),
                ("GetSelected", "Return the selected text"),
                ("GetSelectedIndex", "Return the selected index"),
                ("SetSelectedIndex", "Select an item by index"),
                ("GetCount", "Return the item count"),
            ],
        ),
        "PictureBox" => (
            UNIVERSAL_VISUAL,
            &[
                ("SetImage", "Load an image from a file path"),
                ("Clear", "Clear the displayed image"),
            ],
        ),
        "Animator" => (
            UNIVERSAL_VISUAL,
            &[
                ("Play", "Play the animation"),
                ("Pause", "Pause the animation"),
                ("Stop", "Stop the animation"),
                ("SetSource", "Load a new animated image"),
            ],
        ),
        "DataGrid" => (
            UNIVERSAL_VISUAL,
            &[
                ("ExportCSV", "Export rows as CSV"),
                ("GetRowCount", "Return the row count"),
                ("GetCellValue", "Read a cell (row, col)"),
                ("SetCellValue", "Write a cell (row, col, value)"),
                ("AddRow", "Append an empty row"),
                ("DeleteRow", "Delete a row by index"),
                ("ClearRows", "Remove all rows"),
                ("RefreshBinding", "Reload rows from the bound data source"),
                ("Sort", "Sort by a column"),
                ("SetFilter", "Filter a column by text"),
                ("ClearFilters", "Remove all active filters"),
                ("FreezeColumns", "Freeze left columns"),
                ("FreezeRows", "Freeze top rows"),
                ("SetRowHeight", "Set the default row height"),
                ("SetColumnWidth", "Set a column width"),
                ("GetSelectedText", "Return selected cell text"),
                ("CopySelection", "Copy selected cell text"),
            ],
        ),
        "TreeView" => (
            UNIVERSAL_VISUAL,
            &[
                ("AddNode", "Add a node (parent, text)"),
                ("RemoveNode", "Remove a node by path"),
                ("Clear", "Remove all nodes"),
                ("ExpandAll", "Expand every node"),
                ("CollapseAll", "Collapse every node"),
                ("GetSelectedNode", "Return the selected node"),
                ("SetSelectedNode", "Select a node by path"),
            ],
        ),
        "TabControl" => (
            UNIVERSAL_VISUAL,
            &[
                ("SelectTab", "Activate a tab by index/name"),
                ("GetSelectedTab", "Return the active tab"),
                ("AddTab", "Add a tab"),
                ("RemoveTab", "Remove a tab by index"),
            ],
        ),
        "ProgressBar" => (
            UNIVERSAL_VISUAL,
            &[
                ("SetValue", "Set the current value"),
                ("GetValue", "Get the current value"),
                ("Increment", "Increase the value by a step"),
                ("Reset", "Reset to the minimum"),
            ],
        ),
        "Slider" => (
            UNIVERSAL_VISUAL,
            &[
                ("SetValue", "Set the thumb position"),
                ("GetValue", "Get the current value"),
            ],
        ),
        "NumericUpDown" => (
            UNIVERSAL_VISUAL,
            &[
                ("GetValue", "Get the current value"),
                ("SetValue", "Set the value"),
                ("Increment", "Add one step"),
                ("Decrement", "Subtract one step"),
            ],
        ),
        "DateTimePicker" => (
            UNIVERSAL_VISUAL,
            &[
                ("GetValue", "Return the selected date/time"),
                ("SetValue", "Set the date/time"),
            ],
        ),
        "MenuBar" | "ToolBar" | "StatusBar" => (
            UNIVERSAL_VISUAL,
            &[
                ("SetItems", "Replace the item list"),
                ("GetItem", "Read an item by index"),
            ],
        ),
        "BarChart" | "LineChart" | "PieChart" | "AreaChart" | "ScatterChart" | "DonutChart" => (
            UNIVERSAL_VISUAL,
            &[
                ("SetData", "Bind a COBOL table/array as data"),
                ("AddSeries", "Add a data series"),
                ("Clear", "Remove all data"),
                ("ExportImage", "Save the chart as an image"),
            ],
        ),
        "Timer" => (
            UNIVERSAL_NONVISUAL,
            &[
                ("Start", "Start / resume the timer"),
                ("Stop", "Pause the timer"),
                ("Reset", "Reset the elapsed time"),
                ("SetInterval", "Change the interval (ms)"),
                ("IsEnabled", "Returns 1 if running"),
            ],
        ),
        "AgentObject" => (
            UNIVERSAL_NONVISUAL,
            &[
                ("Ask", "Send a prompt to the LLM, get a reply"),
                ("SetPrompt", "Set the system prompt"),
                ("SetModel", "Switch the model name"),
                ("Stop", "Abort the current request"),
            ],
        ),
        "RestClient" => (
            UNIVERSAL_NONVISUAL,
            &[
                ("call", "Generic HTTP call"),
                ("get", "HTTP GET request"),
                ("post", "HTTP POST request"),
                ("put", "HTTP PUT request"),
                ("delete", "HTTP DELETE request"),
                ("setHeader", "Add / replace a request header"),
                ("clearHeaders", "Remove all custom headers"),
                ("setTimeout", "Set the timeout (ms)"),
            ],
        ),
        "SqlDatabase" => (
            UNIVERSAL_NONVISUAL,
            &[
                ("open", "Open the database connection"),
                ("close", "Close the connection"),
                ("query", "Run a SELECT, return a cursor"),
                ("execute", "Run an INSERT/UPDATE/DELETE"),
                ("fetch", "Fetch the next row"),
                ("fetchAll", "Fetch all rows"),
            ],
        ),
        // Containers / decorative widgets: universal-visual only.
        "GroupBox" => (UNIVERSAL_VISUAL, &[("SetCaption", "Change the title")]),
        _ => (UNIVERSAL_VISUAL, &[]),
    };
    base.iter().chain(specific).copied().collect()
}

/// The method names available on a control type (universal + type-specific), for
/// validating `::Method(...)` references. Per-instance methods (e.g. RefreshBinding)
/// are added by the caller from the control's `extra_methods`.
pub(crate) fn method_names_for_type(ctrl_type: &str) -> Vec<String> {
    methods_for_type(ctrl_type)
        .into_iter()
        .map(|(n, _)| n.to_string())
        .collect()
}

/// Curated property descriptions (kept for tooltips/docs). The live IntelliSense
/// now derives the full property set from `cobolt_forms::model::property_names_for`
/// so it can never drift from the control model.
#[allow(dead_code)]
fn properties_for_type(ctrl_type: &str) -> &'static [(&'static str, &'static str)] {
    match ctrl_type {
        "Button" => &[
            ("Caption", "Button label text"),
            ("Visible", "1 = visible, 0 = hidden"),
            ("Enabled", "1 = enabled, 0 = disabled"),
            ("Width", "Width in pixels"),
            ("Height", "Height in pixels"),
            ("BackgroundColor", "Background colour (RRGGBB)"),
            ("ForegroundColor", "Text colour (RRGGBB)"),
            ("FontSize", "Font size in points"),
            ("Bold", "1 = bold text"),
            ("CornerRadius", "Border corner radius"),
            ("Transparency", "Transparency 0–100 (0 = opaque)"),
        ],
        "Label" => &[
            ("Caption", "Label text"),
            ("Visible", "1 = visible, 0 = hidden"),
            ("ForegroundColor", "Text colour (RRGGBB)"),
            ("FontSize", "Font size in points"),
            ("Bold", "1 = bold"),
            ("Italic", "1 = italic"),
            ("Underline", "1 = underline"),
            ("Strikethrough", "1 = strikethrough"),
            ("Transparency", "Transparency 0–100 (0 = opaque)"),
        ],
        "TextBox" => &[
            ("Text", "Current text value"),
            ("Visible", "1 = visible, 0 = hidden"),
            ("Enabled", "1 = enabled, 0 = disabled"),
            ("MaximumLength", "Maximum character count"),
            ("Multiline", "1 = multiline input"),
            ("PasswordCharacter", "Masking character (e.g. *)"),
            ("BackgroundColor", "Background colour (RRGGBB)"),
            ("ForegroundColor", "Text colour (RRGGBB)"),
            ("FontSize", "Font size in points"),
            ("ReadOnly", "1 = read-only"),
        ],
        "CheckBox" => &[
            ("Caption", "Checkbox label text"),
            ("Checked", "1 = checked, 0 = unchecked"),
            ("Visible", "1 = visible, 0 = hidden"),
            ("Enabled", "1 = enabled, 0 = disabled"),
            ("ForegroundColor", "Label colour (RRGGBB)"),
        ],
        "RadioButton" => &[
            ("Caption", "Radio button label"),
            ("Checked", "1 = selected, 0 = not selected"),
            ("Visible", "1 = visible, 0 = hidden"),
            ("Enabled", "1 = enabled, 0 = disabled"),
            ("ForegroundColor", "Label colour (RRGGBB)"),
        ],
        "ComboBox" => &[
            ("Text", "Selected / displayed text"),
            ("Items", "Newline-separated item list"),
            ("Visible", "1 = visible, 0 = hidden"),
            ("Enabled", "1 = enabled, 0 = disabled"),
            ("BackgroundColor", "Background colour (RRGGBB)"),
            ("ForegroundColor", "Text colour (RRGGBB)"),
        ],
        "ListBox" => &[
            ("Items", "Newline-separated item list"),
            ("Visible", "1 = visible, 0 = hidden"),
            ("Enabled", "1 = enabled, 0 = disabled"),
            ("BackgroundColor", "Background colour (RRGGBB)"),
            ("ForegroundColor", "Text colour (RRGGBB)"),
        ],
        "PictureBox" => &[
            ("ImagePath", "Absolute path to image file"),
            ("SizeMode", "Normal / StretchImage / Zoom / AutoSize"),
            ("Visible", "1 = visible, 0 = hidden"),
            ("Transparency", "Transparency 0–100 (0 = opaque)"),
            ("Width", "Width in pixels"),
            ("Height", "Height in pixels"),
        ],
        "GroupBox" => &[
            ("Caption", "Group box title"),
            ("Visible", "1 = visible, 0 = hidden"),
            ("BackgroundColor", "Background colour"),
            ("ForegroundColor", "Title text colour"),
        ],
        "Panel" => &[
            ("Visible", "1 = visible, 0 = hidden"),
            ("BackgroundColor", "Background colour (RRGGBB)"),
            ("Transparency", "Transparency 0–100 (0 = opaque)"),
        ],
        "ProgressBar" => &[
            ("Value", "Current value"),
            ("Minimum", "Minimum value"),
            ("Maximum", "Maximum value"),
            ("Visible", "1 = visible, 0 = hidden"),
            ("BarColor", "Fill colour (RRGGBB)"),
            ("ShowValue", "1 = display percentage text"),
        ],
        "Slider" => &[
            ("Value", "Current thumb position"),
            ("Minimum", "Minimum value"),
            ("Maximum", "Maximum value"),
            ("Step", "Step increment"),
            ("Visible", "1 = visible, 0 = hidden"),
        ],
        "DataGrid" => &[
            ("Columns", "Newline-separated column names"),
            ("Visible", "1 = visible, 0 = hidden"),
            ("ExportCSV", "1 = enable CSV export button"),
            ("BackgroundColor", "Background colour"),
        ],
        "Timer" => &[
            ("Interval", "Tick interval in milliseconds"),
            ("Enabled", "1 = running, 0 = stopped"),
        ],
        "AgentObject" => &[
            ("AgentModel", "LLM model name  (e.g. llama3.2)"),
            ("AgentURL", "Endpoint base URL"),
            ("SystemPrompt", "Optional system prompt"),
        ],
        "RestClient" => &[
            ("BaseURL", "Base URL for all requests"),
            ("DefaultMethod", "Default HTTP method (GET/POST…)"),
            ("Timeout", "Request timeout in ms"),
        ],
        _ => &[
            ("Caption", "Display text"),
            ("Visible", "1 = visible, 0 = hidden"),
            ("Enabled", "1 = enabled, 0 = disabled"),
            ("Width", "Width in pixels"),
            ("Height", "Height in pixels"),
            ("BackgroundColor", "Background colour (RRGGBB)"),
            ("ForegroundColor", "Foreground colour (RRGGBB)"),
            ("Transparency", "Transparency 0–100 (0 = opaque)"),
        ],
    }
}

// ── Auto-completion types ─────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum AcKind {
    Keyword,
    Snippet,
    Paragraph,
    DataItem,
    Property,
    Method,
    Control,
}

#[derive(Clone)]
struct AcItem {
    label: String,
    insert: String,
    detail: String,
    kind: AcKind,
}

impl AcItem {
    fn kw(word: &str) -> Self {
        // Insert the bare word + a trailing space and let the user keep typing —
        // never an auto-closed template (e.g. `DISPLAY ""`).
        Self {
            label: word.into(),
            insert: format!("{word} "),
            detail: "keyword".into(),
            kind: AcKind::Keyword,
        }
    }
    fn property(name: &str) -> Self {
        // Property name inside a string ref: accepting it closes the quote.
        Self {
            label: name.into(),
            insert: format!("{name}\""),
            detail: "property".into(),
            kind: AcKind::Property,
        }
    }
    fn para(name: &str) -> Self {
        Self {
            label: name.into(),
            insert: name.into(),
            detail: "paragraph".into(),
            kind: AcKind::Paragraph,
        }
    }
    fn data(name: &str) -> Self {
        Self {
            label: name.into(),
            insert: name.into(),
            detail: "data item".into(),
            kind: AcKind::DataItem,
        }
    }
    fn prop(name: &str, detail: &str) -> Self {
        Self {
            label: name.into(),
            insert: name.into(),
            detail: detail.into(),
            kind: AcKind::Property,
        }
    }
    fn method(name: &str, detail: &str) -> Self {
        // Bare identifier form. Suitable for the modern inline syntax
        // `CTRL::SetCaption(arg)` (parse_method_tail eats identifier after ::).
        // The classic `INVOKE ctrl "Method"` path also accepts a bare identifier
        // (or string) for the method name, so this produces parseable code for
        // the live interpreter re-parse as well.
        Self {
            label: name.into(),
            insert: name.into(),
            detail: detail.into(),
            kind: AcKind::Method,
        }
    }
    fn ctrl(id: &str, ctrl_type: &str) -> Self {
        Self {
            label: id.into(),
            insert: id.into(),
            detail: format!("{ctrl_type} control"),
            kind: AcKind::Control,
        }
    }

    fn badge(&self) -> (&str, Color32) {
        match self.kind {
            AcKind::Keyword => ("K", Color32::from_rgb(86, 156, 214)),
            AcKind::Snippet => ("S", Color32::from_rgb(220, 180, 60)),
            AcKind::Paragraph => ("¶", Color32::from_rgb(197, 134, 192)),
            AcKind::DataItem => ("D", Color32::from_rgb(78, 201, 176)),
            AcKind::Property => ("●", Color32::from_rgb(120, 220, 110)), // green (spec 010)
            AcKind::Method => ("M", Color32::from_rgb(100, 190, 245)),   // light blue (spec 010)
            AcKind::Control => ("C", Color32::from_rgb(140, 200, 255)),
        }
    }
}

// ── AutoComplete state ────────────────────────────────────────────────────────

/// How far the editor may shift before an open completion popup is considered
/// unanchored. Sub-pixel jitter from layout rounding must not close it, but any
/// real move or resize must.
const ANCHOR_EPSILON: f32 = 0.5;

/// Whether the editor has moved out from under an open popup. `popup_pos` is a
/// screen position fixed when the list opened, so any real shift of the text —
/// window drag, splitter, panel resize, scroll — leaves the list pointing at
/// the wrong place.
fn ac_anchor_moved(current: Pos2, anchor: Pos2) -> bool {
    (current - anchor).length() > ANCHOR_EPSILON
}

/// Whether the word being completed has ended. With no prefix left and no
/// member/property context active there is nothing to complete, so a list still
/// on screen describes a word the developer already finished typing.
fn ac_context_ended(prefix: &str, has_member_context: bool) -> bool {
    prefix.is_empty() && !has_member_context
}

#[derive(Default)]
struct AutoComplete {
    visible: bool,
    items: Vec<AcItem>,
    selected: usize,
    prefix: String,
    trigger_pos: usize,
    popup_pos: Pos2,
    /// Where the editor's text started when the popup opened. `popup_pos` is a
    /// SCREEN position pinned to the cursor at that instant, so once the editor
    /// moves or is resized under it — window drag, splitter, panel resize,
    /// scroll — the list is left floating away from the text it describes.
    /// Comparing this each frame lets the popup close instead; typing again
    /// reopens it at the cursor's new position.
    anchor: Pos2,
    /// When true the popup is showing members of a specific control (property/method list).
    member_mode: bool,
    /// Set when the selection moved via the keyboard, so the popup scrolls the
    /// highlighted row back into view on the next frame.
    scroll_to_sel: bool,
}

// ── Search / Find state ───────────────────────────────────────────────────────

struct SearchState {
    visible: bool,
    query: String,
    /// Replacement text for the find/replace bar.
    replace: String,
    /// Byte offsets of match starts in the active tab.
    matches: Vec<usize>,
    /// Index into `matches` currently highlighted.
    current: usize,
    /// Set to `true` when the next render should scroll to `current`.
    needs_scroll: bool,
    /// Set to `true` only when the scroll comes from an explicit navigation
    /// (next/prev/Enter) — then keyboard focus moves into the editor. Incremental
    /// typing in the Find box scrolls but must NOT steal focus from the box.
    focus_editor_on_scroll: bool,
    /// When `true` (default) the query matches regardless of letter case.
    case_insensitive: bool,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            visible: false,
            query: String::new(),
            replace: String::new(),
            matches: Vec::new(),
            current: 0,
            needs_scroll: false,
            focus_editor_on_scroll: false,
            case_insensitive: true,
        }
    }
}

// ── EditorTab ─────────────────────────────────────────────────────────────────

pub struct EditorTab {
    pub path: PathBuf,
    pub content: String,
    pub dirty: bool,
    /// RAD-generated code: shown in blue, never editable.
    pub read_only: bool,
}

impl EditorTab {
    pub fn new(path: PathBuf, content: String) -> Self {
        Self {
            path,
            content,
            dirty: false,
            read_only: false,
        }
    }
    pub fn title(&self) -> String {
        let name = self
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("untitled");
        if self.read_only {
            format!("🔒 {name}")
        } else if self.dirty {
            format!("● {name}")
        } else {
            name.into()
        }
    }

    fn is_markdown(&self) -> bool {
        self.path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown")
            })
    }
}

// ── Known control (for IntelliSense) ─────────────────────────────────────────

#[derive(Clone)]
pub struct KnownControl {
    pub id: String,
    pub ctrl_type: String,
    /// Property names this control exposes (for `"Prop" OF Ctrl` completion).
    pub properties: Vec<String>,
    /// Extra methods available only for this specific control instance
    /// (e.g. "RefreshBinding" when a GroupBox is databound as a ControlArray / repeating group).
    pub extra_methods: Vec<String>,
}

/// Build KnownControl list from a form, enriching with dynamic methods
/// (e.g. RefreshBinding for databound repeating GroupBoxes).
pub fn build_known_controls(form: &cobolt_forms::Form) -> Vec<KnownControl> {
    let mut list: Vec<KnownControl> = form
        .controls
        .iter()
        .map(|c| {
            let type_name = format!("{:?}", c.control_type);
            let mut extra_methods = vec![];
            if matches!(c.control_type, cobolt_forms::ControlType::GroupBox) {
                let is_array = form.data_bindings.iter().any(|b| {
                    if let cobolt_forms::BindingTargetDescriptor::ControlArray {
                        array_id, ..
                    } = &b.target
                    {
                        c.explicit_control_array_id().as_deref() == Some(array_id.as_str())
                            || c.id.eq_ignore_ascii_case(array_id)
                    } else {
                        false
                    }
                });
                if is_array {
                    extra_methods.push("RefreshBinding".to_string());
                }
            }
            let props = cobolt_forms::model::property_names_for(&type_name);
            KnownControl {
                id: c.id.clone(),
                ctrl_type: type_name,
                properties: props,
                extra_methods,
            }
        })
        .collect();

    list.push(KnownControl {
        id: "self".to_string(),
        ctrl_type: "Form".to_string(),
        properties: vec![
            "X".into(),
            "Y".into(),
            "Width".into(),
            "Height".into(),
            "Title".into(),
            "TitleBar".into(),
            "border".into(),
            "icon".into(),
        ],
        extra_methods: vec![
            "Close".into(),
            "OpenForm".into(),
            "Alert".into(),
            "Minimize".into(),
            "Restore".into(),
            "Maximize".into(),
        ],
    });

    list
}

/// Collect the form's global data-item names (from the WORKING-STORAGE source the
/// developer wrote at form level) for IntelliSense inside an event handler, whose
/// own buffer does not contain those declarations. The user's WS source is emitted
/// verbatim into the outer program's WORKING-STORAGE, so we extract elementary /
/// group item names by prefixing a synthetic section header to give the parser the
/// data-division context it keys on.
pub fn build_known_data_items(form: &cobolt_forms::Form) -> Vec<String> {
    if form.user_ws_source.trim().is_empty() {
        return Vec::new();
    }
    let with_ctx = format!("WORKING-STORAGE SECTION.\n{}", form.user_ws_source);
    extract_data_items(&with_ctx)
}

/// Collect data names useful in an AI prompt: form-level WORKING-STORAGE plus
/// the currently edited handler's local WORKING-STORAGE / LOCAL-STORAGE /
/// LINKAGE / FILE SECTION records.
pub fn build_prompt_data_items(form: &cobolt_forms::Form, local_source: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for name in build_known_data_items(form)
        .into_iter()
        .chain(extract_data_items(local_source))
    {
        if seen.insert(name.to_ascii_uppercase()) {
            out.push(name);
        }
    }
    out
}

// ── EditorPanel ───────────────────────────────────────────────────────────────

pub struct EditorPanel {
    pub tabs: Vec<EditorTab>,
    pub active: usize,
    pub diags: HashMap<PathBuf, Vec<DiagMsg>>,
    pub show_line_numbers: bool,
    pub known_controls: Vec<KnownControl>,
    /// Global/form-level data item names (from the form's WORKING-STORAGE) offered
    /// for IntelliSense even though they aren't in the current buffer — so an event
    /// handler can complete global data names without leaving the editor.
    pub known_data_items: Vec<String>,
    /// When true, completions are limited to project/form context: controls,
    /// properties, FD/data records, and data items. Used by AI prompt editors
    /// where COBOL reserved-word snippets are noise.
    context_only_completions: bool,
    /// Active breakpoint line numbers per file (1-based).
    pub breakpoints: HashMap<PathBuf, HashSet<u32>>,
    /// Line being highlighted by the debugger (current pause location).
    pub debug_line: Option<(PathBuf, u32)>,
    /// Stable namespace for egui widget IDs owned by this editor instance. The
    /// main editor can use the default; embedded modal editors must not collide.
    ui_id_salt: String,
    ac: AutoComplete,
    search: SearchState,
    font_size: f32,

    // ── AI assistant (only used when an LLM is configured) ───────────────────
    /// The current prompt text in the editor's AI bar.
    ai_prompt: String,
    /// Per-file conversation history (loaded lazily from disk).
    ai_history: HashMap<PathBuf, Vec<crate::llm::ChatTurn>>,
    /// Paths whose history has already been loaded from disk this session.
    ai_loaded: HashSet<PathBuf>,
    /// In-flight request: the channel the worker thread will answer on, plus
    /// the path it targets (so a tab switch mid-flight applies to the right file).
    ai_pending: Option<(PathBuf, std::sync::mpsc::Receiver<crate::llm::LlmResponse>)>,
    /// Last status / error line shown under the AI bar.
    ai_status: Option<String>,
    /// Whether the conversation panel is expanded.
    ai_show_history: bool,
    /// In-flight compaction (summarization) request: target path + reply channel.
    ai_compact_pending: Option<(PathBuf, std::sync::mpsc::Receiver<crate::llm::LlmResponse>)>,
    /// Target whose history the user asked to clear, awaiting confirmation.
    ai_confirm_clear: Option<PathBuf>,
    /// Streaming text chunk for the active LLM request.
    ai_streaming_reply: HashMap<PathBuf, String>,

    // ── Status bar ───────────────────────────────────────────────────────────
    /// 1-based caret line / column in the active tab (last known).
    cur_line: usize,
    cur_col: usize,
    /// Overwrite (vs. insert) typing mode — toggled with the Insert key.
    overwrite: bool,
    /// Trim trailing whitespace from every line when saving.
    pub trim_on_save: bool,
}

impl Default for EditorPanel {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            active: 0,
            diags: HashMap::new(),
            show_line_numbers: true,
            known_controls: Vec::new(),
            known_data_items: Vec::new(),
            context_only_completions: false,
            breakpoints: HashMap::new(),
            debug_line: None,
            ui_id_salt: "main".to_string(),
            ac: AutoComplete::default(),
            search: SearchState::default(),
            font_size: EDITOR_FONT_SIZE,
            ai_prompt: String::new(),
            ai_history: HashMap::new(),
            ai_loaded: HashSet::new(),
            ai_pending: None,
            ai_status: None,
            ai_show_history: false,
            ai_compact_pending: None,
            ai_confirm_clear: None,
            ai_streaming_reply: HashMap::new(),
            cur_line: 1,
            cur_col: 1,
            overwrite: false,
            trim_on_save: true,
        }
    }
}

/// Render one conversation turn as a chat balloon: the developer's messages sit
/// on the right, the assistant's COBOL responses on the left. Shared by the
/// code/structure editor `ai_bar` and the event editor transcript so the
/// conversation reads like a natural chat.
pub(crate) fn chat_bubble(ui: &mut egui::Ui, role: &str, content: &str) {
    chat_bubble_with_font_size(ui, role, content, 14.0);
}

#[derive(Clone, Default)]
struct ChatResponseActionState {
    status: Option<String>,
    error: Option<String>,
}

fn changed_documentation_roots() -> &'static Mutex<HashSet<PathBuf>> {
    static ROOTS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    ROOTS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn mark_chat_documentation_changed(project_root: &Path) {
    changed_documentation_roots()
        .lock()
        .unwrap()
        .insert(project_root.to_path_buf());
}

pub(crate) fn take_chat_documentation_changed(project_root: &Path) -> bool {
    changed_documentation_roots()
        .lock()
        .unwrap()
        .remove(project_root)
}

fn save_agent_response_as_markdown(
    project_root: &Path,
    selected_path: &Path,
    content: &str,
) -> Result<PathBuf, String> {
    let documentation_root =
        project_root.join(cobolt_agents::project_knowledge::KNOWLEDGE_BASE_ROOT);
    let mut selected_path = selected_path.to_path_buf();
    if selected_path
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("md")
    {
        selected_path.set_extension("md");
    }
    if !selected_path.starts_with(&documentation_root) {
        return Err(format!(
            "Agent responses can only be saved inside {}.",
            documentation_root.display()
        ));
    }
    let relative = selected_path
        .strip_prefix(project_root)
        .map_err(|error| error.to_string())?;
    let relative_text = relative.to_string_lossy();
    let markdown = format!("{}\n", content.trim_end());
    let saved = cobolt_agents::project_knowledge::write_document(
        project_root,
        relative_text.as_ref(),
        &markdown,
    )?;
    mark_chat_documentation_changed(project_root);
    Ok(saved)
}

fn chat_bubble_fill(is_user: bool) -> Color32 {
    if is_user {
        Color32::from_rgba_premultiplied(0x61, 0xC6, 0x54, 0xFF)
    } else {
        Color32::from_rgba_premultiplied(0x3D, 0x8B, 0xCD, 0xFF)
    }
}

pub(crate) fn chat_bubble_with_font_size(
    ui: &mut egui::Ui,
    role: &str,
    content: &str,
    font_size: f32,
) {
    render_chat_bubble(ui, role, content, font_size);
    ui.add_space(5.0);
}

/// Heuristic: does chat content carry HEAVY Markdown structure (headings,
/// tables, fenced code) that needs the theme-colored document card? Simple
/// prose and short bullet lists stay in the regular blue dialog bubble —
/// Grace's concise summaries belong there; the document cards are for the
/// detailed specialist content shown in verbose mode.
pub(crate) fn looks_like_markdown(content: &str) -> bool {
    content.contains("```")
        || content.lines().any(|line| {
            let t = line.trim_start();
            (t.starts_with('#') && t.trim_start_matches('#').starts_with(' '))
                || t.starts_with("| ")
        })
}

/// The "agents are still working" indicator, rendered as its own
/// assistant-side balloon at the TAIL of a chat history — the transcript
/// itself shows who is typing, instead of a spinner orphaned in the input
/// column.
///
/// High-contrast by requirement, borrowed from the assistant balloons (blue
/// fill, white foreground) which hardcode their contrast: deriving colors
/// from `ui.visuals()` proved unreliable here — the IDE's glass themes can
/// leave the visuals' "strong" text DARK while the chat backdrop is dark too,
/// which rendered the indicator as an empty dark pill with an invisible
/// spinner. What the balloons do to stay readable, the indicator does.
pub(crate) fn chat_thinking_indicator(
    ui: &mut egui::Ui,
    label: &str,
    font_size: f32,
    tokens: Option<(u64, u64)>,
) {
    let fill = chat_bubble_fill(false);
    let fg = Color32::WHITE;
    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
        egui::Frame::NONE
            .fill(fill)
            .corner_radius(egui::CornerRadius::same(15))
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.add(egui::Spinner::new().size(font_size + 4.0).color(fg));
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(label)
                        .size(font_size)
                        .strong()
                        .color(fg),
                );
                token_counter(ui, tokens, font_size, fg);
            });
    });
    ui.add_space(5.0);
}

/// The `↑in ↓out` counter shown while a model is working.
///
/// Drawn only when something has been counted, so a surface with no totals
/// yet — the first call of a session, before the provider has reported any
/// usage — shows the spinner alone rather than a misleading `↑0 ↓0`.
///
/// The same white the label uses, dimmed: it is a secondary reading beside the
/// status text, and the balloon palette is hardcoded here for the reason the
/// indicator's own doc gives — the glass themes make `ui.visuals()` unreliable
/// against this backdrop.
pub(crate) fn token_counter(
    ui: &mut egui::Ui,
    tokens: Option<(u64, u64)>,
    font_size: f32,
    fg: Color32,
) {
    let Some((input, output)) = tokens.filter(|(i, o)| *i > 0 || *o > 0) else {
        return;
    };
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(format!(
            "↑{} ↓{}",
            crate::llm::compact_tokens(input),
            crate::llm::compact_tokens(output)
        ))
        .size(font_size - 1.0)
        .color(fg.gamma_multiply(0.75)),
    )
    .on_hover_text(format!(
        "{input} input / {output} output tokens, counted as each model call returns"
    ));
}

/// Spec 036 R1: the live current-action line — the same high-contrast
/// indicator balloon as [`chat_thinking_indicator`], but naming the agent's
/// actual step ("Form Designer Agent: Drafting response — T1") instead of a
/// generic "Thinking…". The caller passes the THROTTLED action
/// (`GraceSession::current_action`), never the raw stream.
pub(crate) fn chat_current_action(
    ui: &mut egui::Ui,
    action: &crate::agent_actions::AgentAction,
    tr: &crate::i18n::Tr,
    font_size: f32,
    tokens: Option<(u64, u64)>,
) {
    chat_thinking_indicator(ui, &action.display_line(tr), font_size, tokens);
}

/// Live Knowledge Base indexing progress bar: shown while chunk records are
/// being embedded so a long first index never looks stuck. Contrast is
/// hardcoded from the balloon palette (blue track fill, white text) — never
/// `ui.visuals()`, which glass themes make unreliable in the chat panes.
pub(crate) fn chat_indexing_bar(
    ui: &mut egui::Ui,
    done: u64,
    total: u64,
    tr: &crate::i18n::Tr,
    font_size: f32,
) {
    if total == 0 || done >= total {
        return;
    }
    let label = tr
        .kb_indexing
        .replacen("{}", &done.to_string(), 1)
        .replacen("{}", &total.to_string(), 1);
    let fraction = (done as f32 / total as f32).clamp(0.0, 1.0);
    let max_w = (ui.available_width() * 0.82).max(120.0);
    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
        ui.set_max_width(max_w);
        ui.add(
            egui::ProgressBar::new(fraction)
                .fill(chat_bubble_fill(false))
                .text(
                    egui::RichText::new(label)
                        .size(font_size)
                        .strong()
                        .color(Color32::WHITE),
                ),
        );
    });
    ui.add_space(5.0);
}

/// Chat-footer "model + context" indicator: the name of the model that served
/// the most recent agent call, plus a small ring gauge showing how much of
/// that model's context window the call's input consumed. Before any call has
/// run, the caller's fallback label (the surface's configured model) shows
/// with an empty ring. Gauge colors are hardcoded from the balloon palette
/// (blue arc on a dim track, red past 90%) — never `ui.visuals()`, which
/// glass themes make unreliable in the chat panes.
pub(crate) fn chat_model_context_indicator(
    ui: &mut egui::Ui,
    tr: &crate::i18n::Tr,
    fallback_model: Option<&str>,
) {
    let last = crate::llm::last_model_call();
    let label = last
        .as_ref()
        .map(|call| {
            if call.provider.trim().is_empty() {
                call.model.clone()
            } else {
                format!("{}/{}", call.provider, call.model)
            }
        })
        .or_else(|| fallback_model.map(str::to_owned));
    let Some(label) = label else {
        return;
    };
    ui.label(
        egui::RichText::new(label)
            .small()
            .color(crate::theme::active().text_dim),
    )
    .on_hover_text(tr.chat_model_hover);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(13.0, 13.0), egui::Sense::hover());
    let center = rect.center();
    let radius = rect.height() / 2.0 - 1.0;
    ui.painter()
        .circle_stroke(center, radius, egui::Stroke::new(1.6, Color32::from_gray(110)));
    let Some(call) = last else {
        response.on_hover_text(tr.chat_model_hover);
        return;
    };
    let window = crate::llm::context_window_hint(&call.model).max(1);
    let fraction = (call.input_tokens as f32 / window as f32).clamp(0.0, 1.0);
    if fraction > 0.0 {
        let color = if fraction >= 0.9 {
            Color32::from_rgb(0xE0, 0x5A, 0x4A)
        } else {
            chat_bubble_fill(false)
        };
        // Clockwise from 12 o'clock, like a clock filling up.
        let start = -std::f32::consts::FRAC_PI_2;
        let sweep = fraction * std::f32::consts::TAU;
        let segments = (fraction * 40.0).ceil().max(2.0) as usize;
        let points: Vec<egui::Pos2> = (0..=segments)
            .map(|i| {
                let angle = start + sweep * i as f32 / segments as f32;
                egui::pos2(
                    center.x + radius * angle.cos(),
                    center.y + radius * angle.sin(),
                )
            })
            .collect();
        ui.painter()
            .add(egui::Shape::line(points, egui::Stroke::new(2.2, color)));
    }
    let pct = (fraction * 100.0).round() as u32;
    response.on_hover_text(
        tr.chat_context_gauge_hover
            .replace("{used}", &call.input_tokens.to_string())
            .replace("{window}", &window.to_string())
            .replace("{pct}", &pct.to_string()),
    );
}

/// Spec 036 R3: the collapsed action history — an assistant-side balloon
/// holding a `CollapsingHeader` ("Agent actions (N)", collapsed by default)
/// with one attributed line per action. Contrast is hardcoded from the
/// balloon palette like every chat widget (glass themes make
/// `ui.visuals()`-derived colors unreliable here). `id` must be stable for
/// the widget's lifetime (the title text changes as actions accumulate, so
/// the open-state id cannot derive from it).
pub(crate) fn chat_action_history(
    ui: &mut egui::Ui,
    id: egui::Id,
    actions: &[crate::agent_actions::AgentAction],
    tr: &crate::i18n::Tr,
    font_size: f32,
) {
    if actions.is_empty() {
        return;
    }
    let fill = chat_bubble_fill(false);
    let fg = Color32::WHITE;
    let max_w = (ui.available_width() * 0.82).max(120.0);
    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
        egui::Frame::NONE
            .fill(fill)
            .corner_radius(egui::CornerRadius::same(15))
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.set_max_width(max_w);
                egui::CollapsingHeader::new(
                    egui::RichText::new(format!(
                        "{} ({})",
                        tr.agent_actions_header,
                        actions.len()
                    ))
                    .size(font_size)
                    .strong()
                    .color(fg),
                )
                .id_salt(id)
                .default_open(false)
                .show(ui, |ui| {
                    for action in actions {
                        ui.label(
                            egui::RichText::new(action.display_line(tr))
                                .size((font_size - 1.0).max(10.0))
                                .color(fg),
                        );
                    }
                });
            });
    });
    ui.add_space(5.0);
}

fn render_chat_bubble(ui: &mut egui::Ui, role: &str, content: &str, font_size: f32) {
    // An agent's question to the developer: its own balloon, red background,
    // white foreground, agent-side alignment. Always plain text — the red
    // fill and the Markdown card would fight each other.
    let is_question = role == "question";
    // Telemetry balloons (coordination transcript, run statistics, retrieval
    // savings) are agent-side like an assistant reply — they are excluded from
    // an agent's conversation history, not from the developer's view, so they
    // must keep looking the way they always have.
    let is_user =
        !is_question && role != "assistant" && role != crate::llm::TELEMETRY_ROLE;
    let fill = if is_question {
        Color32::from_rgb(0xC0, 0x2A, 0x22)
    } else {
        chat_bubble_fill(is_user)
    };
    let fg = egui::Color32::WHITE;
    let max_w = (ui.available_width() * 0.82).max(120.0);

    // Typographic rule: Markdown content in the history is rendered as
    // Markdown, not shown as raw text. The card uses the theme background so
    // the theme-aware renderer stays readable in light and dark modes.
    if !is_user && !is_question && looks_like_markdown(content) {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
            egui::Frame::NONE
                .fill(ui.visuals().extreme_bg_color)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .corner_radius(egui::CornerRadius::same(15))
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_max_width(max_w);
                    ui.vertical(|ui| {
                        let opts = crate::panels::md_render::RenderOpts {
                            search: "",
                            base: font_size,
                            scroll_to_heading: None,
                            active_match: None,
                            scroll_to_active: false,
                            anchors: &[],
                        };
                        crate::panels::md_render::render(
                            ui,
                            content.trim(),
                            &opts,
                            &mut |ui, code| {
                                ui.label(egui::RichText::new(code).monospace());
                            },
                        );
                    });
                });
        });
        return;
    }

    // Developer bubbles hug the right, assistant bubbles the left; text inside both
    // reads left-to-right.
    let layout = if is_user {
        egui::Layout::right_to_left(egui::Align::TOP)
    } else {
        egui::Layout::left_to_right(egui::Align::TOP)
    };
    ui.with_layout(layout, |ui| {
        egui::Frame::NONE
            .fill(fill)
            .corner_radius(egui::CornerRadius::same(15))
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.set_max_width(max_w);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(content.trim())
                            .monospace()
                            .size(font_size)
                            .color(fg),
                    )
                    .wrap(),
                );
            });
    });
}

pub(crate) fn chat_bubble_with_response_actions(
    ui: &mut egui::Ui,
    role: &str,
    content: &str,
    font_size: f32,
    project_root: Option<&Path>,
    action_id: egui::Id,
) {
    render_chat_bubble(ui, role, content, font_size);
    if role != "assistant" {
        ui.add_space(5.0);
        return;
    }

    let state_id = action_id.with("state");
    let dialog_key = format!("chat-response-markdown-{}", action_id.value());
    let mut state = ui
        .ctx()
        .data(|data| data.get_temp::<ChatResponseActionState>(state_id))
        .unwrap_or_default();

    if let Some(Some(path)) = crate::file_dialog::take(&dialog_key) {
        if let Some(root) = project_root {
            match save_agent_response_as_markdown(root, &path, content) {
                Ok(relative) => {
                    state.status = Some(format!("Saved {}", relative.display()));
                    state.error = None;
                }
                Err(error) => {
                    state.status = None;
                    state.error = Some(error);
                }
            }
        }
    }

    ui.horizontal(|ui| {
        if ui
            .small_button("📋")
            .on_hover_text("Copy agent response to the clipboard")
            .clicked()
        {
            ui.ctx().copy_text(content.to_owned());
            state.status = Some("Copied to clipboard".into());
        }

        let save_tooltip = if project_root.is_some() {
            "Save agent response as a Markdown file in this project's Knowledge Base"
        } else {
            "Open a project before saving an agent response"
        };
        if ui
            .add_enabled(
                project_root.is_some() && !crate::file_dialog::is_open(&dialog_key),
                egui::Button::new("💾").small(),
            )
            .on_hover_text(save_tooltip)
            .clicked()
        {
            let root = project_root.expect("save is enabled only with an open project");
            let documentation_root =
                root.join(cobolt_agents::project_knowledge::KNOWLEDGE_BASE_ROOT);
            match std::fs::create_dir_all(&documentation_root) {
                Ok(()) => crate::file_dialog::begin(
                    ui.ctx(),
                    &dialog_key,
                    crate::file_dialog::DialogSpec::save()
                        .filter("Markdown", &["md"])
                        .directory(documentation_root)
                        .file_name("agent-response.md"),
                ),
                Err(error) => {
                    state.status = None;
                    state.error = Some(format!(
                        "Could not prepare the project Knowledge Base: {error}"
                    ));
                }
            }
        }

        if let Some(status) = &state.status {
            ui.label(
                egui::RichText::new(status)
                    .small()
                    .color(Color32::from_rgb(125, 214, 160)),
            );
        }
    });

    if let Some(error) = state.error.clone() {
        let mut open = true;
        let mut dismiss = false;
        egui::Window::new("Save agent response error")
            .id(action_id.with("error"))
            .collapsible(false)
            .resizable(true)
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.label(&error);
                ui.horizontal(|ui| {
                    if ui.small_button("📋").on_hover_text("Copy error").clicked() {
                        ui.ctx().copy_text(error.clone());
                    }
                    if ui.button("OK").clicked() {
                        dismiss = true;
                    }
                });
            });
        if dismiss || !open {
            state.error = None;
        }
    }

    ui.ctx().data_mut(|data| data.insert_temp(state_id, state));
    ui.add_space(5.0);
}

#[cfg(test)]
mod autocomplete_dismissal_tests {
    use super::{ac_anchor_moved, ac_context_ended};
    use egui::Pos2;

    /// The popup stays put while the editor does. Sub-pixel layout jitter must
    /// not make it flicker away between frames.
    #[test]
    fn a_still_editor_keeps_the_popup() {
        let anchor = Pos2::new(120.0, 340.0);
        assert!(!ac_anchor_moved(anchor, anchor));
        assert!(!ac_anchor_moved(Pos2::new(120.2, 340.1), anchor));
    }

    /// Resizing the box or moving the window shifts the text under a popup
    /// pinned to screen coordinates, so it must close rather than float away.
    #[test]
    fn moving_or_resizing_the_editor_unanchors_the_popup() {
        let anchor = Pos2::new(120.0, 340.0);
        assert!(ac_anchor_moved(Pos2::new(120.0, 366.0), anchor), "vertical move");
        assert!(ac_anchor_moved(Pos2::new(48.0, 340.0), anchor), "horizontal move");
    }

    /// Typing a space or deleting the word ends the completion: nothing is
    /// being written, so nothing should be offered.
    #[test]
    fn an_empty_prefix_ends_the_completion() {
        assert!(ac_context_ended("", false));
        assert!(!ac_context_ended("Corner", false));
    }

    /// A member list (`Ctrl::`, `INVOKE ctrl '`, `"Prop" OF`) is legitimately
    /// open with no prefix yet — it lists every member until one is typed.
    #[test]
    fn a_member_context_survives_an_empty_prefix() {
        assert!(!ac_context_ended("", true));
    }
}

#[cfg(test)]
mod chat_bubble_tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn user_fill_matches_requested_rgba() {
        assert_eq!(
            super::chat_bubble_fill(true).to_array(),
            [0x61, 0xC6, 0x54, 0xFF]
        );
    }

    #[test]
    fn markdown_detection_targets_heavy_structure_only() {
        // Headings, tables, and fenced code get the document card.
        assert!(super::looks_like_markdown("### Clarification\n- file name?"));
        assert!(super::looks_like_markdown("| Field | Decision |\n| --- | --- |"));
        assert!(super::looks_like_markdown("```cobol\nMOVE A TO B\n```"));
        // Concise summaries — plain prose and simple bullet lists — stay in
        // the blue dialog bubble.
        assert!(!super::looks_like_markdown("**T1** — done"));
        assert!(!super::looks_like_markdown(
            "Executed:\n\n- indexed_file.write — wrote indexed/idx-company.cidx"
        ));
        assert!(!super::looks_like_markdown(
            "The workflow completed. All tasks were approved."
        ));
    }

    #[test]
    fn agent_response_markdown_is_scoped_and_indexed() {
        let root = std::env::temp_dir().join(format!(
            "prc-chat-response-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let selected = root.join("Knowledge Base/answer.txt");
        let saved = super::save_agent_response_as_markdown(
            &root,
            &selected,
            "# Agent answer\n\nIndexed payment guidance.",
        )
        .unwrap();

        assert_eq!(saved, PathBuf::from("Knowledge Base/answer.md"));
        assert!(root.join(&saved).exists());
        assert_eq!(
            cobolt_agents::project_knowledge::search(&root, "payment guidance", 2)
                .unwrap()
                .len(),
            1
        );
        assert!(super::take_chat_documentation_changed(&root));
        assert!(
            super::save_agent_response_as_markdown(&root, &root.join("outside.md"), "outside")
                .is_err()
        );
        let _ = std::fs::remove_dir_all(root);
    }
}

impl EditorPanel {
    pub fn new() -> Self {
        Self::default()
    }

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
        self.tabs
            .get(self.active)
            .and_then(|t| self.breakpoints.get(&t.path))
    }

    /// Return all breakpoint lines for a given path (for the runner to sync).
    pub fn breakpoints_for(&self, path: &PathBuf) -> Vec<u32> {
        self.breakpoints
            .get(path)
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
        self.ui_id_salt = path.to_string_lossy().to_string();
        self.tabs = vec![EditorTab::new(path, content)];
        self.active = 0;
        self.search.visible = false;
        self.ac.visible = false;
    }

    /// Restrict IntelliSense to contextual symbols (no COBOL reserved words or
    /// paragraph labels). Intended for natural-language AI prompt boxes that need
    /// accurate project/form names, not source templates.
    pub fn set_context_only_completions(&mut self, enabled: bool) {
        self.context_only_completions = enabled;
    }

    fn ui_id(&self, key: &'static str) -> egui::Id {
        egui::Id::new(("cobolt_editor_panel", self.ui_id_salt.as_str(), key))
    }

    /// The active buffer's text (for reading an embedded editor back).
    pub fn buffer_content(&self) -> Option<&str> {
        self.tabs.get(self.active).map(|t| t.content.as_str())
    }

    /// The active buffer's text, trimmed of trailing whitespace when the
    /// `Trim on save` toggle is on (used when an embedded editor is committed).
    pub fn buffer_for_save(&self) -> Option<String> {
        self.tabs.get(self.active).map(|t| {
            if self.trim_on_save {
                trim_trailing_ws(&t.content)
            } else {
                t.content.clone()
            }
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
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return Ok(());
        };
        if tab.read_only {
            return Ok(());
        } // never write generated source
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

    /// True when any writable tab has unsaved edits (drives the close prompt).
    pub fn any_dirty(&self) -> bool {
        self.tabs.iter().any(|t| t.dirty && !t.read_only)
    }

    /// Save every dirty, writable tab to disk. Used by the close-confirmation
    /// flow ("Save before close"). Stops at the first I/O error.
    pub fn save_all_dirty(&mut self) -> std::io::Result<()> {
        let trim = self.trim_on_save;
        for tab in &mut self.tabs {
            if tab.read_only || !tab.dirty {
                continue;
            }
            if trim {
                let trimmed = trim_trailing_ws(&tab.content);
                if trimmed != tab.content {
                    tab.content = trimmed;
                }
            }
            std::fs::write(&tab.path, &tab.content)?;
            tab.dirty = false;
        }
        Ok(())
    }

    /// "Beautify" the active tab: a conservative whitespace tidy that never
    /// touches COBOL area-A/B alignment — trim trailing spaces, collapse runs of
    /// blank lines, and end with a single newline.
    pub fn beautify_active(&mut self) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        if tab.read_only || tab.is_markdown() {
            return;
        }
        let tidy = beautify_cobol(&tab.content);
        if tidy != tab.content {
            tab.content = tidy;
            tab.dirty = true;
        }
    }

    pub fn active_source(&self) -> Option<(&PathBuf, &str)> {
        self.tabs
            .get(self.active)
            .map(|t| (&t.path, t.content.as_str()))
    }

    pub fn clear_diags(&mut self) {
        self.diags.clear();
    }

    pub fn add_diag(&mut self, path: &PathBuf, diag: DiagMsg) {
        self.diags.entry(path.clone()).or_default().push(diag);
    }

    // ── Search helpers ────────────────────────────────────────────────────────

    fn update_search_matches(&mut self) {
        self.search.matches.clear();
        self.search.current = 0;
        if self.search.query.is_empty() {
            return;
        }
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        // ASCII-lowercase (not Unicode) so byte length is preserved and match
        // offsets stay valid in the original content.
        let (hay, needle) = if self.search.case_insensitive {
            (
                tab.content.to_ascii_lowercase(),
                self.search.query.to_ascii_lowercase(),
            )
        } else {
            (tab.content.clone(), self.search.query.clone())
        };
        let qlen = needle.len();
        if qlen == 0 {
            return;
        }
        let mut start = 0;
        while start + qlen <= hay.len() {
            if let Some(rel) = hay[start..].find(&needle) {
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
        let Some(tab) = self.tabs.get(self.active) else {
            return false;
        };
        let needle = paragraph.trim().to_ascii_uppercase();
        if needle.is_empty() {
            return false;
        }
        let upper = tab.content.to_ascii_uppercase();

        // Prefer a real definition: the name at the start of an indented line,
        // either as a paragraph header (`NAME.`) or `PROGRAM-ID. NAME.`.
        let mut found: Option<usize> = None;
        let mut off = 0usize;
        for line in upper.split_inclusive('\n') {
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
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
        if self.search.matches.is_empty() {
            return;
        }
        self.search.current = (self.search.current + 1) % self.search.matches.len();
        self.search.needs_scroll = true;
        self.search.focus_editor_on_scroll = true;
    }

    fn search_prev(&mut self) {
        if self.search.matches.is_empty() {
            return;
        }
        self.search.current = if self.search.current == 0 {
            self.search.matches.len() - 1
        } else {
            self.search.current - 1
        };
        self.search.needs_scroll = true;
        self.search.focus_editor_on_scroll = true;
    }

    /// Replace the currently-highlighted match with the replacement text, then
    /// re-scan and keep the find cursor valid.
    fn replace_current(&mut self) {
        let q = self.search.query.clone();
        if q.is_empty() {
            return;
        }
        let repl = self.search.replace.clone();
        let cur = self.search.current;
        let Some(&byte_off) = self.search.matches.get(cur) else {
            return;
        };
        {
            let Some(tab) = self.tabs.get_mut(self.active) else {
                return;
            };
            if tab.read_only {
                return;
            }
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
        if q.is_empty() {
            return;
        }
        let repl = self.search.replace.clone();
        {
            let Some(tab) = self.tabs.get_mut(self.active) else {
                return;
            };
            if tab.read_only {
                return;
            }
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
        panel_ui: &mut egui::Ui,
        ctx: &Context,
        cfg: &crate::llm::LlmConfig,
        tr: &crate::i18n::Tr,
        panel_id: &str,
        target: &std::path::Path,
        code: &str,
        read_only: bool,
        project_root: Option<&std::path::Path>,
    ) -> Option<String> {
        self.ai_bar_impl(
            Some(panel_ui),
            ctx,
            cfg,
            tr,
            panel_id,
            target,
            code,
            read_only,
            project_root,
            None,
        )
    }

    /// Same assistant, rendered **inline** into an existing `ui` (for a modal
    /// window that can't host a `TopBottomPanel`, e.g. the COBOL Structure editor).
    #[allow(clippy::too_many_arguments)]
    pub fn ai_bar_inline(
        &mut self,
        ui: &mut egui::Ui,
        cfg: &crate::llm::LlmConfig,
        tr: &crate::i18n::Tr,
        panel_id: &str,
        target: &std::path::Path,
        code: &str,
        read_only: bool,
        project_root: Option<&std::path::Path>,
    ) -> Option<String> {
        let ctx = ui.ctx().clone();
        self.ai_bar_impl(
            None,
            &ctx,
            cfg,
            tr,
            panel_id,
            target,
            code,
            read_only,
            project_root,
            Some(ui),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn ai_bar_impl(
        &mut self,
        panel_ui: Option<&mut egui::Ui>,
        ctx: &Context,
        cfg: &crate::llm::LlmConfig,
        tr: &crate::i18n::Tr,
        panel_id: &str,
        target: &std::path::Path,
        code: &str,
        read_only: bool,
        project_root: Option<&std::path::Path>,
        inline_ui: Option<&mut egui::Ui>,
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
        if let Some((pending_path, rx)) = self.ai_pending.take() {
            if pending_path == path {
                let mut keep_pending = true;
                loop {
                    match rx.try_recv() {
                        Ok(crate::llm::LlmResponse::Chunk(text)) => {
                            self.ai_streaming_reply
                                .entry(path.clone())
                                .or_default()
                                .push_str(&text);
                            ctx.request_repaint();
                        }
                        Ok(resp) => {
                            self.ai_streaming_reply.remove(&path);
                            completed = Some(resp);
                            keep_pending = false;
                            break;
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            ctx.request_repaint();
                            break;
                        }
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            completed = Some(crate::llm::LlmResponse::Err(
                                "The assistant worker stopped unexpectedly.".into(),
                            ));
                            keep_pending = false;
                            break;
                        }
                    }
                }
                if keep_pending {
                    self.ai_pending = Some((pending_path, rx));
                }
            } else {
                self.ai_pending = Some((pending_path, rx));
                ctx.request_repaint();
            }
        }
        let mut applied: Option<String> = None;
        if let Some(resp) = completed {
            applied = self.apply_ai_response(&path, resp, tr, read_only, project_root);
        }

        // Poll an in-flight compaction request for this target.
        let mut compaction: Option<crate::llm::LlmResponse> = None;
        if let Some((cpath, crx)) = &self.ai_compact_pending {
            if cpath == &path {
                match crx.try_recv() {
                    Ok(resp) => compaction = Some(resp),
                    Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint(),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        compaction = Some(crate::llm::LlmResponse::Err(
                            "The assistant worker stopped unexpectedly.".into(),
                        ));
                    }
                }
            } else {
                ctx.request_repaint();
            }
        }
        if let Some(resp) = compaction {
            self.ai_compact_pending = None;
            self.apply_compaction(&path, resp, tr, project_root);
        }

        let busy = self
            .ai_pending
            .as_ref()
            .map(|(p, _)| *p == path)
            .unwrap_or(false);
        let compacting = self
            .ai_compact_pending
            .as_ref()
            .map(|(p, _)| *p == path)
            .unwrap_or(false);
        let history_len = self.ai_history.get(&path).map(|v| v.len()).unwrap_or(0);

        // Snapshot UI-owned state so the panel closure borrows locals, not `self`.
        let mut prompt = std::mem::take(&mut self.ai_prompt);
        let mut show_history = self.ai_show_history;
        let status = self.ai_status.clone();
        let history_snapshot: Vec<crate::llm::ChatTurn> =
            self.ai_history.get(&path).cloned().unwrap_or_default();

        let mut do_send = false;
        let mut do_clear = false;
        let mut do_save = false;
        let mut do_compact = false;

        let mut render = |ui: &mut egui::Ui| {
            // Conversation transcript.
            let streaming_text = self.ai_streaming_reply.get(&path).cloned();
            if show_history && (!history_snapshot.is_empty() || streaming_text.is_some()) {
                egui::ScrollArea::vertical()
                    .max_height(170.0)
                    .auto_shrink([false, true])
                    .id_salt(transcript_salt)
                    .show(ui, |ui| {
                        for (index, turn) in history_snapshot.iter().enumerate() {
                            chat_bubble_with_response_actions(
                                ui,
                                &turn.role,
                                &turn.content,
                                14.0,
                                project_root,
                                egui::Id::new((panel_id, &path, index)),
                            );
                        }
                        if let Some(text) = streaming_text {
                            chat_bubble(ui, "assistant", &text);
                        }
                    });
                ui.separator();
            }

            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("✨").size(15.0));
                if busy || compacting {
                    ui.add(egui::Spinner::new());
                    let msg = if compacting {
                        tr.ai_compacting
                    } else {
                        tr.ai_thinking
                    };
                    ui.label(
                        egui::RichText::new(msg)
                            .small()
                            .color(Color32::from_gray(170)),
                    );
                    // This surface runs one call at a time and keeps no
                    // accumulator of its own, so it reads the process-wide
                    // meter: the session's running total, which moves when
                    // this request returns.
                    token_counter(
                        ui,
                        Some(crate::llm::token_meter()),
                        11.0,
                        Color32::from_gray(170),
                    );
                }
                if history_len > 0 {
                    ui.toggle_value(
                        &mut show_history,
                        egui::RichText::new(format!("💬 {history_len}")).small(),
                    );
                    if ui
                        .small_button("💾")
                        .on_hover_text(tr.ai_save_history)
                        .clicked()
                    {
                        do_save = true;
                    }
                    if ui
                        .add_enabled(!busy && !compacting, egui::Button::new("🗜").small())
                        .on_hover_text(tr.ai_compact_history)
                        .clicked()
                    {
                        do_compact = true;
                    }
                    if ui
                        .small_button("🗑")
                        .on_hover_text(tr.ai_clear_history)
                        .clicked()
                    {
                        do_clear = true;
                    }
                }
            });

            ui.horizontal(|ui| {
                let prompt_width =
                    super::chat_prompt_width(ui.available_width(), ui.spacing().item_spacing.x);
                let resp = ui.add_sized(
                    [prompt_width, ui.spacing().interact_size.y],
                    egui::TextEdit::singleline(&mut prompt)
                        .hint_text(tr.ai_prompt_placeholder)
                        .interactive(!busy),
                );
                let entered = resp.lost_focus()
                    && ui.input(|i| i.key_pressed(Key::Enter))
                    && !prompt.trim().is_empty();
                if entered && !busy {
                    do_send = true;
                    show_history = true;
                }
                let can_send = !busy && !prompt.trim().is_empty();
                if ui
                    .add_enabled(
                        can_send,
                        egui::Button::new(tr.ai_send).min_size(egui::vec2(
                            super::CHAT_SEND_BUTTON_WIDTH,
                            ui.spacing().interact_size.y,
                        )),
                    )
                    .clicked()
                {
                    do_send = true;
                    show_history = true;
                }
            });

            if read_only {
                ui.label(
                    egui::RichText::new(tr.ai_read_only)
                        .small()
                        .color(Color32::from_gray(150)),
                );
            } else if let Some(s) = &status {
                ui.label(
                    egui::RichText::new(s)
                        .small()
                        .color(Color32::from_gray(165)),
                );
            }
        };
        match inline_ui {
            Some(ui) => render(ui),
            None => {
                let frame = crate::theme::glass_panel_frame(
                    ctx.global_style().visuals.panel_fill,
                    &crate::theme::active(),
                );
                let host = panel_ui.expect("ai_bar panel variant requires a host Ui");
                Panel::top(panel).frame(frame).show(host, |ui| render(ui));
            }
        }

        // Restore UI-owned state.
        self.ai_prompt = prompt;
        self.ai_show_history = show_history;

        // Save: force-persist this target's current conversation (it is also
        // auto-saved after every turn) and confirm to the developer.
        if do_save {
            let turns = self.ai_history.get(&path).cloned().unwrap_or_default();
            Self::persist_history(project_root, &path, &turns);
            self.ai_status = Some(tr.ai_history_saved.to_string());
        }

        // Compact: summarise the conversation on a worker thread.
        if do_compact && !busy && !compacting {
            self.start_compaction(&path, cfg);
        }

        // Clear: ask for confirmation before deleting.
        if do_clear {
            self.ai_confirm_clear = Some(path.clone());
        }

        // Clear-history confirmation dialog (per target).
        if self.ai_confirm_clear.as_deref() == Some(path.as_path()) {
            let mut cancel = false;
            let mut confirm = false;
            egui::Window::new(tr.ai_clear_confirm_title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(tr.ai_clear_confirm_body);
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
                self.ai_confirm_clear = None;
            }
            if confirm {
                self.ai_confirm_clear = None;
                self.ai_history.get_mut(&path).map(|h| h.clear());
                self.ai_streaming_reply.remove(&path);
                Self::persist_history(project_root, &path, &[]);
                self.ai_status = None;
                self.ai_show_history = false;
            }
        }

        if do_send && !busy {
            self.send_ai_prompt(&path, cfg, code, project_root, *tr);
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
        let key = path
            .strip_prefix(root)
            .ok()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| {
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
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
        tr: crate::i18n::Tr,
    ) {
        let prompt = self.ai_prompt.trim().to_string();
        if prompt.is_empty() {
            return;
        }
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("source.cbl")
            .to_string();

        let prior = self.ai_history.get(path).cloned().unwrap_or_default();
        // Include the project's RustCOBOL skills (agentic_ai/skills) as reference
        // material so the assistant follows the same conventions as the dev agent.
        let skills = project_root
            .map(crate::agent::load_skills)
            .unwrap_or_default();
        let rx = match project_root {
            Some(root) => crate::grace_session::spawn_contextual_request(
                root,
                cfg,
                &prior,
                &prompt,
                "COBOL code editor chatbot",
                None,
                &format!("Current file `{filename}`:\n```cobol\n{code}\n```"),
                tr,
            ),
            None => crate::llm::spawn_request(cfg, &prior, &prompt, code, &filename, &skills, None),
        };

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
                match crate::llm::extract_code(&reply) {
                    Some(code) if !read_only => {
                        let log = self.ai_history.entry(path.clone()).or_default();
                        log.push(crate::llm::ChatTurn::assistant(&reply));
                        Self::persist_history(project_root, path, log);
                        self.ai_status = Some(tr.ai_updated.to_string());
                        Some(code)
                    }
                    Some(_) => {
                        let log = self.ai_history.entry(path.clone()).or_default();
                        log.push(crate::llm::ChatTurn::assistant(&reply));
                        Self::persist_history(project_root, path, log);
                        self.ai_status = Some(tr.ai_read_only.to_string());
                        None
                    }
                    None => {
                        // No code block ⇒ Grace answered in prose or asked a
                        // clarifying question. Split it the same way the RAD
                        // designer's own prompt box does, so a question gets its
                        // own highlighted balloon instead of blending into a
                        // plain reply.
                        let (context, questions) =
                            crate::grace_host::split_developer_questions(&reply);
                        let log = self.ai_history.entry(path.clone()).or_default();
                        if questions.is_empty() {
                            log.push(crate::llm::ChatTurn::assistant(&reply));
                        } else {
                            if !context.trim().is_empty() {
                                log.push(crate::llm::ChatTurn::assistant(context));
                            }
                            for q in questions {
                                log.push(crate::llm::ChatTurn::question(q));
                            }
                        }
                        Self::persist_history(project_root, path, log);
                        self.ai_status = None; // Treat as a valid conversational turn
                        None
                    }
                }
            }
            crate::llm::LlmResponse::Err(e) => {
                self.ai_status = Some(e);
                None
            }
            crate::llm::LlmResponse::Chunk(_) => None,
        }
    }

    /// Kick off a compaction (summarization) request for `path`'s conversation.
    fn start_compaction(&mut self, path: &PathBuf, cfg: &crate::llm::LlmConfig) {
        let history = self.ai_history.get(path).cloned().unwrap_or_default();
        if history.is_empty() {
            return;
        }
        let rx = crate::llm::spawn_compaction(cfg, &history);
        self.ai_compact_pending = Some((path.clone(), rx));
        self.ai_status = None;
        self.ai_show_history = true;
    }

    /// Replace `path`'s conversation with the model's compacted summary. On error
    /// the existing history is left untouched.
    fn apply_compaction(
        &mut self,
        path: &PathBuf,
        resp: crate::llm::LlmResponse,
        tr: &crate::i18n::Tr,
        project_root: Option<&std::path::Path>,
    ) {
        match resp {
            crate::llm::LlmResponse::Ok(summary) => {
                let summary = summary.trim();
                if summary.is_empty() {
                    self.ai_status = Some(tr.ai_no_code.to_string());
                    return;
                }
                let turns = vec![crate::llm::ChatTurn::user(format!(
                    "[Compacted conversation summary]\n\n{summary}"
                ))];
                Self::persist_history(project_root, path, &turns);
                self.ai_history.insert(path.clone(), turns);
                self.ai_status = Some(tr.ai_compacted.to_string());
            }
            crate::llm::LlmResponse::Err(e) => {
                self.ai_status = Some(e);
            }
            crate::llm::LlmResponse::Chunk(_) => {}
        }
    }

    /// The bottom status bar: caret position, Insert/Overwrite mode, a
    /// trim-on-save toggle, and a Beautify command. Dimmed-green text.
    fn show_status_bar(&mut self, panel_ui: &mut egui::Ui, ctx: &Context) {
        let frame = egui::Frame::default()
            .fill(ctx.global_style().visuals.panel_fill)
            .inner_margin(egui::Margin::symmetric(8, 3));
        Panel::bottom("editor_status")
            .frame(frame)
            .show(panel_ui, |ui| {
                self.status_row(ui);
            });
    }

    /// Draw the status row (caret position · Insert/Overwrite · Trim-on-save,
    /// plus Beautify for non-Markdown documents) into an arbitrary `ui`, in
    /// dimmed green. Shared by the main editor and embedded RAD editor.
    pub(crate) fn status_row(&mut self, ui: &mut egui::Ui) {
        let active_tab = self.tabs.get(self.active);
        let read_only = active_tab.map(|tab| tab.read_only).unwrap_or(false);
        let show_beautify = active_tab.is_some_and(|tab| !tab.is_markdown());
        let green = Color32::from_rgb(118, 158, 110); // dimmed green
        let txt = |s: String| egui::RichText::new(s).monospace().size(12.0).color(green);
        let mut do_beautify = false;

        ui.horizontal(|ui| {
            ui.label(txt(format!("Ln {}, Col {}", self.cur_line, self.cur_col)));
            ui.label(txt("│".into()));
            let mode = if self.overwrite { "OVR" } else { "INS" };
            if ui
                .add(egui::Label::new(txt(mode.into())).sense(egui::Sense::click()))
                .on_hover_text("Toggle Insert/Overwrite (Insert key)")
                .clicked()
            {
                self.overwrite = !self.overwrite;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if show_beautify
                    && ui
                        .add_enabled(!read_only, egui::Button::new(txt("✨ Beautify".into())))
                        .on_hover_text("Tidy whitespace (safe for COBOL columns)")
                        .clicked()
                {
                    do_beautify = true;
                }
                ui.add_enabled(
                    !read_only,
                    egui::Checkbox::new(&mut self.trim_on_save, txt("Trim on save".into())),
                );
            });
        });

        if do_beautify {
            self.beautify_active();
        }
    }

    pub fn show(
        &mut self,
        panel_ui: &mut egui::Ui,
        ctx: &Context,
        llm: Option<&crate::llm::LlmConfig>,
        tr: &crate::i18n::Tr,
        project_root: Option<&std::path::Path>,
    ) {
        // ─── Tab bar ─────────────────────────────────────────────────────────
        Panel::top("editor_tabs").show(panel_ui, |ui| {
            ui.horizontal(|ui| {
                let mut close_idx: Option<usize> = None;
                for (i, tab) in self.tabs.iter().enumerate() {
                    let sel = i == self.active;
                    let resp = ui.selectable_label(sel, tab.title());
                    if resp.clicked() {
                        self.active = i;
                    }
                    if resp.middle_clicked() {
                        close_idx = Some(i);
                    }
                    if sel && ui.small_button("×").clicked() {
                        close_idx = Some(i);
                    }
                    ui.separator();
                }
                if let Some(idx) = close_idx {
                    self.tabs.remove(idx);
                    if self.active >= self.tabs.len() && !self.tabs.is_empty() {
                        self.active = self.tabs.len() - 1;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("A+")
                        .on_hover_text("Increase font size")
                        .clicked()
                    {
                        self.font_size = (self.font_size + 1.0).min(24.0);
                    }
                    if ui
                        .small_button("A−")
                        .on_hover_text("Decrease font size")
                        .clicked()
                    {
                        self.font_size = (self.font_size - 1.0).max(8.0);
                    }
                    ui.label(
                        egui::RichText::new(format!("{}pt", self.font_size as u32))
                            .small()
                            .color(Color32::from_gray(160)),
                    );
                });
            });
        });

        // ─── AI assistant bar (only for editable tabs when a model is configured) ───────────────
        if let Some(cfg) = llm {
            if cfg.is_configured() && !self.tabs.is_empty() {
                let (tpath, tcode, tro) = {
                    let t = &self.tabs[self.active];
                    (t.path.clone(), t.content.clone(), t.read_only)
                };
                if !tro {
                    if let Some(new_code) = self.ai_bar(
                        panel_ui,
                        ctx,
                        cfg,
                        tr,
                        "editor_ai",
                        &tpath,
                        &tcode,
                        false,
                        project_root,
                    ) {
                        if let Some(t) = self.tabs.iter_mut().find(|t| t.path == tpath) {
                            t.content = new_code;
                            t.dirty = true;
                        }
                    }
                }
            }
        }

        // ─── Status bar (bottom) ──────────────────────────────────────────────
        if !self.tabs.is_empty() {
            self.show_status_bar(panel_ui, ctx);
        }

        // ─── Editor body ──────────────────────────────────────────────────────
        let body_frame = crate::theme::glass_panel_frame(
            ctx.global_style().visuals.panel_fill,
            &crate::theme::active(),
        );
        CentralPanel::default()
            .frame(body_frame)
            .show(panel_ui, |ui| {
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
                    egui::RichText::new("Select an item in the project tree to see its details.")
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
                ctx.memory_mut(|m| m.request_focus(self.ui_id("search_input")));
            }
        }

        // Key intercept for auto-complete navigation
        let mut key_down = false;
        let mut key_up = false;
        let mut key_apply = false;
        let mut key_dismiss = false;
        let trigger_manual = ctx.input(|i| i.key_pressed(Key::Space) && i.modifiers.ctrl);

        // Insert key toggles Insert/Overwrite typing mode.
        if ctx.input(|i| i.key_pressed(Key::Insert) && i.modifiers.is_none()) {
            self.overwrite = !self.overwrite;
        }

        // Search key handling (only when search focused)
        let search_has_focus = ctx.memory(|m| m.has_focus(self.ui_id("search_input")));
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
                    egui::Event::Key {
                        key: Key::ArrowDown,
                        pressed: true,
                        modifiers,
                        ..
                    } if modifiers.is_none() => {
                        key_down = true;
                        false
                    }
                    egui::Event::Key {
                        key: Key::ArrowUp,
                        pressed: true,
                        modifiers,
                        ..
                    } if modifiers.is_none() => {
                        key_up = true;
                        false
                    }
                    egui::Event::Key {
                        key: Key::Tab,
                        pressed: true,
                        ..
                    } => {
                        key_apply = true;
                        false
                    }
                    egui::Event::Key {
                        key: Key::Enter,
                        pressed: true,
                        modifiers,
                        ..
                    } if modifiers.is_none() => {
                        key_apply = true;
                        false
                    }
                    egui::Event::Key {
                        key: Key::Escape,
                        pressed: true,
                        ..
                    } => {
                        key_dismiss = true;
                        false
                    }
                    _ => true,
                });
            });
            if key_down {
                self.ac.selected =
                    (self.ac.selected + 1).min(self.ac.items.len().saturating_sub(1));
            }
            if key_up {
                self.ac.selected = self.ac.selected.saturating_sub(1);
            }
            if key_down || key_up {
                self.ac.scroll_to_sel = true;
            }
            if key_dismiss {
                self.ac.visible = false;
            }
        }

        // ── Apply selected completion ──────────────────────────────────────
        let mut set_cursor_to: Option<usize> = None;
        // Whether to move keyboard focus into the editor after repositioning the
        // cursor. Autocomplete-apply always does; search only on navigation.
        let mut focus_editor_after = false;
        if key_apply {
            if let Some(item) = self.ac.items.get(self.ac.selected).cloned() {
                let tab = &mut self.tabs[self.active];
                // trigger_pos is a *char* index; convert to byte offset for replace_range.
                let trigger_byte = tab
                    .content
                    .char_indices()
                    .nth(self.ac.trigger_pos)
                    .map(|(b, _)| b)
                    .unwrap_or(tab.content.len());
                let end_byte = (trigger_byte + self.ac.prefix.len()).min(tab.content.len());
                tab.content
                    .replace_range(trigger_byte..end_byte, &item.insert);
                // set_cursor_to is a *char* count for CCursor
                let insert_chars = item.insert.chars().count();
                set_cursor_to = Some(self.ac.trigger_pos + insert_chars);
                tab.dirty = true;
                focus_editor_after = true;
            }
            self.ac.visible = false;
        }

        // ── Layout ────────────────────────────────────────────────────────
        let font = FontId::monospace(self.font_size);
        let editor_id = self.ui_id("editor");

        let kw_set: std::collections::HashSet<&'static str> = VERBS
            .iter()
            .chain(DIVISION_KEYWORDS.iter())
            .chain(DATA_KEYWORDS.iter())
            .chain(COBOL2002_KEYWORDS.iter())
            .copied()
            .collect();
        let font_hl = font.clone();
        // Read-only RAD-generated source renders in flat blue (no syntax
        // colours) so it's visually distinct from editable Common Code.
        let read_only = self
            .tabs
            .get(self.active)
            .map(|t| t.read_only)
            .unwrap_or(false);
        let mut layouter =
            move |ui: &egui::Ui, buf: &dyn egui::TextBuffer, _wrap: f32| -> Arc<egui::Galley> {
                let text = buf.as_str();
                let lj = if read_only {
                    mono_layout_job(text, font_hl.clone(), crate::theme::active().ed_generated)
                } else {
                    cobol_layout_job(text, font_hl.clone(), &kw_set)
                };
                ui.fonts_mut(|f| f.layout_job(lj))
            };

        let avail = ui.available_size();
        // Stable editor viewport rect — captured BEFORE the ScrollArea, so it
        // does not move when navigating matches scrolls the text. Anchoring the
        // floating find bar to this (instead of the scrolling TextEdit content
        // rect, whose `min.y` slides with the scroll offset) keeps the bar from
        // drifting up and down during a search.
        let editor_viewport = ui.max_rect();

        ScrollArea::both()
            .id_salt(self.ui_id("scroll"))
            .auto_shrink([false, false])
            .min_scrolled_height(avail.y)
            .show(ui, |ui| {
                ui.set_min_height(avail.y);
                ui.horizontal_top(|ui| {
                    // ── Breakpoint + line-number gutter ──────────────────
                    // We only RESERVE the gutter here; the numbers are painted
                    // after the TextEdit using the galley's real row positions
                    // so they line up exactly with the text (computing a row
                    // height up front drifts because egui pixel-rounds each row).
                    let mut gutter_state: Option<(
                        egui::Rect,
                        f32,
                        std::collections::HashSet<u32>,
                        Option<u32>,
                    )> = None;
                    if self.show_line_numbers {
                        let n_lines = self.tabs[self.active].content.lines().count().max(1);
                        let line_h = ui.fonts_mut(|f| f.row_height(&font));
                        let gutter_w = 54.0_f32;
                        let (gutter_rect, gutter_resp) = ui.allocate_exact_size(
                            egui::vec2(gutter_w, line_h * n_lines as f32),
                            egui::Sense::click(),
                        );
                        if gutter_resp.clicked() {
                            if let Some(pos) = gutter_resp.interact_pointer_pos() {
                                let rel_y = (pos.y - gutter_rect.min.y).max(0.0);
                                let clicked_line = (rel_y / line_h).floor() as u32 + 1;
                                self.toggle_breakpoint(clicked_line);
                            }
                        }
                        let bp_set: std::collections::HashSet<u32> =
                            self.active_breakpoints().cloned().unwrap_or_default();
                        let debug_line = self.debug_line.as_ref().and_then(|(p, l)| {
                            self.tabs
                                .get(self.active)
                                .filter(|t| t.path == *p)
                                .map(|_| *l)
                        });
                        gutter_state = Some((gutter_rect, gutter_w, bp_set, debug_line));
                        ui.add(egui::Separator::default().vertical().spacing(2.0));
                    }

                    // ── Overwrite mode ────────────────────────────────────
                    // egui's TextEdit is insert-only, so we emulate overwrite:
                    // when a printable character is about to be typed and the
                    // caret is collapsed, pre-select the next character so the
                    // insert replaces it (unless at end-of-line / EOF).
                    if self.overwrite && !self.tabs[self.active].read_only {
                        let typing = ctx.input(|i| {
                            i.events.iter().any(|e| {
                                matches!(e, egui::Event::Text(t)
                                    if t.chars().any(|c| !c.is_control()))
                            })
                        });
                        if typing {
                            let content = self.tabs[self.active].content.clone();
                            if let Some(mut st) = egui::TextEdit::load_state(ctx, editor_id) {
                                if let Some(range) = st.cursor.char_range() {
                                    if range.primary == range.secondary {
                                        let idx = range.primary.index.0;
                                        let next = content.chars().nth(idx);
                                        if matches!(next, Some(c) if c != '\n') {
                                            let mut r = range;
                                            r.secondary.index = egui::text::CharIndex(idx + 1);
                                            st.cursor.set_char_range(Some(r));
                                            st.store(ctx, editor_id);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // ── Source (read-only for RAD-generated code) ─────────
                    if !self.tabs[self.active].read_only
                        && ctx.memory(|m| m.has_focus(editor_id))
                        && !self.ac.visible
                        && !search_has_focus
                    {
                        let mut auto_indent = false;
                        ctx.input_mut(|inp| {
                            inp.events.retain(|ev| match ev {
                                egui::Event::Key {
                                    key: Key::Enter,
                                    pressed: true,
                                    modifiers,
                                    ..
                                } if modifiers.is_none() => {
                                    auto_indent = true;
                                    false
                                }
                                _ => true,
                            });
                        });
                        if auto_indent {
                            if let Some(mut state) = egui::TextEdit::load_state(ctx, editor_id) {
                                if let Some(range) = state.cursor.char_range() {
                                    let tab = &mut self.tabs[self.active];
                                    let new_cursor =
                                        insert_auto_indented_newline(&mut tab.content, range);
                                    state.cursor.set_char_range(Some(
                                        egui::text::CCursorRange::one(egui::text::CCursor::new(
                                            new_cursor,
                                        )),
                                    ));
                                    state.store(ctx, editor_id);
                                    tab.dirty = true;
                                }
                            }
                        }
                    }

                    let tab = &mut self.tabs[self.active];
                    let te_out = TextEdit::multiline(&mut tab.content)
                        .id(editor_id)
                        .font(font.clone())
                        .desired_width(f32::INFINITY)
                        .frame(egui::Frame::NONE) // no border/inset → gutter aligns
                        .margin(egui::Margin::ZERO)
                        .lock_focus(true)
                        .interactive(!tab.read_only)
                        .layouter(&mut layouter)
                        .show(ui);

                    // Paint the line-number gutter using the galley's actual
                    // per-row rectangles, so every number aligns with its row.
                    if let Some((gutter_rect, gutter_w, bp_set, debug_line)) = &gutter_state {
                        let painter = ui.painter().with_clip_rect(ui.clip_rect());
                        let num_font = FontId::monospace(self.font_size - 1.0);
                        for (i, row) in te_out.galley.rows.iter().enumerate() {
                            let line_num = (i + 1) as u32;
                            let yc = te_out.galley_pos.y + row.rect().center().y;
                            let row_h = row.rect().height();
                            if *debug_line == Some(line_num) {
                                painter.rect_filled(
                                    egui::Rect::from_min_size(
                                        egui::pos2(gutter_rect.min.x, yc - row_h * 0.5),
                                        egui::vec2(*gutter_w, row_h),
                                    ),
                                    0.0,
                                    Color32::from_rgba_unmultiplied(255, 220, 0, 40),
                                );
                                painter.text(
                                    egui::pos2(gutter_rect.min.x + 4.0, yc),
                                    egui::Align2::LEFT_CENTER,
                                    "→",
                                    num_font.clone(),
                                    Color32::from_rgb(255, 200, 0),
                                );
                            }
                            if bp_set.contains(&line_num) {
                                painter.circle_filled(
                                    egui::pos2(gutter_rect.min.x + 6.0, yc),
                                    4.5,
                                    Color32::from_rgb(220, 60, 60),
                                );
                            }
                            painter.text(
                                egui::pos2(gutter_rect.max.x - 4.0, yc),
                                egui::Align2::RIGHT_CENTER,
                                format!("{line_num}"),
                                num_font.clone(),
                                Color32::from_gray(if *debug_line == Some(line_num) {
                                    220
                                } else {
                                    100
                                }),
                            );
                        }
                    }

                    if te_out.response.changed() && !tab.read_only {
                        tab.dirty = true;
                    }

                    // Reposition cursor (completion apply or search navigate)
                    // Search navigation: set cursor to match position
                    if self.search.needs_scroll {
                        self.search.needs_scroll = false;
                        if let Some(&byte_off) = self.search.matches.get(self.search.current) {
                            let char_idx = tab.content[..byte_off.min(tab.content.len())]
                                .chars()
                                .count();
                            set_cursor_to = Some(char_idx);
                            if self.search.focus_editor_on_scroll {
                                self.search.focus_editor_on_scroll = false;
                                focus_editor_after = true;
                            }
                            // Scroll the viewport so the match is visible
                            let content_before = &tab.content[..byte_off.min(tab.content.len())];
                            let line_num = content_before.matches('\n').count();
                            let line_h = ui.fonts_mut(|f| f.row_height(&font));
                            let match_y = te_out.galley_pos.y + line_num as f32 * line_h;
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
                        if let Some(mut state) = egui::TextEdit::load_state(ctx, te_out.response.id)
                        {
                            let cc = egui::text::CCursor::new(pos);
                            state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::one(cc)));
                            state.store(ctx, te_out.response.id);
                        }
                        // Move keyboard focus into the editor for autocomplete-apply
                        // and explicit search navigation (next/prev/Enter) — but NOT
                        // while typing in the Find box, or every keystroke would kick
                        // the user out of it.
                        if focus_editor_after {
                            ctx.memory_mut(|m| m.request_focus(editor_id));
                        }
                    }

                    // ── IntelliSense update ───────────────────────────────
                    if let Some(cr) = te_out.cursor_range {
                        // The popup is anchored to the cursor as it was when it
                        // opened. If the editor has moved or been resized since,
                        // that anchor is stale and the list would sit away from
                        // the text — close it and let the next keystroke reopen
                        // it in the right place.
                        if self.ac.visible && ac_anchor_moved(te_out.galley_pos, self.ac.anchor)
                        {
                            self.ac.visible = false;
                            self.ac.member_mode = false;
                        }
                        let char_idx = cr.primary.index.0;
                        let (l, c) = char_index_to_line_col(&tab.content, char_idx);
                        self.cur_line = l;
                        self.cur_col = c;
                        let (word_start, prefix) = word_before_cursor(&tab.content, char_idx);

                        // Current line up to the cursor (for property refs).
                        let cur_byte = tab
                            .content
                            .char_indices()
                            .nth(char_idx)
                            .map(|(b, _)| b)
                            .unwrap_or(tab.content.len());
                        let line_start = tab.content[..cur_byte]
                            .rfind('\n')
                            .map(|p| p + 1)
                            .unwrap_or(0);
                        let line_to_cursor = &tab.content[line_start..cur_byte];

                        let inside_plain_string =
                            is_inside_plain_string_literal(&tab.content, char_idx);

                        // PowerCOBOL property reference: `"Prop" OF Widget`.
                        let prop_ref = if inside_plain_string {
                            None
                        } else {
                            detect_property_ref(line_to_cursor)
                        };

                        // Detect INVOKE … ' context → method completions
                        let invoke = if prop_ref.is_none() && !inside_plain_string {
                            detect_invoke_context(&tab.content, char_idx, &self.known_controls)
                        } else {
                            None
                        };

                        // Detect exact control ID → member (property+method) popup
                        let member_ctrl =
                            if invoke.is_none() && prop_ref.is_none() && !inside_plain_string {
                                detect_control_exact(&prefix, &self.known_controls)
                            } else {
                                None
                            };

                        // A word boundary ends the completion. Typing a space or
                        // deleting back to nothing leaves no prefix to complete,
                        // and whatever is still listed refers to a word the
                        // developer has already finished. Handled before the
                        // refresh guard below, which an empty prefix never passes.
                        let has_member_context =
                            invoke.is_some() || member_ctrl.is_some() || prop_ref.is_some();
                        if self.ac.visible && ac_context_ended(&prefix, has_member_context) {
                            self.ac.visible = false;
                            self.ac.member_mode = false;
                        }

                        let refresh = trigger_manual
                            || (te_out.response.changed() && prefix.len() >= 2)
                            || (te_out.response.changed() && invoke.is_some())
                            || (te_out.response.changed() && member_ctrl.is_some())
                            || (te_out.response.changed() && prop_ref.is_some());

                        if refresh
                            || (self.ac.visible && prefix.len() >= 1)
                            || (self.ac.visible && prop_ref.is_some())
                            // An open popup re-filters on EVERY edit, so a
                            // keystroke that matches nothing closes it on the
                            // spot instead of waiting for a prefix long enough
                            // to satisfy `refresh`.
                            || (self.ac.visible && te_out.response.changed())
                        {
                            let (items, member_mode) = if let Some(pc) = &prop_ref {
                                let v = match pc {
                                    PropRefCtx::PropertyName => {
                                        property_name_items(&self.known_controls, &prefix)
                                    }
                                    PropRefCtx::OfKeyword => of_qualifier_items(&prefix),
                                    PropRefCtx::ControlForProp { property } => {
                                        controls_with_property(
                                            &self.known_controls,
                                            property,
                                            &prefix,
                                        )
                                    }
                                };
                                (v, true)
                            } else if let Some((ctrl_id, ctrl_type, member_pfx)) = &invoke {
                                // `INVOKE ctrl '…'`, `ctrl::`, and `ctrl::"` all
                                // list the control's properties (green) + methods
                                // (light-blue), filtered by the typed prefix
                                // (spec 010 R10/R11).
                                if let Some(k) = self
                                    .known_controls
                                    .iter()
                                    .find(|k| k.id.eq_ignore_ascii_case(ctrl_id))
                                {
                                    (member_completions(k, member_pfx), true)
                                } else {
                                    (
                                        member_completions(
                                            &KnownControl {
                                                id: ctrl_id.clone(),
                                                ctrl_type: ctrl_type.clone(),
                                                properties: vec![],
                                                extra_methods: vec![],
                                            },
                                            member_pfx,
                                        ),
                                        true,
                                    )
                                }
                            } else if let Some((ctrl_id, ctrl_type, member_pfx)) = &member_ctrl {
                                // Exact control ID typed → show EVERY property + methods, using the specific instance's extra_methods.
                                if let Some(k) = self
                                    .known_controls
                                    .iter()
                                    .find(|k| k.id.eq_ignore_ascii_case(ctrl_id))
                                {
                                    (member_completions(k, member_pfx), true)
                                } else {
                                    (
                                        member_completions(
                                            &KnownControl {
                                                id: ctrl_id.clone(),
                                                ctrl_type: ctrl_type.clone(),
                                                properties: vec![],
                                                extra_methods: vec![],
                                            },
                                            member_pfx,
                                        ),
                                        true,
                                    )
                                }
                            } else if inside_plain_string {
                                (vec![], false)
                            } else {
                                (
                                    build_completions(
                                        &prefix,
                                        &tab.content,
                                        &self.known_controls,
                                        &self.known_data_items,
                                        self.context_only_completions,
                                    ),
                                    false,
                                )
                            };

                            if !items.is_empty() {
                                let ppos = {
                                    // Use galley-based exact cursor position when available
                                    let cr_rect = te_out.galley.pos_from_cursor(cr.primary);
                                    let raw_x = te_out.galley_pos.x + cr_rect.min.x;
                                    let raw_y = te_out.galley_pos.y + cr_rect.max.y + 4.0;
                                    let cursor_top_y = te_out.galley_pos.y + cr_rect.min.y;
                                    let scr = ctx.content_rect();
                                    let popup_h = 280.0_f32;
                                    let popup_w = 480.0_f32;
                                    // Clamp horizontally so popup stays on screen
                                    let x =
                                        raw_x.min(scr.max.x - popup_w - 8.0).max(scr.min.x + 4.0);
                                    // If popup would clip the bottom, show it above the cursor instead
                                    let y = if raw_y + popup_h > scr.max.y {
                                        (cursor_top_y - popup_h - 4.0).max(scr.min.y)
                                    } else {
                                        raw_y
                                    };
                                    Pos2::new(x, y)
                                };
                                if !self.ac.visible
                                    || self.ac.prefix != prefix
                                    || self.ac.member_mode != member_mode
                                {
                                    self.ac.selected = 0;
                                }
                                self.ac.visible = true;
                                self.ac.member_mode = member_mode;
                                self.ac.items = items;
                                self.ac.prefix = prefix.clone();
                                self.ac.trigger_pos = word_start;
                                self.ac.popup_pos = ppos;
                                self.ac.anchor = te_out.galley_pos;
                            } else {
                                // NOTHING matches what is being typed, so there is
                                // nothing to suggest. The old rule kept a non-member
                                // popup on screen for any prefix of two characters or
                                // more, which is exactly the case that matters: the
                                // developer types past the suggestion, the list stops
                                // matching, and stale entries stay up offering to
                                // insert a word that is no longer being written.
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
            let popup_pos = self.ac.popup_pos;
            let items = self.ac.items.clone();
            let selected = self.ac.selected;
            let member_mode = self.ac.member_mode;
            let scroll_sel = self.ac.scroll_to_sel;
            let mut clicked: Option<usize> = None;

            let area = egui::Area::new(self.ui_id("ac_popup"))
                .fixed_pos(popup_pos)
                .order(egui::Order::Tooltip)
                .interactable(true)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style())
                        .corner_radius(egui::CornerRadius::same(7))
                        .show(ui, |ui| {
                            ui.set_min_width(320.0);
                            ui.set_max_width(480.0);

                            if member_mode {
                                ui.label(
                                    egui::RichText::new("  Properties & Methods")
                                        .small()
                                        .color(Color32::from_gray(160)),
                                );
                                ui.separator();
                            }

                            ScrollArea::vertical()
                                .id_salt(self.ui_id("ac_list"))
                                .max_height(220.0)
                                .show(ui, |ui| {
                                    for (i, item) in items.iter().enumerate() {
                                        let is_sel = i == selected;
                                        let (badge, badge_col) = item.badge();

                                        let row_frame = if is_sel {
                                            egui::Frame::default()
                                                .fill(Color32::from_rgba_unmultiplied(
                                                    65, 115, 225, 170,
                                                ))
                                                .corner_radius(egui::CornerRadius::same(4))
                                        } else {
                                            egui::Frame::default()
                                        };

                                        let row_resp = row_frame.show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new(badge)
                                                        .monospace()
                                                        .size(10.0)
                                                        .color(badge_col),
                                                );
                                                ui.label(
                                                    egui::RichText::new(&item.label)
                                                        .monospace()
                                                        .size(12.0),
                                                );
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        ui.label(
                                                            egui::RichText::new(&item.detail)
                                                                .small()
                                                                .color(Color32::from_gray(145)),
                                                        );
                                                    },
                                                );
                                            });
                                        });

                                        // Frame responses only sense hover by default.
                                        // Use ui.interact() over the same rect with a
                                        // unique per-row ID to properly detect clicks.
                                        let click_resp = ui
                                            .interact(
                                                row_resp.response.rect,
                                                self.ui_id("ac_row").with(i),
                                                egui::Sense::click(),
                                            )
                                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                                        if click_resp.clicked() {
                                            clicked = Some(i);
                                        }
                                        // Keep the keyboard-selected row visible.
                                        if is_sel && scroll_sel {
                                            click_resp.scroll_to_me(Some(egui::Align::Center));
                                        }
                                    }
                                });

                            ui.separator();
                            ui.label(
                                egui::RichText::new(
                                    "↑↓ navigate   Tab/↵ insert   Esc dismiss   Ctrl+Space force",
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
                        let trigger_byte = tab
                            .content
                            .char_indices()
                            .nth(self.ac.trigger_pos)
                            .map(|(b, _)| b)
                            .unwrap_or(tab.content.len());
                        let end_byte = (trigger_byte + self.ac.prefix.len()).min(tab.content.len());
                        tab.content
                            .replace_range(trigger_byte..end_byte, &item.insert);
                        tab.dirty = true;
                        // Move cursor (char index) to end of inserted text, restore focus.
                        let new_pos = self.ac.trigger_pos + item.insert.chars().count();
                        if let Some(mut state) = egui::TextEdit::load_state(ctx, editor_id) {
                            let cc = egui::text::CCursor::new(new_pos);
                            state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::one(cc)));
                            state.store(ctx, editor_id);
                        }
                        ctx.memory_mut(|m| m.request_focus(editor_id));
                    }
                }
                self.ac.visible = false;
                self.ac.member_mode = false;
            }

            // Scroll request is one-shot.
            self.ac.scroll_to_sel = false;

            // Click anywhere outside the popup dismisses it.
            if area.response.clicked_elsewhere() {
                self.ac.visible = false;
                self.ac.member_mode = false;
            }
        }

        // ─── Find / Search bar (Cmd+F) ─────────────────────────────────────
        if self.search.visible && !self.tabs.is_empty() {
            // Anchor to the STABLE editor viewport (not the scrolling text
            // content) so the bar keeps its position while searching scrolls the
            // editor. `default_pos` + `movable` below lets the user drag it
            // elsewhere and egui remembers where they left it.
            let bar_w = 320.0_f32;
            let bar_x = (editor_viewport.max.x - bar_w - 8.0).max(editor_viewport.min.x);
            let bar_y = editor_viewport.min.y + 6.0;

            let prev_query = self.search.query.clone();
            let active_ro = self
                .tabs
                .get(self.active)
                .map(|t| t.read_only)
                .unwrap_or(false);
            let mut do_replace_one = false;
            let mut do_replace_all = false;
            let search_input_id = self.ui_id("search_input");
            let replace_input_id = self.ui_id("replace_input");

            egui::Area::new(self.ui_id("search_bar"))
                // `default_pos` (not `fixed_pos`): egui applies it only on first
                // appearance, then keeps the position the user dragged it to —
                // so it stays put during a search and is draggable.
                .default_pos(Pos2::new(bar_x, bar_y))
                .movable(true)
                .constrain(true)
                .order(egui::Order::Foreground)
                .interactable(true)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style())
                        .corner_radius(egui::CornerRadius::same(7))
                        .inner_margin(egui::Margin::same(6))
                        .show(ui, |ui| {
                            ui.set_min_width(bar_w - 12.0);
                            ui.horizontal(|ui| {
                                // Search icon label
                                ui.label(
                                    egui::RichText::new("🔍")
                                        .size(13.0)
                                        .color(Color32::from_gray(160)),
                                );

                                // Query text input
                                let te_resp = ui.add(
                                    TextEdit::singleline(&mut self.search.query)
                                        .id(search_input_id)
                                        .desired_width(165.0)
                                        .hint_text("Find…"),
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
                                    ("".to_owned(), crate::theme::active().text_dim)
                                } else if total == 0 {
                                    ("No matches".to_owned(), Color32::from_rgb(255, 100, 100))
                                } else {
                                    let cur = self.search.current + 1;
                                    (format!("{cur}/{total}"), crate::theme::active().text_bright)
                                };
                                ui.label(egui::RichText::new(count_txt).small().color(count_col));

                                ui.separator();

                                // Case-insensitive toggle (on by default).
                                if ui
                                    .selectable_label(self.search.case_insensitive, "Aa")
                                    .on_hover_text("Case-insensitive search")
                                    .clicked()
                                {
                                    self.search.case_insensitive = !self.search.case_insensitive;
                                    self.update_search_matches();
                                    self.search.needs_scroll = !self.search.matches.is_empty();
                                }

                                ui.separator();

                                // < previous match
                                if ui
                                    .small_button("<")
                                    .on_hover_text("Previous match (Shift+Enter)")
                                    .clicked()
                                {
                                    self.search_prev();
                                }
                                // > next match
                                if ui
                                    .small_button(">")
                                    .on_hover_text("Next match (Enter)")
                                    .clicked()
                                {
                                    self.search_next();
                                }
                                // ✕ close
                                if ui.small_button("✕").on_hover_text("Close (Esc)").clicked() {
                                    self.search.visible = false;
                                }
                            });

                            // ── Replace row ───────────────────────────────
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("⇄")
                                        .size(13.0)
                                        .color(Color32::from_gray(160)),
                                );
                                ui.add(
                                    TextEdit::singleline(&mut self.search.replace)
                                        .id(replace_input_id)
                                        .desired_width(165.0)
                                        .hint_text("Replace…"),
                                );
                                ui.separator();
                                let can = !active_ro
                                    && !self.search.query.is_empty()
                                    && !self.search.matches.is_empty();
                                if ui
                                    .add_enabled(can, egui::Button::new("Replace").small())
                                    .on_hover_text("Replace this match")
                                    .clicked()
                                {
                                    do_replace_one = true;
                                }
                                if ui
                                    .add_enabled(can, egui::Button::new("All").small())
                                    .on_hover_text("Replace all matches")
                                    .clicked()
                                {
                                    do_replace_all = true;
                                }
                            });

                            ui.label(
                                egui::RichText::new(
                                    "Enter = next   Shift+Enter = prev   Esc = close",
                                )
                                .small()
                                .color(Color32::from_gray(120)),
                            );
                        });
                });

            if do_replace_one {
                self.replace_current();
            }
            if do_replace_all {
                self.replace_all();
            }
        }
    }
}

// ── Status-bar / save helpers ──────────────────────────────────────────────────

/// Case-insensitive (ASCII) replace-all. COBOL source is ASCII, so we match on
/// ASCII-folded bytes and copy any non-matching UTF-8 char through verbatim.
fn replace_all_ci(haystack: &str, needle: &str, repl: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
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
    let mut col = 1usize;
    for (i, ch) in text.chars().enumerate() {
        if i >= char_idx {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
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
enum CobolDiv {
    Ident,
    Env,
    Data,
    Proc,
}

#[derive(Clone, Copy, PartialEq)]
enum CobolScope {
    If,
    Evaluate,
    When,
    Perform,
}

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
    let reserved: std::collections::HashSet<&'static str> = VERBS
        .iter()
        .chain(DIVISION_KEYWORDS.iter())
        .chain(DATA_KEYWORDS.iter())
        .chain(COBOL2002_KEYWORDS.iter())
        .copied()
        .collect();

    let mut out = String::with_capacity(text.len());
    let mut div = CobolDiv::Ident;
    let mut scopes: Vec<CobolScope> = Vec::new();
    let mut data_levels: Vec<u32> = Vec::new();
    let mut prev_blank = false;

    // Append `content` at 1-based `col` (col-1 leading spaces).
    fn put(out: &mut String, col: usize, content: &str) {
        for _ in 1..col {
            out.push(' ');
        }
        out.push_str(content);
        out.push('\n');
    }
    let word_at = |words: &[&str], i: usize| -> String {
        words
            .get(i)
            .map(|w| w.trim_end_matches('.').to_string())
            .unwrap_or_default()
    };

    for raw in text.lines() {
        let t = raw.trim();
        if t.is_empty() {
            if !prev_blank {
                out.push('\n');
                prev_blank = true;
            }
            continue;
        }
        prev_blank = false;

        // Full-line comment → indicator in column 7.
        if t.starts_with("*>") || t.starts_with('*') || t.starts_with('/') {
            put(&mut out, 7, t);
            continue;
        }

        let content = uppercase_reserved_words(&collapse_spaces_keep_pic(t), &reserved);
        let upper = content.to_ascii_uppercase();
        let words: Vec<&str> = upper.split_whitespace().collect();
        let first = words.first().copied().unwrap_or("");
        let ends_period = content.trim_end().ends_with('.');

        // Division header → Area A, and switch context.
        if word_at(&words, 1) == "DIVISION"
            && matches!(
                first,
                "IDENTIFICATION" | "ID" | "ENVIRONMENT" | "DATA" | "PROCEDURE"
            )
        {
            put(&mut out, 8, &content);
            div = match first {
                "PROCEDURE" => CobolDiv::Proc,
                "DATA" => CobolDiv::Data,
                "ENVIRONMENT" => CobolDiv::Env,
                _ => CobolDiv::Ident,
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
                if words.len() == 1
                    && first.ends_with('.')
                    && !reserved.contains(first.trim_end_matches('.'))
                {
                    put(&mut out, 8, &content);
                    scopes.clear();
                    continue;
                }

                // Dedent-before for terminators / case labels.
                if first.starts_with("END-") {
                    if first == "END-EVALUATE" && matches!(scopes.last(), Some(CobolScope::When)) {
                        scopes.pop(); // close a trailing WHEN body
                    }
                    scopes.pop();
                } else if first == "WHEN" && matches!(scopes.last(), Some(CobolScope::When)) {
                    scopes.pop(); // close the previous WHEN body
                }

                let mut level = scopes.len();
                if first == "ELSE" {
                    level = level.saturating_sub(1);
                }
                put(&mut out, 12 + level * 4, &content);

                // Indent-after for openers (only when the scope continues onto
                // following lines — no inline terminator and no closing period).
                if !ends_period {
                    match first {
                        "IF" if !upper.contains("END-IF") => scopes.push(CobolScope::If),
                        "EVALUATE" if !upper.contains("END-EVALUATE") => {
                            scopes.push(CobolScope::Evaluate)
                        }
                        "WHEN" => scopes.push(CobolScope::When),
                        "PERFORM"
                            if !upper.contains("END-PERFORM") && is_inline_perform(&words) =>
                        {
                            scopes.push(CobolScope::Perform)
                        }
                        _ => {}
                    }
                }
                // A period ends the sentence → all in-line scopes close.
                if ends_period {
                    scopes.clear();
                }
            }
            CobolDiv::Data => {
                if matches!(first, "FD" | "SD" | "RD" | "CD") {
                    data_levels.clear();
                    put(&mut out, 8, &content);
                } else if let Some(level) = cobol_leading_level(&content) {
                    let col = data_level_column(level, &mut data_levels);
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

    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn insert_auto_indented_newline(text: &mut String, range: egui::text::CCursorRange) -> usize {
    let start_char = range.primary.index.0.min(range.secondary.index.0);
    let end_char = range.primary.index.0.max(range.secondary.index.0);
    let start_byte = char_to_byte(text, start_char);
    let end_byte = char_to_byte(text, end_char);
    let indent = first_nonblank_column_indent(text, start_byte);
    let insertion = format!("\n{indent}");
    text.replace_range(start_byte..end_byte, &insertion);
    start_char + insertion.chars().count()
}

fn char_to_byte(text: &str, char_idx: usize) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(text.len())
}

fn first_nonblank_column_indent(text: &str, cursor_byte: usize) -> String {
    let line_start = text[..cursor_byte.min(text.len())]
        .rfind('\n')
        .map(|p| p + 1)
        .unwrap_or(0);
    let line = &text[line_start..cursor_byte.min(text.len())];
    line.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect()
}

fn data_level_column(level: u32, levels: &mut Vec<u32>) -> usize {
    if matches!(level, 1 | 77 | 78 | 88) {
        levels.clear();
        levels.push(level);
        return 8;
    }
    while levels.last().is_some_and(|prev| *prev >= level) {
        levels.pop();
    }
    let col = 8 + levels.len() * 3;
    levels.push(level);
    col
}

fn uppercase_reserved_words(s: &str, reserved: &std::collections::HashSet<&'static str>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut word = String::new();
    let mut quote: Option<char> = None;

    let flush_word = |out: &mut String, word: &mut String| {
        if word.is_empty() {
            return;
        }
        let trailing_period = word.ends_with('.');
        let core = if trailing_period {
            &word[..word.len() - 1]
        } else {
            word.as_str()
        };
        let upper = core.to_ascii_uppercase();
        if reserved.contains(upper.as_str()) {
            out.push_str(&upper);
            if trailing_period {
                out.push('.');
            }
        } else {
            out.push_str(word);
        }
        word.clear();
    };

    for c in s.chars() {
        if let Some(q) = quote {
            flush_word(&mut out, &mut word);
            out.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }
        if c == '"' || c == '\'' {
            flush_word(&mut out, &mut word);
            quote = Some(c);
            out.push(c);
        } else if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
            word.push(c);
        } else {
            flush_word(&mut out, &mut word);
            out.push(c);
        }
    }
    flush_word(&mut out, &mut word);
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
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
            out.push(c);
            i += 1;
            continue;
        }
        if c == ' ' {
            let start = i;
            while i < chars.len() && chars[i] == ' ' {
                i += 1;
            }
            let run = i - start;
            let rest: String = chars[i..].iter().collect();
            let next = rest.trim_start().to_ascii_uppercase();
            let before_pic = next.starts_with("PIC ") || next.starts_with("PICTURE");
            if before_pic && run > 1 {
                for _ in 0..run {
                    out.push(' ');
                } // keep alignment
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
        None => true, // bare PERFORM
        Some("UNTIL") | Some("VARYING") | Some("WITH") | Some("FOREVER") => true,
        _ => words
            .last()
            .map_or(false, |w| w.trim_end_matches('.') == "TIMES"),
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
        .char_indices()
        .rev()
        .find(|(_, c)| !c.is_alphanumeric() && *c != '-' && *c != '_')
        .map(|(p, c)| p + c.len_utf8())
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
) -> Option<(
    String, /*ctrl_id*/
    String, /*type*/
    String, /*member_pfx*/
)> {
    // Case 1: the currently typed word IS a known control ID exactly.
    if let Some(ctrl) = controls.iter().find(|c| c.id.eq_ignore_ascii_case(prefix)) {
        return Some((ctrl.id.clone(), ctrl.ctrl_type.clone(), String::new()));
    }
    None
}

/// Returns true if the cursor is inside a plain string literal ( "..." or '...' )
/// whose opening quote was **not** immediately preceded by `::` (or `:: ` etc).
/// The special `id::"method"` / `id::'method'` syntax is allowed for member completion.
/// Also returns true if the cursor is exactly at the closing quote of such a plain string.
fn is_inside_plain_string_literal(text: &str, cursor_char: usize) -> bool {
    let char_indices: Vec<(usize, char)> = text.char_indices().collect();
    if cursor_char > char_indices.len() {
        return false;
    }
    let cursor_byte = if cursor_char < char_indices.len() {
        char_indices[cursor_char].0
    } else {
        text.len()
    };
    let line_start = text[..cursor_byte].rfind('\n').map(|p| p + 1).unwrap_or(0);
    let line_up_to_cursor = &text[line_start..cursor_byte];

    // Scan to see if we are inside a string, tracking if the current open was special.
    let mut current_open_is_special = false;
    let mut in_plain_string = false;
    for (i, c) in line_up_to_cursor.char_indices() {
        if in_plain_string {
            // We are in a plain (we only enter for non-special)
            if c == '"' || c == '\'' {
                // closing a plain one
                in_plain_string = false;
                // if cursor is at or after this closer, consider it "at end quote of plain"
                if i >= cursor_byte - (if cursor_byte > 0 { 1 } else { 0 }) {
                    // rough for at the quote
                    return true;
                }
            }
        } else if c == '"' || c == '\'' {
            let before = &line_up_to_cursor[..i].trim_end();
            let is_special = before.ends_with("::");
            if !is_special {
                in_plain_string = true;
                current_open_is_special = false;
            }
            // else special, we allow member completion, do not set in_plain
        }
    }

    if in_plain_string {
        return true;
    }

    false
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

    // ── ctrl-id::  ·  ctrl-id::"  ·  chain tail `… )::` / `…::member::`
    //    (spec 010 R10/R11, extended for member chains in spec 011) ──────────
    if let Some(pos) = line.rfind("::") {
        let before = line[..pos].trim_end();
        // The whole chain expression is the last whitespace-delimited token, e.g.
        // `Grid::Rows(0)`. Its **root** is the identifier before the first `::`
        // (a leading subscript stripped), so chain-tail completion offers the
        // root control's members (deep element-type inference is out of scope).
        let chain_expr = before
            .rsplit(|c: char| c.is_whitespace())
            .next()
            .unwrap_or("");
        let root = chain_expr
            .split("::")
            .next()
            .unwrap_or("")
            .split('(')
            .next()
            .unwrap_or("");
        let ctrl_tok = root.to_ascii_uppercase();
        // The quoted form `ctrl::"member` opens the same list; the leading quote
        // is not part of the member being filtered, so strip it.
        let after = &line[pos + 2..];
        let mprefix = after.strip_prefix('"').unwrap_or(after);
        let ctrl_type = controls
            .iter()
            .find(|c| c.id.eq_ignore_ascii_case(&ctrl_tok))
            .map(|c| c.ctrl_type.clone())
            .unwrap_or_else(|| "Generic".into());
        return Some((ctrl_tok, ctrl_type, mprefix.into()));
    }

    None
}

/// Build the member-completion list for a specific control instance, filtered by `member_pfx`.
/// Includes type methods + any extra_methods on the instance (e.g. RefreshBinding for
/// databound array/repeating GroupBoxes).
fn member_completions(known: &KnownControl, member_pfx: &str) -> Vec<AcItem> {
    let up = member_pfx.to_ascii_uppercase();
    let mut v: Vec<AcItem> = known
        .properties
        .iter()
        .filter(|p| p.to_ascii_uppercase().starts_with(&up))
        .map(|p| AcItem::prop(p, "property"))
        .collect();
    for (m, d) in methods_for_type(&known.ctrl_type)
        .iter()
        .filter(|(m, _)| m.to_ascii_uppercase().starts_with(&up))
    {
        v.push(AcItem::method(m, d));
    }
    for m in &known.extra_methods {
        if m.to_ascii_uppercase().starts_with(&up) {
            let desc = if m == "RefreshBinding" {
                "Refresh / recreate databound array instances and re-apply effects"
            } else {
                "Method"
            };
            v.push(AcItem::method(m, desc));
        }
    }
    v
}

/// PowerCOBOL-style property reference context: `"Property" OF Widget`.
/// Deprecated in favour of `obj::prop` (spec 005); kept only so the (now inert)
/// completion arms still compile.
#[allow(dead_code)]
#[derive(Debug, PartialEq)]
enum PropRefCtx {
    /// A property name is being typed inside an open double quote.
    PropertyName,
    /// Just after `"Prop"` — offer / accept the `OF` qualifier.
    OfKeyword,
    /// After `"Prop" OF ` — offer widgets that expose `property`.
    ControlForProp { property: String },
}

/// Detect a property-reference context from the current line up to the cursor.
fn detect_property_ref(_line: &str) -> Option<PropRefCtx> {
    // Deprecated (spec 005): a double quote is a plain string literal now, not the
    // start of a PowerCOBOL `"Prop" OF Ctrl` reference. Property and method access
    // use `obj::prop` / `obj::method()` instead, handled by control-member
    // completion — so typing `"` never opens a property/method popup.
    None
}

/// All property names across the known controls (union), sorted + prefix-filtered.
fn property_name_items(controls: &[KnownControl], prefix: &str) -> Vec<AcItem> {
    let up = prefix.to_ascii_uppercase();
    let mut names: Vec<&str> = controls
        .iter()
        .flat_map(|c| c.properties.iter().map(|s| s.as_str()))
        .collect();
    names.sort_unstable();
    names.dedup();
    names
        .into_iter()
        .filter(|n| n.to_ascii_uppercase().starts_with(&up))
        .map(AcItem::property)
        .take(60)
        .collect()
}

/// The `OF` qualifier completion (shown while its prefix still matches).
fn of_qualifier_items(prefix: &str) -> Vec<AcItem> {
    if "OF".starts_with(&prefix.to_ascii_uppercase()) {
        vec![AcItem {
            label: "OF".into(),
            insert: "OF ".into(),
            detail: "qualifier".into(),
            kind: AcKind::Keyword,
        }]
    } else {
        Vec::new()
    }
}

/// Widgets that expose `property`, filtered by id prefix.
fn controls_with_property(controls: &[KnownControl], property: &str, prefix: &str) -> Vec<AcItem> {
    let up = prefix.to_ascii_uppercase();
    controls
        .iter()
        .filter(|c| {
            c.properties
                .iter()
                .any(|p| p.eq_ignore_ascii_case(property))
        })
        .filter(|c| c.id.to_ascii_uppercase().starts_with(&up))
        .map(|c| AcItem::ctrl(&c.id, &c.ctrl_type))
        .collect()
}

/// Build the completion list for a given prefix string.
fn build_completions(
    prefix: &str,
    source: &str,
    controls: &[KnownControl],
    global_data_items: &[String],
    context_only: bool,
) -> Vec<AcItem> {
    let up = prefix.to_ascii_uppercase();
    let mut seen: std::collections::HashSet<String> = Default::default();
    let mut items: Vec<AcItem> = Vec::new();

    // ── 1. COBOL keywords (insert the bare word + space, then await input) ──
    if !context_only {
        for &kw in VERBS
            .iter()
            .chain(DIVISION_KEYWORDS)
            .chain(DATA_KEYWORDS)
            .chain(COBOL2002_KEYWORDS)
        {
            if kw.starts_with(&up) && seen.insert(kw.into()) {
                items.push(AcItem::kw(kw));
            }
        }
    }

    // ── 3. Paragraph names ────────────────────────────────────────────────
    if !context_only {
        for p in extract_paragraphs(source) {
            if p.to_ascii_uppercase().starts_with(&up) && seen.insert(p.to_ascii_uppercase()) {
                items.push(AcItem::para(&p));
            }
        }
    }

    // ── 4. Data items (current buffer + form-level globals) ────────────────
    for d in extract_data_items(source)
        .iter()
        .cloned()
        .chain(global_data_items.iter().cloned())
    {
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

    // ── 6. Property names (prompt/context mode only; source mode offers these
    // through `control::` and PowerCOBOL property contexts).
    if context_only {
        for item in property_name_items(controls, prefix) {
            if seen.insert(item.label.to_ascii_uppercase()) {
                items.push(item);
            }
        }
    }

    items.truncate(25);
    items
}

fn extract_paragraphs(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in source.lines() {
        let t = line.trim();
        if t.starts_with("*>") || t.starts_with('*') {
            continue;
        }
        if t.ends_with("DIVISION.") || t.ends_with("SECTION.") {
            continue;
        }
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
        if upper.contains("WORKING-STORAGE")
            || upper.contains("LOCAL-STORAGE")
            || upper.contains("LINKAGE")
            || upper.contains("FILE SECTION")
        {
            in_data = true;
        }
        if upper.contains("PROCEDURE DIVISION") {
            in_data = false;
        }
        if !in_data {
            continue;
        }
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.len() >= 2 && parts[0].eq_ignore_ascii_case("FD") {
            let name = parts[1].trim_end_matches('.');
            if name != "FILLER"
                && name.chars().all(|c| c.is_alphanumeric() || c == '-')
                && name
                    .chars()
                    .next()
                    .map(|c| c.is_alphabetic())
                    .unwrap_or(false)
            {
                out.push(name.into());
            }
            continue;
        }
        if parts.len() >= 2 && parts[0].chars().all(|c| c.is_ascii_digit()) {
            let name = parts[1].trim_end_matches('.');
            if name != "FILLER"
                && name.chars().all(|c| c.is_alphanumeric() || c == '-')
                && name
                    .chars()
                    .next()
                    .map(|c| c.is_alphabetic())
                    .unwrap_or(false)
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
    let cursor_byte = text
        .char_indices()
        .nth(cursor_char)
        .map(|(b, _)| b)
        .unwrap_or(text.len());
    let before = &text[..cursor_byte];
    let lines: Vec<&str> = before.split('\n').collect();
    let line_num = lines.len().saturating_sub(1);
    let col_num = lines.last().map(|l| l.chars().count()).unwrap_or(0);
    let char_w = font_size * 0.601;
    let line_h = font_size * 1.45;
    let x = (editor_rect.min.x + col_num as f32 * char_w).max(editor_rect.min.x);
    let y = editor_rect.min.y + (line_num + 1) as f32 * line_h + 4.0;
    Pos2::new(x, y)
}

// ── Syntax highlighting ───────────────────────────────────────────────────────

pub fn cobol_layout_job(
    text: &str,
    font_id: FontId,
    kw_set: &std::collections::HashSet<&'static str>,
) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};

    // Syntax colours come from the active IDE theme (published once per frame).
    let th = crate::theme::active();
    let c_plain = th.ed_plain;
    let c_kw = th.ed_keyword;
    let c_data = th.ed_data;
    let c_para = th.ed_paragraph;
    let c_str = th.ed_string;
    let c_comment = th.ed_comment;

    let fmt = |c: Color32| TextFormat {
        font_id: font_id.clone(),
        color: c,
        ..Default::default()
    };

    let mut job = LayoutJob::default();
    for (li, line) in text.split('\n').enumerate() {
        if li > 0 {
            job.append("\n", 0.0, fmt(c_plain));
        }
        cobol_highlight_line(
            &mut job, line, kw_set, &fmt, c_plain, c_kw, c_data, c_para, c_str, c_comment,
        );
    }
    job
}

/// Lay out `text` in a single flat colour (used for read-only generated code).
pub fn mono_layout_job(text: &str, font_id: FontId, color: Color32) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};
    let mut job = LayoutJob::default();
    job.append(
        text,
        0.0,
        TextFormat {
            font_id,
            color,
            ..Default::default()
        },
    );
    job
}

pub fn highlight_cobol(text: &str) -> egui::text::LayoutJob {
    let kw: std::collections::HashSet<&'static str> = VERBS
        .iter()
        .chain(DIVISION_KEYWORDS.iter())
        .chain(DATA_KEYWORDS.iter())
        .chain(COBOL2002_KEYWORDS.iter())
        .copied()
        .collect();
    cobol_layout_job(text, FontId::monospace(EDITOR_FONT_SIZE), &kw)
}

#[allow(clippy::too_many_arguments)]
fn cobol_highlight_line(
    job: &mut egui::text::LayoutJob,
    line: &str,
    kw_set: &std::collections::HashSet<&'static str>,
    fmt: &impl Fn(Color32) -> egui::text::TextFormat,
    c_plain: Color32,
    c_kw: Color32,
    c_data: Color32,
    c_para: Color32,
    c_str: Color32,
    c_comment: Color32,
) {
    if line
        .chars()
        .nth(6)
        .map(|c| c == '*' || c == '/')
        .unwrap_or(false)
    {
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
        let fe = trimmed
            .find(|c: char| c.is_whitespace())
            .unwrap_or(trimmed.len());
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
        let fe = trimmed
            .find(|c: char| c.is_whitespace())
            .unwrap_or(trimmed.len());
        let fw = &trimmed[..fe];
        if !fw.is_empty() && !fw.starts_with(|c: char| c.is_ascii_digit()) {
            let rest = trimmed[fe..].trim_start().trim_end_matches('.');
            let fw_upper = fw.trim_end_matches('.').to_ascii_uppercase();
            if !kw_set.contains(fw_upper.as_str())
                && rest.is_empty()
                && fw
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
            {
                first_is_para = true;
            }
        }
    }

    let bytes = line.as_bytes();
    let n = line.len();
    let mut i = 0usize;
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
                    seg = i;
                    in_str = None;
                }
            } else {
                i += line[i..].chars().next().map_or(1, |c| c.len_utf8());
            }
            continue;
        }

        if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'>') {
            if i > seg {
                emit_word(
                    job,
                    &line[seg..i],
                    tok_num,
                    next_is_data,
                    first_is_para,
                    kw_set,
                    fmt,
                    c_plain,
                    c_kw,
                    c_data,
                    c_para,
                );
            }
            job.append(&line[i..], 0.0, fmt(c_comment));
            return;
        }

        if bytes[i] == b'"' || bytes[i] == b'\'' {
            if i > seg {
                emit_word(
                    job,
                    &line[seg..i],
                    tok_num,
                    next_is_data,
                    first_is_para,
                    kw_set,
                    fmt,
                    c_plain,
                    c_kw,
                    c_data,
                    c_para,
                );
            }
            seg = i;
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
                    emit_word(
                        job,
                        word,
                        tok_num,
                        next_is_data,
                        first_is_para,
                        kw_set,
                        fmt,
                        c_plain,
                        c_kw,
                        c_data,
                        c_para,
                    );
                    tok_num += 1;
                } else {
                    job.append(word, 0.0, fmt(c_plain));
                }
            }
            let end = i + ch.len_utf8();
            job.append(&line[i..end], 0.0, fmt(c_plain));
            seg = end;
            i = end;
        }
    }

    if seg < n {
        if in_str.is_some() {
            job.append(&line[seg..], 0.0, fmt(c_str));
        } else {
            let word = &line[seg..];
            if word.chars().any(|c| c.is_alphanumeric()) {
                emit_word(
                    job,
                    word,
                    tok_num,
                    next_is_data,
                    first_is_para,
                    kw_set,
                    fmt,
                    c_plain,
                    c_kw,
                    c_data,
                    c_para,
                );
            } else {
                job.append(word, 0.0, fmt(c_plain));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn emit_word(
    job: &mut egui::text::LayoutJob,
    word: &str,
    tok_num: usize,
    next_is_data: bool,
    first_is_para: bool,
    kw_set: &std::collections::HashSet<&'static str>,
    fmt: &impl Fn(Color32) -> egui::text::TextFormat,
    c_plain: Color32,
    c_kw: Color32,
    c_data: Color32,
    c_para: Color32,
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
        ed.tabs.push(EditorTab::new(
            PathBuf::from("main.cbl"),
            content.to_owned(),
        ));
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
        assert!(
            SRC[off..]
                .to_ascii_uppercase()
                .starts_with("BTN-OK--CLICK."),
            "expected to land on the header, got: {:?}",
            &SRC[off..off + 20]
        );
    }

    #[test]
    fn jumps_to_program_id() {
        let mut ed = editor_with(SRC);
        assert!(ed.goto_paragraph("main"));
        let off = ed.search.matches[0];
        // Lands on the PROGRAM-ID line (its definition), case-insensitively.
        assert!(SRC[off..]
            .to_ascii_uppercase()
            .starts_with("PROGRAM-ID. MAIN."));
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
        assert_eq!(char_index_to_line_col(t, 2), (1, 3)); // before the \n
        assert_eq!(char_index_to_line_col(t, 3), (2, 1)); // start of line 2
        assert_eq!(char_index_to_line_col(t, 7), (3, 1)); // 'F'
    }

    #[test]
    fn word_before_cursor_handles_multibyte_separator() {
        assert_eq!(word_before_cursor("a˜", 2), (2, String::new()));
        assert_eq!(word_before_cursor("a˜BTN-1", 7), (2, "BTN-1".to_owned()));
    }

    #[test]
    fn trim_trailing_ws_preserves_lines() {
        let s = "AB  \n  CD\t\nEF\n";
        assert_eq!(trim_trailing_ws(s), "AB\n  CD\nEF\n");
        assert_eq!(trim_trailing_ws("no newline   "), "no newline");
    }

    #[test]
    fn markdown_extensions_are_identified_case_insensitively() {
        for path in ["README.md", "guide.MD", "notes.markdown"] {
            assert!(EditorTab::new(PathBuf::from(path), String::new()).is_markdown());
        }
        assert!(!EditorTab::new(PathBuf::from("main.cbl"), String::new()).is_markdown());
    }

    #[test]
    fn beautify_active_leaves_markdown_untouched() {
        let content = "# Heading\n\n\n-  Preserve markdown spacing\n";
        let mut editor = EditorPanel::new();
        editor.tabs.push(EditorTab::new(
            PathBuf::from("Knowledge Base/README.md"),
            content.into(),
        ));

        editor.beautify_active();

        assert_eq!(editor.tabs[0].content, content);
        assert!(!editor.tabs[0].dirty);
    }

    #[test]
    fn beautify_indents_to_cobol_columns() {
        let input = "\
ENVIRONMENT DIVISION.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-X PIC 9(4).
05 WS-NAME PIC X(20).
PROCEDURE DIVISION.
MOVE 1 TO WS-X
IF WS-X > 0
DISPLAY \"POS\"
END-IF
*> trailing note
";
        let out = beautify_cobol(input);
        // Area A (col 8 = 7 spaces): divisions, sections, 01 items.
        assert!(
            out.starts_with("       ENVIRONMENT DIVISION.\n"),
            "got: {out:?}"
        );
        assert!(out.contains("\n       WORKING-STORAGE SECTION.\n"));
        assert!(out.contains("\n       01 WS-X PIC 9(4).\n"));
        assert!(out.contains("\n          05 WS-NAME PIC X(20).\n"));
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
        assert_eq!(
            collapse_spaces_keep_pic("MOVE    1   TO   WS-X"),
            "MOVE 1 TO WS-X"
        );
        // Spaces inside a string literal are untouched.
        assert_eq!(
            collapse_spaces_keep_pic("DISPLAY \"a    b\""),
            "DISPLAY \"a    b\""
        );
    }

    #[test]
    fn beautify_uppercases_reserved_words_outside_quotes() {
        let out = beautify_cobol(
            "\
identification division.
program-id. demo.
procedure division.
display \"move stays lower\".
move 1 to ws-x.
",
        );
        assert!(out.starts_with("       IDENTIFICATION DIVISION.\n"));
        assert!(out.contains("\n       PROGRAM-ID. demo.\n"));
        assert!(out.contains("\n           DISPLAY \"move stays lower\".\n"));
        assert!(out.contains("\n           MOVE 1 TO ws-x.\n"));
    }

    #[test]
    fn beautify_data_levels_use_area_a_and_three_space_nesting() {
        let out = beautify_cobol(
            "\
DATA DIVISION.
WORKING-STORAGE SECTION.
01 customer-record.
05 customer-name pic x(50).
10 customer-first pic x(25).
05 customer-flags.
88 customer-active value \"Y\".
77 standalone pic 9.
78 max-count value 10.
",
        );
        assert!(out.contains("\n       01 customer-record.\n"));
        assert!(out.contains("\n          05 customer-name PIC x(50).\n"));
        assert!(out.contains("\n             10 customer-first PIC x(25).\n"));
        assert!(out.contains("\n          05 customer-flags.\n"));
        assert!(out.contains("\n       88 customer-active VALUE \"Y\".\n"));
        assert!(out.contains("\n       77 standalone PIC 9.\n"));
        assert!(out.contains("\n       78 max-count VALUE 10.\n"));
    }

    #[test]
    fn auto_indent_uses_previous_line_first_character_column() {
        let mut text = "       PROCEDURE DIVISION.\n           DISPLAY \"A\"".to_string();
        let pos = text.chars().count();
        let new_pos = insert_auto_indented_newline(
            &mut text,
            egui::text::CCursorRange::one(egui::text::CCursor::new(pos)),
        );
        assert_eq!(
            text,
            "       PROCEDURE DIVISION.\n           DISPLAY \"A\"\n           "
        );
        assert_eq!(new_pos, text.chars().count());
    }

    #[test]
    fn string_literal_detection_suppresses_plain_quote_completion() {
        let line = "           DISPLAY \"Button-1";
        assert!(is_inside_plain_string_literal(line, line.chars().count()));
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
        assert!(out.contains("\n           EVALUATE WS-X\n")); // col 12
        assert!(out.contains("\n               WHEN 1\n")); // col 16
        assert!(out.contains("\n                   MOVE A TO B\n")); // col 20
        assert!(out.contains("\n               WHEN OTHER\n")); // col 16
        assert!(out.contains("\n           END-EVALUATE\n")); // col 12
    }

    #[test]
    fn keyword_inserts_word_and_space_not_template() {
        assert_eq!(AcItem::kw("DISPLAY").insert, "DISPLAY ");
        assert_eq!(AcItem::kw("MOVE").insert, "MOVE ");
        // property completion closes the opening quote
        assert_eq!(AcItem::property("Caption").insert, "Caption\"");
    }

    #[test]
    fn context_only_completion_excludes_reserved_words_and_paragraphs() {
        let controls = vec![KnownControl {
            id: "LineChart-1".to_string(),
            ctrl_type: "LineChart".to_string(),
            properties: vec!["ShadowEnabled".to_string(), "Caption".to_string()],
            extra_methods: vec![],
        }];
        let source = "\
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-P-VALUE PIC 9(4).
       PROCEDURE DIVISION.
       DISPLAY-PARAGRAPH.
           DISPLAY WS-P-VALUE.
";
        let items = build_completions("D", source, &controls, &[], true);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(
            !labels.contains(&"DISPLAY"),
            "prompt IntelliSense should not show COBOL reserved words"
        );
        assert!(
            !labels.contains(&"DISPLAY-PARAGRAPH"),
            "prompt IntelliSense should not show paragraph labels"
        );

        let items = build_completions("S", source, &controls, &[], true);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"ShadowEnabled"));

        let items = build_completions("Line", source, &controls, &[], true);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"LineChart-1"));
    }

    #[test]
    fn context_only_completion_includes_fd_and_data_items() {
        let source = "\
       DATA DIVISION.
       FILE SECTION.
       FD CUSTOMER-FILE.
       01 CUSTOMER-REC.
          05 CUSTOMER-NAME PIC X(30).
       WORKING-STORAGE SECTION.
       01 WS-TOTAL PIC 9(5).
       PROCEDURE DIVISION.
";
        let items = build_completions("C", source, &[], &["GLOBAL-CUSTOMER".to_string()], true);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"CUSTOMER-FILE"));
        assert!(labels.contains(&"CUSTOMER-REC"));
        assert!(labels.contains(&"CUSTOMER-NAME"));

        let items = build_completions("G", source, &[], &["GLOBAL-CUSTOMER".to_string()], true);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"GLOBAL-CUSTOMER"));
    }

    #[test]
    fn double_quote_is_a_literal_not_a_property_ref() {
        // Spec 005: a double quote starts a string literal, never a PowerCOBOL
        // `"Prop" OF Ctrl` property reference — so no property/method popup.
        assert_eq!(detect_property_ref("           MOVE \""), None);
        assert_eq!(detect_property_ref("           MOVE \"Captio"), None);
        assert_eq!(detect_property_ref("           MOVE \"Caption\" OF "), None);
        assert_eq!(
            detect_property_ref("           DISPLAY \"hello world"),
            None
        );
    }

    #[test]
    fn property_and_widget_completions() {
        let controls = vec![
            KnownControl {
                id: "Button-1".into(),
                ctrl_type: "Button".into(),
                properties: vec!["Caption".into(), "FontSize".into()],
                extra_methods: vec![],
            },
            KnownControl {
                id: "Label-1".into(),
                ctrl_type: "Label".into(),
                properties: vec!["Text".into(), "FontSize".into()],
                extra_methods: vec![],
            },
            KnownControl {
                id: "Button-2".into(),
                ctrl_type: "Button".into(),
                properties: vec!["Caption".into()],
                extra_methods: vec![],
            },
        ];
        // union, sorted, deduped, prefix-filtered
        let all: Vec<String> = property_name_items(&controls, "")
            .into_iter()
            .map(|i| i.label)
            .collect();
        assert_eq!(all, vec!["Caption", "FontSize", "Text"]);
        let cap: Vec<String> = property_name_items(&controls, "Capt")
            .into_iter()
            .map(|i| i.label)
            .collect();
        assert_eq!(cap, vec!["Caption"]);
        // widgets exposing Caption, filtered by "Bu"
        let w: Vec<String> = controls_with_property(&controls, "Caption", "Bu")
            .into_iter()
            .map(|i| i.label)
            .collect();
        assert_eq!(w, vec!["Button-1", "Button-2"]);
        // OF qualifier appears only while its prefix matches
        assert_eq!(of_qualifier_items("").len(), 1);
        assert_eq!(of_qualifier_items("O").len(), 1);
        assert_eq!(of_qualifier_items("X").len(), 0);
    }

    #[test]
    fn member_completion_triggers_on_colons_and_quote_010() {
        // Spec 010 R10/R11/R12: `::` and `::"` open the member list and filter on
        // subsequent non-`"` characters; a lone `"` opens no popup.
        let controls = vec![KnownControl {
            id: "Button-1".into(),
            ctrl_type: "Button".into(),
            properties: vec!["Caption".into()],
            extra_methods: vec![],
        }];
        let ctx = |line: &str| {
            let n = line.chars().count();
            detect_invoke_context(line, n, &controls)
        };
        // `::` → list, empty filter, control type resolved
        let (_id, ty, pre) = ctx("           DISPLAY Button-1::").expect("`::` should trigger");
        assert_eq!((ty.as_str(), pre.as_str()), ("Button", ""));
        // `::Cap` → filter "Cap"
        assert_eq!(ctx("           DISPLAY Button-1::Cap").unwrap().2, "Cap");
        // `::"` → list, quote stripped from the filter
        assert_eq!(ctx("           DISPLAY Button-1::\"").unwrap().2, "");
        // `::"Cap` → filter "Cap"
        assert_eq!(ctx("           DISPLAY Button-1::\"Cap").unwrap().2, "Cap");
        // A lone double quote (no `::`) opens no property/method popup.
        assert_eq!(detect_property_ref("           DISPLAY \""), None);
    }

    #[test]
    fn member_chain_tail_resolves_root_control_011() {
        // Spec 011: a chain prefix (`Grid::Rows(0)::`) still triggers the member
        // list, resolved against the chain's ROOT control type.
        let controls = vec![KnownControl {
            id: "Grid".into(),
            ctrl_type: "DataGrid".into(),
            properties: vec!["Rows".into()],
            extra_methods: vec![],
        }];
        let ctx = |line: &str| {
            let n = line.chars().count();
            detect_invoke_context(line, n, &controls)
        };
        let (_id, ty, pre) =
            ctx("           DISPLAY Grid::Rows(0)::").expect("chain `::` triggers");
        assert_eq!((ty.as_str(), pre.as_str()), ("DataGrid", ""));
        // Filtering still applies on the chain tail.
        assert_eq!(
            ctx("           DISPLAY Grid::Rows(0)::Val").unwrap().2,
            "Val"
        );
    }

    #[test]
    fn member_completions_list_both_properties_and_methods_010() {
        // Spec 010 R10/R11: the `::` / `::"` (and INVOKE) member list must contain
        // BOTH properties (green ●) and methods (light-blue M) — the regression was
        // a list that showed only methods.
        let dummy = KnownControl {
            id: "s".into(),
            ctrl_type: "Slider".into(),
            properties: cobolt_forms::model::property_names_for("Slider"),
            extra_methods: vec![],
        };
        let items = member_completions(&dummy, "");
        assert!(
            items.iter().any(|i| i.kind == AcKind::Property),
            "member list must contain property items: {:?}",
            items.iter().map(|i| i.label.clone()).collect::<Vec<_>>()
        );
        assert!(
            items.iter().any(|i| i.kind == AcKind::Method),
            "member list must contain method items"
        );
        // Universal geometry/visibility methods are always present.
        assert!(items
            .iter()
            .any(|i| i.label == "Show" && i.kind == AcKind::Method));
        // Field-backed properties are always present (Visible/Enabled/X/Y/…).
        assert!(items
            .iter()
            .any(|i| i.label == "Visible" && i.kind == AcKind::Property));
        // The prefix filters both kinds case-insensitively.
        let dummy = KnownControl {
            id: "s".into(),
            ctrl_type: "Slider".into(),
            properties: cobolt_forms::model::property_names_for("Slider"),
            extra_methods: vec![],
        };
        let v = member_completions(&dummy, "vis");
        assert!(v
            .iter()
            .all(|i| i.label.to_ascii_uppercase().starts_with("VIS")));
        assert!(v.iter().any(|i| i.label == "Visible"));
    }

    #[test]
    fn methods_for_type_universal_and_specific_coverage() {
        for t in [
            "Button", "BarChart", "TreeView", "Label", "PieChart", "DataGrid",
        ] {
            let m: Vec<&str> = methods_for_type(t).iter().map(|(n, _)| *n).collect();
            assert!(m.contains(&"MoveTo"), "{t} missing MoveTo");
            assert!(
                m.contains(&"Show") && m.contains(&"Hide"),
                "{t} missing Show/Hide"
            );
            assert!(m.contains(&"PlayAnimation"), "{t} missing PlayAnimation");
            assert!(m.contains(&"Validate"), "{t} missing Validate");
        }
        // Charts expose data binding.
        assert!(methods_for_type("BarChart")
            .iter()
            .any(|(n, _)| *n == "SetData"));
        let grid: Vec<&str> = methods_for_type("DataGrid")
            .iter()
            .map(|(n, _)| *n)
            .collect();
        for expected in [
            "RefreshBinding",
            "SetFilter",
            "ClearFilters",
            "FreezeColumns",
            "FreezeRows",
            "SetRowHeight",
            "SetColumnWidth",
            "GetSelectedText",
            "CopySelection",
            "ExportCSV",
        ] {
            assert!(grid.contains(&expected), "DataGrid missing {expected}");
        }
        // Non-visual widgets: no geometry methods, but specific + generic.
        let timer: Vec<&str> = methods_for_type("Timer").iter().map(|(n, _)| *n).collect();
        assert!(!timer.contains(&"MoveTo"));
        assert!(timer.contains(&"Start") && timer.contains(&"SetProperty"));
        let sql: Vec<&str> = methods_for_type("SqlDatabase")
            .iter()
            .map(|(n, _)| *n)
            .collect();
        assert!(sql.contains(&"query") && sql.contains(&"fetchAll"));
    }

    #[test]
    fn replace_all_ci_is_case_insensitive() {
        assert_eq!(
            replace_all_ci("Move move MOVE", "move", "ADD"),
            "ADD ADD ADD"
        );
        assert_eq!(replace_all_ci("abc", "x", "y"), "abc");
        // Non-matching UTF-8 passes through untouched.
        assert_eq!(replace_all_ci("café move", "MOVE", "ADD"), "café ADD");
    }
}
