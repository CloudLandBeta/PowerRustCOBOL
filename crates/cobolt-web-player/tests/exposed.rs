// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The exposure list (WASM-017 R14, AC11): every `#[wasm_bindgen]` type of the player and its parts
//! is on `cobolt_web_player::EXPOSED`, each listed type has a disposal path this test can see in the
//! sources, and the shipped player carries no test hook (R15).

use std::path::{Path, PathBuf};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// The types a source exposes: a `#[wasm_bindgen]` attribute whose item (after further attributes
/// and doc comments) is a `struct` or an `impl`.
fn exposed_types(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if !l.starts_with("#[wasm_bindgen") {
            continue;
        }
        let Some(item) = lines[i + 1..]
            .iter()
            .find(|n| !n.starts_with("#[") && !n.starts_with("///") && !n.starts_with("//"))
        else {
            continue;
        };
        let name = if let Some(r) = item.strip_prefix("pub struct ") {
            r
        } else if let Some(r) = item.strip_prefix("impl ") {
            r
        } else {
            continue;
        };
        let name: String = name
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// Exposed types not on the list.
fn unlisted(found: &[String]) -> Vec<String> {
    found
        .iter()
        .filter(|t| !cobolt_web_player::EXPOSED.iter().any(|(n, _)| n == t))
        .cloned()
        .collect()
}

/// The body of the first `fn name` / `name(` block after `start` in `text`, up to its closing line.
fn body_of<'a>(text: &'a str, start: &str) -> &'a str {
    let at = text
        .find(start)
        .unwrap_or_else(|| panic!("{start:?} not found"));
    let rest = &text[at..];
    let indent = rest
        .lines()
        .next()
        .map(|_| text[..at].rsplit('\n').next().unwrap_or("").len())
        .unwrap_or(0);
    let close = format!("\n{}}}", " ".repeat(indent));
    &rest[..rest.find(&close).unwrap_or(rest.len())]
}

/// Whether the sources show a disposal path for `ty`.
fn disposed(ty: &str) -> Result<String, String> {
    let read = |rel: &str| {
        std::fs::read_to_string(crate_dir().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    };
    match ty {
        "Player" => {
            let loader = read("../cobolt-web-bundle/src/hostpage.rs");
            let unmount = body_of(&loader, "async unmount() {");
            unmount
                .contains("player.free()")
                .then(|| "the loader's unmount() frees it".to_owned())
                .ok_or_else(|| "the loader's unmount() does not call player.free()".into())
        }
        "Surface2D" => {
            let dom = read("src/dom/mod.rs");
            let canvas = read("src/dom/canvas.rs");
            let unmount = body_of(&dom, "fn unmount(self: &Rc<Self>)");
            let all = body_of(&canvas, "pub(crate) fn clear_all_canvases");
            let close = body_of(&canvas, "pub(crate) fn clear_canvas(");
            (unmount.contains("clear_all_canvases")
                && all.contains("\"free\"")
                && close.contains("\"free\""))
            .then(|| "freed by clear_canvas() when its instance closes and by unmount()".to_owned())
            .ok_or_else(|| "no free() from the player's unmount()".into())
        }
        "Canvas" => {
            let mut files = Vec::new();
            rust_files(&crate_dir().join("src"), &mut files);
            let made = files.iter().any(|f| {
                std::fs::read_to_string(f)
                    .unwrap()
                    .contains("&\"Canvas\".into()")
            });
            (!made)
                .then(|| "page-owned: the player never makes one".to_owned())
                .ok_or_else(|| "the player makes a Canvas it does not free".into())
        }
        other => Err(format!(
            "{other} is listed but this test knows no disposal path for it"
        )),
    }
}

#[test]
fn every_exposed_type_is_listed_and_disposed_017() {
    let mut files = Vec::new();
    rust_files(&crate_dir().join("src"), &mut files);
    for part in std::fs::read_dir(crate_dir().join("parts"))
        .unwrap()
        .flatten()
    {
        let lib = part.path().join("src/lib.rs");
        if lib.is_file() {
            files.push(lib);
        }
    }
    let mut found = Vec::new();
    for f in &files {
        for t in exposed_types(&std::fs::read_to_string(f).unwrap()) {
            if !found.contains(&t) {
                found.push(t);
            }
        }
    }
    found.sort();
    assert_eq!(
        unlisted(&found),
        Vec::<String>::new(),
        "exposed through wasm-bindgen but not on EXPOSED"
    );
    let mut paths = Vec::new();
    for (ty, how) in cobolt_web_player::EXPOSED {
        assert!(
            found.iter().any(|t| t == ty),
            "{ty} is listed but no longer exposed"
        );
        let seen = disposed(ty).unwrap_or_else(|e| panic!("{ty}: {e} (EXPOSED says: {how})"));
        paths.push(format!("{ty}: {seen}"));
    }
    // A planted type is caught.
    let planted =
        exposed_types("/// x\n#[wasm_bindgen]\n#[derive(Debug)]\npub struct Planted { a: u8 }\n");
    assert_eq!(
        unlisted(&planted),
        ["Planted"],
        "a planted unlisted type is reported"
    );
    println!(
        "exposed: {} types found in {} files, {} listed; {}",
        found.len(),
        files.len(),
        cobolt_web_player::EXPOSED.len(),
        paths.join("; ")
    );
}

/// R15: `force_panic_for_test` is a test hook — gated by the `test-hooks` feature, absent from the
/// shipped player, and the build record says the shipped player has no feature.
#[test]
fn the_shipped_player_has_no_test_hook_017() {
    let api = std::fs::read_to_string(crate_dir().join("src/api.rs")).unwrap();
    let at = api
        .find("pub fn force_panic_for_test")
        .expect("the hook exists");
    assert!(
        api[..at]
            .rsplit("\n\n")
            .next()
            .unwrap()
            .contains("#[cfg(feature = \"test-hooks\")]"),
        "the hook is gated"
    );
    let assets = crate_dir().join("../../assets/web-player");
    let mut searched = 0;
    for v in ["plain", "threaded"] {
        let js = std::fs::read_to_string(assets.join(v).join("player.js")).unwrap();
        assert!(
            !js.contains("force_panic_for_test"),
            "the shipped {v} player exports the test hook"
        );
        searched += 1;
    }
    let record: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(assets.join("player-build.json")).unwrap())
            .unwrap();
    assert_eq!(
        record["features"],
        serde_json::json!([]),
        "the shipped player is built with no feature"
    );
    println!("shipped player: {searched} variants searched, no force_panic_for_test; features []");
}
