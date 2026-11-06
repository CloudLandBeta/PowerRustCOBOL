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
    // application: a program's own codes are 1..=7999; these belong to the platform
    /// The caller asked to cancel and the call stopped (WASM-014 R13).
    pub const CANCELLED: u32 = 8000;
    /// The highest code a backend program may report with `COBOL::"WS-FAIL"`.
    pub const APPLICATION_MAX: u32 = 7999;
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

/// The longest message the platform's own failures may carry. Many say what they found, and what they found may be a stranger's text (a
/// key, a slot, a number, a contract version): the message is cut, so a megabyte sent is never a megabyte answered, logged or shown
/// (WASM-013 T20: found by the dispatch fuzz). A program's own failure ([`ErrorClass::Application`]) says what the program says.
pub const MESSAGE_MAX: usize = 400;

/// `s` cut to `max` characters (on a character, never inside one), with `…` where it was cut.
pub fn clipped(mut s: String, max: usize) -> String {
    if let Some((cut, _)) = s.char_indices().nth(max) {
        s.truncate(cut);
        s.push('…');
    }
    s
}

impl CallError {
    pub fn new(class: ErrorClass, code: u32, message: impl Into<String>) -> Self {
        let message = message.into();
        let message = if class == ErrorClass::Application { message } else { clipped(message, MESSAGE_MAX) };
        CallError { class, code, message, detail: None, may_have_run: None }
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
        // The path names the keys the payload used, which are the sender's: it is cut like the message.
        CallError::new(ErrorClass::Validation, codes::INVALID_PAYLOAD, message).with_detail(clipped(path.into(), MESSAGE_MAX))
    }
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}: {}", self.class.wire(), self.code, self.message)
    }
}

impl std::error::Error for CallError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_platform_says_about_a_strangers_text_is_cut_and_a_programs_own_words_are_not() {
        let long = "x".repeat(100_000);
        // Every class the platform raises is bounded, whatever the code puts in the message.
        for class in ErrorClass::ALL.into_iter().filter(|c| *c != ErrorClass::Application) {
            let e = CallError::new(class, 1, format!("no member named \"{long}\""));
            assert!(e.message.chars().count() <= MESSAGE_MAX + 1, "{class:?}: {} characters", e.message.chars().count());
            assert!(e.message.starts_with("no member named"), "the part that says what happened stays");
            assert!(e.message.ends_with('…'));
        }
        // The path of an invalid payload names the sender's keys.
        let e = CallError::validation("unknown-key", format!("PAYLOAD.{long}"));
        assert!(e.detail.as_deref().is_some_and(|d| d.chars().count() <= MESSAGE_MAX + 1 && d.starts_with("PAYLOAD.")));
        // A program's own failure says what the program says, whole.
        let app = CallError::new(ErrorClass::Application, 12, "y".repeat(1200));
        assert_eq!(app.message.len(), 1200);
        // A short message is untouched; a cut falls on a character.
        assert_eq!(CallError::new(ErrorClass::Limit, 7000, "a message is larger").message, "a message is larger");
        let wide = clipped("日".repeat(500), 10);
        assert_eq!(wide.chars().count(), 11);
        assert!(wide.ends_with('…'));
    }
}
