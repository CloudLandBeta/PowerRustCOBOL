// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Backend container (spec 114 R3, T19): what a built Backend carries inside the runner's executable, as the
//! payload's [`Kind::Backend`](crate::Kind::Backend) part.
//!
//! It holds what the server needs to answer its clients without a program's source: the interface description the
//! contract publishes, the table of exports (each program's name, parameters, `RETURNING` and the control it seeds),
//! the released interfaces of earlier contract versions, the upload slots, the state mode, and the WebAssembly module
//! that runs the programs. The texts are JSON the server's own types read and write, kept here as bytes so this crate
//! does not depend on the server.
//!
//! **The format is permanent.** A container starts with its magic and a version number, the first version being 1; a
//! reader reads every version ever written and refuses a newer one by name. Version 1 is the magic, the version, and six
//! fields in this order, each a little-endian `u32` length and that many bytes: `state` (UTF-8), `interface`,
//! `exports`, `history`, `slots` and `module`. A later version appends fields; a version 1 container read by a later
//! reader has them empty.

use crate::PackageError;

/// The container's magic, at its first byte.
pub const CONTAINER_MAGIC: [u8; 8] = *b"PRBCNTR\0";
/// The version this crate writes. A reader accepts every version up to its own.
pub const CONTAINER_VERSION: u32 = 1;

/// The Backend a payload carries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BackendContainer {
    /// Whether `WORKING-STORAGE` outlives a call, in the project's words (`per-call`).
    pub state: String,
    /// The interface description of the current contract (JSON).
    pub interface: Vec<u8>,
    /// The exported programs: name, parameter and `RETURNING` names, identity, the control each seeds (JSON).
    pub exports: Vec<u8>,
    /// The released interface descriptions of earlier contract versions, oldest first (a JSON array).
    pub history: Vec<u8>,
    /// The upload slots of the forms' file controls (a JSON array).
    pub slots: Vec<u8>,
    /// The WebAssembly module (spec 114 A59).
    pub module: Vec<u8>,
}

impl BackendContainer {
    /// The container as bytes.
    pub fn encode(&self) -> Vec<u8> {
        let fields: [&[u8]; 6] = [self.state.as_bytes(), &self.interface, &self.exports, &self.history, &self.slots, &self.module];
        let mut out = Vec::with_capacity(12 + fields.iter().map(|f| 4 + f.len()).sum::<usize>());
        out.extend_from_slice(&CONTAINER_MAGIC);
        out.extend_from_slice(&CONTAINER_VERSION.to_le_bytes());
        for f in fields {
            out.extend_from_slice(&(f.len() as u32).to_le_bytes());
            out.extend_from_slice(f);
        }
        out
    }

    /// A container read from bytes. Refused, in words: a wrong magic, a version newer than this reader, a field that runs
    /// past the end, bytes left over, a state that is not UTF-8.
    pub fn decode(bytes: &[u8]) -> Result<BackendContainer, PackageError> {
        let damaged = |why: &str| PackageError::Damaged(format!("the Backend container {why}"));
        if bytes.len() < 12 || bytes[..8] != CONTAINER_MAGIC {
            return Err(damaged("has no header"));
        }
        let version = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        if version == 0 {
            return Err(damaged("has version 0, which no Build wrote"));
        }
        if version > CONTAINER_VERSION {
            return Err(PackageError::NewerFormat(version));
        }
        let mut at = 12usize;
        let mut field = |name: &str| -> Result<Vec<u8>, PackageError> {
            let len_end = at.checked_add(4).filter(|&e| e <= bytes.len()).ok_or_else(|| damaged(&format!("ends before the length of its {name}")))?;
            let len = u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize;
            let end = len_end.checked_add(len).filter(|&e| e <= bytes.len()).ok_or_else(|| damaged(&format!("ends inside its {name}")))?;
            let out = bytes[len_end..end].to_vec();
            at = end;
            Ok(out)
        };
        let state = field("state")?;
        let interface = field("interface")?;
        let exports = field("exports")?;
        let history = field("history")?;
        let slots = field("slots")?;
        let module = field("module")?;
        if at != bytes.len() {
            return Err(damaged("has bytes after its last field"));
        }
        let state = String::from_utf8(state).map_err(|_| damaged("has a state that is not text"))?;
        Ok(BackendContainer { state, interface, exports, history, slots, module })
    }
}
