// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DATA DIVISION parser.
//!
//! Parses the four standard sections (FILE, WORKING-STORAGE, LOCAL-STORAGE,
//! LINKAGE, SCREEN) and builds the level-number tree for each section's data
//! items.

use cobolt_ast::data::{
    ConditionValue, DataDecl, FileDescription, Linage, OccursClause, PicClause, PicKind, ScreenItem,
    Usage,
};
use cobolt_ast::expr::Literal;
use cobolt_ast::program::{DataDivision, DataSection};
use cobolt_lexer::{Span, Token};

use crate::expr::parse_literal;
use crate::stmt::is_word;
use crate::parser::Parser;

// ── Entry point ───────────────────────────────────────────────────────────────

/// Parse the DATA DIVISION (returns `None` if the division is absent).
pub(crate) fn parse_data_division(p: &mut Parser) -> Option<DataDivision> {
    if !p.at(&Token::Data) {
        return None;
    }
    let span = p.peek_span();
    p.advance(); // DATA
    p.expect(&Token::Division);
    p.expect_period();

    let mut sections: Vec<DataSection> = Vec::new();

    loop {
        match p.peek().clone() {
            // FILE SECTION.
            Token::File => {
                p.advance();
                p.expect(&Token::Section);
                p.expect_period();
                let fds = parse_file_section(p);
                sections.push(DataSection::FileSection(fds));
            }
            // WORKING-STORAGE SECTION.
            Token::WorkingStorage => {
                p.advance();
                p.expect(&Token::Section);
                p.expect_period();
                let decls = parse_data_declarations(p);
                sections.push(DataSection::WorkingStorage(build_tree(decls)));
            }
            // LOCAL-STORAGE SECTION.
            Token::LocalStorage => {
                p.advance();
                p.expect(&Token::Section);
                p.expect_period();
                let decls = parse_data_declarations(p);
                sections.push(DataSection::LocalStorage(build_tree(decls)));
            }
            // LINKAGE SECTION.
            Token::Linkage => {
                p.advance();
                p.expect(&Token::Section);
                p.expect_period();
                let decls = parse_data_declarations(p);
                sections.push(DataSection::Linkage(build_tree(decls)));
            }
            // SCREEN SECTION.
            Token::Screen => {
                p.advance();
                p.expect(&Token::Section);
                p.expect_period();
                let items = parse_screen_section(p);
                sections.push(DataSection::Screen(items));
            }
            // Next division header or EOF — stop.
            Token::Procedure | Token::Environment | Token::Identification | Token::Eof => break,
            // Spec 041 R21 — placement. An `EXEC RUST` block is either an
            // item-level one (CONFIGURATION SECTION, after REPOSITORY) or a
            // statement-level one (PROCEDURE DIVISION). The DATA DIVISION is
            // neither, and a hard error here is the point of the feature: this
            // used to be the kind of thing the old executor swallowed.
            Token::ExecRustBlock(_) => {
                // The advice has to work for the developer who is looking at a
                // FORM, not at a listing. In the designer there are no division
                // headers to aim at — there are COBOL Structure blocks — and the
                // one that lands inside the CONFIGURATION SECTION is REPOSITORY.
                // Naming only the divisions sent people to WORKING-STORAGE,
                // which is the DATA DIVISION, and straight back to this error.
                p.emit_error(
                    "EXEC RUST is not allowed in the DATA DIVISION. An item-level \
                     block (use, struct, impl, fn) goes in the CONFIGURATION \
                     SECTION after REPOSITORY — in a form, that is the COBOL \
                     Structure panel's REPOSITORY block, below the CLASS entries, \
                     NOT WORKING-STORAGE. A statement-level block goes in the \
                     PROCEDURE DIVISION — in a form, an event handler or a common \
                     procedure",
                );
                p.advance();
                p.eat(&Token::Period);
            }
            // Unknown — skip with a warning.
            _ => {
                p.emit_warning(format!("unexpected token in DATA DIVISION: {:?}", p.peek()));
                p.sync_to_period();
            }
        }
    }

    Some(DataDivision { sections, span })
}

// ── File section ──────────────────────────────────────────────────────────────

fn parse_file_section(p: &mut Parser) -> Vec<FileDescription> {
    let mut fds = Vec::new();
    while p.at(&Token::Fd) || p.at(&Token::Sd) {
        let span = p.peek_span();
        p.advance(); // FD or SD
        let name = p.expect_identifier("file name");
        let mut is_global = false;
        let mut linage: Option<Linage> = None;
        // Consume optional clauses until period. Most are recorded elsewhere or
        // do not affect execution; LINAGE does, so it is read rather than
        // skipped.
        while !p.at(&Token::Period) && !p.at(&Token::Eof) {
            if p.at(&Token::Global) {
                is_global = true;
            }
            if is_word(p.peek(), "LINAGE") {
                linage = parse_linage_clause(p);
                continue;
            }
            p.advance();
        }
        p.expect_period();
        // Parse record descriptions
        let records = parse_data_declarations(p);
        fds.push(FileDescription {
            name,
            is_global,
            records: build_tree(records),
            linage,
            span,
        });
    }
    fds
}

