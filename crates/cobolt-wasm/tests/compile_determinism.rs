// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 R2 (fix F1): the same program and options give the same
//! module bytes on every compile. The generator walks no unordered map while
//! it emits code; before F1 the ALTER slots were initialised in the order of a
//! `HashMap`, so a program with several `ALTER` targets came out different
//! from one compile to the next.
//!
//! Each program is compiled [`COMPILES`] times in this one process, in both
//! memory variants. Every `HashMap` built along the way gets fresh hash keys,
//! so a walk over one shows up as differing bytes here.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_wasm::{compile_program, Options};
use std::path::{Path, PathBuf};
use std::time::Instant;

const COMPILES: usize = 20;

/// Fixed, so that `FUNCTION WHEN-COMPILED` is not what differs.
const WHEN: &str = "2026111400000000+0000";

/// Eight altered paragraphs, their `ALTER` statements spread over three
/// paragraphs: the generator keeps one slot per altered paragraph.
const ALTER_FIXTURE: &str = "IDENTIFICATION DIVISION.
PROGRAM-ID. ALTER-ORDER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N PIC 9(2) VALUE 0.
PROCEDURE DIVISION.
A-1.
    ALTER G1 TO PROCEED TO T1
    ALTER G2 TO PROCEED TO T2
    ALTER G3 TO PROCEED TO T3.
A-2.
    ALTER G4 TO PROCEED TO T4
    ALTER G5 TO PROCEED TO T5
    ALTER G6 TO PROCEED TO T6.
A-3.
    ALTER G7 TO PROCEED TO T7
    ALTER G8 TO PROCEED TO T8
    GO TO G1.
G1. GO TO DONE.
G2. GO TO DONE.
G3. GO TO DONE.
G4. GO TO DONE.
G5. GO TO DONE.
G6. GO TO DONE.
G7. GO TO DONE.
G8. GO TO DONE.
T1. ADD 1 TO WS-N. GO TO G2.
T2. ADD 1 TO WS-N. GO TO G3.
T3. ADD 1 TO WS-N. GO TO G4.
T4. ADD 1 TO WS-N. GO TO G5.
T5. ADD 1 TO WS-N. GO TO G6.
T6. ADD 1 TO WS-N. GO TO G7.
T7. ADD 1 TO WS-N. GO TO G8.
T8. ADD 1 TO WS-N. GO TO DONE.
DONE.
    DISPLAY \"ALTERED \" WS-N
    STOP RUN.
";

/// Two sort files declaring the same record name; the program releases the
/// first one's record. The file a released record belongs to is found by a
/// walk over a `HashMap` of record names per file (`sort_file_names`), whose
/// answer depends on the map's order only when two files share the name. The
/// generator refuses such a program before that answer is used (the record
/// lookup is ambiguous), so today this fixture checks that the refusal is the
/// same every time; if the program is ever accepted, it checks the bytes.
const SORT_FIXTURE: &str = "IDENTIFICATION DIVISION.
PROGRAM-ID. SORT-DUP.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT WORK-A ASSIGN TO \"WORKA\".
    SELECT WORK-B ASSIGN TO \"WORKB\".
DATA DIVISION.
FILE SECTION.
SD WORK-A.
01 SORT-REC.
   05 S-KEY PIC 9(4).
SD WORK-B.
01 SORT-REC.
   05 S-KEY PIC 9(4).
WORKING-STORAGE SECTION.
01 WS-I PIC 9(4) VALUE 0.
PROCEDURE DIVISION.
MAIN-PARA.
    SORT WORK-A ON ASCENDING KEY S-KEY OF SORT-REC OF WORK-A
        INPUT PROCEDURE IS FEED
        OUTPUT PROCEDURE IS DRAIN
    STOP RUN.
FEED.
    PERFORM VARYING WS-I FROM 3 BY -1 UNTIL WS-I = 0
        MOVE WS-I TO S-KEY OF SORT-REC OF WORK-A
        RELEASE SORT-REC OF WORK-A
    END-PERFORM.
DRAIN.
    RETURN WORK-A AT END CONTINUE END-RETURN.
";

fn programs(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            programs(&p, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("cbl")) {
            out.push(p);
        }
    }
}

/// FNV-1a over the bytes: a stable name for a module in the result block.
fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The program's AST, or why it is not compiled here.
fn parse(src: &str, path: Option<&Path>) -> Result<cobolt_ast::program::Program, String> {
    let src = match path.and_then(|p| cobolt_lexer::preprocess_program(src, p, SourceFormat::Free)) {
        Some(x) if !x.errors.is_empty() => return Err(format!("COPY: {}", x.errors[0])),
        Some(x) => x.text,
        None => src.to_string(),
    };
    let parsed = cobolt_parser::parse(tokenize(&src, SourceFormat::Free));
    if let Some(d) = parsed.diagnostics.iter().find(|d| d.is_error()) {
        return Err(format!("does not parse: line {}: {}", d.span.line, d.message));
    }
    parsed.program.ok_or_else(|| "no program".to_string())
}

