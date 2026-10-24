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
//! | The JavaScript boundary and the page (drawn by Leptos) | `api`, `dom` | yes (`wasm32` only) |

pub mod auth;
pub mod base_css;
pub mod css;
pub mod grid;
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
#[cfg(not(target_arch = "wasm32"))]
pub mod fingerprint;

/// Every Rust type the player and its parts expose to JavaScript through `wasm-bindgen`, and how it
/// is disposed (WASM-017 R14, AC11). None is left to the JavaScript garbage collector alone;
/// `tests/exposed.rs` fails on a `#[wasm_bindgen]` type missing here and checks each disposal path.
/// WASM-018 adds its graphics handles to this list.
pub const EXPOSED: &[(&str, &str)] = &[
    ("Player", "the loader's unmount() calls player.free() after player.unmount()"),
    ("Surface2D", "made by the player for each Canvas2D control; freed when its instance closes and by unmount()"),
    ("Canvas", "made only by page code through partModule('canvas'), which frees it; the player never makes one"),
];

#[cfg(target_arch = "wasm32")]
pub mod api;
#[cfg(target_arch = "wasm32")]
mod superlink;
#[cfg(target_arch = "wasm32")]
mod dom;
