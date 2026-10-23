// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Structured logs (WASM-003 R21, WASM-013 R13).
//!
//! One record per finished call and per connection event, as one JSON line:
//! when, which call, which program, who (the token's subject), how long, how it
//! ended. **Never** a token, a password, a payload or a claim: a record has no
//! field that could hold one, which is stronger than a filter that tries to
//! remove them. Sizes are logged, contents are not.

use std::sync::{Arc, Mutex};

use serde::Serialize;

/// What happened.
#[derive(Debug, Clone, Serialize)]
pub struct LogRecord {
    /// Seconds since the epoch, with milliseconds.
    pub ts: f64,
    /// `call`, `connect`, `disconnect`, `refused`, `auth`, `start`, `stop`, `warning`.
    pub event: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    /// The token's `sub`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
    /// `ok` or the error class.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_in: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_out: Option<usize>,
    /// A fixed-vocabulary note (never data from the caller).
    /// `per-call` or `per-connection` on a call record: the mode, never what it kept.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl LogRecord {
    pub fn new(event: &'static str) -> Self {
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
        LogRecord { ts, event, call: None, connection: None, program: None, subject: None, duration_ms: None, outcome: None, code: None, bytes_in: None, bytes_out: None, state: None, note: None }
    }
}

/// Where records go.
pub trait LogSink: Send + Sync {
    fn emit(&self, record: &LogRecord);
}

/// JSON lines on standard output (the default of a deployment; the platform's
/// log shipper reads them).
pub struct StdoutSink;

impl LogSink for StdoutSink {
    fn emit(&self, record: &LogRecord) {
        if let Ok(line) = serde_json::to_string(record) {
            println!("{line}");
        }
    }
}

/// Records kept in memory — the tests', and the IDE's Output panel's.
#[derive(Default, Clone)]
pub struct MemorySink {
    records: Arc<Mutex<Vec<LogRecord>>>,
}

impl MemorySink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn records(&self) -> Vec<LogRecord> {
        self.records.lock().map(|r| r.clone()).unwrap_or_default()
    }

    /// Every record as the JSON line it would have been.
    pub fn lines(&self) -> Vec<String> {
        self.records().iter().filter_map(|r| serde_json::to_string(r).ok()).collect()
    }
}

impl LogSink for MemorySink {
    fn emit(&self, record: &LogRecord) {
        if let Ok(mut r) = self.records.lock() {
            r.push(record.clone());
        }
    }
}
