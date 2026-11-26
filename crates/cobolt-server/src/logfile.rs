// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A log file that cannot grow without end (spec 113 A3, plan Q5): where the Backend writes when it
//! runs as a service on a system that keeps no journal for it (macOS, Windows). Each record is
//! written whole; before one that would pass the limit the file is renamed `.1`, `.1` to `.2`, and
//! so on up to `keep`, the oldest being replaced.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::log::{LogRecord, LogSink};

/// 10 MiB: where the Backend's log file rotates unless told otherwise.
pub const DEFAULT_MAX_BYTES: u64 = 10 * 1024 * 1024;
/// Rotated files kept beside the current one.
pub const DEFAULT_KEEP: u32 = 5;

struct Inner {
    /// `None` only while the file is renamed.
    file: Option<File>,
    size: u64,
    /// The size at which rotation is tried again after it failed (a viewer holding the file on Windows).
    retry_at: u64,
}

/// A size-rotating log file, safe to share between threads.
pub struct RotatingLog {
    path: PathBuf,
    max: u64,
    keep: u32,
    inner: Mutex<Inner>,
}

fn numbered(path: &Path, n: u32) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(format!(".{n}"));
    PathBuf::from(s)
}

impl RotatingLog {
    /// Open (creating the folder and the file) `path`, to rotate at `max_bytes` keeping `keep` old files.
    pub fn open(path: impl Into<PathBuf>, max_bytes: u64, keep: u32) -> io::Result<RotatingLog> {
        let path = path.into();
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let size = file.metadata()?.len();
        Ok(RotatingLog { path, max: max_bytes.max(1), keep: keep.max(1), inner: Mutex::new(Inner { file: Some(file), size, retry_at: 0 }) })
    }

    /// Open with the defaults: 10 MiB, five old files.
    pub fn open_default(path: impl Into<PathBuf>) -> io::Result<RotatingLog> {
        Self::open(path, DEFAULT_MAX_BYTES, DEFAULT_KEEP)
    }

    /// Where the current file is.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn rotate(&self, inner: &mut Inner) -> io::Result<()> {
        // The oldest is replaced, the others move up one, the current one becomes `.1`.
        for n in (1..self.keep).rev() {
            let from = numbered(&self.path, n);
            if from.exists() {
                std::fs::rename(&from, numbered(&self.path, n + 1))?;
            }
        }
        // The open handle is released first: Windows will not rename a file that is open.
        inner.file = None;
        let renamed = std::fs::rename(&self.path, numbered(&self.path, 1));
        // Whether or not the rename worked, the log goes on in the file at `path`.
        let file = OpenOptions::new().create(true).append(true).open(&self.path)?;
        inner.size = if renamed.is_ok() { 0 } else { file.metadata().map(|m| m.len()).unwrap_or(inner.size) };
        inner.file = Some(file);
        renamed
    }

    /// Append one whole record (it should end with a newline). A record is never split: rotation
    /// happens before it, and one larger than the limit is written alone.
    pub fn write_record(&self, bytes: &[u8]) -> io::Result<()> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.size > 0 && inner.size + bytes.len() as u64 > self.max && inner.size >= inner.retry_at && self.rotate(&mut inner).is_err() {
            // Writing matters more than rotating: keep appending, and try again after another limit's worth.
            inner.retry_at = inner.size + self.max;
        }
        match inner.file.as_mut() {
            Some(f) => f.write_all(bytes)?,
            None => return Err(io::Error::other("the log file is closed")),
        }
        inner.size += bytes.len() as u64;
        Ok(())
    }

    /// Append a line of text.
    pub fn line(&self, text: &str) {
        let mut b = Vec::with_capacity(text.len() + 1);
        b.extend_from_slice(text.as_bytes());
        if !text.ends_with('\n') {
            b.push(b'\n');
        }
        let _ = self.write_record(&b);
    }
}

/// Now, in UTC, as `2026-11-25T22:14:03Z` (no time-zone database: a log line needs a clock, not a calendar library).
pub fn stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    stamp_of(secs)
}

