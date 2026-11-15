// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! R16(b), AC8: the programs of `tests/cobol/` that fall inside the browser
//! subset print the same output on the module (both memory variants) and on
//! the interpreter. A program outside the subset is listed with the construct
//! that keeps it out. A difference is never skipped: every program that
//! differs must be in [`KNOWN`], tied to its open row of `specs/WASM-PARITY.md`,
//! and a known difference that has gone away fails too, so the list stays true.

mod common;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_wasm::{compile_program, Options};
use std::path::{Path, PathBuf};

/// Programs whose output differs, with the `WASM-PARITY.md` row that says why.
const KNOWN: &[(&str, u32)] = &[];

fn programs(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            programs(&p, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("cbl")) {
            out.push(p);
        }
    }
}

/// A line that prints a measurement of the clock (an elapsed time, a rate).
/// It says so in words (`elapsed`, `per second`, `/second`, `/sec`) or gives a
/// number of milliseconds (`460 ms`), as `conditionals/test-arith-relation-
/// after-and.cbl` does (2.0.504). A count with no unit (`TIMED HITS : 1000`)
/// is not one.
fn is_clock_line(l: &str) -> bool {
    let low = l.to_ascii_lowercase();
    if low.contains("elapsed") || low.contains("per second") || low.contains("/sec") {
        return true;
    }
    let words: Vec<&str> = low.split_whitespace().collect();
    words.windows(2).any(|w| {
        w[1] == "ms"
            && w[0].chars().any(|c| c.is_ascii_digit())
            && w[0].chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '+' | '-'))
    })
}

/// Two runs of one engine already differ in a clock line, so only its label
/// (the text before its first `:`) is compared. Every other line is compared whole.
fn clock_free(l: &str) -> String {
    if is_clock_line(l) {
        l.split(':').next().unwrap_or(l).to_string()
    } else {
        l.to_string()
    }
}

/// Clock lines in the interpreter's output of the programs compared, and those programs.
static CLOCK_LINES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static CLOCK_PROGRAMS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

enum Verdict {
    /// Lines compared, and how many of them were clock lines compared by label.
    Identical(usize, usize),
    Differs(String),
    Excluded(String),
}

