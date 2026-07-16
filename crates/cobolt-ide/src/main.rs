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
pub mod agent_inspection;
mod app;
pub mod data_binding_guardian;
pub mod docs_embed;
pub mod file_dialog;
pub mod fonts;
pub mod form_runtime;
pub mod i18n;
pub mod inspector;
pub mod llm;
mod panels;
pub mod pdf_export;
mod project_model;
mod runner;
pub mod theme;
pub mod theme_ui;
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

    let ide_title = format!("PowerRustCOBOL {VERSION}");
    set_os_app_name(&ide_title);

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
        ..Default::default()
    };

    eframe::run_native(
        &ide_title,
        native_options,
        Box::new(|cc| Ok(Box::new(CoboltApp::new(cc)))),
    )
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
