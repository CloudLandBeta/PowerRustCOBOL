// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **AWS through MCP** (spec 078).
//!
//! The AWS controls reach AWS as an MCP client of AWS's own MCP servers,
//! started as child processes and spoken to over stdio. The servers do their
//! own TLS and SigV4 signing and read the standard AWS credential chain, so
//! this module links no AWS SDK, no HTTP stack and no TLS, and signs nothing
//! itself. The one credential it ever holds is a connection's keys that the
//! program set at run time (spec 092, amendment A6): kept in memory only,
//! handed to that connection's server through its environment, and shown
//! nowhere — see [`credentials`].
//!
//! - [`credentials`] — keys a program set for a connection, in memory only.
//! - [`process`] — starting a server: the built environment, stderr, masking.
//! - [`pool`] — one shared server per connection and server, timeouts,
//!   restarts.
//! - [`lifetime`] — making sure no server outlives the application.
//! - [`routes`] — which server and tool serve each operation, as data.

pub mod b64;
pub mod connections;
pub mod credentials;
pub mod diagnose;
pub mod lifetime;
pub mod ops;
pub mod pool;
pub mod process;
pub mod routes;
pub mod secrets;
