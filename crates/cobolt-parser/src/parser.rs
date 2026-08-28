// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Core [`Parser`] struct and token-cursor API.

use cobolt_ast::data::{ConditionValue, DataDecl, PicClause, PicKind, Usage};
use cobolt_ast::expr::Literal;
use cobolt_ast::program::{
    AccessMode, AlternateKey, DataDivision, DataSection, EnvironmentDivision, FileControl,
    FileOrganization, InputOutputSection, RustItemBlock, StorageMode,
};
use cobolt_ast::stmt::Stmt;
use cobolt_lexer::{Span, SpannedToken, Token};

use crate::data::parse_data_division;
use crate::error::{Diagnostic, ParseResult, Severity};
use crate::identification::parse_identification_division;
use crate::procedure::parse_procedure_division;

// ── Parser ────────────────────────────────────────────────────────────────────

pub struct Parser {
    pub(crate) tokens: Vec<SpannedToken>,
    pub(crate) pos: usize,
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// Set by `SPECIAL-NAMES. DECIMAL-POINT IS COMMA`. When true, numeric literals
    /// use `,` as the decimal separator and edited PICs swap `.`/`,` roles.
    pub(crate) decimal_comma: bool,
    /// `REPOSITORY. CLASS name IS "external"` bindings captured during the
    /// CONFIGURATION SECTION (spec 005 Rust-FFI bridge); moved into the program.
    pub(crate) repository: Vec<(String, String)>,
    /// Item-level `EXEC RUST` blocks captured in the CONFIGURATION SECTION
    /// (spec 041 R19); moved into the program alongside `repository`.
    pub(crate) rust_items: Vec<RustItemBlock>,
    /// `SPECIAL-NAMES. CLASS name IS lit [THRU lit] …` — user-defined classes,
    /// moved into `Program::classes`. Distinct from the `CLASS` that appears
    /// under `REPOSITORY`, which binds a Rust type: the two are told apart by
    /// what follows the name (a string literal *and* nothing else is the
    /// repository form; a class definition lists literals or ranges).
    pub(crate) classes: Vec<(String, Vec<(char, char)>)>,
    /// `SPECIAL-NAMES. ALPHABET name IS …` — collating sequences, moved into
    /// `Program::alphabets`.
    pub(crate) alphabets: Vec<(String, cobolt_ast::program::AlphabetSpec)>,
    /// `OBJECT-COMPUTER. … PROGRAM COLLATING SEQUENCE IS name` — moved into
    /// `Program::collating_sequence`.
    pub(crate) collating_sequence: Option<String>,
    /// `SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal` — the character an edited
    /// PICTURE may use in place of `$`. Read while parsing the DATA DIVISION's
    /// PICTURE clauses, which is why it lives on the parser and not only on the
    /// finished `Program`: the ENVIRONMENT DIVISION always precedes the DATA
    /// DIVISION, so the symbol is known by the time a picture needs it.
    pub(crate) currency: char,
    /// `SPECIAL-NAMES. <switch> IS <mnemonic> ON STATUS IS <name> OFF STATUS IS
    /// <name>` — external switches, as
    /// `(implementor name, mnemonic, on-name, off-name)`.
    ///
    /// A switch has no storage of its own in this implementation: the mnemonic
    /// becomes a one-character WORKING-STORAGE item holding `"1"` (on) or
    /// `"0"` (off), and each status name becomes an 88-level condition on it,
    /// so `IF ON-SWITCH-1` and `SET SW-1 TO ON` both work through machinery
    /// that already exists.
    pub(crate) switches: Vec<(String, String, Option<String>, Option<String>)>,
    /// `SPECIAL-NAMES. <implementor-name> [IS] <mnemonic>` — the ordinary
    /// mnemonic clause (`CONSOLE IS CRT`, `XXXXX057 IS ACCEPT-INPUT-DEVICE`),
    /// as `(implementor name, mnemonic)`, both uppercased.
    ///
    /// It lives on the parser for the same reason `currency` does: `ACCEPT x
    /// FROM <mnemonic>` is Format 1 (read the device) while `ACCEPT x FROM
    /// <undeclared name>` is the environment-variable extension, and the
    /// ENVIRONMENT DIVISION always precedes the PROCEDURE DIVISION, so the
    /// declared set is known by the time an `ACCEPT` needs it.
    pub(crate) mnemonics: Vec<(String, String)>,
    /// Scratch: the class name from the most recently parsed
    /// `USAGE OBJECT REFERENCE <class>`, consumed by the data item being built.
    pub(crate) pending_object_class: Option<String>,
    /// Next id for a statement-level `EXEC RUST` block, handed out in source
    /// order (spec 041).
    pub(crate) next_block_id: u32,
    /// Statements a single parse function produced *in addition* to the one it
    /// returned, drained by `parse_stmts` straight after it. `ALTER a TO
    /// PROCEED TO b, c TO PROCEED TO d` is one statement in the source and a
    /// series of `Stmt::Alter`s in the AST; queueing the extras keeps
    /// `Stmt::Alter` a single pair, so the bincode-serialized AST is untouched.
    pub(crate) pending: Vec<Stmt>,
    /// Set while a subscript list is being parsed, so `+3` written with a space
    /// before the sign and none after it is read as a **signed literal that
    /// starts the next subscript** rather than as addition. See
    /// [`Parser::starts_signed_subscript`].
    pub(crate) in_subscript: bool,
}

impl Parser {
    pub fn new(tokens: Vec<SpannedToken>) -> Self {
        Self::with_block_id_base(tokens, 0)
    }

    /// A parser that hands out `EXEC RUST` block ids from `block_id_base`.
    ///
    /// See [`crate::parse_from`] for why a build needs this: several programs
    /// share one compiled block registry, so their ids must not overlap.
    pub fn with_block_id_base(tokens: Vec<SpannedToken>, block_id_base: u32) -> Self {
        // Filter out comment tokens — the parser doesn't need them.
        let tokens: Vec<_> = tokens
            .into_iter()
            .filter(|st| !matches!(st.token, Token::Comment(_)))
            .collect();
        Self {
            tokens,
            pos: 0,
            diagnostics: Vec::new(),
            decimal_comma: false,
            repository: Vec::new(),
            rust_items: Vec::new(),
            classes: Vec::new(),
            alphabets: Vec::new(),
            collating_sequence: None,
            currency: '$',
            switches: Vec::new(),
            mnemonics: Vec::new(),
            pending_object_class: None,
            next_block_id: block_id_base,
            pending: Vec::new(),
            in_subscript: false,
        }
    }

