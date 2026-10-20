// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The command batch (ABI v1 §5), the queries (§6) and the event arguments (§7).
//!
//! Everything is little-endian where fixed-size and unsigned LEB128 where it is a
//! `varint`; a `text` is a `varint` byte length followed by that many UTF-8 bytes.
//! [`BatchReader`] decodes without copying (the strings borrow the buffer);
//! [`BatchWriter`] is what the tests, the fixture modules and any future
//! producer use to build the same bytes.

use crate::codes::op;

/// Why a batch could not be read. Every variant carries the byte offset it was
/// found at, so a host can report where a hostile or buggy module went wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    Truncated { at: usize },
    VarintOverflow { at: usize },
    BadUtf8 { at: usize },
    UnknownOp { op: u8, at: usize },
    TooManySegments { at: usize, count: u32 },
    TooManyArguments { at: usize, count: u32 },
    /// A query must be the last command of its batch (§6).
    QueryNotLast { at: usize },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DecodeError::Truncated { at } => write!(f, "the batch ends in the middle of a command (byte {at})"),
            DecodeError::VarintOverflow { at } => write!(f, "a varint is longer than 5 bytes or exceeds 32 bits (byte {at})"),
            DecodeError::BadUtf8 { at } => write!(f, "a text is not valid UTF-8 (byte {at})"),
            DecodeError::UnknownOp { op, at } => write!(f, "unknown opcode 0x{op:02x} (byte {at})"),
            DecodeError::TooManySegments { at, count } => write!(f, "a path has {count} segments, over the limit (byte {at})"),
            DecodeError::TooManyArguments { at, count } => write!(f, "a segment has {count} arguments, over the limit (byte {at})"),
            DecodeError::QueryNotLast { at } => write!(f, "a query is not the last command of its batch (byte {at})"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// The most segments one path may have (`a::b::c(1)` is 3). A real program stays
/// in single digits; the cap stops a hostile module from asking the host to
/// allocate without bound.
pub const MAX_SEGMENTS: u32 = 64;
/// The most argument values one segment may carry.
pub const MAX_ARGS: u32 = 256;

// ── varint and text ─────────────────────────────────────────────────────────

/// Append `v` as an unsigned LEB128.
pub fn write_varint(out: &mut Vec<u8>, mut v: u32) {
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// How many bytes `write_varint` writes for `v`.
pub fn varint_len(v: u32) -> usize {
    match v {
        0..=0x7f => 1,
        0x80..=0x3fff => 2,
        0x4000..=0x1f_ffff => 3,
        0x20_0000..=0x0fff_ffff => 4,
        _ => 5,
    }
}

/// Read an unsigned LEB128 `u32` at `*pos`, advancing it. At most 5 bytes, and
/// the fifth may only carry the top 4 bits.
pub fn read_varint(buf: &[u8], pos: &mut usize) -> Result<u32, DecodeError> {
    let start = *pos;
    let mut result: u32 = 0;
    for i in 0..5 {
        let Some(&byte) = buf.get(*pos) else {
            return Err(DecodeError::Truncated { at: start });
        };
        *pos += 1;
        let payload = (byte & 0x7f) as u32;
        if i == 4 && payload > 0x0f {
            return Err(DecodeError::VarintOverflow { at: start });
        }
        result |= payload << (7 * i);
        if byte & 0x80 == 0 {
            return Ok(result);
        }
    }
    Err(DecodeError::VarintOverflow { at: start })
}

/// Append a `text`: its byte length, then the bytes.
pub fn write_text(out: &mut Vec<u8>, s: &str) {
    write_varint(out, s.len() as u32);
    out.extend_from_slice(s.as_bytes());
}

/// Read a `text` at `*pos`, borrowing the buffer. The bytes are validated as UTF-8
/// here, once — that is the one conversion WASM-017 R12 allows in this direction.
pub fn read_text<'a>(buf: &'a [u8], pos: &mut usize) -> Result<&'a str, DecodeError> {
    let at = *pos;
    let len = read_varint(buf, pos)? as usize;
    let end = pos.checked_add(len).ok_or(DecodeError::Truncated { at })?;
    let Some(bytes) = buf.get(*pos..end) else {
        return Err(DecodeError::Truncated { at });
    };
    let s = core::str::from_utf8(bytes).map_err(|_| DecodeError::BadUtf8 { at: *pos })?;
    *pos = end;
    Ok(s)
}

