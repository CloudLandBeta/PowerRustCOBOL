// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `cobolt-server` — the backend and the client that calls it
//! (WASM-003, WASM-014, WASM-009).
//!
//! **The server** (`feature = "server"`) is headless: it listens on a
//! WebSocket over TLS, checks a bearer token on every call, and runs the
//! programs the project *exports* — and only those — on the native runtime,
//! each call in its own interpreter. Nothing in it knows egui or eframe.
//!
//! **The loader** (`feature = "loader"`) reads a backend project and packs it;
//! the compiler uses it alone to build the headless binary.
//!
//! **The client** (`feature = "client"`) implements
//! `cobolt_wire::call::Transport` for a desktop or native host, so that
//! `CALL … WITH WEBSOCKET LINKAGE` works from the interpreter; the browser has
//! its own client in the player.

#[cfg(feature = "loader")]
pub mod backend;
#[cfg(feature = "loader")]
pub mod bundle;
#[cfg(feature = "server")]
pub mod config;
#[cfg(feature = "server")]
pub mod dev;
#[cfg(feature = "server")]
pub mod exec;
#[cfg(feature = "server")]
pub mod http;
#[cfg(feature = "server")]
pub mod jwks;
#[cfg(feature = "server")]
pub mod jwt;
#[cfg(feature = "server")]
pub mod limits;
#[cfg(feature = "server")]
pub mod log;
#[cfg(feature = "server")]
pub mod serve;
#[cfg(feature = "server")]
pub mod standalone;
#[cfg(feature = "net")]
pub mod tlsutil;

#[cfg(feature = "server")]
pub use serve::{start, Options, ServerHandle};
