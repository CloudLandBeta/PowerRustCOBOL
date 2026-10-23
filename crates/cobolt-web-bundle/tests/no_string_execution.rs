// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The JavaScript a bundle ships — the loader, the Worker, the host page's script, the optional
//! service-worker shim, and the **glue `wasm-bindgen` generated for the player and for each part** —
//! evaluates no string and inserts no markup (WASM-013 R5, WASM-016 R15, AC10). The page's CSP has no
//! `'unsafe-eval'`, so such a call would fail in the browser; this test makes it fail first, in the
//! build, naming the file and the line.
//!
//! The Rust sources of the player have their own search (`crates/cobolt-web-player/tests/sinks.rs`).

mod support;

use cobolt_web_bundle::{BundleOptions, PlayerInput};

/// `(needle, why)`: a hit is a call or a property, never part of a longer identifier.
const FORBIDDEN: &[(&str, &str)] = &[
    ("eval", "evaluates a string as script"),
    ("Function", "`new Function(...)` / `Function(...)` builds script from a string"),
    ("innerHTML", "inserts markup"),
    ("outerHTML", "inserts markup"),
    ("insertAdjacentHTML", "inserts markup"),
    ("createContextualFragment", "inserts markup"),
    ("DOMParser", "parses markup into a document"),
    ("srcdoc", "an iframe built from markup"),
    ("importScripts", "loads script by address, outside the CSP's module rules"),
];

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// Lines that are only a comment are not code.
fn code_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines().enumerate().map(|(i, l)| (i + 1, l)).filter(|(_, l)| {
        let t = l.trim_start();
        !(t.starts_with("//") || t.starts_with('*') || t.starts_with("/*"))
    })
}

/// Every place `needle` is used as a name of its own (not inside a longer identifier). For `Function`
/// and `eval` only a **call** counts (`new Function(`, `eval(`): the glue prints `Function(name)` in a
/// template literal for debugging, and `typeof x == 'function'` is lower case.
fn hits(text: &str, needle: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (n, line) in code_lines(text) {
        let mut from = 0;
        while let Some(at) = line[from..].find(needle) {
            let start = from + at;
            let end = start + needle.len();
            from = end;
            let before = line[..start].chars().next_back();
            let after = line[end..].chars().next();
            if before.is_some_and(is_ident) || after.is_some_and(is_ident) {
                continue;
            }
            let call = line[end..].trim_start().starts_with('(');
            let constructed = line[..start].trim_end().ends_with("new");
            // An odd number of backticks before the hit: it is inside a template literal (a debugging message).
            let in_template = line[..start].matches('`').count() % 2 == 1;
            let needs_call = matches!(needle, "eval" | "Function");
            if needs_call && !(call && (needle == "eval" || constructed)) {
                continue;
            }
            if needs_call && in_template {
                continue;
            }
            out.push((n, line.trim().chars().take(120).collect()));
        }
    }
    out
}

#[test]
fn no_javascript_of_the_bundle_evaluates_a_string_or_inserts_markup() {
    let player_out = support::built_player();
    let plain = support::player_variant(&player_out, "plain");
    let out = support::assemble(
        PlayerInput { plain: Some(plain), parts: support::player_parts(&player_out), ..Default::default() },
        BundleOptions { service_worker_shim: true, ..Default::default() },
    );
    let mut scanned = Vec::new();
    let mut problems = Vec::new();
    for f in &out.files {
        if !f.path.ends_with(".js") {
            continue;
        }
        let text = String::from_utf8_lossy(&f.bytes);
        scanned.push(format!("{} ({} bytes)", f.path, f.bytes.len()));
        for (needle, why) in FORBIDDEN {
            for (line, code) in hits(&text, needle) {
                problems.push(format!("{}:{line}: `{needle}` ({why}): {code}", f.path));
            }
        }
    }
    println!("scanned {} script files:", scanned.len());
    for s in &scanned {
        println!("  {s}");
    }
    assert!(scanned.iter().any(|s| s.contains("player.js")) && scanned.iter().any(|s| s.contains("mermaid.js")) && scanned.iter().any(|s| s.contains("loader.")), "the player's glue, the part's glue and the loader are all in the scan: {scanned:?}");
    assert!(problems.is_empty(), "string execution or markup insertion found:\n{}", problems.join("\n"));
}

/// Every `.js` under a built **threaded** player (the glue and the `wasm-bindgen-rayon` worker helper),
/// wherever `build_player` wrote one under `target-player/`. The test's own build is the plain player
/// (the nightly build takes minutes), so this scans what is there and says what it found.
fn threaded_scripts() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, in_threaded: bool, out: &mut Vec<(String, String)>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                walk(&p, in_threaded || name == "threaded", out);
            } else if in_threaded && name.ends_with(".js") {
                out.push((p.display().to_string(), std::fs::read_to_string(&p).unwrap_or_default()));
            }
        }
    }
    let mut out = Vec::new();
    // The committed player a Build copies (WASM-016 Q2): its threaded glue and worker helper.
    walk(&support::repo_root().join("assets/web-player/threaded"), true, &mut out);
    let root = support::repo_root().join("crates/cobolt-web-player/target-player");
    for e in std::fs::read_dir(&root).into_iter().flatten().flatten() {
        if e.file_name().to_string_lossy().starts_with("out-") {
            walk(&e.path(), false, &mut out);
        }
    }
    out
}

#[test]
fn the_threaded_players_glue_and_worker_helper_are_clean_too() {
    let files = threaded_scripts();
    println!("threaded scripts found: {}", files.len());
    for (path, _) in &files {
        println!("  {path}");
    }
    assert!(files.iter().any(|(p, _)| p.ends_with("workerHelpers.js")), "the committed threaded player's worker helper is scanned");
    let mut problems = Vec::new();
    for (path, text) in &files {
        for (needle, why) in FORBIDDEN {
            for (line, code) in hits(text, needle) {
                problems.push(format!("{path}:{line}: `{needle}` ({why}): {code}"));
            }
        }
    }
    assert!(problems.is_empty(), "string execution or markup insertion in the threaded player:\n{}", problems.join("\n"));
}

#[test]
fn the_search_finds_what_it_looks_for_and_only_that() {
    assert_eq!(hits("x = eval('1');", "eval").len(), 1);
    assert_eq!(hits("const f = new Function('a', 'return a');", "Function").len(), 1);
    assert_eq!(hits("el.innerHTML = s;", "innerHTML").len(), 1);
    assert_eq!(hits("p.insertAdjacentHTML('beforeend', s)", "insertAdjacentHTML").len(), 1);
    // Not hits: another identifier, a comment, a type test, a debugging template literal.
    assert!(hits("retrieval(x); evaluate(y); myeval(z)", "eval").is_empty());
    assert!(hits("// never eval( anything\n   * new Function(x)\n", "Function").is_empty());
    assert!(hits("if (typeof x == 'function') {}", "Function").is_empty());
    assert!(hits("return `Function(${name})`;", "Function").is_empty());
    assert!(hits("WebAssembly.Function(x)", "Function").is_empty(), "not constructed with `new`");
}