/// Parse `LINAGE IS n LINES [WITH FOOTING AT f] [LINES AT TOP t]
/// [LINES AT BOTTOM b]`, positioned on the word `LINAGE`.
///
/// Every noise word is optional (`IS`, `LINES`, `WITH`, `AT`), so the clause is
/// read by looking for its four numbers in order rather than by matching a
/// fixed shape — which is also why a missing `FOOTING` defaults to the body
/// size: end of page is then raised only when the body is full, exactly as the
/// standard says.
///
/// None of `LINAGE`, `FOOTING`, `TOP` or `BOTTOM` is a lexer keyword, so they
/// are matched by spelling — the same approach `CLOSE … WITH LOCK` uses, and it
/// leaves a developer free to name an item `TOP`.
fn parse_linage_clause(p: &mut Parser) -> Option<Linage> {
    p.advance(); // LINAGE
    p.eat(&Token::Is);
    let lines = eat_required_integer(p)?;
    // `LINES` and `LINE` are lexer keywords; the rest of the clause's words are
    // not. Both spellings are noise here.
    if !p.eat(&Token::Lines) {
        p.eat(&Token::Line);
    }

    let mut footing = None;
    let mut top = 0u32;
    let mut bottom = 0u32;

    loop {
        p.eat(&Token::With);
        if is_word(p.peek(), "FOOTING") {
            p.advance();
            if is_word(p.peek(), "AT") { p.advance(); }
            footing = eat_required_integer(p);
            continue;
        }
        if p.at(&Token::Lines) || p.at(&Token::Line) {
            // `LINES AT TOP n` / `LINES AT BOTTOM n`
            p.advance();
            if is_word(p.peek(), "AT") { p.advance(); }
            if is_word(p.peek(), "TOP") {
                p.advance();
                top = eat_required_integer(p).unwrap_or(0);
                continue;
            }
            if is_word(p.peek(), "BOTTOM") {
                p.advance();
                bottom = eat_required_integer(p).unwrap_or(0);
                continue;
            }
            continue;
        }
        if is_word(p.peek(), "TOP") {
            p.advance();
            top = eat_required_integer(p).unwrap_or(0);
            continue;
        }
        if is_word(p.peek(), "BOTTOM") {
            p.advance();
            bottom = eat_required_integer(p).unwrap_or(0);
            continue;
        }
        break;
    }

    Some(Linage {
        lines,
        footing: footing.unwrap_or(lines),
        top,
        bottom,
    })
}

// ── Screen section (simplified) ───────────────────────────────────────────────

fn parse_screen_section(p: &mut Parser) -> Vec<ScreenItem> {
    // For MVP: collect screen items flat without tree building.
    let mut items = Vec::new();
    while let Token::LevelNumber(_) = p.peek() {
        let span = p.peek_span();
        let level = if let Token::LevelNumber(n) = p.peek().clone() {
            n
        } else {
            0
        };
        p.advance();

        let name = parse_item_name(p);

        // Consume all clauses until period
        let mut picture = None;
        while !p.at(&Token::Period)
            && !p.at(&Token::Eof)
            && !matches!(p.peek(), Token::LevelNumber(_))
        {
            if p.at(&Token::Pic) {
                p.advance();
                picture = parse_pic_clause(p);
            } else {
                p.advance();
            }
        }
        p.eat(&Token::Period);

        items.push(ScreenItem {
            level,
            name,
            picture,
            from: None,
            to: None,
            using: None,
            foreground: None,
            background: None,
            highlight: false,
            reverse: false,
            blink: false,
            children: Vec::new(),
            span,
        });
    }
    items
}

// ── Data declarations (flat) ──────────────────────────────────────────────────

/// Parse zero or more data declarations into a flat list.
/// Stops when a non-level-number token (division/section keyword or EOF) is seen.
fn parse_data_declarations(p: &mut Parser) -> Vec<DataDecl> {
    let mut items = Vec::new();
    loop {
        match p.peek().clone() {
            Token::LevelNumber(level) => {
                let span = p.peek_span();
                p.advance();
                let item = parse_data_item(p, level, span);
                items.push(item);
            }
            // Stop at section/division headers, END PROGRAM, or EOF
            Token::WorkingStorage
            | Token::LocalStorage
            | Token::Linkage
            | Token::Screen
            | Token::File
            | Token::Fd
            | Token::Sd
            | Token::Procedure
            | Token::Environment
            | Token::Identification
            | Token::End
            | Token::Eof => break,
            _ => break,
        }
    }
    items
}

// ── Single data item ──────────────────────────────────────────────────────────

