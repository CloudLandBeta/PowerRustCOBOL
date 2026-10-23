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
    /// The program unit that declared it (`u32::MAX` for hidden items).
    pub unit: u32,
    /// A LINKAGE item: the region offset of the 4-byte slot holding the address
    /// of the caller's record this item lies in. `off` is then relative to
    /// that record.
    pub link: Option<u32>,
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
    /// Index-names (`INDEXED BY`): `"unit:NAME"` -> hidden item.
    pub index_names: HashMap<String, usize>,
    /// The program unit being laid out or lowered.
    pub unit: u32,
    /// Units whose items a name may denote, innermost first: the current unit,
    /// then the units that contain it (whose `GLOBAL` items it sees).
    pub scope: Vec<u32>,
    /// `EXTERNAL` items: name -> (offset, size) of the one storage they share.
    externals: HashMap<String, (u32, u32)>,
    /// Per unit: the region range its declared storage occupies.
    pub unit_ranges: Vec<(u32, u32)>,
}

impl Layout {
    pub fn new() -> Layout {
        Layout {
            items: Vec::new(),
            by_name: HashMap::new(),
            roots: Vec::new(),
            total: 0,
            index_names: HashMap::new(),
            unit: 0,
            scope: vec![0],
            externals: HashMap::new(),
            unit_ranges: Vec::new(),
        }
    }

    /// Start laying out unit `u`. Its storage begins where the last one ended.
    pub fn begin_unit(&mut self, u: u32, scope: Vec<u32>) {
        self.unit = u;
        self.scope = scope;
        while self.unit_ranges.len() <= u as usize {
            self.unit_ranges.push((self.total, self.total));
        }
        self.unit_ranges[u as usize] = (self.total, self.total);
    }

    /// The unit's declared storage ends here.
    pub fn end_unit(&mut self) {
        self.total = (self.total + 7) & !7;
        let u = self.unit as usize;
        self.unit_ranges[u].1 = self.total;
    }

    /// Select the names visible to unit `u` (while lowering it).
    pub fn set_scope(&mut self, u: u32, scope: Vec<u32>) {
        self.unit = u;
        self.scope = scope;
    }

    /// Does the current scope see item `i`? Its own unit's items, the `GLOBAL`
    /// items of the units containing it, and hidden items.
    fn visible(&self, i: usize) -> bool {
        let it = &self.items[i];
        it.unit == u32::MAX || it.unit == self.unit || (self.scope.contains(&it.unit) && self.is_global(i))
    }

    /// Is the item `GLOBAL`, itself or through a record or group above it?
    fn is_global(&self, i: usize) -> bool {
        let mut cur = Some(i);
        while let Some(c) = cur {
            if self.items[c].global {
                return true;
            }
            cur = self.items[c].parent;
        }
        false
    }

    /// The hidden item behind an `INDEXED BY` name of the current unit (or of a
    /// unit containing it).
    pub fn index_item(&self, name: &str) -> Option<usize> {
        for u in &self.scope {
            if let Some(&i) = self.index_names.get(&format!("{u}:{name}")) {
                return Some(i);
            }
        }
        None
    }

    /// Where item `i` ends, every occurrence counted; an item that would end
    /// past what a 32-bit WebAssembly memory addresses is refused by name
    /// (WASM-017 R7: data that cannot be sized is a compile error).
    fn end_of(&self, i: usize) -> GenResult<u32> {
        let it = &self.items[i];
        let n = u64::from(it.occurs.as_ref().map_or(1, |o| o.0));
        let end = u64::from(it.off) + u64::from(it.size) * n;
        u32::try_from(end).map_err(|_| {
            let name = it.name.as_deref().unwrap_or("FILLER");
            GenError::kinded(
                it.span_line,
                crate::error::ErrorKind::Other,
                name,
                format!("{name}: {} bytes × {n} occurrences do not fit in a WebAssembly memory (4 GiB)", it.size),
            )
        })
    }

