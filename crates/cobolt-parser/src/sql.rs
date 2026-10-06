// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Embedded SQL statements (spec 087 R2–R9, R14, R16, R21, R23–R36, R41–R45).
//!
//! The lexer hands the parser each `EXEC SQL … END-EXEC` block whole
//! ([`Token::ExecSqlBlock`]). Here the block's text is classified into a
//! statement kind, its host variables become the statement's inputs or
//! targets, and the SQL itself is kept as written — line comments removed
//! (R4) — for the database.
//!
//! Every offset below is into the block's ORIGINAL text, so a diagnostic names
//! the line the developer wrote ([`SqlBlock::line_of`]).

use cobolt_ast::program::{Program, ProcedureBody};
use cobolt_ast::sql::*;
use cobolt_ast::stmt::Stmt;
use cobolt_lexer::sql::{self as scan, HostRef, SqlBlock};
use cobolt_lexer::{Span, Token};

use crate::error::Diagnostic;
use crate::parser::Parser;

/// COBOL's figurative constants: never meaningful inside SQL (R14).
const FIGURATIVES: &[&str] = &[
    "SPACE", "SPACES", "ZERO", "ZEROS", "ZEROES", "HIGH-VALUE", "HIGH-VALUES", "LOW-VALUE",
    "LOW-VALUES", "QUOTE", "QUOTES",
];

/// One word-level piece of a block, for classifying it.
#[derive(Debug, Clone, PartialEq)]
enum W {
    /// A word (keyword or name), upper-cased.
    Word(String),
    /// A `'…'` string's contents.
    Str(String),
    /// A host variable: the index into the block's host references.
    Host(usize),
    /// Any other single character of code (`(`, `,`, `=` …).
    Punct(char),
}

#[derive(Debug, Clone)]
struct Tok {
    w: W,
    start: usize,
    end: usize,
    depth: i32,
}

/// The block read as words, with its host variables.
struct Block<'a> {
    b: &'a SqlBlock,
    hosts: Vec<HostRef>,
    toks: Vec<Tok>,
}

