// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `ACCEPT`: the date and time (from the host clock) and, in a conformance
//! build, a line of operator input.
//!
//! The clock is `glue.now()`, milliseconds since the Unix epoch in **UTC**; the
//! page can offer local time later by adjusting what it returns. `DATE` is
//! `YYMMDD`, `TIME` is `HHMMSSCC`, `DAY` is `YYDDD` and `DAY-OF-WEEK` is one
//! digit with Monday as 1 (COBOL-85 6.2).

use crate::codec;
use crate::host;
use crate::mem;
use crate::mv;
use crate::opd::*;
use crate::state::{st, SCRATCH_SIZE, WS_SCRATCH};
use crate::val::Val;
use cobolt_decimal::CobolNumeric;

pub const ACC_CONSOLE: u32 = 0;
pub const ACC_DATE: u32 = 1;
pub const ACC_TIME: u32 = 2;
pub const ACC_DAY: u32 = 3;
pub const ACC_DAY_OF_WEEK: u32 = 4;

/// Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn digits(buf: &mut [u8], at: &mut usize, mut v: u64, width: usize) {
    for i in (0..width).rev() {
        buf[*at + i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    *at += width;
}

/// The characters `kind` yields at time `ms`.
fn clock_text(kind: u32, ms: f64, out: &mut [u8; 16]) -> usize {
    let secs = libm::floor(ms / 1000.0) as i64;
    let cs = (((ms - (secs as f64) * 1000.0) / 10.0) as u64).min(99);
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400) as u64;
    let (y, m, d) = civil(days);
    let mut at = 0;
    match kind {
        ACC_DATE => {
            digits(out, &mut at, (y % 100) as u64, 2);
            digits(out, &mut at, m as u64, 2);
            digits(out, &mut at, d as u64, 2);
        }
        ACC_TIME => {
            digits(out, &mut at, rem / 3600, 2);
            digits(out, &mut at, (rem / 60) % 60, 2);
            digits(out, &mut at, rem % 60, 2);
            digits(out, &mut at, cs, 2);
        }
        ACC_DAY => {
            let jan1 = {
                // days from 1970-01-01 to January 1st of year y
                let yy = y - 1;
                (yy * 365 + yy / 4 - yy / 100 + yy / 400) - (1969 * 365 + 1969 / 4 - 1969 / 100 + 1969 / 400)
            };
            digits(out, &mut at, (y % 100) as u64, 2);
            digits(out, &mut at, (days - jan1 + 1) as u64, 3);
        }
        _ => {
            // 1970-01-01 was a Thursday (4 with Monday = 1).
            let dow = (days + 3).rem_euclid(7) + 1;
            digits(out, &mut at, dow as u64, 1);
        }
    }
    at
}

/// Put characters into the receiver the way `MOVE` of an unsigned integer
/// string does.
pub(crate) fn put(base: u32, target: u32, text: &[u8]) {
    let loc = locate(base, &parse(target));
    if loc.fd.is_numeric() {
        if let Some(n) = codec::parse_text(text) {
            codec::store_value(&loc, Val::Dec(n), false, false);
        }
    } else {
        mv::put_bytes(&loc, text, loc.fd.justified());
    }
}

pub fn accept(base: u32, target: u32, kind: u32) {
    if kind == ACC_CONSOLE {
        accept_console(base, target);
        return;
    }
    accept_clock(base, target, kind);
}

/// One line of operator input (conformance builds: handle 0). Apart from the
/// clock so that a browser module's `ACCEPT … FROM DATE` does not link the
/// file import.
pub fn accept_console(base: u32, target: u32) {
    let scratch = st().ws + WS_SCRATCH;
    // SAFETY: the glue writes at most `cap` bytes into the scratch area.
    let n = unsafe { host::file_read(0, scratch, SCRATCH_SIZE.min(4096)) };
    let line: &[u8] = if n > 0 { mem::bytes(scratch, n as u32) } else { &[] };
    let loc = locate(base, &parse(target));
    if n >= 0 || loc.fd.is_alnum_like() {
        put(base, target, line);
    }
}

/// `ACCEPT … FROM DATE | TIME | DAY | DAY-OF-WEEK`, from the host clock.
pub fn accept_clock(base: u32, target: u32, kind: u32) {
    // SAFETY: plain host call.
    let ms = unsafe { host::now() };
    let mut out = [0u8; 16];
    let n = clock_text(kind, ms, &mut out);
    put(base, target, &out[..n]);
    let _ = CobolNumeric::integer(0);
}

// ── A desktop module (DESKTOP-WASM-001 T12, ABI A29) ────────────────────────
// The console, the environment and the command line, through the desktop
// runner's `prn` imports. What a browser module links must keep its bytes
// (AC16), and the blob's routines are compiled in the order of their symbol
// names, each placed where it is first named: these names sort after
// `accept_clock`, which names `put` first. They use only signatures the blob
// already had (the generator copies the blob's types in order), index nothing
// and divide by nothing that can be zero (a bounds or a zero check adds a panic
// location to the read-only data), and stay below every line of the code above
// (a panic location records its line).

