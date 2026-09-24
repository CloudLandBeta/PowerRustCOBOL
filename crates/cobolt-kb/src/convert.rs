// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! "Document in, text out" — the seam every format enters through (spec 068
//! R14). The formats themselves live in `cobolt-docs` (spec 074): Word,
//! PowerPoint, Excel, OpenDocument, tables, PDF, HTML, Markdown, plain text
//! and archives of them. A program can register a [`Converter`] of its own,
//! tried before those, with nothing else in the Knowledge Base changing
//! (074 R21).

use std::path::Path;

pub use cobolt_docs::{code, Converted, Limits, Part, Skip, PATH_SEPARATOR};

/// Turns one document's bytes into Markdown, one [`Part`] per document (an
/// archive holds several).
pub trait Converter: Send + Sync {
    /// Whether this converter reads the document at `path`.
    fn handles(&self, path: &Path, bytes: &[u8]) -> bool;
    fn convert(&self, path: &Path, bytes: &[u8]) -> Result<Converted, Skip>;
}

/// The converters a Knowledge Base tries: those registered, then every
/// format `cobolt-docs` reads, within the archive [`Limits`].
#[derive(Default)]
pub struct Converters {
    list: Vec<Box<dyn Converter>>,
    limits: Limits,
}

impl Converters {
    /// The built-in formats, with these archive bounds.
    pub fn with_limits(limits: Limits) -> Self {
        Converters {
            list: Vec::new(),
            limits,
        }
    }

    /// Add a converter, tried before the ones already present.
    pub fn register(&mut self, converter: Box<dyn Converter>) {
        self.list.insert(0, converter);
    }

    /// The document's parts, or why it cannot be read.
    pub fn convert(&self, path: &Path, bytes: &[u8]) -> Result<Converted, Skip> {
        match self.list.iter().find(|c| c.handles(path, bytes)) {
            Some(c) => c.convert(path, bytes),
            None => cobolt_docs::convert(&path.to_string_lossy(), bytes, &self.limits),
        }
    }
}
