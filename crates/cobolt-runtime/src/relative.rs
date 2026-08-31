//! `ORGANIZATION IS RELATIVE` — the numbered-slot file engine.
//!
//! A relative file is a table of **numbered slots**, not a list of records.
//! Slot *n* either holds a record or is empty, and an empty slot keeps its
//! number: deleting record 7 does not renumber record 8. Every access — random,
//! or sequential in ascending slot order — is by that number, the RELATIVE KEY,
//! which the program keeps in WORKING-STORAGE rather than inside the record.
//!
//! That last point is the whole reason this is a separate engine rather than a
//! mode of the indexed ones: [`crate::indexed::IndexedStore`] extracts its keys
//! *from the record bytes*, and a relative file has nowhere to put one.
//!
//! Two containers, chosen by `STORAGE [MODE] IS MEMORY | DISK` exactly as the
//! indexed engines choose theirs:
//!
//! * **MEMORY** — a `BTreeMap` from slot number to record. Ordered, so a
//!   sequential read is a range scan.
//! * **DISK** — container `PRCREL1`: a fixed header followed by equal-sized
//!   slots, so slot *n* is one seek away and the file needs no in-RAM
//!   directory. Each slot carries a presence flag *and* the record's own
//!   length, which is what lets a `RECORD IS VARYING` file store a short record
//!   without padding it into ambiguity.
//!
//! Both containers answer the same statuses, because the COBOL program must not
//! be able to tell them apart.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::indexed::{status, Bytes, OpenMode, ReadDir, StartOp};

/// Container magic — a relative file, version 1.
const MAGIC: &[u8; 8] = b"PRCREL1\0";
/// Bytes before the first slot.
const HEADER_LEN: u64 = 32;
/// Per-slot overhead: one presence flag plus a big-endian `u32` record length.
const SLOT_OVERHEAD: u64 = 5;

/// Where the slots actually live.
enum Container {
    /// `STORAGE IS MEMORY` — slot number → record.
    Memory(BTreeMap<u64, Bytes>),
    /// `STORAGE IS DISK` — the `PRCREL1` container, seeked into directly.
    Disk(File),
}

/// One open `ORGANIZATION IS RELATIVE` file.
pub struct RelativeFile {
    path: PathBuf,
    /// The largest record the FD describes — the slot width on disk.
    max_len: usize,
    /// Which container this file wants when it is opened.
    want_memory: bool,
    /// `STORAGE IS MEMORY WITH PERSISTENCE` — write the container out on CLOSE.
    persist: bool,
    container: Option<Container>,
    mode: Option<OpenMode>,
    /// Highest slot number the file has ever allocated. Slots at or below it
    /// may be empty; slots above it do not exist.
    slot_count: u64,
    /// Lowest slot a `READ NEXT` may deliver.
    next_from: u64,
    /// Highest slot a `READ PREVIOUS` may deliver.
    prev_from: u64,
    /// The slot a sequential `WRITE` fills next.
    next_write: u64,
    /// The slot the last successful READ delivered — what a sequential REWRITE
    /// or DELETE acts on.
    current: Option<u64>,
}

impl RelativeFile {
    pub fn new(path: &str, max_len: usize, memory: bool) -> Self {
        Self {
            path: PathBuf::from(path),
            max_len: max_len.max(1),
            want_memory: memory,
            persist: false,
            container: None,
            mode: None,
            slot_count: 0,
            next_from: 1,
            prev_from: 0,
            next_write: 1,
            current: None,
        }
    }

    /// `STORAGE IS MEMORY WITH PERSISTENCE` — keep the container on disk too.
    pub fn set_persist(&mut self, persist: bool) {
        self.persist = persist;
    }

    pub fn is_open(&self) -> bool {
        self.container.is_some()
    }

    /// The slot width on disk, including the flag and length prefix.
    fn slot_size(&self) -> u64 {
        SLOT_OVERHEAD + self.max_len as u64
    }

    fn slot_offset(&self, n: u64) -> u64 {
        HEADER_LEN + (n - 1) * self.slot_size()
    }

    // ── container primitives ────────────────────────────────────────────────
    //
    // Everything above these three is shared between MEMORY and DISK, so the
    // access-mode and status rules are written once.

