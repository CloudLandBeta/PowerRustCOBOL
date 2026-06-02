// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Non-blocking native file dialogs.
//!
//! A **synchronous** `rfd::FileDialog::pick_file()` runs a nested macOS modal
//! event loop. When called from inside the egui frame (which already runs inside
//! winit's event handler) that re-enters the event loop, and winit 0.30 aborts
//! the process with *"tried to handle event while another event is currently
//! being handled"*.
//!
//! Instead we drive `rfd::AsyncFileDialog` on a worker thread (rfd shows the
//! panel on the main run loop between frames, so nothing is nested) and deliver
//! the result through a keyed inbox that the UI polls on later frames.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Mutex, OnceLock};

type Pending = HashMap<String, Receiver<Option<PathBuf>>>;

fn pending() -> &'static Mutex<Pending> {
    static P: OnceLock<Mutex<Pending>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Describes a file dialog to open.
#[derive(Clone, Default)]
pub struct DialogSpec {
    save: bool,
    filters: Vec<(String, Vec<String>)>,
    directory: Option<PathBuf>,
    file_name: Option<String>,
}

impl DialogSpec {
    /// An "open / pick file" dialog.
    pub fn open() -> Self {
        Self::default()
    }
    /// A "save file" dialog.
    pub fn save() -> Self {
        Self { save: true, ..Self::default() }
    }
    /// Add a named extension filter (e.g. `("COBOL", &["cbl","cob"])`).
    pub fn filter(mut self, name: &str, exts: &[&str]) -> Self {
        self.filters.push((name.to_owned(), exts.iter().map(|s| s.to_string()).collect()));
        self
    }
    /// Start the dialog in this directory.
    pub fn directory(mut self, dir: impl Into<PathBuf>) -> Self {
        self.directory = Some(dir.into());
        self
    }
    /// Suggested file name (save dialogs).
    pub fn file_name(mut self, name: impl Into<String>) -> Self {
        self.file_name = Some(name.into());
        self
    }
}

/// Whether a dialog started under `key` is still open (awaiting the user).
pub fn is_open(key: &str) -> bool {
    pending().lock().unwrap().contains_key(key)
}

/// Begin a native dialog under `key`. Safe to call from inside the egui frame —
/// it never nests the OS event loop. Poll [`take`] on later frames for the
/// result. A no-op if a dialog under `key` is already open.
pub fn begin(key: &str, spec: DialogSpec) {
    if is_open(key) {
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut dlg = rfd::AsyncFileDialog::new();
        for (name, exts) in &spec.filters {
            let refs: Vec<&str> = exts.iter().map(|s| s.as_str()).collect();
            dlg = dlg.add_filter(name, &refs);
        }
        if let Some(dir) = &spec.directory {
            dlg = dlg.set_directory(dir);
        }
        if let Some(name) = &spec.file_name {
            dlg = dlg.set_file_name(name);
        }
        let handle = if spec.save {
            pollster::block_on(dlg.save_file())
        } else {
            pollster::block_on(dlg.pick_file())
        };
        let _ = tx.send(handle.map(|h| h.path().to_path_buf()));
    });
    pending().lock().unwrap().insert(key.to_owned(), rx);
}

/// Convenience: begin an "open file" dialog with a single filter.
pub fn open_file(key: &str, filter: &str, exts: &[&str]) {
    begin(key, DialogSpec::open().filter(filter, exts));
}

/// Poll a dialog started under `key`:
/// * `Some(Some(path))` — the user picked a file,
/// * `Some(None)` — the user cancelled,
/// * `None` — still open (or never started).
pub fn take(key: &str) -> Option<Option<PathBuf>> {
    let mut map = pending().lock().unwrap();
    let result = match map.get(key) {
        Some(rx) => match rx.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Some(None),
        },
        None => return None,
    };
    map.remove(key);
    result
}
