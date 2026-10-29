// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Analysis (spec 108, §5.2, §5.4, R6): resolves a parsed statement against a
//! [`Schema`] and refuses, before any file is opened, everything the schema
//! makes wrong — an unknown name, a Number compared with a String, a list
//! index of 0 or past the last occurrence, `ORDER BY` a field outside the key
//! of reference, an `UPDATE` or `DELETE` that does not pin the whole RECORD
//! KEY, `SET` on a key field.
//!
//! No file is touched: Check, the guardian and Test query call this without a
//! data file.

use std::collections::HashMap;

use super::parser::{
    Clause, CmpOp, Cond, CondKind, Delete, Expr, Insert, Parsed, Path, Projection, Returning, Seg, Select, Statement,
    TableRef, Update,
};
use super::schema::{Class, Key, Schema};
use super::value::Value;
use super::{PartiqlError, Pos};

/// The type of a value: what §5.2 calls Number and String.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Num,
    Str,
}

impl Ty {
    pub fn name(self) -> &'static str {
        match self {
            Ty::Num => "Number",
            Ty::Str => "String",
        }
    }
}

/// An occurrence of a field: `occ` counts from 0 here (the text counts from 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldRef {
    pub field: usize,
    pub occ: usize,
}

/// A value expression with every name resolved.
#[derive(Clone, Debug, PartialEq)]
pub enum RExpr {
    Lit(Value),
    /// The `?` with this ordinal across the script.
    Param(usize),
    Field(FieldRef),
    /// `size(field)` of a String field: its length without trailing spaces.
    SizeText(FieldRef),
    Add(Box<RExpr>, Box<RExpr>),
    Sub(Box<RExpr>, Box<RExpr>),
}

impl RExpr {
    /// Whether the expression reads no field (so it is a value known before
    /// the record is).
    pub fn is_constant(&self) -> bool {
        match self {
            RExpr::Lit(_) | RExpr::Param(_) => true,
            RExpr::Field(_) | RExpr::SizeText(_) => false,
            RExpr::Add(a, b) | RExpr::Sub(a, b) => a.is_constant() && b.is_constant(),
        }
    }
}

/// A condition with every name resolved.
#[derive(Clone, Debug, PartialEq)]
pub enum RCond {
    Cmp(RExpr, CmpOp, RExpr),
    Between(RExpr, RExpr, RExpr),
    In(RExpr, Vec<RExpr>),
    And(Box<RCond>, Box<RCond>),
    Or(Box<RCond>, Box<RCond>),
    Not(Box<RCond>),
    /// `IS MISSING`, `attribute_type`: decided by the schema alone.
    Const(bool),
    BeginsWith(RExpr, RExpr),
    ContainsText(RExpr, RExpr),
    /// `contains(table-field, value)`: an occurrence equal to the value.
    ContainsOccurs(usize, RExpr),
}

/// `EXISTS(SELECT * FROM T WHERE full-key AND condition)`: a check.
#[derive(Clone, Debug, PartialEq)]
pub struct RExists {
    pub pins: Vec<(usize, RExpr)>,
    pub further: Option<RCond>,
    pub where_text: String,
}