/// Parse the name and clauses for a single data item.
/// `level` and `span` have already been consumed/captured by the caller.
fn parse_data_item(p: &mut Parser, level: u8, span: Span) -> DataDecl {
    let name = parse_item_name(p);

    let mut picture: Option<PicClause> = None;
    let mut value: Option<Literal> = None;
    let mut usage = Usage::Display;
    let mut occurs: Option<OccursClause> = None;
    let mut redefines: Option<String> = None;
    let mut renames: Option<cobolt_ast::data::RenamesClause> = None;
    let mut condition_values: Vec<ConditionValue> = Vec::new();
    let mut is_global = false;
    let mut is_external = false;
    let mut blank_when_zero = false;
    let mut justified = false;
    let mut sign: Option<cobolt_ast::data::SignClause> = None;

    // Parse clauses until the period that terminates this item.
    loop {
        match p.peek().clone() {
            // End of item
            Token::Period | Token::Eof => {
                p.eat(&Token::Period);
                break;
            }
            // Next level number starts the next item
            Token::LevelNumber(_) => break,

            // PIC / PICTURE
            Token::Pic => {
                p.advance();
                p.eat(&Token::Is); // optional IS
                picture = parse_pic_clause(p);
            }

            // VALUE / VALUES
            Token::Value | Token::Values => {
                p.advance();
                // `VALUE IS` and `VALUES ARE` are both spellings of the same
                // clause; the plural takes the plural copula.
                p.eat(&Token::Is);
                p.eat(&Token::Are);
                if level == 88 {
                    // 88-level: collect one or more values/ranges
                    condition_values = parse_88_values(p);
                } else {
                    // Fold an optional leading sign (the lexer emits it separately).
                    let neg = if p.eat(&Token::Minus) {
                        true
                    } else {
                        p.eat(&Token::Plus);
                        false
                    };
                    if let Some((lit, _)) = parse_literal(p) {
                        value = Some(if neg { negate_literal(lit) } else { lit });
                        // THRU literal (ignore for now)
                        if p.at(&Token::Through) {
                            p.advance();
                            parse_literal(p);
                        }
                    }
                }
            }

            // USAGE [IS]
            Token::Usage => {
                p.advance();
                p.eat(&Token::Is);
                usage = parse_usage_clause(p);
            }
            // Inline usage keywords (without USAGE keyword)
            Token::Display
            | Token::Binary
            | Token::Comp
            | Token::Comp1
            | Token::Comp2
            | Token::Comp3
            | Token::Comp5
            | Token::PackedDecimal
            | Token::Index
            | Token::Pointer => {
                usage = parse_usage_clause(p);
            }

            // OCCURS
            Token::Occurs => {
                p.advance();
                occurs = Some(parse_occurs_clause(p));
            }

            // REDEFINES
            Token::Redefines => {
                p.advance();
                redefines = Some(p.expect_identifier("REDEFINES target"));
            }

            // RENAMES item-1 [{THRU|THROUGH} item-2]  (66-level)
            Token::Renames => {
                p.advance();
                let from = p.expect_identifier("RENAMES start item");
                let thru = if p.eat(&Token::Thru) || p.eat(&Token::Through) {
                    Some(p.expect_identifier("RENAMES THRU item"))
                } else {
                    None
                };
                renames = Some(cobolt_ast::data::RenamesClause { from, thru });
            }

            // JUSTIFIED [RIGHT] — right-align an alphanumeric receiver.
            Token::Justified => {
                p.advance();
                p.eat(&Token::Right);
                justified = true;
            }

            // SYNCHRONIZED [LEFT | RIGHT] — ignored
            Token::Synchronized => {
                p.advance();
                p.eat(&Token::Left);
                p.eat(&Token::Right);
            }

            // BLANK WHEN ZERO
            Token::Blank => {
                p.advance();
                p.eat(&Token::When);
                p.eat(&Token::Zeros);
                blank_when_zero = true;
            }

            // SIGN IS LEADING | TRAILING [SEPARATE CHARACTER]
            //
            // Only SEPARATE changes storage — it adds one character position
            // holding a literal `+`/`-`. TRAILING is the COBOL default, so the
            // clause is recorded either way and the runtime decides.
            Token::Sign => {
                p.advance();
                p.eat(&Token::Is);
                let leading = p.eat(&Token::Leading);
                if !leading {
                    p.eat(&Token::Trailing);
                }
                let separate = p.eat(&Token::Separate);
                p.eat(&Token::Character);
                sign = Some(cobolt_ast::data::SignClause { leading, separate });
            }

            // Optional `IS` connective before a clause keyword, e.g.
            // `01 X IS GLOBAL` / `IS EXTERNAL` — a COBOL-85 noise word. Consume
            // it silently so it doesn't trip the unknown-clause warning below.
            Token::Is => {
                p.advance();
            }

            // GLOBAL — item visible to all nested programs
            Token::Global => {
                p.advance();
                is_global = true;
            }
            // EXTERNAL — item shared across the run unit
            Token::External => {
                p.advance();
                is_external = true;
            }

            // Division / section tokens or END PROGRAM — break without consuming
            Token::WorkingStorage
            | Token::LocalStorage
            | Token::Linkage
            | Token::Screen
            | Token::File
            | Token::Fd
            | Token::Sd
            | Token::Procedure
            | Token::Environment
            | Token::Identification
            | Token::End => break,

            // Unknown clause token — skip
            _ => {
                p.emit_warning(format!("skipping unknown data clause: {:?}", p.peek()));
                p.advance();
            }
        }
    }

    DataDecl {
        level,
        name,
        picture,
        value,
        usage,
        object_class: p.pending_object_class.take(),
        occurs,
        redefines,
        renames,
        condition_values,
        is_global,
        is_external,
        blank_when_zero,
        children: Vec::new(), // filled in by build_tree
        span,
        justified,
        sign,
    }
}

/// Parse item name: FILLER keyword → None, identifier → Some(name).
fn parse_item_name(p: &mut Parser) -> Option<String> {
    if p.at(&Token::Filler) {
        p.advance();
        return None;
    }
    // Optional: bare period for unnamed filler
    if p.at(&Token::Period) {
        return None;
    }
    if let Some((name, _)) = p.eat_identifier() {
        return Some(name);
    }
    None
}

// ── PIC clause ────────────────────────────────────────────────────────────────

