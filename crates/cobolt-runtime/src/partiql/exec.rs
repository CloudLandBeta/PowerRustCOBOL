// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The executor (spec 108, R1, R11–R17, R19, R23–R27): runs an analysed
//! statement on any [`IndexedStore`].
//!
//! It reads through `read_key`, `start` + `read_seq` and writes through
//! `write` / `rewrite` / `delete`, so it behaves the same on every engine. A
//! transaction unit is undone by **compensation** (D3): each write keeps its
//! before-image, and a failure replays the inverses in reverse — exactly the
//! unit's own changes, nothing the program wrote before it.

use std::cmp::Ordering;
use std::sync::Arc;

use super::analyze::{
    Analyzed, Column, FieldRef, RClause, RCond, RDelete, RExists, RExpr, RInsert, RSelect, RStmt, RUpdate, Ty,
};
use super::parser::{CmpOp, Returning};
use super::plan::{key_bytes, plan, Access, Plan, ReadClass};
use super::schema::{Key, Schema};
use super::value::Value;
use super::PartiqlError;
use crate::files::{decode_field, encode_field, EncodeError};
use crate::indexed::{status, IndexedStore, ReadDir, StartOp};
use crate::value::parse_decimal;

/// One column of an item: its name and, when the value is a plain field, the
/// COBOL name the control publishes it under as a property.
#[derive(Clone, Debug, PartialEq)]
pub struct ColInfo {
    pub name: String,
    pub prop: Option<String>,
    /// Zero-fill width of an unsigned whole-number field (see `Column::pad`).
    pub pad: usize,
}

impl Item {
    /// The text of value `i` as a result set shows it: a Number with its
    /// decimal point; an unsigned whole-number field as the item shows it,
    /// zero-filled to its PICTURE; a String as it is.
    pub fn text(&self, i: usize) -> String {
        match (self.values.get(i), self.cols.get(i)) {
            (Some(Value::Num(n)), Some(c)) if c.pad > 0 && n.decimals == 0 && n.mantissa >= 0 => {
                format!("{:0>w$}", n.mantissa, w = c.pad)
            }
            (Some(v), _) => v.display(),
            _ => String::new(),
        }
    }
}

/// One item of a result set.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub cols: Arc<Vec<ColInfo>>,
    pub values: Vec<Value>,
    /// The whole record the item was read from — what `CurrentRecordDataItem`
    /// receives (R21), whatever the projection kept.
    pub rec: Vec<u8>,
}

/// An argument after the statement: typed by its source (a data item by its
/// PICTURE, a literal by its form) or untyped text from a control property
/// (O2), coerced to the type of what it meets.
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Value(Value),
    Text(String),
}

/// What a statement or a unit did.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// The FILE STATUS: `00` when it ran.
    pub status: &'static str,
    pub error: Option<PartiqlError>,
    /// Items in the result set (`SELECT`, `RETURNING`), else records affected;
    /// statements that ran for a unit; 0 on failure.
    pub count: usize,
    pub class: Option<ReadClass>,
    pub items: Vec<Item>,
    /// The projection's columns, kept when no item matched (`ColumnNames`).
    pub columns: Arc<Vec<ColInfo>>,
}

impl Outcome {
    fn ok(count: usize, class: Option<ReadClass>, items: Vec<Item>, columns: Arc<Vec<ColInfo>>) -> Outcome {
        Outcome { status: status::OK, error: None, count, class, items, columns }
    }

    /// A statement that did not run: its status, its error, nothing else.
    pub fn failed(e: PartiqlError) -> Outcome {
        Outcome { status: e.status, error: Some(e), count: 0, class: None, items: Vec::new(), columns: Arc::new(Vec::new()) }
    }

    /// The text `LastError` holds; empty on success.
    pub fn last_error(&self) -> String {
        self.error.as_ref().map(PartiqlError::text).unwrap_or_default()
    }
}

