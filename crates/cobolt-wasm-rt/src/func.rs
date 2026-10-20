// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Intrinsic functions (`FUNCTION name(args)`).
//!
//! A call is a **record** in the constant pool: the function number, the
//! number of arguments, two flags the function may need (the decimal-comma
//! switch and the currency sign) and one operand address per argument. The
//! generator flattens `ALL` subscripts into one operand per element and
//! evaluates nested calls and computed arguments first, so by the time a
//! routine here runs every argument is a plain operand.
//!
//! A numeric function leaves its result on the register stack
//! ([`eval_num`]); a character function writes its bytes into one of the
//! **character temporaries** ([`eval_chars`]) that the operand tag
//! `T_CTEMP` names. The arithmetic follows the interpreter's: `f64` for the
//! transcendental and statistical functions, exact decimals where it keeps
//! them (`SUM`, `MAX`, `MIN`, the integer functions).
//!
//! ```text
//!  0  id        u32   F_*
//!  4  argc      u32
//!  8  flags     u32   bit 0: decimal comma
//! 12  currency  u32   the program's currency sign
//! 16  args      argc × u32 operand addresses
//! ```

use crate::cmp;
use crate::host;
use crate::mem;
use crate::opd::*;
use crate::state::{err, push, st, CTEMP_SIZE, NCTEMP, WS_CTEMP};
use crate::val::Val;

/// Function numbers. The generator's table of names is `NAMES`.
pub mod id {
    pub const ABS: u32 = 0;
    pub const ACOS: u32 = 1;
    pub const ANNUITY: u32 = 2;
    pub const ASIN: u32 = 3;
    pub const ATAN: u32 = 4;
    pub const COS: u32 = 5;
    pub const EXP: u32 = 6;
    pub const EXP10: u32 = 7;
    pub const FACTORIAL: u32 = 8;
    pub const FRACTION_PART: u32 = 9;
    pub const INTEGER: u32 = 10;
    pub const INTEGER_PART: u32 = 11;
    pub const LOG: u32 = 12;
    pub const LOG10: u32 = 13;
    pub const MAX: u32 = 14;
    pub const MEAN: u32 = 15;
    pub const MEDIAN: u32 = 16;
    pub const MIDRANGE: u32 = 17;
    pub const MIN: u32 = 18;
    pub const MOD: u32 = 19;
    pub const NUMVAL: u32 = 20;
    pub const NUMVAL_C: u32 = 21;
    pub const NUMVAL_F: u32 = 22;
    pub const ORD: u32 = 23;
    pub const ORD_MAX: u32 = 24;
    pub const ORD_MIN: u32 = 25;
    pub const PI: u32 = 26;
    pub const PRESENT_VALUE: u32 = 27;
    pub const RANDOM: u32 = 28;
    pub const RANGE: u32 = 29;
    pub const REM: u32 = 30;
    pub const SIN: u32 = 31;
    pub const SQRT: u32 = 32;
    pub const STANDARD_DEVIATION: u32 = 33;
    pub const SUM: u32 = 34;
    pub const TAN: u32 = 35;
    pub const VARIANCE: u32 = 36;
    pub const LENGTH: u32 = 37;
    pub const STORED_CHAR_LENGTH: u32 = 38;
    pub const TEST_NUMVAL: u32 = 39;
    pub const INTEGER_OF_DATE: u32 = 40;
    pub const DATE_OF_INTEGER: u32 = 41;
    pub const INTEGER_OF_DAY: u32 = 42;
    pub const DAY_OF_INTEGER: u32 = 43;
    pub const YEAR_TO_YYYY: u32 = 44;
    // Character-valued from here.
    pub const UPPER_CASE: u32 = 64;
    pub const LOWER_CASE: u32 = 65;
    pub const REVERSE: u32 = 66;
    pub const CHAR: u32 = 67;
    pub const TRIM: u32 = 68;
    pub const CONCATENATE: u32 = 69;
    pub const CURRENT_DATE: u32 = 70;
    /// `MAX` / `MIN` over alphanumeric arguments answer with the argument.
    pub const MAX_CHARS: u32 = 71;
    pub const MIN_CHARS: u32 = 72;
}

