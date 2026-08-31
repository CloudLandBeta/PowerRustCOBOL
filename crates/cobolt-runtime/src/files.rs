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
    /// The names of this field's enclosing groups, **innermost first**, up to
    /// but not including the `01` record itself.
    ///
    /// A record may hold several fields of the same name in different groups,
    /// and then only the group names tell them apart: IX215A's `IX-FD3`
    /// declares its prime key and both alternates as `IX-FD3-KEY`, qualified
    /// `IN IX-FD3-RECKEY-AREA`, `OF IX-FD3-ALTKEY1-AREA` and `IN
    /// IX-FD3-ALTKEY2-AREA`. Without the path all three resolve to the first
    /// one, and every key of the file indexes the same bytes.
    pub quals: Vec<String>,
    pub offset: usize,
    pub len: usize,
    pub numeric: bool,
    pub decimals: u8,
    /// Whether the PICTURE is signed (`S9…`) with the sign carried IN the
    /// digits (no `SEPARATE`): the record image stores it as a trailing
    /// overpunch, and dropping it read `-5432` back as `5432` — CCVS85
    /// ST127A's descending keys sorted unsigned and eighty assertions failed.
    pub signed: bool,
    /// Whether this entry describes a **group** rather than an elementary item.
    ///
    /// A group holds no value of its own — its characters are its children's,
    /// concatenated — so its bytes have to be asked for as a group. Reading one
    /// as if it were an elementary item finds nothing and yields spaces, which
    /// is what made a `RECORD KEY` naming a group search the file for blanks.
    pub is_group: bool,
}

/// Byte layout of an FD record: total length plus every elementary field.
#[derive(Debug, Clone, Default)]
pub struct RecordLayout {
    pub len: usize,
    pub fields: Vec<FieldPos>,
    /// The record's **group** items, with the extent each one spans.
    ///
    /// `fields` holds elementary items only, because those are what a record
    /// is read into and written from. A `RECORD KEY`, though, usually names a
    /// *group* — IX214A's is `IX-FS1-KEY`, three subordinate items across 13
    /// bytes — and with nowhere to find it the key resolved to nothing and the
    /// engine fell back to indexing the whole record. Keys are looked up here
    /// when no elementary field matches; nothing else consults it, so reading
    /// and writing records behave exactly as before.
    pub groups: Vec<FieldPos>,
}

/// Lay out one data description: recurse into a group's children, or take an
/// elementary item's bytes. `OCCURS` repeats the subtree.
fn walk(
    d: &DataDecl,
    sign: Option<SignClause>,
    path: &mut Vec<String>,
    offset: &mut usize,
    fields: &mut Vec<FieldPos>,
    groups: &mut Vec<FieldPos>,
) {
    let times = d
        .occurs
        .as_ref()
        .map(|o| o.max.max(1) as usize)
        .unwrap_or(1);
    // `SIGN IS … SEPARATE` written on a group applies to every subordinate
    // signed item that does not carry its own.
    let sign = d.sign.or(sign);
    for i in 0..times {
        if !d.children.is_empty() {
            // A group name qualifies everything under it. `FILLER` groups
            // have no name and qualify nothing.
            let began = *offset;
            let pushed = match &d.name {
                Some(n) => {
                    path.push(n.to_ascii_uppercase());
                    true
                }
                None => false,
            };
            walk_siblings(&d.children, sign, path, offset, fields, groups);
            if pushed {
                path.pop();
            }
            // Record the group's extent, so a key that names it can be found.
            // Only the first occurrence: an unsubscripted key reference means
            // the first one, and the rest would just be duplicates of the name.
            if i == 0 {
                if let Some(name) = &d.name {
                    groups.push(FieldPos {
                        name: name.to_ascii_uppercase(),
                        quals: path.iter().rev().cloned().collect(),
                        offset: began,
                        len: offset.saturating_sub(began).max(1),
                        numeric: false,
                        decimals: 0,
                        signed: false,
                        is_group: true,
                    });
                }
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
                    // Innermost group first, which is the order a COBOL
                    // `OF`/`IN` chain is written in.
                    quals: path.iter().rev().cloned().collect(),
                    offset: *offset,
                    len,
                    numeric,
                    decimals: pic.decimals.min(u8::MAX as u16) as u8,
                    signed: matches!(pic.kind, PicKind::Numeric)
                        && pic.template.to_ascii_uppercase().starts_with('S')
                        && !sign.is_some_and(|s| s.separate),
                    is_group: false,
                });
            }
            *offset += len;
        }
    }
}

