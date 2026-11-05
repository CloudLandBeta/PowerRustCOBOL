// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A Canvas2D's drawing methods as a display list in the module's own memory (ABI A23).
//!
//! A drawing method (`FillRect`, `Fill`, `DrawText` ...) does not reach the host as a `CALL`.
//! The generator lowers it to [`cmd`], which reads the arguments the path buffer holds, encodes the
//! command with `cobolt-graphics-wire` — the one definition the interpreter and the player share —
//! at the end of the canvas's list in the **staging area** the generator reserved, and goes on.
//! [`send`] ends the list: it writes the header and puts one `DRAW` in the batch. The glue copies
//! the staged bytes into the arena's graphics region as the batch is handed over.
//!
//! Anything this module cannot encode exactly as the interpreter does — a colour name, an operand
//! that is not a plain number, a list operand — is **not** guessed: the call goes to the host as an
//! ordinary `CALL`, after the list that was waiting, and the host answers it as it always has.

use cobolt_graphics_wire as wire;
use wire::{Sink, WireKind, Writer};

use crate::cmd;
use crate::disp;
use crate::mem;
use crate::state::{st, BATCH_SIZE, WS_GFX_META, WS_PATH};

/// No canvas has commands waiting.
const NONE: u32 = u32::MAX;
/// The opcode of `Present`.
const OP_PRESENT: u8 = 0x05;
/// The batch opcode of `DRAW` (ABI A23).
pub const OP_DRAW: u8 = 0x08;

/// Give the routines the staging area (`pr_init`, after `rt_init`, only when the module has a canvas).
pub fn init(at: u32, cap: u32) {
    let s = st();
    s.gfx_at = at;
    s.gfx_cap = cap;
    s.gfx_len = 0;
    s.gfx_cmds = 0;
    s.gfx_ctrl = NONE;
    mem::wr_u32(s.ws + WS_GFX_META, at);
    mem::wr_u32(s.ws + WS_GFX_META + 4, 0);
}

/// Bytes of the list the glue must copy with the batch being handed over (0: none).
fn pending() -> u32 {
    mem::rd_u32(st().ws + WS_GFX_META + 4)
}

fn set_pending(n: u32) {
    mem::wr_u32(st().ws + WS_GFX_META + 4, n);
}

/// The batch was handed to the host: the staged list has been copied and may be overwritten.
pub fn delivered() {
    if st().gfx_cap != 0 {
        set_pending(0);
    }
}

/// Why a command was not encoded.
enum Fail {
    /// The list region has no room for it.
    Full,
    /// The operand is not one this module encodes (the call goes to the host).
    Other,
}

/// The arguments of the call in the path buffer.
struct Args {
    n: usize,
    at: [(u32, u32); 12],
}

impl Args {
    fn text(&self, i: usize) -> Result<&'static [u8], Fail> {
        let (a, l) = *self.at.get(i).ok_or(Fail::Other)?;
        if i >= self.n {
            return Err(Fail::Other);
        }
        Ok(mem::bytes(a, l))
    }
}

fn varint(buf: &[u8], pos: &mut usize) -> Option<u32> {
    let mut v = 0u32;
    let mut shift = 0;
    loop {
        let b = *buf.get(*pos)?;
        *pos += 1;
        v |= ((b & 0x7F) as u32).checked_shl(shift)?;
        if b & 0x80 == 0 {
            return Some(v);
        }
        shift += 7;
        if shift > 28 {
            return None;
        }
    }
}

/// `(ctrl, args)` of the single-segment call the path buffer holds.
fn read_path() -> Option<(u32, Args)> {
    let s = st();
    let base = s.ws + WS_PATH;
    let buf = mem::bytes(base, s.path_len);
    let mut pos = 0;
    let _inst = varint(buf, &mut pos)?;
    let ctrl = varint(buf, &mut pos)?;
    if *buf.get(pos)? != 1 {
        return None;
    }
    pos += 1;
    let _name = varint(buf, &mut pos)?;
    let nargs = *buf.get(pos)? as usize;
    pos += 1;
    let mut a = Args { n: nargs, at: [(0, 0); 12] };
    if nargs > a.at.len() {
        return None;
    }
    for slot in a.at.iter_mut().take(nargs) {
        let len = varint(buf, &mut pos)?;
        *slot = (base + pos as u32, len);
        pos += len as usize;
    }
    Some((ctrl, a))
}

/// The control of the path being built (its first two numbers are the instance and the control).
fn path_ctrl() -> Option<u32> {
    let s = st();
    let buf = mem::bytes(s.ws + WS_PATH, s.path_len);
    let mut pos = 0;
    varint(buf, &mut pos)?;
    varint(buf, &mut pos)
}

