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
    /// A file verb, served by the desktop runner through the capability of the file's organisation
    /// (`files.sequential`, `files.line-sequential`, `files.relative` or `files.indexed`, ABI A34;
    /// DESKTOP-WASM-001 T38). A file the runner cannot keep yet sends the Build to the Cargo path at
    /// its first verb, naming the file and why.
    /// `SORT` and `MERGE` sort in the module and read their `USING` files and write their `GIVING`
    /// files through the runner; `UNLOCK`, `COMMIT` and `ROLLBACK` reach an INDEXED file's engine
    /// (`files.indexed`, A35; T39).
    Files,
    /// Compiled into a form's program and served by the runner's ABI host with the form, its
    /// controls and the application object (DESKTOP-WASM-001 M2, T33). A program with no form
    /// has none of them, so there it sends the Build to the Cargo path ([`D_FORMS`]).
    Form,
    /// Not on the WebAssembly path yet, or never (`EXEC RUST`): the Build takes the Cargo path, for this reason.
    Cargo(&'static str),
}

// The reasons of the Desktop column. Each names the milestone of DESKTOP-WASM-001 that moves the entry.
/// Why a program with no form takes the Cargo path for an entry of class [`Native::Form`] (DESKTOP-WASM-001 T33).
pub const D_FORMS: &str = "a program with no form has no controls or application object in its module, so it takes the Cargo path for them (DESKTOP-WASM-001 M2 serves them to a form's program)";
/// The `BINDING-…` built-ins drive a Data Binding, which moves with the bound controls (M6), not with
/// SQL (M4): T43 relabelled them.
const D_BINDING: &str = "the Data Binding built-ins are not on the WebAssembly path yet (DESKTOP-WASM-001 M6)";
const D_MEDIA: &str = "animation sequences and layouts are not on the WebAssembly path yet (DESKTOP-WASM-001 M6)";
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
        // A browser module does not lower it (DESKTOP-WASM-001 C13); a desktop
        // module does (T45b, `lower_ptr`).
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
    c("APPEND-FILE", Support::BackendOnly(FILES), Native::Host(cap::FILES_TEXT)),
    web("AUDIO-PAUSE", "HTMLAudioElement", BOTH, Native::Host(cap::AUDIO)),
    web("AUDIO-PLAY", "HTMLAudioElement", BOTH, Native::Host(cap::AUDIO)),
    web("AUDIO-RESUME", "HTMLAudioElement", BOTH, Native::Host(cap::AUDIO)),
    c("AUDIO-STATE", Support::Unsupported("its answers have no way back into a browser module yet"), Native::Host(cap::AUDIO)),
    web("AUDIO-STOP", "HTMLAudioElement", BOTH, Native::Host(cap::AUDIO)),
    c("AUTH-OP", Support::BackendOnly(IDENTITY), cargo(D_REMOTE)),
    c("AWS-CLEAR-CREDENTIALS", Support::BackendOnly(SECRETS), Native::Host(cap::AWS)),
    c("AWS-CREDENTIALS-SET", Support::BackendOnly(SECRETS), Native::Host(cap::AWS)),
    c("AWS-SET-CREDENTIALS", Support::BackendOnly(SECRETS), Native::Host(cap::AWS)),
    c("BINDING-LOAD", Support::BackendOnly(SQL), cargo(D_BINDING)),
    c("BINDING-MARK-CLEAN", Support::BackendOnly(SQL), cargo(D_BINDING)),
    c("BINDING-POPULATE", Support::BackendOnly(SQL), cargo(D_BINDING)),
    c("BINDING-SET-PENDING", Support::BackendOnly(SQL), cargo(D_BINDING)),
    c("BINDING-SET-READ-ONLY", Support::BackendOnly(SQL), cargo(D_BINDING)),
    c("BINDING-UPDATE", Support::BackendOnly(SQL), cargo(D_BINDING)),
    c("CHART-ADD-POINT", Support::Module, Native::Form),
    c("CHART-CLEAR", Support::Module, Native::Form),
    c("CHART-REFRESH", Support::Module, Native::Form),
    c("CHART-SET-TABLE", Support::Module, Native::Form),
    // The runner keeps the connections and runs these with the interpreter's own code (T41b).
    c("CLOSE-DB", Support::BackendOnly(SQL), Native::Host(cap::SQL)),
    c("EXEC-SQL", Support::BackendOnly(SQL), Native::Host(cap::SQL)),
    c("FETCH-ROW", Support::BackendOnly(SQL), Native::Host(cap::SQL)),
    // Every file verb leaves its status in the module, where this reads it (T39c).
    c("FILE-STATUS", Support::BackendOnly(FILES), Native::Module),
    // `showDirectoryPicker` is Chrome's only: Q7's parity rule leaves it out.
    c("FOLDER-DIALOG", Support::Unsupported("a browser page cannot choose a folder in every engine (showDirectoryPicker is Chrome's only)"), Native::Host(cap::DIALOGS)),
    c("GET-PROPERTY", Support::Module, Native::Form),
    // The runner keeps each window's client and runs these with the interpreter's own (T42).
    c("HTTP-CLEAR-HEADERS", Support::BackendOnly(HTTP), Native::Host(cap::HTTP)),
    c("HTTP-DELETE", Support::BackendOnly(HTTP), Native::Host(cap::HTTP)),
    c("HTTP-DOWNLOAD", Support::BackendOnly(HTTP), Native::Host(cap::HTTP)),
    c("HTTP-GET", Support::BackendOnly(HTTP), Native::Host(cap::HTTP)),
    c("HTTP-POST", Support::BackendOnly(HTTP), Native::Host(cap::HTTP)),
    c("HTTP-PUT", Support::BackendOnly(HTTP), Native::Host(cap::HTTP)),
    c("HTTP-SET-HEADER", Support::BackendOnly(HTTP), Native::Host(cap::HTTP)),
    c("IMAGE-FIT", Support::BackendOnly(FILES), Native::Host(cap::IMAGES)),
    c("IMAGE-INFO", Support::BackendOnly(FILES), Native::Host(cap::IMAGES)),
    c("INIT-FORM", Support::Module, Native::Form),
    // The runner answers the model list, the keys and the providers with the interpreter's rules (T48, A53).
    c("KEY-IS-SET", Support::BackendOnly(SECRETS), Native::Host(cap::MODELS)),
    c("KEY-REMOVE", Support::BackendOnly(SECRETS), Native::Host(cap::MODELS)),
    c("KEY-SET", Support::BackendOnly(SECRETS), Native::Host(cap::MODELS)),
    c("LAYOUT", Support::Unsupported("the player has no command to install a layout script at run time yet (spec 109 S6, WASM-PARITY #47)"), cargo(D_MEDIA)),
    // The runner searches the files the form's AgentObjects allowed, with the interpreter's function (T48, A54).
    c("MCP-SEARCH", Support::BackendOnly(SECRETS), Native::Host(cap::MCP_SEARCH)),
    c("MODEL-COUNT", Support::BackendOnly(SECRETS), Native::Host(cap::MODELS)),
    c("MODEL-GET", Support::BackendOnly(SECRETS), Native::Host(cap::MODELS)),
    c("MODEL-LIST", Support::BackendOnly(SECRETS), Native::Host(cap::MODELS)),
    c("MODEL-LIST-GET", Support::BackendOnly(SECRETS), Native::Host(cap::MODELS)),
    c("MODEL-REMOVE", Support::BackendOnly(SECRETS), Native::Host(cap::MODELS)),
    c("MODEL-SET", Support::BackendOnly(SECRETS), Native::Host(cap::MODELS)),
    c("MODEL-TEST", Support::BackendOnly(SECRETS), Native::Host(cap::MODELS)),
    c("NEXT-ROW", Support::BackendOnly(SQL), Native::Host(cap::SQL)),
    c("OPEN-DB", Support::BackendOnly(SQL), Native::Host(cap::SQL)),
    c("OPEN-FILE-DIALOG", Support::Unsupported(NO_PATHS), Native::Host(cap::DIALOGS)),
    c("PARTIQL", Support::BackendOnly(FILES), Native::Host(cap::PARTIQL_FILES)),
    c("PARTIQL-ERROR", Support::BackendOnly(FILES), Native::Host(cap::PARTIQL_FILES)),
    c("PARTIQL-FETCH", Support::BackendOnly(FILES), Native::Host(cap::PARTIQL_FILES)),
    c("PROVIDER-COUNT", Support::BackendOnly(SECRETS), Native::Host(cap::MODELS)),
    c("PROVIDER-GET", Support::BackendOnly(SECRETS), Native::Host(cap::MODELS)),
    c("RAISE-EVENT", Support::Module, Native::Form),
    c("ROW-COUNT", Support::BackendOnly(SQL), Native::Host(cap::SQL)),
    c("SAVE-FILE-DIALOG", Support::Unsupported(NO_PATHS), Native::Host(cap::DIALOGS)),
    // The phone shell's verbs (WASM-012 R9, ABI A26): a Mobile module compiles
    // them onto the application object; a Web module refuses them at their
    // line before this row is read (`lower_remote::is_mobile_verb`).
    web("SECUREDELETE", PHONE_STORE, BOTH, cargo(D_PHONE)),
    web("SECURELOAD", PHONE_STORE, BOTH, cargo(D_PHONE)),
    web("SECURESTORE", PHONE_STORE, BOTH, cargo(D_PHONE)),
    c("SET-PROPERTY", Support::Module, Native::Form),
    c("WAIT-EVENT", Support::Module, Native::Form),
    c("WRITE-FILE", Support::BackendOnly(FILES), Native::Host(cap::FILES_TEXT)),
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
/// or the arguments, `env`); `STOP RUN` and `GOBACK` end the run in the module
/// (DESKTOP-WASM-001 T12). `RETURN-CODE` is a whole-number register there, handed to
/// the runner's `exit` when the run ends (T15, ABI A30); a use it cannot follow (a
/// fraction, characters, DIVIDE, ON SIZE ERROR, a non-numeric use) is refused, so the
/// program takes the Cargo path.
pub const STATEMENT_TABLE: &[(&str, Support, Native)] = &[
    ("ACCEPT", Support::Module, Native::Host(cap::CONSOLE)),
    ("ADD", Support::Module, Native::Module),
    ("ADD CORRESPONDING", Support::Module, Native::Module),
    ("ALTER", Support::Module, Native::Module),
    ("CALL", Support::Module, Native::Module),
    ("CANCEL", Support::Module, Native::Module),
    ("CLOSE", Support::MemoryFiles(DISK_FILES), Native::Files),
    ("COMMIT", Support::BackendOnly(BACKEND_FILES), Native::Files),
    ("COMPUTE", Support::Module, Native::Module),
    ("CONTINUE", Support::Module, Native::Module),
    ("DELETE", Support::MemoryFiles(DISK_FILES), Native::Files),
    ("DISPLAY", Support::Module, Native::Module),
    ("DIVIDE", Support::Module, Native::Module),
    ("EVALUATE", Support::Module, Native::Module),
    ("EXEC RUST", Support::Unsupported("EXEC RUST needs a native build; move it to the backend"), Native::Cargo(D_EXEC_RUST)),
    ("EXEC SQL", Support::BackendOnly("embedded SQL runs on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE"), Native::Host(cobolt_wasm_abi::codes::cap::SQL)),
    ("EXIT", Support::Module, Native::Module),
    ("GO TO", Support::Module, Native::Module),
    ("GO TO DEPENDING", Support::Module, Native::Module),
    ("GOBACK", Support::Module, Native::Module),
    ("IF", Support::Module, Native::Module),
    ("INITIALIZE", Support::Module, Native::Module),
    ("INSPECT", Support::Module, Native::Module),
    ("INVOKE", Support::Module, Native::Form),
    ("MERGE", Support::BackendOnly("MERGE reads files, which are on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE"), Native::Files),
    ("MOVE", Support::Module, Native::Module),
    ("MOVE CORRESPONDING", Support::Module, Native::Module),
    ("MULTIPLY", Support::Module, Native::Module),
    ("NEXT SENTENCE", Support::Module, Native::Module),
    ("OPEN", Support::MemoryFiles(DISK_FILES), Native::Files),
    ("PERFORM", Support::Module, Native::Module),
    ("READ", Support::MemoryFiles(DISK_FILES), Native::Files),
    ("RELEASE", Support::MemoryFiles(SORT_FILES), Native::Module),
    ("RETURN", Support::MemoryFiles(SORT_FILES), Native::Module),
    ("REWRITE", Support::MemoryFiles(DISK_FILES), Native::Files),
    ("ROLLBACK", Support::BackendOnly(BACKEND_FILES), Native::Files),
    ("SEARCH", Support::Module, Native::Module),
    ("SET (pointer)", Support::BackendOnly(POINTERS), Native::Module),
    ("SORT", Support::MemoryFiles(SORT_FILES), Native::Files),
    ("START", Support::MemoryFiles(DISK_FILES), Native::Files),
    ("STOP", Support::Module, Native::Module),
    ("STRING", Support::Module, Native::Module),
    ("SUBTRACT", Support::Module, Native::Module),
    ("SUBTRACT CORRESPONDING", Support::Module, Native::Module),
    ("THROW", Support::Module, Native::Module),
    ("TRY", Support::Module, Native::Module),
    ("UNLOCK", Support::BackendOnly(BACKEND_FILES), Native::Files),
    ("UNSTRING", Support::Module, Native::Module),
    ("WRITE", Support::MemoryFiles(DISK_FILES), Native::Files),
    ("control assignment", Support::Module, Native::Form),
    ("end of sentence", Support::Module, Native::Module),
    ("member call", Support::Module, Native::Form),
    ("window operation", Support::Module, Native::Form),
];

