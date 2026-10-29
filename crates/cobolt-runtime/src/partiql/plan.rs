// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! How a `SELECT` reads the file (spec 108, §5.3): keyed, range or scan.
//!
//! **Sound by construction** (D5). Keys compare bytewise in every engine, so a
//! condition is used to read by the key only when the byte order of the part
//! *is* the value order and the literal fits the part exactly: an unsigned
//! numeric part (zero-padded digits) or an alphanumeric one. A signed or
//! national part, a negative literal for an unsigned part, a literal with more
//! decimals than the part keeps, a string longer than the part — each stops
//! the planning there, and the rest is a scan with a filter. The whole
//! `WHERE` is evaluated again on every record read, so a read window that is
//! only a superset of the answer is always right.

use super::analyze::{RCond, RExpr, RSelect};
use super::parser::CmpOp;
use super::schema::{Class, Key, KeyPart, Schema};
use super::value::Value;

/// The class of §5.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadClass {
    Keyed,
    Range,
    Scan,
}

impl ReadClass {
    pub fn name(self) -> &'static str {
        match self {
            ReadClass::Keyed => "keyed",
            ReadClass::Range => "range",
            ReadClass::Scan => "scan",
        }
    }
}

/// What the executor does on the file.
#[derive(Clone, Debug, PartialEq)]
pub enum Access {
    /// One read per full key, in key order, duplicates removed.
    Keyed(Vec<Vec<u8>>),
    /// `START` at `lo`, `READ NEXT` while the key is not past `hi` (both as
    /// long as the key; `lo` padded with `0x00`, `hi` with `0xFF`).
    Range { lo: Vec<u8>, hi: Vec<u8> },
    /// `START` at the first record, `READ NEXT` to the end.
    Scan,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub class: ReadClass,
    pub access: Access,
}

impl Plan {
    fn scan() -> Plan {
        Plan { class: ReadClass::Scan, access: Access::Scan }
    }
}

/// Plan the read of `sel`. `param` gives the value bound to a `?`; for Check
/// and Test query, which have no arguments, it returns `None` and the `?` is
/// taken to be a value that fits.
pub fn plan(sel: &RSelect, schema: &Schema, param: &dyn Fn(usize) -> Option<Value>) -> Plan {
    let Some(filter) = &sel.filter else { return Plan::scan() };
    let key = schema.key(sel.kor);
    let mut conj: Vec<&RCond> = Vec::new();
    flatten(filter, &mut conj);

    let mut prefix: Vec<u8> = Vec::new();
    let mut parts_done = 0usize;
    let mut in_values: Option<Vec<Vec<u8>>> = None;
    let mut window: Option<(Vec<u8>, Vec<u8>)> = None;

    for part in &key.parts {
        let Some(usable) = usable(schema, part) else { break };
        let ph = if usable.number.is_some() { Value::int(0) } else { Value::Str(String::new()) };
        let preds = predicates(&conj, usable.field, param, &ph);
        // `=` on this part?
        if let Some(v) = preds.eq {
            match enc_eq(&usable, &v) {
                Some(b) => {
                    prefix.extend(b);
                    parts_done += 1;
                    continue;
                }
                None => break,
            }
        }
        // `IN [..]`: only on the last part, with every earlier part `=`.
        if let Some(list) = preds.in_list {
            if parts_done + 1 == key.parts.len() {
                let enc: Option<Vec<Vec<u8>>> = list.iter().map(|v| enc_eq(&usable, v)).collect();
                if let Some(mut vals) = enc {
                    vals.sort();
                    vals.dedup();
                    in_values = Some(vals);
                    parts_done += 1;
                }
            }
            break;
        }
        // A range on this part.
        let mut lo: Option<Vec<u8>> = None;
        let mut hi: Option<Vec<u8>> = None;
        for (op, v) in &preds.bounds {
            let (l, h) = enc_bound(&usable, *op, v);
            if let Some(l) = l {
                lo = Some(match lo {
                    Some(old) if old > l => old,
                    _ => l,
                });
            }
            if let Some(h) = h {
                hi = Some(match hi {
                    Some(old) if old < h => old,
                    _ => h,
                });
            }
        }
        if let Some((l, h)) = preds.begins {
            if let Some((bl, bh)) = enc_prefix(&usable, &l) {
                lo = Some(match lo {
                    Some(old) if old > bl => old,
                    _ => bl,
                });
                hi = Some(match hi {
                    Some(old) if old < bh => old,
                    _ => bh,
                });
            }
            let _ = h;
        }
        if lo.is_some() || hi.is_some() {
            window = Some((lo.unwrap_or_default(), hi.unwrap_or_default()));
            // The bounds may be shorter than the part (a prefix); they are
            // padded below.
        }
        break;
    }

    let total = key.parts.iter().map(|p| p.len).sum::<usize>();
    if parts_done == key.parts.len() && !key.parts.is_empty() {
        let keys = match in_values {
            Some(vals) => vals.into_iter().map(|v| [prefix.clone(), v].concat()).collect(),
            None => vec![prefix],
        };
        return Plan { class: ReadClass::Keyed, access: Access::Keyed(keys) };
    }
    if !prefix.is_empty() || window.is_some() {
        let (wl, wh) = window.unwrap_or_default();
        let mut lo = prefix.clone();
        lo.extend(wl);
        lo.resize(total, 0x00);
        let mut hi = prefix;
        hi.extend(wh);
        hi.resize(total, 0xFF);
        return Plan { class: ReadClass::Range, access: Access::Range { lo, hi } };
    }
    Plan::scan()
}

