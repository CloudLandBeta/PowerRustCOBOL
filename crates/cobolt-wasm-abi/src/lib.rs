// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **WASM ABI v1** — the contract between the generated program module, the
//! prebuilt player and the host page (`specs/WASM-ABI-v1.md`, normative).
//!
//! The crate holds what all three share and nothing they do not:
//!
//! - [`codes`] — opcodes, statuses, error codes, reserved control ids;
//! - [`batch`] — the command batch and queries (§5–§6), event arguments (§7):
//!   a zero-copy [`BatchReader`](batch::BatchReader) and a
//!   [`BatchWriter`](batch::BatchWriter);
//! - [`manifest`] — the manifest (§10) and the arena layout it implies (§2);
//! - `scan` (feature `scan`) — reading a module without running it, and the
//!   build-time checks a bundle member must pass;
//! - `fixture` (feature `fixture`) — hand-assembled test modules, one per ABI
//!   variant, standing in for the generator's output in tests.
//!
//! It has no dependency on egui, `web-sys` or the file system, so the player
//! (`wasm32`), the bundle assembler and the generator can all link it.

pub mod batch;
pub mod codes;
pub mod manifest;

#[cfg(feature = "scan")]
pub mod scan;

#[cfg(feature = "fixture")]
pub mod fixture;

pub use batch::{decode_batch, BatchReader, BatchWriter, Command, DecodeError, Path, PathSpec, Seg};
pub use codes::{Status, ABI_VERSION};
pub use manifest::{ArenaLayout, Manifest, ManifestError};
