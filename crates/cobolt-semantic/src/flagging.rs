// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Conformance **flagging** — reporting language elements the COBOL-85 standard
//! marks as obsolete.
//!
//! This is not error checking. Every construct named here compiles and runs
//! exactly as it always did; flagging only *reports* that the standard lists it
//! in its obsolete-element set, scheduled for removal from the next revision. A
//! program is never rejected for carrying one, which is why these never become
//! `Severity::Error` and why the analysis is a separate entry point rather than
//! part of [`crate::analyze`] — an ordinary build must not start warning about
//! `AUTHOR`.
//!
//! **Why the token stream and not the AST.** Most obsolete elements are
//! IDENTIFICATION DIVISION paragraphs (`AUTHOR`, `INSTALLATION`, `SECURITY`, …)
//! and the `MEMORY SIZE` clause, all of which the parser consumes and discards:
//! they carry no meaning to preserve. Recording them in the AST purely so a
//! diagnostic pass could find them again would enlarge `Program` for something
//! no execution path reads. The source text is where they exist, so the source
//! text is where they are counted.
//!
//! NIST CCVS85 validates this directly. `NC302M` and `NC303M` have no PASS/FAIL
//! machinery at all — each obsolete construct is followed by the comment
//! `*Message expected for above statement: OBSOLETE`, and the program ends with
//! `*TOTAL NUMBER OF FLAGS EXPECTED = N.`. What is under test is the diagnostic
//! the compiler produces, so the two are scored by comparing this analysis
//! against those comments.

use cobolt_lexer::{Span, SpannedToken, Token};

/// Which conformance list an element falls in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagClass {
    /// In the COBOL-85 obsolete-element set: valid now, removed next revision.
    Obsolete,
}

impl FlagClass {
    /// The wording CCVS85 uses for this class in its expectation comments.
    pub fn ccvs_name(self) -> &'static str {
        match self {
            FlagClass::Obsolete => "OBSOLETE",
        }
    }
}

/// One flagged construct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flag {
    /// 1-based source line of the construct that was flagged.
    pub line: u32,
    pub class: FlagClass,
    /// The element's name in the standard, e.g. `"AUTHOR paragraph"`.
    pub element: &'static str,
}

impl Flag {
    fn obsolete(element: &'static str, span: Span) -> Self {
        Self {
            line: span.line,
            class: FlagClass::Obsolete,
            element,
        }
    }
}

/// Which division the scan is currently inside.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Division {
    Identification,
    Environment,
    Data,
    Procedure,
}

/// True if `tok` is an identifier equal (case-insensitively) to `w`.
///
/// The lexer keeps a dedicated token only for the words the *parser* needs to
/// branch on; everything else — `INSTALLATION`, `SECURITY`, `ALTER`, `MEMORY` —
/// arrives as an ordinary identifier.
fn is_word(tok: &Token, w: &str) -> bool {
    matches!(tok, Token::Identifier(s) if s.eq_ignore_ascii_case(w))
}

