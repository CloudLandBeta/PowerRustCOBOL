// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The hash of a file going to the backend (WASM-014 T15, R15): SHA-256 taken incrementally over the pieces the browser reads with
//! `File.slice`, so a file of any size is hashed without being held whole. The backend verifies it when the upload ends.

use sha2::{Digest, Sha256};

/// An incremental SHA-256.
#[derive(Default)]
pub struct FileHash {
    hasher: Sha256,
    bytes: u64,
}

impl FileHash {
    pub fn new() -> FileHash {
        FileHash::default()
    }

    /// Take in the next piece.
    pub fn update(&mut self, piece: &[u8]) {
        self.hasher.update(piece);
        self.bytes += piece.len() as u64;
    }

    /// How many bytes have been taken in.
    pub fn len(&self) -> u64 {
        self.bytes
    }

    pub fn is_empty(&self) -> bool {
        self.bytes == 0
    }

    /// The hash as 64 lower-case hex digits.
    pub fn finish_hex(self) -> String {
        self.hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }
}
