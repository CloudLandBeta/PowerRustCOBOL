// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Recognising a format — by content where the content says, by name
//! otherwise (spec 074 R7), so a `.docx` renamed `.txt` is still Word.

use std::io::{Cursor, Read};

/// What a document is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    Html,
    /// Comma- or tab-separated table.
    Csv,
    Pdf,
    Docx,
    Pptx,
    /// Excel (`.xlsx`, old `.xls`) or OpenDocument spreadsheet.
    Workbook,
    /// OpenDocument text or presentation.
    OpenDocument,
    Zip,
    Tar,
    Gzip,
    /// Old binary Word or PowerPoint.
    LegacyOffice,
    Protected,
    /// Claims a format it does not hold.
    Damaged,
    Unsupported,
}

impl Kind {
    pub fn is_archive(self) -> bool {
        matches!(self, Kind::Zip | Kind::Tar | Kind::Gzip)
    }
}

/// Markdown and plain text.
const TEXT_EXTENSIONS: [&str; 5] = ["md", "markdown", "txt", "text", "log"];
const HTML_EXTENSIONS: [&str; 3] = ["html", "htm", "xhtml"];
const TABLE_EXTENSIONS: [&str; 3] = ["csv", "tsv", "tab"];
/// Extensions of the zip-based Office and OpenDocument formats: a compound
/// file wearing one is an encrypted package.
const PACKAGE_EXTENSIONS: [&str; 22] = [
    "docx", "docm", "dotx", "dotm", "pptx", "pptm", "potx", "potm", "ppsx", "ppsm", "xlsx",
    "xlsm", "xltx", "xltm", "odt", "ott", "odm", "oth", "ods", "ots", "odp", "otp",
];

const CFB_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

pub fn kind(name: &str, bytes: &[u8]) -> Kind {
    let ext = extension(name);
    let ext = ext.as_str();
    let head = &bytes[..bytes.len().min(1024)];

    if find(head, b"%PDF-").is_some() {
        return Kind::Pdf;
    }
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        return zip_kind(bytes, ext);
    }
    if bytes.starts_with(&CFB_MAGIC) {
        return compound_kind(bytes, ext);
    }
    if bytes.starts_with(&[0x1F, 0x8B]) {
        return Kind::Gzip;
    }
    if is_tar(bytes) {
        return Kind::Tar;
    }
    if is_program(bytes) {
        return Kind::Unsupported;
    }
    if HTML_EXTENSIONS.contains(&ext) {
        return Kind::Html;
    }
    if TABLE_EXTENSIONS.contains(&ext) {
        return Kind::Csv;
    }
    if TEXT_EXTENSIONS.contains(&ext) {
        return Kind::Text;
    }
    if ext.is_empty() && std::str::from_utf8(bytes).is_ok() {
        return if looks_like_html(bytes) { Kind::Html } else { Kind::Text };
    }
    if PACKAGE_EXTENSIONS.contains(&ext) || ext == "pdf" {
        return Kind::Damaged;
    }
    Kind::Unsupported
}

fn extension(name: &str) -> String {
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
    match file.rsplit_once('.') {
        Some((stem, e)) if !stem.is_empty() => e.to_ascii_lowercase(),
        _ => String::new(),
    }
}

/// A ZIP holds an Office package, an OpenDocument or an archive.
fn zip_kind(bytes: &[u8], ext: &str) -> Kind {
    let Ok(mut zip) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return Kind::Damaged;
    };
    let has = |zip: &zip::ZipArchive<Cursor<&[u8]>>, n: &str| zip.index_for_name(n).is_some();
    if has(&zip, "[Content_Types].xml") {
        if has(&zip, "word/document.xml") {
            return Kind::Docx;
        }
        if has(&zip, "ppt/presentation.xml") {
            return Kind::Pptx;
        }
        if has(&zip, "xl/workbook.xml") {
            return Kind::Workbook;
        }
    }
    if let Ok(mut f) = zip.by_name("mimetype") {
        let mut mime = String::new();
        let _ = f.by_ref().take(200).read_to_string(&mut mime);
        let mime = mime.trim();
        if mime.starts_with("application/vnd.oasis.opendocument.spreadsheet") {
            return Kind::Workbook;
        }
        if mime.starts_with("application/vnd.oasis.opendocument.text")
            || mime.starts_with("application/vnd.oasis.opendocument.presentation")
        {
            return Kind::OpenDocument;
        }
    }
    if PACKAGE_EXTENSIONS.contains(&ext) {
        // Named as a package, but not one: its main part is missing.
        return Kind::Damaged;
    }
    Kind::Zip
}

