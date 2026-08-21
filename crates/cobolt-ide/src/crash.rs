// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Crash diagnostics and work recovery (operator, 2026-08-20).
//!
//! The IDE vanished mid-session while a form was open, leaving nothing behind:
//! no crash report from the OS, no message on screen, and no way to reproduce
//! it. Two separate problems hide inside that sentence, and they need two
//! separate mechanisms.
//!
//! # 1. Nothing was left to diagnose
//!
//! A windowed application has no terminal attached, so the panic message, its
//! `file:line`, and the backtrace were all printed to a stderr nobody was
//! reading. [`install`] fixes that with a panic hook that writes the same
//! information to `<data>/cobolt/crash/` instead, where it survives the process.
//!
//! # 2. The developer's work went with it
//!
//! A panic hook cannot save the day on its own, because **the failures most
//! likely to lose work do not run one**. A stack overflow faults on the guard
//! page and is delivered as `SIGSEGV`; the OOM killer sends `SIGKILL`; a second
//! panic while unwinding aborts. In all three the hook is never called. Anything
//! that only lives in a panic hook is therefore a partial answer.
//!
//! So the work is saved by a **timer**, not by the hook: [`autosave`] writes
//! every dirty buffer to `<data>/cobolt/recovery/` every
//! [`AUTOSAVE_SECS`] seconds. That covers panics, `SIGSEGV`, `SIGKILL`, and the
//! power going out, because it has already happened by the time anything goes
//! wrong. The cost is bounded: at most [`AUTOSAVE_SECS`] of edits.
//!
//! # Recovered work is never written over the original
//!
//! After a panic the process state cannot be trusted — that is what a panic
//! means. Recovery copies go to their own directory and the developer is asked
//! on the next launch whether to take them. Silently overwriting a good file on
//! disk with a buffer from a process that had just lost its footing would turn
//! a crash into data loss.

use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

/// How often dirty buffers are copied to the recovery directory.
///
/// The upper bound on how much work a hard kill can cost. Twenty seconds keeps
/// the write cheap enough to go unnoticed while typing.
pub const AUTOSAVE_SECS: f64 = 20.0;

/// Crash logs — one file per panic.
pub fn crash_dir() -> PathBuf {
    crate::llm::base_dir().join("crash")
}

/// Autosaved copies of unsaved work.
pub fn recovery_dir() -> PathBuf {
    crate::llm::base_dir().join("recovery")
}

/// Present while the IDE is running; removed on a clean exit. Finding one at
/// startup is what "the last session did not end properly" means.
fn marker_path() -> PathBuf {
    crate::llm::base_dir().join("session.running")
}

/// What was open when things went wrong, for the crash log.
///
/// Kept separate from the buffer contents: this is refreshed cheaply every
/// frame, whereas copying buffer text is done on the autosave timer.
static OPEN_PATHS: LazyLock<Mutex<Vec<PathBuf>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// One unsaved buffer worth rescuing — an editor tab or a form designer.
pub struct Recoverable {
    /// Where it belongs on disk.
    pub origin: PathBuf,
    /// Its current contents: source text, or serialized `.cfrm` XML.
    pub body: String,
}

/// A rescued buffer found at startup.
pub struct Recovered {
    /// Where it belongs on disk.
    pub origin: PathBuf,
    /// The autosaved copy.
    pub saved: PathBuf,
}

/// Seconds since the Unix epoch — a timestamp with no dependency behind it.
fn stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Take a lock without caring that a previous holder panicked.
///
/// The hook runs *because* something panicked, quite possibly while holding
/// this lock. Refusing to read a poisoned mutex there would throw away the
/// diagnostics for the exact failure worth diagnosing.
fn lock_through_poison<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Install the panic hook and mark the session as running.
///
/// Call once, before the event loop. The previous hook still runs afterwards,
/// so the usual message keeps going to stderr for anyone who *does* have a
/// terminal attached.
pub fn install() {
    let _ = std::fs::create_dir_all(crash_dir());
    let _ = std::fs::write(marker_path(), stamp().to_string());

    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Nothing in here may panic: a panic inside the hook aborts the process
        // immediately and destroys the report we are trying to write.
        let mut report = String::new();
        report.push_str(&format!("PowerRustCOBOL {}\n", crate::version::VERSION));
        report.push_str(&format!("when:    {} (unix seconds)\n", stamp()));
        report.push_str(&format!("os:      {}\n", std::env::consts::OS));
        report.push_str(&format!(
            "thread:  {}\n",
            std::thread::current().name().unwrap_or("<unnamed>")
        ));

        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic payload>".to_string());
        report.push_str(&format!("panic:   {message}\n"));

        match info.location() {
            Some(l) => report.push_str(&format!("at:      {}:{}:{}\n", l.file(), l.line(), l.column())),
            None => report.push_str("at:      <unknown location>\n"),
        }

        {
            let open = lock_through_poison(&OPEN_PATHS);
            report.push_str(&format!("open:    {} file(s)\n", open.len()));
            for p in open.iter() {
                report.push_str(&format!("         {}\n", p.display()));
            }
        }

        // Forced, so a report is useful without the developer having known to
        // set RUST_BACKTRACE before the crash they could not predict.
        report.push_str("\nbacktrace:\n");
        report.push_str(&format!("{}\n", std::backtrace::Backtrace::force_capture()));

        let _ = std::fs::create_dir_all(crash_dir());
        let _ = std::fs::write(crash_dir().join(format!("crash-{}.log", stamp())), &report);

        previous(info);
    }));
}

/// Record the exit as intentional, so the next launch does not offer recovery.
pub fn mark_clean_exit() {
    let _ = std::fs::remove_file(marker_path());
    let _ = std::fs::remove_dir_all(recovery_dir());
}