/// The text of an engine status.
pub fn status_text(code: &str) -> &'static str {
    match code {
        "10" => "end of the file",
        "21" => "key out of sequence",
        "22" => "duplicate key",
        "23" => "record not found",
        "35" => "file not found",
        "37" => "the file does not allow this open mode",
        "39" => "the file does not match the declared layout",
        "46" => "no current record",
        "47" => "the file is not open for input",
        "48" => "the file is not open for output; open it I-O to write",
        "49" => "the file is not open for update; open it I-O to change records",
        "90" => "input/output error",
        "92" => "logic error",
        "93" => "the file is in use by another run",
        "94" => "the statement is not valid",
        "99" => "record locked",
        _ => "file error",
    }
}

fn file_error(code: &'static str) -> PartiqlError {
    PartiqlError { status: code, exception: "", message: status_text(code).to_string(), pos: None }
}

fn validation(msg: impl Into<String>) -> PartiqlError {
    PartiqlError::validation(msg, None)
}

fn exception(code: &'static str, name: &'static str, message: impl Into<String>) -> PartiqlError {
    PartiqlError { status: code, exception: name, message: message.into(), pos: None }
}

/// Bind the arguments to the `?` of the statements of one run, in order
/// (R3). Never splices a value into the text; the messages name the argument
/// by its number only (R29).
pub fn bind(analyzed: &[&Analyzed], args: &[Arg]) -> Result<Vec<Value>, PartiqlError> {
    let total: usize = analyzed.iter().map(|a| a.param_count).sum();
    if args.len() != total {
        return Err(validation(format!(
            "the statement has {total} ? parameter(s) but {} argument(s) were given",
            args.len()
        )));
    }
    let mut expect: Vec<Option<Ty>> = vec![None; total];
    for a in analyzed {
        for (i, e) in a.expect.iter().enumerate() {
            if let Some(slot) = expect.get_mut(a.first_param + i) {
                *slot = *e;
            }
        }
    }
    args.iter()
        .zip(expect)
        .enumerate()
        .map(|(n, (arg, want))| {
            let n = n + 1;
            match (arg, want) {
                (Arg::Value(v), Some(t)) => {
                    let have = if v.is_num() { Ty::Num } else { Ty::Str };
                    if have != t {
                        Err(validation(format!(
                            "argument {n} is a {} but the statement compares it with a {}",
                            have.name(),
                            t.name()
                        )))
                    } else {
                        Ok(v.clone())
                    }
                }
                (Arg::Value(Value::Str(s)), None) => Ok(Value::text(s)),
                (Arg::Value(v), None) => Ok(v.clone()),
                (Arg::Text(s), Some(Ty::Num)) => parse_decimal(s.trim())
                    .map(Value::Num)
                    .ok_or_else(|| validation(format!("argument {n} is not a number, but the statement compares it with a Number"))),
                (Arg::Text(s), _) => Ok(Value::text(s)),
            }
        })
        .collect()
}

#[derive(Clone, Copy)]
struct Ctx<'a> {
    schema: &'a Schema,
    params: &'a [Value],
}

fn mismatch() -> PartiqlError {
    validation("a Number cannot be compared with a String, nor added to one")
}

