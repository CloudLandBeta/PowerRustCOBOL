// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Core [`Parser`] struct and token-cursor API.

use cobolt_lexer::{Span, SpannedToken, Token};
use cobolt_ast::program::{
    AccessMode, AlternateKey, EnvironmentDivision, FileControl, FileOrganization,
    InputOutputSection, StorageMode,
};

use crate::error::{Diagnostic, ParseResult, Severity};
use crate::identification::parse_identification_division;
use crate::data::parse_data_division;
use crate::procedure::parse_procedure_division;

// ── Parser ────────────────────────────────────────────────────────────────────

pub struct Parser {
    pub(crate) tokens: Vec<SpannedToken>,
    pub(crate) pos: usize,
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// Set by `SPECIAL-NAMES. DECIMAL-POINT IS COMMA`. When true, numeric literals
    /// use `,` as the decimal separator and edited PICs swap `.`/`,` roles.
    pub(crate) decimal_comma: bool,
}

impl Parser {
    pub fn new(tokens: Vec<SpannedToken>) -> Self {
        // Filter out comment tokens — the parser doesn't need them.
        let tokens: Vec<_> = tokens
            .into_iter()
            .filter(|st| !matches!(st.token, Token::Comment(_)))
            .collect();
        Self { tokens, pos: 0, diagnostics: Vec::new(), decimal_comma: false }
    }

    // ── Token inspection ─────────────────────────────────────────────────────

    /// Current token (does not advance).
    pub(crate) fn peek(&self) -> &Token {
        self.tokens.get(self.pos).map(|st| &st.token).unwrap_or(&Token::Eof)
    }

    /// Span of the current token.
    pub(crate) fn peek_span(&self) -> Span {
        self.tokens.get(self.pos).map(|st| st.span).unwrap_or(Span::dummy())
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
            self.emit_warning(format!(
                "expected '.', found {:?}",
                self.peek()
            ));
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

    // PROCEDURE DIVISION (required)
    let procedure = parse_procedure_division(p);

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
        identification,
        environment,
        data,
        procedure,
        nested_programs,
        end_program_name,
        decimal_comma: p.decimal_comma,
    }
}

/// Parse the ENVIRONMENT DIVISION, capturing the INPUT-OUTPUT SECTION's
/// FILE-CONTROL entries (SELECT … ASSIGN …). The CONFIGURATION SECTION is
/// skipped. Stops at DATA / PROCEDURE / END / EOF.
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
                // but capture `DECIMAL-POINT IS COMMA` from SPECIAL-NAMES.
                while !matches!(
                    p.peek(),
                    Token::InputOutput | Token::Data | Token::Procedure
                        | Token::Identification | Token::Eof
                ) {
                    if let Token::Identifier(s) = p.peek() {
                        if s.eq_ignore_ascii_case("DECIMAL-POINT") {
                            // … IS COMMA  (IS optional) within the next few tokens
                            for k in 1..=3 {
                                if let Token::Identifier(s2) = p.peek_at(k) {
                                    if s2.eq_ignore_ascii_case("COMMA") {
                                        p.decimal_comma = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    p.advance();
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
                input_output = Some(InputOutputSection { file_controls, span: io_span });
            }
            _ => { p.advance(); }
        }
    }

    Some(EnvironmentDivision { configuration: None, input_output, span })
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
        if w.eq_ignore_ascii_case("OPTIONAL") { p.advance(); }
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
                        if is_compression(p.peek()) { data_compressing = true; }
                        else { persist = true; }
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
                        alternate_keys.push(AlternateKey { field, with_duplicates });
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
            if is_compression(p.peek()) { data_compressing = true; }
            else { persist = true; }
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
                if p.eat(&Token::Sequential) { access = AccessMode::Sequential; }
                else if p.eat(&Token::Random) { access = AccessMode::Random; }
                else if p.eat(&Token::Dynamic) { access = AccessMode::Dynamic; }
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
            _ => { p.advance(); }
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
