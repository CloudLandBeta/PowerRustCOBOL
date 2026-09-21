// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 062 R19 / T10 — **the pages the program wrote are the pages the reader
//! turns.**
//!
//! Nothing in the Viewer changed for this. `viewer::index_text` has always cut
//! a plain-text document at a form feed, and the runtime now emits one for
//! `WRITE ... AFTER ADVANCING PAGE`. This test is what makes that meeting a
//! promise rather than a coincidence: it takes the bytes the runtime produces
//! for a three-page report and asserts the Viewer finds three pages in them,
//! each holding its own lines.

use cobolt_forms::viewer::{decode_text_page, index_text, DocumentSource};

/// The bytes `exec_write` produces for a report whose program wrote three
/// pages, character for character — a form feed before each page after the
/// first, and one newline per record.
const REPORT: &str = "SALES — page 1\nRegion A          1,204\n\u{000C}SALES — page 2\nRegion B          3,318\nRegion C            907\n\u{000C}SALES — page 3\nTOTAL             5,429\n";

#[test]
fn a_report_breaks_its_pages_where_the_program_did() {
    let source = DocumentSource::Bytes(REPORT.as_bytes().into());
    let index = index_text(&source).expect("index the report");

    println!("  {} bytes → {} pages", REPORT.len(), index.page_count());
    for (i, span) in index.pages.iter().enumerate() {
        let text = decode_text_page(&source, *span).expect("decode");
        println!(
            "    page {}: bytes {}..{} {:?}",
            i + 1,
            span.start,
            span.end,
            text.lines().next().unwrap_or("")
        );
    }

    assert_eq!(
        index.page_count(),
        3,
        "three ADVANCING PAGE breaks, three pages"
    );

    let page = |i: usize| decode_text_page(&source, index.pages[i]).expect("decode");
    assert!(page(0).contains("page 1") && page(0).contains("Region A"));
    assert!(
        page(1).contains("page 2") && page(1).contains("Region C"),
        "a page holds every line written on it"
    );
    assert!(page(2).contains("TOTAL"));
    assert!(
        !page(0).contains("page 2"),
        "and none of the next one's"
    );
}

/// A report with no page breaks is one page — the reader is not given
/// pagination the program never asked for.
#[test]
fn a_report_with_no_page_breaks_is_one_page() {
    let flat = "line one\nline two\nline three\n";
    let source = DocumentSource::Bytes(flat.as_bytes().into());
    let index = index_text(&source).expect("index");
    println!("  {:?} → {} page(s)", flat, index.page_count());
    assert_eq!(index.page_count(), 1);
}
