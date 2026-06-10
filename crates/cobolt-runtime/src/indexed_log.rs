// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Optional per-file transaction log for INDEXED files (observability).
//!
//! When enabled (`rcrun --indexed-log <basic|full>` / `COBOL_INDEXED_LOG`), each
//! indexed file gets a sidecar log at `<assign-path>.log` (e.g. `customers.idx`
//! → `customers.idx.log`). One line is appended per transaction event (`OPEN`,
//! `COMMIT`, `ROLLBACK`, `CLOSE`) in a compact, greppable `key=value` format.
//!
//! Fields are cheap and self-tracked (record counts, bytes, duration, rates, and
//! the *ordering quality* of the written keys). The `full` level additionally
//! appends redb index statistics (tree height, page counts, stored/fragmented
//! bytes) on `CLOSE` — this walks the index, so its cost scales with file size
//! and it is therefore opt-in.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// How much to log.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LogLevel {
    /// Logging disabled.
    #[default]
    Off,
    /// Per-transaction metrics (cheap, self-tracked).
    Basic,
    /// `Basic` plus redb index statistics on CLOSE (walks the index).
    Full,
}

impl LogLevel {
    /// Parse a flag value: `false/off/0/no` → Off, `full/stats` → Full, anything
    /// else truthy (`true/on/1/basic/yes`) → Basic.
    pub fn parse(s: &str) -> LogLevel {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "false" | "off" | "0" | "no" | "none" => LogLevel::Off,
            "full" | "stats" | "verbose" => LogLevel::Full,
            _ => LogLevel::Basic,
        }
    }
    pub fn is_on(self) -> bool {
        self != LogLevel::Off
    }
}

/// Output format for the log lines.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LogFormat {
    /// `key=value` (logfmt) — Loki parses this with `| logfmt`.
    #[default]
    Text,
    /// One JSON object per line (NDJSON) — Loki parses this with `| json`.
    /// Numeric fields are emitted as JSON numbers so Grafana can graph them.
    Json,
}

impl LogFormat {
    pub fn parse(s: &str) -> LogFormat {
        match s.trim().to_ascii_lowercase().as_str() {
            "json" | "ndjson" | "grafana" | "loki" => LogFormat::Json,
            _ => LogFormat::Text,
        }
    }
}

/// One field value in a log record: a number (graphable) or a string (label).
pub enum FieldVal {
    U(u64),
    Str(String),
}

/// An ordered set of fields for one transaction event, rendered to either
/// logfmt (`Text`) or NDJSON (`Json`).
#[derive(Default)]
pub struct LogRecord {
    fields: Vec<(&'static str, FieldVal)>,
}

impl LogRecord {
    pub fn new() -> Self {
        LogRecord::default()
    }
    /// Add a numeric field (rendered as a bare number in JSON).
    pub fn num(&mut self, key: &'static str, v: u64) -> &mut Self {
        self.fields.push((key, FieldVal::U(v)));
        self
    }
    /// Add a string field.
    pub fn str(&mut self, key: &'static str, v: impl Into<String>) -> &mut Self {
        self.fields.push((key, FieldVal::Str(v.into())));
        self
    }

    /// Render the record to a single line in the requested format.
    pub fn render(&self, fmt: LogFormat) -> String {
        match fmt {
            LogFormat::Text => {
                let mut out = String::new();
                for (i, (k, v)) in self.fields.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    match v {
                        FieldVal::U(n) => {
                            out.push_str(k);
                            out.push('=');
                            out.push_str(&n.to_string());
                        }
                        FieldVal::Str(s) => out.push_str(&field(k, s)),
                    }
                }
                out
            }
            LogFormat::Json => {
                let mut out = String::from("{");
                for (i, (k, v)) in self.fields.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push('"');
                    out.push_str(k);
                    out.push_str("\":");
                    match v {
                        FieldVal::U(n) => out.push_str(&n.to_string()),
                        FieldVal::Str(s) => {
                            out.push('"');
                            out.push_str(&json_escape(s));
                            out.push('"');
                        }
                    }
                }
                out.push('}');
                out
            }
        }
    }
}