/// The class a statement will have, for Check, Test query and the debugger.
pub fn classify(sel: &RSelect, schema: &Schema) -> ReadClass {
    plan(sel, schema, &|_| None).class
}

/// The joined key bytes of a record for key `kor`.
pub fn key_bytes(key: &Key, rec: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for p in &key.parts {
        let end = (p.offset + p.len).min(rec.len());
        let start = p.offset.min(end);
        out.extend_from_slice(&rec[start..end]);
        out.resize(out.len() + (p.offset + p.len - end.max(start)), b' ');
    }
    out
}

// ── Predicates on one key field ──

struct Usable {
    field: usize,
    len: usize,
    number: Option<u8>,
}

fn usable(schema: &Schema, part: &KeyPart) -> Option<Usable> {
    let f = part.field?;
    let fld = &schema.fields[f];
    if fld.occ.len() != 1 || fld.occ[0].len != part.len {
        return None;
    }
    match fld.class {
        Class::Text => Some(Usable { field: f, len: part.len, number: None }),
        Class::Number { decimals, signed: false } => Some(Usable { field: f, len: part.len, number: Some(decimals) }),
        _ => None,
    }
}

#[derive(Default)]
struct Preds {
    eq: Option<Value>,
    in_list: Option<Vec<Value>>,
    bounds: Vec<(CmpOp, Value)>,
    begins: Option<(Value, ())>,
}

fn flatten<'a>(c: &'a RCond, out: &mut Vec<&'a RCond>) {
    match c {
        RCond::And(a, b) => {
            flatten(a, out);
            flatten(b, out);
        }
        _ => out.push(c),
    }
}

fn const_value(e: &RExpr, param: &dyn Fn(usize) -> Option<Value>) -> Option<Option<Value>> {
    // `Some(None)`: a `?` with no value yet (Check) — fits by assumption.
    match e {
        RExpr::Lit(v) => Some(Some(v.clone())),
        RExpr::Param(n) => Some(param(*n)),
        RExpr::Add(a, b) | RExpr::Sub(a, b) => {
            let (x, y) = (const_value(a, param)?, const_value(b, param)?);
            match (x, y) {
                (Some(x), Some(y)) => {
                    let r = if matches!(e, RExpr::Add(..)) { x.add(&y) } else { x.sub(&y) };
                    r.ok().map(Some)
                }
                _ => Some(None),
            }
        }
        _ => None,
    }
}

fn flip(op: CmpOp) -> CmpOp {
    match op {
        CmpOp::Lt => CmpOp::Gt,
        CmpOp::Le => CmpOp::Ge,
        CmpOp::Gt => CmpOp::Lt,
        CmpOp::Ge => CmpOp::Le,
        o => o,
    }
}

