// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T21 (R5, R23; AC4): the module corpus, kept for ever.
//!
//! `tests/module-corpus/` holds, for every release since 2.0.520, the modules its Build made from
//! the console fixtures. Every one must run on this runner and print, byte for byte, what its
//! release recorded, and answer the same exit code: a module stays runnable on every later runner.
//! The index is append-only, and this test fails when an entry, or one of its files, is gone or
//! changed. Each release adds its entry with
//! `cargo run -p cobolt-compiler --example module_corpus_add` and its line to [`KNOWN`].

#[path = "support/corpus.rs"]
mod corpus;

use std::path::{Path, PathBuf};
use std::time::Instant;

/// Every entry the corpus has had, with its chain: none may leave the index.
const KNOWN: &[(&str, &str)] = &[
    ("2.0.520", "92e2efd9bb234c1ae257f9a0512bf555a0c876820770bbeccb24f69e8a8e5142"),
    ("2.0.593", "869ffae935841d6de53a40ba3aeaea1642257e67f4dfb562755c34556b4498c1"),
    ("2.0.594", "b8f1a15a8a56fb343921cfe1d6cc747f88a1d71f1408d3571efb75a2c87c4ab4"),
    ("2.0.599", "e5080f2415ebeef0b866c0834d61a20b534d1bf91b0490cafa0ecf04ed55f561"),
    ("2.0.601", "d7a1ab2b346ec3ab529a46c26b53e72690373baa9ec0d9930f5d03b12e377ed7"),
];

/// A folder of its own under the system's temporary folder, removed at the end.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("prc-corpus-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Scratch(p)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `src`'s files, copied under `dst`.
fn copy_tree(src: &Path, dst: &Path) {
    for e in std::fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let to = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&to).unwrap();
            copy_tree(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), to).unwrap();
        }
    }
}