/// The browser class of [`STATEMENT_TABLE`]'s row for a statement name.
pub fn statement_entry(name: &str) -> Option<Support> {
    STATEMENT_TABLE.iter().find(|(n, _, _)| *n == name).map(|(_, s, _)| *s)
}

/// The Desktop class of [`STATEMENT_TABLE`]'s row for a statement name (DESKTOP-WASM-001 R12).
pub fn statement_native(name: &str) -> Option<Native> {
    STATEMENT_TABLE.iter().find(|(n, _, _)| *n == name).map(|(_, _, c)| *c)
}

/// Why a desktop module cannot hold the statement `s` yet: the reason of its Desktop class when that
/// class is `Cargo`, or `Form` outside a form's program (`in_form` false) (DESKTOP-WASM-001 T16,
/// T33). A file verb and `SORT` are judged at their file instead, since a file held in memory and a
/// SORT with procedures are compiled into the module.
pub fn desktop_refusal(s: &Stmt, in_form: bool) -> Option<&'static str> {
    if matches!(support(s), Support::MemoryFiles(_)) {
        return None;
    }
    match statement_native(stmt_name(s))? {
        Native::Cargo(why) => Some(why),
        Native::Form => (!in_form).then_some(D_FORMS),
        Native::Module | Native::Host(_) | Native::Files => None,
    }
}

