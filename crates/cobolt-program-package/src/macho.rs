// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Mach-O carrier: a `__PRPAYLOAD` segment, read-only, inserted where `__LINKEDIT` began, and
//! the file signed again ad hoc ([`crate::codesign`]).
//!
//! An overlay after the image would sit outside the code signature, and macOS refuses such a file
//! under strict validation; inside a segment the payload is covered by the signature like the rest.
//!
//! The steps, on a thin 64-bit executable:
//! 1. one `LC_SEGMENT_64` command (72 bytes, no sections) goes in before `__LINKEDIT`'s, in the
//!    header's free room between the load commands and the first section (the runner is linked
//!    with `-Wl,-headerpad,0x1000` to have it);
//! 2. the payload takes `__LINKEDIT`'s old place, padded to a 16 KiB page, and `__LINKEDIT` moves up
//!    by that much in the file and in memory, with every file offset that points into it
//!    (`LC_SYMTAB`, `LC_DYSYMTAB`, `LC_DYLD_INFO`, the `linkedit_data_command`s, `LC_NOTE`);
//! 3. the old signature is dropped and a new one written at the end of `__LINKEDIT`.

use crate::{codesign, PackageError};

const MH_MAGIC_64: u32 = 0xfeed_facf;
const MH_EXECUTE: u32 = 2;
const CPU_TYPE_ARM64: u32 = 0x0100_000c;
const HEADER: usize = 32;
const LC_SEGMENT_64: u32 = 0x19;
const LC_SYMTAB: u32 = 0x02;
const LC_DYSYMTAB: u32 = 0x0b;
const LC_DYLD_INFO: u32 = 0x22;
const LC_DYLD_INFO_ONLY: u32 = 0x8000_0022;
const LC_CODE_SIGNATURE: u32 = 0x1d;
const LC_NOTE: u32 = 0x31;
/// The `linkedit_data_command`s: code signature, split info, function starts, data in code, code
/// signing DRs, optimisation hints, atom info, exports trie, chained fixups.
const LINKEDIT_DATA: [u32; 9] = [0x1d, 0x1e, 0x26, 0x29, 0x2b, 0x2e, 0x36, 0x8000_0033, 0x8000_0034];
const SEGMENT_LEN: usize = 72;
const SECTION_LEN: usize = 80;
const SIGNATURE_CMD_LEN: usize = 16;
/// The page segments are aligned to: Apple silicon's 16 KiB, a multiple of Intel's 4 KiB.
const PAGE: usize = 0x4000;
const VM_PROT_READ: u32 = 1;

pub(crate) const SEGNAME: [u8; 16] = *b"__PRPAYLOAD\0\0\0\0\0";
const LINKEDIT: [u8; 16] = *b"__LINKEDIT\0\0\0\0\0\0";
const TEXT: [u8; 16] = *b"__TEXT\0\0\0\0\0\0\0\0\0\0";

fn damaged(why: &str) -> PackageError {
    PackageError::Damaged(format!("Mach-O: {why}"))
}

fn rd32(b: &[u8], at: usize) -> Result<u32, PackageError> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap())).ok_or_else(|| damaged("a load command runs past the file"))
}

fn rd64(b: &[u8], at: usize) -> Result<u64, PackageError> {
    b.get(at..at + 8).map(|s| u64::from_le_bytes(s.try_into().unwrap())).ok_or_else(|| damaged("a load command runs past the file"))
}

fn wr32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn wr64(b: &mut [u8], at: usize, v: u64) {
    b[at..at + 8].copy_from_slice(&v.to_le_bytes());
}

fn align(n: usize, to: usize) -> usize {
    n.div_ceil(to) * to
}

#[derive(Clone, Copy)]
struct Cmd {
    at: usize,
    cmd: u32,
    size: usize,
}

struct Layout {
    filetype: u32,
    ncmds: u32,
    sizeofcmds: usize,
    cmds: Vec<Cmd>,
}

fn layout(b: &[u8]) -> Result<Layout, PackageError> {
    if rd32(b, 0)? != MH_MAGIC_64 {
        return Err(PackageError::UnsupportedFormat("not a 64-bit little-endian Mach-O".into()));
    }
    let filetype = rd32(b, 12)?;
    let ncmds = rd32(b, 16)?;
    let sizeofcmds = rd32(b, 20)? as usize;
    let end = HEADER.checked_add(sizeofcmds).filter(|e| *e <= b.len()).ok_or_else(|| damaged("the load commands run past the file"))?;
    let mut cmds = Vec::with_capacity(ncmds as usize);
    let mut at = HEADER;
    for _ in 0..ncmds {
        let cmd = rd32(b, at)?;
        let size = rd32(b, at + 4)? as usize;
        if size < 8 || !size.is_multiple_of(8) || at + size > end {
            return Err(damaged("a load command has a bad size"));
        }
        cmds.push(Cmd { at, cmd, size });
        at += size;
    }
    Ok(Layout { filetype, ncmds, sizeofcmds, cmds })
}

