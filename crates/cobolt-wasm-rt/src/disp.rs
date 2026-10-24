// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `DISPLAY`, and the command batch it writes into.
//!
//! The batch is a buffer in the workspace that grows until the handler ends,
//! suspends or yields — or until it is full, when it is flushed early. Each
//! operand becomes one `DISPLAY` command; the host concatenates them, so a
//! line is a run of commands ending in one that carries the newline.

use crate::codec;
use crate::host;
use crate::mem;
use crate::opd::*;
use crate::state::{st, BATCH_SIZE, DISP_SIZE, WS_BATCH, WS_DISP};
use crate::val::Val;

/// ABI v1 §5 opcodes.
pub const CMD_DISPLAY: u8 = 0x03;

/// Hand the staged batch to the host.
pub fn flush() {
    let s = st();
    if s.batch_len > 0 {
        let len = s.batch_len;
        s.batch_len = 0;
        // SAFETY: the glue function reads `len` staged bytes.
        unsafe { host::flush(s.ws + WS_BATCH, len) };
    }
}

fn put_varint(out: &mut [u8], mut v: u32) -> usize {
    let mut i = 0;
    loop {
        let mut b = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out[i] = b;
        i += 1;
        if v == 0 {
            return i;
        }
    }
}

/// Append raw command bytes to the batch, flushing first if they would not fit.
pub fn batch_put(bytes: &[u8]) {
    if bytes.len() > BATCH_SIZE as usize {
        crate::fail(crate::state::err::SCRATCH_OVERFLOW);
    }
    if (st().batch_len as usize) + bytes.len() > BATCH_SIZE as usize {
        flush();
    }
    let s = st();
    let at = s.ws + WS_BATCH + s.batch_len;
    mem::bytes_mut(at, bytes.len() as u32).copy_from_slice(bytes);
    s.batch_len += bytes.len() as u32;
}

pub(crate) fn put_varint_pub(out: &mut [u8], v: u32) -> usize {
    put_varint(out, v)
}

/// Append one `DISPLAY` command carrying `text`, splitting if it is larger than
/// the batch.
pub fn emit_text(text: &[u8]) {
    let mut rest = text;
    // A command is 1 opcode byte + up to 5 length bytes + the text.
    let max_chunk = (BATCH_SIZE as usize) - 8;
    loop {
        let n = rest.len().min(max_chunk);
        let s = st();
        if (s.batch_len as usize) + 6 + n > BATCH_SIZE as usize {
            flush();
        }
        let s = st();
        let at = s.ws + WS_BATCH + s.batch_len;
        let out = mem::bytes_mut(at, 6 + n as u32);
        out[0] = CMD_DISPLAY;
        let vl = put_varint(&mut out[1..], n as u32);
        out[1 + vl..1 + vl + n].copy_from_slice(&rest[..n]);
        s.batch_len += (1 + vl + n) as u32;
        rest = &rest[n..];
        if rest.is_empty() {
            return;
        }
    }
}

/// The text of a number as `DISPLAY` writes a bare numeric literal or a
/// computed value: the decimal form, a leading `-`, a point when there are
/// decimals (the interpreter's `CobolNumeric` display).
pub fn format_dec(n: &cobolt_decimal::CobolNumeric, out: &mut [u8; 64]) -> usize {
    let mut tmp = [0u8; 40];
    let mut i = tmp.len();
    let mut m = n.mantissa.unsigned_abs();
    loop {
        i -= 1;
        tmp[i] = b'0' + (m % 10) as u8;
        m /= 10;
        if m == 0 {
            break;
        }
    }
    let digits = &tmp[i..];
    let mut w = 0;
    if n.mantissa < 0 {
        out[w] = b'-';
        w += 1;
    }
    let dec = n.decimals as usize;
    if dec == 0 {
        out[w..w + digits.len()].copy_from_slice(digits);
        return w + digits.len();
    }
    if digits.len() <= dec {
        out[w] = b'0';
        out[w + 1] = b'.';
        w += 2;
        for _ in 0..(dec - digits.len()) {
            out[w] = b'0';
            w += 1;
        }
        out[w..w + digits.len()].copy_from_slice(digits);
        w + digits.len()
    } else {
        let split = digits.len() - dec;
        out[w..w + split].copy_from_slice(&digits[..split]);
        w += split;
        out[w] = b'.';
        w += 1;
        out[w..w + dec].copy_from_slice(&digits[split..]);
        w + dec
    }
}

