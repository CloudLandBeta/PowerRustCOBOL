// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Where every data item lives: the byte-accurate layout of WASM-010 R3.
//!
//! The layout of a program is a tree of [`Item`]s with offsets into the
//! **region** of a form instance (or of the single run, for a program run on
//! its own). `PICTURE` and `USAGE` decide an item's size; `OCCURS` repeats an
//! element at a fixed stride (at its maximum for `DEPENDING ON`); `REDEFINES`
//! starts at the redefined item's offset; a group is the sum of its children
//! (the longest alternative of a redefinition chain); `FILLER` takes room and
//! has no name; `88` and `66` take none.

use crate::error::{GenError, GenResult};
use crate::pic;
use cobolt_ast::data::{ConditionValue, DataDecl, PicKind, SignClause, Usage};
use cobolt_ast::expr::Literal;
use cobolt_wasm_rt::fd::*;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Item {
    /// Upper-case; `None` for `FILLER`.
    pub name: Option<String>,
    pub level: u8,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// Offset of the first occurrence from the region base.
    pub off: u32,
    /// Bytes of ONE occurrence.
    pub size: u32,
    /// `(max, DEPENDING ON name)`.
    pub occurs: Option<(u32, Option<String>)>,
    pub fd: Fd,
    pub is_group: bool,
    pub redefines: Option<usize>,
    pub value: Option<Literal>,
    /// `88` condition-names: the values, with `parent` the host item.
    pub cond: Vec<ConditionValue>,
    /// `66 RENAMES from [THRU thru]`.
    pub renames: Option<(String, Option<String>)>,
    pub span_line: u32,
    /// Hidden items the generator allocated (loop counters, temporaries).
    pub hidden: bool,
    pub external: bool,
    pub global: bool,
    pub pic_template: Option<String>,
    pub pic_kind: Option<PicKind>,
    pub usage: Usage,
    pub blank_when_zero: bool,
    /// `INDEXED BY` names of this table, in order.
    pub index_names: Vec<String>,
    /// `ASCENDING/DESCENDING KEY` fields `(name, ascending)`.
    pub keys: Vec<(String, bool)>,
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub items: Vec<Item>,
    /// Upper-case name -> every item so named.
    by_name: HashMap<String, Vec<usize>>,
    /// Top-level (01/77/FD record) items in declaration order.
    pub roots: Vec<usize>,
    /// Bytes of the region.
    pub total: u32,
    /// Index-names (`INDEXED BY`): name -> hidden item.
    pub index_names: HashMap<String, usize>,
}

impl Layout {
    pub fn new() -> Layout {
        Layout { items: Vec::new(), by_name: HashMap::new(), roots: Vec::new(), total: 0, index_names: HashMap::new() }
    }

    /// Lay out one section's declarations after what is already there.
    pub fn add_section(&mut self, decls: &[DataDecl]) -> GenResult<()> {
        for d in decls {
            if d.level == 66 || d.level == 88 {
                continue; // handled after the records they refer to
            }
            // A level-01 REDEFINES shares the storage of the record it names.
            let (start, redef) = match &d.redefines {
                Some(target) => {
                    let t = target.to_ascii_uppercase();
                    let ti = self
                        .roots
                        .iter()
                        .rev()
                        .copied()
                        .find(|&r| self.items[r].name.as_deref() == Some(t.as_str()))
                        .ok_or_else(|| GenError::new(d.span.line, format!("REDEFINES {target}: no such record")))?;
                    (self.items[ti].off, Some(ti))
                }
                None => (self.total, None),
            };
            let idx = self.add_decl(d, None, start, None)?;
            self.items[idx].redefines = redef;
            self.roots.push(idx);
            let it = &self.items[idx];
            let end = it.off + it.size * it.occurs.as_ref().map_or(1, |o| o.0);
            self.total = self.total.max(end);
        }
        // 66-level items come after their records.
        for d in decls {
            if d.level == 66 {
                if let Some(r) = &d.renames {
                    let idx = self.items.len();
                    self.items.push(Item {
                        name: d.name.as_ref().map(|n| n.to_ascii_uppercase()),
                        level: 66,
                        parent: None,
                        children: vec![],
                        off: 0,
                        size: 0,
                        occurs: None,
                        fd: Fd::alnum(0),
                        is_group: false,
                        redefines: None,
                        value: None,
                        cond: vec![],
                        renames: Some((r.from.to_ascii_uppercase(), r.thru.as_ref().map(|t| t.to_ascii_uppercase()))),
                        span_line: d.span.line,
                        hidden: false,
                        external: false,
                        global: false,
                        pic_template: None,
                        pic_kind: None,
                        usage: Usage::Display,
                        blank_when_zero: false,
                        index_names: vec![],
                        keys: vec![],
                    });
                    if let Some(n) = &self.items[idx].name {
                        self.by_name.entry(n.clone()).or_default().push(idx);
                    }
                }
            }
        }
        Ok(())
    }

