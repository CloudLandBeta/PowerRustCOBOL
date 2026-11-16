// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The file verbs on an open file, without the interpreter** (DESKTOP-WASM-001 T38, plan C5).
//!
//! What `OPEN`, `READ`, `WRITE`, `REWRITE`, `DELETE`, `START` and `CLOSE` do to a file once the
//! program has worked out the record, the key and the record number: which engine call each makes,
//! what it remembers for the next statement, and which `FILE STATUS` it answers. These rules were
//! the interpreter's own `exec_*` code and moved here unchanged. The interpreter calls them with
//! the values it reads from its data; the native runner calls them with the bytes a WebAssembly
//! module hands it. A program's files therefore behave alike on both paths.
//!
//! What stays with each caller: finding the file and its path, the record's bytes, the key's and
//! the RELATIVE KEY's values, writing the status into the FILE STATUS item, the statement's
//! phrases and the USE procedures, a report printed into a Viewer, and `LINAGE`.

use crate::files::RecordLayout;
use crate::indexed::{status, IndexedEngine, IndexedStore, KeySpec, OpenMode, ReadDir, StartOp};
use crate::relative::RelativeFile;
use crate::seqfile::{RecordPlace, SeqRead, SequentialFile};

/// An open file of one of the three organisations.
pub enum Handle {
    /// SEQUENTIAL or LINE SEQUENTIAL.
    Sequential(SequentialFile),
    /// INDEXED: a keyed engine, in memory or on disk.
    Indexed(Box<dyn IndexedStore>),
    /// RELATIVE: numbered slots.
    Relative(Box<RelativeFile>),
}

impl Handle {
    /// The file, borrowed for one verb.
    pub fn as_mut(&mut self) -> HandleMut<'_> {
        match self {
            Handle::Sequential(f) => HandleMut::Sequential(f),
            Handle::Indexed(e) => HandleMut::Indexed(e.as_mut()),
            Handle::Relative(e) => HandleMut::Relative(e.as_mut()),
        }
    }
}