/// Name, number. `MAX` and `MIN` have a character form the generator picks
/// when any argument is alphanumeric.
pub const NAMES: &[(&str, u32)] = &[
    ("ABS", id::ABS),
    ("ACOS", id::ACOS),
    ("ANNUITY", id::ANNUITY),
    ("ASIN", id::ASIN),
    ("ATAN", id::ATAN),
    ("COS", id::COS),
    ("EXP", id::EXP),
    ("EXP10", id::EXP10),
    ("FACTORIAL", id::FACTORIAL),
    ("FRACTION-PART", id::FRACTION_PART),
    ("INTEGER", id::INTEGER),
    ("INTEGER-PART", id::INTEGER_PART),
    ("LOG", id::LOG),
    ("LOG10", id::LOG10),
    ("MAX", id::MAX),
    ("MEAN", id::MEAN),
    ("MEDIAN", id::MEDIAN),
    ("MIDRANGE", id::MIDRANGE),
    ("MIN", id::MIN),
    ("MOD", id::MOD),
    ("NUMVAL", id::NUMVAL),
    ("NUMVAL-C", id::NUMVAL_C),
    ("NUMVAL-F", id::NUMVAL_F),
    ("ORD", id::ORD),
    ("ORD-MAX", id::ORD_MAX),
    ("ORD-MIN", id::ORD_MIN),
    ("PI", id::PI),
    ("PRESENT-VALUE", id::PRESENT_VALUE),
    ("RANDOM", id::RANDOM),
    ("RANGE", id::RANGE),
    ("REM", id::REM),
    ("SIN", id::SIN),
    ("SQRT", id::SQRT),
    ("STANDARD-DEVIATION", id::STANDARD_DEVIATION),
    ("SUM", id::SUM),
    ("TAN", id::TAN),
    ("VARIANCE", id::VARIANCE),
    ("LENGTH", id::LENGTH),
    ("BYTE-LENGTH", id::LENGTH),
    ("LENGTH-AN", id::LENGTH),
    ("STORED-CHAR-LENGTH", id::STORED_CHAR_LENGTH),
    ("TEST-NUMVAL", id::TEST_NUMVAL),
    ("INTEGER-OF-DATE", id::INTEGER_OF_DATE),
    ("DATE-OF-INTEGER", id::DATE_OF_INTEGER),
    ("INTEGER-OF-DAY", id::INTEGER_OF_DAY),
    ("DAY-OF-INTEGER", id::DAY_OF_INTEGER),
    ("YEAR-TO-YYYY", id::YEAR_TO_YYYY),
    ("UPPER-CASE", id::UPPER_CASE),
    ("LOWER-CASE", id::LOWER_CASE),
    ("REVERSE", id::REVERSE),
    ("CHAR", id::CHAR),
    ("TRIM", id::TRIM),
    ("CONCATENATE", id::CONCATENATE),
    ("CURRENT-DATE", id::CURRENT_DATE),
];

/// Most arguments one call takes (an `ALL` over a table counts each element).
pub const MAX_ARGS: usize = 256;

struct Call {
    base: u32,
    rec: u32,
    argc: usize,
    decimal_comma: bool,
    currency: u8,
}