/// True when the previous session did not shut down cleanly.
pub fn ended_badly() -> bool {
    marker_path().exists()
}

/// Turn a path into one flat, safe file name, keeping the original readable.
///
/// `/Users/me/proj/forms/Main.cfrm` → `Users_me_proj_forms_Main.cfrm`. Flat
/// because the recovery directory should be openable without digging, and
/// prefixed with the full path because two projects can both hold `Main.cfrm`.
fn flatten(origin: &Path) -> String {
    let s = origin.to_string_lossy();
    let flat: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
        .collect();
    flat.trim_matches('_').to_string()
}

/// Copy every unsaved buffer into the recovery directory.
///
/// Best-effort throughout: autosave must never interrupt the developer, so an
/// unwritable directory is silently skipped rather than raised. Losing the
/// safety net is bad; a modal in the middle of typing because the safety net
/// could not be written is worse.
pub fn autosave(items: &[Recoverable]) {
    if items.is_empty() {
        return;
    }
    let dir = recovery_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }

    let mut manifest = String::from("# PowerRustCOBOL recovered work\n");
    for item in items {
        let name = flatten(&item.origin);
        if std::fs::write(dir.join(&name), &item.body).is_err() {
            continue;
        }
        manifest.push_str("\n[[entry]]\norigin = ");
        manifest.push_str(&toml_string(&item.origin.to_string_lossy()));
        manifest.push_str("\nsaved = ");
        manifest.push_str(&toml_string(&name));
        manifest.push('\n');
    }
    let _ = std::fs::write(dir.join("manifest.toml"), manifest);
}

/// Quote a value for the manifest without pulling in a serializer for two
/// fields. Windows paths are full of backslashes, so escaping is not optional.
fn toml_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Read back whatever the last session managed to autosave.
///
/// Entries whose recovered file has since gone are dropped rather than
/// reported: offering to restore something that is not there would be a worse
/// lie than saying nothing.
pub fn recovered() -> Vec<Recovered> {
    let dir = recovery_dir();
    let Ok(text) = std::fs::read_to_string(dir.join("manifest.toml")) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let mut origin: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("origin = ") {
            origin = unquote(v);
        } else if let Some(v) = line.strip_prefix("saved = ") {
            if let (Some(o), Some(s)) = (origin.take(), unquote(v)) {
                let saved = dir.join(s);
                if saved.exists() {
                    out.push(Recovered {
                        origin: PathBuf::from(o),
                        saved,
                    });
                }
            }
        }
    }
    out
}

/// Write each recovered copy beside its original, as `<name>.recovered.<ext>`.
///
/// Deliberately **not** an overwrite, and deliberately not an in-memory restore
/// either. The copy came out of a process that had already lost its footing, so
/// the developer — not the IDE — decides which version wins. Putting it next to
/// the original is what makes that comparison a two-second job.
///
/// Returns the paths written, for the console.
pub fn restore_beside_originals(items: &[Recovered]) -> Vec<PathBuf> {
    let mut written = Vec::new();
    for item in items {
        let Some(name) = item.origin.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let target = match name.rsplit_once('.') {
            Some((stem, ext)) => item.origin.with_file_name(format!("{stem}.recovered.{ext}")),
            None => item.origin.with_file_name(format!("{name}.recovered")),
        };
        if std::fs::copy(&item.saved, &target).is_ok() {
            written.push(target);
        }
    }
    written
}

/// Throw the autosaved copies away and clear the unclean-shutdown marker.
pub fn discard() {
    let _ = std::fs::remove_dir_all(recovery_dir());
    let _ = std::fs::remove_file(marker_path());
}

/// Inverse of [`toml_string`].
fn unquote(v: &str) -> Option<String> {
    let v = v.trim();
    let inner = v.strip_prefix('"')?.strip_suffix('"')?;
    Some(inner.replace("\\\"", "\"").replace("\\\\", "\\"))
}

/// Tell the crash log which files were open. Cheap enough to call every frame.
pub fn note_open(paths: Vec<PathBuf>) {
    let mut open = lock_through_poison(&OPEN_PATHS);
    if *open != paths {
        *open = paths;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_keeps_the_path_readable_and_the_name_safe() {
        let flat = flatten(Path::new("/Users/me/proj/forms/Main.cfrm"));
        assert!(flat.ends_with("Main.cfrm"), "got {flat}");
        assert!(flat.contains("proj"));
        assert!(!flat.contains('/'));
    }

    /// Two projects may each hold `Main.cfrm`; recovery must not collapse them
    /// onto one file and silently lose the other.
    #[test]
    fn different_projects_do_not_collide() {
        let a = flatten(Path::new("/a/forms/Main.cfrm"));
        let b = flatten(Path::new("/b/forms/Main.cfrm"));
        assert_ne!(a, b);
    }

    #[test]
    fn manifest_values_survive_a_round_trip() {
        for raw in [
            r"C:\Users\me\My Forms\Main.cfrm",
            "/home/me/a \"quoted\" name.cbl",
            "/plain/path.cbl",
        ] {
            let quoted = toml_string(raw);
            assert_eq!(unquote(&quoted).as_deref(), Some(raw), "round trip of {raw}");
        }
    }

    /// A poisoned lock must still yield its contents: the hook runs precisely
    /// when something else has just panicked, possibly holding this lock.
    #[test]
    fn a_poisoned_lock_is_still_readable() {
        let m = Mutex::new(vec![PathBuf::from("/x")]);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = m.lock().unwrap();
            panic!("poison it");
        }));
        assert!(m.is_poisoned());
        assert_eq!(lock_through_poison(&m).len(), 1);
    }
}
