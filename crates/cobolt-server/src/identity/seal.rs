// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Sealing (WASM-009 D7): what the backend hands out and takes back — the refresh cookie, the
//! hosted-flow cookie, a challenge's continuation — is encrypted and authenticated with
//! XChaCha20-Poly1305 under `[identity] cookie_key`.
//!
//! A sealed value is `base64url(nonce ‖ ciphertext)`, the nonce 24 random bytes. Inside, the
//! first 8 bytes are the issue time (seconds since the epoch); the **purpose** (`refresh`,
//! `hosted`, `challenge`, …) is the associated data, so a value sealed for one use cannot be
//! replayed as another. [`Sealer::open`] refuses a value that was changed, sealed under another
//! key or purpose, issued in the future, or older than the age it is given.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};

const NONCE: usize = 24;
/// A clock a little ahead of ours is not a forgery.
const SKEW: u64 = 60;

pub struct Sealer {
    cipher: XChaCha20Poly1305,
}

impl std::fmt::Debug for Sealer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sealer(<key>)")
    }
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Sealer {
    pub fn new(key: &[u8; 32]) -> Sealer {
        Sealer { cipher: XChaCha20Poly1305::new(key.into()) }
    }

    /// From the `cookie_key` setting's text (base64 or hex, 32 bytes).
    pub fn from_setting(text: &str) -> Result<Sealer, String> {
        crate::config::cookie_key_bytes(text).map(|k| Sealer::new(&k))
    }

    pub fn seal(&self, purpose: &str, payload: &[u8]) -> String {
        self.seal_at(purpose, payload, now_secs())
    }

    pub fn seal_at(&self, purpose: &str, payload: &[u8], issued: u64) -> String {
        let mut nonce = [0u8; NONCE];
        // A failed draw would repeat a nonce: never seal with one.
        getrandom::fill(&mut nonce).expect("the operating system's random source");
        let mut plain = issued.to_be_bytes().to_vec();
        plain.extend_from_slice(payload);
        let sealed = self.cipher.encrypt(&XNonce::from(nonce), Payload { msg: &plain, aad: purpose.as_bytes() }).expect("XChaCha20-Poly1305 seals any length the backend gives it");
        let mut out = nonce.to_vec();
        out.extend_from_slice(&sealed);
        URL_SAFE_NO_PAD.encode(out)
    }

    /// The payload, when `text` is a value this key sealed for `purpose` within `max_age`.
    pub fn open(&self, purpose: &str, text: &str, max_age: Duration) -> Option<Vec<u8>> {
        self.open_at(purpose, text, max_age, now_secs())
    }

    pub fn open_at(&self, purpose: &str, text: &str, max_age: Duration, now: u64) -> Option<Vec<u8>> {
        if text.len() > 8192 {
            return None;
        }
        let raw = URL_SAFE_NO_PAD.decode(text.trim()).ok()?;
        if raw.len() < NONCE + 8 + 16 {
            return None;
        }
        let (nonce, sealed) = raw.split_at(NONCE);
        let nonce: [u8; NONCE] = nonce.try_into().ok()?;
        let plain = self.cipher.decrypt(&XNonce::from(nonce), Payload { msg: sealed, aad: purpose.as_bytes() }).ok()?;
        let issued = u64::from_be_bytes(plain[..8].try_into().ok()?);
        if issued > now + SKEW || now.saturating_sub(issued) > max_age.as_secs() {
            return None;
        }
        Some(plain[8..].to_vec())
    }

    /// [`Sealer::seal`] of a JSON value.
    pub fn seal_json(&self, purpose: &str, value: &serde_json::Value) -> String {
        self.seal(purpose, value.to_string().as_bytes())
    }

    pub fn open_json(&self, purpose: &str, text: &str, max_age: Duration) -> Option<serde_json::Value> {
        serde_json::from_slice(&self.open(purpose, text, max_age)?).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: Duration = Duration::from_secs(3600);

    #[test]
    fn a_sealed_value_opens_only_unchanged_under_its_key_and_purpose() {
        let s = Sealer::new(&[7; 32]);
        let v = s.seal("refresh", b"the refresh token");
        assert_eq!(s.open("refresh", &v, HOUR).as_deref(), Some(&b"the refresh token"[..]));
        assert!(!v.contains("refresh"), "the payload is not readable: {v}");
        assert_ne!(s.seal("refresh", b"x"), s.seal("refresh", b"x"), "a fresh nonce every time");
        // Tampered: every byte flipped in turn is refused.
        let raw = URL_SAFE_NO_PAD.decode(&v).unwrap();
        let mut refused = 0;
        for i in 0..raw.len() {
            let mut t = raw.clone();
            t[i] ^= 0x01;
            if s.open("refresh", &URL_SAFE_NO_PAD.encode(&t), HOUR).is_none() {
                refused += 1;
            }
        }
        assert_eq!(refused, raw.len(), "every one of {} tampered copies is refused", raw.len());
        assert_eq!(s.open("hosted", &v, HOUR), None, "sealed for another purpose");
        assert_eq!(Sealer::new(&[8; 32]).open("refresh", &v, HOUR), None, "another key");
        assert_eq!(s.open("refresh", "not-base64!", HOUR), None);
        assert_eq!(s.open("refresh", "", HOUR), None);
    }

    #[test]
    fn an_old_cookie_and_one_from_the_future_are_refused() {
        let s = Sealer::new(&[1; 32]);
        let now = 1_800_000_000;
        let v = s.seal_at("refresh", b"t", now - 3600);
        assert!(s.open_at("refresh", &v, HOUR, now).is_some(), "exactly the age");
        assert_eq!(s.open_at("refresh", &v, Duration::from_secs(3599), now), None, "one second too old");
        let ahead = s.seal_at("refresh", b"t", now + 3600);
        assert_eq!(s.open_at("refresh", &ahead, HOUR, now), None, "issued in the future");
        let json = s.seal_json("challenge", &serde_json::json!({"u": "ana"}));
        assert_eq!(s.open_json("challenge", &json, HOUR), Some(serde_json::json!({"u": "ana"})));
    }
}
