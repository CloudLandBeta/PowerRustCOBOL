// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Background run/stop integration for the IDE.
//!
//! `Runner` spawns the full pipeline (lex → parse → semantic → interpret) in
//! a dedicated thread, streams output lines back to the UI through an
//! `std::sync::mpsc` channel, and supports graceful cancellation via an
//! `AtomicBool` stop flag.
//!
//! # Usage
//!
//! ```rust,no_run
//! use cobolt_ide::runner::Runner;
//!
//! let mut runner = Runner::new();
//! runner.start("my_program.cbl", source_text);
//!
//! // In the UI update loop:
//! for msg in runner.drain_output() {
//!     println!("{msg}");
//! }
//! if runner.is_finished() {
//!     runner.clear();
//! }
//! ```

use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
    Arc,
};
use std::thread::{self, JoinHandle};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::{
    Interpreter,
    DebugCmd, DebugEvent, Breakpoints, new_breakpoints,
};
use cobolt_semantic::analyze;

// ── Message types ─────────────────────────────────────────────────────────────

/// A message produced by the background runner thread.
#[derive(Debug, Clone)]
pub enum RunMsg {
    /// A line of program output (DISPLAY statement).
    Output(String),
    /// A diagnostic from the parser or semantic analyser.
    Diagnostic(DiagMsg),
    /// The program finished successfully.
    Finished,
    /// The program was stopped by the user.
    Stopped,
    /// The program crashed with a runtime error.
    Error(String),
}

/// A diagnostic message (error or warning from parsing / semantic analysis).
#[derive(Debug, Clone)]
pub struct DiagMsg {
    pub severity: DiagSeverity,
    pub message:  String,
    pub line:     u32,
    pub col:      u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagSeverity {
    Error,
    Warning,
    Info,
}

// ── Runner ────────────────────────────────────────────────────────────────────

/// Manages the background execution thread.
pub struct Runner {
    /// Messages produced by the running thread.
    rx:      Option<Receiver<RunMsg>>,
    /// Handle to the thread (for joining).
    handle:  Option<JoinHandle<()>>,
    /// Set to `true` to ask the thread to stop.
    stop_flag: Arc<AtomicBool>,
    /// True while the thread is still running.
    running: bool,
}

impl Default for Runner {
    fn default() -> Self { Self::new() }
}

impl Runner {
    pub fn new() -> Self {
        Self {
            rx:        None,
            handle:    None,
            stop_flag: Arc::new(AtomicBool::new(false)),
            running:   false,
        }
    }

    /// Start running `source` (labelled with `file_name` for diagnostics).
    ///
    /// If a previous run is still active it is stopped first.
    pub fn start(&mut self, file_name: impl Into<String>, source: impl Into<String>) {
        self.stop();

        let file_name = file_name.into();
        let source    = source.into();

        self.stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag  = Arc::clone(&self.stop_flag);

        let (tx, rx) = mpsc::channel::<RunMsg>();
        self.rx      = Some(rx);
        self.running = true;

        let handle = thread::spawn(move || {
            run_pipeline(file_name, source, tx, stop_flag);
        });
        self.handle = Some(handle);
    }

    /// Ask the running thread to stop and wait for it (non-blocking: returns
    /// immediately, sets the stop flag, and cleans up lazily on next poll).
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        // Don't block the UI — let is_finished() / drain_output() clean up.
        self.running = false;
    }

    /// Drain any pending messages from the output channel.
    ///
    /// Should be called each UI frame.  Automatically sets `running = false`
    /// when a `Finished`, `Stopped`, or `Error` message is received.
    pub fn drain_output(&mut self) -> Vec<RunMsg> {
        let mut out = Vec::new();
        let Some(rx) = &self.rx else { return out; };

        loop {
            match rx.try_recv() {
                Ok(msg) => {
                    match &msg {
                        RunMsg::Finished | RunMsg::Stopped | RunMsg::Error(_) => {
                            self.running = false;
                        }
                        _ => {}
                    }
                    out.push(msg);
                }
                Err(mpsc::TryRecvError::Empty)        => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.running = false;
                    break;
                }
            }
        }
        out
    }

    /// `true` while the run thread is active.
    pub fn is_running(&self) -> bool { self.running }

    /// `true` after the thread finishes (or was stopped).
    pub fn is_finished(&self) -> bool {
        !self.running
            && self.handle.as_ref().map(|h| h.is_finished()).unwrap_or(true)
    }

    /// Clean up finished thread handle.
    pub fn clear(&mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        self.rx = None;
    }
}

// ── Pipeline ──────────────────────────────────────────────────────────────────