/// Minimal JSON string escaping (quotes, backslash, control chars).
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// The active log file is kept under this size; when the next line would exceed
/// it the file is rotated out and a fresh one is started (logrotate/Grafana style).
pub const MAX_LOG_BYTES: u64 = 100 * 1024;

/// Appends transaction lines to `<assign-path>.log`, rotating the file once it
/// approaches [`MAX_LOG_BYTES`]. A rotated file is renamed to
/// `<user|no-user>.<datafile>.log.<timestamp>` and a fresh active log is started.
pub struct LogWriter {
    /// Active log path, `<idx_path>.log`.
    path: PathBuf,
    /// Directory the rotated files are written into.
    dir: PathBuf,
    /// The data file's name, e.g. `customers.idx`.
    idx_name: String,
    /// Sanitized user for rotated file names (`no-user` when absent).
    user: String,
    file: Option<File>,
    /// Bytes in the active file (tracked to avoid a stat per line).
    written: u64,
    level: LogLevel,
}

impl LogWriter {
    /// Create a writer for the indexed file at `idx_path` (active log is
    /// `idx_path` + `.log`). `user` (from `OPEN … WITH REGISTERED USER`) is used
    /// in rotated file names. The file is opened lazily on the first line.
    pub fn new(idx_path: &Path, level: LogLevel, user: Option<&str>) -> Self {
        let mut os = idx_path.as_os_str().to_owned();
        os.push(".log");
        let path = PathBuf::from(os);
        let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let idx_name = idx_path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".to_string());
        LogWriter { path, dir, idx_name, user: sanitize_user(user), file: None, written: 0, level }
    }

    pub fn level(&self) -> LogLevel {
        self.level
    }

    fn ensure_open(&mut self) {
        if self.file.is_none() {
            self.file = OpenOptions::new().create(true).append(true).open(&self.path).ok();
            // Continue counting from whatever is already in the file (the log
            // persists and is appended across sessions).
            self.written = std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        }
    }

    /// Rotate the active log out to `<user>.<datafile>.log.<timestamp>` and start
    /// a fresh active log on the next write.
    fn rotate(&mut self) {
        self.file = None; // close before renaming
        let rotated = self
            .dir
            .join(format!("{}.{}.log.{}", self.user, self.idx_name, compact_stamp()));
        let _ = std::fs::rename(&self.path, &rotated);
        self.written = 0;
    }

    /// Append one already-formatted line (a trailing newline is added). Errors
    /// are swallowed — logging must never affect program behavior.
    pub fn line(&mut self, line: &str) {
        self.ensure_open();
        let needed = line.len() as u64 + 1;
        // Rotate before writing if this line would push a non-empty file over the
        // cap (a lone oversized line on an empty file is written as-is).
        if self.written > 0 && self.written + needed > MAX_LOG_BYTES {
            self.rotate();
            self.ensure_open();
        }
        if let Some(f) = self.file.as_mut() {
            if writeln!(f, "{line}").is_ok() {
                let _ = f.flush();
                self.written += needed;
            }
        }
    }
}

/// Make a user string safe for a file name (`no-user` when empty/absent).
fn sanitize_user(user: Option<&str>) -> String {
    let cleaned: String = user
        .map(|s| s.trim())
        .unwrap_or("")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' })
        .collect();
    if cleaned.is_empty() {
        "no-user".to_string()
    } else {
        cleaned
    }
}

/// A compact, filesystem-safe UTC timestamp for rotated file names, e.g.
/// `20260610T073000123Z` (ISO-8601 with the separators removed).
pub fn compact_stamp() -> String {
    now_iso().chars().filter(|c| !matches!(c, '-' | ':' | '.')).collect()
}