// ── The controls of a desktop form (DESKTOP-WASM-001 T33, plan §4.3) ─────────────────────────────

const D_SERVICE: &str = "the sign-in control is not on the WebAssembly path yet (DESKTOP-WASM-001 M6)";
/// The IndexedFile control's PartiQL members a desktop module runs (DESKTOP-WASM-001 T48, ABI A55):
/// the methods, which the runner runs by the interpreter's own rules (`cobolt_runtime::partiql::door`),
/// and the properties a statement answers in.
pub const PARTIQL_MEMBERS: &[&str] = &["Execute", "ExecuteTransaction", "Fetch", "Get", "ColumnNames", "ColumnCount", "ResultCount", "StatusCode", "LastError"];

/// The PartiQL methods that write the data items `StatusDataItem` and `CurrentRecordDataItem` name.
pub const PARTIQL_ITEM_WRITERS: &[&str] = &["Execute", "ExecuteTransaction", "Fetch"];

/// Why an IndexedFile that names the program's data items takes the Cargo path when the program runs a
/// statement on it (DESKTOP-WASM-001 T48): `StatusDataItem` receives each statement's FILE STATUS and
/// `CurrentRecordDataItem` the record `Fetch` reads, items the interpreter looks up by name at run time
/// and a module does not. The generated paragraphs of a form that declares the file write them as
/// verbs, which the WebAssembly path compiles.
pub const D_PARTIQL_ITEMS: &str = "the IndexedFile names the program's data items (StatusDataItem, CurrentRecordDataItem), which a PartiQL statement writes by name at run time under the interpreter; the WebAssembly path does not yet (DESKTOP-WASM-001 M6)";

