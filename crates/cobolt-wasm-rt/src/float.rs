// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Floating-point text, without `core::fmt`.
//!
//! `DISPLAY` of a `COMP-1`/`COMP-2` item or of an intrinsic function's result
//! prints what Rust's `f64::to_string` prints: the shortest decimal that reads
//! back as the same double, never in exponent form. This is the intrinsic-
//! function work of phase B; until it lands only whole values are exact.

/// Format `f` into `out`; returns the length.
pub fn format_f64(f: f64, out: &mut [u8; 64]) -> usize {
    if f != f {
        out[..3].copy_from_slice(b"NaN");
        return 3;
    }
    let mut w = 0;
    let mut x = f;
    if x < 0.0 || (x == 0.0 && x.to_bits() >> 63 == 1) {
        out[w] = b'-';
        w += 1;
        x = -x;
    }
    if x == f64::INFINITY {
        out[w..w + 3].copy_from_slice(b"inf");
        return w + 3;
    }
    if x < 1e18 && x == (x as u64) as f64 {
        let mut n = x as u64;
        let mut tmp = [0u8; 20];
        let mut i = tmp.len();
        loop {
            i -= 1;
            tmp[i] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        let d = &tmp[i..];
        out[w..w + d.len()].copy_from_slice(d);
        return w + d.len();
    }
    // Not yet exact for fractions: nine fractional digits, trimmed.
    let int = x as u64;
    let mut frac = ((x - int as f64) * 1e9 + 0.5) as u64;
    let mut n = int;
    let mut tmp = [0u8; 20];
    let mut i = tmp.len();
    loop {
        i -= 1;
        tmp[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    let d = &tmp[i..];
    out[w..w + d.len()].copy_from_slice(d);
    w += d.len();
    out[w] = b'.';
    w += 1;
    let mut fd = [b'0'; 9];
    for k in (0..9).rev() {
        fd[k] = b'0' + (frac % 10) as u8;
        frac /= 10;
    }
    let mut end = 9;
    while end > 1 && fd[end - 1] == b'0' {
        end -= 1;
    }
    out[w..w + end].copy_from_slice(&fd[..end]);
    w + end
}
