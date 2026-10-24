// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

use std::fmt;

/// What kind of refusal a [`GenError`] is, so that the IDE can say it in the
/// developer's language (WASM-010 D6). The English `message` is kept for the
/// tools, the way `NOT_YET_LINKED_MESSAGE` is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ErrorKind {
    /// Anything else: an internal limit or a construct the generator does not handle yet.
    #[default]
    Other,
    /// The construct runs on the backend (files, SQL, HTTP, keys …).
    BackendOnly,
    /// The construct has no meaning in any WebAssembly build.
    Unsupported,
    /// `LAYER-NAME::Property` names no layer of the form.
    UnknownLayer,
    /// The layer has no such property.
    UnknownLayerProperty,
    /// A write to a control's `Layer`, which is read-only.
    LayerWrite,
    /// The statement reads more host values than the character temporaries hold.
    TooManyHostReads,
}

/// A construct the generator cannot compile: it names the construct, the line
/// and — where there is one — the alternative (WASM-010 R8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenError {
    pub line: u32,
    pub message: String,
    pub kind: ErrorKind,
    /// The construct, layer or property the refusal is about, as the developer
    /// wrote it (the argument a translated message needs).
    pub subject: String,
    /// Further diagnostics found in the same program: the generator reports
    /// every construct it cannot compile, not only the first.
    pub more: Vec<GenError>,
    /// The form (index in the project's forms) whose program the construct
    /// is in, when the module is a project's.
    pub form: Option<usize>,
}

impl GenError {
    pub fn new(line: u32, message: impl Into<String>) -> GenError {
        GenError { line, message: message.into(), kind: ErrorKind::Other, subject: String::new(), more: Vec::new(), form: None }
    }

    /// A refusal of a known kind about `subject`.
    pub fn kinded(line: u32, kind: ErrorKind, subject: impl Into<String>, message: impl Into<String>) -> GenError {
        GenError { line, message: message.into(), kind, subject: subject.into(), more: Vec::new(), form: None }
    }

    /// Every diagnostic, the first one included, with its kind.
    /// Say which form every diagnostic belongs to.
    pub fn in_form(mut self, form: usize) -> GenError {
        self.form = Some(form);
        self.more = self.more.into_iter().map(|m| m.in_form(form)).collect();
        self
    }

    pub fn all_kinded(&self) -> Vec<&GenError> {
        let mut v = vec![self];
        v.extend(self.more.iter().flat_map(|m| m.all_kinded()));
        v
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

/// What a warning is about (WASM-010 D6; the IDE translates it by kind).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningKind {
    /// `ACCEPT` from the operator does nothing in a browser module (Q9).
    AcceptIgnored,
    /// A `REDEFINES` whose result may differ from the desktop's (Q5, working assumption).
    RedefinesMayDiffer,
}

/// Something that compiles but that the developer should know about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenWarning {
    pub line: u32,
    pub kind: WarningKind,
    pub subject: String,
    pub message: String,
}

impl fmt::Display for GenWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: warning: {}", self.line, self.message)
    }
}

pub type GenResult<T> = Result<T, GenError>;
