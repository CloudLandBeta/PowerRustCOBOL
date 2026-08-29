// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! File-runtime support shared by every file ORGANIZATION.
//!
//! COBOL's file verbs (`OPEN`, `CLOSE`, `READ`, `WRITE`, `REWRITE`, `DELETE`,
//! `START`) are dispatched by the organization declared in each file's `SELECT`
//! — SEQUENTIAL / LINE SEQUENTIAL / INDEXED today, RELATIVE later — rather than
//! hard-wired to one type. This module holds the pieces common to that dispatch:
//!
//! * [`RecordLayout`] — the byte layout of an FD record (each elementary field's
//!   offset/width), used to *materialize* a record buffer from its subfields
//!   (for WRITE/REWRITE) and to *distribute* a buffer back into them (for READ).
//! * Key resolution — turning RECORD KEY / ALTERNATE KEY field names into the
//!   `[offset, len)` key specs the indexed engine needs.

use cobolt_ast::data::{DataDecl, PicKind, SignClause};

use crate::environment::CobolEnvironment;
use crate::indexed::KeySpec;
use crate::value::{CobolNumeric, CobolValue};

/// One elementary field's position inside a record buffer.
#[derive(Debug, Clone)]
pub struct FieldPos {
    pub name: String,
    pub offset: usize,
    pub len: usize,
    pub numeric: bool,
    pub decimals: u8,
}

/// Byte layout of an FD record: total length plus every elementary field.
#[derive(Debug, Clone, Default)]
pub struct RecordLayout {
    pub len: usize,
    pub fields: Vec<FieldPos>,
}

/// Compute the byte layout of an FD `01` record by walking its subordinate
/// items in declaration order (groups recurse; elementary items take
/// `digits + decimals` bytes). OCCURS multiplies the subtree width.
pub fn compute_layout(record: &DataDecl) -> RecordLayout {
    let mut fields = Vec::new();
    let mut offset = 0usize;

    fn walk(d: &DataDecl, sign: Option<SignClause>, offset: &mut usize, fields: &mut Vec<FieldPos>) {
        let times = d
            .occurs
            .as_ref()
            .map(|o| o.max.max(1) as usize)
            .unwrap_or(1);
        // `SIGN IS … SEPARATE` written on a group applies to every subordinate
        // signed item that does not carry its own.
        let sign = d.sign.or(sign);
        for _ in 0..times {
            if !d.children.is_empty() {
                for c in &d.children {
                    walk(c, sign, offset, fields);
                }
            } else if let Some(pic) = &d.picture {
                // `SIGN IS SEPARATE CHARACTER` gives the sign its own character
                // position, so the item is one byte wider than its digits. The
                // record area is measured by the environment too, and the two
                // have to agree: SQ111A's `PIC S9(5) SIGN IS LEADING SEPARATE`
                // made the layout 155 bytes against the environment's 156, and
                // every field after it landed one byte out.
                let sep = usize::from(
                    matches!(pic.kind, PicKind::Numeric) && sign.is_some_and(|s| s.separate),
                );
                let len = (pic.digits as usize + pic.decimals as usize + sep).max(1);
                // A `FILLER` occupies its bytes like any other item — it simply
                // has no name to read or write. Skipping it *and* its width put
                // every following field at the wrong offset, so a record
                // beginning `03 FILLER PIC X(120). 03 EXT-18 PIC X(18).` looked
                // 18 bytes long with `EXT-18` at offset zero (SQ134A).
                if let Some(name) = &d.name {
                    let numeric = matches!(pic.kind, PicKind::Numeric | PicKind::NumericEdited);
                    fields.push(FieldPos {
                        name: name.to_ascii_uppercase(),
                        offset: *offset,
                        len,
                        numeric,
                        decimals: pic.decimals.min(u8::MAX as u16) as u8,
                    });
                }
                *offset += len;
            }
        }
    }

    if record.children.is_empty() {
        walk(record, record.sign, &mut offset, &mut fields);
    } else {
        for c in &record.children {
            walk(c, record.sign, &mut offset, &mut fields);
        }
    }
    RecordLayout {
        len: offset.max(1),
        fields,
    }
}

impl RecordLayout {
    pub fn field(&self, name: &str) -> Option<&FieldPos> {
        let n = name.to_ascii_uppercase();
        self.fields.iter().find(|f| f.name == n)
    }

    /// A `KeySpec` for the named key field (its slice of the record).
    pub fn key_spec(&self, name: &str, duplicates: bool) -> Option<KeySpec> {
        self.field(name).map(|f| KeySpec {
            offset: f.offset,
            len: f.len,
            duplicates,
        })
    }

    /// The current byte value of a single (key) field.
    pub fn field_value(&self, env: &CobolEnvironment, name: &str) -> Option<Vec<u8>> {
        self.field(name).map(|f| field_bytes(env, f))
    }

    /// Build the contiguous record buffer from the current subfield values.
    pub fn materialize(&self, env: &CobolEnvironment) -> Vec<u8> {
        let mut buf = vec![b' '; self.len];
        self.materialize_into(env, &mut buf);
        buf
    }