/// The open file a verb acts on, borrowed from whoever holds it. `Other` is a file the verbs here
/// do not serve: the interpreter's report printed into a Viewer.
pub enum HandleMut<'a> {
    Sequential(&'a mut SequentialFile),
    Indexed(&'a mut dyn IndexedStore),
    Relative(&'a mut RelativeFile),
    Other,
}

/// What the verbs remember about a file between statements.
#[derive(Debug, Clone, Default)]
pub struct Cursor {
    /// A sequential `READ` reached the end (10 or 14). The next sequential `READ` is then 46.
    pub at_end: bool,
    /// The last I-O statement was a successful `READ`. A `REWRITE` or `DELETE` in the sequential
    /// access mode needs one.
    pub established: bool,
    /// Where that `READ` found its record, for a record-sequential `REWRITE`.
    pub last_read: Option<RecordPlace>,
    /// The RECORD KEY of the last record written to an INDEXED file in the sequential access mode.
    pub last_write_key: Option<Vec<u8>>,
}

impl Cursor {
    /// A fresh `OPEN` positions before the first record again, and leaves no record established for
    /// a `REWRITE` or `DELETE` to act on.
    pub fn opened(&mut self) {
        self.at_end = false;
        self.established = false;
        self.last_write_key = None;
    }

    /// A `CLOSE` forgets the established record and the write sequence.
    pub fn closed(&mut self) {
        self.established = false;
        self.last_write_key = None;
    }
}

// ── OPEN ─────────────────────────────────────────────────────────────────────

/// Where the file `path` names is: joined to the application's folder when it is relative and a
/// folder is anchored (a form application anchors the folder that holds `assets/`), otherwise as
/// written, so the working directory resolves it.
pub fn anchor_path(path: &str) -> String {
    crate::interpreter::anchor_relative_path(path)
}

/// `OPEN` of an INDEXED or RELATIVE file, `open` being the engine's own open.
///
/// `SELECT OPTIONAL` applies to a keyed file exactly as it does to a sequential one. A file that is
/// not there opens anyway, and the program is told so with status **05**. For `INPUT` the engine
/// would refuse a missing file with 35, so the container is made by opening it I-O: reads then find
/// it empty and raise `AT END`, which is what the standard asks for.
pub fn open_keyed(optional: bool, existed: bool, mode: OpenMode, open: impl FnOnce(OpenMode) -> &'static str) -> &'static str {
    let optional_absent = optional && !existed && matches!(mode, OpenMode::Input | OpenMode::Io | OpenMode::Extend);
    let open_as = if optional_absent && mode == OpenMode::Input { OpenMode::Io } else { mode };
    let code = open(open_as);
    if optional_absent && (code == "00" || code == "35") {
        "05"
    } else {
        code
    }
}

/// What an INDEXED file's engine is built from: the record's length, its keys, and the `SELECT`'s
/// clauses that choose and shape the container.
#[derive(Debug, Clone)]
pub struct IndexedDecl {
    /// The primary record description's length (at least 1).
    pub reclen: usize,
    pub primary: KeySpec,
    pub alts: Vec<KeySpec>,
    /// Each key's data-name, the RECORD KEY first.
    pub names: Vec<Option<String>>,
    /// `STORAGE IS MEMORY`.
    pub memory: bool,
    /// `ENGINE IS …`, which governs a file being created.
    pub engine: Option<cobolt_ast::program::EngineChoice>,
    /// `WITH COMPRESSION`.
    pub compressing: bool,
    /// `WITH PERSISTENCE`.
    pub persist: bool,
    /// The FD as COBOL text, which a file this program creates carries inside it.
    pub definition: Option<String>,
}

/// Build an INDEXED file's engine. The concrete backend follows `STORAGE`: MEMORY is the in-RAM
/// engine, DISK the persistent one, whose container is chosen by three sources in this order of
/// authority:
///
///   1. the container's own magic: an existing file names its engine, and nothing overrides that,
///      or the same COBOL could not read data written by a different default;
///   2. `ENGINE IS …` on the SELECT, what the developer chose in the Indexed File editor, which
///      governs the file being CREATED;
///   3. the run's configured engine, for a new file with no clause.
///
/// `WITH COMPRESSION` applies to every engine.
pub fn indexed_engine(
    decl: &IndexedDecl,
    path: &str,
    default: IndexedEngine,
    log_level: crate::indexed_log::LogLevel,
    log_format: crate::indexed_log::LogFormat,
) -> Box<dyn IndexedStore> {
    use crate::indexed::IndexedFile;
    use crate::indexed_disk::DiskIndexedFile;
    use crate::indexed_redb::RedbIndexedFile;
    let (reclen, primary, alts) = (decl.reclen, decl.primary.clone(), decl.alts.clone());
    let engine = detect_container_engine(path).unwrap_or(match decl.engine {
        Some(cobolt_ast::program::EngineChoice::Redb) => IndexedEngine::Redb,
        Some(cobolt_ast::program::EngineChoice::Prcidxd1) => IndexedEngine::Rust,
        None => default,
    });
    // The redb engine is a disk substrate; selecting it routes DISK storage to the crash-safe ACID
    // engine. MEMORY storage always uses the in-RAM engine.
    if engine == IndexedEngine::Redb && !decl.memory {
        let mut e = RedbIndexedFile::new(path, reclen, primary, alts);
        e.set_key_names(decl.names.clone());
        e.set_compressing(decl.compressing);
        e.set_definition(decl.definition.clone());
        e.set_log_level(log_level);
        e.set_log_format(log_format);
        return Box::new(e);
    }
    if decl.memory {
        let mut e = IndexedFile::new(path, reclen, primary, alts);
        e.set_key_names(decl.names.clone());
        e.set_compressing(decl.compressing);
        e.set_persist(decl.persist);
        e.set_definition(decl.definition.clone());
        Box::new(e)
    } else {
        // Rust / RM-COBOL / Fujitsu currently share the PRCIDXD1 container.
        let mut e = DiskIndexedFile::new(path, reclen, primary, alts);
        e.set_key_names(decl.names.clone());
        e.set_compressing(decl.compressing);
        e.set_definition(decl.definition.clone());
        Box::new(e)
    }
}

/// Which engine wrote this container, read from its own magic.
///
/// `None` when the file is absent, too short, or carries no magic we know: a file being created,
/// in other words, where the configured engine decides.
///
/// **The file's format outranks the configured engine, always.** A program must not have to name
/// the engine that happens to match its data: the same COBOL opens a redb container and a PRCIDXD1
/// one, each `SELECT` getting whichever engine wrote the file it points at. Before this, flipping
/// the default left every container written by the previous one unreadable (status 39), which made
/// the default a migration rather than a preference.
///
/// This also means a container keeps its format for life, `OPEN OUTPUT` included. That is
/// deliberate: silently rewriting an ACID redb file as a non-transactional PRCIDXD1 one because a
/// default moved is a worse surprise than keeping it. To change a file's format, delete it and let
/// it be created afresh.
pub fn detect_container_engine(path: &str) -> Option<IndexedEngine> {
    use std::io::Read as _;
    let mut head = [0u8; 8];
    let n = std::fs::File::open(path).ok()?.read(&mut head).ok()?;
    if n >= 4 && &head[0..4] == b"redb" {
        return Some(IndexedEngine::Redb);
    }
    if n >= 8
        && (&head == b"PRCIDXD1"      // the paged B+tree container
            || &head == b"PRCIDX1\0"  // an in-memory file persisted on CLOSE
            || &head == b"PRCISAM1")
    // the legacy records-only container
    {
        return Some(IndexedEngine::Rust);
    }
    None
}

/// Build the numbered-slot engine behind `ORGANIZATION IS RELATIVE`. `slot_len` is the widest
/// record the FD describes, raised to a `RECORD IS VARYING … TO` maximum: a short record still has
/// to fit in a slot sized for a long one.
pub fn relative_engine(path: &str, slot_len: usize, memory: bool, persist: bool) -> RelativeFile {
    let mut e = RelativeFile::new(path, slot_len.max(1), memory);
    e.set_persist(persist);
    e
}

/// The outcome of asking for a file's cross-run-unit lock.
pub enum ShareLock {
    /// Held: the lock lasts as long as this handle.
    Held(std::fs::File),
    /// No lock could be taken, and the file opens anyway: a directory that cannot hold the lock
    /// file, or a file system without locks. Locking is advisory.
    NotLocked,
    /// Another run unit holds a conflicting lock: the `OPEN` is refused with 93.
    Refused,
}

/// Take the advisory lock on `path` that sharing between run units uses: exclusive for a writer
/// or `WITH LOCK`, shared otherwise, waiting up to `wait` for a conflicting one to go away.
///
/// `std::fs::File` has carried advisory locking since Rust 1.89, so this needs no dependency.
pub fn share_lock(path: &str, exclusive: bool, wait: Option<std::time::Duration>) -> ShareLock {
    let lck = format!("{path}.lck");
    let handle = match std::fs::OpenOptions::new().create(true).read(true).write(true).truncate(false).open(&lck) {
        Ok(f) => f,
        // The lock file cannot be made: a read-only directory, say. Do not refuse the open over it:
        // locking is advisory, and failing here would stop programs that used to run.
        Err(_) => return ShareLock::NotLocked,
    };
    let try_once = |h: &std::fs::File| if exclusive { h.try_lock() } else { h.try_lock_shared() };
    let mut got = try_once(&handle);
    if let (Err(std::fs::TryLockError::WouldBlock), Some(wait)) = (&got, wait) {
        let deadline = std::time::Instant::now() + wait;
        // A short nap, capped low: the holder of a file lock keeps it for one OPEN..CLOSE, usually
        // milliseconds, so a long nap makes every waiter sleep past the moment the file came free.
        let mut nap = std::time::Duration::from_millis(1);
        while matches!(got, Err(std::fs::TryLockError::WouldBlock)) && std::time::Instant::now() < deadline {
            std::thread::sleep(nap);
            nap = (nap * 2).min(std::time::Duration::from_millis(8));
            got = try_once(&handle);
        }
    }
    match got {
        Ok(()) => ShareLock::Held(handle),
        // Another run unit holds a conflicting lock. This is the one case that refuses the open.
        Err(std::fs::TryLockError::WouldBlock) => ShareLock::Refused,
        // The platform could not lock at all: a filesystem without it, a network mount. Advisory
        // means advisory: carry on rather than stop a program that ran perfectly well before.
        Err(std::fs::TryLockError::Error(_)) => ShareLock::NotLocked,
    }
}

// ── READ ─────────────────────────────────────────────────────────────────────

/// What a `READ` asks for, worked out by the caller from the statement and the program's data.
pub struct ReadReq<'a> {
    /// A keyed read (by RECORD KEY, `KEY IS`, or record number) rather than the next record.
    pub random: bool,
    pub dir: ReadDir,
    /// The value of the key of reference, for a keyed INDEXED read.
    pub key: Option<&'a [u8]>,
    /// The key of reference: 0 for the RECORD KEY, `n` for the nth alternate key.
    pub kor: usize,
    /// A RELATIVE file's RELATIVE KEY value (`None` for the other organisations).
    pub rel_num: Option<u64>,
    /// The integer digits of the RELATIVE KEY item, when it has any.
    pub rel_digits: Option<u32>,
    /// The primary record description, for a sequential file's record length and line text.
    pub layout: Option<&'a RecordLayout>,
    /// The file's records vary in length.
    pub varying: bool,
    /// `READ … WITH NO LOCK`.
    pub no_lock: bool,
}

/// What a `READ` delivered.
pub struct ReadOut {
    pub bytes: Option<Vec<u8>>,
    pub status: &'static str,
    /// The record number a sequential RELATIVE read delivered, for the RELATIVE KEY item.
    pub rel_delivered: Option<u64>,
}

/// `READ`, dispatched by organisation.
pub fn read(h: Option<HandleMut<'_>>, cur: &mut Cursor, req: &ReadReq<'_>) -> ReadOut {
    // A sequential READ once AT END has been reached is 46, not a second 10: the AT END left no valid
    // next record, and reading on is a different error from reaching the end (SQ136A–SQ138A). Nothing
    // the file remembers changes.
    if !req.random && cur.at_end {
        return ReadOut { bytes: None, status: "46", rel_delivered: None };
    }
    // A record-SEQUENTIAL read also records where it found the record, so a following REWRITE knows
    // what it is replacing.
    let mut read_at: Option<RecordPlace> = None;
    // Which slot a RELATIVE read actually delivered, so the RELATIVE KEY item can be told. A
    // sequential read is how the program discovers it.
    let mut rel_delivered: Option<u64> = None;
    let (mut buf, mut code): (Option<Vec<u8>>, &'static str) = match h {
        Some(HandleMut::Indexed(engine)) => {
            let got = if req.random {
                engine.set_key_of_reference(req.kor);
                match req.key {
                    Some(kb) => engine.read_key(kb),
                    None => (None, status::NOT_FOUND),
                }
            } else {
                engine.read_seq(req.dir)
            };
            // `READ … WITH NO LOCK` releases the lock the engine takes under I-O.
            if req.no_lock {
                engine.unlock();
            }
            got
        }
        Some(HandleMut::Relative(engine)) => {
            if req.random {
                engine.read_key(req.rel_num.unwrap_or(0))
            } else {
                let (rec, n, code) = engine.read_seq(req.dir);
                if code == status::OK {
                    rel_delivered = Some(n);
                }
                (rec, code)
            }
        }
        // SEQUENTIAL / LINE SEQUENTIAL: newline-delimited text records, or records back to back. A
        // national or UTF-8 field is text in the line and an image in the record (spec 077, D10).
        Some(HandleMut::Sequential(f)) => match f.read(req.layout, req.varying) {
            SeqRead::Record { bytes, place } => {
                read_at = place;
                (Some(bytes), status::OK)
            }
            SeqRead::End => (None, status::EOF),
            SeqRead::Failed(e) => {
                tracing::warn!("READ failed: {e}");
                (None, "30")
            }
            SeqRead::NotInput => (None, status::NOT_OPEN_INPUT),
        },
        _ => (None, status::NOT_OPEN_INPUT),
    };

    // **Status 14**: a sequential READ of a relative file whose record number has more significant
    // digits than the RELATIVE KEY item can hold. The program asked to be told where each record
    // sits and the item it supplied cannot say, so the read is unsuccessful: no record is delivered,
    // the key item is left alone, and (14 being an at-end class condition like 10) the AT END phrase
    // is what handles it.
    if let Some(n) = rel_delivered {
        if req.rel_digits.is_some_and(|d| n.to_string().len() as u32 > d) {
            rel_delivered = None;
            buf = None;
            code = "14";
        }
    }

    // Latch or release the end-of-file position this READ leaves behind. 14 latches with 10: both
    // are at-end class conditions that leave the file with no valid next record.
    if (code == status::EOF || code == "14") && !req.random {
        cur.at_end = true;
    } else if code == status::OK {
        cur.at_end = false;
    }
    // Only a *successful* read establishes a record to REWRITE; anything else (AT END included)
    // leaves the file with none, which is what makes a following REWRITE status 43 (SQ133A).
    cur.last_read = match read_at {
        Some(at) if code == status::OK => Some(at),
        _ => None,
    };
    // The same fact, for a file with no byte offset to remember it by.
    cur.established = code == status::OK;
    ReadOut { bytes: buf, status: code, rel_delivered }
}

// ── WRITE ────────────────────────────────────────────────────────────────────

/// What a `WRITE` asks for.
pub struct WriteReq<'a> {
    /// The record's image, at the length it is written.
    pub rec: &'a [u8],
    /// A LINE SEQUENTIAL file's line for it, when it differs from the image (national fields).
    pub line: Option<&'a [u8]>,
    pub varying: bool,
    /// The length is outside the FD's `FROM … TO` range: a boundary violation, nothing written (44).
    pub out_of_range: bool,
    /// The record's RECORD KEY, for an INDEXED file in the sequential access mode, which must be
    /// written in ascending key order.
    pub seq_key: Option<Vec<u8>>,
    /// A RELATIVE file's record number under RANDOM or DYNAMIC access; `None` lets the engine
    /// assign the next slot.
    pub rel_key: Option<u64>,
}

/// What a `WRITE` did.
pub struct WriteOut {
    pub status: &'static str,
    /// The slot a RELATIVE write landed in.
    pub rel_assigned: Option<u64>,
}

/// `WRITE`, dispatched by organisation.
pub fn write(h: Option<HandleMut<'_>>, cur: &mut Cursor, req: WriteReq<'_>) -> WriteOut {
    // Writing an INDEXED file in the sequential access mode requires ascending RECORD KEY order. The
    // key this WRITE carries is compared against the one before it, and a key that is not greater is
    // status 21 with nothing written (IX109A writes 1…50 and then 49, expecting 21). Under RANDOM or
    // DYNAMIC access any order is allowed, and a clash with an existing record is the engine's 22.
    let out_of_sequence = match (&req.seq_key, &cur.last_write_key) {
        (Some(now), Some(prev)) => now.as_slice() <= prev.as_slice(),
        _ => false,
    };
    let mut rel_assigned: Option<u64> = None;
    let code = match h {
        _ if req.out_of_range => "44",
        _ if out_of_sequence => status::SEQUENCE_ERROR,
        Some(HandleMut::Indexed(engine)) => engine.write(req.rec),
        Some(HandleMut::Relative(engine)) => {
            let (code, n) = engine.write(req.rec, req.rel_key);
            rel_assigned = Some(n);
            code
        }
        Some(HandleMut::Sequential(f)) if f.is_output() => match f.write(req.rec, req.line, req.varying) {
            Some(Ok(())) => "00",
            Some(Err(e)) => {
                tracing::warn!("WRITE failed: {e}");
                "30"
            }
            // Not reached: the guard holds an output file.
            None => "48",
        },
        _ => {
            tracing::warn!("WRITE to a file which is not open for output");
            "48"
        }
    };
    // A WRITE is an I-O statement like any other: it displaces whatever record a previous READ had
    // established.
    cur.established = false;
    // Only a record that actually went in moves the sequence forward; a rejected one leaves the
    // previous key standing.
    if code == "00" || code == "02" {
        if let Some(k) = req.seq_key {
            cur.last_write_key = Some(k);
        }
    }
    WriteOut { status: code, rel_assigned }
}

// ── REWRITE / DELETE / START ─────────────────────────────────────────────────

/// What a `REWRITE` asks for.
pub struct RewriteReq<'a> {
    pub rec: &'a [u8],
    /// The access mode addresses records by key (RANDOM or DYNAMIC).
    pub random: bool,
    /// The file is INDEXED: a sequential file's last-read place survives the REWRITE otherwise not.
    pub indexed: bool,
    /// The file is open I-O.
    pub io_mode: bool,
    /// A RELATIVE file's RELATIVE KEY value.
    pub rel_num: Option<u64>,
}