impl Call {
    fn read(base: u32, rec: u32) -> Call {
        let argc = mem::rd_u32(rec + 4) as usize;
        if argc > MAX_ARGS {
            crate::fail(err::BAD_OPERAND);
        }
        Call { base, rec, argc, decimal_comma: mem::rd_u32(rec + 8) & 1 != 0, currency: mem::rd_u32(rec + 12) as u8 }
    }
    fn addr(&self, i: usize) -> u32 {
        mem::rd_u32(self.rec + 16 + 4 * i as u32)
    }
    fn opd(&self, i: usize) -> Opd {
        if i >= self.argc {
            crate::fail(err::BAD_OPERAND);
        }
        parse(self.addr(i))
    }
    fn val(&self, i: usize) -> Val {
        value(self.base, &self.opd(i))
    }
    fn f64(&self, i: usize) -> f64 {
        self.val(i).as_f64()
    }
    /// The integer part of an argument, as the interpreter's `as_i64`.
    fn int(&self, i: usize) -> i64 {
        to_i64(&self.val(i))
    }
    /// The bytes of an argument: an item's or literal's characters, a
    /// number's digits.
    fn bytes(&self, i: usize, buf: &mut [u8; 40]) -> (u32, u32) {
        let o = self.opd(i);
        match o.tag {
            T_LIT_ALNUM | T_ALL | T_CTEMP | T_FIG => {
                let l = locate(self.base, &o);
                (l.addr, l.len)
            }
            T_ITEM => {
                let l = locate(self.base, &o);
                if l.fd.is_numeric() {
                    let n = crate::mv::image_chars(&value(self.base, &o), &l.fd, buf);
                    let (at, _) = crate::mv::scratch();
                    mem::bytes_mut(at, n as u32).copy_from_slice(&buf[..n]);
                    (at, n as u32)
                } else {
                    (l.addr, l.len)
                }
            }
            _ => {
                // A number: its digits, no sign.
                let v = value(self.base, &o);
                let fd = crate::fd::Fd { kind: crate::fd::K_NUM_DISPLAY, flags: 0, digits: 0, scale: 0, len: 0, aux: 0 };
                let n = crate::mv::image_chars(&v, &fd, buf);
                let (at, _) = crate::mv::scratch();
                mem::bytes_mut(at, n as u32).copy_from_slice(&buf[..n]);
                (at, n as u32)
            }
        }
    }
}

fn to_i64(v: &Val) -> i64 {
    match v {
        Val::Dec(n) => {
            let p = cobolt_decimal::pow10(n.decimals as u32).unwrap_or(1);
            (n.mantissa / p) as i64
        }
        Val::F(f) => *f as i64,
    }
}

fn from_f(f: f64) -> Val {
    Val::F(f)
}

// ── numeric functions ─────────────────────────────────────────────────────

/// `rt_func`: evaluate a numeric function and push its result.
pub fn eval_num(base: u32, rec: u32) {
    let c = Call::read(base, rec);
    let fid = mem::rd_u32(rec);
    let r = num(&c, fid);
    push(r);
}

