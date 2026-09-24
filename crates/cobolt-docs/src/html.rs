// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! HTML with `htmd` (spec 074 R5, R23): headings, lists and tables kept;
//! scripts, styles and navigation dropped. `htmd` keeps the text of those
//! elements unless told to skip them.

use crate::{code, Skip};

const SKIPPED: [&str; 10] = [
    "script", "style", "nav", "noscript", "head", "iframe", "svg", "form", "template", "img",
];

pub fn convert(bytes: &[u8]) -> Result<String, Skip> {
    let html = String::from_utf8_lossy(bytes);
    htmd::HtmlToMarkdown::builder()
        .skip_tags(SKIPPED.to_vec())
        .build()
        .convert(html.trim_start_matches('\u{feff}'))
        .map_err(|e| Skip::new(code::DAMAGED, format!("the HTML could not be read: {e}")))
}
