// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Typed DAP bodies for every command PowerRustCOBOL implements.
//!
//! Field names are DAP's, in `camelCase`. Where COBOL needs to say something
//! DAP has no word for — a PICTURE, a level number, a qualified name, whether a
//! line is generated — it goes in an **extra field** rather than a repurposed
//! one. A stock DAP client ignores fields it does not know, so the link stays
//! wire-compatible while still carrying COBOL semantics for our own client.
//! Repurposing `Variable::type` to hold a PICTURE would have been the opposite
//! trade: compatible-looking and wrong.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── initialize / capabilities ─────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeArguments {
    #[serde(default, rename = "clientID", skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_name: Option<String>,
    #[serde(default, rename = "adapterID")]
    pub adapter_id: String,
    /// DAP's own flags: are source lines 1-based, are columns 1-based. COBOL is
    /// a 1-based, column-significant language and our adapter answers in those
    /// terms; a client asking for 0-based gets told so in the response.
    #[serde(default)]
    pub lines_start_at1: Option<bool>,
    #[serde(default)]
    pub columns_start_at1: Option<bool>,
    /// PowerRustCOBOL extension: the protocol revision the client speaks. See
    /// [`PROTOCOL_VERSION`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_protocol_version: Option<u32>,
}

/// The PowerRustCOBOL debug-protocol revision.
///
/// DAP itself has no version negotiation beyond capabilities, but the *COBOL*
/// payloads (statement ids, sidecar shape, snapshot format) do change, and a
/// debuggee built by an older toolchain must be recognised as such rather than
/// misread. Bump on any breaking change to a COBOL extension field.
pub const PROTOCOL_VERSION: u32 = 1;

/// What this adapter can do. Everything the UI gates on is here — the spec's
/// rule is that an action the session cannot perform is *visibly unavailable*,
/// never silently accepted, and this struct is the single source for that.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    #[serde(default)]
    pub supports_configuration_done_request: bool,
    #[serde(default)]
    pub supports_conditional_breakpoints: bool,
    #[serde(default)]
    pub supports_hit_conditional_breakpoints: bool,
    #[serde(default)]
    pub supports_log_points: bool,
    #[serde(default)]
    pub supports_function_breakpoints: bool,
    #[serde(default)]
    pub supports_data_breakpoints: bool,
    #[serde(default)]
    pub supports_set_variable: bool,
    #[serde(default)]
    pub supports_evaluate_for_hovers: bool,
    #[serde(default)]
    pub supports_restart_request: bool,
    #[serde(default)]
    pub supports_terminate_request: bool,
    #[serde(default)]
    pub supports_value_formatting_options: bool,
    /// Run to Cursor. DAP spells it `gotoTargets` + `goto`; we implement it as a
    /// one-shot breakpoint, and report it under DAP's name so a stock client
    /// understands.
    #[serde(default)]
    pub supports_goto_targets_request: bool,
    #[serde(default)]
    pub supports_read_memory_request: bool,
    #[serde(default)]
    pub supports_write_memory_request: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exception_breakpoint_filters: Vec<ExceptionBreakpointsFilter>,
    /// PowerRustCOBOL extension: the revision the adapter speaks.
    #[serde(default)]
    pub cobol_protocol_version: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionBreakpointsFilter {
    pub filter: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub supports_condition: bool,
}

// ── source and breakpoints ────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// PowerRustCOBOL extension: this file is IDE-generated scaffolding, not
    /// something the developer wrote. Drives "Only my code".
    #[serde(default)]
    pub cobol_generated: bool,
}

impl Source {
    pub fn path(path: impl Into<String>) -> Self {
        let path = path.into();
        let name = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned());
        Self {
            name,
            path: Some(path),
            cobol_generated: false,
        }
    }
}

/// A breakpoint as the client *asks* for it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceBreakpoint {
    pub line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    /// A COBOL condition, evaluated in the frame that would stop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// DAP's hit condition: `5`, `>= 5`, `% 3`. Parsed by [`crate::hits`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_condition: Option<String>,
    /// A logpoint: interpolate `{expr}` and emit it, without stopping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_message: Option<String>,
    /// PowerRustCOBOL extension: remove this breakpoint after it fires once.
    #[serde(default)]
    pub cobol_temporary: bool,
}

