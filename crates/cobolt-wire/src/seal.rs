// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The sealed channel's cryptography (WASM-003 R24; WASM-013 R17, plan §1.11).
//!
//! Both sides make an X25519 key pair when they start. The two `hello`s carry the public keys and a nonce
//! each; [`establish`] agrees a shared secret, derives one 32-byte key per direction with HKDF-SHA256 and
//! returns a [`Channel`] whose [`seal`](Channel::seal) and [`open`](Channel::open) carry every later message as one
//! ChaCha20-Poly1305 frame. The nonce of a frame is an implicit 64-bit counter of its direction (4 zero bytes,
//! then the counter, big-endian), never sent, so a replayed, reordered, dropped or truncated frame fails to open.
//!
//! The module takes **no randomness**: its owner hands it 32 bytes for a key pair (`getrandom` on a server,
//! `crypto.getRandomValues` in a browser), so it builds unchanged for `wasm32`. No type here is `Debug`,
//! `Clone` or serialisable, and the secrets are wiped when they drop.

use chacha20poly1305::{aead::Aead, ChaCha20Poly1305, KeyInit};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::error::{codes, CallError};

/// The bytes of a public key and of a hello nonce.
pub const KEY_LEN: usize = 32;
pub const NONCE_LEN: usize = 16;
/// What the AEAD adds to every frame.
pub const TAG_LEN: usize = 16;

const INFO_C2S: &[u8] = b"prc-channel/1 c2s";
const INFO_S2C: &[u8] = b"prc-channel/1 s2c";

/// Which end of the connection this side is.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Client,
    Server,
}

/// One side's key pair, made from 32 bytes the caller drew at random. The private half never leaves.
pub struct KeyPair {
    secret: StaticSecret,
}

impl KeyPair {
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        let secret = StaticSecret::from(bytes);
        // The caller's copy is the caller's to wipe; ours lives in `secret`, which wipes itself.
        KeyPair { secret }
    }

    pub fn public(&self) -> [u8; KEY_LEN] {
        PublicKey::from(&self.secret).to_bytes()
    }
}

fn refused(message: &str) -> CallError {
    CallError::transport(codes::CLOSED, message)
}

/// Agree the two direction keys and return the channel. `peer_key` and `peer_nonce` are the other side's, as
/// read from its `hello`. Refused when the peer's key gives an all-zero shared secret (a low-order point).
pub fn establish(
    role: Role,
    protocol: u32,
    own: &KeyPair,
    own_nonce: &[u8; NONCE_LEN],
    peer_key: &[u8; KEY_LEN],
    peer_nonce: &[u8; NONCE_LEN],
) -> Result<Channel, CallError> {
    let shared = own.secret.diffie_hellman(&PublicKey::from(*peer_key));
    if !shared.was_contributory() {
        return Err(refused("the key exchange failed"));
    }
    let own_key = own.public();
    let (ck, sk, cn, sn) = match role {
        Role::Client => (&own_key, peer_key, own_nonce, peer_nonce),
        Role::Server => (peer_key, &own_key, peer_nonce, own_nonce),
    };
    let mut transcript = Vec::with_capacity(4 + 2 * KEY_LEN + 2 * NONCE_LEN);
    transcript.extend_from_slice(&protocol.to_be_bytes());
    transcript.extend_from_slice(ck);
    transcript.extend_from_slice(sk);
    transcript.extend_from_slice(cn);
    transcript.extend_from_slice(sn);
    let salt = Sha256::digest(&transcript);
    let hk = Hkdf::<Sha256>::new(Some(&salt), shared.as_bytes());
    let mut c2s = Zeroizing::new([0u8; 32]);
    let mut s2c = Zeroizing::new([0u8; 32]);
    hk.expand(INFO_C2S, c2s.as_mut()).map_err(|_| refused("the key exchange failed"))?;
    hk.expand(INFO_S2C, s2c.as_mut()).map_err(|_| refused("the key exchange failed"))?;
    let (send, recv) = match role {
        Role::Client => (&c2s, &s2c),
        Role::Server => (&s2c, &c2s),
    };
    Ok(Channel { send: cipher(send), recv: cipher(recv), send_counter: 0, recv_counter: 0 })
}

fn cipher(key: &Zeroizing<[u8; 32]>) -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new(&(**key).into())
}

fn nonce(counter: u64) -> chacha20poly1305::Nonce {
    let mut n = [0u8; 12];
    n[4..].copy_from_slice(&counter.to_be_bytes());
    n.into()
}

/// An established channel: one cipher and one counter per direction.
pub struct Channel {
    send: ChaCha20Poly1305,
    recv: ChaCha20Poly1305,
    send_counter: u64,
    recv_counter: u64,
}

impl Channel {
    /// The frame for `text`: the ciphertext and its 16-byte tag. Panics never; a counter that would wrap is
    /// refused (2^64 frames is far past any connection's life).
    pub fn seal(&mut self, text: &str) -> Result<Vec<u8>, CallError> {
        if self.send_counter == u64::MAX {
            return Err(refused("the channel has carried its last frame"));
        }
        let frame = self
            .send
            .encrypt(&nonce(self.send_counter), text.as_bytes())
            .map_err(|_| refused("a frame could not be sealed"))?;
        self.send_counter += 1;
        Ok(frame)
    }

