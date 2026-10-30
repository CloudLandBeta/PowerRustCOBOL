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
    /// Compiled on an INDEXED file declared `STORAGE IS MEMORY` (without `WITH
    /// PERSISTENCE`) and on a `SORT` work file (WASM-010 R7, Q8); on any other
    /// file it runs on the backend, and the text says what to do instead.
    MemoryFiles(&'static str),
    /// Runs on the backend; the browser module refuses it with the alternative.
    BackendOnly(&'static str),
    /// Not available on any WebAssembly build.
    Unsupported(&'static str),
}

/// What a file verb on a file that is not in memory is told.
pub const DISK_FILES: &str = "only an INDEXED file declared STORAGE IS MEMORY (without WITH PERSISTENCE) is held in a browser module; other files are on the backend: call a backend program with CALL … WITH WEBSOCKET LINKAGE";
/// What a `SORT` with `USING` or `GIVING` is told.
pub const SORT_FILES: &str = "a browser module sorts records given by an INPUT PROCEDURE and returned to an OUTPUT PROCEDURE; USING and GIVING name files, which are on the backend";

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
        // Files: the browser has none on disk. An INDEXED file held in memory
        // lives with the form instance (Q8); every other file is the
        // backend's. The NIST gate also builds sequential files with host
        // imports (conformance builds only).
        Stmt::Open { .. }
        | Stmt::Close { .. }
        | Stmt::Read { .. }
        | Stmt::Write { .. }
        | Stmt::Rewrite { .. }
        | Stmt::Delete { .. }
        | Stmt::Start { .. } => MemoryFiles(DISK_FILES),
        // A SORT whose records come from an INPUT PROCEDURE and go to an
        // OUTPUT PROCEDURE sorts in memory; USING and GIVING name files.
        Stmt::Sort { .. } | Stmt::Release { .. } | Stmt::Return { .. } => MemoryFiles(SORT_FILES),
        Stmt::Merge { .. } => BackendOnly("MERGE reads files, which are on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE"),
        Stmt::Unlock { .. } | Stmt::Commit { .. } | Stmt::Rollback { .. } => {
            BackendOnly("file input/output runs on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE")
        }
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

/// The browser engines that offer an API a built-in needs (WASM-010 Q7: an iOS
/// application goes by Safari; every other target needs both).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Engines {
    pub chrome: bool,
    pub safari: bool,
}

/// One built-in of `cobolt-runtime`'s `BUILTINS`, classified for a browser
/// module (WASM-010 R7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinClass {
    /// The name without `COBOL-`.
    pub name: &'static str,
    pub support: Support,
    /// The browser API a *browser* entry relies on, and the engines that have it.
    /// `None` when the player does the work with nothing beyond the page itself.
    pub api: Option<(&'static str, Engines)>,
}

const BOTH: Engines = Engines { chrome: true, safari: true };
const SQL: &str = "embedded SQL and databases run on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE";
const FILES: &str = "files are on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE";
const HTTP: &str = "HTTP calls run on the backend, which holds the keys and is not bound by the page's CORS rules; call a backend program with CALL … WITH WEBSOCKET LINKAGE";
const SECRETS: &str = "keys and model providers stay on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE";
const BACKEND_VERB: &str = "it is a verb of a backend program, which runs on the server";
const NO_PATHS: &str = "a browser page has no file paths; let the backend hold the file";

const fn c(name: &'static str, support: Support) -> BuiltinClass {
    BuiltinClass { name, support, api: None }
}
const fn web(name: &'static str, api: &'static str, engines: Engines) -> BuiltinClass {
    BuiltinClass { name, support: Support::Module, api: Some((api, engines)) }
}

/// Every built-in, alphabetically, as `BUILTINS` lists them. A test fails when a
/// name of `BUILTINS` has no row here, or a row names no built-in (R8).
pub const BUILTIN_TABLE: &[BuiltinClass] = &[
    c("APPEND-FILE", Support::BackendOnly(FILES)),
    web("AUDIO-PAUSE", "HTMLAudioElement", BOTH),
    web("AUDIO-PLAY", "HTMLAudioElement", BOTH),
    web("AUDIO-RESUME", "HTMLAudioElement", BOTH),
    c("AUDIO-STATE", Support::Unsupported("its answers have no way back into a browser module yet")),
    web("AUDIO-STOP", "HTMLAudioElement", BOTH),
    c("AWS-CLEAR-CREDENTIALS", Support::BackendOnly(SECRETS)),
    c("AWS-CREDENTIALS-SET", Support::BackendOnly(SECRETS)),
    c("AWS-SET-CREDENTIALS", Support::BackendOnly(SECRETS)),
    c("BINDING-LOAD", Support::BackendOnly(SQL)),
    c("BINDING-MARK-CLEAN", Support::BackendOnly(SQL)),
    c("BINDING-POPULATE", Support::BackendOnly(SQL)),
    c("BINDING-SET-PENDING", Support::BackendOnly(SQL)),
    c("BINDING-SET-READ-ONLY", Support::BackendOnly(SQL)),
    c("BINDING-UPDATE", Support::BackendOnly(SQL)),
    c("CHART-ADD-POINT", Support::Module),
    c("CHART-CLEAR", Support::Module),
    c("CHART-REFRESH", Support::Module),
    c("CHART-SET-TABLE", Support::Module),
    c("CLOSE-DB", Support::BackendOnly(SQL)),
    c("EXEC-SQL", Support::BackendOnly(SQL)),
    c("FETCH-ROW", Support::BackendOnly(SQL)),
    c("FILE-STATUS", Support::BackendOnly(FILES)),
    // `showDirectoryPicker` is Chrome's only: Q7's parity rule leaves it out.
    c("FOLDER-DIALOG", Support::Unsupported("a browser page cannot choose a folder in every engine (showDirectoryPicker is Chrome's only)")),
    c("GET-PROPERTY", Support::Module),
    c("HTTP-CLEAR-HEADERS", Support::BackendOnly(HTTP)),
    c("HTTP-DELETE", Support::BackendOnly(HTTP)),
    c("HTTP-DOWNLOAD", Support::BackendOnly(HTTP)),
    c("HTTP-GET", Support::BackendOnly(HTTP)),
    c("HTTP-POST", Support::BackendOnly(HTTP)),
    c("HTTP-PUT", Support::BackendOnly(HTTP)),
    c("HTTP-SET-HEADER", Support::BackendOnly(HTTP)),
    c("IMAGE-FIT", Support::BackendOnly(FILES)),
    c("IMAGE-INFO", Support::BackendOnly(FILES)),
    c("INIT-FORM", Support::Module),
    c("KEY-IS-SET", Support::BackendOnly(SECRETS)),
    c("KEY-REMOVE", Support::BackendOnly(SECRETS)),
    c("KEY-SET", Support::BackendOnly(SECRETS)),
    c("MCP-SEARCH", Support::BackendOnly(SECRETS)),
    c("MODEL-COUNT", Support::BackendOnly(SECRETS)),
    c("MODEL-GET", Support::BackendOnly(SECRETS)),
    c("MODEL-LIST", Support::BackendOnly(SECRETS)),
    c("MODEL-LIST-GET", Support::BackendOnly(SECRETS)),
    c("MODEL-REMOVE", Support::BackendOnly(SECRETS)),
    c("MODEL-SET", Support::BackendOnly(SECRETS)),
    c("MODEL-TEST", Support::BackendOnly(SECRETS)),
    c("NEXT-ROW", Support::BackendOnly(SQL)),
    c("OPEN-DB", Support::BackendOnly(SQL)),
    c("OPEN-FILE-DIALOG", Support::Unsupported(NO_PATHS)),
    c("PARTIQL", Support::BackendOnly(FILES)),
    c("PARTIQL-ERROR", Support::BackendOnly(FILES)),
    c("PARTIQL-FETCH", Support::BackendOnly(FILES)),
    c("PROVIDER-COUNT", Support::BackendOnly(SECRETS)),
    c("PROVIDER-GET", Support::BackendOnly(SECRETS)),
    c("RAISE-EVENT", Support::Module),
    c("ROW-COUNT", Support::BackendOnly(SQL)),
    c("SAVE-FILE-DIALOG", Support::Unsupported(NO_PATHS)),
    c("SET-PROPERTY", Support::Module),
    c("WAIT-EVENT", Support::Module),
    c("WRITE-FILE", Support::BackendOnly(FILES)),
    c("WS-CANCELLED", Support::BackendOnly(BACKEND_VERB)),
    c("WS-CLAIM", Support::BackendOnly(BACKEND_VERB)),
    c("WS-DOWNLOAD", Support::Unsupported(NO_PATHS)),
    c("WS-EMIT", Support::BackendOnly(BACKEND_VERB)),
    c("WS-FAIL", Support::BackendOnly(BACKEND_VERB)),
    c("WS-FILE-PATH", Support::BackendOnly(BACKEND_VERB)),
    c("WS-FILE-PUBLISH", Support::BackendOnly(BACKEND_VERB)),
    c("WS-LAST-ERROR", Support::Module),
    c("WS-PAUSE", Support::BackendOnly(BACKEND_VERB)),
    c("WS-PROGRESS", Support::BackendOnly(BACKEND_VERB)),
    c("WS-UPLOAD", Support::Unsupported(NO_PATHS)),
    c("WS-WARNING", Support::Unsupported("not in a browser module yet")),
];

/// The row of the built-in `name` (without `COBOL-`), if it is one.
pub fn builtin_entry(name: &str) -> Option<&'static BuiltinClass> {
    BUILTIN_TABLE.iter().find(|b| b.name == name)
}

