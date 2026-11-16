// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Files on disk in a desktop module (DESKTOP-WASM-001 T38, ABI A34): every file of a `native`
//! build except a `SORT` work file is kept by the desktop runner, and its verbs are lowered to the
//! routines of `cobolt_wasm_rt::nfile`, which call the runner's `prn` file imports.
//!
//! What the interpreter works out from the statement and the program's data before it touches the
//! file (`exec_open`, `exec_read`, …) is worked out here or in those routines, the same way: the
//! format of a `READ` and its key of reference (shared with the in-memory files, `lower_idx`),
//! the length a `WRITE` gives the record, the record number a RELATIVE verb carries. The runner
//! does the rest with the interpreter's own code (`cobolt_runtime::file_core`), so a file's
//! contents and its `FILE STATUS` are the interpreter's.
//!
//! What the runner does not keep yet is refused at the file's first verb, so the Build takes the
//! Cargo path and says why: a report printed into a Viewer, `LINAGE`, `SAME AREA`, `OPEN … WITH
//! REGISTERED USER`, and national or UTF-8 text in a LINE SEQUENTIAL record.

use crate::error::{ErrorKind, GenResult};
use crate::lower::{FileInfo, FileKind, Lower};
use crate::lower_call::Cover;
use crate::lower_idx::{FileMeta, KeyMeta};
use cobolt_ast::data::{FileDescription, PicKind, Usage};
use cobolt_ast::expr::{CmpOp, Expr};
use cobolt_ast::program::{AccessMode, EngineChoice, FileControl, FileOrganization, StorageMode};
use cobolt_ast::stmt::{OpenMode, ReadDirection, ShareMode, Stmt};
use cobolt_wasm_abi::codes::cap;
use cobolt_wasm_abi::files::{verb, Access, FileDecl, KeyDecl, Org};
use cobolt_wasm_rt::nfile;

fn up(s: &str) -> String {
    s.to_ascii_uppercase()
}

/// Why a desktop module does not hold a file the runner cannot keep yet.
const D_REPORT: &str = "a report printed into a Viewer is not on the WebAssembly path yet (DESKTOP-WASM-001 M6)";
const D_LINAGE: &str = "LINAGE and SAME AREA are not on the WebAssembly path yet (DESKTOP-WASM-001 M3)";
const D_LINE_TEXT: &str = "national and UTF-8 text in a LINE SEQUENTIAL record is not on the WebAssembly path yet (DESKTOP-WASM-001 M5)";
const D_REGISTERED: &str = "OPEN … WITH REGISTERED USER is not on the WebAssembly path yet (DESKTOP-WASM-001 M3)";

