// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tree-walking interpreter for COBOL programs.
//!
//! # Execution model
//!
//! The interpreter maintains a flat *paragraph map* built from the PROCEDURE
//! DIVISION.  `run()` iterates through paragraphs in declaration order,
//! executing statements inside each one.  Control-flow signals (GO TO, STOP
//! RUN, GOBACK) are propagated as special `RuntimeError` variants and caught at
//! the appropriate level.
//!
//! ## Control flow
//!
//! | Signal                        | Mechanism                          |
//! |-------------------------------|------------------------------------|
//! | STOP RUN                      | `Err(RuntimeError::StopRun)` → `run()` |
//! | GOBACK                        | `Err(RuntimeError::GoBack)` → `run()` |
//! | GO TO *paragraph*             | `Err(RuntimeError::GoTo{..})` → `run()` |
//! | PERFORM *paragraph*           | Recursive `exec_stmts()` call      |
//! | PERFORM … UNTIL/TIMES/VARYING | Rust loop inside `exec_perform`    |

use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::mpsc;

use cobolt_ast::{
    expr::{ArithOp, Condition, CmpOp, DataClass, Expr, FigurativeConstant, Literal,
           SignCond, UnaryOp},
    program::{FileOrganization, ProcedureBody, Program},
    stmt::{
        AcceptSource, CallArg, EvalSubject, InspectSpec, OpenMode, PerformTarget,
        ReplaceWhat, Stmt, TallyFor, UnstringTarget, VaryingAfter, WhenClause, WhenValue,
    },
};
use cobolt_lexer::Span;

use crate::{
    channels::{FormEvent, StateUpdate},
    db_runtime::DbRegistry,
    environment::CobolEnvironment,
    error::RuntimeError,
    exec_rust,
    objects::ObjectRegistry,
    value::{CobolValue, CobolNumeric},
};

// ── File I/O ──────────────────────────────────────────────────────────────────

/// Static description of a SELECT … ASSIGN file (from FILE-CONTROL + FD).
#[derive(Debug, Clone)]
struct FileSpec {
    /// ASSIGN target — either a literal path or the name of a data item that
    /// holds the path (resolved at OPEN time).
    assign: String,
    organization: FileOrganization,
    /// FILE STATUS data-item name (receives the 2-char status code), if any.
    status_field: Option<String>,
    /// The FD's 01-level record names (the buffer WRITE/READ act on).
    record_names: Vec<String>,
}

/// A currently-open file handle.
enum OpenFile {
    Writer { w: std::io::BufWriter<std::fs::File>, org: FileOrganization },
    Reader { r: std::io::BufReader<std::fs::File>, org: FileOrganization },
}

// ── Nested program registry ───────────────────────────────────────────────────

/// A compiled representation of one COBOL-85 nested program.
///
/// Nested programs share the outer program's environment (GLOBAL items are
/// naturally accessible because they live in the same `CobolEnvironment`
/// store).  Each nested program may also declare its own WORKING-STORAGE;
/// those items are pushed onto the outer env for the duration of the call
/// and removed on GOBACK.
#[derive(Debug)]
struct NestedProgram {
    /// Paragraph name → statement list.
    para_map: IndexMap<String, Vec<Stmt>>,
    /// Paragraph names in declaration order.
    para_order: Vec<String>,
    /// Local WORKING-STORAGE items declared inside this nested program.
    /// Format: `(uppercase_name, initial_value)`.
    local_items: Vec<(String, CobolValue)>,
}

/// Recursively register a `Program` and all of its `nested_programs` into
/// `registry`, keyed by the program-id (uppercase).
fn register_nested(prog: &Program, registry: &mut HashMap<String, NestedProgram>) {
    let (para_map, para_order) = build_para_map(&prog.procedure.body);

    // Collect this program's own local data items (everything in its DATA
    // DIVISION — they will be added to the env as a scope overlay on call).
    let local_items: Vec<(String, CobolValue)> = if let Some(data) = &prog.data {
        let local_env = CobolEnvironment::from_data_division(data);
        local_env.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    } else {
        Vec::new()
    };

    let key = prog.identification.program_id.to_ascii_uppercase();
    registry.insert(key, NestedProgram { para_map, para_order, local_items });

    // Recurse into any nested-programs declared inside this one.
    for child in &prog.nested_programs {
        register_nested(child, registry);
    }
}

/// Build the file registry from the program's FILE-CONTROL (SELECT) entries and
/// FILE SECTION (FD) records: `(logical name → spec, record name → file name)`.
fn build_file_specs(
    program: &Program,
) -> (HashMap<String, FileSpec>, HashMap<String, String>) {
    use cobolt_ast::program::DataSection;

    let mut specs: HashMap<String, FileSpec> = HashMap::new();
    let mut record_to_file: HashMap<String, String> = HashMap::new();

    // Collect each FD's 01-record names, keyed by (uppercased) file name.
    let mut fd_records: HashMap<String, Vec<String>> = HashMap::new();
    if let Some(data) = &program.data {
        for section in &data.sections {
            if let DataSection::FileSection(fds) = section {
                for fd in fds {
                    let names: Vec<String> = fd.records.iter()
                        .filter_map(|r| r.name.clone())
                        .map(|n| n.to_ascii_uppercase())
                        .collect();
                    fd_records.insert(fd.name.to_ascii_uppercase(), names);
                }
            }
        }
    }

    if let Some(env) = &program.environment {
        if let Some(io) = &env.input_output {
            for fc in &io.file_controls {
                let key = fc.name.to_ascii_uppercase();
                let record_names = fd_records.get(&key).cloned().unwrap_or_default();
                for rn in &record_names {
                    record_to_file.insert(rn.clone(), key.clone());
                }
                specs.insert(key.clone(), FileSpec {
                    assign: fc.assign.clone(),
                    organization: fc.organization,
                    status_field: fc.file_status.clone().map(|s| s.to_ascii_uppercase()),
                    record_names,
                });
            }
        }
    }

    (specs, record_to_file)
}

// ── Interpreter ───────────────────────────────────────────────────────────────

/// Tree-walking COBOL interpreter.
pub struct Interpreter {
    /// The parsed program (retained for metadata access).
    pub program: Program,
    /// Runtime data store — all DATA DIVISION items live here.
    pub env: CobolEnvironment,
    /// PowerCOBOL form/control object registry.
    pub objects: ObjectRegistry,
    /// Paragraph name (uppercase) → statement list.
    para_map: IndexMap<String, Vec<Stmt>>,
    /// Paragraph names in declaration order (for fall-through and THRU ranges).
    para_order: Vec<String>,
    /// COBOL-85 nested programs: program-id (uppercase) → compiled program.
    nested_registry: HashMap<String, NestedProgram>,
    /// Current PERFORM nesting depth (overflow guard).
    perform_depth: usize,
    /// Database runtime engine (Phase 8) — manages SQLite connections.
    db: DbRegistry,
    /// HTTP client (Phase 10) — manages persistent headers and sends requests.
    http: crate::http_runtime::HttpClient,

    // ── GUI Form Runtime channels (Phase 6) ───────────────────────────────────
    /// Receives UI events (button clicks, text changes, etc.) from the form window.
    /// When `Some`, `COBOL-WAIT-EVENT` blocks on `recv()` instead of quitting.
    event_rx: Option<mpsc::Receiver<FormEvent>>,
    /// Sends property-change notifications to the form window UI thread.
    /// Used by `COBOL-SET-PROPERTY`.
    state_tx: Option<mpsc::Sender<StateUpdate>>,
    /// Sends DISPLAY output to the IDE output panel (instead of stdout).
    display_tx: Option<mpsc::Sender<String>>,

    // ── Debugger channels (Phase 7) ───────────────────────────────────────────
    /// Receives `DebugCmd` from the IDE debugger panel (Continue, StepOver, Pause).
    debug_cmd_rx: Option<mpsc::Receiver<crate::debugger::DebugCmd>>,
    /// Sends `DebugEvent` to the IDE debugger panel (Paused, Resumed, Finished).
    debug_event_tx: Option<mpsc::Sender<crate::debugger::DebugEvent>>,
    /// Active breakpoints shared between the IDE and the interpreter.
    breakpoints: Option<crate::debugger::Breakpoints>,
    /// When `true`, pause before the very next statement (StepOver mode).
    debug_stepping: bool,
    /// Name of the paragraph currently being executed (for Paused events).
    current_paragraph: String,

    // ── File I/O ──────────────────────────────────────────────────────────────
    /// Logical file name → static SELECT/FD description.
    file_specs: HashMap<String, FileSpec>,
    /// FD record (01) name → owning logical file name.
    record_to_file: HashMap<String, String>,
    /// Logical file name → currently-open handle.
    open_files: HashMap<String, OpenFile>,
}

const MAX_PERFORM_DEPTH: usize = 512;

impl Interpreter {
    // ── Construction ──────────────────────────────────────────────────────────

    /// Create a new interpreter from a parsed program.
    ///
    /// The DATA DIVISION is walked to initialise all data items to their
    /// default / VALUE clause values.
    pub fn new(program: Program) -> Self {
        let env = if let Some(data) = &program.data {
            CobolEnvironment::from_data_division(data)
        } else {
            CobolEnvironment::new()
        };
        let (para_map, para_order) = build_para_map(&program.procedure.body);

        // Register all COBOL-85 nested programs (recursively).
        let mut nested_registry: HashMap<String, NestedProgram> = HashMap::new();
        for nested in &program.nested_programs {
            register_nested(nested, &mut nested_registry);
        }

        let (file_specs, record_to_file) = build_file_specs(&program);

        Self {
            program,
            env,
            objects: ObjectRegistry::new(),
            para_map,
            para_order,
            nested_registry,
            perform_depth: 0,
            db:   DbRegistry::new(),
            http: crate::http_runtime::HttpClient::new(),
            event_rx:   None,
            state_tx:   None,
            display_tx: None,
            debug_cmd_rx:      None,
            debug_event_tx:    None,
            breakpoints:       None,
            debug_stepping:    false,
            current_paragraph: String::new(),
            file_specs,
            record_to_file,
            open_files: HashMap::new(),
        }
    }

    /// Create an interpreter wired to the GUI Form Runtime Engine channels.
    ///
    /// - `event_rx`  — receives `FormEvent` from the UI (button clicks, etc.)
    /// - `state_tx`  — sends `StateUpdate` to the UI (SET-PROPERTY changes)
    /// - `display_tx`— sends DISPLAY output lines to the IDE output panel
    pub fn new_with_channels(
        program:    Program,
        event_rx:   mpsc::Receiver<FormEvent>,
        state_tx:   mpsc::Sender<StateUpdate>,
        display_tx: mpsc::Sender<String>,
    ) -> Self {
        let mut interp = Self::new(program);
        interp.event_rx   = Some(event_rx);
        interp.state_tx   = Some(state_tx);
        interp.display_tx = Some(display_tx);
        interp
    }