/// Why the IndexedFile's binding members take the Cargo path (a control bound to a PartiQL query is
/// judged by [`desktop_binding_gap`]).
pub const D_PARTIQL_BINDING: &str = "a PartiQL binding is not on the WebAssembly path yet (DESKTOP-WASM-001 M6)";

/// The IndexedFile members a desktop module refuses, with why.
pub const PARTIQL_GAPS: &[(&str, &str)] = &[
    ("RefreshBinding", D_PARTIQL_BINDING),
    ("BindingQuery", D_PARTIQL_BINDING),
    ("StatusDataItem", D_PARTIQL_ITEMS),
    ("CurrentRecordDataItem", D_PARTIQL_ITEMS),
    // A property set by name at run time may be one of the two.
    ("SetProperty", D_PARTIQL_ITEMS),
];

/// Why the IndexedFile member `member` (in any case, with or without the `GET-` and `SET-` prefixes of
/// a property's accessors) takes the Cargo path, or `None` when the runner serves it (T48).
pub fn partiql_member_gap(member: &str) -> Option<&'static str> {
    let m = member.trim();
    let bare = ["GET-", "SET-"].iter().find_map(|p| m.get(..4).filter(|h| h.eq_ignore_ascii_case(p)).map(|_| &m[4..])).unwrap_or(m);
    PARTIQL_GAPS.iter().find(|(name, _)| name.eq_ignore_ascii_case(bare)).map(|(_, why)| *why)
}

