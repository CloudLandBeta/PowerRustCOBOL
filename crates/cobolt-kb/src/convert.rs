// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! "Document in, text out" — the seam every format enters through (spec 068
//! R14). This crate reads Markdown and plain text; spec 074 adds Word,
//! PowerPoint, Excel, OpenDocument, PDF, HTML and archives by registering more
//! [`Converter`]s, with nothing else in the Knowledge Base changing.

use std::path::Path;

/// Why a document was not indexed. `code` is stable, for a program to translate
/// in its own tables; `message` is the English explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skip {
    pub code: &'static str,
    pub message: String,
}

impl Skip {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Skip {
            code,
            message: message.into(),
        }
    }
}

/// Turns one document's bytes into Markdown or plain text.
pub trait Converter: Send + Sync {
    /// Whether this converter reads the document at `path`.
    fn handles(&self, path: &Path, bytes: &[u8]) -> bool;
    /// The document's text, with its structure as Markdown headings where the
    /// format has any.
    fn to_text(&self, path: &Path, bytes: &[u8]) -> Result<String, Skip>;
}

/// Markdown and plain text, read as UTF-8.
pub struct PlainText;

const TEXT_EXTENSIONS: [&str; 5] = ["md", "markdown", "txt", "text", "log"];

impl Converter for PlainText {
    fn handles(&self, path: &Path, _bytes: &[u8]) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| TEXT_EXTENSIONS.iter().any(|t| e.eq_ignore_ascii_case(t)))
    }

    fn to_text(&self, _path: &Path, bytes: &[u8]) -> Result<String, Skip> {
        String::from_utf8(bytes.to_vec())
            .map(|s| s.trim_start_matches('\u{feff}').to_string())
            .map_err(|_| Skip::new("damaged", "the text is not valid UTF-8"))
    }
}

/// The converters a Knowledge Base tries, in order.
pub struct Converters {
    list: Vec<Box<dyn Converter>>,
}

impl Default for Converters {
    fn default() -> Self {
        Converters {
            list: vec![Box::new(PlainText)],
        }
    }
}

impl Converters {
    /// Add a converter, tried before the ones already present.
    pub fn register(&mut self, converter: Box<dyn Converter>) {
        self.list.insert(0, converter);
    }

    /// The document's text, or why it cannot be read.
    pub fn to_text(&self, path: &Path, bytes: &[u8]) -> Result<String, Skip> {
        match self.list.iter().find(|c| c.handles(path, bytes)) {
            Some(c) => c.to_text(path, bytes),
            None => Err(Skip::new(
                "unsupported",
                format!(
                    "the format of {} is not supported",
                    path.file_name().and_then(|n| n.to_str()).unwrap_or("this document")
                ),
            )),
        }
    }
}
