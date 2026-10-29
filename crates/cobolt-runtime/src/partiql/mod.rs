// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DynamoDB-flavoured PartiQL over an indexed file (spec 108).
//!
//! A pure language core: it lexes, parses and analyses a statement against a
//! record schema, plans how the file is read, and executes on any
//! [`crate::indexed::IndexedStore`]. It knows nothing of the interpreter, the
//! forms or the UI. The grammar is hand-written; the official PartiQL ANTLR
//! grammar is only a cross-check reference for the syntax and precedence.

pub mod analyze;
pub mod exec;
pub mod lexer;
pub mod parser;
pub mod plan;
pub mod schema;
pub mod value;

/// A position in the statement text, 1-based.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
}

/// A failed statement: the FILE STATUS it sets, the exception DynamoDB would
/// name, and a message built from names, positions and conditions only —
/// never from a parameter's value (R29).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartiqlError {
    pub status: &'static str,
    pub exception: &'static str,
    pub message: String,
    pub pos: Option<Pos>,
}

/// FILE STATUS of a statement that is not valid (R27).
pub const STATUS_VALIDATION: &str = "94";

impl PartiqlError {
    /// `ValidationException`: the statement is outside §5.1, or mistyped.
    pub fn validation(message: impl Into<String>, pos: Option<Pos>) -> Self {
        PartiqlError {
            status: STATUS_VALIDATION,
            exception: "ValidationException",
            message: message.into(),
            pos,
        }
    }

    /// `ResourceNotFoundException`: a table or index that is not the file's.
    pub fn resource_not_found(name: &str, pos: Option<Pos>) -> Self {
        PartiqlError {
            status: STATUS_VALIDATION,
            exception: "ResourceNotFoundException",
            message: name.to_string(),
            pos,
        }
    }

    /// The text `LastError` holds: `ValidationException: <what> at line:col`.
    pub fn text(&self) -> String {
        let mut out = String::new();
        if !self.exception.is_empty() {
            out.push_str(self.exception);
            out.push_str(": ");
        }
        out.push_str(&self.message);
        if let Some(p) = self.pos {
            out.push_str(&format!(" at {}:{}", p.line, p.col));
        }
        out
    }
}

impl std::fmt::Display for PartiqlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text())
    }
}

impl std::error::Error for PartiqlError {}
