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
    /// Above the COBOL-85 **high subset** — see [`flag_high_subset`].
    NonConforming,
}

impl FlagClass {
    /// The wording CCVS85 uses for this class in its expectation comments.
    pub fn ccvs_name(self) -> &'static str {
        match self {
            FlagClass::Obsolete => "OBSOLETE",
            FlagClass::NonConforming => "NON-CONFORMING STANDARD",
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

    fn above_subset(element: &'static str, line: u32) -> Self {
        Self {
            line,
            class: FlagClass::NonConforming,
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

/// Report every element above the COBOL-85 **high subset**.
///
/// The standard defines nested conformance levels, and a conforming
/// implementation must be able to tell a program which of the features it uses
/// sit above a chosen one. Everything reported here is **fully valid COBOL-85**
/// that PowerRustCOBOL implements and executes; the analysis says only "this
/// would not compile on a high-subset-only implementation", which is why it is
/// an opt-in entry point and never part of an ordinary build. NIST `NC401M`
/// validates exactly this list, and expects forty flags.
///
/// `source` is needed alongside the tokens because two of the elements are
/// *lexical*: a continuation line exists in the card image and is gone by the
/// time it reaches the token stream, where `MUL` + `-TIPLY` is simply the word
/// `MULTIPLY`.
pub fn flag_high_subset(tokens: &[SpannedToken], source: &str) -> Vec<Flag> {
    let mut flags: Vec<Flag> = Vec::new();

    // ── lexical: continuation lines ─────────────────────────────────────────
    // A hyphen in the indicator area continues the previous line. The lexer has
    // already joined the halves — `MUL` + `-TIPLY` reaches the token stream as
    // the single word `MULTIPLY` — so this is read from the card image.
    //
    // **Only some continuations are above the subset.** Continuing an
    // *alphanumeric literal* is available all the way down, and NC401M does not
    // flag its own
    //     03 MARYPOPPINS PIC X(34) VALUE "SUPERCALIFRAGILISTICEXPIALIDON
    //    -    "CIOUS".
    // What it does flag is continuing a **word** (`MUL` / `-TIPLY`) or a
    // **numeric literal** (`2` / `-0`, and `PIC X(1` / `-00)`). The two are
    // told apart by what the continuation line resumes with: a quotation mark
    // reopens a literal, anything else carries on a word or a number.
    for (i, line) in source.lines().enumerate() {
        if line.chars().nth(6) != Some('-') {
            continue;
        }
        let area_b: String = line.chars().skip(7).collect();
        if matches!(area_b.trim_start().chars().next(), Some('"') | Some('\'')) {
            continue; // continued alphanumeric literal — in subset
        }
        flags.push(Flag::above_subset(
            "continuation of a word or numeric literal",
            i as u32 + 1,
        ));
    }

    let mut division = Division::Identification;
    let mut sentence_start = true;
    // One-shot markers, cleared at every period. A data entry and a procedural
    // sentence both end there, so the same mechanism keeps `MOVE A OF B TO C OF
    // D` to a single qualification flag and an `OCCURS … ASCENDING KEY …
    // DESCENDING KEY …` to a single sorted-table one.
    let mut once: Vec<&'static str> = Vec::new();
    // Levels of the REDEFINES entries still open, outermost first.
    let mut open_redefines: Vec<u8> = Vec::new();
    let mut level: u8 = 0;

    let mut emit = |flags: &mut Vec<Flag>, once: &mut Vec<&'static str>, el: &'static str, line: u32| {
        if once.contains(&el) {
            return;
        }
        once.push(el);
        flags.push(Flag::above_subset(el, line));
    };

    for (i, st) in tokens.iter().enumerate() {
        let next = tokens.get(i + 1).map(|t| &t.token);
        let at_start = sentence_start;
        sentence_start = matches!(st.token, Token::Period);
        if matches!(st.token, Token::Period) {
            once.clear();
        }
        let line = st.span.line;

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
            Division::Identification => {
                if matches!(st.token, Token::DateCompiled)
                    && at_start
                    && matches!(next, Some(Token::Period))
                {
                    emit(&mut flags, &mut once, "DATE-COMPILED paragraph", line);
                }
            }

            Division::Environment => {
                // ALPHABET name IS literal THRU literal — a user-defined
                // collating sequence. The plain `IS NATIVE` form is in subset.
                if is_word(&st.token, "ALPHABET")
                    && tokens[i..]
                        .iter()
                        .take_while(|t| !matches!(t.token, Token::Period))
                        .any(|t| matches!(t.token, Token::Through))
                {
                    emit(&mut flags, &mut once, "ALPHABET with a literal range", line);
                }
                if is_word(&st.token, "SYMBOLIC") {
                    emit(&mut flags, &mut once, "SYMBOLIC CHARACTERS clause", line);
                }
            }

            Division::Data => {
                if let Token::LevelNumber(n) = st.token {
                    level = n;
                    // A level number closes every open REDEFINES at or below it.
                    open_redefines.retain(|&l| l < n);
                    if n == 66 {
                        emit(&mut flags, &mut once, "level 66 RENAMES entry", line);
                    }
                    if n == 88 {
                        emit(&mut flags, &mut once, "level 88 condition-name", line);
                    }
                }
                if matches!(st.token, Token::Redefines) {
                    // A REDEFINES *inside* a redefining description — the
                    // second reading of a second reading.
                    if !open_redefines.is_empty() {
                        emit(&mut flags, &mut once, "REDEFINES of a REDEFINES", line);
                    }
                    open_redefines.push(level);
                }
                if matches!(st.token, Token::Renames)
                    && tokens[i..]
                        .iter()
                        .take_while(|t| !matches!(t.token, Token::Period))
                        .any(|t| matches!(t.token, Token::Through))
                {
                    emit(&mut flags, &mut once, "RENAMES … THROUGH", line);
                }
                // OCCURS DEPENDING ON … INDEXED BY, in the same entry.
                if matches!(st.token, Token::Indexed)
                    && once.contains(&"__odo")
                {
                    emit(
                        &mut flags,
                        &mut once,
                        "variable-length table with INDEXED BY",
                        line,
                    );
                }
                if matches!(st.token, Token::Depending) {
                    once.push("__odo");
                }
                if matches!(st.token, Token::Ascending | Token::Descending) {
                    emit(&mut flags, &mut once, "ASCENDING/DESCENDING KEY", line);
                }
                if matches!(st.token, Token::Value) && matches!(next, Some(Token::All)) {
                    emit(&mut flags, &mut once, "VALUE ALL literal", line);
                }
            }

            Division::Procedure => {
                subset_procedure_flags(tokens, i, st, next, line, &mut flags, &mut once, &mut emit);
            }
        }
    }

    // `END PROGRAM` closes the compilation unit and belongs to the nested-source
    // facility, above the subset. It is the one element NC401M announces with
    // "Message expected for **following** statement".
    for (i, st) in tokens.iter().enumerate() {
        if matches!(st.token, Token::End)
            && matches!(tokens.get(i + 1).map(|t| &t.token), Some(Token::Program))
        {
            flags.push(Flag::above_subset("END PROGRAM header", st.span.line));
        }
    }

    flags.sort_by_key(|f| f.line);
    flags
}

/// The PROCEDURE DIVISION half of [`flag_high_subset`], split out to keep one
/// function from running to three screens.
#[allow(clippy::too_many_arguments)]
fn subset_procedure_flags(
    tokens: &[SpannedToken],
    i: usize,
    st: &SpannedToken,
    next: Option<&Token>,
    line: u32,
    flags: &mut Vec<Flag>,
    once: &mut Vec<&'static str>,
    emit: &mut impl FnMut(&mut Vec<Flag>, &mut Vec<&'static str>, &'static str, u32),
) {
    // ── whole statements ────────────────────────────────────────────────────
    let simple: &[(&'static str, bool)] = &[
        ("COMPUTE statement", matches!(st.token, Token::Compute)),
        ("EVALUATE statement", matches!(st.token, Token::Evaluate)),
        ("INITIALIZE statement", matches!(st.token, Token::Initialize)),
        ("STRING statement", matches!(st.token, Token::StringVerb)),
        ("UNSTRING statement", matches!(st.token, Token::Unstring)),
        ("SEARCH statement", is_word(&st.token, "SEARCH")),
        (
            "CORRESPONDING phrase",
            matches!(st.token, Token::Corresponding),
        ),
        ("DIVIDE … REMAINDER", matches!(st.token, Token::Remainder)),
        ("DISPLAY … UPON", matches!(st.token, Token::Upon)),
        (
            "ACCEPT … FROM DAY-OF-WEEK",
            is_word(&st.token, "DAY-OF-WEEK"),
        ),
        (
            "INSPECT … CONVERTING",
            matches!(st.token, Token::Converting),
        ),
        ("PERFORM … VARYING", matches!(st.token, Token::Varying)),
        ("SET … TO TRUE", matches!(st.token, Token::True_)),
        ("IF … ELSE", matches!(st.token, Token::Else)),
        (
            "reference modification",
            matches!(st.token, Token::Colon),
        ),
        ("qualified data-name", matches!(st.token, Token::Of)),
        (
            "sign condition",
            is_word(&st.token, "NEGATIVE") || is_word(&st.token, "POSITIVE"),
        ),
    ];
    for (element, hit) in simple {
        if *hit {
            emit(flags, once, element, line);
        }
    }

    // ── forms that need a neighbour ─────────────────────────────────────────
    if matches!(st.token, Token::Test) && matches!(next, Some(Token::After)) {
        emit(flags, once, "PERFORM … WITH TEST AFTER", line);
    }
    if is_word(&st.token, "ALTER")
        && tokens[i..]
            .iter()
            .take_while(|t| !matches!(t.token, Token::Period))
            .any(|t| is_word(&t.token, "PROCEED"))
    {
        emit(flags, once, "ALTER … TO PROCEED TO", line);
    }
    if matches!(st.token, Token::GoTo | Token::Go) {
        let after = if matches!(next, Some(Token::To)) {
            tokens.get(i + 2).map(|t| &t.token)
        } else {
            next
        };
        if matches!(after, Some(Token::Period)) {
            emit(flags, once, "GO TO without a procedure-name", line);
        }
    }

    // ── a subscript list deeper than three ──────────────────────────────────
    // The high subset allows three subscripts; NC401M writes five.
    //
    // **Not by counting commas.** A comma followed by a space is a *separator*
    // and the lexer drops it, so `(A, B, C, D, 1)` reaches here as five bare
    // operands — a comma count sees zero and the detector never fires. That is
    // exactly how this went unnoticed: NC401M still scored 40 of 40, because
    // the matcher is greedy in source order and some other flag stood in.
    //
    // Operands are counted instead, less the binary operators among them: each
    // operator joins two operands into one subscript, so `(IN1 + 3)` is one and
    // `(IN1 - 1  3)` is two.
    if matches!(st.token, Token::LParen) {
        let mut depth = 0i32;
        let (mut operands, mut operators, mut colon) = (0usize, 0usize, false);
        for t in &tokens[i..] {
            match t.token {
                Token::LParen => depth += 1,
                Token::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Token::Period => break,
                _ if depth != 1 => {}
                Token::Colon => colon = true,
                Token::Identifier(_) | Token::IntegerLiteral(_) => operands += 1,
                Token::Plus | Token::Minus | Token::Star | Token::Slash => operators += 1,
                _ => {}
            }
        }
        // A reference modification `(1:20)` is a different construct with its
        // own flag, not a two-subscript list.
        if !colon && operands.saturating_sub(operators) > 3 {
            emit(flags, once, "more than three subscripts", line);
        }
    }

    // ── conditions ──────────────────────────────────────────────────────────
    if matches!(st.token, Token::If) {
        let rest = tokens[i..]
            .iter()
            .take_while(|t| !matches!(t.token, Token::Period));
        let (mut arith, mut logical) = (false, false);
        for t in rest {
            match t.token {
                Token::Plus | Token::Minus | Token::Star | Token::Slash | Token::Power => {
                    arith = true
                }
                Token::And | Token::Or => logical = true,
                _ => {}
            }
        }
        if arith {
            emit(flags, once, "arithmetic expression in a relation", line);
        }
        if logical {
            emit(flags, once, "complex condition", line);
        }
    }
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

    // ── high subset ─────────────────────────────────────────────────────────

    fn subset_of(src: &str) -> Vec<&'static str> {
        flag_high_subset(&tokenize(src, SourceFormat::Free), src)
            .into_iter()
            .map(|f| f.element)
            .collect()
    }

    /// Wrap a PROCEDURE DIVISION body in the smallest complete program.
    fn proc(body: &str) -> String {
        format!(
            "       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUB.
       PROCEDURE DIVISION.
       MAIN.
{body}
",
        )
    }

    /// Each above-subset statement is reported once, under its own name.
    #[test]
    fn above_subset_statements_are_named_individually() {
        for (body, want) in [
            ("           COMPUTE BOX-A = 10 + 6.", "COMPUTE statement"),
            ("           INITIALIZE VARB.", "INITIALIZE statement"),
            (
                "           STRING VARD DELIMITED BY VARB INTO VARC.",
                "STRING statement",
            ),
            ("           UNSTRING VARD INTO VARE.", "UNSTRING statement"),
            (
                "           ADD CORRESPONDING GROUP-1 TO GROUP-2.",
                "CORRESPONDING phrase",
            ),
            (
                "           DIVIDE BOX-A INTO BOX-B GIVING BOX-C REMAINDER BOX-D.",
                "DIVIDE … REMAINDER",
            ),
            (
                "           DISPLAY \"PFILE\" UPON VDUNIT.",
                "DISPLAY … UPON",
            ),
            (
                "           ACCEPT DDAY FROM DAY-OF-WEEK.",
                "ACCEPT … FROM DAY-OF-WEEK",
            ),
            (
                "           INSPECT MARYPOPPINS CONVERTING \"A\" TO \"Z\".",
                "INSPECT … CONVERTING",
            ),
            ("           SET CUST-PAID TO TRUE.", "SET … TO TRUE"),
            (
                "           DISPLAY COLONTEST(1:20).",
                "reference modification",
            ),
            (
                "           MOVE GUBBINS OF FREC TO GUBBINS OF FREC-2.",
                "qualified data-name",
            ),
            (
                "           PERFORM P1 THRU P2 VARYING BOX-A FROM 1 BY 1 UNTIL BOX-B = 2.",
                "PERFORM … VARYING",
            ),
            (
                "           PERFORM P1 THRU P2 WITH TEST AFTER UNTIL BOX-B = BOX-A.",
                "PERFORM … WITH TEST AFTER",
            ),
            (
                "           ALTER P1 TO PROCEED TO P2.",
                "ALTER … TO PROCEED TO",
            ),
        ] {
            let got = subset_of(&proc(body));
            assert!(
                got.contains(&want),
                "{want:?} not reported for {body:?}; got {got:?}"
            );
        }
    }

    /// `MOVE A OF B TO C OF D` names two qualifications and is one flag: NC401M
    /// expects a single message for that statement.
    #[test]
    fn a_repeated_element_is_reported_once_per_sentence() {
        let got = subset_of(&proc(
            "           MOVE GUBBINS OF FREC TO GUBBINS OF FREC-2.",
        ));
        assert_eq!(
            got.iter().filter(|e| **e == "qualified data-name").count(),
            1,
            "{got:?}"
        );
    }

    /// The three condition forms are told apart, and a plain relation is not
    /// flagged at all.
    #[test]
    fn condition_forms() {
        let arith = subset_of(&proc(
            "           IF BOX-A + 1 IS NOT GREATER THAN BOX-B + 2 DISPLAY \"X\".",
        ));
        assert!(
            arith.contains(&"arithmetic expression in a relation"),
            "{arith:?}"
        );

        let sign = subset_of(&proc(
            "           IF BOX-A IS NOT NEGATIVE DISPLAY \"X\".",
        ));
        assert!(sign.contains(&"sign condition"), "{sign:?}");

        let complex = subset_of(&proc(
            "           IF BOX-A > BOX-B AND NOT BOX-C > BOX-A MOVE 7 TO BOX-B.",
        ));
        assert!(complex.contains(&"complex condition"), "{complex:?}");

        let plain = subset_of(&proc("           IF BOX-A = BOX-B DISPLAY \"X\"."));
        assert!(plain.is_empty(), "a plain relation is in subset: {plain:?}");
    }

    /// Three subscripts are in subset; a fourth is not.
    #[test]
    fn only_a_fourth_subscript_is_above_subset() {
        let three = subset_of(&proc("           MOVE ZERO TO PM-SALES (A, B, C)."));
        assert!(!three.contains(&"more than three subscripts"), "{three:?}");

        let five = subset_of(&proc(
            "           MOVE ZERO TO PM-SALES (A, B, C, D, 1).",
        ));
        assert!(five.contains(&"more than three subscripts"), "{five:?}");
    }

    /// DATA DIVISION elements, including the REDEFINES-of-a-REDEFINES that only
    /// nesting distinguishes from an ordinary one.
    #[test]
    fn data_division_elements() {
        let got = subset_of(
            "       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUBDATA.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 MARYPOPPINS PIC X(10).
       01 OUTER.
          03 MP-1 REDEFINES MARYPOPPINS.
             04 MP-1-A PICTURE X(5).
             04 MP-1-A-1 REDEFINES MP-1-A.
                05 MP-INNER PICTURE X(5).
       01 CUST-REC.
          03 CUST-CODES PIC X.
             88 CUST-PAID VALUE \"A\".
          03 STATE PIC X(4) VALUE ALL \"A\".
       01 VARS.
          03 VARB PIC X(4).
          03 VARC PIC X(4).
       66 VARA RENAMES VARB THRU VARC.
       PROCEDURE DIVISION.
       MAIN.
           STOP RUN.
",
        );
        assert!(got.contains(&"REDEFINES of a REDEFINES"), "{got:?}");
        assert!(got.contains(&"level 88 condition-name"), "{got:?}");
        assert!(got.contains(&"VALUE ALL literal"), "{got:?}");
        assert!(got.contains(&"level 66 RENAMES entry"), "{got:?}");
        assert!(got.contains(&"RENAMES … THROUGH"), "{got:?}");
        // The outer `REDEFINES MARYPOPPINS` is an ordinary one: exactly one
        // nested case is reported, not two.
        assert_eq!(
            got.iter()
                .filter(|e| **e == "REDEFINES of a REDEFINES")
                .count(),
            1,
            "{got:?}"
        );
    }

    /// A continuation line is lexical — the tokens no longer show it, so it is
    /// read from the card image.
    ///
    /// Continuing a **word** or a **numeric literal** is above the subset;
    /// continuing an **alphanumeric literal** is not, and NC401M leaves its own
    /// `"SUPERCALIFRAGILISTICEXPIALIDO` / `-"CIOUS"` unflagged. Reporting every
    /// continuation alike produced one flag too many — masked, because the
    /// matcher is greedy in source order and the total still came to forty.
    #[test]
    fn only_word_and_numeric_continuations_are_above_subset() {
        // Column 7 is the indicator area: six digits, then `-`.
        let src = "000100 IDENTIFICATION DIVISION.
000200 PROGRAM-ID. CONT.
000300 DATA DIVISION.
000400 WORKING-STORAGE SECTION.
000500 01 LONG-ONE PIC X(12) VALUE \"SUPERCALIFRA
000600-    \"GILISTIC\".
000700 01 WIDE-ONE PIC X(1
000800-                   00).
000900 PROCEDURE DIVISION.
001000 MAIN.
001100     MUL
001200-    TIPLY BOX-A BY BOX-B GIVING BOX-C.
001300     MOVE 2
001400-    0 TO BOX-A.
001500     STOP RUN.
";
        let flags = flag_high_subset(&tokenize(src, SourceFormat::FixedStrict), src);
        let cont: Vec<u32> = flags
            .iter()
            .filter(|f| f.element == "continuation of a word or numeric literal")
            .map(|f| f.line)
            .collect();
        // Line 6 continues an alphanumeric literal and is left alone; 8, 12 and
        // 14 continue a number, a word and a number.
        assert_eq!(cont, vec![8, 12, 14], "{flags:?}");
    }

    /// A program written entirely inside the high subset draws no flags — the
    /// analysis has to be silent on the thing it is measuring against.
    #[test]
    fn a_high_subset_program_is_silent() {
        let got = subset_of(
            "       IDENTIFICATION DIVISION.
       PROGRAM-ID. PLAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COUNTER PIC 9(3) VALUE 0.
       01 TOTAL   PIC 9(5) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           MOVE 5 TO COUNTER.
           ADD COUNTER TO TOTAL.
           IF TOTAL = 5
              DISPLAY \"FIVE\".
           PERFORM UNTIL COUNTER = 0
              SUBTRACT 1 FROM COUNTER
           END-PERFORM.
           STOP RUN.
",
        );
        assert!(got.is_empty(), "{got:?}");
    }

    /// The two classes are independent analyses over the same source: NC303M
    /// wants `DATE-COMPILED` called OBSOLETE and NC401M wants it called
    /// NON-CONFORMING STANDARD, so neither pass may report the other's name.
    #[test]
    fn the_two_classes_stay_separate() {
        let src = "       IDENTIFICATION DIVISION.
       PROGRAM-ID. BOTH.
       DATE-COMPILED. 22ND AUG 1988.
       PROCEDURE DIVISION.
       MAIN.
           COMPUTE BOX-A = 1 + 1.
           STOP RUN.
";
        let tokens = tokenize(src, SourceFormat::Free);
        let obsolete = flag_obsolete(&tokens);
        assert!(obsolete.iter().all(|f| f.class == FlagClass::Obsolete));
        assert!(obsolete
            .iter()
            .any(|f| f.element == "DATE-COMPILED paragraph"));
        assert!(
            !obsolete.iter().any(|f| f.element == "COMPUTE statement"),
            "the obsolete pass must not report subset elements: {obsolete:?}"
        );

        let subset = flag_high_subset(&tokens, src);
        assert!(subset.iter().all(|f| f.class == FlagClass::NonConforming));
        assert!(subset.iter().any(|f| f.element == "COMPUTE statement"));
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
