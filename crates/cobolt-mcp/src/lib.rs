// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The PowerRustCOBOL Model Context Protocol.**
//!
//! A wire-compatible MCP implementation: the JSON-RPC 2.0 envelope, the
//! initialize handshake, tool discovery and invocation, message framing, and a
//! server loop. It knows nothing about COBOL, egui, indexed files or the
//! filesystem — which is what lets a built application, the IDE and a test all
//! link the same protocol and none of them link each other.
//!
//! ```text
//!   an MCP client                    │         a PowerRustCOBOL application
//!   ─────────────                    │         ────────────────────────────
//!   initialize ─────────────────────▶│───────▶ serve() ──▶ McpHandler
//!            ◀──── capabilities ─────│                         │
//!   tools/list ─────────────────────▶│───────▶             the tool
//!   tools/call ─────────────────────▶│───────▶                 │
//!            ◀──── result ───────────│◀────────────────────────┘
//!                                    │
//!                   a byte stream the HOST owns
//! ```
//!
//! # Why the transport is not in here
//!
//! [`server::serve`] reads and writes a `BufRead`/`Write` pair it is handed.
//! stdio, a socket and an HTTP request body all satisfy that, so the choice
//! belongs to whoever embeds this — and none of those choices reaches into this
//! crate's dependency list. That matters more than tidiness: this workspace has
//! gone to real lengths to stay clear of rustls, aws-lc-rs and ring, because
//! they compile C and need cmake. A protocol crate that reached for a
//! TLS-enabled HTTP server would put all three into every application that
//! served a tool.
//!
//! # Why wire-compatible and not merely MCP-shaped
//!
//! The peer is not ours. An MCP client written by someone else must connect
//! without special-casing us, which means honouring the handshake and its
//! version negotiation exactly, and answering an unknown method with a
//! JSON-RPC error rather than a closed connection. A protocol that only talks
//! to its own client is discovered to be wrong by the first real one.
//!
//! Bodies this crate does not model stay [`serde_json::Value`], so an unknown
//! request survives a round trip instead of being dropped on the floor — the
//! same rule `cobolt-dap` follows, for the same reason.

pub mod server;
pub mod transport;
pub mod types;

pub use server::{serve, McpHandler};
pub use types::{
    Content, InitializeResult, Request, Response, RpcError, ServerInfo, Tool, ToolResult,
    PROTOCOL_VERSION,
};
