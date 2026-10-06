// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 077 T3 — national and UTF-8 declarations, literals and INITIALIZE
//! categories, as the parser builds them.

use cobolt_ast::data::{DataDecl, PicKind, Usage};
use cobolt_ast::expr::Literal;
use cobolt_ast::program::DataSection;
use cobolt_ast::stmt::{InitCategory, Stmt};
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};

fn prog(ws: &str, proc_: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{ws}\nPROCEDURE DIVISION.\nMAIN-PARA.\n{proc_}\n    STOP RUN.\n"
    )
}

fn parse_all(src: &str) -> (Option<cobolt_ast::program::Program>, Vec<(u32, String)>) {
    let r = parse(tokenize(src, SourceFormat::Free));
    let errors = r.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| (d.span.line, d.message.clone())).collect();
    (r.program, errors)
}

fn item(src: &str, name: &str) -> DataDecl {
    let (p, errors) = parse_all(src);
    assert!(errors.is_empty(), "{errors:?}");
    fn walk<'a>(d: &'a DataDecl, name: &str) -> Option<&'a DataDecl> {
        if d.name.as_deref() == Some(name) {
            return Some(d);
        }
        d.children.iter().find_map(|c| walk(c, name))
    }
    let p = p.unwrap();
    for sec in &p.data.as_ref().unwrap().sections {
        if let DataSection::WorkingStorage(decls) = sec {
            if let Some(d) = decls.iter().find_map(|d| walk(d, name)) {
                return d.clone();
            }
        }
    }
    panic!("no item {name}")
}

fn kind(d: &DataDecl) -> (PicKind, u16, Usage) {
    let p = d.picture.as_ref().expect("a picture");
    (p.kind, p.digits, d.usage)
}

#[test]
fn national_declarations() {
    let src = prog(
        "01 WS-A PIC N(30).\n01 WS-B PIC NNN USAGE NATIONAL.\n01 WS-C PIC N(5) USAGE IS NATIONAL VALUE N\"Ação\".\n01 WS-D PIC N(4) NATIONAL.\n",
        "",
    );
    assert_eq!(kind(&item(&src, "WS-A")), (PicKind::National, 30, Usage::National), "PIC N implies NATIONAL");
    assert_eq!(kind(&item(&src, "WS-B")), (PicKind::National, 3, Usage::National));
    let c = item(&src, "WS-C");
    assert_eq!(kind(&c), (PicKind::National, 5, Usage::National));
    assert_eq!(c.value, Some(Literal::National("Ação".into())));
    assert_eq!(kind(&item(&src, "WS-D")).2, Usage::National, "NATIONAL without the word USAGE");
}

#[test]
fn utf8_declarations() {
    let src = prog(
        "01 WS-U PIC U(5) VALUE U\"Ação!\".\n01 WS-V PIC U USAGE UTF-8 BYTE-LENGTH 10.\n01 WS-W PIC U(2) UTF-8.\n01 WS-X PIC U(3) VALUE UX\"C3A7\".\n",
        "",
    );
    let u = item(&src, "WS-U");
    assert_eq!(kind(&u), (PicKind::Utf8, 5, Usage::Utf8));
    assert_eq!(u.value, Some(Literal::Utf8("Ação!".into())));
    let v = item(&src, "WS-V");
    assert_eq!((kind(&v), v.byte_length), ((PicKind::Utf8, 1, Usage::Utf8), Some(10)));
    assert_eq!(kind(&item(&src, "WS-W")).2, Usage::Utf8);
    assert_eq!(item(&src, "WS-X").value, Some(Literal::Utf8("ç".into())));
}

/// What Check must later refuse still parses into what it says, so the
/// diagnostic can name it: national numeric keeps both facts.
#[test]
fn national_numeric_keeps_its_picture_and_usage() {
    let src = prog("01 WS-N PIC 9(5) USAGE NATIONAL.\n", "");
    assert_eq!(kind(&item(&src, "WS-N")), (PicKind::Numeric, 5, Usage::National));
}

#[test]
fn initialize_replacing_national_and_utf8() {
    let src = prog("01 WS-A PIC N(3).\n01 WS-U PIC U(3).\n", "    INITIALIZE WS-A WS-U REPLACING NATIONAL DATA BY N\"ab\" UTF-8 BY U\"cd\"");
    let (p, errors) = parse_all(&src);
    assert!(errors.is_empty(), "{errors:?}");
    let p = p.unwrap();
    let mut found = None;
    let cobolt_ast::program::ProcedureBody::Paragraphs(paras) = &p.procedure.body else { panic!("paragraphs") };
    for para in paras {
        for s in &para.stmts {
            if let Stmt::Initialize { replacing, .. } = s {
                found = Some(replacing.iter().map(|(c, _)| *c).collect::<Vec<_>>());
            }
        }
    }
    assert_eq!(found, Some(vec![InitCategory::National, InitCategory::Utf8]));
}

#[test]
fn a_malformed_literal_is_one_error_on_its_line() {
    let src = prog("01 WS-A PIC N(3).\n01 WS-U PIC U(3).", "    MOVE NX\"0041006\" TO WS-A\n    MOVE UX\"C3\" TO WS-U");
    let (_, errors) = parse_all(&src);
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert_eq!(errors[0].0, 9);
    assert!(errors[0].1.contains("four hex digits per character"), "{errors:?}");
    assert_eq!(errors[1].0, 10);
    assert!(errors[1].1.contains("not valid UTF-8"), "{errors:?}");
}

/// An `N"…"` or `U"…"` literal opens an operand like any other literal: in a
/// DISPLAY list, among STRING senders, and as the object of an abbreviated
/// condition.
#[test]
fn national_and_utf8_literals_are_operands() {
    let src = "IDENTIFICATION DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N PIC N(10).
01 WS-R PIC N(20).
PROCEDURE DIVISION.
MAIN-PARA.
    DISPLAY N\"a\" U\"b\" WS-N
    STRING WS-N DELIMITED BY SIZE N\"-\" DELIMITED BY SIZE U\"x\" DELIMITED BY SIZE INTO WS-R
    IF WS-N = N\"a\" OR N\"b\" DISPLAY \"Y\" END-IF
    STOP RUN.
";
    let r = parse(tokenize(src, SourceFormat::Free));
    let errors: Vec<_> = r.diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "{errors:?}");
}