// ── number and colour operands, as the interpreter reads them ────────────────────────────────────

/// A decimal text as an `f64`: sign, digits, an optional fraction and exponent. Exact (correctly
/// rounded) while the digits fit 15 places and the power of ten is at most 22 — the cases a COBOL
/// number has; anything else is left to the host.
fn parse_f64(t: &[u8]) -> Option<f64> {
    let mut i = 0;
    let neg = match t.first()? {
        b'-' => {
            i = 1;
            true
        }
        b'+' => {
            i = 1;
            false
        }
        _ => false,
    };
    let (mut mant, mut digits, mut scale) = (0u64, 0u32, 0i32);
    let mut seen_dot = false;
    let mut any = false;
    while let Some(&c) = t.get(i) {
        match c {
            b'0'..=b'9' => {
                any = true;
                if digits > 0 || c != b'0' {
                    digits += 1;
                    if digits > 15 {
                        return None;
                    }
                }
                mant = mant * 10 + (c - b'0') as u64;
                if seen_dot {
                    scale -= 1;
                }
            }
            b'.' if !seen_dot => seen_dot = true,
            b'e' | b'E' => break,
            _ => return None,
        }
        i += 1;
    }
    if !any {
        return None;
    }
    if i < t.len() {
        // an exponent
        i += 1;
        let eneg = match t.get(i) {
            Some(b'-') => {
                i += 1;
                true
            }
            Some(b'+') => {
                i += 1;
                false
            }
            _ => false,
        };
        let mut e = 0i32;
        let start = i;
        while let Some(&c) = t.get(i) {
            if !c.is_ascii_digit() || e > 400 {
                return None;
            }
            e = e * 10 + (c - b'0') as i32;
            i += 1;
        }
        if i == start {
            return None;
        }
        scale += if eneg { -e } else { e };
    }
    let mut v = mant as f64;
    if scale != 0 {
        if scale.unsigned_abs() > 22 {
            return None;
        }
        let p = POW10[scale.unsigned_abs() as usize];
        v = if scale < 0 { v / p } else { v * p };
    }
    Some(if neg { -v } else { v })
}

const POW10: [f64; 23] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16, 1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
];

fn whole(t: &[u8], lo: f64, hi: f64) -> Result<f64, Fail> {
    let v = parse_f64(t).ok_or(Fail::Other)?;
    if libm::trunc(v) != v || v < lo || v > hi {
        return Err(Fail::Other);
    }
    Ok(v)
}

fn hex(b: u8) -> Option<u32> {
    (b as char).to_digit(16)
}

/// `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, as `Rgba::parse` reads them, as `0xRRGGBBAA`.
fn parse_hex_color(h: &[u8]) -> Option<u32> {
    let n = |i: usize| hex(*h.get(i)?);
    let byte = |i: usize| Some(n(i)? * 16 + n(i + 1)?);
    let (r, g, b, a) = match h.len() {
        3 => (n(0)? * 17, n(1)? * 17, n(2)? * 17, 255),
        4 => (n(0)? * 17, n(1)? * 17, n(2)? * 17, n(3)? * 17),
        6 => (byte(0)?, byte(2)?, byte(4)?, 255),
        8 => (byte(0)?, byte(2)?, byte(4)?, byte(6)?),
        _ => return None,
    };
    Some((r << 24) | (g << 16) | (b << 8) | a)
}

/// One operand kind read from the argument texts.
trait Take: WireKind {
    fn take(a: &Args, i: &mut usize) -> Result<Self::Arg<'static>, Fail>;
}

macro_rules! take_num {
    ($k:ident, $ty:ty, $lo:expr, $hi:expr) => {
        impl Take for wire::$k {
            fn take(a: &Args, i: &mut usize) -> Result<Self::Arg<'static>, Fail> {
                let v = whole(a.text(*i)?, $lo, $hi)?;
                *i += 1;
                Ok(v as $ty)
            }
        }
    };
}
take_num!(U8, u8, 0.0, 255.0);
take_num!(U32, u32, 0.0, u32::MAX as f64);

impl Take for wire::F32 {
    fn take(a: &Args, i: &mut usize) -> Result<f32, Fail> {
        let v = parse_f64(a.text(*i)?).ok_or(Fail::Other)?;
        *i += 1;
        Ok(v as f32)
    }
}

