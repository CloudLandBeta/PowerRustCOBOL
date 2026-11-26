// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Backend as a Windows service (spec 113 R8 to R10, R12): `backend.exe --service` is the program
//! the Service Control Manager starts. It reports `START_PENDING`, then `RUNNING` only once the Backend
//! listens, answers `STOP` and `SHUTDOWN` by stopping cleanly, and ends `STOPPED` with exit code 0, or
//! with the error 1066 and the Backend's own code in the service-specific exit code when it could not
//! start (a refused configuration, a port taken), so the failure is the operator's to read.
//!
//! Every call into the system is in this file, and each `unsafe` block says what it relies on.

use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Condvar, Mutex};

use windows_sys::Win32::Foundation::{ERROR_FAILED_SERVICE_CONTROLLER_CONNECT, ERROR_SERVICE_SPECIFIC_ERROR, NO_ERROR};
use windows_sys::Win32::System::Services::{
    RegisterServiceCtrlHandlerExW, SetServiceStatus, StartServiceCtrlDispatcherW, SERVICE_ACCEPT_SHUTDOWN, SERVICE_ACCEPT_STOP, SERVICE_CONTROL_INTERROGATE, SERVICE_CONTROL_SHUTDOWN, SERVICE_CONTROL_STOP,
    SERVICE_RUNNING, SERVICE_START_PENDING, SERVICE_STATUS, SERVICE_STOPPED, SERVICE_STOP_PENDING, SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS,
};

use crate::standalone::Hooks;

/// The time the system is told a start or a stop may take before it calls the service hung.
const START_HINT_MS: u32 = 60_000;
const STOP_HINT_MS: u32 = 30_000;

/// The message `StartServiceCtrlDispatcherW` returns when the program was not started by the system.
const NOT_A_SERVICE: i32 = ERROR_FAILED_SERVICE_CONTROLLER_CONNECT as i32;

/// The status handle the system gave the service; set once by the service's main function.
static HANDLE: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
/// The work to do, handed from [`run`] to the service's main function (a function the system calls with no arguments of ours).
static JOB: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
/// A stop was asked for.
static STOP: Stop = Stop { asked: Mutex::new(false), signal: Condvar::new() };

struct Stop {
    asked: Mutex<bool>,
    signal: Condvar,
}

impl Stop {
    fn ask(&self) {
        *self.asked.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.signal.notify_all();
    }

    fn wait(&self) {
        let mut asked = self.asked.lock().unwrap_or_else(|e| e.into_inner());
        while !*asked {
            asked = self.signal.wait(asked).unwrap_or_else(|e| e.into_inner());
        }
    }
}

/// What the service's main function runs and what it leaves.
struct Job<'a> {
    name: Vec<u16>,
    body: Option<Box<dyn FnOnce(Hooks<'_>) -> i32 + 'a>>,
    code: i32,
}

fn report(state: u32, accepted: u32, exit_code: i32, checkpoint: u32, hint_ms: u32) {
    let handle = HANDLE.load(Ordering::SeqCst);
    if handle.is_null() {
        return;
    }
    let (win32, specific) = if state == SERVICE_STOPPED && exit_code != 0 { (ERROR_SERVICE_SPECIFIC_ERROR, exit_code as u32) } else { (NO_ERROR, 0) };
    let status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: accepted,
        dwWin32ExitCode: win32,
        dwServiceSpecificExitCode: specific,
        dwCheckPoint: checkpoint,
        dwWaitHint: hint_ms,
    };
    // SAFETY: `handle` came from RegisterServiceCtrlHandlerExW and stays valid until the service is STOPPED;
    // `status` is a fully initialised SERVICE_STATUS that lives through the call.
    unsafe {
        SetServiceStatus(handle, &status);
    }
}

/// The system calls this (on one of its threads) for each control it sends the service.
unsafe extern "system" fn control(code: u32, _event: u32, _data: *mut c_void, _context: *mut c_void) -> u32 {
    match code {
        SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN => {
            report(SERVICE_STOP_PENDING, 0, 0, 1, STOP_HINT_MS);
            STOP.ask();
        }
        // The system asks for the state; the last one reported stands, and nothing needs repeating.
        SERVICE_CONTROL_INTERROGATE => {}
        _ => {}
    }
    NO_ERROR
}

/// The service's main function: the system starts it on a new thread once the dispatcher has the table.
unsafe extern "system" fn service_main(_argc: u32, _argv: *mut windows_sys::core::PWSTR) {
    let job = JOB.swap(null_mut(), Ordering::SeqCst) as *mut Job<'static>;
    if job.is_null() {
        return;
    }
    // SAFETY: `run` put a pointer to a live `Job` in JOB and does not return, so does not drop it, before the
    // dispatcher returns, which is after this function returns. The lifetime is erased only for the cast.
    let job = unsafe { &mut *job };
    // SAFETY: `job.name` is a NUL-terminated UTF-16 string that lives through the call; `control` has the signature the system expects.
    let handle = unsafe { RegisterServiceCtrlHandlerExW(job.name.as_ptr(), Some(control), null()) };
    if handle.is_null() {
        job.code = 1;
        return;
    }
    HANDLE.store(handle, Ordering::SeqCst);
    report(SERVICE_START_PENDING, 0, 0, 1, START_HINT_MS);
    let hooks = Hooks {
        // RUNNING is reported when, and only when, the Backend listens.
        ready: Box::new(|| report(SERVICE_RUNNING, SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN, 0, 0, 0)),
        wait: Box::new(|| STOP.wait()),
        stopping: Box::new(|| report(SERVICE_STOP_PENDING, 0, 0, 2, STOP_HINT_MS)),
    };
    let code = match job.body.take() {
        Some(body) => body(hooks),
        None => 1,
    };
    job.code = code;
    report(SERVICE_STOPPED, 0, code, 0, 0);
}

/// Run `body` as the service called `name`; returns the code the process should exit with. `body` gets the hooks
/// that tell the system when the Backend is ready and when it stops.
pub(crate) fn run<F>(name: &str, body: F) -> i32
where
    F: FnOnce(Hooks<'_>) -> i32,
{
    let mut job = Job { name: name.encode_utf16().chain(Some(0)).collect(), body: Some(Box::new(body)), code: 0 };
    JOB.store(&mut job as *mut Job<'_> as *mut c_void, Ordering::SeqCst);
    let table = [
        SERVICE_TABLE_ENTRYW { lpServiceName: job.name.as_mut_ptr(), lpServiceProc: Some(service_main) },
        SERVICE_TABLE_ENTRYW { lpServiceName: null_mut(), lpServiceProc: None },
    ];
    // SAFETY: `table` ends with the all-null entry the system requires and lives through the call, which blocks until
    // every service of the process has stopped; `job` outlives it for the same reason.
    let started = unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) };
    if started == 0 {
        JOB.store(null_mut(), Ordering::SeqCst);
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(NOT_A_SERVICE) {
            eprintln!("--service: this program must be started by the Windows Service Control Manager, not from a console (error {NOT_A_SERVICE}); run it without --service to run it from a terminal");
        } else {
            eprintln!("--service: the Service Control Manager did not take the service: {error}");
        }
        return 2;
    }
    job.code
}