fn segname(b: &[u8], c: Cmd) -> &[u8] {
    &b[c.at + 8..c.at + 24]
}

/// The payload inside a packaged Mach-O: its `__PRPAYLOAD` segment, cut to the payload's own length.
pub(crate) fn locate(exe: &[u8]) -> Result<&[u8], PackageError> {
    let l = layout(exe)?;
    let seg = l.cmds.iter().copied().find(|c| c.cmd == LC_SEGMENT_64 && c.size >= SEGMENT_LEN && segname(exe, *c) == SEGNAME);
    let Some(c) = seg else { return Err(PackageError::NoPayload) };
    let off = rd64(exe, c.at + 40)?;
    let size = rd64(exe, c.at + 48)?;
    let end = off.checked_add(size).filter(|e| *e <= exe.len() as u64).ok_or_else(|| damaged("__PRPAYLOAD runs past the file"))?;
    let seg = &exe[off as usize..end as usize];
    let total = seg.get(16..24).map(|s| u64::from_le_bytes(s.try_into().unwrap())).ok_or_else(|| damaged("__PRPAYLOAD is too short"))?;
    if total > seg.len() as u64 {
        return Err(damaged("the payload is longer than __PRPAYLOAD"));
    }
    Ok(&seg[..total as usize])
}

/// Move every file offset in a load command that points at or past `from` up by `by`.
fn shift(cmd: &mut [u8], kind: u32, from: usize, by: usize) -> Result<(), PackageError> {
    let fields: &[usize] = match kind {
        LC_SYMTAB => &[8, 16],
        LC_DYSYMTAB => &[32, 40, 48, 56, 64, 72],
        LC_DYLD_INFO | LC_DYLD_INFO_ONLY => &[8, 16, 24, 32, 40],
        k if LINKEDIT_DATA.contains(&k) => &[8],
        _ => &[],
    };
    for &f in fields {
        let v = rd32(cmd, f)? as usize;
        if v != 0 && v >= from {
            let moved = u32::try_from(v + by).map_err(|_| PackageError::TooLarge)?;
            wr32(cmd, f, moved);
        }
    }
    if kind == LC_NOTE {
        let v = rd64(cmd, 24)? as usize;
        if v != 0 && v >= from {
            wr64(cmd, 24, (v + by) as u64);
        }
    }
    Ok(())
}

