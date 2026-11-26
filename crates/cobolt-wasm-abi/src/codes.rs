// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The numeric constants of ABI v1 (`specs/WASM-ABI-v1.md`): opcodes, statuses,
//! error codes, reserved control ids and the variant bits.

/// The ABI version this crate implements (`pr_abi()` of a module).
pub const ABI_VERSION: u32 = 1;

/// The import namespace every host import lives in.
pub const IMPORT_MODULE: &str = "pr";

/// Opcodes of the command batch (§5) and the queries (§6).
pub mod op {
    /// Assign a property or an indexed member.
    pub const SET: u8 = 0x01;
    /// Invoke a method and discard any result.
    pub const CALL: u8 = 0x02;
    /// `DISPLAY` output.
    pub const DISPLAY: u8 = 0x03;
    /// A run-time error with its COBOL position.
    pub const ERROR: u8 = 0x04;
    /// Diagnostic output.
    pub const LOG: u8 = 0x05;
    /// Assign a property named by text, not by a names-table id (A14).
    pub const SET_NAMED: u8 = 0x06;
    /// Queue an event of a control, as the control would raise it (A14).
    pub const RAISE: u8 = 0x07;
    /// Draw a canvas's display list, which the module left in the arena's graphics region (A23).
    pub const DRAW: u8 = 0x08;
    /// A piece of an argument of the next command that has a path, sent ahead because the module's
    /// path had no room for it (A37).
    pub const PART: u8 = 0x09;
    /// First and last opcode of the queries.
    pub const QUERY_FIRST: u8 = 0x10;
    pub const QUERY_LAST: u8 = 0x1F;
    /// The property's text.
    pub const GET: u8 = 0x10;
    /// The method's result as text.
    pub const CALLR: u8 = 0x11;
    /// The number of members of a collection, as decimal text.
    pub const COUNT: u8 = 0x12;
    /// The property's text, from a read the module can wait for (A24): the host may answer
    /// [`super::err::SUSPENDED_QUERY`] and the module asks again after `pr_resume`.
    pub const GET_WAIT: u8 = 0x13;
    /// The method's result as text, from a read the module can wait for (A24).
    pub const CALLR_WAIT: u8 = 0x14;
    /// [`GET_WAIT`] asked by a statement that has an `ON EXCEPTION` (A25): the same path and the same
    /// answers, but a failure is the program's to handle: the host reports no run-time error and only
    /// answers the error code.
    pub const GET_WAIT_SOFT: u8 = 0x15;
    /// [`CALLR_WAIT`] asked by a statement that has an `ON EXCEPTION` (A25).
    pub const CALLR_WAIT_SOFT: u8 = 0x16;

    /// `true` for an opcode in the query range.
    pub const fn is_query(op: u8) -> bool {
        op >= QUERY_FIRST && op <= QUERY_LAST
    }
}

/// Reserved control ids (§5). Controls of a form are numbered from [`FIRST_CONTROL`].
pub mod ctrl {
    /// The form itself (`THIS`).
    pub const FORM: u32 = 0;
    /// The form that opened this one (`super::`).
    pub const SUPER: u32 = 1;
    /// The application object (`COBOL::` host-side verbs).
    pub const APP: u32 = 2;
    /// The id of the first control of a form.
    pub const FIRST_CONTROL: u32 = 3;
    /// A window the program holds a handle to (A40): a windowHandler, the `USAGE OBJECT` item an
    /// `OpenFormSync` / `OpenFormAsync … RETURNING` filled. The path's first segment names the item
    /// and carries the handle's text as its one argument; the segments after it are a path on that
    /// window's form, as on `super::` they are on the opener's. No form has this many controls.
    pub const HANDLE: u32 = u32::MAX;
}

/// The most characters a window handle has (A40): a module keeps a windowHandler in a text of this length.
pub const WINDOW_HANDLE_LEN: u32 = 16;

/// The return value of `pr_event` / `pr_resume` (§4).
pub mod status {
    pub const DONE: i32 = 0;
    pub const SUSPENDED: i32 = 1;
    pub const YIELDED: i32 = 2;
    pub const STOPPED: i32 = 3;
}

