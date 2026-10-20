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
//! | Policy and front halves | [`weblink`], [`auth`], [`kinds`], [`net`], [`par`], [`value`] | no |
//! | The JavaScript boundary and the DOM | `api`, `dom` | yes (`wasm32` only) |

pub mod auth;
pub mod base_css;
pub mod css;
pub mod host;
pub mod kinds;
pub mod layout_css;
pub mod methods;
pub mod net;
pub mod par;
pub mod tree;
pub mod value;
pub mod view;
pub mod weblink;

#[cfg(target_arch = "wasm32")]
pub mod api;
#[cfg(target_arch = "wasm32")]
mod dom;