/// A Compound File: old binary Office, or an encrypted Office package.
fn compound_kind(bytes: &[u8], ext: &str) -> Kind {
    // Directory entry names are UTF-16LE.
    let utf16 = |s: &str| s.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<u8>>();
    if find(bytes, &utf16("EncryptionInfo")).is_some()
        || find(bytes, &utf16("EncryptedPackage")).is_some()
        || PACKAGE_EXTENSIONS.contains(&ext)
    {
        return Kind::Protected;
    }
    // Word and PowerPoint first: their text is UTF-16 too, and may say "Book".
    if find(bytes, &utf16("WordDocument")).is_some()
        || find(bytes, &utf16("PowerPoint Document")).is_some()
    {
        return Kind::LegacyOffice;
    }
    if find(bytes, &utf16("Workbook")).is_some() || find(bytes, &utf16("Book")).is_some() {
        return Kind::Workbook;
    }
    Kind::LegacyOffice
}

/// A TAR header: `ustar` magic, or — for old-style headers, which have none —
/// a name and a checksum that adds up.
fn is_tar(bytes: &[u8]) -> bool {
    if bytes.len() < 512 {
        return false;
    }
    if &bytes[257..262] == b"ustar" {
        return true;
    }
    let header = &bytes[..512];
    let stored = std::str::from_utf8(&header[148..156])
        .ok()
        .map(|s| s.trim_matches(|c: char| c == '\0' || c == ' '))
        .and_then(|s| u32::from_str_radix(s, 8).ok());
    let sum: u32 = header
        .iter()
        .enumerate()
        .map(|(i, b)| if (148..156).contains(&i) { u32::from(b' ') } else { u32::from(*b) })
        .sum();
    header[0] != 0 && stored == Some(sum)
}

/// Windows, Linux and macOS executables.
fn is_program(bytes: &[u8]) -> bool {
    const MAGIC: [&[u8]; 6] = [
        b"MZ",
        b"\x7fELF",
        &[0xFE, 0xED, 0xFA, 0xCE],
        &[0xFE, 0xED, 0xFA, 0xCF],
        &[0xCF, 0xFA, 0xED, 0xFE],
        &[0xCA, 0xFE, 0xBA, 0xBE],
    ];
    MAGIC.iter().any(|m| bytes.starts_with(m))
}

fn looks_like_html(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_ascii_lowercase();
    let head = head.trim_start_matches('\u{feff}').trim_start();
    head.starts_with("<!doctype html") || head.starts_with("<html")
}

pub(crate) fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compound(stream: &str) -> Vec<u8> {
        let mut b = CFB_MAGIC.to_vec();
        b.extend(std::iter::repeat_n(0u8, 504));
        b.extend(stream.encode_utf16().flat_map(u16::to_le_bytes));
        b
    }

    #[test]
    fn a_compound_file_is_told_apart_by_its_streams() {
        assert_eq!(kind("price-list.xls", &compound("Workbook")), Kind::Workbook);
        assert_eq!(kind("renamed.bin", &compound("Workbook")), Kind::Workbook, "by content");
        assert_eq!(kind("letter.doc", &compound("WordDocument")), Kind::LegacyOffice);
        assert_eq!(kind("deck.ppt", &compound("PowerPoint Document")), Kind::LegacyOffice);
        assert_eq!(kind("secret.xlsx", &compound("EncryptedPackage")), Kind::Protected);
        assert_eq!(kind("secret.docx", &compound("nothing known")), Kind::Protected);
    }

    #[test]
    fn text_is_read_by_name_and_binary_is_not_text() {
        assert_eq!(kind("notes.md", b"# Hi"), Kind::Text);
        assert_eq!(kind("page.htm", b"<p>x</p>"), Kind::Html);
        assert_eq!(kind("README", b"<!DOCTYPE html><html></html>"), Kind::Html);
        assert_eq!(kind("prices.tsv", b"a\tb"), Kind::Csv);
        assert_eq!(kind("photo.jpg", &[1, 2, 3]), Kind::Unsupported);
        assert_eq!(kind("tool.txt", b"MZ\x90\x00"), Kind::Unsupported, "a program, whatever its name");
        assert_eq!(kind("report.pdf", b"not a pdf"), Kind::Damaged);
    }
}