/// Error codes (§9). Negative as returned by the host; the `ERROR` command
/// carries the magnitude (or a COBOL run-time error number from 1000 up).
pub mod err {
    pub const GENERIC: i32 = -1;
    pub const BAD_ARGUMENT: i32 = -2;
    pub const NO_FREE_INSTANCE: i32 = -3;
    pub const UNKNOWN_TARGET: i32 = -4;
    pub const PAYLOAD_TOO_LARGE: i32 = -5;
    pub const HOST_REFUSED: i32 = -6;
    pub const NOT_CONNECTED: i32 = -7;
    pub const CANCELLED: i32 = -8;
    pub const TIMEOUT: i32 = -9;
    pub const TRAPPED: i32 = -10;
    /// The answer to a `GET_WAIT` or `CALLR_WAIT` on [`super::ctrl::SUPER`] is in another window
    /// (A24): the host has sent the request, the module returns `SUSPENDED` and asks again after
    /// `pr_resume`. A plain `GET` or `CALLR` is never answered this.
    pub const SUSPENDED_QUERY: i32 = -11;

    /// A short English name for a code, for logs.
    pub fn name(code: i32) -> &'static str {
        match code {
            GENERIC => "generic error",
            BAD_ARGUMENT => "bad argument",
            NO_FREE_INSTANCE => "no free form instance",
            UNKNOWN_TARGET => "unknown form, control or event",
            PAYLOAD_TOO_LARGE => "payload too large for the arena",
            HOST_REFUSED => "refused by the host",
            NOT_CONNECTED => "not connected",
            CANCELLED => "cancelled",
            TIMEOUT => "timeout",
            TRAPPED => "the module trapped",
            SUSPENDED_QUERY => "the answer is in another window; ask again after resume",
            _ => "error",
        }
    }
}

/// Bits of `pr_variant()` (§4).
pub mod variant {
    pub const MULTI_MEMORY: i32 = 1;
    pub const CONFORMANCE: i32 = 2;
}

/// The imports a production module may declare (§3), in the order of the table.
pub const PRODUCTION_IMPORTS: &[&str] = &["flush", "query", "remote", "now", "random", "abort"];

/// The extra imports of a conformance build (§8). A page refuses a module that has them.
pub const CONFORMANCE_IMPORTS: &[&str] = &["file_open", "file_read", "file_write", "file_close", "switch_state"];

/// The import namespace of the native runner (A27). A module that imports from it needs a capability
/// of [`cap`], and runs only on the runner.
pub const NATIVE_IMPORT_MODULE: &str = "prn";

/// The built-ins of the application's model list, its keys and the model providers (A53): each with
/// the fewest arguments it runs with and the arguments it writes, 0-based, in the order it writes them.
/// A desktop module sends the call to the application object with every argument as text, then moves
/// the application's `BuiltinAnswer<n>` into argument `n` (1-based) of each one it writes and has.
/// `cobolt_runtime::model_door::VERBS` is the same table: the runner's tests hold the two equal.
pub const MODEL_VERBS: &[(&str, usize, &[usize])] = &[
    ("MODEL-SET", 4, &[4]),
    ("MODEL-REMOVE", 1, &[1]),
    ("MODEL-COUNT", 1, &[0]),
    ("MODEL-GET", 2, &[1, 2, 3, 4]),
    ("KEY-SET", 1, &[2]),
    ("KEY-REMOVE", 1, &[1]),
    ("KEY-IS-SET", 2, &[1]),
    ("PROVIDER-COUNT", 1, &[0]),
    ("PROVIDER-GET", 2, &[1, 2, 3, 4]),
    ("MODEL-LIST", 4, &[3, 4]),
    ("MODEL-LIST-GET", 2, &[1]),
    ("MODEL-TEST", 4, &[4]),
];

/// The row of [`MODEL_VERBS`] for `name` (any case): the fewest arguments, and the arguments written.
pub fn model_verb(name: &str) -> Option<(usize, &'static [usize])> {
    let n = name.trim().to_ascii_uppercase();
    MODEL_VERBS.iter().find(|(v, _, _)| *v == n).map(|(_, min, out)| (*min, *out))
}

