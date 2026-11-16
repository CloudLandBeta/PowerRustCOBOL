// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The ABI host** (DESKTOP-WASM-001 T27; R9; plan §4.3): what drives a program module through
//! ABI v1 (`specs/WASM-ABI-v1.md`) — the instance of each form, its control tree, the methods and
//! properties the module asks for, the menus, the links, the sign-in state and the backend
//! channel's frames — with the tables they read.
//!
//! These modules were the pure half of the player (WASM-016) and moved here unchanged, with their
//! tests, so that the browser and the desktop runner drive a module with one implementation. The
//! player re-exports each of them under its old path (`cobolt_web_player::host`, `::tree`, …).
//!
//! | Layer | Modules |
//! |---|---|
//! | The ABI host (WASM-ABI-v1) | [`host`], [`tree`], [`methods`], [`menus`] |
//! | Policy and front halves | [`hyperlink`], [`auth`], [`kinds`], [`net`], [`grid`], [`par`], [`value`], [`upload`] |
//! | The style a control is drawn with | [`css`], [`base_css`], [`layout_css`], [`zoom`], [`motion`], [`metrics`] |
//!
//! Nothing here touches a browser or a window.

pub mod auth;
pub mod base_css;
pub mod css;
pub mod grid;
pub mod host;
pub mod hyperlink;
pub mod kinds;
pub mod layout_css;
pub mod menus;
pub mod methods;
pub mod metrics;
pub mod motion;
pub mod net;
pub mod par;
pub mod tree;
pub mod upload;
pub mod value;
pub mod zoom;