/// Report every COBOL-85 obsolete element in a token stream.
///
/// Flags come out in source order. The stream must be the **whole** compilation
/// unit, tokenized the way it will really be compiled: the division a token
/// sits in decides how it is read, and `SECURITY` is an obsolete paragraph in
/// the IDENTIFICATION DIVISION and an ordinary data name anywhere else.
pub fn flag_obsolete(tokens: &[SpannedToken]) -> Vec<Flag> {
    let mut flags = Vec::new();
    let mut division = Division::Identification;
    // Whether this token opens a sentence. A paragraph *header* does, and that
    // is what separates the `SECURITY` in `SECURITY.` from the one in the
    // comment text `NO SECURITY.` that follows it on the same line.
    let mut sentence_start = true;

    for (i, st) in tokens.iter().enumerate() {
        let next = tokens.get(i + 1).map(|t| &t.token);
        let at_start = sentence_start;
        sentence_start = matches!(st.token, Token::Period);

        // ── division tracking ───────────────────────────────────────────────
        // A division header is the division word followed by DIVISION; the
        // second token is required so a data item named `DATA-COUNT` or a
        // paragraph called `PROCEDURE-EXIT` cannot move the scan.
        if matches!(next, Some(Token::Division)) {
            match st.token {
                Token::Identification => division = Division::Identification,
                Token::Environment => division = Division::Environment,
                Token::Data => division = Division::Data,
                Token::Procedure => division = Division::Procedure,
                _ => {}
            }
            continue;
        }

        match division {
            // ── IDENTIFICATION DIVISION paragraphs ──────────────────────────
            // COBOL-85 obsoletes every optional paragraph of this division.
            // Each is a paragraph *header*, so the name is followed by a
            // period — requiring it keeps a data item called `SECURITY` in a
            // later division from being counted, and costs nothing here.
            Division::Identification => {
                let paragraph = at_start && matches!(next, Some(Token::Period));
                let element = match &st.token {
                    Token::Author if paragraph => "AUTHOR paragraph",
                    Token::DateWritten if paragraph => "DATE-WRITTEN paragraph",
                    Token::DateCompiled if paragraph => "DATE-COMPILED paragraph",
                    t if paragraph && is_word(t, "INSTALLATION") => "INSTALLATION paragraph",
                    t if paragraph && is_word(t, "SECURITY") => "SECURITY paragraph",
                    t if paragraph && is_word(t, "REMARKS") => "REMARKS paragraph",
                    _ => continue,
                };
                flags.push(Flag::obsolete(element, st.span));
            }

            // ── ENVIRONMENT DIVISION clauses ────────────────────────────────
            Division::Environment => {
                // OBJECT-COMPUTER ... MEMORY SIZE n CHARACTERS. The SIZE word
                // is what separates the clause from a data name `MEMORY`, and
                // it arrives as `SizeError`: the lexer maps the bare word that
                // way so the parser can fuse `SIZE ERROR`, so matching it as an
                // identifier finds nothing.
                if is_word(&st.token, "MEMORY") && matches!(next, Some(Token::SizeError)) {
                    flags.push(Flag::obsolete("MEMORY SIZE clause", st.span));
                }
            }

            Division::Data => {}

            // ── PROCEDURE DIVISION statements ───────────────────────────────
            Division::Procedure => {
                if is_word(&st.token, "ALTER") {
                    flags.push(Flag::obsolete("ALTER statement", st.span));
                    continue;
                }
                // `STOP literal` is obsolete; `STOP RUN` is not. The literal
                // may be numeric or alphanumeric.
                if matches!(st.token, Token::Stop)
                    && matches!(
                        next,
                        Some(Token::StringLiteral(_))
                            | Some(Token::IntegerLiteral(_))
                            | Some(Token::DecimalLiteral { .. })
                    )
                {
                    flags.push(Flag::obsolete("STOP statement with a literal", st.span));
                    continue;
                }
                // `GO TO.` with no procedure name — the form that only an
                // ALTER can give a destination, and obsolete with it. `GO TO`
                // reaches here as two tokens (`Go` then `To`), so the optional
                // `TO` is stepped over before the period is looked for; the
                // bare `GO.` spelling is the same statement.
                if matches!(st.token, Token::GoTo | Token::Go) {
                    let after = if matches!(next, Some(Token::To)) {
                        tokens.get(i + 2).map(|t| &t.token)
                    } else {
                        next
                    };
                    if matches!(after, Some(Token::Period)) {
                        flags.push(Flag::obsolete("GO TO without a procedure-name", st.span));
                    }
                }
            }
        }
    }

    flags
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_lexer::{tokenize, SourceFormat};

    fn flags_of(src: &str) -> Vec<&'static str> {
        flag_obsolete(&tokenize(src, SourceFormat::Free))
            .into_iter()
            .map(|f| f.element)
            .collect()
    }

    /// The IDENTIFICATION DIVISION's optional paragraphs, all six.
    #[test]
    fn identification_paragraphs_are_obsolete() {
        let got = flags_of(
            "       IDENTIFICATION DIVISION.
       PROGRAM-ID. OBS.
       AUTHOR. SOMEONE.
       INSTALLATION. NCC.
       DATE-WRITTEN. 19TH AUG 1988.
       DATE-COMPILED. 22ND AUG 1988.
       SECURITY. NONE.
       PROCEDURE DIVISION.
       MAIN.
           STOP RUN.
",
        );
        assert_eq!(
            got,
            vec![
                "AUTHOR paragraph",
                "INSTALLATION paragraph",
                "DATE-WRITTEN paragraph",
                "DATE-COMPILED paragraph",
                "SECURITY paragraph",
            ]
        );
    }

    /// `NC302M`'s seven, in its own order.
    #[test]
    fn nc302m_shape() {
        let got = flags_of(
            "       IDENTIFICATION DIVISION.
       PROGRAM-ID. NC302M.
       AUTHOR. DAVID G BAMBER.
       INSTALLATION. NCC.
       DATE-WRITTEN. 19TH AUG 1988.
       SECURITY. NO SECURITY.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       OBJECT-COMPUTER. XXXXX083 MEMORY SIZE 512 CHARACTERS.
       DATA DIVISION.
       PROCEDURE DIVISION.
       NC302M-ALTER.
           ALTER NC302M-PROC1 TO NC302M-PROC2.
       NC302M-PROC1.
           GO TO NC302M-PROC2.
       NC302M-PROC2.
           DISPLAY \"DUMMY PROCEDURE\".
       NC302M-STOP.
           STOP \"FNC302\".
",
        );
        assert_eq!(
            got,
            vec![
                "AUTHOR paragraph",
                "INSTALLATION paragraph",
                "DATE-WRITTEN paragraph",
                "SECURITY paragraph",
                "MEMORY SIZE clause",
                "ALTER statement",
                "STOP statement with a literal",
            ]
        );
        assert_eq!(got.len(), 7, "NC302M declares TOTAL FLAGS EXPECTED = 7");
    }

    /// `NC303M`'s four — note the two bare `GO TO.`, which only an `ALTER` can
    /// give a destination.
    #[test]
    fn nc303m_shape() {
        let got = flags_of(
            "       IDENTIFICATION DIVISION.
       PROGRAM-ID. NC303M.
       DATE-COMPILED. 22ND AUG 1988.
       ENVIRONMENT DIVISION.
       PROCEDURE DIVISION.
       NC303M-CONTROL.
           ALTER NC303M-GOTO TO PROCEED TO NC303M-GOTO-2.
           STOP RUN.
       NC303M-GOTO.
           GO TO.
       NC303M-GOTO-2.
           GO TO.
",
        );
        assert_eq!(
            got,
            vec![
                "DATE-COMPILED paragraph",
                "ALTER statement",
                "GO TO without a procedure-name",
                "GO TO without a procedure-name",
            ]
        );
        assert_eq!(got.len(), 4, "NC303M declares TOTAL FLAGS EXPECTED = 4");
    }

    /// `STOP RUN` is current, and a `GO TO` that names its target is too.
    #[test]
    fn the_current_forms_are_not_flagged() {
        let got = flags_of(
            "       IDENTIFICATION DIVISION.
       PROGRAM-ID. CLEAN.
       PROCEDURE DIVISION.
       MAIN.
           GO TO NEXT-ONE.
       NEXT-ONE.
           STOP RUN.
",
        );
        assert!(got.is_empty(), "{got:?}");
    }

    /// A data item may legitimately be called `SECURITY` or `MEMORY`. Neither
    /// is a flag outside the division whose paragraph or clause it names.
    #[test]
    fn data_names_are_not_paragraphs() {
        let got = flags_of(
            "       IDENTIFICATION DIVISION.
       PROGRAM-ID. NAMES.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SECURITY PIC X(4).
       01 MEMORY PIC 9(4).
       01 AUTHOR PIC X(4).
       PROCEDURE DIVISION.
       MAIN.
           MOVE \"ABCD\" TO SECURITY.
           STOP RUN.
",
        );
        assert!(got.is_empty(), "{got:?}");
    }
}
