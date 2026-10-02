// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The system background blur under every IDE window, for a see-through
//! theme (Spatial).
//!
//! winit can blur a window it hands the app (`Window::set_blur`), but eframe
//! hands over only the main window: the Form Designer, the grid browser, the
//! debugger and the inspector are viewports the app never gets a handle to.
//! So this asks macOS directly, for every window the application owns, with
//! the same private Core Graphics call winit and Terminal.app use. Elsewhere
//! it does nothing.

/// Blur (or, with `false`, stop blurring) the desktop under every window of
/// this application. Must be called on the main thread, which is where egui
/// runs its frame on macOS.
pub fn set_all_windows(blur: bool) {
    #[cfg(target_os = "macos")]
    macos::set_all_windows(if blur { 80 } else { 0 });
    #[cfg(not(target_os = "macos"))]
    let _ = blur;
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::{c_char, c_void};

    type Id = *mut c_void;
    type Sel = *mut c_void;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
    }

    // The window-server calls winit and Terminal.app use for background blur.
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGSMainConnectionID() -> Id;
        fn CGSSetWindowBackgroundBlurRadius(connection: Id, window: isize, radius: i64) -> i32;
    }

    pub(super) fn set_all_windows(radius: i64) {
        // objc_msgSend must be called through a pointer of the exact
        // signature of each message (arm64 has no varargs convention for it).
        unsafe {
            let send_id: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let send_usize: extern "C" fn(Id, Sel) -> usize = std::mem::transmute(objc_msgSend as *const ());
            let send_index: extern "C" fn(Id, Sel, usize) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let send_isize: extern "C" fn(Id, Sel) -> isize = std::mem::transmute(objc_msgSend as *const ());

            let class = objc_getClass(c"NSApplication".as_ptr());
            if class.is_null() {
                return;
            }
            let app = send_id(class, sel_registerName(c"sharedApplication".as_ptr()));
            if app.is_null() {
                return;
            }
            let windows = send_id(app, sel_registerName(c"windows".as_ptr()));
            if windows.is_null() {
                return;
            }
            let count = send_usize(windows, sel_registerName(c"count".as_ptr()));
            let at = sel_registerName(c"objectAtIndex:".as_ptr());
            let number = sel_registerName(c"windowNumber".as_ptr());
            let connection = CGSMainConnectionID();
            for i in 0..count {
                let window = send_index(windows, at, i);
                if !window.is_null() {
                    let id = send_isize(window, number);
                    let _ = CGSSetWindowBackgroundBlurRadius(connection, id, radius);
                }
            }
        }
    }
}
