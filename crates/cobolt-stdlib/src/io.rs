// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! I/O backend abstraction for the Cobolt runtime.
//!
//! By hiding all I/O behind a trait the runtime can be tested without touching
//! stdin/stdout, and alternative front-ends (GUI, web, embedded) can swap in
//! their own implementation.

use std::io::{self, BufRead, Write};

// ── IoBackend ─────────────────────────────────────────────────────────────────

/// All I/O operations available to a running COBOL program.
pub trait IoBackend: Send {
    /// Write a line to standard output (DISPLAY … / DISPLAY … UPON SYSOUT).
    fn write_line(&mut self, text: &str);

    /// Write text without a trailing newline (DISPLAY … NO ADVANCING).
    fn write(&mut self, text: &str);

    /// Read a line from standard input (ACCEPT … from console).
    ///
    /// Returns `None` on EOF.
    fn read_line(&mut self) -> Option<String>;

    /// Write a line to the error output (DISPLAY … UPON SYSERR).
    fn write_err(&mut self, text: &str);
}

// ── ConsoleIo ─────────────────────────────────────────────────────────────────

/// `IoBackend` that uses the real stdin / stdout / stderr.
pub struct ConsoleIo {
    stdin:  io::Stdin,
    stdout: io::Stdout,
    stderr: io::Stderr,
}

impl ConsoleIo {
    pub fn new() -> Self {
        Self {
            stdin:  io::stdin(),
            stdout: io::stdout(),
            stderr: io::stderr(),
        }
    }
}

impl Default for ConsoleIo {
    fn default() -> Self { Self::new() }
}

impl IoBackend for ConsoleIo {
    fn write_line(&mut self, text: &str) {
        let _ = writeln!(self.stdout.lock(), "{text}");
    }

    fn write(&mut self, text: &str) {
        let _ = write!(self.stdout.lock(), "{text}");
        let _ = self.stdout.lock().flush();
    }

    fn read_line(&mut self) -> Option<String> {
        let mut buf = String::new();
        match self.stdin.lock().read_line(&mut buf) {
            Ok(0) => None,
            Ok(_) => {
                // Strip trailing newline/carriage-return.
                let trimmed = buf.trim_end_matches('\n').trim_end_matches('\r').to_owned();
                Some(trimmed)
            }
            Err(_) => None,
        }
    }

    fn write_err(&mut self, text: &str) {
        let _ = writeln!(self.stderr.lock(), "{text}");
    }
}

// ── NullIo ────────────────────────────────────────────────────────────────────

/// `IoBackend` that silently discards all output and returns empty strings for
/// reads.  Useful for tests that don't care about I/O.
#[derive(Debug, Default)]
pub struct NullIo;

impl IoBackend for NullIo {
    fn write_line(&mut self, _text: &str) {}
    fn write(&mut self, _text: &str) {}
    fn read_line(&mut self) -> Option<String> { Some(String::new()) }
    fn write_err(&mut self, _text: &str) {}
}

// ── CapturingIo ───────────────────────────────────────────────────────────────

/// `IoBackend` that collects output into `Vec<String>` — useful in tests that
/// verify program output.
#[derive(Debug, Default)]
pub struct CapturingIo {
    /// Lines written to standard output (one entry per `write_line` call).
    pub stdout_lines: Vec<String>,
    /// Lines written to standard error.
    pub stderr_lines: Vec<String>,
    /// Lines to feed back on `read_line` calls (consumed front-to-back).
    pub stdin_lines: std::collections::VecDeque<String>,
}

impl CapturingIo {
    pub fn new() -> Self { Self::default() }

    /// Pre-load lines to be returned by `read_line`.
    pub fn with_stdin(mut self, lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.stdin_lines = lines.into_iter().map(|s| s.into()).collect();
        self
    }

    /// Concatenate all stdout lines with newlines.
    pub fn stdout(&self) -> String {
        self.stdout_lines.join("\n")
    }
}

impl IoBackend for CapturingIo {
    fn write_line(&mut self, text: &str) {
        self.stdout_lines.push(text.to_owned());
    }

    fn write(&mut self, text: &str) {
        // Append to the last line without a newline.
        if let Some(last) = self.stdout_lines.last_mut() {
            last.push_str(text);
        } else {
            self.stdout_lines.push(text.to_owned());
        }
    }

    fn read_line(&mut self) -> Option<String> {
        self.stdin_lines.pop_front()
    }

    fn write_err(&mut self, text: &str) {
        self.stderr_lines.push(text.to_owned());
    }
}