    /// The record in slot `n`, or `None` when the slot is empty or beyond the
    /// end of the file.
    fn slot_get(&mut self, n: u64) -> Option<Bytes> {
        if n == 0 || n > self.slot_count {
            return None;
        }
        let off = self.slot_offset(n);
        let width = self.max_len;
        match self.container.as_mut()? {
            Container::Memory(map) => map.get(&n).cloned(),
            Container::Disk(f) => {
                let mut buf = vec![0u8; SLOT_OVERHEAD as usize + width];
                f.seek(SeekFrom::Start(off)).ok()?;
                f.read_exact(&mut buf).ok()?;
                if buf[0] != 1 {
                    return None;
                }
                let len = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
                let len = len.min(width);
                Some(buf[SLOT_OVERHEAD as usize..SLOT_OVERHEAD as usize + len].to_vec())
            }
        }
    }

    /// Store `rec` in slot `n`, growing the file with empty slots if it has to.
    fn slot_put(&mut self, n: u64, rec: &[u8]) -> std::io::Result<()> {
        let rec = &rec[..rec.len().min(self.max_len)];
        let off = self.slot_offset(n);
        let width = self.max_len;
        let grow_from = self.slot_count;
        match self.container.as_mut() {
            Some(Container::Memory(map)) => {
                map.insert(n, rec.to_vec());
            }
            Some(Container::Disk(f)) => {
                // Slots between the old end and `n` must exist and read as
                // empty: a relative file is addressed by number, so the gap a
                // random WRITE leaves behind is part of the file.
                if n > grow_from {
                    let blank = vec![0u8; SLOT_OVERHEAD as usize + width];
                    f.seek(SeekFrom::Start(
                        HEADER_LEN + grow_from * (SLOT_OVERHEAD + width as u64),
                    ))?;
                    for _ in grow_from..n - 1 {
                        f.write_all(&blank)?;
                    }
                }
                let mut buf = vec![b' '; SLOT_OVERHEAD as usize + width];
                buf[0] = 1;
                buf[1..5].copy_from_slice(&(rec.len() as u32).to_be_bytes());
                buf[SLOT_OVERHEAD as usize..SLOT_OVERHEAD as usize + rec.len()]
                    .copy_from_slice(rec);
                f.seek(SeekFrom::Start(off))?;
                f.write_all(&buf)?;
            }
            None => return Ok(()),
        }
        if n > self.slot_count {
            self.slot_count = n;
            self.write_header()?;
        }
        Ok(())
    }

    /// Empty slot `n`. The slot itself stays — that is what makes a deleted
    /// record's number still addressable.
    fn slot_erase(&mut self, n: u64) -> std::io::Result<()> {
        let off = self.slot_offset(n);
        match self.container.as_mut() {
            Some(Container::Memory(map)) => {
                map.remove(&n);
            }
            Some(Container::Disk(f)) => {
                f.seek(SeekFrom::Start(off))?;
                f.write_all(&[0u8; SLOT_OVERHEAD as usize])?;
            }
            None => {}
        }
        Ok(())
    }

    /// The first slot at or after `from` that holds a record.
    fn scan_forward(&mut self, from: u64) -> Option<u64> {
        let mut n = from.max(1);
        while n <= self.slot_count {
            if self.slot_get(n).is_some() {
                return Some(n);
            }
            n += 1;
        }
        None
    }

    /// The last slot at or before `from` that holds a record.
    fn scan_backward(&mut self, from: u64) -> Option<u64> {
        let mut n = from.min(self.slot_count);
        while n >= 1 {
            if self.slot_get(n).is_some() {
                return Some(n);
            }
            n -= 1;
        }
        None
    }

    // ── container lifecycle ─────────────────────────────────────────────────

    fn write_header(&mut self) -> std::io::Result<()> {
        let max_len = self.max_len as u32;
        let count = self.slot_count;
        if let Some(Container::Disk(f)) = self.container.as_mut() {
            let mut hdr = [0u8; HEADER_LEN as usize];
            hdr[..8].copy_from_slice(MAGIC);
            hdr[8..10].copy_from_slice(&1u16.to_be_bytes());
            hdr[12..16].copy_from_slice(&max_len.to_be_bytes());
            hdr[16..24].copy_from_slice(&count.to_be_bytes());
            f.seek(SeekFrom::Start(0))?;
            f.write_all(&hdr)?;
        }
        Ok(())
    }

