// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tests for IDENTIFICATION DIVISION parsing.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;

fn src(s: &str) -> String {
    format!("{}\nPROCEDURE DIVISION.\nMAIN.\n    STOP RUN.\n", s)
}

#[test]
fn minimal_identification() {
    let code = src("IDENTIFICATION DIVISION.\nPROGRAM-ID. HELLO.");
    let result = parse(tokenize(&code, SourceFormat::Free));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let prog = result.program.unwrap();
    assert_eq!(prog.identification.program_id, "HELLO");
    assert!(prog.identification.author.is_none());
}

#[test]
fn identification_with_author() {
    let code = src("IDENTIFICATION DIVISION.\nPROGRAM-ID. MYAPP.\nAUTHOR. EMERSON.");
    let result = parse(tokenize(&code, SourceFormat::Free));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let id = result.program.unwrap().identification;
    assert_eq!(id.program_id, "MYAPP");
    assert!(id.author.is_some());
}

#[test]
fn identification_with_date_written() {
    let code = src("IDENTIFICATION DIVISION.\nPROGRAM-ID. DATEPROG.\nDATE-WRITTEN. 2024-01-15.");
    let result = parse(tokenize(&code, SourceFormat::Free));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let id = result.program.unwrap().identification;
    assert_eq!(id.program_id, "DATEPROG");
    assert!(id.date_written.is_some());
}

#[test]
fn identification_program_id_hyphenated() {
    let code = src("IDENTIFICATION DIVISION.\nPROGRAM-ID. MY-PROGRAM.");
    let result = parse(tokenize(&code, SourceFormat::Free));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result.program.unwrap().identification.program_id,
        "MY-PROGRAM"
    );
}

// ── Comment-entry paragraphs ──────────────────────────────────────────────────
//
// COBOL-85 gives AUTHOR, INSTALLATION, DATE-WRITTEN, DATE-COMPILED, SECURITY
// (and the deleted REMARKS) a *comment-entry*: free text that may contain
// reserved words and periods, running across as many lines as the developer
// wrote, and ending only at the next entry in Area A.
//
// The `guard_*` tests pin behaviour that must NOT change; they were written and
// run before the comment-entry rule existed.

fn errs(code: &str) -> Vec<String> {
    parse(tokenize(code, SourceFormat::Free))
        .diagnostics
        .iter()
        .filter(|d| d.severity == cobolt_parser::Severity::Error)
        .map(|d| d.message.clone())
        .collect()
}

fn id_of(code: &str) -> cobolt_ast::program::IdentificationDivision {
    let e = errs(code);
    assert!(e.is_empty(), "unexpected errors: {e:#?}\n{code}");
    parse(tokenize(code, SourceFormat::Free))
        .program
        .expect("no program")
        .identification
}

/// `SECURITY` and `INSTALLATION` are recognised as paragraphs **only** inside
/// the IDENTIFICATION DIVISION. They must not become reserved words: a data
/// item called `SECURITY` is an ordinary thing to write, and reserving the word
/// would silently break every program that has one.
#[test]
fn guard_security_is_not_a_reserved_word() {
    let code = "IDENTIFICATION DIVISION.\n\
                PROGRAM-ID. SECPROG.\n\
                DATA DIVISION.\n\
                WORKING-STORAGE SECTION.\n\
                01  SECURITY     PIC X(4).\n\
                01  INSTALLATION PIC X(4).\n\
                PROCEDURE DIVISION.\n\
                MAIN.\n\
                    MOVE \"ABCD\" TO SECURITY.\n\
                    MOVE \"EFGH\" TO INSTALLATION.\n\
                    STOP RUN.\n";
    assert!(errs(code).is_empty(), "{:#?}", errs(code));
}

/// A program with none of the optional paragraphs is unaffected.
#[test]
fn guard_no_optional_paragraphs() {
    let id = id_of(&src("IDENTIFICATION DIVISION.\nPROGRAM-ID. BARE."));
    assert_eq!(id.program_id, "BARE");
    assert!(id.author.is_none());
    assert!(id.installation.is_none());
    assert!(id.security.is_none());
}

/// The RAD generator puts AUTHOR in every generated form `.cbl`, so this path
/// runs on generated code. It must keep working exactly as before.
#[test]
fn guard_generated_banner_still_parses() {
    let id = id_of(&src(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. FORM1.\n\
         AUTHOR. POWERRUSTCOBOL RAD GENERATOR.",
    ));
    assert_eq!(id.program_id, "FORM1");
    assert!(id.author.is_some());
}

/// DATE-COMPILED is a keyword; it used to fall through the paragraph loop and
/// end the division early, so the parser then demanded a division header where
/// a paragraph name sat. This is CCVS85's NC303M and NC401M.
#[test]
fn date_compiled_paragraph() {
    let id = id_of(&src(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. NC303M.\n\
         DATE-COMPILED.  22ND AUG 1988.",
    ));
    assert!(id.date_compiled.is_some(), "DATE-COMPILED was not captured");
}