impl Ctx<'_> {
    fn eval(&self, e: &RExpr, rec: &[u8]) -> Result<Value, PartiqlError> {
        match e {
            RExpr::Lit(v) => Ok(v.clone()),
            RExpr::Param(n) => Ok(self.params.get(*n).cloned().unwrap_or(Value::Str(String::new()))),
            RExpr::Field(r) => Ok(decode_field(&self.schema.fields[r.field].occ[r.occ], rec)),
            RExpr::SizeText(r) => match decode_field(&self.schema.fields[r.field].occ[r.occ], rec) {
                Value::Str(s) => Ok(Value::int(s.chars().count() as i64)),
                other => Ok(other),
            },
            RExpr::Add(a, b) => self.eval(a, rec)?.add(&self.eval(b, rec)?).map_err(|_| mismatch()),
            RExpr::Sub(a, b) => self.eval(a, rec)?.sub(&self.eval(b, rec)?).map_err(|_| mismatch()),
        }
    }

    fn cond(&self, c: &RCond, rec: &[u8]) -> Result<bool, PartiqlError> {
        Ok(match c {
            RCond::Cmp(l, op, r) => {
                let ord = self.eval(l, rec)?.compare(&self.eval(r, rec)?).map_err(|_| mismatch())?;
                match op {
                    CmpOp::Eq => ord == Ordering::Equal,
                    CmpOp::Ne => ord != Ordering::Equal,
                    CmpOp::Lt => ord == Ordering::Less,
                    CmpOp::Le => ord != Ordering::Greater,
                    CmpOp::Gt => ord == Ordering::Greater,
                    CmpOp::Ge => ord != Ordering::Less,
                }
            }
            RCond::Between(e, lo, hi) => {
                let v = self.eval(e, rec)?;
                v.compare(&self.eval(lo, rec)?).map_err(|_| mismatch())? != Ordering::Less
                    && v.compare(&self.eval(hi, rec)?).map_err(|_| mismatch())? != Ordering::Greater
            }
            RCond::In(e, list) => {
                let v = self.eval(e, rec)?;
                for x in list {
                    if v.compare(&self.eval(x, rec)?).map_err(|_| mismatch())? == Ordering::Equal {
                        return Ok(true);
                    }
                }
                false
            }
            RCond::And(a, b) => self.cond(a, rec)? && self.cond(b, rec)?,
            RCond::Or(a, b) => self.cond(a, rec)? || self.cond(b, rec)?,
            RCond::Not(a) => !self.cond(a, rec)?,
            RCond::Const(b) => *b,
            RCond::BeginsWith(a, b) => match (self.eval(a, rec)?, self.eval(b, rec)?) {
                (Value::Str(a), Value::Str(b)) => a.starts_with(&b),
                _ => return Err(mismatch()),
            },
            RCond::ContainsText(a, b) => match (self.eval(a, rec)?, self.eval(b, rec)?) {
                (Value::Str(a), Value::Str(b)) => a.contains(&b),
                _ => return Err(mismatch()),
            },
            RCond::ContainsOccurs(f, v) => {
                let want = self.eval(v, rec)?;
                let mut hit = false;
                for o in &self.schema.fields[*f].occ {
                    if decode_field(o, rec).compare(&want).map_err(|_| mismatch())? == Ordering::Equal {
                        hit = true;
                        break;
                    }
                }
                hit
            }
        })
    }

    fn passes(&self, filter: &Option<RCond>, rec: &[u8]) -> Result<bool, PartiqlError> {
        match filter {
            Some(c) => self.cond(c, rec),
            None => Ok(true),
        }
    }

    /// The full RECORD KEY bytes the pins name, or `None` when a value cannot
    /// be written into its field exactly (so no record can have it).
    fn pin_key(&self, pins: &[(usize, RExpr)]) -> Result<Option<Vec<u8>>, PartiqlError> {
        let mut buf = vec![b' '; self.schema.record_len];
        for (f, e) in pins {
            let v = self.eval(e, &[])?;
            let pos = &self.schema.fields[*f].occ[0];
            let mut one = buf.clone();
            match encode_field(pos, &v, &mut one) {
                Ok(()) => {}
                Err(EncodeError::Overflow) => return Ok(None),
                Err(EncodeError::TypeMismatch) => return Err(mismatch()),
            }
            // Exact only: what was written must read back as the value given.
            let back = decode_field(pos, &one);
            let same = match (&back, &v) {
                (Value::Str(a), Value::Str(b)) => a == b.trim_end_matches(' '),
                (a, b) => a.compare(b).map(|o| o == Ordering::Equal).unwrap_or(false),
            };
            if !same {
                return Ok(None);
            }
            buf = one;
        }
        Ok(Some(key_bytes(&self.schema.primary, &buf)))
    }
}

fn column_info(c: &Column) -> ColInfo {
    ColInfo { name: c.name.clone(), prop: c.prop.clone(), pad: c.pad }
}