/// The capabilities a manifest may require (A27): what a module needs beyond ABI v1 §3. A manifest
/// without `capabilities` needs none. A name is never retired and never changes its meaning: a new
/// meaning is a new name.
pub mod cap {
    /// The operator's console: `ACCEPT` from it.
    pub const CONSOLE: &str = "console";
    /// The environment, the command line's arguments and the exit code.
    pub const ENV: &str = "env";
    pub const FILES_SEQUENTIAL: &str = "files.sequential";
    pub const FILES_LINE_SEQUENTIAL: &str = "files.line-sequential";
    pub const FILES_RELATIVE: &str = "files.relative";
    pub const FILES_INDEXED: &str = "files.indexed";
    /// `SORT` and `MERGE` with `USING` or `GIVING` files.
    pub const SORT: &str = "sort";
    /// `EXEC SQL` and the program's database object.
    pub const SQL: &str = "sql";
    pub const HTTP: &str = "http";
    /// The desktop's file and message dialogs, asked by the program.
    pub const DIALOGS: &str = "dialogs";
    /// The desktop's sound player: `COBOL::"AUDIO-PLAY"` and its four siblings (A36).
    pub const AUDIO: &str = "audio";
    /// The desktop's AWS controls and the keys typed in at run time: `COBOL::"AWS-SET-CREDENTIALS"` and
    /// its two siblings (A38).
    pub const AWS: &str = "aws";
    /// A line of text written to a file the program names: `COBOL::"APPEND-FILE"` and `"WRITE-FILE"` (A39).
    pub const FILES_TEXT: &str = "files.text";
    /// A picture's size read, and a picture fitted to a size: `COBOL::"IMAGE-INFO"` and `"IMAGE-FIT"` (A39).
    pub const IMAGES: &str = "images";
    /// The desktop's KnowledgeBase control: the application's document collections, indexed and searched
    /// on the runner's threads (A49).
    pub const KB: &str = "kb";
    /// The desktop's AgentObject: a model asked on the runner's threads, its reply streamed back, and
    /// the tools the program declares (A50).
    pub const AGENT: &str = "agent";
    /// The AgentObject's indexed-file tools, which the runner answers itself: `AllowFile`, `DenyFile`,
    /// `RegisterFile` and `UnregisterFile` (A51).
    pub const AGENT_FILES: &str = "agent.files";
    /// The AgentObject's Knowledge Base tools, which the runner answers itself with the form's
    /// KnowledgeBase: `AllowKnowledgeBase` and `DenyKnowledgeBase` (A52).
    pub const AGENT_KB: &str = "agent.kb";
    /// The application's model list, its keys and the model providers: `COBOL::"MODEL-SET"` and the
    /// other built-ins of [`super::MODEL_VERBS`], which the runner answers with the interpreter's own
    /// rules (A53).
    pub const MODELS: &str = "models";
    /// `COBOL::"MCP-SEARCH" ( tool-name arguments-json result )`, the in-process front door onto the
    /// files the form's AgentObjects let a model search, which the runner answers with the interpreter's
    /// own function (A54).
    pub const MCP_SEARCH: &str = "mcp.search";
    /// The IndexedFile control's PartiQL members (`Execute`, `ExecuteTransaction`, `Fetch`, `Get`,
    /// `ColumnNames`, `ColumnCount`), which the runner runs by the interpreter's own rules on its
    /// file store, and the `_START` each such control hears when its form starts (A55).
    pub const PARTIQL: &str = "partiql";
    /// `COBOL::"PARTIQL"`, `"PARTIQL-FETCH"` and `"PARTIQL-ERROR"` on the files the program's own
    /// `SELECT`s declare, which the runner runs by the interpreter's own rules on its file store (A56).
    pub const PARTIQL_FILES: &str = "partiql.files";
    /// `COBOL::"ANIMATIONSEQUENCE" ( action name [script | state] )`: the timed `SHOW`, `HIDE` and `PLAY`
    /// entries of a form instance's sequences, which the runner keeps and fires by the interpreter's own
    /// rules (A57).
    pub const SEQUENCES: &str = "sequences";
    /// `COBOL::"LAYOUT" ( action container [script] )`: a container's layout script installed as breakpoint
    /// overrides, which the runner reads by the interpreter's own rules and hands to the form host (A57).
    pub const LAYOUTS: &str = "layouts";
    /// The desktop's Viewer: its documents loaded, its conversation written, found in, saved, printed
    /// and shared, and the window's answers carried out, by the interpreter's own rules (A58).
    pub const VIEWER: &str = "viewer";

    /// Every capability, in the order of the table in ABI v1 §12 (A27). Only the native runner provides
    /// them: everything a page offers is ABI v1 §3, so the Web bundler refuses a module that requires one.
    pub const ALL: &[&str] = &[
        CONSOLE,
        ENV,
        FILES_SEQUENTIAL,
        FILES_LINE_SEQUENTIAL,
        FILES_RELATIVE,
        FILES_INDEXED,
        SORT,
        SQL,
        HTTP,
        DIALOGS,
        AUDIO,
        AWS,
        FILES_TEXT,
        IMAGES,
        KB,
        AGENT,
        AGENT_FILES,
        AGENT_KB,
        MODELS,
        MCP_SEARCH,
        PARTIQL,
        PARTIQL_FILES,
        SEQUENCES,
        LAYOUTS,
        VIEWER,
    ];

