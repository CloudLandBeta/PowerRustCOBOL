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
use cobolt_wasm_abi::codes::cap;

/// How an entry fares in a **Desktop** program built on the WebAssembly path (DESKTOP-WASM-001 R12), as
/// that path stands at the end of M1: console programs. An entry the path cannot build yet sends the
/// Build to the Cargo path, which says why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Native {
    /// Compiled into the module, with nothing from the runner beyond ABI v1 §3.
    Module,
    /// Compiled into the module, and served by the desktop runner through a capability (a name of
    /// `cobolt_wasm_abi::codes::cap`, WASM-ABI-v1 A27).
    Host(&'static str),
    /// Not on the WebAssembly path yet, or never (`EXEC RUST`): the Build takes the Cargo path, for this reason.
    Cargo(&'static str),
}

// The reasons of the Desktop column. Each names the milestone of DESKTOP-WASM-001 that moves the entry.
const D_FORMS: &str = "forms, their controls and the application object are not on the WebAssembly path yet (DESKTOP-WASM-001 M2)";
const D_DIALOGS: &str = "the desktop's dialogs are not on the WebAssembly path yet (DESKTOP-WASM-001 M2)";
const D_FILES: &str = "files on disk are not on the WebAssembly path yet (DESKTOP-WASM-001 M3); an INDEXED file declared STORAGE IS MEMORY is compiled into the module";
const D_FILE_BUILTINS: &str = "the file and image built-ins are not on the WebAssembly path yet (DESKTOP-WASM-001 M3)";
const D_SORT: &str = "SORT with USING or GIVING files is not on the WebAssembly path yet (DESKTOP-WASM-001 M3); with an INPUT and an OUTPUT PROCEDURE it is compiled into the module";
const D_FILES_M3: &str = "MERGE, COMMIT, ROLLBACK and UNLOCK are not on the WebAssembly path yet (DESKTOP-WASM-001 M3)";
const D_SQL: &str = "SQL is not on the WebAssembly path yet (DESKTOP-WASM-001 M4)";
const D_HTTP: &str = "HTTP is not on the WebAssembly path yet (DESKTOP-WASM-001 M4)";
const D_POINTERS: &str = "pointers and ADDRESS OF are not compiled into a module yet (DESKTOP-WASM-001 M5)";
const D_CONTROLS: &str = "the non-visual controls, the keys and the model lists are not on the WebAssembly path yet (DESKTOP-WASM-001 M6)";
const D_MEDIA: &str = "audio, animation sequences and layouts are not on the WebAssembly path yet (DESKTOP-WASM-001 M6)";
const D_PHONE: &str = "the phone's secure storage is a Mobile built-in, where a desktop program stops; that stop is not on the WebAssembly path yet (DESKTOP-WASM-001 M6)";
const D_REMOTE: &str = "remote calls, backend verbs and identity operations are not on the WebAssembly path yet (DESKTOP-WASM-001 M6)";
const D_EXEC_RUST: &str = "EXEC RUST is Rust compiled into the program, so the Build takes the Cargo path";

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
/// What `SET` a pointer, or `SET ADDRESS OF`, is told: the generator does not
/// lower it (DESKTOP-WASM-001 C13); the backend's interpreter runs it.
pub const POINTERS: &str = "pointers and ADDRESS OF are not compiled into a module; call a backend program with CALL … WITH WEBSOCKET LINKAGE";

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
        // Nothing lowers it (DESKTOP-WASM-001 C13): it used to be classed as
        // compiled, and was refused with the generic "not yet supported".
        Stmt::SetPointer { .. } => BackendOnly(POINTERS),
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
    /// How it fares in a Desktop program built on the WebAssembly path (DESKTOP-WASM-001 R12).
    pub native: Native,
}