/// The columns `*` stands for: every field, an `OCCURS` field once per occurrence.
fn field_columns(schema: &Schema, refs: Option<&[FieldRef]>) -> (Arc<Vec<ColInfo>>, Vec<FieldRef>) {
    let all: Vec<FieldRef>;
    let list: &[FieldRef] = match refs {
        Some(r) => r,
        None => {
            all = schema
                .fields
                .iter()
                .enumerate()
                .filter(|(_, f)| !f.in_occurs_group)
                .flat_map(|(i, f)| (0..f.occ.len()).map(move |k| FieldRef { field: i, occ: k }))
                .collect();
            &all
        }
    };
    let cols = list
        .iter()
        .map(|r| {
            let f = &schema.fields[r.field];
            let unique = schema.fields.iter().filter(|g| g.name == f.name).count() == 1;
            let base = if unique { f.name.clone() } else { f.path() };
            if f.is_table() {
                ColInfo { name: format!("{base}[{}]", r.occ + 1), prop: None, pad: super_pad(f) }
            } else {
                ColInfo { name: base, prop: unique.then(|| f.name.clone()), pad: super_pad(f) }
            }
        })
        .collect();
    (Arc::new(cols), list.to_vec())
}

fn super_pad(f: &super::schema::Field) -> usize {
    match f.class {
        super::schema::Class::Number { decimals: 0, signed: false } => f.occ[0].len,
        _ => 0,
    }
}

fn row_of(cx: Ctx, refs: &[FieldRef], rec: &[u8]) -> Vec<Value> {
    refs.iter().map(|r| decode_field(&cx.schema.fields[r.field].occ[r.occ], rec)).collect()
}

/// Read the records a plan selects, in key order, giving each to `visit`.
fn read_window(
    store: &mut dyn IndexedStore,
    key: &Key,
    kor: usize,
    plan: &Plan,
    mut visit: impl FnMut(&[u8]) -> Result<(), PartiqlError>,
) -> Result<(), PartiqlError> {
    let ok = |s: &str| s == status::OK || s == status::DUP_ALT_OK;
    store.set_key_of_reference(kor);
    let total: usize = key.parts.iter().map(|p| p.len).sum();
    let from = |store: &mut dyn IndexedStore,
                op: StartOp,
                at: &[u8],
                stop: &dyn Fn(&[u8]) -> bool,
                visit: &mut dyn FnMut(&[u8]) -> Result<(), PartiqlError>|
     -> Result<(), PartiqlError> {
        match store.start(op, at) {
            s if ok(s) => {}
            status::NOT_FOUND => return Ok(()),
            s => return Err(file_error(s)),
        }
        loop {
            let (rec, st) = store.read_seq(ReadDir::Next);
            match (rec, st) {
                (Some(r), s) if ok(s) => {
                    if stop(&key_bytes(key, &r)) {
                        return Ok(());
                    }
                    visit(&r)?;
                }
                (_, status::EOF) | (None, status::NOT_FOUND) => return Ok(()),
                (_, s) => return Err(file_error(s)),
            }
        }
    };
    match &plan.access {
        Access::Keyed(keys) => {
            for k in keys {
                if key.duplicates {
                    let k2 = k.clone();
                    from(store, StartOp::Eq, k, &move |kb| kb != k2.as_slice(), &mut visit)?;
                } else {
                    let (rec, st) = store.read_key(k);
                    match (rec, st) {
                        (Some(r), s) if ok(s) => visit(&r)?,
                        (_, status::NOT_FOUND) => {}
                        (_, s) => return Err(file_error(s)),
                    }
                }
            }
        }
        Access::Range { lo, hi } => {
            let hi = hi.clone();
            from(store, StartOp::Ge, lo, &move |kb| kb > hi.as_slice(), &mut visit)?;
        }
        Access::Scan => {
            from(store, StartOp::Ge, &vec![0u8; total], &|_| false, &mut visit)?;
        }
    }
    Ok(())
}

/// What one statement did, and how to take it back.
struct Done {
    count: usize,
    class: Option<ReadClass>,
    items: Vec<Item>,
    columns: Arc<Vec<ColInfo>>,
    undo: Option<Undo>,
}

