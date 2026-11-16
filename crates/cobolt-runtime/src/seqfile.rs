//! `ORGANIZATION IS SEQUENTIAL` and `LINE SEQUENTIAL` files: the handle an
//! `OPEN` makes, and what `READ`, `WRITE`, `REWRITE` and `CLOSE` do with it.
//!
//! Taken out of the interpreter unchanged (DESKTOP-WASM-001 T37, correction
//! C5), so that the native runner can serve the same verbs over the same bytes
//! on disk with the same `FILE STATUS` values. The interpreter still does what
//! surrounds a verb: the record area, `AT END`, the declaratives and the
//! bookkeeping of which record a `REWRITE` may replace. This module is the file
//! alone.
//!
//! Two layouts on disk:
//! * **LINE SEQUENTIAL** — one record per line of text, with no terminator
//!   inside the record. Trailing spaces are not written.
//! * **Record SEQUENTIAL** — records back to back. A fixed-length file holds
//!   each record's bytes as they are; a variable-length file puts a length
//!   prefix ([`VARYING_LEN_PREFIX`]) in front of each.

use std::fs::{File, OpenOptions};
use std::io::{BufRead as _, BufReader, BufWriter, Read as _, Seek as _, SeekFrom, Write as _};

use crate::files::RecordLayout;
use crate::indexed::{status, OpenMode};

/// Bytes of length prefix in front of each record of a **variable-length**
/// record-SEQUENTIAL file.
///
/// A fixed-length file needs no prefix — every record is the same size, so the
/// reader knows where the next one starts. A variable-length file does: the
/// length is part of the record's identity, and `READ` has to hand it back
/// (through `DEPENDING ON`) exactly as `WRITE` was given it. The prefix is a
/// big-endian `u32`, which keeps the record's own bytes at a readable offset in
/// a hex dump and mirrors the record-descriptor word mainframe COBOL puts
/// there. It is written and read only for files whose FD declares varying
/// records, so no fixed-length file's bytes change.
pub const VARYING_LEN_PREFIX: usize = 4;

/// Where a sequential `READ` found the record it delivered.
#[derive(Debug, Clone, Copy)]
pub struct RecordPlace {
    /// Byte offset of the record's data — past the length prefix, if any.
    pub start: u64,
    /// The record's length in bytes.
    pub len: usize,
}

/// What one `READ` gave.
#[derive(Debug)]
pub enum SeqRead {
    /// A record, and, for a record-SEQUENTIAL file, where it sits (a LINE
    /// SEQUENTIAL record cannot be rewritten in place, so it has none).
    Record {
        bytes: Vec<u8>,
        place: Option<RecordPlace>,
    },
    /// No record left: `AT END`.
    End,
    /// The read itself failed.
    Failed(std::io::Error),
    /// The file is open for output, not input.
    NotInput,
}

impl SeqRead {
    /// The `FILE STATUS` this outcome gives.
    pub fn status(&self) -> &'static str {
        match self {
            SeqRead::Record { .. } => status::OK,
            SeqRead::End => status::EOF,
            SeqRead::Failed(_) => "30",
            SeqRead::NotInput => status::NOT_OPEN_INPUT,
        }
    }
}

enum Handle {
    /// Opened `OUTPUT` or `EXTEND`.
    Writer(BufWriter<File>),
    /// Opened `INPUT` or `I-O`. An `I-O` file is one handle used for both
    /// reading and rewriting.
    Reader(BufReader<File>),
}

/// An open SEQUENTIAL or LINE SEQUENTIAL file.
pub struct SequentialFile {
    handle: Handle,
    line_sequential: bool,
}

impl SequentialFile {
    /// Status **35** before any handle is made: `INPUT`, `I-O` and `EXTEND`
    /// all need the file to be there, unless `SELECT OPTIONAL` said the program
    /// can run without it. Only `OUTPUT` creates a file unconditionally.
    pub fn refuses_absent(existed: bool, optional: bool, mode: OpenMode) -> bool {
        !existed
            && !optional
            && matches!(mode, OpenMode::Input | OpenMode::Io | OpenMode::Extend)
    }

