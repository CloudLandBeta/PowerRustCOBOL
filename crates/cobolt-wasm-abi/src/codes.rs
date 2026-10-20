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
    /// First and last opcode of the queries.
    pub const QUERY_FIRST: u8 = 0x10;
    pub const QUERY_LAST: u8 = 0x1F;
    /// The property's text.
    pub const GET: u8 = 0x10;
    /// The method's result as text.
    pub const CALLR: u8 = 0x11;
    /// The number of members of a collection, as decimal text.
    pub const COUNT: u8 = 0x12;

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
}

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
pub const CONFORMANCE_IMPORTS: &[&str] = &["file_open", "file_read", "file_write", "file_close"];

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
/// disposal test (WASM-017 R20) reads them.
pub const DEBUG_EXPORTS: &[&str] = &["pr_dbg_live", "pr_dbg_region_used"];

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
