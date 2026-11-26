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

/// DESKTOP-WASM-001 T11: a program whose module carries the time of its Build.
const STAMP_FIXTURE: &str = "IDENTIFICATION DIVISION.
PROGRAM-ID. STAMP.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-STAMP PIC X(21).
PROCEDURE DIVISION.
    MOVE FUNCTION WHEN-COMPILED TO WS-STAMP.
    DISPLAY WS-STAMP.
    STOP RUN.
";

/// 2025-11-14 00:00:00 UTC, and the next day.
const EPOCH: &str = "1763078400";
const EPOCH_NEXT_DAY: &str = "1763164800";

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

/// DESKTOP-WASM-001 T11 (R2, C3): `FUNCTION WHEN-COMPILED` and the paths of the line table are
/// inputs of the Build. With the same `SOURCE_DATE_EPOCH`, a program that uses WHEN-COMPILED gives
/// the same bytes whether its Common Code path was written on Windows (`src\common\stamp.cbl`) or
/// on macOS and Linux (`src/common/stamp.cbl`); the module answers the epoch's time; another epoch
/// gives other bytes; a malformed epoch and an absolute path are refused. This is the only test of
/// the binary that sets `SOURCE_DATE_EPOCH`; the others name their WHEN-COMPILED text.
#[test]
fn source_date_epoch_and_separators_are_build_inputs() {
    use cobolt_wasm::module::{build_time, compile_project, FormSpec, SourceLine, SOURCE_DATE_EPOCH};
    let started = Instant::now();
    let prog = parse(STAMP_FIXTURE, None).expect("the WHEN-COMPILED fixture parses");
    // Generated lines 7 to 9 are the three statements, written in a Common Code file.
    let spec = |site: &str| FormSpec {
        name: "STAMP".into(),
        main: true,
        instances: 1,
        program: &prog,
        controls: vec![],
        bindings: vec![],
        layers: vec![],
        lines: (7..=9).map(|g| SourceLine { gen_line: g, site: site.into(), line: g - 6, column: 0 }).collect(),
    };
    let (windows, unix) = (spec(r"src\common\stamp.cbl"), spec("src/common/stamp.cbl"));
    let build = |s: &FormSpec, multi: bool| compile_project(std::slice::from_ref(s), &Options { multi_memory: multi, ..Options::default() });

    std::env::set_var(SOURCE_DATE_EPOCH, EPOCH);
    let (mut same, mut sites, mut site_rows) = (0, 0, 0);
    let mut sizes = Vec::new();
    for multi in [true, false] {
        let a = build(&windows, multi).expect("the fixture compiles");
        let b = build(&unix, multi).expect("the fixture compiles");
        if a.wasm == b.wasm {
            same += 1;
        }
        sizes.push(a.wasm.len());
        for e in a.lines.iter().chain(b.lines.iter()).filter(|e| e.line > 0 && e.site.contains("stamp")) {
            site_rows += 1;
            if e.site == "src/common/stamp.cbl" {
                sites += 1;
            }
        }
    }
    let program = compile_program(&prog, &Options::default()).expect("the fixture compiles");
    let run = cobolt_wasm::host::run_program(&program.wasm).expect("the fixture runs");
    let stamp = run.output.lines().next().unwrap_or_default().to_string();
    let first = build(&unix, true).unwrap().wasm;
    std::env::set_var(SOURCE_DATE_EPOCH, EPOCH_NEXT_DAY);
    let next_day = build(&unix, true).unwrap().wasm;
    std::env::set_var(SOURCE_DATE_EPOCH, "tomorrow");
    let malformed = build(&unix, true).err().map(|e| e.message).unwrap_or_default();
    std::env::remove_var(SOURCE_DATE_EPOCH);
    let absolute: Vec<String> = ["/Users/dev/project/src/stamp.cbl", r"C:\work\project\src\stamp.cbl"]
        .iter()
        .filter_map(|p| build(&spec(p), true).err().map(|e| e.message))
        .collect();
    let clock = build_time(None).expect("the clock is always a time");
    // 9999-12-31 23:59:59 UTC in seconds, the last second a four-digit year spells.
    let last_second: u64 = 253_402_300_799;
    let year_9999 = build_time(Some(&last_second.to_string())).expect("the last second WHEN-COMPILED spells");
    let past_9999 = build_time(Some(&(last_second + 1).to_string())).is_err();

    let checks = [
        ("same bytes from both separators, two memories and one", same == 2),
        ("every Common Code row of prlines written with /", site_rows > 0 && sites == site_rows),
        ("WHEN-COMPILED answers the epoch's time", stamp == "2025111400000000+0000"),
        ("another epoch gives other bytes", next_day != first),
        ("a malformed epoch is refused, naming it", malformed.contains(SOURCE_DATE_EPOCH) && malformed.contains("tomorrow")),
        ("an absolute path is refused, both kinds", absolute.len() == 2 && absolute.iter().all(|m| m.contains("relative to the project"))),
        ("unset, the Build's clock (21 characters)", clock.len() == 21),
        ("the year 9999 spelled, the year 10000 refused", year_9999 == "9999123123595900+0000" && past_9999),
    ];
    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== source_date_epoch_and_separators_are_build_inputs (DESKTOP-WASM-001 T11, R2, C3) ==");
    println!("fixture: MOVE FUNCTION WHEN-COMPILED, its three statements mapped to a Common Code file");
    println!("SOURCE_DATE_EPOCH: {EPOCH} (2025-11-14 00:00 UTC), {EPOCH_NEXT_DAY}, \"tomorrow\", unset");
    println!("module sizes (two memories / one memory): {} / {} bytes", sizes[0], sizes[1]);
    println!("prlines rows naming the file: {site_rows}, written with /: {sites}");
    println!("WHEN-COMPILED: {stamp:?}; unset, the clock gave {clock:?}");
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("checks: {passed} of {} in {} ms", checks.len(), started.elapsed().as_millis());
    println!("result: {}", if passed == checks.len() { "PASS" } else { "FAIL" });
    assert_eq!(passed, checks.len(), "{checks:#?}\nmalformed: {malformed}\nabsolute: {absolute:?}\nyear 9999: {year_9999}");
}