    /// What the shipped Backend runner provides (spec 114): the capabilities a program with no window can use, with
    /// the database group. The Build checks a Backend module's manifest against this list before it puts the module in
    /// a runner, so that a module the runner would refuse at start is sent to the Cargo path with the capability named.
    /// The runner's own list (`cobolt_runner::headless_capabilities`, built with `sql`) is held equal to this one by a test.
    pub const BACKEND_RUNNER: &[&str] = &[
        CONSOLE,
        ENV,
        FILES_SEQUENTIAL,
        FILES_LINE_SEQUENTIAL,
        FILES_RELATIVE,
        FILES_INDEXED,
        FILES_TEXT,
        IMAGES,
        HTTP,
        PARTIQL,
        PARTIQL_FILES,
        SQL,
    ];
}

/// The imports in [`NATIVE_IMPORT_MODULE`] each capability brings (A27). A capability's set is fixed
/// when a release first uses it; the capabilities not listed get theirs from the task that implements
/// them, which amends A27.
pub const NATIVE_IMPORTS: &[(&str, &[&str])] = &[
    (cap::CONSOLE, &["con_read"]),
    (cap::ENV, &["env_get", "exit", "tz_offset"]),
    // A34: every organisation's verbs are the same imports; the manifest names the capability of
    // each organisation the program opens.
    (cap::FILES_SEQUENTIAL, FILE_IMPORTS),
    (cap::FILES_LINE_SEQUENTIAL, FILE_IMPORTS),
    (cap::FILES_RELATIVE, FILE_IMPORTS),
    // A35: an INDEXED file's transactions and record locks add `f_tx`.
    (cap::FILES_INDEXED, INDEXED_FILE_IMPORTS),
    // A39: the text files and pictures a built-in names by its path.
    (cap::FILES_TEXT, &["t_write"]),
    (cap::IMAGES, &["img_fit", "img_info"]),
    // A41: every `EXEC SQL` statement, run by the runner with the interpreter's own SQL code
    // (`crate::sql`); A42: the SQL built-ins (`crate::sql::db`); A44: `COBOL::SqlDatabase` (`crate::sql::obj`).
    (cap::SQL, &["sql_exec", "sql_db", "sql_obj"]),
    // A45: the HTTP built-ins, run by the runner with the interpreter's own client (`crate::http`).
    (cap::HTTP, &["http_call"]),
    // A56: `COBOL::"PARTIQL"` and its two siblings, run by the runner with the interpreter's own rules
    // (`crate::partiql`).
    (cap::PARTIQL_FILES, &["pq_file"]),
];

/// The file verbs in [`NATIVE_IMPORT_MODULE`] (A34): a file is named by its number in the module's
/// `prfiles` section (`crate::files`), and records cross in memory 0.
pub const FILE_IMPORTS: &[&str] = &["f_open", "f_close", "f_read", "f_write", "f_rewrite", "f_delete", "f_start"];

/// The INDEXED organisation's imports (A35): the verbs, and `f_tx` for `COMMIT`, `ROLLBACK` and
/// `UNLOCK`, which act on INDEXED files only, as in the interpreter.
pub const INDEXED_FILE_IMPORTS: &[&str] = &["f_open", "f_close", "f_read", "f_write", "f_rewrite", "f_delete", "f_start", "f_tx"];

/// The exports a module must provide (§4), besides its memories.
pub const REQUIRED_EXPORTS: &[&str] = &[
    "pr_abi",
    "pr_variant",
    "pr_arena_base",
    "pr_init",
    "pr_open_form",
    "pr_close_form",
    "pr_event",
    "pr_resume",
    "pr_cancel",
    "pr_tick",
    "pr_status",
];

/// Optional diagnostics exports (§4): a production host ignores them; the
/// disposal test (WASM-017 R20) reads the first two; `pr_line` is the global
/// `$line` of §10 (A10).
pub const DEBUG_EXPORTS: &[&str] = &["pr_dbg_live", "pr_dbg_region_used", "pr_line"];

/// The decoded form of a `pr_event`/`pr_resume`/`pr_status` return value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Done,
    Suspended,
    Yielded,
    Stopped,
    Error(i32),
}

impl Status {
    /// Decode the integer a module returned. An unknown non-negative value is an
    /// error: a host must not guess what a newer module meant.
    pub fn from_code(code: i32) -> Status {
        match code {
            status::DONE => Status::Done,
            status::SUSPENDED => Status::Suspended,
            status::YIELDED => Status::Yielded,
            status::STOPPED => Status::Stopped,
            n if n < 0 => Status::Error(n),
            _ => Status::Error(err::GENERIC),
        }
    }

    pub fn code(self) -> i32 {
        match self {
            Status::Done => status::DONE,
            Status::Suspended => status::SUSPENDED,
            Status::Yielded => status::YIELDED,
            Status::Stopped => status::STOPPED,
            Status::Error(n) => n,
        }
    }
}
