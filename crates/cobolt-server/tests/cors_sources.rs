// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! One CORS function (WASM-013 T10, R6; AC5 server half): the headers that grant a browser access — and above all
//! the one that lets it send cookies — are written in one place, `http::cors_headers`, so no other code can pair a
//! wildcard with credentials. This test searches the source for them.

use std::path::{Path, PathBuf};

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

#[test]
fn the_cors_grant_headers_are_written_only_inside_cors_headers() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    let needles = ["access-control-allow-origin", "access-control-allow-credentials", "access_control_allow_origin", "access_control_allow_credentials"];
    let mut outside = Vec::new();
    let mut inside = 0;
    for f in &files {
        let rel = f.strip_prefix(&src).unwrap().to_string_lossy().replace('\\', "/");
        let text = std::fs::read_to_string(f).unwrap();
        // The unit tests of a file are not the server: cut them off.
        let code = text.split("#[cfg(test)]").next().unwrap();
        // The body of `cors_headers` in http.rs is the one allowed place.
        let (allowed_from, allowed_to) = if rel == "http.rs" {
            let a = code.find("pub fn cors_headers").expect("cors_headers is in http.rs");
            let b = code[a..].find("\n}\n").map(|n| a + n).expect("its end");
            (a, b)
        } else {
            (0, 0)
        };
        for (i, line) in code.lines().enumerate() {
            let at = code.lines().take(i).map(|l| l.len() + 1).sum::<usize>();
            let lower = line.to_ascii_lowercase();
            if line.trim_start().starts_with("//") {
                continue;
            }
            if needles.iter().any(|n| lower.contains(n)) {
                if rel == "http.rs" && at >= allowed_from && at <= allowed_to {
                    inside += 1;
                } else {
                    outside.push(format!("{rel}:{}", i + 1));
                }
            }
        }
    }
    println!("cors sources: {} files searched; {inside} line(s) inside cors_headers, {} outside", files.len(), outside.len());
    assert!(inside >= 2, "the grant headers are in cors_headers");
    assert!(outside.is_empty(), "CORS grant headers written outside cors_headers: {outside:?}");
}