impl<'a> Block<'a> {
    fn new(b: &'a SqlBlock) -> Self {
        let text = &b.text;
        let hosts = scan::host_variables(text);
        let bytes = text.as_bytes();
        let mut toks = Vec::new();
        let mut depth = 0i32;
        for p in scan::scan(text) {
            match p.kind {
                scan::SqlKind::String => {
                    let inner = &text[p.start + 1..p.end.saturating_sub(1).max(p.start + 1)];
                    toks.push(Tok { w: W::Str(inner.replace("''", "'")), start: p.start, end: p.end, depth });
                }
                scan::SqlKind::QuotedIdent => {
                    let inner = &text[p.start + 1..p.end.saturating_sub(1).max(p.start + 1)];
                    toks.push(Tok { w: W::Word(inner.to_string()), start: p.start, end: p.end, depth });
                }
                scan::SqlKind::Code => {
                    let mut i = p.start;
                    while i < p.end {
                        if let Some((k, h)) = hosts.iter().enumerate().find(|(_, h)| h.start == i) {
                            toks.push(Tok { w: W::Host(k), start: h.start, end: h.end, depth });
                            i = h.end.max(i + 1);
                            continue;
                        }
                        let c = bytes[i];
                        if c.is_ascii_whitespace() {
                            i += 1;
                        } else if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c == b'#' || c == b'@' {
                            let s = i;
                            while i < p.end
                                && (bytes[i].is_ascii_alphanumeric()
                                    || matches!(bytes[i], b'_' | b'$' | b'#' | b'@')
                                    || (bytes[i] == b'-'
                                        && i + 1 < p.end
                                        && bytes[i + 1].is_ascii_alphanumeric()
                                        && bytes[s].is_ascii_alphabetic()))
                            {
                                i += 1;
                            }
                            toks.push(Tok {
                                w: W::Word(text[s..i].to_ascii_uppercase()),
                                start: s,
                                end: i,
                                depth,
                            });
                        } else {
                            if c == b')' {
                                depth -= 1;
                            }
                            toks.push(Tok { w: W::Punct(c as char), start: i, end: i + 1, depth });
                            if c == b'(' {
                                depth += 1;
                            }
                            i += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        Self { b, hosts, toks }
    }

    fn word(&self, i: usize) -> Option<&str> {
        match self.toks.get(i).map(|t| &t.w) {
            Some(W::Word(w)) => Some(w.as_str()),
            _ => None,
        }
    }

    fn is(&self, i: usize, w: &str) -> bool {
        self.word(i) == Some(w)
    }

    fn line(&self, offset: usize) -> u32 {
        self.b.line_of(offset)
    }

    fn host_ref(&self, k: usize) -> SqlHostRef {
        let h = &self.hosts[k];
        SqlHostRef { var: conv(&h.var), indicator: h.indicator.as_ref().map(conv), line: self.line(h.start) }
    }

    /// A value at token `i`: a string, a host variable or a word.
    fn value(&self, i: usize) -> Option<SqlValue> {
        Some(match &self.toks.get(i)?.w {
            W::Str(s) => SqlValue::Literal(s.clone()),
            W::Host(k) => SqlValue::Host(self.host_ref(*k)),
            W::Word(w) => SqlValue::Name(w.clone()),
            W::Punct(_) => return None,
        })
    }

    /// Host variables from token `i`, separated by commas; returns them and
    /// the index after the last.
    fn host_list(&self, mut i: usize) -> (Vec<SqlHostRef>, usize) {
        let mut out = Vec::new();
        while let Some(W::Host(k)) = self.toks.get(i).map(|t| &t.w) {
            out.push(self.host_ref(*k));
            i += 1;
            if matches!(self.toks.get(i).map(|t| &t.w), Some(W::Punct(','))) {
                i += 1;
            } else {
                break;
            }
        }
        (out, i)
    }

    /// The SQL text between byte offsets `from` and `to`, host variables made
    /// inputs and `CURRENT OF cursor` made a marker. A host variable whose
    /// range lies in `skip` is left out (the cut-out `INTO` list).
    fn sql_text(&self, from: usize, to: usize, skip: Option<(usize, usize)>) -> SqlText {
        let text = &self.b.text;
        let mut parts: Vec<SqlPart> = Vec::new();
        let mut inputs = Vec::new();
        let mut at = from;
        let push_text = |parts: &mut Vec<SqlPart>, s: &str| {
            let s = scan::strip_line_comments(s);
            if s.is_empty() {
                return;
            }
            if let Some(SqlPart::Text(prev)) = parts.last_mut() {
                prev.push_str(&s);
            } else {
                parts.push(SqlPart::Text(s));
            }
        };
        // Markers in source order: host variables and CURRENT OF.
        let mut marks: Vec<(usize, usize, Option<usize>, Option<String>)> = Vec::new();
        for (k, h) in self.hosts.iter().enumerate() {
            if h.start >= from && h.end <= to && !skip.is_some_and(|(a, b)| h.start >= a && h.end <= b) {
                marks.push((h.start, h.end, Some(k), None));
            }
        }
        for (i, t) in self.toks.iter().enumerate() {
            if t.start >= from && t.end <= to && self.is(i, "CURRENT") && self.is(i + 1, "OF") {
                if let Some(c) = self.word(i + 2) {
                    marks.push((t.start, self.toks[i + 2].end, None, Some(c.to_string())));
                }
            }
        }
        marks.sort_by_key(|m| m.0);
        // Text in [a, b), less the cut-out range.
        let emit = |parts: &mut Vec<SqlPart>, a: usize, b: usize| match skip {
            Some((x, y)) => {
                if a < x {
                    push_text(parts, &text[a..b.min(x)]);
                }
                if b > y {
                    push_text(parts, &text[a.max(y)..b]);
                }
            }
            None => push_text(parts, &text[a..b]),
        };
        for (s, e, host, cursor) in marks {
            if s < at {
                continue;
            }
            emit(&mut parts, at, s);
            match (host, cursor) {
                (Some(k), _) => {
                    parts.push(SqlPart::Input(inputs.len()));
                    inputs.push(self.host_ref(k));
                }
                (_, Some(c)) => parts.push(SqlPart::CurrentOf(c)),
                _ => {}
            }
            at = e;
        }
        if at < to {
            emit(&mut parts, at, to);
        }
        // Trim the outer white space of the statement.
        if let Some(SqlPart::Text(t)) = parts.first_mut() {
            *t = t.trim_start().to_string();
        }
        if let Some(SqlPart::Text(t)) = parts.last_mut() {
            *t = t.trim_end().to_string();
        }
        parts.retain(|p| !matches!(p, SqlPart::Text(t) if t.is_empty()));
        SqlText { parts, inputs }
    }
}

fn conv(h: &scan::HostName) -> SqlHostName {
    SqlHostName { name: h.name.clone(), quals: h.quals.clone(), swapped: h.swapped.clone() }
}

/// What a block declares or does.
enum Parsed {
    Kind(SqlKind),
    Cursor(SqlCursor),
    /// A `WHENEVER` (already applied) or another compile-time declaration.
    Declarative,
    /// `BEGIN/END DECLARE SECTION`, `DECLARE … TABLE`: no effect at all.
    Transparent,
}

/// Does this block run (as opposed to declaring)?
fn is_executable(p: &Parsed) -> bool {
    matches!(p, Parsed::Kind(_))
}

/// Read a block. Errors go to `diags`, each on its own line.
fn classify(p: &mut Parser, b: &SqlBlock, span: Span, in_data: bool, diags: &mut Vec<Diagnostic>) -> Parsed {
    let blk = Block::new(b);
    let at = |offset: usize| Span::new(span.start, span.end, b.line_of(offset), span.col);
    let err = |diags: &mut Vec<Diagnostic>, offset: usize, msg: String| diags.push(Diagnostic::error(msg, at(offset)));

    // R14 — host variables are never subscripted or reference-modified, and
    // COBOL's figurative constants have no meaning inside SQL.
    for h in &blk.hosts {
        if h.subscripted {
            err(
                diags,
                h.start,
                format!(
                    "host variable :{} cannot be subscripted or reference-modified inside EXEC SQL",
                    h.var.name
                ),
            );
        }
    }
    for t in &blk.toks {
        if let W::Word(w) = &t.w {
            if FIGURATIVES.contains(&w.as_str()) {
                let raw = &b.text[t.start..t.end];
                if !raw.starts_with('"') && !raw.starts_with('`') {
                    err(
                        diags,
                        t.start,
                        format!(
                            "figurative constant {w} cannot be used inside EXEC SQL; write the value \
                             as an SQL literal, or quote the name if it is a column"
                        ),
                    );
                }
            }
        }
    }

    let first = blk.word(0).unwrap_or("").to_string();
    let first_off = blk.toks.first().map(|t| t.start).unwrap_or(0);
    let all = (blk.toks.first().map(|t| t.start).unwrap_or(0), b.text.len());
    match first.as_str() {
        "" => {
            err(diags, 0, "empty EXEC SQL block".into());
            Parsed::Transparent
        }
        "BEGIN" | "END" if blk.is(1, "DECLARE") && blk.is(2, "SECTION") => Parsed::Transparent,
        "INCLUDE" => {
            err(
                diags,
                first_off,
                "EXEC SQL INCLUDE was not expanded: this source was read without the COPY \
                 preprocessor, so the copybook it names is missing"
                    .into(),
            );
            Parsed::Transparent
        }
        "WHENEVER" => {
            let (cond, next) = if blk.is(1, "SQLERROR") {
                (0, 2)
            } else if blk.is(1, "SQLWARNING") {
                (1, 2)
            } else if blk.is(1, "NOT") && blk.is(2, "FOUND") {
                (2, 3)
            } else {
                err(diags, first_off, "WHENEVER needs SQLERROR, SQLWARNING or NOT FOUND".into());
                return Parsed::Declarative;
            };
            let target = if blk.is(next, "CONTINUE") {
                None
            } else if blk.is(next, "GOTO") || blk.is(next, "GO") {
                let ti = if blk.is(next, "GO") && blk.is(next + 1, "TO") { next + 2 } else { next + 1 };
                match blk.word(ti) {
                    Some(l) => Some(l.to_string()),
                    None => {
                        err(diags, first_off, "WHENEVER … GO TO needs a paragraph or section name".into());
                        None
                    }
                }
            } else {
                err(diags, first_off, "WHENEVER needs CONTINUE or GO TO paragraph".into());
                None
            };
            match cond {
                0 => p.sql_whenever.sqlerror = target,
                1 => p.sql_whenever.sqlwarning = target,
                _ => p.sql_whenever.not_found = target,
            }
            Parsed::Declarative
        }
        "DECLARE" => {
            let name = blk.word(1).unwrap_or("").to_string();
            if blk.is(2, "TABLE") || blk.is(2, "STATEMENT") {
                return Parsed::Transparent;
            }
            if !blk.is(2, "CURSOR") {
                err(diags, first_off, "DECLARE needs CURSOR, TABLE or STATEMENT".into());
                return Parsed::Transparent;
            }
            let mut i = 3;
            let mut with_hold = false;
            if blk.is(i, "WITH") && blk.is(i + 1, "HOLD") {
                with_hold = true;
                i += 2;
            } else if blk.is(i, "WITHOUT") && blk.is(i + 1, "HOLD") {
                i += 2;
            }
            if !blk.is(i, "FOR") {
                err(diags, first_off, format!("DECLARE {name} CURSOR needs FOR followed by a query"));
                return Parsed::Transparent;
            }
            let q = i + 1;
            let query = if blk.toks.len() == q + 1 && blk.word(q).is_some() {
                SqlCursorQuery::Prepared(blk.word(q).unwrap_or("").to_string())
            } else {
                let from = blk.toks.get(q).map(|t| t.start).unwrap_or(b.text.len());
                SqlCursorQuery::Static(blk.sql_text(from, b.text.len(), None))
            };
            // `FOR UPDATE [OF …]` at the top level of the query.
            let for_update = blk
                .toks
                .iter()
                .enumerate()
                .skip(q)
                .any(|(k, t)| t.depth == 0 && blk.is(k, "FOR") && blk.is(k + 1, "UPDATE"));
            Parsed::Cursor(SqlCursor {
                name,
                owner: p.sql_owner.last().cloned().unwrap_or_default(),
                query,
                with_hold,
                for_update,
                positioned: false,
                in_data_division: in_data,
                line: span.line,
            })
        }
        "SELECT" | "WITH" | "VALUES" => {
            // Cut the top-level `INTO :host, …` out of the query, wherever it
            // stands (after FROM and after WHERE both occur in migrated code).
            let into = blk.toks.iter().enumerate().find(|(k, t)| {
                t.depth == 0 && blk.is(*k, "INTO") && matches!(blk.toks.get(k + 1).map(|t| &t.w), Some(W::Host(_)))
            });
            match into {
                Some((k, t)) => {
                    let (hosts, after) = blk.host_list(k + 1);
                    let cut_end = blk.toks[after - 1].end;
                    let text = blk.sql_text(all.0, all.1, Some((t.start, cut_end)));
                    Parsed::Kind(SqlKind::SelectInto { text, into: hosts })
                }
                None => {
                    err(
                        diags,
                        first_off,
                        "a SELECT outside a cursor needs INTO :host-variables (use DECLARE CURSOR to \
                         read several rows)"
                            .into(),
                    );
                    Parsed::Kind(SqlKind::Execute(blk.sql_text(all.0, all.1, None)))
                }
            }
        }
        "OPEN" => {
            let cursor = blk.word(1).unwrap_or("").to_string();
            let using = using_at(&blk, 2);
            Parsed::Kind(SqlKind::Open { cursor, using })
        }
        "FETCH" => {
            let mut i = 1;
            if blk.is(i, "NEXT") {
                i += 1;
            }
            if blk.is(i, "FROM") || blk.is(i, "IN") {
                i += 1;
            }
            let cursor = blk.word(i).unwrap_or("").to_string();
            i += 1;
            let into = if blk.is(i, "INTO") {
                SqlInto::Hosts(blk.host_list(i + 1).0)
            } else if blk.is(i, "USING") && blk.is(i + 1, "DESCRIPTOR") {
                SqlInto::Descriptor(descriptor_at(&blk, i + 2))
            } else {
                err(diags, first_off, format!("FETCH {cursor} needs INTO :host-variables or USING DESCRIPTOR"));
                SqlInto::Hosts(Vec::new())
            };
            Parsed::Kind(SqlKind::Fetch { cursor, into })
        }
        "CLOSE" => Parsed::Kind(SqlKind::Close { cursor: blk.word(1).unwrap_or("").to_string() }),
        "COMMIT" => Parsed::Kind(SqlKind::Commit),
        "ROLLBACK" if !blk.is(1, "TO") && !(blk.is(1, "WORK") && blk.is(2, "TO")) => {
            Parsed::Kind(SqlKind::Rollback)
        }
        "CONNECT" => {
            if !blk.is(1, "TO") {
                err(diags, first_off, "CONNECT needs TO followed by an SQL connection name".into());
                return Parsed::Transparent;
            }
            let target = blk.value(2).unwrap_or(SqlValue::Name(String::new()));
            let mut i = 3;
            let mut alias = None;
            let mut user = None;
            let mut password = None;
            while i < blk.toks.len() {
                if blk.is(i, "AS") {
                    alias = blk.word(i + 1).map(str::to_string).or_else(|| match blk.value(i + 1) {
                        Some(SqlValue::Literal(s)) => Some(s.to_ascii_uppercase()),
                        _ => None,
                    });
                    i += 2;
                } else if blk.is(i, "USER") {
                    user = blk.value(i + 1);
                    i += 2;
                } else if blk.is(i, "USING") {
                    password = blk.value(i + 1);
                    if matches!(password, Some(SqlValue::Literal(_))) {
                        err(
                            diags,
                            blk.toks[i + 1].start,
                            "CONNECT … USING with a literal password: a password never goes in the \
                             source — name a data item that receives it at run time"
                                .into(),
                        );
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
            Parsed::Kind(SqlKind::Connect { target, alias, user, password })
        }
        "SET" if blk.is(1, "CONNECTION") => {
            Parsed::Kind(SqlKind::SetConnection(blk.value(2).unwrap_or(SqlValue::Name(String::new()))))
        }
        "DISCONNECT" => Parsed::Kind(SqlKind::Disconnect(if blk.toks.len() < 2 || blk.is(1, "CURRENT") {
            SqlDisconnect::Current
        } else if blk.is(1, "ALL") {
            SqlDisconnect::All
        } else {
            SqlDisconnect::Named(blk.value(1).unwrap_or(SqlValue::Name(String::new())))
        })),
        "PREPARE" => {
            let name = blk.word(1).unwrap_or("").to_string();
            if !blk.is(2, "FROM") {
                err(diags, first_off, format!("PREPARE {name} needs FROM :host-variable or a literal"));
            }
            let from = blk.value(3).unwrap_or(SqlValue::Literal(String::new()));
            Parsed::Kind(SqlKind::Prepare { name, from })
        }
        "EXECUTE" if blk.is(1, "IMMEDIATE") => {
            Parsed::Kind(SqlKind::ExecuteImmediate(blk.value(2).unwrap_or(SqlValue::Literal(String::new()))))
        }
        "EXECUTE" => {
            let name = blk.word(1).unwrap_or("").to_string();
            Parsed::Kind(SqlKind::ExecutePrepared { name, using: using_at(&blk, 2) })
        }
        "DESCRIBE" => {
            let mut i = 1;
            let mut input = false;
            if blk.is(i, "INPUT") {
                input = true;
                i += 1;
            } else if blk.is(i, "OUTPUT") {
                i += 1;
            }
            let name = blk.word(i).unwrap_or("").to_string();
            i += 1;
            let sqlda = if blk.is(i, "INTO") {
                descriptor_at(&blk, i + 1)
            } else if blk.is(i, "USING") && blk.is(i + 1, "DESCRIPTOR") {
                descriptor_at(&blk, i + 2)
            } else {
                err(diags, first_off, format!("DESCRIBE {name} needs INTO descriptor-name"));
                SqlHostName { name: String::new(), quals: Vec::new(), swapped: None }
            };
            Parsed::Kind(SqlKind::Describe { name, sqlda, input })
        }
        // INSERT, UPDATE, DELETE, data definition, and anything else that
        // returns no rows goes to the database as written (R23).
        _ => Parsed::Kind(SqlKind::Execute(blk.sql_text(all.0, all.1, None))),
    }
}

/// `USING :a, :b` or `USING DESCRIPTOR name` at token `i`, or nothing.
fn using_at(blk: &Block, i: usize) -> SqlUsing {
    if !blk.is(i, "USING") {
        return SqlUsing::None;
    }
    if blk.is(i + 1, "DESCRIPTOR") {
        return SqlUsing::Descriptor(descriptor_at(blk, i + 2));
    }
    SqlUsing::Hosts(blk.host_list(i + 1).0)
}

/// A descriptor name at token `i`: a word, or a host variable (`:SQLDA`).
fn descriptor_at(blk: &Block, i: usize) -> SqlHostName {
    match blk.toks.get(i).map(|t| &t.w) {
        Some(W::Host(k)) => conv(&blk.hosts[*k].var),
        Some(W::Word(w)) => SqlHostName { name: w.clone(), quals: Vec::new(), swapped: None },
        _ => SqlHostName { name: String::new(), quals: Vec::new(), swapped: None },
    }
}

/// A block in the PROCEDURE DIVISION. The period after `END-EXEC` is left for
/// the statement list, so it ends the sentence (R3).
pub(crate) fn parse_exec_sql_stmt(p: &mut Parser) -> Stmt {
    let span = p.peek_span();
    let block = match p.peek().clone() {
        Token::ExecSqlBlock(b) => {
            p.advance();
            b
        }
        _ => {
            p.emit_error("internal: parse_exec_sql_stmt called without an EXEC SQL block");
            return Stmt::Continue { span };
        }
    };
    let mut diags = Vec::new();
    let parsed = classify(p, &block, span, false, &mut diags);
    p.diagnostics.extend(diags);
    let kind = match parsed {
        Parsed::Kind(k) => k,
        Parsed::Cursor(c) => {
            p.sql_cursors.push(c);
            SqlKind::Declarative
        }
        Parsed::Declarative | Parsed::Transparent => SqlKind::Declarative,
    };
    let whenever = if matches!(kind, SqlKind::Declarative) { SqlWhenever::default() } else { p.sql_whenever.clone() };
    Stmt::ExecSql(Box::new(ExecSql {
        kind,
        whenever,
        owner: p.sql_owner.last().cloned().unwrap_or_default(),
        span,
        last_line: block.end_line,
    }))
}

/// A block in WORKING-STORAGE, LOCAL-STORAGE or LINKAGE (R2, R7, R32): a
/// declare section, a `DECLARE … CURSOR` or `TABLE`, or a `WHENEVER`. An
/// executable statement there is an error. The items around it are ordinary
/// data items. The period after `END-EXEC`, if any, is consumed.
pub(crate) fn parse_exec_sql_data(p: &mut Parser) {
    let span = p.peek_span();
    let Token::ExecSqlBlock(block) = p.peek().clone() else { return };
    p.advance();
    p.eat(&Token::Period);
    let mut diags = Vec::new();
    let parsed = classify(p, &block, span, true, &mut diags);
    p.diagnostics.extend(diags);
    match parsed {
        Parsed::Cursor(c) => p.sql_cursors.push(c),
        ref k if is_executable(k) => p.diagnostics.push(Diagnostic::error(
            "an executable EXEC SQL statement belongs in the PROCEDURE DIVISION; the DATA DIVISION \
             takes declare sections, DECLARE CURSOR, DECLARE TABLE and INCLUDE",
            span,
        )),
        _ => {}
    }
}

/// A block where SQL is never allowed (R2). Consumed with its period.
pub(crate) fn reject_exec_sql(p: &mut Parser, where_: &str) {
    let span = p.peek_span();
    p.advance();
    p.eat(&Token::Period);
    p.diagnostics.push(Diagnostic::error(
        format!(
            "EXEC SQL is not allowed in {where_}: it belongs in WORKING-STORAGE, LOCAL-STORAGE, \
             LINKAGE or the PROCEDURE DIVISION"
        ),
        span,
    ));
}

/// Mark every cursor that a `WHERE CURRENT OF` names (R26): the runtime reads
/// such a cursor so it can find the row again.
pub(crate) fn mark_positioned_cursors(prog: &mut Program) {
    fn visit(prog: &Program, used: &mut Vec<(String, String)>) {
        let mut stmts: Vec<&Stmt> = Vec::new();
        match &prog.procedure.body {
            ProcedureBody::Sections(ss) => {
                for s in ss {
                    for pg in &s.paragraphs {
                        stmts.extend(pg.stmts.iter());
                    }
                }
            }
            ProcedureBody::Paragraphs(ps) => {
                for pg in ps {
                    stmts.extend(pg.stmts.iter());
                }
            }
        }
        for s in stmts {
            s.walk(&mut |st| {
                if let Stmt::ExecSql(e) = st {
                    let text = match &e.kind {
                        SqlKind::Execute(t) => Some(t),
                        _ => None,
                    };
                    for part in text.map(|t| t.parts.iter()).into_iter().flatten() {
                        if let SqlPart::CurrentOf(c) = part {
                            used.push((e.owner.clone(), c.clone()));
                        }
                    }
                }
            });
        }
        for n in &prog.nested_programs {
            visit(n, used);
        }
    }
    fn mark(prog: &mut Program, used: &[(String, String)]) {
        for c in &mut prog.sql_cursors {
            if used.iter().any(|(owner, name)| name == &c.name && (owner == &c.owner || c.in_data_division)) {
                c.positioned = true;
            }
        }
        for n in &mut prog.nested_programs {
            mark(n, used);
        }
    }
    let mut used = Vec::new();
    visit(prog, &mut used);
    if !used.is_empty() {
        mark(prog, &used);
    }
}