    /// Lay out the records of one file description: they share ONE record area
    /// (COBOL-85: the records of an FD implicitly redefine each other), so each
    /// starts where the first does and the area is the longest of them.
    pub fn add_file_records(&mut self, decls: &[DataDecl]) -> GenResult<()> {
        let start = self.total;
        let mut first: Option<usize> = None;
        let mut end = start;
        for d in decls {
            if d.level == 66 || d.level == 88 {
                continue;
            }
            let idx = self.add_decl(d, None, start, None)?;
            match first {
                None => first = Some(idx),
                Some(f) => self.items[idx].redefines = Some(f),
            }
            self.roots.push(idx);
            let it = &self.items[idx];
            end = end.max(it.off + it.size * it.occurs.as_ref().map_or(1, |o| o.0));
        }
        self.total = end;
        Ok(())
    }

    fn add_decl(
        &mut self,
        d: &DataDecl,
        parent: Option<usize>,
        start: u32,
        inherited_sign: Option<SignClause>,
    ) -> GenResult<usize> {
        let line = d.span.line;
        let sign = d.sign.or(inherited_sign);
        let idx = self.items.len();
        let name = d.name.as_ref().map(|n| n.to_ascii_uppercase());
        // Placeholder, filled in below once the size is known.
        self.items.push(Item {
            name: name.clone(),
            level: d.level,
            parent,
            children: vec![],
            off: start,
            size: 0,
            occurs: d.occurs.as_ref().map(|o| (o.max.max(1), o.depending_on.as_ref().map(|s| s.to_ascii_uppercase()))),
            fd: Fd::alnum(0),
            is_group: false,
            redefines: None,
            value: d.value.clone(),
            cond: vec![],
            renames: None,
            span_line: line,
            hidden: false,
            external: d.is_external,
            global: d.is_global,
            pic_template: d.picture.as_ref().map(|p| p.template.clone()),
            pic_kind: d.picture.as_ref().map(|p| p.kind),
            usage: d.usage,
            blank_when_zero: d.blank_when_zero,
            index_names: d.occurs.as_ref().map(|o| o.indexed_by.iter().map(|n| n.to_ascii_uppercase()).collect()).unwrap_or_default(),
            keys: d.occurs.as_ref().map(|o| o.keys.iter().map(|(k, a)| (k.to_ascii_uppercase(), *a)).collect()).unwrap_or_default(),
        });
        if let Some(n) = &name {
            self.by_name.entry(n.clone()).or_default().push(idx);
        }

        let storage_children: Vec<&DataDecl> =
            d.children.iter().filter(|c| c.level != 88 && c.level != 66).collect();
        if d.picture.is_none() && !storage_children.is_empty() {
            // A group.
            let mut cursor = start;
            let mut group_end = start;
            let mut last_by_name: HashMap<String, usize> = HashMap::new();
            for c in &storage_children {
                let (child_start, redef) = match &c.redefines {
                    Some(target) => {
                        let t = target.to_ascii_uppercase();
                        let ti = *last_by_name.get(&t).ok_or_else(|| {
                            GenError::new(c.span.line, format!("REDEFINES {target}: no such item at the same level"))
                        })?;
                        (self.items[ti].off, Some(ti))
                    }
                    None => (cursor, None),
                };
                let ci = self.add_decl(c, Some(idx), child_start, sign)?;
                self.items[ci].redefines = redef;
                self.items[idx].children.push(ci);
                if let Some(n) = &self.items[ci].name {
                    // A redefinition chain starts at its first member.
                    if redef.is_none() {
                        last_by_name.insert(n.clone(), ci);
                    }
                }
                let it = &self.items[ci];
                let end = it.off + it.size * it.occurs.as_ref().map_or(1, |o| o.0);
                if redef.is_none() {
                    cursor = end;
                }
                group_end = group_end.max(end);
            }
            self.items[idx].is_group = true;
            self.items[idx].size = group_end - start;
            self.items[idx].fd = Fd::group(group_end - start);
        } else {
            let fd = item_fd(d, sign)?;
            self.items[idx].size = fd.len;
            self.items[idx].fd = fd;
        }
        // 88-level condition-names under this item.
        for c in d.children.iter().filter(|c| c.level == 88) {
            let ci = self.items.len();
            self.items.push(Item {
                name: c.name.as_ref().map(|n| n.to_ascii_uppercase()),
                level: 88,
                parent: Some(idx),
                children: vec![],
                off: 0,
                size: 0,
                occurs: None,
                fd: Fd::alnum(0),
                is_group: false,
                redefines: None,
                value: None,
                cond: c.condition_values.clone(),
                renames: None,
                span_line: c.span.line,
                hidden: false,
                external: false,
                global: false,
                pic_template: None,
                pic_kind: None,
                usage: Usage::Display,
                blank_when_zero: false,
                index_names: vec![],
                keys: vec![],
            });
            if let Some(n) = &self.items[ci].name {
                self.by_name.entry(n.clone()).or_default().push(ci);
            }
        }
        // INDEXED BY names become hidden four-byte occurrence-number items,
        // placed after everything declared (see `finish`).
        if let Some(o) = &d.occurs {
            for n in &o.indexed_by {
                let up = n.to_ascii_uppercase();
                self.index_names.entry(up).or_insert(usize::MAX);
            }
        }
        Ok(idx)
    }

