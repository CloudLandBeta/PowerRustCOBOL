// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The statement parser (spec 108, §5.1): a hand-written recursive descent
//! over [`super::lexer`] tokens for `SELECT`, `INSERT`, `UPDATE` and `DELETE`.
//!
//! Conditions follow DynamoDB's precedence — comparisons, `IN`, `BETWEEN`,
//! functions, parentheses, `NOT`, `AND`, `OR`, highest first. Everything §5.1
//! does not list is a `ValidationException` naming the construct and its
//! position (R6): the Non-goals, an empty statement, a value of a kind a fixed
//! record cannot hold, a `?` where a name belongs.

use super::lexer::{lex, Tok, Token};
use super::value::Value;
use super::{PartiqlError, Pos};
use crate::value::parse_decimal;

// ── The tree ────────────────────────────────────────────────────────────────

/// One step of a field path.
#[derive(Clone, Debug, PartialEq)]
pub enum Seg {
    Name(String),
    /// An `OCCURS` subscript as written (counted from 1; `[0]` is kept so the
    /// analyser can refuse it with its position).
    Index(u32),
}

/// A field path: `CUST-NAME`, `BILLING.CITY`, `ITEM-QTY[2]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub segs: Vec<Seg>,
    pub pos: Pos,
}

impl Path {
    /// The path as written, without quotes: `BILLING.CITY`, `ITEM-QTY[2]`.
    pub fn text(&self) -> String {
        let mut s = String::new();
        for seg in &self.segs {
            match seg {
                Seg::Name(n) => {
                    if !s.is_empty() {
                        s.push('.');
                    }
                    s.push_str(n);
                }
                Seg::Index(i) => s.push_str(&format!("[{i}]")),
            }
        }
        s
    }
}

/// A value expression: a literal, a `?`, a field, `size(field)` or `+` / `-`.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Lit(Value, Pos),
    /// The `?` with this ordinal (counted from 0 across the whole script).
    Param(usize, Pos),
    Path(Path),
    Size(Path, Pos),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
}

impl Expr {
    pub fn pos(&self) -> Pos {
        match self {
            Expr::Lit(_, p) | Expr::Param(_, p) | Expr::Size(_, p) => *p,
            Expr::Path(p) => p.pos,
            Expr::Add(a, _) | Expr::Sub(a, _) => a.pos(),
        }
    }