#[test]
fn corpus_runs_on_this_runner() {
    let t = Instant::now();
    let dir = corpus::corpus_dir();
    let index = corpus::read_index(&dir).expect("the corpus index");
    let runner = std::fs::read(env!("CARGO_BIN_EXE_prc-runner")).expect("the built prc-runner");
    let scratch = Scratch::new("run");
    let mut rows = Vec::new();
    let mut failures = Vec::new();
    let (mut modules, mut identical, mut compressed, mut raw) = (0, 0, 0u64, 0u64);
    for entry in &index.entries {
        for (module_path, run_path) in corpus::modules(entry) {
            modules += 1;
            let gz = std::fs::read(dir.join(&module_path)).unwrap_or_else(|e| panic!("{module_path}: {e}"));
            let module = corpus::gunzip(&gz).unwrap_or_else(|e| panic!("{module_path}: {e}"));
            compressed += gz.len() as u64;
            raw += module.len() as u64;
            let run = corpus::read_run(&dir.join(&run_path)).unwrap_or_else(|e| panic!("{e}"));
            let name = format!("corpus-{}", module_path.replace(['/', '.'], "-"));
            let got = corpus::run_module(&runner, &module, &run, &scratch.0, &name).unwrap_or_else(|e| panic!("{module_path}: {e}"));
            // A Windows console ends its lines with CR LF; the recorded output ends them with LF.
            let stdout = String::from_utf8_lossy(&got.stdout).replace("\r\n", "\n");
            let same = stdout == run.stdout && got.code == Some(run.exit);
            if same {
                identical += 1;
            } else {
                failures.push(format!(
                    "{module_path}: exit {:?} (recorded {}), output {}:\n{stdout}\n{}",
                    got.code,
                    run.exit,
                    if stdout == run.stdout { "the same" } else { "differs" },
                    got.stderr
                ));
            }
            rows.push(format!(
                "  {} {:<34} {:>7} bytes ({:>6} compressed), {} lines, exit {:?}, {:.0} ms",
                if same { "ok  " } else { "FAIL" },
                module_path,
                module.len(),
                gz.len(),
                stdout.lines().count(),
                got.code,
                got.ms
            ));
        }
    }
    println!("== the module corpus on this runner (DESKTOP-WASM-001 T21: R5, R23; AC4) ==");
    println!(
        "releases {} ({}), modules {modules}, {raw} bytes ({compressed} compressed); each packaged alone, its machine code made by the runner",
        index.entries.len(),
        index.entries.iter().map(|e| e.release.as_str()).collect::<Vec<_>>().join(", ")
    );
    for r in &rows {
        println!("{r}");
    }
    println!("identical output and exit code: {identical} of {modules}, in {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
    assert!(modules > 0, "the corpus has no module");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn corpus_entries_are_never_deleted() {
    let t = Instant::now();
    let mut checks: Vec<(String, bool)> = Vec::new();
    let mut check = |what: &str, ok: bool| checks.push((what.to_owned(), ok));
    let dir = corpus::corpus_dir();

    // The corpus of this checkout is whole, and holds every entry it has had.
    let whole = corpus::problems(&dir, KNOWN);
    check(&format!("this checkout's corpus is whole ({} problems: {whole:?})", whole.len()), whole.is_empty());
    let index = corpus::read_index(&dir).unwrap();
    check(&format!("every KNOWN entry is in the index ({} of {})", KNOWN.len(), KNOWN.len()), index.entries.len() >= KNOWN.len());

    // Planted: a module file deleted from a copy.
    let copy = Scratch::new("planted");
    copy_tree(&dir, &copy.0);
    let first = corpus::modules(&index.entries[0]).remove(0).0;
    std::fs::remove_file(copy.0.join(&first)).unwrap();
    let p = corpus::problems(&copy.0, KNOWN);
    check(&format!("a deleted module is named ({first})"), p.iter().any(|m| m.contains(&first) && m.contains("missing")));

    // Planted: a module changed by one byte.
    copy_tree(&dir, &copy.0);
    let path = copy.0.join(&first);
    let mut bytes = std::fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    std::fs::write(&path, bytes).unwrap();
    let p = corpus::problems(&copy.0, KNOWN);
    check("a module changed by one byte is named", p.iter().any(|m| m.contains(&first) && m.contains("changed")));

    // Planted: the last entry taken out of the index (the files left): KNOWN names it.
    copy_tree(&dir, &copy.0);
    let mut cut = index.clone();
    let gone = cut.entries.pop().unwrap().release;
    std::fs::write(copy.0.join("index.toml"), corpus::index_text(&cut)).unwrap();
    let p = corpus::problems(&copy.0, KNOWN);
    check(&format!("the last entry taken out ({gone}) is named"), p.iter().any(|m| m.contains(&gone) && m.contains("gone")));

    // Planted: an entry taken out of the middle of a three-entry corpus breaks the next one's chain.
    let three = Scratch::new("three");
    let mut idx = corpus::Index::default();
    let mut prev = String::new();
    for (i, release) in ["9.0.1", "9.0.2", "9.0.3"].iter().enumerate() {
        let rel = format!("releases/{release}/m.wasm.gz");
        std::fs::create_dir_all(three.0.join(format!("releases/{release}"))).unwrap();
        let body = corpus::gzip(format!("module {i}").as_bytes());
        std::fs::write(three.0.join(&rel), &body).unwrap();
        let run = format!("releases/{release}/m.toml");
        std::fs::write(three.0.join(&run), b"x").unwrap();
        let files = vec![
            corpus::FileRef { path: rel, sha256: corpus::sha256_hex(&body) },
            corpus::FileRef { path: run, sha256: corpus::sha256_hex(b"x") },
        ];
        let chain = corpus::chain(&prev, release, &files);
        prev = chain.clone();
        idx.entries.push(corpus::Entry { release: (*release).into(), added: "2026-11-16".into(), chain, files });
    }
    std::fs::write(three.0.join("index.toml"), corpus::index_text(&idx)).unwrap();
    check("a synthetic three-entry corpus is whole", corpus::problems(&three.0, &[]).is_empty());
    idx.entries.remove(1);
    std::fs::write(three.0.join("index.toml"), corpus::index_text(&idx)).unwrap();
    let p = corpus::problems(&three.0, &[]);
    check("its middle entry taken out breaks the chain of the next", p.iter().any(|m| m.starts_with("9.0.3") && m.contains("chain")));

    let failures: Vec<&(String, bool)> = checks.iter().filter(|(_, ok)| !ok).collect();
    println!("== the module corpus is kept (DESKTOP-WASM-001 T21: R5, R23; AC4) ==");
    println!("entries {}, KNOWN {}; planted cases 4 (a deleted module, a changed byte, the last entry, a middle entry)", index.entries.len(), KNOWN.len());
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "ok  " } else { "FAIL" });
    }
    println!("{} of {} checks passed in {:.1} ms", checks.len() - failures.len(), checks.len(), t.elapsed().as_secs_f64() * 1000.0);
    assert!(failures.is_empty(), "failed: {failures:?}");
}
