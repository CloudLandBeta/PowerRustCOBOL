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
    ConditionValue, DataDecl, FileDescription, OccursClause, PicClause, PicKind, ScreenItem,
    Usage,
};
use cobolt_ast::expr::Literal;
use cobolt_ast::program::{DataDivision, DataSection};
use cobolt_lexer::{Span, Token};

use crate::expr::parse_literal;
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
            // Unknown — skip with a warning.
            _ => {
                p.emit_warning(format!(
                    "unexpected token in DATA DIVISION: {:?}",
                    p.peek()
                ));
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
        // Consume optional clauses until period
        while !p.at(&Token::Period) && !p.at(&Token::Eof) {
            p.advance();
        }
        p.expect_period();
        // Parse record descriptions
        let records = parse_data_declarations(p);
        fds.push(FileDescription {
            name,
            records: build_tree(records),
            span,
        });
    }
    fds
}

// ── Screen section (simplified) ───────────────────────────────────────────────

fn parse_screen_section(p: &mut Parser) -> Vec<ScreenItem> {
    // For MVP: collect screen items flat without tree building.
    let mut items = Vec::new();
    while let Token::LevelNumber(_) = p.peek() {
        let span = p.peek_span();
        let level = if let Token::LevelNumber(n) = p.peek().clone() { n } else { 0 };
        p.advance();

        let name = parse_item_name(p);

        // Consume all clauses until period
        let mut picture = None;
        while !p.at(&Token::Period) && !p.at(&Token::Eof)
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
    let mut is_global   = false;
    let mut is_external = false;
    let mut blank_when_zero = false;

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
                p.eat(&Token::Is); // optional IS
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

            // JUSTIFIED [RIGHT] — ignored for MVP
            Token::Justified => {
                p.advance();
                p.eat(&Token::Right);
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

            // SIGN IS LEADING/TRAILING [SEPARATE] — ignored
            Token::Sign => {
                p.advance();
                p.eat(&Token::Is);
                p.eat(&Token::Leading);
                p.eat(&Token::Trailing);
                p.eat(&Token::Separate);
                p.eat(&Token::Character);
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
        occurs,
        redefines,
        renames,
        condition_values,
        is_global,
        is_external,
        blank_when_zero,
        children: Vec::new(), // filled in by build_tree
        span,
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

    // Collect tokens until a clause boundary or period
    loop {
        match p.peek().clone() {
            // These keywords start the next clause or end the item
            Token::Value | Token::Values
            | Token::Usage | Token::Occurs
            | Token::Redefines | Token::Justified
            | Token::Synchronized | Token::Blank
            | Token::Sign | Token::Global | Token::External
            | Token::Eof | Token::LevelNumber(_)
            | Token::WorkingStorage | Token::LocalStorage
            | Token::Linkage | Token::Screen
            | Token::Procedure | Token::Environment | Token::Identification => break,

            // Usage keywords that can appear without USAGE keyword
            Token::Display | Token::Binary | Token::Comp
            | Token::Comp1 | Token::Comp2 | Token::Comp3 | Token::Comp5
            | Token::PackedDecimal | Token::Index | Token::Pointer => break,

            // A `.` is the editing decimal point when more picture characters
            // follow it (e.g. `ZZ9.99`); otherwise it terminates the clause.
            Token::Period => {
                if pic_continues(p.peek_at(1)) {
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
            // '$' currency is lexed as an error token.
            Token::Error(ref s) if s == "$" => { p.advance(); template.push('$'); }

            // Collect template characters
            Token::Identifier(s) => {
                let s = s.clone(); p.advance(); template.push_str(&s);
            }
            Token::IntegerLiteral(n) => {
                let n = n; p.advance(); template.push_str(&n.to_string());
            }
            Token::LParen  => { p.advance(); template.push('('); }
            Token::RParen  => { p.advance(); template.push(')'); }
            Token::Plus    => { p.advance(); template.push('+'); }
            Token::Minus   => { p.advance(); template.push('-'); }
            Token::Slash   => { p.advance(); template.push('/'); }
            Token::Star    => { p.advance(); template.push('*'); }
            // `**` is lexed as the exponentiation token; in a PIC it is two stars.
            Token::Power   => { p.advance(); template.push_str("**"); }
            Token::Comma   => { p.advance(); template.push(','); }
            _ => break,
        }
    }

    if template.is_empty() {
        p.emit_error("expected PICTURE template");
        return None;
    }

    let (kind, digits, decimals) = analyze_pic(&template);
    Some(PicClause { template, kind, digits, decimals, span })
}

/// True if `tok` can be part of a PICTURE string (used to tell an editing decimal
/// point apart from the clause-terminating period).
fn pic_continues(tok: &Token) -> bool {
    matches!(
        tok,
        Token::IntegerLiteral(_)
            | Token::DecimalLiteral { .. }
            | Token::Identifier(_)
            | Token::LParen
            | Token::Star
            | Token::Power
            | Token::Plus
            | Token::Minus
            | Token::Slash
            | Token::Comma
    ) || matches!(tok, Token::Error(s) if s == "$")
}

/// Rebuild the picture text for a decimal literal token (`9.99`, `99.99`).
fn decimal_to_pic(mantissa: i128, scale: u8) -> String {
    if scale == 0 {
        return mantissa.to_string();
    }
    let p = 10_i128.pow(scale as u32);
    format!("{}.{:0width$}", mantissa / p, (mantissa % p).abs(), width = scale as usize)
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
        expanded.iter().filter(|&&c| pred(c)).count().min(u16::MAX as usize) as u16
    };

    if expanded.iter().any(|&c| c == 'X') {
        let kind = if has_editing { PicKind::AlphanumericEdited } else { PicKind::Alphanumeric };
        // Width = every character position in the picture.
        return (kind, expanded.len().min(u16::MAX as usize) as u16, 0);
    }
    if expanded.iter().any(|&c| c == 'A') && !expanded.iter().any(|&c| c == '9') {
        return (PicKind::Alphabetic, count(&|c| c == 'A'), 0);
    }
    if expanded.iter().any(|&c| c == '9' || c == 'S') {
        let kind = if has_editing { PicKind::NumericEdited } else { PicKind::Numeric };
        let v_pos = expanded.iter().position(|&c| c == 'V');
        let (int_part, frac_part): (&[char], &[char]) = match v_pos {
            Some(p) => (&expanded[..p], &expanded[p + 1..]),
            None => (&expanded[..], &[]),
        };
        let digits   = int_part.iter().filter(|&&c| c == '9').count().min(u16::MAX as usize) as u16;
        let decimals = frac_part.iter().filter(|&&c| c == '9').count().min(u16::MAX as usize) as u16;
        return (kind, digits, decimals);
    }

    // Fallback — treat as a single alphanumeric position.
    (PicKind::Alphanumeric, expanded.len().max(1).min(u16::MAX as usize) as u16, 0)
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
        Token::Display     => { p.advance(); Usage::Display }
        Token::Binary      => { p.advance(); Usage::Binary }
        Token::Comp        => { p.advance(); Usage::Comp }
        Token::Comp1       => { p.advance(); Usage::Comp1 }
        Token::Comp2       => { p.advance(); Usage::Comp2 }
        Token::Comp3       => { p.advance(); Usage::Comp3 }
        Token::Comp5       => { p.advance(); Usage::Comp5 }
        Token::PackedDecimal => { p.advance(); Usage::PackedDecimal }
        Token::Index       => { p.advance(); Usage::Index }
        Token::Pointer     => { p.advance(); Usage::Pointer }
        _ => {
            p.emit_error(format!("unknown USAGE clause: {:?}", p.peek()));
            Usage::Display
        }
    }
}

// ── OCCURS clause ─────────────────────────────────────────────────────────────

fn parse_occurs_clause(p: &mut Parser) -> OccursClause {
    let span = p.peek_span();

    // OCCURS min TO max | OCCURS n
    let first = match p.peek().clone() {
        Token::IntegerLiteral(n) => { p.advance(); n as u32 }
        _ => {
            p.emit_error("expected integer after OCCURS");
            0
        }
    };

    let (min, max) = if p.at(&Token::To) {
        p.advance();
        let m = match p.peek().clone() {
            Token::IntegerLiteral(n) => { p.advance(); n as u32 }
            _ => {
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

    // INDEXED BY index-name…
    let mut indexed_by = Vec::new();
    if p.at(&Token::Indexed) {
        p.advance();
        p.eat(&Token::By);
        while p.at_identifier() {
            let (name, _) = p.eat_identifier().unwrap();
            indexed_by.push(name);
            p.eat(&Token::Comma);
        }
    }

    // ASCENDING/DESCENDING KEY IS field… (skip for MVP)
    while p.at(&Token::Ascending) || p.at(&Token::Descending) {
        p.advance();
        p.eat(&Token::Key);
        p.eat(&Token::Is);
        while p.at_identifier() { p.advance(); p.eat(&Token::Comma); }
    }

    OccursClause { min, max, depending_on, indexed_by, span }
}

// ── 88-level condition values ─────────────────────────────────────────────────

/// Negate a numeric literal (for a signed `VALUE`).
fn negate_literal(lit: Literal) -> Literal {
    match lit {
        Literal::Integer(n)    => Literal::Integer(-n),
        Literal::Decimal(m, s) => Literal::Decimal(-m, s),
        Literal::Float(f)      => Literal::Float(-f),
        other                  => other,
    }
}

fn parse_88_values(p: &mut Parser) -> Vec<ConditionValue> {
    let mut values = Vec::new();
    loop {
        // Also must not be at a clause boundary
        if p.at(&Token::Period) || p.at(&Token::Eof) || p.at_level_number() {
            break;
        }
        if let Some((lit, _)) = parse_literal(p) {
            if p.at(&Token::Through) {
                p.advance();
                if let Some((lit2, _)) = parse_literal(p) {
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
            None    => roots.push(i),
        }
    }

    // Recursively build nodes
    let mut items_opt: Vec<Option<DataDecl>> = items.into_iter().map(Some).collect();

    fn build_node(
        idx: usize,
        children_of: &[Vec<usize>],
        items: &mut Vec<Option<DataDecl>>,
    ) -> DataDecl {
        let mut node = items[idx].take().unwrap();
        for &ci in &children_of[idx] {
            let child = build_node(ci, children_of, items);
            node.children.push(child);
        }
        node
    }

    roots
        .into_iter()
        .map(|i| build_node(i, &children_of, &mut items_opt))
        .collect()
}

#[cfg(test)]
mod pic_tests {
    use super::{analyze_pic, expand_pic_template};
    use cobolt_ast::data::PicKind;

    #[test]
    fn expands_parenthesised_repetitions() {
        assert_eq!(expand_pic_template("X(3)"), vec!['X', 'X', 'X']);
        assert_eq!(expand_pic_template("XXX"), vec!['X', 'X', 'X']);
        assert_eq!(expand_pic_template("9(3)V99"),
                   vec!['9', '9', '9', 'V', '9', '9']);
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
}