/// Lay out one group's children in order, giving a `REDEFINES` item the same
/// bytes as the item it redefines instead of bytes of its own.
///
/// **A redefining item is another description of storage that already exists**,
/// so it neither adds to the record nor pushes what follows it along. Laying it
/// out as if it were a new item made every later field wrong by the redefining
/// item's width: IX215A's `IX-FD1` describes its 13-byte record key and then
/// `IX-REDF-RECKEY REDEFINES IX-FD1-KEY` over it, and without this the record
/// grew by 13 bytes and the alternate keys after it indexed the wrong columns.
///
/// The target is looked up among the siblings already placed, so a redefinition
/// of a redefinition works — `IX-FD1` has one, `R-REDF-RECKEY-1-7 REDEFINES
/// R-RECKEY-1-7`. A `REDEFINES` naming something not at this level is laid out
/// where it fell, which is what happened to every such item before.
fn walk_siblings(
    children: &[DataDecl],
    sign: Option<SignClause>,
    path: &mut Vec<String>,
    offset: &mut usize,
    fields: &mut Vec<FieldPos>,
    groups: &mut Vec<FieldPos>,
) {
    // Sibling name → where it started and how wide it is, in declaration order.
    let mut placed: Vec<(String, usize)> = Vec::new();
    for c in children {
        let target = c.redefines.as_ref().and_then(|t| {
            let t = t.to_ascii_uppercase();
            placed.iter().find(|(n, _)| *n == t).map(|(_, at)| *at)
        });
        // Where the record would continue if this item were not a redefinition.
        let resume = *offset;
        if let Some(at) = target {
            *offset = at;
        }
        let began = *offset;
        walk(c, sign, path, offset, fields, groups);
        if target.is_some() {
            // Storage is shared, so the record continues where it already was.
            // The `max` is defensive: the standard forbids a redefinition wider
            // than what it redefines, and truncating the record here would be
            // worse than honouring it.
            *offset = resume.max(*offset);
        }
        if let Some(n) = &c.name {
            placed.push((n.to_ascii_uppercase(), began));
        }
    }
}

/// Whether `needle` appears in `hay` in order, though not necessarily
/// adjacently.
///
/// COBOL qualification is by *containment*, not by immediate parentage: a
/// chain may name any subset of the enclosing groups, as long as it names them
/// outward in order.
fn is_subsequence(needle: &[String], hay: &[String]) -> bool {
    let mut it = hay.iter();
    needle.iter().all(|n| it.any(|h| h == n))
}

/// Compute the byte layout of an FD `01` record by walking its subordinate
/// items in declaration order (groups recurse; elementary items take
/// `digits + decimals` bytes). OCCURS multiplies the subtree width.
pub fn compute_layout(record: &DataDecl) -> RecordLayout {
    let mut fields = Vec::new();
    let mut groups = Vec::new();
    let mut offset = 0usize;

    // The `01` itself does not qualify its subordinates — a key is written
    // `IX-FD3-KEY IN IX-FD3-RECKEY-AREA`, naming the group, not the record.
    let mut path: Vec<String> = Vec::new();
    if record.children.is_empty() {
        walk(
            record,
            record.sign,
            &mut path,
            &mut offset,
            &mut fields,
            &mut groups,
        );
    } else {
        walk_siblings(
            &record.children,
            record.sign,
            &mut path,
            &mut offset,
            &mut fields,
            &mut groups,
        );
    }
    RecordLayout {
        len: offset.max(1),
        fields,
        groups,
    }
}

impl RecordLayout {
    pub fn field(&self, name: &str) -> Option<&FieldPos> {
        self.field_qualified(name, &[])
    }

