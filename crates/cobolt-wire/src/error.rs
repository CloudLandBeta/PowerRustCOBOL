// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The failure of a remote call: a closed list of classes, a stable numeric
//! code, an English message for logs, and a place for the program's own detail
//! (WASM-014 R7, R8).
//!
//! A class is never added in a way that breaks a client that does not know it:
//! an unknown class read off the wire is [`ErrorClass::Server`].

use serde::{Deserialize, Serialize};

/// The class of a failed call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorClass {
    /// No connection, the connection closed, or the client's own timeout.
    Transport,
    /// The token is missing, invalid or expired.
    Authentication,
    /// The caller is known and may not do this (role refused).
    Authorisation,
    /// The payload does not fit the program's layout.
    Validation,
    /// No such exported program.
    NotFound,
    /// The client and the backend cannot talk (contract or protocol version).
    Version,
    /// A size, rate, concurrency or time limit.
    Limit,
    /// The program itself reported a failure (a rejected order, a duplicate key).
    Application,
    /// An unexpected fault on the backend.
    Server,
}

impl ErrorClass {
    /// Every class, in the order the wire documents them.
    pub const ALL: [ErrorClass; 9] = [
        ErrorClass::Transport,
        ErrorClass::Authentication,
        ErrorClass::Authorisation,
        ErrorClass::Validation,
        ErrorClass::NotFound,
        ErrorClass::Version,
        ErrorClass::Limit,
        ErrorClass::Application,
        ErrorClass::Server,
    ];

    /// The wire spelling (`not-found`).
    pub fn wire(self) -> &'static str {
        match self {
            ErrorClass::Transport => "transport",
            ErrorClass::Authentication => "authentication",
            ErrorClass::Authorisation => "authorisation",
            ErrorClass::Validation => "validation",
            ErrorClass::NotFound => "not-found",
            ErrorClass::Version => "version",
            ErrorClass::Limit => "limit",
            ErrorClass::Application => "application",
            ErrorClass::Server => "server",
        }
    }

    /// The word a COBOL program tests (`NOT-FOUND`).
    pub fn word(self) -> &'static str {
        match self {
            ErrorClass::Transport => "TRANSPORT",
            ErrorClass::Authentication => "AUTHENTICATION",
            ErrorClass::Authorisation => "AUTHORISATION",
            ErrorClass::Validation => "VALIDATION",
            ErrorClass::NotFound => "NOT-FOUND",
            ErrorClass::Version => "VERSION",
            ErrorClass::Limit => "LIMIT",
            ErrorClass::Application => "APPLICATION",
            ErrorClass::Server => "SERVER",
        }
    }

    /// Read a class off the wire; one this build does not know is [`Server`](Self::Server).
    pub fn from_wire(s: &str) -> ErrorClass {
        ErrorClass::ALL.iter().copied().find(|c| c.wire() == s).unwrap_or(ErrorClass::Server)
    }
}

/// The stable platform codes. An *application* failure carries the program's
/// own code instead, and its code space is its own.
pub mod codes {
    // transport
    pub const NO_CONNECTION: u32 = 1000;
    pub const CLOSED: u32 = 1001;
    pub const TIMEOUT: u32 = 1002;
    pub const NOT_CONFIGURED: u32 = 1003;
    // authentication
    pub const TOKEN_MISSING: u32 = 2000;
    pub const TOKEN_INVALID: u32 = 2001;
    pub const TOKEN_EXPIRED: u32 = 2002;
    // authorisation
    pub const ROLE_REFUSED: u32 = 3000;
    // validation
    pub const INVALID_PAYLOAD: u32 = 4000;
    pub const BAD_MESSAGE: u32 = 4001;
    // not found
    pub const UNKNOWN_PROGRAM: u32 = 5000;
    // version
    pub const CLIENT_TOO_OLD: u32 = 6000;
    pub const CLIENT_TOO_NEW: u32 = 6001;
    pub const PROTOCOL_UNSUPPORTED: u32 = 6002;
    // limit
    pub const FRAME_TOO_LARGE: u32 = 7000;
    pub const RATE_LIMITED: u32 = 7001;
    pub const TOO_MANY_CALLS: u32 = 7002;
    pub const CALL_TIMEOUT: u32 = 7003;
    pub const FILE_TOO_LARGE: u32 = 7004;
    // server
    pub const INTERNAL: u32 = 9000;
    pub const PROGRAM_FAULT: u32 = 9001;
    pub const SHUTTING_DOWN: u32 = 9002;
}

/// A failed call, as the caller sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallError {
    pub class: ErrorClass,
    pub code: u32,
    /// English, for logs and the developer. Never contains a token.
    pub message: String,
    /// The program's own detail (an application failure's data, as JSON text),
    /// or the path of an invalid payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// For a transport failure: could the backend have received the call?
    /// `None` when the question does not apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub may_have_run: Option<bool>,
}

impl CallError {
    pub fn new(class: ErrorClass, code: u32, message: impl Into<String>) -> Self {
        CallError { class, code, message: message.into(), detail: None, may_have_run: None }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn may_have_run(mut self, yes: bool) -> Self {
        self.may_have_run = Some(yes);
        self
    }

    pub fn transport(code: u32, message: impl Into<String>) -> Self {
        CallError::new(ErrorClass::Transport, code, message)
    }

    pub fn validation(message: impl Into<String>, path: impl Into<String>) -> Self {
        CallError::new(ErrorClass::Validation, codes::INVALID_PAYLOAD, message).with_detail(path)
    }
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}: {}", self.class.wire(), self.code, self.message)
    }
}

impl std::error::Error for CallError {}