fn no_columns() -> Arc<Vec<ColInfo>> {
    Arc::new(Vec::new())
}

enum Undo {
    /// Delete the record that was inserted.
    Delete(Vec<u8>),
    /// Rewrite the record as it was.
    Rewrite(Vec<u8>, Vec<u8>),
    /// Write the record that was deleted.
    Write(Vec<u8>),
}

fn run_select(store: &mut dyn IndexedStore, cx: Ctx, sel: &RSelect) -> Result<Done, PartiqlError> {
    let key = cx.schema.key(sel.kor);
    let p = plan(sel, cx.schema, &|n| cx.params.get(n).cloned());
    let cols: Arc<Vec<ColInfo>> = Arc::new(sel.columns.iter().map(column_info).collect());
    let mut items: Vec<Item> = Vec::new();
    read_window(store, key, sel.kor, &p, |rec| {
        if cx.passes(&sel.filter, rec)? {
            let mut values = Vec::with_capacity(sel.columns.len());
            for c in &sel.columns {
                values.push(cx.eval(&c.expr, rec)?);
            }
            items.push(Item { cols: cols.clone(), values, rec: rec.to_vec() });
        }
        Ok(())
    })?;
    if sel.desc {
        items.reverse();
    }
    Ok(Done { count: items.len(), class: Some(p.class), items, columns: cols, undo: None })
}

/// A record with every field at its initial value: spaces, zero.
fn initial_record(schema: &Schema) -> Vec<u8> {
    let mut rec = vec![b' '; schema.record_len];
    for f in &schema.fields {
        for o in &f.occ {
            let v = if f.class.is_number() { Value::int(0) } else { Value::Str(String::new()) };
            let _ = encode_field(o, &v, &mut rec);
        }
    }
    rec
}

fn store_value(cx: Ctx, r: &FieldRef, v: &Value, rec: &mut [u8]) -> Result<(), PartiqlError> {
    let f = &cx.schema.fields[r.field];
    encode_field(&f.occ[r.occ], v, rec).map_err(|e| match e {
        EncodeError::Overflow => validation(format!("the value does not fit the integer digits of {}", f.name)),
        EncodeError::TypeMismatch => mismatch(),
    })
}

fn run_insert(store: &mut dyn IndexedStore, cx: Ctx, ins: &RInsert) -> Result<Done, PartiqlError> {
    let mut rec = initial_record(cx.schema);
    for (r, e) in &ins.sets {
        let v = cx.eval(e, &[])?;
        store_value(cx, r, &v, &mut rec)?;
    }
    let key = key_bytes(&cx.schema.primary, &rec);
    store.set_key_of_reference(0);
    match store.write(&rec) {
        s if s == status::OK || s == status::DUP_ALT_OK => {}
        status::DUP_KEY => return Err(exception(status::DUP_KEY, "DuplicateItemException", "a record with this key exists")),
        s => return Err(file_error(s)),
    }
    Ok(Done { count: 1, class: None, items: Vec::new(), columns: no_columns(), undo: Some(Undo::Delete(key)) })
}

/// Read the record a statement's key names; `None` when there is none.
fn fetch(store: &mut dyn IndexedStore, key: &Option<Vec<u8>>) -> Result<Option<Vec<u8>>, PartiqlError> {
    let Some(k) = key else { return Ok(None) };
    store.set_key_of_reference(0);
    match store.read_key(k) {
        (Some(r), s) if s == status::OK || s == status::DUP_ALT_OK => Ok(Some(r)),
        (_, status::NOT_FOUND) => Ok(None),
        (_, s) => Err(file_error(s)),
    }
}

fn condition_failed(text: &str) -> PartiqlError {
    exception(status::NOT_FOUND, "ConditionalCheckFailedException", text.to_string())
}

