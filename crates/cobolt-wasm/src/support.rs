// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the generator accepts: one table, one entry per statement (WASM-010
//! R7, R8). The match is exhaustive on purpose — a new `Stmt` variant does not
//! compile until it is classified here, which is the "a test fails when a
//! variant has no entry" rule held by the compiler itself.

use cobolt_ast::stmt::Stmt;

/// How a statement fares in a browser module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// Compiled into the module.
    Module,
    /// Compiled, but only in a conformance build (WASM-PLAN P6).
    Conformance,
    /// Runs on the backend; the browser module refuses it with the alternative.
    BackendOnly(&'static str),
    /// Not available on any WebAssembly build.
    Unsupported(&'static str),
}

pub fn support(s: &Stmt) -> Support {
    use Support::*;
    match s {
        Stmt::Move { .. }
        | Stmt::Add { .. }
        | Stmt::Subtract { .. }
        | Stmt::Multiply { .. }
        | Stmt::Divide { .. }
        | Stmt::Compute { .. }
        | Stmt::If { .. }
        | Stmt::Evaluate { .. }
        | Stmt::Perform { .. }
        | Stmt::GoTo { .. }
        | Stmt::GoToDepending { .. }
        | Stmt::Continue { .. }
        | Stmt::Exit { .. }
        | Stmt::NextSentence { .. }
        | Stmt::SentenceEnd { .. }
        | Stmt::Display { .. }
        | Stmt::Stop { .. }
        | Stmt::GoBack { .. } => Module,
        // Phase B and later: planned for the module (WASM-010 R18).
        Stmt::MoveCorresponding { .. }
        | Stmt::AddCorresponding { .. }
        | Stmt::SubtractCorresponding { .. }
        | Stmt::Initialize { .. }
        | Stmt::Alter { .. }
        | Stmt::Search { .. }
        | Stmt::String_ { .. }
        | Stmt::Unstring { .. }
        | Stmt::Inspect { .. }
        | Stmt::Call { .. }
        | Stmt::Cancel { .. }
        | Stmt::Invoke { .. }
        | Stmt::InvokeExpr { .. }
        | Stmt::WindowOp { .. }
        | Stmt::ControlSet { .. }
        | Stmt::SetPointer { .. }
        | Stmt::TryCatch { .. }
        | Stmt::Throw { .. }
        | Stmt::Accept { .. } => Module,
        // Files: the browser has none; the gate builds them with host imports.
        Stmt::Open { .. }
        | Stmt::Close { .. }
        | Stmt::Read { .. }
        | Stmt::Write { .. }
        | Stmt::Rewrite { .. }
        | Stmt::Delete { .. }
        | Stmt::Start { .. }
        | Stmt::Unlock { .. }
        | Stmt::Commit { .. }
        | Stmt::Rollback { .. }
        | Stmt::Sort { .. }
        | Stmt::Merge { .. }
        | Stmt::Release { .. }
        | Stmt::Return { .. } => BackendOnly("file input/output runs on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE"),
        Stmt::ExecRust { .. } => Unsupported("EXEC RUST needs a native build; move it to the backend"),
        Stmt::ExecSql(_) => BackendOnly("embedded SQL runs on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE"),
    }
}

/// The name a diagnostic gives a statement.
pub fn stmt_name(s: &Stmt) -> &'static str {
    match s {
        Stmt::Move { .. } => "MOVE",
        Stmt::MoveCorresponding { .. } => "MOVE CORRESPONDING",
        Stmt::AddCorresponding { .. } => "ADD CORRESPONDING",
        Stmt::SubtractCorresponding { .. } => "SUBTRACT CORRESPONDING",
        Stmt::Initialize { .. } => "INITIALIZE",
        Stmt::Add { .. } => "ADD",
        Stmt::Subtract { .. } => "SUBTRACT",
        Stmt::Multiply { .. } => "MULTIPLY",
        Stmt::Divide { .. } => "DIVIDE",
        Stmt::Compute { .. } => "COMPUTE",
        Stmt::Alter { .. } => "ALTER",
        Stmt::Unlock { .. } => "UNLOCK",
        Stmt::Commit { .. } => "COMMIT",
        Stmt::Rollback { .. } => "ROLLBACK",
        Stmt::SetPointer { .. } => "SET (pointer)",
        Stmt::If { .. } => "IF",
        Stmt::Evaluate { .. } => "EVALUATE",
        Stmt::Perform { .. } => "PERFORM",
        Stmt::Search { .. } => "SEARCH",
        Stmt::GoTo { .. } => "GO TO",
        Stmt::GoToDepending { .. } => "GO TO DEPENDING",
        Stmt::Continue { .. } => "CONTINUE",
        Stmt::Exit { .. } => "EXIT",
        Stmt::NextSentence { .. } => "NEXT SENTENCE",
        Stmt::SentenceEnd { .. } => "end of sentence",
        Stmt::Open { .. } => "OPEN",
        Stmt::Close { .. } => "CLOSE",
        Stmt::Read { .. } => "READ",
        Stmt::Write { .. } => "WRITE",
        Stmt::Rewrite { .. } => "REWRITE",
        Stmt::Delete { .. } => "DELETE",
        Stmt::Start { .. } => "START",
        Stmt::Accept { .. } => "ACCEPT",
        Stmt::Display { .. } => "DISPLAY",
        Stmt::String_ { .. } => "STRING",
        Stmt::Unstring { .. } => "UNSTRING",
        Stmt::Inspect { .. } => "INSPECT",
        Stmt::Sort { .. } => "SORT",
        Stmt::Merge { .. } => "MERGE",
        Stmt::Release { .. } => "RELEASE",
        Stmt::Return { .. } => "RETURN",
        Stmt::Call { .. } => "CALL",
        Stmt::Invoke { .. } => "INVOKE",
        Stmt::InvokeExpr { .. } => "member call",
        Stmt::Cancel { .. } => "CANCEL",
        Stmt::Stop { .. } => "STOP",
        Stmt::GoBack { .. } => "GOBACK",
        Stmt::WindowOp { .. } => "window operation",
        Stmt::ControlSet { .. } => "control assignment",
        Stmt::ExecRust { .. } => "EXEC RUST",
        Stmt::TryCatch { .. } => "TRY",
        Stmt::Throw { .. } => "THROW",
        Stmt::ExecSql(_) => "EXEC SQL",
    }
}
