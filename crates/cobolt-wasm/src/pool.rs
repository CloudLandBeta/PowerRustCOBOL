// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The constant area: literals and operand records, in the layout
//! `cobolt-wasm-rt` reads (its `fd` and `opd` modules are the definition).

use cobolt_wasm_rt::fd::Fd;
use cobolt_wasm_rt::opd::*;
use std::collections::HashMap;

/// One subscript of an operand record.
#[derive(Debug, Clone, Copy)]
pub struct Sub {
    /// Address of the operand record of the subscript expression.
    pub opd: u32,
    pub stride: u32,
    pub max: u32,
    /// Operand record of the `DEPENDING ON` item, or 0.
    pub odo: u32,
    /// `I + 1` / `J - 2`.
    pub add: i32,
}

pub struct Pool {
    /// Absolute address of the first byte.
    pub base: u32,
    pub bytes: Vec<u8>,
    dedup: HashMap<Vec<u8>, u32>,
}

impl Pool {
    pub fn new(base: u32) -> Pool {
        Pool { base, bytes: Vec::new(), dedup: HashMap::new() }
    }

    /// Store `data` (4-byte aligned, de-duplicated); returns its absolute address.
    pub fn intern(&mut self, data: &[u8]) -> u32 {
        if let Some(a) = self.dedup.get(data) {
            return *a;
        }
        while self.bytes.len() % 4 != 0 {
            self.bytes.push(0);
        }
        let at = self.base + self.bytes.len() as u32;
        self.bytes.extend_from_slice(data);
        self.dedup.insert(data.to_vec(), at);
        at
    }

    /// Reserve `n` zero bytes that are NOT de-duplicated (mutable data).
    pub fn reserve(&mut self, n: usize) -> u32 {
        while self.bytes.len() % 8 != 0 {
            self.bytes.push(0);
        }
        let at = self.base + self.bytes.len() as u32;
        self.bytes.resize(self.bytes.len() + n, 0);
        at
    }

    fn header(tag: u8, nsub: u8, flags: u8, fig: u8, fd: &Fd, off: u32, len: u32) -> Vec<u8> {
        let mut h = vec![0u8; HDR as usize];
        h[0] = tag;
        h[1] = nsub;
        h[2] = flags;
        h[3] = fig;
        h[4..16].copy_from_slice(&fd.to_bytes());
        h[16..20].copy_from_slice(&off.to_le_bytes());
        h[20..24].copy_from_slice(&len.to_le_bytes());
        h
    }

    /// A data item: `off` from the region base, optional subscripts and
    /// reference modification `(start operand, length operand or 0)`.
    pub fn opd_item(&mut self, off: u32, fd: &Fd, subs: &[Sub], refmod: Option<(u32, u32)>) -> u32 {
        self.opd_item_in(off, fd, subs, refmod, None)
    }

    /// As [`Pool::opd_item`]; with `link` the item is in a LINKAGE record whose
    /// address lives in the slot at that region offset, and `off` is relative
    /// to the record.
    pub fn opd_item_in(&mut self, off: u32, fd: &Fd, subs: &[Sub], refmod: Option<(u32, u32)>, link: Option<u32>) -> u32 {
        let flags = if refmod.is_some() { OF_REFMOD } else { 0 } | if link.is_some() { cobolt_wasm_rt::opd::OF_LINK } else { 0 };
        let mut r = Self::header(T_ITEM, subs.len() as u8, flags, 0, fd, off, link.unwrap_or(0));
        for s in subs {
            r.extend_from_slice(&s.opd.to_le_bytes());
            r.extend_from_slice(&s.stride.to_le_bytes());
            r.extend_from_slice(&s.max.to_le_bytes());
            r.extend_from_slice(&s.odo.to_le_bytes());
            r.extend_from_slice(&s.add.to_le_bytes());
        }
        if let Some((st, ln)) = refmod {
            r.extend_from_slice(&st.to_le_bytes());
            r.extend_from_slice(&ln.to_le_bytes());
        }
        self.intern(&r)
    }

    pub fn opd_lit_num(&mut self, mantissa: i128, scale: u8) -> u32 {
        self.opd_lit_num_written(mantissa, scale, 0)
    }

