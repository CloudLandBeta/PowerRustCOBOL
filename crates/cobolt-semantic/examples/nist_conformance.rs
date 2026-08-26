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
