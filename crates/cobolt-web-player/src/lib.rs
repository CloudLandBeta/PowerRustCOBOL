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
//! | The ABI host (WASM-ABI-v1), in `cobolt-abi-host` since DESKTOP-WASM-001 T27 | [`host`], [`tree`], [`methods`] | no |
//! | Policy and front halves | [`hyperlink`], [`auth`], [`kinds`], [`net`], [`par`], [`value`], [`texts`] | no |
//! | The JavaScript boundary and the page (drawn by Leptos) | `api`, `dom` | yes (`wasm32` only) |

// The ABI host and what it reads live in `cobolt-abi-host` (DESKTOP-WASM-001 T27), shared with the
// desktop runner; each module keeps its path here.
pub use cobolt_abi_host::{auth, base_css, css, grid, host, hyperlink, kinds, layout_css, methods, metrics, motion, net, par, tree, upload, value, zoom};
pub mod calendar;
pub mod texts;
pub mod theme_css;
pub mod view;
#[cfg(not(target_arch = "wasm32"))]
pub mod fingerprint;

/// Every Rust type the player and its parts expose to JavaScript through `wasm-bindgen`, and how it
/// is disposed (WASM-017 R14, AC11). None is left to the JavaScript garbage collector alone;
/// `tests/exposed.rs` fails on a `#[wasm_bindgen]` type missing here and checks each disposal path.
/// WASM-018 adds its graphics handles to this list.
pub const EXPOSED: &[(&str, &str)] = &[
    ("Player", "the loader's unmount() calls player.free() after player.unmount()"),
    ("Graphics", "made by the player once per form instance that has a graphics object; freed when its instance closes and by unmount()"),
    ("GpuDevice", "opened once by the player when the first 3D form opens (the 3D part's open_gpu); freed by unmount()"),
    ("Picture3D", "made by the player for each Render of a Canvas3D; consumed by finish(), which the player awaits, so nothing outlives the copy"),
];

#[cfg(target_arch = "wasm32")]
pub mod api;
#[cfg(target_arch = "wasm32")]
mod auth_fetch;
#[cfg(target_arch = "wasm32")]
mod superlink;
#[cfg(target_arch = "wasm32")]
mod shell;
#[cfg(target_arch = "wasm32")]
mod dom;
