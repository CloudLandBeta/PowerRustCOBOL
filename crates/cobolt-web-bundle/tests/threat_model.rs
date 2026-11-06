// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The threat model is a file and this is its test (WASM-013 R1, R16; T23; AC1).
//!
//! `specs/WASM-013-security-architecture/threat-model.md` holds the assets, the actors, the seven boundaries, one table
//! of threats (`id | boundary | actor | threat | control | test`), the walk of every boundary through STRIDE, the personal
//! data, the secret-scan pattern names and the residual risks. This test reads it and fails when:
//!
//! * a boundary has no row, or a row names a boundary or an actor the model does not define;
//! * a row has no control, or no STRIDE letter in its threat;
//! * a named test (`crate::function`) does not exist as `fn function` in that crate's sources, or a row names neither a test
//!   nor `owed: WASM-NNN`;
//! * a boundary has an empty STRIDE cell, or a cell cites a row that is not that boundary's;
//! * the pattern list differs from `secrets::PATTERNS`;
//! * the series is marked a release candidate and `release-review.md` is missing, or has no reviewer other than its author.
//!
//! It prints the rows, the boundaries and the **owed count**, which must fall as the specs that owe rows land theirs.
//! The checks are functions over text, so each is also run on a damaged copy of the model to show it can fail.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use cobolt_web_bundle::secrets::{Severity, PATTERNS};

const BOUNDARIES: [&str; 7] = ["B1", "B2", "B3", "B4", "B5", "B6", "B7"];
const ACTORS: [&str; 7] = ["A1", "A2", "A3", "A4", "A5", "A6", "A7"];
const STRIDE: [&str; 6] = ["**S**", "**T**", "**R**", "**I**", "**D**", "**E**"];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the repository root")
}

fn model_path() -> PathBuf {
    repo().join("specs/WASM-013-security-architecture/threat-model.md")
}

fn model() -> String {
    std::fs::read_to_string(model_path()).unwrap_or_else(|e| panic!("{}: {e}", model_path().display()))
}

/// The cells of a table row `| a | b | c |`, trimmed.
fn cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(|c| c.trim().to_owned()).collect()
}

/// The lines of a numbered `## N.` section, without its heading.
fn section<'a>(text: &'a str, n: u32) -> Vec<&'a str> {
    let head = format!("## {n}.");
    let mut inside = false;
    let mut out = Vec::new();
    for line in text.lines() {
        if line.starts_with("## ") {
            inside = line.starts_with(&head);
            continue;
        }
        if inside {
            out.push(line);
        }
    }
    out
}

struct Row {
    id: String,
    boundary: String,
    actors: Vec<String>,
    threat: String,
    control: String,
    test: String,
}

fn rows_of(text: &str) -> Vec<Row> {
    section(text, 4)
        .into_iter()
        .filter(|l| l.starts_with("| TM-"))
        .map(|l| {
            let c = cells(l);
            assert_eq!(c.len(), 6, "a row has six cells (id, boundary, actor, threat, control, test): {l}");
            Row { id: c[0].clone(), boundary: c[1].clone(), actors: c[2].split(',').map(|a| a.trim().to_owned()).collect(), threat: c[3].clone(), control: c[4].clone(), test: c[5].clone() }
        })
        .collect()
}

/// Every `fn name` of every `.rs` file under `crates/<krate>`.
fn functions_of(krate: &str, cache: &mut BTreeMap<String, BTreeSet<String>>) -> BTreeSet<String> {
    if let Some(found) = cache.get(krate) {
        return found.clone();
    }
    fn walk(dir: &Path, out: &mut BTreeSet<String>) {
        let Ok(read) = std::fs::read_dir(dir) else { return };
        for e in read.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                if name != "target" && !name.starts_with('.') {
                    walk(&p, out);
                }
            } else if name.ends_with(".rs") {
                let Ok(text) = std::fs::read_to_string(&p) else { continue };
                let mut rest = text.as_str();
                while let Some(at) = rest.find("fn ") {
                    let after = &rest[at + 3..];
                    let ident: String = after.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                    let boundary_ok = at == 0 || !rest.as_bytes()[at - 1].is_ascii_alphanumeric() && rest.as_bytes()[at - 1] != b'_';
                    if boundary_ok && !ident.is_empty() {
                        out.insert(ident);
                    }
                    rest = &rest[at + 3..];
                }
            }
        }
    }
    let mut found = BTreeSet::new();
    walk(&repo().join("crates").join(krate), &mut found);
    cache.insert(krate.to_owned(), found.clone());
    found
}