pub(crate) fn embed(runner: &[u8], payload: &[u8], identifier: &str) -> Result<Vec<u8>, PackageError> {
    let l = layout(runner)?;
    if l.filetype != MH_EXECUTE {
        return Err(PackageError::UnsupportedFormat("a Mach-O that is not an executable".into()));
    }
    let mut linkedit = None;
    let mut text = None;
    let mut signature = None;
    let mut first_section = usize::MAX;
    for c in l.cmds.iter().copied() {
        match c.cmd {
            LC_SEGMENT_64 => {
                if c.size < SEGMENT_LEN {
                    return Err(damaged("a segment command is too short"));
                }
                let name = segname(runner, c);
                if name == SEGNAME {
                    return Err(PackageError::AlreadyPackaged);
                }
                if name == LINKEDIT {
                    linkedit = Some(c);
                }
                if name == TEXT {
                    text = Some((rd64(runner, c.at + 40)?, rd64(runner, c.at + 48)?));
                }
                let nsects = rd32(runner, c.at + 64)? as usize;
                if SEGMENT_LEN + nsects * SECTION_LEN > c.size {
                    return Err(damaged("a segment lists more sections than it holds"));
                }
                for s in 0..nsects {
                    let off = rd32(runner, c.at + SEGMENT_LEN + s * SECTION_LEN + 48)? as usize;
                    if off != 0 {
                        first_section = first_section.min(off);
                    }
                }
            }
            LC_CODE_SIGNATURE => signature = Some((rd32(runner, c.at + 8)? as usize, rd32(runner, c.at + 12)? as usize)),
            _ => {}
        }
    }
    let linkedit = linkedit.ok_or_else(|| damaged("no __LINKEDIT segment"))?;
    let (text_off, text_size) = text.ok_or_else(|| damaged("no __TEXT segment"))?;
    let le_vm = rd64(runner, linkedit.at + 24)?;
    let le_off = rd64(runner, linkedit.at + 40)? as usize;
    let le_size = rd64(runner, linkedit.at + 48)? as usize;
    if le_off.checked_add(le_size) != Some(runner.len()) {
        return Err(damaged("__LINKEDIT does not end the file"));
    }
    // What of __LINKEDIT is kept: everything before the old signature, which is the file's tail.
    let keep_end = match signature {
        Some((off, size)) if off >= le_off && off.checked_add(size).is_some_and(|e| e <= runner.len()) => off,
        Some(_) => return Err(damaged("the code signature lies outside __LINKEDIT")),
        None => runner.len(),
    };

    // 1. Room for the new command(s) in the header.
    let needed = SEGMENT_LEN + if signature.is_none() { SIGNATURE_CMD_LEN } else { 0 };
    let cmds_end = HEADER + l.sizeofcmds;
    let free = first_section.min(le_off).saturating_sub(cmds_end);
    if free < needed {
        return Err(PackageError::NoHeaderRoom { free, needed });
    }
    if runner[cmds_end..cmds_end + needed].iter().any(|b| *b != 0) {
        return Err(damaged("the header's free room is not empty"));
    }

    // 2. The new load commands, offsets into __LINKEDIT moved past the payload.
    let span = align(payload.len().max(1), PAGE);
    let new_le_off = le_off + span;
    let mut cmds = Vec::with_capacity(l.sizeofcmds + needed);
    let mut le_at = 0;
    let mut sig_at = None;
    for c in l.cmds.iter().copied() {
        let mut bytes = runner[c.at..c.at + c.size].to_vec();
        shift(&mut bytes, c.cmd, le_off, span)?;
        if c.at == linkedit.at {
            let mut seg = [0u8; SEGMENT_LEN];
            wr32(&mut seg, 0, LC_SEGMENT_64);
            wr32(&mut seg, 4, SEGMENT_LEN as u32);
            seg[8..24].copy_from_slice(&SEGNAME);
            wr64(&mut seg, 24, le_vm);
            wr64(&mut seg, 32, span as u64);
            wr64(&mut seg, 40, le_off as u64);
            wr64(&mut seg, 48, span as u64);
            wr32(&mut seg, 56, VM_PROT_READ);
            wr32(&mut seg, 60, VM_PROT_READ);
            cmds.extend_from_slice(&seg);
            le_at = HEADER + cmds.len();
            wr64(&mut bytes, 24, le_vm + span as u64);
            wr64(&mut bytes, 40, new_le_off as u64);
        }
        if c.cmd == LC_CODE_SIGNATURE {
            sig_at = Some(HEADER + cmds.len());
        }
        cmds.extend_from_slice(&bytes);
    }
    let sig_at = match sig_at {
        Some(at) => at,
        None => {
            let at = HEADER + cmds.len();
            let mut sig = [0u8; SIGNATURE_CMD_LEN];
            wr32(&mut sig, 0, LC_CODE_SIGNATURE);
            wr32(&mut sig, 4, SIGNATURE_CMD_LEN as u32);
            cmds.extend_from_slice(&sig);
            at
        }
    };
    let added = if signature.is_none() { 2 } else { 1 };

    // 3. The file: header and everything before __LINKEDIT, the payload, __LINKEDIT without its
    //    old signature, then the new signature at a 16-byte boundary.
    let mut out = Vec::with_capacity(runner.len() + span + 4096);
    out.extend_from_slice(&runner[..le_off]);
    out[HEADER..HEADER + cmds.len()].copy_from_slice(&cmds);
    wr32(&mut out, 16, l.ncmds + added);
    wr32(&mut out, 20, cmds.len() as u32);
    out.extend_from_slice(payload);
    out.resize(new_le_off, 0);
    out.extend_from_slice(&runner[le_off..keep_end]);
    let sig_off = align(out.len(), 16);
    out.resize(sig_off, 0);
    let page_shift = codesign::page_shift(rd32(runner, 4)? == CPU_TYPE_ARM64);
    // The space reserved for the signature is rounded up to 16 bytes, as `codesign` reserves it.
    let sig_len = codesign::signature_len(sig_off, identifier, page_shift);
    let sig_space = align(sig_len, 16);
    if u32::try_from(sig_off + sig_space).is_err() {
        return Err(PackageError::TooLarge);
    }
    let le_filesize = sig_off + sig_space - new_le_off;
    wr64(&mut out, le_at + 48, le_filesize as u64);
    wr64(&mut out, le_at + 32, align(le_filesize, PAGE) as u64);
    wr32(&mut out, sig_at + 8, sig_off as u32);
    wr32(&mut out, sig_at + 12, sig_space as u32);
    let sig = codesign::adhoc(&out, identifier, text_off, text_size, true, page_shift);
    debug_assert_eq!(sig.len(), sig_len);
    out.extend_from_slice(&sig);
    out.resize(sig_off + sig_space, 0);
    Ok(out)
}
