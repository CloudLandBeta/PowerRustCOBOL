// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

use std::fmt;

/// A construct the generator cannot compile: it names the construct, the line
/// and — where there is one — the alternative (WASM-010 R8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenError {
    pub line: u32,
    pub message: String,
    /// Further diagnostics found in the same program: the generator reports
    /// every construct it cannot compile, not only the first.
    pub more: Vec<GenError>,
}

impl GenError {
    pub fn new(line: u32, message: impl Into<String>) -> GenError {
        GenError { line, message: message.into(), more: Vec::new() }
    }

    /// Every diagnostic, the first one included.
    pub fn all(&self) -> Vec<(u32, &str)> {
        let mut v = vec![(self.line, self.message.as_str())];
        v.extend(self.more.iter().flat_map(|m| m.all()));
        v
    }
}

impl fmt::Display for GenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(f, "line {}: {}", self.line, self.message)?;
        } else {
            f.write_str(&self.message)?;
        }
        if !self.more.is_empty() {
            write!(f, " (and {} more)", self.more.len())?;
        }
        Ok(())
    }
}

impl std::error::Error for GenError {}

pub type GenResult<T> = Result<T, GenError>;