impl Take for wire::Color {
    fn take(a: &Args, i: &mut usize) -> Result<u32, Fail> {
        let t = a.text(*i)?;
        *i += 1;
        if let Some(h) = t.strip_prefix(b"#") {
            return parse_hex_color(h).ok_or(Fail::Other);
        }
        whole(t, 0.0, u32::MAX as f64).map(|v| v as u32)
    }
}

impl Take for wire::Str {
    fn take(a: &Args, i: &mut usize) -> Result<&'static str, Fail> {
        let t = a.text(*i)?;
        *i += 1;
        core::str::from_utf8(t).map_err(|_| Fail::Other)
    }
}

macro_rules! take_list {
    ($k:ident, $seq:ident) => {
        impl Take for wire::$k {
            fn take(_: &Args, _: &mut usize) -> Result<&'static dyn wire::$seq, Fail> {
                Err(Fail::Other)
            }
        }
    };
}
take_list!(PointsK, PointSeq);
take_list!(FloatsK, FloatSeq);
take_list!(StopsK, StopSeq);

macro_rules! dispatch {
    ($( $(#[doc = $doc:literal])* $op:literal $name:ident $snake:ident { $( $f:ident : $t:ident ),* $(,)? } )*) => {
        /// Encode command `op` from the argument texts, or say why not.
        fn encode<S: Sink + ?Sized>(op: u8, a: &Args, s: &mut S) -> Result<(), Fail> {
            match op {
                $( $op => {
                    #[allow(unused_mut, unused_variables)]
                    let mut i = 0usize;
                    $( let $f = <wire::$t as Take>::take(a, &mut i)?; )*
                    if i != a.n {
                        return Err(Fail::Other);
                    }
                    wire::encode::$snake(s $(, $f)*).map_err(|_| Fail::Full)
                } )*
                _ => Err(Fail::Other),
            }
        }
    };
}
wire::with_command_table!(dispatch);

/// A drawing method: append command `op` to the list, or hand the call to the host.
pub fn cmd(op: u32) {
    let op = op as u8;
    let s = st();
    let Some((ctrl, args)) = read_path() else { return fallback() };
    if s.gfx_cap == 0 {
        return fallback();
    }
    if s.gfx_cmds > 0 && (s.gfx_ctrl != ctrl || s.gfx_inst != s.inst) {
        send(); // one list belongs to one canvas of one form instance
    }
    for _ in 0..2 {
        let start = wire::HEADER_LEN as u32 + s.gfx_len;
        let room = s.gfx_cap.saturating_sub(start);
        let mut w = Writer::new(mem::bytes_mut(s.gfx_at + start.min(s.gfx_cap), room));
        match encode(op, &args, &mut w) {
            Ok(()) => {
                s.gfx_len += w.len() as u32;
                s.gfx_cmds += 1;
                s.gfx_ctrl = ctrl;
                s.gfx_inst = s.inst;
                if op == OP_PRESENT {
                    send();
                }
                return;
            }
            Err(Fail::Full) if s.gfx_cmds > 0 => send(), // the list is full: send it and start another
            Err(_) => return fallback(),
        }
    }
    fallback();
}

/// The call goes to the host as it always has, after the list that was waiting.
fn fallback() {
    send();
    cmd::call();
}

/// A command is about to name the control of the path being built: if that canvas has a list
/// waiting, it goes first, so a read-back or a property write sees the drawing.
pub fn before_path() {
    let s = st();
    if s.gfx_cmds > 0 && s.gfx_inst == s.inst && path_ctrl() == Some(s.gfx_ctrl) {
        send();
    }
}

/// End the list that is waiting: its header, and one `DRAW` in the batch.
pub fn send() {
    let s = st();
    if s.gfx_cmds == 0 {
        return;
    }
    // The staging area holds one list: an earlier DRAW still waiting for its flush goes first.
    if pending() > 0 {
        disp::flush();
    }
    let total = wire::HEADER_LEN as u32 + s.gfx_len;
    let mut head = Writer::new(mem::bytes_mut(s.gfx_at, wire::HEADER_LEN as u32));
    let _ = wire::write_header(&mut head, s.gfx_cmds, s.gfx_len);
    let mut b = [0u8; 21];
    b[0] = OP_DRAW;
    let mut n = 1;
    for v in [s.gfx_inst, s.gfx_ctrl, 0, total] {
        n += disp::put_varint_pub(&mut b[n..], v);
    }
    if s.batch_len as usize + n > BATCH_SIZE as usize {
        disp::flush();
    }
    set_pending(total);
    disp::batch_put(&b[..n]);
    s.gfx_len = 0;
    s.gfx_cmds = 0;
    s.gfx_ctrl = NONE;
}
