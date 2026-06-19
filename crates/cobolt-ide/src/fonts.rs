// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! System font enumeration + on-demand loading into egui.
//!
//! Moved to `cobolt-forms` (007 T1) so the shared control renderer in
//! `cobolt_forms::paint` can resolve fonts identically in the designer, the
//! preview/run form and compiled binaries. Re-exported here so existing
//! `crate::fonts::…` call sites in the IDE keep working unchanged.

pub use cobolt_forms::fonts::*;