    /// A numeric literal that keeps the digit count it was written with
    /// (`012345`: six), which a move or a comparison with characters transfers.
    pub fn opd_lit_num_written(&mut self, mantissa: i128, scale: u8, written: u8) -> u32 {
        let mut b = [0u8; 17];
        b[..16].copy_from_slice(&mantissa.to_le_bytes());
        b[16] = scale;
        let data = self.intern(&b);
        let fd = Fd { kind: cobolt_wasm_rt::fd::K_NUM_DISPLAY, flags: 0, digits: written, scale: scale as i8, len: 0, aux: 0 };
        let r = Self::header(T_LIT_NUM, 0, 0, 0, &fd, data, 0);
        self.intern(&r)
    }

    pub fn opd_lit_alnum(&mut self, bytes: &[u8]) -> u32 {
        let data = if bytes.is_empty() { self.intern(&[0]) } else { self.intern(bytes) };
        let fd = Fd::alnum(bytes.len() as u32);
        let r = Self::header(T_LIT_ALNUM, 0, 0, 0, &fd, data, bytes.len() as u32);
        self.intern(&r)
    }

    pub fn opd_all(&mut self, bytes: &[u8]) -> u32 {
        let data = self.intern(bytes);
        let fd = Fd::alnum(bytes.len() as u32);
        let r = Self::header(T_ALL, 0, 0, 0, &fd, data, bytes.len() as u32);
        self.intern(&r)
    }

    /// The picture record an edited item's descriptor points at (`aux`).
    pub fn edit_spec(&mut self, template: &str, decimal_comma: bool, currency: char) -> u32 {
        let t = template.as_bytes();
        let mut r = Vec::with_capacity(4 + t.len());
        r.extend_from_slice(&(t.len() as u16).to_le_bytes());
        r.push(decimal_comma as u8);
        r.push(if currency.is_ascii() { currency as u8 } else { b'$' });
        r.extend_from_slice(t);
        self.intern(&r)
    }

    /// `fd` with `aux` pointing at the item's picture when it is edited.
    pub fn fd_with_spec(&mut self, fd: &Fd, template: Option<&str>, decimal_comma: bool, currency: char) -> Fd {
        use cobolt_wasm_rt::fd::{K_ALNUM_EDITED, K_NUM_EDITED};
        let mut f = *fd;
        if matches!(fd.kind, K_NUM_EDITED | K_ALNUM_EDITED) {
            if let Some(t) = template {
                f.aux = self.edit_spec(t, decimal_comma, currency);
            }
        }
        f
    }

    /// A file descriptor for the conformance file routines (`cobolt-wasm-rt`'s
    /// `file` module is the definition of the layout).
    pub fn file_desc(&mut self, name: &[u8], org: u8, optional: bool, handle_off: u32, status_opd: u32) -> u32 {
        let name_addr = if name.is_empty() { self.intern(&[0]) } else { self.intern(name) };
        let mut r = vec![0u8; 20];
        r[0..4].copy_from_slice(&name_addr.to_le_bytes());
        r[4..8].copy_from_slice(&(name.len() as u32).to_le_bytes());
        r[8] = org;
        r[9] = u8::from(optional);
        r[12..16].copy_from_slice(&handle_off.to_le_bytes());
        r[16..20].copy_from_slice(&status_opd.to_le_bytes());
        self.intern(&r)
    }

    /// A value saved in temporary `k` by `rt_save_temp`.
    pub fn opd_temp(&mut self, k: u32) -> u32 {
        let r = Self::header(cobolt_wasm_rt::opd::T_TEMP, 0, 0, 0, &Fd::alnum(0), k, 0);
        self.intern(&r)
    }

    /// The bytes a character-valued function left in character temporary `k`.
    pub fn opd_ctemp(&mut self, k: u32) -> u32 {
        let r = Self::header(cobolt_wasm_rt::opd::T_CTEMP, 0, 0, 0, &Fd::alnum(0), k, 0);
        self.intern(&r)
    }

    /// Spec records (INSPECT, STRING, UNSTRING, INITIALIZE …): a list of `u32`s.
    pub fn spec(&mut self, words: &[u32]) -> u32 {
        let mut b = Vec::with_capacity(words.len() * 4);
        for w in words {
            b.extend_from_slice(&w.to_le_bytes());
        }
        self.intern(&b)
    }

    pub fn opd_fig(&mut self, fig: u8) -> u32 {
        let r = Self::header(T_FIG, 0, 0, fig, &Fd::alnum(1), 0, 0);
        self.intern(&r)
    }
}
