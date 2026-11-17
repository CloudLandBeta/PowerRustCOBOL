// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A desktop program's files on disk** (DESKTOP-WASM-001 T38, ABI A34).
//!
//! The module names a file by its number in the `prfiles` section and by its *slot*, the address
//! of the file's place in the region of the program instance that holds it. Two windows of one form
//! are two instances, and each has its own files, as each runs its own interpreter on the Cargo
//! path. The runner keeps the open file of each slot and does what the interpreter does with it:
//! every verb goes through `cobolt_runtime::file_core`, the interpreter's own code, so the records
//! written, the containers made and every FILE STATUS are the same on both paths.
//!
//! What the module works out itself, as the interpreter does before it touches the file: the path
//! (trimmed), the record's bytes and length, the key and the record number, the format of a READ,
//! the statement's phrases and the USE procedures.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use cobolt_runtime::file_core::{self, Cursor, Handle, IndexedDecl, ReadReq, RewriteReq, ShareLock, WriteReq};
use cobolt_runtime::files::RecordLayout;
use cobolt_runtime::indexed::{IndexedEngine, KeySpec, OpenMode, ReadDir, StartOp};
use cobolt_runtime::indexed_log::{LogFormat, LogLevel};
use cobolt_runtime::seqfile::SequentialFile;
use cobolt_wasm_abi::files::{verb, Access, FileDecl, KeyDecl, Org};

/// A status the runtime cannot name as a number: a permanent error.
const PERMANENT: i32 = 30;

/// The engine new INDEXED files are created with, in the Cargo path's order: the project's choice
/// (`[ide] indexed_engine`, in `prsettings`), else `COBOL_INDEXED_ENGINE` on this machine, else
/// the runtime's default. An existing file always opens with the engine that wrote it.
pub fn indexed_engine(project_choice: &str) -> IndexedEngine {
    IndexedEngine::parse(project_choice)
        .or_else(|| std::env::var("COBOL_INDEXED_ENGINE").ok().and_then(|name| IndexedEngine::parse(&name)))
        .unwrap_or_default()
}

/// One open file.
struct Open {
    handle: Handle,
    mode: OpenMode,
    /// The form instance whose program opened it, when the runner knows it (a form module).
    owner: Option<u32>,
}

/// The files of one run: their declarations, and what each slot holds.
#[derive(Default)]
pub struct Files {
    decls: Arc<Vec<FileDecl>>,
    engine: IndexedEngine,
    open: HashMap<u32, Open>,
    /// What the verbs remember between statements, kept across a CLOSE as the interpreter keeps it.
    cursors: HashMap<u32, Cursor>,
    /// Slots closed `WITH LOCK`: an OPEN is then 38 for the rest of the run.
    locked: HashSet<u32>,
    /// The cross-run-unit lock an INDEXED file holds while it is open.
    share_locks: HashMap<u32, std::fs::File>,
    /// The form instance the program thread is calling into, so an OPEN knows its owner.
    calling: Option<u32>,
    /// The slots opened during the current call, which an `OpenForm` gives to the new instance.
    opened_in_call: Vec<u32>,
}

/// A FILE STATUS as the number the module stores.
fn code(s: &str) -> i32 {
    s.parse().unwrap_or(PERMANENT)
}

fn open_mode(mode: i32) -> Option<OpenMode> {
    Some(match mode {
        verb::OPEN_INPUT => OpenMode::Input,
        verb::OPEN_OUTPUT => OpenMode::Output,
        verb::OPEN_IO => OpenMode::Io,
        verb::OPEN_EXTEND => OpenMode::Extend,
        _ => return None,
    })
}

/// A declared key as the engines take it (the interpreter's `declared_key_spec`).
fn key_spec(k: &KeyDecl, dup: bool) -> KeySpec {
    let parts: Vec<(usize, usize)> = k.parts.iter().map(|&(o, l)| (o as usize, l as usize)).collect();
    if k.split {
        KeySpec::split(parts, dup)
    } else {
        let (offset, len) = parts.first().copied().unwrap_or((0, 0));
        KeySpec { offset, len, duplicates: dup, parts: Vec::new() }
    }
}

