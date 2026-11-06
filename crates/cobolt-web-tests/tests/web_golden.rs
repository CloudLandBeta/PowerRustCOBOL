// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Goldens for what the web Build writes** (WASM-015 T15, R7, AC7): the generated COBOL of the Web
//! template's form, the backend's interface description, the form description, the form's and the
//! application's stylesheets, the host page, `config.json`, the headers (CSP) and the manifest.
//!
//! The project is the Web template, made by the same tool the IDE uses, so a change to the template, the
//! generator or the bundle moves a golden and is seen in review. Content hashes, SRI digests, part sizes and
//! the CSP script hash change with every player or module rebuild and say nothing about the format, so they
//! are replaced by a placeholder before comparing (`normalize`). `UPDATE_GOLDEN=1` rewrites the goldens.
//! Every artefact is compared **before** the test fails, and an artefact whose producer is missing is reported
//! `owed:` and does not fail.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web;
use cobolt_project_tools::{HeadlessHost, ProjectTools};
use serde_json::json;

/// Placeholders for what changes with every rebuild and carries no format information.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let b = text.as_bytes();
    let mut i = 0;
    let is_hex = |c: u8| c.is_ascii_digit() || (b'a'..=b'f').contains(&c);
    while i < b.len() {
        let rest = &text[i..];
        // sha384-/sha256- digests (base64).
        if let Some(r) = rest.strip_prefix("sha384-").or_else(|| rest.strip_prefix("sha256-")) {
            let n = r.bytes().take_while(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/' | b'=')).count();
            out.push_str(&rest[..7]);
            out.push_str("<digest>");
            i += 7 + n;
            continue;
        }
        // a content hash in a name: `.xxxxxxxx.` or `-xxxxxxxx/` or `-xxxxxxxx.` (8 hex digits).
        if matches!(b[i], b'.' | b'-') && i + 9 < b.len() + 1 && i + 9 <= b.len() - 1 + 1 {
            let h = &b[i + 1..(i + 9).min(b.len())];
            let next = b.get(i + 9).copied();
            if h.len() == 8 && h.iter().all(|c| is_hex(*c)) && matches!(next, Some(b'.') | Some(b'/') | Some(b'"')) && h.iter().any(|c| c.is_ascii_digit()) && h.iter().any(|c| c.is_ascii_lowercase()) {
                out.push(b[i] as char);
                out.push_str("<hash>");
                i += 9;
                continue;
            }
        }
        // `"bytes": 123456`
        if let Some(r) = rest.strip_prefix("\"bytes\": ") {
            let n = r.bytes().take_while(u8::is_ascii_digit).count();
            out.push_str("\"bytes\": 0");
            i += 9 + n;
            continue;
        }
        let ch = rest.chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join("golden")
}

fn find(dir: &Path, prefix: &str, suffix: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(prefix) && n.ends_with(suffix))
    })
}