impl Lower<'_, '_> {
    /// Lay out a file the desktop runner keeps: its description in the module's `prfiles` section,
    /// its place in the region and its descriptor among the constants. `fd` is its FD, and
    /// `same_area` says whether I-O-CONTROL joins it to another file's record area.
    pub(crate) fn build_native_file(
        &mut self,
        fc: &FileControl,
        fd: Option<&FileDescription>,
        rec_items: &[usize],
        same_area: bool,
    ) -> GenResult<FileInfo> {
        let name = up(&fc.name);
        let refuse = |why: &str| (name.clone(), why.to_string());
        let mut refusal: Option<(String, String)> = None;
        if fc.viewer_target.is_some() || fc.assign.trim().eq_ignore_ascii_case("VIEWER") || fc.organization.is_rendered() {
            refusal = Some(refuse(D_REPORT));
        } else if fd.is_some_and(|f| f.linage.is_some()) || same_area {
            refusal = Some(refuse(D_LINAGE));
        }
        let (record, area_off, rec_len) = match self.first_record(&fc.name, rec_items) {
            Ok(r) => r,
            // No record description: nothing to lay out, and the verbs say so.
            Err(e) => {
                self.native_refusals.insert(name, e.message.clone());
                return Err(e);
            }
        };
        // The record area holds the longest record description; `READ … INTO` moves all of it.
        let mut longest = record;
        let mut area_len = rec_len;
        for &i in rec_items {
            let it = &self.layout.items[i];
            let end = (it.off + it.size).saturating_sub(area_off);
            if end > area_len {
                area_len = end;
                longest = i;
            }
        }
        if fc.organization == FileOrganization::LineSequential && rec_items.iter().any(|&r| self.holds_text_class(r)) {
            refusal = refusal.or_else(|| Some(refuse(D_LINE_TEXT)));
        }
        if let Some((n, why)) = refusal {
            self.native_refusals.insert(n, why);
        }

        // The keys of an INDEXED file, as `build_memory_file` finds them (the interpreter's
        // `make_indexed_engine`).
        let mut keys: Vec<KeyMeta> = Vec::new();
        let mut key_names: Vec<Option<String>> = Vec::new();
        let mut ordered_writes = false;
        if fc.organization == FileOrganization::Indexed {
            let rk_parts: Vec<(String, Vec<String>)> = fc.record_key_parts.iter().map(|p| (p.name.clone(), p.quals.clone())).collect();
            let primary = fc.record_key.as_deref().and_then(|k| self.key_meta(record, area_off, k, &fc.record_key_quals, &rk_parts, false));
            ordered_writes = primary.is_some();
            keys.push(primary.unwrap_or(KeyMeta { name: String::new(), quals: vec![], declared_parts: vec![], parts: vec![(0, rec_len)], dup: false }));
            key_names.push(fc.record_key.as_deref().map(up));
            for ak in &fc.alternate_keys {
                let parts: Vec<(String, Vec<String>)> = ak.parts.iter().map(|p| (p.name.clone(), p.quals.clone())).collect();
                if let Some(k) = self.key_meta(record, area_off, &ak.field, &ak.quals, &parts, ak.with_duplicates) {
                    keys.push(k);
                    key_names.push(Some(ak.field.clone()));
                }
            }
        }

        let sizing = fd.and_then(|f| f.record_sizing.clone());
        let lens: Vec<u32> = rec_items.iter().map(|&i| self.layout.items[i].size.max(1)).collect();
        let varying = match &sizing {
            Some(s) => s.varying,
            None => lens.windows(2).any(|w| w[0] != w[1]),
        };
        let widest = lens.iter().copied().max().unwrap_or(rec_len).max(rec_len);
        let slot_len = widest.max(sizing.as_ref().and_then(|s| s.max).unwrap_or(0));
        let item_opd = |me: &mut Self, n: &str| -> GenResult<u32> {
            match me.layout.lookup(n, &[]) {
                Ok(i) => me.item_operand(i, &[], None, fc.span.line),
                Err(_) => Ok(0),
            }
        };
        let status_opd = match &fc.file_status {
            Some(sn) => item_opd(self, sn)?,
            None => 0,
        };
        let relkey_opd = match (&fc.relative_key, fc.organization) {
            (Some(k), FileOrganization::Relative) => item_opd(self, k)?,
            _ => 0,
        };
        let depend_opd = match sizing.as_ref().filter(|s| s.varying).and_then(|s| s.depending_on.clone()) {
            Some(d) => item_opd(self, &d)?,
            None => 0,
        };
        // The ASSIGN names a data item whose value is the path, or is the path itself
        // (`resolve_assign_path`).
        let assign = fc.assign.trim();
        let path_opd = item_opd(self, assign)?;
        let path_addr = if path_opd == 0 && !assign.is_empty() { self.pool.intern(assign.as_bytes()) } else { 0 };
        let path_len = if path_opd == 0 { assign.len() as u32 } else { 0 };

        // One file per name in the program, as the interpreter keeps one open file per name; the
        // first declaration describes it.
        let (id, handle_off) = match self.native_ids.get(&name).copied() {
            Some(x) => x,
            None => {
                let decl = FileDecl {
                    name: name.clone(),
                    org: match fc.organization {
                        FileOrganization::LineSequential => Org::LineSequential,
                        FileOrganization::Relative => Org::Relative,
                        FileOrganization::Indexed => Org::Indexed,
                        _ => Org::Sequential,
                    },
                    access: match fc.access {
                        AccessMode::Sequential => Access::Sequential,
                        AccessMode::Random => Access::Random,
                        AccessMode::Dynamic => Access::Dynamic,
                    },
                    optional: fc.optional,
                    record_len: rec_len,
                    slot_len: if fc.organization == FileOrganization::Relative { slot_len } else { 0 },
                    varying,
                    min_len: sizing.as_ref().and_then(|s| s.min),
                    max_len: sizing.as_ref().and_then(|s| s.max),
                    keys: keys.iter().map(|k| KeyDecl { parts: k.parts.clone(), dup: k.dup, split: !k.declared_parts.is_empty() }).collect(),
                    key_names,
                    ordered_writes,
                    memory: fc.storage_mode == StorageMode::Memory,
                    persist: fc.persist,
                    compress: fc.data_compressing,
                    engine: fc.engine.map(|e| match e {
                        EngineChoice::Prcidxd1 => "prcidxd1".to_string(),
                        EngineChoice::Redb => "redb".to_string(),
                    }),
                    definition: match (fc.organization, fd) {
                        (FileOrganization::Indexed, Some(fd)) => Some(cobolt_ast::fd_text::file_definition(fd)),
                        _ => None,
                    },
                    // The interpreter's `relative_key_digits`: the item the RELATIVE KEY names.
                    rel_digits: match (&fc.relative_key, fc.organization) {
                        (Some(k), FileOrganization::Relative) => self
                            .layout
                            .lookup(k, &[])
                            .ok()
                            .and_then(|i| self.layout.items[i].pic_template.as_deref())
                            .map(cobolt_ast::data::pic_integer_digits)
                            .filter(|d| *d > 0),
                        _ => None,
                    },
                };
                let id = self.names.native_files.len() as u32;
                self.names.native_files.push(decl);
                let h = self.layout.alloc_hidden(8);
                self.native_ids.insert(name.clone(), (id, h));
                (id, h)
            }
        };
        let org = match fc.organization {
            FileOrganization::LineSequential => nfile::ORG_LINE_SEQUENTIAL,
            FileOrganization::Relative => nfile::ORG_RELATIVE,
            FileOrganization::Indexed => nfile::ORG_INDEXED,
            _ => nfile::ORG_SEQUENTIAL,
        };
        let flags = if fc.access == AccessMode::Sequential { nfile::F_SEQ_ACCESS } else { 0 };
        let desc = self.pool.spec(&[
            id, org, flags, status_opd, area_off, area_len, path_addr, path_len, path_opd, relkey_opd, depend_opd, handle_off,
        ]);
        self.file_meta.push(FileMeta {
            record,
            area_off,
            rec_len,
            access: fc.access,
            keys,
            b_off: 0,
            state_off: 0,
            org: fc.organization,
            relkey_opd,
        });
        Ok(FileInfo { desc, handle_off, rec_item: Some(longest), kind: FileKind::Native, meta: self.file_meta.len() - 1 })
    }