const BOTH: Engines = Engines { chrome: true, safari: true };
const SQL: &str = "embedded SQL and databases run on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE";
/// What `COBOL::SqlDatabase::…` in a browser module is told (WASM-015 AC11): the program-level
/// database is the Backend's, and the page reaches the Backend's program with a CALL.
pub const SQL_DATABASE_OBJECT: &str = "COBOL::SqlDatabase runs on the backend, not in a browser module: the database and its connection belong to the Backend; write a Backend program that uses COBOL::SqlDatabase and call it from the page with CALL … WITH WEBSOCKET LINKAGE";
const FILES: &str = "files are on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE";
const HTTP: &str = "HTTP calls run on the backend, which holds the keys and is not bound by the page's CORS rules; call a backend program with CALL … WITH WEBSOCKET LINKAGE";
const SECRETS: &str = "keys and model providers stay on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE";
const BACKEND_VERB: &str = "it is a verb of a backend program, which runs on the server";
const NO_PATHS: &str = "a browser page has no file paths; let the backend hold the file";
const IDENTITY: &str = "identity operations run in the Backend, through its /auth routes; on the page the Authentication control signs the user in (WASM-009 D15)";
const PHONE_STORE: &str = "the phone's secure storage, through the mobile shell (Mobile projects only; a Web project's Build refuses it)";

const fn c(name: &'static str, support: Support, native: Native) -> BuiltinClass {
    BuiltinClass { name, support, api: None, native }
}
const fn web(name: &'static str, api: &'static str, engines: Engines, native: Native) -> BuiltinClass {
    BuiltinClass { name, support: Support::Module, api: Some((api, engines)), native }
}

const fn cargo(why: &'static str) -> Native {
    Native::Cargo(why)
}