    /// `true` when the cursor sits on a `+`/`-` that COBOL-85 reads as the sign
    /// of a **signed integer literal opening the next subscript**, not as an
    /// arithmetic operator — `ELEM (IN1 +3)` is `ELEM (IN1, 3)`.
    ///
    /// The separator comma is dropped by the lexer ("a comma means what a space
    /// means"), so `ELEM (IN1, +3)` and `ELEM (IN1 +3)` arrive here as the same
    /// token stream and both have to split. The three conditions are exactly the
    /// standard's: relative indexing (`IN1 - 1`) writes the operator with a
    /// space on **both** sides, so a sign glued to its digits is a literal.
    ///
    /// * a space before the sign — `TBL (I+1)` stays arithmetic, which is what
    ///   RustCOBOL programs already written that way mean;
    /// * no space between the sign and the digits;
    /// * no `:` after the digits — that parenthesis is a reference
    ///   modification, whose leftmost position is an arithmetic expression.
    pub(crate) fn starts_signed_subscript(&self) -> bool {
        if !self.in_subscript {
            return false;
        }
        let Some(sign) = self.tokens.get(self.pos) else {
            return false;
        };
        if !matches!(sign.token, Token::Plus | Token::Minus) {
            return false;
        }
        let Some(num) = self.tokens.get(self.pos + 1) else {
            return false;
        };
        if !matches!(num.token, Token::IntegerLiteral(..)) || num.span.start != sign.span.end {
            return false;
        }
        let glued_left = self
            .pos
            .checked_sub(1)
            .and_then(|i| self.tokens.get(i))
            .map(|t| t.span.end == sign.span.start)
            .unwrap_or(false);
        if glued_left {
            return false;
        }
        !matches!(
            self.tokens.get(self.pos + 2).map(|t| &t.token),
            Some(Token::Colon)
        )
    }

    // ── Token inspection ─────────────────────────────────────────────────────

    /// Current token (does not advance).
    pub(crate) fn peek(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .map(|st| &st.token)
            .unwrap_or(&Token::Eof)
    }

