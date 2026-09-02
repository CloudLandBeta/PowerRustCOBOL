// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The runtime debug agent's state: the logical COBOL stack, the stepping
//! state machine, and the handle table behind lazy variable expansion.
//!
//! Kept out of `interpreter.rs` on purpose. Stepping is the part of a debugger
//! that is easy to get subtly wrong and impossible to check by eye — "Step Over
//! landed one frame too deep" looks like a working debugger until the developer
//! is three PERFORMs from where they meant to be. Here it is a pure state
//! machine over (depth, line), so it can be tested without an interpreter, a
//! program, or a process.
//!
//! # The depth model
//!
//! A COBOL program has no Rust-visible call stack — the runtime is a
//! tree-walking interpreter, so its own frames describe the *interpreter*, not
//! the program. Depth here is the **logical** one the developer sees: the base
//! program body is 0, each PERFORM of a paragraph or section adds one, each
//! CALL adds one, an event handler entered from the form adds one.
//!
//! ```text
//!   MAIN-PARA              depth 0   -- Step Over stops here again
//!     PERFORM VALIDATE     depth 1   -- Step Into stops here
//!       PERFORM CHECK-A    depth 2   -- Step Out from here returns to depth 1
//! ```

use serde::{Deserialize, Serialize};

/// How a frame was entered. Mirrors the DAP extension field of the same name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameKind {
    /// The program's own PROCEDURE DIVISION body — the base frame.
    Program,
    /// An OUT-OF-LINE `PERFORM <paragraph|section>`: control transfers away and
    /// returns. A real frame, shown in the call stack.
    Perform,
    /// The body of an INLINE `PERFORM ... END-PERFORM` (including `UNTIL`,
    /// `VARYING` and `n TIMES`).
    ///
    /// It adds **step depth** but is **not a call**, so it never appears in the
    /// call stack. Both halves of that are load-bearing:
    ///
    /// - Depth, because `PERFORM UNTIL ... END-PERFORM` is ONE COBOL statement.
    ///   Step Over executes it whole and lands past `END-PERFORM`; Step Into is
    ///   what goes inside. Without the depth increment, Step Over on the header
    ///   would stop on the first statement of the body — which is Step Into's
    ///   job (operator ruling, 2026-09-02).
    /// - Not a frame, because a loop is not something the developer called and
    ///   a stack pane that lists loops stops describing the call path.
    InlineLoop,
    Call,
    EventHandler,
    Declarative,
    SortProcedure,
}

impl FrameKind {
    /// Does entering this frame cross a program boundary? A CALL gets a fresh
    /// WORKING-STORAGE and its own LINKAGE; a PERFORM does not.
    pub fn is_program_boundary(self) -> bool {
        matches!(self, Self::Program | Self::Call | Self::EventHandler)
    }

    /// Does this frame belong in the call stack the developer reads?
    ///
    /// Everything except an inline loop, which exists only to carry step depth.
    pub fn is_call_frame(self) -> bool {
        !matches!(self, Self::InlineLoop)
    }
}

/// One frame of the logical COBOL stack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebugFrame {
    pub kind: FrameKind,
    /// PROGRAM-ID of the program this frame runs in.
    pub program: String,
    /// Containing SECTION, when the paragraph is inside one.
    pub section: Option<String>,
    /// The paragraph executing in this frame.
    pub paragraph: String,
    /// The source line currently executing *in this frame*. The top frame's is
    /// the stop location; an outer frame's is the line that performed inward,
    /// which is what makes the stack navigable.
    pub line: u32,
    pub col: u32,
    /// True when this frame's code is IDE-generated scaffolding.
    pub generated: bool,
}

impl DebugFrame {
    pub fn new(kind: FrameKind, program: impl Into<String>, paragraph: impl Into<String>) -> Self {
        Self {
            kind,
            program: program.into(),
            section: None,
            paragraph: paragraph.into(),
            line: 0,
            col: 0,
            generated: false,
        }
    }

    /// What the call-stack pane shows for this frame.
    pub fn display_name(&self) -> String {
        match (&self.section, self.paragraph.is_empty()) {
            (_, true) => self.program.clone(),
            (Some(sec), false) => format!("{} · {} OF {}", self.program, self.paragraph, sec),
            (None, false) => format!("{} · {}", self.program, self.paragraph),
        }
    }
}