// ── paths and commands ──────────────────────────────────────────────────────

/// One segment of a path: a name (an index into the manifest's `names`) and the
/// argument values it was called or subscripted with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seg<'a> {
    pub name: u32,
    pub args: Vec<&'a str>,
}

/// `varint inst, varint ctrl, varint nseg, seg*` (§5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path<'a> {
    pub inst: u32,
    pub ctrl: u32,
    pub segs: Vec<Seg<'a>>,
}

impl<'a> Path<'a> {
    /// The last segment — the property or method the command names.
    pub fn last(&self) -> Option<&Seg<'a>> {
        self.segs.last()
    }
}

/// One command of a batch. The three queries are commands too; a batch holds at
/// most one and it is the last (§6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command<'a> {
    Set { path: Path<'a>, value: &'a str },
    Call { path: Path<'a> },
    Display(&'a str),
    Error { code: u32, message: &'a str, unit: u32, line: u32, form: u32, event: u32 },
    Log { level: u32, text: &'a str },
    Get(Path<'a>),
    CallR(Path<'a>),
    Count(Path<'a>),
}

impl Command<'_> {
    pub fn is_query(&self) -> bool {
        matches!(self, Command::Get(_) | Command::CallR(_) | Command::Count(_))
    }
    pub fn opcode(&self) -> u8 {
        match self {
            Command::Set { .. } => op::SET,
            Command::Call { .. } => op::CALL,
            Command::Display(_) => op::DISPLAY,
            Command::Error { .. } => op::ERROR,
            Command::Log { .. } => op::LOG,
            Command::Get(_) => op::GET,
            Command::CallR(_) => op::CALLR,
            Command::Count(_) => op::COUNT,
        }
    }
}

fn read_path<'a>(buf: &'a [u8], pos: &mut usize) -> Result<Path<'a>, DecodeError> {
    let inst = read_varint(buf, pos)?;
    let ctrl = read_varint(buf, pos)?;
    let at = *pos;
    let nseg = read_varint(buf, pos)?;
    if nseg > MAX_SEGMENTS {
        return Err(DecodeError::TooManySegments { at, count: nseg });
    }
    let mut segs = Vec::with_capacity(nseg as usize);
    for _ in 0..nseg {
        let name = read_varint(buf, pos)?;
        let at = *pos;
        let nargs = read_varint(buf, pos)?;
        if nargs > MAX_ARGS {
            return Err(DecodeError::TooManyArguments { at, count: nargs });
        }
        let mut args = Vec::with_capacity(nargs as usize);
        for _ in 0..nargs {
            args.push(read_text(buf, pos)?);
        }
        segs.push(Seg { name, args });
    }
    Ok(Path { inst, ctrl, segs })
}

/// Reads the commands of a batch one at a time. After an error the iterator ends:
/// the rest of the bytes cannot be trusted to be on a command boundary.
pub struct BatchReader<'a> {
    buf: &'a [u8],
    pos: usize,
    failed: bool,
    saw_query: bool,
}

impl<'a> BatchReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        BatchReader { buf, pos: 0, failed: false, saw_query: false }
    }

    /// The offset of the next command.
    pub fn position(&self) -> usize {
        self.pos
    }

    fn read_one(&mut self) -> Result<Command<'a>, DecodeError> {
        let at = self.pos;
        let opcode = self.buf[at];
        self.pos += 1;
        if self.saw_query {
            return Err(DecodeError::QueryNotLast { at });
        }
        let buf = self.buf;
        let pos = &mut self.pos;
        let cmd = match opcode {
            op::SET => {
                let path = read_path(buf, pos)?;
                let value = read_text(buf, pos)?;
                Command::Set { path, value }
            }
            op::CALL => Command::Call { path: read_path(buf, pos)? },
            op::DISPLAY => Command::Display(read_text(buf, pos)?),
            op::ERROR => {
                let code = read_varint(buf, pos)?;
                let message = read_text(buf, pos)?;
                let unit = read_varint(buf, pos)?;
                let line = read_varint(buf, pos)?;
                let form = read_varint(buf, pos)?;
                let event = read_varint(buf, pos)?;
                Command::Error { code, message, unit, line, form, event }
            }
            op::LOG => {
                let level = read_varint(buf, pos)?;
                Command::Log { level, text: read_text(buf, pos)? }
            }
            op::GET => Command::Get(read_path(buf, pos)?),
            op::CALLR => Command::CallR(read_path(buf, pos)?),
            op::COUNT => Command::Count(read_path(buf, pos)?),
            other => return Err(DecodeError::UnknownOp { op: other, at }),
        };
        if cmd.is_query() {
            self.saw_query = true;
        }
        Ok(cmd)
    }
}