    /// Span of the current token.
    pub(crate) fn peek_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|st| st.span)
            .unwrap_or(Span::dummy())
    }

    /// Look N tokens ahead (0 = current).
    pub(crate) fn peek_at(&self, offset: usize) -> &Token {
        self.tokens
            .get(self.pos + offset)
            .map(|st| &st.token)
            .unwrap_or(&Token::Eof)
    }

    /// Span of the token N ahead (0 = current).
    pub(crate) fn peek_span_at(&self, offset: usize) -> Span {
        self.tokens
            .get(self.pos + offset)
            .map(|st| st.span)
            .unwrap_or(Span::dummy())
    }

    /// Does the token at `offset` begin a new source line?
    ///
    /// True when its line number is greater than the previous token's, and for
    /// the very first token of the program.
    ///
    /// This is how the IDENTIFICATION DIVISION finds the end of a comment-entry.
    /// COBOL-85 ends such an entry at the next entry beginning in **Area A**,
    /// but by the time the parser runs, fixed-format source has been flattened
    /// and re-tokenized as free form, so the format is no longer known. "Starts
    /// a line" is the part of that rule which survives, and combined with a
    /// check on what the token *is*, it separates a paragraph header from prose
    /// that merely contains a reserved word.
    pub(crate) fn at_line_start(&self, offset: usize) -> bool {
        crate::identification::starts_line(&self.tokens, self.pos + offset)
    }

    /// `true` if the current token equals `tok`.
    pub(crate) fn at(&self, tok: &Token) -> bool {
        self.peek() == tok
    }

    /// `true` if current token is an `Identifier`.
    pub(crate) fn at_identifier(&self) -> bool {
        matches!(self.peek(), Token::Identifier(_))
    }

    /// `true` if current token is a `LevelNumber`.
    pub(crate) fn at_level_number(&self) -> bool {
        matches!(self.peek(), Token::LevelNumber(_))
    }

    /// `true` if current token is `Period` or `Eof`.
    pub(crate) fn at_end_of_sentence(&self) -> bool {
        matches!(self.peek(), Token::Period | Token::Eof)
    }

    // ── Token consumption ─────────────────────────────────────────────────────

    /// Consume and return the current token.
    pub(crate) fn advance(&mut self) -> SpannedToken {
        if self.pos < self.tokens.len() {
            let st = self.tokens[self.pos].clone();
            self.pos += 1;
            st
        } else {
            SpannedToken::new(Token::Eof, Span::dummy())
        }
    }

    /// Consume the current token if it equals `tok`; return whether consumed.
    pub(crate) fn eat(&mut self, tok: &Token) -> bool {
        if self.peek() == tok {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Consume the current token if it is an `Identifier`; return name + span.
    pub(crate) fn eat_identifier(&mut self) -> Option<(String, Span)> {
        if let Token::Identifier(_) = self.peek() {
            let st = self.advance();
            if let Token::Identifier(name) = st.token {
                return Some((name, st.span));
            }
        }
        None
    }

    /// Consume the current token if it is a `StringLiteral`; return value + span.
    pub(crate) fn eat_string(&mut self) -> Option<(String, Span)> {
        if let Token::StringLiteral(_) = self.peek() {
            let st = self.advance();
            if let Token::StringLiteral(s) = st.token {
                return Some((s, st.span));
            }
        }
        None
    }

    /// Expect `tok`; emit an error and return `false` if not found.
    pub(crate) fn expect(&mut self, tok: &Token) -> bool {
        if self.peek() == tok {
            self.advance();
            true
        } else {
            let msg = format!("expected {:?}, found {:?}", tok, self.peek());
            self.emit_error(msg);
            false
        }
    }

    /// Expect an identifier; return its name or emit an error and return a
    /// placeholder.
    pub(crate) fn expect_identifier(&mut self, context: &str) -> String {
        if let Some((name, _)) = self.eat_identifier() {
            name
        } else {
            self.emit_error(format!(
                "expected identifier for {context}, found {:?}",
                self.peek()
            ));
            "<missing>".into()
        }
    }

    /// Consume a `Period` or emit a warning if missing.
    pub(crate) fn expect_period(&mut self) {
        if !self.eat(&Token::Period) {
            self.emit_warning(format!("expected '.', found {:?}", self.peek()));
        }
    }

    // ── Error recovery ────────────────────────────────────────────────────────

    /// Skip tokens until (and including) the next period or EOF.
    pub(crate) fn sync_to_period(&mut self) {
        while !matches!(self.peek(), Token::Period | Token::Eof) {
            self.advance();
        }
        self.eat(&Token::Period);
    }

    /// Skip tokens until (and including) the next period or until `stop`
    /// is encountered (stop token is NOT consumed).
    pub(crate) fn sync_to_period_or(&mut self, stop: &Token) {
        while !matches!(self.peek(), Token::Period | Token::Eof) && self.peek() != stop {
            self.advance();
        }
        self.eat(&Token::Period);
    }

    // ── Diagnostics ───────────────────────────────────────────────────────────

    /// Hand out the next `EXEC RUST` block id, in source order (spec 041).
    pub(crate) fn next_exec_rust_id(&mut self) -> u32 {
        let id = self.next_block_id;
        self.next_block_id += 1;
        id
    }

    pub(crate) fn emit_error(&mut self, msg: impl Into<String>) {
        let span = self.peek_span();
        self.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: msg.into(),
            span,
        });
    }

    pub(crate) fn emit_warning(&mut self, msg: impl Into<String>) {
        let span = self.peek_span();
        self.diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message: msg.into(),
            span,
        });
    }

    /// Scan the raw token stream for redeclared unique elements and emit a hard
    /// [`Severity::Error`] for each. A program unit must declare PROGRAM-ID and
    /// each of the ENVIRONMENT / DATA / PROCEDURE DIVISION headers at most once.
    ///
    /// Program-unit boundaries are tracked structurally: a new IDENTIFICATION
    /// DIVISION (or `ID DIVISION`) starts a fresh unit — that covers both nested
    /// and sequentially-written programs — and an `END PROGRAM` closes one. The
    /// per-unit counters reset at each boundary, so a legitimate nested program
    /// is never mistaken for a redeclaration.
    fn detect_duplicate_declarations(&mut self) {
        #[derive(Default)]
        struct Counts {
            program_id: u32,
            environment: u32,
            data: u32,
            procedure: u32,
        }
        let mut counts = Counts::default();
        let mut errors: Vec<(String, Span)> = Vec::new();

        let mut i = 0;
        while i < self.tokens.len() {
            // Skip the free text of an IDENTIFICATION comment-entry.
            //
            // A comment-entry may say anything, including the words "PROCEDURE
            // DIVISION" — `SECURITY. SEE THE PROCEDURE DIVISION BELOW.` is legal
            // COBOL-85. This scan runs over raw tokens before anything is
            // parsed, so without this it would count that prose as a real
            // division header and report a redeclaration that does not exist.
            if let Some(mut j) = crate::identification::comment_entry_header_at(&self.tokens, i) {
                while j < self.tokens.len()
                    && !crate::identification::ends_comment_entry_at(&self.tokens, j)
                {
                    j += 1;
                }
                i = j;
                continue;
            }

            let tok = &self.tokens[i].token;
            let span = self.tokens[i].span;
            let next = self.tokens.get(i + 1).map(|s| &s.token);
            match tok {
                // A new IDENTIFICATION/ID DIVISION begins a fresh program unit.
                Token::Identification => {
                    counts = Counts::default();
                }
                // END PROGRAM closes the current unit.
                Token::End if next == Some(&Token::Program) => {
                    counts = Counts::default();
                }
                Token::ProgramId => {
                    counts.program_id += 1;
                    if counts.program_id > 1 {
                        errors.push((
                            "PROGRAM-ID is declared more than once in the same \
                             program unit; each program may have only one PROGRAM-ID"
                                .to_string(),
                            span,
                        ));
                    }
                }
                Token::Environment if next == Some(&Token::Division) => {
                    counts.environment += 1;
                    if counts.environment > 1 {
                        errors.push((
                            "ENVIRONMENT DIVISION is declared more than once in the \
                             same program unit"
                                .to_string(),
                            span,
                        ));
                    }
                }
                Token::Data if next == Some(&Token::Division) => {
                    counts.data += 1;
                    if counts.data > 1 {
                        errors.push((
                            "DATA DIVISION is declared more than once in the same \
                             program unit"
                                .to_string(),
                            span,
                        ));
                    }
                }
                Token::Procedure if next == Some(&Token::Division) => {
                    counts.procedure += 1;
                    if counts.procedure > 1 {
                        errors.push((
                            "PROCEDURE DIVISION is declared more than once in the \
                             same program unit"
                                .to_string(),
                            span,
                        ));
                    }
                }
                _ => {}
            }
            i += 1;
        }

        for (message, span) in errors {
            self.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message,
                span,
            });
        }
    }

    // ── Top-level parse ───────────────────────────────────────────────────────

    pub fn parse_program(mut self) -> ParseResult {
        // Before structural parsing, scan the raw token stream for redeclared
        // unique elements (a second PROGRAM-ID, or a second ENVIRONMENT/DATA/
        // PROCEDURE DIVISION header within the same program unit). The AST keeps
        // only one of each, so these duplicates are invisible after parsing and
        // must be detected here.
        self.detect_duplicate_declarations();

        let mut program = parse_single_program(&mut self);

        // A source file may hold several program units written *in sequence* —
        // each terminated by its own `END PROGRAM name.` (separately-structured
        // units), as opposed to true nesting (units appearing before the first
        // program's terminator, which `parse_single_program` already collects).
        // Both forms share one run unit and are dispatched by the runtime's flat
        // program registry, so attach any trailing siblings as nested programs of
        // the first so they remain CALL-able.
        loop {
            while self.eat(&Token::Period) {}
            if !self.at(&Token::Identification) {
                break;
            }
            let sibling = parse_single_program(&mut self);
            program.nested_programs.push(sibling);
        }

        ParseResult {
            program: Some(program),
            diagnostics: self.diagnostics,
            next_block_id: self.next_block_id,
        }
    }
}

