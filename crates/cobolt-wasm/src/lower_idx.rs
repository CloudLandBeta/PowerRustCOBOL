// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! INDEXED files held in memory and `SORT` work files (WASM-010 R7, Q8): the
//! verbs lowered to the routines of `cobolt_wasm_rt::idx`, which keep the
//! records in the module's file heap.
//!
//! What a statement decides at compile time is decided here the way the
//! interpreter decides it at run time (`interpreter.rs`, `exec_read` and
//! friends): whether a `READ` is random or sequential, which key a `READ … KEY`
//! or `START … KEY` names (`key_of_reference`), and what value it searches for
//! (`declared_key_value`). Only the first record description of the FD
//! describes the keys, as the interpreter's `FileSpec::layout` does.

use crate::error::{ErrorKind, GenError, GenResult};
use crate::lower::{FileInfo, FileKind, Lower};
use crate::lower_call::Cover;
use cobolt_ast::expr::{CmpOp, Expr};
use cobolt_ast::program::{AccessMode, FileControl};
use cobolt_ast::stmt::{OpenMode, ReadDirection, SortKey, Stmt};
use cobolt_wasm_rt::idx;
use std::collections::{HashMap, HashSet};
use wasm_encoder::Instruction as I;

/// One key of an in-memory file.
#[derive(Debug, Clone)]
pub(crate) struct KeyMeta {
    /// The data-name the SELECT gave it, upper case (empty for a file with no
    /// RECORD KEY, whose key is the whole record).
    pub name: String,
    pub quals: Vec<String>,
    /// The fields of a split key, as declared (empty for an ordinary key).
    pub declared_parts: Vec<(String, Vec<String>)>,
    /// `(offset in the record, length)` of each part, in key order.
    pub parts: Vec<(u32, u32)>,
    pub dup: bool,
}

impl KeyMeta {
    fn off(&self) -> u32 {
        self.parts.first().map_or(0, |p| p.0)
    }
    fn len(&self) -> u32 {
        self.parts.iter().map(|p| p.1).sum()
    }
}

/// What the generator knows of an in-memory file or a SORT work file.
#[derive(Debug, Clone)]
pub(crate) struct FileMeta {
    /// The first record description's item, region offset and length.
    pub record: usize,
    pub area_off: u32,
    pub rec_len: u32,
    pub access: AccessMode,
    /// The RECORD KEY, then the alternate keys.
    pub keys: Vec<KeyMeta>,
    /// SORT: the second record area and the 16 bytes of work-file state.
    pub b_off: u32,
    pub state_off: u32,
}

/// `name` and `quals` agree with `want_quals` when one chain is a subsequence
/// of the other (the interpreter's `key_of_reference::agrees`).
fn agrees(a: &[String], b: &[String]) -> bool {
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let mut it = long.iter();
    short.iter().all(|s| it.any(|l| l == s))
}

fn up(s: &str) -> String {
    s.to_ascii_uppercase()
}

impl Lower<'_, '_> {
    /// The files a `SORT`, `MERGE`, `RELEASE` or `RETURN` of the unit names: its
    /// SORT work files.
    pub(crate) fn sort_file_names(
        &self,
        records: &HashMap<String, Vec<String>>,
    ) -> HashSet<String> {
        let mut out = HashSet::new();
        for p in &self.paras {
            for s in p.stmts {
                s.walk(&mut |s| match s {
                    Stmt::Sort { file, .. }
                    | Stmt::Merge { file, .. }
                    | Stmt::Return { file, .. } => {
                        out.insert(up(file));
                    }
                    Stmt::Release { record, .. } => {
                        let base = match record {
                            Expr::Subscript { base, .. } => base.as_ref(),
                            other => other,
                        };
                        if let Expr::Identifier(n, _) | Expr::Qualified { name: n, .. } = base {
                            let n = up(n);
                            if let Some((f, _)) = records.iter().find(|(_, rs)| rs.contains(&n)) {
                                out.insert(f.clone());
                            }
                        }
                    }
                    _ => {}
                });
            }
        }
        out
    }

