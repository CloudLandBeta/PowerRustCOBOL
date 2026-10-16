// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **AWS through MCP** (spec 078, amended by spec 103).
//!
//! The AWS controls reach AWS as an MCP client of AWS's own MCP servers, in
//! two ways, chosen per server by the route table:
//!
//! - **A child process, over stdio** — the five AWS Labs servers (Lambda,
//!   Bedrock Knowledge Bases, AgentCore, S3 Tables, Glue), started with `uvx`.
//!   They do their own TLS and SigV4 signing and read the standard AWS
//!   credential chain, so this path links no AWS SDK and signs nothing itself.
//! - **HTTPS, signed here** — the hosted AWS MCP Server, which needs nothing
//!   installed: [`https`] speaks MCP streamable HTTP and signs each request with
//!   SigV4 (the official AWS crates, built without their own HTTPS client; the
//!   product's `ureq` and the operating system's TLS carry the bytes).
//!
//! The one credential the runtime ever holds is a connection's keys that the
//! program set at run time (spec 092, amendment A6): kept in memory only, handed
//! to that connection's child process through its environment or lent to the
//! signer for one signature, and shown nowhere — see [`credentials`].
//!
//! - [`credentials`] — keys a program set for a connection, in memory only.
//! - [`process`] — starting a server: the built environment, stderr, masking.
//! - [`https`] — a hosted server over HTTPS: the signed link (spec 103).
//! - [`chain`] — the sign-in a profile names, read by the official AWS crates.
//! - [`pool`] — one shared server per connection and server, timeouts,
//!   restarts; a child process or an https link behind one interface.
//! - [`lifetime`] — making sure no server process outlives the application.
//! - [`routes`] — which server and tool serve each operation, as data.

pub mod b64;
pub mod chain;
pub mod connections;
pub mod credentials;
pub mod diagnose;
pub mod https;
pub mod lifetime;
pub mod ops;
pub mod pool;
pub mod process;
pub mod routes;
pub mod secrets;