fn stamp_of(secs: u64) -> String {
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

impl LogSink for RotatingLog {
    fn emit(&self, record: &LogRecord) {
        if let Ok(line) = serde_json::to_string(record) {
            self.line(&line);
        }
    }
}

/// What the `tracing` subscriber writes through: each event is one record.
pub struct LogWriter(pub Arc<RotatingLog>);

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write_record(buf)?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogWriter {
    type Writer = LogWriter;
    fn make_writer(&'a self) -> LogWriter {
        LogWriter(self.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("prc-logfile-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn lines_of(path: &Path) -> Vec<String> {
        std::fs::read_to_string(path).map(|t| t.lines().map(str::to_owned).collect()).unwrap_or_default()
    }

    #[test]
    fn the_utc_stamp_is_right_for_known_instants() {
        assert_eq!(stamp_of(0), "1970-01-01T00:00:00Z");
        assert_eq!(stamp_of(951_782_400), "2000-02-29T00:00:00Z"); // a leap day
        assert_eq!(stamp_of(1_764_108_843), "2025-11-25T22:14:03Z");
        assert_eq!(stamp_of(4_102_444_799), "2099-12-31T23:59:59Z");
        assert_eq!(stamp().len(), 20);
    }

    /// Rotation, pruning, whole records and many writers, with one result block.
    #[test]
    fn the_rotating_log_reports_what_it_checked() {
        let started = Instant::now();
        let mut cases = 0;
        let mut ok = |what: &str, cond: bool| {
            assert!(cond, "{what}");
            cases += 1;
        };

        // Rotation at the limit; nothing pruned while `keep` is large.
        let d = dir("rotate");
        let log = RotatingLog::open(d.join("backend.log"), 1000, 50).unwrap();
        let total = 100;
        for i in 0..total {
            log.line(&format!("record {i:04} {}", "x".repeat(90)));
        }
        let mut files: Vec<PathBuf> = (1..=50).map(|n| numbered(log.path(), n)).filter(|p| p.exists()).collect();
        files.push(log.path().to_owned());
        let size_ok = files.iter().all(|f| std::fs::metadata(f).unwrap().len() <= 1000);
        let lines: usize = files.iter().map(|f| lines_of(f).len()).sum();
        ok("every file is within the limit", size_ok);
        ok("more than one file was made", files.len() > 1);
        ok("no record was lost or split", lines == total);
        ok("the current file holds the newest record", lines_of(log.path()).last().map(|l| l.starts_with("record 0099")) == Some(true));
        ok("`.1` holds older records than the current file", lines_of(&numbered(log.path(), 1)).last().map(|l| l.as_str()) < lines_of(log.path()).first().map(|l| l.as_str()));

        // Pruning: only `keep` old files remain, and the newest records survive.
        let d2 = dir("prune");
        let log2 = RotatingLog::open(d2.join("b.log"), 500, 3).unwrap();
        for i in 0..200 {
            log2.line(&format!("record {i:04} {}", "y".repeat(90)));
        }
        let olds = (1..=10).filter(|n| numbered(log2.path(), *n).exists()).count();
        ok("only `keep` old files remain", olds == 3);
        ok("the newest record is in the current file", lines_of(log2.path()).last().map(|l| l.starts_with("record 0199")) == Some(true));
        ok("the oldest records are gone", !(0..=10).any(|n| (if n == 0 { lines_of(log2.path()) } else { lines_of(&numbered(log2.path(), n)) }).iter().any(|l| l.starts_with("record 0000"))));

        // A record larger than the limit is written whole, alone.
        let d3 = dir("big");
        let log3 = RotatingLog::open(d3.join("b.log"), 100, 2).unwrap();
        log3.line("small");
        log3.line(&"z".repeat(400));
        let all: Vec<String> = lines_of(&numbered(log3.path(), 1)).into_iter().chain(lines_of(log3.path())).collect();
        ok("a record over the limit is whole", all.iter().any(|l| l.len() == 400));

        // Eight threads, no record lost, and a reopen continues the size.
        let d4 = dir("threads");
        let log4 = Arc::new(RotatingLog::open(d4.join("b.log"), 4096, 200).unwrap());
        let t = Instant::now();
        let per = 500;
        let handles: Vec<_> = (0..8)
            .map(|th| {
                let l = log4.clone();
                std::thread::spawn(move || {
                    for i in 0..per {
                        l.line(&format!("thread {th} record {i:05} {}", "w".repeat(40)));
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let ms = t.elapsed().as_millis().max(1);
        let mut all4: Vec<String> = lines_of(log4.path());
        for n in 1..=200 {
            all4.extend(lines_of(&numbered(log4.path(), n)));
        }
        ok("eight writers lose no record", all4.len() == 8 * per);
        ok("every record is whole", all4.iter().all(|l| l.starts_with("thread ") && l.ends_with(&"w".repeat(40))));
        let rate = (8 * per) as u128 * 1000 / ms;
        drop(log4);
        let reopened = RotatingLog::open(d4.join("b.log"), 4096, 200).unwrap();
        ok("a reopened log continues from the file's size", reopened.inner.lock().unwrap().size == std::fs::metadata(d4.join("b.log")).unwrap().len());

        for p in [d, d2, d3, d4] {
            let _ = std::fs::remove_dir_all(p);
        }
        println!("cobolt-server logfile: {cases} checks, 0 failed; 8 threads x {per} records in {ms} ms ({rate} records/sec); total {} ms", started.elapsed().as_millis());
    }
}