fn num(c: &Call, fid: u32) -> Val {
    use id::*;
    match fid {
        ABS => from_f(libm::fabs(c.f64(0))),
        ACOS => from_f(libm::acos(c.f64(0))),
        ASIN => from_f(libm::asin(c.f64(0))),
        ATAN => from_f(libm::atan(c.f64(0))),
        COS => from_f(libm::cos(c.f64(0))),
        SIN => from_f(libm::sin(c.f64(0))),
        TAN => from_f(libm::tan(c.f64(0))),
        LOG => from_f(libm::log(c.f64(0))),
        LOG10 => from_f(libm::log10(c.f64(0))),
        EXP => from_f(libm::exp(c.f64(0))),
        EXP10 => from_f(libm::pow(10.0, c.f64(0))),
        SQRT => from_f(libm::sqrt(c.f64(0))),
        PI => from_f(core::f64::consts::PI),
        INTEGER => Val::int(libm::floor(c.f64(0)) as i64),
        INTEGER_PART => Val::int(libm::trunc(c.f64(0)) as i64),
        FRACTION_PART => {
            let x = c.f64(0);
            from_f(x - libm::trunc(x))
        }
        MOD => {
            let (a, b) = (c.f64(0), c.f64(1));
            if b == 0.0 {
                crate::fail(err::DIVISION_BY_ZERO);
            }
            from_f(a - libm::floor(a / b) * b)
        }
        REM => {
            let (a, b) = (c.f64(0), c.f64(1));
            if b == 0.0 {
                crate::fail(err::DIVISION_BY_ZERO);
            }
            from_f(a - libm::trunc(a / b) * b)
        }
        FACTORIAL => {
            let n = c.int(0).max(0) as i128;
            let mut f: i128 = 1;
            let mut k: i128 = 2;
            while k <= n {
                f = f.wrapping_mul(k);
                k += 1;
            }
            Val::int(f as i64)
        }
        MAX | MIN => {
            if c.argc == 0 {
                return Val::int(0);
            }
            let best = extreme(c, fid == MAX);
            c.val(best)
        }
        ORD_MAX | ORD_MIN => {
            if c.argc == 0 {
                return Val::int(0);
            }
            Val::int(extreme(c, fid == ORD_MAX) as i64 + 1)
        }
        SUM => {
            let mut total = Val::int(0);
            for i in 0..c.argc {
                total = total.add(&c.val(i));
            }
            total
        }
        MEAN => {
            if c.argc == 0 {
                return Val::int(0);
            }
            let mut s = 0.0;
            for i in 0..c.argc {
                s += c.f64(i);
            }
            from_f(s / c.argc as f64)
        }
        MEDIAN => {
            let mut xs = [0.0f64; MAX_ARGS];
            let n = c.argc;
            for (i, x) in xs.iter_mut().enumerate().take(n) {
                *x = c.f64(i);
            }
            let xs = &mut xs[..n];
            xs.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
            let m = if n == 0 {
                0.0
            } else if n % 2 == 1 {
                xs[n / 2]
            } else {
                (xs[n / 2 - 1] + xs[n / 2]) / 2.0
            };
            from_f(m)
        }
        MIDRANGE | RANGE => {
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            for i in 0..c.argc {
                let x = c.f64(i);
                lo = if x < lo { x } else { lo };
                hi = if x > hi { x } else { hi };
            }
            from_f(if fid == RANGE { hi - lo } else { (lo + hi) / 2.0 })
        }
        VARIANCE | STANDARD_DEVIATION => {
            if c.argc == 0 {
                return Val::int(0);
            }
            let mut s = 0.0;
            for i in 0..c.argc {
                s += c.f64(i);
            }
            let mean = s / c.argc as f64;
            let mut q = 0.0;
            for i in 0..c.argc {
                let d = c.f64(i) - mean;
                q += powi(d, 2);
            }
            let var = q / c.argc as f64;
            from_f(if fid == VARIANCE { var } else { libm::sqrt(var) })
        }
        ANNUITY => {
            let (rate, n) = (c.f64(0), c.f64(1));
            let v = if rate == 0.0 {
                if n == 0.0 {
                    0.0
                } else {
                    1.0 / n
                }
            } else {
                rate / (1.0 - libm::pow(1.0 + rate, -n))
            };
            from_f(v)
        }
        PRESENT_VALUE => {
            let rate = c.f64(0);
            let mut total = 0.0;
            for i in 1..c.argc {
                total += c.f64(i) / powi(1.0 + rate, i as i32);
            }
            from_f(total)
        }
        NUMVAL | NUMVAL_C => {
            let mut buf = [0u8; 40];
            let (at, len) = c.bytes(0, &mut buf);
            let text = mem::bytes(at, len);
            let mut cur = [0u8; 8];
            let mut cur_len = 0usize;
            if fid == NUMVAL_C {
                if c.argc > 1 {
                    let mut b2 = [0u8; 40];
                    let (a2, l2) = c.bytes(1, &mut b2);
                    let t = trim(mem::bytes(a2, l2));
                    cur_len = t.len().min(8);
                    cur[..cur_len].copy_from_slice(&t[..cur_len]);
                } else {
                    cur[0] = c.currency;
                    cur_len = 1;
                }
            }
            from_f(numval(text, &cur[..cur_len], c.decimal_comma))
        }
        NUMVAL_F => {
            let mut buf = [0u8; 40];
            let (at, len) = c.bytes(0, &mut buf);
            from_f(parse_f64_lenient(mem::bytes(at, len)))
        }
        TEST_NUMVAL => {
            let mut buf = [0u8; 40];
            let (at, len) = c.bytes(0, &mut buf);
            Val::int(test_numval(mem::bytes(at, len)))
        }
        LENGTH => {
            let o = c.opd(0);
            let n = match o.tag {
                T_LIT_NUM | T_TEMP => {
                    let mut buf = [0u8; 40];
                    let fd = crate::fd::Fd { kind: crate::fd::K_NUM_DISPLAY, flags: 0, digits: 0, scale: 0, len: 0, aux: 0 };
                    crate::mv::image_chars(&value(c.base, &o), &fd, &mut buf) as u32
                }
                _ => locate(c.base, &o).len,
            };
            Val::int(n as i64)
        }
        STORED_CHAR_LENGTH => {
            let mut buf = [0u8; 40];
            let (at, len) = c.bytes(0, &mut buf);
            let b = mem::bytes(at, len);
            let end = b.iter().rposition(|&x| x != b' ').map_or(0, |i| i + 1);
            Val::int(end as i64)
        }
        ORD => {
            let mut buf = [0u8; 40];
            let (at, len) = c.bytes(0, &mut buf);
            let b = if len > 0 { mem::rd_u8(at) } else { 0 };
            Val::int(b as i64 + 1)
        }
        RANDOM => {
            if c.argc > 0 {
                let seed = c.int(0);
                st().random = mix_seed(seed as u64);
            }
            from_f(next_random())
        }
        INTEGER_OF_DATE => Val::int(integer_of_date(c.int(0))),
        DATE_OF_INTEGER => Val::int(date_of_integer(c.int(0))),
        INTEGER_OF_DAY => Val::int(integer_of_day(c.int(0))),
        DAY_OF_INTEGER => Val::int(day_of_integer(c.int(0))),
        YEAR_TO_YYYY => {
            let yy = c.int(0);
            let window = if c.argc > 1 { c.int(1) } else { 50 };
            // SAFETY: plain host call.
            let ms = unsafe { host::now() };
            let days = libm::floor(ms / 86_400_000.0) as i64;
            let cur_year = ymd_from_days(days).0;
            let max_year = cur_year + window;
            let mut yyyy = (max_year / 100) * 100 + yy;
            if yyyy > max_year {
                yyyy -= 100;
            }
            Val::int(yyyy)
        }
        _ => crate::fail(err::BAD_OPERAND),
    }
}