impl<'a> Iterator for BatchReader<'a> {
    type Item = Result<Command<'a>, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.pos >= self.buf.len() {
            return None;
        }
        let item = self.read_one();
        if item.is_err() {
            self.failed = true;
        }
        Some(item)
    }
}

/// Decode a whole batch, collecting the commands.
pub fn decode_batch(bytes: &[u8]) -> Result<Vec<Command<'_>>, DecodeError> {
    BatchReader::new(bytes).collect()
}

// ── writing ─────────────────────────────────────────────────────────────────

/// A path under construction: the instance and control, then segments.
#[derive(Debug, Clone, Default)]
pub struct PathSpec {
    pub inst: u32,
    pub ctrl: u32,
    pub segs: Vec<(u32, Vec<String>)>,
}

impl PathSpec {
    /// `ctrl::name` with no arguments — a property of a control.
    pub fn prop(inst: u32, ctrl: u32, name: u32) -> Self {
        PathSpec { inst, ctrl, segs: vec![(name, Vec::new())] }
    }
    /// `ctrl::name(args…)` — a method call, or a subscripted member.
    pub fn call(inst: u32, ctrl: u32, name: u32, args: &[&str]) -> Self {
        PathSpec { inst, ctrl, segs: vec![(name, args.iter().map(|s| (*s).to_owned()).collect())] }
    }
    pub fn then(mut self, name: u32, args: &[&str]) -> Self {
        self.segs.push((name, args.iter().map(|s| (*s).to_owned()).collect()));
        self
    }
}

fn write_path(out: &mut Vec<u8>, p: &PathSpec) {
    write_varint(out, p.inst);
    write_varint(out, p.ctrl);
    write_varint(out, p.segs.len() as u32);
    for (name, args) in &p.segs {
        write_varint(out, *name);
        write_varint(out, args.len() as u32);
        for a in args {
            write_text(out, a);
        }
    }
}

/// Builds the bytes of a batch.
#[derive(Debug, Clone, Default)]
pub struct BatchWriter {
    buf: Vec<u8>,
}