/// Why the program stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    /// First statement of the session.
    Entry,
    /// A source, paragraph, section or program breakpoint. Carries the ids that
    /// fired, so the UI can highlight them.
    Breakpoint(Vec<i64>),
    /// A step completed.
    Step,
    /// The developer asked to pause.
    Pause,
    /// A watched data item changed. Carries the item and its new value.
    DataChanged { name: String, value: String },
    /// A runtime condition matched an enabled exception filter.
    Exception { filter: String, detail: String },
    /// Run to Cursor arrived.
    Goto,
}

impl StopReason {
    /// The DAP `reason` string.
    pub fn dap_reason(&self) -> &'static str {
        match self {
            Self::Entry => "entry",
            Self::Breakpoint(_) => "breakpoint",
            Self::Step => "step",
            Self::Pause => "pause",
            Self::DataChanged { .. } => "data breakpoint",
            Self::Exception { .. } => "exception",
            Self::Goto => "goto",
        }
    }

    /// The short phrase the session strip shows after "Paused · ".
    pub fn headline(&self) -> String {
        match self {
            Self::Entry => "Entry".into(),
            Self::Breakpoint(_) => "Breakpoint".into(),
            Self::Step => "Step".into(),
            Self::Pause => "Manual pause".into(),
            Self::DataChanged { name, .. } => format!("Data changed · {name}"),
            Self::Exception { filter, .. } => format!("Runtime error · {filter}"),
            Self::Goto => "Run to cursor".into(),
        }
    }
}

/// What the interpreter is trying to do between safepoints.
///
/// Every variant except [`Run`](StepMode::Run) means "stop again soon"; which
/// safepoint counts as *soon* is the whole difference between Step Over, Step
/// Into and Step Out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepMode {
    /// Run until something else stops us — a breakpoint, a watchpoint, an
    /// exception, or a pause request.
    Run,
    /// Stop at the next safepoint at this depth **or shallower**. Deeper
    /// safepoints — the insides of a PERFORM or CALL — are crossed.
    Over { depth: usize },
    /// Stop at the very next safepoint, wherever it is.
    Into,
    /// Stop only once the stack is shallower than this. Used to leave the
    /// current PERFORM or CALL.
    Out { depth: usize },
    /// Run until a given line, then stop. Breakpoints still win, and so does
    /// the target's disappearing — see [`StepDecision::TargetUnreachable`].
    ToCursor { line: u32 },
}

impl StepMode {
    /// Is the interpreter free-running? Used to decide whether a non-blocking
    /// check for a Pause request is worth making.
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Run | Self::ToCursor { .. })
    }

    /// Decide whether to stop at a safepoint on `line` at logical `depth`.
    ///
    /// `start_depth` is the depth the step was issued from, used only to notice
    /// that a Run-to-Cursor has escaped its frame.
    pub fn decide(&self, line: u32, depth: usize, start_depth: usize) -> StepDecision {
        match *self {
            Self::Run => StepDecision::Continue,
            // Shallower counts: a PERFORM that returns lands the developer on
            // the statement after it, which is exactly what Step Over means at
            // the end of a paragraph.
            Self::Over { depth: d } => {
                if depth <= d {
                    StepDecision::Stop
                } else {
                    StepDecision::Continue
                }
            }
            Self::Into => StepDecision::Stop,
            Self::Out { depth: d } => {
                if depth < d {
                    StepDecision::Stop
                } else {
                    StepDecision::Continue
                }
            }
            Self::ToCursor { line: target } => {
                if line == target {
                    StepDecision::Stop
                } else if depth < start_depth {
                    StepDecision::TargetUnreachable
                } else {
                    StepDecision::Continue
                }
            }
        }
    }
}

/// The verdict at one safepoint, before breakpoints are consulted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepDecision {
    /// Keep going.
    Continue,
    /// Stop here; the step finished.
    Stop,
    /// A Run-to-Cursor whose target can no longer be reached — the frame that
    /// contained it has returned. Stopping is better than running to the end of
    /// the program: the developer asked to arrive somewhere, and silently
    /// resuming forever is the worst answer.
    TargetUnreachable,
}

