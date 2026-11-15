// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! An ad hoc code signature for a Mach-O executable, in pure Rust: the embedded signature
//! `codesign -s -` writes, so the Build needs neither Xcode nor the system `codesign`.
//!
//! A SuperBlob (big-endian, like every code-signing structure) holding:
//! * a CodeDirectory, version 0x20400: SHA-256 of every page of the file up to the signature
//!   (16 KiB on Apple silicon, 4 KiB on Intel, as `codesign` chooses), the identifier, two special slots (the Info.plist slot, empty; the requirements
//!   slot, the hash of the blob below) and the executable segment (`__TEXT`);
//! * an empty requirement set;
//! * an empty signature wrapper: ad hoc means no certificate.

use sha2::{Digest, Sha256};

const EMBEDDED_SIGNATURE: u32 = 0xfade_0cc0;
const CODE_DIRECTORY: u32 = 0xfade_0c02;
const SLOT_CODE_DIRECTORY: u32 = 0;
const SLOT_REQUIREMENTS: u32 = 2;
const SLOT_SIGNATURE: u32 = 0x1_0000;
const CS_ADHOC: u32 = 0x2;
const EXECSEG_MAIN_BINARY: u64 = 0x1;
const VERSION: u32 = 0x2_0400;
const HASH_SHA256: u8 = 2;
const CD_HEADER: usize = 88;
const SPECIAL_SLOTS: usize = 2;
/// An empty requirement set (`CSMAGIC_REQUIREMENTS`, length 12, count 0).
const REQUIREMENTS: [u8; 12] = [0xfa, 0xde, 0x0c, 0x01, 0, 0, 0, 12, 0, 0, 0, 0];
/// An empty signature wrapper (`CSMAGIC_BLOBWRAPPER`, length 8).
const WRAPPER: [u8; 8] = [0xfa, 0xde, 0x0b, 0x01, 0, 0, 0, 8];
const SUPERBLOB_HEADER: usize = 12 + 3 * 8;

fn ident(identifier: &str) -> String {
    let clean: String = identifier.chars().filter(|c| *c != '\0').collect();
    if clean.is_empty() { "program".into() } else { clean }
}

/// The page each hash covers, as a power of two: 16 KiB on Apple silicon, 4 KiB on Intel, as
/// `codesign` chooses for each.
pub(crate) fn page_shift(arm64: bool) -> u8 {
    if arm64 { 14 } else { 12 }
}

fn cd_len(code_len: usize, identifier: &str, page_shift: u8) -> usize {
    CD_HEADER + ident(identifier).len() + 1 + 32 * (SPECIAL_SLOTS + code_len.div_ceil(1 << page_shift))
}

/// The signature's length for `code_len` bytes signed: known before the header that records it is
/// written, since the header is among the bytes signed.
pub(crate) fn signature_len(code_len: usize, identifier: &str, page_shift: u8) -> usize {
    SUPERBLOB_HEADER + cd_len(code_len, identifier, page_shift) + REQUIREMENTS.len() + WRAPPER.len()
}

/// The ad hoc signature of `code` (the file up to where the signature goes), whose executable
/// segment is `exec_base..exec_base + exec_limit`, hashed in pages of `1 << page_shift` bytes.
pub(crate) fn adhoc(code: &[u8], identifier: &str, exec_base: u64, exec_limit: u64, main_binary: bool, page_shift: u8) -> Vec<u8> {
    let ident = ident(identifier);
    let page = 1usize << page_shift;
    let pages = code.len().div_ceil(page);
    let hash_offset = CD_HEADER + ident.len() + 1 + 32 * SPECIAL_SLOTS;
    let cd_len = cd_len(code.len(), &ident, page_shift);

    let mut cd = Vec::with_capacity(cd_len);
    for v in [CODE_DIRECTORY, cd_len as u32, VERSION, CS_ADHOC, hash_offset as u32, CD_HEADER as u32, SPECIAL_SLOTS as u32, pages as u32, code.len() as u32] {
        cd.extend_from_slice(&v.to_be_bytes());
    }
    cd.extend_from_slice(&[32, HASH_SHA256, 0, page_shift]); // hash size, hash type, platform, page size
    cd.extend_from_slice(&[0; 4]); // spare2
    cd.extend_from_slice(&[0; 4]); // scatter offset
    cd.extend_from_slice(&[0; 4]); // team offset
    cd.extend_from_slice(&[0; 4]); // spare3
    cd.extend_from_slice(&0u64.to_be_bytes()); // code limit 64
    cd.extend_from_slice(&exec_base.to_be_bytes());
    cd.extend_from_slice(&exec_limit.to_be_bytes());
    cd.extend_from_slice(&(if main_binary { EXECSEG_MAIN_BINARY } else { 0 }).to_be_bytes());
    debug_assert_eq!(cd.len(), CD_HEADER);
    cd.extend_from_slice(ident.as_bytes());
    cd.push(0);
    // Special slots, from the most negative: -2 the requirements, -1 the Info.plist (none).
    cd.extend_from_slice(&Sha256::digest(REQUIREMENTS));
    cd.extend_from_slice(&[0; 32]);
    for chunk in code.chunks(page) {
        cd.extend_from_slice(&Sha256::digest(chunk));
    }
    debug_assert_eq!(cd.len(), cd_len);

    let total = signature_len(code.len(), &ident, page_shift);
    let mut out = Vec::with_capacity(total);
    for v in [EMBEDDED_SIGNATURE, total as u32, 3] {
        out.extend_from_slice(&v.to_be_bytes());
    }
    let req_at = SUPERBLOB_HEADER + cd.len();
    let wrapper_at = req_at + REQUIREMENTS.len();
    for (slot, at) in [(SLOT_CODE_DIRECTORY, SUPERBLOB_HEADER), (SLOT_REQUIREMENTS, req_at), (SLOT_SIGNATURE, wrapper_at)] {
        out.extend_from_slice(&slot.to_be_bytes());
        out.extend_from_slice(&(at as u32).to_be_bytes());
    }
    out.extend_from_slice(&cd);
    out.extend_from_slice(&REQUIREMENTS);
    out.extend_from_slice(&WRAPPER);
    debug_assert_eq!(out.len(), total);
    out
}