    /// Create an interpreter wired to the IDE debugger channels.
    ///
    /// - `debug_cmd_rx`  — receives `DebugCmd` from the IDE (Continue, StepOver, Pause)
    /// - `debug_event_tx`— sends `DebugEvent` to the IDE (Paused, Resumed, Finished)
    /// - `breakpoints`   — shared set of active breakpoint line numbers
    pub fn new_with_debug_channels(
        program:        Program,
        debug_cmd_rx:   mpsc::Receiver<crate::debugger::DebugCmd>,
        debug_event_tx: mpsc::Sender<crate::debugger::DebugEvent>,
        breakpoints:    crate::debugger::Breakpoints,
    ) -> Self {
        let mut interp = Self::new(program);
        interp.debug_cmd_rx   = Some(debug_cmd_rx);
        interp.debug_event_tx = Some(debug_event_tx);
        interp.breakpoints    = Some(breakpoints);
        interp.debug_stepping = true; // start paused at line 1
        interp
    }

    // ── Entry point ───────────────────────────────────────────────────────────

    /// Run the program to completion.
    ///
    /// Execution starts at the first paragraph and falls through subsequent
    /// paragraphs in declaration order.  GO TO, STOP RUN, and GOBACK are
    /// handled as control-flow signals; all other errors bubble up to the
    /// caller.
    pub fn run(&mut self) -> Result<(), RuntimeError> {
        let mut idx = 0usize;
        while idx < self.para_order.len() {
            let name = self.para_order[idx].clone();
            self.current_paragraph = name.clone();
            let stmts = match self.para_map.get(&name) {
                Some(s) => s.clone(),
                None => { idx += 1; continue; }
            };
            match self.exec_stmts(&stmts) {
                Ok(()) => idx += 1,
                Err(RuntimeError::GoTo { target }) => {
                    let upper = target.to_ascii_uppercase();
                    match self.para_order.iter().position(|n| n == &upper) {
                        Some(pos) => idx = pos,
                        None => return Err(RuntimeError::UndefinedParagraph {
                            name: upper,
                            span: Span::dummy(),
                        }),
                    }
                }
                // Normal program termination signals — treat as success.
                Err(RuntimeError::StopRun) | Err(RuntimeError::GoBack) => {
                    self.send_debug_finished();
                    self.db.close_all();
                    return Ok(());
                }
                Err(e) => return Err(e),
            }
        }
        self.send_debug_finished();
        self.db.close_all();
        Ok(())
    }

    fn send_debug_finished(&self) {
        if let Some(tx) = &self.debug_event_tx {
            let _ = tx.send(crate::debugger::DebugEvent::Finished);
        }
    }

