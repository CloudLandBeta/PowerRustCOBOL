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

    /// An EXEC RUST block failed to execute.
    #[error("EXEC RUST error at {span}: {message}")]
    ExecRustError { message: String, span: Span },

    /// GO TO control-flow signal — not a real error; caught by the main run loop.
    #[error("GO TO {target}")]
    GoTo { target: String },

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
}

impl RuntimeError {
    /// `true` if this error is actually a normal program exit signal
    /// (STOP RUN or GO BACK) rather than a fault.
    pub fn is_exit_signal(&self) -> bool {
        matches!(self, RuntimeError::StopRun | RuntimeError::GoBack)
    }
}