/// Parse a PICTURE template from the token stream.
/// The template is reassembled from individual tokens.
fn parse_pic_clause(p: &mut Parser) -> Option<PicClause> {
    let span = p.peek_span();
    let mut template = String::new();
    // The ENVIRONMENT DIVISION is parsed before the DATA DIVISION, so a
    // `CURRENCY` clause has already been read by the time any picture is.
    let currency = p.currency;

    // Collect tokens until a clause boundary or period
    loop {
        match p.peek().clone() {
            // These keywords start the next clause or end the item
            Token::Value
            | Token::Values
            | Token::Usage
            | Token::Occurs
            | Token::Redefines
            | Token::Justified
            | Token::Synchronized
            | Token::Blank
            | Token::Sign
            | Token::Global
            | Token::External
            | Token::Eof
            | Token::LevelNumber(_)
            | Token::WorkingStorage
            | Token::LocalStorage
            | Token::Linkage
            | Token::Screen
            | Token::Procedure
            | Token::Environment
            | Token::Identification => break,

            // Usage keywords that can appear without USAGE keyword
            Token::Display
            | Token::Binary
            | Token::Comp
            | Token::Comp1
            | Token::Comp2
            | Token::Comp3
            | Token::Comp5
            | Token::PackedDecimal
            | Token::Index
            | Token::Pointer => break,

            // A `.` is the editing decimal point when more picture characters
            // follow it (e.g. `ZZ9.99`); otherwise it terminates the clause.
            Token::Period => {
                // `PIC ZZ,ZZZ.9` — the lexer turns an integer that follows a
                // period into a *level number*, because that is where the next
                // data item normally begins. Glued to the period it is no such
                // thing: it is the picture's fractional digit. The spans say
                // which, exactly as `B-C` vs `B - C` is decided in the lexer.
                // Without this the template silently truncated at the period
                // (`ZZ,ZZZ.9` → `ZZ,ZZZ`), losing both the digit and, with it,
                // the item's numeric category. `.99` never had the problem —
                // 99 is not a level number.
                let glued_digits = match p.peek_at(1) {
                    Token::LevelNumber(n) if p.peek_span_at(1).start == p.peek_span().end => {
                        Some(*n)
                    }
                    _ => None,
                };
                if let Some(n) = glued_digits {
                    p.advance(); // the '.'
                    p.advance(); // the digit(s)
                    template.push('.');
                    template.push_str(&n.to_string());
                    continue;
                }
                // Two periods in a row: the first is the picture's own editing
                // decimal point and the second ends the entry —
                // `02 WRK-EDIT-006 PIC 999999999999..`. Reading both as
                // terminators dropped the point from the template and left a
                // stray period that desynchronised the whole DATA DIVISION.
                if pic_continues(p.peek_at(1), currency) || matches!(p.peek_at(1), Token::Period) {
                    p.advance();
                    template.push('.');
                } else {
                    break;
                }
            }
            // `9.99`, `99.99` are lexed as one decimal literal — rebuild the text.
            Token::DecimalLiteral { mantissa, scale } => {
                p.advance();
                template.push_str(&decimal_to_pic(mantissa, scale));
            }
            // The currency symbol — `$` unless SPECIAL-NAMES said otherwise.
            // It is normalised to `$` in the template whatever the program
            // calls it: `$` is the internal marker for "currency position", so
            // every width and digit-count rule downstream stays written once.
            // Only the formatter substitutes the real character back.
            ref t if is_currency_token(t, currency) => {
                p.advance();
                template.push('$');
            }

            // Collect template characters
            Token::Identifier(s) => {
                let s = s.clone();
                p.advance();
                template.push_str(&s);
            }
            Token::IntegerLiteral(n, digits) => {
                // A run of picture characters that happens to look like a
                // number is one integer token, and its **value** loses the
                // leading zeros: `PIC 090909` (three digit positions between
                // zero insertions) came back as `90909` and edited one
                // character too narrow. The token records how many digits were
                // written, so they go back from that.
                p.advance();
                let text = n.to_string();
                let written = digits as usize;
                // …but a **repeat count** is a number, so `PIC 9(06)` means six
                // digits, not `9(06)`. Only digits outside the parentheses are
                // picture characters.
                let is_repeat_count = template.ends_with('(');
                if !is_repeat_count && written > text.len() {
                    template.push_str(&"0".repeat(written - text.len()));
                }
                template.push_str(&text);
            }
            Token::LParen => {
                p.advance();
                template.push('(');
            }
            Token::RParen => {
                p.advance();
                template.push(')');
            }
            Token::Plus => {
                p.advance();
                template.push('+');
            }
            Token::Minus => {
                p.advance();
                template.push('-');
            }
            Token::Slash => {
                p.advance();
                template.push('/');
            }
            Token::Star => {
                p.advance();
                template.push('*');
            }
            // `**` is lexed as the exponentiation token; in a PIC it is two stars.
            Token::Power => {
                p.advance();
                template.push_str("**");
            }
            Token::Comma => {
                p.advance();
                template.push(',');
            }
            _ => break,
        }
    }

    if template.is_empty() {
        p.emit_error("expected PICTURE template");
        return None;
    }

    let (kind, digits, decimals) = analyze_pic(&template);
    Some(PicClause {
        template,
        kind,
        digits,
        decimals,
        span,
    })
}

/// True if `tok` can be part of a PICTURE string (used to tell an editing decimal
/// point apart from the clause-terminating period).
fn pic_continues(tok: &Token, currency: char) -> bool {
    matches!(
        tok,
        Token::IntegerLiteral(..)
            | Token::DecimalLiteral { .. }
            | Token::Identifier(_)
            | Token::LParen
            | Token::Star
            | Token::Power
            | Token::Plus
            | Token::Minus
            | Token::Slash
            | Token::Comma
    ) || is_currency_token(tok, currency)
}

