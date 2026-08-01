// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! macOS Keychain backend — hand-written FFI to `Security.framework` and
//! `CoreFoundation.framework`, zero external crate dependencies (the
//! `#[link(kind = "framework")]` attributes below tell the linker to pull
//! them in directly — no `build.rs` needed).
//!
//! Stores each secret as a **generic password** Keychain item
//! (`kSecClassGenericPassword`) keyed by `(kSecAttrService, kSecAttrAccount)`
//! — `SERVICE` and the caller's `account` respectively — which is exactly
//! the shape `security(1)`/every other desktop credential helper uses.
//!
//! # Safety and memory management
//!
//! Core Foundation's "Create Rule" applies throughout: anything returned by
//! a function whose name contains `Create` or `Copy` is owned by the caller
//! and must be released exactly once. [`CfOwned`] is a tiny RAII guard that
//! calls `CFRelease` on drop, so every `Cf*Create*` result here is wrapped
//! immediately and released automatically — including on early `return`.

use super::StoreOutcome;
use std::ffi::c_void;
use std::ptr;

// ── Core Foundation / Security FFI surface ──────────────────────────────────
//
// These types are opaque pointers in the real C headers (`typedef const
// struct __CFString *CFStringRef`, …) — representing them as `*const
// c_void` is exactly what the C `typedef` already boils down to.
type CFAllocatorRef = *const c_void;
type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CFDataRef = *const c_void;
type CFDictionaryRef = *const c_void;
type CFBooleanRef = *const c_void;
type CFIndex = isize;
type OSStatus = i32;
type Boolean = u8;
type CFStringEncoding = u32;

const K_CF_STRING_ENCODING_UTF8: CFStringEncoding = 0x0800_0100;

const ERR_SEC_SUCCESS: OSStatus = 0;
const ERR_SEC_ITEM_NOT_FOUND: OSStatus = -25300;
const ERR_SEC_DUPLICATE_ITEM: OSStatus = -25299;

/// `kCFTypeDictionaryKeyCallBacks`/`...ValueCallBacks` are **struct values**
/// at their symbol (not pointers stored there) — this placeholder type only
/// exists so `&STATIC` yields the correct address to hand to
/// `CFDictionaryCreate`; its fields are never read, so its layout does not
/// need to match the real (much larger) `CFDictionaryKeyCallBacks` struct.
#[repr(C)]
struct OpaqueCallBacks {
    _unused: u8,
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithBytes(
        alloc: CFAllocatorRef,
        bytes: *const u8,
        num_bytes: CFIndex,
        encoding: CFStringEncoding,
        is_external_representation: Boolean,
    ) -> CFStringRef;
    fn CFDataCreate(allocator: CFAllocatorRef, bytes: *const u8, length: CFIndex) -> CFDataRef;
    fn CFDataGetLength(the_data: CFDataRef) -> CFIndex;
    fn CFDataGetBytePtr(the_data: CFDataRef) -> *const u8;
    fn CFDictionaryCreate(
        allocator: CFAllocatorRef,
        keys: *const *const c_void,
        values: *const *const c_void,
        num_values: CFIndex,
        key_call_backs: *const OpaqueCallBacks,
        value_call_backs: *const OpaqueCallBacks,
    ) -> CFDictionaryRef;
    fn CFRelease(cf: CFTypeRef);

    static kCFTypeDictionaryKeyCallBacks: OpaqueCallBacks;
    static kCFTypeDictionaryValueCallBacks: OpaqueCallBacks;
    static kCFBooleanTrue: CFBooleanRef;
}

#[link(name = "Security", kind = "framework")]
extern "C" {
    fn SecItemAdd(attributes: CFDictionaryRef, result: *mut CFTypeRef) -> OSStatus;
    fn SecItemCopyMatching(query: CFDictionaryRef, result: *mut CFTypeRef) -> OSStatus;
    fn SecItemUpdate(query: CFDictionaryRef, attributes_to_update: CFDictionaryRef) -> OSStatus;
    fn SecItemDelete(query: CFDictionaryRef) -> OSStatus;

    static kSecClass: CFStringRef;
    static kSecClassGenericPassword: CFStringRef;
    static kSecAttrService: CFStringRef;
    static kSecAttrAccount: CFStringRef;
    static kSecValueData: CFStringRef;
    static kSecReturnData: CFStringRef;
    static kSecMatchLimit: CFStringRef;
    static kSecMatchLimitOne: CFStringRef;
}