/// Parse one complete COBOL program (outer or nested).
///
/// Expects the cursor to be positioned at `IDENTIFICATION` (or `ID`).
/// Consumes through `END PROGRAM name.` if present, collecting any
/// nested programs found between the PROCEDURE DIVISION and the terminator.
pub(crate) fn parse_single_program(p: &mut Parser) -> cobolt_ast::program::Program {
    use cobolt_ast::program::Program;

    // IDENTIFICATION DIVISION (required)
    let identification = parse_identification_division(p);

    // ENVIRONMENT DIVISION (optional)
    let environment = if p.at(&Token::Environment) {
        parse_environment_division(p)
    } else {
        None
    };

    // DATA DIVISION (optional)
    let data = if p.at(&Token::Data) {
        parse_data_division(p)
    } else {
        None
    };

    // Give each SPECIAL-NAMES switch a home in WORKING-STORAGE before the
    // PROCEDURE DIVISION is parsed, so `IF ON-SWITCH-1` resolves through the
    // ordinary 88-level machinery instead of needing a switch concept of its
    // own. See `Parser::switches`.
    let mut data = data;
    let switch_names: Vec<(String, String)> = p
        .switches
        .iter()
        .map(|(external, mnemonic, _, _)| (external.clone(), mnemonic.clone()))
        .collect();
    if !p.switches.is_empty() {
        let switches = std::mem::take(&mut p.switches);
        let decls: Vec<DataDecl> = switches
            .into_iter()
            .map(|(_, mnemonic, on_name, off_name)| {
                let mut children = Vec::new();
                if let Some(on) = on_name {
                    children.push(switch_status_decl(on, "1"));
                }
                if let Some(off) = off_name {
                    children.push(switch_status_decl(off, "0"));
                }
                DataDecl {
                    level: 1,
                    name: Some(mnemonic),
                    picture: Some(PicClause {
                        template: "X".into(),
                        kind: PicKind::Alphanumeric,
                        digits: 1,
                        decimals: 0,
                        span: Span::dummy(),
                    }),
                    // A switch is off until something turns it on.
                    value: Some(Literal::String("0".into())),
                    usage: Usage::Display,
                    object_class: None,
                    occurs: None,
                    redefines: None,
                    renames: None,
                    condition_values: Vec::new(),
                    is_global: false,
                    is_external: false,
                    blank_when_zero: false,
                    children,
                    span: Span::dummy(),
                    justified: false,
                    sign: None,
                }
            })
            .collect();
        let dd = data.get_or_insert_with(|| DataDivision {
            sections: Vec::new(),
            span: Span::dummy(),
        });
        match dd
            .sections
            .iter_mut()
            .find_map(|s| match s {
                DataSection::WorkingStorage(items) => Some(items),
                _ => None,
            }) {
            Some(items) => items.extend(decls),
            None => dd.sections.push(DataSection::WorkingStorage(decls)),
        }
    }

    // PROCEDURE DIVISION (required)
    let procedure = parse_procedure_division(p);

    // Claim this program's REPOSITORY and item-level EXEC RUST blocks **before**
    // any nested program is parsed.
    //
    // Both are accumulated in parser state and moved out when a `Program` is
    // built. Nested programs are parsed below, so if the move happened at the
    // end, the first nested program to be built would take the outer program's
    // entries and the outer would be left with none — which is exactly what
    // happened: a form's `CLASS RUST-STRING IS "Rust.String"` vanished the
    // moment the form gained an event handler, and every `OBJECT REFERENCE`
    // item in it silently stopped resolving.
    let repository = std::mem::take(&mut p.repository);
    let rust_items = std::mem::take(&mut p.rust_items);
    let classes = std::mem::take(&mut p.classes);
    let alphabets = std::mem::take(&mut p.alphabets);
    let collating_sequence = p.collating_sequence.take();

    // Collect nested programs until END PROGRAM or EOF
    let mut nested_programs = Vec::new();
    let mut end_program_name: Option<String> = None;

    loop {
        // Skip stray periods between nested programs
        while p.eat(&Token::Period) {}

        if p.at(&Token::Eof) {
            break;
        }

        // Nested program starts with IDENTIFICATION (or ID) DIVISION
        if p.at(&Token::Identification) {
            let nested = parse_single_program(p);
            nested_programs.push(nested);
            continue;
        }

        // END PROGRAM name.
        if p.at(&Token::End) && matches!(p.peek_at(1), Token::Program) {
            p.advance(); // END
            p.advance(); // PROGRAM
            end_program_name = p.eat_identifier().map(|(n, _)| n);
            p.expect_period();
            break;
        }

        // Anything else — stop (outer caller will handle it)
        break;
    }

    Program {
        span: Span::dummy(),
        rust_items,
        identification,
        environment,
        data,
        procedure,
        nested_programs,
        end_program_name,
        decimal_comma: p.decimal_comma,
        repository,
        classes,
        switch_names,
        alphabets,
        collating_sequence,
        currency: p.currency,
    }
}

/// Parse the ENVIRONMENT DIVISION, capturing the INPUT-OUTPUT SECTION's
/// FILE-CONTROL entries (SELECT … ASSIGN …). The CONFIGURATION SECTION is
/// skipped. Stops at DATA / PROCEDURE / END / EOF.
/// One `88 <status-name> VALUE "<digit>"` for a switch's ON or OFF condition.
fn switch_status_decl(name: String, digit: &str) -> DataDecl {
    DataDecl {
        level: 88,
        name: Some(name),
        picture: None,
        value: None,
        usage: Usage::Display,
        object_class: None,
        occurs: None,
        redefines: None,
        renames: None,
        condition_values: vec![ConditionValue::Single(Literal::String(digit.into()))],
        is_global: false,
        is_external: false,
        blank_when_zero: false,
        children: Vec::new(),
        span: Span::dummy(),
        justified: false,
        sign: None,
    }
}

/// `true` when the cursor sits on a SPECIAL-NAMES **switch** clause.
///
/// The shape `<implementor-name> IS <mnemonic>` is shared with the ordinary
/// mnemonic clause (`CONSOLE IS CRT`), so what identifies a switch is the
/// `ON`/`OFF` status clause that follows it. Requiring one keeps every other
/// mnemonic on the skip path it has always taken.
fn at_switch_clause(p: &Parser) -> bool {
    if !matches!(p.peek(), Token::Identifier(_)) {
        return false;
    }
    // `IS` is optional here too: `SWITCH-1 SW-1 ON STATUS IS …`.
    let after = if matches!(p.peek_at(1), Token::Is) { 2 } else { 1 };
    if !matches!(p.peek_at(after), Token::Identifier(_)) {
        return false;
    }
    matches!(p.peek_at(after + 1), Token::On)
        || matches!(p.peek_at(after + 1), Token::Identifier(w) if w.eq_ignore_ascii_case("OFF"))
}