    /// Lay this record's subfields over an existing buffer, leaving every byte
    /// no named field covers as it was.
    ///
    /// An FD's several `01` record descriptions all describe **one** record
    /// area, so a `WRITE` that names one of them still sends the whole area:
    /// where the named record has `FILLER`, what another record description put
    /// there shows through. Overlaying them in declaration order with the
    /// written one last reproduces that, and it is why this takes a buffer
    /// rather than making its own.
    pub fn materialize_into(&self, env: &CobolEnvironment, buf: &mut [u8]) {
        for f in &self.fields {
            let bytes = field_bytes(env, f);
            let end = (f.offset + f.len).min(buf.len());
            if f.offset >= end {
                continue;
            }
            let n = (end - f.offset).min(bytes.len());
            buf[f.offset..f.offset + n].copy_from_slice(&bytes[..n]);
        }
    }

    /// Distribute a record buffer back into the subfields.
    pub fn distribute(&self, env: &mut CobolEnvironment, buf: &[u8]) {
        for f in &self.fields {
            if f.offset >= buf.len() {
                continue;
            }
            let end = (f.offset + f.len).min(buf.len());
            let slice = &buf[f.offset..end];
            if f.numeric {
                let digits: String = slice
                    .iter()
                    .map(|&b| if b.is_ascii_digit() { b as char } else { '0' })
                    .collect();
                let mantissa: i128 = digits.parse().unwrap_or(0);
                env.set(
                    &f.name,
                    CobolValue::Numeric(CobolNumeric::new(mantissa, f.decimals)),
                );
            } else {
                env.set_str(&f.name, &String::from_utf8_lossy(slice));
            }
        }
    }
}

/// The exact-`len` byte image of one field's current value.
fn field_bytes(env: &CobolEnvironment, f: &FieldPos) -> Vec<u8> {
    match env.get(&f.name) {
        Some(CobolValue::Numeric(n)) => {
            let digits = n.mantissa.unsigned_abs().to_string();
            let mut s = if digits.len() < f.len {
                format!("{}{}", "0".repeat(f.len - digits.len()), digits)
            } else {
                digits
            };
            if s.len() > f.len {
                s = s[s.len() - f.len..].to_string(); // keep low-order digits
            }
            s.into_bytes()
        }
        Some(v) => {
            let mut b = v.as_display_string().into_bytes();
            b.resize(f.len, b' ');
            b
        }
        None => vec![b' '; f.len],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_ast::data::{PicClause, PicKind};
    use cobolt_lexer::Span;

    fn pic(template: &str, kind: PicKind, digits: u16, decimals: u16) -> PicClause {
        PicClause {
            template: template.into(),
            kind,
            digits,
            decimals,
            span: Span::dummy(),
        }
    }
    fn elem(name: &str, p: PicClause) -> DataDecl {
        DataDecl {
            level: 5,
            name: Some(name.into()),
            picture: Some(p),
            value: None,
            usage: Default::default(),
            object_class: None,
            occurs: None,
            redefines: None,
            renames: None,
            condition_values: vec![],
            is_global: false,
            is_external: false,
            blank_when_zero: false,
            children: vec![],
            span: Span::dummy(),
            justified: false,
            sign: None,
        }
    }
    fn group(name: &str, children: Vec<DataDecl>) -> DataDecl {
        DataDecl {
            level: 1,
            name: Some(name.into()),
            picture: None,
            value: None,
            usage: Default::default(),
            object_class: None,
            occurs: None,
            redefines: None,
            renames: None,
            condition_values: vec![],
            is_global: false,
            is_external: false,
            blank_when_zero: false,
            children,
            span: Span::dummy(),
            justified: false,
            sign: None,
        }
    }

    #[test]
    fn layout_offsets_and_round_trip() {
        // 01 REC. 05 ID PIC 9(4).  05 NAME PIC X(6).
        let rec = group(
            "REC",
            vec![
                elem("ID", pic("9(4)", PicKind::Numeric, 4, 0)),
                elem("NAME", pic("X(6)", PicKind::Alphanumeric, 6, 0)),
            ],
        );
        let layout = compute_layout(&rec);
        assert_eq!(layout.len, 10);
        assert_eq!(layout.field("ID").unwrap().offset, 0);
        assert_eq!(layout.field("NAME").unwrap().offset, 4);
        let ks = layout.key_spec("ID", false).unwrap();
        assert_eq!((ks.offset, ks.len), (0, 4));

        // materialize from subfields, then distribute back.
        let mut env = CobolEnvironment::new();
        env.set("ID", CobolValue::Numeric(CobolNumeric::new(42, 0)));
        env.set_str("NAME", "BOB");
        let buf = layout.materialize(&env);
        assert_eq!(&buf, b"0042BOB   ");

        let mut env2 = CobolEnvironment::new();
        env2.set("ID", CobolValue::Numeric(CobolNumeric::new(0, 0)));
        env2.set("NAME", CobolValue::spaces(6)); // PIC X(6) capacity
        layout.distribute(&mut env2, b"0007ALICE ");
        assert_eq!(env2.get("ID").unwrap().as_i64(), Some(7));
        assert_eq!(env2.get_string("NAME").as_deref(), Some("ALICE "));
    }
}
