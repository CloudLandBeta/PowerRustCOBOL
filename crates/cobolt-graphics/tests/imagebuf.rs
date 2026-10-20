// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! ImageBuffer (WASM-018 R11, R15, R20): every operation gives the same bytes on 1
//! thread and on many; the decoders refuse decompression bombs from the header and
//! survive damaged files (AC7, AC10).
#![cfg(feature = "imagebuf")]

mod common;

use cobolt_graphics::display_list::consts::BlendMode;
use cobolt_graphics::imagebuf::{decode, encode, ImageBuffer, ImageFormat, ResizeFilter};
use cobolt_graphics::par::with_threads;
use cobolt_graphics::{Limits, PixelBuffer, Rgba};
use common::fingerprint;

/// A repeatable pseudo-random picture with soft areas and hard edges.
fn photo(w: u32, h: u32) -> PixelBuffer {
    let mut b = PixelBuffer::new(w, h).unwrap();
    let mut s = 0x1234_5678_9abc_def0u64;
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let noise = (s & 0x1f) as i64;
            let r = ((x * 255 / w as i64) + noise).clamp(0, 255) as u8;
            let g = ((y * 255 / h as i64) + noise).clamp(0, 255) as u8;
            let blk = if ((x / 23) + (y / 17)) % 2 == 0 { 40 } else { 200 };
            let a = if (x + y) % 97 < 8 { 120 } else { 255 };
            b.set(x, y, Rgba::new(r, g, blk, a));
        }
    }
    b
}

fn image(w: u32, h: u32) -> ImageBuffer {
    ImageBuffer::from_pixels(photo(w, h))
}

fn same_on_one_thread_and_many(name: &str, op: impl Fn(&mut ImageBuffer) + Send + Sync) {
    let run = || {
        let mut b = image(301, 203);
        op(&mut b);
        b.pixels().as_bytes().to_vec()
    };
    let one = with_threads(1, run);
    let many = with_threads(8, run);
    assert!(one == many, "{name}: the result depends on the number of threads ({:016x} vs {:016x})", fingerprint(&one), fingerprint(&many));
}

#[test]
fn every_filter_gives_the_same_bytes_on_one_thread_and_on_many() {
    let lim = Limits::default();
    same_on_one_thread_and_many("grayscale", |b| b.grayscale());
    same_on_one_thread_and_many("invert", |b| b.invert());
    same_on_one_thread_and_many("brightness", |b| b.brightness(35.0));
    same_on_one_thread_and_many("contrast", |b| b.contrast(-40.0));
    same_on_one_thread_and_many("hue", |b| b.hue(137.0));
    same_on_one_thread_and_many("saturation", |b| b.saturation(2.5));
    same_on_one_thread_and_many("threshold", |b| b.threshold(120));
    for sigma in [1.0f32, 3.3, 6.0, 6.5, 20.0, 90.0] {
        same_on_one_thread_and_many(&format!("gaussian {sigma}"), move |b| b.blur_gaussian(sigma));
    }
    same_on_one_thread_and_many("box 4", |b| b.blur_box(4));
    same_on_one_thread_and_many("sharpen", |b| b.sharpen(1.5));
    same_on_one_thread_and_many("edges", |b| b.edge_detect());
    same_on_one_thread_and_many("convolve 3", |b| b.convolve(&[0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0], 1.0, 0.0).unwrap());
    same_on_one_thread_and_many("convolve 5", |b| b.convolve(&[1.0; 25], 25.0, 3.0).unwrap());
    same_on_one_thread_and_many("dilate", |b| b.dilate(2).unwrap());
    same_on_one_thread_and_many("erode", |b| b.erode(3).unwrap());
    same_on_one_thread_and_many("flip h", |b| b.flip(true));
    same_on_one_thread_and_many("flip v", |b| b.flip(false));
    same_on_one_thread_and_many("rotate 90", move |b| b.rotate(90.0, Rgba::TRANSPARENT, &Limits::default()).unwrap());
    same_on_one_thread_and_many("rotate 33", move |b| b.rotate(33.0, Rgba::rgb(9, 9, 9), &Limits::default()).unwrap());
    for f in [ResizeFilter::Nearest, ResizeFilter::Bilinear, ResizeFilter::CatmullRom, ResizeFilter::Lanczos] {
        same_on_one_thread_and_many(&format!("shrink {f:?}"), move |b| b.resize(113, 71, f, &Limits::default()).unwrap());
        same_on_one_thread_and_many(&format!("grow {f:?}"), move |b| b.resize(777, 533, f, &Limits::default()).unwrap());
    }
    same_on_one_thread_and_many("composite", |b| {
        let top = image(120, 80);
        b.composite(&top, 40, 30, BlendMode::Overlay, 0.7).unwrap();
    });
    let _ = lim;
}