/// Parse `<implementor-name> [IS] <mnemonic> {ON|OFF} [STATUS] [IS] <name> …`.
///
/// The switch itself is not modelled: [`Parser::switches`] records the mnemonic
/// and its status names, and the data division synthesises a one-character item
/// with an 88 per status. See [`Parser::switches`] for why.
fn parse_special_names_switch(p: &mut Parser) {
    // The implementor's switch name (XXXXX051, SWITCH-1, …) — how the switch is
    // known *outside* the program, and so how it is set.
    let external = match p.peek() {
        Token::Identifier(n) => n.to_ascii_uppercase(),
        _ => return,
    };
    p.advance();
    p.eat(&Token::Is);
    let mnemonic = match p.peek() {
        Token::Identifier(n) => {
            let n = n.to_ascii_uppercase();
            p.advance();
            n
        }
        _ => return,
    };
    let mut on_name: Option<String> = None;
    let mut off_name: Option<String> = None;
    loop {
        let is_on = matches!(p.peek(), Token::On);
        let is_off = matches!(p.peek(), Token::Identifier(w) if w.eq_ignore_ascii_case("OFF"));
        if !is_on && !is_off {
            break;
        }
        p.advance(); // ON / OFF
        p.eat(&Token::Status); // optional STATUS
        p.eat(&Token::Is); // optional IS
        let name = match p.peek() {
            Token::Identifier(n) => {
                let n = n.to_ascii_uppercase();
                p.advance();
                n
            }
            _ => break,
        };
        if is_on {
            on_name = Some(name);
        } else {
            off_name = Some(name);
        }
    }
    p.switches.push((external, mnemonic, on_name, off_name));
}

/// `true` when the cursor sits on an ordinary SPECIAL-NAMES **mnemonic** clause
/// — `<implementor-name> [IS] <mnemonic>` with no `ON`/`OFF` status after it.
///
/// The caller has already ruled out a switch, so the remaining ambiguity is the
/// handful of SPECIAL-NAMES clauses that share the shape but name a facility
/// rather than a device (`CURSOR IS …`, `CRT STATUS IS …`); those open with a
/// reserved clause word, which is what excludes them here.
fn at_mnemonic_clause(p: &Parser) -> bool {
    let Token::Identifier(first) = p.peek() else {
        return false;
    };
    if is_special_names_clause_word(first) {
        return false;
    }
    let after = if matches!(p.peek_at(1), Token::Is) { 2 } else { 1 };
    // Without an explicit `IS`, two adjacent identifiers are the only reading —
    // but the second must not itself open the next clause, or `CONSOLE IS CRT
    // CURRENCY "$"` would claim CURRENCY as a mnemonic.
    matches!(p.peek_at(after), Token::Identifier(m) if !is_special_names_clause_word(m))
}

/// Parse `<implementor-name> [IS] <mnemonic>` and record the pair.
///
/// Nothing about the device is modelled: what the entry buys is the ability to
/// recognise the mnemonic later, so `ACCEPT x FROM <mnemonic>` reads the
/// hardware device (Format 1) instead of an environment variable.
fn parse_special_names_mnemonic(p: &mut Parser) {
    let Token::Identifier(system) = p.peek() else {
        return;
    };
    let system = system.to_ascii_uppercase();
    p.advance();
    p.eat(&Token::Is); // optional IS
    let Token::Identifier(mnemonic) = p.peek() else {
        return;
    };
    let mnemonic = mnemonic.to_ascii_uppercase();
    p.advance();
    p.mnemonics.push((system, mnemonic));
}

/// Parse `CLASS <name> [IS] {literal [{THROUGH|THRU} literal]} …`.
///
/// Operands the CCVS leaves as implementor placeholders (`XXXXX090`, an
/// *ordinal* the validator substitutes) are consumed and dropped: the class
/// then has fewer ranges than the source describes, which is a suite
/// substitution the harness does not perform — not a parse failure.
fn parse_special_names_class(p: &mut Parser) {
    p.advance(); // CLASS
    let name = match p.peek() {
        Token::Identifier(n) => {
            let n = n.to_ascii_uppercase();
            p.advance();
            n
        }
        _ => return,
    };
    p.eat(&Token::Is);

    /// One operand's characters, empty for a placeholder identifier.
    fn take_operand(p: &mut Parser) -> Option<Vec<char>> {
        match p.peek().clone() {
            Token::StringLiteral(s) => {
                p.advance();
                Some(s.chars().collect())
            }
            // A numeric operand is the character's ordinal position, 1-based.
            Token::IntegerLiteral(n, _) => {
                p.advance();
                Some(
                    u32::try_from(n - 1)
                        .ok()
                        .and_then(char::from_u32)
                        .into_iter()
                        .collect(),
                )
            }
            Token::Identifier(w) if !is_special_names_clause_word(&w) => {
                p.advance();
                Some(Vec::new())
            }
            _ => None,
        }
    }

    let mut ranges: Vec<(char, char)> = Vec::new();
    while let Some(first) = take_operand(p) {
        // `lit-1 THRU lit-2` is one inclusive range between the two literals'
        // first characters; a bare literal puts **every** character of itself
        // in the class, each as a range spanning only itself. `CLASS ABCD IS
        // "ABCD"` describing only `A` is how `'ADCBA' IS ACTUAL-ABCD` failed.
        if matches!(p.peek(), Token::Through | Token::Thru) {
            p.advance();
            let second = take_operand(p).unwrap_or_default();
            if let (Some(a), Some(b)) = (first.first(), second.first()) {
                ranges.push(if a <= b { (*a, *b) } else { (*b, *a) });
            }
            continue;
        }
        ranges.extend(first.into_iter().map(|c| (c, c)));
    }
    p.classes.push((name, ranges));
}

