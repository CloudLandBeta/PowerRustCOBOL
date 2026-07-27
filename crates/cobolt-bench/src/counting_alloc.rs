// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A global allocator that counts, so "memory efficiency" is a number.
//!
//! Rust has no garbage collector, so the interesting question under load is not
//! pause time — there are no pauses — but **churn**: how many allocations a
//! workload makes, how many bytes pass through the allocator, and how much is
//! live at the peak. Those three numbers localise a regression in a way wall
//! time cannot: a change that allocates half as much and runs the same speed is
//! still the change you want, because the cost shows up under concurrency and
//! on smaller machines.
//!
//! This wraps the system allocator and adds four atomics. It measures the
//! process, so a workload must be measured with [`Counters::snapshot`] around it
//! and read as a delta. Peak live bytes is process-wide by nature, so it is
//! reset per workload rather than differenced.
//!
//! Overhead is two relaxed atomic adds per allocation. That is not free, but it
//! is uniform across workloads and across the three platforms, so comparisons
//! stay honest — and the allocation *counts* it reports are exact.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_LIVE: AtomicUsize = AtomicUsize::new(0);

pub struct CountingAlloc;

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            record_alloc(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            // A realloc is one trip through the allocator, and only the growth
            // is new bytes — counting the whole new size would inflate any
            // workload that grows a Vec, which is most of them.
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            if new_size > layout.size() {
                let grew = new_size - layout.size();
                ALLOC_BYTES.fetch_add(grew as u64, Ordering::Relaxed);
                bump_live(grew);
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        new_ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            record_alloc(layout.size());
        }
        ptr
    }
}

fn record_alloc(size: usize) {
    ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
    ALLOC_BYTES.fetch_add(size as u64, Ordering::Relaxed);
    bump_live(size);
}

/// Add to the live total and raise the peak if this is a new high-water mark.
/// A compare-and-swap loop rather than a plain store: two threads growing at
/// once must not let the smaller one overwrite the larger peak.
fn bump_live(size: usize) {
    let live = LIVE_BYTES.fetch_add(size, Ordering::Relaxed) + size;
    let mut peak = PEAK_LIVE.load(Ordering::Relaxed);
    while live > peak {
        match PEAK_LIVE.compare_exchange_weak(peak, live, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(current) => peak = current,
        }
    }
}

/// A point-in-time reading of the allocator.
#[derive(Clone, Copy, Debug, Default)]
pub struct Counters {
    pub allocations: u64,
    pub bytes: u64,
    pub live: usize,
    pub peak_live: usize,
}

impl Counters {
    pub fn snapshot() -> Self {
        Counters {
            allocations: ALLOC_COUNT.load(Ordering::Relaxed),
            bytes: ALLOC_BYTES.load(Ordering::Relaxed),
            live: LIVE_BYTES.load(Ordering::Relaxed),
            peak_live: PEAK_LIVE.load(Ordering::Relaxed),
        }
    }

    /// What happened between `self` (taken first) and `later`.
    pub fn since(&self, later: Counters) -> Counters {
        Counters {
            allocations: later.allocations.saturating_sub(self.allocations),
            bytes: later.bytes.saturating_sub(self.bytes),
            // Live is a level, not a flow: a positive delta is memory the
            // workload kept, which is the number worth seeing.
            live: later.live.saturating_sub(self.live),
            peak_live: later.peak_live,
        }
    }
}

/// Drop the peak high-water mark to the current live total, so the next
/// workload's peak is its own rather than the largest seen so far.
pub fn reset_peak() {
    PEAK_LIVE.store(LIVE_BYTES.load(Ordering::Relaxed), Ordering::Relaxed);
}
