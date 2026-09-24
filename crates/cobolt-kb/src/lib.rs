// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The application Knowledge Base** (spec 068).
//!
//! The Knowledge Base a *built application* owns: folders of its users'
//! documents — *collections* — each with a searchable index derived from them.
//! It is not the IDE's System or Project Knowledge Base, shares no file, table
//! or path with them, and depends on neither the IDE nor `cobolt-agents`: an
//! application links this crate and nothing of the tooling (spec 068 R1–R4).
//!
//! It knows nothing about COBOL, forms or HTTP. A host supplies the network
//! through [`embed::Transport`], which is what keeps TLS out of this crate.

pub mod chunk;
pub mod embed;
pub mod store;
pub mod convert;
pub mod refresh;
pub mod search;
pub mod model;
#[cfg(feature = "semantic")]
pub mod semantic;