/// True if `tok` is this program's currency symbol.
///
/// `SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal` lets a program pick a
/// different character for the currency position in an edited PICTURE, and the
/// lexer has no idea which one that is — it tokenizes `<` as a less-than
/// operator, and `$` as an unclassifiable character, long before the picture is
/// read. The mapping back is therefore explicit.
///
/// COBOL-85 forbids a currency symbol that would collide with a picture
/// character or a separator: not a digit, not one of `A B C D E G N P R S V X
/// Z`, and none of `space * + - , . ; ( ) " / =`. What remains reaches the
/// parser as an operator token (`<`, `>`, `&`), as an unclassifiable character
/// (`$`, `#`, `@`, `%`), or — for the permitted letters — as a one-character
/// identifier.
fn is_currency_token(tok: &Token, currency: char) -> bool {
    let one = |s: &str| s.chars().eq(std::iter::once(currency));
    match tok {
        Token::Lt => currency == '<',
        Token::Gt => currency == '>',
        Token::Ampersand => currency == '&',
        Token::Error(s) => one(s),
        // Guarded on a declared symbol: with the default `$` in force, a
        // one-character identifier in a picture is `PIC A` or `PIC S`, never a
        // currency position.
        Token::Identifier(s) => currency != '$' && one(s),
        _ => false,
    }
}

/// Rebuild the picture text for a decimal literal token (`9.99`, `99.99`).
fn decimal_to_pic(mantissa: i128, scale: u8) -> String {
    if scale == 0 {
        return mantissa.to_string();
    }
    let p = 10_i128.pow(scale as u32);
    format!(
        "{}.{:0width$}",
        mantissa / p,
        (mantissa % p).abs(),
        width = scale as usize
    )
}

/// Classify a raw PIC template string.
///
/// Returns `(kind, digits, decimals)` where, for numeric pictures, `digits` is
/// the count of integer digit positions and `decimals` the fractional ones; for
/// alphabetic/alphanumeric pictures `digits` is the total character width.
/// Parenthesised repetition counts (`X(20)`, `9(5)V99`) are expanded.
fn analyze_pic(template: &str) -> (PicKind, u16, u16) {
    let t = template.to_uppercase();

    // Expand the template into one entry per character position, e.g.
    // "9(3)V99" → ['9','9','9','V','9','9'] and "X(20)" → twenty 'X'es.
    let expanded = expand_pic_template(&t);

    // Editing characters imply edited categories. A `.` (actual decimal point),
    // sign, slash, zero/blank insertion, CR/DB and currency all qualify.
    let has_editing = expanded
        .iter()
        .any(|&c| matches!(c, 'Z' | 'B' | '*' | '+' | '-' | '/' | ',' | '.' | '0' | '$'))
        || t.contains("CR")
        || t.contains("DB");
    let count = |pred: &dyn Fn(char) -> bool| -> u16 {
        expanded
            .iter()
            .filter(|&&c| pred(c))
            .count()
            .min(u16::MAX as usize) as u16
    };

    if expanded.iter().any(|&c| c == 'X') {
        let kind = if has_editing {
            PicKind::AlphanumericEdited
        } else {
            PicKind::Alphanumeric
        };
        // Width = every character position in the picture.
        return (kind, expanded.len().min(u16::MAX as usize) as u16, 0);
    }
    if expanded.iter().any(|&c| c == 'A') && !expanded.iter().any(|&c| c == '9') {
        // `PIC ABA` / `PIC A/AA` are alphanumeric-**edited**: the `B` and `/`
        // are insertion characters that occupy positions of their own. Counting
        // only the `A`s made the item two or three characters wide instead of
        // three or four, so its own VALUE no longer fitted it (NC114M).
        if has_editing {
            return (
                PicKind::AlphanumericEdited,
                expanded.len().min(u16::MAX as usize) as u16,
                0,
            );
        }
        return (PicKind::Alphabetic, count(&|c| c == 'A'), 0);
    }
    if expanded.iter().any(|&c| c == '9' || c == 'S') {
        let kind = if has_editing {
            PicKind::NumericEdited
        } else {
            PicKind::Numeric
        };
        let v_pos = expanded.iter().position(|&c| c == 'V');
        let (int_part, frac_part): (&[char], &[char]) = match v_pos {
            Some(p) => (&expanded[..p], &expanded[p + 1..]),
            None => (&expanded[..], &[]),
        };
        let digits = int_part
            .iter()
            .filter(|&&c| c == '9')
            .count()
            .min(u16::MAX as usize) as u16;
        let decimals = frac_part
            .iter()
            .filter(|&&c| c == '9')
            .count()
            .min(u16::MAX as usize) as u16;
        return (kind, digits, decimals);
    }

    // A picture built only from numeric-editing characters — carrying no `9` and
    // no `S` at all, e.g. `ZZZZ`, `$.**`, `$**.**CR`, `----` — is still numeric
    // edited: `Z`, `*` and a floating `$`/`+`/`-` each stand for a digit
    // position. Reaching the alphanumeric fallback made such an item a
    // non-numeric one, so a perfectly legal `DIVIDE … GIVING` receiver was
    // rejected. The digit-position rule mirrors `numedit::counts`, which is what
    // actually formats the value at run time.
    if expanded
        .iter()
        .any(|&c| matches!(c, 'Z' | '*' | '$' | '+' | '-'))
    {
        let point = expanded.iter().position(|&c| c == 'V' || c == '.');
        let (int_part, frac_part): (&[char], &[char]) = match point {
            Some(p) => (&expanded[..p], &expanded[p + 1..]),
            None => (&expanded[..], &[]),
        };
        // A repeated `$`/`+`/`-` is a *floating* insertion: every occurrence but
        // the leading one is a digit position.
        let floats = |sym: char| expanded.iter().filter(|&&c| c == sym).count() > 1;
        let (float_dollar, float_plus, float_minus) = (floats('$'), floats('+'), floats('-'));
        let mut int_digits = 0usize;
        for &c in int_part {
            match c {
                'Z' | '*' => int_digits += 1,
                '$' if float_dollar => int_digits += 1,
                '+' if float_plus => int_digits += 1,
                '-' if float_minus => int_digits += 1,
                _ => {}
            }
        }
        // A floating run reserves one leading position for the symbol itself.
        let anchor = float_dollar as usize + float_plus as usize + float_minus as usize;
        let int_digits = int_digits.saturating_sub(anchor);
        let frac_digits = frac_part
            .iter()
            .filter(|&&c| matches!(c, 'Z' | '*'))
            .count();
        return (
            PicKind::NumericEdited,
            int_digits.min(u16::MAX as usize) as u16,
            frac_digits.min(u16::MAX as usize) as u16,
        );
    }

    // Fallback — treat as a single alphanumeric position.
    (
        PicKind::Alphanumeric,
        expanded.len().max(1).min(u16::MAX as usize) as u16,
        0,
    )
}