/// `SPECIAL-NAMES. ALPHABET alphabet-name IS {NATIVE | STANDARD-1 | STANDARD-2
/// | EBCDIC | literal-phrase}`.
///
/// The literal phrase is an ordered list of *positions*. A bare literal puts
/// each of its characters at the next position in turn; `lit-1 THRU lit-2`
/// expands to one position per character of the inclusive native range; and
/// `ALSO` folds the operands it joins into a single shared position, which is
/// how `"I" ALSO "J" ALSO "K"` makes those three characters compare equal.
/// A figurative constant names its native character, so `ALSO HIGH-VALUE`
/// gives `0xFF` the position it is written at (NC215A, NC219A).
fn parse_special_names_alphabet(p: &mut Parser) {
    use cobolt_ast::program::AlphabetSpec;
    p.advance(); // ALPHABET
    let Some((name, _)) = p.eat_identifier() else {
        return;
    };
    let name = name.to_ascii_uppercase();
    p.eat(&Token::Is);

    // A named standard sequence stands alone — no operand list follows.
    if let Token::Identifier(w) = p.peek().clone() {
        let spec = match w.to_ascii_uppercase().as_str() {
            "NATIVE" => Some(AlphabetSpec::Native),
            "STANDARD-1" | "STANDARD-2" => Some(AlphabetSpec::Standard),
            "EBCDIC" => Some(AlphabetSpec::Ebcdic),
            _ => None,
        };
        if let Some(spec) = spec {
            p.advance();
            p.alphabets.push((name, spec));
            return;
        }
    }

    /// `,` and `;` are pure separators in COBOL and may sit between any two
    /// operands — NC215A writes `"I" ALSO "J", ALSO "K", ALSO "L"`. Stopping
    /// at one truncated the alphabet silently: every character after the first
    /// comma went unlisted, so `ALSO` never folded and the rest of the sequence
    /// fell back to native order.
    fn skip_separators(p: &mut Parser) {
        while p.eat(&Token::Comma) || p.eat(&Token::Semicolon) {}
    }

    /// One operand's characters; `None` when the cursor is not on an operand.
    fn take_operand(p: &mut Parser) -> Option<Vec<char>> {
        skip_separators(p);
        match p.peek().clone() {
            Token::StringLiteral(s) => {
                p.advance();
                Some(s.chars().collect())
            }
            // A numeric operand is the character's ordinal position, 1-based.
            Token::IntegerLiteral(n, _) => {
                p.advance();
                Some(
                    u32::try_from(n - 1)
                        .ok()
                        .and_then(char::from_u32)
                        .into_iter()
                        .collect(),
                )
            }
            Token::HighValues => {
                p.advance();
                Some(vec!['\u{ff}'])
            }
            Token::LowValues => {
                p.advance();
                Some(vec!['\u{0}'])
            }
            Token::Spaces => {
                p.advance();
                Some(vec![' '])
            }
            Token::Quotes => {
                p.advance();
                Some(vec!['"'])
            }
            Token::Zeros => {
                p.advance();
                Some(vec!['0'])
            }
            _ => None,
        }
    }

    let mut groups: Vec<Vec<char>> = Vec::new();
    while let Some(first) = take_operand(p) {
        skip_separators(p);
        // `lit-1 THRU lit-2` — one position per character of the native range.
        if matches!(p.peek(), Token::Through | Token::Thru) {
            p.advance();
            let second = take_operand(p).unwrap_or_default();
            if let (Some(&a), Some(&b)) = (first.first(), second.first()) {
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                for c in lo..=hi {
                    groups.push(vec![c]);
                }
            }
            continue;
        }
        // `lit-1 ALSO lit-2 ALSO …` — every operand shares ONE position.
        let mut group: Vec<char> = first;
        let mut folded = false;
        loop {
            skip_separators(p);
            // `ALSO` is a reserved word, not an identifier — matching it as one
            // meant the fold never triggered and every operand took a position
            // of its own, so `"I" ALSO "J"` left I and J unequal.
            if !p.eat(&Token::Also) {
                break;
            }
            let Some(more) = take_operand(p) else { break };
            group.extend(more);
            folded = true;
        }
        if folded {
            groups.push(group);
        } else {
            // A bare literal contributes each of its characters in turn, so the
            // whole COBOL character set can be written as one long literal.
            groups.extend(group.into_iter().map(|c| vec![c]));
        }
    }
    p.alphabets.push((name, AlphabetSpec::Literal(groups)));
}

/// Words that open another SPECIAL-NAMES clause, so an operand scan must stop
/// rather than swallow them.
fn is_special_names_clause_word(w: &str) -> bool {
    [
        "CLASS",
        "ALPHABET",
        "SYMBOLIC",
        "CURRENCY",
        "DECIMAL-POINT",
        "CURSOR",
        "CRT",
        "REPOSITORY",
    ]
    .iter()
    .any(|k| w.eq_ignore_ascii_case(k))
}