/// The position of the greatest (or least) argument; the first of equals wins.
fn extreme(c: &Call, want_max: bool) -> usize {
    let mut best = 0usize;
    for i in 1..c.argc {
        let op = if want_max { cmp::OP_GT } else { cmp::OP_LT };
        if cmp::relation(c.base, c.addr(i), c.addr(best), op) {
            best = i;
        }
    }
    best
}

/// `x.powi(n)` as the compiler's runtime computes it (square and multiply).
fn powi(mut a: f64, n: i32) -> f64 {
    let recip = n < 0;
    let mut b = n;
    let mut r = 1.0;
    loop {
        if b & 1 != 0 {
            r *= a;
        }
        b /= 2;
        if b == 0 {
            break;
        }
        a *= a;
    }
    if recip {
        1.0 / r
    } else {
        r
    }
}

fn trim(b: &[u8]) -> &[u8] {
    let s = b.iter().position(|&x| x != b' ').unwrap_or(b.len());
    let e = b.iter().rposition(|&x| x != b' ').map_or(s, |i| i + 1);
    &b[s..e.max(s)]
}

/// The value of a `NUMVAL` / `NUMVAL-C` argument: a sign at either end (or
/// `CR`/`DB`), spaces nearly anywhere, a currency string, digit-group
/// separators and at most one decimal point.
fn numval(arg: &[u8], currency: &[u8], decimal_comma: bool) -> f64 {
    let mut t = trim(arg);
    let mut negative = false;
    let n = t.len();
    let tail2 = if n >= 2 { [t[n - 2].to_ascii_uppercase(), t[n - 1].to_ascii_uppercase()] } else { [0, 0] };
    if &tail2 == b"CR" || &tail2 == b"DB" {
        t = &t[..n - 2];
        negative = true;
    } else if let Some((&last, rest)) = t.split_last().filter(|(l, _)| **l == b'-') {
        let _ = last;
        t = trim(rest);
        negative = true;
    } else if let Some((&first, rest)) = t.split_first().filter(|(f, _)| **f == b'-') {
        let _ = first;
        t = trim(rest);
        negative = true;
    } else if let Some((_, rest)) = t.split_last().filter(|(l, _)| **l == b'+') {
        t = trim(rest);
    } else if let Some((_, rest)) = t.split_first().filter(|(f, _)| **f == b'+') {
        t = trim(rest);
    }
    let point = if decimal_comma { b',' } else { b'.' };
    let mut digits = [0u8; 64];
    let mut k = 0usize;
    let mut i = 0usize;
    while i < t.len() {
        // Remove every occurrence of the currency string.
        if !currency.is_empty() && t[i..].starts_with(currency) {
            i += currency.len();
            continue;
        }
        let ch = t[i];
        if ch.is_ascii_digit() {
            if k < digits.len() {
                digits[k] = ch;
                k += 1;
            }
        } else if ch == point && k < digits.len() {
            digits[k] = b'.';
            k += 1;
        }
        i += 1;
    }
    let v = parse_f64(&digits[..k]).unwrap_or(0.0);
    if negative {
        -v
    } else {
        v
    }
}