fn predicates(conj: &[&RCond], field: usize, param: &dyn Fn(usize) -> Option<Value>, ph: &Value) -> Preds {
    let mut p = Preds::default();
    let is_field = |e: &RExpr| matches!(e, RExpr::Field(r) if r.field == field && r.occ == 0);
    for c in conj {
        match c {
            RCond::Cmp(l, op, r) => {
                let (op, v) = if is_field(l) {
                    (*op, const_value(r, param))
                } else if is_field(r) {
                    (flip(*op), const_value(l, param))
                } else {
                    continue;
                };
                let Some(v) = v else { continue };
                // A `?` with no value (Check) stands for one that fits.
                let v = v.unwrap_or_else(|| ph.clone());
                match op {
                    CmpOp::Eq => {
                        if p.eq.is_none() {
                            p.eq = Some(v);
                        }
                    }
                    CmpOp::Ne => {}
                    o => p.bounds.push((o, v)),
                }
            }
            RCond::Between(e, lo, hi) if is_field(e) => {
                if let (Some(Some(l)), Some(Some(h))) = (const_value(lo, param), const_value(hi, param)) {
                    p.bounds.push((CmpOp::Ge, l));
                    p.bounds.push((CmpOp::Le, h));
                } else if let (Some(None), Some(None)) = (const_value(lo, param), const_value(hi, param)) {
                    p.bounds.push((CmpOp::Ge, ph.clone()));
                    p.bounds.push((CmpOp::Le, ph.clone()));
                }
            }
            RCond::In(e, list) if is_field(e) => {
                let vals: Option<Vec<Value>> = list
                    .iter()
                    .map(|x| const_value(x, param).map(|v| v.unwrap_or_else(|| ph.clone())))
                    .collect();
                if let Some(v) = vals {
                    if p.in_list.is_none() {
                        p.in_list = Some(v);
                    }
                }
            }
            RCond::BeginsWith(e, pfx) if is_field(e) => {
                if let Some(v) = const_value(pfx, param) {
                    p.begins = Some((v.unwrap_or_else(|| ph.clone()), ()));
                }
            }
            _ => {}
        }
    }
    p
}

// ── Encoding a value into key bytes ──

/// The digits of an unsigned Number laid out for a part of `len` digits with
/// `decimals` implied; `None` when it does not fit exactly.
fn number_bytes(len: usize, decimals: u8, v: &Value) -> Option<Vec<u8>> {
    let Value::Num(n) = v else { return None };
    if n.mantissa < 0 || n.decimals > decimals {
        // More decimals than the part keeps can only match by truncation,
        // which `=` does not do; a trailing-zero literal is the one exception.
        if n.mantissa >= 0 && n.truncate_to(decimals).cmp(n) == std::cmp::Ordering::Equal {
            return number_bytes(len, decimals, &Value::Num(n.truncate_to(decimals)));
        }
        return None;
    }
    let scaled = n.truncate_to(decimals).mantissa;
    let digits = scaled.to_string();
    if digits.len() > len {
        return None;
    }
    let mut out = vec![b'0'; len - digits.len()];
    out.extend(digits.bytes());
    Some(out)
}

fn text_bytes(len: usize, v: &Value) -> Option<Vec<u8>> {
    let Value::Str(s) = v else { return None };
    let b = s.as_bytes();
    if b.len() > len {
        return None;
    }
    Some(b.to_vec())
}

/// Exact key bytes for `=`.
fn enc_eq(u: &Usable, v: &Value) -> Option<Vec<u8>> {
    match u.number {
        Some(d) => number_bytes(u.len, d, v),
        None => text_bytes(u.len, v).map(|mut b| {
            b.resize(u.len, b' ');
            b
        }),
    }
}

/// The lower and upper bound a comparison gives, as a superset window.
fn enc_bound(u: &Usable, op: CmpOp, v: &Value) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let bytes = match u.number {
        Some(d) => number_bytes(u.len, d, v),
        None => text_bytes(u.len, v),
    };
    let Some(b) = bytes else { return (None, None) };
    match op {
        CmpOp::Ge | CmpOp::Gt => (Some(b), None),
        CmpOp::Le | CmpOp::Lt => (None, Some(b)),
        _ => (None, None),
    }
}

