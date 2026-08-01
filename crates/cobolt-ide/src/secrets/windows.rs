// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Windows Credential Manager backend — hand-written FFI to `advapi32.dll`
//! (`CredWriteW`/`CredReadW`/`CredFree`/`CredDeleteW`), zero external crate
//! dependencies. Backed by DPAPI, tied to the signed-in Windows account —
//! the same store Windows' own "Credential Manager" Control Panel applet
//! shows.
//!
//! Unlike Keychain's `(service, account)` pair, a Windows credential has a
//! single flat `TargetName` string — `"<SERVICE>/<account>"` here, e.g.
//! `"PowerRustCOBOL/google-maps"`.
//!
//! This module cannot be exercised on the macOS machine this was written
//! and reviewed on (no Windows target has a linkable `advapi32.lib` here);
//! every struct field and function signature below was checked against the
//! current Microsoft Learn `wincred.h` documentation, and `cargo check
//! --target x86_64-pc-windows-gnu` was used to at least type-check it.
//! **Wants real Windows verification before being trusted in production.**

use super::StoreOutcome;
use std::ffi::c_void;
use std::os::raw::c_ushort;
use std::ptr;

type Bool = i32;
type Dword = u32;
type Lpwstr = *mut u16;
type Lpcwstr = *const u16;
type Lpbyte = *mut u8;
type Pvoid = *mut c_void;

const CRED_TYPE_GENERIC: Dword = 1;
const CRED_PERSIST_LOCAL_MACHINE: Dword = 2;

#[repr(C)]
struct FileTime {
    dw_low_date_time: u32,
    dw_high_date_time: u32,
}

/// Matches `CREDENTIALW` (`wincred.h`) field-for-field and in order — this
/// is a `#[repr(C)]` FFI boundary type, so layout must match exactly.
#[repr(C)]
struct CredentialW {
    flags: Dword,
    r#type: Dword,
    target_name: Lpwstr,
    comment: Lpwstr,
    last_written: FileTime,
    credential_blob_size: Dword,
    credential_blob: Lpbyte,
    persist: Dword,
    attribute_count: Dword,
    attributes: Pvoid,
    target_alias: Lpwstr,
    user_name: Lpwstr,
}

#[link(name = "advapi32")]
extern "system" {
    fn CredWriteW(credential: *mut CredentialW, flags: Dword) -> Bool;
    fn CredReadW(
        target_name: Lpcwstr,
        r#type: Dword,
        flags: Dword,
        credential: *mut *mut CredentialW,
    ) -> Bool;
    fn CredFree(buffer: Pvoid);
    fn CredDeleteW(target_name: Lpcwstr, r#type: Dword, flags: Dword) -> Bool;
    fn GetLastError() -> Dword;
}

/// UTF-16, NUL-terminated — every `LPCWSTR`/`LPWSTR` field Windows reads as
/// a C wide string needs this, `CredentialBlob` (an opaque byte blob, not a
/// wide string) does not.
fn wide(s: &str) -> Vec<c_ushort> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn target_name(account: &str) -> String {
    format!("{}/{account}", super::SERVICE)
}

pub fn store(account: &str, secret: &str) -> StoreOutcome {
    let mut target = wide(&target_name(account));
    let mut blob = secret.as_bytes().to_vec();

    let mut cred = CredentialW {
        flags: 0,
        r#type: CRED_TYPE_GENERIC,
        target_name: target.as_mut_ptr(),
        comment: ptr::null_mut(),
        last_written: FileTime {
            dw_low_date_time: 0,
            dw_high_date_time: 0,
        },
        credential_blob_size: blob.len() as Dword,
        credential_blob: blob.as_mut_ptr(),
        persist: CRED_PERSIST_LOCAL_MACHINE,
        attribute_count: 0,
        attributes: ptr::null_mut(),
        target_alias: ptr::null_mut(),
        user_name: ptr::null_mut(),
    };

    let ok = unsafe { CredWriteW(&mut cred, 0) };
    if ok != 0 {
        StoreOutcome::Stored
    } else {
        let err = unsafe { GetLastError() };
        StoreOutcome::Failed(format!("CredWriteW failed (GetLastError {err})"))
    }
}

pub fn retrieve(account: &str) -> Option<String> {
    let target = wide(&target_name(account));
    let mut out: *mut CredentialW = ptr::null_mut();
    let ok = unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut out) };
    if ok == 0 || out.is_null() {
        return None; // covers "not found" and every other read failure alike.
    }
    // SAFETY: CredReadW succeeded and populated `out`; it stays valid until
    // the matching CredFree below.
    let (ptr, len) = unsafe { ((*out).credential_blob, (*out).credential_blob_size as usize) };
    let value = if ptr.is_null() || len == 0 {
        None
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
        String::from_utf8(bytes.to_vec()).ok()
    };
    unsafe { CredFree(out as Pvoid) };
    value
}

pub fn delete(account: &str) {
    let target = wide(&target_name(account));
    // A missing credential (ERROR_NOT_FOUND) is a normal outcome here —
    // nothing further this best-effort caller can or needs to do either way.
    unsafe {
        CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0);
    }
}
