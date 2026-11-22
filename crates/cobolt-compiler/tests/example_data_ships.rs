// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! PowerOficina ships with its data (2026-11-22).
//!
//! The installer stages `examples/` with `git archive HEAD`, so an installed example holds only
//! what git tracks. PowerOficina's `data/*.idx` had never been committed (the 2.0.389 entry said
//! the Backend "seeds them with `seed-load`", which nothing runs for a person who has just
//! opened the example), so on a machine with no earlier installation every screen was empty.
//! This test fails when a file the example needs at first start is not in the repository, when
//! it is not an indexed file, or when it lacks the rows the screens start from.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap().to_path_buf()
}

/// The files git tracks under `dir`, or `None` when this is not a git checkout (a source archive).
fn tracked(dir: &str) -> Option<Vec<String>> {
    let out = Command::new("git").args(["ls-files", "--", dir]).current_dir(repo()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let files: Vec<String> = text.lines().map(str::to_owned).collect();
    // A checkout that git does not know (a copy without .git) answers with nothing for every path.
    if files.is_empty() && !repo().join(".git").exists() { None } else { Some(files) }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn poweroficina_ships_the_data_its_screens_start_from() {
    let t = Instant::now();
    let project = repo().join("examples/PowerOficina");
    let mut definitions: Vec<String> = std::fs::read_dir(project.join("indexed"))
        .expect("PowerOficina has indexed-file definitions")
        .flatten()
        .filter_map(|e| e.path().file_stem().map(|s| s.to_string_lossy().into_owned()).filter(|_| e.path().extension().is_some_and(|x| x == "cidx")))
        .collect();
    definitions.sort();
    assert!(definitions.len() >= 16, "the definitions are found: {definitions:?}");

    let git = tracked("examples/PowerOficina/data");
    let mut problems: Vec<String> = Vec::new();
    let mut bytes = 0usize;
    for name in &definitions {
        let rel = format!("examples/PowerOficina/data/{name}.idx");
        let Ok(data) = std::fs::read(repo().join(&rel)) else {
            problems.push(format!("{rel}: missing"));
            continue;
        };
        bytes += data.len();
        if !data.starts_with(b"PRCIDXD1") {
            problems.push(format!("{rel}: not an indexed file (no PRCIDXD1 header)"));
        }
        if let Some(files) = &git {
            if !files.iter().any(|f| f == &rel) {
                problems.push(format!("{rel}: exists but git does not track it, so an installed example has none"));
            }
        }
    }

    // What the screens start from: the catalog's seed rows and the demo customers.
    let catalog = std::fs::read(project.join("data/catalog.idx")).unwrap_or_default();
    let customers = std::fs::read(project.join("data/customers.idx")).unwrap_or_default();
    let quotes = std::fs::read(project.join("data/quotes.idx")).unwrap_or_default();
    let seeded = contains(&catalog, b"PAD-FREIO");
    let demo = contains(&customers, b"exemplo.com.br");
    if !seeded {
        problems.push("data/catalog.idx: the seed's first catalog row (PAD-FREIO) is not there".into());
    }
    if !demo {
        problems.push("data/customers.idx: no demo customer (an exemplo.com.br address) is there".into());
    }
    if quotes.len() < 16 * 1024 {
        problems.push(format!("data/quotes.idx: {} bytes, too small to hold the demo quotes", quotes.len()));
    }

    let ms = t.elapsed().as_secs_f64() * 1000.0;
    let ok = definitions.iter().filter(|n| !problems.iter().any(|p| p.contains(&format!("data/{n}.idx")))).count();
    println!(
        "┌─ poweroficina_ships_the_data_its_screens_start_from\n│ {} indexed-file definitions, each with its data file: header checked, {}\n│ catalog holds the seed row PAD-FREIO: {seeded}; customers hold the demo addresses: {demo}; quotes.idx {} bytes\n│ {} bytes of data in all; time: {ms:.2} ms ({:.0} files/s)\n└─ {ok}/{} files ship, {} problem(s)",
        definitions.len(),
        match &git {
            Some(_) => "tracked by git".to_owned(),
            None => "git not available here, existence only".to_owned(),
        },
        quotes.len(),
        bytes,
        definitions.len() as f64 / (ms / 1000.0).max(1e-9),
        definitions.len(),
        problems.len()
    );
    assert!(problems.is_empty(), "PowerOficina's data does not ship:\n  {}", problems.join("\n  "));
}