#[test]
fn encoded_bytes_are_identical_on_one_thread_and_many() {
    // AC7: "an ImageBuffer resized, blurred and encoded gives the same bytes with one thread and with many".
    for format in ImageFormat::ALL {
        let run = || {
            let mut b = image(257, 190);
            b.resize(200, 150, ResizeFilter::Lanczos, &Limits::default()).unwrap();
            b.blur_gaussian(2.0);
            b.encode(format, 85).unwrap()
        };
        let one = with_threads(1, run);
        let many = with_threads(8, run);
        assert!(one == many, "{format:?}: {} bytes vs {} bytes", one.len(), many.len());
        // And what was written can be read back.
        let back = decode(&one, &Limits::default()).unwrap();
        assert_eq!((back.width(), back.height()), (200, 150), "{format:?}");
    }
}

// ---- hostile files ---------------------------------------------------------------

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (n, t) in table.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for b in data {
        c = table[((c ^ *b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

fn png_header(w: u32, h: u32) -> Vec<u8> {
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(b"IHDR");
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    v.extend_from_slice(&13u32.to_be_bytes());
    v.extend_from_slice(&ihdr);
    v.extend_from_slice(&crc32(&ihdr).to_be_bytes());
    // A tiny, empty IDAT and IEND so the file looks finished.
    let idat = b"IDAT\x78\x9c\x03\x00\x00\x00\x00\x01";
    v.extend_from_slice(&8u32.to_be_bytes());
    v.extend_from_slice(idat);
    v.extend_from_slice(&crc32(idat).to_be_bytes());
    let iend = b"IEND";
    v.extend_from_slice(&0u32.to_be_bytes());
    v.extend_from_slice(iend);
    v.extend_from_slice(&crc32(iend).to_be_bytes());
    v
}

#[test]
fn a_decompression_bomb_is_refused_from_its_header() {
    let lim = Limits::default();
    // 60000 x 60000 is 3.6 gigapixels in a few dozen bytes.
    let t = std::time::Instant::now();
    let e = decode(&png_header(60_000, 60_000), &lim).unwrap_err();
    assert_eq!(e.code(), "limit-exceeded", "{e}");
    assert!(e.to_string().contains("max_image_pixels"));
    assert!(t.elapsed() < std::time::Duration::from_secs(2), "refusing must not need the pixels");
    // Just over and just under the limit.
    let lim = Limits { max_image_pixels: 1_000_000, ..Limits::default() };
    assert_eq!(decode(&png_header(1001, 1000), &lim).unwrap_err().code(), "limit-exceeded");
    assert_ne!(decode(&png_header(1000, 1000), &lim).unwrap_err().code(), "limit-exceeded", "within the limit it fails later, on the missing data");
    // BMP and QOI headers claiming huge sizes.
    let mut bmp = vec![0u8; 54];
    bmp[0] = b'B';
    bmp[1] = b'M';
    bmp[10] = 54;
    bmp[14] = 40;
    bmp[18..22].copy_from_slice(&200_000i32.to_le_bytes());
    bmp[22..26].copy_from_slice(&200_000i32.to_le_bytes());
    bmp[26] = 1;
    bmp[28] = 32;
    assert!(decode(&bmp, &Limits::default()).is_err());
    let mut qoi = b"qoif".to_vec();
    qoi.extend_from_slice(&u32::MAX.to_be_bytes());
    qoi.extend_from_slice(&u32::MAX.to_be_bytes());
    qoi.extend_from_slice(&[4, 0]);
    assert!(decode(&qoi, &Limits::default()).is_err());
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

fn iters() -> u64 {
    std::env::var("GRAPHICS_FUZZ_ITERS").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(1).max(1)
}

#[test]
fn damaged_files_never_panic_a_decoder() {
    // Bounded-input fuzzing of every decoder: valid files of every format, then flipped,
    // truncated, extended and spliced. The limit keeps a lucky header from asking for much.
    let lim = Limits { max_image_pixels: 4_000_000, max_encoded_bytes: 1 << 20, ..Limits::default() };
    let src = photo(33, 21);
    let mut rng = Rng(0x00C0_FFEE_D00D_1234);
    let (mut decoded, mut refused) = (0u64, 0u64);
    for format in ImageFormat::ALL {
        let good = encode(&src, format, 80).unwrap();
        for i in 0..6000 * iters() {
            let mut bytes = good.clone();
            match i % 5 {
                0 => {
                    for _ in 0..1 + rng.below(6) {
                        let at = rng.below(bytes.len() as u64) as usize;
                        bytes[at] ^= 1 << rng.below(8);
                    }
                }
                1 => bytes.truncate(rng.below(bytes.len() as u64) as usize),
                2 => {
                    let at = rng.below(bytes.len() as u64) as usize;
                    let n = rng.below(16) as usize;
                    let junk: Vec<u8> = (0..n).map(|_| rng.next() as u8).collect();
                    bytes.splice(at..at, junk);
                }
                3 => {
                    // Overwrite a run with a constant (zeros and 0xFF find edge cases).
                    let at = rng.below(bytes.len() as u64) as usize;
                    let n = (rng.below(24) as usize).min(bytes.len() - at);
                    let v = if rng.below(2) == 0 { 0 } else { 0xFF };
                    bytes[at..at + n].fill(v);
                }
                _ => {
                    // The header region gets the most attention.
                    let at = rng.below(bytes.len().min(48) as u64) as usize;
                    bytes[at] = rng.next() as u8;
                }
            }
            match decode(&bytes, &lim) {
                Ok(img) => {
                    decoded += 1;
                    assert!(img.width() as u64 * img.height() as u64 <= 4_000_000);
                }
                Err(_) => refused += 1,
            }
        }
    }
    assert!(decoded > 1000 && refused > 1000, "decoded {decoded}, refused {refused}");
}

#[test]
fn arbitrary_bytes_with_a_valid_signature_never_panic() {
    let lim = Limits { max_image_pixels: 1_000_000, max_encoded_bytes: 1 << 16, ..Limits::default() };
    let mut rng = Rng(0xBAD_5EED_0000_0001);
    let signatures: [&[u8]; 6] = [b"\x89PNG\r\n\x1a\n", b"\xff\xd8\xff\xe0", b"GIF89a", b"RIFF\x00\x00\x00\x00WEBP", b"BM", b"qoif"];
    for _ in 0..20_000 * iters() {
        let sig = signatures[rng.below(6) as usize];
        let mut bytes = sig.to_vec();
        for _ in 0..rng.below(120) {
            bytes.push(rng.next() as u8);
        }
        let _ = decode(&bytes, &lim);
    }
}

#[test]
fn every_format_survives_a_round_trip_of_odd_sizes() {
    for (w, h) in [(1u32, 1u32), (1, 40), (40, 1), (3, 5), (17, 9), (64, 64)] {
        let src = photo(w, h);
        for f in ImageFormat::ALL {
            let bytes = encode(&src, f, 90).unwrap_or_else(|e| panic!("{f:?} {w}x{h}: {e}"));
            let back = decode(&bytes, &Limits::default()).unwrap_or_else(|e| panic!("{f:?} {w}x{h}: {e}"));
            assert_eq!((back.width(), back.height()), (w, h), "{f:?}");
        }
    }
}