fn run_update(store: &mut dyn IndexedStore, cx: Ctx, up: &RUpdate) -> Result<Done, PartiqlError> {
    let key = cx.pin_key(&up.pins)?;
    let Some(old) = fetch(store, &key)? else {
        return Err(condition_failed(&format!("no record has the key named in WHERE {}", up.where_text)));
    };
    if !cx.passes(&up.further, &old)? {
        return Err(condition_failed(&up.where_text));
    }
    let mut new = old.clone();
    let mut modified: Vec<FieldRef> = Vec::new();
    let mut note = |r: &FieldRef| {
        if !modified.contains(r) {
            modified.push(r.clone());
        }
    };
    for c in &up.clauses {
        match c {
            RClause::Set(r, e) => {
                // A right-hand side reads the record as it was.
                let v = cx.eval(e, &old)?;
                store_value(cx, r, &v, &mut new)?;
                note(r);
            }
            RClause::Remove(refs) => {
                for r in refs {
                    let f = &cx.schema.fields[r.field];
                    let v = if f.class.is_number() { Value::int(0) } else { Value::Str(String::new()) };
                    store_value(cx, r, &v, &mut new)?;
                    note(r);
                }
            }
        }
    }
    let key_bytes_v = key.clone().unwrap_or_default();
    match store.rewrite(&new, Some(&key_bytes_v)) {
        s if s == status::OK || s == status::DUP_ALT_OK => {}
        status::DUP_KEY => {
            return Err(exception(status::DUP_KEY, "DuplicateItemException", "the change duplicates an alternate key"))
        }
        s => return Err(file_error(s)),
    }
    let mut items = Vec::new();
    if let Some(r) = up.returning {
        let (rec, refs): (&[u8], Option<&[FieldRef]>) = match r {
            Returning::AllOld => (&old, None),
            Returning::AllNew => (&new, None),
            Returning::ModifiedOld => (&old, Some(&modified)),
            Returning::ModifiedNew => (&new, Some(&modified)),
        };
        let (cols, list) = field_columns(cx.schema, refs);
        items.push(Item { cols, values: row_of(cx, &list, rec), rec: rec.to_vec() });
    }
    Ok(Done { count: 1, class: None, items, columns: no_columns(), undo: Some(Undo::Rewrite(key_bytes_v, old)) })
}

fn run_delete(store: &mut dyn IndexedStore, cx: Ctx, del: &RDelete) -> Result<Done, PartiqlError> {
    let key = cx.pin_key(&del.pins)?;
    let Some(old) = fetch(store, &key)? else {
        // DynamoDB's rule: deleting a key no record has succeeds, affecting none.
        return Ok(Done { count: 0, class: None, items: Vec::new(), columns: no_columns(), undo: None });
    };
    if !cx.passes(&del.further, &old)? {
        return Err(condition_failed(&del.where_text));
    }
    let key_bytes_v = key.unwrap_or_default();
    match store.delete(Some(&key_bytes_v)) {
        s if s == status::OK => {}
        s => return Err(file_error(s)),
    }
    let mut items = Vec::new();
    if del.returning {
        let (cols, list) = field_columns(cx.schema, None);
        items.push(Item { cols, values: row_of(cx, &list, &old), rec: old.clone() });
    }
    Ok(Done { count: 1, class: None, items, columns: no_columns(), undo: Some(Undo::Write(old)) })
}

fn run_exists(store: &mut dyn IndexedStore, cx: Ctx, ex: &RExists) -> Result<Done, PartiqlError> {
    let key = cx.pin_key(&ex.pins)?;
    match fetch(store, &key)? {
        Some(rec) if cx.passes(&ex.further, &rec)? => {
            Ok(Done { count: 1, class: None, items: Vec::new(), columns: no_columns(), undo: None })
        }
        _ => Err(condition_failed(&ex.where_text)),
    }
}

fn run(store: &mut dyn IndexedStore, cx: Ctx, a: &Analyzed) -> Result<Done, PartiqlError> {
    let r = match &a.stmt {
        RStmt::Select(s) => run_select(store, cx, s),
        RStmt::Insert(i) => run_insert(store, cx, i),
        RStmt::Update(u) => run_update(store, cx, u),
        RStmt::Delete(d) => run_delete(store, cx, d),
        RStmt::Exists(e) => run_exists(store, cx, e),
    };
    store.unlock();
    r
}