/// Expand a PICTURE template, turning each `C(n)` group into `n` copies of `C`.
/// Unparenthesised symbols contribute a single position each. The decimal point
/// marker `V` is preserved so the caller can split integer/fraction digits.
fn expand_pic_template(t: &str) -> Vec<char> {
    let mut out: Vec<char> = Vec::new();
    let chars: Vec<char> = t.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '(' {
            // Parse the repetition count and apply it to the previous symbol.
            let mut j = i + 1;
            let mut num = String::new();
            while j < chars.len() && chars[j].is_ascii_digit() {
                num.push(chars[j]);
                j += 1;
            }
            // Skip the closing ')'.
            if j < chars.len() && chars[j] == ')' {
                j += 1;
            }
            if let (Some(&sym), Ok(n)) = (out.last().copied().as_ref(), num.parse::<usize>()) {
                // We already pushed one copy of the symbol; add the remaining n-1.
                if n >= 1 {
                    out.extend(std::iter::repeat(sym).take(n - 1));
                }
            }
            i = j;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

// ── USAGE clause ──────────────────────────────────────────────────────────────

fn parse_usage_clause(p: &mut Parser) -> Usage {
    match p.peek().clone() {
        Token::Display => {
            p.advance();
            Usage::Display
        }
        Token::Binary => {
            p.advance();
            Usage::Binary
        }
        Token::Comp => {
            p.advance();
            Usage::Comp
        }
        Token::Comp1 => {
            p.advance();
            Usage::Comp1
        }
        Token::Comp2 => {
            p.advance();
            Usage::Comp2
        }
        Token::Comp3 => {
            p.advance();
            Usage::Comp3
        }
        Token::Comp5 => {
            p.advance();
            Usage::Comp5
        }
        Token::PackedDecimal => {
            p.advance();
            Usage::PackedDecimal
        }
        Token::Index => {
            p.advance();
            Usage::Index
        }
        Token::Pointer => {
            p.advance();
            Usage::Pointer
        }
        // OBJECT REFERENCE <class-name>  (COBOL-2002; spec 005 Rust-FFI bridge).
        // The class name is captured onto the data item being built so the
        // interpreter can resolve it to a Rust type via REPOSITORY.
        Token::Identifier(ref s) if s.eq_ignore_ascii_case("OBJECT") => {
            p.advance(); // OBJECT
            p.eat(&Token::Reference); // REFERENCE (optional word)
            if let Token::Identifier(c) = p.peek() {
                let c = c.clone();
                p.advance(); // <class-name>
                p.pending_object_class = Some(c.to_ascii_uppercase());
            }
            Usage::ObjectReference
        }
        _ => {
            p.emit_error(format!("unknown USAGE clause: {:?}", p.peek()));
            Usage::Display
        }
    }
}

// ── OCCURS clause ─────────────────────────────────────────────────────────────

/// Read an integer where the grammar requires one, accepting the `LevelNumber`
/// spelling of the same digits.
///
/// The lexer decides between `IntegerLiteral` and `LevelNumber` from POSITION —
/// a number that opens a line is taken for a level number — and it cannot know
/// better, because a level number is only recognisable from context. So a
/// clause whose count spills onto the next line arrives mis-typed:
///
/// ```cobol
///     10  STUFF-1 OCCURS
///             31 TIMES.
/// ```
///
/// `31` opens its line, becomes `LevelNumber(31)`, and `expected integer after
/// OCCURS` follows. Where an integer is *syntactically required*, the two
/// spellings are the same number, so accepting both is exact rather than
/// lenient — a real level number can never appear in this position.
fn eat_required_integer(p: &mut Parser) -> Option<u32> {
    match p.peek().clone() {
        Token::IntegerLiteral(n, _) => {
            p.advance();
            Some(n as u32)
        }
        Token::LevelNumber(n) => {
            p.advance();
            Some(n as u32)
        }
        _ => None,
    }
}

fn parse_occurs_clause(p: &mut Parser) -> OccursClause {
    let span = p.peek_span();

    // OCCURS min TO max | OCCURS n
    let first = match eat_required_integer(p) {
        Some(n) => n,
        None => {
            p.emit_error("expected integer after OCCURS");
            0
        }
    };

    let (min, max) = if p.at(&Token::To) {
        p.advance();
        let m = match eat_required_integer(p) {
            Some(n) => n,
            None => {
                p.emit_error("expected integer after TO in OCCURS");
                first
            }
        };
        (first, m)
    } else {
        (0, first)
    };

    p.eat(&Token::Times); // optional TIMES keyword

    // DEPENDING ON data-item
    let depending_on = if p.at(&Token::Depending) {
        p.advance();
        p.eat(&Token::On);
        Some(p.expect_identifier("DEPENDING ON target"))
    } else {
        None
    };

    // `ASCENDING/DESCENDING KEY IS field…` and `INDEXED BY index-name…` may
    // appear in either order (COBOL-85 allows the KEY phrase before or after
    // INDEXED BY); loop over both until neither is present.
    let mut indexed_by = Vec::new();
    let mut keys: Vec<(String, bool)> = Vec::new();
    loop {
        if p.at(&Token::Indexed) {
            p.advance();
            p.eat(&Token::By);
            while p.at_identifier() {
                let (name, _) = p.eat_identifier().unwrap();
                indexed_by.push(name);
                p.eat(&Token::Comma);
            }
        } else if p.at(&Token::Ascending) || p.at(&Token::Descending) {
            let ascending = p.at(&Token::Ascending);
            p.advance();
            p.eat(&Token::Key);
            p.eat(&Token::Is);
            while p.at_identifier() {
                let (name, _) = p.eat_identifier().unwrap();
                keys.push((name, ascending));
                p.eat(&Token::Comma);
            }
        } else {
            break;
        }
    }

    OccursClause {
        min,
        max,
        depending_on,
        indexed_by,
        keys,
        span,
    }
}

// ── 88-level condition values ─────────────────────────────────────────────────

/// Negate a numeric literal (for a signed `VALUE`).
fn negate_literal(lit: Literal) -> Literal {
    match lit {
        Literal::Integer(n) => Literal::Integer(-n),
        // The written digit count is the count of *digits*; the sign is not one
        // of them, so it survives negation unchanged.
        Literal::IntegerDigits(n, d) => Literal::IntegerDigits(-n, d),
        Literal::Decimal(m, s) => Literal::Decimal(-m, s),
        Literal::Float(f) => Literal::Float(-f),
        other => other,
    }
}

/// Read one value of an 88-level list, folding an optional leading sign.
///
/// The lexer emits `+`/`-` as its own token so `COMPUTE X = Y - 3` stays
/// unambiguous, which leaves every signed literal to be reassembled by the
/// parser — `88 F VALUE -9 THRU -2` is three signed literals, not a
/// subtraction.
fn parse_signed_literal(p: &mut Parser) -> Option<(Literal, Span)> {
    let neg = if p.eat(&Token::Minus) {
        true
    } else {
        p.eat(&Token::Plus);
        false
    };
    let (lit, span) = parse_literal(p)?;
    Some((if neg { negate_literal(lit) } else { lit }, span))
}

fn parse_88_values(p: &mut Parser) -> Vec<ConditionValue> {
    let mut values = Vec::new();
    loop {
        // Also must not be at a clause boundary. A period *glued* to its digits
        // is not one: it opens a numeric literal (`88 E VALUE .01, .11`), which
        // COBOL-85 allows because only a *trailing* decimal point is forbidden.
        if (p.at(&Token::Period) && !crate::expr::at_leading_decimal_point(p)) || p.at(&Token::Eof) {
            break;
        }
        // A value that opens a line is offered by the lexer as a level number,
        // because that is where the next entry normally begins:
        //
        //     88 COND-2  VALUES ARE 06 THRU 10
        //                           16 THRU 20  00.
        //
        // `16` is a value, not a new entry. What tells them apart is the
        // data-name a real entry must carry next; a value is followed by
        // `THRU`, another value, a comma, or the closing period.
        if p.at_level_number() && matches!(p.peek_at(1), Token::Identifier(_)) {
            break;
        }
        if let Some((lit, _)) = parse_signed_literal(p) {
            // `THRU` and `THROUGH` are the same word; the lexer keeps them as
            // distinct tokens, so both spellings have to be accepted here.
            if p.at(&Token::Through) || p.at(&Token::Thru) {
                p.advance();
                if let Some((lit2, _)) = parse_signed_literal(p) {
                    values.push(ConditionValue::Range(lit, lit2));
                } else {
                    values.push(ConditionValue::Single(lit));
                }
            } else {
                values.push(ConditionValue::Single(lit));
            }
            p.eat(&Token::Comma);
        } else {
            break;
        }
    }
    values
}

// ── Level-number tree builder ─────────────────────────────────────────────────

/// Convert a flat list of `DataDecl`s into a proper parent–child tree.
///
/// The algorithm is O(n²) in the worst case but n is small in practice.
/// Items with level 77 or 66 are always roots.
fn build_tree(items: Vec<DataDecl>) -> Vec<DataDecl> {
    if items.is_empty() {
        return items;
    }

    let n = items.len();

    // parent_idx[i] = Some(j) means items[j] is the direct parent of items[i].
    let mut parent_idx: Vec<Option<usize>> = vec![None; n];

    for i in 1..n {
        let level = items[i].level;
        // Special levels are always root-level
        if level == 77 || level == 66 {
            continue;
        }
        // Find the last preceding item with a strictly lower level
        for j in (0..i).rev() {
            if items[j].level < level {
                parent_idx[i] = Some(j);
                break;
            }
        }
    }

    // Build a children list indexed by parent
    let mut children_of: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut roots: Vec<usize> = Vec::new();

    for i in 0..n {
        match parent_idx[i] {
            Some(p) => children_of[p].push(i),
            None => roots.push(i),
        }
    }

    // Recursively build nodes
    let mut items_opt: Vec<Option<DataDecl>> = items.into_iter().map(Some).collect();

    fn build_node(
        idx: usize,
        children_of: &[Vec<usize>],
        items: &mut Vec<Option<DataDecl>>,
        inherited_sign: Option<cobolt_ast::data::SignClause>,
    ) -> DataDecl {
        let mut node = items[idx].take().unwrap();
        // A SIGN clause written on a group applies to every subordinate signed
        // numeric DISPLAY item that does not carry one of its own, and a nested
        // group overrides it for its own subtree (NC116A TEST-17/TEST-18).
        node.sign = node.sign.or(inherited_sign);
        let pass_down = node.sign;
        for &ci in &children_of[idx] {
            let child = build_node(ci, children_of, items, pass_down);
            node.children.push(child);
        }
        node
    }

    roots
        .into_iter()
        .map(|i| build_node(i, &children_of, &mut items_opt, None))
        .collect()
}

#[cfg(test)]
mod pic_tests {
    use super::{analyze_pic, expand_pic_template, parse_pic_clause};
    use crate::parser::Parser;
    use cobolt_ast::data::PicKind;
    use cobolt_lexer::{tokenize, SourceFormat, Token};

    #[test]
    fn expands_parenthesised_repetitions() {
        assert_eq!(expand_pic_template("X(3)"), vec!['X', 'X', 'X']);
        assert_eq!(expand_pic_template("XXX"), vec!['X', 'X', 'X']);
        assert_eq!(
            expand_pic_template("9(3)V99"),
            vec!['9', '9', '9', 'V', '9', '9']
        );
        assert_eq!(expand_pic_template("X(256)").len(), 256);
    }

    #[test]
    fn alphanumeric_width_uses_repetition_count() {
        assert_eq!(analyze_pic("X(20)"), (PicKind::Alphanumeric, 20, 0));
        assert_eq!(analyze_pic("X(256)"), (PicKind::Alphanumeric, 256, 0));
        // Wide fields like PowerDEMO's PIC X(32767) must be exact (needs u16).
        assert_eq!(analyze_pic("X(32767)"), (PicKind::Alphanumeric, 32767, 0));
        assert_eq!(analyze_pic("XXX"), (PicKind::Alphanumeric, 3, 0));
    }

    #[test]
    fn numeric_digits_and_decimals_use_repetition_count() {
        assert_eq!(analyze_pic("9(5)"), (PicKind::Numeric, 5, 0));
        assert_eq!(analyze_pic("9(7)V99"), (PicKind::Numeric, 7, 2));
        assert_eq!(analyze_pic("S9(4)"), (PicKind::Numeric, 4, 0));
        assert_eq!(analyze_pic("999"), (PicKind::Numeric, 3, 0));
    }

    /// An all-editing picture carries no `9`, but `Z`, `*` and a floating
    /// `$`/`+`/`-` are digit positions all the same — NIST NC175A/NC203A use
    /// exactly these as `DIVIDE`/`SUBTRACT … GIVING` receivers.
    #[test]
    fn editing_only_pictures_are_numeric_edited() {
        assert_eq!(analyze_pic("ZZZZ"), (PicKind::NumericEdited, 4, 0));
        assert_eq!(analyze_pic("$.**"), (PicKind::NumericEdited, 0, 2));
        assert_eq!(analyze_pic("$**.**CR"), (PicKind::NumericEdited, 2, 2));
        // A floating run spends one position on the symbol itself.
        assert_eq!(analyze_pic("----"), (PicKind::NumericEdited, 3, 0));
        assert_eq!(analyze_pic("$$$$.**"), (PicKind::NumericEdited, 3, 2));
        // A picture that *does* carry a `9` keeps the long-standing counting.
        assert_eq!(analyze_pic("$$$$.99"), (PicKind::NumericEdited, 2, 0));
        // Editing characters that are *not* digit positions stay alphanumeric.
        assert_eq!(analyze_pic("BBB"), (PicKind::Alphanumeric, 3, 0));
    }

    /// `PIC ZZ,ZZZ.9` — the lexer calls the `9` after a period a level number,
    /// since that is where a new data item usually starts. Glued to the period
    /// it is the picture's fractional digit, and the template must keep it.
    #[test]
    fn editing_decimal_point_keeps_a_single_trailing_digit() {
        // The template is fed in exactly as it follows `PIC`, with a real data
        // item behind it: the clause must take the glued digit and still stop
        // before the next level number.
        let pic = |src: &str| {
            let text = format!(" {src}.\n01  B PIC XX.\n");
            let toks = tokenize(&text, SourceFormat::Free);
            let mut p = Parser::new(toks);
            let clause = parse_pic_clause(&mut p).expect("a PICTURE clause");
            // The clause stops *at* the terminating period, leaving it for the
            // caller — and the next data item must still be intact behind it.
            assert!(
                matches!(p.peek(), Token::Period)
                    && matches!(p.peek_at(1), Token::LevelNumber(1)),
                "the next data item must survive, got {:?} then {:?}",
                p.peek(),
                p.peek_at(1)
            );
            clause
        };

        let a = pic("ZZ,ZZZ.9");
        assert_eq!(a.template, "ZZ,ZZZ.9");
        assert_eq!(a.kind, PicKind::NumericEdited);
        assert_eq!((a.digits, a.decimals), (1, 0));

        // Two digits never truncated: 99 is not a level number.
        assert_eq!(pic("ZZ,ZZZ.99").template, "ZZ,ZZZ.99");
        // …and the all-editing form keeps its digit too.
        assert_eq!(pic("****.9").template, "****.9");
        assert_eq!(pic("****.9").kind, PicKind::NumericEdited);
    }
}
