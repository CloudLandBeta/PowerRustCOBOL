// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! NIST CCVS85 conformance probe — **read-only measurement, not a fix**.
//!
//! Splits `NIST/newcob.val,cbl` (the official NIST COBOL-85 validation suite,
//! CCVS85 4.0) into its individual programs and runs each one through the
//! RustCOBOL front end (lexer -> parser -> semantic analyser), then reports
//! which programs fail and which distinct diagnostics dominate.
//!
//! Passes, so the column-rule defect stays separable from the language gaps:
//!
//! * `strict`— **the real path**: the untouched member compiled with
//!             `SourceFormat::FixedStrict` (`rcrun --source-format=fixed`). The
//!             compiler applies the column and continuation rules itself.
//! * `raw`   — the untouched member under the relaxed `Fixed` reading; the
//!             "before" number.
//! * `col72` — the harness truncates at column 72 itself, then uses `Fixed`.
//! * `nist` / `nistdel` — `col72` plus the harness's own normalisation of the
//!             CCVS selector letters in column 7 (activate / drop).
//!
//! The last three are kept only so the pre-implementation measurements stay
//! reproducible; `strict` is what the product actually does.
//!
//! Usage:
//!   cargo run -p cobolt-semantic --example nist_conformance -- [pass] [filter]
//!     pass   = strict | raw | col72 | nist | nistdel
//!              | dump <NAME> | bisect <NAME> | extract <NAME>
//!              | probe <path.cbl> | features
//!     filter = module prefix, e.g. NC, SQ, IX   (default: all)

use std::collections::BTreeMap;
use std::path::PathBuf;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_semantic::flagging::FlagClass;

/// One extracted CCVS85 member.
struct Member {
    kind: String,
    name: String,
    text: String,
}

/// Split the CCVS85 distribution on its `*HEADER,<kind>,<name>` /
/// `*END-OF,<name>` control cards.
fn split_members(source: &str) -> Vec<Member> {
    let mut out: Vec<Member> = Vec::new();
    let mut current: Option<Member> = None;

    for line in source.lines() {
        if let Some(rest) = line.strip_prefix("*HEADER,") {
            if let Some(prev) = current.take() {
                out.push(prev);
            }
            // `*HEADER,COBOL,ST101A` — a test program.
            // `*HEADER,COBOL,ST101A,SUBPRG,ST103A` — a called subprogram whose
            //   own PROGRAM-ID is the 4th field; the 2nd names the driving test.
            // Trailing text past the name (some cards carry a sequence stamp) is
            // not part of the name.
            let mut parts = rest.split(',');
            let kind = parts.next().unwrap_or("").trim().to_string();
            let test = parts.next().unwrap_or("").trim().to_string();
            let sub = parts.next().unwrap_or("").trim().to_string();
            let subname = parts.next().unwrap_or("").trim().to_string();
            let raw = if sub == "SUBPRG" && !subname.is_empty() {
                subname
            } else {
                test
            };
            let name = raw
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            current = Some(Member {
                kind,
                name,
                text: String::new(),
            });
            continue;
        }
        if line.starts_with("*END-OF") {
            if let Some(prev) = current.take() {
                out.push(prev);
            }
            continue;
        }
        if let Some(m) = current.as_mut() {
            m.text.push_str(line);
            m.text.push('\n');
        }
    }
    if let Some(prev) = current.take() {
        out.push(prev);
    }
    out
}

/// Apply the classic fixed-format rule NIST depends on: columns 73-80 are the
/// identification area and are discarded before compilation.
fn truncate_at_col72(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let cut = line
            .char_indices()
            .nth(72)
            .map(|(b, _)| b)
            .unwrap_or(line.len());
        out.push_str(&line[..cut]);
        out.push('\n');
    }
    out
}

/// Turn a raw CCVS85 member into the text and source format a pass hands to the
/// front end.
///
/// * `strict` — **the real thing**: the untouched member, compiled with
///   `SourceFormat::FixedStrict`. The compiler applies the column rules and the
///   continuation rules itself, so the harness prepares nothing.
/// * `raw` — the untouched member under the relaxed `Fixed` reading; the
///   "before" number.
/// * `col72` / `nist` / `nistdel` — the harness's own preparation, kept so the
///   pre-implementation measurements stay reproducible.
/// Write the suite's COPY library to a directory and return it.
///
/// CCVS85 ships its copybooks inside the same distribution, as `*HEADER,CLBRY`
/// members — 51 of them. The Source Text Manipulation module is *about* `COPY`
/// and `REPLACE`, so without the library those programs cannot be measured at
/// all: `COPY K1PRA.` reaches the parser as an ordinary word and every SM
/// program stops there.
///
/// This is a harness gap, not a compiler one — `rcrun` has expanded copybooks
/// all along (`cobolt_lexer::expand_copybooks`); the harness simply tokenized
/// without ever running the preprocessor. The directory is built once per run.
fn copy_library_dir(members: &[Member]) -> Option<PathBuf> {
    let dir = std::env::temp_dir().join("nist-copy-library");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).ok()?;
    let mut written = 0usize;
    for m in members.iter().filter(|m| m.kind == "CLBRY") {
        if m.name.is_empty() {
            continue;
        }
        // The library members are card images too: strip the identification
        // area so a copied line does not carry its program stamp into the
        // including program.
        let body = truncate_at_col72(&m.text);
        if std::fs::write(dir.join(&m.name), body).is_ok() {
            written += 1;
        }
    }
    if written == 0 {
        None
    } else {
        Some(dir)
    }
}

fn prepare(pass: &str, text: &str) -> (String, SourceFormat) {
    match pass {
        "strict" => (text.to_string(), SourceFormat::FixedStrict),
        "raw" => (text.to_string(), SourceFormat::Fixed),
        "nist" => (
            normalize_indicators(&truncate_at_col72(text), true),
            SourceFormat::Fixed,
        ),
        "nistdel" => (
            normalize_indicators(&truncate_at_col72(text), false),
            SourceFormat::Fixed,
        ),
        _ => (truncate_at_col72(text), SourceFormat::Fixed),
    }
}

/// COBOL-85 reserves the indicator area (column 7) for space, `*`, `/`, `-`
/// and `D`. CCVS85 additionally puts a *selector* letter there to mark optional
/// source lines the installer chooses to activate or drop. Neither activation
/// nor removal is a compiler concern, so the harness normalises them first.
fn is_standard_indicator(c: char) -> bool {
    matches!(c, ' ' | '*' | '/' | '-' | 'D')
}

