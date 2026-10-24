// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The subset table (WASM-010 R7, R8, AC6): every built-in of the interpreter
//! is classified for a browser module, and what is outside the subset is
//! refused at its line with the alternative.

mod common;

use cobolt_wasm::support::{builtin_available, builtin_entry, builtin_support, Support, BUILTIN_TABLE};
use cobolt_wasm::{compile_program, ErrorKind, Options};

/// The names that have no row in the table.
fn unclassified<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    names.into_iter().filter(|n| builtin_entry(n).is_none()).collect()
}

#[test]
fn every_builtin_of_the_interpreter_is_classified() {
    let names: Vec<&str> = cobolt_runtime::builtins::BUILTINS.iter().map(|b| b.name).collect();
    let missing = unclassified(names.iter().copied());
    let (mut module, mut backend, mut unsupported) = (0, 0, 0);
    for n in &names {
        match builtin_support(n) {
            Support::Module | Support::Conformance => module += 1,
            Support::BackendOnly(_) | Support::MemoryFiles(_) => backend += 1,
            Support::Unsupported(_) => unsupported += 1,
        }
    }
    eprintln!(
        "subset table: {} of {} classified ({module} browser, {backend} backend only, {unsupported} unsupported)",
        names.len() - missing.len(),
        names.len()
    );
    assert!(missing.is_empty(), "built-ins with no entry in cobolt_wasm::support::BUILTIN_TABLE: {missing:?}");
    // And no row names a built-in that does not exist.
    let stale: Vec<&str> = BUILTIN_TABLE.iter().map(|b| b.name).filter(|n| !names.contains(n)).collect();
    assert!(stale.is_empty(), "rows for built-ins the interpreter does not have: {stale:?}");
    assert_eq!(BUILTIN_TABLE.len(), names.len(), "one row per built-in");
}

#[test]
fn a_planted_unknown_builtin_is_caught() {
    let names = ["OPEN-DB", "NO-SUCH-BUILTIN"];
    assert_eq!(unclassified(names), vec!["NO-SUCH-BUILTIN"]);
    assert!(matches!(builtin_support("NO-SUCH-BUILTIN"), Support::Unsupported(_)));
}

#[test]
fn engine_columns_follow_the_q7_rule() {
    // Every browser entry that names an API is in both engines, or it would not be
    // available outside iOS.
    for b in BUILTIN_TABLE {
        if let (Support::Module, Some((api, e))) = (b.support, b.api) {
            assert!(e.safari, "{} needs {api}, which an iOS build reads from Safari", b.name);
            assert!(builtin_available(b.name, true) == e.safari && builtin_available(b.name, false) == (e.chrome && e.safari));
        }
    }
    assert!(builtin_available("AUDIO-PLAY", false) && builtin_available("AUDIO-PLAY", true));
    assert!(!builtin_available("OPEN-DB", false) && !builtin_available("FOLDER-DIALOG", false));
}

