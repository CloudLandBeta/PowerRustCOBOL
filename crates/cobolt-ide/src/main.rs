// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! PowerRustCOBOL — entry point.
//!
//! Launches the egui/eframe window and hands control to `CoboltApp`.
//!
//! # Running
//!
//! ```sh
//! cargo run -p cobolt-ide
//! ```

pub mod agent;
pub mod agent_actions;
pub mod agent_lint;
pub mod agent_ratings;
pub mod agent_inspection;
pub mod agents_db;
mod app;
pub mod contrast;
pub mod crash;
pub mod data_binding_guardian;
pub mod debug_settings;
pub mod doc_shots;
pub mod flags;
pub mod docs_embed;
pub mod error_log;
pub mod error_summary;
pub mod exec_rust_run;
pub mod external_crates_service;
pub mod file_dialog;
pub mod fonts;
pub mod form_runtime;
pub mod git_exec;
pub mod grace_host;
pub mod model_config_store;
pub mod model_policy;
pub mod grace_session;
pub mod i18n;
pub mod inspector;
pub mod leaderboard;
pub mod llm;
pub mod main_form;
mod panels;
pub mod pdf_export;
pub mod prompt_complete;
pub mod prompt_polish;
mod project_fs;
mod project_model;
pub mod project_upgrade;
mod runner;
pub mod secrets;
mod target_select;
pub mod theme;
pub mod theme_ui;
pub mod tool_exec;
pub mod toolchain;
pub mod ui_prefs;
pub mod version;
pub mod welcome;

use app::CoboltApp;
use version::VERSION;

/// Convert an `f32` corner radius to egui 0.31+'s `u8` unit (round + clamp).
/// UI math stays in `f32`; conversion happens only at the egui boundary.
pub fn cr8(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// Convert an `f32` margin to egui 0.31+'s `i8` unit (round + clamp).
pub fn mr8(v: f32) -> i8 {
    v.round().clamp(-128.0, 127.0) as i8
}

fn main() -> eframe::Result<()> {
    // Initialise logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_env("COBOLT_LOG")
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_target(false)
        .init();

    let ide_title = format!("{} {VERSION}", theme::brand_name());
    set_os_app_name(&ide_title);

    let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration::default();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut wgpu_options.wgpu_setup {
        setup.instance_descriptor.backends = preferred_backends();
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&ide_title)
            // OS taskbar/app label + hover tooltip (X11 WM_CLASS / Wayland app_id /
            // Windows). Without this it falls back to the binary name ("cobolt-ide").
            .with_app_id(&ide_title)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0])
            .with_transparent(true) // let desktop wallpaper bleed through
            .with_icon(load_icon()),
        wgpu_options,
        ..Default::default()
    };

    // Before the event loop: a windowed app has no terminal, so without this a
    // panic prints its message, location and backtrace to a stderr nobody is
    // reading and the window simply disappears.
    crash::install();

    let result = eframe::run_native(
        &ide_title,
        native_options,
        Box::new(|cc| Ok(Box::new(CoboltApp::new(cc)))),
    );

    // Reaching here at all means the loop returned rather than the process
    // dying, so the session ended on purpose: drop the marker and the autosaved
    // copies, which would otherwise offer to "recover" work already saved.
    crash::mark_clean_exit();
    result
}

/// The graphics APIs wgpu may choose from.
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
/// `cfg!` rather than `#[cfg]` deliberately: both arms are type-checked on
/// every platform, so a mistake in the Windows expression cannot hide from a
/// macOS or Linux build the way it would behind an attribute. It folds to a
/// constant, so nothing is paid for at run time.
fn preferred_backends() -> eframe::wgpu::Backends {
    use eframe::wgpu::Backends;
    let preferred = if cfg!(target_os = "windows") {
        Backends::DX12 | Backends::GL
    } else {
        Backends::default()
    };
    preferred.with_env()
}