/// `activate` = blank the selector so the line becomes ordinary source;
/// otherwise drop the line entirely.
fn normalize_indicators(text: &str, activate: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let ind = line.chars().nth(6);
        match ind {
            Some(c) if !is_standard_indicator(c) => {
                if activate {
                    let a = char_byte(line, 6);
                    let b = char_byte(line, 7);
                    out.push_str(&line[..a]);
                    out.push(' ');
                    out.push_str(&line[b..]);
                    out.push('\n');
                } else {
                    out.push('\n');
                }
            }
            _ => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}

fn char_byte(s: &str, col: usize) -> usize {
    s.char_indices().nth(col).map(|(b, _)| b).unwrap_or(s.len())
}

/// Collapse a diagnostic message into a bucket key, so `expected X, found FOO`
/// and `expected X, found BAR` land in one bucket.
fn bucket(msg: &str) -> String {
    let mut s = msg.to_string();
    if let Some(idx) = s.find(", found") {
        s.truncate(idx);
        s.push_str(", found …");
    }
    if let Some(idx) = s.find(": `") {
        s.truncate(idx);
        s.push_str(": `…`");
    }
    if let Some(idx) = s.find(" '") {
        // `unknown data item 'FOO'` -> `unknown data item '…'`
        let head = &s[..idx];
        if head.len() > 8 {
            s = format!("{head} '…'");
        }
    }
    s
}

/// Modules RustCOBOL does not implement, and is not planning to.
///
/// `CM` is the Communication module (teleprocessing message queues), `RW` the
/// Report Writer, `OB*` the obsolete-feature variants that test compiler
/// *flagging* rather than execution, and `EXEC85` is NIST's own COBOL driver
/// program, replaced here by a Rust harness. See
/// `specs/nist/NIST-spec-out-of-scope-modules.md`.
fn is_out_of_scope(module: &str) -> bool {
    matches!(module, "CM" | "RW" | "OBSQ" | "OBIC" | "OBNC" | "EXEC")
}

/// Members whose **verdict** does not apply to this implementation, though the
/// program itself is perfectly good COBOL.
///
/// Excluded from the execution score only — they still compile, and still
/// count in the `strict` census, because there is nothing wrong with them as
/// source.
///
/// `IX301M` says what it is in its own header: "TESTS THE FLAGGING OF
/// INTERMEDIATE SUBSET FEATURES THAT ARE USED IN LEVEL 1 INDEXED
/// INPUT-OUTPUT". Every construct it expects flagged — `ORGANIZATION IS
/// INDEXED`, `ACCESS MODE IS RANDOM`, `RECORD KEY IS`, and the `NOT INVALID
/// KEY` phrases — is one PowerRustCOBOL implements. A compiler validating at
/// the **high** subset must not flag a feature it supports, so those seven
/// expectations are unreachable by design rather than by defect. Only a
/// minimum-subset validation would satisfy them.
///
/// `RL301M` is the same program one module over — "tests the flagging of
/// intermediate subset features that are used in relative input-output" — and
/// its six expectations are `ORGANIZATION IS RELATIVE`, `ACCESS MODE IS
/// RANDOM`, `RELATIVE KEY IS` and the `NOT INVALID KEY` phrases. Same reading,
/// same ruling.
///
/// Operator ruling, 2026-08-29. Compare `IX401M`, which asks for *high*-subset
/// flagging and scores 10 of 10.
fn verdict_does_not_apply(name: &str) -> bool {
    matches!(name, "IX301M" | "RL301M")
}

fn module_of(name: &str) -> String {
    let cut = name
        .char_indices()
        .find(|(_, c)| c.is_ascii_digit())
        .map(|(b, _)| b)
        .unwrap_or(name.len());
    name[..cut].to_string()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pass = args.first().cloned().unwrap_or_else(|| "col72".to_string());
    let filter = args.get(1).cloned().unwrap_or_default();

    let root0 = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("NIST")
        .join("newcob.val,cbl");

    let source = std::fs::read_to_string(&root0)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", root0.display()));

    // ── snippet probe: `probe <path.cbl>` ────────────────────────────────────
    // Parse one hand-written fixed-format file, so a single construct can be
    // isolated away from the noise of a 1000-line CCVS program.
    if pass == "probe" {
        let text = std::fs::read_to_string(&filter)
            .unwrap_or_else(|e| panic!("cannot read {filter}: {e}"));
        // Probe files are hand-written card images: compile them the real way.
        let lines: Vec<&str> = text.lines().collect();
        let tokens = tokenize(&text, SourceFormat::FixedStrict);
        let pr = cobolt_parser::parse(tokens);
        let errs: Vec<_> = pr.diagnostics.iter().filter(|d| d.is_error()).collect();
        if errs.is_empty() {
            println!("PARSE OK  ({filter})");
        }
        for d in errs {
            let ln = d.span.line as usize;
            println!(
                "L{:<4} {}\n      | {}",
                ln,
                d.message,
                lines
                    .get(ln.saturating_sub(1))
                    .copied()
                    .unwrap_or("<eof>")
                    .trim_end()
            );
        }
        if let Some(prog) = pr.program.as_ref() {
            let sr = cobolt_semantic::analyze(prog);
            for d in sr.errors() {
                println!("SEM L{:<4} {}", d.span.line, d.message);
            }
        }
        return;
    }

    let members = split_members(&source);

    // ── extract mode: `extract <NAME>` — the member, verbatim ────────────────
    // Untouched card images, identification area and all, so the extracted file
    // is exactly what `rcrun --source-format=fixed` has to cope with.
    if pass == "extract" {
        let want = filter.to_uppercase();
        let m = members
            .iter()
            .find(|m| m.name == want)
            .unwrap_or_else(|| panic!("no member named {want}"));
        print!("{}", m.text);
        return;
    }

    // ── feature census: `features` ───────────────────────────────────────────
    // How many of the 459 programs exercise each construct. This is what sizes
    // a spec: "affects N programs" has to be a measurement, not a guess.
    if pass == "features" {
        let mut hits: BTreeMap<&str, usize> = BTreeMap::new();
        let mut n = 0usize;
        for m in members.iter().filter(|m| m.kind == "COBOL") {
            n += 1;
            let prepared = truncate_at_col72(&m.text);
            let mut found: BTreeMap<&str, bool> = BTreeMap::new();
            for line in prepared.lines() {
                if matches!(line.chars().nth(6), Some('*') | Some('/')) {
                    continue;
                }
                let ind = line.chars().nth(6);
                let body = line.get(char_byte(line, 7)..).unwrap_or("");
                let t = body.trim_start();
                if ind == Some('-') {
                    found.insert("continuation line (any)", true);
                    if t.starts_with('"') || t.starts_with('\'') {
                        found.insert("literal continuation", true);
                    }
                }
                let up = body.to_uppercase();
                // leading-decimal-point numeric literal: `.5`, `(.999`, `, .09`
                let bytes: Vec<char> = body.chars().collect();
                for i in 1..bytes.len().saturating_sub(1) {
                    if bytes[i] == '.'
                        && bytes[i + 1].is_ascii_digit()
                        && matches!(bytes[i - 1], ' ' | '(' | ',')
                    {
                        found.insert("numeric literal with leading '.'", true);
                    }
                }
                for (key, pat) in [
                    ("DATE-COMPILED paragraph", "DATE-COMPILED"),
                    ("INSTALLATION paragraph", "INSTALLATION."),
                    ("SECURITY paragraph", "SECURITY."),
                    ("REMARKS paragraph", "REMARKS."),
                    ("CLOSE ... WITH LOCK", "WITH LOCK"),
                    ("LINAGE clause", "LINAGE"),
                    ("USE FOR DEBUGGING", "USE FOR DEBUGGING"),
                    ("WITH DEBUGGING MODE", "DEBUGGING MODE"),
                    ("COPY statement", "COPY "),
                    ("REPLACE statement", "REPLACE "),
                    ("ALPHABET clause", "ALPHABET "),
                    ("CURRENCY SIGN", "CURRENCY"),
                    ("DECIMAL-POINT IS COMMA", "DECIMAL-POINT"),
                    ("SAME ... AREA", "SAME "),
                    ("MULTIPLE FILE TAPE", "MULTIPLE FILE"),
                    ("RERUN clause", "RERUN"),
                    ("ORGANIZATION RELATIVE", "RELATIVE"),
                    ("END PROGRAM", "END PROGRAM"),
                    ("intrinsic FUNCTION", "FUNCTION "),
                    ("FUNCTION arg (ALL)", "(ALL)"),
                    ("SEGMENT-LIMIT", "SEGMENT-LIMIT"),
                    ("REPORT SECTION", "REPORT SECTION"),
                    ("COMMUNICATION SECTION", "COMMUNICATION SECTION"),
                    ("EXTERNAL clause", " EXTERNAL"),
                    ("GLOBAL clause", " GLOBAL"),
                    ("ALTER statement", "ALTER "),
                    ("SORT/MERGE", "SORT "),
                    ("INSPECT CONVERTING", "CONVERTING"),
                ] {
                    if up.contains(pat) {
                        found.insert(key, true);
                    }
                }
                // `NAME SECTION 18.` — a segment priority number.
                if let Some(p) = up.find(" SECTION ") {
                    let rest = up[p + 9..].trim_start();
                    if rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                        found.insert("SECTION with priority number", true);
                    }
                }
                // A comma used as an operand separator (not inside a literal and
                // not a decimal comma): `MOVE ZERO TO DN3, DN4.`
                if !body.contains('"') && up.contains(", ") {
                    found.insert("comma operand separator", true);
                }
            }
            for k in found.keys() {
                *hits.entry(k).or_insert(0) += 1;
            }
        }
        println!("=== construct census over {n} CCVS85 programs ===");
        let mut v: Vec<(&&str, &usize)> = hits.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1));
        for (k, c) in v {
            println!("  {c:>4} programs  ({:>5.1}%)  {k}", *c as f64 * 100.0 / n as f64);
        }
        return;
    }

    // ── bisect mode: `bisect <NAME>` ─────────────────────────────────────────
    // Some programs produce only an end-of-stream `expected PROCEDURE DIVISION`
    // because the offending declaration was swallowed silently during recovery.
    // Re-parse growing prefixes of the DATA DIVISION, each capped with a valid
    // stub PROCEDURE DIVISION, and report the first prefix that breaks.
    if pass == "bisect" {
        let want = filter.to_uppercase();
        let m = members
            .iter()
            .find(|m| m.name == want)
            .unwrap_or_else(|| panic!("no member named {want}"));
        let prepared = m.text.clone();
        let lines: Vec<&str> = prepared.lines().collect();

        // Everything before the PROCEDURE DIVISION header is the declaration part.
        let proc_at = lines
            .iter()
            .position(|l| {
                // Skip comment lines — `*   PROCEDURE DIVISION` in a banner is
                // not the header.
                if matches!(l.chars().nth(6), Some('*') | Some('/')) {
                    return false;
                }
                let t = l.get(7..).unwrap_or("").trim_start();
                t.starts_with("PROCEDURE")
            })
            .unwrap_or(lines.len());

        const STUB: &str = "\n PROCEDURE DIVISION.\n BISECT-STUB.\n     STOP RUN.\n";
        let mut prev_ok = 0usize;
        for k in 1..=proc_at {
            let mut src: String = lines[..k].join("\n");
            src.push_str(STUB);
            let toks = tokenize(&src, SourceFormat::Fixed);
            let r = cobolt_parser::parse(toks);
            let errs: Vec<_> = r.diagnostics.iter().filter(|d| d.is_error()).collect();
            if errs.is_empty() {
                prev_ok = k;
            } else if prev_ok == 0 {
                // Still inside the IDENTIFICATION DIVISION — a prefix that has
                // not reached the program name yet is trivially incomplete.
                continue;
            } else {
                println!("first bad prefix at declaration line {k}:");
                for l in lines[prev_ok..k].iter() {
                    println!("    | {}", l.trim_end());
                }
                for d in errs.iter().take(4) {
                    println!("    -> L{} {}", d.span.line, d.message);
                }
                return;
            }
        }
        println!("declarations clean to line {proc_at}; growing the PROCEDURE DIVISION:");
        // Grow the procedure division a line at a time. A statement spanning
        // several lines makes a transient error, so only report a failure that
        // is still there two lines later.
        let mut j = proc_at;
        while j < lines.len() {
            let errs_at = |upto: usize| -> Vec<String> {
                let src: String = lines[..upto.min(lines.len())].join("\n");
                let toks = tokenize(&src, SourceFormat::Fixed);
                cobolt_parser::parse(toks)
                    .diagnostics
                    .iter()
                    .filter(|d| d.is_error())
                    .map(|d| format!("L{} {}", d.span.line, d.message))
                    .collect()
            };
            let e = errs_at(j + 1);
            if !e.is_empty() && !errs_at(j + 3).is_empty() && !errs_at(j + 6).is_empty() {
                println!("  first persistent error after line {}:", j + 1);
                for l in lines[j.saturating_sub(2)..=j].iter() {
                    println!("    | {}", l.trim_end());
                }
                for m in e.iter().take(3) {
                    println!("    -> {m}");
                }
                return;
            }
            j += 1;
        }
        println!("  no persistent procedure-division error found");
        return;
    }

    // ── EXECUTION scoring: `run [filter]` ────────────────────────────────────
    //
    // Every other pass measures **compilation**: does the front end accept the
    // program. This one measures whether it *works* — it runs each program that
    // compiles clean and reads the program's own `PASS`/`FAIL` report.
    //
    // The distinction is not academic. A CCVS85 program marks each assertion
    // `PASS ` or `FAIL*` in its printed report and tallies them at the end, so
    // the suite scores itself; a program can compile perfectly and still report
    // every assertion failed. 32 of the 35 RELATIVE programs compile against a
    // runtime with no RELATIVE engine at all.
    if pass == "run" {
        run_pass(&members, &filter);
        return;
    }

    // ── failure-detail mode: `fails <MODULE|NAME>` ───────────────────────────
    if pass == "fails" {
        fails_pass(&members, &filter);
        return;
    }

    // ── flagging mode: `flag <MODULE|NAME>` ──────────────────────────────────
    if pass == "flag" {
        flag_pass(&members, &filter);
        return;
    }

    // ── whole-report mode: `report <NAME>` ───────────────────────────────────
    if pass == "report" {
        report_pass(&members, &filter);
        return;
    }

    // ── drill-down mode: `dump <NAME>` ────────────────────────────────────────
    // Print every diagnostic for one member against its own (truncated) source
    // line, so a bucket like `unexpected token: Identifier("Y")` can be traced
    // back to the exact COBOL construct that produced it.
    if pass == "dump" {
        let want = filter.to_uppercase();
        let m = members
            .iter()
            .find(|m| m.name == want)
            .unwrap_or_else(|| panic!("no member named {want}"));
        let text = m.text.clone();
        let lines: Vec<&str> = text.lines().collect();
        let tokens = tokenize(&text, SourceFormat::FixedStrict);
        let pr = cobolt_parser::parse(tokens);
        println!("=== {} ({} source lines) ===", m.name, lines.len());
        let mut shown = 0usize;
        for d in pr.diagnostics.iter().filter(|d| d.is_error()) {
            let ln = d.span.line as usize;
            let src = lines.get(ln.saturating_sub(1)).copied().unwrap_or("<eof>");
            println!("L{:<5} col{:<4} {}", ln, d.span.col, d.message);
            println!("        | {}", src.trim_end());
            shown += 1;
            if shown >= 80 {
                println!("        ... (truncated at 80)");
                break;
            }
        }
        if let Some(prog) = pr.program.as_ref() {
            let sr = cobolt_semantic::analyze(prog);
            for d in sr.errors().take(20) {
                println!("SEM L{:<5} {}", d.span.line, d.message);
            }
        }
        return;
    }

    // The suite's own COPY library, written out once for this run.
    let copy_dir = copy_library_dir(&members);

    let mut total = 0usize;
    let mut clean = 0usize;
    let mut parse_buckets: BTreeMap<String, (usize, usize)> = BTreeMap::new(); // msg -> (hits, programs)
    let mut sem_buckets: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut per_module: BTreeMap<String, (usize, usize)> = BTreeMap::new(); // module -> (total, clean)
    let mut failures: Vec<(String, usize, usize, String)> = Vec::new();
    let mut root_causes: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();

    for m in &members {
        if m.kind != "COBOL" {
            continue;
        }
        let module = module_of(&m.name);
        if !filter.is_empty() && !m.name.starts_with(&filter) {
            continue;
        }
        total += 1;

        let (text, fmt) = prepare(&pass, &m.text);

        // Expand `COPY` / `REPLACE` exactly the way `rcrun` does, against the
        // suite's own copybook library. Without this the Source Text
        // Manipulation module cannot be measured at all — its whole subject is
        // the directive the harness would otherwise leave unexpanded.
        //
        // ⚠️ **Expansion flattens to free form**, which is why `rcrun` then
        // tokenizes `Free` (cobolt-cli/src/main.rs). Flattening the result a
        // second time as `FixedStrict` strips the column areas twice and
        // corrupts every program — measured: it cost 19 programs across six
        // modules while gaining 3 in SM.
        let (text, lex_fmt) = match copy_dir.as_deref() {
            Some(dir) => (
                cobolt_lexer::expand_copybooks(&text, dir, fmt).text,
                SourceFormat::Free,
            ),
            None => (text, fmt),
        };

        eprintln!("[{total:>3}/459] {} ({} lines)", m.name, text.lines().count());
        let t0 = std::time::Instant::now();
        let tokens = tokenize(&text, lex_fmt);
        let t_lex = t0.elapsed();
        let pr = cobolt_parser::parse(tokens);
        let t_all = t0.elapsed();
        if t_all.as_millis() > 500 {
            eprintln!(
                "        SLOW: lex {:?}, parse {:?}",
                t_lex,
                t_all - t_lex
            );
        }

        let perrs: Vec<&cobolt_parser::Diagnostic> =
            pr.diagnostics.iter().filter(|d| d.is_error()).collect();

        let mut seen_in_prog: BTreeMap<String, ()> = BTreeMap::new();
        for d in &perrs {
            let b = bucket(&d.message);
            seen_in_prog.insert(b.clone(), ());
            parse_buckets.entry(b).or_insert((0, 0)).0 += 1;
        }
        for b in seen_in_prog.keys() {
            parse_buckets.entry(b.clone()).or_insert((0, 0)).1 += 1;
        }

        let mut serr_count = 0usize;
        if let Some(prog) = pr.program.as_ref() {
            let sr = cobolt_semantic::analyze(prog);
            let mut seen_s: BTreeMap<String, ()> = BTreeMap::new();
            for d in sr.errors() {
                serr_count += 1;
                let b = bucket(&d.message);
                seen_s.insert(b.clone(), ());
                sem_buckets.entry(b).or_insert((0, 0)).0 += 1;
            }
            for b in seen_s.keys() {
                sem_buckets.entry(b.clone()).or_insert((0, 0)).1 += 1;
            }
        }

        let e = per_module.entry(module).or_insert((0, 0));
        e.0 += 1;
        if perrs.is_empty() && serr_count == 0 {
            clean += 1;
            e.1 += 1;
        } else {
            let first = perrs
                .first()
                .map(|d| format!("L{} {}", d.span.line, d.message))
                .unwrap_or_else(|| "(semantic only)".to_string());
            // The first diagnostic is often an end-of-stream `expected
            // PROCEDURE DIVISION` raised long after the real mistake, because
            // the parser swallowed the offending construct during recovery.
            // Prefer the first diagnostic that actually points at a line.
            let root = perrs
                .iter()
                .find(|d| d.span.line > 0)
                .or_else(|| perrs.first());
            if let Some(d) = root {
                let src_lines: Vec<&str> = text.lines().collect();
                let ln = d.span.line as usize;
                let src = src_lines
                    .get(ln.saturating_sub(1))
                    .copied()
                    .unwrap_or("<eof>")
                    .to_string();
                root_causes
                    .entry(bucket(&d.message))
                    .or_default()
                    .push((m.name.clone(), src));
            }
            failures.push((m.name.clone(), perrs.len(), serr_count, first));
        }
    }

    println!("=== NIST CCVS85 front-end conformance — pass `{pass}` ===");
    println!("programs analysed : {total}");
    println!(
        "clean (0 errors)  : {clean}  ({:.1}%)",
        if total == 0 {
            0.0
        } else {
            clean as f64 * 100.0 / total as f64
        }
    );
    println!();

    // ── PASS / FAIL / N-A ────────────────────────────────────────────────────
    // The headline split. `N/A` is not a euphemism for failure: those modules
    // are declared out of RustCOBOL's scope in
    // specs/nist/NIST-spec-out-of-scope-modules.md and are not being worked on.
    let mut na_total = 0usize;
    let mut in_total = 0usize;
    let mut in_clean = 0usize;
    for (module, (t, c)) in &per_module {
        if is_out_of_scope(module) {
            na_total += t;
        } else {
            in_total += t;
            in_clean += c;
        }
    }
    let pct = |n: usize, d: usize| if d == 0 { 0.0 } else { n as f64 * 100.0 / d as f64 };
    println!("--- PASS / FAIL / N-A ---");
    println!(
        "  PASS  {in_clean:>3} / {in_total}   ({:.1}% of the in-scope suite)",
        pct(in_clean, in_total)
    );
    println!(
        "  FAIL  {:>3} / {in_total}   ({:.1}%)",
        in_total - in_clean,
        pct(in_total - in_clean, in_total)
    );
    println!(
        "  N-A   {na_total:>3} / {total}   (out of RustCOBOL scope: CM, RW, OB*, EXEC85)"
    );
    println!();

    println!("--- per module (clean / total) ---");
    for (module, (t, c)) in &per_module {
        let mark = if is_out_of_scope(module) { "  N-A" } else { "" };
        println!("  {module:<6} {c:>3} / {t:<3}{mark}");
    }
    println!();

    let mut pv: Vec<(&String, &(usize, usize))> = parse_buckets.iter().collect();
    pv.sort_by(|a, b| b.1 .1.cmp(&a.1 .1).then(b.1 .0.cmp(&a.1 .0)));
    println!("--- top PARSE diagnostics (progs affected / total hits) ---");
    for (msg, (hits, progs)) in pv.iter().take(45) {
        println!("  {progs:>3} progs / {hits:>6} hits   {msg}");
    }
    println!("  ... {} distinct parse buckets total", parse_buckets.len());
    println!();

    let mut sv: Vec<(&String, &(usize, usize))> = sem_buckets.iter().collect();
    sv.sort_by(|a, b| b.1 .1.cmp(&a.1 .1).then(b.1 .0.cmp(&a.1 .0)));
    println!("--- top SEMANTIC diagnostics (progs affected / total hits) ---");
    for (msg, (hits, progs)) in sv.iter().take(25) {
        println!("  {progs:>3} progs / {hits:>6} hits   {msg}");
    }
    println!("  ... {} distinct semantic buckets total", sem_buckets.len());
    println!();

    // Every failure, not a sample: a truncated list silently hides whole
    // categories, and the tail is where the unfamiliar ones are.
    println!("--- first failing diagnostic per program (all {}) ---", failures.len());
    for (name, p, s, first) in failures.iter() {
        println!("  {name:<8} parse={p:<5} sem={s:<5} {first}");
    }
    println!("  ... {} failing programs total", failures.len());
    println!();

    // ── root-cause census ────────────────────────────────────────────────────
    // Only the FIRST error of each program is a genuine root cause; everything
    // after it is recovery debris. Bucket those, and show the COBOL line that
    // provoked each, which is what a spec actually needs.
    println!("=== ROOT CAUSE census (first error per program) ===");
    let mut rv: Vec<(&String, &Vec<(String, String)>)> = root_causes.iter().collect();
    rv.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    for (msg, samples) in rv.iter() {
        println!("\n[{} programs] {}", samples.len(), msg);
        let mut shown: Vec<&str> = Vec::new();
        for (prog, line) in samples.iter() {
            let t = line.trim_end();
            if shown.iter().any(|s| *s == t) {
                continue;
            }
            println!("    {prog:<8} | {t}");
            shown.push(t);
            if shown.len() >= 12 {
                break;
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Execution scoring
// ─────────────────────────────────────────────────────────────────────────────

/// How one program ended.
#[derive(Debug, Clone, PartialEq)]
enum RunOutcome {
    /// The front end rejected it — this pass never ran it.
    CompileFail,
    /// It ran to completion. `(pass, fail, deleted)` from its own report.
    Ran(usize, usize, usize),
    /// It ran to completion but printed no CCVS report at all.
    NoReport,
    /// Still running when the wall-clock budget expired.
    Timeout,
    /// Killed for writing more than the output budget — a runaway loop.
    Runaway(u64),
    /// The runtime refused it (a diagnostic, a panic, a non-zero exit).
    Crash(String),
}

/// Wall-clock budget for one program.
const RUN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// Output budget for one program's print file.
///
/// Not a nicety: `IF101A` compiles clean and then loops writing blank lines,
/// producing **4.2 GB in ten minutes** on the first attempt at this pass. A
/// wall-clock timeout alone would still let a full sweep fill the disk, so the
/// size is checked on the same tick as the clock.
const RUN_MAX_OUTPUT: u64 = 2 * 1024 * 1024;

/// The CCVS85 print file. Programs `SELECT PRINT-FILE ASSIGN TO XXXXX055`,
/// the installation card for the system printer, and RustCOBOL resolves an
/// unbound ASSIGN name to a file of that name in the working directory — so
/// each program is run in its own directory and this is where its report lands.
const CCVS_PRINT_FILE: &str = "XXXXX055";

/// Where the child's console output is captured.
///
/// `run_one` used to hand the child a **pipe** for stdout and then never read
/// it, which is a deadlock with a fuse on it: a program that displays more than
/// the operating system's pipe buffer blocks forever on the write and is scored
/// as a timeout rather than as whatever it actually was. Redirecting to a file
/// drains it unconditionally, and it makes a program that reports on the
/// console instead of to `XXXXX055` scoreable at all — see
/// [`score_console_report`]. The name is deliberately not an `XXXXX0nn`
/// installation card, so it can never collide with a member's own `ASSIGN`.
const CCVS_CONSOLE_FILE: &str = "rcrun-console.txt";

/// What the operator types, for the members that read from the console.
///
/// CCVS85 Format 1 `ACCEPT` reads from the operator, and the suite's run
/// instructions tell them what to enter. The deck is **recovered from the
/// source, not invented**: every accepted value is compared against a paired
/// `VALUE` literal in the program's own DATA DIVISION, so each line below is
/// the literal its test expects. A member not listed here is given a closed
/// stdin, exactly as before.
fn operator_input(name: &str) -> Option<String> {
    let lines: &[&str] = match name {
        "NC109M" => NC109M_OPERATOR_LINES,
        "NC204M" => NC204M_OPERATOR_LINES,
        _ => return None,
    };
    let mut s = lines.join("\n");
    s.push('\n');
    Some(s)
}

/// The member that creates the data file this one reads, when the program says
/// it inherits one.
///
/// Most CCVS85 programs build whatever they need and are self-contained. A few
/// are the second half of a pair: they read a file an **earlier member** wrote,
/// and each one says so in its own header comment. Run such a program in an
/// empty directory and it opens a file that is not there, then correctly
/// reports the absence as a failure — IX110A scores 2 FAIL / 2 PASS alone and
/// **4 PASS / 0 FAIL** with its producer run first, against an unchanged
/// runtime.
///
/// Every entry below is quoted from the consumer's own header. Nothing is
/// inferred from which X-cards two programs happen to share.
///
/// **Blanket sharing is wrong**, and measuring it is how that was settled:
/// giving a whole module one directory took IX from 13 to 15 clean but broke
/// members that need a file to be *absent* — IX111A ("THIS PROGRAM USES THE
/// FILE IX-NOP WHICH DOES NOT EXIST", expecting status 35) and IX216A (`OPEN
/// EXTEND` on an OPTIONAL file, expecting 05) both went red against leftovers
/// from earlier programs. A validating installation scratches files between
/// programs except where a member declares it inherits one, which is what this
/// table encodes.
fn inherits_from(name: &str) -> Option<&'static str> {
    Some(match name {
        // "THE FILE USED AS INPUT IS THAT CREATED BY IX101."
        "IX102A" => "IX101A",
        // "THE FILE USED IS THAT RESULTING FROM IX102." Its own file is card
        // 24 spelled `XXXXD024`, which IX102A wrote as `XXXXP024` — see
        // `canonical_x_cards`.
        "IX103A" => "IX102A",
        // "THE ROUTINE USES THE FILE IX-FS3 WHICH HAS BEEN CREATED BY IX109."
        "IX110A" => "IX109A",
        // "THIS ROUTINE USES THE MASS STORAGE FILE IX-FS3 CREATED IN IX113A."
        "IX114A" | "IX115A" | "IX116A" | "IX117A" | "IX118A" | "IX119A" | "IX120A" => "IX113A",
        // "THE FILE USED AS INPUT IS THAT CREATED BY IX201A."
        "IX202A" => "IX201A",
        // "THE FILE USED IS THAT RESULTING FROM IX202." The IX2xx series
        // mirrors IX1xx: one member creates the file, the next two process it.
        "IX203A" => "IX202A",
        // ── RL: the same three-generation shape, four times over ───────────
        // "THE FILE USED AS INPUT IS THAT FILE CREATED BY RL101." Each series
        // is a creator, an updater and a verifier over one file, all four of
        // them on card 21 (`XXXXP021` writing, `XXXXD021` reading — see
        // `canonical_x_cards`), so the whole chain shares one directory.
        "RL102A" => "RL101A",
        // "THE FILE USED IS THAT RESULTING FROM RL102."
        "RL103A" => "RL102A",
        "RL109A" => "RL108A",
        // "THE FILE USED IS THAT RESULTING FROM RL109A."
        "RL110A" => "RL109A",
        "RL202A" => "RL201A",
        // The header says "RESULTING FROM RL102", but RL203A is the third of
        // the RL2xx series and reads what RL202A left on card 21 — the suite
        // copied the RL1xx comment and did not renumber it.
        "RL203A" => "RL202A",
        "RL207A" => "RL206A",
        // "THE FILE USED IS THAT RESULTING FROM RL206A" — same off-by-one
        // comment as RL203A; the producer is the updater, RL207A.
        "RL208A" => "RL207A",
        // "THE FILE USED AS INPUT IS THE FILE 'RL-FS1' CREATED BY RL212A AND
        // THE OTHER FILE 'RL-FS2' WILL NOT BE PRESENT." RL212A writes card 21
        // only, so card 22 stays absent exactly as the program requires.
        "RL213A" => "RL212A",
        _ => return None,
    })
}

/// Run the chain of producers a member declares, into that member's own
/// directory, so it finds the file it says it inherits.
///
/// Follows [`inherits_from`] transitively — a producer that is itself a
/// consumer is run first — and runs each producer for its side effects only;
/// its report is discarded. A member that declares nothing costs nothing here.
fn run_producers(
    rcrun: &std::path::Path,
    dir: &std::path::Path,
    members: &[Member],
    target: &str,
) {
    // **Every producer runs in the consumer's own directory** — one directory
    // for the whole chain. Deriving a directory per generation put IX102A
    // where IX101A had never run, so it processed an empty file and left a
    // short one: IX103A's scan reached 35 records of 500 while IX101A and
    // IX102A both reported perfectly clean. `producer_chain` is flat and
    // oldest-first, which makes that mistake unavailable.
    for name in producer_chain(target) {
        match members.iter().find(|m| m.name == name) {
            Some(m) => {
                run_one_in(rcrun, dir, &m.name, &m.text);
            }
            None => eprintln!("  ! {target} declares producer {name}, not in the suite"),
        }
    }
}

/// The producers `target` depends on, **oldest first**.
///
/// Flat rather than recursive so the caller cannot accidentally vary anything
/// per generation. The chain is short and [`declared_producers_terminate`]
/// pins that it ends; the guard here is a backstop against a future cycle.
fn producer_chain(target: &str) -> Vec<&'static str> {
    let mut chain = Vec::new();
    let mut at = target;
    while let Some(p) = inherits_from(at) {
        if chain.contains(&p) {
            break;
        }
        chain.push(p);
        at = p;
    }
    chain.reverse();
    chain
}

/// The data files the **installation** supplies, planted into a member's work
/// directory before it runs.
///
/// CCVS85 leaves some inputs for the installation to provide, exactly as it
/// leaves the operator deck, the external switches and the `XXXXX053` RERUN
/// card. `XXXXD001` is one of them: it is the *file present* half of SQ203A's
/// `SELECT OPTIONAL` test, and **no member of the suite writes it** — all 459
/// were checked. Without it that half cannot run at all in a fresh directory,
/// and the program correctly reports the absence as a failure.
///
/// A member not listed here gets nothing, and its directory is untouched.
fn installation_data_files(name: &str) -> &'static [(&'static str, &'static str)] {
    match name {
        "SQ203A" => &[("XXXXX001", SQ203A_XXXXD001)],
        _ => &[],
    }
}