/// Display a computed value.
pub fn display_val(v: &Val) {
    let mut out = [0u8; 64];
    let n = match v {
        Val::Dec(d) => format_dec(d, &mut out),
        Val::F(f) => crate::float::format_f64(*f, &mut out),
    };
    line_text(&out[..n]);
}

/// A numeric item as `DISPLAY` shows it: its digit positions zero-padded to
/// the picture, `-` before them when negative, the sign character where a
/// separate sign sits; `BLANK WHEN ZERO` blanks it.
fn display_numeric_item(loc: &Loc) {
    let fd = &loc.fd;
    let v = codec::load_value(loc);
    if fd.is_float() {
        display_val(&v);
        return;
    }
    let Val::Dec(n) = v else { return };
    let mut digits = [0u8; 40];
    let w = crate::mv::digit_chars(&v, fd, &mut digits);
    let mut out = [0u8; 48];
    let mut k = 0;
    let neg = n.mantissa < 0 && fd.signed();
    if fd.blank_when_zero() && n.mantissa == 0 {
        let blanks = [b' '; 48];
        line_text(&blanks[..w + usize::from(fd.sign_separate())]);
        return;
    }
    if fd.sign_separate() && fd.signed() && fd.sign_leading() {
        out[k] = if neg { b'-' } else { b'+' };
        k += 1;
    } else if neg && !fd.sign_separate() {
        out[k] = b'-';
        k += 1;
    }
    out[k..k + w].copy_from_slice(&digits[..w]);
    k += w;
    if fd.sign_separate() && fd.signed() && !fd.sign_leading() {
        out[k] = if neg { b'-' } else { b'+' };
        k += 1;
    }
    line_text(&out[..k]);
}

/// `DISPLAY` one operand (no newline).
pub fn display_operand(base: u32, o: u32) {
    let op = parse(o);
    match op.tag {
        T_LIT_NUM => display_val(&value(base, &op)),
        T_FIG => {
            let b = fig_byte(op.fig);
            line_text(&[b]);
        }
        T_ALL => {
            let l = locate(base, &op);
            line_text(mem::bytes(l.addr, l.len));
        }
        T_LIT_ALNUM => {
            let l = locate(base, &op);
            line_text(mem::bytes(l.addr, l.len));
        }
        _ => {
            let loc = locate(base, &op);
            if loc.fd.is_numeric() {
                display_numeric_item(&loc);
            } else {
                line_text(mem::bytes(loc.addr, loc.len));
            }
        }
    }
}

/// End a `DISPLAY` statement: its line goes out as one `DISPLAY` command (ABI
/// §5: one command, one line of the host's console), unless `NO ADVANCING`
/// keeps the line open for the next `DISPLAY`.
pub fn display_end(no_advancing: bool) {
    if !no_advancing {
        end_line();
    }
}

/// Add `text` to the line being displayed; a line longer than the buffer goes
/// out in pieces.
fn line_text(text: &[u8]) {
    let mut rest = text;
    while !rest.is_empty() {
        let s = st();
        let room = (DISP_SIZE - s.disp_len) as usize;
        if room == 0 {
            end_line();
            continue;
        }
        let n = rest.len().min(room);
        let at = s.ws + WS_DISP + s.disp_len;
        mem::bytes_mut(at, n as u32).copy_from_slice(&rest[..n]);
        s.disp_len += n as u32;
        rest = &rest[n..];
    }
}

/// The line being displayed, as one command.
fn end_line() {
    let s = st();
    let (at, n) = (s.ws + WS_DISP, s.disp_len);
    s.disp_len = 0;
    emit_text(mem::bytes(at, n));
}

/// [`flush`], after a line a `NO ADVANCING` left open: the task is giving the
/// host control, and what it displayed must reach the console.
pub fn flush_all() {
    if st().disp_len > 0 {
        end_line();
    }
    flush();
}