    /// The text of the next frame. A frame that does not open (tampered, replayed, reordered, truncated, from
    /// another connection) is refused, and the counter does not move.
    pub fn open(&mut self, frame: &[u8]) -> Result<String, CallError> {
        if frame.len() < TAG_LEN || self.recv_counter == u64::MAX {
            return Err(refused("a frame did not open"));
        }
        let bytes = self
            .recv
            .decrypt(&nonce(self.recv_counter), frame)
            .map_err(|_| refused("a frame did not open"))?;
        self.recv_counter += 1;
        String::from_utf8(bytes).map_err(|_| refused("a frame did not open"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        let s: String = s.split_whitespace().collect();
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }
    fn arr32(s: &str) -> [u8; 32] {
        hex(s).try_into().unwrap()
    }

    fn pair() -> (Channel, Channel) {
        let (a, b) = (KeyPair::from_bytes([7; 32]), KeyPair::from_bytes([9; 32]));
        let (na, nb) = ([1u8; NONCE_LEN], [2u8; NONCE_LEN]);
        let c = establish(Role::Client, 2, &a, &na, &b.public(), &nb).unwrap();
        let s = establish(Role::Server, 2, &b, &nb, &a.public(), &na).unwrap();
        (c, s)
    }

    /// RFC 7748 §6.1: the X25519 vector through the wrapper (public keys and the shared secret).
    #[test]
    fn x25519_matches_rfc_7748() {
        let a = KeyPair::from_bytes(arr32("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a"));
        let b = KeyPair::from_bytes(arr32("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb"));
        assert_eq!(a.public(), arr32("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"));
        assert_eq!(b.public(), arr32("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f"));
        let k = a.secret.diffie_hellman(&PublicKey::from(b.public()));
        assert_eq!(k.to_bytes(), arr32("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742"));
    }

    /// RFC 8439 §2.8.2: the ChaCha20-Poly1305 vector (with its AAD) through the same crate the channel uses.
    #[test]
    fn chacha20_poly1305_matches_rfc_8439() {
        use chacha20poly1305::aead::Payload;
        let key: [u8; 32] = core::array::from_fn(|i| 0x80 + i as u8);
        let n: [u8; 12] = [0x07, 0, 0, 0, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47];
        let aad = hex("50515253c0c1c2c3c4c5c6c7");
        let pt = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let out = ChaCha20Poly1305::new(&key.into())
            .encrypt(&n.into(), Payload { msg: pt, aad: &aad })
            .unwrap();
        assert_eq!(&out[..16], &hex("d31a8d34648e60db7b86afbc53ef7ec2")[..]);
        assert_eq!(&out[out.len() - 16..], &hex("1ae10b594f09e26a7e902ecbd0600691")[..]);
    }

    #[test]
    fn both_sides_agree_and_talk_both_ways() {
        let (mut c, mut s) = pair();
        for i in 0..5 {
            let f = c.seal(&format!("call {i}")).unwrap();
            assert_eq!(f.len(), format!("call {i}").len() + TAG_LEN);
            assert_eq!(s.open(&f).unwrap(), format!("call {i}"));
            let g = s.seal(&format!("reply {i}")).unwrap();
            assert_eq!(c.open(&g).unwrap(), format!("reply {i}"));
        }
    }

    #[test]
    fn two_connections_of_the_same_key_pairs_derive_different_keys() {
        let (a, b) = (KeyPair::from_bytes([7; 32]), KeyPair::from_bytes([9; 32]));
        let mk = |na: u8, nb: u8| {
            let mut c = establish(Role::Client, 2, &a, &[na; 16], &b.public(), &[nb; 16]).unwrap();
            c.seal("same text").unwrap()
        };
        assert_ne!(mk(1, 2), mk(3, 2), "another client nonce, another key");
        assert_ne!(mk(1, 2), mk(1, 4), "another server nonce, another key");
        assert_eq!(mk(1, 2), mk(1, 2), "the same inputs derive the same key");
        let c2s_vs_s2c = {
            let (mut c, mut s) = pair();
            (c.seal("x").unwrap(), s.seal("x").unwrap())
        };
        assert_ne!(c2s_vs_s2c.0, c2s_vs_s2c.1, "the two directions have their own keys");
    }

    #[test]
    fn a_frame_that_was_tampered_replayed_reordered_or_truncated_is_refused() {
        let (mut c, mut s) = pair();
        let f0 = c.seal("one").unwrap();
        let f1 = c.seal("two").unwrap();
        let mut bad = f0.clone();
        bad[0] ^= 1;
        assert!(s.open(&bad).is_err(), "tampered");
        assert!(s.open(&f0[..f0.len() - 1]).is_err(), "truncated");
        assert!(s.open(&[]).is_err(), "empty");
        assert!(s.open(&f1).is_err(), "reordered: frame 1 before frame 0");
        assert_eq!(s.open(&f0).unwrap(), "one", "a refusal did not move the counter");
        assert!(s.open(&f0).is_err(), "replayed");
        assert_eq!(s.open(&f1).unwrap(), "two");
    }

    #[test]
    fn a_frame_from_another_connection_is_refused() {
        let (mut c, _) = pair();
        let (_, mut other) = {
            let (a, b) = (KeyPair::from_bytes([7; 32]), KeyPair::from_bytes([9; 32]));
            let s = establish(Role::Server, 2, &b, &[5; 16], &a.public(), &[1; 16]).unwrap();
            (0, s)
        };
        assert!(other.open(&c.seal("x").unwrap()).is_err());
    }

    #[test]
    fn a_low_order_peer_key_is_refused() {
        let me = KeyPair::from_bytes([7; 32]);
        for key in [[0u8; 32], { let mut k = [0u8; 32]; k[0] = 1; k }] {
            assert!(establish(Role::Client, 2, &me, &[0; 16], &key, &[0; 16]).is_err());
        }
    }

    #[test]
    fn other_bytes_make_another_key_pair() {
        assert_ne!(KeyPair::from_bytes([1; 32]).public(), KeyPair::from_bytes([2; 32]).public());
        assert_eq!(KeyPair::from_bytes([1; 32]).public(), KeyPair::from_bytes([1; 32]).public());
    }
}