/// `XXXXD001` — one 120-character record, for SQ203A's present optional file.
///
/// **The content is the suite's own, not the assertions'.** SQ203A's
/// `READ-TEST-GF-04` creates a file of exactly this shape for `SQ-FS3` a few
/// paragraphs further down, and every field below is set the way that
/// paragraph sets it, against the `FILE-RECORD-INFO` skeleton the program
/// declares. Only two fields differ, and both are forced by which file this
/// is: `XFILE-NAME` names `SQ-FS1` rather than `SQ-FS3`, and
/// `RECORDS-IN-FILE` says 1 rather than 750 because this file holds one
/// record. The fields that paragraph never sets are zero or blank.
///
/// `SQ-FS1` is `ORGANIZATION IS SEQUENTIAL` with a single fixed 120-byte `01`,
/// so the file is these 120 bytes and nothing else — no length prefix and no
/// line terminator, or the `READ` would return a short or shifted record.
const SQ203A_XXXXD001: &str = concat!(
    "     ",  //   0  FILLER              PIC X(5)
    "SQ-FS1", //   5  XFILE-NAME          PIC X(6)
    "        ", // 11  FILLER              PIC X(8)
    "R1-F-G", //  19  XRECORD-NAME        PIC X(6)
    " ",      //  25  FILLER              PIC X(1)
    "1",      //  26  REELUNIT-NUMBER     PIC 9(1)
    "       ", // 27  FILLER              PIC X(7)
    "000001", //  34  XRECORD-NUMBER      PIC 9(6)
    "      ", //  40  FILLER              PIC X(6)
    "00",     //  46  UPDATE-NUMBER       PIC 9(2)
    "     ",  //  48  FILLER              PIC X(5)
    "0000",   //  53  ODO-NUMBER          PIC 9(4)
    "     ",  //  57  FILLER              PIC X(5)
    "SQ203",  //  62  XPROGRAM-NAME       PIC X(5)  ("SQ203A" truncated to X(5))
    "       ", // 67  FILLER              PIC X(7)
    "000120", //  74  XRECORD-LENGTH      PIC 9(6)
    "       ", // 80  FILLER              PIC X(7)
    "RC",     //  87  CHARS-OR-RECORDS    PIC X(2)
    " ",      //  89  FILLER              PIC X(1)
    "0001",   //  90  XBLOCK-SIZE         PIC 9(4)
    "      ", //  94  FILLER              PIC X(6)
    "000001", // 100  RECORDS-IN-FILE     PIC 9(6)
    "     ",  // 106  FILLER              PIC X(5)
    "SQ",     // 111  XFILE-ORGANIZATION  PIC X(2)
    "      ", // 113  FILLER              PIC X(6)
    "S",      // 119  XLABEL-TYPE         PIC X(1)
);

