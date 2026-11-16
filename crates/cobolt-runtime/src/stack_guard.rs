// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! How much of the running thread's stack is left, so that a program nesting
//! `PERFORM` or `CALL` too deeply ends with a runtime error instead of
//! overflowing the stack, which ends the whole process with no message (and,
//! in the IDE, the IDE with it: `catch_unwind` cannot catch an overflow).
//!
//! The answer comes from the operating system's own record of the thread's
//! stack, so it holds on every thread of every host, whoever spawned it and
//! with whatever size. Where the system cannot say, there is no answer and the
//! fixed limit on `PERFORM` depth is the only guard, as before.

use std::cell::Cell;

/// The program threads' stack: enough for the deepest nesting the interpreter
/// allows (512 `PERFORM` levels) in an unoptimised build, with room to spare.
/// Only the pages a run touches are committed.
pub const PROGRAM_STACK_BYTES: usize = 64 * 1024 * 1024;

/// What a new `PERFORM` or `CALL` level must find free before it starts: one
/// level of an unoptimised build takes 30 to 50 KB, and a statement between two
/// levels may take several times that.
pub(crate) const LEVEL_RESERVE_BYTES: usize = 1024 * 1024;

thread_local! {
    /// The lowest usable address of this thread's stack, asked once.
    static LOW: Cell<Option<Option<usize>>> = const { Cell::new(None) };
}

/// Bytes left below the caller's frame, or `None` where the system does not say.
#[inline(never)]
pub(crate) fn remaining() -> Option<usize> {
    let marker = 0u8;
    let here = std::ptr::addr_of!(marker) as usize;
    let low = LOW.with(|c| match c.get() {
        Some(v) => v,
        None => {
            let v = stack_low();
            c.set(Some(v));
            v
        }
    })?;
    here.checked_sub(low)
}

/// Whether a new level would leave less than the reserve.
pub(crate) fn exhausted() -> bool {
    remaining().is_some_and(|left| left < LEVEL_RESERVE_BYTES)
}

/// Spawn a thread that runs a program, with [`PROGRAM_STACK_BYTES`] of stack.
pub fn spawn_program_thread<F, T>(name: &str, f: F) -> std::io::Result<std::thread::JoinHandle<T>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    std::thread::Builder::new().name(name.to_string()).stack_size(PROGRAM_STACK_BYTES).spawn(f)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn stack_low() -> Option<usize> {
    // SAFETY: both calls only read the calling thread's own record.
    unsafe {
        let me = libc::pthread_self();
        let top = libc::pthread_get_stackaddr_np(me) as usize;
        let size = libc::pthread_get_stacksize_np(me);
        top.checked_sub(size)
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn stack_low() -> Option<usize> {
    // SAFETY: the attribute object is initialised by `pthread_getattr_np` and
    // destroyed once read; nothing else touches it.
    unsafe {
        let mut attr: libc::pthread_attr_t = std::mem::zeroed();
        if libc::pthread_getattr_np(libc::pthread_self(), &mut attr) != 0 {
            return None;
        }
        let mut addr: *mut libc::c_void = std::ptr::null_mut();
        let mut size: libc::size_t = 0;
        let got = libc::pthread_attr_getstack(&attr, &mut addr, &mut size);
        libc::pthread_attr_destroy(&mut attr);
        (got == 0).then_some(addr as usize)
    }
}

#[cfg(windows)]
fn stack_low() -> Option<usize> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThreadStackLimits(low: *mut usize, high: *mut usize);
    }
    let (mut low, mut high) = (0usize, 0usize);
    // SAFETY: the call writes the calling thread's two limits into the two locals.
    unsafe { GetCurrentThreadStackLimits(&mut low, &mut high) };
    (low != 0).then_some(low)
}

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "linux", target_os = "android", windows)))]
fn stack_low() -> Option<usize> {
    None
}
