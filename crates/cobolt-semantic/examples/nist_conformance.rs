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

        eprintln!("[{total:>3}/459] {} ({} lines)", m.name, text.lines().count());
        let t0 = std::time::Instant::now();
        let tokens = tokenize(&text, SourceFormat::FixedStrict);
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

    println!("--- first failing diagnostic per program (first 60) ---");
    for (name, p, s, first) in failures.iter().take(60) {
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
            if shown.len() >= 4 {
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
        if is_out_of_scope(&module) {
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

        let outcome = run_one(&rcrun, &workroot, &m.name, &m.text);
        outcomes.push((m.name.clone(), module, outcome));
    }

    let _ = std::fs::remove_dir_all(&workroot);
    report_run(&outcomes);
}

/// Run one program in its own directory, under both budgets.
fn run_one(
    rcrun: &std::path::Path,
    workroot: &std::path::Path,
    name: &str,
    raw: &str,
) -> RunOutcome {
    let dir = workroot.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    if std::fs::create_dir_all(&dir).is_err() {
        return RunOutcome::Crash("cannot create the work directory".into());
    }
    let src = dir.join(format!("{name}.cbl"));
    if std::fs::write(&src, raw).is_err() {
        return RunOutcome::Crash("cannot write the source".into());
    }

    let child = std::process::Command::new(rcrun)
        .arg("run")
        .arg(&src)
        .arg("--source-format")
        .arg("fixed")
        .current_dir(&dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return RunOutcome::Crash(format!("cannot spawn rcrun: {e}")),
    };

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

    let result = match outcome {
        Some(bad) => bad,
        None => match std::fs::read_to_string(&print_file) {
            Ok(report) => match score_ccvs_report(&report) {
                Some((p, f, d)) => RunOutcome::Ran(p, f, d),
                None => RunOutcome::NoReport,
            },
            Err(_) => RunOutcome::NoReport,
        },
    };
    // Reclaim the directory immediately — a sweep is hundreds of programs and
    // a runaway print file is measured in gigabytes.
    let _ = std::fs::remove_dir_all(&dir);
    result
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
