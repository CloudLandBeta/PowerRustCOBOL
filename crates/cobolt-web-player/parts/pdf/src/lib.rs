// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The PDF part of the player** (WASM-016 R13).
//!
//! [`read`] takes the bytes of a PDF and answers, as text, one page per block: the page number, its size
//! in points and its text, as `cobolt_forms::viewer::parse_pdf` reads them (the desktop Viewer's own
//! reader). The answer is JSON: `{"pages":[{"width":612,"height":792,"text":"..."}]}`.

use wasm_bindgen::prelude::*;

fn describe(bytes: &[u8]) -> Result<String, String> {
    let doc = cobolt_forms::viewer::parse_pdf(bytes)?;
    let mut out = String::from("{\"pages\":[");
    for (i, p) in doc.pages.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!("{{\"width\":{},\"height\":{},\"text\":{}}}", p.width_pt, p.height_pt, json_string(&p.text)));
    }
    out.push_str("]}");
    Ok(out)
}

fn json_string(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// The pages of a PDF, as JSON text, or the reader's own reason it cannot be read.
#[wasm_bindgen]
pub fn read(bytes: &[u8]) -> Result<String, JsError> {
    describe(bytes).map_err(|e| JsError::new(&e))
}

#[cfg(test)]
mod tests {
    /// A one-page PDF with correct offsets, built by hand: the part reads what the desktop Viewer reads.
    fn one_page_pdf() -> Vec<u8> {
        let content = "BT /F1 18 Tf 72 700 Td (Hello from the PDF part) Tj 0 -24 Td (Second line) Tj ET";
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_owned(),
            format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        ];
        let mut out = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (i, o) in objs.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
        out
    }

    #[test]
    fn a_document_that_is_not_a_pdf_says_why() {
        assert!(super::describe(b"not a pdf").is_err());
    }

    #[test]
    fn a_pdf_answers_its_pages_size_and_text_as_json() {
        let json = super::describe(&one_page_pdf()).expect("a PDF");
        assert!(json.starts_with("{\"pages\":[{\"width\":612,\"height\":792,\"text\":\""), "{json}");
        assert!(json.contains("Hello from the PDF part") && json.contains("Second line"), "{json}");
    }
}