/// The `` `crate::function` `` references of a test cell.
fn named_tests(cell: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut parts = cell.split('`');
    parts.next();
    while let (Some(inside), _) = (parts.next(), parts.next()) {
        if let Some((k, f)) = inside.split_once("::") {
            out.push((k.to_owned(), f.to_owned()));
        }
    }
    out
}

fn is_owed(cell: &str) -> bool {
    let Some(rest) = cell.strip_prefix("owed: WASM-") else { return false };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.len() == 3 && rest[3..].trim_start().starts_with('—') && rest.len() > 12
}

/// What is wrong with the model's rows, boundaries and matrix; empty when it holds. Also counts what it checked.
fn problems(text: &str, tally: &mut Tally) -> Vec<String> {
    let mut bad = Vec::new();
    let mut cache = BTreeMap::new();

    let defined: Vec<String> = section(text, 3).into_iter().filter(|l| l.starts_with("| B")).map(|l| cells(l)[0].clone()).collect();
    if defined != BOUNDARIES {
        bad.push(format!("§3 must define exactly {BOUNDARIES:?}, in order; it defines {defined:?}"));
    }
    let actors: Vec<String> = section(text, 2).into_iter().filter(|l| l.starts_with("| A")).map(|l| cells(l)[0].clone()).collect();
    if actors != ACTORS {
        bad.push(format!("§2 must define exactly {ACTORS:?}, in order; it defines {actors:?}"));
    }

    let rows = rows_of(text);
    tally.rows = rows.len();
    let mut ids = BTreeSet::new();
    for r in &rows {
        if !ids.insert(r.id.clone()) {
            bad.push(format!("{}: the id is used twice", r.id));
        }
        if !BOUNDARIES.contains(&r.boundary.as_str()) {
            bad.push(format!("{}: the boundary {:?} is not one of the seven", r.id, r.boundary));
        }
        if r.actors.iter().any(|a| !ACTORS.contains(&a.as_str())) {
            bad.push(format!("{}: an actor of {:?} is not one the model defines", r.id, r.actors));
        }
        if !STRIDE.iter().any(|s| r.threat.contains(s)) {
            bad.push(format!("{}: the threat carries no STRIDE letter in bold (S, T, R, I, D or E)", r.id));
        }
        if r.control.chars().count() < 30 {
            bad.push(format!("{}: a row needs a control (rule and code path), found {:?}", r.id, r.control));
        }
        let tests = named_tests(&r.test);
        if is_owed(&r.test) {
            if !tests.is_empty() {
                bad.push(format!("{}: an owed row names no test (a row is either tested or owed)", r.id));
            }
            tally.owed += 1;
        } else if tests.is_empty() {
            bad.push(format!("{}: the test cell names neither a test (`crate::function`) nor `owed: WASM-NNN — what is owed`: {:?}", r.id, r.test));
        }
        for (krate, name) in tests {
            tally.tests += 1;
            if !functions_of(&krate, &mut cache).contains(&name) {
                bad.push(format!("{}: `{krate}::{name}` does not exist (no `fn {name}` under crates/{krate})", r.id));
            }
        }
    }
    for b in BOUNDARIES {
        if !rows.iter().any(|r| r.boundary == b) {
            bad.push(format!("{b}: a boundary with no row"));
        }
    }

    // §5: each boundary walked through the six letters.
    let by_id: BTreeMap<&str, &Row> = rows.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut walked = 0;
    for line in section(text, 5).into_iter().filter(|l| l.starts_with("| B")) {
        let c = cells(line);
        let boundary = c[0].split_whitespace().next().unwrap_or("").to_owned();
        if c.len() != 7 {
            bad.push(format!("§5 {boundary}: six letters expected, found {} cells", c.len().saturating_sub(1)));
            continue;
        }
        for (i, letter) in ["S", "T", "R", "I", "D", "E"].iter().enumerate() {
            let cell = &c[i + 1];
            let refs = refs_in(cell);
            if let Some(reason) = cell.strip_prefix('—') {
                if reason.trim().chars().count() < 10 {
                    bad.push(format!("§5 {boundary} {letter}: a dash needs its reason"));
                }
            } else if refs.is_empty() {
                bad.push(format!("§5 {boundary} {letter}: the cell is empty (name the rows, or a dash and why none applies)"));
            } else {
                for id in refs {
                    match by_id.get(id.as_str()) {
                        None => bad.push(format!("§5 {boundary} {letter}: {id} is not a row")),
                        Some(r) if r.boundary != boundary => bad.push(format!("§5 {boundary} {letter}: {id} belongs to {}", r.boundary)),
                        Some(_) => {}
                    }
                }
            }
            walked += 1;
        }
    }
    if walked != BOUNDARIES.len() * 6 {
        bad.push(format!("§5 walks {walked} boundary/letter cells, {} expected", BOUNDARIES.len() * 6));
    }
    tally.cells = walked;

    // §6: the subject is listed as personal data.
    if !text.contains("**`subject` is personal data**") {
        bad.push("§6 must say that `subject` is personal data".into());
    }

    // §7: the pattern names are the scan's.
    let listed: Vec<(String, String)> = section(text, 7).into_iter().filter(|l| l.starts_with("| ")).map(cells).filter(|c| c.len() == 2 && c[0] != "pattern" && !c[0].starts_with('-')).map(|c| (c[0].clone(), c[1].clone())).collect();
    let real: Vec<(String, String)> = PATTERNS.iter().map(|(n, s)| ((*n).to_owned(), if *s == Severity::Error { "error" } else { "warning" }.to_owned())).collect();
    tally.patterns = listed.len();
    if listed != real {
        bad.push(format!("§7 lists {} patterns, `secrets::PATTERNS` holds {}; names, order and severity must agree (listed {:?}, real {:?})", listed.len(), real.len(), listed.iter().map(|p| &p.0).collect::<Vec<_>>(), real.iter().map(|p| &p.0).collect::<Vec<_>>()));
    }
    bad
}

