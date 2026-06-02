// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Cobolt standard library.
//!
//! Provides the I/O backend abstraction and supplementary functions that
//! sit on top of the core runtime.  The interpreter delegates file I/O,
//! console I/O, and system calls through the traits defined here.
//!
//! # Modules
//!
//! | Module          | Purpose                                               |
//! |-----------------|-------------------------------------------------------|
//! | [`io`]          | `IoBackend` trait + console and null implementations  |
//! | [`intrinsics`]  | Additional intrinsic COBOL functions                  |
//! | [`file`]        | Sequential / indexed file I/O (stub → Phase 5)        |

pub mod file;
pub mod intrinsics;
pub mod io;
