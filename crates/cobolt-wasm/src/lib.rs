// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The WebAssembly generator (spec WASM-010).

pub mod collate;
pub mod error;
pub mod image;
pub mod ir;
pub mod layout;
pub mod link;
pub mod lower;
pub mod lower_call;
pub mod lower_form;
pub mod lower_more;
pub mod pic;
pub mod pool;
pub mod module;
pub mod rtsrc;
pub mod support;

#[cfg(feature = "host")]
pub mod host;

pub use module::{compile_program, Compiled, Options};
