// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A search of the player's Rust sources for what must never be in them (WASM-013 R5, WASM-016 R15
//! and AC10, WASM-017 R14):
//!
//! * **no API that inserts markup or runs a string** — `innerHTML`, `outerHTML`, `insertAdjacentHTML`,
//!   `set_inner_html`, `DOMParser`, `createContextualFragment`, `srcdoc`, `document.write`,
//!   `Function`/`eval`, a string-argument timer or `importScripts`. The player builds every node with
//!   `createElement`, `setAttribute`, `textContent` and the CSSOM, the Viewer included;
//! * **no `Closure::forget`** (or `.forget()` on one): every closure the player gives the browser is
//!   owned by a value that is dropped with the instance, so unmounting leaves nothing behind;
//! * **no `unsafe-inline`**: a `style` attribute would be refused by the page's CSP, so none is written.
//!
//! What the player's JavaScript *glue* contains is searched where the bundle is assembled
//! (`cobolt-web-bundle/tests/no_string_execution.rs`).

use std::path::{Path, PathBuf};

const FORBIDDEN: &[(&str, &str)] = &[
    ("innerHTML", "inserts markup"),
    ("outerHTML", "inserts markup"),
    ("insertAdjacentHTML", "inserts markup"),
    ("insert_adjacent_html", "inserts markup"),
    ("set_inner_html", "inserts markup"),
    ("inner_html", "inserts markup (Leptos's `inner_html` attribute, WASM-016 R15)"),
    ("set_outer_html", "inserts markup"),
    ("insert_adjacent_element_html", "inserts markup"),
    ("createContextualFragment", "inserts markup"),
    ("create_contextual_fragment", "inserts markup"),
    ("DOMParser", "parses markup into a document"),
    ("srcdoc", "an iframe built from markup"),
    ("set_srcdoc", "an iframe built from markup"),
    ("document.write", "writes markup into the page"),
    ("write_ln", "writes markup into the page (`Document::write_ln`)"),
    ("Function::new_", "builds script from a string"),
    ("new Function", "builds script from a string"),
    ("\"eval\"", "looks up `eval` by name"),
    ("eval(", "evaluates a string as script"),
    ("set_timeout_with_str", "a timer that evaluates a string"),
    ("set_interval_with_str", "a timer that evaluates a string"),
    ("importScripts", "loads script by address"),
    ("import_scripts", "loads script by address"),
    ("Closure::forget", "a closure the page can never free"),
    (".forget()", "a closure the page can never free"),
    ("unsafe-inline", "a `style` attribute or inline script the CSP refuses"),
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).expect("a readable source directory").flatten() {
        let p = e.path();
        if p.is_dir() {
            // The vendored Mermaid renderer is not the player's code, and its own crate never touches the DOM.
            if p.file_name().is_some_and(|n| n == "vendor" || n == "target" || n == "target-player") {
                continue;
            }
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Lines that are only a comment are not code (the documentation says what is *not* done).
fn is_code(line: &str) -> bool {
    let t = line.trim_start();
    !(t.starts_with("//"))
}

/// The same text with everything from the first `#[cfg(test)]` on removed: a test may name a
/// forbidden API in a string to prove the search itself works.
fn without_tests(text: &str) -> &str {
    match text.find("#[cfg(test)]") {
        Some(at) => &text[..at],
        None => text,
    }
}

#[test]
fn the_players_sources_insert_no_markup_run_no_string_and_leak_no_closure() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src"), &mut files);
    rust_files(&root.join("parts"), &mut files);
    files.sort();
    let mut problems = Vec::new();
    let mut lines_scanned = 0usize;
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        // A file that is itself a test module (`host/auth_tests.rs`, declared `#[cfg(test)] mod`) is not shipped.
        if f.file_name().is_some_and(|n| n.to_string_lossy().ends_with("_tests.rs")) {
            continue;
        }
        for (n, line) in without_tests(&text).lines().enumerate() {
            if !is_code(line) {
                continue;
            }
            lines_scanned += 1;
            for (needle, why) in FORBIDDEN {
                if line.contains(needle) {
                    problems.push(format!("{}:{}: `{needle}` ({why}): {}", f.strip_prefix(root).unwrap().display(), n + 1, line.trim()));
                }
            }
        }
    }
    println!("searched {} source files, {lines_scanned} lines of code, {} forbidden names", files.len(), FORBIDDEN.len());
    assert!(files.len() > 30, "the search found only {} files: is it looking in the right place?", files.len());
    assert!(problems.is_empty(), "forbidden APIs in the player:\n{}", problems.join("\n"));
}

#[test]
fn every_node_the_page_gets_is_made_with_the_dom_builder_and_the_viewer_never_takes_markup() {
    // The positive half: the elements of the page are made by Leptos's element builders in
    // `dom/reactive.rs` (the page's own few by `dom/h.rs`, with `createElement`), their attributes
    // and styles are set through `dom/attrs.rs` with `setAttribute` and the CSSOM, and the Viewer's
    // view module has no function that takes a string of markup into the page.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let h = std::fs::read_to_string(root.join("src/dom/h.rs")).unwrap();
    assert!(h.contains("create_element"), "the page's own elements are made with createElement");
    let attrs = std::fs::read_to_string(root.join("src/dom/attrs.rs")).unwrap();
    assert!(attrs.contains("set_attribute") && attrs.contains("h::css("), "attributes through setAttribute, styles through the CSSOM");
    let reactive = std::fs::read_to_string(root.join("src/dom/reactive.rs")).unwrap();
    assert!(reactive.contains("leptos::html::custom") && reactive.contains("super::attrs::"), "the Leptos layer builds elements with Leptos and applies attributes by the shared rules");
    assert!(!root.join("src/dom/apply.rs").exists(), "the hand-written patch is retired (WASM-016 T24)");
    let viewer = std::fs::read_to_string(root.join("src/view/viewer.rs")).unwrap();
    let code: String = without_tests(&viewer).lines().filter(|l| is_code(l)).collect::<Vec<_>>().join("\n");
    assert!(code.contains("VNode::new(\"p\")") && code.contains("RawHtml"), "the Viewer builds nodes and keeps raw HTML as text");
    assert!(!code.contains("web_sys"), "the Viewer's view layer touches no browser API at all");
}

#[test]
fn the_search_itself_finds_what_it_looks_for() {
    // A planted violation is found by the same rule the real search uses.
    let sample = "let _ = el.set_inner_html(&s);\n// an explanation: never innerHTML\nlet v = view! { <div inner_html=s /> };\nlet c = Closure::forget(f);";
    let found: Vec<&str> = sample.lines().filter(|l| is_code(l)).flat_map(|l| FORBIDDEN.iter().filter(move |(n, _)| l.contains(n)).map(|(n, _)| *n)).collect();
    assert_eq!(found, ["set_inner_html", "inner_html", "inner_html", "Closure::forget"], "the comment line is not code; Leptos's `inner_html` is caught");
}