    /// Open the file at `path`. The caller has already applied
    /// [`Self::refuses_absent`].
    ///
    /// `SELECT OPTIONAL` lets `INPUT` open a file that is not there: it is
    /// created so the handle is real, and the first `READ` then finds it empty
    /// and raises `AT END`, which is exactly what the standard asks for.
    pub fn open(
        path: &str,
        mode: OpenMode,
        optional: bool,
        line_sequential: bool,
    ) -> std::io::Result<SequentialFile> {
        let handle = match mode {
            OpenMode::Output => Handle::Writer(BufWriter::new(File::create(path)?)),
            OpenMode::Extend => Handle::Writer(BufWriter::new(
                OpenOptions::new().create(true).append(true).open(path)?,
            )),
            OpenMode::Input if optional => Handle::Reader(BufReader::new(
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(path)?,
            )),
            OpenMode::Input => Handle::Reader(BufReader::new(File::open(path)?)),
            OpenMode::Io => Handle::Reader(BufReader::new(
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(path)?,
            )),
        };
        Ok(SequentialFile {
            handle,
            line_sequential,
        })
    }

    /// The status of an `OPEN` that succeeded. A file that was not there and
    /// is `OPTIONAL` opens, but the program is told so: **05**, "the file was
    /// not present and was created". Otherwise **00**.
    pub fn opened_status(existed: bool, optional: bool, mode: OpenMode) -> &'static str {
        let created_optional = optional
            && !existed
            && matches!(mode, OpenMode::Input | OpenMode::Io | OpenMode::Extend);
        if created_optional {
            "05"
        } else {
            status::OK
        }
    }

    /// The status of an `OPEN` that failed: **35** for an `INPUT` file that is
    /// not there, **30** for anything else.
    pub fn failed_status(mode: OpenMode, error: &std::io::Error) -> &'static str {
        if mode == OpenMode::Input && error.kind() == std::io::ErrorKind::NotFound {
            status::FILE_NOT_FOUND
        } else {
            "30"
        }
    }

    /// Whether the file is LINE SEQUENTIAL.
    pub fn is_line_sequential(&self) -> bool {
        self.line_sequential
    }

    /// Whether the file was opened `INPUT` or `I-O`.
    pub fn is_input(&self) -> bool {
        matches!(self.handle, Handle::Reader(_))
    }

    /// Whether the file was opened `OUTPUT` or `EXTEND`.
    pub fn is_output(&self) -> bool {
        matches!(self.handle, Handle::Writer(_))
    }

    /// Read the next record.
    ///
    /// `layout` is the FD's record layout: a LINE SEQUENTIAL line carries
    /// national and UTF-8 fields as text and the record holds their images
    /// (spec 077, D10), and a fixed-length record is the layout's length.
    /// Without one, a line is its own bytes and a record is one byte.
    /// `varying` is the FD's `RECORD IS VARYING`.
    pub fn read(&mut self, layout: Option<&RecordLayout>, varying: bool) -> SeqRead {
        let Handle::Reader(r) = &mut self.handle else {
            return SeqRead::NotInput;
        };
        // LINE SEQUENTIAL: newline-delimited text records.
        if self.line_sequential {
            let mut line = String::new();
            return match r.read_line(&mut line) {
                Ok(0) => SeqRead::End,
                Ok(_) => {
                    while line.ends_with('\n') || line.ends_with('\r') {
                        line.pop();
                    }
                    let bytes = match layout {
                        Some(l) => l.line_to_image(line.as_bytes()),
                        None => line.into_bytes(),
                    };
                    SeqRead::Record { bytes, place: None }
                }
                Err(e) => SeqRead::Failed(e),
            };
        }
        // Record SEQUENTIAL: no terminator. A fixed-length file gives one
        // record's worth of bytes per READ; a variable-length one reads its
        // length prefix first and then that many bytes.
        //
        // Where this record starts, so a `REWRITE` can replace it.
        let at = r.stream_position().unwrap_or(0);
        let mut read_exactly = |n: usize| -> Result<Vec<u8>, std::io::Error> {
            let mut bytes = vec![0u8; n];
            r.read_exact(&mut bytes)?;
            Ok(bytes)
        };
        let got = if varying {
            read_exactly(VARYING_LEN_PREFIX).and_then(|p| {
                let n = u32::from_be_bytes([p[0], p[1], p[2], p[3]]) as usize;
                read_exactly(n)
            })
        } else {
            read_exactly(layout.map(|l| l.len.max(1)).unwrap_or(1))
        };
        match got {
            Ok(bytes) => {
                let start = at + if varying { VARYING_LEN_PREFIX as u64 } else { 0 };
                let place = RecordPlace {
                    start,
                    len: bytes.len(),
                };
                SeqRead::Record {
                    bytes,
                    place: Some(place),
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => SeqRead::End,
            Err(e) => SeqRead::Failed(e),
        }
    }

    /// Write one record. `line` is the LINE SEQUENTIAL line, when the record
    /// holds national or UTF-8 fields whose text differs from their image;
    /// otherwise the record is written. A variable-length record carries its
    /// own length, so the reader can find where it ends.
    ///
    /// `None` when the file is not open for output (status **48**, which the
    /// caller gives).
    pub fn write(
        &mut self,
        record: &[u8],
        line: Option<&[u8]>,
        varying: bool,
    ) -> Option<std::io::Result<()>> {
        let Handle::Writer(w) = &mut self.handle else {
            return None;
        };
        Some(if self.line_sequential {
            let s = String::from_utf8_lossy(line.unwrap_or(record));
            writeln!(w, "{}", s.trim_end())
        } else if varying {
            w.write_all(&(record.len() as u32).to_be_bytes())
                .and_then(|()| w.write_all(record))
        } else {
            w.write_all(record)
        })
    }

    /// `REWRITE` of a record-SEQUENTIAL file: replace the record the last
    /// `READ` delivered, in place.
    ///
    /// `io_mode` is whether the file was opened `I-O`; `last` is where the
    /// last successful `READ` found its record, if the file still has one.
    pub fn rewrite(
        &mut self,
        io_mode: bool,
        last: Option<RecordPlace>,
        record: &[u8],
    ) -> &'static str {
        let Handle::Reader(r) = &mut self.handle else {
            return status::NOT_OPEN_IO;
        };
        if !io_mode {
            return status::NOT_OPEN_IO; // REWRITE on a file not open I-O
        }
        match last {
            // No successful READ since the file was positioned here — there is
            // no record to replace (SQ133A).
            None => "43",
            // A sequential REWRITE may not change the record's length: the
            // record after it would have to move (SQ134A rewrites a 120-byte
            // record over a 138-byte one and expects 44).
            Some(at) if at.len != record.len() => "44",
            Some(at) => match rewrite_in_place(r, at.start, record) {
                Ok(()) => status::OK,
                Err(e) => {
                    tracing::warn!("REWRITE failed: {e}");
                    "30"
                }
            },
        }
    }

    /// `CLOSE`: what is buffered for output goes to the file.
    pub fn close(&mut self) -> &'static str {
        if let Handle::Writer(w) = &mut self.handle {
            let _ = w.flush();
        }
        status::OK
    }
}