impl BatchWriter {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.buf.len()
    }
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    pub fn set(&mut self, path: &PathSpec, value: &str) -> &mut Self {
        self.buf.push(op::SET);
        write_path(&mut self.buf, path);
        write_text(&mut self.buf, value);
        self
    }
    pub fn call(&mut self, path: &PathSpec) -> &mut Self {
        self.buf.push(op::CALL);
        write_path(&mut self.buf, path);
        self
    }
    pub fn display(&mut self, text: &str) -> &mut Self {
        self.buf.push(op::DISPLAY);
        write_text(&mut self.buf, text);
        self
    }
    pub fn error(&mut self, code: u32, message: &str, unit: u32, line: u32, form: u32, event: u32) -> &mut Self {
        self.buf.push(op::ERROR);
        write_varint(&mut self.buf, code);
        write_text(&mut self.buf, message);
        for v in [unit, line, form, event] {
            write_varint(&mut self.buf, v);
        }
        self
    }
    pub fn log(&mut self, level: u32, text: &str) -> &mut Self {
        self.buf.push(op::LOG);
        write_varint(&mut self.buf, level);
        write_text(&mut self.buf, text);
        self
    }
    pub fn get(&mut self, path: &PathSpec) -> &mut Self {
        self.buf.push(op::GET);
        write_path(&mut self.buf, path);
        self
    }
    pub fn call_result(&mut self, path: &PathSpec) -> &mut Self {
        self.buf.push(op::CALLR);
        write_path(&mut self.buf, path);
        self
    }
    pub fn count(&mut self, path: &PathSpec) -> &mut Self {
        self.buf.push(op::COUNT);
        write_path(&mut self.buf, path);
        self
    }
}

// ── event arguments (§7) ────────────────────────────────────────────────────

/// `argc` consecutive `text` values, as the host writes them at the start of the
/// answer region before `pr_event`.
pub fn encode_event_args(args: &[&str]) -> Vec<u8> {
    let mut out = Vec::new();
    for a in args {
        write_text(&mut out, a);
    }
    out
}