/// `TM-01, 02, 03` or `TM-21, 22` in a cell: the row ids it cites.
fn refs_in(cell: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = cell;
    while let Some(at) = rest.find("TM-") {
        let mut tail = &rest[at + 3..];
        loop {
            let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                break;
            }
            out.push(format!("TM-{digits}"));
            tail = &tail[digits.len()..];
            match tail.strip_prefix(", ").or_else(|| tail.strip_prefix(',')) {
                Some(more) if more.starts_with(|c: char| c.is_ascii_digit()) => tail = more,
                _ => break,
            }
        }
        rest = tail;
    }
    out
}

#[derive(Default, Debug)]
struct Tally {
    rows: usize,
    owed: usize,
    tests: usize,
    cells: usize,
    patterns: usize,
}

/// What is wrong with a release review record; empty when it holds.
fn review_problems(text: &str) -> Vec<String> {
    let mut kv: BTreeMap<String, String> = BTreeMap::new();
    // The record opens with its `key = value` lines; the first blank line after them ends the header.
    for line in text.lines() {
        if line.trim().is_empty() {
            if kv.is_empty() {
                continue;
            }
            break;
        }
        if let Some((k, v)) = line.split_once('=') {
            kv.insert(k.trim().to_owned(), v.trim().to_owned());
        }
    }
    let mut bad = Vec::new();
    match kv.get("kind").map(String::as_str) {
        Some("review") | Some("penetration-test") => {}
        other => bad.push(format!("`kind` must be review or penetration-test, found {other:?}")),
    }
    for key in ["reviewer", "author", "scope", "date"] {
        if kv.get(key).map_or(true, |v| v.is_empty()) {
            bad.push(format!("`{key}` is missing"));
        }
    }
    if let (Some(r), Some(a)) = (kv.get("reviewer"), kv.get("author")) {
        if r.eq_ignore_ascii_case(a) {
            bad.push("the reviewer must be someone other than the author".into());
        }
    }
    bad
}

/// Whether `WASM-PROGRESS.md` marks the line as a release candidate.
fn marks_release_candidate(progress: &str) -> bool {
    progress.lines().any(|l| l.trim_start().strip_prefix("RELEASE CANDIDATE:").is_some_and(|rest| rest.trim_start().starts_with("reached")))
}

#[test]
fn every_boundary_has_a_row_every_row_a_control_and_every_named_test_exists() {
    let text = model();
    let mut tally = Tally::default();
    let bad = problems(&text, &mut tally);
    assert!(bad.is_empty(), "the threat model does not hold:\n  {}", bad.join("\n  "));

    let rows = rows_of(&text);
    let per_boundary: Vec<String> = BOUNDARIES.iter().map(|b| format!("{b}: {}", rows.iter().filter(|r| r.boundary == *b).count())).collect();
    let owed: Vec<String> = rows.iter().filter(|r| is_owed(&r.test)).map(|r| format!("{} {}", r.id, r.test.trim_start_matches("owed: "))).collect();
    println!(
        "\n  threat model: {} rows over {} boundaries ({}), {} actors, {} named tests found, {} STRIDE cells walked, {} secret-scan patterns equal to `secrets::PATTERNS`; **owed rows: {}**",
        tally.rows,
        BOUNDARIES.len(),
        per_boundary.join(", "),
        ACTORS.len(),
        tally.tests,
        tally.cells,
        tally.patterns,
        tally.owed
    );
    for o in &owed {
        println!("    owed {o}");
    }
    assert_eq!(tally.owed, owed.len());
}