/// Owns one Core Foundation object and releases it on drop (the "Create
/// Rule" — see module docs). `as_ref()` lends the raw ref without
/// transferring ownership, for passing into another CF call.
struct CfOwned(CFTypeRef);

impl CfOwned {
    fn as_ref(&self) -> CFTypeRef {
        self.0
    }
}

impl Drop for CfOwned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

fn cf_string(s: &str) -> Option<CfOwned> {
    let bytes = s.as_bytes();
    let cf = unsafe {
        CFStringCreateWithBytes(
            ptr::null(),
            bytes.as_ptr(),
            bytes.len() as CFIndex,
            K_CF_STRING_ENCODING_UTF8,
            0,
        )
    };
    if cf.is_null() {
        None
    } else {
        Some(CfOwned(cf))
    }
}

fn cf_data(bytes: &[u8]) -> Option<CfOwned> {
    let cf = unsafe { CFDataCreate(ptr::null(), bytes.as_ptr(), bytes.len() as CFIndex) };
    if cf.is_null() {
        None
    } else {
        Some(CfOwned(cf))
    }
}

/// Build an immutable `CFDictionary` from `(key, value)` pairs, all borrowed
/// — the dictionary retains its own references to them (the "CFType"
/// callbacks), so the caller's `CfOwned` guards may still be dropped after
/// this call returns; the dictionary keeps the CF objects alive itself.
fn cf_dict(pairs: &[(CFTypeRef, CFTypeRef)]) -> Option<CfOwned> {
    let keys: Vec<*const c_void> = pairs.iter().map(|(k, _)| *k).collect();
    let values: Vec<*const c_void> = pairs.iter().map(|(_, v)| *v).collect();
    let cf = unsafe {
        CFDictionaryCreate(
            ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            pairs.len() as CFIndex,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        )
    };
    if cf.is_null() {
        None
    } else {
        Some(CfOwned(cf))
    }
}

pub fn store(account: &str, secret: &str) -> StoreOutcome {
    let (Some(service), Some(acct), Some(data)) =
        (cf_string(super::SERVICE), cf_string(account), cf_data(secret.as_bytes()))
    else {
        return StoreOutcome::Failed("could not allocate CFString/CFData".into());
    };

    let status = {
        let Some(add_query) = cf_dict(&[
            (unsafe { kSecClass }, unsafe { kSecClassGenericPassword }),
            (unsafe { kSecAttrService }, service.as_ref()),
            (unsafe { kSecAttrAccount }, acct.as_ref()),
            (unsafe { kSecValueData }, data.as_ref()),
        ]) else {
            return StoreOutcome::Failed("could not allocate CFDictionary".into());
        };
        unsafe { SecItemAdd(add_query.as_ref(), ptr::null_mut()) }
    };

    if status == ERR_SEC_SUCCESS {
        return StoreOutcome::Stored;
    }
    if status != ERR_SEC_DUPLICATE_ITEM {
        return StoreOutcome::Failed(format!("SecItemAdd failed (OSStatus {status})"));
    }

    // An item for this (service, account) already exists — update it in
    // place instead of failing the whole store attempt.
    let Some(match_query) = cf_dict(&[
        (unsafe { kSecClass }, unsafe { kSecClassGenericPassword }),
        (unsafe { kSecAttrService }, service.as_ref()),
        (unsafe { kSecAttrAccount }, acct.as_ref()),
    ]) else {
        return StoreOutcome::Failed("could not allocate CFDictionary".into());
    };
    let Some(update_attrs) = cf_dict(&[(unsafe { kSecValueData }, data.as_ref())]) else {
        return StoreOutcome::Failed("could not allocate CFDictionary".into());
    };
    let status2 = unsafe { SecItemUpdate(match_query.as_ref(), update_attrs.as_ref()) };
    if status2 == ERR_SEC_SUCCESS {
        StoreOutcome::Stored
    } else {
        StoreOutcome::Failed(format!("SecItemUpdate failed (OSStatus {status2})"))
    }
}