/// An INDEXED file's engine description (the interpreter's `make_indexed_engine`).
fn indexed_decl(d: &FileDecl) -> IndexedDecl {
    let reclen = (d.record_len as usize).max(1);
    let primary = d
        .keys
        .first()
        .map(|k| key_spec(k, false))
        .unwrap_or(KeySpec { offset: 0, len: reclen, duplicates: false, parts: Vec::new() });
    IndexedDecl {
        reclen,
        primary,
        alts: d.keys.iter().skip(1).map(|k| key_spec(k, k.dup)).collect(),
        names: d.key_names.clone(),
        memory: d.memory,
        engine: d.engine.as_deref().and_then(|e| match e {
            "redb" => Some(cobolt_ast::program::EngineChoice::Redb),
            "prcidxd1" => Some(cobolt_ast::program::EngineChoice::Prcidxd1),
            _ => None,
        }),
        compressing: d.compress,
        persist: d.persist,
        definition: d.definition.clone(),
    }
}

/// A record number the module sent: negative is none.
fn rel(n: i64) -> Option<u64> {
    u64::try_from(n).ok()
}

/// What a run's files are made from: the module's `prfiles` declarations and the engine a new
/// INDEXED file is created with.
#[derive(Clone, Default)]
pub struct FileSetup {
    pub decls: Arc<Vec<FileDecl>>,
    pub engine: IndexedEngine,
}

impl FileSetup {
    /// The declarations in the module's `prfiles` section (none when it has no such section), and
    /// the engine the project chose (`prsettings`).
    pub fn from_module(section: Option<&[u8]>, project_engine: &str) -> Result<FileSetup, String> {
        let decls = match section {
            Some(bytes) => cobolt_wasm_abi::files::FilesSection::decode(bytes)?.files,
            None => Vec::new(),
        };
        Ok(FileSetup { decls: Arc::new(decls), engine: indexed_engine(project_engine) })
    }
}

impl Files {
    pub fn new(setup: FileSetup) -> Files {
        Files { decls: setup.decls, engine: setup.engine, ..Files::default() }
    }

    fn decl(&self, id: u32) -> Option<&FileDecl> {
        self.decls.get(id as usize)
    }

    fn cursor(&mut self, slot: u32) -> &mut Cursor {
        self.cursors.entry(slot).or_default()
    }

    /// `OPEN` of file `id` at `slot` in `mode` (`verb::OPEN_*`), on the file at `path` (the
    /// ASSIGN's value, trimmed); `exclusive` is `WITH LOCK` or `SHARING WITH NO OTHER`.
    pub fn open(&mut self, id: u32, slot: u32, mode: i32, path: &str, exclusive: bool) -> i32 {
        // Closed WITH LOCK earlier in this run unit (38), or already open (41): nothing changes.
        if self.locked.contains(&slot) {
            return 38;
        }
        if self.open.contains_key(&slot) {
            return 41;
        }
        let (Some(d), Some(mode)) = (self.decl(id).cloned(), open_mode(mode)) else {
            return PERMANENT;
        };
        self.cursor(slot).opened();
        let path = file_core::anchor_path(path);
        // Read before opening: OUTPUT creates the file, and `SELECT OPTIONAL` needs to know whether
        // it was there beforehand.
        let existed = std::path::Path::new(&path).exists();
        let owner = self.calling;
        match d.org {
            Org::Indexed => {
                // Sharing between run units is decided before the engine is built: a refused open
                // must not create or touch the container.
                if !self.share_locks.contains_key(&slot) {
                    match file_core::share_lock(&path, exclusive, None) {
                        ShareLock::Held(h) => {
                            self.share_locks.insert(slot, h);
                        }
                        ShareLock::NotLocked => {}
                        ShareLock::Refused => return 93,
                    }
                }
                let mut engine = file_core::indexed_engine(&indexed_decl(&d), &path, self.engine, LogLevel::Off, LogFormat::Text);
                let status = file_core::open_keyed(d.optional, existed, mode, |m| engine.open(m));
                if status.starts_with('0') {
                    self.open.insert(slot, Open { handle: Handle::Indexed(engine), mode, owner });
                    self.opened_in_call.push(slot);
                } else {
                    self.share_locks.remove(&slot);
                }
                code(status)
            }
            Org::Relative => {
                let mut engine = file_core::relative_engine(&path, d.slot_len as usize, d.memory, d.persist);
                let status = file_core::open_keyed(d.optional, existed, mode, |m| engine.open(m));
                if status.starts_with('0') {
                    self.open.insert(slot, Open { handle: Handle::Relative(Box::new(engine)), mode, owner });
                    self.opened_in_call.push(slot);
                }
                code(status)
            }
            Org::Sequential | Org::LineSequential => {
                if SequentialFile::refuses_absent(existed, d.optional, mode) {
                    return 35;
                }
                match SequentialFile::open(&path, mode, d.optional, d.org == Org::LineSequential) {
                    Ok(f) => {
                        self.open.insert(slot, Open { handle: Handle::Sequential(f), mode, owner });
                        self.opened_in_call.push(slot);
                        code(SequentialFile::opened_status(existed, d.optional, mode))
                    }
                    Err(e) => code(SequentialFile::failed_status(mode, &e)),
                }
            }
        }
    }

