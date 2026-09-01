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

pub mod debug_link;
pub mod diagnostics;
pub mod file_dialog;
pub mod host;
pub mod seeding;
pub mod shell;
/// The Snackbar's live stack (spec 055) — raise, expire, hover-pause, reflow and
/// overflow. It lives here rather than in `cobolt-forms` because a notification
/// has a LIFETIME, and the engine owns nothing that outlives a frame. Both live
/// surfaces consume this crate, so they cannot drift (R25).
pub mod snackbar_stack;
pub mod state;

pub use host::{
    fx_duration_ms, load_host_icon, run, ChildThemeSource, FormHost, FormHostConfig, FormSource,
    HostHooks, NoHooks, Surface,
};

/// The graphics APIs wgpu may choose from, for every window this crate opens.
///
/// eframe 0.36 renders through **wgpu** — its default feature set includes
/// `wgpu` and not `glow` — and wgpu's default backend set includes Vulkan and
/// reaches for it first on Windows.
///
/// A freshly installed Windows 11 has no vendor GPU driver and therefore no
/// working Vulkan ICD. The loader fails its registry lookup for layer manifests
/// and the process then dies at `0xc0000005` (STATUS_ACCESS_VIOLATION) **inside
/// driver code**, before eframe gets far enough to report anything: the window
/// never appears and nothing is printed, which is exactly what a user hit on a
/// clean Windows 11 machine that ran the same build fine before (2026-08-21).
///
/// An access violation is not a `Result`, so there is nothing to catch and fall
/// back from — a backend that crashes on creation has to not be *attempted*.
/// DX12 is the native Windows API and is present on Windows 11 even with only
/// Microsoft's inbox driver, with GL behind it as a last resort.
///
/// `with_env` is applied last, so `WGPU_BACKEND` still overrides everything:
/// anyone with a working Vulkan driver can ask for it back, and that same
/// variable is the one-line experiment that identifies a driver problem on a
/// machine we cannot reach.
///
/// `cfg!` rather than `#[cfg]` deliberately: both arms are type-checked on
/// every platform, so a mistake in the Windows expression cannot hide from a
/// macOS or Linux build the way it would behind an attribute. It folds to a
/// constant, so nothing is paid for at run time.
///
/// The IDE keeps its own copy of this rule (`cobolt-ide/src/main.rs`) because
/// it deliberately takes no runtime dependency on this crate — Run Form talks
/// to it through the `rcrun` child process. `wgpu_backend_rule_matches_the_ide`
/// in that crate pins the two equal through the dev-dependency, so the copies
/// cannot drift.
pub fn preferred_backends() -> eframe::wgpu::Backends {
    use eframe::wgpu::Backends;
    let preferred = if cfg!(target_os = "windows") {
        Backends::DX12 | Backends::GL
    } else {
        Backends::default()
    };
    preferred.with_env()
}

/// [`eframe::NativeOptions`] with this crate's backend rule applied.
///
/// Every window this crate opens goes through here, so a host that forgets is
/// a compile-time impossibility rather than a machine that silently dies at
/// startup.
pub fn native_options(viewport: eframe::egui::ViewportBuilder) -> eframe::NativeOptions {
    let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration::default();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut wgpu_options.wgpu_setup {
        setup.instance_descriptor.backends = preferred_backends();
    }
    eframe::NativeOptions {
        viewport,
        wgpu_options,
        ..Default::default()
    }
}

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