// The kinds of `rt_accept_env`.
/// `FROM ENVIRONMENT "name"`: `key` is the name's literal.
pub const ACC_ENV_VAR: u32 = 5;
/// `FROM ENVIRONMENT-VALUE`: `key` is the register `DISPLAY … UPON
/// ENVIRONMENT-NAME` fills.
pub const ACC_ENV_VALUE: u32 = 6;
pub const ACC_COMMAND_LINE: u32 = 7;
pub const ACC_ARG_COUNT: u32 = 8;
/// `FROM ARGUMENT-VALUE`: `key` is the pointer `DISPLAY … UPON ARGUMENT-NUMBER`
/// sets (0 until then, which means 1).
pub const ACC_ARG_VALUE: u32 = 9;

/// The kinds of `env_get` (ABI A29).
const ENV_VAR: u32 = 0;
const ENV_ARG: u32 = 1;
const ENV_ARG_COUNT: u32 = 2;
const ENV_COMMAND_LINE: u32 = 3;

/// One line of the operator's console in a desktop module (ABI A29
/// `con_read`), its line end removed. At the end of the input an
/// alphanumeric receiver gets spaces and a numeric one keeps its value, as in
/// the conformance build.
pub fn accept_native_console(base: u32, target: u32) {
    let scratch = st().ws + WS_SCRATCH;
    // SAFETY: the host writes at most `cap` bytes into the scratch area.
    let n = unsafe { host::con_read(scratch, SCRATCH_SIZE) };
    let line: &[u8] = if n > 0 { mem::bytes(scratch, (n as u32).min(SCRATCH_SIZE)) } else { &[] };
    let loc = locate(base, &parse(target));
    if n >= 0 || loc.fd.is_alnum_like() {
        put(base, target, line);
    }
}

/// Where the name a register holds starts and ends: without the white space
/// around it (the desktop trims it) and the zero bytes of a register never set.
fn native_name_bounds(b: &[u8]) -> (usize, usize) {
    let pad = |c: &u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C | 0);
    let start = b.iter().position(|c| !pad(c)).unwrap_or(b.len());
    let end = b.iter().rposition(|c| !pad(c)).map_or(start, |e| e + 1);
    (start, end)
}

/// `ACCEPT … FROM ENVIRONMENT "name" | ENVIRONMENT-VALUE | COMMAND-LINE |
/// ARGUMENT-NUMBER | ARGUMENT-VALUE` in a desktop module (ABI A29 `env_get`,
/// `ACC_ENV_VAR` …). `key` is the name's literal, the register `DISPLAY …
/// UPON ENVIRONMENT-NAME` fills, or the argument pointer `DISPLAY … UPON
/// ARGUMENT-NUMBER` sets (0 until then, which means 1). A variable or an
/// argument that is not there is empty text, as on the desktop.
pub fn accept_native_environment(base: u32, target: u32, kind: u32, key: u32) {
    let scratch = st().ws + WS_SCRATCH;
    // SAFETY (every call): the host writes at most `cap` bytes into the
    // scratch area, over the name it read there.
    let got = match kind {
        ACC_ENV_VAR | ACC_ENV_VALUE => {
            let loc = locate(base, &parse(key));
            let (start, end) = if kind == ACC_ENV_VALUE { native_name_bounds(mem::bytes(loc.addr, loc.len)) } else { (0, loc.len as usize) };
            let len = ((end - start) as u32).min(SCRATCH_SIZE);
            mem::copy(loc.addr + start as u32, scratch, len);
            unsafe { host::env_get(ENV_VAR, len, scratch, SCRATCH_SIZE) }
        }
        ACC_COMMAND_LINE => unsafe { host::env_get(ENV_COMMAND_LINE, 0, scratch, SCRATCH_SIZE) },
        ACC_ARG_VALUE => {
            let at = match value(base, &parse(key)) {
                Val::Dec(n) => n.mantissa.checked_div(cobolt_decimal::pow10(n.decimals as u32).unwrap_or(1)).unwrap_or(0) as i64,
                Val::F(f) => f as i64,
            };
            unsafe { host::env_get(ENV_ARG, at.clamp(1, u32::MAX as i64) as u32, scratch, SCRATCH_SIZE) }
        }
        _ => {
            // ACC_ARG_COUNT: the count, as the digits a MOVE of it gives.
            let mut v = unsafe { host::env_get(ENV_ARG_COUNT, 0, 0, 0) }.max(0) as u32;
            let mut out = [0u8; 10];
            let mut at = out.len();
            for slot in out.iter_mut().rev() {
                *slot = b'0' + (v % 10) as u8;
                v /= 10;
                at -= 1;
                if v == 0 {
                    break;
                }
            }
            put(base, target, out.get(at..).unwrap_or(&[]));
            return;
        }
    };
    let text: &[u8] = if got > 0 { mem::bytes(scratch, (got as u32).min(SCRATCH_SIZE)) } else { &[] };
    put(base, target, text);
}