    /// Whether the record `r` holds a national or UTF-8 field.
    fn holds_text_class(&self, r: usize) -> bool {
        let it = &self.layout.items[r];
        matches!(it.pic_kind, Some(PicKind::National | PicKind::Utf8))
            || matches!(it.usage, Usage::National | Usage::Utf8)
            || it.children.iter().any(|&c| self.holds_text_class(c))
    }

    /// The file `f` of a verb, which the runner keeps; its capability goes in the manifest.
    fn native_file(&mut self, f: &str, line: u32) -> GenResult<FileInfo> {
        let fi = self.file(f, line)?;
        if fi.kind != FileKind::Native {
            return self.err(line, format!("file {f} is a SORT work file"));
        }
        let c = match self.file_meta[fi.meta].org {
            FileOrganization::LineSequential => cap::FILES_LINE_SEQUENTIAL,
            FileOrganization::Relative => cap::FILES_RELATIVE,
            FileOrganization::Indexed => cap::FILES_INDEXED,
            _ => cap::FILES_SEQUENTIAL,
        };
        self.names.file_caps.insert(c);
        Ok(fi)
    }

    /// The file whose record `record` is, which the runner keeps.
    fn native_record_file(&mut self, record: &Expr, line: u32) -> GenResult<(String, FileInfo)> {
        let (name, _) = self.file_of_record(record, line)?;
        let fi = self.native_file(&name, line)?;
        Ok((name, fi))
    }

    /// The length of the record description `record` names.
    fn record_len_of(&mut self, record: &Expr) -> GenResult<u32> {
        let base = match record {
            Expr::Subscript { base, .. } => base.as_ref(),
            other => other,
        };
        let rec = self.resolve_item(base)?;
        Ok(self.layout.items[rec].size)
    }