/// The IndexedFile controls of `form` whose design names one of the program's data items
/// (`StatusDataItem`, `CurrentRecordDataItem`), as `FORM/CONTROL` in upper case, for
/// [`crate::module::Options::partiql_items`]: a statement run on one is refused ([`D_PARTIQL_ITEMS`]).
/// The form's generated paragraphs write the same items with verbs, so naming one is no gap by itself.
pub fn partiql_item_controls(form: &cobolt_forms::Form) -> Vec<String> {
    fn walk(form: &str, cs: &[cobolt_forms::Control], out: &mut Vec<String>) {
        for c in cs {
            let named = |k: &str| c.get_prop(k).is_some_and(|v| !v.as_str().trim().is_empty());
            if c.control_type == cobolt_forms::model::ControlType::IndexedFile && (named("StatusDataItem") || named("CurrentRecordDataItem")) {
                out.push(format!("{form}/{}", c.id).to_ascii_uppercase());
            }
            walk(form, &c.children, out);
        }
    }
    let mut out = Vec::new();
    walk(&form.name, &form.controls, &mut out);
    out
}
/// Why a SqlDatabase that names the program's data items takes the Cargo path (DESKTOP-WASM-001
/// T41c): `ConnectionDataItem` receives the handle `Open` gives and `ResultSetDataItem` the row
/// `Fetch` reads, an item the interpreter looks up by name at run time and a module does not.
pub const D_SQL_ITEMS: &str = "the SqlDatabase names the program's data items (ConnectionDataItem, ResultSetDataItem), which the interpreter looks up by name at run time; the WebAssembly path does not yet (DESKTOP-WASM-001 M6)";

/// The members of a SqlDatabase that name one of the program's data items, or write a property by
/// its name: a program that uses one takes the Cargo path ([`D_SQL_ITEMS`]).
pub const SQL_ITEM_MEMBERS: &[&str] = &["ConnectionDataItem", "ResultSetDataItem", "SetProperty"];

/// Is `member` one of [`SQL_ITEM_MEMBERS`], in any case, with or without the `GET-` and `SET-`
/// prefixes of a property's accessors?
pub fn is_sql_item_member(member: &str) -> bool {
    let m = member.trim();
    let bare = ["GET-", "SET-"].iter().find_map(|p| m.get(..4).filter(|h| h.eq_ignore_ascii_case(p)).map(|_| &m[4..])).unwrap_or(m);
    SQL_ITEM_MEMBERS.iter().any(|s| s.eq_ignore_ascii_case(bare))
}

/// The verbs of `COBOL::SqlDatabase` a desktop module runs (DESKTOP-WASM-001 T41d, ABI A44): the
/// SqlDatabase control's own, which the runner runs with the interpreter's code, and `Cancel` and
/// `IsBusy`, which every asynchronous object answers.
pub const SQL_OBJECT_VERBS: &[&str] = &[
    "Open", "Execute", "Exec", "Query", "BeginTransaction", "Commit", "Rollback", "IsInTransaction", "Fetch", "FetchAll", "ColumnNames",
    "ColumnCount", "ColumnName", "Close", "Cancel", "IsBusy",
];