    /// `CLOSE` (`verb::CLOSE_*`).
    pub fn close(&mut self, slot: u32, how: i32) -> i32 {
        if how == verb::CLOSE_LOCK {
            self.locked.insert(slot);
        }
        // A disk has no reels: 07, and the file stays open (42 when it is not open).
        if how == verb::CLOSE_REEL {
            return if self.open.contains_key(&slot) { 7 } else { 42 };
        }
        self.cursor(slot).closed();
        // The lock is held until the container is closed, so no other run unit reads it half written.
        let _held_until_closed = self.share_locks.remove(&slot);
        match self.open.remove(&slot) {
            Some(mut f) => code(file_core::close(f.handle.as_mut())),
            None => 42,
        }
    }

    /// `READ`: `how` holds `verb::READ_*`, `key` the key's bytes for a keyed INDEXED read and `rel`
    /// a RELATIVE file's record number. Answers the status, the record on success, and the record
    /// number a sequential RELATIVE read delivered.
    pub fn read(&mut self, id: u32, slot: u32, how: i32, kor: u32, key: Option<&[u8]>, rel_num: i64) -> (i32, Option<Vec<u8>>, Option<u64>) {
        let Some(d) = self.decl(id).cloned() else { return (PERMANENT, None, None) };
        let layout = RecordLayout { len: d.record_len as usize, ..RecordLayout::default() };
        let req = ReadReq {
            random: how & verb::READ_KEYED != 0,
            dir: if how & verb::READ_PREVIOUS != 0 { ReadDir::Previous } else { ReadDir::Next },
            key,
            kor: kor as usize,
            rel_num: (d.org == Org::Relative).then(|| rel(rel_num).unwrap_or(0)),
            rel_digits: d.rel_digits,
            layout: Some(&layout),
            varying: d.varying,
            no_lock: how & verb::READ_NO_LOCK != 0,
        };
        let mut cur = self.cursors.remove(&slot).unwrap_or_default();
        let out = file_core::read(self.open.get_mut(&slot).map(|f| f.handle.as_mut()), &mut cur, &req);
        self.cursors.insert(slot, cur);
        let status = code(out.status);
        (status, out.bytes.filter(|_| status == 0), out.rel_delivered)
    }

    /// `WRITE` of `rec` (at the length the statement gives it). Answers the status and the slot a
    /// RELATIVE write landed in.
    pub fn write(&mut self, id: u32, slot: u32, rec: &[u8], rel_num: i64) -> (i32, Option<u64>) {
        let Some(d) = self.decl(id).cloned() else { return (PERMANENT, None) };
        let in_range = rec.len() >= d.min_len.unwrap_or(0) as usize && rec.len() <= d.max_len.unwrap_or(u32::MAX) as usize;
        // An INDEXED file in the sequential access mode takes its records in ascending RECORD KEY
        // order (the interpreter's `record_key_of`).
        let seq_key = (d.org == Org::Indexed && d.access == Access::Sequential && d.ordered_writes)
            .then(|| d.keys.first().map(|k| key_spec(k, false)))
            .flatten()
            .and_then(|ks| {
                let end = ks.ranges().iter().map(|(o, l)| o + l).max().unwrap_or(0);
                (rec.len() >= end).then(|| ks.key_of(rec))
            });
        let req = WriteReq {
            rec,
            line: None,
            varying: d.varying,
            out_of_range: d.varying && !in_range,
            seq_key,
            rel_key: if d.org == Org::Relative && d.access != Access::Sequential { Some(rel(rel_num).unwrap_or(0)) } else { None },
        };
        let mut cur = self.cursors.remove(&slot).unwrap_or_default();
        let out = file_core::write(self.open.get_mut(&slot).map(|f| f.handle.as_mut()), &mut cur, req);
        self.cursors.insert(slot, cur);
        (code(out.status), out.rel_assigned)
    }

