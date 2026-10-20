// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Helpers shared by the integration tests.
#![allow(dead_code)]

pub mod glb;
pub mod scenes;
pub mod synthfont;

use std::path::PathBuf;

use cobolt_graphics::PixelBuffer;

/// Where the golden images live.
pub fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden")
}

/// How two pictures may differ.
#[derive(Clone, Copy, Debug)]
pub struct Tolerance {
    /// Largest allowed difference of any channel of any pixel.
    pub max_channel: u8,
    /// Largest allowed fraction of pixels that differ by more than `loose` in any channel.
    pub max_fraction: f64,
    /// The per-channel difference that counts a pixel as "different".
    pub loose: u8,
}

impl Tolerance {
    /// Byte for byte.
    pub const EXACT: Tolerance = Tolerance { max_channel: 0, max_fraction: 0.0, loose: 0 };
}

/// Compare a picture with `tests/golden/<name>.png`, writing the file instead when
/// `GOLDEN_UPDATE=1` is set in the environment.
pub fn assert_golden(name: &str, got: &PixelBuffer, tol: Tolerance) {
    let path = golden_dir().join(format!("{name}.png"));
    if std::env::var("GOLDEN_UPDATE").map(|v| v == "1").unwrap_or(false) {
        std::fs::create_dir_all(golden_dir()).unwrap();
        write_png(&path, got);
        eprintln!("golden updated: {}", path.display());
        return;
    }
    let want = match read_png(&path) {
        Some(w) => w,
        None => panic!(
            "golden image {} is missing; run the test once with GOLDEN_UPDATE=1 to create it and look at it before committing",
            path.display()
        ),
    };
    assert_eq!((want.width(), want.height()), (got.width(), got.height()), "golden {name}: size differs");
    let (mut worst, mut loose_px) = (0u8, 0usize);
    for (a, b) in want.as_bytes().chunks_exact(4).zip(got.as_bytes().chunks_exact(4)) {
        let d = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0);
        worst = worst.max(d);
        if d > tol.loose {
            loose_px += 1;
        }
    }
    let total = (want.width() * want.height()) as usize;
    let fraction = loose_px as f64 / total as f64;
    assert!(worst <= tol.max_channel, "golden {name}: a channel differs by {worst}, the most allowed is {}", tol.max_channel);
    assert!(
        fraction <= tol.max_fraction,
        "golden {name}: {loose_px} of {total} pixels differ by more than {} ({fraction:.5} of the picture, allowed {})",
        tol.loose,
        tol.max_fraction
    );
}

pub fn write_png(path: &std::path::Path, img: &PixelBuffer) {
    let buf = image::RgbaImage::from_raw(img.width(), img.height(), img.as_bytes().to_vec()).unwrap();
    buf.save_with_format(path, image::ImageFormat::Png).unwrap();
}

pub fn read_png(path: &std::path::Path) -> Option<PixelBuffer> {
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).ok()?.to_rgba8();
    PixelBuffer::from_rgba(img.width(), img.height(), img.into_raw()).ok()
}

/// FNV-1a over the bytes, to compare pictures cheaply and print a short fingerprint.
pub fn fingerprint(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}