/// `NC109M`'s operator deck — 11 values, in the order the program accepts them.
///
/// Leading and trailing spaces are significant and are inside the quotes on
/// purpose: `ACCEPT-D15` expects a single space and `ACCEPT-D12` a value that
/// both starts and ends with one. `exec_accept` strips only the line
/// terminator, so what is written here is what the program receives.
const NC109M_OPERATOR_LINES: &[&str] = &[
    "ABCDEFGHIJKLMNOPQRSTUVWXY Z", // ACCEPT-D1  vs ACCEPT-D2,  PIC X(27)
    "0123456789",                  // ACCEPT-D3  vs ACCEPT-D4,  PIC 9(10)
    "().+-*/$, =",                 // ACCEPT-D5  vs ACCEPT-D6,  PIC X(11)
    "9",                           // ACCEPT-D7  vs ACCEPT-D8
    "0",                           // ACCEPT-D9  vs ACCEPT-D10
    " ABC            XYZ ",        // ACCEPT-D11 vs ACCEPT-D12, PIC A(20)
    "012345678",                   // ACCEPT-D13 vs ACCEPT-D14, PIC 9(9)
    " ",                           // ACCEPT-D15 vs ACCEPT-D16, VALUE SPACE
    "\"",                          // ACCEPT-D17 vs ACCEPT-D18, VALUE QUOTE
    "ABCD",                        // TAB-ACCEPT(2): "...." ABCD "...." = ACCEPT-D22
    // ACCEPT-RESULTS, PIC X(80) — the 63 significant characters; ACCEPT
    // space-pads the rest of the field.
    "A B C D E F G H I J K L M N O P Q R S T U V W X Y Z  0123456789",
];