/// One column of a result set.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    /// The name `ColumnNames` shows: a field's name or path, an occurrence as
    /// `NAME[2]`, a computed expression by its text.
    pub name: String,
    pub expr: RExpr,
    pub ty: Option<Ty>,
    /// The COBOL name the control publishes this value under as a property:
    /// a plain, unique, single-occurrence field.
    pub prop: Option<String>,
    /// The width an unsigned whole-number field is shown zero-filled to, as the
    /// item itself shows it (`PIC 9(8)` holding 42 is `00000042`); 0 for
    /// anything else (a signed or decimal field, a computed value).
    pub pad: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RSelect {
    pub columns: Vec<Column>,
    pub filter: Option<RCond>,
    /// The key of reference: 0 is the RECORD KEY, `i + 1` the alternate `i`.
    pub kor: usize,
    pub desc: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RInsert {
    pub sets: Vec<(FieldRef, RExpr)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RClause {
    Set(FieldRef, RExpr),
    /// Every listed occurrence takes its initial value.
    Remove(Vec<FieldRef>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RUpdate {
    /// One equality per field of the RECORD KEY.
    pub pins: Vec<(usize, RExpr)>,
    /// The conditions beyond the key.
    pub further: Option<RCond>,
    pub where_text: String,
    pub clauses: Vec<RClause>,
    pub returning: Option<Returning>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RDelete {
    pub pins: Vec<(usize, RExpr)>,
    pub further: Option<RCond>,
    pub where_text: String,
    pub returning: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RStmt {
    Select(RSelect),
    Insert(RInsert),
    Update(RUpdate),
    Delete(RDelete),
    Exists(RExists),
}

/// A statement ready to run.
#[derive(Clone, Debug, PartialEq)]
pub struct Analyzed {
    pub stmt: RStmt,
    pub text: String,
    pub pos: Pos,
    pub first_param: usize,
    pub param_count: usize,
    /// The type each of this statement's `?` is compared or assigned with
    /// (`None` when the statement does not fix it), by local index.
    pub expect: Vec<Option<Ty>>,
}

/// Analyse one parsed statement. `in_transaction` allows `EXISTS`.
pub fn analyze(parsed: &Parsed, schema: &Schema, in_transaction: bool) -> Result<Analyzed, PartiqlError> {
    let mut a = Analyzer { schema, in_tx: in_transaction, expect: HashMap::new() };
    let stmt = match &parsed.statement {
        Statement::Select(s) => RStmt::Select(a.select(s)?),
        Statement::Insert(i) => RStmt::Insert(a.insert(i)?),
        Statement::Update(u) => RStmt::Update(a.update(u)?),
        Statement::Delete(d) => RStmt::Delete(a.delete(d)?),
        Statement::Exists(s) => RStmt::Exists(a.exists(s, parsed.pos)?),
    };
    let expect = (0..parsed.param_count).map(|i| a.expect.get(&(parsed.first_param + i)).copied()).collect();
    Ok(Analyzed {
        stmt,
        text: parsed.text.clone(),
        pos: parsed.pos,
        first_param: parsed.first_param,
        param_count: parsed.param_count,
        expect,
    })
}

/// A path resolved to what it names.
enum Target {
    Field(FieldRef),
    /// An `OCCURS` field named without a subscript.
    Table(usize),
    Group(usize),
}

struct Analyzer<'a> {
    schema: &'a Schema,
    in_tx: bool,
    expect: HashMap<usize, Ty>,
}

fn err(msg: impl Into<String>, pos: Pos) -> PartiqlError {
    PartiqlError::validation(msg, Some(pos))
}

impl<'a> Analyzer<'a> {
    // ── Tables and keys ──

    fn table(&self, t: &TableRef) -> Result<usize, PartiqlError> {
        if !t.table.eq_ignore_ascii_case(&self.schema.table) {
            return Err(PartiqlError::resource_not_found(&t.table, Some(t.pos)));
        }
        match &t.index {
            None => Ok(0),
            Some(i) => self
                .schema
                .key_of_reference(Some(i))
                .ok_or_else(|| PartiqlError::resource_not_found(&format!("{}.{}", t.table, i), Some(t.pos))),
        }
    }

    /// The table of a write: no index (the RECORD KEY names the record).
    fn write_table(&self, t: &TableRef) -> Result<(), PartiqlError> {
        if self.table(t)? != 0 {
            return Err(err("a write names the table only, not an index", t.pos));
        }
        Ok(())
    }

    // ── Paths ──

    fn target(&self, path: &Path) -> Result<Target, PartiqlError> {
        let mut names: Vec<String> = Vec::new();
        let mut index: Option<u32> = None;
        for (i, seg) in path.segs.iter().enumerate() {
            match seg {
                Seg::Name(n) => {
                    if index.is_some() {
                        return Err(err("a subscript can only end a path", path.pos));
                    }
                    names.push(n.to_ascii_uppercase());
                }
                Seg::Index(n) => {
                    if i + 1 != path.segs.len() {
                        return Err(err("a subscript can only end a path", path.pos));
                    }
                    index = Some(*n);
                }
            }
        }
        let last = names.last().cloned().unwrap_or_default();
        let given: Vec<&String> = names[..names.len().saturating_sub(1)].iter().collect();
        let subseq = |hay_outer_first: &[String]| {
            let mut it = hay_outer_first.iter();
            given.iter().all(|g| it.any(|h| h == *g))
        };
        let mut found: Vec<Target> = Vec::new();
        for (i, f) in self.schema.fields.iter().enumerate() {
            if f.name == last {
                let outer: Vec<String> = f.quals.iter().rev().cloned().collect();
                if subseq(&outer) {
                    found.push(if f.is_table() { Target::Table(i) } else { Target::Field(FieldRef { field: i, occ: 0 }) });
                }
            }
        }
        for (i, g) in self.schema.groups.iter().enumerate() {
            if g.name == last {
                let outer: Vec<String> = g.quals.iter().rev().cloned().collect();
                if subseq(&outer) {
                    found.push(Target::Group(i));
                }
            }
        }
        let t = match found.len() {
            0 => {
                return Err(err(
                    format!("{} is not a field of {}", path.text(), self.schema.table),
                    path.pos,
                ))
            }
            1 => found.pop().unwrap(),
            _ => {
                return Err(err(
                    format!("{} is ambiguous; qualify it with its group (GROUP.{})", path.text(), last),
                    path.pos,
                ))
            }
        };
        match (t, index) {
            (Target::Group(g), None) => Ok(Target::Group(g)),
            (Target::Group(_), Some(_)) => Err(err(format!("{} is a group and has no subscript", last), path.pos)),
            (Target::Field(r), None) => self.not_in_table(r, path),
            (Target::Field(_), Some(_)) => Err(err(format!("{last} has no OCCURS, so it takes no subscript"), path.pos)),
            (Target::Table(f), None) => {
                if self.schema.fields[f].in_occurs_group {
                    return Err(self.in_group_err(f, path));
                }
                Ok(Target::Table(f))
            }
            (Target::Table(f), Some(n)) => {
                let fld = &self.schema.fields[f];
                if fld.in_occurs_group {
                    return Err(self.in_group_err(f, path));
                }
                if n == 0 {
                    return Err(err(
                        format!("subscripts count from 1, as COBOL's do: {last}[0] is not valid; the first is {last}[1]"),
                        path.pos,
                    ));
                }
                if n as usize > fld.occ.len() {
                    return Err(err(
                        format!("{last} has {} occurrences; {last}[{n}] is past the last", fld.occ.len()),
                        path.pos,
                    ));
                }
                Ok(Target::Field(FieldRef { field: f, occ: n as usize - 1 }))
            }
        }
    }

    fn not_in_table(&self, r: FieldRef, path: &Path) -> Result<Target, PartiqlError> {
        if self.schema.fields[r.field].in_occurs_group {
            return Err(self.in_group_err(r.field, path));
        }
        Ok(Target::Field(r))
    }

    fn in_group_err(&self, f: usize, path: &Path) -> PartiqlError {
        err(
            format!(
                "{} sits under a group with OCCURS and is not addressable in this release",
                self.schema.fields[f].name
            ),
            path.pos,
        )
    }

    /// A path that must name one elementary occurrence.
    fn field_ref(&self, path: &Path) -> Result<FieldRef, PartiqlError> {
        match self.target(path)? {
            Target::Field(r) => Ok(r),
            Target::Table(f) => Err(err(
                format!(
                    "{0} has {1} occurrences; write {0}[n] (counted from 1)",
                    self.schema.fields[f].name,
                    self.schema.fields[f].occ.len()
                ),
                path.pos,
            )),
            Target::Group(g) => Err(err(
                format!("{} is a group, not a value; name its fields ({}.FIELD)", self.schema.groups[g].name, self.schema.groups[g].name),
                path.pos,
            )),
        }
    }

    fn class_of(&self, r: &FieldRef) -> &Class {
        &self.schema.fields[r.field].class
    }

    fn ty_of_field(&self, r: &FieldRef) -> Ty {
        if self.class_of(r).is_number() {
            Ty::Num
        } else {
            Ty::Str
        }
    }

    // ── Expressions and types ──

    fn ty(&self, e: &RExpr) -> Option<Ty> {
        match e {
            RExpr::Lit(Value::Num(_)) => Some(Ty::Num),
            RExpr::Lit(Value::Str(_)) => Some(Ty::Str),
            RExpr::Param(_) => None,
            RExpr::Field(r) => Some(self.ty_of_field(r)),
            RExpr::SizeText(_) | RExpr::Add(..) | RExpr::Sub(..) => Some(Ty::Num),
        }
    }

    /// `e` must be of type `want`; a `?` takes it as its expected type.
    fn require(&mut self, e: &RExpr, want: Ty, pos: Pos, what: &str) -> Result<(), PartiqlError> {
        match e {
            RExpr::Param(n) => match self.expect.get(n) {
                Some(t) if *t != want => Err(err(
                    format!("a parameter is used both as a {} and as a {}", t.name(), want.name()),
                    pos,
                )),
                _ => {
                    self.expect.insert(*n, want);
                    Ok(())
                }
            },
            _ => match self.ty(e) {
                Some(t) if t != want => {
                    Err(err(format!("{what} needs a {}, found a {}", want.name(), t.name()), pos))
                }
                _ => Ok(()),
            },
        }
    }

    /// Two sides of a comparison or an assignment must be of one type; a `?`
    /// takes the other side's.
    fn unify(&mut self, a: &RExpr, b: &RExpr, pos: Pos) -> Result<(), PartiqlError> {
        match (self.ty(a), self.ty(b)) {
            (Some(x), Some(y)) if x != y => Err(err(
                format!("a {} cannot be compared with a {}; write the value as a {}", x.name(), y.name(), x.name()),
                pos,
            )),
            (Some(t), None) => self.require(b, t, pos, "the comparison"),
            (None, Some(t)) => self.require(a, t, pos, "the comparison"),
            _ => Ok(()),
        }
    }

    fn expr(&mut self, e: &Expr, fields_ok: bool) -> Result<RExpr, PartiqlError> {
        match e {
            Expr::Lit(v, _) => Ok(RExpr::Lit(v.clone())),
            Expr::Param(n, _) => Ok(RExpr::Param(*n)),
            Expr::Path(p) => {
                if !fields_ok {
                    return Err(err(format!("{} is a field; a value is expected here", p.text()), p.pos));
                }
                Ok(RExpr::Field(self.field_ref(p)?))
            }
            Expr::Size(p, pos) => {
                if !fields_ok {
                    return Err(err("size() reads a field; a value is expected here", *pos));
                }
                match self.target(p)? {
                    Target::Field(r) => {
                        if self.class_of(&r).is_number() {
                            return Err(err(
                                format!("size({}) applies to Strings and collections, not to a Number", p.text()),
                                *pos,
                            ));
                        }
                        Ok(RExpr::SizeText(r))
                    }
                    Target::Table(f) => Ok(RExpr::Lit(Value::int(self.schema.fields[f].occ.len() as i64))),
                    Target::Group(g) => Ok(RExpr::Lit(Value::int(self.schema.groups[g].fields as i64))),
                }
            }
            Expr::Add(a, b) | Expr::Sub(a, b) => {
                let ra = self.expr(a, fields_ok)?;
                let rb = self.expr(b, fields_ok)?;
                let pos = a.pos();
                self.require(&ra, Ty::Num, pos, "arithmetic")?;
                self.require(&rb, Ty::Num, pos, "arithmetic")?;
                Ok(if matches!(e, Expr::Add(..)) {
                    RExpr::Add(Box::new(ra), Box::new(rb))
                } else {
                    RExpr::Sub(Box::new(ra), Box::new(rb))
                })
            }
        }
    }

    // ── Conditions ──

    fn cond(&mut self, c: &Cond) -> Result<RCond, PartiqlError> {
        Ok(match &c.kind {
            CondKind::Cmp(l, op, r) => {
                let (rl, rr) = (self.expr(l, true)?, self.expr(r, true)?);
                self.unify(&rl, &rr, c.pos)?;
                RCond::Cmp(rl, *op, rr)
            }
            CondKind::Between(e, lo, hi) => {
                let (re, rlo, rhi) = (self.expr(e, true)?, self.expr(lo, true)?, self.expr(hi, true)?);
                self.unify(&re, &rlo, c.pos)?;
                self.unify(&re, &rhi, c.pos)?;
                self.unify(&rlo, &rhi, c.pos)?;
                RCond::Between(re, rlo, rhi)
            }
            CondKind::In(e, list) => {
                let re = self.expr(e, true)?;
                let mut items = Vec::new();
                for v in list {
                    let rv = self.expr(v, true)?;
                    self.unify(&re, &rv, c.pos)?;
                    items.push(rv);
                }
                RCond::In(re, items)
            }
            CondKind::And(a, b) => RCond::And(Box::new(self.cond(a)?), Box::new(self.cond(b)?)),
            CondKind::Or(a, b) => RCond::Or(Box::new(self.cond(a)?), Box::new(self.cond(b)?)),
            CondKind::Not(a) => RCond::Not(Box::new(self.cond(a)?)),
            CondKind::Missing { path, negated } => {
                // A declared field is never missing; a name that is not a
                // field is refused by the resolution.
                self.target(path)?;
                RCond::Const(*negated)
            }
            CondKind::BeginsWith(a, b) => {
                let (ra, rb) = (self.expr(a, true)?, self.expr(b, true)?);
                self.require(&ra, Ty::Str, a.pos(), "begins_with")?;
                self.require(&rb, Ty::Str, b.pos(), "begins_with")?;
                RCond::BeginsWith(ra, rb)
            }
            CondKind::Contains(a, b) => {
                if let Expr::Path(p) = a {
                    if let Target::Table(f) = self.target(p)? {
                        let rb = self.expr(b, true)?;
                        let want = if self.schema.fields[f].class.is_number() { Ty::Num } else { Ty::Str };
                        self.require(&rb, want, b.pos(), "contains")?;
                        return Ok(RCond::ContainsOccurs(f, rb));
                    }
                }
                let (ra, rb) = (self.expr(a, true)?, self.expr(b, true)?);
                self.require(&ra, Ty::Str, a.pos(), "contains")?;
                self.require(&rb, Ty::Str, b.pos(), "contains")?;
                RCond::ContainsText(ra, rb)
            }
            CondKind::AttributeType(p, code) => {
                let class_is_num = match self.target(p)? {
                    Target::Field(r) => Some(self.class_of(&r).is_number()),
                    Target::Table(f) => Some(self.schema.fields[f].class.is_number()),
                    Target::Group(_) => None,
                };
                RCond::Const(match (code.to_ascii_uppercase().as_str(), class_is_num) {
                    ("S", Some(false)) | ("N", Some(true)) => true,
                    _ => false,
                })
            }
        })
    }

    /// The leaf fields that carry the RECORD KEY.
    fn key_fields(&self, key: &Key, who: &str, pos: Pos) -> Result<Vec<usize>, PartiqlError> {
        let mut out = Vec::new();
        for part in &key.parts {
            if let Some(f) = part.field {
                out.push(f);
                continue;
            }
            // A part spanning a group: the leaves inside it, left to right,
            // skipping any that overlap one already taken (a REDEFINES).
            let mut inside: Vec<usize> = (0..self.schema.fields.len())
                .filter(|&i| {
                    let o = &self.schema.fields[i].occ[0];
                    o.offset >= part.offset && o.offset + o.len <= part.offset + part.len
                })
                .collect();
            inside.sort_by_key(|&i| self.schema.fields[i].occ[0].offset);
            let mut end = part.offset;
            let mut covered = false;
            for i in inside {
                let o = &self.schema.fields[i].occ[0];
                if o.offset >= end {
                    out.push(i);
                    end = o.offset + o.len;
                    covered = true;
                }
            }
            if !covered || end != part.offset + part.len {
                return Err(err(
                    format!("{who}: the RECORD KEY part {} is not made of whole fields", part.field_name),
                    pos,
                ));
            }
        }
        Ok(out)
    }

    /// Split a `WHERE` into one `=` per RECORD KEY field and the rest.
    fn split_pins(&mut self, c: &Cond, who: &str) -> Result<(Vec<(usize, RExpr)>, Option<RCond>), PartiqlError> {
        let needed = self.key_fields(&self.schema.primary.clone(), who, c.pos)?;
        let mut conj: Vec<&Cond> = Vec::new();
        flatten_and(c, &mut conj);
        let mut pins: Vec<(usize, RExpr)> = Vec::new();
        let mut rest: Vec<RCond> = Vec::new();
        for cj in conj {
            if let CondKind::Cmp(l, CmpOp::Eq, r) = &cj.kind {
                let pin = match (l, r) {
                    (Expr::Path(p), v) | (v, Expr::Path(p)) => self.pin_of(p, v, &needed, &pins)?,
                    _ => None,
                };
                if let Some((f, rv, rl)) = pin {
                    self.unify(&rl, &rv, cj.pos)?;
                    pins.push((f, rv));
                    continue;
                }
            }
            rest.push(self.cond(cj)?);
        }
        for f in &needed {
            if !pins.iter().any(|(p, _)| p == f) {
                return Err(err(
                    format!(
                        "{who} must name the whole RECORD KEY with =; {} is not pinned",
                        self.schema.fields[*f].name
                    ),
                    c.pos,
                ));
            }
        }
        let further = rest.into_iter().reduce(|a, b| RCond::And(Box::new(a), Box::new(b)));
        Ok((pins, further))
    }

    /// `path = value` as a pin of a key field, when it is one.
    fn pin_of(
        &mut self,
        p: &Path,
        v: &Expr,
        needed: &[usize],
        already: &[(usize, RExpr)],
    ) -> Result<Option<(usize, RExpr, RExpr)>, PartiqlError> {
        let Target::Field(r) = self.target(p)? else { return Ok(None) };
        if r.occ != 0 || !needed.contains(&r.field) || already.iter().any(|(f, _)| *f == r.field) {
            return Ok(None);
        }
        let rv = self.expr(v, false);
        let Ok(rv) = rv else { return Ok(None) };
        Ok(Some((r.field, rv, RExpr::Field(r))))
    }

    // ── Statements ──

    fn select(&mut self, s: &Select) -> Result<RSelect, PartiqlError> {
        let kor = self.table(&s.table)?;
        let mut columns: Vec<Column> = Vec::new();
        match &s.projection {
            Projection::Star => {
                for (i, f) in self.schema.fields.iter().enumerate() {
                    if f.in_occurs_group {
                        return Err(err(
                            format!("{} sits under a group with OCCURS and is not addressable; name the fields you want", f.name),
                            s.table.pos,
                        ));
                    }
                    let unique = self.schema.fields.iter().filter(|g| g.name == f.name).count() == 1;
                    for (k, _) in f.occ.iter().enumerate() {
                        let base = if unique { f.name.clone() } else { f.path() };
                        let (name, prop) = if f.is_table() {
                            (format!("{base}[{}]", k + 1), None)
                        } else {
                            (base, unique.then(|| f.name.clone()))
                        };
                        columns.push(Column {
                            name,
                            expr: RExpr::Field(FieldRef { field: i, occ: k }),
                            ty: Some(self.ty_of_field(&FieldRef { field: i, occ: k })),
                            prop,
                            pad: pad_of(f),
                        });
                    }
                }
            }
            Projection::Items(items) => {
                for it in items {
                    let expr = self.expr(&it.expr, true)?;
                    let (name, prop) = match (&it.expr, &expr) {
                        (Expr::Path(p), RExpr::Field(r)) => {
                            let f = &self.schema.fields[r.field];
                            let unique = self.schema.fields.iter().filter(|g| g.name == f.name).count() == 1;
                            let names = p.segs.iter().filter(|s| matches!(s, Seg::Name(_))).count();
                            let written_with_group = names > 1;
                            let base = if written_with_group || !unique { f.path() } else { f.name.clone() };
                            if f.is_table() {
                                (format!("{base}[{}]", r.occ + 1), None)
                            } else {
                                (base, (unique && !written_with_group).then(|| f.name.clone()))
                            }
                        }
                        _ => (it.text.clone(), None),
                    };
                    let pad = match &expr {
                        RExpr::Field(r) => pad_of(&self.schema.fields[r.field]),
                        _ => 0,
                    };
                    columns.push(Column { ty: self.ty(&expr), expr, name, prop, pad });
                }
            }
        }
        let filter = match &s.filter {
            Some(w) => Some(self.cond(&w.cond)?),
            None => None,
        };
        let mut desc = false;
        if let Some(ob) = &s.order_by {
            let r = self.field_ref(&ob.path)?;
            let key = self.schema.key(kor);
            if !key.parts.iter().any(|p| p.field == Some(r.field)) {
                let names: Vec<&str> = key.parts.iter().map(|p| p.field_name.as_str()).collect();
                return Err(err(
                    format!(
                        "ORDER BY {}: only a field of the key of reference ({}) may order the result",
                        ob.path.text(),
                        names.join(", ")
                    ),
                    ob.path.pos,
                ));
            }
            desc = ob.desc;
        }
        Ok(RSelect { columns, filter, kor, desc })
    }

    fn insert(&mut self, i: &Insert) -> Result<RInsert, PartiqlError> {
        self.write_table(&i.table)?;
        let mut sets: Vec<(FieldRef, RExpr)> = Vec::new();
        for (name, pos, value) in &i.fields {
            let path = Path { segs: path_segs(name), pos: *pos };
            let r = self.field_ref(&path)?;
            if sets.iter().any(|(s, _)| *s == r) {
                return Err(err(format!("{name} is named twice"), *pos));
            }
            let rv = self.expr(value, false)?;
            let want = self.ty_of_field(&r);
            self.require(&rv, want, *pos, &format!("the field {name}"))?;
            sets.push((r, rv));
        }
        // Every part of the RECORD KEY — and of an alternate that allows no
        // duplicates — must be given; the rest take their initial values.
        let mut covered: Vec<(usize, usize)> = Vec::new();
        for (r, _) in &sets {
            let o = &self.schema.fields[r.field].occ[r.occ];
            covered.push((o.offset, o.offset + o.len));
        }
        let mut required: Vec<(&Key, &str)> = vec![(&self.schema.primary, "RECORD KEY")];
        for k in &self.schema.alternates {
            if !k.duplicates {
                required.push((k, "alternate key"));
            }
        }
        for (key, what) in required {
            for part in &key.parts {
                let (s, e) = (part.offset, part.offset + part.len);
                let all = (s..e).all(|b| covered.iter().any(|(cs, ce)| *cs <= b && b < *ce));
                if !all {
                    let which = key.name.as_deref().map(|n| format!(" {n}")).unwrap_or_default();
                    return Err(err(
                        format!("INSERT must give {} of the {what}{which}", part.field_name),
                        i.table.pos,
                    ));
                }
            }
        }
        Ok(RInsert { sets })
    }

    /// Whether a field's bytes overlap the RECORD KEY.
    fn overlaps_record_key(&self, r: &FieldRef) -> bool {
        let o = &self.schema.fields[r.field].occ[r.occ];
        self.schema
            .primary
            .parts
            .iter()
            .any(|p| o.offset < p.offset + p.len && p.offset < o.offset + o.len)
    }

    fn update(&mut self, u: &Update) -> Result<RUpdate, PartiqlError> {
        self.write_table(&u.table)?;
        let mut clauses = Vec::new();
        for c in &u.clauses {
            match c {
                Clause::Set(path, value) => {
                    let r = self.field_ref(path)?;
                    if self.overlaps_record_key(&r) {
                        return Err(err(
                            format!("SET {}: a field of the RECORD KEY cannot be changed", path.text()),
                            path.pos,
                        ));
                    }
                    let rv = self.expr(value, true)?;
                    let want = self.ty_of_field(&r);
                    self.require(&rv, want, path.pos, &format!("SET {}", path.text()))?;
                    clauses.push(RClause::Set(r, rv));
                }
                Clause::Remove(path) => {
                    let refs: Vec<FieldRef> = match self.target(path)? {
                        Target::Field(r) => vec![r],
                        Target::Table(f) => (0..self.schema.fields[f].occ.len()).map(|occ| FieldRef { field: f, occ }).collect(),
                        Target::Group(_) => {
                            return Err(err(format!("REMOVE {}: a group is not a field", path.text()), path.pos))
                        }
                    };
                    if refs.iter().any(|r| self.overlaps_record_key(r)) {
                        return Err(err(
                            format!("REMOVE {}: a field of the RECORD KEY cannot be removed", path.text()),
                            path.pos,
                        ));
                    }
                    clauses.push(RClause::Remove(refs));
                }
            }
        }
        let (pins, further) = self.split_pins(&u.filter.cond, "UPDATE")?;
        Ok(RUpdate { pins, further, where_text: u.filter.text.clone(), clauses, returning: u.returning })
    }

    fn exists(&mut self, sel: &Select, pos: Pos) -> Result<RExists, PartiqlError> {
        if !self.in_tx {
            return Err(err("EXISTS is allowed inside a transaction only (ExecuteTransaction)", pos));
        }
        if self.table(&sel.table)? != 0 {
            return Err(err("EXISTS names the table and its RECORD KEY, not an index", sel.table.pos));
        }
        let Some(w) = &sel.filter else {
            return Err(err("EXISTS needs a WHERE that names the whole RECORD KEY", pos));
        };
        let (pins, further) = self.split_pins(&w.cond, "EXISTS")?;
        Ok(RExists { pins, further, where_text: w.text.clone() })
    }

    fn delete(&mut self, d: &Delete) -> Result<RDelete, PartiqlError> {
        self.write_table(&d.table)?;
        let (pins, further) = self.split_pins(&d.filter.cond, "DELETE")?;
        Ok(RDelete { pins, further, where_text: d.filter.text.clone(), returning: d.returning })
    }
}

fn pad_of(f: &super::schema::Field) -> usize {
    match f.class {
        Class::Number { decimals: 0, signed: false } => f.occ[0].len,
        _ => 0,
    }
}

fn flatten_and<'c>(c: &'c Cond, out: &mut Vec<&'c Cond>) {
    match &c.kind {
        CondKind::And(a, b) => {
            flatten_and(a, out);
            flatten_and(b, out);
        }
        _ => out.push(c),
    }
}

/// The path an `INSERT` field name stands for: a dotted name is a path, a
/// subscripted one an occurrence.
fn path_segs(name: &str) -> Vec<Seg> {
    let mut segs = Vec::new();
    for (i, part) in name.split('.').enumerate() {
        let _ = i;
        if let Some((n, rest)) = part.split_once('[') {
            segs.push(Seg::Name(n.to_string()));
            if let Some(idx) = rest.strip_suffix(']').and_then(|s| s.parse::<u32>().ok()) {
                segs.push(Seg::Index(idx));
            }
        } else {
            segs.push(Seg::Name(part.to_string()));
        }
    }
    segs
}