/// Every built-in, alphabetically, as `BUILTINS` lists them. A test fails when a
/// name of `BUILTINS` has no row here, or a row names no built-in (R8).
pub const BUILTIN_TABLE: &[BuiltinClass] = &[
    c("ANIMATIONSEQUENCE", Support::Unsupported("the player has no scheduler for timed SHOW / HIDE / PLAY entries yet (spec 109 S7, WASM-PARITY #46)"), cargo(D_MEDIA)),
    c("APPEND-FILE", Support::BackendOnly(FILES), cargo(D_FILE_BUILTINS)),
    web("AUDIO-PAUSE", "HTMLAudioElement", BOTH, cargo(D_MEDIA)),
    web("AUDIO-PLAY", "HTMLAudioElement", BOTH, cargo(D_MEDIA)),
    web("AUDIO-RESUME", "HTMLAudioElement", BOTH, cargo(D_MEDIA)),
    c("AUDIO-STATE", Support::Unsupported("its answers have no way back into a browser module yet"), cargo(D_MEDIA)),
    web("AUDIO-STOP", "HTMLAudioElement", BOTH, cargo(D_MEDIA)),
    c("AUTH-OP", Support::BackendOnly(IDENTITY), cargo(D_REMOTE)),
    c("AWS-CLEAR-CREDENTIALS", Support::BackendOnly(SECRETS), cargo(D_CONTROLS)),
    c("AWS-CREDENTIALS-SET", Support::BackendOnly(SECRETS), cargo(D_CONTROLS)),
    c("AWS-SET-CREDENTIALS", Support::BackendOnly(SECRETS), cargo(D_CONTROLS)),
    c("BINDING-LOAD", Support::BackendOnly(SQL), cargo(D_SQL)),
    c("BINDING-MARK-CLEAN", Support::BackendOnly(SQL), cargo(D_SQL)),
    c("BINDING-POPULATE", Support::BackendOnly(SQL), cargo(D_SQL)),
    c("BINDING-SET-PENDING", Support::BackendOnly(SQL), cargo(D_SQL)),
    c("BINDING-SET-READ-ONLY", Support::BackendOnly(SQL), cargo(D_SQL)),
    c("BINDING-UPDATE", Support::BackendOnly(SQL), cargo(D_SQL)),
    c("CHART-ADD-POINT", Support::Module, cargo(D_FORMS)),
    c("CHART-CLEAR", Support::Module, cargo(D_FORMS)),
    c("CHART-REFRESH", Support::Module, cargo(D_FORMS)),
    c("CHART-SET-TABLE", Support::Module, cargo(D_FORMS)),
    c("CLOSE-DB", Support::BackendOnly(SQL), cargo(D_SQL)),
    c("EXEC-SQL", Support::BackendOnly(SQL), cargo(D_SQL)),
    c("FETCH-ROW", Support::BackendOnly(SQL), cargo(D_SQL)),
    c("FILE-STATUS", Support::BackendOnly(FILES), cargo(D_FILE_BUILTINS)),
    // `showDirectoryPicker` is Chrome's only: Q7's parity rule leaves it out.
    c("FOLDER-DIALOG", Support::Unsupported("a browser page cannot choose a folder in every engine (showDirectoryPicker is Chrome's only)"), cargo(D_DIALOGS)),
    c("GET-PROPERTY", Support::Module, cargo(D_FORMS)),
    c("HTTP-CLEAR-HEADERS", Support::BackendOnly(HTTP), cargo(D_HTTP)),
    c("HTTP-DELETE", Support::BackendOnly(HTTP), cargo(D_HTTP)),
    c("HTTP-DOWNLOAD", Support::BackendOnly(HTTP), cargo(D_HTTP)),
    c("HTTP-GET", Support::BackendOnly(HTTP), cargo(D_HTTP)),
    c("HTTP-POST", Support::BackendOnly(HTTP), cargo(D_HTTP)),
    c("HTTP-PUT", Support::BackendOnly(HTTP), cargo(D_HTTP)),
    c("HTTP-SET-HEADER", Support::BackendOnly(HTTP), cargo(D_HTTP)),
    c("IMAGE-FIT", Support::BackendOnly(FILES), cargo(D_FILE_BUILTINS)),
    c("IMAGE-INFO", Support::BackendOnly(FILES), cargo(D_FILE_BUILTINS)),
    c("INIT-FORM", Support::Module, cargo(D_FORMS)),
    c("KEY-IS-SET", Support::BackendOnly(SECRETS), cargo(D_CONTROLS)),
    c("KEY-REMOVE", Support::BackendOnly(SECRETS), cargo(D_CONTROLS)),
    c("KEY-SET", Support::BackendOnly(SECRETS), cargo(D_CONTROLS)),
    c("LAYOUT", Support::Unsupported("the player has no command to install a layout script at run time yet (spec 109 S6, WASM-PARITY #47)"), cargo(D_MEDIA)),
    c("MCP-SEARCH", Support::BackendOnly(SECRETS), cargo(D_CONTROLS)),
    c("MODEL-COUNT", Support::BackendOnly(SECRETS), cargo(D_CONTROLS)),
    c("MODEL-GET", Support::BackendOnly(SECRETS), cargo(D_CONTROLS)),
    c("MODEL-LIST", Support::BackendOnly(SECRETS), cargo(D_CONTROLS)),
    c("MODEL-LIST-GET", Support::BackendOnly(SECRETS), cargo(D_CONTROLS)),
    c("MODEL-REMOVE", Support::BackendOnly(SECRETS), cargo(D_CONTROLS)),
    c("MODEL-SET", Support::BackendOnly(SECRETS), cargo(D_CONTROLS)),
    c("MODEL-TEST", Support::BackendOnly(SECRETS), cargo(D_CONTROLS)),
    c("NEXT-ROW", Support::BackendOnly(SQL), cargo(D_SQL)),
    c("OPEN-DB", Support::BackendOnly(SQL), cargo(D_SQL)),
    c("OPEN-FILE-DIALOG", Support::Unsupported(NO_PATHS), cargo(D_DIALOGS)),
    c("PARTIQL", Support::BackendOnly(FILES), cargo(D_CONTROLS)),
    c("PARTIQL-ERROR", Support::BackendOnly(FILES), cargo(D_CONTROLS)),
    c("PARTIQL-FETCH", Support::BackendOnly(FILES), cargo(D_CONTROLS)),
    c("PROVIDER-COUNT", Support::BackendOnly(SECRETS), cargo(D_CONTROLS)),
    c("PROVIDER-GET", Support::BackendOnly(SECRETS), cargo(D_CONTROLS)),
    c("RAISE-EVENT", Support::Module, cargo(D_FORMS)),
    c("ROW-COUNT", Support::BackendOnly(SQL), cargo(D_SQL)),
    c("SAVE-FILE-DIALOG", Support::Unsupported(NO_PATHS), cargo(D_DIALOGS)),
    // The phone shell's verbs (WASM-012 R9, ABI A26): a Mobile module compiles
    // them onto the application object; a Web module refuses them at their
    // line before this row is read (`lower_remote::is_mobile_verb`).
    web("SECUREDELETE", PHONE_STORE, BOTH, cargo(D_PHONE)),
    web("SECURELOAD", PHONE_STORE, BOTH, cargo(D_PHONE)),
    web("SECURESTORE", PHONE_STORE, BOTH, cargo(D_PHONE)),
    c("SET-PROPERTY", Support::Module, cargo(D_FORMS)),
    c("WAIT-EVENT", Support::Module, cargo(D_FORMS)),
    c("WRITE-FILE", Support::BackendOnly(FILES), cargo(D_FILE_BUILTINS)),
    c("WS-CANCELLED", Support::BackendOnly(BACKEND_VERB), cargo(D_REMOTE)),
    c("WS-CLAIM", Support::BackendOnly(BACKEND_VERB), cargo(D_REMOTE)),
    c("WS-DOWNLOAD", Support::Unsupported(NO_PATHS), cargo(D_REMOTE)),
    c("WS-EMIT", Support::BackendOnly(BACKEND_VERB), cargo(D_REMOTE)),
    c("WS-FAIL", Support::BackendOnly(BACKEND_VERB), cargo(D_REMOTE)),
    c("WS-FILE-PATH", Support::BackendOnly(BACKEND_VERB), cargo(D_REMOTE)),
    c("WS-FILE-PUBLISH", Support::BackendOnly(BACKEND_VERB), cargo(D_REMOTE)),
    c("WS-LAST-ERROR", Support::Module, cargo(D_REMOTE)),
    c("WS-PAUSE", Support::BackendOnly(BACKEND_VERB), cargo(D_REMOTE)),
    c("WS-PROGRESS", Support::BackendOnly(BACKEND_VERB), cargo(D_REMOTE)),
    c("WS-SHOW-PROGRESS", Support::Unsupported("not in a browser module yet"), cargo(D_REMOTE)),
    c("WS-STREAM-ITEM", Support::Unsupported("not in a browser module yet"), cargo(D_REMOTE)),
    c("WS-SUBSCRIBE", Support::Unsupported("not in a browser module yet"), cargo(D_REMOTE)),
    c("WS-UNSUBSCRIBE", Support::Unsupported("not in a browser module yet"), cargo(D_REMOTE)),
    c("WS-UPLOAD", Support::Unsupported(NO_PATHS), cargo(D_REMOTE)),
    c("WS-VERSION", Support::Unsupported("not in a browser module yet"), cargo(D_REMOTE)),
    c("WS-WARNING", Support::Unsupported("not in a browser module yet"), cargo(D_REMOTE)),
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

/// Every statement, by the name [`stmt_name`] gives it, with its class in a browser
/// module and in a Desktop program on the WebAssembly path: what the System KB's
/// "What runs in a browser module" lists (WASM-010 D7; DESKTOP-WASM-001 R12). The
/// exhaustive [`support`] is the definition of the browser class;
/// `tests/subset_table.rs` holds this table equal to it statement by statement.
/// On the Desktop, `ACCEPT` from the console reaches `console` (from the environment
/// or the arguments, `env`), and `STOP RUN` and `GOBACK` hand the exit code to `env`.
pub const STATEMENT_TABLE: &[(&str, Support, Native)] = &[
    ("ACCEPT", Support::Module, Native::Host(cap::CONSOLE)),
    ("ADD", Support::Module, Native::Module),
    ("ADD CORRESPONDING", Support::Module, Native::Module),
    ("ALTER", Support::Module, Native::Module),
    ("CALL", Support::Module, Native::Module),
    ("CANCEL", Support::Module, Native::Module),
    ("CLOSE", Support::MemoryFiles(DISK_FILES), Native::Cargo(D_FILES)),
    ("COMMIT", Support::BackendOnly(BACKEND_FILES), Native::Cargo(D_FILES_M3)),
    ("COMPUTE", Support::Module, Native::Module),
    ("CONTINUE", Support::Module, Native::Module),
    ("DELETE", Support::MemoryFiles(DISK_FILES), Native::Cargo(D_FILES)),
    ("DISPLAY", Support::Module, Native::Module),
    ("DIVIDE", Support::Module, Native::Module),
    ("EVALUATE", Support::Module, Native::Module),
    ("EXEC RUST", Support::Unsupported("EXEC RUST needs a native build; move it to the backend"), Native::Cargo(D_EXEC_RUST)),
    ("EXEC SQL", Support::BackendOnly("embedded SQL runs on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE"), Native::Cargo(D_SQL)),
    ("EXIT", Support::Module, Native::Module),
    ("GO TO", Support::Module, Native::Module),
    ("GO TO DEPENDING", Support::Module, Native::Module),
    ("GOBACK", Support::Module, Native::Host(cap::ENV)),
    ("IF", Support::Module, Native::Module),
    ("INITIALIZE", Support::Module, Native::Module),
    ("INSPECT", Support::Module, Native::Module),
    ("INVOKE", Support::Module, Native::Cargo(D_FORMS)),
    ("MERGE", Support::BackendOnly("MERGE reads files, which are on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE"), Native::Cargo(D_FILES_M3)),
    ("MOVE", Support::Module, Native::Module),
    ("MOVE CORRESPONDING", Support::Module, Native::Module),
    ("MULTIPLY", Support::Module, Native::Module),
    ("NEXT SENTENCE", Support::Module, Native::Module),
    ("OPEN", Support::MemoryFiles(DISK_FILES), Native::Cargo(D_FILES)),
    ("PERFORM", Support::Module, Native::Module),
    ("READ", Support::MemoryFiles(DISK_FILES), Native::Cargo(D_FILES)),
    ("RELEASE", Support::MemoryFiles(SORT_FILES), Native::Module),
    ("RETURN", Support::MemoryFiles(SORT_FILES), Native::Module),
    ("REWRITE", Support::MemoryFiles(DISK_FILES), Native::Cargo(D_FILES)),
    ("ROLLBACK", Support::BackendOnly(BACKEND_FILES), Native::Cargo(D_FILES_M3)),
    ("SEARCH", Support::Module, Native::Module),
    ("SET (pointer)", Support::BackendOnly(POINTERS), Native::Cargo(D_POINTERS)),
    ("SORT", Support::MemoryFiles(SORT_FILES), Native::Cargo(D_SORT)),
    ("START", Support::MemoryFiles(DISK_FILES), Native::Cargo(D_FILES)),
    ("STOP", Support::Module, Native::Host(cap::ENV)),
    ("STRING", Support::Module, Native::Module),
    ("SUBTRACT", Support::Module, Native::Module),
    ("SUBTRACT CORRESPONDING", Support::Module, Native::Module),
    ("THROW", Support::Module, Native::Module),
    ("TRY", Support::Module, Native::Module),
    ("UNLOCK", Support::BackendOnly(BACKEND_FILES), Native::Cargo(D_FILES_M3)),
    ("UNSTRING", Support::Module, Native::Module),
    ("WRITE", Support::MemoryFiles(DISK_FILES), Native::Cargo(D_FILES)),
    ("control assignment", Support::Module, Native::Cargo(D_FORMS)),
    ("end of sentence", Support::Module, Native::Module),
    ("member call", Support::Module, Native::Cargo(D_FORMS)),
    ("window operation", Support::Module, Native::Cargo(D_FORMS)),
];

/// The browser class of [`STATEMENT_TABLE`]'s row for a statement name.
pub fn statement_entry(name: &str) -> Option<Support> {
    STATEMENT_TABLE.iter().find(|(n, _, _)| *n == name).map(|(_, s, _)| *s)
}

/// The Desktop class of [`STATEMENT_TABLE`]'s row for a statement name (DESKTOP-WASM-001 R12).
pub fn statement_native(name: &str) -> Option<Native> {
    STATEMENT_TABLE.iter().find(|(n, _, _)| *n == name).map(|(_, _, c)| *c)
}