/// Modules scanned for `memory.grow` (none has one).
static SCANNED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn judge(path: &Path) -> Verdict {
    let raw = std::fs::read(path).unwrap();
    let src = String::from_utf8_lossy(&raw).into_owned();
    let src = match cobolt_lexer::preprocess_program(&src, path, SourceFormat::Free) {
        Some(x) if !x.errors.is_empty() => {
            return Verdict::Excluded(format!("COPY: {}", x.errors[0]))
        }
        Some(x) => x.text,
        None => src,
    };
    let parsed = cobolt_parser::parse(tokenize(&src, SourceFormat::Free));
    if let Some(d) = parsed.diagnostics.iter().find(|d| d.is_error()) {
        return Verdict::Excluded(format!(
            "does not parse: line {}: {}",
            d.span.line, d.message
        ));
    }
    let Some(prog) = parsed.program else {
        return Verdict::Excluded("no program".into());
    };
    if let Err(e) = compile_program(&prog, &Options::default()) {
        let why = format!("line {}: {}", e.line, e.message);
        return Verdict::Excluded(why);
    }
    let interpreted = common::run_interpreter(&src);
    let clocks = interpreted.iter().filter(|l| is_clock_line(l)).count();
    if clocks > 0 {
        CLOCK_LINES.fetch_add(clocks, std::sync::atomic::Ordering::Relaxed);
        CLOCK_PROGRAMS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    let want: Vec<String> = interpreted.iter().map(|l| clock_free(l)).collect();
    for multi in [true, false] {
        let c = compile_program(
            &prog,
            &Options {
                multi_memory: multi,
                ..Options::default()
            },
        )
        .unwrap();
        // WASM-017 R7, AC5: fixed memory — no module of the corpus grows it.
        let scan = cobolt_wasm_abi::scan::scan_module(&c.wasm).unwrap();
        assert!(!scan.has_memory_grow, "{}: multi={multi}: memory.grow", path.display());
        SCANNED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let o = cobolt_wasm::host::run_program(&c.wasm).unwrap();
        if o.trapped.is_some() || !o.errors.is_empty() {
            return Verdict::Differs(format!(
                "multi={multi}: trapped {:?} errors {:?}",
                o.trapped, o.errors
            ));
        }
        let got: Vec<String> = common::wasm_lines(&o)
            .iter()
            .map(|l| clock_free(l))
            .collect();
        if got != want {
            let at = got
                .iter()
                .zip(want.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(got.len().min(want.len()));
            return Verdict::Differs(format!(
                "multi={multi}: line {}: module {:?}, interpreter {:?} ({} against {} lines)",
                at + 1,
                got.get(at),
                want.get(at),
                got.len(),
                want.len()
            ));
        }
    }
    Verdict::Identical(want.len(), clocks)
}

/// The comparison is planted with pairs of lines: a clock line agrees with
/// itself under any reading of the clock, and nothing else is let through.
#[test]
fn only_a_clock_line_is_compared_by_its_label() {
    let started = std::time::Instant::now();
    let same = |a: &str, b: &str| clock_free(a) == clock_free(b);
    // Pairs a run of the module and of the interpreter may print: equal.
    let agree = [
        ("CHECK PHASE     :    12 ms", "CHECK PHASE     :    31 ms"),
        ("TIMED PHASE     :  1000 evaluations in   460 ms", "TIMED PHASE     :  1000 evaluations in   970 ms"),
        ("THROUGHPUT      :  2173 evaluations/sec", "THROUGHPUT      : under the clock's 10 ms"),
        ("THROUGHPUT      : 40 calls/sec", "THROUGHPUT      : 51 calls/sec"),
        ("Elapsed seconds     : 3", "Elapsed seconds     : 4"),
        ("  elapsed (ms)    : 120", "  elapsed (ms)    : 95"),
        ("RATE : 9 records per second", "RATE : 12 records per second"),
    ];
    // Pairs that differ in what the program computed or in a clock line's label: unequal.
    let differ = [
        ("TIMED HITS      : 1000", "TIMED HITS      : 999"),
        ("TESTS PASSED    : 14", "TESTS PASSED    : 13"),
        ("OVERALL RESULT  : PASS", "OVERALL RESULT  : FAIL"),
        ("CHECK PHASE     :    12 ms", "CHECK PHASES    :    12 ms"),
        ("TIMED PHASE     : 1000 evaluations in 460 ms", "TIMING PHASE    : 1000 evaluations in 460 ms"),
        ("P010 PERFORM 4 TIMES -> 4 iterations", "P010 PERFORM 4 TIMES -> 5 iterations"),
        ("TERMS : 12 items", "TERMS : 13 items"),
        ("UNIT : 12 msec", "UNIT : 13 msec"),
    ];
    let agreed = agree.iter().filter(|(a, b)| same(a, b)).count();
    let caught = differ.iter().filter(|(a, b)| !same(a, b)).count();
    eprintln!(
        "clock lines: {agreed} of {} clock pairs agree, {caught} of {} planted differences caught, {:.2} ms",
        agree.len(),
        differ.len(),
        started.elapsed().as_secs_f64() * 1000.0
    );
    assert_eq!(agreed, agree.len(), "a clock line was compared beyond its label");
    assert_eq!(caught, differ.len(), "a real difference was let through");
}

#[test]
fn the_corpus_inside_the_subset_matches_the_interpreter() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol");
    let mut files = Vec::new();
    programs(&root, &mut files);
    let (mut identical, mut excluded, mut lines) = (0, 0, 0);
    let mut differs: Vec<(String, String)> = Vec::new();
    for f in &files {
        let rel = f
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        match judge(f) {
            Verdict::Identical(n, 0) => {
                identical += 1;
                lines += n;
                eprintln!("  identical  {rel} ({n} lines)");
            }
            Verdict::Identical(n, clocks) => {
                identical += 1;
                lines += n;
                eprintln!("  identical  {rel} ({n} lines, {clocks} of them clock lines compared by label)");
            }
            Verdict::Excluded(why) => {
                excluded += 1;
                eprintln!("  excluded   {rel}: {why}");
            }
            Verdict::Differs(why) => {
                eprintln!("  DIFFERS    {rel}: {why}");
                differs.push((rel, why));
            }
        }
    }
    eprintln!(
        "corpus: {} programs, {} compiled, {identical} identical ({lines} lines), {} differ, {excluded} excluded with the construct named",
        files.len(),
        identical + differs.len(),
        differs.len()
    );
    eprintln!("corpus: {} modules scanned, 0 with memory.grow (WASM-017 R7)", SCANNED.load(std::sync::atomic::Ordering::Relaxed));
    eprintln!(
        "corpus: {} clock lines in {} compiled programs compared by their label only",
        CLOCK_LINES.load(std::sync::atomic::Ordering::Relaxed),
        CLOCK_PROGRAMS.load(std::sync::atomic::Ordering::Relaxed)
    );
    let unknown: Vec<_> = differs
        .iter()
        .filter(|(r, _)| !KNOWN.iter().any(|(k, _)| k == r))
        .collect();
    assert!(
        unknown.is_empty(),
        "differences with no WASM-PARITY.md row: {unknown:#?}"
    );
    let gone: Vec<_> = KNOWN
        .iter()
        .filter(|(k, _)| !differs.iter().any(|(r, _)| r == k))
        .collect();
    assert!(
        gone.is_empty(),
        "known differences that no longer differ (close their rows): {gone:?}"
    );
}