fn parse_environment_division(p: &mut Parser) -> Option<EnvironmentDivision> {
    let span = p.peek_span();
    p.advance(); // ENVIRONMENT
    p.eat(&Token::Division);
    p.expect_period();

    let mut input_output: Option<InputOutputSection> = None;

    while !matches!(
        p.peek(),
        Token::Data | Token::Procedure | Token::Identification | Token::Eof
    ) {
        match p.peek() {
            Token::Configuration => {
                p.advance();
                p.eat(&Token::Section);
                p.expect_period();
                // Skip configuration paragraphs until the next section/division,
                // but capture `DECIMAL-POINT IS COMMA`, the SPECIAL-NAMES
                // switches and classes, and the REPOSITORY bindings.
                let mut in_repository = false;
                // Only the SPECIAL-NAMES paragraph declares mnemonics. The
                // `<name> IS <name>` shape also occurs elsewhere in this
                // section, so the paragraph the cursor is in is what makes a
                // clause a mnemonic — the same reasoning `in_repository`
                // already uses to tell the two meanings of CLASS apart.
                let mut in_special_names = false;
                while !matches!(
                    p.peek(),
                    Token::InputOutput
                        | Token::Data
                        | Token::Procedure
                        | Token::Identification
                        | Token::Eof
                ) {
                    // Item-level `EXEC RUST … END-EXEC` (spec 041 R19). The
                    // lexer already collapsed the whole block into one token,
                    // so this only has to file it. It sits here, in the
                    // CONFIGURATION SECTION beside REPOSITORY, because both
                    // declare what Rust types the program has — and because a
                    // statement-level block compiles to a function body, where
                    // a `struct` or `impl` could not be seen by another block.
                    if let Token::ExecRustBlock(src) = p.peek() {
                        let source = src.clone();
                        let span = p.peek_span();
                        p.advance();
                        p.eat(&Token::Period);
                        p.rust_items.push(RustItemBlock { source, span });
                        continue;
                    }
                    // `OBJECT-COMPUTER. … PROGRAM COLLATING SEQUENCE IS name`.
                    // `PROGRAM` lexes as its own keyword rather than an
                    // identifier, so it is matched here and not in the
                    // identifier dispatch below.
                    if matches!(p.peek(), Token::Program)
                        && matches!(p.peek_at(1), Token::Identifier(w) if w.eq_ignore_ascii_case("COLLATING"))
                    {
                        p.advance(); // PROGRAM
                        p.advance(); // COLLATING
                        p.eat(&Token::Sequence);
                        p.eat(&Token::Is);
                        if let Some((n, _)) = p.eat_identifier() {
                            p.collating_sequence = Some(n.to_ascii_uppercase());
                        }
                        continue;
                    }
                    let ident = if let Token::Identifier(s) = p.peek() {
                        Some(s.clone())
                    } else {
                        None
                    };
                    match ident.as_deref() {
                        // `CLASS` means two different things in this section:
                        // under REPOSITORY it binds a Rust type (spec 005),
                        // under SPECIAL-NAMES it defines a character class.
                        // Nothing about the clause itself tells them apart, so
                        // the paragraph the cursor is in does.
                        Some(s) if s.eq_ignore_ascii_case("REPOSITORY") => {
                            in_repository = true;
                            in_special_names = false;
                            p.advance();
                        }
                        Some(s) if s.eq_ignore_ascii_case("SPECIAL-NAMES") => {
                            in_repository = false;
                            in_special_names = true;
                            p.advance();
                        }
                        Some(s)
                            if s.eq_ignore_ascii_case("SOURCE-COMPUTER")
                                || s.eq_ignore_ascii_case("OBJECT-COMPUTER") =>
                        {
                            in_special_names = false;
                            p.advance();
                        }
                        Some(s) if s.eq_ignore_ascii_case("DECIMAL-POINT") => {
                            // … IS COMMA  (IS optional) within the next few tokens
                            for k in 1..=3 {
                                if let Token::Identifier(s2) = p.peek_at(k) {
                                    if s2.eq_ignore_ascii_case("COMMA") {
                                        p.decimal_comma = true;
                                        break;
                                    }
                                }
                            }
                            p.advance();
                        }
                        // SPECIAL-NAMES: CURRENCY [SIGN] [IS] "<char>"
                        Some(s) if s.eq_ignore_ascii_case("CURRENCY") => {
                            p.advance(); // CURRENCY
                            // SIGN and IS are both optional — NC108M writes the
                            // clause as a bare `CURRENCY "<".`. SIGN has its own
                            // token (the data-description `SIGN IS LEADING`
                            // clause needs one), so matching it as an identifier
                            // silently skipped nothing and left the literal
                            // unread.
                            if p.at(&Token::Sign) {
                                p.advance();
                            }
                            p.eat(&Token::Is);
                            if let Token::StringLiteral(lit) = p.peek() {
                                // The standard allows exactly one character.
                                // A longer literal is left alone rather than
                                // truncated into something the program did not
                                // ask for.
                                let mut cs = lit.chars();
                                if let (Some(c), None) = (cs.next(), cs.next()) {
                                    p.currency = c;
                                }
                                p.advance();
                            }
                        }
                        // SPECIAL-NAMES: CLASS <name> [IS] lit [THRU lit] …
                        Some(s) if s.eq_ignore_ascii_case("CLASS") && !in_repository => {
                            parse_special_names_class(p);
                        }
                        // SPECIAL-NAMES: ALPHABET <name> [IS] <sequence>
                        Some(s) if s.eq_ignore_ascii_case("ALPHABET") && !in_repository => {
                            parse_special_names_alphabet(p);
                        }
                        // A switch: `<implementor-name> IS <mnemonic>` followed
                        // by an ON and/or OFF status clause. Without one of
                        // those clauses the same shape is an ordinary mnemonic
                        // (`CONSOLE IS CRT`), which stays skipped.
                        Some(_) if at_switch_clause(p) => {
                            parse_special_names_switch(p);
                        }
                        // `<implementor-name> [IS] <mnemonic>` — the ordinary
                        // mnemonic clause. It used to be skipped a token at a
                        // time, which left `ACCEPT x FROM <mnemonic>` unable to
                        // tell a declared device from an environment variable.
                        Some(_) if in_special_names && at_mnemonic_clause(p) => {
                            parse_special_names_mnemonic(p);
                        }
                        // REPOSITORY: CLASS <name> [IS|AS] "<external>"  (spec 005).
                        Some(s) if s.eq_ignore_ascii_case("CLASS") => {
                            p.advance(); // CLASS
                            let name = if let Token::Identifier(n) = p.peek() {
                                let n = n.clone();
                                p.advance();
                                n
                            } else {
                                String::new()
                            };
                            p.eat(&Token::Is); // optional IS
                            if matches!(p.peek(), Token::Identifier(a) if a.eq_ignore_ascii_case("AS"))
                            {
                                p.advance(); // optional AS
                            }
                            if let Token::StringLiteral(ext) = p.peek() {
                                let ext = ext.clone();
                                if !name.is_empty() {
                                    p.repository.push((name.to_ascii_uppercase(), ext));
                                }
                                p.advance();
                            }
                        }
                        _ => {
                            p.advance();
                        }
                    }
                }
            }
            Token::InputOutput => {
                let io_span = p.peek_span();
                p.advance();
                p.eat(&Token::Section);
                p.expect_period();

                let mut file_controls = Vec::new();
                if p.at(&Token::FileControl) {
                    p.advance();
                    p.expect_period();
                    while p.at(&Token::Select) {
                        if let Some(fc) = parse_file_control_entry(p) {
                            file_controls.push(fc);
                        }
                    }
                }
                input_output = Some(InputOutputSection {
                    file_controls,
                    span: io_span,
                });
            }
            _ => {
                p.advance();
            }
        }
    }

    Some(EnvironmentDivision {
        configuration: None,
        input_output,
        span,
    })
}

