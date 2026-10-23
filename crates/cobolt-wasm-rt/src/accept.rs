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
fn put(base: u32, target: u32, text: &[u8]) {
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