/// A check that cannot fail proves nothing: damage the model four ways and each must be reported.
#[test]
fn the_check_fails_when_the_model_is_damaged() {
    let text = model();
    let first_test = rows_of(&text).iter().find_map(|r| named_tests(&r.test).into_iter().next()).expect("a tested row");
    let (krate, name) = first_test;

    let missing_test = text.replacen(&format!("`{krate}::{name}`"), &format!("`{krate}::{name}_that_was_never_written`"), 1);
    let bad = problems(&missing_test, &mut Tally::default());
    assert!(bad.iter().any(|b| b.contains("does not exist")), "a test name that does not exist is reported: {bad:?}");

    let no_boundary_rows = text.lines().filter(|l| !l.contains("| B3 | A")).collect::<Vec<_>>().join("\n");
    let bad = problems(&no_boundary_rows, &mut Tally::default());
    assert!(bad.iter().any(|b| b.contains("a boundary with no row")), "a boundary with no row is reported: {bad:?}");

    let no_control = text.replacen("`cobolt-server/src/serve.rs` `origin_allowed`", "", 1).replacen("WebSocket `Origin` is checked against the configured list at the upgrade and counted when refused (R6; WASM-003 R14): ", "", 1);
    let bad = problems(&no_control, &mut Tally::default());
    assert!(bad.iter().any(|b| b.contains("needs a control")), "a row without a control is reported: {bad:?}");

    let bad_pattern = text.replacen("| Slack token | error |", "| Slack tokens | error |", 1);
    let bad = problems(&bad_pattern, &mut Tally::default());
    assert!(bad.iter().any(|b| b.contains("`secrets::PATTERNS`")), "a pattern list that differs from the scan's is reported: {bad:?}");

    let owed_badly = text.replacen("owed: WASM-016 —", "owed later", 1);
    let bad = problems(&owed_badly, &mut Tally::default());
    assert!(bad.iter().any(|b| b.contains("names neither a test")), "a row that is neither tested nor owed in the form asked for is reported: {bad:?}");
    println!("\n  threat model check: 5 damaged copies, each reported");
}

#[test]
fn the_release_review_is_required_once_the_line_is_a_release_candidate_and_judged_when_it_is() {
    // The rule on records, on text.
    let good = "kind = review\nreviewer = Ana\nauthor = Emerson\nscope = the whole line\ndate = 2027-01-15\n\nFindings: none.\n";
    assert!(review_problems(good).is_empty(), "{:?}", review_problems(good));
    assert!(review_problems(&good.replace("kind = review", "kind = penetration-test")).is_empty());
    assert!(!review_problems(&good.replace("kind = review", "kind = audit")).is_empty(), "an unknown kind is refused");
    assert!(!review_problems(&good.replace("reviewer = Ana", "reviewer = emerson")).is_empty(), "the author cannot review their own work");
    assert!(!review_problems(&good.replace("reviewer = Ana\n", "")).is_empty(), "a reviewer is required");
    assert!(!review_problems("").is_empty());

    // The marker, on text.
    assert!(marks_release_candidate("# ledger\n\nRELEASE CANDIDATE: reached on 2027-01-10\n"));
    assert!(!marks_release_candidate("# ledger\n\n| WASM-028 | release-candidate steps |\nRELEASE CANDIDATE: not yet\n"));

    // The rule on the repository.
    let progress = std::fs::read_to_string(repo().join("specs/WASM-PROGRESS.md")).expect("WASM-PROGRESS.md");
    if !marks_release_candidate(&progress) {
        println!("\n  release review: not a release candidate (WASM-PROGRESS.md has no `RELEASE CANDIDATE: reached` line); nothing to judge");
        return;
    }
    let path = repo().join("specs/WASM-013-security-architecture/release-review.md");
    let record = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("the line is marked a release candidate and {} does not exist (R16)", path.display()));
    let bad = review_problems(&record);
    assert!(bad.is_empty(), "release-review.md does not hold (R16): {bad:?}");
    println!("\n  release review: a release candidate, and release-review.md names a reviewer other than its author");
}