    /// `REWRITE` of `rec`.
    pub fn rewrite(&mut self, id: u32, slot: u32, rec: &[u8], rel_num: i64) -> i32 {
        let Some(d) = self.decl(id).cloned() else { return PERMANENT };
        let io_mode = self.open.get(&slot).is_some_and(|f| f.mode == OpenMode::Io);
        let req = RewriteReq {
            rec,
            random: d.access != Access::Sequential,
            indexed: d.org == Org::Indexed,
            io_mode,
            rel_num: (d.org == Org::Relative).then(|| rel(rel_num).unwrap_or(0)),
        };
        let mut cur = self.cursors.remove(&slot).unwrap_or_default();
        let status = file_core::rewrite(self.open.get_mut(&slot).map(|f| f.handle.as_mut()), &mut cur, req);
        self.cursors.insert(slot, cur);
        code(status)
    }

    /// `DELETE`: `key` is the RECORD KEY's value, `rel` the RELATIVE KEY's.
    pub fn delete(&mut self, id: u32, slot: u32, key: Option<&[u8]>, rel_num: i64) -> i32 {
        let Some(d) = self.decl(id).cloned() else { return PERMANENT };
        let random = d.access != Access::Sequential;
        let rel_num = (d.org == Org::Relative).then(|| rel(rel_num).unwrap_or(0));
        let mut cur = self.cursors.remove(&slot).unwrap_or_default();
        let status = file_core::delete(self.open.get_mut(&slot).map(|f| f.handle.as_mut()), &mut cur, random, key, rel_num);
        self.cursors.insert(slot, cur);
        code(status)
    }

    /// `START` with `op` (`verb::START_*`) on key of reference `kor` (`key` its bytes) or, for a
    /// RELATIVE file, record number `rel`.
    pub fn start(&mut self, id: u32, slot: u32, op: i32, kor: u32, key: Option<&[u8]>, rel_num: i64) -> i32 {
        let Some(d) = self.decl(id).cloned() else { return PERMANENT };
        let op = match op {
            verb::START_GT => StartOp::Gt,
            verb::START_GE => StartOp::Ge,
            verb::START_LT => StartOp::Lt,
            verb::START_LE => StartOp::Le,
            _ => StartOp::Eq,
        };
        let rel_num = (d.org == Org::Relative).then(|| rel(rel_num).unwrap_or(0));
        let mut cur = self.cursors.remove(&slot).unwrap_or_default();
        let status = file_core::start(self.open.get_mut(&slot).map(|f| f.handle.as_mut()), &mut cur, op, kor as usize, key, rel_num);
        self.cursors.insert(slot, cur);
        code(status)
    }

    /// `UNLOCK` (`verb::TX_UNLOCK`, the file in `slot`), `COMMIT` or `ROLLBACK` (every INDEXED file
    /// the calling program holds open), as the interpreter's: only an INDEXED file's engine acts, and
    /// no status is set (A35, T39).
    pub fn tx(&mut self, op: i32, _id: i32, slot: i32) {
        let calling = self.calling;
        for (s, f) in self.open.iter_mut() {
            let Handle::Indexed(engine) = &mut f.handle else { continue };
            match op {
                verb::TX_UNLOCK if *s == slot as u32 => engine.unlock(),
                verb::TX_COMMIT if f.owner == calling => engine.commit(),
                verb::TX_ROLLBACK if f.owner == calling => engine.rollback(),
                _ => {}
            }
        }
    }

    /// The files form instance `inst` opened and left open, let go when its window closes, as the
    /// interpreter of a closed form drops its own: no CLOSE runs, no status is set. The same slots
    /// open afresh when the instance is used again.
    pub fn forget_instance(&mut self, inst: u32) {
        let gone: Vec<u32> = self.open.iter().filter(|(_, f)| f.owner == Some(inst)).map(|(s, _)| *s).collect();
        for slot in gone {
            self.open.remove(&slot);
            self.share_locks.remove(&slot);
            self.cursors.remove(&slot);
            self.locked.remove(&slot);
        }
    }

    /// A call into the module begins: `inst` is the form instance it acts on, when the call names
    /// one (an event, a close); an OPEN during it belongs to that instance.
    pub fn begin_call(&mut self, inst: Option<u32>) {
        self.calling = inst;
        self.opened_in_call.clear();
    }

    /// The form instance an `OpenForm` call made is `inst`: the files its program opened while it
    /// started, before the instance was known, are its own.
    pub fn opened_form(&mut self, inst: u32) {
        for slot in std::mem::take(&mut self.opened_in_call) {
            if let Some(f) = self.open.get_mut(&slot) {
                f.owner.get_or_insert(inst);
            }
        }
    }
}