    /// A compact grouping form, for tests and debugging: `(+ A 1)`.
    pub fn sexp(&self) -> String {
        match self {
            Expr::Lit(Value::Num(n), _) => n.to_string(),
            Expr::Lit(Value::Str(s), _) => format!("'{s}'"),
            Expr::Param(_, _) => "?".to_string(),
            Expr::Path(p) => p.text(),
            Expr::Size(p, _) => format!("(size {})", p.text()),
            Expr::Add(a, b) => format!("(+ {} {})", a.sexp(), b.sexp()),
            Expr::Sub(a, b) => format!("(- {} {})", a.sexp(), b.sexp()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl CmpOp {
    pub fn symbol(self) -> &'static str {
        match self {
            CmpOp::Eq => "=",
            CmpOp::Ne => "<>",
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CondKind {
    Cmp(Expr, CmpOp, Expr),
    Between(Expr, Expr, Expr),
    In(Expr, Vec<Expr>),
    And(Box<Cond>, Box<Cond>),
    Or(Box<Cond>, Box<Cond>),
    Not(Box<Cond>),
    Missing { path: Path, negated: bool },
    BeginsWith(Expr, Expr),
    Contains(Expr, Expr),
    AttributeType(Path, String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cond {
    pub kind: CondKind,
    pub pos: Pos,
}

impl Cond {
    /// A compact grouping form, for tests and debugging:
    /// `(or (and (= A 1) (= B 2)) (= C 3))`.
    pub fn sexp(&self) -> String {
        match &self.kind {
            CondKind::Cmp(l, op, r) => format!("({} {} {})", op.symbol(), l.sexp(), r.sexp()),
            CondKind::Between(e, lo, hi) => {
                format!("(between {} {} {})", e.sexp(), lo.sexp(), hi.sexp())
            }
            CondKind::In(e, list) => format!(
                "(in {} [{}])",
                e.sexp(),
                list.iter().map(Expr::sexp).collect::<Vec<_>>().join(" ")
            ),
            CondKind::And(a, b) => format!("(and {} {})", a.sexp(), b.sexp()),
            CondKind::Or(a, b) => format!("(or {} {})", a.sexp(), b.sexp()),
            CondKind::Not(a) => format!("(not {})", a.sexp()),
            CondKind::Missing { path, negated } => {
                format!("({} {})", if *negated { "present" } else { "missing" }, path.text())
            }
            CondKind::BeginsWith(a, b) => format!("(begins_with {} {})", a.sexp(), b.sexp()),
            CondKind::Contains(a, b) => format!("(contains {} {})", a.sexp(), b.sexp()),
            CondKind::AttributeType(p, t) => format!("(attribute_type {} '{t}')", p.text()),
        }
    }
}

/// A `WHERE` clause with the text it was written as (naming it in a failure).
#[derive(Clone, Debug, PartialEq)]
pub struct Where {
    pub cond: Cond,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableRef {
    pub table: String,
    pub index: Option<String>,
    pub pos: Pos,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectItem {
    pub expr: Expr,
    /// The text the projection was written as; a computed column is named by it.
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Projection {
    Star,
    Items(Vec<SelectItem>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct OrderBy {
    pub path: Path,
    pub desc: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Select {
    pub projection: Projection,
    pub table: TableRef,
    pub filter: Option<Where>,
    pub order_by: Option<OrderBy>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Insert {
    pub table: TableRef,
    /// `'FIELD': value` in the order written.
    pub fields: Vec<(String, Pos, Expr)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Clause {
    Set(Path, Expr),
    Remove(Path),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Returning {
    AllOld,
    ModifiedOld,
    AllNew,
    ModifiedNew,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Update {
    pub table: TableRef,
    pub clauses: Vec<Clause>,
    pub filter: Where,
    pub returning: Option<Returning>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Delete {
    pub table: TableRef,
    pub filter: Where,
    pub returning: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Statement {
    Select(Select),
    Insert(Insert),
    Update(Update),
    Delete(Delete),
    /// `EXISTS(SELECT * FROM T WHERE full-key AND condition)`: a condition
    /// check, a statement of a transaction only (checked by the analyser).
    Exists(Select),
}

/// A parsed statement with where it sits and which `?` ordinals are its own.
#[derive(Clone, Debug, PartialEq)]
pub struct Parsed {
    pub statement: Statement,
    /// The statement's text (for messages and the debug channel).
    pub text: String,
    /// The position of its first token in the script.
    pub pos: Pos,
    /// Ordinal of its first `?` and how many it holds.
    pub first_param: usize,
    pub param_count: usize,
}

// ── Entry points ────────────────────────────────────────────────────────────

/// Parse exactly one statement (a trailing `;` is allowed).
pub fn parse_statement(text: &str) -> Result<Parsed, PartiqlError> {
    let mut all = parse_script(text)?;
    if all.len() > 1 {
        return Err(PartiqlError::validation(
            "more than one statement; use ExecuteTransaction for a unit",
            Some(all[1].pos),
        ));
    }
    Ok(all.remove(0))
}

/// Parse the statements of a transaction, separated by `;`. `?` ordinals run
/// across the whole script, as the arguments bind in order.
pub fn parse_script(text: &str) -> Result<Vec<Parsed>, PartiqlError> {
    let toks = lex(text)?;
    let mut p = Parser { toks, i: 0, text, params: 0 };
    let mut out = Vec::new();
    loop {
        while p.eat(&Tok::Semi) {}
        if p.at_eof() {
            break;
        }
        out.push(p.statement()?);
        if !p.at_eof() && !p.eat(&Tok::Semi) {
            return Err(p.unexpected("end of the statement"));
        }
    }
    if out.is_empty() {
        return Err(PartiqlError::validation("empty statement", Some(Pos { line: 1, col: 1 })));
    }
    Ok(out)
}

// ── The parser ──────────────────────────────────────────────────────────────

/// Words a bare name may not be: they must be quoted (§5.1).
const RESERVED: &[&str] = &[
    "SELECT", "FROM", "WHERE", "ORDER", "BY", "INSERT", "INTO", "VALUE", "UPDATE", "SET", "REMOVE",
    "DELETE", "RETURNING", "AND", "OR", "NOT", "IN", "BETWEEN", "IS", "MISSING", "ASC", "DESC",
    "EXISTS", "SIZE",
];

/// Clauses and statements outside §5.1, named when they turn up.
const UNSUPPORTED: &[(&str, &str)] = &[
    ("LIMIT", "LIMIT"),
    ("OFFSET", "OFFSET"),
    ("JOIN", "JOIN"),
    ("INNER", "JOIN"),
    ("LEFT", "JOIN"),
    ("RIGHT", "JOIN"),
    ("OUTER", "JOIN"),
    ("CROSS", "JOIN"),
    ("GROUP", "GROUP BY"),
    ("HAVING", "HAVING"),
    ("DISTINCT", "DISTINCT"),
    ("LIKE", "LIKE"),
    ("AS", "AS (an alias)"),
    ("UNION", "UNION"),
    ("INTERSECT", "INTERSECT"),
    ("EXCEPT", "EXCEPT"),
    ("PIVOT", "PIVOT"),
    ("UNPIVOT", "UNPIVOT"),
    ("AT", "AT"),
    ("CREATE", "CREATE"),
    ("DROP", "DROP"),
    ("ALTER", "ALTER"),
    ("UPSERT", "UPSERT"),
    ("REPLACE", "REPLACE"),
    ("EXEC", "EXEC"),
    ("TRUE", "a Boolean value"),
    ("FALSE", "a Boolean value"),
    ("NULL", "a NULL value"),
];

const AGGREGATES: &[&str] = &["COUNT", "SUM", "AVG", "MIN", "MAX"];
const COLLECTION_FUNCTIONS: &[&str] = &["LIST_APPEND", "SET_ADD", "SET_DELETE"];

struct Parser<'a> {
    toks: Vec<Token>,
    i: usize,
    text: &'a str,
    params: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> &Tok {
        &self.toks[self.i.min(self.toks.len() - 1)].tok
    }

    fn peek_at(&self, n: usize) -> &Tok {
        &self.toks[(self.i + n).min(self.toks.len() - 1)].tok
    }

    fn pos(&self) -> Pos {
        self.toks[self.i.min(self.toks.len() - 1)].pos
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), Tok::Eof)
    }

    fn bump(&mut self) -> Token {
        let t = self.toks[self.i.min(self.toks.len() - 1)].clone();
        if self.i < self.toks.len() - 1 {
            self.i += 1;
        }
        t
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == t {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Whether the next token is the bare keyword `kw` (a quoted name never is).
    fn is_kw(&self, kw: &str) -> bool {
        matches!(self.peek(), Tok::Ident(s) if s.eq_ignore_ascii_case(kw))
    }

    fn is_kw_at(&self, n: usize, kw: &str) -> bool {
        matches!(self.peek_at(n), Tok::Ident(s) if s.eq_ignore_ascii_case(kw))
    }

    fn eat_kw(&mut self, kw: &str) -> bool {
        if self.is_kw(kw) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_kw(&mut self, kw: &str) -> Result<(), PartiqlError> {
        if self.eat_kw(kw) {
            Ok(())
        } else {
            Err(self.unexpected(kw))
        }
    }

    fn expect(&mut self, t: Tok, what: &str) -> Result<(), PartiqlError> {
        if self.eat(&t) {
            Ok(())
        } else {
            Err(self.unexpected(what))
        }
    }

    fn describe(t: &Tok) -> String {
        match t {
            Tok::Ident(s) => s.clone(),
            Tok::Quoted(s) => format!("\"{s}\""),
            Tok::Str(s) => format!("'{s}'"),
            Tok::Num(s) => s.clone(),
            Tok::Param => "?".into(),
            Tok::LParen => "(".into(),
            Tok::RParen => ")".into(),
            Tok::LBracket => "[".into(),
            Tok::RBracket => "]".into(),
            Tok::LBrace => "{".into(),
            Tok::RBrace => "}".into(),
            Tok::Comma => ",".into(),
            Tok::Dot => ".".into(),
            Tok::Colon => ":".into(),
            Tok::Semi => ";".into(),
            Tok::Star => "*".into(),
            Tok::Plus => "+".into(),
            Tok::Minus => "-".into(),
            Tok::Eq => "=".into(),
            Tok::Ne => "<>".into(),
            Tok::Lt => "<".into(),
            Tok::Le => "<=".into(),
            Tok::Gt => ">".into(),
            Tok::Ge => ">=".into(),
            Tok::Eof => "the end of the statement".into(),
        }
    }

    /// "expected X, found Y" — or, when Y is a construct §5.1 leaves out, that.
    fn unexpected(&self, expected: &str) -> PartiqlError {
        if let Tok::Ident(s) = self.peek() {
            if let Some((_, what)) = UNSUPPORTED.iter().find(|(k, _)| s.eq_ignore_ascii_case(k)) {
                return PartiqlError::validation(format!("{what} is not supported"), Some(self.pos()));
            }
        }
        PartiqlError::validation(
            format!("expected {expected}, found {}", Self::describe(self.peek())),
            Some(self.pos()),
        )
    }

    fn err_here(&self, message: impl Into<String>) -> PartiqlError {
        PartiqlError::validation(message, Some(self.pos()))
    }

    fn slice_from(&self, first_tok: usize) -> String {
        let last = self.i.saturating_sub(1).max(first_tok);
        let a = self.toks[first_tok].start;
        let b = self.toks[last].end.max(a);
        self.text[a..b].to_string()
    }

    // ── Statements ──

    fn statement(&mut self) -> Result<Parsed, PartiqlError> {
        let first_tok = self.i;
        let pos = self.pos();
        let first_param = self.params;
        let statement = if self.eat_kw("SELECT") {
            Statement::Select(self.select_body()?)
        } else if self.eat_kw("INSERT") {
            Statement::Insert(self.insert()?)
        } else if self.eat_kw("UPDATE") {
            Statement::Update(self.update()?)
        } else if self.eat_kw("DELETE") {
            Statement::Delete(self.delete()?)
        } else if self.is_kw("EXISTS") && matches!(self.peek_at(1), Tok::LParen) {
            self.bump();
            self.bump();
            self.expect_kw("SELECT")?;
            let sel = self.select_body()?;
            self.expect(Tok::RParen, ")")?;
            Statement::Exists(sel)
        } else {
            return Err(self.unexpected("SELECT, INSERT, UPDATE or DELETE"));
        };
        Ok(Parsed {
            statement,
            text: self.slice_from(first_tok),
            pos,
            first_param,
            param_count: self.params - first_param,
        })
    }

    fn select_body(&mut self) -> Result<Select, PartiqlError> {
        if self.is_kw("DISTINCT") {
            return Err(self.err_here("DISTINCT is not supported"));
        }
        if self.is_kw("VALUE") {
            return Err(self.err_here("SELECT VALUE is not supported"));
        }
        let projection = if self.eat(&Tok::Star) {
            Projection::Star
        } else {
            let mut items = Vec::new();
            loop {
                let first = self.i;
                let expr = self.arith()?;
                items.push(SelectItem { expr, text: self.slice_from(first) });
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
            Projection::Items(items)
        };
        if !self.is_kw("FROM") {
            return Err(self.unexpected("FROM"));
        }
        self.bump();
        let table = self.table_ref()?;
        let filter = if self.is_kw("WHERE") {
            self.bump();
            Some(self.where_clause()?)
        } else {
            None
        };
        let order_by = if self.is_kw("ORDER") {
            self.bump();
            self.expect_kw("BY")?;
            let path = self.path()?;
            let desc = if self.eat_kw("DESC") {
                true
            } else {
                self.eat_kw("ASC");
                false
            };
            if self.is_kw("LIMIT") || self.eat(&Tok::Comma) {
                return Err(self.err_here("ORDER BY takes one key field"));
            }
            Some(OrderBy { path, desc })
        } else {
            None
        };
        Ok(Select { projection, table, filter, order_by })
    }

    fn insert(&mut self) -> Result<Insert, PartiqlError> {
        self.expect_kw("INTO")?;
        let table = self.table_ref()?;
        self.expect_kw("VALUE")?;
        if !matches!(self.peek(), Tok::LBrace) {
            return Err(self.unexpected("{ after VALUE"));
        }
        self.bump();
        let mut fields = Vec::new();
        if !self.eat(&Tok::RBrace) {
            loop {
                let pos = self.pos();
                let name = match self.bump().tok {
                    Tok::Str(s) | Tok::Quoted(s) => s,
                    _ => {
                        self.i -= 1;
                        return Err(self.unexpected("a field name in quotes"));
                    }
                };
                self.expect(Tok::Colon, ":")?;
                let value = self.arith()?;
                fields.push((name, pos, value));
                if self.eat(&Tok::Comma) {
                    continue;
                }
                self.expect(Tok::RBrace, "} or ,")?;
                break;
            }
        }
        Ok(Insert { table, fields })
    }

    fn update(&mut self) -> Result<Update, PartiqlError> {
        let table = self.table_ref()?;
        let mut clauses = Vec::new();
        loop {
            if self.eat_kw("SET") {
                let path = self.path()?;
                self.expect(Tok::Eq, "=")?;
                let value = self.arith()?;
                clauses.push(Clause::Set(path, value));
            } else if self.eat_kw("REMOVE") {
                clauses.push(Clause::Remove(self.path()?));
            } else {
                break;
            }
        }
        if clauses.is_empty() {
            return Err(self.unexpected("SET or REMOVE"));
        }
        if !self.is_kw("WHERE") {
            return Err(self.err_here("UPDATE needs a WHERE that names the whole RECORD KEY"));
        }
        self.bump();
        let filter = self.where_clause()?;
        let returning = if self.eat_kw("RETURNING") {
            let r = if self.eat_kw("ALL") {
                if self.eat_kw("OLD") {
                    Returning::AllOld
                } else {
                    self.expect_kw("NEW")?;
                    Returning::AllNew
                }
            } else if self.eat_kw("MODIFIED") {
                if self.eat_kw("OLD") {
                    Returning::ModifiedOld
                } else {
                    self.expect_kw("NEW")?;
                    Returning::ModifiedNew
                }
            } else {
                return Err(self.unexpected("ALL or MODIFIED"));
            };
            self.expect(Tok::Star, "*")?;
            Some(r)
        } else {
            None
        };
        Ok(Update { table, clauses, filter, returning })
    }

    fn delete(&mut self) -> Result<Delete, PartiqlError> {
        self.expect_kw("FROM")?;
        let table = self.table_ref()?;
        if !self.is_kw("WHERE") {
            return Err(self.err_here("DELETE needs a WHERE that names the whole RECORD KEY"));
        }
        self.bump();
        let filter = self.where_clause()?;
        let returning = if self.eat_kw("RETURNING") {
            self.expect_kw("ALL")?;
            self.expect_kw("OLD")?;
            self.expect(Tok::Star, "*")?;
            true
        } else {
            false
        };
        Ok(Delete { table, filter, returning })
    }

    fn table_ref(&mut self) -> Result<TableRef, PartiqlError> {
        let pos = self.pos();
        let table = self.name("a table name")?;
        let index = if self.eat(&Tok::Dot) { Some(self.name("an index name")?) } else { None };
        Ok(TableRef { table, index, pos })
    }

    fn where_clause(&mut self) -> Result<Where, PartiqlError> {
        let first = self.i;
        let cond = self.or_cond()?;
        Ok(Where { cond, text: self.slice_from(first) })
    }

    // ── Names and paths ──

    /// A bare or double-quoted name; a reserved word must be quoted.
    fn name(&mut self, what: &str) -> Result<String, PartiqlError> {
        match self.peek().clone() {
            Tok::Quoted(s) => {
                self.bump();
                Ok(s)
            }
            Tok::Ident(s) => {
                if RESERVED.iter().any(|k| s.eq_ignore_ascii_case(k)) {
                    return Err(self.err_here(format!("{s} is a keyword; write \"{s}\" in double quotes to use it as a name")));
                }
                self.bump();
                Ok(s)
            }
            Tok::Param => Err(self.err_here("a ? stands for a value, not for a name")),
            _ => Err(self.unexpected(what)),
        }
    }

    fn path(&mut self) -> Result<Path, PartiqlError> {
        let pos = self.pos();
        let mut segs = vec![Seg::Name(self.name("a field name")?)];
        loop {
            if self.eat(&Tok::Dot) {
                segs.push(Seg::Name(self.name("a field name after '.'")?));
            } else if matches!(self.peek(), Tok::LBracket) {
                self.bump();
                let n = match self.peek().clone() {
                    Tok::Num(s) if s.bytes().all(|b| b.is_ascii_digit()) => {
                        self.bump();
                        s.parse::<u32>().map_err(|_| self.err_here("subscript out of range"))?
                    }
                    _ => return Err(self.unexpected("a whole-number subscript")),
                };
                self.expect(Tok::RBracket, "]")?;
                segs.push(Seg::Index(n));
            } else {
                break;
            }
        }
        Ok(Path { segs, pos })
    }

    // ── Value expressions ──

    fn arith(&mut self) -> Result<Expr, PartiqlError> {
        let mut left = self.term()?;
        loop {
            if self.eat(&Tok::Plus) {
                let r = self.term()?;
                left = Expr::Add(Box::new(left), Box::new(r));
            } else if self.eat(&Tok::Minus) {
                let r = self.term()?;
                left = Expr::Sub(Box::new(left), Box::new(r));
            } else {
                return Ok(left);
            }
        }
    }

    fn term(&mut self) -> Result<Expr, PartiqlError> {
        let pos = self.pos();
        match self.peek().clone() {
            Tok::Num(s) => {
                self.bump();
                Ok(Expr::Lit(Value::Num(number(&s, pos)?), pos))
            }
            Tok::Str(s) => {
                self.bump();
                Ok(Expr::Lit(Value::Str(s), pos))
            }
            Tok::Param => {
                self.bump();
                let n = self.params;
                self.params += 1;
                Ok(Expr::Param(n, pos))
            }
            Tok::LParen => {
                if self.is_kw_at(1, "SELECT") {
                    return Err(self.err_here("a subquery is not supported (only EXISTS(SELECT …) in a transaction)"));
                }
                self.bump();
                let e = self.arith()?;
                self.expect(Tok::RParen, ")")?;
                Ok(e)
            }
            Tok::LBrace => Err(self.err_here("a map value is not supported: a record has fixed fields")),
            Tok::LBracket => Err(self.err_here("a list value is not supported: a record has fixed fields")),
            Tok::Lt if matches!(self.peek_at(1), Tok::Lt) => {
                Err(self.err_here("a set value is not supported: a record has fixed fields"))
            }
            Tok::Ident(s) if matches!(self.peek_at(1), Tok::LParen) => {
                let up = s.to_ascii_uppercase();
                if up == "SIZE" {
                    self.bump();
                    self.bump();
                    let p = self.path()?;
                    self.expect(Tok::RParen, ")")?;
                    Ok(Expr::Size(p, pos))
                } else if AGGREGATES.contains(&up.as_str()) {
                    Err(self.err_here(format!("the aggregate {up} is not supported")))
                } else if COLLECTION_FUNCTIONS.contains(&up.as_str()) {
                    Err(self.err_here(format!("{} is not supported: a record has no variable-length collection", s.to_ascii_lowercase())))
                } else if matches!(up.as_str(), "BEGINS_WITH" | "CONTAINS" | "ATTRIBUTE_TYPE") {
                    Err(self.err_here(format!("{} gives a condition, not a value", s.to_ascii_lowercase())))
                } else if up == "EXISTS" {
                    Err(self.err_here("EXISTS is a statement of a transaction, not a condition"))
                } else {
                    Err(self.err_here(format!("unknown function {s}")))
                }
            }
            Tok::Ident(s) if UNSUPPORTED.iter().any(|(k, w)| s.eq_ignore_ascii_case(k) && w.contains("value")) => {
                let (_, what) = UNSUPPORTED.iter().find(|(k, _)| s.eq_ignore_ascii_case(k)).unwrap();
                Err(self.err_here(format!("{what} is not supported")))
            }
            Tok::Ident(_) | Tok::Quoted(_) => Ok(Expr::Path(self.path()?)),
            _ => Err(self.unexpected("a value")),
        }
    }

    // ── Conditions ──

    fn or_cond(&mut self) -> Result<Cond, PartiqlError> {
        let mut left = self.and_cond()?;
        while self.is_kw("OR") {
            let pos = self.pos();
            self.bump();
            let right = self.and_cond()?;
            left = Cond { kind: CondKind::Or(Box::new(left), Box::new(right)), pos };
        }
        Ok(left)
    }

    fn and_cond(&mut self) -> Result<Cond, PartiqlError> {
        let mut left = self.not_cond()?;
        while self.is_kw("AND") {
            let pos = self.pos();
            self.bump();
            let right = self.not_cond()?;
            left = Cond { kind: CondKind::And(Box::new(left), Box::new(right)), pos };
        }
        Ok(left)
    }

    fn not_cond(&mut self) -> Result<Cond, PartiqlError> {
        if self.is_kw("NOT") {
            let pos = self.pos();
            self.bump();
            let inner = self.not_cond()?;
            return Ok(Cond { kind: CondKind::Not(Box::new(inner)), pos });
        }
        self.primary_cond()
    }

    fn primary_cond(&mut self) -> Result<Cond, PartiqlError> {
        if matches!(self.peek(), Tok::LParen) {
            if self.is_kw_at(1, "SELECT") {
                return Err(self.err_here("a subquery is not supported (only EXISTS(SELECT …) in a transaction)"));
            }
            // `( condition )` first; when that does not fit, the parenthesis
            // belongs to an arithmetic operand: `(A + 1) > 2`.
            let save = (self.i, self.params);
            let pos = self.pos();
            self.bump();
            if let Ok(c) = self.or_cond() {
                if self.eat(&Tok::RParen) {
                    return Ok(Cond { pos, ..c });
                }
            }
            self.i = save.0;
            self.params = save.1;
        }
        self.predicate()
    }

    fn predicate(&mut self) -> Result<Cond, PartiqlError> {
        let pos = self.pos();
        // Functions that give a condition.
        if let Tok::Ident(s) = self.peek().clone() {
            if matches!(self.peek_at(1), Tok::LParen) {
                let up = s.to_ascii_uppercase();
                match up.as_str() {
                    "BEGINS_WITH" | "CONTAINS" => {
                        self.bump();
                        self.bump();
                        let a = self.arith()?;
                        self.expect(Tok::Comma, ",")?;
                        let b = self.arith()?;
                        self.expect(Tok::RParen, ")")?;
                        let kind = if up == "CONTAINS" {
                            CondKind::Contains(a, b)
                        } else {
                            CondKind::BeginsWith(a, b)
                        };
                        return Ok(Cond { kind, pos });
                    }
                    "ATTRIBUTE_TYPE" => {
                        self.bump();
                        self.bump();
                        let p = self.path()?;
                        self.expect(Tok::Comma, ",")?;
                        let t = match self.bump().tok {
                            Tok::Str(s) => s,
                            _ => {
                                self.i -= 1;
                                return Err(self.unexpected("a type code in quotes"));
                            }
                        };
                        self.expect(Tok::RParen, ")")?;
                        return Ok(Cond { kind: CondKind::AttributeType(p, t), pos });
                    }
                    _ => {}
                }
            }
        }
        let left = self.arith()?;
        let kind = match self.peek().clone() {
            Tok::Eq => self.cmp(left, CmpOp::Eq)?,
            Tok::Ne => self.cmp(left, CmpOp::Ne)?,
            Tok::Lt => self.cmp(left, CmpOp::Lt)?,
            Tok::Le => self.cmp(left, CmpOp::Le)?,
            Tok::Gt => self.cmp(left, CmpOp::Gt)?,
            Tok::Ge => self.cmp(left, CmpOp::Ge)?,
            Tok::Ident(s) if s.eq_ignore_ascii_case("BETWEEN") => {
                self.bump();
                let lo = self.arith()?;
                self.expect_kw("AND")?;
                let hi = self.arith()?;
                CondKind::Between(left, lo, hi)
            }
            Tok::Ident(s) if s.eq_ignore_ascii_case("IN") => {
                self.bump();
                let close = match self.peek() {
                    Tok::LBracket => Tok::RBracket,
                    Tok::LParen => Tok::RParen,
                    Tok::Lt if matches!(self.peek_at(1), Tok::Lt) => {
                        return Err(self.err_here("a set value is not supported: a record has fixed fields"))
                    }
                    _ => return Err(self.unexpected("[ or ( after IN")),
                };
                self.bump();
                let mut list = vec![self.arith()?];
                while self.eat(&Tok::Comma) {
                    list.push(self.arith()?);
                }
                self.expect(close, "the end of the IN list")?;
                CondKind::In(left, list)
            }
            Tok::Ident(s) if s.eq_ignore_ascii_case("IS") => {
                self.bump();
                let negated = self.eat_kw("NOT");
                self.expect_kw("MISSING")?;
                match left {
                    Expr::Path(path) => CondKind::Missing { path, negated },
                    _ => return Err(PartiqlError::validation("IS MISSING needs a field", Some(pos))),
                }
            }
            _ => return Err(self.unexpected("a comparison, BETWEEN, IN or IS MISSING")),
        };
        Ok(Cond { kind, pos })
    }

    fn cmp(&mut self, left: Expr, op: CmpOp) -> Result<CondKind, PartiqlError> {
        self.bump();
        let right = self.arith()?;
        Ok(CondKind::Cmp(left, op, right))
    }
}

/// A numeric literal as an exact Number: `12`, `12.50`, `-3`, `1e3`, `2.5E-2`.
fn number(text: &str, pos: Pos) -> Result<crate::value::CobolNumeric, PartiqlError> {
    let (mantissa_text, exp) = match text.find(['e', 'E']) {
        Some(i) => (&text[..i], text[i + 1..].parse::<i32>().ok()),
        None => (text, Some(0)),
    };
    let bad = || PartiqlError::validation(format!("the number {text} is not valid"), Some(pos));
    let exp = exp.ok_or_else(bad)?;
    let n = parse_decimal(mantissa_text).ok_or_else(bad)?;
    if exp == 0 {
        return Ok(n);
    }
    // Shift the decimal point: the mantissa stays exact, the scale moves.
    let mut mantissa = n.mantissa;
    let mut decimals = n.decimals as i32 - exp;
    while decimals < 0 {
        mantissa = mantissa.checked_mul(10).ok_or_else(bad)?;
        decimals += 1;
    }
    if decimals > 31 {
        return Err(bad());
    }
    Ok(crate::value::CobolNumeric::new(mantissa, decimals as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel(text: &str) -> Select {
        match parse_statement(text).unwrap().statement {
            Statement::Select(s) => s,
            o => panic!("not a select: {o:?}"),
        }
    }

    fn cond(text: &str) -> String {
        sel(&format!("SELECT * FROM T WHERE {text}"))
            .filter
            .unwrap()
            .cond
            .sexp()
    }

    #[test]
    fn precedence_is_comparison_not_and_or() {
        assert_eq!(cond("A = 1 AND B = 2 OR C = 3"), "(or (and (= A 1) (= B 2)) (= C 3))");
        assert_eq!(cond("A = 1 OR B = 2 AND C = 3"), "(or (= A 1) (and (= B 2) (= C 3)))");
        assert_eq!(cond("NOT A = 1 AND B = 2"), "(and (not (= A 1)) (= B 2))");
        assert_eq!(cond("NOT (A = 1 OR B = 2)"), "(not (or (= A 1) (= B 2)))");
        assert_eq!(cond("(A = 1 OR B = 2) AND C = 3"), "(and (or (= A 1) (= B 2)) (= C 3))");
    }

    #[test]
    fn between_keeps_its_own_and() {
        assert_eq!(cond("A BETWEEN 1 AND 5 AND B = 2"), "(and (between A 1 5) (= B 2))");
        assert_eq!(cond("A BETWEEN ? AND ?"), "(between A ? ?)");
    }

    #[test]
    fn in_takes_brackets_and_parentheses() {
        assert_eq!(cond("A IN [1, 2, 3]"), "(in A [1 2 3])");
        assert_eq!(cond("A IN (1, 2)"), "(in A [1 2])");
        assert_eq!(cond("A IN ('x')"), "(in A ['x'])");
    }

    #[test]
    fn arithmetic_groups_left_to_right_and_a_paren_operand_is_not_a_condition() {
        assert_eq!(cond("A - 1 + 2 > 0"), "(> (+ (- A 1) 2) 0)");
        assert_eq!(cond("(A + 1) > 2"), "(> (+ A 1) 2)");
        assert_eq!(cond("QTY-1 > 2"), "(> QTY-1 2)");
        assert_eq!(cond("QTY - 1 > 2"), "(> (- QTY 1) 2)");
    }

    #[test]
    fn functions_and_missing() {
        assert_eq!(cond("begins_with(NAME, 'A')"), "(begins_with NAME 'A')");
        assert_eq!(cond("contains(ITEM-QTY, 3)"), "(contains ITEM-QTY 3)");
        assert_eq!(cond("size(NAME) > 3"), "(> (size NAME) 3)");
        assert_eq!(cond("attribute_type(NAME, 'S')"), "(attribute_type NAME 'S')");
        assert_eq!(cond("NAME IS MISSING"), "(missing NAME)");
        assert_eq!(cond("NAME IS NOT MISSING"), "(present NAME)");
    }

    #[test]
    fn paths_and_quoted_names() {
        assert_eq!(cond("BILLING.CITY = 'Recife'"), "(= BILLING.CITY 'Recife')");
        assert_eq!(cond("ITEM-QTY[2] = 1"), "(= ITEM-QTY[2] 1)");
        assert_eq!(cond("\"VALUE\" = 1"), "(= VALUE 1)");
        let s = sel("select * from \"CUSTOMERS\".\"BY-NAME\"");
        assert_eq!(s.table.table, "CUSTOMERS");
        assert_eq!(s.table.index.as_deref(), Some("BY-NAME"));
    }

    #[test]
    fn select_forms() {
        let s = sel("SELECT CUST-ID, CUST-BALANCE - 1, size(NAME) FROM CUSTOMERS ORDER BY CUST-ID DESC");
        let Projection::Items(items) = &s.projection else { panic!() };
        assert_eq!(items.len(), 3);
        assert_eq!(items[1].text, "CUST-BALANCE - 1");
        assert_eq!(items[1].expr.sexp(), "(- CUST-BALANCE 1)");
        assert!(s.order_by.unwrap().desc);
        assert!(matches!(sel("SELECT * FROM T").projection, Projection::Star));
    }

    #[test]
    fn insert_update_delete_forms() {
        let Statement::Insert(i) = parse_statement("INSERT INTO T VALUE {'A': 1, 'B': 'x', 'C': ?}").unwrap().statement
        else {
            panic!()
        };
        assert_eq!(i.fields.len(), 3);
        let Statement::Update(u) = parse_statement(
            "UPDATE T SET A = A - ? SET B = 'x' REMOVE C WHERE K = 1 RETURNING ALL NEW *",
        )
        .unwrap()
        .statement
        else {
            panic!()
        };
        assert_eq!(u.clauses.len(), 3);
        assert_eq!(u.returning, Some(Returning::AllNew));
        assert_eq!(u.filter.text, "K = 1");
        for (txt, r) in [
            ("ALL OLD", Returning::AllOld),
            ("MODIFIED OLD", Returning::ModifiedOld),
            ("MODIFIED NEW", Returning::ModifiedNew),
        ] {
            let Statement::Update(u) = parse_statement(&format!("UPDATE T SET A = 1 WHERE K = 1 RETURNING {txt} *")).unwrap().statement else { panic!() };
            assert_eq!(u.returning, Some(r));
        }
        let Statement::Delete(d) = parse_statement("DELETE FROM T WHERE K = ? RETURNING ALL OLD *").unwrap().statement
        else {
            panic!()
        };
        assert!(d.returning);
    }

    #[test]
    fn parameters_are_numbered_across_a_script() {
        let all = parse_script("UPDATE T SET A = ? WHERE K = ?; DELETE FROM T WHERE K = ?;").unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!((all[0].first_param, all[0].param_count), (0, 2));
        assert_eq!((all[1].first_param, all[1].param_count), (2, 1));
        assert_eq!(all[1].text, "DELETE FROM T WHERE K = ?");
    }

    #[test]
    fn numbers_keep_their_scale() {
        let s = sel("SELECT * FROM T WHERE A > 1000.50 AND B = 1e3 AND C = -3 AND D = 2.5E-2");
        let c = s.filter.unwrap().cond.sexp();
        assert_eq!(c, "(and (and (and (> A 1000.50) (= B 1000)) (= C -3)) (= D 0.025))");
    }

    fn rejected(text: &str) -> PartiqlError {
        parse_statement(text).unwrap_err()
    }

    #[test]
    fn every_non_goal_is_refused_naming_its_construct_and_position() {
        let cases = [
            ("SELECT COUNT(*) FROM T", "COUNT", 8),
            ("SELECT SUM(A) FROM T", "SUM", 8),
            ("SELECT * FROM T LIMIT 5", "LIMIT", 17),
            ("SELECT * FROM T OFFSET 5", "OFFSET", 17),
            ("SELECT * FROM T JOIN U", "JOIN", 17),
            ("SELECT * FROM T GROUP BY A", "GROUP BY", 17),
            ("SELECT DISTINCT A FROM T", "DISTINCT", 8),
            ("SELECT * FROM T WHERE A LIKE 'x'", "LIKE", 25),
            ("SELECT A AS B FROM T", "AS", 10),
            ("SELECT * FROM T WHERE A = (SELECT B FROM U)", "subquery", 27),
            ("CREATE TABLE T", "CREATE", 1),
            ("DROP TABLE T", "DROP", 1),
            ("SELECT * FROM T WHERE A = TRUE", "Boolean", 27),
            ("SELECT * FROM T WHERE A = NULL", "NULL", 27),
            ("INSERT INTO T VALUE {'A': {'B': 1}}", "map value", 27),
            ("INSERT INTO T VALUE {'A': [1, 2]}", "list value", 27),
            ("UPDATE T SET A = list_append(A, 1) WHERE K = 1", "list_append", 18),
            ("UPDATE T SET A = set_add(A, 1) WHERE K = 1", "set_add", 18),
            ("UPDATE T SET A = set_delete(A, 1) WHERE K = 1", "set_delete", 18),
        ];
        for (text, needle, col) in cases {
            let e = rejected(text);
            assert_eq!(e.exception, "ValidationException", "{text}");
            assert_eq!(e.status, "94", "{text}");
            assert!(
                e.message.to_ascii_lowercase().contains(&needle.to_ascii_lowercase()),
                "{text}: {}",
                e.message
            );
            assert_eq!(e.pos.map(|p| p.col), Some(col), "{text}: {}", e.text());
        }
    }

    #[test]
    fn an_empty_statement_a_stray_parameter_and_a_keyword_name_are_refused() {
        assert!(rejected("").message.contains("empty statement"));
        assert!(rejected("  ;  ").message.contains("empty statement"));
        assert!(rejected("SELECT * FROM ?").message.contains("not for a name"));
        assert!(rejected("SELECT VALUE FROM T").message.contains("SELECT VALUE"));
        assert!(rejected("SELECT * FROM T WHERE SET = 1").message.contains("double quotes"));
        assert!(rejected("UPDATE T SET A = 1").message.contains("WHERE"));
        assert!(rejected("DELETE FROM T").message.contains("WHERE"));
        assert!(rejected("SELECT * FROM T; SELECT * FROM U").message.contains("ExecuteTransaction"));
    }

    #[test]
    fn a_name_that_is_a_keyword_works_in_quotes() {
        let s = sel("SELECT \"VALUE\", \"SET\" FROM \"FROM\"");
        assert_eq!(s.table.table, "FROM");
    }

    #[test]
    fn truncating_any_statement_never_panics() {
        let corpus = [
            "SELECT CUST-ID, CUST-NAME FROM CUSTOMERS WHERE CUST-ID BETWEEN ? AND ? AND NOT begins_with(CUST-NAME, 'A') ORDER BY CUST-ID DESC",
            "INSERT INTO ORDERS VALUE {'ORD-ID': ?, 'AMOUNT': 12.50, 'NOTE': 'O''Brien'}",
            "UPDATE CUSTOMERS SET CUST-BALANCE = CUST-BALANCE - ? REMOVE NOTE WHERE CUST-ID = ? AND (CUST-BALANCE >= ? OR size(NOTE) > 1) RETURNING MODIFIED NEW *",
            "DELETE FROM CUSTOMERS WHERE CUST-ID IN [1, 2, 3] RETURNING ALL OLD *; EXISTS(SELECT * FROM T WHERE K = 1 AND A > 1); SELECT * FROM ORDERS.\"BY-DATE\"",
        ];
        let mut parsed = 0usize;
        for s in corpus {
            for n in 0..=s.len() {
                if !s.is_char_boundary(n) {
                    continue;
                }
                let _ = parse_script(&s[..n]);
                parsed += 1;
            }
        }
        assert!(parsed > 400);
    }
}