// ── Lazy variable handles ─────────────────────────────────────────────────────

/// What a `variablesReference` stands for.
///
/// The old debugger serialised **every data item in the program** into every
/// pause event. For a form with a few thousand items that is a large allocation
/// and a large message per single-step, most of it for rows the developer never
/// opens. A handle is issued instead, and the children are produced only when
/// the row is actually expanded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarRef {
    /// A whole COBOL scope of one frame.
    Scope { frame: i64, scope: ScopeKind },
    /// The children of a group item, or the occurrences of a table.
    Children { frame: i64, key: String },
    /// The 88-level condition names declared under one item.
    Conditions { frame: i64, key: String },
    /// The items that REDEFINE one item — the same bytes seen another way.
    Redefines { frame: i64, key: String },
    /// One open file's record buffer.
    FileRecord { frame: i64, file: String },
}

/// The COBOL scopes the inspector groups by, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ScopeKind {
    /// The running program's USING parameters.
    Arguments,
    LocalStorage,
    WorkingStorage,
    Linkage,
    FileSection,
    ScreenState,
    SpecialRegisters,
}

impl ScopeKind {
    /// Display order, arguments first — the same order a developer reads a
    /// PROCEDURE DIVISION header in.
    pub const ORDER: [ScopeKind; 7] = [
        Self::Arguments,
        Self::LocalStorage,
        Self::WorkingStorage,
        Self::Linkage,
        Self::FileSection,
        Self::ScreenState,
        Self::SpecialRegisters,
    ];

    /// The COBOL name, which is what the pane shows. Not translated: these are
    /// section names from the language, and a COBOL developer looks for
    /// "WORKING-STORAGE", not "Almacenamiento de trabajo".
    pub fn cobol_name(self) -> &'static str {
        match self {
            Self::Arguments => "PROCEDURE DIVISION USING",
            Self::LocalStorage => "LOCAL-STORAGE SECTION",
            Self::WorkingStorage => "WORKING-STORAGE SECTION",
            Self::Linkage => "LINKAGE SECTION",
            Self::FileSection => "FILE SECTION",
            Self::ScreenState => "SCREEN STATE",
            Self::SpecialRegisters => "SPECIAL REGISTERS",
        }
    }

    /// Is this scope costly enough that a client should leave it collapsed?
    pub fn is_expensive(self) -> bool {
        // Reading record buffers touches file state; the rest are in memory.
        matches!(self, Self::FileSection)
    }
}

/// Issues and resolves `variablesReference` handles.
///
/// Handles are per-stop: every reference is invalidated when the program moves,
/// because a handle into a frame that has returned would otherwise resolve
/// against whatever now occupies that depth. DAP allows exactly this — a client
/// must not reuse references across stops — and enforcing it here is what makes
/// a stale expansion impossible rather than merely unlikely.
#[derive(Debug, Default)]
pub struct VarRefTable {
    refs: Vec<VarRef>,
}

impl VarRefTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget every handle. Called on each resume.
    pub fn clear(&mut self) {
        self.refs.clear();
    }

    /// Issue a handle. References start at 1 — DAP reserves 0 for "this row has
    /// no children", so a valid handle can never be 0.
    pub fn issue(&mut self, r: VarRef) -> i64 {
        // Re-issue the same handle for the same target within one stop, so
        // expanding, collapsing and re-expanding a row is stable.
        if let Some(i) = self.refs.iter().position(|existing| *existing == r) {
            return i as i64 + 1;
        }
        self.refs.push(r);
        self.refs.len() as i64
    }

    /// Resolve a handle, or `None` if it is stale or was never issued.
    pub fn get(&self, handle: i64) -> Option<&VarRef> {
        if handle <= 0 {
            return None;
        }
        self.refs.get(handle as usize - 1)
    }

    pub fn len(&self) -> usize {
        self.refs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }
}

#[cfg(test)]
mod step_machine_tests {
    use super::*;
    use StepDecision::*;