    /// The first record description of a file: its item, offset and length.
    fn first_record(&self, fc_name: &str, rec_items: &[usize]) -> GenResult<(usize, u32, u32)> {
        let Some(&first) = rec_items.first() else {
            return Err(GenError::new(
                0,
                format!("file {fc_name} has no record description"),
            ));
        };
        let it = &self.layout.items[first];
        Ok((first, it.off, it.size.max(1)))
    }

    /// The item `name OF quals` when it lies in the record `record`: its region
    /// offset and length.
    fn field_in_record(&self, record: usize, name: &str, quals: &[String]) -> Option<(u32, u32)> {
        let i = self.layout.lookup(name, quals).ok()?;
        let mut root = i;
        while let Some(p) = self.layout.items[root].parent {
            root = p;
        }
        if root != record || self.layout.items[i].link.is_some() {
            return None;
        }
        let it = &self.layout.items[i];
        Some((it.off, it.size * it.occurs.as_ref().map_or(1, |o| o.0)))
    }

    /// A key the SELECT declares, as offsets in the record.
    fn key_meta(
        &self,
        record: usize,
        area_off: u32,
        name: &str,
        quals: &[String],
        parts: &[(String, Vec<String>)],
        dup: bool,
    ) -> Option<KeyMeta> {
        let mut out = Vec::new();
        if parts.is_empty() {
            let (off, len) = self.field_in_record(record, name, quals)?;
            out.push((off - area_off, len));
        } else {
            for (n, q) in parts {
                let (off, len) = self.field_in_record(record, n, q)?;
                out.push((off - area_off, len));
            }
        }
        Some(KeyMeta {
            name: up(name),
            quals: quals.iter().map(|q| up(q)).collect(),
            declared_parts: parts.to_vec(),
            parts: out,
            dup,
        })
    }

    /// Lay out an in-memory INDEXED file: its control block in the region, its
    /// descriptor and key table among the constants.
    pub(crate) fn build_memory_file(
        &mut self,
        fc: &FileControl,
        rec_items: &[usize],
    ) -> GenResult<FileInfo> {
        let (record, area_off, rec_len) = self.first_record(&fc.name, rec_items)?;
        let mut keys = Vec::new();
        let rk_parts: Vec<(String, Vec<String>)> = fc
            .record_key_parts
            .iter()
            .map(|p| (p.name.clone(), p.quals.clone()))
            .collect();
        // No RECORD KEY (or one outside the record): the whole record is the key.
        let primary = fc
            .record_key
            .as_deref()
            .and_then(|k| {
                self.key_meta(record, area_off, k, &fc.record_key_quals, &rk_parts, false)
            })
            .unwrap_or(KeyMeta {
                name: String::new(),
                quals: vec![],
                declared_parts: vec![],
                parts: vec![(0, rec_len)],
                dup: false,
            });
        keys.push(primary);
        // An alternate key whose field is not in the record is skipped, as the
        // interpreter's engine skips it.
        for ak in &fc.alternate_keys {
            let parts: Vec<(String, Vec<String>)> = ak
                .parts
                .iter()
                .map(|p| (p.name.clone(), p.quals.clone()))
                .collect();
            if let Some(k) = self.key_meta(
                record,
                area_off,
                &ak.field,
                &ak.quals,
                &parts,
                ak.with_duplicates,
            ) {
                keys.push(k);
            }
        }
        let status_opd = match &fc.file_status {
            Some(sn) => match self.layout.lookup(sn, &[]) {
                Ok(i) => self.item_operand(i, &[], None, fc.span.line)?,
                Err(_) => 0,
            },
            None => 0,
        };
        let pk_len = keys[0].len();
        let fcb_off = self
            .layout
            .alloc_hidden(idx::fcb_size(keys.len() as u32, pk_len));
        let mut table: Vec<u32> = Vec::new();
        for k in &keys {
            let parts: Vec<u32> = k.parts.iter().flat_map(|&(o, l)| [o, l]).collect();
            let parts_addr = self.pool.spec(&parts);
            table.extend_from_slice(&[k.dup as u32, k.len(), k.parts.len() as u32, parts_addr]);
        }
        let keys_addr = self.pool.spec(&table);
        let flags = fc.optional as u32 | ((fc.access == AccessMode::Sequential) as u32) << 1;
        let desc = self.pool.spec(&[
            rec_len,
            area_off,
            status_opd,
            fcb_off,
            flags,
            keys.len() as u32,
            keys_addr,
        ]);
        self.uses_heap = true;
        self.file_meta.push(FileMeta {
            record,
            area_off,
            rec_len,
            access: fc.access,
            keys,
            b_off: 0,
            state_off: 0,
        });
        Ok(FileInfo {
            desc,
            handle_off: fcb_off,
            rec_item: Some(record),
            kind: FileKind::Memory,
            meta: self.file_meta.len() - 1,
        })
    }