fn parse_f64(b: &[u8]) -> Option<f64> {
    core::str::from_utf8(b).ok()?.parse::<f64>().ok()
}

/// `NUMVAL-F`: like a float literal, `+` and spaces ignored.
fn parse_f64_lenient(b: &[u8]) -> f64 {
    let mut buf = [0u8; 64];
    let mut k = 0usize;
    for &ch in trim(b) {
        if ch != b'+' && ch != b' ' && k < buf.len() {
            buf[k] = ch;
            k += 1;
        }
    }
    parse_f64(&buf[..k]).unwrap_or(0.0)
}

/// `TEST-NUMVAL`: 0 when the string is a valid argument, else the position of
/// the first offending character (as the interpreter reports it).
fn test_numval(b: &[u8]) -> i64 {
    let t = trim(b);
    let ok_char = |c: u8| c.is_ascii_digit() || matches!(c, b'.' | b'+' | b'-' | b',' | b' ');
    let ok = !t.is_empty() && t.iter().all(|&c| ok_char(c));
    if ok && parse_f64(t).is_some() {
        return 0;
    }
    match t.iter().position(|&c| !ok_char(c)) {
        Some(p) => p as i64 + 1,
        None => t.len() as i64 + 1,
    }
}

// ── dates ─────────────────────────────────────────────────────────────────

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y.max(0)) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn integer_of_date(yyyymmdd: i64) -> i64 {
    let (y, m, d) = (yyyymmdd / 10000, (yyyymmdd / 100) % 100, yyyymmdd % 100);
    let mut days = 0i64;
    let mut yy = 1601;
    while yy < y {
        days += if is_leap(yy) { 366 } else { 365 };
        yy += 1;
    }
    for mm in 1..m {
        days += days_in_month(y, mm);
    }
    days + d
}

fn date_of_integer(n: i64) -> i64 {
    let mut rem = n;
    let mut y = 1601i64;
    loop {
        let dy = if is_leap(y) { 366 } else { 365 };
        if rem > dy {
            rem -= dy;
            y += 1;
        } else {
            break;
        }
    }
    let mut m = 1i64;
    loop {
        let dm = days_in_month(y, m);
        if rem > dm {
            rem -= dm;
            m += 1;
        } else {
            break;
        }
    }
    y * 10000 + m * 100 + rem
}

fn integer_of_day(yyyyddd: i64) -> i64 {
    let (y, ddd) = (yyyyddd / 1000, yyyyddd % 1000);
    let mut days = 0i64;
    let mut yy = 1601;
    while yy < y {
        days += if is_leap(yy) { 366 } else { 365 };
        yy += 1;
    }
    days + ddd
}

fn day_of_integer(n: i64) -> i64 {
    let mut rem = n;
    let mut y = 1601i64;
    loop {
        let dy = if is_leap(y) { 366 } else { 365 };
        if rem > dy {
            rem -= dy;
            y += 1;
        } else {
            break;
        }
    }
    y * 1000 + rem
}

/// `(year, month, day)` from days since 1970-01-01.
fn ymd_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ── RANDOM ────────────────────────────────────────────────────────────────

const LCG_MUL: u64 = 6364136223846793005;
const LCG_ADD: u64 = 1442695040888963407;

/// The generator's state after `FUNCTION RANDOM(seed)`.
pub fn mix_seed(seed: u64) -> u64 {
    seed.wrapping_mul(LCG_MUL).wrapping_add(LCG_ADD)
}