/// A breakpoint on a named COBOL unit: a paragraph, a section, a program, or an
/// event handler. DAP's `functionBreakpoint` is the nearest standard concept.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionBreakpoint {
    /// The COBOL name, optionally qualified `PARA OF SECTION`.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_condition: Option<String>,
    /// PowerRustCOBOL extension: which kind of unit the name refers to. Absent
    /// means "resolve it — paragraph, then section, then program".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_unit: Option<CobolUnit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CobolUnit {
    Paragraph,
    Section,
    Program,
    EventHandler,
}

/// A breakpoint as the adapter *resolved* it. `verified` false means the
/// adapter accepted the request but could not bind it — the UI shows a hollow
/// marker and the `message` says why.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Breakpoint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetBreakpointsArguments {
    pub source: Source,
    #[serde(default)]
    pub breakpoints: Vec<SourceBreakpoint>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetBreakpointsResponse {
    pub breakpoints: Vec<Breakpoint>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetFunctionBreakpointsArguments {
    #[serde(default)]
    pub breakpoints: Vec<FunctionBreakpoint>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetExceptionBreakpointsArguments {
    #[serde(default)]
    pub filters: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataBreakpoint {
    /// The id from `dataBreakpointInfo` — for us, a data item's canonical
    /// storage key.
    pub data_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_type: Option<DataAccess>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_condition: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DataAccess {
    Read,
    Write,
    ReadWrite,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDataBreakpointsArguments {
    #[serde(default)]
    pub breakpoints: Vec<DataBreakpoint>,
}

// ── threads, stack, scopes, variables ─────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Thread {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ThreadsResponse {
    pub threads: Vec<Thread>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackTraceArguments {
    pub thread_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_frame: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<u32>,
}

/// One frame of the **logical COBOL** stack: a PERFORM range, a CALL, or the
/// event handler an interaction entered through. It is not a Rust frame — the
/// runtime is a tree-walking interpreter, so its own call stack says nothing
/// about where the COBOL program is.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackFrame {
    pub id: i64,
    /// What the developer reads: `BUTTON-1--ONCLICK · VALIDATE-INPUT`.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    pub line: u32,
    #[serde(default)]
    pub column: u32,
    /// DAP's hint. `subtle` renders a frame greyed; `label` makes it a
    /// non-selectable group header. Runtime and generated frames use these so
    /// "Only my code" can *fold* them rather than lie about their absence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<String>,
    /// PowerRustCOBOL extension: the program this frame belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_program: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_section: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_paragraph: Option<String>,
    /// How this frame was entered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_frame_kind: Option<FrameKind>,
    /// True when the frame sits in IDE-generated scaffolding.
    #[serde(default)]
    pub cobol_generated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FrameKind {
    /// The program's own PROCEDURE DIVISION body.
    Program,
    /// Entered by PERFORM.
    Perform,
    /// Entered by CALL.
    Call,
    /// Entered because a form event fired.
    EventHandler,
    /// A DECLARATIVES section running for a condition.
    Declarative,
    /// A SORT/MERGE INPUT or OUTPUT PROCEDURE.
    SortProcedure,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackTraceResponse {
    pub stack_frames: Vec<StackFrame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_frames: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopesArguments {
    pub frame_id: i64,
}

/// A collapsible group in the data inspector. The COBOL sections are scopes:
/// LINKAGE, WORKING-STORAGE, LOCAL-STORAGE, FILE SECTION, plus the special
/// registers and the screen state.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
    pub name: String,
    pub variables_reference: i64,
    /// DAP asks an adapter to say when a scope is costly, so a client can leave
    /// it collapsed. A FILE SECTION scope reads record buffers; WORKING-STORAGE
    /// of a large program is thousands of items.
    #[serde(default)]
    pub expensive: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub named_variables: Option<u32>,
    /// PowerRustCOBOL extension: which COBOL scope this is, for stable ordering
    /// and iconography independent of the display name's language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_scope: Option<CobolScope>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CobolScope {
    /// USING parameters of the running program — shown first, like arguments.
    Arguments,
    LocalStorage,
    WorkingStorage,
    Linkage,
    FileSection,
    ScreenState,
    /// RETURN-CODE, TALLY, the file-status items, WHEN-COMPILED …
    SpecialRegisters,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopesResponse {
    pub scopes: Vec<Scope>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariablesArguments {
    pub variables_reference: i64,
    /// Paging, for an OCCURS of a million.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
}

/// How a value should be read. A COBOL item is bytes; what those bytes *mean*
/// depends on the viewer the developer picked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ValueView {
    /// The item's own category, formatted as COBOL would DISPLAY it.
    #[default]
    Cobol,
    /// Text with whitespace made visible.
    Text,
    /// Raw storage bytes in hex.
    Hex,
    /// Numeric interpretation, including packed-decimal digits and sign.
    Numeric,
    Json,
    Xml,
    DateTime,
}

/// One row of the data inspector.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub name: String,
    pub value: String,
    /// DAP's type column. We put the COBOL *category* here (`alphanumeric`,
    /// `packed-decimal`, `group`), and the PICTURE in `cobol_picture`.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    /// Non-zero when this row has children to fetch.
    #[serde(default)]
    pub variables_reference: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub named_variables: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexed_variables: Option<u32>,
    /// The expression that re-evaluates to this item — its qualified name. What
    /// "Add to watches" copies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluate_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<VariablePresentationHint>,
    // ── PowerRustCOBOL extensions ─────────────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_level: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_picture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_usage: Option<String>,
    /// This item REDEFINES that one — the same bytes, read another way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_redefines: Option<String>,
    /// OCCURS bound, and the ODO controlling item when the bound is variable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_occurs: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_occurs_depending_on: Option<String>,
    /// Storage length in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_length: Option<u32>,
    /// How the value is being shown, and which other views are available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_view: Option<ValueView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cobol_available_views: Vec<ValueView>,
    /// A value that is not simply text. The inspector must never render
    /// LOW-VALUES and an empty string the same way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_special: Option<SpecialValue>,
    /// This item changed at the last stop — the restrained "recently changed"
    /// marker.
    #[serde(default)]
    pub cobol_changed: bool,
    /// May the developer edit it while paused? A group with a REDEFINES overlay
    /// or an unavailable LINKAGE item may not.
    #[serde(default)]
    pub cobol_editable: bool,
}

/// The states a COBOL value can be in that are *not* an ordinary value, kept
/// apart so the inspector can render each distinctly instead of showing seven
/// different things as an empty cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpecialValue {
    /// Zero-length.
    EmptyString,
    /// All spaces.
    Spaces,
    LowValues,
    HighValues,
    /// A LINKAGE item with no argument, or an unset pointer.
    NullReference,
    /// Out of scope right now, or the debuggee could not produce it.
    Unavailable,
    /// Reading it raised an error; the message is in `value`.
    EvaluationError,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariablePresentationHint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariablesResponse {
    pub variables: Vec<Variable>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetVariableArguments {
    pub variables_reference: i64,
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetVariableResponse {
    pub value: String,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(default)]
    pub variables_reference: i64,
}

// ── evaluate ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateArguments {
    pub expression: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_id: Option<i64>,
    /// `watch`, `repl`, `hover`, `clipboard`. A hover must never have side
    /// effects; a repl entry may, once the developer has confirmed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// PowerRustCOBOL extension: the developer has been warned this expression
    /// may have side effects and chose to proceed. Absent or false means the
    /// adapter refuses anything that could mutate state.
    #[serde(default)]
    pub cobol_allow_side_effects: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_view: Option<ValueView>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateResponse {
    pub result: String,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(default)]
    pub variables_reference: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_picture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_special: Option<SpecialValue>,
    /// The adapter declined because evaluating this would change program state.
    /// The UI turns this into the "may have side effects — evaluate anyway?"
    /// prompt rather than a bare failure.
    #[serde(default)]
    pub cobol_side_effects_refused: bool,
}

// ── execution control ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadIdArguments {
    #[serde(default)]
    pub thread_id: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueResponse {
    #[serde(default)]
    pub all_threads_continued: bool,
}

/// Run to Cursor, in the DAP spelling we report as supported.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GotoTargetsArguments {
    pub source: Source,
    pub line: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectArguments {
    /// Kill the debuggee, or leave it running and just detach.
    #[serde(default)]
    pub terminate_debuggee: bool,
}

// ── events ────────────────────────────────────────────────────────────────────

/// Why execution stopped. The session strip shows this verbatim, so the set is
/// closed and each variant means one thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    /// A source, paragraph, section or program breakpoint.
    Breakpoint,
    /// A step finished.
    Step,
    /// The developer pressed Pause.
    Pause,
    /// A data breakpoint fired.
    DataBreakpoint,
    /// An exception filter matched: a runtime error, a bad file status, an
    /// INVALID KEY, AT END, SIZE ERROR.
    Exception,
    /// The session stopped at the program's first statement.
    Entry,
    /// Run to Cursor reached its target.
    Goto,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoppedEvent {
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub thread_id: i64,
    /// The developer-facing detail: which breakpoint, which condition, which
    /// file status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default)]
    pub all_threads_stopped: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hit_breakpoint_ids: Vec<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuedEvent {
    #[serde(default)]
    pub thread_id: i64,
    #[serde(default)]
    pub all_threads_continued: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputEvent {
    /// `console` (debugger's own messages), `stdout`, `stderr`, `important`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub output: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// PowerRustCOBOL extension: which investigation tab this belongs in, so a
    /// file operation lands in File I/O and a form event in Events rather than
    /// everything piling into the console.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_channel: Option<OutputChannel>,
    /// Milliseconds since the session started, for the Timeline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobol_elapsed_ms: Option<u64>,
    /// This entry records an irreversible side effect — external I/O, a network
    /// call, an SQL statement. The Timeline marks it so the developer can see
    /// what the program has already done to the world.
    #[serde(default)]
    pub cobol_side_effect: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputChannel {
    Console,
    Events,
    FileIo,
    Problems,
    Timeline,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExitedEvent {
    pub exit_code: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitiesEvent {
    pub capabilities: Capabilities,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakpointEvent {
    /// `changed`, `new`, `removed`.
    pub reason: String,
    pub breakpoint: Breakpoint,
}

/// The state of one open COBOL file, for the File I/O tab. Sent as the body of
/// the PowerRustCOBOL `cobolFileState` event.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStateEvent {
    /// The SELECT name.
    pub name: String,
    /// What ASSIGN TO resolved to on disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physical_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_position: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock_state: Option<String>,
    /// The two-character FILE STATUS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_status: Option<String>,
    /// A variables reference for the current record buffer, 0 when no record is
    /// current.
    #[serde(default)]
    pub record_reference: i64,
}

/// A raw, unmodelled body — used when a body is forwarded verbatim.
pub type RawBody = Value;

#[cfg(test)]
mod type_tests {
    use super::*;

    /// The whole point of the extension convention: our COBOL fields travel as
    /// ordinary JSON keys, and a client that does not know them simply drops
    /// them. This asserts they are *present and camelCase*, so a stock DAP
    /// client sees well-formed extra keys rather than malformed standard ones.
    #[test]
    fn cobol_extensions_serialise_as_camel_case_extra_keys() {
        let v = Variable {
            name: "WS-HTTP-STATUS".into(),
            value: "200".into(),
            cobol_picture: Some("9(3)".into()),
            cobol_level: Some(5),
            cobol_changed: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains(r#""cobolPicture":"9(3)""#), "{json}");
        assert!(json.contains(r#""cobolLevel":5"#), "{json}");
        assert!(json.contains(r#""cobolChanged":true"#), "{json}");
        // And the standard fields keep their standard names.
        assert!(json.contains(r#""name":"WS-HTTP-STATUS""#), "{json}");
    }

    /// A group item with no PICTURE must not emit `"cobolPicture":null` — a
    /// strict client would read that as "the PICTURE is null" rather than
    /// "there is none".
    #[test]
    fn absent_extensions_are_omitted_not_null() {
        let json = serde_json::to_string(&Variable {
            name: "WS-GROUP".into(),
            value: "".into(),
            ..Default::default()
        })
        .unwrap();
        assert!(!json.contains("cobolPicture"), "{json}");
        assert!(!json.contains("null"), "{json}");
    }

    #[test]
    fn capabilities_round_trip() {
        let caps = Capabilities {
            supports_conditional_breakpoints: true,
            supports_log_points: true,
            cobol_protocol_version: PROTOCOL_VERSION,
            exception_breakpoint_filters: vec![ExceptionBreakpointsFilter {
                filter: "fileStatus".into(),
                label: "File status class".into(),
                default: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        let back: Capabilities =
            serde_json::from_str(&serde_json::to_string(&caps).unwrap()).unwrap();
        assert_eq!(back, caps);
    }

    #[test]
    fn the_seven_non_values_are_distinct() {
        use SpecialValue::*;
        let all = [
            EmptyString,
            Spaces,
            LowValues,
            HighValues,
            NullReference,
            Unavailable,
            EvaluationError,
        ];
        let rendered: Vec<String> = all
            .iter()
            .map(|s| serde_json::to_string(s).unwrap())
            .collect();
        let unique: std::collections::HashSet<&String> = rendered.iter().collect();
        assert_eq!(unique.len(), all.len(), "each must be tellable apart: {rendered:?}");
    }

    #[test]
    fn a_source_derives_its_display_name_from_the_path() {
        let s = Source::path("/tmp/proj/generated/inner-form2.cbl");
        assert_eq!(s.name.as_deref(), Some("inner-form2.cbl"));
    }
}
