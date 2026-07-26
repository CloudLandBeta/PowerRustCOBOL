// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Where the diagnostic log files go — one answer for every platform.
//!
//! PowerRustCOBOL runs on Linux, macOS and Windows, so no shipping code may
//! hardcode a POSIX path. Several diagnostics used to open `/tmp/…` literally;
//! on Windows that resolves to `\tmp\…` on the current drive, a directory that
//! normally does not exist, so every one of those writes silently failed and the
//! diagnostics appeared to do nothing.
//!
//! Both crates that write diagnostics (`cobolt-cli`'s run-form app and the IDE)
//! depend on this crate, so this is the lowest shared home for the rule.

use std::path::PathBuf;

/// The directory diagnostic logs are written to.
///
/// On Unix this is literally `/tmp`, deliberately *not*
/// [`std::env::temp_dir`]: on macOS that returns a private per-process
/// `/var/folders/…` path, which would hide the files from the developer who was
/// told to look in `/tmp` (the documented, user-facing location). On Windows
/// there is no `/tmp`, so the platform temp directory is the only correct
/// answer, and `%TEMP%` is where a Windows developer looks anyway.
pub fn diagnostics_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::temp_dir()
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/tmp")
    }
}

/// Full path of the diagnostic file named `name` inside [`diagnostics_dir`].
///
/// `name` must be a bare file name; a path separator in it would escape the
/// diagnostics directory, so any leading directory part is dropped.
pub fn diagnostics_file(name: &str) -> PathBuf {
    let leaf = name
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("cobolt-diagnostics.log");
    diagnostics_dir().join(leaf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_directory_is_absolute_and_exists_on_this_platform() {
        let dir = diagnostics_dir();
        assert!(dir.is_absolute(), "{dir:?} must be absolute");
        assert!(
            dir.is_dir(),
            "{dir:?} must already exist — diagnostics are best-effort and never mkdir"
        );
    }

    #[test]
    fn a_file_lands_directly_in_the_directory() {
        let p = diagnostics_file("databinding.log");
        assert_eq!(p.parent(), Some(diagnostics_dir().as_path()));
        assert_eq!(p.file_name().unwrap(), "databinding.log");
    }

    /// A name carrying a separator must not be able to write outside the
    /// diagnostics directory (`../../etc/passwd`, `C:\Windows\x`).
    #[test]
    fn separators_in_the_name_cannot_escape() {
        for name in ["../../etc/passwd", "sub/dir/file.log", r"C:\Windows\evil"] {
            let p = diagnostics_file(name);
            assert_eq!(
                p.parent(),
                Some(diagnostics_dir().as_path()),
                "{name} escaped to {p:?}"
            );
        }
    }

    #[test]
    fn an_empty_or_separator_only_name_still_yields_a_file() {
        for name in ["", "/", "///"] {
            let p = diagnostics_file(name);
            assert_eq!(p.file_name().unwrap(), "cobolt-diagnostics.log");
        }
    }

    /// No shipping path may be a POSIX literal — this is the rule the module
    /// exists to enforce, checked on whatever platform CI runs.
    #[cfg(windows)]
    #[test]
    fn windows_never_uses_the_posix_tmp() {
        let dir = diagnostics_dir();
        assert_ne!(dir, PathBuf::from("/tmp"));
        assert_ne!(dir, PathBuf::from(r"\tmp"));
    }
}