    /// Execute a set of paragraphs given an explicit map + order (used for
    /// nested-program dispatch where the para_map differs from the outer one).
    ///
    /// Handles GO TO within the nested program's own paragraph space.
    /// GOBACK is propagated as-is so the caller can treat it as a normal return.
    fn run_para_sequence(
        &mut self,
        para_map:   &IndexMap<String, Vec<Stmt>>,
        para_order: &[String],
    ) -> Result<(), RuntimeError> {
        let mut idx = 0usize;
        while idx < para_order.len() {
            let name = &para_order[idx];
            self.current_paragraph = name.clone();
            let stmts = match para_map.get(name) {
                Some(s) => s.clone(),
                None => { idx += 1; continue; }
            };
            match self.exec_stmts(&stmts) {
                Ok(()) => idx += 1,
                Err(RuntimeError::GoTo { target }) => {
                    let upper = target.to_ascii_uppercase();
                    match para_order.iter().position(|n| n == &upper) {
                        Some(pos) => idx = pos,
                        None => return Err(RuntimeError::UndefinedParagraph {
                            name: upper,
                            span: Span::dummy(),
                        }),
                    }
                }
                // GOBACK / STOP RUN / errors propagate to caller.
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    // ── Statement dispatch ────────────────────────────────────────────────────

    fn exec_stmts(&mut self, stmts: &[Stmt]) -> Result<(), RuntimeError> {
        for stmt in stmts {
            self.debug_check(stmt)?;
            self.exec_stmt(stmt)?;
        }
        Ok(())
    }

    /// Called before every statement when a debug session is active.
    ///
    /// Pauses execution (blocking on `debug_cmd_rx`) when:
    ///   - `debug_stepping` is true (StepOver mode), OR
    ///   - the statement's source line matches an active breakpoint.
    ///
    /// While paused, sends `DebugEvent::Paused` with a full variable snapshot.
    /// Resumes on `DebugCmd::Continue` or `DebugCmd::StepOver`.
    fn debug_check(&mut self, stmt: &Stmt) -> Result<(), RuntimeError> {
        // Short-circuit when no debug session is attached.
        let (Some(cmd_rx), Some(ev_tx)) =
            (self.debug_cmd_rx.as_ref(), self.debug_event_tx.as_ref())
        else {
            return Ok(());
        };

        let span = stmt_span(stmt);
        let line = span.map(|s| s.line).unwrap_or(0);

        // Decide whether to pause.
        let hit_breakpoint = line > 0 && self.breakpoints.as_ref().map(|bp| {
            bp.lock().map(|set| set.contains(&line)).unwrap_or(false)
        }).unwrap_or(false);

        if !self.debug_stepping && !hit_breakpoint {
            // Check for async Pause command without blocking.
            match cmd_rx.try_recv() {
                Ok(crate::debugger::DebugCmd::Pause) => self.debug_stepping = true,
                _ => return Ok(()),
            }
        }

        // Build variable snapshot.
        let vars: Vec<crate::debugger::VarSnapshot> = self.env.iter()
            .map(|(k, v)| crate::debugger::VarSnapshot {
                name:  k.clone(),
                value: v.as_display_string(),
            })
            .collect();

        let _ = ev_tx.send(crate::debugger::DebugEvent::Paused {
            line:      line,
            col:       span.map(|s| s.col).unwrap_or(0),
            paragraph: self.current_paragraph.clone(),
            vars,
        });

        // Block until the IDE sends a command.
        self.debug_stepping = false; // reset; StepOver re-enables it below
        loop {
            match cmd_rx.recv() {
                Ok(crate::debugger::DebugCmd::Continue) => {
                    let _ = ev_tx.send(crate::debugger::DebugEvent::Resumed);
                    break;
                }
                Ok(crate::debugger::DebugCmd::StepOver) => {
                    self.debug_stepping = true; // pause again after this stmt
                    let _ = ev_tx.send(crate::debugger::DebugEvent::Resumed);
                    break;
                }
                Ok(crate::debugger::DebugCmd::Pause) => {
                    // Already paused; just re-send paused (no-op).
                }
                Err(_) => {
                    // Channel dropped — IDE closed. Stop the program.
                    return Err(RuntimeError::StopRun);
                }
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<(), RuntimeError> {
        match stmt {
            // ── Data movement ─────────────────────────────────────────────────
            Stmt::Move { from, to, .. } =>
                self.exec_move(from, to),
            Stmt::MoveCorresponding { .. } => {
                tracing::debug!("MOVE CORRESPONDING — not fully implemented");
                Ok(())
            }

            // ── Arithmetic ────────────────────────────────────────────────────
            Stmt::Add { operands, to, giving, rounded, on_size_error, not_on_size_error, span } =>
                self.exec_add(operands, to, giving.as_ref(), *rounded, on_size_error, not_on_size_error, *span),
            Stmt::Subtract { operands, from, giving, rounded, on_size_error, not_on_size_error, span } =>
                self.exec_subtract(operands, from, giving.as_ref(), *rounded, on_size_error, not_on_size_error, *span),
            Stmt::Multiply { lhs, by, giving, rounded, on_size_error, not_on_size_error, span } =>
                self.exec_multiply(lhs, by, giving.as_ref(), *rounded, on_size_error, not_on_size_error, *span),
            Stmt::Divide { lhs, by, giving, remainder, rounded, on_size_error, not_on_size_error, span } =>
                self.exec_divide(lhs, by, giving.as_ref(), remainder.as_ref(), *rounded, on_size_error, not_on_size_error, *span),
            Stmt::Compute { target, expr, rounded, on_size_error, not_on_size_error, span } =>
                self.exec_compute(target, expr, *rounded, on_size_error, not_on_size_error, *span),

            // ── Control flow ──────────────────────────────────────────────────
            Stmt::If { condition, then_stmts, else_stmts, .. } =>
                self.exec_if(condition, then_stmts, else_stmts),
            Stmt::Evaluate { subject, whens, other_stmts, .. } =>
                self.exec_evaluate(subject, whens, other_stmts),
            Stmt::Perform { target, span } =>
                self.exec_perform(target, *span),
            Stmt::GoTo { target, .. } =>
                Err(RuntimeError::GoTo { target: target.clone() }),
            Stmt::GoToDepending { targets, depending, span } =>
                self.exec_go_to_depending(targets, depending, *span),
            Stmt::Continue { .. } | Stmt::NextSentence { .. } => Ok(()),

            // ── I/O ───────────────────────────────────────────────────────────
            Stmt::Accept { target, from, span } =>
                self.exec_accept(target, from.as_ref(), *span),
            Stmt::Display { operands, no_advancing, .. } =>
                self.exec_display(operands, *no_advancing),
            Stmt::Open { mode, files, span } =>
                self.exec_open(*mode, files, *span),
            Stmt::Close { files, .. } =>
                self.exec_close(files),
            Stmt::Write { record, from, span, .. } =>
                self.exec_write(record, from.as_ref(), *span),
            Stmt::Read { file, into, at_end, not_at_end, span, .. } =>
                self.exec_read(file, into.as_ref(), at_end, not_at_end, *span),
            Stmt::Rewrite { .. } | Stmt::Delete { .. } | Stmt::Start { .. } => {
                tracing::warn!("REWRITE/DELETE/START not yet implemented — statement skipped");
                Ok(())
            }

            // ── String handling ───────────────────────────────────────────────
            Stmt::String_ { operands, into, pointer, span } =>
                self.exec_string(operands, into, pointer.as_ref(), *span),
            Stmt::Unstring { from, delimited_by, all, into, pointer, tallying, span } =>
                self.exec_unstring(from, delimited_by, *all, into,
                                   pointer.as_ref(), tallying.as_ref(), *span),
            Stmt::Inspect { target, spec, span } =>
                self.exec_inspect(target, spec, *span),

            // ── Sorting ───────────────────────────────────────────────────────
            Stmt::Sort { .. } | Stmt::Merge { .. } => {
                tracing::warn!("SORT/MERGE not yet implemented — statement skipped");
                Ok(())
            }

            // ── Subprogram linkage ────────────────────────────────────────────
            Stmt::Call { program, using, returning, on_exception, span } =>
                self.exec_call(program, using, returning.as_ref(), on_exception, *span),

            // ── Program termination ───────────────────────────────────────────
            Stmt::Stop { run: true, .. } => Err(RuntimeError::StopRun),
            Stmt::Stop { run: false, literal, .. } => {
                if let Some(lit) = literal {
                    let s = match lit {
                        Literal::String(s)  => s.clone(),
                        Literal::Integer(n) => n.to_string(),
                        _                   => String::new(),
                    };
                    if !s.is_empty() { println!("{s}"); }
                }
                Ok(())
            }
            Stmt::GoBack { .. } => Err(RuntimeError::GoBack),

            // ── EXEC RUST ─────────────────────────────────────────────────────
            Stmt::ExecRust { .. } =>
                exec_rust::execute(stmt, &mut self.env, &mut self.objects),

            // ── TRY / CATCH EXCEPTION / FINALLY ──────────────────────────────
            Stmt::TryCatch { try_stmts, exception_var, catch_stmts, finally_stmts, .. } => {
                // Execute the TRY body, catching any UserException.
                let try_result = self.exec_stmts(try_stmts);

                let handled = match &try_result {
                    Err(RuntimeError::UserException { message }) => {
                        // Bind the exception message to the named variable if given.
                        let msg = message.clone();
                        if let Some(var) = exception_var {
                            self.env.set_str(var, &msg);
                        }
                        // Run the CATCH body.
                        self.exec_stmts(catch_stmts)?;
                        true
                    }
                    _ => false,
                };

                // Always run FINALLY regardless of outcome.
                self.exec_stmts(finally_stmts)?;

                // If the error was not a UserException (or there was no catch),
                // propagate it now (after FINALLY ran).
                if !handled {
                    try_result?;
                }
                Ok(())
            }

            Stmt::Throw { message, span } => {
                let val = self.eval_expr(message, *span)?;
                Err(RuntimeError::UserException { message: val.as_display_string() })
            }

            // ── PowerCOBOL extensions ─────────────────────────────────────────
            Stmt::WindowOp { op, .. } => {
                tracing::debug!("WindowOp: {:?}", op);
                Ok(())
            }
            Stmt::ControlSet { control, property, value, span } => {
                let ctrl = self.expr_to_name(control);
                let val  = self.eval_expr(value, *span)?;
                self.objects.set_property(&ctrl, property, val.as_display_string());
                Ok(())
            }
        }
    }

    // ── MOVE ─────────────────────────────────────────────────────────────────

    fn exec_move(&mut self, from: &Expr, to: &[Expr]) -> Result<(), RuntimeError> {
        let val = self.eval_expr(from, from.span())?;
        for target in to {
            let name = self.expr_to_name(target);
            self.env.set(&name, val.clone());
        }
        Ok(())
    }

    // ── Arithmetic ────────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn exec_add(
        &mut self,
        operands: &[Expr],
        to: &[Expr],
        giving: Option<&Expr>,
        rounded: bool,
        on_size_error: &[Stmt],
        not_on_size_error: &[Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let sum = self.eval_sum(operands, span)?;
        let has = !on_size_error.is_empty();
        let mut size_err = false;
        if let Some(g) = giving {
            // `ADD a … TO b … GIVING c` → c = sum(a…) + sum(b…).
            let mut total = sum;
            for t in to {
                let v = self.eval_expr(t, span)?;
                total = total.add_val(&v);
            }
            let name = self.expr_to_name(g);
            size_err = self.store_arith(&name, total, rounded, has);
        } else {
            for t in to {
                let name = self.expr_to_name(t);
                let cur = self.env.get(&name).cloned()
                    .unwrap_or_else(|| CobolValue::from_i64(0));
                let result = cur.add_val(&sum);
                size_err |= self.store_arith(&name, result, rounded, has);
            }
        }
        self.run_size_error(size_err, on_size_error, not_on_size_error)
    }

    #[allow(clippy::too_many_arguments)]
    fn exec_subtract(
        &mut self,
        operands: &[Expr],
        from: &[Expr],
        giving: Option<&Expr>,
        rounded: bool,
        on_size_error: &[Stmt],
        not_on_size_error: &[Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let sub = self.eval_sum(operands, span)?;
        let has = !on_size_error.is_empty();
        let mut size_err = false;
        if let Some(g) = giving {
            // SUBTRACT … FROM base GIVING target
            let base = if from.is_empty() {
                CobolValue::from_i64(0)
            } else {
                self.eval_expr(&from[0], span)?
            };
            let result = base.sub_val(&sub);
            let name = self.expr_to_name(g);
            size_err = self.store_arith(&name, result, rounded, has);
        } else {
            for f in from {
                let name = self.expr_to_name(f);
                let cur = self.env.get(&name).cloned()
                    .unwrap_or_else(|| CobolValue::from_i64(0));
                let result = cur.sub_val(&sub);
                size_err |= self.store_arith(&name, result, rounded, has);
            }
        }
        self.run_size_error(size_err, on_size_error, not_on_size_error)
    }

    #[allow(clippy::too_many_arguments)]
    fn exec_multiply(
        &mut self,
        lhs: &Expr,
        by: &Expr,
        giving: Option<&Expr>,
        rounded: bool,
        on_size_error: &[Stmt],
        not_on_size_error: &[Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let l = self.eval_expr(lhs, span)?;
        let r = self.eval_expr(by, span)?;
        let result = l.mul_val(&r);
        let name = if let Some(g) = giving {
            self.expr_to_name(g)
        } else {
            self.expr_to_name(lhs)
        };
        let size_err = self.store_arith(&name, result, rounded, !on_size_error.is_empty());
        self.run_size_error(size_err, on_size_error, not_on_size_error)
    }

    #[allow(clippy::too_many_arguments)]
    fn exec_divide(
        &mut self,
        lhs: &Expr,
        by: &Expr,
        giving: Option<&Expr>,
        remainder: Option<&Expr>,
        rounded: bool,
        on_size_error: &[Stmt],
        not_on_size_error: &[Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let l = self.eval_expr(lhs, span)?;
        let r = self.eval_expr(by, span)?;
        let quotient = match l.div_val(&r) {
            Some(q) => q,
            None => {
                // Division by zero raises a size error if a handler is present.
                if !on_size_error.is_empty() {
                    return self.run_size_error(true, on_size_error, not_on_size_error);
                }
                return Err(RuntimeError::DivisionByZero { span });
            }
        };

        if let Some(rem_expr) = remainder {
            // COBOL REMAINDER uses the *integer* quotient: rem = dividend − (intq × divisor).
            let int_q = CobolValue::from_i64(quotient.as_i64().unwrap_or(0));
            let rem_val = l.sub_val(&int_q.mul_val(&r));
            let rname = self.expr_to_name(rem_expr);
            self.env.set(&rname, rem_val);
        }

        let name = if let Some(g) = giving {
            self.expr_to_name(g)
        } else {
            self.expr_to_name(lhs)
        };
        let size_err = self.store_arith(&name, quotient, rounded, !on_size_error.is_empty());
        self.run_size_error(size_err, on_size_error, not_on_size_error)
    }

    fn exec_compute(
        &mut self,
        target: &Expr,
        expr: &Expr,
        rounded: bool,
        on_size_error: &[Stmt],
        not_on_size_error: &[Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let val = self.eval_expr(expr, span)?;
        let name = self.expr_to_name(target);
        let size_err = self.store_arith(&name, val, rounded, !on_size_error.is_empty());
        self.run_size_error(size_err, on_size_error, not_on_size_error)
    }

    /// After an arithmetic store, run the appropriate conditional imperative.
    fn run_size_error(
        &mut self,
        size_err: bool,
        on_size_error: &[Stmt],
        not_on_size_error: &[Stmt],
    ) -> Result<(), RuntimeError> {
        if size_err {
            if !on_size_error.is_empty() {
                self.exec_stmts(on_size_error)?;
            }
        } else if !not_on_size_error.is_empty() {
            self.exec_stmts(not_on_size_error)?;
        }
        Ok(())
    }

    /// Store an arithmetic result into `name`, returning `true` if a size error
    /// occurred (the value's integer part exceeds the field's PIC capacity).
    ///
    /// When `rounded`, round (half away from zero) to the field's scale;
    /// otherwise `assign` truncates, per COBOL's default. On a size error *with*
    /// a handler (`suppress_on_overflow`), the field is left unchanged.
    fn store_arith(
        &mut self,
        name: &str,
        value: CobolValue,
        rounded: bool,
        suppress_on_overflow: bool,
    ) -> bool {
        let value = if rounded {
            let scale = match self.env.get(name) {
                Some(CobolValue::Numeric(f)) => Some(f.decimals),
                _ => None,
            };
            match (scale, value.as_exact()) {
                (Some(s), Some(num)) => CobolValue::Numeric(num.round_to(s)),
                _ => value,
            }
        } else {
            value
        };

        // Size error: does the integer part fit the receiving field's capacity?
        let overflow = match (self.env.integer_capacity(name), value.as_exact()) {
            (Some(cap), Some(num)) => num.integer_digit_count() > cap as u32,
            _ => false,
        };

        if overflow && suppress_on_overflow {
            // Leave the receiving field unchanged; caller runs ON SIZE ERROR.
            return true;
        }
        self.env.set(name, value);
        overflow
    }

    /// Sum a list of expressions to a single `CobolValue`.
    fn eval_sum(&mut self, operands: &[Expr], span: Span) -> Result<CobolValue, RuntimeError> {
        let mut total = CobolValue::from_i64(0);
        for op in operands {
            let v = self.eval_expr(op, span)?;
            total = total.add_val(&v);
        }
        Ok(total)
    }

    // ── Control flow ──────────────────────────────────────────────────────────

    fn exec_if(
        &mut self,
        condition: &Condition,
        then_stmts: &[Stmt],
        else_stmts: &[Stmt],
    ) -> Result<(), RuntimeError> {
        if self.eval_condition(condition)? {
            self.exec_stmts(then_stmts)
        } else {
            self.exec_stmts(else_stmts)
        }
    }

    fn exec_evaluate(
        &mut self,
        subject: &EvalSubject,
        whens: &[WhenClause],
        other_stmts: &[Stmt],
    ) -> Result<(), RuntimeError> {
        'when: for when in whens {
            for val in &when.values {
                let matched = self.when_value_matches(subject, val)?;
                if matched {
                    // Don't match purely-OTHER clauses here (handled below).
                    if !matches!(val, WhenValue::Other) {
                        return self.exec_stmts(&when.stmts);
                    }
                    // If ALL values in this WHEN are OTHER, skip to other_stmts.
                    let all_other = when.values.iter().all(|v| matches!(v, WhenValue::Other));
                    if !all_other {
                        return self.exec_stmts(&when.stmts);
                    }
                }
            }
        }
        // WHEN OTHER / no match
        self.exec_stmts(other_stmts)
    }

    fn when_value_matches(
        &mut self,
        subject: &EvalSubject,
        val: &WhenValue,
    ) -> Result<bool, RuntimeError> {
        match (subject, val) {
            (_, WhenValue::Any) => Ok(true),
            (_, WhenValue::Other) => Ok(false), // handled specially in exec_evaluate
            (EvalSubject::True_, WhenValue::Condition(c)) => self.eval_condition(c),
            (EvalSubject::False_, WhenValue::Condition(c)) => {
                Ok(!self.eval_condition(c)?)
            }
            (EvalSubject::Expr(e), WhenValue::Literal(lit)) => {
                let subj = self.eval_expr(e, e.span())?;
                let lv   = literal_to_value(lit);
                Ok(compare_values(&subj, &lv, CmpOp::Eq))
            }
            (EvalSubject::Expr(e), WhenValue::Range(lo, hi)) => {
                let subj  = self.eval_expr(e, e.span())?;
                let lo_v  = literal_to_value(lo);
                let hi_v  = literal_to_value(hi);
                Ok(compare_values(&subj, &lo_v, CmpOp::Ge)
                    && compare_values(&subj, &hi_v, CmpOp::Le))
            }
            (EvalSubject::Expr(e), WhenValue::Condition(c)) => {
                // EVALUATE expr WHEN condition — treat condition as boolean check
                let _ = e;
                self.eval_condition(c)
            }
            _ => Ok(false),
        }
    }

    fn exec_perform(&mut self, target: &PerformTarget, span: Span) -> Result<(), RuntimeError> {
        if self.perform_depth >= MAX_PERFORM_DEPTH {
            return Err(RuntimeError::PerformDepthExceeded { max: MAX_PERFORM_DEPTH });
        }
        self.perform_depth += 1;
        let result = self.exec_perform_inner(target, span);
        self.perform_depth -= 1;
        // Absorb GoBack inside a PERFORM (it means "return from this PERFORM").
        match result {
            Err(RuntimeError::GoBack) => Ok(()),
            other => other,
        }
    }

    fn exec_perform_inner(&mut self, target: &PerformTarget, span: Span) -> Result<(), RuntimeError> {
        match target {
            PerformTarget::Paragraph(name, s) => {
                let stmts = self.para_stmts(name, *s)?;
                self.exec_stmts(&stmts)
            }
            PerformTarget::Section(name, s) => {
                // Treat a section PERFORM as executing all paragraphs in it.
                // We collect paragraphs whose names start with SECTION-NAME-*
                // (or exactly match).  Simplified: just find by name.
                match self.para_stmts(name, *s) {
                    Ok(stmts) => self.exec_stmts(&stmts),
                    Err(_) => {
                        // Try section as a block of paragraphs
                        let upper = name.to_ascii_uppercase();
                        let stmts = self.collect_section_stmts(&upper);
                        self.exec_stmts(&stmts)
                    }
                }
            }
            PerformTarget::Thru { from, to, span: s } => {
                let stmts = self.thru_stmts(from, to, *s)?;
                self.exec_stmts(&stmts)
            }
            PerformTarget::Inline { stmts } =>
                self.exec_stmts(stmts),
            PerformTarget::Times { count, stmts } => {
                let n = self.eval_expr(count, span)?.as_i64().unwrap_or(0).max(0);
                for _ in 0..n {
                    self.exec_stmts(stmts)?;
                }
                Ok(())
            }
            PerformTarget::Until { condition, test_before, stmts } => {
                if *test_before {
                    while !self.eval_condition(condition)? {
                        self.exec_stmts(stmts)?;
                    }
                } else {
                    loop {
                        self.exec_stmts(stmts)?;
                        if self.eval_condition(condition)? { break; }
                    }
                }
                Ok(())
            }
            PerformTarget::Varying { var, from, by, until, stmts, after } => {
                self.exec_perform_varying(var, from, by, until, stmts, after, span)
            }
        }
    }

    fn exec_perform_varying(
        &mut self,
        var: &Expr,
        from: &Expr,
        by: &Expr,
        until: &Condition,
        stmts: &[Stmt],
        after: &[VaryingAfter],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let from_val = self.eval_expr(from, span)?;
        let var_name = self.expr_to_name(var);
        self.env.set(&var_name, from_val);

        // Initialise AFTER variables
        for aft in after {
            let aft_from = self.eval_expr(&aft.from, span)?;
            let aft_name = self.expr_to_name(&aft.var);
            self.env.set(&aft_name, aft_from);
        }

        loop {
            if self.eval_condition(until)? { break; }

            // Inner AFTER loops (right-most varies fastest)
            self.exec_perform_after(after, stmts, span)?;

            // Increment outer variable
            let by_val = self.eval_expr(by, span)?;
            let cur = self.env.get(&var_name).cloned()
                .unwrap_or_else(|| CobolValue::from_i64(0));
            self.env.set(&var_name, cur.add_val(&by_val));
        }
        Ok(())
    }

    fn exec_perform_after(
        &mut self,
        after: &[VaryingAfter],
        stmts: &[Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        if after.is_empty() {
            return self.exec_stmts(stmts);
        }
        let (head, tail) = (&after[0], &after[1..]);
        let from_val = self.eval_expr(&head.from, span)?;
        let var_name = self.expr_to_name(&head.var);
        self.env.set(&var_name, from_val);

        loop {
            if self.eval_condition(&head.until)? { break; }
            self.exec_perform_after(tail, stmts, span)?;
            let by_val = self.eval_expr(&head.by, span)?;
            let cur = self.env.get(&var_name).cloned()
                .unwrap_or_else(|| CobolValue::from_i64(0));
            self.env.set(&var_name, cur.add_val(&by_val));
        }
        Ok(())
    }

    fn exec_go_to_depending(
        &mut self,
        targets: &[String],
        depending: &Expr,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let idx = self.eval_expr(depending, span)?.as_i64().unwrap_or(0);
        if idx >= 1 && (idx as usize) <= targets.len() {
            Err(RuntimeError::GoTo { target: targets[(idx - 1) as usize].clone() })
        } else {
            Ok(()) // out-of-range → fall through
        }
    }

    // ── ACCEPT / DISPLAY ──────────────────────────────────────────────────────

    fn exec_accept(
        &mut self,
        target: &Expr,
        from: Option<&AcceptSource>,
        _span: Span,
    ) -> Result<(), RuntimeError> {
        let name = self.expr_to_name(target);
        match from {
            None => {
                // Read one line from stdin.
                use std::io::BufRead;
                let stdin = std::io::stdin();
                let mut line = String::new();
                let _ = stdin.lock().read_line(&mut line);
                let s = line.trim_end_matches('\n').trim_end_matches('\r').to_owned();
                self.env.set_str(&name, &s);
            }
            Some(AcceptSource::Date)      => self.env.set_str(&name, &runtime_date()),
            Some(AcceptSource::Time)      => self.env.set_str(&name, &runtime_time()),
            Some(AcceptSource::Day)       => self.env.set_str(&name, &runtime_julian_day()),
            Some(AcceptSource::DayOfWeek) => self.env.set_i64(&name, runtime_day_of_week()),
            Some(AcceptSource::CommandLine) => {} // no-op
            Some(AcceptSource::Environment(var)) => {
                let val = std::env::var(var).unwrap_or_default();
                self.env.set_str(&name, &val);
            }
        }
        Ok(())
    }

    fn exec_display(&mut self, operands: &[Expr], no_advancing: bool) -> Result<(), RuntimeError> {
        let mut out = String::new();
        for op in operands {
            let val = self.eval_expr(op, op.span())?;
            out.push_str(&val.as_display_string());
        }
        // GUI mode: send through the display channel so the IDE output panel receives it.
        if let Some(tx) = &self.display_tx {
            let _ = tx.send(out.clone());
        } else if no_advancing {
            print!("{out}");
            use std::io::Write;
            let _ = std::io::stdout().flush();
        } else {
            println!("{out}");
        }
        Ok(())
    }

    // ── STRING ────────────────────────────────────────────────────────────────

    fn exec_string(
        &mut self,
        operands: &[(Expr, Option<Expr>)],
        into: &Expr,
        _pointer: Option<&Expr>,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let mut result = String::new();
        for (src_expr, delim_expr) in operands {
            let src = self.eval_expr(src_expr, span)?.as_display_string();
            if let Some(delim_e) = delim_expr {
                let delim = self.eval_expr(delim_e, span)?.as_display_string();
                let delim_upper = delim.trim().to_ascii_uppercase();
                if delim_upper == "SIZE" {
                    result.push_str(&src);
                } else if delim_upper == "SPACE" || delim_upper == "SPACES" {
                    result.push_str(src.trim_end());
                } else if let Some(pos) = src.find(delim.as_str()) {
                    result.push_str(&src[..pos]);
                } else {
                    result.push_str(&src);
                }
            } else {
                result.push_str(&src);
            }
        }
        let name = self.expr_to_name(into);
        self.env.set_str(&name, &result);
        Ok(())
    }

    // ── UNSTRING ──────────────────────────────────────────────────────────────

    fn exec_unstring(
        &mut self,
        from: &Expr,
        delimited_by: &[Expr],
        _all: bool,
        into: &[UnstringTarget],
        _pointer: Option<&Expr>,
        _tallying: Option<&Expr>,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let src = self.eval_expr(from, span)?.as_display_string();
        let delims: Vec<String> = delimited_by.iter()
            .map(|d| self.eval_expr(d, span)
                .unwrap_or_else(|_| CobolValue::from_str(" ", 1))
                .as_display_string())
            .collect();

        // Split source by all delimiters in sequence.
        let mut parts: Vec<String> = vec![src];
        for delim in &delims {
            let mut new_parts = Vec::new();
            for part in &parts {
                for sub in part.split(delim.as_str()) {
                    new_parts.push(sub.to_string());
                }
            }
            parts = new_parts;
        }

        for (i, target) in into.iter().enumerate() {
            let name = self.expr_to_name(&target.target);
            let val = parts.get(i).map(|s| s.as_str()).unwrap_or("");
            self.env.set_str(&name, val);
            if let Some(count_expr) = &target.count {
                let cname = self.expr_to_name(count_expr);
                self.env.set_i64(&cname, val.len() as i64);
            }
        }
        Ok(())
    }

    // ── INSPECT ───────────────────────────────────────────────────────────────

    fn exec_inspect(&mut self, target: &Expr, spec: &InspectSpec, span: Span) -> Result<(), RuntimeError> {
        let name = self.expr_to_name(target);
        let val  = self.env.get(&name).cloned()
            .unwrap_or_else(|| CobolValue::from_str("", 0));
        let mut s = val.as_display_string();

        match spec {
            InspectSpec::Tallying(tallies) => {
                for tally in tallies {
                    let ctr_name = self.expr_to_name(&tally.counter);
                    let mut count = 0i64;
                    for for_ in &tally.for_ {
                        count += match for_ {
                            TallyFor::Characters => s.len() as i64,
                            TallyFor::All(e) => {
                                let pat = self.eval_expr(e, span)?.as_display_string();
                                s.matches(pat.as_str()).count() as i64
                            }
                            TallyFor::Leading(e) => {
                                let pat = self.eval_expr(e, span)?.as_display_string();
                                s.chars()
                                    .take_while(|c| pat.contains(*c))
                                    .count() as i64
                            }
                            TallyFor::Trailing(e) => {
                                let pat = self.eval_expr(e, span)?.as_display_string();
                                s.chars().rev()
                                    .take_while(|c| pat.contains(*c))
                                    .count() as i64
                            }
                        };
                    }
                    self.env.set_i64(&ctr_name, count);
                }
            }
            InspectSpec::Replacing(replaces) => {
                for rep in replaces {
                    let by = self.eval_expr(&rep.by, span)?.as_display_string();
                    match &rep.what {
                        ReplaceWhat::All(e) => {
                            let pat = self.eval_expr(e, span)?.as_display_string();
                            s = s.replace(pat.as_str(), &by);
                        }
                        ReplaceWhat::First(e) => {
                            let pat = self.eval_expr(e, span)?.as_display_string();
                            if let Some(pos) = s.find(pat.as_str()) {
                                s.replace_range(pos..pos + pat.len(), &by);
                            }
                        }
                        ReplaceWhat::Leading(e) => {
                            let pat = self.eval_expr(e, span)?.as_display_string();
                            while s.starts_with(pat.as_str()) {
                                let end = pat.len();
                                let repl_len = by.len().min(end);
                                s.replace_range(0..end, &by[..repl_len]);
                            }
                        }
                        ReplaceWhat::Trailing(e) => {
                            let pat = self.eval_expr(e, span)?.as_display_string();
                            while s.ends_with(pat.as_str()) {
                                let start = s.len() - pat.len();
                                let repl_len = by.len().min(pat.len());
                                s.replace_range(start.., &by[..repl_len]);
                            }
                        }
                        ReplaceWhat::Characters => {
                            let new_s: String = s.chars()
                                .map(|_| by.chars().next().unwrap_or(' '))
                                .collect();
                            s = new_s;
                        }
                    }
                }
                self.env.set_str(&name, &s);
            }
            InspectSpec::Converting { from, to } => {
                let from_s = self.eval_expr(from, span)?.as_display_string();
                let to_s   = self.eval_expr(to,   span)?.as_display_string();
                for (fc, tc) in from_s.chars().zip(to_s.chars()) {
                    s = s.replace(fc, &tc.to_string());
                }
                self.env.set_str(&name, &s);
            }
            InspectSpec::TallyingReplacing(tallies, replaces) => {
                self.exec_inspect(target, &InspectSpec::Tallying(tallies.clone()), span)?;
                self.exec_inspect(target, &InspectSpec::Replacing(replaces.clone()), span)?;
            }
        }
        Ok(())
    }

    // ── File I/O (SEQUENTIAL / LINE SEQUENTIAL) ──────────────────────────────
    //
    // Two organisations are supported, modelled on the COBOL-85 sequential file
    // verbs: record SEQUENTIAL (fixed-length records, no terminators) and LINE
    // SEQUENTIAL (newline-terminated text records, trailing spaces not stored).
    // Each operation updates the file's FILE STATUS item (if declared) with the
    // usual codes: 00 ok, 10 end-of-file, 30 permanent error, 35 not found.

    /// Resolve a SELECT … ASSIGN target to a filesystem path. If the assign
    /// value names a data item, that item's current (trimmed) value is used;
    /// otherwise the assign string itself is the path.
    fn resolve_assign_path(&self, assign: &str) -> String {
        let key = assign.trim().to_ascii_uppercase();
        if let Some(v) = self.env.get_string(&key) {
            return v.trim_end().to_string();
        }
        assign.trim().to_string()
    }

    /// Set a file's FILE STATUS data item (if declared) to a 2-character code.
    fn set_file_status(&mut self, file: &str, code: &str) {
        if let Some(field) = self.file_specs.get(file).and_then(|s| s.status_field.clone()) {
            self.env.set_str(&field, code);
        }
    }

    fn exec_open(&mut self, mode: OpenMode, files: &[String], _span: Span)
        -> Result<(), RuntimeError>
    {
        use std::fs::OpenOptions;
        use std::io::{BufReader, BufWriter};

        for raw in files {
            let file = raw.to_ascii_uppercase();
            let Some(spec) = self.file_specs.get(&file).cloned() else {
                tracing::warn!("OPEN: unknown file '{}'", raw);
                continue;
            };
            let path = self.resolve_assign_path(&spec.assign);
            let org  = spec.organization;

            let result: std::io::Result<OpenFile> = match mode {
                OpenMode::Output =>
                    std::fs::File::create(&path)
                        .map(|f| OpenFile::Writer { w: BufWriter::new(f), org }),
                OpenMode::Extend =>
                    OpenOptions::new().create(true).append(true).open(&path)
                        .map(|f| OpenFile::Writer { w: BufWriter::new(f), org }),
                OpenMode::Input =>
                    std::fs::File::open(&path)
                        .map(|f| OpenFile::Reader { r: BufReader::new(f), org }),
                OpenMode::InputOutput =>
                    OpenOptions::new().read(true).write(true).create(true).open(&path)
                        .map(|f| OpenFile::Reader { r: BufReader::new(f), org }),
            };

            match result {
                Ok(handle) => {
                    self.open_files.insert(file.clone(), handle);
                    self.set_file_status(&file, "00");
                }
                Err(e) => {
                    tracing::warn!("OPEN '{}' ({}) failed: {}", raw, path, e);
                    let code = if matches!(mode, OpenMode::Input)
                        && e.kind() == std::io::ErrorKind::NotFound { "35" } else { "30" };
                    self.set_file_status(&file, code);
                }
            }
        }
        Ok(())
    }

    fn exec_close(&mut self, files: &[String]) -> Result<(), RuntimeError> {
        use std::io::Write as _;
        for raw in files {
            let file = raw.to_ascii_uppercase();
            if let Some(mut handle) = self.open_files.remove(&file) {
                if let OpenFile::Writer { w, .. } = &mut handle {
                    let _ = w.flush();
                }
                self.set_file_status(&file, "00");
            } else {
                self.set_file_status(&file, "42"); // CLOSE of a file not open
            }
        }
        Ok(())
    }

    fn exec_write(&mut self, record: &Expr, from: Option<&Expr>, _span: Span)
        -> Result<(), RuntimeError>
    {
        use std::io::Write as _;
        // WRITE rec FROM src ⇒ move src into the record buffer first.
        if let Some(src) = from {
            self.exec_move(src, std::slice::from_ref(record))?;
        }
        let rec_name = self.expr_to_name(record);
        let Some(file) = self.record_to_file.get(&rec_name).cloned() else {
            tracing::warn!("WRITE: record '{}' is not part of any FD", rec_name);
            return Ok(());
        };
        let bytes = self.env.get_string(&rec_name).unwrap_or_default();

        let res = if let Some(OpenFile::Writer { w, org }) = self.open_files.get_mut(&file) {
            match org {
                FileOrganization::LineSequential =>
                    writeln!(w, "{}", bytes.trim_end()),
                _ => w.write_all(bytes.as_bytes()),
            }
        } else {
            tracing::warn!("WRITE to '{}' which is not open for output", file);
            self.set_file_status(&file, "48");
            return Ok(());
        };

        match res {
            Ok(()) => self.set_file_status(&file, "00"),
            Err(e) => { tracing::warn!("WRITE failed: {}", e); self.set_file_status(&file, "30"); }
        }
        Ok(())
    }

    fn exec_read(
        &mut self,
        file_name: &str,
        into: Option<&Expr>,
        at_end: &[Stmt],
        not_at_end: &[Stmt],
        _span: Span,
    ) -> Result<(), RuntimeError> {
        use std::io::BufRead as _;

        let file = file_name.to_ascii_uppercase();
        let rec_name = match self.file_specs.get(&file) {
            Some(spec) => spec.record_names.first().cloned(),
            None => {
                tracing::warn!("READ: unknown file '{}'", file_name);
                return Ok(());
            }
        };

        // Read one record into an owned buffer, then drop the handle borrow.
        let mut line = String::new();
        let read_n = if let Some(OpenFile::Reader { r, .. }) = self.open_files.get_mut(&file) {
            r.read_line(&mut line)
        } else {
            self.set_file_status(&file, "47"); // READ on file not open for input
            Ok(0)
        };

        match read_n {
            Ok(0) => {
                self.set_file_status(&file, "10"); // end of file
                self.exec_stmts(at_end)?;
            }
            Ok(_) => {
                while line.ends_with('\n') || line.ends_with('\r') { line.pop(); }
                if let Some(rn) = &rec_name {
                    self.env.set_str(rn, &line);
                    if let Some(tgt) = into {
                        let tname = self.expr_to_name(tgt);
                        if let Some(v) = self.env.get(rn).cloned() {
                            self.env.set(&tname, v);
                        }
                    }
                }
                self.set_file_status(&file, "00");
                self.exec_stmts(not_at_end)?;
            }
            Err(e) => {
                tracing::warn!("READ failed: {}", e);
                self.set_file_status(&file, "30");
                self.exec_stmts(at_end)?;
            }
        }
        Ok(())
    }

    // ── CALL ──────────────────────────────────────────────────────────────────

    fn exec_call(
        &mut self,
        program: &Expr,
        using: &[CallArg],
        _returning: Option<&Expr>,
        _on_exception: &[Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let prog_name = self.eval_expr(program, span)?
            .as_display_string()
            .trim()
            .to_ascii_uppercase();

        match prog_name.as_str() {
            // ── Built-in PowerCOBOL runtime calls (COBOL-* prefix) ────────────
            // COBOL-INIT-FORM USING form-name  — initialise the form; no-op in CLI mode
            "COBOL-INIT-FORM" | "COBOLT-INIT-FORM" => {
                // Nothing to do in non-GUI (CLI) mode.
            }

            // COBOL-WAIT-EVENT USING event-id control-id
            // GUI mode: block until the UI sends a FormEvent, then populate the two fields.
            // CLI mode: immediately set COBOL-QUIT = 1 so the event loop exits cleanly.
            "COBOL-WAIT-EVENT" | "COBOLT-WAIT-EVENT" => {
                if let Some(rx) = &self.event_rx {
                    // Block the interpreter thread until the UI sends an event.
                    match rx.recv() {
                        Ok(ev) => {
                            // Populate COBOL-EVENT-ID and COBOL-CONTROL-ID (args 0 and 1).
                            if using.len() >= 1 {
                                let n = self.expr_to_name(call_arg_expr(&using[0]));
                                self.env.set_str(&n, &ev.event_id);
                            }
                            if using.len() >= 2 {
                                let n = self.expr_to_name(call_arg_expr(&using[1]));
                                self.env.set_str(&n, &ev.ctrl_id);
                            }
                            // Sentinel: UI closed the form → exit event loop.
                            if ev.ctrl_id == "__QUIT__" {
                                self.env.set_str("COBOL-QUIT", "1");
                            }
                        }
                        Err(_) => {
                            // Channel disconnected (UI closed) → stop the loop.
                            self.env.set_str("COBOL-QUIT", "1");
                        }
                    }
                } else {
                    // CLI mode — no UI attached, terminate the event loop immediately.
                    for arg in using.iter().take(2) {
                        let e = call_arg_expr(arg);
                        let n = self.expr_to_name(e);
                        self.env.set_str(&n, "");
                    }
                    self.env.set_str("COBOL-QUIT", "1");
                }
            }

            // COBOL-SET-PROPERTY obj prop value
            "COBOL-SET-PROPERTY" | "COBOLT-SET-PROPERTY" if using.len() >= 3 => {
                let obj  = self.eval_call_arg(&using[0], span)?.as_display_string();
                let prop = self.eval_call_arg(&using[1], span)?.as_display_string();
                let val  = self.eval_call_arg(&using[2], span)?.as_display_string();
                let obj_t  = obj.trim().to_owned();
                let prop_t = prop.trim().to_owned();
                let val_t  = val.trim().to_owned();
                self.objects.set_property(&obj_t, &prop_t, val_t.clone());
                // GUI mode: notify the UI thread so the form window updates.
                if let Some(tx) = &self.state_tx {
                    let _ = tx.send(StateUpdate::new(obj_t, prop_t, val_t));
                }
            }

            // COBOL-GET-PROPERTY obj prop dest
            "COBOL-GET-PROPERTY" | "COBOLT-GET-PROPERTY" if using.len() >= 3 => {
                let obj  = self.eval_call_arg(&using[0], span)?.as_display_string();
                let prop = self.eval_call_arg(&using[1], span)?.as_display_string();
                if let Some(pv) = self.objects.get_property(obj.trim(), prop.trim()) {
                    let val_s = pv.to_string();
                    let n = self.expr_to_name(call_arg_expr(&using[2]));
                    self.env.set_str(&n, &val_s);
                }
            }

            // ── Text file output ──────────────────────────────────────────────
            //
            // COBOL-APPEND-FILE USING path text [status]
            //   Append `text` followed by a newline to the file at `path`
            //   (creating it if it does not exist). Optional `status` receives
            //   "" on success or an error message on failure.
            //
            // COBOL-WRITE-FILE  USING path text [status]
            //   Same, but truncates/overwrites the file first (use to (re)write a
            //   header line).
            "COBOL-APPEND-FILE" | "COBOLT-APPEND-FILE"
            | "COBOL-WRITE-FILE" | "COBOLT-WRITE-FILE" if using.len() >= 2 => {
                use std::io::Write as _;
                let append = prog_name.contains("APPEND");
                let path = self.eval_call_arg(&using[0], span)?.as_display_string();
                let text = self.eval_call_arg(&using[1], span)?.as_display_string();
                // COBOL fixed-length fields are space-padded; trim the trailing
                // padding so files don't accumulate runs of spaces.
                let path = path.trim().to_owned();
                let text = text.trim_end().to_owned();

                let result = std::fs::OpenOptions::new()
                    .create(true)
                    .append(append)
                    .write(true)
                    .truncate(!append)
                    .open(&path)
                    .and_then(|mut f| writeln!(f, "{text}"));

                if using.len() >= 3 {
                    let n = self.expr_to_name(call_arg_expr(&using[2]));
                    match &result {
                        Ok(()) => self.env.set_str(&n, ""),
                        Err(e) => self.env.set_str(&n, &e.to_string()),
                    }
                }
                if let Err(e) = result {
                    tracing::warn!("{prog_name} failed for '{path}': {e}");
                }
            }

            // ── Chart runtime calls (stubs — real rendering is in the GUI) ────
            // COBOL-CHART-SET-TABLE chart-id table count
            "COBOL-CHART-SET-TABLE" => {
                tracing::debug!(
                    "COBOL-CHART-SET-TABLE: chart='{}' (CLI mode — rendering skipped)",
                    using.first().map(|a| {
                        self.eval_call_arg(a, span)
                            .map(|v| v.as_display_string())
                            .unwrap_or_default()
                    }).unwrap_or_default()
                );
            }
            // COBOL-CHART-ADD-POINT chart-id label value
            "COBOL-CHART-ADD-POINT" => {
                tracing::debug!("COBOL-CHART-ADD-POINT: CLI mode — skipped");
            }
            // COBOL-CHART-CLEAR chart-id
            "COBOL-CHART-CLEAR" => {
                tracing::debug!("COBOL-CHART-CLEAR: CLI mode — skipped");
            }
            // COBOL-CHART-REFRESH chart-id
            "COBOL-CHART-REFRESH" => {
                tracing::debug!("COBOL-CHART-REFRESH: CLI mode — skipped");
            }
            // ── Database Runtime Engine (Phase 8) — SQLite built-ins ──────────
            //
            // COBOL-OPEN-DB   USING conn-string-var, handle-var, status-var
            //   Opens a SQLite connection. Stores the integer handle in
            //   handle-var (PIC 9(9)) and clears status-var on success, or
            //   writes an error message into status-var on failure.
            "COBOL-OPEN-DB" if using.len() >= 3 => {
                let conn_str = self.eval_call_arg(&using[0], span)?.as_display_string();
                let conn_str = conn_str.trim().to_owned();
                let handle_name  = self.expr_to_name(call_arg_expr(&using[1]));
                let status_name  = self.expr_to_name(call_arg_expr(&using[2]));
                match self.db.open(&conn_str) {
                    Ok(h) => {
                        self.env.set(&handle_name, CobolValue::from_i64(h as i64));
                        self.env.set_str(&status_name, "");
                    }
                    Err(e) => {
                        self.env.set(&handle_name, CobolValue::from_i64(0));
                        self.env.set_str(&status_name, &e);
                        tracing::warn!("COBOL-OPEN-DB failed: {e}");
                    }
                }
            }

            // COBOL-EXEC-SQL  USING handle-var, query-var, row-count-var, status-var
            //   Execute the SQL in query-var on the connection identified by
            //   handle-var.  Stores row / affected count in row-count-var.
            "COBOL-EXEC-SQL" if using.len() >= 4 => {
                let handle = self.eval_call_arg(&using[0], span)?.as_i64().unwrap_or(0) as u32;
                let query  = self.eval_call_arg(&using[1], span)?.as_display_string();
                let query  = query.trim().to_owned();
                let count_name  = self.expr_to_name(call_arg_expr(&using[2]));
                let status_name = self.expr_to_name(call_arg_expr(&using[3]));
                match self.db.exec(handle, &query) {
                    Ok(n) => {
                        self.env.set(&count_name, CobolValue::from_i64(n as i64));
                        self.env.set_str(&status_name, "");
                    }
                    Err(e) => {
                        self.env.set(&count_name, CobolValue::from_i64(0));
                        self.env.set_str(&status_name, &e);
                        tracing::warn!("COBOL-EXEC-SQL failed: {e}");
                    }
                }
            }

            // COBOL-FETCH-ROW USING handle-var, col-index-var, dest-var, status-var
            //   Reads column col-index (1-based) of the current row into dest-var.
            //   status-var is cleared on success or contains an error.
            "COBOL-FETCH-ROW" if using.len() >= 4 => {
                let handle  = self.eval_call_arg(&using[0], span)?.as_i64().unwrap_or(0) as u32;
                let col_idx = self.eval_call_arg(&using[1], span)?.as_i64().unwrap_or(1) as usize;
                let dest_name   = self.expr_to_name(call_arg_expr(&using[2]));
                let status_name = self.expr_to_name(call_arg_expr(&using[3]));
                if handle == 0 || self.db.is_exhausted(handle) {
                    self.env.set_str(&dest_name, "");
                    self.env.set_str(&status_name, "No current row");
                } else {
                    let val = self.db.fetch_col(handle, col_idx);
                    self.env.set_str(&dest_name, &val);
                    self.env.set_str(&status_name, "");
                }
            }

            // COBOL-NEXT-ROW  USING handle-var, more-flag-var
            //   Advances the cursor.  Sets more-flag-var to 'Y' if another
            //   row exists, or 'N' when the result set is exhausted.
            "COBOL-NEXT-ROW" if using.len() >= 2 => {
                let handle    = self.eval_call_arg(&using[0], span)?.as_i64().unwrap_or(0) as u32;
                let flag_name = self.expr_to_name(call_arg_expr(&using[1]));
                let has_more  = self.db.next_row(handle);
                self.env.set_str(&flag_name, if has_more { "Y" } else { "N" });
            }

            // COBOL-ROW-COUNT USING handle-var, count-var
            //   Stores the total number of rows in the last result set.
            "COBOL-ROW-COUNT" if using.len() >= 2 => {
                let handle     = self.eval_call_arg(&using[0], span)?.as_i64().unwrap_or(0) as u32;
                let count_name = self.expr_to_name(call_arg_expr(&using[1]));
                let n = self.db.row_count(handle);
                self.env.set(&count_name, CobolValue::from_i64(n as i64));
            }

            // COBOL-CLOSE-DB  USING handle-var
            //   Closes the connection identified by handle-var and frees
            //   resources.  Silently ignores unknown handles.
            "COBOL-CLOSE-DB" if !using.is_empty() => {
                let handle = self.eval_call_arg(&using[0], span)?.as_i64().unwrap_or(0) as u32;
                self.db.close(handle);
            }

            // ── Phase 10: HTTP REST client built-in CALLs ─────────────────────
            //
            // COBOL-HTTP-GET   USING url-var, response-var, status-var
            //   Performs an HTTP GET.  Writes the response body into response-var
            //   and the numeric status code (200, 404, …) into status-var.
            //   On network error status-var is set to 0.
            "COBOL-HTTP-GET" if using.len() >= 3 => {
                let url  = self.eval_call_arg(&using[0], span)?.as_display_string();
                let resp_name   = self.expr_to_name(call_arg_expr(&using[1]));
                let status_name = self.expr_to_name(call_arg_expr(&using[2]));
                let (body, status) = self.http.get(url.trim());
                self.env.set_str(&resp_name, &body);
                self.env.set(&status_name, CobolValue::from_i64(status as i64));
            }

            // COBOL-HTTP-POST  USING url-var, body-var, response-var, status-var
            //   Performs an HTTP POST with body-var as the request body.
            //   Content-Type defaults to application/json.
            "COBOL-HTTP-POST" if using.len() >= 4 => {
                let url  = self.eval_call_arg(&using[0], span)?.as_display_string();
                let body = self.eval_call_arg(&using[1], span)?.as_display_string();
                let resp_name   = self.expr_to_name(call_arg_expr(&using[2]));
                let status_name = self.expr_to_name(call_arg_expr(&using[3]));
                let (resp, status) = self.http.post(url.trim(), body.trim());
                self.env.set_str(&resp_name, &resp);
                self.env.set(&status_name, CobolValue::from_i64(status as i64));
            }

            // COBOL-HTTP-PUT   USING url-var, body-var, response-var, status-var
            "COBOL-HTTP-PUT" if using.len() >= 4 => {
                let url  = self.eval_call_arg(&using[0], span)?.as_display_string();
                let body = self.eval_call_arg(&using[1], span)?.as_display_string();
                let resp_name   = self.expr_to_name(call_arg_expr(&using[2]));
                let status_name = self.expr_to_name(call_arg_expr(&using[3]));
                let (resp, status) = self.http.put(url.trim(), body.trim());
                self.env.set_str(&resp_name, &resp);
                self.env.set(&status_name, CobolValue::from_i64(status as i64));
            }

            // COBOL-HTTP-DELETE  USING url-var, response-var, status-var
            "COBOL-HTTP-DELETE" if using.len() >= 3 => {
                let url = self.eval_call_arg(&using[0], span)?.as_display_string();
                let resp_name   = self.expr_to_name(call_arg_expr(&using[1]));
                let status_name = self.expr_to_name(call_arg_expr(&using[2]));
                let (resp, status) = self.http.delete(url.trim());
                self.env.set_str(&resp_name, &resp);
                self.env.set(&status_name, CobolValue::from_i64(status as i64));
            }

            // COBOL-HTTP-SET-HEADER  USING name-var, value-var
            //   Adds / overwrites a persistent request header sent on every
            //   subsequent COBOL-HTTP-GET / POST / PUT / DELETE call.
            "COBOL-HTTP-SET-HEADER" if using.len() >= 2 => {
                let name  = self.eval_call_arg(&using[0], span)?.as_display_string();
                let value = self.eval_call_arg(&using[1], span)?.as_display_string();
                self.http.set_header(name.trim(), value.trim());
            }

            // COBOL-HTTP-CLEAR-HEADERS  (no arguments)
            //   Removes all persistent request headers.
            "COBOL-HTTP-CLEAR-HEADERS" => {
                self.http.clear_headers();
            }

            // ── COBOL-85 nested program CALL ──────────────────────────────────
            _ if self.nested_registry.contains_key(&prog_name) => {
                // Clone the para_map, para_order, and local_items out of the
                // registry before taking any mutable borrows on self.
                let (para_map, para_order, local_items) = {
                    let np = &self.nested_registry[&prog_name];
                    (np.para_map.clone(), np.para_order.clone(), np.local_items.clone())
                };

                // Push the nested program's local WS items into the shared env.
                // GLOBAL items from the outer program are already there and are
                // NOT overwritten, so nested programs see them naturally.
                let inserted_keys = self.env.push_local_scope(&local_items);

                // Run the nested program's paragraphs in declaration order.
                let result = self.run_para_sequence(&para_map, &para_order);

                // Remove the nested program's local items regardless of outcome.
                self.env.pop_local_scope(&inserted_keys);

                match result {
                    Ok(()) | Err(RuntimeError::GoBack) => {} // GOBACK = normal return
                    Err(e) => return Err(e),
                }
            }

            // ── Internal paragraph CALL (flat / legacy programs) ──────────────
            _ => {
                if self.para_map.contains_key(&prog_name) {
                    let stmts = self.para_map[&prog_name].clone();
                    match self.exec_stmts(&stmts) {
                        Err(RuntimeError::GoBack) => {} // normal sub-program return
                        other => other?,
                    }
                } else {
                    tracing::warn!("CALL to unknown program '{}' — ignored", prog_name);
                }
            }
        }
        Ok(())
    }

    fn eval_call_arg(&mut self, arg: &CallArg, span: Span) -> Result<CobolValue, RuntimeError> {
        self.eval_expr(call_arg_expr(arg), span)
    }

    // ── Expression evaluation ─────────────────────────────────────────────────

    /// Evaluate an expression to a `CobolValue`.
    pub fn eval_expr(&mut self, expr: &Expr, span: Span) -> Result<CobolValue, RuntimeError> {
        match expr {
            Expr::Literal(lit, _) => Ok(literal_to_value(lit)),

            Expr::Identifier(name, _) => {
                let upper = name.to_ascii_uppercase();
                Ok(self.env.get(&upper).cloned().unwrap_or_else(|| {
                    tracing::debug!("Identifier '{upper}' not found in environment — using 0");
                    CobolValue::from_i64(0)
                }))
            }

            Expr::Qualified { name, .. } => {
                // Simplified: ignore the OF/IN qualifier; look up by name alone.
                let upper = name.to_ascii_uppercase();
                Ok(self.env.get(&upper).cloned().unwrap_or(CobolValue::from_i64(0)))
            }

            Expr::Subscript { base, .. } => {
                // TODO: full table subscript support.
                self.eval_expr(base, span)
            }

            Expr::FunctionCall { name, args, span: s } =>
                self.eval_function(name, args, *s),

            Expr::Arithmetic { op, lhs, rhs, span: s } => {
                let l = self.eval_expr(lhs, *s)?;
                let r = self.eval_expr(rhs, *s)?;
                let result = match op {
                    ArithOp::Add => l.add_val(&r),
                    ArithOp::Sub => l.sub_val(&r),
                    ArithOp::Mul => l.mul_val(&r),
                    ArithOp::Div => l.div_val(&r)
                        .ok_or(RuntimeError::DivisionByZero { span: *s })?,
                    // Exponentiation is inherently floating-point.
                    ArithOp::Pow => CobolValue::from_f64(l.as_f64().powf(r.as_f64())),
                };
                Ok(result)
            }

            Expr::Unary { op, operand, span: s } => {
                let v = self.eval_expr(operand, *s)?;
                Ok(match op {
                    // 0 − v keeps exact decimals; Pos is a no-op.
                    UnaryOp::Neg => CobolValue::from_i64(0).sub_val(&v),
                    UnaryOp::Pos => v,
                })
            }
        }
    }

    // ── Intrinsic functions ───────────────────────────────────────────────────

    fn eval_function(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<CobolValue, RuntimeError> {
        match name.to_ascii_uppercase().as_str() {
            "LENGTH" => {
                let v = self.eval_expr(&args[0], span)?;
                let len = match &v {
                    CobolValue::String { bytes, .. } => bytes.len(),
                    _ => v.as_display_string().len(),
                };
                Ok(CobolValue::from_i64(len as i64))
            }
            "UPPER-CASE" => {
                let s = self.eval_expr(&args[0], span)?.as_display_string()
                    .to_ascii_uppercase();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            "LOWER-CASE" => {
                let s = self.eval_expr(&args[0], span)?.as_display_string()
                    .to_ascii_lowercase();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            "NUMVAL" | "NUMVAL-C" => {
                let s = self.eval_expr(&args[0], span)?.as_display_string();
                let f: f64 = s.trim()
                    .replace(',', "")
                    .replace('$', "")
                    .replace('£', "")
                    .parse()
                    .unwrap_or(0.0);
                Ok(CobolValue::from_f64(f))
            }
            "MAX" => {
                let vals = self.eval_args(args, span)?;
                let max = vals.iter().map(|v| v.as_f64()).fold(f64::NEG_INFINITY, f64::max);
                Ok(CobolValue::from_f64(max))
            }
            "MIN" => {
                let vals = self.eval_args(args, span)?;
                let min = vals.iter().map(|v| v.as_f64()).fold(f64::INFINITY, f64::min);
                Ok(CobolValue::from_f64(min))
            }
            "SQRT" => {
                let v = self.eval_expr(&args[0], span)?.as_f64();
                Ok(CobolValue::from_f64(v.sqrt()))
            }
            "MOD" => {
                let a = self.eval_expr(&args[0], span)?.as_f64();
                let b = self.eval_expr(&args[1], span)?.as_f64();
                if b == 0.0 { return Err(RuntimeError::DivisionByZero { span }); }
                Ok(CobolValue::from_f64(a - (a / b).floor() * b))
            }
            "REM" => {
                let a = self.eval_expr(&args[0], span)?.as_f64();
                let b = self.eval_expr(&args[1], span)?.as_f64();
                if b == 0.0 { return Err(RuntimeError::DivisionByZero { span }); }
                Ok(CobolValue::from_f64(a - (a / b).trunc() * b))
            }
            "ABS" => {
                let v = self.eval_expr(&args[0], span)?.as_f64();
                Ok(CobolValue::from_f64(v.abs()))
            }
            "INTEGER" => {
                let v = self.eval_expr(&args[0], span)?.as_f64();
                Ok(CobolValue::from_i64(v.floor() as i64))
            }
            "INTEGER-PART" => {
                let v = self.eval_expr(&args[0], span)?.as_f64();
                Ok(CobolValue::from_i64(v.trunc() as i64))
            }
            "RANDOM" => Ok(CobolValue::from_f64(pseudo_random())),
            "CURRENT-DATE" => {
                let s = current_date_string();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            "TRIM" => {
                let s = self.eval_expr(&args[0], span)?.as_display_string()
                    .trim().to_owned();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            "REVERSE" => {
                let s: String = self.eval_expr(&args[0], span)?.as_display_string()
                    .chars().rev().collect();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            "CONCATENATE" => {
                let vals = self.eval_args(args, span)?;
                let s: String = vals.iter().map(|v| v.as_display_string()).collect();
                let len = s.len();
                Ok(CobolValue::from_str(&s, len))
            }
            _ => {
                tracing::warn!("Unknown intrinsic function '{}' — returning 0", name);
                Ok(CobolValue::from_i64(0))
            }
        }
    }

    fn eval_args(&mut self, args: &[Expr], span: Span) -> Result<Vec<CobolValue>, RuntimeError> {
        let mut out = Vec::with_capacity(args.len());
        for a in args {
            out.push(self.eval_expr(a, span)?);
        }
        Ok(out)
    }

    // ── Condition evaluation ──────────────────────────────────────────────────

    /// Evaluate a boolean condition.
    pub fn eval_condition(&mut self, cond: &Condition) -> Result<bool, RuntimeError> {
        match cond {
            Condition::Comparison { lhs, op, rhs, span } => {
                let l = self.eval_expr(lhs, *span)?;
                let r = self.eval_expr(rhs, *span)?;
                Ok(compare_values(&l, &r, *op))
            }
            Condition::Not(inner, _) => Ok(!self.eval_condition(inner)?),
            Condition::And(a, b, _)  => Ok(self.eval_condition(a)? && self.eval_condition(b)?),
            Condition::Or(a, b, _)   => Ok(self.eval_condition(a)? || self.eval_condition(b)?),
            Condition::ClassTest { expr, negated, class, span } => {
                let v = self.eval_expr(expr, *span)?;
                let s = v.as_display_string();
                let result = match class {
                    DataClass::Numeric => v.is_numeric()
                        || s.trim().parse::<f64>().is_ok(),
                    DataClass::Alphabetic =>
                        s.chars().all(|c| c.is_ascii_alphabetic() || c == ' '),
                    DataClass::AlphabeticLower =>
                        s.chars().all(|c| c.is_ascii_lowercase() || c == ' '),
                    DataClass::AlphabeticUpper =>
                        s.chars().all(|c| c.is_ascii_uppercase() || c == ' '),
                };
                Ok(if *negated { !result } else { result })
            }
            Condition::SignTest { expr, negated, sign, span } => {
                let v = self.eval_expr(expr, *span)?.as_f64();
                let result = match sign {
                    SignCond::Positive => v > 0.0,
                    SignCond::Negative => v < 0.0,
                    SignCond::Zero     => v == 0.0,
                };
                Ok(if *negated { !result } else { result })
            }
            Condition::ConditionName(name, _) => {
                // 88-level condition-names: the value is truthy if non-zero/non-space.
                let upper = name.to_ascii_uppercase();
                let v = self.env.get(&upper).cloned()
                    .unwrap_or_else(|| CobolValue::from_i64(0));
                Ok(!v.is_zero())
            }
        }
    }

    // ── Paragraph / section helpers ───────────────────────────────────────────

    fn para_stmts(&self, name: &str, span: Span) -> Result<Vec<Stmt>, RuntimeError> {
        let upper = name.to_ascii_uppercase();
        self.para_map.get(&upper)
            .cloned()
            .ok_or(RuntimeError::UndefinedParagraph { name: upper, span })
    }

    /// Collect all paragraphs that belong to a section (identified by
    /// consecutive paragraphs after the named entry in `para_order`).
    fn collect_section_stmts(&self, section_upper: &str) -> Vec<Stmt> {
        let mut found = false;
        let mut result = Vec::new();
        for name in &self.para_order {
            if name == section_upper {
                found = true;
                continue;
            }
            if found {
                // Stop at the next section marker (paragraphs inside a section
                // are typically named SECTION-name-PARAGRAPH-name or just
                // listed consecutively; we collect until end for simplicity).
                if let Some(stmts) = self.para_map.get(name) {
                    result.extend_from_slice(stmts);
                }
            }
        }
        result
    }

    fn thru_stmts(&self, from: &str, to: &str, span: Span) -> Result<Vec<Stmt>, RuntimeError> {
        let from_u = from.to_ascii_uppercase();
        let to_u   = to.to_ascii_uppercase();
        let from_pos = self.para_order.iter().position(|n| n == &from_u)
            .ok_or_else(|| RuntimeError::UndefinedParagraph { name: from_u.clone(), span })?;
        let to_pos = self.para_order.iter().position(|n| n == &to_u)
            .ok_or_else(|| RuntimeError::UndefinedParagraph { name: to_u.clone(), span })?;

        let mut stmts = Vec::new();
        for i in from_pos..=to_pos {
            if let Some(ps) = self.para_map.get(&self.para_order[i]) {
                stmts.extend_from_slice(ps);
            }
        }
        Ok(stmts)
    }

    /// Extract the target name from an lvalue expression.
    fn expr_to_name(&self, expr: &Expr) -> String {
        match expr {
            Expr::Identifier(name, _)  => name.to_ascii_uppercase(),
            Expr::Qualified { name, .. } => name.to_ascii_uppercase(),
            Expr::Subscript { base, .. } => self.expr_to_name(base),
            _ => "__UNKNOWN__".to_owned(),
        }
    }
}

// ── Debugger span extractor ───────────────────────────────────────────────────

/// Return the source span of a statement as `Some(span)`.
///
/// Delegates to `Stmt::span()` which covers every variant.
#[inline]
fn stmt_span(stmt: &Stmt) -> Option<Span> {
    Some(stmt.span())
}

// ── Paragraph map builder ─────────────────────────────────────────────────────

fn build_para_map(body: &ProcedureBody) -> (IndexMap<String, Vec<Stmt>>, Vec<String>) {
    let mut map:   IndexMap<String, Vec<Stmt>> = IndexMap::new();
    let mut order: Vec<String>                 = Vec::new();

    match body {
        ProcedureBody::Paragraphs(paras) => {
            for para in paras {
                let key = para.name.to_ascii_uppercase();
                order.push(key.clone());
                map.insert(key, para.stmts.clone());
            }
        }
        ProcedureBody::Sections(sections) => {
            for section in sections {
                // Optionally register the section name itself as an entry.
                let sec_key = section.name.to_ascii_uppercase();
                order.push(sec_key.clone());
                map.insert(sec_key, Vec::new()); // empty placeholder
                for para in &section.paragraphs {
                    let key = para.name.to_ascii_uppercase();
                    order.push(key.clone());
                    map.insert(key, para.stmts.clone());
                }
            }
        }
    }
    (map, order)
}

// ── Free functions ────────────────────────────────────────────────────────────

/// Convert an AST literal to a runtime `CobolValue`.
pub fn literal_to_value(lit: &Literal) -> CobolValue {
    match lit {
        Literal::Integer(n) => CobolValue::from_i64(*n),
        Literal::Float(f)   => CobolValue::from_f64(*f),
        Literal::Decimal(m, s) => CobolValue::Numeric(CobolNumeric::new(*m, *s)),
        Literal::String(s)  => CobolValue::from_str(s, s.len()),
        Literal::Figurative(fig) => match fig {
            FigurativeConstant::Zero     => CobolValue::from_i64(0),
            FigurativeConstant::Space    => CobolValue::spaces(1),
            FigurativeConstant::HighValue => CobolValue::figurative_high_values(1),
            FigurativeConstant::LowValue  => CobolValue::figurative_low_values(1),
            FigurativeConstant::Quote    => CobolValue::from_str("\"", 1),
            FigurativeConstant::Null     => CobolValue::from_i64(0),
            FigurativeConstant::All(inner) => literal_to_value(inner),
        },
    }
}

/// Compare two `CobolValue`s using the given operator.
pub fn compare_values(l: &CobolValue, r: &CobolValue, op: CmpOp) -> bool {
    // Numeric comparison when both sides are numeric.
    if l.is_numeric() && r.is_numeric() {
        // Exact integer comparison when both are fixed-point decimals.
        if let (Some(a), Some(b)) = (l.as_exact(), r.as_exact()) {
            use std::cmp::Ordering;
            let ord = a.cmp(&b);
            return match op {
                CmpOp::Eq => ord == Ordering::Equal,
                CmpOp::Ne => ord != Ordering::Equal,
                CmpOp::Lt => ord == Ordering::Less,
                CmpOp::Le => ord != Ordering::Greater,
                CmpOp::Gt => ord == Ordering::Greater,
                CmpOp::Ge => ord != Ordering::Less,
            };
        }
        let lf = l.as_f64();
        let rf = r.as_f64();
        return match op {
            CmpOp::Eq => (lf - rf).abs() < 1e-10,
            CmpOp::Ne => (lf - rf).abs() >= 1e-10,
            CmpOp::Lt => lf < rf,
            CmpOp::Le => lf <= rf,
            CmpOp::Gt => lf > rf,
            CmpOp::Ge => lf >= rf,
        };
    }
    // Cross-type: numeric vs string — compare as f64 if parsable, else string.
    if l.is_numeric() || r.is_numeric() {
        let lf = l.as_f64();
        let rf = r.as_f64();
        return match op {
            CmpOp::Eq => (lf - rf).abs() < 1e-10,
            CmpOp::Ne => (lf - rf).abs() >= 1e-10,
            CmpOp::Lt => lf < rf,
            CmpOp::Le => lf <= rf,
            CmpOp::Gt => lf > rf,
            CmpOp::Ge => lf >= rf,
        };
    }
    // Alphanumeric comparison. Per COBOL rules the shorter operand is padded on
    // the RIGHT with spaces to the length of the longer one, then compared
    // byte-by-byte. This makes e.g. `"BTN-OK"` (a literal) equal to a `PIC X(64)`
    // field holding "BTN-OK" followed by trailing spaces.
    let ls = l.as_display_string();
    let rs = r.as_display_string();
    let width = ls.len().max(rs.len());
    let lp = format!("{ls:<width$}");
    let rp = format!("{rs:<width$}");
    match op {
        CmpOp::Eq => lp == rp,
        CmpOp::Ne => lp != rp,
        CmpOp::Lt => lp < rp,
        CmpOp::Le => lp <= rp,
        CmpOp::Gt => lp > rp,
        CmpOp::Ge => lp >= rp,
    }
}

/// Extract the expression from a `CallArg`.
fn call_arg_expr(arg: &CallArg) -> &Expr {
    match arg {
        CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e,
    }
}

// ── Date / time utilities (no external crate dependency) ─────────────────────

/// Return the current date as `YYYYMMDD` (8 chars).
fn runtime_date() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86400;
    days_to_ymd(days)
}

/// Return the current time as `HHMMSScc` (8 chars, cc = centiseconds).
fn runtime_time() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600)  / 60;
    let s = secs % 60;
    format!("{h:02}{m:02}{s:02}00")
}

/// Return Julian day as `YYDDD` (5 chars).
fn runtime_julian_day() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86400;
    let (y, _, _) = ymd_from_days(days);
    let jan1 = days_since_epoch_jan1(y);
    let doy = days - jan1 + 1;
    format!("{:02}{:03}", y % 100, doy)
}

/// Return day-of-week as `i64`: 1 = Monday … 7 = Sunday (ISO 8601).
fn runtime_day_of_week() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86400;
    // 1970-01-01 was a Thursday → day 4
    ((days + 3) % 7 + 1) as i64
}

/// Return a 21-char CURRENT-DATE string: `YYYYMMDDHHMMSSCC+HHMM`.
fn current_date_string() -> String {
    format!("{}{}-0000", runtime_date(), runtime_time())
}

// ── Simple calendar arithmetic (no external crate) ────────────────────────────

fn is_leap(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn days_in_year(year: u64) -> u64 {
    if is_leap(year) { 366 } else { 365 }
}

/// Days since Unix epoch for January 1 of `year`.
fn days_since_epoch_jan1(year: u64) -> u64 {
    let mut d = 0u64;
    let mut y = 1970u64;
    while y < year {
        d += days_in_year(y);
        y += 1;
    }
    d
}

/// Convert days-since-epoch to (year, month, day).
fn ymd_from_days(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let dy = days_in_year(year);
        if days < dy { break; }
        days -= dy;
        year += 1;
    }
    let month_days: [u64; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    for md in &month_days {
        if days < *md { break; }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

fn days_to_ymd(days: u64) -> String {
    let (y, m, d) = ymd_from_days(days);
    format!("{y:04}{m:02}{d:02}")
}

// ── Pseudo-random number generator ───────────────────────────────────────────

static RANDOM_STATE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(6364136223846793005);

/// Return a pseudo-random `f64` in [0, 1).
fn pseudo_random() -> f64 {
    use std::sync::atomic::Ordering;
    let s = RANDOM_STATE.load(Ordering::Relaxed);
    let next = s
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    RANDOM_STATE.store(next, Ordering::Relaxed);
    // Top 53 bits → double in [0, 1)
    (next >> 11) as f64 / (1u64 << 53) as f64
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_ast::expr::{CmpOp, Literal};

    #[test]
    fn compare_integers() {
        let a = CobolValue::from_i64(10);
        let b = CobolValue::from_i64(5);
        assert!(compare_values(&a, &b, CmpOp::Gt));
        assert!(compare_values(&b, &a, CmpOp::Lt));
        assert!(compare_values(&a, &a, CmpOp::Eq));
    }

    #[test]
    fn compare_strings() {
        let a = CobolValue::from_str("ALPHA", 5);
        let b = CobolValue::from_str("BETA",  4);
        assert!(compare_values(&a, &b, CmpOp::Lt));
        assert!(compare_values(&a, &a, CmpOp::Eq));
    }

    #[test]
    fn literal_to_value_integer() {
        let v = literal_to_value(&Literal::Integer(42));
        assert_eq!(v.as_i64(), Some(42));
    }

    #[test]
    fn literal_to_value_string() {
        let v = literal_to_value(&Literal::String("HELLO".to_owned()));
        assert_eq!(v.as_display_string(), "HELLO");
    }

    #[test]
    fn ymd_epoch() {
        // Unix epoch = 1970-01-01
        assert_eq!(days_to_ymd(0), "19700101");
    }

    #[test]
    fn ymd_known_date() {
        // 2024-01-01: 54 years since 1970 (with leap years)
        let d = days_to_ymd(19723); // 19723 days = 2024-01-01
        assert!(d.starts_with("202"), "got {d}");
    }
}