/// Quote a value for the `key=value` line if it contains spaces.
pub fn field(name: &str, val: &str) -> String {
    if val.contains(' ') {
        format!("{name}=\"{val}\"")
    } else {
        format!("{name}={val}")
    }
}

/// An ISO-8601 UTC timestamp with millisecond precision, e.g.
/// `2026-06-10T07:30:00.123Z`. Computed without external crates.
pub fn now_iso() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    iso_from_unix_millis(d.as_millis() as i64)
}

/// Format a UNIX-epoch millisecond count as ISO-8601 UTC.
pub fn iso_from_unix_millis(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let millis = ms.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (h, mi, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{millis:03}Z")
}

/// Days since 1970-01-01 → (year, month, day). Howard Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_epoch_and_known_dates() {
        assert_eq!(iso_from_unix_millis(0), "1970-01-01T00:00:00.000Z");
        // 2026-06-10T07:30:00.123Z
        let ms = 1_781_076_600_123;
        assert_eq!(iso_from_unix_millis(ms), "2026-06-10T07:30:00.123Z");
    }

    #[test]
    fn level_parse() {
        assert_eq!(LogLevel::parse("true"), LogLevel::Basic);
        assert_eq!(LogLevel::parse("basic"), LogLevel::Basic);
        assert_eq!(LogLevel::parse("full"), LogLevel::Full);
        assert_eq!(LogLevel::parse("off"), LogLevel::Off);
        assert_eq!(LogLevel::parse("false"), LogLevel::Off);
    }

    #[test]
    fn format_parse() {
        assert_eq!(LogFormat::parse("json"), LogFormat::Json);
        assert_eq!(LogFormat::parse("grafana"), LogFormat::Json);
        assert_eq!(LogFormat::parse("text"), LogFormat::Text);
        assert_eq!(LogFormat::parse(""), LogFormat::Text);
    }

    fn unique_dir(tag: &str) -> PathBuf {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let d = std::env::temp_dir().join(format!("prc-logrot-{tag}-{n}"));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn rotates_over_100k_with_user_in_name() {
        let dir = unique_dir("user");
        let idx = dir.join("customers.idx");
        let mut w = LogWriter::new(&idx, LogLevel::Basic, Some("alice"));

        let line = "x".repeat(200); // ~201 bytes/line
        for _ in 0..700 {
            w.line(&line); // ~140 KiB total → at least one rotation
        }

        // Active log stays under the cap.
        let active = dir.join("customers.idx.log");
        let active_len = std::fs::metadata(&active).unwrap().len();
        assert!(active_len <= MAX_LOG_BYTES, "active log too big: {active_len}");

        // A rotated file exists, named `<user>.<datafile>.log.<timestamp>`.
        let rotated: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("alice.customers.idx.log.") && n != "customers.idx.log")
            .collect();
        assert!(!rotated.is_empty(), "no rotated file found in {dir:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rotation_uses_no_user_when_absent() {
        let dir = unique_dir("nouser");
        let idx = dir.join("orders.dat");
        let mut w = LogWriter::new(&idx, LogLevel::Basic, None);
        let line = "y".repeat(200);
        for _ in 0..700 {
            w.line(&line);
        }
        let has_no_user = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with("no-user.orders.dat.log."));
        assert!(has_no_user, "expected a no-user rotated file");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_renders_both_formats() {
        let mut r = LogRecord::new();
        r.str("ts", "2026-06-10T07:30:00.123Z")
            .str("file", "my file.idx")
            .num("tx", 2)
            .str("kind", "COMMIT")
            .num("records", 100)
            .str("order", "ordered");

        assert_eq!(
            r.render(LogFormat::Text),
            "ts=2026-06-10T07:30:00.123Z file=\"my file.idx\" tx=2 kind=COMMIT records=100 order=ordered"
        );
        assert_eq!(
            r.render(LogFormat::Json),
            r#"{"ts":"2026-06-10T07:30:00.123Z","file":"my file.idx","tx":2,"kind":"COMMIT","records":100,"order":"ordered"}"#
        );
    }
}