#[test]
fn the_web_build_outputs_match_their_goldens() {
    let base = std::env::temp_dir().join(format!("prc-web-golden-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let folder = base.join("Hello");
    let mut tools = ProjectTools::new(HeadlessHost::new(base.clone(), "test"));
    let made = tools.call("create_project", &json!({"folder": folder.to_string_lossy(), "name": "Hello", "template": "web"})).unwrap();
    let manifest = folder.join(made["project"].as_str().unwrap());
    tools.call("open_project", &json!({"path": manifest.to_string_lossy()})).unwrap();
    tools.call("regenerate", &json!({})).unwrap();
    web::link(Arc::new(GeneratorBackend));
    let built = tools.call("build", &json!({})).unwrap();
    assert_eq!(built["status"], "built", "{built}");
    let dist = folder.join(built["bundle"].as_str().unwrap());
    let interface = cobolt_server::dev::write_interface(&folder.join("backend")).map_err(|e| e.join("; "));

    // (golden name, the produced text or why it is owed)
    let mut artefacts: Vec<(&str, Result<String, String>)> = Vec::new();
    let read = |p: Option<PathBuf>, owed: &str| p.and_then(|p| std::fs::read_to_string(p).ok()).ok_or_else(|| owed.to_owned());
    artefacts.push(("main-form.cbl", read(Some(folder.join("generated/main-form.cbl")), "owed: WASM-004 (the generated COBOL)")));
    artefacts.push((
        "interface.json",
        interface.and_then(|p| std::fs::read_to_string(p).map_err(|e| e.to_string())).map_err(|e| format!("owed: WASM-003 ({e})")),
    ));
    artefacts.push(("form-description.json", read(find(&dist.join("forms"), "MAIN-FORM.", ".json").filter(|p| !p.to_string_lossy().contains(".menu.")), "owed: WASM-004 (the form description)")));
    artefacts.push(("form.css", read(find(&dist.join("forms"), "MAIN-FORM.", ".css"), "owed: WASM-007 (the form's stylesheet)")));
    artefacts.push(("application.css", read(find(&dist, "app.", ".css"), "owed: WASM-007 (the application stylesheet)")));
    artefacts.push(("stylesheet.css", read(find(&dist, "stylesheet.", ".css"), "owed: WASM-007 (the project stylesheet)")));
    artefacts.push(("index.html", read(Some(dist.join("index.html")), "owed: WASM-002 (the host page)")));
    artefacts.push(("config.json", read(Some(dist.join("config.json")), "owed: WASM-002 (config.json)")));
    artefacts.push(("_headers", read(Some(dist.join("_headers")), "owed: WASM-013 (the headers and CSP)")));
    artefacts.push(("project.toml", read(Some(manifest.clone()), "owed: WASM-001 (the manifest)")));

    let update = std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1");
    let dir = golden_dir();
    if update {
        std::fs::create_dir_all(&dir).unwrap();
    }
    let (mut same, mut different, mut missing, mut owed) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (name, produced) in &artefacts {
        let text = match produced {
            Ok(t) => normalize(&t.replace("\r\n", "\n")),
            Err(why) => {
                owed.push(format!("{name} — {why}"));
                continue;
            }
        };
        let path = dir.join(name);
        if update {
            std::fs::write(&path, &text).unwrap();
            same.push(*name);
            continue;
        }
        match std::fs::read_to_string(&path) {
            Err(_) => missing.push(*name),
            Ok(g) if g == text => same.push(*name),
            Ok(g) => {
                let line = g.lines().zip(text.lines()).position(|(a, b)| a != b).map_or(g.lines().count().min(text.lines().count()) + 1, |i| i + 1);
                different.push(format!("{name} (first difference at line {line})"));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&base);
    println!("web goldens: {} match{}, {} different, {} missing, {} owed", same.len(), if update { " (rewritten)" } else { "" }, different.len(), missing.len(), owed.len());
    for o in &owed {
        println!("  owed: {o}");
    }
    assert!(different.is_empty() && missing.is_empty(), "differ: {different:?}; missing: {missing:?} (UPDATE_GOLDEN=1 rewrites them)");
}

#[cfg(test)]
mod normalizer {
    use super::normalize;

    #[test]
    fn hashes_digests_and_sizes_are_replaced_and_the_format_is_not() {
        let t = "\"url\": \"forms/MAIN-FORM.6e8863e0.json\", \"integrity\": \"sha384-cBjIV5nJ3nCNY53L1eRaJtgGABuHY5dw+/=\", \"bytes\": 2226935, \"js\": \"player/part-pdf-356395c0/pdf.js\", \"v\": 1, \"name\": \"state-of-the-art.html\"";
        let n = normalize(t);
        assert_eq!(
            n,
            "\"url\": \"forms/MAIN-FORM.<hash>.json\", \"integrity\": \"sha384-<digest>\", \"bytes\": 0, \"js\": \"player/part-pdf-<hash>/pdf.js\", \"v\": 1, \"name\": \"state-of-the-art.html\""
        );
    }
}