    /// Step Over crosses a PERFORM: the safepoints inside it are deeper, and
    /// deeper is skipped. This is the difference between Over and Into, and the
    /// single most consequential line of arithmetic in the debugger.
    #[test]
    fn step_over_crosses_a_perform_and_stops_after_it() {
        let mode = StepMode::Over { depth: 0 };
        // The PERFORM's own statement is at depth 0 — we are leaving it.
        // Everything the PERFORM runs is at depth 1 and must be crossed.
        assert_eq!(mode.decide(10, 1, 0), Continue, "inside the PERFORM");
        assert_eq!(mode.decide(11, 2, 0), Continue, "nested deeper still");
        // Back at depth 0: the statement after the PERFORM.
        assert_eq!(mode.decide(6, 0, 0), Stop);
    }

    /// Step Over issued in a nested frame must also stop when that frame
    /// *returns* — otherwise the last statement of a paragraph would run away
    /// to the end of the program.
    #[test]
    fn step_over_at_the_end_of_a_paragraph_stops_in_the_caller() {
        let mode = StepMode::Over { depth: 2 };
        assert_eq!(mode.decide(30, 3, 2), Continue);
        assert_eq!(mode.decide(31, 2, 2), Stop, "same depth");
        assert_eq!(
            StepMode::Over { depth: 2 }.decide(12, 1, 2),
            Stop,
            "shallower — the PERFORM returned"
        );
    }

    /// The operator's scenario, 2026-09-02:
    ///
    /// ```text
    /// PERFORM UNTIL WS-DONE = TRUE   <- Step Over here
    ///     MOVE A TO B                     runs the WHOLE loop, no animation
    ///     ADD 1 TO WS-I
    /// END-PERFORM
    /// DISPLAY "AFTER"                <- and lands HERE
    /// ```
    ///
    /// `PERFORM UNTIL ... END-PERFORM` is one COBOL statement, so Step Over
    /// executes it completely. Landing on `MOVE A TO B` would be Step Into.
    /// This works only because the inline body carries a depth of its own.
    #[test]
    fn step_over_an_inline_perform_runs_the_whole_loop() {
        let mode = StepMode::Over { depth: 0 };
        // Header at depth 0; the body is depth 1 for every iteration.
        for iteration in 0..3 {
            assert_eq!(
                mode.decide(11, 1, 0),
                Continue,
                "MOVE, iteration {iteration} — crossed, not stopped on"
            );
            assert_eq!(mode.decide(12, 1, 0), Continue, "ADD, iteration {iteration}");
        }
        // Past END-PERFORM, back at the statement level.
        assert_eq!(mode.decide(14, 0, 0), Stop, "DISPLAY \"AFTER\"");
    }

    /// The other half of the same rule: Step INTO the header does go inside,
    /// and once inside, Step Over walks the body one statement at a time.
    #[test]
    fn step_into_enters_an_inline_loop_and_step_over_then_walks_it() {
        assert_eq!(StepMode::Into.decide(11, 1, 0), Stop, "into the body");
        // Now at depth 1 on MOVE; Step Over goes to ADD, not out of the loop.
        let inside = StepMode::Over { depth: 1 };
        assert_eq!(inside.decide(12, 1, 1), Stop, "the next statement in the body");
        // And when the loop finally ends, the statement after it is shallower.
        assert_eq!(inside.decide(14, 0, 1), Stop, "after END-PERFORM");
    }

    #[test]
    fn step_into_stops_at_the_very_next_safepoint_whatever_the_depth() {
        for depth in 0..4 {
            assert_eq!(StepMode::Into.decide(99, depth, 0), Stop, "depth {depth}");
        }
    }

    #[test]
    fn step_out_only_stops_once_the_stack_is_shallower() {
        let mode = StepMode::Out { depth: 2 };
        assert_eq!(mode.decide(20, 3, 2), Continue, "deeper");
        assert_eq!(mode.decide(21, 2, 2), Continue, "same frame is not out yet");
        assert_eq!(mode.decide(12, 1, 2), Stop, "returned to the caller");
    }