/// The properties of `COBOL::SqlDatabase` a desktop module reads (T41d): the ones its verbs read and
/// write.
pub const SQL_OBJECT_PROPERTIES: &[&str] = &["ConnectionString", "SqlConnection", "Mode", "TimeoutMs", "LastError", "StatusCode", "ResultCount", "Busy"];

/// The properties of `COBOL::SqlDatabase` a desktop module writes (T41d): not `Mode`
/// ([`D_SQL_OBJECT_ASYNC`]) and not `Busy`, which the runtime keeps.
pub const SQL_OBJECT_WRITABLE: &[&str] = &["ConnectionString", "SqlConnection", "TimeoutMs", "LastError", "StatusCode", "ResultCount"];

/// Why a write to `COBOL::SqlDatabase::Mode` takes the Cargo path (T41d).
pub const D_SQL_OBJECT_ASYNC: &str = "an asynchronous statement of the program-level database (a write to its Mode) takes the Cargo path: its result arrives between events, with no handler to hear it (DESKTOP-WASM-001 M6)";

/// Why any other member of `COBOL::SqlDatabase` takes the Cargo path (T41d).
pub const D_SQL_OBJECT_MEMBER: &str = "a desktop module runs the database's verbs (Open, Execute, Exec, Query, BeginTransaction, Commit, Rollback, IsInTransaction, Fetch, FetchAll, ColumnNames, ColumnCount, ColumnName, Close, Cancel, IsBusy, each called with its parentheses) and its properties (ConnectionString, SqlConnection, Mode, TimeoutMs, LastError, StatusCode, ResultCount, Busy) one at a time; anything else takes the Cargo path";

/// Why a RestClient or a WebSearch that names the program's data items takes the Cargo path
/// (DESKTOP-WASM-001 T42b, T47a): a request's body is read from the item `RequestDataItem` names, and its answer and status
/// written to the items `ResponseDataItem` and `StatusDataItem` name, items the interpreter looks up
/// by name at run time and a module does not.
pub const D_REST_ITEMS: &str = "the control names the program's data items (RequestDataItem, ResponseDataItem, StatusDataItem), which the interpreter looks up by name at run time; the WebAssembly path does not yet (DESKTOP-WASM-001 M6)";

/// The members of a RestClient or a WebSearch that name one of the program's data items, or write a
/// property by its name: a program that uses one takes the Cargo path ([`D_REST_ITEMS`]). The
/// interpreter writes a search's answer into the items a WebSearch names as it does a RestClient's.
pub const REST_ITEM_MEMBERS: &[&str] = &["RequestDataItem", "ResponseDataItem", "StatusDataItem", "SetProperty"];

/// Is `member` one of [`REST_ITEM_MEMBERS`], in any case, with or without the `GET-` and `SET-`
/// prefixes of a property's accessors?
pub fn is_rest_item_member(member: &str) -> bool {
    let m = member.trim();
    let bare = ["GET-", "SET-"].iter().find_map(|p| m.get(..4).filter(|h| h.eq_ignore_ascii_case(p)).map(|_| &m[4..])).unwrap_or(m);
    REST_ITEM_MEMBERS.iter().any(|s| s.eq_ignore_ascii_case(bare))
}

/// Why an AgentObject that names the program's data item takes the Cargo path (DESKTOP-WASM-001
/// T47c): the reply is written into the item `ResponseDataItem` names, an item the interpreter looks
/// up by name at run time and a module does not.
pub const D_AGENT_ITEMS: &str = "the AgentObject names the program's data item (ResponseDataItem), which the interpreter looks up by name at run time; the WebAssembly path does not yet (DESKTOP-WASM-001 M6)";

/// Why an AgentObject's `AllowFile` that names its file in a data item takes the Cargo path.
pub const D_AGENT_FILE_NAME: &str = "the AgentObject's AllowFile names its file in a data item, which the interpreter looks up by name at run time; the WebAssembly path takes the file's name as a literal (DESKTOP-WASM-001 M6)";

/// Why an AgentObject's `AllowFile` that names a file that is not INDEXED takes the Cargo path.
pub const D_AGENT_FILE_ORG: &str = "the AgentObject's AllowFile names a file that is not INDEXED; the WebAssembly path lays out only an INDEXED file's records for a model's search (DESKTOP-WASM-001 M6)";

