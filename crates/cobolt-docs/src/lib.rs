// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Document import** (spec 074): a document's bytes in, Markdown out.
//!
//! Word, PowerPoint and Excel files (and old binary `.xls`), OpenDocument text
//! and spreadsheets, delimited tables, PDF, HTML, Markdown and plain text, and
//! ZIP / TAR archives of any of them. Structure is kept as Markdown headings —
//! document headings, one `## Slide N` per slide, one `## <sheet>` per sheet,
//! one `## Page N` per PDF page — so a reader can split by subject.
//!
//! A format is recognised by its content first and by its name second (R7). A
//! document that cannot be read comes back as a [`Skip`] with a stable code.
//! Nothing a document contains is ever executed (R17), and archives are read
//! in memory within [`Limits`] (R15, R16).
//!
//! This crate knows nothing of the Knowledge Base; the KB depends on it, never
//! the reverse (R22).

mod archive;
mod html;
mod office;
mod pdf;
mod pptx;
mod sniff;

use std::panic::{catch_unwind, AssertUnwindSafe};

pub use sniff::Kind;

/// Why a document was not converted. `code` is stable, for a program to
/// translate in its own tables; `message` is the English explanation.
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

/// The skip codes (spec 074 R13), for a program's translation table.
pub mod code {
    /// Not a format this crate reads — an image, a program, a binary file.
    pub const UNSUPPORTED: &str = "unsupported";
    /// Old binary Word or PowerPoint (`.doc`, `.ppt`).
    pub const LEGACY_OFFICE: &str = "legacy_office";
    pub const PASSWORD_PROTECTED: &str = "password_protected";
    /// A PDF with no text layer — typically a scan.
    pub const NO_TEXT: &str = "no_text";
    /// The file claims a format and does not hold it — truncated, corrupt.
    pub const DAMAGED: &str = "damaged";
    /// An archive past its size, file-count or nesting bound.
    pub const TOO_LARGE: &str = "too_large";
}

/// One converted document. A plain document is one part with no name; an
/// archive gives one part per document inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    /// The path inside the archive — `legal/nda.docx`, or through nested
    /// archives `old.zip › legal/nda.docx`. `None` for the document itself.
    pub name: Option<String>,
    pub markdown: String,
}

/// What a conversion produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Converted {
    pub parts: Vec<Part>,
    /// Documents inside an archive that could not be read, by their path in
    /// it. They never stop the rest of the archive (R14).
    pub skipped: Vec<(String, Skip)>,
}

/// Bounds on archive expansion (R15), shared across every level of nesting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Total bytes unpacked from an archive, nested archives included.
    pub max_unpacked_bytes: u64,
    /// Files in an archive, nested archives included.
    pub max_files: usize,
    /// Archives within archives: 1 reads an archive but none inside it.
    pub max_depth: u8,
}

impl Default for Limits {
    /// 500 MB, 10,000 files, 3 levels (spec 074 Q2).
    fn default() -> Self {
        Limits {
            max_unpacked_bytes: 500 * 1024 * 1024,
            max_files: 10_000,
            max_depth: 3,
        }
    }
}

/// Separates an archive from the path inside it in a part's name (R10).
pub const PATH_SEPARATOR: &str = " › ";

/// Convert one document. `name` is its file name or path, used only when the
/// content does not identify the format.
pub fn convert(name: &str, bytes: &[u8], limits: &Limits) -> Result<Converted, Skip> {
    let mut budget = archive::Budget::new(limits);
    convert_at(name, bytes, 0, &mut budget)
}

/// Convert at a nesting depth (0 = not inside an archive).
fn convert_at(
    name: &str,
    bytes: &[u8],
    depth: u8,
    budget: &mut archive::Budget,
) -> Result<Converted, Skip> {
    let kind = sniff::kind(name, bytes);
    if kind.is_archive() {
        return archive::expand(kind, name, bytes, depth + 1, budget);
    }
    let markdown = convert_document(kind, name, bytes)?;
    Ok(Converted {
        parts: vec![Part {
            name: None,
            markdown,
        }],
        skipped: Vec::new(),
    })
}

/// One document, not an archive. A reader that panics on a hostile file
/// costs that file, never the application (R14).
fn convert_document(kind: Kind, name: &str, bytes: &[u8]) -> Result<String, Skip> {
    catch_unwind(AssertUnwindSafe(|| convert_kind(kind, name, bytes))).unwrap_or_else(|_| {
        Err(Skip::new(
            code::DAMAGED,
            format!("{} could not be read", display_name(name)),
        ))
    })
}

fn convert_kind(kind: Kind, name: &str, bytes: &[u8]) -> Result<String, Skip> {
    match kind {
        Kind::Text => text(bytes),
        Kind::Html => html::convert(bytes),
        Kind::Csv => office::table(bytes),
        Kind::Pdf => pdf::convert(bytes),
        Kind::Docx => office::docx(bytes),
        Kind::Pptx => pptx::convert(bytes),
        Kind::Workbook => office::workbook(bytes),
        Kind::OpenDocument => office::opendoc(bytes),
        Kind::LegacyOffice => Err(Skip::new(
            code::LEGACY_OFFICE,
            format!(
                "{} is an old binary Office document; save it as .docx or .pptx",
                display_name(name)
            ),
        )),
        Kind::Protected => Err(Skip::new(
            code::PASSWORD_PROTECTED,
            format!("{} is password protected", display_name(name)),
        )),
        Kind::Damaged => Err(Skip::new(
            code::DAMAGED,
            format!("{} is damaged or incomplete", display_name(name)),
        )),
        Kind::Unsupported | Kind::Zip | Kind::Tar | Kind::Gzip => Err(Skip::new(
            code::UNSUPPORTED,
            format!("the format of {} is not supported", display_name(name)),
        )),
    }
}

fn text(bytes: &[u8]) -> Result<String, Skip> {
    std::str::from_utf8(bytes)
        .map(|s| s.trim_start_matches('\u{feff}').to_string())
        .map_err(|_| Skip::new(code::DAMAGED, "the text is not valid UTF-8"))
}

fn display_name(name: &str) -> &str {
    name.rsplit(['/', '\\']).next().filter(|n| !n.is_empty()).unwrap_or("this document")
}
