// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! Who and where this process is, in words, for the install log.
//!
//! Whether a custom action runs as `SYSTEM` or as the installing user, whether
//! it is elevated and which session it is in decide how the toolchain installs
//! and the compile behave. The spike of spec 112 reads them out of a real
//! installer run; the supervisor writes the same line into every step's log so a
//! support request carries it.

use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, GetCurrentThread, OpenProcessToken, OpenThreadToken,
};
use windows_sys::Win32::System::WindowsProgramming::GetUserNameW;

/// The account this thread acts as. While a thread impersonates, this is the impersonated account, not the process's.
pub fn user_name() -> String {
    let mut buf = [0u16; 257];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` and `len` are valid for the call and `len` is its capacity in characters.
    let ok = unsafe { GetUserNameW(buf.as_mut_ptr(), &mut len) };
    if ok == 0 {
        return "?".to_owned();
    }
    String::from_utf16_lossy(&buf[..(len as usize).saturating_sub(1)])
}

/// The Windows session this process is in. Session 0 has no desktop, so a step there can show no window.
pub fn session_id() -> Option<u32> {
    let mut session = 0u32;
    // SAFETY: plain out-parameter.
    let ok = unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) };
    (ok != 0).then_some(session)
}

/// Whether the process's own token is elevated (an administrator's full token).
pub fn is_elevated() -> Option<bool> {
    let mut token: HANDLE = null_mut();
    // SAFETY: the token handle is closed below; the out-parameters are valid for the calls.
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return None;
        }
        let mut info = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut returned = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            (&mut info as *mut TOKEN_ELEVATION).cast(),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        );
        CloseHandle(token);
        (ok != 0).then_some(info.TokenIsElevated != 0)
    }
}

/// Whether this thread is impersonating (it has a token of its own beside the process's).
pub fn is_impersonating() -> bool {
    let mut token: HANDLE = null_mut();
    // SAFETY: the handle is closed when the call succeeded.
    unsafe {
        let ok = OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut token);
        if ok != 0 {
            CloseHandle(token);
        }
        ok != 0
    }
}

/// One line for a log: `user=… session=… elevated=… impersonating=…`.
pub fn describe() -> String {
    let show = |v: Option<String>| v.unwrap_or_else(|| "?".to_owned());
    format!(
        "user={} session={} elevated={} impersonating={}",
        user_name(),
        show(session_id().map(|s| s.to_string())),
        show(is_elevated().map(|e| e.to_string())),
        is_impersonating(),
    )
}