/// `PRC_TEST_SEED` in a desktop module (DESKTOP-WASM-001 R28, T15): the
/// generator calls this first, in a program that uses `FUNCTION RANDOM`, with
/// `name` the operand of the literal `PRC_TEST_SEED`. A whole number from 0 to
/// 2^64 - 1 (white space around it and a leading `+` allowed, as the
/// interpreter reads the variable) starts the generator as `FUNCTION
/// RANDOM(seed)` starts it; anything else leaves its first state, as the
/// interpreter leaves its own. The arithmetic is `func::mix_seed`'s, written
/// out so that function keeps its one caller (see above).
pub fn native_random_pin(base: u32, name: u32) {
    let scratch = st().ws + WS_SCRATCH;
    let loc = locate(base, &parse(name));
    let len = loc.len.min(SCRATCH_SIZE);
    mem::copy(loc.addr, scratch, len);
    // SAFETY: the host writes at most `cap` bytes into the scratch area, over the name.
    let got = unsafe { host::env_get(ENV_VAR, len, scratch, SCRATCH_SIZE) };
    if got <= 0 {
        return;
    }
    let text = mem::bytes(scratch, (got as u32).min(SCRATCH_SIZE));
    let blank = |c: &u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C);
    let start = text.iter().position(|c| !blank(c)).unwrap_or(text.len());
    let end = text.iter().rposition(|c| !blank(c)).map_or(start, |e| e + 1);
    let mut digits = text.get(start..end).unwrap_or(&[]);
    if let Some((b'+', rest)) = digits.split_first() {
        digits = rest;
    }
    if digits.is_empty() {
        return;
    }
    let mut seed: u64 = 0;
    for &c in digits {
        if !c.is_ascii_digit() {
            return;
        }
        match seed.checked_mul(10).and_then(|s| s.checked_add(u64::from(c.wrapping_sub(b'0')))) {
            Some(s) => seed = s,
            None => return,
        }
    }
    st().random = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
}

/// The external switches of a desktop program (WASM-PARITY #55): the generator calls this before
/// the first statement of a program that declares one, with `name` the operand of the literal
/// `COBOL_SWITCHES` and `table` the one `rt_switches` takes. The variable, read through `env_get`,
/// holds `NAME=ON|OFF` pairs separated by commas, as `rcrun run` reads it: the first pair that
/// names a switch, by its implementor name or its mnemonic and in any case, sets it; `ON`, `1`,
/// `TRUE` and `YES` turn it on, `OFF`, `0`, `FALSE` and `NO` off, and a pair with any other state
/// is passed over. A switch no pair names keeps its first value, OFF.
pub fn native_switches(base: u32, table: u32, name: u32) {
    let scratch = st().ws + WS_SCRATCH;
    let loc = locate(base, &parse(name));
    let len = loc.len.min(SCRATCH_SIZE);
    mem::copy(loc.addr, scratch, len);
    // SAFETY: the host writes at most `cap` bytes into the scratch area, over the name.
    let got = unsafe { host::env_get(ENV_VAR, len, scratch, SCRATCH_SIZE) };
    if got <= 0 {
        return;
    }
    let text = mem::bytes(scratch, (got as u32).min(SCRATCH_SIZE));
    let blank = |c: &u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C);
    let trim = |s: &'static [u8]| -> &'static [u8] {
        let start = s.iter().position(|c| !blank(c)).unwrap_or(s.len());
        let end = s.iter().rposition(|c| !blank(c)).map_or(start, |e| e + 1);
        s.get(start..end).unwrap_or(&[])
    };
    let n = mem::rd_u32(table);
    for i in 0..n {
        let e = table + 4 + i * 20;
        let ext = mem::bytes(mem::rd_u32(e), mem::rd_u32(e + 4));
        let mn = mem::bytes(mem::rd_u32(e + 8), mem::rd_u32(e + 12));
        for pair in text.split(|c| *c == b',') {
            let Some(eq) = pair.iter().position(|c| *c == b'=') else { continue };
            let key = trim(pair.get(..eq).unwrap_or(&[]));
            if !key.eq_ignore_ascii_case(ext) && !key.eq_ignore_ascii_case(mn) {
                continue;
            }
            // The state in capitals as one number, so no table of words joins the blob's data
            // (which would move every module's data, AC16). A variable holds no zero byte.
            let state = trim(pair.get(eq + 1..).unwrap_or(&[]));
            if state.len() > 5 {
                continue;
            }
            let word = state.iter().fold(0u64, |w, c| w << 8 | u64::from(c.to_ascii_uppercase()));
            let on = match word {
                // ON, 1, TRUE, YES
                0x4F4E | 0x31 | 0x5452_5545 | 0x59_4553 => b'1',
                // OFF, 0, FALSE, NO
                0x4F_4646 | 0x30 | 0x46_414C_5345 | 0x4E4F => b'0',
                _ => continue,
            };
            mem::wr_u8(base + mem::rd_u32(e + 16), on);
            break;
        }
    }
}
