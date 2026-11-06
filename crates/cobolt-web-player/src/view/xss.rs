// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Text, never markup (WASM-013 R5, AC4). A value a program sets — a caption, a list item, a grid cell, a
//! tooltip — and a document a Viewer shows reach the page as the text they are. Here the node tree the
//! player describes (what the patch turns into `createElement` and `textContent`) is searched for what
//! hostile markup would have produced: an `img`/`script` element, an `on*` attribute, a `javascript:` link.
//! `tests/sinks.rs` covers the other half: the player has no API that could parse a string into nodes.
//!
//! These tests live beside the views because the tree builders are crate-private test scaffolding.

use super::viewer::{blocks_to_nodes, link_allowed, view as viewer_view, DocFormat, LoadedDoc};
use super::{controls, datagrid, Empty, VNode};
use crate::hyperlink::Policy;
use crate::tree::testing::tree_with;

const HOSTILE: &str = "<img src=x onerror=alert(1)>";

/// Every element a script or a handler could ride on.
const DANGEROUS: &[&str] = &["script", "img", "iframe", "object", "embed", "link", "style", "base", "form", "svg"];

/// What is wrong with `n`, in words; empty when it is clean. SVG is allowed only where the player itself draws it.
fn findings(n: &VNode, allow_tags: &[&str]) -> Vec<String> {
    let mut all = Vec::new();
    n.find_all(&|_| true, &mut all);
    let mut out = Vec::new();
    for x in all {
        if DANGEROUS.contains(&x.tag) && !allow_tags.contains(&x.tag) {
            out.push(format!("element <{}>", x.tag));
        }
        for (k, v) in &x.attrs {
            if k.to_ascii_lowercase().starts_with("on") {
                out.push(format!("attribute {k} on <{}>", x.tag));
            }
            if matches!(k.as_str(), "href" | "src" | "action" | "formaction" | "xlink:href") && v.trim().to_ascii_lowercase().starts_with("javascript:") {
                out.push(format!("{k}={v} on <{}>", x.tag));
            }
        }
    }
    out
}

fn control(kind: &str, props: &[(&str, &str)]) -> VNode {
    let (t, i) = tree_with(kind, "CTL-1", props);
    let e = Empty::default();
    controls::view(&e.cx(&t), i)
}

#[test]
fn a_caption_an_item_a_cell_and_a_tooltip_set_to_markup_stay_text() {
    // A Label's Caption and its tooltip.
    let label = control("Label", &[("Caption", HOSTILE), ("Tooltip", HOSTILE)]);
    assert!(findings(&label, &[]).is_empty(), "{:?}\n{}", findings(&label, &[]), label.describe());
    assert!(label.all_text().contains(HOSTILE), "the caption is the literal text: {}", label.describe());
    assert_eq!(label.attr_of("title"), Some(HOSTILE), "the tooltip is an attribute value, set as text");

    // A Button's Caption.
    let button = control("Button", &[("Caption", HOSTILE)]);
    assert!(findings(&button, &[]).is_empty(), "{}", button.describe());
    assert!(button.all_text().contains(HOSTILE));

    // A ListBox item (one per line).
    let items = format!("one\n{HOSTILE}\n<script>alert(1)</script>");
    let list = control("ListBox", &[("Items", &items)]);
    assert!(findings(&list, &[]).is_empty(), "{}", list.describe());
    assert!(list.all_text().contains(HOSTILE) && list.all_text().contains("<script>alert(1)</script>"), "{}", list.describe());

    // A DataGrid header and cell.
    let cols = format!("{HOSTILE}:string");
    let rows = format!("{HOSTILE}\n<script>alert(2)</script>");
    let (t, i) = tree_with("DataGrid", "GRD-1", &[("Columns", &cols), ("Rows", &rows), ("RowHeight", "30")]);
    let mut e = Empty::default();
    e.ui.box_sizes.insert(i, (600.0, 300.0));
    let grid = datagrid::data_grid(&e.cx(&t), i);
    assert!(findings(&grid, &[]).is_empty(), "{:?}", findings(&grid, &[]));
    assert!(grid.all_text().contains(HOSTILE), "the grid shows the cell as text: {}", grid.describe());
    println!("xss controls: label, button, listbox, datagrid with `{HOSTILE}` -> 0 dangerous nodes, literal text kept");
}

fn viewer_of(html: &str) -> VNode {
    let (t, i) = tree_with("Viewer", "VW-1", &[]);
    let mut e = Empty::default();
    e.ui.viewer_docs.insert(i, LoadedDoc { format: DocFormat::Html, text: html.to_owned(), error: None });
    viewer_view(&e.cx(&t), i)
}

#[test]
fn the_viewer_never_turns_a_documents_markup_into_script_a_handler_or_a_live_link() {
    let doc = concat!(
        "<h1 onclick=\"alert(1)\">Title</h1>",
        "<p onmouseover=alert(2)>Hello <script>alert(3)</script>world</p>",
        "<a href=\"javascript:alert(4)\">js link</a> <a href=\"data:text/html,<b>x</b>\">data link</a> <a href=\"https://example.com/\">ok</a>",
        "<img src=x onerror=alert(5)><iframe src=\"https://evil.example/\"></iframe>",
        "<style>body{background:url(https://evil.example/x)}</style>",
    );
    let n = viewer_of(doc);
    assert!(findings(&n, &[]).is_empty(), "{:?}\n{}", findings(&n, &[]), n.describe());
    let mut anchors = Vec::new();
    n.find_all(&|x| x.tag == "a", &mut anchors);
    let hrefs: Vec<&str> = anchors.iter().filter_map(|a| a.attr_of("href")).collect();
    assert_eq!(hrefs, ["https://example.com/"], "only the allowed link is an anchor: {}", n.describe());
    for kept in ["Title", "js link", "data link"] {
        assert!(n.all_text().contains(kept), "{kept} stays as plain text: {}", n.describe());
    }

    // The same through the blocks directly, and the scheme check on its own.
    let blocks = LoadedDoc { format: DocFormat::Html, text: doc.to_owned(), error: None }.blocks();
    let (t, i) = tree_with("Viewer", "VW-2", &[]);
    let e = Empty::default();
    let nodes = blocks_to_nodes(&e.cx(&t), &blocks, &format!("{i}"));
    for node in &nodes {
        assert!(findings(node, &[]).is_empty(), "{:?}", findings(node, &[]));
    }
    let policy = Policy::default();
    for bad in ["javascript:alert(1)", " JavaScript:alert(1)", "data:text/html,<b>x</b>", "vbscript:x", ""] {
        assert!(!link_allowed(bad, &policy), "{bad:?} must be refused");
    }
    println!("xss viewer: script, onclick/onmouseover, img, iframe, style, javascript:/data: links -> 0 dangerous nodes; 1 of 3 links kept");
}

#[test]
fn the_search_itself_catches_what_it_looks_for() {
    let bad = VNode::new("div").child(VNode::new("img").attr("onerror", "alert(1)")).child(VNode::new("a").attr("href", " JavaScript:alert(1)"));
    let f = findings(&bad, &[]);
    assert_eq!(f.len(), 3, "{f:?}");
    assert!(findings(&VNode::new("div").text("<b>x</b>"), &[]).is_empty());
}