/// Overwrite `len` bytes at `start` and leave the reader where it was.
///
/// A record-SEQUENTIAL file opened `I-O` is one handle used for both, so the
/// rewrite has to put the read position back: the program's next `READ` must
/// still deliver the record *after* the one just replaced. Seeking through the
/// `BufReader` rather than the file underneath is what keeps the two in step —
/// a seek drops the buffer, so the write below cannot be shadowed by bytes the
/// reader had already buffered.
fn rewrite_in_place(r: &mut BufReader<File>, start: u64, buf: &[u8]) -> std::io::Result<()> {
    let resume = r.stream_position()?;
    r.seek(SeekFrom::Start(start))?;
    r.get_mut().write_all(buf)?;
    r.get_mut().flush()?;
    r.seek(SeekFrom::Start(resume))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(len: usize) -> RecordLayout {
        RecordLayout {
            len,
            ..RecordLayout::default()
        }
    }

    fn path_in(dir: &tempfile::TempDir, name: &str) -> String {
        dir.path().join(name).to_string_lossy().into_owned()
    }

    fn records(f: &mut SequentialFile, l: Option<&RecordLayout>, varying: bool) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while let SeqRead::Record { bytes, .. } = f.read(l, varying) {
            out.push(bytes);
        }
        out
    }

    #[test]
    fn open_statuses_follow_optional_and_presence() {
        let dir = tempfile::tempdir().unwrap();
        let p = path_in(&dir, "absent.dat");
        assert!(SequentialFile::refuses_absent(false, false, OpenMode::Input));
        assert!(SequentialFile::refuses_absent(false, false, OpenMode::Io));
        assert!(SequentialFile::refuses_absent(false, false, OpenMode::Extend));
        assert!(!SequentialFile::refuses_absent(false, false, OpenMode::Output));
        assert!(!SequentialFile::refuses_absent(false, true, OpenMode::Input));
        assert_eq!(SequentialFile::opened_status(false, true, OpenMode::Input), "05");
        assert_eq!(SequentialFile::opened_status(true, true, OpenMode::Input), "00");
        assert_eq!(SequentialFile::opened_status(false, true, OpenMode::Output), "00");
        let err = SequentialFile::open(&p, OpenMode::Input, false, false)
            .err()
            .expect("an absent INPUT file does not open");
        assert_eq!(SequentialFile::failed_status(OpenMode::Input, &err), "35");
        assert_eq!(SequentialFile::failed_status(OpenMode::Io, &err), "30");
        // OPTIONAL INPUT creates the file, and the first READ is at end.
        let mut f = SequentialFile::open(&p, OpenMode::Input, true, false).unwrap();
        assert_eq!(f.read(Some(&layout(4)), false).status(), "10");
    }

    #[test]
    fn fixed_varying_and_line_records_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let fixed = path_in(&dir, "fixed.dat");
        let mut w = SequentialFile::open(&fixed, OpenMode::Output, false, false).unwrap();
        assert!(w.write(b"AAAA", None, false).unwrap().is_ok());
        assert!(w.write(b"BBBB", None, false).unwrap().is_ok());
        assert_eq!(w.read(Some(&layout(4)), false).status(), "47");
        assert_eq!(w.close(), "00");
        assert_eq!(std::fs::read(&fixed).unwrap(), b"AAAABBBB");
        let mut r = SequentialFile::open(&fixed, OpenMode::Input, false, false).unwrap();
        assert!(r.write(b"CCCC", None, false).is_none(), "INPUT refuses WRITE");
        assert_eq!(records(&mut r, Some(&layout(4)), false), vec![b"AAAA".to_vec(), b"BBBB".to_vec()]);

        let varying = path_in(&dir, "varying.dat");
        let mut w = SequentialFile::open(&varying, OpenMode::Output, false, false).unwrap();
        w.write(b"ONE", None, true).unwrap().unwrap();
        w.write(b"THREE", None, true).unwrap().unwrap();
        w.close();
        assert_eq!(std::fs::read(&varying).unwrap(), b"\0\0\0\x03ONE\0\0\0\x05THREE");
        let mut r = SequentialFile::open(&varying, OpenMode::Input, false, false).unwrap();
        assert_eq!(records(&mut r, None, true), vec![b"ONE".to_vec(), b"THREE".to_vec()]);

        let line = path_in(&dir, "line.txt");
        let mut w = SequentialFile::open(&line, OpenMode::Output, false, true).unwrap();
        w.write(b"FIRST   ", None, false).unwrap().unwrap();
        w.write(b"IGNORED", Some(b"SECOND  "), false).unwrap().unwrap();
        w.close();
        assert_eq!(std::fs::read_to_string(&line).unwrap(), "FIRST\nSECOND\n");
        let mut e = SequentialFile::open(&line, OpenMode::Extend, false, true).unwrap();
        e.write(b"THIRD", None, false).unwrap().unwrap();
        e.close();
        let mut r = SequentialFile::open(&line, OpenMode::Input, false, true).unwrap();
        assert_eq!(
            records(&mut r, None, false),
            vec![b"FIRST".to_vec(), b"SECOND".to_vec(), b"THIRD".to_vec()]
        );
    }

    #[test]
    fn rewrite_replaces_the_record_read_and_names_each_refusal() {
        let dir = tempfile::tempdir().unwrap();
        let p = path_in(&dir, "io.dat");
        std::fs::write(&p, b"AAAABBBBCCCC").unwrap();
        let l = layout(4);
        let mut f = SequentialFile::open(&p, OpenMode::Io, false, false).unwrap();
        assert_eq!(f.rewrite(true, None, b"XXXX"), "43");
        let SeqRead::Record { place, .. } = f.read(Some(&l), false) else {
            panic!("a record");
        };
        let SeqRead::Record { place: second, .. } = f.read(Some(&l), false) else {
            panic!("a record");
        };
        assert_eq!(f.rewrite(false, second, b"YYYY"), "49");
        assert_eq!(f.rewrite(true, second, b"YYYYY"), "44");
        assert_eq!(f.rewrite(true, second, b"YYYY"), "00");
        // The next READ still delivers the record after the one replaced.
        let SeqRead::Record { bytes, .. } = f.read(Some(&l), false) else {
            panic!("a record");
        };
        assert_eq!(bytes, b"CCCC");
        assert_eq!(place.map(|p| (p.start, p.len)), Some((0, 4)));
        f.close();
        assert_eq!(std::fs::read(&p).unwrap(), b"AAAAYYYYCCCC");
    }
}