    #[test]
    fn run_never_stops_on_its_own() {
        for depth in 0..4 {
            assert_eq!(StepMode::Run.decide(1, depth, 0), Continue);
        }
        assert!(StepMode::Run.is_running());
        assert!(!StepMode::Into.is_running());
    }

    #[test]
    fn run_to_cursor_stops_on_its_line() {
        let mode = StepMode::ToCursor { line: 42 };
        assert_eq!(mode.decide(41, 0, 0), Continue);
        assert_eq!(mode.decide(42, 0, 0), Stop);
        assert_eq!(mode.decide(42, 7, 0), Stop, "any depth — the line is the target");
    }

    /// The case that would otherwise hang the developer: Run to Cursor into a
    /// paragraph that returns before reaching the line. Silently continuing to
    /// the end of the program is the wrong answer; stopping and saying so is
    /// the right one.
    #[test]
    fn run_to_cursor_gives_up_when_its_frame_returns() {
        let mode = StepMode::ToCursor { line: 42 };
        assert_eq!(mode.decide(30, 2, 2), Continue, "still in the frame");
        assert_eq!(mode.decide(31, 3, 2), Continue, "deeper is fine");
        assert_eq!(mode.decide(12, 1, 2), TargetUnreachable, "the frame returned");
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;

    #[test]
    fn a_frame_reads_as_a_developer_would_name_the_place() {
        let mut f = DebugFrame::new(FrameKind::Perform, "INNER-FORM2", "VALIDATE-INPUT");
        assert_eq!(f.display_name(), "INNER-FORM2 · VALIDATE-INPUT");
        f.section = Some("CHECKS".into());
        assert_eq!(f.display_name(), "INNER-FORM2 · VALIDATE-INPUT OF CHECKS");
        let base = DebugFrame::new(FrameKind::Program, "INNER-FORM2", "");
        assert_eq!(base.display_name(), "INNER-FORM2");
    }

    /// A CALL gets fresh storage, a PERFORM does not — the inspector shows a
    /// different WORKING-STORAGE on either side of that line.
    /// An inline loop carries depth but is not a call, so it must never be
    /// listed as a frame — a stack pane that shows loops stops describing the
    /// call path.
    #[test]
    fn an_inline_loop_carries_depth_but_is_not_a_call_frame() {
        assert!(!FrameKind::InlineLoop.is_call_frame());
        assert!(!FrameKind::InlineLoop.is_program_boundary());
        for k in [
            FrameKind::Program,
            FrameKind::Perform,
            FrameKind::Call,
            FrameKind::EventHandler,
            FrameKind::Declarative,
            FrameKind::SortProcedure,
        ] {
            assert!(k.is_call_frame(), "{k:?} belongs in the stack");
        }
    }

    #[test]
    fn only_some_frames_cross_a_program_boundary() {
        assert!(FrameKind::Call.is_program_boundary());
        assert!(FrameKind::EventHandler.is_program_boundary());
        assert!(FrameKind::Program.is_program_boundary());
        assert!(!FrameKind::Perform.is_program_boundary());
        assert!(!FrameKind::Declarative.is_program_boundary());
    }

    #[test]
    fn every_stop_reason_says_something_a_person_can_read() {
        let reasons = [
            StopReason::Entry,
            StopReason::Breakpoint(vec![1]),
            StopReason::Step,
            StopReason::Pause,
            StopReason::DataChanged {
                name: "WS-TOTAL".into(),
                value: "100".into(),
            },
            StopReason::Exception {
                filter: "fileStatus".into(),
                detail: "35".into(),
            },
            StopReason::Goto,
        ];
        for r in &reasons {
            assert!(!r.headline().is_empty(), "{r:?}");
            assert!(!r.dap_reason().is_empty(), "{r:?}");
        }
        assert_eq!(
            StopReason::DataChanged {
                name: "WS-TOTAL".into(),
                value: "100".into()
            }
            .headline(),
            "Data changed · WS-TOTAL"
        );
    }
}

#[cfg(test)]
mod var_ref_tests {
    use super::*;

    #[test]
    fn handles_start_at_one_because_zero_means_no_children() {
        let mut t = VarRefTable::new();
        let h = t.issue(VarRef::Scope {
            frame: 0,
            scope: ScopeKind::WorkingStorage,
        });
        assert_eq!(h, 1, "0 is DAP's 'this row does not expand'");
        assert!(t.get(0).is_none());
        assert!(t.get(h).is_some());
    }

