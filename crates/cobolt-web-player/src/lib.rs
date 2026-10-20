// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The PowerRustCOBOL player** (WASM-016): the prebuilt form engine of a Web or Mobile
//! application. See `specs/WASM-016-prebuilt-player/plan.md`.
//!
//! The crate is layered so that almost everything can be run and tested natively:
//!
//! | Layer | Modules | Touches the browser? |
//! |---|---|---|
//! | The ABI host (WASM-ABI-v1) | [`host`], [`tree`], [`methods`] | no |
//! | Policy and front halves | [`weblink`], [`auth`], [`kinds`], [`par`], [`value`] | no |

pub mod auth;
pub mod host;
pub mod kinds;
pub mod methods;
pub mod par;
pub mod tree;
pub mod value;
pub mod weblink;