    /// The statement, when it is a verb on a file of a desktop module; `None` leaves it to the rest
    /// of the lowering (a `SORT`, `RELEASE` or `RETURN` on a work file).
    pub(crate) fn native_file_stmt(&mut self, s: &Stmt, line: u32) -> Option<GenResult<()>> {
        if !self.native {
            return None;
        }
        Some(match s {
            Stmt::Open { mode, files, sharing, lock, registered_user, extra_modes, .. } => {
                if registered_user.is_some() {
                    return Some(self.err_kind(line, ErrorKind::Other, "OPEN … WITH REGISTERED USER", format!("OPEN … WITH REGISTERED USER: {D_REGISTERED}")));
                }
                let flags = if *lock || matches!(sharing, Some(ShareMode::NoOther)) { verb::OPEN_EXCLUSIVE } else { 0 };
                let mut groups: Vec<(OpenMode, &[String])> = vec![(*mode, files.as_slice())];
                groups.extend(extra_modes.iter().map(|(m, f)| (*m, f.as_slice())));
                (|| {
                    for (m, fs) in groups {
                        for f in fs {
                            let fi = self.native_file(f, line)?;
                            let code = match m {
                                OpenMode::Input => verb::OPEN_INPUT,
                                OpenMode::Output => verb::OPEN_OUTPUT,
                                OpenMode::InputOutput => verb::OPEN_IO,
                                OpenMode::Extend => verb::OPEN_EXTEND,
                            };
                            self.base();
                            self.i32c(fi.desc as i32);
                            self.i32c(code);
                            self.i32c(flags);
                            self.call_rt("rt_nf_open");
                            self.file_verb_done_cover(f, fi.handle_off, Cover::None);
                        }
                    }
                    Ok(())
                })()
            }
            Stmt::Close { files, locked, reel, .. } => (|| {
                for f in files {
                    let fi = self.native_file(f, line)?;
                    let how = if reel.iter().any(|r| r.eq_ignore_ascii_case(f)) {
                        verb::CLOSE_REEL
                    } else if locked.iter().any(|r| r.eq_ignore_ascii_case(f)) {
                        verb::CLOSE_LOCK
                    } else {
                        verb::CLOSE_PLAIN
                    };
                    self.base();
                    self.i32c(fi.desc as i32);
                    self.i32c(how);
                    self.call_rt("rt_nf_close");
                    self.file_verb_done_cover(f, fi.handle_off, Cover::None);
                }
                Ok(())
            })(),
            Stmt::Read { file, into, key, direction, lock, at_end, not_at_end, invalid_key, not_invalid_key, .. } => {
                // A `READ` of a SORT work file is not a READ at all; leave it to the rest.
                match self.file(file, line) {
                    Ok(fi) if fi.kind == FileKind::Native => {}
                    Ok(_) => return None,
                    Err(e) => return Some(Err(e)),
                }
                self.n_read(file, into.as_ref(), key.as_ref(), *direction, *lock, (at_end, not_at_end), (invalid_key, not_invalid_key), line)
            }
            Stmt::Write { record, from, invalid_key, not_invalid_key, .. } => (|| {
                let (name, fi) = self.native_record_file(record, line)?;
                if let Some(src) = from {
                    self.move_(src, std::slice::from_ref(record), line)?;
                }
                // ADVANCING moves the paper of a report, which the runner does not keep: on any other
                // file the interpreter ignores it too.
                let declared = self.record_len_of(record)?;
                self.base();
                self.i32c(fi.desc as i32);
                self.i32c(declared as i32);
                self.call_rt("rt_nf_write");
                self.key_outcome(&name, fi, invalid_key, not_invalid_key)
            })(),
            Stmt::Rewrite { record, from, invalid_key, not_invalid_key, .. } => (|| {
                let (name, fi) = self.native_record_file(record, line)?;
                if let Some(src) = from {
                    self.move_(src, std::slice::from_ref(record), line)?;
                }
                // An INDEXED record is its first record description, whole.
                let m = &self.file_meta[fi.meta];
                let declared = if m.org == FileOrganization::Indexed { m.rec_len } else { self.record_len_of(record)? };
                self.base();
                self.i32c(fi.desc as i32);
                self.i32c(declared as i32);
                self.call_rt("rt_nf_rewrite");
                self.key_outcome(&name, fi, invalid_key, not_invalid_key)
            })(),
            Stmt::Delete { file, invalid_key, not_invalid_key, .. } => (|| {
                let fi = self.native_file(file, line)?;
                // The RECORD KEY's value, from the record area.
                let ks = if self.file_meta[fi.meta].org == FileOrganization::Indexed { self.key_ref(fi.meta, None).1 } else { 0 };
                self.base();
                self.i32c(fi.desc as i32);
                self.i32c(ks as i32);
                self.call_rt("rt_nf_delete");
                self.key_outcome(file, fi, invalid_key, not_invalid_key)
            })(),
            Stmt::Start { file, key, invalid_key, not_invalid_key, .. } => self.n_start(file, key.as_ref(), invalid_key, not_invalid_key, line),
            _ => return None,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn n_read(
        &mut self,
        file: &str,
        into: Option<&Expr>,
        key: Option<&Expr>,
        direction: ReadDirection,
        lock: Option<bool>,
        (at_end, not_at_end): (&[Stmt], &[Stmt]),
        (invalid, not_invalid): (&[Stmt], &[Stmt]),
        line: u32,
    ) -> GenResult<()> {
        let fi = self.native_file(file, line)?;
        let (access, org) = (self.file_meta[fi.meta].access, self.file_meta[fi.meta].org);
        // The format, as `idx_read` and the interpreter decide it: NEXT/PREVIOUS force the
        // sequential one, KEY IS the keyed one, and an AT END phrase settles it otherwise.
        let has_at_end = !at_end.is_empty() || !not_at_end.is_empty();
        let random = direction == ReadDirection::Default
            && (key.is_some() || (!has_at_end && matches!(access, AccessMode::Random | AccessMode::Dynamic)));
        let (kor, ks) = if random && org == FileOrganization::Indexed { self.key_ref(fi.meta, key) } else { (0, 0) };
        let mut how = 0;
        if random {
            how |= verb::READ_KEYED;
        }
        if direction == ReadDirection::Previous {
            how |= verb::READ_PREVIOUS;
        }
        if lock == Some(false) {
            how |= verb::READ_NO_LOCK;
        }
        self.base();
        self.i32c(fi.desc as i32);
        self.i32c(how);
        self.i32c(kor as i32);
        self.i32c(ks as i32);
        self.call_rt("rt_nf_read");
        let (ok_list, fail_list): (&[Stmt], &[Stmt]) = if random {
            (if !not_invalid.is_empty() { not_invalid } else { not_at_end }, if !invalid.is_empty() { invalid } else { at_end })
        } else {
            (if !not_at_end.is_empty() { not_at_end } else { not_invalid }, if !at_end.is_empty() { at_end } else { invalid })
        };
        let cover = match (fail_list.is_empty(), random) {
            (true, _) => Cover::None,
            (false, true) => Cover::Keys,
            (false, false) => Cover::AtEnd,
        };
        self.file_verb_done_cover(file, fi.handle_off, cover);
        let hold = self.status_slot();
        self.read_outcome(hold, fi, &into.cloned(), line, (ok_list, fail_list), random)
    }

    fn n_start(
        &mut self,
        file: &str,
        key: Option<&(CmpOp, Expr)>,
        invalid: &[Stmt],
        not_invalid: &[Stmt],
        line: u32,
    ) -> GenResult<()> {
        let fi = self.native_file(file, line)?;
        let (org, relkey) = (self.file_meta[fi.meta].org, self.file_meta[fi.meta].relkey_opd);
        let op = match key.map(|k| k.0) {
            None | Some(CmpOp::Eq) => verb::START_EQ,
            Some(CmpOp::Gt) => verb::START_GT,
            // Not standard for START; the interpreter reads it as ≥.
            Some(CmpOp::Ge) | Some(CmpOp::Ne) => verb::START_GE,
            Some(CmpOp::Lt) => verb::START_LT,
            Some(CmpOp::Le) => verb::START_LE,
        };
        let (kor, ks) = if org == FileOrganization::Indexed { self.key_ref(fi.meta, key.map(|k| &k.1)) } else { (0, 0) };
        // A RELATIVE START compares record numbers: the KEY phrase's item, else the RELATIVE KEY.
        let rel_opd = match (org, key) {
            (FileOrganization::Relative, Some((_, e))) => self.operand(e)?.unwrap_or(0),
            (FileOrganization::Relative, None) => relkey,
            _ => 0,
        };
        self.base();
        self.i32c(fi.desc as i32);
        self.i32c(op);
        self.i32c(kor as i32);
        self.i32c(ks as i32);
        self.i32c(rel_opd as i32);
        self.call_rt("rt_nf_start");
        // START performs no USE procedure in the interpreter.
        self.file_verb_done_cover(file, fi.handle_off, Cover::All);
        if invalid.is_empty() && not_invalid.is_empty() {
            return Ok(());
        }
        let hold = self.status_slot();
        let ok_b = self.new_block("start-ok");
        let bad_b = self.new_block("start-invalid-key");
        let join = self.new_block("end-start");
        self.key_success(hold);
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
}
