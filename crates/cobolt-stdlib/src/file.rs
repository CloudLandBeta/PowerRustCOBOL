// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! File I/O backend — Phase 5 placeholder.
//!
//! This module will implement sequential and indexed (SQLite-backed) file I/O
//! when Phase 5 work begins.  For now it provides the type stubs needed for
//! the rest of the codebase to compile.

/// File open modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileMode {
    Input,
    Output,
    InputOutput,
    Extend,
}

/// A file organisation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOrg {
    Sequential,
    Relative,
    Indexed,
}

/// Status code returned after every file I/O operation.
///
/// Values follow the standard two-character FILE STATUS convention:
/// `"00"` = success, `"10"` = EOF, `"22"` = duplicate key, etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStatus(pub String);

impl FileStatus {
    pub fn ok()  -> Self { FileStatus("00".into()) }
    pub fn eof() -> Self { FileStatus("10".into()) }
    pub fn not_found() -> Self { FileStatus("35".into()) }

    pub fn is_ok(&self) -> bool { self.0 == "00" }
    pub fn is_eof(&self) -> bool { self.0 == "10" }
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Placeholder file handle — will be replaced with a real implementation.
#[derive(Debug)]
pub struct FileHandle {
    pub logical_name: String,
    pub mode: FileMode,
    pub org: FileOrg,
}

impl FileHandle {
    pub fn new(logical_name: impl Into<String>, mode: FileMode, org: FileOrg) -> Self {
        Self {
            logical_name: logical_name.into(),
            mode,
            org,
        }
    }
}
