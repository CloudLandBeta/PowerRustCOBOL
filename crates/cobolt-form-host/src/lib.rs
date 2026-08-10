// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The **one** form host (spec 042).
//!
//! A designed form window — its control state, backdrop, window properties,
//! spec-038 entrance/exit effects, spec-037 lifecycle, event dispatch and
//! pacing — implemented once and consumed by both live surfaces:
//!
//! - **`rcrun run-form`** (`crates/cobolt-cli/src/form_gui.rs`) — Run Form's
//!   external process. Its glue parses args, runs parse/check diagnostics,
//!   spawns the interpreter thread *with the IDE debugger channel*, and
//!   discovers theme packs on disk.
//! - **The compiled application** (template in
//!   `crates/cobolt-compiler/src/lib.rs`) — every shipped binary. Its glue
//!   loads embedded forms/themes, spawns the interpreter *with compiled
//!   `EXEC RUST` blocks registered*, and replays `cobolt_windows` viewports
//!   through [`HostHooks::per_frame`].
//!
//! Everything else is shared — a bug fixed here is fixed in every host, which
//! is the point (three drifted copies is how the 1.60.33 caption bug shipped).
//!
//! # The seam (spec 042 R30)
//!
//! The **only** per-host behaviour, by design:
//!
//! | Where | What |
//! |-------|------|
//! | run-form glue | debugger channel + `@DBG` stdin protocol |
//! | compiled glue | `cobolt_windows` replay ([`HostHooks::per_frame`]), compiled-block registration, headless fallback |
//! | both glues | theme-pack *source* (disk discovery vs embedded art) and interpreter-thread spawn |

pub mod diagnostics;
pub mod file_dialog;
pub mod host;
pub mod seeding;
pub mod shell;
pub mod state;

pub use host::{
    fx_duration_ms, load_host_icon, run, FormHost, FormHostConfig, HostHooks, NoHooks, Surface,
};

/// Depth-first flatten of a designed control tree into draw order input —
/// every host renders from the flat list, z-sorted by the caller.
pub fn flatten_controls(
    controls: &[cobolt_forms::Control],
    out: &mut Vec<cobolt_forms::Control>,
) {
    for c in controls {
        out.push(c.clone());
        flatten_controls(&c.children, out);
    }
}