/// A comment-entry may contain reserved words. CCVS85's CM101M has
/// `AUTOMATED DATA AND TELECOMMUNICATION SERVICE.` inside its INSTALLATION
/// entry — collection used to stop at `DATA`, conclude the DATA DIVISION had
/// begun, and then fail looking for `DIVISION`.
#[test]
fn comment_entry_may_contain_reserved_words() {
    let code = "IDENTIFICATION DIVISION.\n\
                PROGRAM-ID. RESERVED.\n\
                SECURITY.\n\
                    AUTOMATED DATA AND TELECOMMUNICATION SERVICE.\n\
                    THIS PROCEDURE DIVISION ENVIRONMENT TEXT IS PROSE.\n\
                DATA DIVISION.\n\
                WORKING-STORAGE SECTION.\n\
                01  WS-X PIC X.\n\
                PROCEDURE DIVISION.\n\
                MAIN.\n\
                    STOP RUN.\n";
    assert!(errs(code).is_empty(), "{:#?}", errs(code));
    // The DATA DIVISION after the entry must still be found.
    let prog = parse(tokenize(code, SourceFormat::Free)).program.unwrap();
    assert!(prog.data.is_some(), "the DATA DIVISION was swallowed");
}

/// A comment-entry runs across lines and past periods. This is CCVS85's
/// CM101M INSTALLATION entry, nine lines of it, verbatim.
#[test]
fn comment_entry_spans_many_lines_and_periods() {
    let code = "IDENTIFICATION DIVISION.\n\
                PROGRAM-ID. CM101M.\n\
                AUTHOR.\n\
                    FEDERAL COMPILER TESTING CENTER.\n\
                INSTALLATION.\n\
                    GENERAL SERVICES ADMINISTRATION\n\
                    AUTOMATED DATA AND TELECOMMUNICATION SERVICE.\n\
                    SOFTWARE DEVELOPMENT OFFICE.\n\
                    5203 LEESBURG PIKE  SUITE 1100\n\
                    FALLS CHURCH VIRGINIA 22041.\n\
                    PHONE   (703) 756-6153\n\
                    \" HIGH       \".\n\
                DATE-WRITTEN.\n\
                    CCVS-74 VERSION 4.0 - 1980 JULY 1.\n\
                SECURITY.\n\
                    NONE.\n\
                PROCEDURE DIVISION.\n\
                MAIN.\n\
                    STOP RUN.\n";
    assert!(errs(code).is_empty(), "{:#?}", errs(code));
    let id = parse(tokenize(code, SourceFormat::Free))
        .program
        .unwrap()
        .identification;

    let inst = id.installation.expect("INSTALLATION not captured");
    // Both ends of the nine-line entry must be present: stopping at the first
    // period would have kept only the first line.
    assert!(inst.contains("GENERAL"), "entry truncated at the front: {inst}");
    assert!(
        inst.contains("CHURCH"),
        "entry stopped at an interior period: {inst}"
    );
    assert!(id.author.is_some());
    assert!(id.date_written.is_some());
    assert!(id.security.is_some());
    // AUTHOR must not have swallowed INSTALLATION.
    assert!(
        !id.author.as_deref().unwrap_or("").contains("GENERAL"),
        "AUTHOR ran past its paragraph: {:?}",
        id.author
    );
}

/// EXEC85's INSTALLATION is two lines, each a quoted literal ending in a
/// period — the case that proves period-termination is wrong.
#[test]
fn comment_entry_of_quoted_lines() {
    let code = "IDENTIFICATION DIVISION.\n\
                PROGRAM-ID. EXEC85.\n\
                INSTALLATION.\n\
                    \"ON-SITE VALIDATION, NATIONAL INSTITUTE OF STD & TECH.     \".\n\
                    \"COBOL 85 VERSION 4.2, Apr  1993 SSVG                      \".\n\
                ENVIRONMENT DIVISION.\n\
                PROCEDURE DIVISION.\n\
                MAIN.\n\
                    STOP RUN.\n";
    assert!(errs(code).is_empty(), "{:#?}", errs(code));
    let inst = parse(tokenize(code, SourceFormat::Free))
        .program
        .unwrap()
        .identification
        .installation
        .expect("INSTALLATION not captured");
    assert!(inst.contains("ON-SITE"), "{inst}");
    assert!(inst.contains("SSVG"), "second line was dropped: {inst}");
}

/// Any order, any subset (R5), and every field lands in its own slot.
#[test]
fn paragraphs_in_scrambled_order() {
    let id = id_of(&src(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. SCRAMBLE.\n\
         SECURITY. LEVEL FOUR.\n\
         DATE-COMPILED. TODAY.\n\
         AUTHOR. SOMEONE.\n\
         INSTALLATION. SOMEWHERE.\n\
         DATE-WRITTEN. YESTERDAY.",
    ));
    assert!(id.author.is_some(), "author");
    assert!(id.installation.is_some(), "installation");
    assert!(id.date_written.is_some(), "date_written");
    assert!(id.date_compiled.is_some(), "date_compiled");
    assert!(id.security.is_some(), "security");
}

/// REMARKS was deleted from COBOL in 1985 and CCVS85 contains none, but source
/// carried over from COBOL-74 still has it. Accepted, text discarded.
#[test]
fn remarks_paragraph_is_accepted() {
    let id = id_of(&src(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. OLDPROG.\n\
         REMARKS. THIS PROGRAM IS FROM 1979.\n\
         AUTHOR. SOMEONE.",
    ));
    assert!(id.author.is_some(), "REMARKS swallowed the AUTHOR paragraph");
}

/// An IDENTIFICATION-only program: the entry is ended by end of file.
#[test]
fn comment_entry_terminated_by_eof() {
    let code = "IDENTIFICATION DIVISION.\nPROGRAM-ID. IDONLY.\nAUTHOR. NOBODY AT ALL.\n";
    // No PROCEDURE DIVISION, so a diagnostic is expected — but it must be the
    // missing division, not a runaway comment-entry, and it must not hang.
    let e = errs(code);
    assert!(
        e.iter().all(|m| !m.contains("Division, found")),
        "the entry ran away instead of stopping at EOF: {e:#?}"
    );
}