    /// Allocate the hidden items (index-names) and round the region up.
    pub fn finish(&mut self) {
        let names: Vec<String> = self.index_names.keys().cloned().collect();
        let mut names = names;
        names.sort();
        for n in names {
            let off = self.total;
            let idx = self.items.len();
            self.items.push(Item {
                name: Some(n.clone()),
                level: 77,
                parent: None,
                children: vec![],
                off,
                size: 4,
                occurs: None,
                fd: Fd { kind: K_COMP5, flags: F_SIGNED, digits: 9, scale: 0, len: 4, aux: 0 },
                is_group: false,
                redefines: None,
                value: None,
                cond: vec![],
                renames: None,
                span_line: 0,
                hidden: true,
                external: false,
                global: false,
                pic_template: None,
                pic_kind: None,
                usage: Usage::Comp5,
                blank_when_zero: false,
                index_names: vec![],
                keys: vec![],
            });
            self.by_name.entry(n.clone()).or_default().push(idx);
            self.index_names.insert(n, idx);
            self.total += 4;
        }
        self.total = (self.total + 7) & !7;
    }

    /// Reserve `size` hidden bytes at the end of the region; returns the offset.
    pub fn alloc_hidden(&mut self, size: u32) -> u32 {
        let off = self.total;
        self.total = (self.total + size + 7) & !7;
        off
    }

    /// The items bearing `name`.
    pub fn named(&self, name: &str) -> &[usize] {
        self.by_name.get(&name.to_ascii_uppercase()).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Resolve `name OF q1 OF q2 …` (qualifiers innermost first, as written).
    pub fn lookup(&self, name: &str, quals: &[String]) -> Result<usize, String> {
        let cands = self.named(name);
        let mut hits: Vec<usize> = Vec::new();
        'c: for &c in cands {
            // Every qualifier must name an ancestor, in order outward.
            let mut cur = self.items[c].parent;
            for q in quals {
                let q = q.to_ascii_uppercase();
                loop {
                    match cur {
                        Some(p) => {
                            cur = self.items[p].parent;
                            if self.items[p].name.as_deref() == Some(q.as_str()) {
                                break;
                            }
                        }
                        None => continue 'c,
                    }
                }
            }
            hits.push(c);
        }
        match hits.len() {
            0 => Err(format!("{name} is not defined")),
            1 => Ok(hits[0]),
            _ => Err(format!("{name} is ambiguous; qualify it with OF")),
        }
    }

    /// The `OCCURS` levels above or at `idx`, outermost first, as
    /// `(stride, max, depending-on name)`.
    pub fn occurs_chain(&self, idx: usize) -> Vec<(u32, u32, Option<String>)> {
        let mut chain = Vec::new();
        let mut cur = Some(idx);
        while let Some(i) = cur {
            if let Some((max, odo)) = &self.items[i].occurs {
                chain.push((self.items[i].size, *max, odo.clone()));
            }
            cur = self.items[i].parent;
        }
        chain.reverse();
        chain
    }
}