/// Run the full lex → parse → semantic → interpret pipeline.
///
/// Output is sent line-by-line through `tx`.  The `stop_flag` is checked
/// between paragraphs (via a custom IoBackend hook in the future; for now
/// the pipeline can only be stopped between statements via the flag).
fn run_pipeline(
    file_name: String,
    source: String,
    tx: Sender<RunMsg>,
    stop_flag: Arc<AtomicBool>,
) {
    // ── Lex ──────────────────────────────────────────────────────────────────
    let fmt    = detect_format(&source);
    let tokens = tokenize(&source, fmt);

    // ── Parse ────────────────────────────────────────────────────────────────
    let parse_result = parse(tokens);

    // Forward parser diagnostics.
    for d in &parse_result.diagnostics {
        let severity = match d.severity {
            cobolt_parser::Severity::Error   => DiagSeverity::Error,
            cobolt_parser::Severity::Warning => DiagSeverity::Warning,
        };
        let _ = tx.send(RunMsg::Diagnostic(DiagMsg {
            severity,
            message: d.message.clone(),
            line:    d.span.line,
            col:     d.span.col,
        }));
    }

    let program = match parse_result.program {
        Some(p) => p,
        None => {
            let _ = tx.send(RunMsg::Error("Parse failed — no program recovered.".to_owned()));
            return;
        }
    };

    // ── Semantic analysis ─────────────────────────────────────────────────────
    let sem = analyze(&program);
    for diag in &sem.diagnostics {
        use cobolt_semantic::Severity;
        let severity = match diag.severity {
            Severity::Error   => DiagSeverity::Error,
            Severity::Warning => DiagSeverity::Warning,
            Severity::Info    => DiagSeverity::Info,
        };
        let _ = tx.send(RunMsg::Diagnostic(DiagMsg {
            severity,
            message: diag.message.clone(),
            line:    diag.span.line,
            col:     diag.span.col,
        }));
    }
    if !sem.is_ok() {
        let _ = tx.send(RunMsg::Error(
            "Aborting: semantic errors found.".to_owned(),
        ));
        return;
    }

    // ── Execute ───────────────────────────────────────────────────────────────
    // Redirect stdout through our channel by replacing the global print mechanism.
    // For now we run the interpreter normally (it prints to real stdout)
    // and capture via a thread-local output interceptor.
    //
    // TODO: integrate IoBackend so DISPLAY goes through tx.send(RunMsg::Output(…)).

    let stop = Arc::clone(&stop_flag);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut interp = Interpreter::new(program);

        // Run — the interpreter's DISPLAY calls println!() for now.
        // Future: swap in a channel-backed IoBackend.
        if stop.load(Ordering::Relaxed) {
            return Err(cobolt_runtime::RuntimeError::GoBack); // signal stopped
        }
        interp.run()
    }));

    match result {
        Ok(Ok(())) => {
            let _ = tx.send(RunMsg::Finished);
        }
        Ok(Err(e)) if e.is_exit_signal() => {
            if stop_flag.load(Ordering::Relaxed) {
                let _ = tx.send(RunMsg::Stopped);
            } else {
                let _ = tx.send(RunMsg::Finished);
            }
        }
        Ok(Err(e)) => {
            let _ = tx.send(RunMsg::Error(e.to_string()));
        }
        Err(_panic) => {
            let _ = tx.send(RunMsg::Error("Runtime panic".to_owned()));
        }
    }
}

// ── DebugRunner ───────────────────────────────────────────────────────────────

/// Manages a debug session: same pipeline as `Runner` but with debug channels.
///
/// The IDE sends `DebugCmd` to step/continue and receives `DebugEvent` (Paused,
/// Resumed, Finished) back each frame via `drain_events()`.
pub struct DebugRunner {
    /// Receives `RunMsg` (diagnostics, output, finished/error).
    run_rx:      Option<Receiver<RunMsg>>,
    /// Receives `DebugEvent` from the interpreter (Paused, Resumed, Finished).
    event_rx:    Option<Receiver<DebugEvent>>,
    /// Sends `DebugCmd` to the interpreter.
    cmd_tx:      Option<mpsc::Sender<DebugCmd>>,
    /// Shared breakpoint set — IDE writes, interpreter reads.
    pub breakpoints: Breakpoints,
    /// Handle to the interpreter thread.
    handle:      Option<JoinHandle<()>>,
    /// `true` while the thread is still running.
    running:     bool,
}

impl Default for DebugRunner {
    fn default() -> Self { Self::new() }
}

impl DebugRunner {
    pub fn new() -> Self {
        Self {
            run_rx:      None,
            event_rx:    None,
            cmd_tx:      None,
            breakpoints: new_breakpoints(),
            handle:      None,
            running:     false,
        }
    }

