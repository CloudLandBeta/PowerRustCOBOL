// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! PDF: each page's text, read one page at a time the way the Viewer reads it
//! (`cobolt-forms` `viewer::parse_pdf`), under a `## Page N` heading so a hit
//! can name its page (spec 074 R4, R9). No OCR: a page with no text layer has
//! nothing to give.

use crate::sniff::find;
use crate::{code, Skip};

pub fn convert(bytes: &[u8]) -> Result<String, Skip> {
    let protected = || Skip::new(code::PASSWORD_PROTECTED, "the PDF is password protected");
    let doc = match lopdf::Document::load_mem(bytes) {
        Ok(doc) => doc,
        // The reader has no decryption; an encrypted PDF may fail to load.
        Err(_) if find(bytes, b"/Encrypt").is_some() => return Err(protected()),
        Err(e) => return Err(Skip::new(code::DAMAGED, format!("the PDF could not be read: {e}"))),
    };
    if doc.trailer.get(b"Encrypt").is_ok() {
        return Err(protected());
    }
    let mut out = String::new();
    let mut any_text = false;
    for &number in doc.get_pages().keys() {
        // One page at a time: a page this reader cannot follow costs that
        // page's text, not the document's.
        let text = doc.extract_text(&[number]).unwrap_or_default();
        let text = text.trim();
        any_text |= !text.is_empty();
        out.push_str(&format!("## Page {number}\n\n{text}\n\n"));
    }
    if !any_text {
        return Err(Skip::new(
            code::NO_TEXT,
            "the PDF has no text layer (a scan?); nothing could be indexed",
        ));
    }
    Ok(out)
}