/// True if the token is the `COMPRESSION` word of a `WITH COMPRESSION` clause.
fn is_compression(tok: &cobolt_lexer::Token) -> bool {
    matches!(tok, cobolt_lexer::Token::Identifier(w) if w.eq_ignore_ascii_case("COMPRESSION"))
}

fn is_persistence(tok: &cobolt_lexer::Token) -> bool {
    matches!(tok, cobolt_lexer::Token::Identifier(w) if w.eq_ignore_ascii_case("PERSISTENCE"))
}

/// Parse a single `SELECT … ASSIGN …` entry in FILE-CONTROL.
fn parse_file_control_entry(p: &mut Parser) -> Option<FileControl> {
    let span = p.peek_span();
    p.advance(); // SELECT

    // Optional OPTIONAL keyword (no dedicated token).
    if let Token::Identifier(w) = p.peek() {
        if w.eq_ignore_ascii_case("OPTIONAL") {
            p.advance();
        }
    }

    let name = p.expect_identifier("file name in SELECT");

    let mut assign = String::new();
    let mut organization = FileOrganization::Sequential;
    let mut access = AccessMode::Sequential;
    let mut record_key: Option<String> = None;
    let mut file_status: Option<String> = None;
    let mut alternate_keys: Vec<AlternateKey> = Vec::new();
    // No STORAGE clause ⇒ default to DISK.
    let mut storage_mode = StorageMode::Disk;
    let mut data_compressing = false;
    let mut persist = false;

    while !p.at(&Token::Period) && !p.at(&Token::Eof) {
        // Clauses introduced by a non-keyword word (STORAGE, ALTERNATE).
        if let Token::Identifier(id) = p.peek() {
            match id.to_ascii_uppercase().as_str() {
                // STORAGE [MODE] IS MEMORY | DISK  [WITH COMPRESSION]
                "STORAGE" => {
                    p.advance(); // STORAGE
                    p.eat(&Token::Mode); // optional MODE
                    p.eat(&Token::Is);
                    if let Some((w, _)) = p.eat_identifier() {
                        storage_mode = if w.eq_ignore_ascii_case("MEMORY") {
                            StorageMode::Memory
                        } else {
                            StorageMode::Disk
                        };
                    }
                    // optional `WITH {COMPRESSION | PERSISTENCE}` phrases, in
                    // any order and repeatable.
                    while p.at(&Token::With)
                        && (is_compression(p.peek_at(1)) || is_persistence(p.peek_at(1)))
                    {
                        p.advance(); // WITH
                        if is_compression(p.peek()) {
                            data_compressing = true;
                        } else {
                            persist = true;
                        }
                        p.advance(); // COMPRESSION | PERSISTENCE
                    }
                    continue;
                }
                // ALTERNATE [RECORD] KEY [IS] data-name [WITH DUPLICATES]
                "ALTERNATE" => {
                    p.advance(); // ALTERNATE
                    p.eat(&Token::Record);
                    p.eat(&Token::Key);
                    p.eat(&Token::Is);
                    if let Some((field, _)) = p.eat_identifier() {
                        let mut with_duplicates = false;
                        p.eat(&Token::With);
                        if let Token::Identifier(d) = p.peek() {
                            if d.eq_ignore_ascii_case("DUPLICATES") {
                                p.advance();
                                with_duplicates = true;
                            }
                        }
                        alternate_keys.push(AlternateKey {
                            field,
                            with_duplicates,
                        });
                    }
                    continue;
                }
                _ => {}
            }
        }
        // A standalone "WITH COMPRESSION" / "WITH PERSISTENCE" clause (no STORAGE
        // clause); the file uses the default storage backend with that option on.
        if p.at(&Token::With) && (is_compression(p.peek_at(1)) || is_persistence(p.peek_at(1))) {
            p.advance(); // WITH
            if is_compression(p.peek()) {
                data_compressing = true;
            } else {
                persist = true;
            }
            p.advance(); // COMPRESSION | PERSISTENCE
            continue;
        }
        match p.peek() {
            Token::Assign => {
                p.advance();
                p.eat(&Token::To);
                if let Some((s, _)) = p.eat_string() {
                    assign = s;
                } else if p.at_identifier() {
                    assign = p.eat_identifier().map(|(n, _)| n).unwrap_or_default();
                } else {
                    p.advance();
                }
            }
            Token::Organization => {
                p.advance();
                p.eat(&Token::Is);
                if p.eat(&Token::Line) {
                    p.eat(&Token::Sequential);
                    organization = FileOrganization::LineSequential;
                } else if p.eat(&Token::Sequential) {
                    organization = FileOrganization::Sequential;
                } else if p.eat(&Token::Relative) {
                    organization = FileOrganization::Relative;
                } else if p.eat(&Token::Indexed) {
                    organization = FileOrganization::Indexed;
                }
            }
            Token::Access => {
                p.advance();
                p.eat(&Token::Mode);
                p.eat(&Token::Is);
                if p.eat(&Token::Sequential) {
                    access = AccessMode::Sequential;
                } else if p.eat(&Token::Random) {
                    access = AccessMode::Random;
                } else if p.eat(&Token::Dynamic) {
                    access = AccessMode::Dynamic;
                }
            }
            // FILE STATUS [IS] data-name
            Token::File => {
                p.advance();
                if p.eat(&Token::Status) {
                    p.eat(&Token::Is);
                    if p.at_identifier() {
                        file_status = p.eat_identifier().map(|(n, _)| n);
                    }
                }
            }
            // STATUS [IS] data-name (FILE keyword omitted)
            Token::Status => {
                p.advance();
                p.eat(&Token::Is);
                if p.at_identifier() {
                    file_status = p.eat_identifier().map(|(n, _)| n);
                }
            }
            // RECORD KEY [IS] data-name
            Token::Record => {
                p.advance();
                if p.eat(&Token::Key) {
                    p.eat(&Token::Is);
                    if p.at_identifier() {
                        record_key = p.eat_identifier().map(|(n, _)| n);
                    }
                }
            }
            _ => {
                p.advance();
            }
        }
    }
    p.expect_period();

    Some(FileControl {
        name,
        assign,
        organization,
        access,
        record_key,
        alternate_keys,
        file_status,
        storage_mode,
        data_compressing,
        persist,
        span,
    })
}