impl Default for Layout {
    fn default() -> Self {
        Layout::new()
    }
}

/// The descriptor of an elementary item.
fn item_fd(d: &DataDecl, sign: Option<SignClause>) -> GenResult<Fd> {
    let line = d.span.line;
    let mut flags = 0u8;
    if d.justified {
        flags |= F_JUSTIFIED;
    }
    if d.blank_when_zero {
        flags |= F_BLANK_ZERO;
    }
    let Some(p) = d.picture.as_ref() else {
        return Ok(match d.usage {
            Usage::Comp1 => Fd { kind: K_FLOAT32, flags: F_SIGNED, digits: 0, scale: 0, len: 4, aux: 0 },
            Usage::Comp2 => Fd { kind: K_FLOAT64, flags: F_SIGNED, digits: 0, scale: 0, len: 8, aux: 0 },
            Usage::Index | Usage::Pointer | Usage::ObjectReference => {
                Fd { kind: K_INDEX, flags: 0, digits: 9, scale: 0, len: 4, aux: 0 }
            }
            _ => return Err(GenError::new(line, "an elementary item needs a PICTURE")),
        });
    };
    let pic = pic::parse(&p.template, p.kind);
    Ok(match p.kind {
        PicKind::Numeric => {
            if pic.digits > 38 {
                return Err(GenError::new(line, format!("PICTURE {} has more than 38 digits", p.template)));
            }
            let mut f = flags | if pic.signed { F_SIGNED } else { 0 };
            let scale = pic.scale().clamp(-100, 100) as i8;
            match d.usage {
                Usage::Display => {
                    if let Some(s) = sign {
                        if s.leading {
                            f |= F_SIGN_LEADING;
                        }
                        if s.separate {
                            f |= F_SIGN_SEPARATE;
                        }
                    }
                    let sep = pic.signed && f & F_SIGN_SEPARATE != 0;
                    Fd { kind: K_NUM_DISPLAY, flags: f, digits: pic.digits as u8, scale, len: pic.digits + sep as u32, aux: 0 }
                }
                Usage::Binary | Usage::Comp | Usage::Comp5 => {
                    let len = match pic.digits {
                        0..=4 => 2,
                        5..=9 => 4,
                        10..=18 => 8,
                        _ => 16,
                    };
                    let kind = if d.usage == Usage::Comp5 { K_COMP5 } else { K_BINARY };
                    Fd { kind, flags: f, digits: pic.digits as u8, scale, len, aux: 0 }
                }
                Usage::Comp3 | Usage::PackedDecimal => {
                    Fd { kind: K_PACKED, flags: f, digits: pic.digits as u8, scale, len: pic.digits / 2 + 1, aux: 0 }
                }
                Usage::Comp1 => Fd { kind: K_FLOAT32, flags: F_SIGNED, digits: 0, scale: 0, len: 4, aux: 0 },
                Usage::Comp2 => Fd { kind: K_FLOAT64, flags: F_SIGNED, digits: 0, scale: 0, len: 8, aux: 0 },
                Usage::Index | Usage::Pointer | Usage::ObjectReference => {
                    Fd { kind: K_INDEX, flags: 0, digits: 9, scale: 0, len: 4, aux: 0 }
                }
                other => {
                    return Err(GenError::new(line, format!("USAGE {other:?} is not supported in a WebAssembly module")))
                }
            }
        }
        PicKind::Alphanumeric => Fd { kind: K_ALNUM, flags, digits: 0, scale: 0, len: pic.chars, aux: 0 },
        PicKind::Alphabetic => Fd { kind: K_ALNUM, flags: flags | F_ALPHABETIC, digits: 0, scale: 0, len: pic.chars, aux: 0 },
        PicKind::NumericEdited => Fd { kind: K_NUM_EDITED, flags, digits: 0, scale: 0, len: pic.chars, aux: 0 },
        PicKind::AlphanumericEdited => Fd { kind: K_ALNUM_EDITED, flags, digits: 0, scale: 0, len: pic.chars, aux: 0 },
        PicKind::National => Fd { kind: K_NATIONAL, flags, digits: 0, scale: 0, len: pic.chars * 2, aux: 0 },
        PicKind::Utf8 => {
            let len = d.byte_length.unwrap_or(pic.chars * 4);
            Fd { kind: K_ALNUM, flags, digits: 0, scale: 0, len, aux: 0 }
        }
    })
}