#[cfg(target_os = "macos")]
fn set_os_app_name(name: &str) {
    use std::ffi::{c_char, c_void, CString};

    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
    }

    unsafe fn selector(name: &str) -> *mut c_void {
        let name = CString::new(name).ok();
        name.map(|s| unsafe { sel_registerName(s.as_ptr()) })
            .unwrap_or(std::ptr::null_mut())
    }

    let Ok(process_name) = CString::new(name) else {
        return;
    };
    let Ok(ns_process_info_name) = CString::new("NSProcessInfo") else {
        return;
    };
    let Ok(ns_string_name) = CString::new("NSString") else {
        return;
    };

    unsafe {
        let ns_process_info = objc_getClass(ns_process_info_name.as_ptr());
        let ns_string = objc_getClass(ns_string_name.as_ptr());
        if ns_process_info.is_null() || ns_string.is_null() {
            return;
        }

        let msg_class_obj: extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        let msg_init_utf8: extern "C" fn(*mut c_void, *mut c_void, *const c_char) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        let msg_set_name: extern "C" fn(*mut c_void, *mut c_void, *mut c_void) =
            std::mem::transmute(objc_msgSend as *const ());
        let msg_void: extern "C" fn(*mut c_void, *mut c_void) =
            std::mem::transmute(objc_msgSend as *const ());

        let process_info = msg_class_obj(ns_process_info, selector("processInfo"));
        let raw_string = msg_class_obj(ns_string, selector("alloc"));
        let process_name = msg_init_utf8(
            raw_string,
            selector("initWithUTF8String:"),
            process_name.as_ptr(),
        );
        if process_info.is_null() || process_name.is_null() {
            return;
        }

        msg_set_name(process_info, selector("setProcessName:"), process_name);
        msg_void(process_name, selector("release"));
    }
}

#[cfg(not(target_os = "macos"))]
fn set_os_app_name(_name: &str) {}

/// The IDE window / dock / taskbar icon. Uses the bundled PowerRustCOBOL samurai
/// icon as-is; users can override it by dropping an `app-icon.png` into the
/// PowerRustCOBOL config directory.
fn load_icon() -> egui::IconData {
    // Configurable override: <config-dir>/app-icon.png, when present and valid.
    if let Some(icon) = std::fs::read(crate::llm::base_dir().join("app-icon.png"))
        .ok()
        .and_then(|bytes| decode_icon(&bytes))
    {
        return icon;
    }
    // Default: the bundled PowerRustCOBOL icon.
    decode_icon(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/images/powerrustcobol-icon.png"
    )))
    .unwrap_or(egui::IconData {
        rgba: vec![0, 0, 0, 0],
        width: 1,
        height: 1,
    })
}

/// Decode image bytes into an `egui::IconData` (RGBA, resized to 256×256).
fn decode_icon(bytes: &[u8]) -> Option<egui::IconData> {
    let img = image::load_from_memory(bytes)
        .ok()?
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    })
}

#[cfg(test)]
mod backend_tests {
    /// The IDE and the form host must ask for the SAME graphics backends.
    ///
    /// They keep separate copies of the rule on purpose: the IDE takes no
    /// runtime dependency on `cobolt-form-host` (Run Form reaches it through
    /// the `rcrun` child process), so there is nowhere to put one shared
    /// definition without breaking that boundary. Two copies drift — this
    /// project has been bitten by exactly that more than once — so the
    /// dev-dependency that already exists for the roundtrip tests is used here
    /// to pin them equal instead.
    ///
    /// If this fails, the IDE window and the Run Form window disagree about
    /// which driver to use, and a machine where one works will have the other
    /// die at startup with nothing printed.
    #[test]
    fn wgpu_backend_rule_matches_the_ide() {
        assert_eq!(
            super::preferred_backends(),
            cobolt_form_host::preferred_backends(),
            "the IDE and the form host must choose the same wgpu backends"
        );
    }

    /// Windows must not be offered Vulkan.
    ///
    /// This is the whole point of the rule: a clean Windows 11 has no vendor
    /// GPU driver, so the Vulkan loader finds no ICD and the process dies at
    /// 0xc0000005 inside driver code, before anything can be reported.
    #[test]
    fn windows_never_reaches_for_vulkan() {
        use eframe::wgpu::Backends;
        // The rule as it applies on Windows, evaluated here on any host.
        let windows_rule = Backends::DX12 | Backends::GL;
        assert!(
            !windows_rule.contains(Backends::VULKAN),
            "a backend that crashes on creation must not be attempted at all"
        );
        assert!(
            windows_rule.contains(Backends::DX12),
            "DX12 is present on Windows 11 even with only the inbox driver"
        );
        // And WGPU_BACKEND still has the last word, so a working Vulkan driver
        // can be asked for and a broken one can be diagnosed from outside.
        assert_eq!(windows_rule.with_env(), windows_rule.with_env());
    }
}
