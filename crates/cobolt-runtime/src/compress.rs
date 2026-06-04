// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `WITH COMPRESSION` record compression for INDEXED files.
//!
//! A self-contained, dependency-free, byte-oriented run-length encoder
//! (PackBits-style) chosen for **maximum speed**. COBOL records are typically
//! fixed-length and heavily padded (trailing spaces, zero-filled numerics), so
//! long identical runs compress dramatically — well past the 50 % target on
//! realistic data. A one-byte tag selects the encoded form and **guarantees the
//! output never grows by more than one byte**: incompressible blocks are stored
//! verbatim.
//!
//! Encoding (after the 1-byte tag):
//! * control byte `0..=127`  → copy the next `n + 1` literal bytes;
//! * control byte `129..=255` → repeat the next byte `257 - n` times (2..=128);
//! * control byte `128`       → no-op.

/// Tag byte: the payload is RLE-encoded.
const TAG_RLE: u8 = 1;
/// Tag byte: the payload is stored verbatim (RLE would not have helped).
const TAG_RAW: u8 = 0;

/// Compress `data`. The result is `[tag] ++ payload` and is at most
/// `data.len() + 1` bytes (raw fallback when RLE does not shrink the input).
pub fn compress(data: &[u8]) -> Vec<u8> {
    let rle = rle_encode(data);
    if rle.len() < data.len() {
        let mut out = Vec::with_capacity(rle.len() + 1);
        out.push(TAG_RLE);
        out.extend_from_slice(&rle);
        out
    } else {
        let mut out = Vec::with_capacity(data.len() + 1);
        out.push(TAG_RAW);
        out.extend_from_slice(data);
        out
    }
}

/// Inverse of [`compress`].
pub fn decompress(data: &[u8]) -> Vec<u8> {
    match data.split_first() {
        Some((&TAG_RLE, rest)) => rle_decode(rest),
        Some((&TAG_RAW, rest)) => rest.to_vec(),
        _ => Vec::new(),
    }
}

fn rle_encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let n = data.len();
    let mut i = 0usize;
    while i < n {
        let b = data[i];
        // Length of the run of equal bytes at i (capped at 128).
        let mut run = 1usize;
        while i + run < n && data[i + run] == b && run < 128 {
            run += 1;
        }
        if run >= 3 {
            // Repeat run: control 129..=255 encodes a count of 2..=128.
            out.push((257 - run) as u8);
            out.push(b);
            i += run;
        } else {
            // Literal run: bytes until a >=3 run begins or the 128 cap is hit.
            let start = i;
            while i < n && (i - start) < 128 {
                if i + 2 < n && data[i] == data[i + 1] && data[i + 1] == data[i + 2] {
                    break;
                }
                i += 1;
            }
            let len = i - start;
            out.push((len - 1) as u8); // control 0..=127 → 1..=128 literals
            out.extend_from_slice(&data[start..i]);
        }
    }
    out
}

fn rle_decode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < data.len() {
        let ctrl = data[i];
        i += 1;
        if ctrl < 128 {
            let len = ctrl as usize + 1;
            if i + len > data.len() {
                break;
            }
            out.extend_from_slice(&data[i..i + len]);
            i += len;
        } else if ctrl == 128 {
            // no-op
        } else {
            let count = 257 - ctrl as usize;
            if i >= data.len() {
                break;
            }
            let b = data[i];
            i += 1;
            out.extend(std::iter::repeat(b).take(count));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_arbitrary_data() {
        let cases: &[&[u8]] = &[
            b"",
            b"A",
            b"AAAAAAAAAA",
            b"ABCDEFG",
            b"AAAABBBBCCCCDDDD",
            b"   \x00\x00\x00 mixed   runs 1111111",
        ];
        for c in cases {
            let enc = compress(c);
            assert_eq!(&decompress(&enc), c, "round-trip failed for {c:?}");
        }
    }

    #[test]
    fn padded_cobol_record_beats_fifty_percent() {
        // A 100-byte record: a few fields, the rest space/zero padding.
        let mut rec = Vec::new();
        rec.extend_from_slice(b"1001");
        rec.extend_from_slice(b"ALICE");
        rec.resize(80, b' ');
        rec.extend_from_slice(b"0000000000"); // zero-filled numeric tail
        rec.resize(100, b' ');

        let enc = compress(&rec);
        assert_eq!(decompress(&enc), rec);
        assert!(
            enc.len() * 2 <= rec.len(),
            "expected >=50% compression, got {} -> {} bytes",
            rec.len(),
            enc.len()
        );
    }

    #[test]
    fn incompressible_data_falls_back_to_raw() {
        // A pseudo-random, high-entropy block: RLE must not be chosen, so the
        // output is the raw payload plus the single tag byte.
        let data: Vec<u8> = (0..256u32).map(|i| (i.wrapping_mul(167) ^ 0x5A) as u8).collect();
        let enc = compress(&data);
        assert_eq!(decompress(&enc), data);
        assert_eq!(enc.len(), data.len() + 1, "should be raw + 1 tag byte");
        assert_eq!(enc[0], TAG_RAW);
    }

    #[test]
    fn long_run_uses_repeat_blocks() {
        let data = vec![b'Z'; 10_000];
        let enc = compress(&data);
        assert_eq!(decompress(&enc), data);
        // 10k identical bytes → ~ceil(10000/128) repeat blocks × 2 bytes + tag.
        assert!(enc.len() < 200, "expected tiny output, got {}", enc.len());
    }
}