/// AC6: a file verb, `EXEC SQL`, `EXEC RUST` and a backend built-in each fail at
/// their line, naming the construct and the alternative; each refusal carries
/// its kind (T5, D6), and so does the limit on host reads.
#[test]
fn constructs_outside_the_subset_are_refused_at_their_line_with_the_alternative() {
    let cases: &[(&str, u32, ErrorKind, &[&str])] = &[
        (
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. F1.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT IN-FILE ASSIGN TO \"in.txt\".\nDATA DIVISION.\nFILE SECTION.\nFD IN-FILE.\n01 IN-REC PIC X(10).\nPROCEDURE DIVISION.\n    OPEN INPUT IN-FILE.\n    STOP RUN.\n",
            12,
            ErrorKind::BackendOnly,
            &["browser module", "backend", "CALL … WITH WEBSOCKET LINKAGE"],
        ),
        (
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. S1.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 9(4).\nPROCEDURE DIVISION.\n    EXEC SQL SELECT COUNT(*) INTO :N FROM T END-EXEC.\n    STOP RUN.\n",
            7,
            ErrorKind::BackendOnly,
            &["EXEC SQL", "backend", "CALL … WITH WEBSOCKET LINKAGE"],
        ),
        (
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. R1.\nPROCEDURE DIVISION.\n    EXEC RUST\n        println!(\"hi\");\n    END-EXEC.\n    STOP RUN.\n",
            4,
            ErrorKind::Unsupported,
            &["EXEC RUST", "native build", "backend"],
        ),
        (
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. B1.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 H PIC 9(4).\n01 ST PIC X(10).\nPROCEDURE DIVISION.\n    CALL \"COBOL-OPEN-DB\" USING \"x.db\" H ST.\n    STOP RUN.\n",
            8,
            ErrorKind::BackendOnly,
            &["OPEN-DB", "backend", "CALL … WITH WEBSOCKET LINKAGE"],
        ),
        (
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. T1.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A PIC X(3) VALUE \"abc\".\nPROCEDURE DIVISION.\n    DISPLAY FUNCTION UPPER-CASE(A) FUNCTION UPPER-CASE(A) FUNCTION UPPER-CASE(A)\n            FUNCTION UPPER-CASE(A) FUNCTION UPPER-CASE(A).\n    STOP RUN.\n",
            // the line of the fifth function, where the limit is crossed
            8,
            ErrorKind::TooManyHostReads,
            &["more than four", "split it"],
        ),
    ];
    let mut refused = 0;
    for (src, line, kind, words) in cases {
        let prog = common::parse(src);
        let err = match compile_program(&prog, &Options::default()) {
            Ok(_) => panic!("compiled, but should be refused:\n{src}"),
            Err(e) => e,
        };
        let all = err.all_kinded();
        let hit = all.iter().find(|e| e.line == *line).unwrap_or_else(|| panic!("no diagnostic at line {line}: {all:?}"));
        for w in *words {
            assert!(hit.message.contains(w), "line {line}: {:?} lacks {w:?}", hit.message);
        }
        assert_eq!(hit.kind, *kind, "line {line}: {}", hit.message);
        assert!(!hit.subject.is_empty(), "line {line}: a kinded refusal names its subject");
        refused += 1;
    }
    eprintln!("subset refusals: {refused} of {} constructs refused at their line with the alternative", cases.len());
}

/// Q9: `ACCEPT` from the operator is ignored in a browser module, with one
/// warning at its line; the item keeps its value; `FROM DATE` still reads the
/// host clock and warns about nothing.
#[test]
fn accept_from_the_operator_is_ignored_with_a_warning() {
    let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. A1.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 X PIC X(4) VALUE \"KEEP\".\n01 D PIC 9(6).\nPROCEDURE DIVISION.\n    ACCEPT X.\n    ACCEPT D FROM DATE.\n    DISPLAY X.\n    STOP RUN.\n";
    for multi in [true, false] {
        let opts = Options { multi_memory: multi, ..Options::default() };
        let c = common::compile(src, &opts);
        assert_eq!(c.warnings.len(), 1, "{:?}", c.warnings);
        let w = &c.warnings[0];
        assert_eq!((w.line, w.kind), (8, cobolt_wasm::WarningKind::AcceptIgnored), "{w}");
        assert!(w.message.contains("ignored") && w.message.contains("keeps its value"), "{w}");
        let o = cobolt_wasm::host::run_program(&c.wasm).unwrap();
        assert!(o.trapped.is_none() && o.errors.is_empty(), "{o:?}");
        assert_eq!(common::wasm_lines(&o), vec!["KEEP".to_string()], "multi={multi}");
    }
    eprintln!("accept: 2 variants, 1 warning each, the item kept its value");
}

/// D7: the statement table the System KB is written from has one row per
/// statement name `stmt_name` gives (read from its source, so a new `Stmt`
/// variant with a name and no row fails here), and agrees with the exhaustive
/// `support()` on every statement of the sample programs.
#[test]
fn the_statement_table_is_the_support_match() {
    use cobolt_wasm::support::{statement_entry, stmt_name, support, STATEMENT_TABLE};
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/support.rs")).unwrap();
    let body = src.split("pub fn stmt_name").nth(1).unwrap().split("\n}\n").next().unwrap();
    let names: Vec<&str> = body.lines().filter_map(|l| l.split("=> \"").nth(1)).map(|r| r.trim_end_matches("\",")).collect();
    assert!(names.len() > 40, "{names:?}");
    for n in &names {
        assert!(statement_entry(n).is_some(), "statement {n} has no row in STATEMENT_TABLE");
    }
    assert_eq!(STATEMENT_TABLE.len(), names.len(), "a row names no statement");
    // Agreement with support(), statement by statement.
    let programs = [
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. S1.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT F ASSIGN TO \"f\" ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC RECORD KEY IS K STORAGE IS MEMORY.\n    SELECT W ASSIGN TO \"w\".\n\
         DATA DIVISION.\nFILE SECTION.\nFD F.\n01 R.\n    05 K PIC 9(4).\nSD W.\n01 WR PIC X(4).\nWORKING-STORAGE SECTION.\n01 A PIC 9(4).\n01 B PIC 9(4).\n01 T PIC X(10).\n01 G1.\n    05 A1 PIC 9.\n01 G2.\n    05 A1 PIC 9.\n01 TB.\n    05 E PIC 9 OCCURS 3 INDEXED BY IX.\n\
         PROCEDURE DIVISION.\nP1.\n    MOVE 1 TO A.\n    ADD 1 TO A.\n    SUBTRACT 1 FROM A.\n    MULTIPLY 2 BY A.\n    DIVIDE 2 INTO A.\n    COMPUTE A = A + 1.\n\
         \x20   ADD CORRESPONDING G1 TO G2.\n    SUBTRACT CORRESPONDING G1 FROM G2.\n    MOVE CORRESPONDING G1 TO G2.\n    IF A = 1 CONTINUE END-IF.\n    EVALUATE A WHEN 1 CONTINUE END-EVALUATE.\n    PERFORM P2.\n    INITIALIZE A.\n\
         \x20   STRING \"A\" DELIMITED BY SIZE INTO T.\n    UNSTRING T INTO B.\n    INSPECT T TALLYING A FOR ALL \"A\".\n    SEARCH E AT END CONTINUE WHEN E (IX) = 1 CONTINUE END-SEARCH.\n    ALTER P2 TO PROCEED TO P3.\n    GO TO P2 DEPENDING ON A.\n\
         \x20   OPEN I-O F.\n    READ F.\n    WRITE R.\n    REWRITE R.\n    DELETE F.\n    START F.\n    CLOSE F.\n    DISPLAY A.\n    ACCEPT A.\n    CALL \"X\".\n    CANCEL \"X\".\n\
         \x20   SORT W ON ASCENDING KEY WR INPUT PROCEDURE P2 OUTPUT PROCEDURE P3.\n    GO TO P3.\nP2.\n    RELEASE WR.\n    NEXT SENTENCE.\nP3.\n    RETURN W AT END CONTINUE END-RETURN.\n    EXIT.\nP4.\n    STOP RUN.\n",
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. S2.\nPROCEDURE DIVISION.\n    EXEC SQL COMMIT END-EXEC.\n    EXEC RUST\n        let x = 1;\n    END-EXEC.\n    GOBACK.\n",
    ];
    let mut checked = 0;
    let mut seen = std::collections::BTreeSet::new();
    for p in programs {
        let prog = common::parse(p);
        let cobolt_ast::program::ProcedureBody::Paragraphs(ps) = &prog.procedure.body else { panic!("paragraphs") };
        for para in ps {
            for s in &para.stmts {
                s.walk(&mut |s| {
                    let name = stmt_name(s);
                    assert_eq!(statement_entry(name), Some(support(s)), "{name}");
                    seen.insert(name);
                    checked += 1;
                });
            }
        }
    }
    eprintln!("statement table: {} rows, one per statement name; {checked} parsed statements ({} kinds) agree with support(): {seen:?}", STATEMENT_TABLE.len(), seen.len());
}
