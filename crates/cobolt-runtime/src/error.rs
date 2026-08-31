// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Runtime error types.

use cobolt_lexer::Span;

/// An error that occurred during COBOL program execution.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// A data item referenced in a statement was not found in the environment.
    #[error("undefined data item '{name}' at {span}")]
    UndefinedItem { name: String, span: Span },

    /// Division by zero.
    #[error("division by zero at {span}")]
    DivisionByZero { span: Span },

    /// A GO TO targeted a paragraph that doesn't exist.
    #[error("undefined paragraph '{name}' at {span}")]
    UndefinedParagraph { name: String, span: Span },

    /// PERFORM depth exceeded (stack overflow guard).
    #[error("PERFORM nesting too deep (max {max}) — possible infinite recursion")]
    PerformDepthExceeded { max: usize },

    /// STOP RUN was executed — not a real error, used as a control-flow signal.
    #[error("STOP RUN")]
    StopRun,

    /// GO BACK executed (subprogram return).
    #[error("GO BACK")]
    GoBack,

    /// The host requested cancellation (form window closed, IDE relaunching /
    /// exiting). Not a fault — a cooperative-abort signal checked between
    /// statements so a long-running or looping handler can stop promptly and
    /// never hang the UI thread. `run()` treats it like a clean exit.
    #[error("cancelled")]
    Cancelled,

    /// An EXEC RUST block failed to execute.
    #[error("EXEC RUST error at {span}: {message}")]
    ExecRustError { message: String, span: Span },

    /// An intrinsic function was called with fewer arguments than it takes.
    ///
    /// Reported rather than panicked: a COBOL program must never be able to
    /// crash the interpreter, and "FUNCTION MOD needs 2 arguments, 1 given" is
    /// something the developer can act on where an index-out-of-bounds is not.
    #[error("FUNCTION {name} at {span} needs {needed} arguments, {given} given")]
    IntrinsicArity {
        name: String,
        needed: usize,
        given: usize,
        span: Span,
    },

    /// GO TO control-flow signal — not a real error; caught by the main run loop.
    ///
    /// `section` carries a `GO TO paragraph {OF|IN} section` qualifier through
    /// to whichever loop catches the signal: every one of them resolves the
    /// name against `para_order`, which is keyed by bare name and hands back
    /// the first definition anywhere in the program.
    #[error("GO TO {target}")]
    GoTo {
        target: String,
        section: Option<String>,
    },

    /// `EXIT PERFORM [CYCLE]` — control-flow signal caught by the nearest
    /// enclosing inline PERFORM loop. `cycle` = continue to the next iteration;
    /// otherwise terminate the loop.
    #[error("EXIT PERFORM")]
    ExitPerform { cycle: bool },

    /// `EXIT PARAGRAPH` — return from the current paragraph.
    #[error("EXIT PARAGRAPH")]
    ExitParagraph,

    /// `EXIT SECTION` — return from the current section.
    #[error("EXIT SECTION")]
    ExitSection,

    /// `NEXT SENTENCE` — transfer control past the next sentence boundary.
    #[error("NEXT SENTENCE")]
    NextSentence,

    /// Arithmetic overflow.
    #[error("arithmetic overflow at {span}")]
    Overflow { span: Span },

    /// General runtime error with a free-form message.
    #[error("{message}")]
    General { message: String },

    /// A THROW / RAISE statement was executed.
    ///
    /// This is a control-flow signal; it is caught by the nearest enclosing
    /// TRY block.  If no TRY block catches it the interpreter surfaces it as
    /// an unhandled exception error.
    #[error("unhandled exception: {message}")]
    UserException { message: String },

    /// A Rust panic contained at an `EXEC RUST` block boundary (spec 041 R12).
    ///
    /// Caught **only** by `CATCH RUST-EXCEPTION` (R24). It is deliberately not a
    /// [`Self::UserException`]: a panic is a distinct failure class, and letting
    /// a plain `CATCH EXCEPTION` swallow it would report a memory-safety or
    /// logic fault as a business error. With no `RUST-EXCEPTION` clause it
    /// propagates after `FINALLY` and ends the run (R25).
    ///
    /// `message` is plain text — the panic payload and where it happened — so
    /// `DISPLAY` of the caught name is readable with no substring picking (R23).
    #[error("Rust panic: {message}")]
    RustPanic { message: String },
}

impl RuntimeError {
    /// `true` if this error is actually a normal program exit signal
    /// (STOP RUN or GO BACK) rather than a fault.
    pub fn is_exit_signal(&self) -> bool {
        matches!(self, RuntimeError::StopRun | RuntimeError::GoBack)
    }
}