/// Run one statement (the arguments already bound by [`bind`]).
pub fn execute(store: &mut dyn IndexedStore, schema: &Schema, a: &Analyzed, params: &[Value]) -> Outcome {
    match run(store, Ctx { schema, params }, a) {
        Ok(d) => Outcome::ok(d.count, d.class, d.items, d.columns),
        Err(e) => Outcome::failed(e),
    }
}

fn apply_undo(store: &mut dyn IndexedStore, u: Undo) -> &'static str {
    store.set_key_of_reference(0);
    let st = match u {
        Undo::Delete(key) => store.delete(Some(&key)),
        Undo::Rewrite(key, old) => store.rewrite(&old, Some(&key)),
        Undo::Write(old) => store.write(&old),
    };
    store.unlock();
    if st == status::OK || st == status::DUP_ALT_OK {
        status::OK
    } else {
        st
    }
}

/// Run a transaction unit: all of its statements or none of its changes.
///
/// Before anything runs, no statement may act on a record an earlier one of
/// the unit wrote (R26). On a failure the unit's own writes are taken back in
/// reverse; what the program wrote before the unit stays as it was (R25).
pub fn execute_unit(store: &mut dyn IndexedStore, schema: &Schema, list: &[Analyzed], params: &[Value]) -> Outcome {
    let cx = Ctx { schema, params };
    // R26: the records each write names.
    let mut written: Vec<(usize, Vec<u8>)> = Vec::new();
    for (n, a) in list.iter().enumerate() {
        let key = match &a.stmt {
            RStmt::Update(u) => cx.pin_key(&u.pins),
            RStmt::Delete(d) => cx.pin_key(&d.pins),
            RStmt::Exists(e) => cx.pin_key(&e.pins),
            RStmt::Insert(i) => {
                let mut rec = initial_record(schema);
                let r: Result<(), PartiqlError> = i.sets.iter().try_for_each(|(r, e)| {
                    let v = cx.eval(e, &[])?;
                    store_value(cx, r, &v, &mut rec)
                });
                r.map(|_| Some(key_bytes(&schema.primary, &rec)))
            }
            RStmt::Select(_) => continue,
        };
        let key = match key {
            Ok(Some(k)) => k,
            Ok(None) => continue,
            Err(e) => return Outcome::failed(unit_error(n + 1, &e)),
        };
        if let Some((m, _)) = written.iter().find(|(_, k)| *k == key) {
            return Outcome::failed(PartiqlError::validation(
                format!("statement {} acts on the record statement {} wrote; a unit cannot do that", n + 1, m),
                Some(a.pos),
            ));
        }
        if !matches!(a.stmt, RStmt::Exists(_)) {
            written.push((n + 1, key));
        }
    }
    let mut undo: Vec<Undo> = Vec::new();
    let mut items: Vec<Item> = Vec::new();
    let mut columns = no_columns();
    for (n, a) in list.iter().enumerate() {
        match run(store, cx, a) {
            Ok(d) => {
                if let Some(u) = d.undo {
                    undo.push(u);
                }
                if columns.is_empty() {
                    columns = d.columns;
                }
                items.extend(d.items);
            }
            Err(e) => {
                for u in undo.into_iter().rev() {
                    let st = apply_undo(store, u);
                    if st != status::OK {
                        return Outcome::failed(exception(
                            "90",
                            "TransactionCanceledException",
                            format!(
                                "statement {}: {}; undoing the unit failed with status {st} — treat the file as needing ROLLBACK",
                                n + 1,
                                e.text()
                            ),
                        ));
                    }
                }
                return Outcome::failed(unit_error(n + 1, &e));
            }
        }
    }
    Outcome::ok(list.len(), None, items, columns)
}

fn unit_error(n: usize, e: &PartiqlError) -> PartiqlError {
    PartiqlError {
        status: e.status,
        exception: "TransactionCanceledException",
        message: format!("statement {n}: {}", e.text()),
        pos: None,
    }
}
