// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The one error type of the crate (WASM-018 R15: "refuse past the limits with
//! a readable error").
//!
//! Every message is a complete English sentence a COBOL developer can act on;
//! the host (IDE, player) shows it as is or looks the stable [`GraphicsError::code`]
//! up in its own six-language table. Nothing in this crate ever panics on bad
//! input - bad input is an `Err`.

use std::fmt;

/// Result alias used across the crate.
pub type Result<T, E = GraphicsError> = std::result::Result<T, E>;

/// Everything that can go wrong when a program uses the graphics objects.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum GraphicsError {
    /// A configured [`Limits`](crate::limits::Limits) value would be exceeded.
    LimitExceeded {
        /// What was being created ("canvas pixels", "decoded image pixels", ...).
        what: &'static str,
        /// What the caller asked for.
        requested: u64,
        /// The limit in force.
        limit: u64,
        /// The name of the setting that raises the limit.
        setting: &'static str,
    },
    /// A display list is malformed (bad magic, truncated, unknown command ...).
    BadDisplayList(String),
    /// A handle that was never issued by this registry.
    UnknownHandle(u32),
    /// A handle that was valid once and has been released.
    ReleasedHandle(u32),
    /// A handle of one kind used where another is required.
    WrongHandleKind {
        /// The handle.
        handle: u32,
        /// What it names.
        found: &'static str,
        /// What the method needed.
        expected: &'static str,
    },
    /// A method name the object does not have.
    UnknownMethod {
        /// The object kind ("Canvas2D").
        object: &'static str,
        /// The name the program used.
        method: String,
    },
    /// Wrong number of arguments.
    ArgumentCount {
        /// "Canvas2D.FillRect".
        method: String,
        /// How many the method takes.
        expected: usize,
        /// How many were given.
        got: usize,
    },
    /// An argument of the wrong type or outside its range.
    BadArgument {
        /// "Canvas2D.SetLineCap".
        method: String,
        /// Zero-based position.
        index: usize,
        /// What is wrong.
        message: String,
    },
    /// An image or model could not be decoded (corrupt, unsupported, truncated).
    Decode(String),
    /// An image could not be encoded.
    Encode(String),
    /// A font file could not be used.
    Font(String),
    /// A feature that is compiled out or not available on this platform.
    Unsupported(String),
    /// No graphics adapter exists (3D only).
    NoAdapter(String),
    /// The graphics device was lost or a GPU operation failed (3D only).
    Gpu(String),
    /// Anything else a caller did wrong, with an explanation.
    Invalid(String),
}

impl GraphicsError {
    /// A short, stable, machine-readable code for the localisation tables.
    pub fn code(&self) -> &'static str {
        match self {
            Self::LimitExceeded { .. } => "limit-exceeded",
            Self::BadDisplayList(_) => "bad-display-list",
            Self::UnknownHandle(_) => "unknown-handle",
            Self::ReleasedHandle(_) => "released-handle",
            Self::WrongHandleKind { .. } => "wrong-handle-kind",
            Self::UnknownMethod { .. } => "unknown-method",
            Self::ArgumentCount { .. } => "argument-count",
            Self::BadArgument { .. } => "bad-argument",
            Self::Decode(_) => "decode",
            Self::Encode(_) => "encode",
            Self::Font(_) => "font",
            Self::Unsupported(_) => "unsupported",
            Self::NoAdapter(_) => "no-adapter",
            Self::Gpu(_) => "gpu",
            Self::Invalid(_) => "invalid",
        }
    }

    pub(crate) fn invalid(msg: impl Into<String>) -> Self {
        Self::Invalid(msg.into())
    }

    pub(crate) fn bad_list(msg: impl Into<String>) -> Self {
        Self::BadDisplayList(msg.into())
    }
}

/// Thousands separators so a limit message reads at a glance.
fn grouped(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

impl fmt::Display for GraphicsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded { what, requested, limit, setting } => write!(
                f,
                "{what}: {} requested, but the limit is {} (raise it with the \"{setting}\" setting)",
                grouped(*requested),
                grouped(*limit)
            ),
            Self::BadDisplayList(m) => write!(f, "the drawing commands are not valid: {m}"),
            Self::UnknownHandle(h) => write!(f, "handle {h} does not name anything"),
            Self::ReleasedHandle(h) => write!(f, "handle {h} was released and can no longer be used"),
            Self::WrongHandleKind { handle, found, expected } => {
                write!(f, "handle {handle} names a {found}, but this method needs a {expected}")
            }
            Self::UnknownMethod { object, method } => write!(f, "{object} has no method named {method}"),
            Self::ArgumentCount { method, expected, got } => {
                write!(f, "{method} takes {expected} argument(s), but {got} were given")
            }
            Self::BadArgument { method, index, message } => {
                write!(f, "argument {} of {method}: {message}", index + 1)
            }
            Self::Decode(m) => write!(f, "the data could not be read: {m}"),
            Self::Encode(m) => write!(f, "the image could not be written: {m}"),
            Self::Font(m) => write!(f, "font problem: {m}"),
            Self::Unsupported(m) => write!(f, "not supported here: {m}"),
            Self::NoAdapter(m) => write!(f, "no graphics adapter is available: {m}"),
            Self::Gpu(m) => write!(f, "graphics device error: {m}"),
            Self::Invalid(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for GraphicsError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_message_is_readable() {
        let e = GraphicsError::LimitExceeded {
            what: "canvas pixels",
            requested: 400_000_000,
            limit: 16_777_216,
            setting: "max_canvas_pixels",
        };
        let m = e.to_string();
        assert!(m.contains("400,000,000"), "{m}");
        assert!(m.contains("16,777,216"), "{m}");
        assert!(m.contains("max_canvas_pixels"), "{m}");
        assert_eq!(e.code(), "limit-exceeded");
    }

    #[test]
    fn grouping() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1000), "1,000");
        assert_eq!(grouped(1_234_567), "1,234,567");
    }
}