/// The inverse of [`encode_event_args`].
pub fn decode_event_args(buf: &[u8], argc: usize) -> Result<Vec<&str>, DecodeError> {
    let mut pos = 0;
    let mut out = Vec::with_capacity(argc);
    for _ in 0..argc {
        out.push(read_text(buf, &mut pos)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_round_trips_across_every_length_boundary() {
        for v in [0u32, 1, 0x7f, 0x80, 0x3fff, 0x4000, 0x1f_ffff, 0x20_0000, 0x0fff_ffff, 0x1000_0000, u32::MAX] {
            let mut buf = Vec::new();
            write_varint(&mut buf, v);
            assert_eq!(buf.len(), varint_len(v), "length of {v}");
            let mut pos = 0;
            assert_eq!(read_varint(&buf, &mut pos).unwrap(), v);
            assert_eq!(pos, buf.len());
        }
    }

    #[test]
    fn varint_golden_vectors_from_the_spec_encoding() {
        // Unsigned LEB128, as WebAssembly defines it.
        let mut b = Vec::new();
        write_varint(&mut b, 624485);
        assert_eq!(b, [0xe5, 0x8e, 0x26]);
        let mut p = 0;
        assert_eq!(read_varint(&[0xe5, 0x8e, 0x26], &mut p).unwrap(), 624485);
    }

    #[test]
    fn varint_rejects_overlong_and_overflowing_encodings() {
        let mut p = 0;
        assert!(matches!(read_varint(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x01], &mut p), Err(DecodeError::VarintOverflow { .. })));
        let mut p = 0;
        // A fifth byte carrying more than 4 bits would not fit in 32.
        assert!(matches!(read_varint(&[0xff, 0xff, 0xff, 0xff, 0x1f], &mut p), Err(DecodeError::VarintOverflow { .. })));
        let mut p = 0;
        assert!(matches!(read_varint(&[0x80], &mut p), Err(DecodeError::Truncated { .. })));
    }

    #[test]
    fn text_is_validated_as_utf8_once_and_borrows_the_buffer() {
        let mut buf = Vec::new();
        write_text(&mut buf, "Ação — 日本語 😀");
        let mut pos = 0;
        let s = read_text(&buf, &mut pos).unwrap();
        assert_eq!(s, "Ação — 日本語 😀");
        // A lone continuation byte is not UTF-8.
        let mut bad = vec![1u8, 0x80];
        let mut p = 0;
        assert!(matches!(read_text(&bad, &mut p), Err(DecodeError::BadUtf8 { .. })));
        bad[0] = 5; // length beyond the buffer
        let mut p = 0;
        assert!(matches!(read_text(&bad, &mut p), Err(DecodeError::Truncated { .. })));
    }

    #[test]
    fn a_batch_round_trips_every_command_kind() {
        let mut w = BatchWriter::new();
        let caption = PathSpec::prop(0, 3, 7);
        let items = PathSpec::call(0, 4, 9, &["Item 日本"]);
        w.set(&caption, "Olá, mundo")
            .call(&items)
            .display("hello")
            .log(2, "diag")
            .error(1001, "subscript out of range", 1, 42, 0, 3)
            .get(&PathSpec::prop(0, 3, 7));
        let cmds = decode_batch(w.as_bytes()).unwrap();
        assert_eq!(cmds.len(), 6);
        match &cmds[0] {
            Command::Set { path, value } => {
                assert_eq!((path.inst, path.ctrl), (0, 3));
                assert_eq!(path.segs[0].name, 7);
                assert_eq!(*value, "Olá, mundo");
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(&cmds[1], Command::Call { path } if path.segs[0].args == ["Item 日本"]));
        assert_eq!(cmds[2], Command::Display("hello"));
        assert_eq!(cmds[3], Command::Log { level: 2, text: "diag" });
        assert_eq!(
            cmds[4],
            Command::Error { code: 1001, message: "subscript out of range", unit: 1, line: 42, form: 0, event: 3 }
        );
        assert!(cmds[5].is_query() && cmds[5].opcode() == op::GET);
    }

    #[test]
    fn a_golden_set_command_has_the_exact_bytes_the_spec_describes() {
        // SET inst=0 ctrl=3 nseg=1 [name=7 nargs=0] text("OK")
        let mut w = BatchWriter::new();
        w.set(&PathSpec::prop(0, 3, 7), "OK");
        assert_eq!(w.as_bytes(), [0x01, 0x00, 0x03, 0x01, 0x07, 0x00, 0x02, b'O', b'K']);
    }

    #[test]
    fn a_query_must_be_last() {
        let mut w = BatchWriter::new();
        w.get(&PathSpec::prop(0, 3, 1)).display("after");
        let err = decode_batch(w.as_bytes()).unwrap_err();
        assert!(matches!(err, DecodeError::QueryNotLast { .. }), "{err}");
    }

    #[test]
    fn hostile_input_is_refused_not_allocated() {
        // nseg = 100 000: refused before any Vec is sized from it.
        let mut buf = vec![op::CALL, 0, 3];
        write_varint(&mut buf, 100_000);
        assert!(matches!(decode_batch(&buf), Err(DecodeError::TooManySegments { .. })));
        // A text claiming 4 GiB.
        let mut buf = vec![op::DISPLAY];
        write_varint(&mut buf, u32::MAX);
        assert!(matches!(decode_batch(&buf), Err(DecodeError::Truncated { .. })));
        // Unknown opcode.
        assert!(matches!(decode_batch(&[0x7e]), Err(DecodeError::UnknownOp { op: 0x7e, .. })));
        // Truncation at every prefix of a valid batch is an error, never a panic.
        let mut w = BatchWriter::new();
        w.set(&PathSpec::call(0, 3, 7, &["a", "bc"]), "value").error(1, "m", 1, 2, 3, 4);
        let full = w.into_bytes();
        for cut in 0..full.len() {
            let _ = decode_batch(&full[..cut]);
        }
    }

    #[test]
    fn event_arguments_round_trip_with_multibyte_text() {
        let enc = encode_event_args(&["a", "日本語", "😀"]);
        let dec = decode_event_args(&enc, 3).unwrap();
        assert_eq!(dec, ["a", "日本語", "😀"]);
        assert!(decode_event_args(&enc, 4).is_err());
    }
}