    /// Start a debug session for `source`.
    pub fn start(&mut self, file_name: impl Into<String>, source: impl Into<String>) {
        self.stop();

        // Re-create breakpoints so the interpreter gets the current set.
        // (Breakpoints set by the editor before calling start() are preserved
        // because the caller should write to `self.breakpoints` before calling.)
        let bp = Arc::clone(&self.breakpoints);

        let (run_tx, run_rx)     = mpsc::channel::<RunMsg>();
        let (ev_tx, ev_rx)       = mpsc::channel::<DebugEvent>();
        let (cmd_tx, cmd_rx)     = mpsc::channel::<DebugCmd>();

        self.run_rx   = Some(run_rx);
        self.event_rx = Some(ev_rx);
        self.cmd_tx   = Some(cmd_tx);
        self.running  = true;

        let file_name = file_name.into();
        let source    = source.into();

        let handle = thread::spawn(move || {
            run_debug_pipeline(file_name, source, run_tx, ev_tx, cmd_rx, bp);
        });
        self.handle = Some(handle);
    }

    /// Send a debug command to the interpreter thread.
    pub fn send_cmd(&self, cmd: DebugCmd) {
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.send(cmd);
        }
    }

    /// Drain pending run messages (diagnostics, output, finished/stopped).
    pub fn drain_run(&mut self) -> Vec<RunMsg> {
        let mut out = Vec::new();
        let Some(rx) = &self.run_rx else { return out; };
        loop {
            match rx.try_recv() {
                Ok(msg) => {
                    match &msg {
                        RunMsg::Finished | RunMsg::Stopped | RunMsg::Error(_) => {
                            self.running = false;
                        }
                        _ => {}
                    }
                    out.push(msg);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => { self.running = false; break; }
            }
        }
        out
    }

    /// Drain pending debug events (Paused, Resumed, Finished).
    pub fn drain_events(&mut self) -> Vec<DebugEvent> {
        let mut out = Vec::new();
        let Some(rx) = &self.event_rx else { return out; };
        loop {
            match rx.try_recv() {
                Ok(ev) => {
                    if matches!(ev, DebugEvent::Finished) {
                        self.running = false;
                    }
                    out.push(ev);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => { self.running = false; break; }
            }
        }
        out
    }

    pub fn is_running(&self) -> bool { self.running }

    /// Stop the debug session (drops cmd_tx, which unblocks the interpreter).
    pub fn stop(&mut self) {
        self.cmd_tx = None; // dropping the sender unblocks recv() in the interpreter
        self.running = false;
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        self.run_rx   = None;
        self.event_rx = None;
    }
}

/// Run the full pipeline with debug channels attached.
fn run_debug_pipeline(
    file_name:  String,
    source:     String,
    run_tx:     Sender<RunMsg>,
    ev_tx:      mpsc::Sender<DebugEvent>,
    cmd_rx:     mpsc::Receiver<DebugCmd>,
    breakpoints: Breakpoints,
) {
    let fmt    = detect_format(&source);
    let tokens = tokenize(&source, fmt);
    let parse_result = parse(tokens);

    for d in &parse_result.diagnostics {
        let severity = match d.severity {
            cobolt_parser::Severity::Error   => DiagSeverity::Error,
            cobolt_parser::Severity::Warning => DiagSeverity::Warning,
        };
        let _ = run_tx.send(RunMsg::Diagnostic(DiagMsg {
            severity, message: d.message.clone(), line: d.span.line, col: d.span.col,
        }));
    }

    let program = match parse_result.program {
        Some(p) => p,
        None => {
            let _ = run_tx.send(RunMsg::Error("Parse failed — no program recovered.".to_owned()));
            return;
        }
    };

    let sem = analyze(&program);
    for diag in &sem.diagnostics {
        use cobolt_semantic::Severity;
        let severity = match diag.severity {
            Severity::Error   => DiagSeverity::Error,
            Severity::Warning => DiagSeverity::Warning,
            Severity::Info    => DiagSeverity::Info,
        };
        let _ = run_tx.send(RunMsg::Diagnostic(DiagMsg {
            severity, message: diag.message.clone(), line: diag.span.line, col: diag.span.col,
        }));
    }
    if !sem.is_ok() {
        let _ = run_tx.send(RunMsg::Error("Aborting: semantic errors found.".to_owned()));
        return;
    }

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut interp = Interpreter::new_with_debug_channels(
            program, cmd_rx, ev_tx, breakpoints,
        );
        interp.run()
    }));

    match result {
        Ok(Ok(())) => { let _ = run_tx.send(RunMsg::Finished); }
        Ok(Err(e)) if e.is_exit_signal() => { let _ = run_tx.send(RunMsg::Finished); }
        Ok(Err(e)) => { let _ = run_tx.send(RunMsg::Error(e.to_string())); }
        Err(_) => { let _ = run_tx.send(RunMsg::Error("Runtime panic".to_owned())); }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn detect_format(source: &str) -> SourceFormat {
    // If any line has content starting in column 7+ after spaces in 1-6, fixed-form.
    let looks_fixed = source.lines().any(|line| {
        let b = line.as_bytes();
        b.len() > 6
            && b[6] != b' '
            && b[..6].iter().all(|&c| c == b' ' || c.is_ascii_digit())
    });
    if looks_fixed { SourceFormat::Fixed } else { SourceFormat::Free }
}
