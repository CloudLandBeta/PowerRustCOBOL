// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! No AWS server outlives its application (spec 078 R9).
//!
//! Three independent guards, so no single failure orphans a process:
//!
//! 1. **stdin EOF.** An MCP stdio server exits when its input closes, and the
//!    operating system closes the pipe whenever the application ends — a
//!    crash and a kill included. This alone covers every platform in
//!    practice.
//! 2. **Orderly shutdown.** [`super::pool::shutdown`] closes every server's
//!    input, waits two seconds, then kills. Every host calls it on its way
//!    out ([`crate::shutdown_child_processes`]).
//! 3. **An operating-system backstop**, set up here when a server starts:
//!    - **Linux:** the server is killed when the application dies
//!      (`PR_SET_PDEATHSIG`), and runs in a process group of its own so the
//!      group (`uvx` and the Python it starts) can be ended together.
//!    - **Windows:** every server joins one Job Object created with
//!      kill-on-close; the job's handle belongs to the application, and the
//!      system closes it — killing every member — when the application ends,
//!      however it ends.
//!    - **macOS** has no equivalent; guards 1 and 2 are what it relies on.
//!
//! No crate is added for any of this: the few system calls are declared here
//! by hand, the way `cobolt-forms/src/text_scale.rs` declares `RegGetValueW`.

use std::process::{Child, Command};

/// Prepare a server's command before it starts.
pub fn prepare(cmd: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Its own group: `uvx` starts Python, and ending the group ends both.
        cmd.process_group(0);
    }
    #[cfg(target_os = "linux")]
    linux::die_with_parent(cmd);
    #[cfg(not(unix))]
    let _ = cmd;
}

/// Take charge of a started server.
pub fn adopt(child: &Child) {
    #[cfg(windows)]
    windows::join_kill_on_close_job(child);
    #[cfg(not(windows))]
    let _ = child;
}

#[cfg(target_os = "linux")]
mod linux {
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    const PR_SET_PDEATHSIG: i32 = 1;
    const SIGKILL: u64 = 9;

    extern "C" {
        fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
    }

    pub fn die_with_parent(cmd: &mut Command) {
        // SAFETY: runs in the child between fork and exec, and calls only
        // `prctl`, which is async-signal-safe.
        unsafe {
            cmd.pre_exec(|| {
                prctl(PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0);
                Ok(())
            });
        }
    }
}

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use std::sync::OnceLock;

    type Handle = *mut c_void;

    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: i32 = 9;
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;

    #[repr(C)]
    #[derive(Default)]
    struct BasicLimitInformation {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct IoCounters {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    #[repr(C)]
    #[derive(Default)]
    struct ExtendedLimitInformation {
        basic: BasicLimitInformation,
        io_info: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(attributes: *mut c_void, name: *const u16) -> Handle;
        fn SetInformationJobObject(job: Handle, class: i32, info: *mut c_void, length: u32) -> i32;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
    }

    /// The handle, kept as an integer so it can live in a static.
    fn job() -> Option<usize> {
        static JOB: OnceLock<Option<usize>> = OnceLock::new();
        *JOB.get_or_init(|| {
            // SAFETY: null attributes and name create an anonymous job; the
            // info struct is the documented layout, passed with its size.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
                if job.is_null() {
                    return None;
                }
                let mut info = ExtendedLimitInformation::default();
                info.basic.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    job,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
                    (&mut info as *mut ExtendedLimitInformation).cast(),
                    std::mem::size_of::<ExtendedLimitInformation>() as u32,
                );
                (ok != 0).then_some(job as usize)
            }
        })
    }

    pub fn join_kill_on_close_job(child: &Child) {
        if let Some(job) = job() {
            // SAFETY: both handles are live: the job is never closed while the
            // application runs, and the child was just started.
            unsafe {
                AssignProcessToJobObject(job as Handle, child.as_raw_handle() as Handle);
            }
        }
    }
}
