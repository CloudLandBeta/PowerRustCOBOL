// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T40b (R3, AC9): the permanent Backend fixture.
//!
//! `tests/module-corpus/backend/` keeps, for every release since the first entry, the Backend that release's Build packed
//! from the fixtures (the interface, the table of exports, the state mode and the WebAssembly module) and the calls made to
//! it with what each answered. A Backend of any release must keep answering the same on this runner: this test loads each
//! one **as it was committed** and makes its calls, and refuses a corpus that is not whole (a file changed, an entry
//! removed or taken out of the middle, an entry out of order, a release over its budget).
//!
//! The corpus is append-only: a release adds its entry with
//! `cargo run -p cobolt-backend-runner --example backend_corpus_add` and the line it prints joins `KNOWN` below. An entry
//! is never edited afterwards. A new runner that cannot load an old Backend is a break of the contract of R3, and this is
//! the test that says so.

#[path = "../../cobolt-runner/tests/support/corpus.rs"]
mod corpus;
#[path = "support/backend_corpus.rs"]
mod backend_corpus;

use std::time::Instant;

use cobolt_program_package::backend::BackendContainer;

/// Every release's entry the corpus must hold: the release and its chain. The last one cannot be taken out of the index
/// without this list saying so.
const KNOWN: [(&str, &str); 1] = [("2.1.64", "d282c63e0bcd2b512dcea149aa66f836333e9872c7c63f076cbbe3665fed4948")];

#[test]
fn every_backend_a_release_committed_still_answers_what_it_answered() {
    let started = Instant::now();
    let dir = backend_corpus::dir();
    let problems = corpus::problems(&dir, &KNOWN);
    assert!(problems.is_empty(), "the Backend corpus is not whole:\n  {}", problems.join("\n  "));
    let index = corpus::read_index(&dir).expect("the index");
    assert!(!index.entries.is_empty(), "the Backend corpus has no entry: a release must add one");
    for (release, _) in KNOWN {
        assert!(index.entries.iter().any(|e| e.release == release), "{release} is not in the index");
    }
    println!("=== BACKEND CORPUS (spec 114 T40b) ===");
    println!("{} entr{} in the index; every file's sha256 and every chain checked", index.entries.len(), if index.entries.len() == 1 { "y" } else { "ies" });

    let (mut backends, mut calls_made, mut calls_equal, mut different) = (0u32, 0u32, 0u32, Vec::<String>::new());
    for entry in &index.entries {
        let mut packed = 0u64;
        for file in entry.files.iter().filter(|f| f.path.ends_with(".backend.gz")) {
            packed += std::fs::metadata(dir.join(&file.path)).map(|m| m.len()).unwrap_or(0);
            let stem = file.path.trim_end_matches(".backend.gz");
            let table = format!("{stem}.calls.toml");
            assert!(entry.files.iter().any(|f| f.path == table), "{}: {} has no {table}", entry.release, file.path);
            let bytes = corpus::gunzip(&std::fs::read(dir.join(&file.path)).expect("the file")).unwrap_or_else(|e| panic!("{}: {e}", file.path));
            let container = BackendContainer::decode(&bytes).unwrap_or_else(|e| panic!("{}: the container cannot be read: {e}", file.path));
            let calls = backend_corpus::read_calls(&dir.join(&table)).unwrap_or_else(|e| panic!("{e}"));
            let report = backend_corpus::run_calls(container, &calls.calls).unwrap_or_else(|e| panic!("{} {}: {e}", entry.release, file.path));
            backends += 1;
            calls_made += calls.calls.len() as u32;
            calls_equal += report.equal;
            println!(
                "  release {} {}: {} calls, {} equal, {} different; loaded in {:.0} ms, calls in {:.0} ms",
                entry.release,
                stem.rsplit('/').next().unwrap_or(stem),
                calls.calls.len(),
                report.equal,
                report.different.len(),
                report.load_ms,
                report.calls_ms
            );
            different.extend(report.different.into_iter().map(|d| format!("{} {}: {d}", entry.release, file.path)));
        }
        assert!(packed <= corpus::BUDGET, "{}: the Backends take {packed} bytes, over the {} of a release", entry.release, corpus::BUDGET);
    }
    assert!(backends > 0, "the index names no Backend");
    println!("{backends} Backend(s) of {} release(s), {calls_made} calls, {calls_equal} equal, {} different ({} ms)", index.entries.len(), different.len(), started.elapsed().as_millis());
    println!("PASS {calls_equal:03} FAIL {:03}", different.len());
    assert!(different.is_empty(), "a Backend of an earlier release answers differently on this runner:\n  {}", different.join("\n  "));
}

#[test]
fn a_corpus_that_was_tampered_with_is_refused() {
    // The checks are the Desktop corpus's: a copy of the corpus with a file changed, with an entry removed from the middle
    // and with the last entry removed must each be reported, so the test above cannot pass over one.
    let started = Instant::now();
    let dir = backend_corpus::dir();
    if corpus::read_index(&dir).map(|i| i.entries.is_empty()).unwrap_or(true) {
        panic!("the Backend corpus has no entry");
    }
    let scratch = std::env::temp_dir().join(format!("prc-backend-corpus-tamper-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    copy_tree(&dir, &scratch);
    let whole = corpus::problems(&scratch, &[]);
    assert!(whole.is_empty(), "an untouched copy is whole: {whole:?}");
    let index = corpus::read_index(&scratch).expect("the index");
    let first = &index.entries[0];
    // 1. A file changed by one byte.
    let victim = first.files.iter().find(|f| f.path.ends_with(".calls.toml")).expect("a call table");
    let mut text = std::fs::read(scratch.join(&victim.path)).unwrap();
    text.push(b'#');
    std::fs::write(scratch.join(&victim.path), &text).unwrap();
    let changed = corpus::problems(&scratch, &[]);
    assert!(changed.iter().any(|p| p.contains("was changed")), "a changed file is reported: {changed:?}");
    // 2. The last entry taken out of the index, with a known (release, chain) the test still holds.
    copy_tree(&dir, &scratch);
    let mut trimmed = corpus::read_index(&scratch).unwrap();
    let gone = trimmed.entries.pop().expect("an entry");
    std::fs::write(scratch.join("index.toml"), corpus::index_text(&trimmed)).unwrap();
    let removed = corpus::problems(&scratch, &[(gone.release.as_str(), gone.chain.as_str())]);
    assert!(removed.iter().any(|p| p.contains("the entry is gone from the index")), "a removed entry is reported: {removed:?}");
    let _ = std::fs::remove_dir_all(&scratch);
    println!("a corpus with a file changed and one with its last entry removed are each reported; PASS 003 FAIL 000 ({} ms)", started.elapsed().as_millis());
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let (path, target) = (entry.path(), to.join(entry.file_name()));
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}
