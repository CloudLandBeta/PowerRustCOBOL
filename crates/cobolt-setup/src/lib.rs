// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The Windows setup's own code (spec 112).
//!
//! The `.msi` carries this crate's DLL in its Binary table and names its entry
//! points as custom actions. Everything that talks to Windows Installer is
//! `#[cfg(windows)]`; what to run ([`data`]), how its output becomes lines
//! ([`lines`]), the progress bar ([`ticks`]), the watchdog ([`watch`]) and the
//! rollback rule ([`rollback`]) are plain Rust, built and tested on every
//! operating system.

pub mod buildtools;
pub mod data;
pub mod disk;
pub mod facts;
pub mod figures;
pub mod lines;
pub mod prepare;
pub mod progress;
pub mod result;
pub mod rollback;
pub mod ticks;
pub mod watch;

/// Guards on the installer's source (`wix/product.wxs` and the release workflow): tests only.
#[cfg(test)]
mod installer;

#[cfg(windows)]
mod entries;
#[cfg(windows)]
pub mod ident;
#[cfg(windows)]
mod supervisor;
#[cfg(windows)]
pub use entries::{PrcReadResult, PrcRollback, PrcTicks};
#[cfg(windows)]
pub use facts::system::PrcFacts;
#[cfg(windows)]
pub use supervisor::PrcRun;