/// `NC204M`'s operator deck — 15 values, in the order the program accepts them.
///
/// NC204M is NC109M's twin: where NC109M writes Format 1 `ACCEPT` bare, NC204M
/// routes every read through the mnemonic `ACCEPT-INPUT-DEVICE` that its
/// `SPECIAL-NAMES` associates with the input device. The deck is recovered the
/// same way — each accepted item is compared against a paired item whose value
/// the program sets just above the `ACCEPT`, so every line here is that value.
///
/// The two lines the program's own numbering skips (`ACCEPT-D14`) are absent
/// because NC204M has no `ACC-TEST` for them; the count is what the section
/// actually reads, not what the data division declares.
const NC204M_OPERATOR_LINES: &[&str] = &[
    "ABCDEFGHIJKLMNOPQRSTUVWXY Z", // ACCEPT-D1  vs ACCEPT-D2,  X(20) + X(7)
    "0123456789",                  // ACCEPT-D3  vs ACCEPT-D4,  PIC 9(10)
    "().+-*/$, =",                 // ACCEPT-D5  vs ACCEPT-D6,  PIC X(11)
    "9",                           // ACCEPT-D7  vs ACCEPT-D8
    "0",                           // ACCEPT-D9  vs ACCEPT-D10
    " ABC            XYZ ",        // ACCEPT-D11 vs ACCEPT-D12, PIC A(20)
    " 9",                          // ACCEPT-D15 vs ACCEPT-D16, PIC XX
    "\"",                          // ACCEPT-D17 vs ACCEPT-D18, VALUE QUOTE
    "Q",                           // QUAL-ACCEPT OF ACCEPT-D19 vs ACCEPT-D20
    "ABCD",                        // TAB-ACCEPT(2):  "...." ABCD "...."
    "ABCD",                        // TAB-A IN ACCEPT-D23 (SUB=5): 16 dashes + ABCD
    // 80X-CHARACTER-FIELD vs ACCEPT-RESULTS, PIC X(80) — the 63 significant
    // characters; `ACCEPT` space-pads the rest of the field.
    "A B C D E F G H I J K L M N O P Q R S T U V W X Y Z  0123456789",
    // ACCEPT-D13 vs DISPLAY-F, PIC X(200) — DISPLAY-G then DISPLAY-H, as
    // `ACC-INIT-F1-13` sets them. The `D` in place of a `*` separator at 020
    // and 040 is the program's own marker, not a transcription slip.
    concat!(
        "D001*002*003*004*005*006*007*008*009*010*011*012*013*014*015*016*017*018*019*020D021*022*023*024*025",
        "*026*027*028*029*030*031*032*033*034*035*036*037*038*039*040D041*042*043*044*045*046*047*048*049*050",
    ),
    // ACCEPT-TEST-14-DATA, PIC X(15), read twice — the test exists to show a
    // device asking for more input when one record cannot fill the item (VI-71
    // 6.5.4 GR4(a)), so the two records are what the operator types.
    //
    // Read the overlays, not their names: both `FILLER REDEFINES
    // ACCEPT-TEST-14-DATA` groups start at the item's first byte, so
    // `ACC-14-CHARS-1-10` is bytes 1–10 and `ACC-14-CHARS-11-15`, in spite of
    // what it is called, is bytes 1–5. Each `ACCEPT` therefore has to *begin*
    // with what the paragraph after it checks.
    "ABCDEFGHIJ", // ACC-TEST-F1-14-1 checks bytes 1–10
    "KLMNO",      // ACC-TEST-F1-14-2 checks bytes 1–5
];

fn run_pass(members: &[Member], filter: &str) {
    let rcrun = match locate_rcrun() {
        Some(p) => p,
        None => {
            eprintln!(
                "cannot find the `rcrun` binary. Build it first:\n    cargo build --release -p cobolt-cli"
            );
            std::process::exit(2);
        }
    };
    let workroot = std::env::temp_dir().join("nist-exec-scoring");
    let _ = std::fs::remove_dir_all(&workroot);
    std::fs::create_dir_all(&workroot).expect("cannot create the work directory");

    let programs: Vec<&Member> = members
        .iter()
        .filter(|m| m.kind == "COBOL")
        .filter(|m| filter.is_empty() || m.name.to_uppercase().starts_with(&filter.to_uppercase()))
        .collect();

    let mut outcomes: Vec<(String, String, RunOutcome)> = Vec::new();

    for (i, m) in programs.iter().enumerate() {
        let module = module_of(&m.name);
        if is_out_of_scope(&module) || verdict_does_not_apply(&m.name) {
            continue;
        }
        eprintln!("[{}/{}] {}", i + 1, programs.len(), m.name);

        let (text, fmt) = prepare("strict", &m.text);
        let pr = cobolt_parser::parse(tokenize(&text, fmt));
        let compiles = pr.diagnostics.iter().all(|d| !d.is_error())
            && pr
                .program
                .as_ref()
                .map(|p| cobolt_semantic::analyze(p).errors().count() == 0)
                .unwrap_or(false);
        if !compiles {
            outcomes.push((m.name.clone(), module, RunOutcome::CompileFail));
            continue;
        }

        // A flagging member is a **compile-time** test. It carries no PASS/FAIL
        // machinery at all — what is validated is the diagnostics the compiler
        // emits for the constructs it contains — so it is scored against its
        // own expectation comments and never executed. Running one scores
        // nothing whatever it does, and `NC401M` loops.
        if is_flagging_member(&m.text) {
            let (p, f, _) = score_flagging(&m.text);
            outcomes.push((m.name.clone(), module, RunOutcome::Ran(p, f, 0)));
            continue;
        }

        let (outcome, _) = run_one(&rcrun, &workroot, members, &m.name, &m.text);
        outcomes.push((m.name.clone(), module, outcome));
    }

    let _ = std::fs::remove_dir_all(&workroot);
    report_run(&outcomes);
}

/// Run one program and hand back the failure detail its own report carries.
///
/// A CCVS `FAIL*` record is followed by `COMPUTED=` and `CORRECT =` records
/// that name the defect outright, so reading them across a whole module is how
/// a shared cause is found — one program's detail is an anecdote, forty
/// programs' is a bucket. Records are `PIC X(120)` with no newline between
/// them, so the report is re-split on that fixed width, exactly as
/// [`score_ccvs_report`] counts markers without assuming line breaks.
fn fails_pass(members: &[Member], filter: &str) {
    let rcrun = match locate_rcrun() {
        Some(p) => p,
        None => {
            eprintln!(
                "cannot find the `rcrun` binary. Build it first:\n    cargo build --release -p cobolt-cli"
            );
            std::process::exit(2);
        }
    };
    let workroot = std::env::temp_dir().join("nist-fail-detail");
    let _ = std::fs::remove_dir_all(&workroot);
    std::fs::create_dir_all(&workroot).expect("cannot create the work directory");

    for m in members.iter().filter(|m| m.kind == "COBOL") {
        if !filter.is_empty() && !m.name.to_uppercase().starts_with(&filter.to_uppercase()) {
            continue;
        }
        if is_out_of_scope(&module_of(&m.name)) {
            continue;
        }
        let (outcome, report) = run_one(&rcrun, &workroot, members, &m.name, &m.text);
        let report = match (&outcome, report) {
            (RunOutcome::Ran(_, f, _), Some(r)) if *f > 0 => r,
            _ => continue,
        };
        // Split on **bytes**, not characters: the record is `PIC X(120)`, a
        // byte count. A report carrying `HIGH-VALUES` holds multi-byte
        // sequences, and chunking by character drifted the alignment a little
        // further with every one of them until the columns were unreadable.
        let recs: Vec<String> = report
            .as_bytes()
            .chunks(120)
            .map(|c| String::from_utf8_lossy(c).trim_end().to_string())
            .collect();
        println!("╔═══ {} ═══", m.name);
        // CCVS writes a failing detail line twice on purpose
        // (`IF P-OR-F EQUAL TO "FAIL*" PERFORM WRITE-LINE`), so the same record
        // is skipped when it repeats.
        let mut prev = String::new();
        for (i, r) in recs.iter().enumerate() {
            if !r.contains("FAIL*") || *r == prev {
                prev.clone_from(r);
                continue;
            }
            prev.clone_from(r);
            println!("║ {r}");
            // The record straight after a failure is that same failure written
            // a second time; the `COMPUTED=` / `CORRECT =` pair that names the
            // defect comes after it.
            for follow in recs.iter().skip(i + 1).take(4) {
                if follow == r {
                    continue;
                }
                if follow.contains("FAIL*") || follow.is_empty() {
                    break;
                }
                println!("║     {follow}");
            }
        }
    }
    let _ = std::fs::remove_dir_all(&workroot);
}

/// Print one program's whole CCVS report, re-split into its 120-byte records.
///
/// [`fails_pass`] shows only the `FAIL*` neighbourhood, which is enough to
/// bucket a cause across a module but not to diagnose one program: the
/// `COMPUTED=` / `CORRECT =` pair a test writes can sit several records away,
/// and the paragraph headings that say *which* feature is under test are
/// dropped entirely. This pass prints everything, in order.
fn report_pass(members: &[Member], filter: &str) {
    let rcrun = match locate_rcrun() {
        Some(p) => p,
        None => {
            eprintln!(
                "cannot find the `rcrun` binary. Build it first:\n    cargo build --release -p cobolt-cli"
            );
            std::process::exit(2);
        }
    };
    let want = filter.to_uppercase();
    let m = match members.iter().find(|m| m.name == want) {
        Some(m) => m,
        None => {
            eprintln!("no member named {want}");
            std::process::exit(2);
        }
    };
    let workroot = std::env::temp_dir().join("nist-report");
    let _ = std::fs::remove_dir_all(&workroot);
    std::fs::create_dir_all(&workroot).expect("cannot create the work directory");
    let (outcome, report) = run_one(&rcrun, &workroot, members, &m.name, &m.text);
    println!("=== {} — {outcome:?} ===", m.name);
    if let Some(r) = report {
        // Byte chunks, not characters: see `fails_pass`.
        for rec in r.as_bytes().chunks(120) {
            println!("{}", String::from_utf8_lossy(rec).trim_end());
        }
    }
    let _ = std::fs::remove_dir_all(&workroot);
}