/// The initial state, as the interpreter starts it.
pub const RANDOM_INITIAL: u64 = LCG_MUL;

fn next_random() -> f64 {
    let s = st();
    let next = s.random.wrapping_mul(LCG_MUL).wrapping_add(LCG_ADD);
    s.random = next;
    (next >> 11) as f64 / (1u64 << 53) as f64
}

// ── character functions ───────────────────────────────────────────────────

/// `rt_func_chars`: evaluate a character function into character temporary `k`.
pub fn eval_chars(base: u32, rec: u32, k: u32) {
    let c = Call::read(base, rec);
    let fid = mem::rd_u32(rec);
    if k as usize >= NCTEMP {
        crate::fail(err::BAD_OPERAND);
    }
    let at = st().ws + WS_CTEMP + k * CTEMP_SIZE;
    let out = |n: usize| -> &'static mut [u8] { mem::bytes_mut(at, n as u32) };
    let mut len = 0usize;
    let mut put = |bytes: &[u8], len: &mut usize| {
        if *len + bytes.len() > CTEMP_SIZE as usize {
            crate::fail(err::SCRATCH_OVERFLOW);
        }
        out(*len + bytes.len())[*len..].copy_from_slice(bytes);
        *len += bytes.len();
    };
    use id::*;
    match fid {
        UPPER_CASE | LOWER_CASE | REVERSE | TRIM => {
            let mut buf = [0u8; 40];
            let (a, l) = c.bytes(0, &mut buf);
            let src = mem::bytes(a, l);
            match fid {
                UPPER_CASE => {
                    if src.len() > CTEMP_SIZE as usize {
                        crate::fail(err::SCRATCH_OVERFLOW);
                    }
                    let o = out(src.len());
                    for (d, s) in o.iter_mut().zip(src) {
                        *d = s.to_ascii_uppercase();
                    }
                    len = src.len();
                }
                LOWER_CASE => {
                    if src.len() > CTEMP_SIZE as usize {
                        crate::fail(err::SCRATCH_OVERFLOW);
                    }
                    let o = out(src.len());
                    for (d, s) in o.iter_mut().zip(src) {
                        *d = s.to_ascii_lowercase();
                    }
                    len = src.len();
                }
                REVERSE => {
                    if src.len() > CTEMP_SIZE as usize {
                        crate::fail(err::SCRATCH_OVERFLOW);
                    }
                    let o = out(src.len());
                    for (i, d) in o.iter_mut().enumerate() {
                        *d = src[src.len() - 1 - i];
                    }
                    len = src.len();
                }
                _ => {
                    let t = trim(src);
                    put(t, &mut len);
                }
            }
        }
        CHAR => {
            let n = c.int(0).clamp(1, 256);
            put(&[(n - 1) as u8], &mut len);
        }
        CONCATENATE => {
            for i in 0..c.argc {
                let mut buf = [0u8; 40];
                let (a, l) = c.bytes(i, &mut buf);
                // A snapshot: the argument may live in this very slot.
                let src = mem::bytes(a, l);
                put(src, &mut len);
            }
        }
        CURRENT_DATE => {
            // SAFETY: plain host call.
            let ms = unsafe { host::now() };
            let mut text = [0u8; 21];
            current_date(ms, &mut text);
            put(&text, &mut len);
        }
        MAX_CHARS | MIN_CHARS => {
            if c.argc == 0 {
                put(b"0", &mut len);
            } else {
                let best = extreme(&c, fid == MAX_CHARS);
                let mut buf = [0u8; 40];
                let (a, l) = c.bytes(best, &mut buf);
                let src = mem::bytes(a, l);
                put(src, &mut len);
            }
        }
        _ => crate::fail(err::BAD_OPERAND),
    }
    st().ctemp_len[k as usize] = len as u32;
}

