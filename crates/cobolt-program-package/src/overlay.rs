// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The ELF and PE carrier: the payload after the image, and the `PRLOC` trailer at the very end of
//! the file saying where it starts. Neither loader reads past the image, so the program runs as
//! before; a tool that strips the file removes the payload, and [`locate`] then says so.
//!
//! The trailer, 32 bytes, little-endian: the payload's offset (u64), its length (u64), the trailer's
//! version (u32), four reserved bytes, and `PRLOC\0\0\0`.

use crate::PackageError;

const MAGIC: [u8; 8] = *b"PRLOC\0\0\0";
const VERSION: u32 = 1;
const TRAILER_LEN: usize = 32;

pub(crate) fn embed(runner: &[u8], payload: &[u8]) -> Result<Vec<u8>, PackageError> {
    if runner.ends_with(&MAGIC) {
        return Err(PackageError::AlreadyPackaged);
    }
    let start = (runner.len() + 15) & !15;
    let mut out = Vec::with_capacity(start + payload.len() + TRAILER_LEN);
    out.extend_from_slice(runner);
    out.resize(start, 0);
    out.extend_from_slice(payload);
    out.extend_from_slice(&(start as u64).to_le_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&MAGIC);
    Ok(out)
}

pub(crate) fn locate(exe: &[u8]) -> Result<&[u8], PackageError> {
    if exe.len() < TRAILER_LEN || !exe.ends_with(&MAGIC) {
        return Err(PackageError::NoPayload);
    }
    let t = &exe[exe.len() - TRAILER_LEN..];
    let off = u64::from_le_bytes(t[0..8].try_into().unwrap());
    let len = u64::from_le_bytes(t[8..16].try_into().unwrap());
    let version = u32::from_le_bytes(t[16..20].try_into().unwrap());
    if version != VERSION {
        return Err(PackageError::Damaged(format!("PRLOC trailer version {version}")));
    }
    let end = off.checked_add(len).filter(|e| *e == (exe.len() - TRAILER_LEN) as u64);
    match end {
        Some(_) => Ok(&exe[off as usize..exe.len() - TRAILER_LEN]),
        None => Err(PackageError::Damaged("the PRLOC trailer does not point at the payload before it".into())),
    }
}
