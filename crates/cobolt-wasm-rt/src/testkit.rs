// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Builders for the native tests: lay out a form-instance region, field
//! descriptors and operand records in a [`TestMem`], the way the generator lays
//! them out in a module.

use crate::fd::Fd;
use crate::mem::TestMem;
use crate::opd::*;
use crate::state;

pub struct Kit {
    pub mem: TestMem,
    next: u32,
    pub base: u32,
    pub ws: u32,
}

impl Kit {
    /// A 1 MiB memory with the rt workspace and a 16 KiB instance region.
    pub fn new() -> Kit {
        let mem = TestMem::new(1 << 20);
        let mut k = Kit { mem, next: 64, base: 0, ws: 0 };
        k.ws = k.alloc(state::WS_SIZE);
        k.base = k.alloc(16 * 1024);
        crate::host::take_log();
        state::reset();
        crate::rt_init(k.ws);
        k
    }

    /// Reserve `size` zeroed bytes (8-aligned).
    pub fn alloc(&mut self, size: u32) -> u32 {
        let at = (self.next + 7) & !7;
        self.next = at + size;
        at
    }

    pub fn put(&mut self, bytes: &[u8]) -> u32 {
        let at = self.alloc(bytes.len() as u32);
        self.mem.put(at, bytes);
        at
    }

    /// Write bytes into the instance region at `off`.
    pub fn poke(&mut self, off: u32, bytes: &[u8]) {
        let at = self.base + off;
        self.mem.put(at, bytes);
    }

    /// Read bytes out of the instance region.
    pub fn peek(&self, off: u32, len: u32) -> &[u8] {
        self.mem.get(self.base + off, len)
    }

    fn header(&mut self, tag: u8, nsub: u8, flags: u8, fig: u8, fd: &Fd, off: u32, len: u32, extra: usize) -> u32 {
        let at = self.alloc(HDR + extra as u32);
        let mut h = [0u8; HDR as usize];
        h[0] = tag;
        h[1] = nsub;
        h[2] = flags;
        h[3] = fig;
        h[4..16].copy_from_slice(&fd.to_bytes());
        h[16..20].copy_from_slice(&off.to_le_bytes());
        h[20..24].copy_from_slice(&len.to_le_bytes());
        self.mem.put(at, &h);
        at
    }

    /// A data item at `off` in the instance region.
    pub fn item(&mut self, off: u32, fd: Fd) -> u32 {
        self.header(T_ITEM, 0, 0, 0, &fd, off, 0, 0)
    }

    /// A subscripted item: `subs` are `(subscript operand, stride, max, odo operand)`.
    pub fn item_sub(&mut self, off: u32, fd: Fd, subs: &[(u32, u32, u32, u32)]) -> u32 {
        let at = self.header(T_ITEM, subs.len() as u8, 0, 0, &fd, off, 0, subs.len() * 16);
        for (i, s) in subs.iter().enumerate() {
            let p = at + HDR + (i as u32) * SUB_SIZE;
            let mut b = [0u8; 16];
            b[0..4].copy_from_slice(&s.0.to_le_bytes());
            b[4..8].copy_from_slice(&s.1.to_le_bytes());
            b[8..12].copy_from_slice(&s.2.to_le_bytes());
            b[12..16].copy_from_slice(&s.3.to_le_bytes());
            self.mem.put(p, &b);
        }
        at
    }

    /// A reference-modified item `item(start:len)`.
    pub fn item_refmod(&mut self, off: u32, fd: Fd, start: u32, len: u32) -> u32 {
        let at = self.header(T_ITEM, 0, OF_REFMOD, 0, &fd, off, 0, 8);
        let mut b = [0u8; 8];
        b[0..4].copy_from_slice(&start.to_le_bytes());
        b[4..8].copy_from_slice(&len.to_le_bytes());
        self.mem.put(at + HDR, &b);
        at
    }

    pub fn lit_num(&mut self, mantissa: i128, scale: u8) -> u32 {
        let mut b = [0u8; 17];
        b[..16].copy_from_slice(&mantissa.to_le_bytes());
        b[16] = scale;
        let data = self.put(&b);
        let fd = Fd { kind: crate::fd::K_NUM_DISPLAY, flags: 0, digits: 0, scale: scale as i8, len: 0, aux: 0 };
        self.header(T_LIT_NUM, 0, 0, 0, &fd, data, 0, 0)
    }

    pub fn lit_alnum(&mut self, text: &[u8]) -> u32 {
        let data = self.put(text);
        let fd = Fd::alnum(text.len() as u32);
        self.header(T_LIT_ALNUM, 0, 0, 0, &fd, data, text.len() as u32, 0)
    }

    pub fn all_lit(&mut self, text: &[u8]) -> u32 {
        let data = self.put(text);
        let fd = Fd::alnum(text.len() as u32);
        self.header(T_ALL, 0, 0, 0, &fd, data, text.len() as u32, 0)
    }

    pub fn fig(&mut self, fig: u8) -> u32 {
        let fd = Fd::alnum(1);
        self.header(T_FIG, 0, 0, fig, &fd, 0, 0, 0)
    }
}

impl Default for Kit {
    fn default() -> Self {
        Kit::new()
    }
}