    /// Read an existing container's header. Returns the status the OPEN should
    /// report: `00` when it was read, `39` when the file is not a `PRCREL1`
    /// container at all.
    fn read_header(&mut self) -> &'static str {
        let Some(Container::Disk(f)) = self.container.as_mut() else {
            return status::OK;
        };
        let mut hdr = [0u8; HEADER_LEN as usize];
        if f.seek(SeekFrom::Start(0)).is_err() || f.read_exact(&mut hdr).is_err() {
            // A zero-length file is a container waiting to be written, not a
            // broken one: OPEN OUTPUT and OPEN I-O of an OPTIONAL file both
            // land here.
            self.slot_count = 0;
            return match self.write_header() {
                Ok(()) => status::OK,
                Err(_) => status::IO_ERROR,
            };
        }
        if &hdr[..8] != MAGIC {
            return status::ATTR_MISMATCH;
        }
        // The container's own record width wins over the FD's. A relative file
        // written by one program is read by another whose FD may describe a
        // shorter record, and the slots do not move because of it.
        let stored = u32::from_be_bytes([hdr[12], hdr[13], hdr[14], hdr[15]]) as usize;
        if stored > 0 {
            self.max_len = stored;
        }
        self.slot_count = u64::from_be_bytes([
            hdr[16], hdr[17], hdr[18], hdr[19], hdr[20], hdr[21], hdr[22], hdr[23],
        ]);
        status::OK
    }

    /// Load a `PRCREL1` container into the in-memory map (`STORAGE IS MEMORY
    /// WITH PERSISTENCE`).
    fn load_memory_image(&mut self) {
        let Ok(mut f) = File::open(&self.path) else {
            return;
        };
        let mut hdr = [0u8; HEADER_LEN as usize];
        if f.read_exact(&mut hdr).is_err() || &hdr[..8] != MAGIC {
            return;
        }
        let stored = u32::from_be_bytes([hdr[12], hdr[13], hdr[14], hdr[15]]) as usize;
        if stored > 0 {
            self.max_len = stored;
        }
        let count = u64::from_be_bytes([
            hdr[16], hdr[17], hdr[18], hdr[19], hdr[20], hdr[21], hdr[22], hdr[23],
        ]);
        let mut map = BTreeMap::new();
        let mut buf = vec![0u8; SLOT_OVERHEAD as usize + self.max_len];
        for n in 1..=count {
            if f.read_exact(&mut buf).is_err() {
                break;
            }
            if buf[0] == 1 {
                let len = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
                let len = len.min(self.max_len);
                map.insert(n, buf[SLOT_OVERHEAD as usize..SLOT_OVERHEAD as usize + len].to_vec());
            }
        }
        self.slot_count = count;
        self.container = Some(Container::Memory(map));
    }

    /// Write the in-memory map out as a `PRCREL1` container.
    fn save_memory_image(&mut self) {
        let Some(Container::Memory(map)) = self.container.as_ref() else {
            return;
        };
        let Ok(mut f) = File::create(&self.path) else {
            return;
        };
        let mut hdr = [0u8; HEADER_LEN as usize];
        hdr[..8].copy_from_slice(MAGIC);
        hdr[8..10].copy_from_slice(&1u16.to_be_bytes());
        hdr[12..16].copy_from_slice(&(self.max_len as u32).to_be_bytes());
        hdr[16..24].copy_from_slice(&self.slot_count.to_be_bytes());
        if f.write_all(&hdr).is_err() {
            return;
        }
        let width = self.max_len;
        for n in 1..=self.slot_count {
            let mut buf = vec![b' '; SLOT_OVERHEAD as usize + width];
            if let Some(rec) = map.get(&n) {
                let len = rec.len().min(width);
                buf[0] = 1;
                buf[1..5].copy_from_slice(&(len as u32).to_be_bytes());
                buf[SLOT_OVERHEAD as usize..SLOT_OVERHEAD as usize + len]
                    .copy_from_slice(&rec[..len]);
            } else {
                buf[..SLOT_OVERHEAD as usize].fill(0);
            }
            if f.write_all(&buf).is_err() {
                return;
            }
        }
    }

    // ── the verbs ───────────────────────────────────────────────────────────

    pub fn open(&mut self, mode: OpenMode) -> &'static str {
        let existed = Path::new(&self.path).exists();
        if !existed && matches!(mode, OpenMode::Input | OpenMode::Io | OpenMode::Extend) {
            return status::FILE_NOT_FOUND;
        }
        self.slot_count = 0;
        self.current = None;
        if self.want_memory {
            self.container = Some(Container::Memory(BTreeMap::new()));
            if self.persist && existed && mode != OpenMode::Output {
                self.load_memory_image();
            }
        } else {
            let opened = match mode {
                OpenMode::Output => OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&self.path),
                OpenMode::Input => OpenOptions::new().read(true).open(&self.path),
                OpenMode::Io | OpenMode::Extend => OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(&self.path),
            };
            match opened {
                Ok(f) => self.container = Some(Container::Disk(f)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return status::FILE_NOT_FOUND
                }
                Err(_) => return status::IO_ERROR,
            }
            let code = self.read_header();
            if code != status::OK {
                self.container = None;
                return code;
            }
        }
        self.mode = Some(mode);
        self.next_from = 1;
        self.prev_from = self.slot_count;
        // OUTPUT builds the file from slot 1; EXTEND continues past the last
        // slot that exists, whether or not it holds a record.
        self.next_write = match mode {
            OpenMode::Extend => self.slot_count + 1,
            _ => 1,
        };
        status::OK
    }

    pub fn close(&mut self) -> &'static str {
        if self.container.is_none() {
            return status::LOGIC_ERROR;
        }
        if self.want_memory && self.persist {
            self.save_memory_image();
        }
        if let Some(Container::Disk(f)) = self.container.as_mut() {
            let _ = f.flush();
        }
        self.container = None;
        self.mode = None;
        self.current = None;
        status::OK
    }

    /// `WRITE`. `relnum` is the RELATIVE KEY the program set — `None` in the
    /// sequential access mode, where the engine assigns the number instead and
    /// hands it back so the caller can store it in the key item.
    pub fn write(&mut self, rec: &[u8], relnum: Option<u64>) -> (&'static str, u64) {
        if self.container.is_none() {
            return (status::NOT_OPEN_OUTPUT, 0);
        }
        if !matches!(
            self.mode,
            Some(OpenMode::Output) | Some(OpenMode::Extend) | Some(OpenMode::Io)
        ) {
            return (status::NOT_OPEN_OUTPUT, 0);
        }
        let n = match relnum {
            // A RELATIVE KEY of zero addresses no slot: the file has none, and
            // asking for one is a boundary violation rather than a missing
            // record.
            Some(0) => return (status::BOUNDARY, 0),
            Some(n) => n,
            None => self.next_write,
        };
        // Random WRITE onto a slot that already holds a record is the relative
        // file's duplicate-key condition.
        if relnum.is_some() && self.slot_get(n).is_some() {
            return (status::DUP_KEY, n);
        }
        match self.slot_put(n, rec) {
            Ok(()) => {
                if relnum.is_none() {
                    self.next_write = n + 1;
                }
                self.next_from = n + 1;
                self.prev_from = n.saturating_sub(1);
                (status::OK, n)
            }
            Err(e) => {
                tracing::warn!("RELATIVE WRITE failed: {e}");
                (status::IO_ERROR, n)
            }
        }
    }

    /// Random `READ` — the record in the slot the RELATIVE KEY names.
    pub fn read_key(&mut self, relnum: u64) -> (Option<Bytes>, &'static str) {
        if self.container.is_none() {
            return (None, status::NOT_OPEN_INPUT);
        }
        if !matches!(self.mode, Some(OpenMode::Input) | Some(OpenMode::Io)) {
            return (None, status::NOT_OPEN_INPUT);
        }
        match self.slot_get(relnum) {
            Some(rec) => {
                self.current = Some(relnum);
                self.next_from = relnum + 1;
                self.prev_from = relnum.saturating_sub(1);
                (Some(rec), status::OK)
            }
            // An empty slot, or one past the end of the file, is "record not
            // found" — the invalid-key condition, not end-of-file.
            None => {
                self.current = None;
                (None, status::NOT_FOUND)
            }
        }
    }

    /// Sequential `READ NEXT` / `READ PREVIOUS`. Returns the record *and* the
    /// slot it came from, which the caller puts in the RELATIVE KEY item.
    pub fn read_seq(&mut self, dir: ReadDir) -> (Option<Bytes>, u64, &'static str) {
        if self.container.is_none() {
            return (None, 0, status::NOT_OPEN_INPUT);
        }
        if !matches!(self.mode, Some(OpenMode::Input) | Some(OpenMode::Io)) {
            return (None, 0, status::NOT_OPEN_INPUT);
        }
        let found = match dir {
            ReadDir::Next => self.scan_forward(self.next_from),
            ReadDir::Previous => {
                if self.prev_from == 0 {
                    None
                } else {
                    self.scan_backward(self.prev_from)
                }
            }
        };
        match found {
            Some(n) => {
                let rec = self.slot_get(n);
                self.current = Some(n);
                self.next_from = n + 1;
                self.prev_from = n.saturating_sub(1);
                (rec, n, status::OK)
            }
            None => {
                self.current = None;
                (None, 0, status::EOF)
            }
        }
    }

    /// `START` — position the file without delivering a record.
    pub fn start(&mut self, op: StartOp, relnum: u64) -> &'static str {
        if self.container.is_none() {
            return status::NOT_OPEN_INPUT;
        }
        if !matches!(self.mode, Some(OpenMode::Input) | Some(OpenMode::Io)) {
            return status::NOT_OPEN_INPUT;
        }
        let found = match op {
            StartOp::Eq => self.slot_get(relnum).map(|_| relnum),
            StartOp::Ge => self.scan_forward(relnum.max(1)),
            StartOp::Gt => self.scan_forward(relnum.saturating_add(1)),
            StartOp::Le => self.scan_backward(relnum),
            StartOp::Lt => {
                if relnum <= 1 {
                    None
                } else {
                    self.scan_backward(relnum - 1)
                }
            }
        };
        match found {
            Some(n) => {
                // START positions *on* the record, so the next sequential READ
                // in either direction delivers it.
                self.next_from = n;
                self.prev_from = n;
                self.current = None;
                status::OK
            }
            None => status::NOT_FOUND,
        }
    }

    /// `REWRITE`. `relnum` is the RELATIVE KEY under random or dynamic access;
    /// `None` means "the record the last READ delivered".
    pub fn rewrite(&mut self, rec: &[u8], relnum: Option<u64>) -> &'static str {
        if self.container.is_none() {
            return status::NOT_OPEN_IO;
        }
        if self.mode != Some(OpenMode::Io) {
            return status::NOT_OPEN_IO;
        }
        let n = match relnum.or(self.current) {
            Some(0) | None => return status::NOT_FOUND,
            Some(n) => n,
        };
        if self.slot_get(n).is_none() {
            return status::NOT_FOUND;
        }
        match self.slot_put(n, rec) {
            Ok(()) => status::OK,
            Err(e) => {
                tracing::warn!("RELATIVE REWRITE failed: {e}");
                status::IO_ERROR
            }
        }
    }

    /// `DELETE`. Empties the slot; the slot number itself remains addressable.
    pub fn delete(&mut self, relnum: Option<u64>) -> &'static str {
        if self.container.is_none() {
            return status::NOT_OPEN_IO;
        }
        if self.mode != Some(OpenMode::Io) {
            return status::NOT_OPEN_IO;
        }
        let n = match relnum.or(self.current) {
            Some(0) | None => return status::NOT_FOUND,
            Some(n) => n,
        };
        if self.slot_get(n).is_none() {
            return status::NOT_FOUND;
        }
        match self.slot_erase(n) {
            Ok(()) => {
                self.current = None;
                status::OK
            }
            Err(e) => {
                tracing::warn!("RELATIVE DELETE failed: {e}");
                status::IO_ERROR
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> String {
        let mut p = std::env::temp_dir();
        p.push(format!("rl_engine_{name}_{}.rel", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p.to_string_lossy().into_owned()
    }

    fn rec(n: usize) -> Vec<u8> {
        format!("{n:08}").into_bytes()
    }

    /// Sequential WRITE numbers the slots 1, 2, 3 … and hands each number back.
    #[test]
    fn sequential_write_assigns_ascending_numbers() {
        let mut f = RelativeFile::new(&temp("seqwrite"), 8, false);
        assert_eq!(f.open(OpenMode::Output), status::OK);
        for i in 1..=5u64 {
            let (code, n) = f.write(&rec(i as usize), None);
            assert_eq!(code, status::OK);
            assert_eq!(n, i);
        }
        assert_eq!(f.close(), status::OK);
    }

    /// A random WRITE leaves a gap, and the gap is part of the file: reading it
    /// is "record not found", and a sequential walk steps over it.
    #[test]
    fn a_gap_is_addressable_and_skipped() {
        let path = temp("gap");
        let mut f = RelativeFile::new(&path, 8, false);
        f.open(OpenMode::Output);
        f.write(&rec(1), Some(1));
        f.write(&rec(9), Some(9));
        f.close();

        let mut f = RelativeFile::new(&path, 8, false);
        assert_eq!(f.open(OpenMode::Input), status::OK);
        assert_eq!(f.read_key(5).1, status::NOT_FOUND);
        assert_eq!(f.read_key(1).1, status::OK);
        let (_, n, code) = f.read_seq(ReadDir::Next);
        assert_eq!((n, code), (9, status::OK));
        assert_eq!(f.read_seq(ReadDir::Next).2, status::EOF);
    }

    /// Writing onto an occupied slot is 22; onto slot zero, 24.
    #[test]
    fn occupied_slot_is_22_and_slot_zero_is_24() {
        let mut f = RelativeFile::new(&temp("dup"), 8, false);
        f.open(OpenMode::Output);
        assert_eq!(f.write(&rec(1), Some(3)).0, status::OK);
        assert_eq!(f.write(&rec(2), Some(3)).0, status::DUP_KEY);
        assert_eq!(f.write(&rec(2), Some(0)).0, status::BOUNDARY);
    }

    /// A deleted record's slot keeps its number and reads as absent.
    #[test]
    fn delete_empties_the_slot_but_keeps_it() {
        let path = temp("delete");
        let mut f = RelativeFile::new(&path, 8, false);
        f.open(OpenMode::Output);
        for i in 1..=3u64 {
            f.write(&rec(i as usize), None);
        }
        f.close();

        let mut f = RelativeFile::new(&path, 8, false);
        assert_eq!(f.open(OpenMode::Io), status::OK);
        assert_eq!(f.delete(Some(2)), status::OK);
        assert_eq!(f.read_key(2).1, status::NOT_FOUND);
        // Slot 3 did not move down.
        assert_eq!(f.read_key(3).1, status::OK);
        // Deleting it twice is still "not found", not a second success.
        assert_eq!(f.delete(Some(2)), status::NOT_FOUND);
    }

    /// START positions on the record, so the next READ NEXT delivers it.
    #[test]
    fn start_positions_on_the_record_it_found() {
        let path = temp("start");
        let mut f = RelativeFile::new(&path, 8, false);
        f.open(OpenMode::Output);
        for i in 1..=6u64 {
            f.write(&rec(i as usize), None);
        }
        f.close();

        let mut f = RelativeFile::new(&path, 8, false);
        f.open(OpenMode::Input);
        assert_eq!(f.start(StartOp::Ge, 4), status::OK);
        assert_eq!(f.read_seq(ReadDir::Next).1, 4);
        assert_eq!(f.start(StartOp::Gt, 4), status::OK);
        assert_eq!(f.read_seq(ReadDir::Next).1, 5);
        assert_eq!(f.start(StartOp::Eq, 99), status::NOT_FOUND);
    }

    /// A REWRITE with no preceding READ has no record to replace.
    #[test]
    fn sequential_rewrite_without_a_read_is_not_found() {
        let path = temp("rewrite");
        let mut f = RelativeFile::new(&path, 8, false);
        f.open(OpenMode::Output);
        f.write(&rec(1), None);
        f.close();

        let mut f = RelativeFile::new(&path, 8, false);
        f.open(OpenMode::Io);
        assert_eq!(f.rewrite(&rec(2), None), status::NOT_FOUND);
        assert_eq!(f.read_seq(ReadDir::Next).2, status::OK);
        assert_eq!(f.rewrite(&rec(2), None), status::OK);
    }

    /// Records of different lengths come back at the length they went in with.
    #[test]
    fn varying_records_keep_their_own_length() {
        let path = temp("varying");
        let mut f = RelativeFile::new(&path, 40, false);
        f.open(OpenMode::Output);
        f.write(b"short", None);
        f.write(b"a considerably longer record", None);
        f.close();

        let mut f = RelativeFile::new(&path, 40, false);
        f.open(OpenMode::Input);
        assert_eq!(f.read_seq(ReadDir::Next).0.unwrap(), b"short");
        assert_eq!(
            f.read_seq(ReadDir::Next).0.unwrap(),
            b"a considerably longer record"
        );
    }

    /// The two containers answer identically — the program must not be able to
    /// tell them apart.
    #[test]
    fn memory_and_disk_agree() {
        for memory in [false, true] {
            let path = temp(if memory { "mem" } else { "disk" });
            let mut f = RelativeFile::new(&path, 8, memory);
            f.set_persist(true);
            assert_eq!(f.open(OpenMode::Output), status::OK);
            for i in 1..=4u64 {
                assert_eq!(f.write(&rec(i as usize), None).0, status::OK);
            }
            assert_eq!(f.close(), status::OK);

            let mut f = RelativeFile::new(&path, 8, memory);
            f.set_persist(true);
            assert_eq!(f.open(OpenMode::Io), status::OK);
            assert_eq!(f.delete(Some(2)), status::OK);
            assert_eq!(f.read_key(2).1, status::NOT_FOUND);
            assert_eq!(f.read_key(4).1, status::OK);
            assert_eq!(f.start(StartOp::Ge, 1), status::OK);
            assert_eq!(f.read_seq(ReadDir::Next).1, 1);
            assert_eq!(f.read_seq(ReadDir::Next).1, 3);
            assert_eq!(f.close(), status::OK);
        }
    }

    /// The verbs report the open mode they need.
    #[test]
    fn wrong_open_mode_is_reported() {
        let path = temp("modes");
        let mut f = RelativeFile::new(&path, 8, false);
        f.open(OpenMode::Output);
        f.write(&rec(1), None);
        f.close();

        let mut f = RelativeFile::new(&path, 8, false);
        f.open(OpenMode::Input);
        assert_eq!(f.write(&rec(2), Some(2)).0, status::NOT_OPEN_OUTPUT);
        assert_eq!(f.rewrite(&rec(2), Some(1)), status::NOT_OPEN_IO);
        assert_eq!(f.delete(Some(1)), status::NOT_OPEN_IO);
        f.close();

        let mut f = RelativeFile::new(&path, 8, false);
        f.open(OpenMode::Output);
        assert_eq!(f.read_key(1).1, status::NOT_OPEN_INPUT);
        assert_eq!(f.read_seq(ReadDir::Next).2, status::NOT_OPEN_INPUT);
    }

    /// OPEN of a file that is not there, and of one that is not a relative
    /// container at all.
    #[test]
    fn open_reports_missing_and_foreign_files() {
        let path = temp("missing");
        let mut f = RelativeFile::new(&path, 8, false);
        assert_eq!(f.open(OpenMode::Input), status::FILE_NOT_FOUND);

        let other = temp("foreign");
        std::fs::write(&other, b"this is not a PRCREL1 container at all").unwrap();
        let mut f = RelativeFile::new(&other, 8, false);
        assert_eq!(f.open(OpenMode::Input), status::ATTR_MISMATCH);
    }

    /// EXTEND appends after the last slot that exists, gaps included.
    #[test]
    fn extend_continues_past_the_last_slot() {
        let path = temp("extend");
        let mut f = RelativeFile::new(&path, 8, false);
        f.open(OpenMode::Output);
        f.write(&rec(1), Some(1));
        f.write(&rec(5), Some(5));
        f.close();

        let mut f = RelativeFile::new(&path, 8, false);
        assert_eq!(f.open(OpenMode::Extend), status::OK);
        assert_eq!(f.write(&rec(6), None), (status::OK, 6));
    }
}