/// `begins_with(text-part, 'p')` as a window.
fn enc_prefix(u: &Usable, v: &Value) -> Option<(Vec<u8>, Vec<u8>)> {
    if u.number.is_some() {
        return None;
    }
    text_bytes(u.len, v).map(|b| (b.clone(), b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partiql::analyze::{analyze, RStmt};
    use crate::partiql::parser::parse_statement;
    use crate::partiql::schema::{KeyInput, Schema};

    pub fn schema() -> Schema {
        let fd = "FD CUSTOMERS.\n01 CUST-REC.\n   05 CUST-ID PIC 9(8).\n   05 CUST-NAME PIC X(20).\n   05 CUST-BALANCE PIC S9(6)V99.\n   05 CUST-CITY PIC X(10).\n";
        Schema::from_fd_text(
            "CUSTOMERS",
            fd,
            KeyInput { name: None, parts: vec![("CUST-ID".into(), 0, 8)], duplicates: false },
            vec![KeyInput { name: Some("BY-NAME".into()), parts: vec![("CUST-NAME".into(), 8, 20)], duplicates: true }],
        )
        .unwrap()
    }

    fn class(sql: &str) -> ReadClass {
        let s = schema();
        let a = analyze(&parse_statement(sql).unwrap(), &s, false).unwrap();
        let RStmt::Select(sel) = a.stmt else { panic!() };
        classify(&sel, &s)
    }

    #[test]
    fn the_four_statements_of_ac1() {
        assert_eq!(class("SELECT * FROM CUSTOMERS WHERE CUST-ID = 42"), ReadClass::Keyed);
        assert_eq!(class("SELECT * FROM CUSTOMERS WHERE CUST-ID BETWEEN 10 AND 99"), ReadClass::Range);
        assert_eq!(class("SELECT * FROM CUSTOMERS WHERE CUST-BALANCE > 1000.50"), ReadClass::Scan);
        assert_eq!(class("SELECT * FROM CUSTOMERS WHERE CUST-ID IN [1, 2, 3]"), ReadClass::Keyed);
    }

    #[test]
    fn other_shapes() {
        assert_eq!(class("SELECT * FROM CUSTOMERS"), ReadClass::Scan);
        assert_eq!(class("SELECT * FROM CUSTOMERS WHERE CUST-ID >= ?"), ReadClass::Range);
        assert_eq!(class("SELECT * FROM CUSTOMERS WHERE CUST-ID = 1 OR CUST-ID = 2"), ReadClass::Scan);
        assert_eq!(class("SELECT * FROM CUSTOMERS.\"BY-NAME\" WHERE begins_with(CUST-NAME, 'A')"), ReadClass::Range);
        assert_eq!(class("SELECT * FROM CUSTOMERS.\"BY-NAME\" WHERE CUST-NAME = 'Ana'"), ReadClass::Keyed);
        // A literal that does not fit the part is a scan, never a wrong answer.
        assert_eq!(class("SELECT * FROM CUSTOMERS WHERE CUST-ID = -5"), ReadClass::Scan);
        assert_eq!(class("SELECT * FROM CUSTOMERS WHERE CUST-ID = 1.5"), ReadClass::Scan);
        assert_eq!(class("SELECT * FROM CUSTOMERS WHERE CUST-ID = 123456789"), ReadClass::Scan);
    }

    #[test]
    fn key_bytes_are_zero_padded_digits_and_space_padded_text() {
        let s = schema();
        let a = analyze(&parse_statement("SELECT * FROM CUSTOMERS WHERE CUST-ID = ?").unwrap(), &s, false).unwrap();
        let RStmt::Select(sel) = a.stmt else { panic!() };
        let p = plan(&sel, &s, &|_| Some(Value::int(42)));
        assert_eq!(p.access, Access::Keyed(vec![b"00000042".to_vec()]));
        let a = analyze(&parse_statement("SELECT * FROM CUSTOMERS.\"BY-NAME\" WHERE CUST-NAME = ?").unwrap(), &s, false).unwrap();
        let RStmt::Select(sel) = a.stmt else { panic!() };
        let p = plan(&sel, &s, &|_| Some(Value::text("Ana")));
        assert_eq!(p.access, Access::Keyed(vec![b"Ana                 ".to_vec()]));
    }
}