/// `YYYYMMDDHHMMSSCC+0000`: the host clock is UTC.
fn current_date(ms: f64, out: &mut [u8; 21]) {
    let secs = libm::floor(ms / 1000.0) as i64;
    let cs = (((ms - (secs as f64) * 1000.0) / 10.0) as u64).min(99);
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400) as u64;
    let (y, m, d) = ymd_from_days(days);
    let mut at = 0usize;
    let mut digits = |v: u64, w: usize, at: &mut usize| {
        let mut v = v;
        for i in (0..w).rev() {
            out[*at + i] = b'0' + (v % 10) as u8;
            v /= 10;
        }
        *at += w;
    };
    digits(y as u64, 4, &mut at);
    digits(m as u64, 2, &mut at);
    digits(d as u64, 2, &mut at);
    digits(rem / 3600, 2, &mut at);
    digits((rem / 60) % 60, 2, &mut at);
    digits(rem % 60, 2, &mut at);
    digits(cs, 2, &mut at);
    out[16..21].copy_from_slice(b"+0000");
}

/// The 21 characters `CURRENT-DATE` yields at `ms` milliseconds since the Unix
/// epoch (UTC). The generator uses it for `WHEN-COMPILED`.
pub fn current_date_text(ms: f64) -> [u8; 21] {
    let mut out = [0u8; 21];
    current_date(ms, &mut out);
    out
}

/// The location a `T_CTEMP` operand names.
pub fn ctemp_loc(k: u32) -> (u32, u32) {
    let k = (k as usize) % NCTEMP;
    (st().ws + WS_CTEMP + k as u32 * CTEMP_SIZE, st().ctemp_len[k])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip_through_the_integer_calendar() {
        // 1601-01-01 is day 1; the standard's own examples.
        assert_eq!(integer_of_date(16010101), 1);
        assert_eq!(integer_of_date(16010102), 2);
        assert_eq!(integer_of_date(16020101), 366);
        for n in (1..200_000).step_by(37) {
            let d = date_of_integer(n);
            assert_eq!(integer_of_date(d), n, "day {n} -> {d}");
            let j = day_of_integer(n);
            assert_eq!(integer_of_day(j), n, "day {n} -> julian {j}");
        }
        // 2000 is a leap year, 1900 is not.
        assert_eq!(date_of_integer(integer_of_date(20000229)), 20000229);
        assert_eq!(integer_of_date(19000301) - integer_of_date(19000228), 1);
    }

    #[test]
    fn numval_reads_signs_currency_and_credit_suffixes() {
        assert_eq!(numval(b"  -  4929.0323", b"", false), -4929.0323);
        assert_eq!(numval(b"200.0002  - ", b"", false), -200.0002);
        assert_eq!(numval(b"12.5CR", b"", false), -12.5);
        assert_eq!(numval(b"12.5db", b"", false), -12.5);
        assert_eq!(numval(b"+ 7", b"", false), 7.0);
        assert_eq!(numval(b"$1,234.50", b"$", false), 1234.5);
        assert_eq!(numval(b"1.234,50", b"", true), 1234.5);
        assert_eq!(numval(b"", b"", false), 0.0);
    }

    #[test]
    fn test_numval_names_the_first_bad_character() {
        assert_eq!(test_numval(b" 12.5 "), 0);
        assert_eq!(test_numval(b"12a"), 3);
        assert_eq!(test_numval(b""), 1);
    }

    #[test]
    fn the_random_sequence_is_the_interpreters() {
        // seed 7, then the next value: fixed by the shared LCG.
        let s0 = mix_seed(7);
        let n1 = s0.wrapping_mul(LCG_MUL).wrapping_add(LCG_ADD);
        let v1 = (n1 >> 11) as f64 / (1u64 << 53) as f64;
        crate::state::reset();
        crate::state::st().random = s0;
        assert_eq!(next_random(), v1);
        assert!((0.0..1.0).contains(&v1));
    }

    #[test]
    fn powi_matches_repeated_multiplication() {
        assert_eq!(powi(1.1, 3), 1.1 * 1.1 * 1.1);
        assert_eq!(powi(2.0, 10), 1024.0);
        assert_eq!(powi(2.0, -2), 0.25);
        assert_eq!(powi(5.0, 0), 1.0);
    }

    #[test]
    fn the_current_date_text_is_twenty_one_characters() {
        // 2026-10-20 12:34:56.78 UTC
        let ms = 1_792_499_696_780.0;
        let t = current_date_text(ms);
        assert_eq!(&t[..], b"2026102012345678+0000");
    }
}
