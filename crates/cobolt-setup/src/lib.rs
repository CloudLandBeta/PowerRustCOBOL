// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The Windows setup's own code (spec 112).
//!
//! The `.msi` carries this crate's DLL in its Binary table and names its entry
//! points as custom actions. Everything that talks to Windows Installer is
//! `#[cfg(windows)]`; what to run ([`data`]) and how its output becomes lines
//! ([`lines`]) are plain Rust, built and tested on every operating system.

pub mod data;
pub mod lines;

#[cfg(windows)]
pub mod ident;
#[cfg(windows)]
mod supervisor;
#[cfg(windows)]
pub use supervisor::PrcRun;