/// The CCVS85 implementor substitutions, applied to a member's source before
/// it is compiled and run.
///
/// `XXXXX090` and `XXXXX091` are the suite's placeholders for the **ordinal
/// positions, in the implementor's native character set, of the letters `A`
/// and `D`**. NC174A and NC254A declare the same three classes twice — once
/// with the literals `"A"` and `"D"`, once with these placeholders — and check
/// that the two spellings agree. Left unsubstituted the ordinal classes name no
/// character at all, so `'A' IS ORDINAL-A-ONLY` was false while its `ACTUAL-`
/// twin passed. This is the same kind of implementor input the switch settings
/// and the operator decks already supply, not a change to what is tested.
///
/// **Each replacement is padded to the placeholder's own eight characters.**
/// These are fixed-format decks: a shorter operand drags the sequence area in
/// columns 73-80 leftwards into the content area, where `NC1744.2` would then
/// be read as code.
/// Collapse the `XXXXP nnn` / `XXXXD nnn` spellings of an X-card onto the
/// canonical `XXXXX nnn`.
///
/// An X-card is identified by its **number**, not by the letter in position 5.
/// IX103A's own header lists "X-24 INDEXED FILE IMPLEMENTOR-NAME IN ASSGN TO
/// CLAUSE FOR DATA FILE IX-FS1" and "X-44 … FOR INDEX FILE IX-FS1" — the number
/// says which file, and card 24 appears in the deck as `XXXXX024` (43 times),
/// `XXXXP024` (6) and `XXXXD024` (2). A validating installation replaces each
/// card with one implementor name, so all three spellings name one file.
///
/// Left alone they are three different files, and a chain breaks in silence:
/// IX102A creates IX-FS1 as `XXXXP024`, IX103A processes "THE FILE USED IS
/// THAT RESULTING FROM IX102" as `XXXXD024`, and every one of its sequential
/// reads hit AT END on the first call because the file it opened had never
/// been written.
///
/// **Only `P` and `D` are collapsed**, the two for which the deck gives direct
/// evidence. `XXXXY382` and `XXXXY066` are left as they are — their numbers
/// match no other card, so there is nothing to say they are variants of
/// anything.
///
/// The replacement is the same eight characters, which matters: these are
/// fixed-format decks and a shorter operand would drag the sequence area in
/// columns 73-80 into the content area.
fn canonical_x_cards(raw: &str) -> String {
    let b = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    while i < b.len() {
        // `XXXX` + (`P` | `D`) + three digits, and nothing alphanumeric before
        // it, so a longer identifier that merely ends this way is untouched.
        let card_here = i + 8 <= b.len()
            && &b[i..i + 4] == b"XXXX"
            && matches!(b[i + 4], b'P' | b'D')
            && b[i + 5..i + 8].iter().all(u8::is_ascii_digit)
            // A COBOL word may contain hyphens, so `MY-XXXXP024` is one
            // identifier and not a card reference.
            && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'-'));
        if card_here {
            out.push_str("XXXXX");
            out.push_str(&raw[i + 5..i + 8]);
            i += 8;
        } else {
            out.push(raw[i..].chars().next().unwrap_or('\0'));
            i += raw[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    out
}

/// Comment out the `U` opt-code lines, selecting the `T` alternative.
///
/// A CCVS85 line may carry an opt-code letter in the indicator column, and the
/// installation's `*OPT` card says which letters are active — "THE LETTER
/// CORRESPONDS TO A CHARACTER IN POSITION 7 OF THE SOURCE LINE". Most letters
/// mark additions that are harmless to include. **`T` and `U` are different:
/// they are mutually exclusive alternatives**, and taking both makes a record
/// longer than either reading intends.
///
/// IX208A's `IX-FS2R1-F-G-240` is the clearest case. Its own name says 240
/// characters; with `T` alone it is 240, with `U` alone it is 240, and with
/// both it is **250**, every field after the first key displaced by five.
///
/// **`T` is the reading these programs are written for.** IX208A builds
/// `WRK-IX-FS2-ALTKEY` from a `T` line plus a five-digit number, making it ten
/// characters — the same shape as `IX-FS2-ALTKEY1` under `T` and not under `U`.
/// The program moves one into the other before every `START`, so they have to
/// agree.
///
/// There are exactly **ten** `U` lines in the suite, in IX107A, IX207A and
/// IX208A. No member of any other module carries one, so this cannot disturb a
/// finished module.
///
/// The letter is replaced by `*` rather than removed, so every column after it
/// stays where it was in these fixed-format decks.
fn select_opt_t_over_u(raw: &str) -> String {
    raw.lines()
        .map(|line| {
            if line.len() > 6 && line.as_bytes()[6] == b'U' {
                let mut s = line.to_string();
                s.replace_range(6..7, "*");
                s
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn substitute_implementor_names(raw: &str) -> String {
    let raw = &select_opt_t_over_u(&canonical_x_cards(raw));
    // ASCII puts `A` at 65 and `D` at 68, so the 1-based ordinal positions are
    // 66 and 69 — the values `parse_special_names_class` turns back into
    // characters with `char::from_u32(n - 1)`.
    raw.replace("XXXXX090", "66      ")
        .replace("XXXXX091", "69      ")
        // `XXXXX053` is the I-O-CONTROL **RERUN clause** card: CCVS85 leaves it
        // for the installation to fill in, because the clause names an
        // implementor's checkpoint file. Left as the bare word it is neither a
        // RERUN nor anything else, so SQ302M's first `Message expected …
        // OBSOLETE` refers to a construct that is not in the source. Filling it
        // in is what a validating installation does, and it is the only way
        // that expectation can be tested at all. One line in, one line out.
        .replace("XXXXX053", "RERUN ON TFIL EVERY 5000 RECORDS")
}

/// Run one program in its own directory, under both budgets.
///
/// The raw report is returned alongside the score so [`fails_pass`] can read
/// the detail records without running the program a second time.
fn run_one(
    rcrun: &std::path::Path,
    workroot: &std::path::Path,
    members: &[Member],
    name: &str,
    raw: &str,
) -> (RunOutcome, Option<String>) {
    let dir = workroot.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    // Whatever this member says it inherits, written into its directory first.
    run_producers(rcrun, &dir, members, name);
    let r = run_one_in(rcrun, &dir, name, raw);
    // Reclaim the directory — a sweep is hundreds of programs and a runaway
    // print file is measured in gigabytes.
    let _ = std::fs::remove_dir_all(&dir);
    r
}

/// Run one program in an already-chosen directory, leaving it in place.
///
/// Split out from [`run_one`] so a producer can be run into its consumer's
/// directory without that directory being cleared underneath it.
fn run_one_in(
    rcrun: &std::path::Path,
    dir: &std::path::Path,
    name: &str,
    raw: &str,
) -> (RunOutcome, Option<String>) {
    let dir = dir.to_path_buf();
    if std::fs::create_dir_all(&dir).is_err() {
        return (
            RunOutcome::Crash("cannot create the work directory".into()),
            None,
        );
    }
    let src = dir.join(format!("{name}.cbl"));
    if std::fs::write(&src, substitute_implementor_names(raw)).is_err() {
        return (RunOutcome::Crash("cannot write the source".into()), None);
    }

    // Whatever the installation is expected to have on disk before the program
    // starts. Written before the run, never after, so the program sees it on
    // its first OPEN.
    for (fname, content) in installation_data_files(name) {
        if std::fs::write(dir.join(fname), content).is_err() {
            return (
                RunOutcome::Crash(format!("cannot write the {fname} data file")),
                None,
            );
        }
    }

    let console_path = dir.join(CCVS_CONSOLE_FILE);
    let (console, errs) = match (
        std::fs::File::create(&console_path),
        std::fs::File::create(dir.join("rcrun-stderr.txt")),
    ) {
        (Ok(o), Ok(e)) => (o, e),
        _ => {
            return (
                RunOutcome::Crash("cannot create the console files".into()),
                None,
            )
        }
    };
    let deck = operator_input(name);

    let child = std::process::Command::new(rcrun)
        .arg("run")
        .arg(&src)
        .arg("--source-format")
        .arg("fixed")
        // The CCVS85 run instructions require the operator to set external
        // switch 1 ON and switch 2 OFF before the suite runs: NC174A, NC253A
        // and NC254A test `ON STATUS` / `OFF STATUS` against exactly that
        // state and report "SWITCH-1 EXPECTED ON" when it is not set. The
        // suite spells the two switches `XXXXX051` and `XXXXX052`, its
        // placeholders for whatever the implementor calls them; `SWITCH-1` /
        // `SWITCH-2` are set too so a substituted copy of the suite behaves
        // the same. A program that declares no switch ignores all four.
        .arg("--switch")
        .arg("XXXXX051=ON")
        .arg("--switch")
        .arg("XXXXX052=OFF")
        .arg("--switch")
        .arg("SWITCH-1=ON")
        .arg("--switch")
        .arg("SWITCH-2=OFF")
        .current_dir(&dir)
        // A member that reads from the operator gets a pipe it can drain; every
        // other one keeps the closed stdin, so an unexpected `ACCEPT` still
        // returns immediately rather than hanging the sweep.
        .stdin(if deck.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        // Both to files, never to pipes — see `CCVS_CONSOLE_FILE`. `stderr`
        // gets its own file rather than sharing: it carries the runtime's
        // diagnostics, and a diagnostic interleaved into the console transcript
        // could be scored as part of the program's report.
        .stdout(std::process::Stdio::from(console))
        .stderr(std::process::Stdio::from(errs))
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return (RunOutcome::Crash(format!("cannot spawn rcrun: {e}")), None),
    };

    // Hand over the whole deck at once and close the pipe, so an `ACCEPT` past
    // the end of the deck reads end-of-file instead of blocking.
    if let Some(deck) = deck {
        if let Some(mut sink) = child.stdin.take() {
            use std::io::Write;
            let _ = sink.write_all(deck.as_bytes());
        }
    }

    let print_file = dir.join(CCVS_PRINT_FILE);
    let started = std::time::Instant::now();
    let outcome = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    break None; // completed; the report is read below
                }
                break Some(RunOutcome::Crash(format!("exit {status}")));
            }
            Ok(None) => {}
            Err(e) => break Some(RunOutcome::Crash(format!("wait failed: {e}"))),
        }
        let written = std::fs::metadata(&print_file).map(|md| md.len()).unwrap_or(0);
        if written > RUN_MAX_OUTPUT {
            let _ = child.kill();
            let _ = child.wait();
            break Some(RunOutcome::Runaway(written));
        }
        if started.elapsed() > RUN_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            break Some(RunOutcome::Timeout);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };

    let (result, report) = match outcome {
        Some(bad) => (bad, None),
        None => {
            // Read as **bytes**, then decode lossily. A CCVS report is not
            // guaranteed to be UTF-8: NC107A prints the figurative constants,
            // so its report carries real `HIGH-VALUE` (0xFF) and `LOW-VALUE`
            // (0x00) bytes. `read_to_string` rejects that whole file, and the
            // program's 177 passing assertions were scored as "no report
            // printed" — a runtime that faithfully writes the byte it was told
            // to write must not look like a failure here.
            let printed = std::fs::read(&print_file)
                .ok()
                .map(|b| String::from_utf8_lossy(&b).into_owned());
            match printed.as_deref().and_then(score_ccvs_report) {
                Some((p, f, d)) => (RunOutcome::Ran(p, f, d), printed),
                // Nothing in the print file. The member may have reported on
                // the console instead — see [`score_console_report`].
                None => {
                    let console = std::fs::read(&console_path)
                        .map(|b| String::from_utf8_lossy(&b).into_owned())
                        .unwrap_or_default();
                    match score_console_report(name, &console) {
                        Some((p, f, d)) => (RunOutcome::Ran(p, f, d), Some(console)),
                        None => (RunOutcome::NoReport, printed),
                    }
                }
            }
        }
    };
    // Reclaim this program's own artifacts immediately — a sweep is hundreds of
    // programs and a runaway print file is measured in gigabytes — but leave
    // the **data files** in place. They are the module's file chain, and the
    // next member is entitled to find them: removing the directory here is what
    // made a chained program open a file its producer had just written and
    // score the absence as a failure. The whole workroot goes at the end of the
    // pass.
    for transient in [
        src.as_path(),
        print_file.as_path(),
        console_path.as_path(),
        &dir.join("rcrun-stderr.txt"),
    ] {
        let _ = std::fs::remove_file(transient);
    }
    (result, report)
}

/// Count the assertions a CCVS85 program reported.
///
/// Each assertion line carries `P-OR-F`, which the program's own `PASS`,
/// `FAIL` and `DE-LETE` paragraphs set to `PASS `, `FAIL*` or `*****`. Counting
/// those markers is the program's own verdict, not the harness's opinion of it.
///
/// Returns `None` when the output carries no marker at all — the program
/// produced something, but not a CCVS report.
fn score_ccvs_report(report: &str) -> Option<(usize, usize, usize)> {
    // **Not line-based.** The CCVS print file is declared `PIC X(120)` on a
    // record-SEQUENTIAL file, so RustCOBOL writes fixed 120-byte records with
    // no newline between them — the whole report is one very long line. A
    // line-based scorer read that as a single record and scored nothing.
    // Counting the markers across the whole text is independent of how the
    // records happen to be delimited.
    let pass = count_occurrences(report, "PASS ");
    let fail = count_occurrences(report, "FAIL*");
    let deleted = count_occurrences(report, "TEST DELETED");
    if pass + fail + deleted == 0 {
        None
    } else {
        Some((pass, fail, deleted))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Flagging members
// ─────────────────────────────────────────────────────────────────────────────

/// One `*Message expected for … statement: CLASS` comment.
struct Expectation {
    /// 1-based line of the **comment**, not of the construct it refers to.
    line: u32,
    /// The class CCVS85 names, e.g. `OBSOLETE`.
    class: String,
    /// `false` for "above statement", `true` for "following statement".
    following: bool,
}

/// Read a member's flagging contract out of its own comments.
///
/// A flagging member has no PASS/FAIL machinery: it states what the compiler is
/// supposed to say about each construct, in a comment attached to that
/// construct, and ends with `*TOTAL NUMBER OF FLAGS EXPECTED = N.`. The contract
/// is machine-readable, so it is read rather than transcribed.
///
/// **Truncation at column 72 is required, not tidiness.** `NC401M` writes
/// `… statement: NON-CONFORMING STANDARDNC4014.2` — the class runs straight into
/// the identification area with no separator, so reading the untruncated line
/// yields a class name that matches nothing.
fn flagging_expectations(text: &str) -> Vec<Expectation> {
    let mut out = Vec::new();
    for (i, line) in truncate_at_col72(text).lines().enumerate() {
        if line.chars().nth(6) != Some('*') {
            continue; // not a comment line
        }
        let upper = line.to_ascii_uppercase();
        if !upper.contains("MESSAGE EXPECTED FOR") {
            continue;
        }
        let Some((_, class)) = line.split_once(':') else {
            continue;
        };
        out.push(Expectation {
            line: i as u32 + 1,
            class: class.trim().to_ascii_uppercase(),
            following: upper.contains("FOLLOWING STATEMENT"),
        });
    }
    out
}

/// The `*TOTAL NUMBER OF FLAGS EXPECTED = N.` a flagging member declares.
fn declared_flag_total(text: &str) -> Option<usize> {
    for line in truncate_at_col72(text).lines() {
        let upper = line.to_ascii_uppercase();
        let Some(at) = upper.find("TOTAL NUMBER OF FLAGS EXPECTED") else {
            continue;
        };
        let digits: String = line[at..]
            .chars()
            .skip_while(|c| *c != '=')
            .filter(|c| c.is_ascii_digit())
            .collect();
        return digits.parse().ok();
    }
    None
}

/// `true` if this member is scored by what the compiler *says* about it.
fn is_flagging_member(text: &str) -> bool {
    !flagging_expectations(text).is_empty()
}

/// Score a flagging member: its own expectations against what is really flagged.
///
/// Matching is greedy in source order, which is what the comments mean —
/// "for above statement" claims the nearest preceding unclaimed flag of that
/// class, "for following statement" the nearest following one.
///
/// A flag nothing expected counts as a **failure**, not as a free extra. The
/// member declares a total, and a compiler that flags a construct the standard
/// does not list is as wrong as one that stays silent about a construct it does.
fn score_flagging(text: &str) -> (usize, usize, Vec<String>) {
    let expectations = flagging_expectations(text);
    let tokens = tokenize(&substitute_implementor_names(text), SourceFormat::FixedStrict);
    // A member declares which class it is about, so only that analysis is run:
    // `DATE-COMPILED` is both an obsolete element *and* above the high subset,
    // and NC303M and NC401M each want it under their own name. Running both
    // passes would hand every member the other's flags as false positives.
    let wants_subset = expectations
        .iter()
        .any(|e| e.class == FlagClass::NonConforming.ccvs_name());
    let mut flags = if wants_subset {
        cobolt_semantic::flagging::flag_high_subset(&tokens, text)
    } else {
        cobolt_semantic::flagging::flag_obsolete(&tokens)
    };
    flags.sort_by_key(|f| f.line);
    let mut claimed = vec![false; flags.len()];
    let mut pass = 0usize;
    let mut detail: Vec<String> = Vec::new();

    for e in &expectations {
        let hit = (0..flags.len()).find(|&i| {
            !claimed[i]
                && flags[i].class.ccvs_name() == e.class
                && if e.following {
                    flags[i].line > e.line
                } else {
                    flags[i].line < e.line
                }
        });
        match hit {
            Some(i) => {
                claimed[i] = true;
                pass += 1;
            }
            None => detail.push(format!(
                "L{:<5} expected {:<24} nothing flagged",
                e.line, e.class
            )),
        }
    }
    for (i, f) in flags.iter().enumerate() {
        if !claimed[i] {
            detail.push(format!(
                "L{:<5} flagged  {:<24} {} — not expected here",
                f.line,
                f.class.ccvs_name(),
                f.element
            ));
        }
    }
    (pass, detail.len(), detail)
}

/// Both raw lists for one member: `(line, element)` flags and `(line, class)`
/// expectations, each in source order.
fn flagging_detail(text: &str) -> (Vec<(u32, String)>, Vec<(u32, String)>) {
    let tokens = tokenize(&substitute_implementor_names(text), SourceFormat::FixedStrict);
    let expectations = flagging_expectations(text);
    let wants_subset = expectations
        .iter()
        .any(|e| e.class == FlagClass::NonConforming.ccvs_name());
    let mut flags = if wants_subset {
        cobolt_semantic::flagging::flag_high_subset(&tokens, text)
    } else {
        cobolt_semantic::flagging::flag_obsolete(&tokens)
    };
    flags.sort_by_key(|f| f.line);
    (
        flags.into_iter().map(|f| (f.line, f.element.to_string())).collect(),
        expectations
            .into_iter()
            .map(|e| {
                (
                    e.line,
                    if e.following {
                        format!("{} (following)", e.class)
                    } else {
                        e.class
                    },
                )
            })
            .collect(),
    )
}

/// `flag <FILTER>` — what each flagging member expects against what is flagged.
fn flag_pass(members: &[Member], filter: &str) {
    println!("\n=== NIST CCVS85 FLAGGING ===");
    println!("Members whose verdict is the diagnostics the compiler emits.\n");
    let (mut tot_p, mut tot_f) = (0usize, 0usize);
    for m in members.iter().filter(|m| m.kind == "COBOL") {
        if !filter.is_empty() && !m.name.to_uppercase().starts_with(&filter.to_uppercase()) {
            continue;
        }
        if is_out_of_scope(&module_of(&m.name)) || !is_flagging_member(&m.text) {
            continue;
        }
        let (p, f, detail) = score_flagging(&m.text);
        let declared = declared_flag_total(&m.text);
        tot_p += p;
        tot_f += f;
        println!(
            "╔═══ {} — {p} matched, {f} wrong (declares {})",
            m.name,
            declared.map_or("no total".to_string(), |n| n.to_string())
        );
        for d in &detail {
            println!("║ {d}");
        }
        // With a mismatch, the surplus cascades: the matcher is greedy in
        // source order, so one spurious flag early leaves the *last* one
        // unclaimed and the report names a line that is perfectly correct.
        // Seeing every flag beside every expectation is the only way to find
        // the real culprit, so both lists are printed whenever they disagree.
        if f > 0 {
            let (flags, expectations) = flagging_detail(&m.text);
            println!("║ ── flags emitted ──");
            for (l, e) in flags {
                println!("║   L{l:<5} {e}");
            }
            println!("║ ── expectations ──");
            for (l, c) in expectations {
                println!("║   L{l:<5} {c}");
            }
        }
    }
    println!("\n  matched : {tot_p}\n  wrong   : {tot_f}");
}

/// Score a member that reports on the console rather than to `XXXXX055`.
///
/// **The per-member reading comes first, and for `NC110M` it must.** That
/// program states its own contract in the text it prints —
/// `" PERFORM     THIS TEST FAILS UNLESS PASS APPEARS BELOW.   "` — and that
/// sentence contains the literal marker `PASS ` that [`score_ccvs_report`]
/// counts, trailing space and all. Handing the console to the generic scorer
/// first therefore scores the *description* of a test as the *result* of one
/// and calls the program clean before it has run anything.
///
/// `NC110M`'s PROCEDURE DIVISION is, by its own comment, "entirely of paragraph
/// names and DISPLAY literal statements", and it declares two tests:
///
/// * `GO TO       THIS TEST PASSES UNLESS FAIL APPEARS BELOW.`
/// * `PERFORM     THIS TEST FAILS UNLESS PASS APPEARS BELOW.`
///
/// Both are read exactly that way, against the indented one-word detail lines
/// the program writes (`"             PASS"` / `"             FAIL"`), so the
/// sentences above cannot be mistaken for the results below them.
fn score_console_report(name: &str, console: &str) -> Option<(usize, usize, usize)> {
    if name == "NC110M" {
        let detail = |word: &str| console.lines().any(|l| l.trim() == word);
        let go_to_ok = !detail("FAIL"); // "passes unless FAIL appears below"
        let perform_ok = detail("PASS"); // "fails unless PASS appears below"
        let pass = usize::from(go_to_ok) + usize::from(perform_ok);
        return Some((pass, 2 - pass, 0));
    }
    score_ccvs_report(console)
}

/// Count non-overlapping occurrences of `needle`.
fn count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let mut n = 0usize;
    let mut rest = haystack;
    while let Some(i) = rest.find(needle) {
        n += 1;
        rest = &rest[i + needle.len()..];
    }
    n
}

/// Find the `rcrun` binary next to this example.
fn locate_rcrun() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("RCRUN") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    // target/<profile>/examples/nist_conformance → target/<profile>/rcrun
    let exe = std::env::current_exe().ok()?;
    let profile_dir = exe.parent()?.parent()?;
    for candidate in [profile_dir.join("rcrun"), profile_dir.join("rcrun.exe")] {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn report_run(outcomes: &[(String, String, RunOutcome)]) {
    println!("\n=== NIST CCVS85 EXECUTION scoring ===");
    println!("Each program is run and its own PASS/FAIL report is read.\n");

    let mut compile_fail = 0usize;
    let mut ran = 0usize;
    let mut all_pass = 0usize;
    let mut some_fail = 0usize;
    let mut no_report = 0usize;
    let mut timeout = 0usize;
    let mut runaway = 0usize;
    let mut crash = 0usize;
    let (mut tot_p, mut tot_f, mut tot_d) = (0usize, 0usize, 0usize);

    // module → (in scope, executed clean)
    let mut per_module: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut problems: Vec<(&str, &str, &RunOutcome)> = Vec::new();

    for (name, module, o) in outcomes {
        let e = per_module.entry(module.clone()).or_insert((0, 0));
        e.0 += 1;
        match o {
            RunOutcome::CompileFail => compile_fail += 1,
            RunOutcome::Ran(p, f, d) => {
                ran += 1;
                tot_p += p;
                tot_f += f;
                tot_d += d;
                if *f == 0 {
                    all_pass += 1;
                    e.1 += 1;
                } else {
                    some_fail += 1;
                    problems.push((name, module, o));
                }
            }
            RunOutcome::NoReport => {
                no_report += 1;
                problems.push((name, module, o));
            }
            RunOutcome::Timeout => {
                timeout += 1;
                problems.push((name, module, o));
            }
            RunOutcome::Runaway(_) => {
                runaway += 1;
                problems.push((name, module, o));
            }
            RunOutcome::Crash(_) => {
                crash += 1;
                problems.push((name, module, o));
            }
        }
    }
    let in_scope = outcomes.len();

    println!("--- programs ---");
    println!("  in scope                : {in_scope}");
    println!("  did not compile         : {compile_fail}");
    println!("  ran to completion       : {ran}");
    println!("    …reporting 0 failures : {all_pass}   <-- the real conformance figure");
    println!("    …reporting failures   : {some_fail}");
    println!("  ran but printed no report: {no_report}");
    println!("  timed out (>{}s)        : {timeout}", RUN_TIMEOUT.as_secs());
    println!(
        "  runaway output (>{} MB) : {runaway}",
        RUN_MAX_OUTPUT / (1024 * 1024)
    );
    println!("  crashed / refused       : {crash}");

    println!("\n--- assertions, as the programs themselves report ---");
    println!("  PASS    : {tot_p}");
    println!("  FAIL    : {tot_f}");
    println!("  DELETED : {tot_d}");
    let scored = tot_p + tot_f;
    if scored > 0 {
        println!(
            "  rate    : {:.1}% of {} scored assertions",
            100.0 * tot_p as f64 / scored as f64,
            scored
        );
    }

    println!("\n--- per module (executed clean / in scope) ---");
    for (module, (total, clean)) in &per_module {
        println!("  {module:<6} {clean:>4} / {total:<4}");
    }

    if !problems.is_empty() {
        println!("\n--- every program that compiled but did not run clean ---");
        for (name, module, o) in problems.iter().take(80) {
            let what = match o {
                RunOutcome::Ran(p, f, _) => format!("{f} FAIL, {p} PASS"),
                RunOutcome::NoReport => "no CCVS report printed".to_string(),
                RunOutcome::Timeout => format!("timed out after {}s", RUN_TIMEOUT.as_secs()),
                RunOutcome::Runaway(n) => format!("runaway output, {} MB and climbing", n / (1024 * 1024)),
                RunOutcome::Crash(e) => format!("crashed: {e}"),
                RunOutcome::CompileFail => unreachable!(),
            };
            println!("  {name:<9} {module:<5} {what}");
        }
        if problems.len() > 80 {
            println!("  … and {} more", problems.len() - 80);
        }
    }

    println!(
        "\nNOTE: 'ran to completion reporting 0 failures' is the figure that means\n\
         \"this program works\". The compilation score counts programs the front end\n\
         accepts, which is a strictly weaker claim."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `XXXXD001` must be exactly one 120-character record.
    ///
    /// A record one byte long or short still satisfies SQ203A — the two fields
    /// it checks sit in the first 40 bytes, so a trailing stray byte lands past
    /// everything the program reads and the test passes over a malformed file.
    /// Length is therefore checked here rather than inferred from a green run.
    #[test]
    fn sq203a_installation_record_is_one_120_byte_record() {
        assert_eq!(
            SQ203A_XXXXD001.len(),
            120,
            "SQ-FS1 is a fixed 120-byte sequential file; the record must fill it exactly"
        );
        assert!(
            SQ203A_XXXXD001.is_ascii(),
            "a multi-byte character would push every later field out of position"
        );
    }

    /// Each named field sits where the `FILE-RECORD-INFO` skeleton puts it.
    ///
    /// The offsets are recomputed here from the skeleton's own PICTURE widths,
    /// independently of the `concat!` above, so a mis-sized FILLER in the
    /// constant shows up as a field landing in the wrong place instead of
    /// silently shifting its neighbours.
    #[test]
    fn sq203a_installation_record_fields_are_where_the_skeleton_says() {
        let r = SQ203A_XXXXD001;
        for (off, len, want, field) in [
            (5usize, 6usize, "SQ-FS1", "XFILE-NAME"),
            (19, 6, "R1-F-G", "XRECORD-NAME"),
            (26, 1, "1", "REELUNIT-NUMBER"),
            (34, 6, "000001", "XRECORD-NUMBER"),
            (62, 5, "SQ203", "XPROGRAM-NAME"),
            (74, 6, "000120", "XRECORD-LENGTH"),
            (87, 2, "RC", "CHARS-OR-RECORDS"),
            (90, 4, "0001", "XBLOCK-SIZE"),
            (100, 6, "000001", "RECORDS-IN-FILE"),
            (111, 2, "SQ", "XFILE-ORGANIZATION"),
            (119, 1, "S", "XLABEL-TYPE"),
        ] {
            assert_eq!(
                &r[off..off + len],
                want,
                "{field} should occupy bytes {off}..{}",
                off + len
            );
        }
    }

    /// The producer table is opt-in and terminates.
    ///
    /// A cycle would make `run_producers` recurse forever, and a self-reference
    /// would run the member under test as its own prerequisite.
    #[test]
    fn declared_producers_terminate() {
        for consumer in [
            "IX102A", "IX110A", "IX114A", "IX115A", "IX116A", "IX117A", "IX118A", "IX119A",
            "IX120A", "IX202A",
        ] {
            let mut seen = vec![consumer.to_string()];
            let mut at = consumer;
            while let Some(p) = inherits_from(at) {
                assert!(
                    !seen.iter().any(|s| s == p),
                    "{consumer}'s producer chain cycles at {p}"
                );
                seen.push(p.to_string());
                at = p;
                assert!(seen.len() < 16, "{consumer}'s producer chain does not end");
            }
            assert!(seen.len() > 1, "{consumer} should declare a producer");
        }
    }

    /// The producer chain is flat, oldest-first, and excludes the target.
    ///
    /// IX103A inherits from IX102A, which inherits from IX101A, so IX101A must
    /// run first — and all of them in the consumer's own directory. Running a
    /// generation anywhere else left IX102A processing an empty file.
    #[test]
    fn producer_chain_is_oldest_first() {
        assert_eq!(producer_chain("IX103A"), vec!["IX101A", "IX102A"]);
        assert_eq!(producer_chain("IX102A"), vec!["IX101A"]);
        assert_eq!(producer_chain("IX203A"), vec!["IX201A", "IX202A"]);
        assert_eq!(producer_chain("IX110A"), vec!["IX109A"]);
        assert_eq!(producer_chain("IX114A"), vec!["IX113A"]);
        // RL runs the same three-generation shape four times over, so the
        // verifier of each series carries both of its ancestors.
        assert_eq!(producer_chain("RL103A"), vec!["RL101A", "RL102A"]);
        assert_eq!(producer_chain("RL110A"), vec!["RL108A", "RL109A"]);
        assert_eq!(producer_chain("RL203A"), vec!["RL201A", "RL202A"]);
        assert_eq!(producer_chain("RL208A"), vec!["RL206A", "RL207A"]);
        assert_eq!(producer_chain("RL213A"), vec!["RL212A"]);
        // Self-contained members bring nothing with them.
        assert!(producer_chain("IX101A").is_empty());
        assert!(producer_chain("NC101A").is_empty());
        // The target itself is never in its own chain.
        for t in ["IX103A", "IX110A", "IX202A", "RL103A", "RL208A"] {
            assert!(!producer_chain(t).contains(&t), "{t} would run twice");
        }
    }

    /// A member that builds its own file declares no producer.
    ///
    /// IX111A needs `IX-NOP` to be **absent** (it expects status 35), and
    /// IX112A/IX113A create their files themselves — giving any of them a
    /// prerequisite would plant a file the test requires not to be there.
    #[test]
    fn self_contained_members_declare_no_producer() {
        for solo in [
            "IX101A", "IX109A", "IX111A", "IX112A", "IX113A", "IX201A", "IX216A", "SQ203A",
            "NC101A",
            // The creator of each RL series, and RL212A whose consumer needs
            // card 22 to stay absent.
            "RL101A", "RL108A", "RL201A", "RL206A", "RL212A",
        ] {
            assert_eq!(
                inherits_from(solo),
                None,
                "{solo} builds its own files and must run in a clean directory"
            );
        }
    }

    /// An X-card is identified by its number, not the letter in position 5.
    ///
    /// Card 24 appears in the deck as `XXXXX024`, `XXXXP024` and `XXXXD024`.
    /// Left as three names it is three files, and IX102A writing `XXXXP024`
    /// while IX103A reads `XXXXD024` silently breaks a chain the suite's own
    /// header declares.
    #[test]
    fn x_card_letter_variants_collapse_onto_one_card() {
        assert_eq!(canonical_x_cards("XXXXP024"), "XXXXX024");
        assert_eq!(canonical_x_cards("XXXXD024"), "XXXXX024");
        assert_eq!(canonical_x_cards("XXXXX024"), "XXXXX024");
        // Same width, or the sequence area in columns 73-80 slides into the
        // content area of a fixed-format deck.
        assert_eq!(canonical_x_cards("XXXXD001").len(), 8);
        // Untouched: their numbers match no other card, so there is nothing to
        // say they are variants of anything.
        assert_eq!(canonical_x_cards("XXXXY382"), "XXXXY382");
        // Not a card: an identifier that merely ends this way keeps its shape.
        assert_eq!(canonical_x_cards("MY-XXXXP024"), "MY-XXXXP024");
        // In context, with the rest of the line intact.
        assert_eq!(
            canonical_x_cards("     SELECT IX-FS1 ASSIGN XXXXP024   IX1024.2"),
            "     SELECT IX-FS1 ASSIGN XXXXX024   IX1024.2"
        );
    }

    /// `T` and `U` opt-code lines are alternatives; only `T` survives.
    #[test]
    fn opt_code_u_lines_are_commented_out() {
        // The letter becomes `*`, and every other column stays put — these are
        // fixed-format decks.
        let deck = "014400U    05 FILLER             PIC X(24).                             IX1074.2";
        let out = select_opt_t_over_u(deck);
        assert_eq!(&out[6..7], "*", "the U line must become a comment");
        assert_eq!(out.len(), deck.len(), "columns must not shift");
        assert_eq!(&out[7..], &deck[7..], "only column 7 changes");
        // `T` is the surviving alternative and is left exactly as it is.
        let t = "014300T        10 FILLER         PIC X(24).                             IX1074.2";
        assert_eq!(select_opt_t_over_u(t), t);
        // Other opt letters are additions, not alternatives, and are untouched.
        for other in [
            "033500Y    IF RECORD-COUNT GREATER 50                            DB1014.2",
            "005500P    SELECT RAW-DATA   ASSIGN TO                           IX1094.2",
            "007700C    LABEL RECORDS ARE STANDARD                            SQ2034.2",
        ] {
            assert_eq!(select_opt_t_over_u(other), other);
        }
        // A `U` anywhere but the indicator column is ordinary text.
        let word = "001000     MOVE UNIT-COUNT TO X.                                 NC1014.2";
        assert_eq!(select_opt_t_over_u(word), word);
    }

    /// Only the members that need one get a data file.
    #[test]
    fn installation_data_files_are_opt_in() {
        assert_eq!(installation_data_files("SQ203A").len(), 1);
        assert_eq!(installation_data_files("SQ203A")[0].0, "XXXXX001");
        for untouched in ["SQ202A", "NC101A", "IX101A"] {
            assert!(
                installation_data_files(untouched).is_empty(),
                "{untouched} should get no planted files"
            );
        }
    }
}