    /// Lay out one section's declarations after what is already there.
    pub fn add_section(&mut self, decls: &[DataDecl]) -> GenResult<()> {
        let first_root = self.roots.len();
        for d in decls {
            if d.level == 66 || d.level == 88 {
                continue; // handled after the records they refer to
            }
            // A level-01 REDEFINES shares the storage of the record it names.
            // An EXTERNAL item is one storage for every unit that declares it.
            let external_at = if d.is_external { d.name.as_ref().and_then(|n| self.externals.get(&n.to_ascii_uppercase()).copied()) } else { None };
            let (start, redef) = match &d.redefines {
                _ if external_at.is_some() => (external_at.unwrap().0, None),
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
            let idx = self.add_decl(d, None, start, None, Usage::Display)?;
            self.items[idx].redefines = redef;
            self.roots.push(idx);
            let end = self.end_of(idx)?;
            let it = &self.items[idx];
            let (it_off, it_len) = (it.off, end - it.off);
            if d.is_external {
                self.mark_external(idx);
            }
            if d.is_external && external_at.is_none() {
                if let Some(n) = &d.name {
                    self.externals.insert(n.to_ascii_uppercase(), (it_off, it_len));
                }
            }
            // Storage shared with an earlier EXTERNAL declaration adds none.
            if external_at.is_none() {
                self.total = self.total.max(end);
            }
        }
        // 66-level items come after their records. A 66 belongs to the record
        // it follows, which is what a qualifier (`RENAME-5 OF T-RENAMES-DATA`)
        // names.
        let mut seen = 0usize;
        for d in decls {
            if d.level != 66 && d.level != 88 {
                seen += 1;
            }
            if d.level == 66 {
                let owner = if seen > 0 { self.roots.get(first_root + seen - 1).copied() } else { None };
                if let Some(r) = &d.renames {
                    self.push_renames(d, r, owner);
                }
            }
        }
        Ok(())
    }

    /// Flag an item and everything below it as EXTERNAL (shared storage the
    /// image does not initialise).
    fn mark_external(&mut self, i: usize) {
        self.items[i].external = true;
        let kids = self.items[i].children.clone();
        for k in kids {
            self.mark_external(k);
        }
    }

    /// A 66-level item: a character view over the items it renames, resolved
    /// in [`Layout::finish`]. It belongs to the record it follows.
    fn push_renames(&mut self, d: &DataDecl, r: &cobolt_ast::data::RenamesClause, owner: Option<usize>) {
        let idx = self.items.len();
        self.items.push(Item {
            name: d.name.as_ref().map(|n| n.to_ascii_uppercase()),
            level: 66,
            parent: owner,
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
            unit: self.unit,
            link: owner.and_then(|o| self.items[o].link),
        });
        if let Some(n) = &self.items[idx].name {
            self.by_name.entry(n.clone()).or_default().push(idx);
        }
    }

    /// The LINKAGE SECTION: records with no storage of their own. Each 01/77
    /// gets a four-byte slot in the region that a `CALL` fills with the address
    /// of the argument; the items below are laid out from offset 0 of the
    /// record, and every reference reaches them through the slot.
    pub fn add_linkage(&mut self, decls: &[DataDecl]) -> GenResult<()> {
        let first_root = self.roots.len();
        let mut seen = 0usize;
        for d in decls {
            if d.level == 66 || d.level == 88 {
                continue;
            }
            seen += 1;
            // A level-01 REDEFINES shares the record, and so the slot, of the
            // record it names.
            let shared = d.redefines.as_ref().and_then(|t| {
                let up = t.to_ascii_uppercase();
                self.roots
                    .iter()
                    .rev()
                    .copied()
                    .find(|&r| self.items[r].name.as_deref() == Some(up.as_str()) && self.items[r].unit == self.unit)
                    .and_then(|r| self.items[r].link.map(|l| (r, l)))
            });
            let slot = match shared {
                Some((_, l)) => l,
                None => self.alloc_hidden(4),
            };
            let idx = self.add_decl(d, None, 0, None, Usage::Display)?;
            if let Some((r, _)) = shared {
                self.items[idx].redefines = Some(r);
            }
            self.roots.push(idx);
            // The record and everything under it reach their storage through the slot.
            let mut stack = vec![idx];
            while let Some(i) = stack.pop() {
                self.items[i].link = Some(slot);
                stack.extend(self.items[i].children.iter().copied());
            }
            // Conditions under the record's items carry the link too (they read the host).
            let all: Vec<usize> = (0..self.items.len()).filter(|&k| self.items[k].level == 88).collect();
            for k in all {
                if let Some(p) = self.items[k].parent {
                    if self.items[p].link.is_some() && self.items[k].link.is_none() {
                        self.items[k].link = self.items[p].link;
                    }
                }
            }
        }
        // 66 and 88 under linkage records: handled as in WORKING-STORAGE (88s above;
        // a 66 over linkage items resolves in `finish`).
        let _ = (first_root, seen);
        for d in decls {
            if d.level == 66 {
                if let Some(r) = &d.renames {
                    let owner = if seen > 0 { self.roots.last().copied() } else { None };
                    self.push_renames(d, r, owner);
                }
            }
        }
        Ok(())
    }

    /// Lay out the records of one file description: they share ONE record area
    /// (COBOL-85: the records of an FD implicitly redefine each other), so each
    /// starts where the first does and the area is the longest of them.
    pub fn add_file_records(&mut self, decls: &[DataDecl]) -> GenResult<()> {
        self.add_file_records_of("", false, decls)
    }

    /// As [`Layout::add_file_records`] for the FD called `fd_name`; an
    /// `EXTERNAL` FD's record area is one storage for every unit that declares
    /// it.
    pub fn add_file_records_of(&mut self, fd_name: &str, external: bool, decls: &[DataDecl]) -> GenResult<()> {
        let key = format!("FD:{}", fd_name.to_ascii_uppercase());
        let shared = if external { self.externals.get(&key).copied() } else { None };
        let start = match shared {
            Some((off, _)) => off,
            None => self.total,
        };
        let mut first: Option<usize> = None;
        let mut end = start;
        for d in decls {
            if d.level == 66 || d.level == 88 {
                continue;
            }
            let idx = self.add_decl(d, None, start, None, Usage::Display)?;
            match first {
                None => first = Some(idx),
                Some(f) => self.items[idx].redefines = Some(f),
            }
            self.roots.push(idx);
            end = end.max(self.end_of(idx)?);
        }
        if external {
            // The records of an EXTERNAL FD are never initialised by the image.
            let mine: Vec<usize> = self.roots.iter().copied().filter(|&r| r >= first.unwrap_or(usize::MAX)).collect();
            for r in mine {
                self.mark_external(r);
            }
        }
        if shared.is_none() {
            if external {
                self.externals.insert(key, (start, end - start));
            }
            self.total = end;
        }
        Ok(())
    }

    fn add_decl(
        &mut self,
        d: &DataDecl,
        parent: Option<usize>,
        start: u32,
        inherited_sign: Option<SignClause>,
        inherited_usage: Usage,
    ) -> GenResult<usize> {
        // A USAGE written on a group applies to every elementary item below it.
        let usage = if d.usage != Usage::Display { d.usage } else { inherited_usage };
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
            usage,
            blank_when_zero: d.blank_when_zero,
            index_names: d.occurs.as_ref().map(|o| o.indexed_by.iter().map(|n| n.to_ascii_uppercase()).collect()).unwrap_or_default(),
            keys: d.occurs.as_ref().map(|o| o.keys.iter().map(|(k, a)| (k.to_ascii_uppercase(), *a)).collect()).unwrap_or_default(),
            unit: self.unit,
            link: None,
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
                let ci = self.add_decl(c, Some(idx), child_start, sign, usage)?;
                self.items[ci].redefines = redef;
                self.items[idx].children.push(ci);
                if let Some(n) = &self.items[ci].name {
                    // A redefinition chain starts at its first member.
                    if redef.is_none() {
                        last_by_name.insert(n.clone(), ci);
                    }
                }
                let end = self.end_of(ci)?;
                if redef.is_none() {
                    cursor = end;
                }
                group_end = group_end.max(end);
            }
            self.items[idx].is_group = true;
            self.items[idx].size = group_end - start;
            self.items[idx].fd = Fd::group(group_end - start);
        } else {
            let fd = item_fd(d, sign, usage)?;
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
                unit: self.unit,
                link: None,
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
                self.index_names.entry(format!("{}:{}", self.unit, up)).or_insert(usize::MAX);
            }
        }
        Ok(idx)
    }

    /// Allocate the hidden items (index-names) and round the region up.
    pub fn finish(&mut self) {
        // 66 RENAMES: a character view over the records from `from` through `thru`.
        let renames: Vec<usize> = (0..self.items.len()).filter(|&i| self.items[i].renames.is_some()).collect();
        for r in renames {
            let (from, thru) = self.items[r].renames.clone().unwrap();
            let owner = self.items[r].parent;
            let fi = self.lookup_within(&from, owner);
            let ti = match &thru {
                Some(t) => self.lookup_within(t, owner),
                None => fi,
            };
            if let (Some(f), Some(t)) = (fi, ti) {
                let start = self.items[f].off;
                let end = self.items[t].off + self.items[t].size * self.items[t].occurs.as_ref().map_or(1, |o| o.0);
                let len = end.saturating_sub(start);
                self.items[r].off = start;
                self.items[r].size = len;
                self.items[r].fd = Fd::alnum(len);
                // `RENAMES a` alone renames one item and takes its description:
                // a numeric-edited item stays one, so a MOVE into the new name
                // edits (NC252A RENAM-TEST-15).
                if thru.is_none() && self.items[f].occurs.is_none() {
                    let src = self.items[f].clone();
                    let it = &mut self.items[r];
                    it.fd = src.fd;
                    it.is_group = src.is_group;
                    it.pic_template = src.pic_template;
                    it.pic_kind = src.pic_kind;
                    it.usage = src.usage;
                    it.blank_when_zero = src.blank_when_zero;
                }
            }
        }
        let names: Vec<String> = self.index_names.keys().cloned().collect();
        let mut names = names;
        names.sort();
        for key in names {
            let (unit, n) = match key.split_once(':') {
                Some((u, n)) => (u.parse::<u32>().unwrap_or(0), n.to_string()),
                None => (0, key.clone()),
            };
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
                unit,
                link: None,
            });
            self.by_name.entry(n.clone()).or_default().push(idx);
            self.index_names.insert(key, idx);
            self.total += 4;
        }
        self.total = (self.total + 7) & !7;
    }

    /// An item the program used without declaring it (a conformance build
    /// tolerates the CCVS85 X-cards, as the interpreter's
    /// `COBOL_TOLERATE_UNDECLARED` does): thirty-two characters, spaces.
    pub fn add_implicit(&mut self, name: &str) -> usize {
        let up = name.to_ascii_uppercase();
        let off = self.total;
        let idx = self.items.len();
        self.items.push(Item {
            name: Some(up.clone()),
            level: 77,
            parent: None,
            children: vec![],
            off,
            size: 32,
            occurs: None,
            fd: Fd::alnum(32),
            is_group: false,
            redefines: None,
            value: None,
            cond: vec![],
            renames: None,
            span_line: 0,
            hidden: false,
            external: false,
            global: false,
            pic_template: None,
            pic_kind: None,
            usage: Usage::Display,
            blank_when_zero: false,
            index_names: vec![],
            keys: vec![],
            unit: self.unit,
            link: None,
        });
        self.by_name.entry(up).or_default().push(idx);
        self.total = (self.total + 32 + 7) & !7;
        idx
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
            if !self.visible(c) {
                continue;
            }
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
        // An item of the current unit hides one of the same name that a
        // containing unit declared GLOBAL, and so on outward.
        if hits.len() > 1 {
            for u in self.scope.clone() {
                let here: Vec<usize> = hits.iter().copied().filter(|&h| self.items[h].unit == u).collect();
                if !here.is_empty() {
                    hits = here;
                    break;
                }
            }
        }
        // Two condition-names of one spelling (a SPECIAL-NAMES switch status and an
        // 88-level the program also declares): the later declaration wins, as it
        // does in the interpreter's table.
        if hits.len() > 1 && hits.iter().all(|&h| self.items[h].level == 88) {
            return Ok(*hits.last().unwrap());
        }
        match hits.len() {
            0 => Err(format!("{name} is not defined")),
            1 => Ok(hits[0]),
            _ => Err(format!("{name} is ambiguous; qualify it with OF")),
        }
    }

    /// A name as a `RENAMES` clause reads it: among the items of the record the
    /// 66 follows when that finds exactly one, else anywhere.
    fn lookup_within(&self, name: &str, owner: Option<usize>) -> Option<usize> {
        if let Some(o) = owner {
            let inside: Vec<usize> = self
                .named(name)
                .iter()
                .copied()
                .filter(|&c| self.visible(c))
                .filter(|&c| {
                    let mut cur = self.items[c].parent;
                    while let Some(p) = cur {
                        if p == o {
                            return true;
                        }
                        cur = self.items[p].parent;
                    }
                    c == o
                })
                .collect();
            if inside.len() == 1 {
                return Some(inside[0]);
            }
        }
        self.lookup(name, &[]).ok()
    }

    /// The variable-length table inside group `idx`, if there is one, as
    /// `(stride, max, DEPENDING ON name)`. A record holds at most one.
    pub fn odo_inside(&self, idx: usize) -> Option<(u32, u32, String)> {
        for &c in &self.items[idx].children {
            if let Some((max, Some(name))) = &self.items[c].occurs {
                return Some((self.items[c].size, *max, name.clone()));
            }
            if let Some(found) = self.odo_inside(c) {
                return Some(found);
            }
        }
        None
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
/// The item's name as a refusal names it: upper-case, `FILLER` when it has none.
fn decl_name(d: &DataDecl) -> String {
    d.name.as_deref().map_or_else(|| "FILLER".to_owned(), str::to_ascii_uppercase)
}

fn item_fd(d: &DataDecl, sign: Option<SignClause>, usage: Usage) -> GenResult<Fd> {
    let line = d.span.line;
    let mut flags = 0u8;
    if d.justified {
        flags |= F_JUSTIFIED;
    }
    if d.blank_when_zero {
        flags |= F_BLANK_ZERO;
    }
    let Some(p) = d.picture.as_ref() else {
        return Ok(match usage {
            Usage::Comp1 => Fd { kind: K_FLOAT32, flags: F_SIGNED, digits: 0, scale: 0, len: 4, aux: 0 },
            Usage::Comp2 => Fd { kind: K_FLOAT64, flags: F_SIGNED, digits: 0, scale: 0, len: 8, aux: 0 },
            Usage::Index | Usage::Pointer | Usage::ObjectReference => {
                Fd { kind: K_INDEX, flags: 0, digits: 9, scale: 0, len: 4, aux: 0 }
            }
            _ => return Err(GenError::new(line, format!("{}: an elementary item needs a PICTURE", decl_name(d)))),
        });
    };
    let pic = pic::parse(&p.template, p.kind);
    Ok(match p.kind {
        PicKind::Numeric => {
            if pic.digits > 38 {
                return Err(GenError::new(line, format!("{}: PICTURE {} has more than 38 digits", decl_name(d), p.template)));
            }
            let mut f = flags | if pic.signed { F_SIGNED } else { 0 };
            let scale = pic.scale().clamp(-100, 100) as i8;
            match usage {
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
                    let kind = if usage == Usage::Comp5 { K_COMP5 } else { K_BINARY };
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