    /// Lay out a SORT work file: a second record area for the comparisons and
    /// the work file's state, both in the region.
    pub(crate) fn build_sort_file(
        &mut self,
        name: &str,
        rec_items: &[usize],
    ) -> GenResult<FileInfo> {
        let (record, area_off, rec_len) = self.first_record(name, rec_items)?;
        let b_off = self.layout.alloc_hidden(rec_len);
        let state_off = self.layout.alloc_hidden(16);
        let desc = self.pool.spec(&[rec_len, area_off, b_off, state_off, 0, 0]);
        self.uses_heap = true;
        self.file_meta.push(FileMeta {
            record,
            area_off,
            rec_len,
            access: AccessMode::Sequential,
            keys: vec![],
            b_off,
            state_off,
        });
        Ok(FileInfo {
            desc,
            handle_off: state_off,
            rec_item: Some(record),
            kind: FileKind::Sort,
            meta: self.file_meta.len() - 1,
        })
    }

    /// The statement, when it is a verb on a file held in memory or a SORT
    /// work file; `None` leaves it to the ordinary lowering.
    pub(crate) fn memory_file_stmt(&mut self, s: &Stmt, line: u32) -> Option<GenResult<()>> {
        match s {
            Stmt::Open {
                mode,
                files,
                extra_modes,
                ..
            } => {
                let mut groups: Vec<(OpenMode, &[String])> = vec![(*mode, files.as_slice())];
                groups.extend(extra_modes.iter().map(|(m, f)| (*m, f.as_slice())));
                if !groups
                    .iter()
                    .any(|(_, fs)| fs.iter().any(|f| self.is_memory(f)))
                {
                    return None;
                }
                Some((|| {
                    for (m, fs) in groups {
                        for f in fs {
                            let fi = self.file(f, line)?;
                            if fi.kind == FileKind::Memory {
                                let code = match m {
                                    OpenMode::Input => idx::MODE_INPUT,
                                    OpenMode::Output => idx::MODE_OUTPUT,
                                    OpenMode::InputOutput => idx::MODE_IO,
                                    OpenMode::Extend => idx::MODE_EXTEND,
                                };
                                self.base();
                                self.i32c(fi.desc as i32);
                                self.i32c(code as i32);
                                self.call_rt("rt_idx_open");
                                self.file_verb_done_cover(f, fi.handle_off, Cover::None);
                            } else {
                                self.open_(m, std::slice::from_ref(f), line)?;
                            }
                        }
                    }
                    Ok(())
                })())
            }
            Stmt::Close {
                files,
                locked,
                reel,
                ..
            } => {
                if !files.iter().any(|f| self.is_memory(f)) {
                    return None;
                }
                Some((|| {
                    for f in files {
                        let fi = self.file(f, line)?;
                        if fi.kind != FileKind::Memory {
                            self.close_(std::slice::from_ref(f), line)?;
                            continue;
                        }
                        let how = if reel.iter().any(|r| r.eq_ignore_ascii_case(f)) {
                            2
                        } else if locked.iter().any(|r| r.eq_ignore_ascii_case(f)) {
                            1
                        } else {
                            0
                        };
                        self.base();
                        self.i32c(fi.desc as i32);
                        self.i32c(how);
                        self.call_rt("rt_idx_close");
                        self.file_verb_done_cover(f, fi.handle_off, Cover::None);
                    }
                    Ok(())
                })())
            }
            Stmt::Write {
                record,
                from,
                invalid_key,
                not_invalid_key,
                ..
            } => {
                let (name, fi) = match self.file_of_record(record, line) {
                    Ok(x) => x,
                    Err(e) => return Some(Err(e)),
                };
                if fi.kind != FileKind::Memory {
                    return None;
                }
                Some(self.idx_write(
                    &name,
                    fi,
                    record,
                    from.as_ref(),
                    invalid_key,
                    not_invalid_key,
                    line,
                ))
            }
            Stmt::Read {
                file,
                into,
                key,
                direction,
                at_end,
                not_at_end,
                invalid_key,
                not_invalid_key,
                ..
            } => {
                let fi = match self.file(file, line) {
                    Ok(f) => f,
                    Err(e) => return Some(Err(e)),
                };
                if fi.kind != FileKind::Memory {
                    return None;
                }
                Some(self.idx_read(
                    file,
                    fi,
                    into.as_ref(),
                    key.as_ref(),
                    *direction,
                    at_end,
                    not_at_end,
                    invalid_key,
                    not_invalid_key,
                    line,
                ))
            }
            Stmt::Rewrite {
                record,
                from,
                invalid_key,
                not_invalid_key,
                ..
            } => {
                let (name, fi) = match self.file_of_record(record, line) {
                    Ok(x) => x,
                    Err(e) => return Some(Err(e)),
                };
                if fi.kind != FileKind::Memory {
                    return Some(self.err(line, format!("REWRITE of file {name} is not yet supported by the WebAssembly generator")));
                }
                Some((|| {
                    if let Some(src) = from {
                        self.move_(src, std::slice::from_ref(record), line)?;
                    }
                    let random = self.file_meta[fi.meta].access != AccessMode::Sequential;
                    self.base();
                    self.i32c(fi.desc as i32);
                    self.i32c(random as i32);
                    self.call_rt("rt_idx_rewrite");
                    self.key_outcome(&name, fi, invalid_key, not_invalid_key)
                })())
            }
            Stmt::Delete {
                file,
                invalid_key,
                not_invalid_key,
                ..
            } => {
                let fi = match self.file(file, line) {
                    Ok(f) => f,
                    Err(e) => return Some(Err(e)),
                };
                if fi.kind != FileKind::Memory {
                    return Some(self.err(line, format!("DELETE of file {file} is not yet supported by the WebAssembly generator")));
                }
                let random = self.file_meta[fi.meta].access != AccessMode::Sequential;
                self.base();
                self.i32c(fi.desc as i32);
                self.i32c(random as i32);
                self.call_rt("rt_idx_delete");
                Some(self.key_outcome(file, fi, invalid_key, not_invalid_key))
            }
            Stmt::Start {
                file,
                key,
                invalid_key,
                not_invalid_key,
                ..
            } => {
                let fi = match self.file(file, line) {
                    Ok(f) => f,
                    Err(e) => return Some(Err(e)),
                };
                if fi.kind != FileKind::Memory {
                    return Some(self.err(line, format!("START of file {file} is not yet supported by the WebAssembly generator")));
                }
                Some(self.idx_start(file, fi, key.as_ref(), invalid_key, not_invalid_key))
            }
            Stmt::Sort {
                file,
                keys,
                using,
                giving,
                input_proc,
                output_proc,
                collating,
                ..
            } => Some(self.sort_(
                file,
                keys,
                using,
                giving,
                input_proc.as_ref(),
                output_proc.as_ref(),
                collating.as_deref(),
                line,
            )),
            Stmt::Release { record, from, .. } => Some((|| {
                let (name, fi) = self.file_of_record(record, line)?;
                if fi.kind != FileKind::Sort {
                    return self.err(
                        line,
                        format!("RELEASE names a record of {name}, which no SORT sorts"),
                    );
                }
                if let Some(src) = from {
                    self.move_(src, std::slice::from_ref(record), line)?;
                }
                self.base();
                self.i32c(fi.desc as i32);
                self.call_rt("rt_sort_release");
                Ok(())
            })()),
            Stmt::Return {
                file,
                into,
                at_end,
                not_at_end,
                ..
            } => Some((|| {
                let fi = self.file(file, line)?;
                if fi.kind != FileKind::Sort {
                    return self.err(line, format!("RETURN names {file}, which no SORT sorts"));
                }
                self.base();
                self.i32c(fi.desc as i32);
                self.call_rt("rt_sort_return");
                let hold = self.hold_status();
                self.read_outcome(hold, fi, into, line, (not_at_end, at_end), false)
            })()),
            _ => None,
        }
    }