    /// The field called `name` whose enclosing groups match `quals`.
    ///
    /// `quals` is a COBOL `OF`/`IN` chain, innermost first, and — as the
    /// standard requires — it need only be a **subsequence** of the field's
    /// ancestors: `B OF D` names the field even when it sits in `B` in `C` in
    /// `D`. An empty chain matches the first field of that name, which is the
    /// behaviour every unqualified caller had before.
    ///
    /// With no match on the qualifiers this falls back to the bare name, so a
    /// qualifier naming a group that is not in the layout still finds the
    /// field rather than silently losing the key.
    /// A **group** name resolves to the extent it spans, which is how a
    /// `RECORD KEY` naming a group is found — elementary fields are searched
    /// first, so nothing that already resolved changes.
    pub fn field_qualified(&self, name: &str, quals: &[String]) -> Option<&FieldPos> {
        let n = name.to_ascii_uppercase();
        let wanted: Vec<String> = quals.iter().map(|q| q.to_ascii_uppercase()).collect();
        let matching = |set: &'_ [FieldPos]| -> Option<usize> {
            set.iter()
                .position(|f| f.name == n && is_subsequence(&wanted, &f.quals))
                .or_else(|| set.iter().position(|f| f.name == n))
        };
        matching(&self.fields)
            .map(|i| &self.fields[i])
            .or_else(|| matching(&self.groups).map(|i| &self.groups[i]))
    }

    /// A `KeySpec` for the named key field (its slice of the record).
    pub fn key_spec(&self, name: &str, duplicates: bool) -> Option<KeySpec> {
        self.key_spec_qualified(name, &[], duplicates)
    }

    /// A `KeySpec` for a key field named with an `OF`/`IN` qualification.
    pub fn key_spec_qualified(
        &self,
        name: &str,
        quals: &[String],
        duplicates: bool,
    ) -> Option<KeySpec> {
        self.field_qualified(name, quals).map(|f| KeySpec {
            offset: f.offset,
            len: f.len,
            duplicates,
        })
    }

    /// The current byte value of a single (key) field.
    pub fn field_value(&self, env: &CobolEnvironment, name: &str) -> Option<Vec<u8>> {
        self.field_value_qualified(env, name, &[])
    }

    /// The current byte value of a key field named with an `OF`/`IN`
    /// qualification.
    pub fn field_value_qualified(
        &self,
        env: &CobolEnvironment,
        name: &str,
        quals: &[String],
    ) -> Option<Vec<u8>> {
        self.field_qualified(name, quals).map(|f| field_bytes(env, f))
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
            let key = env_key(env, f);
            if f.numeric {
                // The trailing byte may carry the sign overpunch; a leading
                // `-` (a SIGN SEPARATE writer, or hand-made data) is honoured
                // too.
                let mut neg = slice.first() == Some(&b'-');
                let mut work: Vec<u8> = slice.to_vec();
                if f.signed {
                    if let Some(last) = work.last_mut() {
                        if let Some((d, n)) = de_overpunch(*last) {
                            *last = b'0' + d;
                            neg = n;
                        }
                    }
                }
                let digits: String = work
                    .iter()
                    .map(|&b| if b.is_ascii_digit() { b as char } else { '0' })
                    .collect();
                let mut mantissa: i128 = digits.parse().unwrap_or(0);
                if neg {
                    mantissa = -mantissa;
                }
                env.set(
                    &key,
                    CobolValue::Numeric(CobolNumeric::new(mantissa, f.decimals)),
                );
            } else {
                env.set_str(&key, &String::from_utf8_lossy(slice));
            }
        }
    }
}

/// The ASCII overpunch for one digit: `{` and A–I carry +0..+9, `}` and J–R
/// carry −0..−9 — the convention IBM's tables map onto ASCII, and the one the
/// suite's own signed record images use.
pub(crate) fn overpunch(digit: u8, negative: bool) -> u8 {
    match (digit, negative) {
        (0, false) => b'{',
        (d, false) => b'A' + d - 1,
        (0, true) => b'}',
        (d, true) => b'J' + d - 1,
    }
}

/// The digit and sign an overpunched byte carries, when it is one.
pub(crate) fn de_overpunch(b: u8) -> Option<(u8, bool)> {
    match b {
        b'{' => Some((0, false)),
        b'A'..=b'I' => Some((b - b'A' + 1, false)),
        b'}' => Some((0, true)),
        b'J'..=b'R' => Some((b - b'J' + 1, true)),
        _ => None,
    }
}

/// The environment's storage key for one field of the record.
///
/// A name that is unique resolves to itself, so this is the bare name for
/// almost every field. Where a record holds the same name in several groups —
/// IX215A's `IX-FD3` has three `IX-FD3-KEY`s — the environment keys them by
/// their path, and reading or writing the bare name would hit whichever one
/// happened to be stored under it.
fn env_key(env: &CobolEnvironment, f: &FieldPos) -> String {
    env.resolve_name(&f.name, &f.quals)
}

