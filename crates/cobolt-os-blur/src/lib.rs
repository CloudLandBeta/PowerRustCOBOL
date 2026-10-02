// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Blur the desktop behind every window of this application (spec 083).
//!
//! A see-through theme — the IDE's Spatial, and the Spatial form theme an
//! application can choose — wants the desktop behind its transparent windows
//! blurred, the way a floating glass panel softens the room behind it.
//!
//! winit can blur the one window it hands an app (`Window::set_blur`), but
//! eframe hands over only the main window: designer windows, child forms and
//! dialogs are viewports nobody holds a handle to. So this asks the operating
//! system directly, for every window the application owns.
//!
//! - **macOS:** the window server's background blur, through the same private
//!   Core Graphics call winit and Terminal.app use.
//! - **Windows:** the DWM system backdrop (acrylic) on Windows 11, and the
//!   older blur-behind on Windows 10, for every top-level window on the
//!   calling thread.
//! - **Linux and elsewhere:** nothing here. The compositor decides; the host
//!   asks winit for the main window, which KDE on Wayland honours.
//!
//! Call it on the thread that owns the windows — the main thread, where egui
//! runs its frame. It is cheap and idempotent, so a host may call it every
//! second to catch windows opened since the last call.

/// Blur (`true`) or stop blurring (`false`) the desktop behind every window of
/// this application. Does nothing where the platform offers no blur.
pub fn set_all_windows(blur: bool) {
    #[cfg(target_os = "macos")]
    macos::set_all_windows(if blur { 80 } else { 0 });
    #[cfg(target_os = "windows")]
    windows::set_all_windows(blur);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = blur;
}

/// Whether this platform blurs from [`set_all_windows`]. On the others a host
/// should still ask winit for its main window (Linux/KDE on Wayland).
pub const fn supported() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
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
        // objc_msgSend must be called through a pointer of each message's
        // exact signature (arm64 has no varargs convention for it).
        unsafe {
            let send_id: extern "C" fn(Id, Sel) -> Id =
                std::mem::transmute(objc_msgSend as *const ());
            let send_usize: extern "C" fn(Id, Sel) -> usize =
                std::mem::transmute(objc_msgSend as *const ());
            let send_index: extern "C" fn(Id, Sel, usize) -> Id =
                std::mem::transmute(objc_msgSend as *const ());
            let send_isize: extern "C" fn(Id, Sel) -> isize =
                std::mem::transmute(objc_msgSend as *const ());

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

#[cfg(target_os = "windows")]
mod windows {
    use std::ffi::c_void;

    type Hwnd = *mut c_void;
    type Bool = i32;

    #[repr(C)]
    struct Margins {
        left: i32,
        right: i32,
        top: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct BlurBehind {
        flags: u32,
        enable: Bool,
        region: *mut c_void,
        transition_on_maximized: Bool,
    }

    /// `DWMWA_SYSTEMBACKDROP_TYPE` (Windows 11 22H2 and later).
    const SYSTEM_BACKDROP_TYPE: u32 = 38;
    /// `DWMSBT_NONE` / `DWMSBT_TRANSIENTWINDOW` (acrylic).
    const BACKDROP_NONE: u32 = 1;
    const BACKDROP_ACRYLIC: u32 = 3;
    /// `DWM_BB_ENABLE`.
    const BB_ENABLE: u32 = 1;

    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmSetWindowAttribute(hwnd: Hwnd, attribute: u32, value: *const c_void, size: u32) -> i32;
        fn DwmExtendFrameIntoClientArea(hwnd: Hwnd, margins: *const Margins) -> i32;
        fn DwmEnableBlurBehindWindow(hwnd: Hwnd, blur: *const BlurBehind) -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumThreadWindows(
            thread: u32,
            callback: unsafe extern "system" fn(Hwnd, isize) -> Bool,
            param: isize,
        ) -> Bool;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }

    unsafe extern "system" fn apply(hwnd: Hwnd, blur: isize) -> Bool {
        let on = blur != 0;
        unsafe {
            // Windows 11: the acrylic system backdrop, shown through the
            // transparent client area once the frame is extended over it.
            let kind = if on { BACKDROP_ACRYLIC } else { BACKDROP_NONE };
            let _ = DwmSetWindowAttribute(
                hwnd,
                SYSTEM_BACKDROP_TYPE,
                &kind as *const u32 as *const c_void,
                std::mem::size_of::<u32>() as u32,
            );
            let extend = if on { -1 } else { 0 };
            let margins = Margins { left: extend, right: extend, top: extend, bottom: extend };
            let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
            // Windows 10: the older blur-behind. Harmless where the backdrop
            // above already applies.
            let behind = BlurBehind {
                flags: BB_ENABLE,
                enable: on as Bool,
                region: std::ptr::null_mut(),
                transition_on_maximized: 0,
            };
            let _ = DwmEnableBlurBehindWindow(hwnd, &behind);
        }
        1
    }

    pub(super) fn set_all_windows(blur: bool) {
        unsafe {
            let _ = EnumThreadWindows(GetCurrentThreadId(), apply, blur as isize);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn support_matches_the_platform() {
        assert_eq!(super::supported(), cfg!(any(target_os = "macos", target_os = "windows")));
    }
}