    /// Is `name` a file of this unit (or a GLOBAL one it sees) held in memory?
    fn is_memory(&self, name: &str) -> bool {
        self.file(name, 0)
            .map(|f| f.kind == FileKind::Memory)
            .unwrap_or(false)
    }

    /// Keep the status on the stack in the status slot; its offset.
    fn hold_status(&mut self) -> u32 {
        let hold = self.status_slot();
        self.raw(I::LocalSet(0));
        self.base();
        self.raw(I::LocalGet(0));
        self.raw(I::I32Store(wasm_encoder::MemArg {
            offset: hold as u64,
            align: 2,
            memory_index: 0,
        }));
        hold
    }

    fn load_hold(&mut self, hold: u32) {
        self.base();
        self.raw(I::I32Load(wasm_encoder::MemArg {
            offset: hold as u64,
            align: 2,
            memory_index: 0,
        }));
    }

    /// After a `WRITE`, `REWRITE` or `DELETE` (status on the stack): the USE
    /// procedures, then `NOT INVALID KEY` on 00 and `INVALID KEY` otherwise
    /// (`run_key_outcome`).
    fn key_outcome(
        &mut self,
        file: &str,
        fi: FileInfo,
        invalid: &[Stmt],
        not_invalid: &[Stmt],
    ) -> GenResult<()> {
        let cover = if invalid.is_empty() {
            Cover::None
        } else {
            Cover::All
        };
        self.file_verb_done_cover(file, fi.handle_off, cover);
        if invalid.is_empty() && not_invalid.is_empty() {
            return Ok(());
        }
        let hold = self.status_slot();
        let ok_b = self.new_block("not-invalid-key");
        let bad_b = self.new_block("invalid-key");
        let join = self.new_block("end-key-verb");
        self.load_hold(hold);
        self.raw(I::I32Eqz);
        self.branch(ok_b, bad_b);
        for (b, list) in [(ok_b, not_invalid), (bad_b, invalid)] {
            self.begin(b);
            self.stmts(list)?;
            if self.cur.is_some() {
                self.jump(join);
            }
        }
        self.begin(join);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn idx_write(
        &mut self,
        file: &str,
        fi: FileInfo,
        record: &Expr,
        from: Option<&Expr>,
        invalid: &[Stmt],
        not_invalid: &[Stmt],
        line: u32,
    ) -> GenResult<()> {
        if let Some(src) = from {
            self.move_(src, std::slice::from_ref(record), line)?;
        }
        let base = match record {
            Expr::Subscript { base, .. } => base.as_ref(),
            other => other,
        };
        let rec = self.resolve_item(base)?;
        let wlen = self.layout.items[rec].size;
        self.base();
        self.i32c(fi.desc as i32);
        self.i32c(wlen as i32);
        self.call_rt("rt_idx_write");
        self.key_outcome(file, fi, invalid, not_invalid)
    }

    /// The `(name, quals)` a key phrase names (`expr_key_parts`).
    fn key_parts_of(&self, e: &Expr) -> (String, Vec<String>) {
        match e {
            Expr::Identifier(n, _) => (up(n), vec![]),
            Expr::Qualified { name, of, .. } => {
                (up(name), self.quals_of(of).iter().map(|q| up(q)).collect())
            }
            Expr::Subscript { base, .. } | Expr::RefMod { base, .. } => self.key_parts_of(base),
            _ => (String::new(), vec![]),
        }
    }

    /// The key of reference a `READ … KEY` / `START … KEY` names, and the spec
    /// of the value it searches for (`[n, (region offset, len) × n]`, 0 when the
    /// name is not a field of the record).
    fn key_ref(&mut self, meta: usize, key: Option<&Expr>) -> (u32, u32) {
        let m = self.file_meta[meta].clone();
        let (name, quals) = match key {
            Some(e) => self.key_parts_of(e),
            None => (m.keys[0].name.clone(), m.keys[0].quals.clone()),
        };
        if name.is_empty() {
            return (0, 0);
        }
        // key_of_reference: the declared names first, then the bytes the item covers.
        let field = self.field_in_record(m.record, &name, &quals);
        let kor = if m.keys[0].name == name && agrees(&quals, &m.keys[0].quals) {
            0
        } else if let Some(i) = m
            .keys
            .iter()
            .skip(1)
            .position(|k| k.name == name && agrees(&quals, &k.quals))
        {
            i + 1
        } else if let Some((off, len)) = field {
            let rel = off - m.area_off;
            m.keys
                .iter()
                .position(|k| k.off() == rel && k.len() == len)
                .or_else(|| m.keys.iter().position(|k| k.off() == rel && len < k.len()))
                .unwrap_or(0)
        } else {
            0
        };
        // declared_key_value: a split key's fields joined, else the field's bytes.
        let declared = if m.keys[0].name == name && (quals.is_empty() || quals == m.keys[0].quals) {
            Some(&m.keys[0])
        } else {
            m.keys
                .iter()
                .skip(1)
                .find(|k| k.name == name && !k.declared_parts.is_empty())
        };
        let parts: Vec<(u32, u32)> = match declared {
            Some(k) if !k.declared_parts.is_empty() => {
                k.parts.iter().map(|&(o, l)| (o + m.area_off, l)).collect()
            }
            _ => match field {
                Some(f) => vec![f],
                None => return (kor as u32, 0),
            },
        };
        let mut words = vec![parts.len() as u32];
        for (o, l) in parts {
            words.push(o);
            words.push(l);
        }
        (kor as u32, self.pool.spec(&words))
    }

    #[allow(clippy::too_many_arguments)]
    fn idx_read(
        &mut self,
        file: &str,
        fi: FileInfo,
        into: Option<&Expr>,
        key: Option<&Expr>,
        direction: ReadDirection,
        at_end: &[Stmt],
        not_at_end: &[Stmt],
        invalid: &[Stmt],
        not_invalid: &[Stmt],
        _line: u32,
    ) -> GenResult<()> {
        let access = self.file_meta[fi.meta].access;
        // NEXT/PREVIOUS force the sequential format; an AT END phrase settles it
        // when the direction does not; KEY IS forces the keyed one.
        let has_at_end = !at_end.is_empty() || !not_at_end.is_empty();
        let random = direction == ReadDirection::Default
            && (key.is_some()
                || (!has_at_end && matches!(access, AccessMode::Random | AccessMode::Dynamic)));
        let (kor, ks) = if random {
            self.key_ref(fi.meta, key)
        } else {
            (0, 0)
        };
        let how = random as i32 | (((direction == ReadDirection::Previous) as i32) << 1);
        self.base();
        self.i32c(fi.desc as i32);
        self.i32c(how);
        self.i32c(kor as i32);
        self.i32c(ks as i32);
        self.call_rt("rt_idx_read");
        let (ok_list, fail_list): (&[Stmt], &[Stmt]) = if random {
            (
                if !not_invalid.is_empty() {
                    not_invalid
                } else {
                    not_at_end
                },
                if !invalid.is_empty() { invalid } else { at_end },
            )
        } else {
            (
                if !not_at_end.is_empty() {
                    not_at_end
                } else {
                    not_invalid
                },
                if !at_end.is_empty() { at_end } else { invalid },
            )
        };
        let cover = match (fail_list.is_empty(), random) {
            (true, _) => Cover::None,
            (false, true) => Cover::Keys,
            (false, false) => Cover::Eof,
        };
        self.file_verb_done_cover(file, fi.handle_off, cover);
        let hold = self.status_slot();
        self.read_outcome(hold, fi, &into.cloned(), 0, (ok_list, fail_list), random)
    }

    /// After a `READ` or `RETURN` (status in `hold`): on 00 the record goes
    /// `INTO` the item and the success phrase runs; on the condition the
    /// statement's phrase covers (10, or 21–24 for a keyed read) the failure
    /// phrase runs; on anything else neither.
    fn read_outcome(
        &mut self,
        hold: u32,
        fi: FileInfo,
        into: &Option<Expr>,
        line: u32,
        (ok_list, fail_list): (&[Stmt], &[Stmt]),
        keyed: bool,
    ) -> GenResult<()> {
        if into.is_none() && ok_list.is_empty() && fail_list.is_empty() {
            return Ok(());
        }
        let ok_b = self.new_block("read-ok");
        let not_ok = self.new_block("read-not-ok");
        let fail_b = self.new_block("read-condition");
        let join = self.new_block("end-read");
        self.load_hold(hold);
        self.raw(I::I32Eqz);
        self.branch(ok_b, not_ok);
        self.begin(not_ok);
        self.load_hold(hold);
        if keyed {
            self.i32c(21);
            self.raw(I::I32Sub);
            self.i32c(3);
            self.raw(I::I32LeU);
        } else {
            self.i32c(10);
            self.raw(I::I32Eq);
        }
        self.branch(fail_b, join);
        self.begin(ok_b);
        if let Some(target) = into {
            let rec = fi.rec_item.expect("a file with a record");
            let src = self.item_operand(rec, &[], None, line)?;
            self.move_operand_to(src, target, line)?;
        }
        self.stmts(ok_list)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(fail_b);
        self.stmts(fail_list)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(join);
        Ok(())
    }

    fn idx_start(
        &mut self,
        file: &str,
        fi: FileInfo,
        key: Option<&(CmpOp, Expr)>,
        invalid: &[Stmt],
        not_invalid: &[Stmt],
    ) -> GenResult<()> {
        let rel = match key.map(|k| k.0) {
            None | Some(CmpOp::Eq) => idx::START_EQ,
            Some(CmpOp::Gt) => idx::START_GT,
            // Not standard for START; the interpreter reads it as ≥.
            Some(CmpOp::Ge) | Some(CmpOp::Ne) => idx::START_GE,
            Some(CmpOp::Lt) => idx::START_LT,
            Some(CmpOp::Le) => idx::START_LE,
        };
        let (kor, ks) = self.key_ref(fi.meta, key.map(|k| &k.1));
        self.base();
        self.i32c(fi.desc as i32);
        self.i32c(rel as i32);
        self.i32c(kor as i32);
        self.i32c(ks as i32);
        self.call_rt("rt_idx_start");
        // START performs no USE procedure in the interpreter.
        self.file_verb_done_cover(file, fi.handle_off, Cover::All);
        if invalid.is_empty() && not_invalid.is_empty() {
            return Ok(());
        }
        let hold = self.status_slot();
        let ok_b = self.new_block("start-ok");
        let bad_b = self.new_block("start-invalid-key");
        let join = self.new_block("end-start");
        self.load_hold(hold);
        self.raw(I::I32Eqz);
        self.branch(ok_b, bad_b);
        for (b, list) in [(ok_b, not_invalid), (bad_b, invalid)] {
            self.begin(b);
            self.stmts(list)?;
            if self.cur.is_some() {
                self.jump(join);
            }
        }
        self.begin(join);
        Ok(())
    }

    /// `SORT sd ON keys INPUT PROCEDURE … OUTPUT PROCEDURE …`.
    #[allow(clippy::too_many_arguments)]
    fn sort_(
        &mut self,
        file: &str,
        keys: &[SortKey],
        using: &[String],
        giving: &[String],
        input_proc: Option<&(String, Option<String>)>,
        output_proc: Option<&(String, Option<String>)>,
        collating: Option<&str>,
        line: u32,
    ) -> GenResult<()> {
        if !using.is_empty() || !giving.is_empty() {
            return self.err_kind(
                line,
                ErrorKind::BackendOnly,
                "SORT … USING/GIVING",
                format!(
                    "SORT … USING/GIVING is not available in a browser module: {}",
                    crate::support::SORT_FILES
                ),
            );
        }
        if collating.is_some() {
            return self.err_kind(line, ErrorKind::Unsupported, "SORT … COLLATING SEQUENCE", "SORT … COLLATING SEQUENCE is not available in a browser module yet; sort by the program collating sequence");
        }
        let fi = self.file(file, line)?;
        if fi.kind != FileKind::Sort {
            return self.err(line, format!("SORT names {file}, which is not a sort file"));
        }
        let m = self.file_meta[fi.meta].clone();
        let mut words: Vec<u32> = Vec::new();
        let mut n = 0u32;
        for k in keys {
            for f in &k.fields {
                let i = self.resolve_item(f)?;
                let it = self.layout.items[i].clone();
                let mut root = i;
                while let Some(p) = self.layout.items[root].parent {
                    root = p;
                }
                if root != m.record {
                    return self.err(
                        line,
                        format!(
                            "the SORT key {} is not in the record of {file}",
                            it.name.clone().unwrap_or_default()
                        ),
                    );
                }
                let a = self.item_operand(i, &[], None, line)?;
                let fd = self.pool.fd_with_spec(
                    &it.fd,
                    it.pic_template.as_deref(),
                    self.prog.decimal_comma,
                    self.prog.currency,
                );
                let b = self
                    .pool
                    .opd_item(it.off - m.area_off + m.b_off, &fd, &[], None);
                words.extend_from_slice(&[a, b, k.ascending as u32]);
                n += 1;
            }
        }
        let keys_addr = self.pool.spec(&words);
        let desc = self
            .pool
            .spec(&[m.rec_len, m.area_off, m.b_off, m.state_off, n, keys_addr]);
        let procedure = |me: &mut Self,
                         p: Option<&(String, Option<String>)>,
                         what: &str|
         -> GenResult<()> {
            let Some((name, thru)) = p else {
                return me.err_kind(line, ErrorKind::BackendOnly, "SORT … USING/GIVING", format!("a SORT without an {what} PROCEDURE is not available in a browser module: {}", crate::support::SORT_FILES));
            };
            let (from, to) = me
                .name_range(name, thru.as_deref(), line)?
                .expect("a paragraph range");
            let k = me.new_block("after-sort-procedure");
            me.call_range(from, to, k);
            me.begin(k);
            Ok(())
        };
        self.base();
        self.i32c(desc as i32);
        self.call_rt("rt_sort_begin");
        procedure(self, input_proc, "INPUT")?;
        self.base();
        self.i32c(desc as i32);
        self.call_rt("rt_sort_sort");
        procedure(self, output_proc, "OUTPUT")?;
        self.base();
        self.i32c(desc as i32);
        self.call_rt("rt_sort_end");
        Ok(())
    }
}