    /// Expanding, collapsing and re-expanding must not leak a new handle each
    /// time — the table is cleared per stop, and a long inspection session
    /// inside one stop would otherwise grow without bound.
    #[test]
    fn the_same_target_re_issues_the_same_handle() {
        let mut t = VarRefTable::new();
        let target = VarRef::Children {
            frame: 0,
            key: "WS-HEADERS".into(),
        };
        assert_eq!(t.issue(target.clone()), t.issue(target.clone()));
        assert_eq!(t.len(), 1);
    }

    /// The invariant that makes a stale expansion impossible: resuming forgets
    /// every handle, so a reference into a frame that has returned resolves to
    /// nothing rather than to whatever now sits at that depth.
    #[test]
    fn resuming_invalidates_every_handle() {
        let mut t = VarRefTable::new();
        let h = t.issue(VarRef::Children {
            frame: 3,
            key: "WS-X".into(),
        });
        t.clear();
        assert!(t.get(h).is_none(), "a handle must not survive a resume");
        assert!(t.is_empty());
    }

    #[test]
    fn an_unknown_handle_resolves_to_nothing_rather_than_panicking() {
        let t = VarRefTable::new();
        for h in [-5, 0, 1, 99] {
            assert!(t.get(h).is_none());
        }
    }

    #[test]
    fn the_scope_order_is_the_one_a_cobol_developer_reads() {
        assert_eq!(ScopeKind::ORDER[0], ScopeKind::Arguments);
        assert_eq!(ScopeKind::ORDER.len(), 7);
        assert!(ScopeKind::FileSection.is_expensive());
        assert!(!ScopeKind::WorkingStorage.is_expensive());
        for s in ScopeKind::ORDER {
            assert!(s.cobol_name().chars().all(|c| !c.is_lowercase()), "{s:?}");
        }
    }
}

/// Parse a DAP hit condition. `None` when the text is not one.
///
/// Lives here rather than in `cobolt-dap` so the interpreter does not depend on
/// the protocol crate: the algebra is four lines and the duplication is cheaper
/// than the coupling.
pub fn parse_hit_condition(text: &str) -> Option<HitCondition> {
    HitCondition::parse(text)
}

/// The hit-count algebra: `5`, `>= 5`, `% 3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitCondition {
    Equals(u64),
    AtLeast(u64),
    GreaterThan(u64),
    AtMost(u64),
    LessThan(u64),
    Multiple(u64),
}

impl HitCondition {
    pub fn parse(text: &str) -> Option<Self> {
        let c: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        // Two-character operators first: `>=` must not be read as `>`.
        let (ctor, digits): (fn(u64) -> Self, &str) = if let Some(r) = c.strip_prefix(">=") {
            (Self::AtLeast, r)
        } else if let Some(r) = c.strip_prefix("<=") {
            (Self::AtMost, r)
        } else if let Some(r) = c.strip_prefix("==") {
            (Self::Equals, r)
        } else if let Some(r) = c.strip_prefix('>') {
            (Self::GreaterThan, r)
        } else if let Some(r) = c.strip_prefix('<') {
            (Self::LessThan, r)
        } else if let Some(r) = c.strip_prefix('=') {
            (Self::Equals, r)
        } else if let Some(r) = c.strip_prefix('%') {
            (Self::Multiple, r)
        } else {
            (Self::Equals, c.as_str())
        };
        let n: u64 = digits.parse().ok()?;
        // `% 0` divides by zero and `= 0` can never hold: hits start at 1.
        if n == 0 {
            return None;
        }
        Some(ctor(n))
    }

    pub fn fires_at(&self, hits: u64) -> bool {
        match *self {
            Self::Equals(n) => hits == n,
            Self::AtLeast(n) => hits >= n,
            Self::GreaterThan(n) => hits > n,
            Self::AtMost(n) => hits <= n,
            Self::LessThan(n) => hits < n,
            Self::Multiple(n) => n != 0 && hits % n == 0,
        }
    }
}
