// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `cobolt-wire` — what a frontend and a backend agree on (WASM-005, WASM-014).
//!
//! * [`layout`] — the shape of a COBOL group as the wire sees it.
//! * [`node`] — the value of a group, a tree shaped like its layout.
//! * [`mapping`] — the COBOL group <-> JSON mapping, in both directions, with
//!   the rules and the error list the golden vectors pin.
//! * [`controls`] — the class C catalogue: one row per control operation that
//!   runs on the backend on the web, its layouts and its endpoint name.
//! * [`json`], [`number`] — the strict JSON reader and the exact decimal text
//!   the mapping is built on.
//! * [`retry`] — the retry policy of a broken connection both clients follow.
//!
//! The crate is the **reference** implementation of the mapping. The browser
//! module's generator carries its own implementation of the same rules
//! (WASM-005 R5b) and is held to the same vectors, so a difference between the
//! two is a failing test and not a field in production.

pub mod call;
pub mod compat;
pub mod contract;
pub mod controls;
pub mod error;
pub mod evolution;
pub mod json;
pub mod layout;
pub mod mapping;
pub mod node;
pub mod number;
pub mod proto;
pub mod retry;

pub use call::{ArgMode, ArgPayload, BackendFiles, CallObserver, CallReply, CallRequest, DownloadRequest, DownloadResult, NoObserver, Transport, UploadRequest, UploadResult};
pub use error::{CallError, ErrorClass};
pub use layout::{from_decl, Item, Kind, LayoutError, Occurs, Step};
pub use mapping::{decode, decode_json, encode, ErrorKind, MapError};
pub use node::Node;
