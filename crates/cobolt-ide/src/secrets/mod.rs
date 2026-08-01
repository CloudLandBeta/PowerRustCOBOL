// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! OS-native secret storage — the platform's own credential store, reached
//! with **zero external crate dependencies** (hand-written FFI on macOS and
//! Windows, a subprocess call to the platform's standard CLI on Linux).
//!
//! This is deliberately a thin, best-effort layer: every operation degrades
//! to [`StoreOutcome::Unavailable`] rather than an error whenever the native
//! store cannot be reached at all (no keychain daemon, no Secret Service
//! session, an unsupported OS). The caller (`llm.rs`) is expected to fall
//! back to its existing plaintext JSON storage in that case — "if security
//! doesn't matter to you here, you get plaintext" is the explicit, accepted
//! behaviour, not a bug.
//!
//! `SERVICE` is the single namespace every credential is stored under; each
//! individual secret (an LLM provider key, the Google Maps key, the Custom
//! Search key, …) is one `account` within it — the same shape `llm.rs`
//! already uses for its `api_keys` slot names, so a slot name doubles
//! directly as the native store's account name.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
mod linux;

/// The service/namespace every PowerRustCOBOL credential is filed under in
/// the native store. Stable — changing it would orphan already-stored
/// secrets (they would still exist in the OS store, just no longer found).
pub const SERVICE: &str = "PowerRustCOBOL";

/// Kill switch: forces every native-store operation below to behave as if no
/// store were reachable, so `llm.rs` always falls back to its plaintext JSON
/// storage. There is no Settings UI yet to let the developer inspect, clear,
/// or rotate a Keychain/Credential-Manager-stored key (only ever write one),
/// and this is pre-release with no installed base to migrate — shipping the
/// native store now would strand a key a developer could only remove by
/// finding it in the OS's own credential manager. Flip this back to `false`
/// (and delete it, along with this comment) once release-candidate 1.90.0
/// ships a proper secrets-management UI.
const NATIVE_STORE_DISABLED: bool = true;

/// Result of attempting to write a secret to the native store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreOutcome {
    /// Written to the native store — the caller must NOT also persist this
    /// value in plaintext.
    Stored,
    /// No native store reachable on this platform/environment at all (e.g.
    /// Linux with no Secret Service session, or an unsupported OS). The
    /// caller's plaintext fallback is the intended behaviour here, not an
    /// error to surface.
    Unavailable,
    /// A native store IS reachable, but this specific write failed (e.g. the
    /// keychain is locked and the user declined the unlock prompt). Carries
    /// a short reason for logging; the caller still falls back to plaintext
    /// so the developer's key isn't silently dropped.
    Failed(String),
}

/// Store `secret` under `account` in the platform's native credential store.
/// Overwrites any existing entry for the same `(SERVICE, account)` pair.
pub fn store(account: &str, secret: &str) -> StoreOutcome {
    if account.trim().is_empty() {
        return StoreOutcome::Failed("empty account name".into());
    }
    if NATIVE_STORE_DISABLED {
        return StoreOutcome::Unavailable;
    }
    #[cfg(test)]
    if test_support::force_unavailable() {
        return StoreOutcome::Unavailable;
    }
    imp::store(account, secret)
}

/// Read a secret previously stored under `account`. `None` covers every
/// "not there" case uniformly — no native store available, the store is
/// reachable but has no entry for this account, or the read itself failed —
/// callers already treat "no key" as the ordinary empty-credential state.
pub fn retrieve(account: &str) -> Option<String> {
    if account.trim().is_empty() {
        return None;
    }
    if NATIVE_STORE_DISABLED {
        return None;
    }
    #[cfg(test)]
    if test_support::force_unavailable() {
        return None;
    }
    imp::retrieve(account)
}

/// Remove the entry for `account`, if any. Best-effort: a missing entry or
/// an unreachable store is not an error — there is nothing left to do.
pub fn delete(account: &str) {
    if account.trim().is_empty() {
        return;
    }
    if NATIVE_STORE_DISABLED {
        return;
    }
    #[cfg(test)]
    if test_support::force_unavailable() {
        return;
    }
    imp::delete(account);
}

/// Test-only gate: by default, every test in this whole crate's `cargo
/// test` run sees `store`/`retrieve`/`delete` (the public API above, NOT
/// this module's own platform-backend unit tests, which call their
/// backend's functions directly) as if no native store existed. Hundreds
/// of pre-existing `llm.rs` tests save/load a throwaway API key and never
/// anticipated touching a real OS credential store — this keeps them on
/// exactly the plaintext-fallback path they exercised before this module
/// existed, instead of writing real (if short-lived) entries into the
/// developer's actual Keychain/Credential Manager/Secret Service on every
/// `cargo test` run.
///
/// The few tests that specifically exercise the native-store integration
/// (in `llm.rs`) opt back in with [`test_support::AllowNativeStoreForTest`].
///
/// This is **thread-local, not a process-global flag** — deliberately: an
/// early version used a global `AtomicBool` (+ a `Mutex` to serialize the
/// opted-in tests against everything else), and it leaked a real, if
/// short-lived, Keychain entry from an unrelated pre-existing test on the
/// very first parallel `cargo test` run, because that test never expected
/// to need any synchronization at all — a global flag makes literally
/// every test in the binary a participant whether it knows it or not.
/// Rust's default test harness runs each `#[test]` fn on its own freshly
/// spawned OS thread, so a `thread_local!` gives every test perfect,
/// automatic isolation for free: no locking, and no other test can ever
/// observe one test's opt-in.
#[cfg(test)]
pub mod test_support {
    use std::cell::Cell;

    thread_local! {
        static FORCE_UNAVAILABLE: Cell<bool> = const { Cell::new(true) };
    }

    pub(super) fn force_unavailable() -> bool {
        FORCE_UNAVAILABLE.with(|f| f.get())
    }

    #[must_use = "the native store is only enabled while this guard is held"]
    pub struct AllowNativeStoreForTest {
        _not_send: std::marker::PhantomData<*const ()>,
    }

    impl AllowNativeStoreForTest {
        pub fn new() -> Self {
            FORCE_UNAVAILABLE.with(|f| f.set(false));
            Self {
                _not_send: std::marker::PhantomData,
            }
        }
    }

    impl Default for AllowNativeStoreForTest {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Drop for AllowNativeStoreForTest {
        fn drop(&mut self) {
            FORCE_UNAVAILABLE.with(|f| f.set(true));
        }
    }
}

#[cfg(target_os = "macos")]
use macos as imp;
#[cfg(target_os = "windows")]
use windows as imp;
#[cfg(target_os = "linux")]
use linux as imp;

/// Every other target (other Unix flavours, WASM, …): no native secret store
/// is attempted at all. This is the "let him eat what he produce" tier —
/// `llm.rs`'s plaintext JSON fallback is the only storage on these
/// platforms, by design, not by omission.
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod other {
    pub fn store(_account: &str, _secret: &str) -> super::StoreOutcome {
        super::StoreOutcome::Unavailable
    }
    pub fn retrieve(_account: &str) -> Option<String> {
        None
    }
    pub fn delete(_account: &str) {}
}
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
use other as imp;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_account_never_touches_the_native_store() {
        assert_eq!(
            store("", "secret"),
            StoreOutcome::Failed("empty account name".into())
        );
        assert_eq!(retrieve(""), None);
        delete(""); // must not panic
    }
}