/// `REWRITE`, dispatched by organisation.
pub fn rewrite(h: Option<HandleMut<'_>>, cur: &mut Cursor, req: RewriteReq<'_>) -> &'static str {
    let code = match h {
        // In the sequential access mode a REWRITE replaces the record the previous READ delivered,
        // so the standard requires one: with none, the status is 43 (IX120A).
        Some(HandleMut::Indexed(_)) if !req.random && !cur.established => "43",
        Some(HandleMut::Indexed(engine)) => engine.rewrite(req.rec, if req.random { Some(req.rec) } else { None }),
        // The same rule for a numbered slot.
        Some(HandleMut::Relative(_)) if !req.random && !cur.established => "43",
        Some(HandleMut::Relative(engine)) => engine.rewrite(req.rec, if req.random { req.rel_num } else { None }),
        // Record SEQUENTIAL, open I-O: replace the record the last READ delivered, in place (49 if
        // not open I-O, 43 with no record read, 44 for a different length).
        Some(HandleMut::Sequential(f)) if f.is_input() && !f.is_line_sequential() => f.rewrite(req.io_mode, cur.last_read, req.rec),
        Some(_) => {
            tracing::warn!("REWRITE on a file which is not open for I-O");
            "49"
        }
        None => status::NOT_OPEN_IO,
    };
    // The record is consumed either way: a second REWRITE with no READ between them is 43.
    if !req.indexed {
        cur.last_read = None;
    }
    cur.established = false;
    code
}