/// How the built-in `name` (without `COBOL-`) fares in a browser module.
pub fn builtin_support(name: &str) -> Support {
    builtin_entry(name).map(|b| b.support).unwrap_or(Support::Unsupported("it is not a built-in of this version"))
}

/// Whether the built-in compiles into a module for this target (Q7): a
/// *browser* entry whose API Safari lacks is unavailable on iOS, and one that
/// either engine lacks is unavailable everywhere else.
pub fn builtin_available(name: &str, ios: bool) -> bool {
    match builtin_entry(name) {
        Some(BuiltinClass { support: Support::Module, api: None, .. }) => true,
        Some(BuiltinClass { support: Support::Module, api: Some((_, e)), .. }) => {
            if ios {
                e.safari
            } else {
                e.chrome && e.safari
            }
        }
        _ => false,
    }
}

const BACKEND_FILES: &str = "file input/output runs on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE";

/// Every statement, by the name [`stmt_name`] gives it, with its class: what
/// the System KB's "What runs in a browser module" lists (WASM-010 D7). The
/// exhaustive [`support`] is the definition; `tests/subset_table.rs` holds this
/// table equal to it statement by statement.
pub const STATEMENT_TABLE: &[(&str, Support)] = &[
    ("ACCEPT", Support::Module),
    ("ADD", Support::Module),
    ("ADD CORRESPONDING", Support::Module),
    ("ALTER", Support::Module),
    ("CALL", Support::Module),
    ("CANCEL", Support::Module),
    ("CLOSE", Support::MemoryFiles(DISK_FILES)),
    ("COMMIT", Support::BackendOnly(BACKEND_FILES)),
    ("COMPUTE", Support::Module),
    ("CONTINUE", Support::Module),
    ("DELETE", Support::MemoryFiles(DISK_FILES)),
    ("DISPLAY", Support::Module),
    ("DIVIDE", Support::Module),
    ("EVALUATE", Support::Module),
    ("EXEC RUST", Support::Unsupported("EXEC RUST needs a native build; move it to the backend")),
    ("EXEC SQL", Support::BackendOnly("embedded SQL runs on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE")),
    ("EXIT", Support::Module),
    ("GO TO", Support::Module),
    ("GO TO DEPENDING", Support::Module),
    ("GOBACK", Support::Module),
    ("IF", Support::Module),
    ("INITIALIZE", Support::Module),
    ("INSPECT", Support::Module),
    ("INVOKE", Support::Module),
    ("MERGE", Support::BackendOnly("MERGE reads files, which are on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE")),
    ("MOVE", Support::Module),
    ("MOVE CORRESPONDING", Support::Module),
    ("MULTIPLY", Support::Module),
    ("NEXT SENTENCE", Support::Module),
    ("OPEN", Support::MemoryFiles(DISK_FILES)),
    ("PERFORM", Support::Module),
    ("READ", Support::MemoryFiles(DISK_FILES)),
    ("RELEASE", Support::MemoryFiles(SORT_FILES)),
    ("RETURN", Support::MemoryFiles(SORT_FILES)),
    ("REWRITE", Support::MemoryFiles(DISK_FILES)),
    ("ROLLBACK", Support::BackendOnly(BACKEND_FILES)),
    ("SEARCH", Support::Module),
    ("SET (pointer)", Support::Module),
    ("SORT", Support::MemoryFiles(SORT_FILES)),
    ("START", Support::MemoryFiles(DISK_FILES)),
    ("STOP", Support::Module),
    ("STRING", Support::Module),
    ("SUBTRACT", Support::Module),
    ("SUBTRACT CORRESPONDING", Support::Module),
    ("THROW", Support::Module),
    ("TRY", Support::Module),
    ("UNLOCK", Support::BackendOnly(BACKEND_FILES)),
    ("UNSTRING", Support::Module),
    ("WRITE", Support::MemoryFiles(DISK_FILES)),
    ("control assignment", Support::Module),
    ("end of sentence", Support::Module),
    ("member call", Support::Module),
    ("window operation", Support::Module),
];

/// The row of [`STATEMENT_TABLE`] for a statement name.
pub fn statement_entry(name: &str) -> Option<Support> {
    STATEMENT_TABLE.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}