/// The members of an AgentObject that name one of the program's data items, or write a property by
/// its name ([`D_AGENT_ITEMS`]).
pub const AGENT_ITEM_MEMBERS: &[&str] = &["ResponseDataItem", "SetProperty"];

/// The members of an AgentObject that hand a model a Knowledge Base collection, which the runner
/// searches itself with the KnowledgeBase's settings (T47c, ABI A52): the module requires `agent.kb`.
pub const AGENT_KB_MEMBERS: &[&str] = &["AllowKnowledgeBase", "DenyKnowledgeBase"];

/// The members of an AgentObject that offer a model an indexed file, which the runner searches itself
/// (T47c, ABI A51): the module requires `agent.files`.
pub const AGENT_FILE_MEMBERS: &[&str] = &["AllowFile", "DenyFile", "RegisterFile", "UnregisterFile"];

/// Why the AgentObject member `member` (in any case, with or without the `GET-` and `SET-` prefixes
/// of a property's accessors) takes the Cargo path, or `None` when the runner serves it.
pub fn agent_member_gap(member: &str) -> Option<&'static str> {
    let m = member.trim();
    let bare = ["GET-", "SET-"].iter().find_map(|p| m.get(..4).filter(|h| h.eq_ignore_ascii_case(p)).map(|_| &m[4..])).unwrap_or(m);
    AGENT_ITEM_MEMBERS.iter().any(|s| s.eq_ignore_ascii_case(bare)).then_some(D_AGENT_ITEMS)
}

/// Why the desktop control `c` takes the Cargo path for what its design holds, beyond its type
/// ([`desktop_control_gap`]): a SqlDatabase ([`D_SQL_ITEMS`]), a RestClient or a WebSearch
/// ([`D_REST_ITEMS`]), or an AgentObject ([`D_AGENT_ITEMS`]), that names a data item.
pub fn desktop_design_gap(c: &cobolt_forms::Control) -> Option<&'static str> {
    use cobolt_forms::model::ControlType as T;
    let named = |k: &str| c.get_prop(k).is_some_and(|v| !v.as_str().trim().is_empty());
    match c.control_type {
        T::SqlDatabase => (named("ConnectionDataItem") || named("ResultSetDataItem")).then_some(D_SQL_ITEMS),
        T::RestClient | T::WebSearch => (named("RequestDataItem") || named("ResponseDataItem") || named("StatusDataItem")).then_some(D_REST_ITEMS),
        T::AgentObject => named("ResponseDataItem").then_some(D_AGENT_ITEMS),
        _ => None,
    }
}

/// Why a project whose KnowledgeBase uses the built-in semantic model takes the Cargo path
/// (DESKTOP-WASM-001 T47b): the runner carries the Knowledge Base without the model, which would make every
/// program built on the WebAssembly path larger by the model's engine; the Build links it into a program
/// of its own instead. A project setting, `[rag] embedder = "builtin"`, not a control's design.
pub const D_KB_SEMANTIC: &str = "the project links the built-in semantic model ([rag] embedder = \"builtin\"), which the PowerRustCOBOL runner does not carry; the Cargo path builds it into the program";

const D_VIEWER: &str = "the Viewer's documents and verbs are not on the WebAssembly path yet (DESKTOP-WASM-001 M6)";
const D_UNKNOWN: &str = "a control of a type the WebAssembly path does not know takes the Cargo path (DESKTOP-WASM-001 M6)";