/// `DELETE`, dispatched by organisation. `key` is the RECORD KEY's value, `rel_num` the RELATIVE
/// KEY's.
pub fn delete(h: Option<HandleMut<'_>>, cur: &mut Cursor, random: bool, key: Option<&[u8]>, rel_num: Option<u64>) -> &'static str {
    let code = match h {
        // A sequential DELETE removes the record the previous READ delivered, and so requires one:
        // the same rule, and the same status 43, that governs a sequential REWRITE (IX119A).
        Some(HandleMut::Indexed(_)) if !random && !cur.established => "43",
        Some(HandleMut::Indexed(engine)) => engine.delete(if random { key } else { None }),
        Some(HandleMut::Relative(_)) if !random && !cur.established => "43",
        // The slot is emptied, not removed: its number stays addressable.
        Some(HandleMut::Relative(engine)) => engine.delete(if random { rel_num } else { None }),
        Some(_) => {
            tracing::warn!("DELETE on a sequential file is not valid");
            "37"
        }
        None => status::NOT_OPEN_IO,
    };
    // The record is consumed: a second DELETE with no READ between them is 43.
    cur.established = false;
    code
}

/// `START`, dispatched by organisation: `kor` and `key` for an INDEXED file, `rel_num` for a
/// RELATIVE one.
pub fn start(h: Option<HandleMut<'_>>, cur: &mut Cursor, op: StartOp, kor: usize, key: Option<&[u8]>, rel_num: Option<u64>) -> &'static str {
    let code = match h {
        Some(HandleMut::Indexed(engine)) => {
            engine.set_key_of_reference(kor);
            match key {
                Some(kb) => engine.start(op, kb),
                None => status::NOT_FOUND,
            }
        }
        Some(HandleMut::Relative(engine)) => engine.start(op, rel_num.unwrap_or(0)),
        Some(_) => {
            tracing::warn!("START on a sequential file is not valid");
            "30"
        }
        None => status::NOT_OPEN_INPUT,
    };
    // A successful START establishes a position again, so a following sequential READ is a normal
    // read, not the 46 a past AT END would otherwise leave standing.
    if code == status::OK {
        cur.at_end = false;
    }
    // START only positions the file; it delivers no record.
    cur.established = false;
    code
}

/// `CLOSE` of an open file.
pub fn close(h: HandleMut<'_>) -> &'static str {
    match h {
        HandleMut::Sequential(f) => f.close(),
        HandleMut::Indexed(engine) => engine.close(),
        HandleMut::Relative(engine) => engine.close(),
        HandleMut::Other => status::OK,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexed::ReadDir;

    fn key(offset: usize, len: usize) -> KeySpec {
        KeySpec { offset, len, duplicates: false, parts: Vec::new() }
    }

    fn indexed(path: &str, memory: bool) -> Handle {
        let decl = IndexedDecl {
            reclen: 8,
            primary: key(0, 3),
            alts: vec![],
            names: vec![Some("K".into())],
            memory,
            engine: None,
            compressing: false,
            persist: false,
            definition: None,
        };
        Handle::Indexed(indexed_engine(&decl, path, IndexedEngine::Rust, Default::default(), Default::default()))
    }

    fn seq_read(h: &mut Handle, cur: &mut Cursor) -> ReadOut {
        let req = ReadReq { random: false, dir: ReadDir::Next, key: None, kor: 0, rel_num: None, rel_digits: None, layout: None, varying: false, no_lock: false };
        read(Some(h.as_mut()), cur, &req)
    }

    /// The statuses the verbs answer on an INDEXED file in the sequential access mode: the key
    /// order (21), the end (10, then 46), and a REWRITE or DELETE with no record read (43).
    #[test]
    fn indexed_statuses_follow_the_interpreters_rules() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ix.dat").to_string_lossy().into_owned();
        let mut h = indexed(&path, false);
        let mut cur = Cursor::default();
        let mut tally = (0, 0);
        let mut check = |what: &str, got: &str, want: &str| {
            assert_eq!(got, want, "{what}");
            tally.0 += 1;
            if got == want {
                tally.1 += 1;
            }
        };
        let HandleMut::Indexed(e) = h.as_mut() else { unreachable!() };
        check("open output", e.open(OpenMode::Output), "00");
        cur.opened();
        for (rec, want) in [(b"001AAAAA", "00"), (b"003CCCCC", "00"), (b"002BBBBB", "21")] {
            let w = write(Some(h.as_mut()), &mut cur, WriteReq { rec, line: None, varying: false, out_of_range: false, seq_key: Some(rec[..3].to_vec()), rel_key: None });
            check("write in key order", w.status, want);
        }
        let HandleMut::Indexed(e) = h.as_mut() else { unreachable!() };
        check("close", e.close(), "00");
        cur.closed();
        let HandleMut::Indexed(e) = h.as_mut() else { unreachable!() };
        check("open i-o", e.open(OpenMode::Io), "00");
        cur.opened();
        let req = RewriteReq { rec: b"001ZZZZZ", random: false, indexed: true, io_mode: true, rel_num: None };
        check("rewrite before a read", rewrite(Some(h.as_mut()), &mut cur, req), "43");
        check("delete before a read", delete(Some(h.as_mut()), &mut cur, false, None, None), "43");
        check("first read", seq_read(&mut h, &mut cur).status, "00");
        let req = RewriteReq { rec: b"001ZZZZZ", random: false, indexed: true, io_mode: true, rel_num: None };
        check("rewrite after a read", rewrite(Some(h.as_mut()), &mut cur, req), "00");
        check("second read", seq_read(&mut h, &mut cur).status, "00");
        check("at end", seq_read(&mut h, &mut cur).status, "10");
        check("past the end", seq_read(&mut h, &mut cur).status, "46");
        check("start", start(Some(h.as_mut()), &mut cur, StartOp::Ge, 0, Some(b"001"), None), "00");
        let r = seq_read(&mut h, &mut cur);
        check("read after start", r.status, "00");
        assert_eq!(r.bytes.as_deref(), Some(&b"001ZZZZZ"[..]));
        check("close", close(h.as_mut()), "00");
        check("read when closed", read(None, &mut cur, &ReadReq { random: true, dir: ReadDir::Next, key: Some(b"001"), kor: 0, rel_num: None, rel_digits: None, layout: None, varying: false, no_lock: false }).status, "47");
        println!("file_core INDEXED statuses: {} of {} as the interpreter answers (21, 43, 10, 46, 47)", tally.1, tally.0);
    }

    /// A RELATIVE file: a sequential write is given its slot, a sequential read reports the slot it
    /// delivered, and one that does not fit the RELATIVE KEY item is 14.
    #[test]
    fn relative_reports_slots_and_status_14() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rl.dat").to_string_lossy().into_owned();
        let mut h = Handle::Relative(Box::new(relative_engine(&path, 4, false, false)));
        let mut cur = Cursor::default();
        let HandleMut::Relative(e) = h.as_mut() else { unreachable!() };
        assert_eq!(e.open(OpenMode::Output), "00");
        let mut slots = Vec::new();
        for rec in [b"AAAA", b"BBBB"] {
            let w = write(Some(h.as_mut()), &mut cur, WriteReq { rec, line: None, varying: false, out_of_range: false, seq_key: None, rel_key: None });
            assert_eq!(w.status, "00");
            slots.push(w.rel_assigned);
        }
        assert_eq!(slots, vec![Some(1), Some(2)]);
        let w = write(Some(h.as_mut()), &mut cur, WriteReq { rec: b"JJJJ", line: None, varying: false, out_of_range: false, seq_key: None, rel_key: Some(10) });
        assert_eq!(w.status, "00");
        assert_eq!(close(h.as_mut()), "00");
        let HandleMut::Relative(e) = h.as_mut() else { unreachable!() };
        assert_eq!(e.open(OpenMode::Input), "00");
        cur.opened();
        let req = ReadReq { random: false, dir: ReadDir::Next, key: None, kor: 0, rel_num: Some(0), rel_digits: Some(1), layout: None, varying: false, no_lock: false };
        let mut got = Vec::new();
        for _ in 0..4 {
            let r = read(Some(h.as_mut()), &mut cur, &req);
            got.push((r.status, r.rel_delivered));
        }
        assert_eq!(got, vec![("00", Some(1)), ("00", Some(2)), ("14", None), ("46", None)]);
        println!("file_core RELATIVE: slots 1 and 2 assigned, reads {got:?}");
    }

    /// `SELECT OPTIONAL`: a keyed file that is not there opens with 05.
    #[test]
    fn optional_keyed_file_opens_with_05() {
        assert_eq!(open_keyed(true, false, OpenMode::Input, |m| if m == OpenMode::Io { "00" } else { "35" }), "05");
        assert_eq!(open_keyed(false, false, OpenMode::Input, |_| "35"), "35");
        assert_eq!(open_keyed(true, true, OpenMode::Input, |_| "00"), "00");
        println!("file_core OPTIONAL: 3 of 3 opens answer 05, 35, 00");
    }
}