/// The exact-`len` byte image of one field's current value.
fn field_bytes(env: &CobolEnvironment, f: &FieldPos) -> Vec<u8> {
    // A group's characters are its children's, concatenated, and only the
    // environment can assemble them — it holds no value under the group's own
    // name. `env.get` returns nothing for one, which yielded a key of spaces.
    if f.is_group {
        let mut b = env.display_bytes(&env_key(env, f)).unwrap_or_default();
        b.resize(f.len, b' ');
        return b;
    }
    match env.get(&env_key(env, f)) {
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
            let mut b = s.into_bytes();
            // A signed field carries its sign as a trailing overpunch in the
            // record image — `{`/A–I for +0..+9, `}`/J–R for −0..−9 — and
            // `distribute` reads the same convention back.
            if f.signed {
                if let Some(last) = b.last_mut() {
                    let d = last.wrapping_sub(b'0');
                    *last = overpunch(d, n.mantissa < 0);
                }
            }
            b
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

    /// A `REDEFINES` item describes storage that already exists.
    ///
    /// It adds nothing to the record and does not push what follows it along.
    /// Laying it out as a new item made every later field wrong by its width —
    /// IX215A's `IX-FD1` redefines its 13-byte record key, and the record grew
    /// by 13 bytes so the alternate keys after it indexed the wrong columns.
    #[test]
    fn a_redefines_shares_the_bytes_it_redefines() {
        // 01 REC.
        //    05 KEY-AREA.  10 K1 PIC X(5).  10 K2 PIC X(3).
        //    05 REDF REDEFINES KEY-AREA.  10 R1 PIC X(8).
        //    05 TAIL PIC X(4).
        let mut redf = group("REDF", vec![elem("R1", pic("X(8)", PicKind::Alphanumeric, 8, 0))]);
        redf.redefines = Some("KEY-AREA".into());
        let rec = group(
            "REC",
            vec![
                group(
                    "KEY-AREA",
                    vec![
                        elem("K1", pic("X(5)", PicKind::Alphanumeric, 5, 0)),
                        elem("K2", pic("X(3)", PicKind::Alphanumeric, 3, 0)),
                    ],
                ),
                redf,
                elem("TAIL", pic("X(4)", PicKind::Alphanumeric, 4, 0)),
            ],
        );
        let layout = compute_layout(&rec);
        assert_eq!(layout.len, 12, "8 shared bytes plus TAIL's 4, not 20");
        assert_eq!(layout.field("K1").unwrap().offset, 0);
        assert_eq!(layout.field("K2").unwrap().offset, 5);
        assert_eq!(
            layout.field("R1").unwrap().offset,
            0,
            "the redefinition starts where the redefined item starts"
        );
        assert_eq!(
            layout.field("TAIL").unwrap().offset,
            8,
            "TAIL follows KEY-AREA, not the redefinition"
        );
    }

    /// A redefinition of a redefinition resolves against its own level.
    ///
    /// `IX-FD1` has one — `R-REDF-RECKEY-1-7 REDEFINES R-RECKEY-1-7`, itself
    /// inside an item that redefines the record key.
    #[test]
    fn a_redefines_of_a_redefines_resolves_at_its_own_level() {
        // 01 REC.
        //    05 OUTER.  10 A PIC X(7).  10 B REDEFINES A. 15 B1 PIC X(5). 15 B2 PIC XX.
        //               10 C PIC X(6).
        //    05 TAIL PIC X(2).
        let mut b = group(
            "B",
            vec![
                elem("B1", pic("X(5)", PicKind::Alphanumeric, 5, 0)),
                elem("B2", pic("XX", PicKind::Alphanumeric, 2, 0)),
            ],
        );
        b.redefines = Some("A".into());
        let rec = group(
            "REC",
            vec![
                group(
                    "OUTER",
                    vec![
                        elem("A", pic("X(7)", PicKind::Alphanumeric, 7, 0)),
                        b,
                        elem("C", pic("X(6)", PicKind::Alphanumeric, 6, 0)),
                    ],
                ),
                elem("TAIL", pic("X(2)", PicKind::Alphanumeric, 2, 0)),
            ],
        );
        let layout = compute_layout(&rec);
        assert_eq!(layout.field("A").unwrap().offset, 0);
        assert_eq!(layout.field("B1").unwrap().offset, 0);
        assert_eq!(layout.field("B2").unwrap().offset, 5);
        assert_eq!(layout.field("C").unwrap().offset, 7, "C follows A, not B");
        assert_eq!(layout.field("TAIL").unwrap().offset, 13);
        assert_eq!(layout.len, 15);
    }

    /// A field's enclosing groups are recorded, so same-named fields in
    /// different groups can be told apart.
    #[test]
    fn fields_carry_their_qualification_path() {
        // 01 REC.  05 P.  10 K PIC X(4).   05 Q.  10 K PIC X(4).
        let rec = group(
            "REC",
            vec![
                group("P", vec![elem("K", pic("X(4)", PicKind::Alphanumeric, 4, 0))]),
                group("Q", vec![elem("K", pic("X(4)", PicKind::Alphanumeric, 4, 0))]),
            ],
        );
        let layout = compute_layout(&rec);
        assert_eq!(
            layout.field_qualified("K", &["P".into()]).unwrap().offset,
            0
        );
        assert_eq!(
            layout.field_qualified("K", &["Q".into()]).unwrap().offset,
            4
        );
        // Unqualified still means the first of that name.
        assert_eq!(layout.field("K").unwrap().offset, 0);
        // A qualifier naming no group falls back rather than losing the field.
        assert_eq!(
            layout.field_qualified("K", &["NOPE".into()]).unwrap().offset,
            0
        );
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