/// Why the runner does not serve a control of type `t` in a desktop form yet, or `None` when it does
/// (DESKTOP-WASM-001 T33, plan §4.3, R13). The runner serves a control whose whole state the ABI
/// host holds, as the browser's player does, and the form host draws: every class A and B control
/// of the web table ([`cobolt_forms::web_class`]), the DataGrid with rows of its own (a grid bound
/// to data is judged by [`desktop_binding_gap`]), and since T50 the canvases and the graphics
/// objects (class E), which the runner draws with the interpreter's own graphics library, and since
/// T48 the AWS controls, whose operations the runner carries out with the interpreter's own
/// connections and route table (A38), and since T39c the FileDropZone, whose files the form host
/// takes in and whose `CommitFiles()` the runner copies with the interpreter's own code, and the
/// IndexedFile control, whose generated paragraphs reach the file with the program's own verbs and
/// `COBOL::"FILE-STATUS"` (since T48 its PartiQL members run in the runner, [`PARTIQL_MEMBERS`];
/// [`partiql_member_gap`] names the ones that do not), and since T41c
/// the SqlDatabase, whose methods the runner runs with the interpreter's own code (one that names a
/// data item is judged by [`desktop_design_gap`]), and since T42b the RestClient, whose requests the
/// runner makes by the interpreter's own rules (`cobolt_runtime::rest_door`) with the window's HTTP
/// client (one that names a data item is judged by [`desktop_design_gap`] too), and since T47a the
/// WebSearch, whose searches are RestClient requests built by its own rules
/// (`cobolt_runtime::search_door`), and since T47b the KnowledgeBase, whose
/// methods and background operations the runner carries out by the interpreter's own rules
/// (`cobolt_runtime::kb_door`; a project that links the built-in semantic model is judged by the
/// Build, [`D_KB_SEMANTIC`]), and since T47c the AgentObject, whose `Ask`, streamed reply,
/// continuations and declared tools the runner carries out by the interpreter's own rules
/// (`cobolt_runtime::agent_door`; its runtime-answered tools and a data item it names are judged per
/// member, [`agent_member_gap`], and by [`desktop_design_gap`]), and since T48 the Maps control, whose
/// places, directions and road routes the runner asks for and whose markers, routes and regions it
/// edits by the interpreter's own rules (`cobolt_runtime::maps_door`). It does not serve yet: the
/// sign-in control (class C), the Viewer (its documents and verbs) and a type it does not know
/// (class D). A form
/// holding one takes the Cargo path, which names the form and the control.
pub fn desktop_control_gap(t: &cobolt_forms::model::ControlType) -> Option<&'static str> {
    use cobolt_forms::model::ControlType as T;
    use cobolt_forms::web_class::{class_of, Class};
    match t {
        T::DataGrid | T::FileDropZone | T::IndexedFile | T::SqlDatabase | T::RestClient | T::WebSearch | T::KnowledgeBase | T::AgentObject | T::Maps => None,
        t if t.is_aws() => None,
        T::Viewer => Some(D_VIEWER),
        _ => match class_of(t) {
            Class::A | Class::B | Class::E => None,
            Class::C => Some(D_SERVICE),
            Class::D => Some(D_UNKNOWN),
        },
    }
}

/// The data a control can be bound to (Data Binding), and why a control bound to it takes the Cargo
/// path (DESKTOP-WASM-001 T33): a binding is filled by the interpreter's runtime, which a module
/// does not carry yet. One row per source, in [`desktop_binding_gap`]'s order.
pub const DESKTOP_BINDING_GAPS: &[(&str, &str)] = &[
    // The program's own file verbs are the runner's since T38; a bound grid is filled, refreshed and
    // saved back by the grid's binding, which moves with the other bound controls.
    ("an indexed file", "a control bound to an indexed file is not on the WebAssembly path yet (DESKTOP-WASM-001 M6)"),
    ("a SQL query", "a control bound to a SQL query is not on the WebAssembly path yet (DESKTOP-WASM-001 M6)"),
    ("a REST endpoint", "a control bound to a REST endpoint is not on the WebAssembly path yet (DESKTOP-WASM-001 M6)"),
    ("a COBOL table", "a control bound to a COBOL table is not on the WebAssembly path yet (DESKTOP-WASM-001 M6)"),
    ("an agent's output", "a control bound to an agent's output is not on the WebAssembly path yet (DESKTOP-WASM-001 M6)"),
    ("a PartiQL query", "a control bound to a PartiQL query is not on the WebAssembly path yet (DESKTOP-WASM-001 M6)"),
];

/// Why the runner does not serve a control bound to `source` yet ([`DESKTOP_BINDING_GAPS`]). Every
/// source has a reason: a form with a binding takes the Cargo path.
pub fn desktop_binding_gap(source: &cobolt_forms::model::BindingSourceDescriptor) -> &'static str {
    use cobolt_forms::model::BindingSourceDescriptor as S;
    let row = match source {
        S::IndexedFile { .. } => 0,
        S::Sql { .. } => 1,
        S::RestApi { .. } => 2,
        S::CobolTable { .. } => 3,
        S::AgentAi { .. } => 4,
        S::PartiQl { .. } => 5,
    };
    DESKTOP_BINDING_GAPS[row].1
}
