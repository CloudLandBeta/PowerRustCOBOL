// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Linear memory, seen as a flat array of bytes addressed by `u32`.
//!
//! On `wasm32` an address *is* the pointer. In the native unit tests there is a
//! [`TestMem`] whose buffer stands in for memory 0, so the code under test is
//! byte-for-byte the code that is linked into a generated module.
//!
//! Everything here is `unsafe` inside and safe outside **by contract**: the
//! generator hands the routines only addresses it laid out itself, and a
//! routine that computes an address checks it against the item's declared
//! length first. Overlapping ranges are handled with `ptr::copy`; no function
//! here ever holds two slices over the same bytes.

#[cfg(not(target_arch = "wasm32"))]
std::thread_local! {
    static MEM_BASE: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
    static MEM_LEN: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

#[inline(always)]
fn ptr(addr: u32) -> *mut u8 {
    #[cfg(target_arch = "wasm32")]
    {
        addr as usize as *mut u8
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base = MEM_BASE.with(|b| b.get());
        let len = MEM_LEN.with(|l| l.get());
        assert!(base != 0, "no TestMem is live on this thread");
        assert!((addr as usize) < len, "address {addr} outside the test memory ({len} bytes)");
        (base + addr as usize) as *mut u8
    }
}

/// A read-only view of `len` bytes at `addr`.
///
/// The slice must not be kept across a call that writes the same bytes.
#[inline]
pub fn bytes(addr: u32, len: u32) -> &'static [u8] {
    if len == 0 {
        return &[];
    }
    #[cfg(not(target_arch = "wasm32"))]
    assert!(addr as usize + len as usize <= MEM_LEN.with(|l| l.get()), "range {addr}+{len} outside the test memory");
    // SAFETY: see the module contract; the range was laid out by the generator.
    unsafe { core::slice::from_raw_parts(ptr(addr), len as usize) }
}

/// A writable view of `len` bytes at `addr`.
#[inline]
pub fn bytes_mut(addr: u32, len: u32) -> &'static mut [u8] {
    if len == 0 {
        return &mut [];
    }
    #[cfg(not(target_arch = "wasm32"))]
    assert!(addr as usize + len as usize <= MEM_LEN.with(|l| l.get()), "range {addr}+{len} outside the test memory");
    // SAFETY: as for `bytes`.
    unsafe { core::slice::from_raw_parts_mut(ptr(addr), len as usize) }
}

/// `memmove`: copy `len` bytes from `src` to `dst`, ranges may overlap.
#[inline]
pub fn copy(src: u32, dst: u32, len: u32) {
    if len == 0 || src == dst {
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let n = MEM_LEN.with(|l| l.get());
        assert!(src as usize + len as usize <= n && dst as usize + len as usize <= n, "copy outside the test memory");
    }
    // SAFETY: both ranges are in memory; `copy` allows overlap.
    unsafe { core::ptr::copy(ptr(src), ptr(dst), len as usize) }
}

/// Fill `len` bytes at `dst` with `b`.
#[inline]
pub fn fill(dst: u32, len: u32, b: u8) {
    if len == 0 {
        return;
    }
    bytes_mut(dst, len).fill(b);
}

#[inline]
pub fn rd_u8(addr: u32) -> u8 {
    bytes(addr, 1)[0]
}

#[inline]
pub fn rd_u16(addr: u32) -> u16 {
    let b = bytes(addr, 2);
    u16::from_le_bytes([b[0], b[1]])
}

#[inline]
pub fn rd_u32(addr: u32) -> u32 {
    let b = bytes(addr, 4);
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

#[inline]
pub fn rd_i128(addr: u32) -> i128 {
    let b = bytes(addr, 16);
    let mut a = [0u8; 16];
    a.copy_from_slice(b);
    i128::from_le_bytes(a)
}

#[inline]
pub fn wr_u8(addr: u32, v: u8) {
    bytes_mut(addr, 1)[0] = v;
}

#[inline]
pub fn wr_u32(addr: u32, v: u32) {
    bytes_mut(addr, 4).copy_from_slice(&v.to_le_bytes());
}

/// A fixed-size linear memory for the native tests.
///
/// Creating one makes it *the* memory of the current thread; dropping it
/// detaches it. Address 0 is valid, as on wasm32; tests place their data from
/// 64 up.
#[cfg(not(target_arch = "wasm32"))]
pub struct TestMem {
    buf: std::vec::Vec<u8>,
}

#[cfg(not(target_arch = "wasm32"))]
impl TestMem {
    pub fn new(size: usize) -> TestMem {
        let mut buf = std::vec![0u8; size];
        MEM_BASE.with(|b| b.set(buf.as_mut_ptr() as usize));
        MEM_LEN.with(|l| l.set(size));
        TestMem { buf }
    }

    /// Write `data` at `addr`.
    pub fn put(&mut self, addr: u32, data: &[u8]) {
        self.buf[addr as usize..addr as usize + data.len()].copy_from_slice(data);
    }

    /// Read `len` bytes at `addr`.
    pub fn get(&self, addr: u32, len: u32) -> &[u8] {
        &self.buf[addr as usize..(addr + len) as usize]
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for TestMem {
    fn drop(&mut self) {
        MEM_BASE.with(|b| b.set(0));
        MEM_LEN.with(|l| l.set(0));
    }
}