/// What one compile gave: the module's bytes, or the refusal's line and text.
fn outcome(prog: &cobolt_ast::program::Program, opts: &Options) -> Result<Vec<u8>, String> {
    compile_program(prog, opts).map(|c| c.wasm).map_err(|e| format!("line {}: {}", e.line, e.message))
}

/// Compiles `prog` [`COMPILES`] times in one memory variant. Every compile
/// must give the same outcome, a refusal included. `Ok(Ok(hash))` for
/// identical bytes, `Ok(Err(refusal))` for the same refusal every time, `Err`
/// naming the first compile that differed.
fn same_bytes(prog: &cobolt_ast::program::Program, multi_memory: bool) -> Result<Result<u64, String>, String> {
    let opts = Options { multi_memory, when_compiled: Some(WHEN.into()), ..Options::default() };
    let first = outcome(prog, &opts);
    for n in 2..=COMPILES {
        let again = outcome(prog, &opts);
        if again != first {
            let says = |o: &Result<Vec<u8>, String>| match o {
                Ok(b) => format!("{} bytes", b.len()),
                Err(e) => format!("refused, {e}"),
            };
            return Err(format!("compile {n} of {COMPILES} differs ({} vs {})", says(&again), says(&first)));
        }
    }
    Ok(first.map(|b| fnv(&b)))
}

#[test]
fn same_input_same_bytes() {
    let started = Instant::now();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol");
    let mut files = Vec::new();
    programs(&root, &mut files);

    let mut cases: Vec<(String, cobolt_ast::program::Program)> = Vec::new();
    cases.push(("fixture: eight ALTER targets".into(), parse(ALTER_FIXTURE, None).expect("the ALTER fixture parses")));
    cases.push(("fixture: one record name in two sort files".into(), parse(SORT_FIXTURE, None).expect("the SORT fixture parses")));
    let fixtures = cases.len();
    let mut not_parsed = Vec::new();
    for f in &files {
        let rel = f.strip_prefix(&root).unwrap_or(f).display().to_string().replace('\\', "/");
        let src = String::from_utf8_lossy(&std::fs::read(f).unwrap()).into_owned();
        match parse(&src, Some(f)) {
            Ok(p) => cases.push((rel, p)),
            Err(why) => not_parsed.push((rel, why)),
        }
    }

    let (mut identical, mut refused, mut compiles) = (0usize, 0usize, 0usize);
    let mut differing = Vec::new();
    let mut hashes = Vec::new();
    for (name, prog) in &cases {
        let mut row = Vec::new();
        for multi in [true, false] {
            compiles += COMPILES;
            match same_bytes(prog, multi) {
                Ok(Err(why)) => {
                    refused += 1;
                    let short: String = why.chars().take(90).collect();
                    row.push(format!("refused ({short})"));
                }
                Ok(Ok(h)) => {
                    identical += 1;
                    row.push(format!("{h:016x}"));
                }
                Err(why) => {
                    differing.push(format!("{name} [{}]: {why}", if multi { "two memories" } else { "one memory" }));
                    row.push("DIFFERS".to_string());
                }
            }
        }
        hashes.push(format!("  {name}: {}", row.join(" / ")));
    }
    let ms = started.elapsed().as_millis();

    println!("== same_input_same_bytes (DESKTOP-WASM-001 R2, fix F1) ==");
    println!("programs: {} ({} from tests/cobol, {fixtures} fixtures); {} not parsed", cases.len(), cases.len() - fixtures, not_parsed.len());
    println!("variants: two memories and one memory; {COMPILES} compiles each");
    println!("module hashes (FNV-1a, two memories / one memory):");
    for h in &hashes {
        println!("{h}");
    }
    for (n, why) in &not_parsed {
        println!("  not parsed: {n}: {why}");
    }
    println!(
        "variants: {}; identical bytes {identical}; the same refusal every time {refused}; differing {}",
        identical + refused + differing.len(),
        differing.len()
    );
    println!("compiles: {compiles} in {ms} ms ({:.0} compiles/second)", compiles as f64 / (ms.max(1) as f64 / 1000.0));
    println!("result: {}", if differing.is_empty() { "PASS" } else { "FAIL" });
    assert!(differing.is_empty(), "module bytes changed between compiles:\n{}", differing.join("\n"));
    assert!(!hashes[0].contains("refused"), "the ALTER fixture must compile: {}", hashes[0]);
}