pub fn retrieve(account: &str) -> Option<String> {
    let service = cf_string(super::SERVICE)?;
    let acct = cf_string(account)?;

    let query = cf_dict(&[
        (unsafe { kSecClass }, unsafe { kSecClassGenericPassword }),
        (unsafe { kSecAttrService }, service.as_ref()),
        (unsafe { kSecAttrAccount }, acct.as_ref()),
        (unsafe { kSecReturnData }, unsafe { kCFBooleanTrue }),
        (unsafe { kSecMatchLimit }, unsafe { kSecMatchLimitOne }),
    ])?;

    let mut result: CFTypeRef = ptr::null();
    let status = unsafe { SecItemCopyMatching(query.as_ref(), &mut result) };
    if status != ERR_SEC_SUCCESS || result.is_null() {
        return None; // ERR_SEC_ITEM_NOT_FOUND and every other failure alike.
    }
    let data = CfOwned(result); // now owned — SecItemCopyMatching follows the Create Rule too.

    let len = unsafe { CFDataGetLength(data.as_ref()) };
    let ptr = unsafe { CFDataGetBytePtr(data.as_ref()) };
    if ptr.is_null() || len < 0 {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    String::from_utf8(bytes.to_vec()).ok()
}

pub fn delete(account: &str) {
    let Some(service) = cf_string(super::SERVICE) else {
        return;
    };
    let Some(acct) = cf_string(account) else {
        return;
    };
    let Some(query) = cf_dict(&[
        (unsafe { kSecClass }, unsafe { kSecClassGenericPassword }),
        (unsafe { kSecAttrService }, service.as_ref()),
        (unsafe { kSecAttrAccount }, acct.as_ref()),
    ]) else {
        return;
    };
    // ERR_SEC_ITEM_NOT_FOUND is a normal outcome here (nothing to delete);
    // any other status is a best-effort operation with nothing further this
    // caller can do about it.
    let status = unsafe { SecItemDelete(query.as_ref()) };
    let _ = status == ERR_SEC_SUCCESS || status == ERR_SEC_ITEM_NOT_FOUND;
}

#[cfg(test)]
mod tests {
    use super::*;

    // These exercise the REAL macOS Keychain (no mocking) — each test uses a
    // unique, clearly-scoped account name and deletes it in a guard so a
    // panic mid-test still cleans up, never leaving a stray Keychain item
    // behind from a test run.
    struct CleanupGuard(String);
    impl Drop for CleanupGuard {
        fn drop(&mut self) {
            delete(&self.0);
        }
    }

    #[test]
    fn store_then_retrieve_round_trips() {
        let account = "test-round-trip-1";
        let _cleanup = CleanupGuard(account.to_string());
        delete(account); // start from a clean slate in case a prior run left one

        let outcome = store(account, "s3cr3t-value");
        assert_eq!(outcome, StoreOutcome::Stored, "store failed: {outcome:?}");
        assert_eq!(retrieve(account), Some("s3cr3t-value".to_string()));
    }

    #[test]
    fn storing_twice_updates_in_place_not_duplicate_item() {
        let account = "test-round-trip-2";
        let _cleanup = CleanupGuard(account.to_string());
        delete(account);

        assert_eq!(store(account, "first"), StoreOutcome::Stored);
        assert_eq!(store(account, "second"), StoreOutcome::Stored);
        assert_eq!(retrieve(account), Some("second".to_string()));
    }

    #[test]
    fn delete_then_retrieve_is_none() {
        let account = "test-round-trip-3";
        delete(account);
        assert_eq!(store(account, "will be deleted"), StoreOutcome::Stored);
        delete(account);
        assert_eq!(retrieve(account), None);
    }

    #[test]
    fn retrieve_of_never_stored_account_is_none() {
        assert_eq!(retrieve("test-never-stored-account-xyz"), None);
    }

    #[test]
    fn delete_of_never_stored_account_does_not_panic() {
        delete("test-never-stored-account-abc"); // must not panic
    }
}