/// Spec 114 T14: a Backend module is built from the same walks, so the same programs and options give the same
/// bytes on every compile, in both memory variants. Three programs, each nested in the others as
/// `cobolt_server::Backend::build` nests them: a record with a table that depends on a count, a plain adder and a
/// caller, so the entry glue, the descriptors and the nested copies all have an order a map could disturb.
#[test]
fn a_backend_module_has_the_same_bytes_every_compile() {
    use cobolt_wasm::{compile_backend, BackendProgram};
    const SOURCES: [&str; 3] = [
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. ORDERS.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-I PIC 9(2).\nLINKAGE SECTION.\n01 REQ.\n    05 CNT PIC 9(2).\n    05 LINE-ITEM OCCURS 1 TO 5 DEPENDING ON CNT.\n        10 SKU PIC X(6).\n        10 QTY PIC 9(3).\n01 RES.\n    05 TOTAL PIC 9(5).\nPROCEDURE DIVISION USING REQ RETURNING RES.\nMAIN-PARA.\n    MOVE 0 TO TOTAL\n    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > CNT\n        ADD QTY (WS-I) TO TOTAL\n    END-PERFORM\n    GOBACK.\n",
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. ADDER.\nDATA DIVISION.\nLINKAGE SECTION.\n01 A PIC 9(4).\n01 B PIC 9(4).\n01 S PIC 9(5).\nPROCEDURE DIVISION USING A B S.\nMAIN-PARA.\n    COMPUTE S = A + B.\n    GOBACK.\n",
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. CALLER.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 W PIC 9(5).\nLINKAGE SECTION.\n01 X PIC 9(4).\n01 Y PIC 9(5).\nPROCEDURE DIVISION USING X Y.\nMAIN-PARA.\n    CALL \"ADDER\" USING X X W.\n    MOVE W TO Y.\n    GOBACK.\n",
    ];
    let started = Instant::now();
    let programs: Vec<cobolt_ast::program::Program> = SOURCES.iter().map(|s| parse(s, None).expect("the Backend fixture parses")).collect();
    let tops: Vec<cobolt_ast::program::Program> = programs
        .iter()
        .map(|p| {
            let mut top = p.clone();
            for other in &programs {
                if other.identification.program_id != p.identification.program_id {
                    top.nested_programs.push(other.clone());
                }
            }
            top
        })
        .collect();
    let list: Vec<BackendProgram<'_>> = tops.iter().map(|t| BackendProgram { name: t.identification.program_id.to_ascii_uppercase(), program: t }).collect();
    let mut sizes = Vec::new();
    let mut compiles = 0usize;
    for multi_memory in [true, false] {
        let opts = Options { multi_memory, native: true, when_compiled: Some(WHEN.into()), ..Options::default() };
        let first = compile_backend(&list, &opts).unwrap_or_else(|e| panic!("the Backend fixture compiles: {e}")).wasm;
        for n in 2..=COMPILES {
            let again = compile_backend(&list, &opts).unwrap_or_else(|e| panic!("compile {n}: {e}")).wasm;
            assert!(again == first, "compile {n} of {COMPILES} differs from the first ({} vs {} bytes), multi_memory={multi_memory}", again.len(), first.len());
        }
        compiles += COMPILES;
        sizes.push((multi_memory, first.len(), fnv(&first)));
    }
    let ms = started.elapsed().as_millis();
    println!("== a_backend_module_has_the_same_bytes_every_compile (spec 114 T14) ==");
    println!("programs: {} nested in each other; variants: two memories and one memory; {COMPILES} compiles each", programs.len());
    for (m, n, h) in &sizes {
        println!("  {}: {n} bytes, FNV-1a {h:016x}", if *m { "two memories" } else { "one memory" });
    }
    println!("compiles: {compiles} in {ms} ms ({:.0} compiles/second); differing 0", compiles as f64 / (ms.max(1) as f64 / 1000.0));
    println!("result: PASS");
}
