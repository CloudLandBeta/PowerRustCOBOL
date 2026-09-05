// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Where a project-relative file actually lives, this run.
//!
//! A form stores an image as `assets/logo.png` — relative to the project, the
//! same shape `.cidx` already stores an assign path in. Nothing resolved it:
//! `paint.rs` handed the stored string straight to `std::fs::read`, so a
//! relative path was resolved by the OS against the process's CURRENT WORKING
//! DIRECTORY, which is wherever the application happened to be launched from.
//!
//! The consequence was that only absolute paths worked, so the designer wrote
//! absolute paths — and a form carried `/Users/<someone>/Documents/<project>/…`
//! into a repository, where it was broken for every other machine and for the
//! author too as soon as the project moved (operator, 2026-09-04).
//!
//! The anchor is set once at start-up: the project directory in the IDE, and
//! the executable's own directory in a built application, so `bin/` and
//! `dist/` both work with no launch-directory ceremony.

use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

fn base() -> &'static RwLock<Option<PathBuf>> {
    static BASE: OnceLock<RwLock<Option<PathBuf>>> = OnceLock::new();
    BASE.get_or_init(|| RwLock::new(None))
}

/// Anchor project-relative paths at `dir` for the rest of the run.
///
/// Called with the project directory by the IDE and the form host, and with
/// the executable's directory by a built application. Setting it again
/// replaces it — the IDE opens one project after another in one process.
pub fn set_base(dir: impl Into<PathBuf>) {
    if let Ok(mut b) = base().write() {
        *b = Some(dir.into());
    }
}

/// The anchor, when one has been set.
pub fn current_base() -> Option<PathBuf> {
    base().read().ok().and_then(|b| b.clone())
}

/// Where `stored` actually is.
///
/// An absolute path is returned unchanged — a developer who points outside the
/// project meant it. A relative one is joined to the anchor; if that does not
/// exist, the path is returned as-is so the OS resolves it against the working
/// directory exactly as before, which keeps every pre-existing setup working
/// and lets the caller's own "file not found" reporting stay in charge.
pub fn resolve(stored: &str) -> PathBuf {
    let trimmed = stored.trim();
    let p = Path::new(trimmed);
    if trimmed.is_empty() || p.is_absolute() {
        return p.to_path_buf();
    }
    match current_base() {
        Some(dir) => {
            let joined = dir.join(p);
            if joined.exists() {
                joined
            } else {
                // Not under the anchor: fall back to the old behaviour rather
                // than inventing a path that never existed.
                p.to_path_buf()
            }
        }
        None => p.to_path_buf(),
    }
}

/// How a path should be STORED in a form: project-relative when it is inside
/// the project, absolute when it is not.
///
/// Mirrors `cobolt_indexed::paths::store_path`, which has stored `.cidx`
/// assign paths this way since indexed files were introduced; forms simply
/// never adopted it.
pub fn store(project_dir: &Path, abs: &Path) -> String {
    use std::path::Component;
    match abs.strip_prefix(project_dir) {
        Ok(rel) => rel
            .components()
            .filter_map(|c| match c {
                Component::Normal(s) => s.to_str(),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/"),
        Err(_) => abs.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absolute_path_is_left_alone() {
        set_base("/proj");
        assert_eq!(resolve("/elsewhere/logo.png"), PathBuf::from("/elsewhere/logo.png"));
    }

    #[test]
    fn a_relative_path_that_is_not_under_the_anchor_keeps_the_old_behaviour() {
        set_base("/nowhere-at-all");
        assert_eq!(resolve("assets/logo.png"), PathBuf::from("assets/logo.png"));
    }

    #[test]
    fn an_empty_path_stays_empty() {
        set_base("/proj");
        assert_eq!(resolve("   "), PathBuf::from(""));
    }

    /// Storing mirrors the `.cidx` rule, including the forward slashes that
    /// make a form portable between Windows and Unix.
    #[test]
    fn a_path_inside_the_project_is_stored_relative() {
        let root = Path::new("/proj");
        assert_eq!(
            store(root, Path::new("/proj/assets/logo.png")),
            "assets/logo.png"
        );
        assert_eq!(
            store(root, Path::new("/proj/assets/icons/save.png")),
            "assets/icons/save.png"
        );
    }

    #[test]
    fn a_path_outside_the_project_is_stored_absolute() {
        let root = Path::new("/proj");
        assert_eq!(
            store(root, Path::new("/elsewhere/logo.png")),
            "/elsewhere/logo.png"
        );
    }

    /// The anchor resolves a real file: written here rather than assumed,
    /// because "exists" is the whole condition the fallback turns on.
    #[test]
    fn a_relative_path_under_the_anchor_resolves_to_it() {
        let dir = std::env::temp_dir().join("prc-assets-test");
        let sub = dir.join("assets");
        std::fs::create_dir_all(&sub).unwrap();
        let file = sub.join("logo.png");
        std::fs::write(&file, b"x").unwrap();

        set_base(&dir);
        assert_eq!(resolve("assets/logo.png"), file);

        std::fs::remove_dir_all(&dir).ok();
    }
}
